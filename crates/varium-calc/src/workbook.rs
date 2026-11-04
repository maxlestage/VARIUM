//! Classeur, feuilles et cellules ; opérations d'édition.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use crate::bank::CategoryRule;
use crate::cond::{CondFormat, CondFormatter, CondPaint};
use crate::convert::RatesTable;
use crate::eval::{Computed, is_formula, recalc, sheet_resolver};
use crate::filter::AutoFilter;
use crate::format::{CellFormat, HAlign, NumberFormat, format_value, parse_literal, today_serial};
use crate::parser::check_formula;
use crate::pivot::PivotTable;
use crate::refs::{CellRef, MAX_COLS, MAX_ROWS, Range, quote_sheet_name};
use crate::rewrite::{
    Structural, adjust_formula, normalize_formula, ref_text, rewrite_refs, shift_formula,
};
use crate::value::Value;

/// Largeur de colonne par défaut, en points d'écran (pixels logiques).
pub const DEFAULT_COL_WIDTH: f32 = 100.0;
/// Hauteur de ligne par défaut, en points d'écran.
pub const DEFAULT_ROW_HEIGHT: f32 = 24.0;
/// Longueur maximale d'un nom de feuille (limite d'Excel).
pub const MAX_SHEET_NAME: usize = 31;

/// Contenu d'une cellule : la saisie telle que tapée et sa mise en forme.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cell {
    pub input: String,
    #[serde(skip_serializing_if = "CellFormat::is_default")]
    pub format: CellFormat,
}

/// Une feuille de calcul.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sheet {
    pub name: String,
    #[serde(with = "cells_serde")]
    pub(crate) cells: BTreeMap<CellRef, Cell>,
    /// Largeurs de colonnes modifiées, en points d'écran (index de colonne → largeur).
    pub col_widths: BTreeMap<u32, f32>,
    /// Hauteurs de lignes modifiées, en points d'écran.
    pub row_heights: BTreeMap<u32, f32>,
    /// Volets figés : nombre de lignes du haut toujours visibles.
    #[serde(skip_serializing_if = "is_zero")]
    pub frozen_rows: u32,
    /// Volets figés : nombre de colonnes de gauche toujours visibles.
    #[serde(skip_serializing_if = "is_zero")]
    pub frozen_cols: u32,
    /// Règles de mise en forme conditionnelle, dans l'ordre d'application
    /// (voir [`crate::cond`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cond_formats: Vec<CondFormat>,
    /// Filtre automatique (boutons dans la ligne d'en-tête ; voir [`AutoFilter`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_filter: Option<AutoFilter>,
    /// Tableaux croisés dynamiques dont le résultat est dans cette feuille.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pivots: Vec<PivotTable>,
    /// Graphiques enregistrés avec la feuille.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub charts: Vec<SheetChart>,
}

/// Un graphique enregistré avec une feuille : type (`barres`, `barres-h`,
/// `empilees`, `courbes`, `aires`, `nuage`, `secteurs`, `anneau`, `gantt`),
/// plage des données et titre.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SheetChart {
    pub kind: String,
    #[serde(with = "crate::cond::range_serde")]
    pub range: Range,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
}

impl Default for SheetChart {
    fn default() -> Self {
        Self {
            kind: "barres".into(),
            range: Range::cell(CellRef::default()),
            title: String::new(),
        }
    }
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

mod cells_serde {
    use super::{BTreeMap, Cell, CellRef};
    use serde::de::Error;
    use serde::ser::SerializeMap;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        cells: &BTreeMap<CellRef, Cell>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(cells.len()))?;
        for (k, v) in cells {
            map.serialize_entry(&k.name(), v)?;
        }
        map.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<BTreeMap<CellRef, Cell>, D::Error> {
        let raw: BTreeMap<String, Cell> = BTreeMap::deserialize(d)?;
        raw.into_iter()
            .map(|(k, v)| {
                CellRef::parse(&k)
                    .map(|c| (c, v))
                    .ok_or_else(|| D::Error::custom(format!("référence de cellule invalide : {k}")))
            })
            .collect()
    }
}

impl Sheet {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Cellule à cette position, si elle a un contenu ou une mise en forme.
    pub fn cell(&self, at: CellRef) -> Option<&Cell> {
        self.cells.get(&at)
    }

    /// Toutes les cellules (contenu ou mise en forme), dans l'ordre de lecture.
    pub fn cells(&self) -> impl Iterator<Item = (CellRef, &Cell)> {
        self.cells.iter().map(|(k, v)| (*k, v))
    }

    /// Nombre de lignes et de colonnes occupées par des cellules non vides.
    pub fn used_size(&self) -> (u32, u32) {
        let mut rows = 0;
        let mut cols = 0;
        for (c, cell) in &self.cells {
            if !cell.input.is_empty() {
                rows = rows.max(c.row + 1);
                cols = cols.max(c.col + 1);
            }
        }
        (rows, cols)
    }

    /// Largeur d'une colonne (valeur par défaut si elle n'a pas été changée).
    pub fn col_width(&self, col: u32) -> f32 {
        self.col_widths
            .get(&col)
            .copied()
            .unwrap_or(DEFAULT_COL_WIDTH)
    }

    /// Hauteur d'une ligne (valeur par défaut si elle n'a pas été changée).
    pub fn row_height(&self, row: u32) -> f32 {
        self.row_heights
            .get(&row)
            .copied()
            .unwrap_or(DEFAULT_ROW_HEIGHT)
    }

    /// Largeurs valides pour les exports (index dans la feuille, valeur finie).
    pub(crate) fn clean_col_widths(&self) -> BTreeMap<u32, f32> {
        clean_sizes(&self.col_widths, MAX_COLS)
    }

    /// Hauteurs valides pour les exports.
    pub(crate) fn clean_row_heights(&self) -> BTreeMap<u32, f32> {
        clean_sizes(&self.row_heights, MAX_ROWS)
    }

    pub(crate) fn put(&mut self, at: CellRef, input: String, format: CellFormat) {
        if input.is_empty() && format.is_default() {
            self.cells.remove(&at);
        } else {
            self.cells.insert(at, Cell { input, format });
        }
    }
}

fn clean_sizes(sizes: &BTreeMap<u32, f32>, limit: u32) -> BTreeMap<u32, f32> {
    sizes
        .iter()
        .filter(|(k, v)| **k < limit && v.is_finite() && **v > 0.0)
        .map(|(k, v)| (*k, v.clamp(1.0, 4000.0)))
        .collect()
}

/// Cache des valeurs calculées (non sérialisé). Clonable : un clone (pour
/// l'annulation, par exemple) partage les valeurs déjà calculées.
#[derive(Default)]
struct Cache(Mutex<Option<Arc<Computed>>>);

impl Cache {
    fn lock(&self) -> MutexGuard<'_, Option<Arc<Computed>>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Clone for Cache {
    fn clone(&self) -> Self {
        Cache(Mutex::new(self.lock().clone()))
    }
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ready = self.lock().is_some();
        write!(f, "Cache {{ calculé: {ready} }}")
    }
}

/// Morceau de feuille copié (presse-papiers interne).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    /// Coin d'origine de la copie ; `None` pour du texte venu d'ailleurs (les
    /// formules ne sont alors pas décalées au collage).
    pub origin: Option<CellRef>,
    pub rows: u32,
    pub cols: u32,
    /// Cellules non vides, positions relatives au coin haut gauche.
    pub cells: Vec<ClipCell>,
}

/// Une cellule copiée.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ClipCell {
    pub row: u32,
    pub col: u32,
    pub input: String,
    pub format: CellFormat,
    /// Valeur affichée au moment de la copie (pour le presse-papiers du système).
    pub display: String,
    /// Valeur calculée au moment de la copie (« Collage spécial » des valeurs).
    #[serde(default, skip_serializing_if = "Value::is_empty")]
    pub value: Value,
}

impl Clip {
    pub fn is_empty(&self) -> bool {
        self.rows == 0 || self.cols == 0
    }

    /// Valeurs affichées, séparées par des tabulations (presse-papiers du système).
    pub fn to_text(&self) -> String {
        let mut grid = vec![vec![String::new(); self.cols as usize]; self.rows as usize];
        for c in &self.cells {
            if let Some(slot) = grid
                .get_mut(c.row as usize)
                .and_then(|r| r.get_mut(c.col as usize))
            {
                slot.clone_from(&c.display);
            }
        }
        grid.iter()
            .map(|r| {
                r.iter()
                    .map(|s| s.replace(['\t', '\n', '\r'], " "))
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Texte collé depuis une autre application : lignes, et cellules séparées
    /// par des tabulations. Chaque cellule est saisie telle quelle.
    pub fn from_text(text: &str) -> Clip {
        let text = text.strip_suffix('\n').unwrap_or(text);
        let text = text.strip_suffix('\r').unwrap_or(text);
        let mut clip = Clip::default();
        for (r, line) in text.split('\n').enumerate().take(MAX_ROWS as usize) {
            let line = line.strip_suffix('\r').unwrap_or(line);
            clip.rows = r as u32 + 1;
            for (c, field) in line.split('\t').enumerate().take(MAX_COLS as usize) {
                clip.cols = clip.cols.max(c as u32 + 1);
                if !field.is_empty() {
                    clip.cells.push(ClipCell {
                        row: r as u32,
                        col: c as u32,
                        input: field.to_string(),
                        format: CellFormat::default(),
                        display: field.to_string(),
                        value: Value::Empty,
                    });
                }
            }
        }
        clip
    }
}

/// Un classeur : plusieurs feuilles, et les valeurs calculées en cache.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Workbook {
    pub title: String,
    pub sheets: Vec<Sheet>,
    /// Règles de classement des opérations bancaires (mot-clé → catégorie) ;
    /// vide : les règles par défaut.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bank_rules: Vec<CategoryRule>,
    /// Taux de change utilisés par la fonction `DEVISE`.
    #[serde(skip_serializing_if = "RatesTable::is_empty")]
    pub rates: RatesTable,
    /// Date du jour imposée (numéro de série), pour des tests reproductibles.
    #[serde(skip)]
    today: Option<f64>,
    #[serde(skip)]
    cache: Cache,
}

impl Default for Workbook {
    fn default() -> Self {
        Self::new()
    }
}

/// Clé de tri à la française : sans casse ni accents.
pub(crate) fn collation_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => out.push('a'),
            'é' | 'è' | 'ê' | 'ë' => out.push('e'),
            'î' | 'ï' | 'í' | 'ì' => out.push('i'),
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => out.push('o'),
            'ù' | 'û' | 'ü' | 'ú' => out.push('u'),
            'ç' => out.push('c'),
            'ÿ' => out.push('y'),
            'ñ' => out.push('n'),
            'œ' => out.push_str("oe"),
            'æ' => out.push_str("ae"),
            'ß' => out.push_str("ss"),
            c => out.push(c),
        }
    }
    out
}

/// Ordre de tri croissant : nombres, textes, valeurs logiques, erreurs, vides.
pub(crate) fn sort_order(a: &Value, b: &Value) -> Ordering {
    let rank = |v: &Value| match v {
        Value::Number(_) => 0,
        Value::Text(_) => 1,
        Value::Bool(_) => 2,
        Value::Error(_) => 3,
        Value::Empty => 4,
    };
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (Value::Text(x), Value::Text(y)) => collation_key(x)
            .cmp(&collation_key(y))
            .then_with(|| x.cmp(y)),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        _ => rank(a).cmp(&rank(b)),
    }
}

/// Nettoie un nom de feuille : caractères interdits par Excel retirés, 31
/// caractères au plus.
pub(crate) fn clean_sheet_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\') && !c.is_control())
        .take(MAX_SHEET_NAME)
        .collect();
    cleaned.trim().trim_matches('\'').trim().to_string()
}

/// Saisie de formule qui donne une date (`=AUJOURDHUI()`, `=DATE(…)`).
fn returns_date(input: &str) -> bool {
    let head: String = input
        .trim_start_matches('=')
        .trim_start()
        .chars()
        .take_while(|c| *c != '(')
        .collect();
    matches!(
        crate::functions::lookup(head.trim()),
        Some(crate::functions::F::Aujourdhui | crate::functions::F::Date)
    )
}

impl Workbook {
    /// Classeur vide avec une feuille « Feuille 1 ».
    pub fn new() -> Self {
        Self {
            title: "Sans titre".into(),
            sheets: vec![Sheet::new("Feuille 1")],
            bank_rules: Vec::new(),
            rates: RatesTable::default(),
            today: None,
            cache: Cache::default(),
        }
    }

    /// Classeur d'exemple (budget et facture), voir [`crate::sample`].
    pub fn sample() -> Self {
        crate::sample::sample()
    }

    // ------------------------------------------------------------ Cache

    /// Oublie les valeurs calculées (le prochain accès recalcule tout). Utile
    /// après une modification directe de `sheets`.
    pub fn invalidate(&self) {
        *self.cache.lock() = None;
    }

    pub(crate) fn computed(&self) -> Arc<Computed> {
        let mut guard = self.cache.lock();
        if let Some(c) = guard.as_ref()
            && c.values.len() == self.sheets.len()
        {
            return c.clone();
        }
        let today = self.today.unwrap_or_else(today_serial);
        let c = Arc::new(recalc(&self.sheets, today, &self.rates));
        *guard = Some(c.clone());
        c
    }

    /// Date du jour utilisée pour le calcul (numéro de série).
    pub(crate) fn today(&self) -> f64 {
        self.today.unwrap_or_else(today_serial)
    }

    /// Recalcule tout de suite (sinon, le calcul a lieu au premier accès à une valeur).
    pub fn recalculate(&self) {
        self.invalidate();
        let _ = self.computed();
    }

    /// Impose la date du jour (numéro de série) utilisée par `AUJOURDHUI()` ;
    /// `None` reprend l'horloge du système.
    pub fn set_today(&mut self, serial: Option<f64>) {
        self.today = serial;
        self.invalidate();
    }

    // ------------------------------------------------------------ Feuilles

    /// Index d'une feuille d'après son nom (casse ignorée).
    pub fn sheet_index(&self, name: &str) -> Option<usize> {
        let lower = name.to_lowercase();
        self.sheets
            .iter()
            .position(|s| s.name.to_lowercase() == lower)
    }

    fn name_taken(&self, name: &str, except: Option<usize>) -> bool {
        let lower = name.to_lowercase();
        self.sheets
            .iter()
            .enumerate()
            .any(|(i, s)| Some(i) != except && s.name.to_lowercase() == lower)
    }

    /// Ajoute une feuille « Feuille N » à la fin ; renvoie son index.
    pub fn add_sheet(&mut self) -> usize {
        let mut n = self.sheets.len() + 1;
        while self.name_taken(&format!("Feuille {n}"), None) {
            n += 1;
        }
        self.sheets.push(Sheet::new(format!("Feuille {n}")));
        self.invalidate();
        self.sheets.len() - 1
    }

    /// Supprime une feuille (jamais la dernière). Les références vers elle
    /// deviennent `#REF!`.
    pub fn remove_sheet(&mut self, i: usize) {
        if self.sheets.len() <= 1 || i >= self.sheets.len() {
            return;
        }
        let removed = self.sheets.remove(i).name.to_lowercase();
        self.rewrite_all_formulas(|_, input| {
            rewrite_refs(input, |r, _| {
                r.sheet
                    .as_ref()
                    .filter(|p| p.name.to_lowercase() == removed)
                    .map(|_| "#REF!".to_string())
            })
        });
        self.invalidate();
    }

    /// Renomme une feuille et met à jour les formules qui la citent. Les
    /// caractères interdits (`[ ] : * ? / \`) sont retirés ; renvoie `false` si
    /// le nom est vide ou déjà pris.
    pub fn rename_sheet(&mut self, i: usize, name: &str) -> bool {
        let name = clean_sheet_name(name);
        if i >= self.sheets.len() || name.is_empty() || self.name_taken(&name, Some(i)) {
            return false;
        }
        let old = self.sheets[i].name.to_lowercase();
        let prefix = format!("{}!", quote_sheet_name(&name));
        self.rewrite_all_formulas(|_, input| {
            rewrite_refs(input, |r, _| {
                r.sheet.as_ref().filter(|p| p.name.to_lowercase() == old)?;
                Some(ref_text(&prefix, r.start, r.end))
            })
        });
        self.sheets[i].name = name;
        self.invalidate();
        true
    }

    /// Déplace une feuille (ordre des onglets).
    pub fn move_sheet(&mut self, from: usize, to: usize) {
        if from >= self.sheets.len() || to >= self.sheets.len() || from == to {
            return;
        }
        let s = self.sheets.remove(from);
        self.sheets.insert(to, s);
        self.invalidate();
    }

    /// Applique `f(feuille, saisie)` à toutes les formules du classeur.
    fn rewrite_all_formulas(&mut self, mut f: impl FnMut(usize, &str) -> String) {
        for (s, sheet) in self.sheets.iter_mut().enumerate() {
            for cell in sheet.cells.values_mut() {
                if is_formula(&cell.input) {
                    let new = f(s, &cell.input);
                    if new != cell.input {
                        cell.input = new;
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------ Cellules

    /// Saisie d'une cellule (`""` si elle est vide).
    pub fn input(&self, sheet: usize, at: CellRef) -> &str {
        self.sheets
            .get(sheet)
            .and_then(|s| s.cells.get(&at))
            .map(|c| c.input.as_str())
            .unwrap_or("")
    }

    /// Change la saisie d'une cellule. `""` efface le contenu (la mise en forme
    /// reste). Une saisie comme `12,5 %`, `15/03/2026` ou `12 €` donne aussi
    /// son format à la cellule si elle n'en avait pas.
    pub fn set_input(&mut self, sheet: usize, at: CellRef, input: &str) {
        if at.row >= MAX_ROWS || at.col >= MAX_COLS {
            return;
        }
        let Some(sh) = self.sheets.get_mut(sheet) else {
            return;
        };
        let mut format = sh.cells.get(&at).map(|c| c.format).unwrap_or_default();
        let input = if is_formula(input) {
            normalize_formula(input)
        } else {
            input.to_string()
        };
        if format.number == NumberFormat::General && !input.is_empty() {
            if is_formula(&input) {
                if returns_date(&input) {
                    format.number = NumberFormat::Date;
                }
            } else if let (_, Some(suggested)) = parse_literal(&input) {
                format.number = suggested;
            }
        }
        sh.put(at, input, format);
        self.invalidate();
    }

    /// Mise en forme d'une cellule.
    pub fn format(&self, sheet: usize, at: CellRef) -> CellFormat {
        self.sheets
            .get(sheet)
            .and_then(|s| s.cells.get(&at))
            .map(|c| c.format)
            .unwrap_or_default()
    }

    /// Change la mise en forme d'une cellule.
    pub fn set_format(&mut self, sheet: usize, at: CellRef, f: CellFormat) {
        if at.row >= MAX_ROWS || at.col >= MAX_COLS {
            return;
        }
        let Some(sh) = self.sheets.get_mut(sheet) else {
            return;
        };
        let (input, old) = sh
            .cells
            .get(&at)
            .map(|c| (c.input.clone(), c.format))
            .unwrap_or_default();
        let text_changed = (old.number == NumberFormat::Text) != (f.number == NumberFormat::Text);
        sh.put(at, input, f);
        // Seul le format Texte change la valeur d'une saisie.
        if text_changed {
            self.invalidate();
        }
    }

    /// Valeur calculée d'une cellule (mise en cache jusqu'à la prochaine modification).
    pub fn value(&self, sheet: usize, at: CellRef) -> Value {
        self.computed()
            .values
            .get(sheet)
            .and_then(|m| m.get(&at))
            .cloned()
            .unwrap_or_default()
    }

    /// Texte affiché dans la cellule, selon son format, à la française.
    pub fn display(&self, sheet: usize, at: CellRef) -> String {
        let fmt = self.format(sheet, at).number;
        let computed = self.computed();
        match computed.values.get(sheet).and_then(|m| m.get(&at)) {
            Some(v) => format_value(v, fmt),
            None => String::new(),
        }
    }

    /// Alignement effectif : celui choisi, sinon nombres à droite, valeurs
    /// logiques et erreurs centrées, texte à gauche.
    pub fn effective_align(&self, sheet: usize, at: CellRef) -> HAlign {
        if let Some(a) = self.format(sheet, at).align {
            return a;
        }
        match self.value(sheet, at) {
            Value::Number(_) => HAlign::Right,
            Value::Bool(_) | Value::Error(_) => HAlign::Center,
            _ => HAlign::Left,
        }
    }

    /// Explication d'une erreur : syntaxe de la formule ou erreur de calcul.
    pub fn error_message(&self, sheet: usize, at: CellRef) -> Option<String> {
        if let Err(msg) = check_formula(self.input(sheet, at)) {
            return Some(msg);
        }
        match self.value(sheet, at) {
            Value::Error(e) => Some(format!("{} : {}", e.code(), e.description())),
            _ => None,
        }
    }

    /// Nombre de lignes et de colonnes occupées par des cellules non vides.
    pub fn used_size(&self, sheet: usize) -> (u32, u32) {
        self.sheets
            .get(sheet)
            .map(Sheet::used_size)
            .unwrap_or((0, 0))
    }

    /// Efface le contenu d'une plage (la mise en forme reste).
    pub fn clear_range(&mut self, sheet: usize, r: Range) {
        let Some(sh) = self.sheets.get_mut(sheet) else {
            return;
        };
        let keys: Vec<CellRef> = sh
            .cells
            .range(r.start..=r.end)
            .map(|(k, _)| *k)
            .filter(|k| r.contains(*k))
            .collect();
        for k in keys {
            let f = sh.cells.get(&k).map(|c| c.format).unwrap_or_default();
            sh.put(k, String::new(), f);
        }
        self.invalidate();
    }

    // ------------------------------------------------------------ Lignes et colonnes

    fn structural(&mut self, sheet: usize, edit: Structural) {
        if sheet >= self.sheets.len() || edit.n == 0 || edit.at >= edit.limit_max() {
            return;
        }
        let resolver = sheet_resolver(&self.sheets);
        for (s, sh) in self.sheets.iter_mut().enumerate() {
            for cell in sh.cells.values_mut() {
                if !is_formula(&cell.input) {
                    continue;
                }
                let new = adjust_formula(&cell.input, edit, |name| match name {
                    None => s == sheet,
                    Some(n) => resolver.get(&n.to_lowercase()) == Some(&sheet),
                });
                cell.input = new;
            }
        }
        let sh = &mut self.sheets[sheet];
        let old = std::mem::take(&mut sh.cells);
        for (c, cell) in old {
            let moved = if edit.rows {
                edit.map(c.row).map(|r| CellRef::new(r, c.col))
            } else {
                edit.map(c.col).map(|col| CellRef::new(c.row, col))
            };
            if let Some(m) = moved {
                sh.cells.insert(m, cell);
            }
        }
        // Les plages des règles conditionnelles suivent les cellules.
        sh.cond_formats.retain_mut(|cf| {
            let r = cf.range;
            let (lo, hi) = if edit.rows {
                (r.start.row, r.end.row)
            } else {
                (r.start.col, r.end.col)
            };
            let Some((lo, hi)) = edit.map_span(lo, hi) else {
                return false;
            };
            cf.range = if edit.rows {
                Range::new(CellRef::new(lo, r.start.col), CellRef::new(hi, r.end.col))
            } else {
                Range::new(CellRef::new(r.start.row, lo), CellRef::new(r.end.row, hi))
            };
            true
        });
        // Le filtre automatique aussi (il disparaît avec sa ligne d'en-tête).
        if let Some(af) = &mut sh.auto_filter {
            let r = af.range;
            let keep = if edit.rows {
                edit.map(r.start.row).and_then(|top| {
                    let (_, hi) = edit.map_span(r.start.row, r.end.row)?;
                    af.range =
                        Range::new(CellRef::new(top, r.start.col), CellRef::new(hi, r.end.col));
                    Some(())
                })
            } else {
                edit.map_span(r.start.col, r.end.col).map(|(lo, hi)| {
                    af.range =
                        Range::new(CellRef::new(r.start.row, lo), CellRef::new(r.end.row, hi));
                    af.columns.retain_mut(|c| match edit.map(c.col) {
                        Some(n) => {
                            c.col = n;
                            true
                        }
                        None => false,
                    });
                })
            };
            if keep.is_none() {
                sh.auto_filter = None;
            }
        }
        let sizes = if edit.rows {
            &mut sh.row_heights
        } else {
            &mut sh.col_widths
        };
        let old = std::mem::take(sizes);
        for (k, v) in old {
            if let Some(nk) = edit.map(k) {
                sizes.insert(nk, v);
            }
        }
        self.invalidate();
    }

    /// Insère `n` lignes vides avant la ligne `at_row` ; les formules de tout le
    /// classeur sont adaptées.
    pub fn insert_rows(&mut self, sheet: usize, at_row: u32, n: u32) {
        self.structural(
            sheet,
            Structural {
                rows: true,
                insert: true,
                at: at_row,
                n,
            },
        );
    }

    /// Supprime `n` lignes à partir de `at_row` ; les références vers ces lignes
    /// deviennent `#REF!`.
    pub fn delete_rows(&mut self, sheet: usize, at_row: u32, n: u32) {
        let n = n.min(MAX_ROWS.saturating_sub(at_row));
        self.structural(
            sheet,
            Structural {
                rows: true,
                insert: false,
                at: at_row,
                n,
            },
        );
    }

    /// Insère `n` colonnes vides avant la colonne `at_col`.
    pub fn insert_cols(&mut self, sheet: usize, at_col: u32, n: u32) {
        self.structural(
            sheet,
            Structural {
                rows: false,
                insert: true,
                at: at_col,
                n,
            },
        );
    }

    /// Supprime `n` colonnes à partir de `at_col`.
    pub fn delete_cols(&mut self, sheet: usize, at_col: u32, n: u32) {
        let n = n.min(MAX_COLS.saturating_sub(at_col));
        self.structural(
            sheet,
            Structural {
                rows: false,
                insert: false,
                at: at_col,
                n,
            },
        );
    }

    // ------------------------------------------------------------ Copier, coller, recopier

    /// Copie une plage (saisies, mises en forme et valeurs affichées).
    pub fn copy_range(&self, sheet: usize, r: Range) -> Clip {
        let mut clip = Clip {
            origin: Some(r.start),
            rows: r.rows(),
            cols: r.cols(),
            cells: Vec::new(),
        };
        let Some(sh) = self.sheets.get(sheet) else {
            return clip;
        };
        for (c, cell) in sh.cells.range(r.start..=r.end) {
            if !r.contains(*c) {
                continue;
            }
            clip.cells.push(ClipCell {
                row: c.row - r.start.row,
                col: c.col - r.start.col,
                input: cell.input.clone(),
                format: cell.format,
                display: self.display(sheet, *c),
                value: self.value(sheet, *c),
            });
        }
        clip
    }

    /// Colle une copie à partir de `at` : la zone collée est remplacée, et les
    /// références relatives des formules sont décalées (les `$` restent fixes).
    pub fn paste(&mut self, sheet: usize, at: CellRef, clip: &Clip) {
        if sheet >= self.sheets.len() || clip.is_empty() {
            return;
        }
        let end = CellRef::new(
            (u64::from(at.row) + u64::from(clip.rows) - 1).min(u64::from(MAX_ROWS) - 1) as u32,
            (u64::from(at.col) + u64::from(clip.cols) - 1).min(u64::from(MAX_COLS) - 1) as u32,
        );
        let area = Range::new(at, end);
        let sh = &mut self.sheets[sheet];
        let old: Vec<CellRef> = sh
            .cells
            .range(area.start..=area.end)
            .map(|(k, _)| *k)
            .filter(|k| area.contains(*k))
            .collect();
        for k in old {
            sh.cells.remove(&k);
        }
        for c in &clip.cells {
            let Some(target) = at.offset(i64::from(c.row), i64::from(c.col)) else {
                continue;
            };
            let input = match clip.origin {
                Some(o) => {
                    let drow = i64::from(at.row) - i64::from(o.row);
                    let dcol = i64::from(at.col) - i64::from(o.col);
                    shift_formula(&c.input, drow as i32, dcol as i32)
                }
                None if is_formula(&c.input) => normalize_formula(&c.input),
                None => c.input.clone(),
            };
            let mut format = c.format;
            if clip.origin.is_none()
                && format.number == NumberFormat::General
                && let (_, Some(suggested)) = parse_literal(&input)
            {
                format.number = suggested;
            }
            sh.put(target, input, format);
        }
        self.invalidate();
    }

    /// Recopie la première ligne de la plage vers le bas (Ctrl+D). Si la plage
    /// n'a qu'une ligne, c'est la ligne du dessus qui est recopiée.
    pub fn fill_down(&mut self, sheet: usize, r: Range) {
        let (src, first) = if r.rows() == 1 {
            if r.start.row == 0 {
                return;
            }
            (r.start.row - 1, r.start.row)
        } else {
            (r.start.row, r.start.row + 1)
        };
        let Some(sh) = self.sheets.get_mut(sheet) else {
            return;
        };
        for col in r.start.col..=r.end.col {
            let source = sh
                .cells
                .get(&CellRef::new(src, col))
                .cloned()
                .unwrap_or_default();
            for row in first..=r.end.row {
                let input = shift_formula(&source.input, (row - src) as i32, 0);
                sh.put(CellRef::new(row, col), input, source.format);
            }
        }
        self.invalidate();
    }

    /// Recopie la première colonne de la plage vers la droite (Ctrl+R). Si la
    /// plage n'a qu'une colonne, c'est la colonne de gauche qui est recopiée.
    pub fn fill_right(&mut self, sheet: usize, r: Range) {
        let (src, first) = if r.cols() == 1 {
            if r.start.col == 0 {
                return;
            }
            (r.start.col - 1, r.start.col)
        } else {
            (r.start.col, r.start.col + 1)
        };
        let Some(sh) = self.sheets.get_mut(sheet) else {
            return;
        };
        for row in r.start.row..=r.end.row {
            let source = sh
                .cells
                .get(&CellRef::new(row, src))
                .cloned()
                .unwrap_or_default();
            for col in first..=r.end.col {
                let input = shift_formula(&source.input, 0, (col - src) as i32);
                sh.put(CellRef::new(row, col), input, source.format);
            }
        }
        self.invalidate();
    }

    /// Trie les lignes d'une plage selon la colonne `by_col` (index de colonne de
    /// la feuille ; si elle est hors de la plage et plus petite que sa largeur,
    /// c'est un décalage depuis la première colonne). Nombres d'abord, puis
    /// textes (sans casse ni accents), valeurs logiques, erreurs ; les cellules
    /// vides restent à la fin. Le tri est stable ; les formules déplacées sont
    /// décalées comme par un copier-coller.
    pub fn sort_rows(&mut self, sheet: usize, r: Range, by_col: u32, ascending: bool) {
        if sheet >= self.sheets.len() || r.rows() < 2 {
            return;
        }
        let key_col = if (r.start.col..=r.end.col).contains(&by_col) || by_col >= r.cols() {
            by_col
        } else {
            r.start.col + by_col
        };
        let mut order: Vec<(u32, Value)> = (r.start.row..=r.end.row)
            .map(|row| (row, self.value(sheet, CellRef::new(row, key_col))))
            .collect();
        order.sort_by(|(_, a), (_, b)| match (a.is_empty(), b.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ if ascending => sort_order(a, b),
            _ => sort_order(b, a),
        });
        let sh = &mut self.sheets[sheet];
        let mut taken: BTreeMap<CellRef, Cell> = BTreeMap::new();
        let keys: Vec<CellRef> = sh
            .cells
            .range(r.start..=r.end)
            .map(|(k, _)| *k)
            .filter(|k| r.contains(*k))
            .collect();
        for k in keys {
            if let Some(c) = sh.cells.remove(&k) {
                taken.insert(k, c);
            }
        }
        for (i, (old_row, _)) in order.iter().enumerate() {
            let new_row = r.start.row + i as u32;
            for col in r.start.col..=r.end.col {
                if let Some(cell) = taken.remove(&CellRef::new(*old_row, col)) {
                    let drow = i64::from(new_row) - i64::from(*old_row);
                    let input = shift_formula(&cell.input, drow as i32, 0);
                    sh.put(CellRef::new(new_row, col), input, cell.format);
                }
            }
        }
        self.invalidate();
    }

    // ------------------------------------------------------------ Affichage

    /// Fige les `rows` premières lignes et les `cols` premières colonnes
    /// (0 et 0 : libère les volets).
    pub fn set_frozen(&mut self, sheet: usize, rows: u32, cols: u32) {
        if let Some(sh) = self.sheets.get_mut(sheet) {
            sh.frozen_rows = rows.min(MAX_ROWS - 1);
            sh.frozen_cols = cols.min(MAX_COLS - 1);
        }
    }

    /// Prépare la mise en forme conditionnelle d'une feuille (statistiques des
    /// plages) ; à interroger ensuite cellule par cellule.
    pub fn cond_formatter(&self, sheet: usize) -> CondFormatter {
        let computed = self.computed();
        let values = computed.values.get(sheet);
        let value = |at: CellRef| values.and_then(|m| m.get(&at)).cloned().unwrap_or_default();
        match self.sheets.get(sheet) {
            Some(sh) => CondFormatter::new(sh, &value),
            None => CondFormatter::new(&Sheet::default(), &value),
        }
    }

    /// Mise en forme conditionnelle d'une seule cellule (pour beaucoup de
    /// cellules, préférer [`Workbook::cond_formatter`]).
    pub fn cond_paint(&self, sheet: usize, at: CellRef) -> Option<CondPaint> {
        self.cond_formatter(sheet).paint(at, &self.value(sheet, at))
    }

    // ------------------------------------------------------------ Fichiers

    /// Enregistre le classeur en JSON (format `varium-calc`, version 1).
    pub fn to_json(&self) -> String {
        #[derive(Serialize)]
        struct Out<'a> {
            format: &'static str,
            version: u32,
            title: &'a str,
            sheets: &'a [Sheet],
            #[serde(skip_serializing_if = "<[CategoryRule]>::is_empty")]
            bank_rules: &'a [CategoryRule],
            #[serde(skip_serializing_if = "RatesTable::is_empty")]
            rates: &'a RatesTable,
        }
        serde_json::to_string_pretty(&Out {
            format: "varium-calc",
            version: 1,
            title: &self.title,
            sheets: &self.sheets,
            bank_rules: &self.bank_rules,
            rates: &self.rates,
        })
        .unwrap_or_default()
    }

    /// Ouvre un classeur enregistré par [`Workbook::to_json`].
    pub fn from_json(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct In {
            format: Option<String>,
            version: Option<u32>,
            #[serde(default)]
            title: String,
            #[serde(default)]
            sheets: Vec<Sheet>,
            #[serde(default)]
            bank_rules: Vec<CategoryRule>,
            #[serde(default)]
            rates: RatesTable,
        }
        let parsed: In = serde_json::from_str(text.trim_start_matches('\u{FEFF}'))
            .map_err(|e| format!("Fichier de classeur illisible : {e}"))?;
        match parsed.format.as_deref() {
            Some("varium-calc") => {}
            Some(other) => {
                return Err(format!(
                    "Ce fichier n'est pas un classeur Varium (format « {other} »)"
                ));
            }
            None => return Err("Ce fichier n'est pas un classeur Varium".into()),
        }
        if parsed.version.unwrap_or(1) > 1 {
            return Err("Ce classeur a été créé par une version plus récente de Varium".into());
        }
        let mut wb = Workbook::new();
        wb.title = parsed.title;
        wb.bank_rules = parsed.bank_rules;
        wb.rates = parsed.rates;
        if !parsed.sheets.is_empty() {
            wb.sheets = parsed.sheets;
        }
        Ok(wb)
    }

    /// Exporte une feuille en CSV (séparateur `;`, valeurs telles qu'affichées).
    pub fn to_csv(&self, sheet: usize) -> String {
        crate::csv::to_csv(self, sheet)
    }

    /// Importe un fichier CSV (séparateur `;`, `,` ou tabulation détecté).
    pub fn from_csv(title: &str, text: &str) -> Result<Self, String> {
        crate::csv::from_csv(title, text)
    }

    /// Exporte au format Office Open XML (`.xlsx`).
    pub fn to_xlsx(&self) -> Vec<u8> {
        crate::xlsx::to_xlsx(self)
    }

    /// Exporte au format OpenDocument (`.ods`).
    pub fn to_ods(&self) -> Vec<u8> {
        crate::ods::to_ods(self)
    }

    /// Ouvre un fichier Excel (`.xlsx`) : valeurs, formules, formats, largeurs
    /// de colonnes, mise en forme conditionnelle et volets figés.
    pub fn from_xlsx(title: &str, bytes: &[u8]) -> Result<Self, String> {
        crate::xlsx_read::from_xlsx(title, bytes)
    }

    /// Ouvre un fichier OpenDocument (`.ods`).
    pub fn from_ods(title: &str, bytes: &[u8]) -> Result<Self, String> {
        crate::ods_read::from_ods(title, bytes)
    }

    /// Valeurs calculées de toutes les cellules (exports).
    pub(crate) fn snapshot(&self) -> Arc<Computed> {
        self.computed()
    }
}

impl Structural {
    fn limit_max(&self) -> u32 {
        if self.rows { MAX_ROWS } else { MAX_COLS }
    }
}
