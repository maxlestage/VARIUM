//! Tableau croisé dynamique : des champs en lignes, en colonnes et en
//! valeurs (somme, moyenne, nombre, minimum, maximum, pourcentage du total),
//! des filtres, des regroupements (dates par mois, trimestre ou année ;
//! nombres par tranches), des sous-totaux et des totaux.
//!
//! La définition est enregistrée dans la feuille qui reçoit le résultat
//! ([`Sheet::pivots`](crate::Sheet)) ; le résultat est écrit en valeurs
//! (exportées telles quelles en XLSX et ODS) et se met à jour par
//! [`Workbook::refresh_pivots`] quand la source change.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use varium_core::Lang;

use crate::cond::range_serde;
use crate::format::{CellFormat, NumberFormat, serial_to_ymd};
use crate::i18n::{loc, month_name};
use crate::refs::{CellRef, MAX_COLS, MAX_ROWS, Range};
use crate::value::Value;
use crate::workbook::Workbook;

/// Plus grand nombre de lignes (ou de colonnes) de résultat.
pub const MAX_PIVOT_KEYS: usize = 2_000;
/// Plus grand nombre de lignes de source lues.
const MAX_SOURCE_ROWS: u32 = 200_000;

/// Regroupement des valeurs d'un champ de ligne ou de colonne.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum PivotGroup {
    /// Chaque valeur distincte est une ligne.
    #[default]
    None,
    /// Dates regroupées par mois.
    Month,
    /// Dates regroupées par trimestre.
    Quarter,
    /// Dates regroupées par année.
    Year,
    /// Nombres regroupés par tranches de `step` (0–99, 100–199…).
    Range { step: f64 },
}

/// Calcul d'un champ de valeurs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PivotAgg {
    #[default]
    Sum,
    Average,
    Count,
    Min,
    Max,
    /// Somme, en pourcentage du total général.
    PercentOfTotal,
}

impl PivotAgg {
    pub const ALL: [PivotAgg; 6] = [
        PivotAgg::Sum,
        PivotAgg::Average,
        PivotAgg::Count,
        PivotAgg::Min,
        PivotAgg::Max,
        PivotAgg::PercentOfTotal,
    ];

    /// Libellé (« Somme », « Moyenne »…) dans la langue donnée.
    pub fn label(self, lang: Lang) -> &'static str {
        loc(
            lang,
            match self {
                PivotAgg::Sum => "Somme",
                PivotAgg::Average => "Moyenne",
                PivotAgg::Count => "Nombre",
                PivotAgg::Min => "Min",
                PivotAgg::Max => "Max",
                PivotAgg::PercentOfTotal => "% du total",
            },
        )
    }
}

/// Champ de ligne ou de colonne : colonne de la source (décalage depuis la
/// première colonne) et regroupement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PivotField {
    pub col: u32,
    pub group: PivotGroup,
}

/// Champ de valeurs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PivotValue {
    pub col: u32,
    pub agg: PivotAgg,
}

/// Filtre : les valeurs (telles qu'affichées) d'une colonne qui sont exclues.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PivotFilter {
    pub col: u32,
    pub hidden: Vec<String>,
}

/// Définition d'un tableau croisé dynamique.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PivotTable {
    /// Nom de la feuille des données (suit les déplacements de feuilles).
    pub source_sheet: String,
    /// Plage des données, ligne de titres comprise.
    #[serde(with = "range_serde")]
    pub source: Range,
    pub rows: Vec<PivotField>,
    pub cols: Vec<PivotField>,
    pub values: Vec<PivotValue>,
    pub filters: Vec<PivotFilter>,
    /// Coin haut gauche du résultat, dans la feuille qui porte la définition.
    pub at: CellRef,
    /// Lignes et colonnes de total général.
    pub totals: bool,
    /// Sous-totaux par valeur du premier champ de ligne (s'il y en a plusieurs).
    pub subtotals: bool,
    /// Taille du dernier résultat écrit (effacé avant d'écrire le suivant).
    pub written: (u32, u32),
}

impl Default for PivotTable {
    fn default() -> Self {
        Self {
            source_sheet: String::new(),
            source: Range::cell(CellRef::default()),
            rows: Vec::new(),
            cols: Vec::new(),
            values: Vec::new(),
            filters: Vec::new(),
            at: CellRef::default(),
            totals: true,
            subtotals: true,
            written: (0, 0),
        }
    }
}

/// Une cellule du résultat.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PivotCell {
    pub value: Value,
    pub format: NumberFormat,
    pub bold: bool,
    /// Ligne de titre ou de total (fond coloré).
    pub header: bool,
}

/// Pourquoi un tableau croisé ne peut pas être calculé.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PivotError {
    /// La feuille des données n'existe plus.
    NoSource,
    /// La source n'a pas de lignes de données sous ses titres.
    NoData,
    /// Aucun champ de valeurs ni de lignes.
    NoFields,
    /// Trop de lignes ou de colonnes de résultat.
    TooLarge,
}

impl PivotError {
    /// Explication dans la langue donnée.
    pub fn message(&self, lang: Lang) -> String {
        loc(
            lang,
            match self {
                PivotError::NoSource => "La feuille des données du tableau croisé n'existe plus.",
                PivotError::NoData => "La source n'a pas de données sous sa ligne de titres.",
                PivotError::NoFields => "Glissez au moins un champ dans Lignes ou dans Valeurs.",
                PivotError::TooLarge => {
                    "Trop de valeurs différentes : regroupez les dates ou les nombres."
                }
            },
        )
        .to_string()
    }
}

/// Clé de regroupement : ordre de tri et libellé.
#[derive(Clone, Debug)]
struct Key {
    rank: u8,
    num: f64,
    label: String,
}

impl PartialEq for Key {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Key {
    fn cmp(&self, o: &Self) -> Ordering {
        self.rank
            .cmp(&o.rank)
            .then(self.num.total_cmp(&o.num))
            .then_with(|| {
                crate::workbook::collation_key(&self.label)
                    .cmp(&crate::workbook::collation_key(&o.label))
            })
            .then_with(|| self.label.cmp(&o.label))
    }
}

/// Accumulateur d'un champ de valeurs.
#[derive(Clone, Copy, Debug)]
struct Acc {
    sum: f64,
    n: usize,
    count: usize,
    min: f64,
    max: f64,
}

impl Default for Acc {
    fn default() -> Self {
        Self {
            sum: 0.0,
            n: 0,
            count: 0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
        }
    }
}

impl Acc {
    fn add(&mut self, v: &Value) {
        if v.is_empty() {
            return;
        }
        self.count += 1;
        if let Value::Number(x) = v
            && x.is_finite()
        {
            self.sum += x;
            self.n += 1;
            self.min = self.min.min(*x);
            self.max = self.max.max(*x);
        }
    }

    fn merge(&mut self, o: &Acc) {
        self.sum += o.sum;
        self.n += o.n;
        self.count += o.count;
        self.min = self.min.min(o.min);
        self.max = self.max.max(o.max);
    }

    fn result(&self, agg: PivotAgg, grand: f64) -> Value {
        match self.raw(agg, grand) {
            Value::Number(x) => Value::Number(tidy(x)),
            v => v,
        }
    }

    fn raw(&self, agg: PivotAgg, grand: f64) -> Value {
        match agg {
            PivotAgg::Sum => Value::Number(self.sum),
            PivotAgg::Count => Value::Number(self.count as f64),
            PivotAgg::Average if self.n > 0 => Value::Number(self.sum / self.n as f64),
            PivotAgg::Min if self.n > 0 => Value::Number(self.min),
            PivotAgg::Max if self.n > 0 => Value::Number(self.max),
            PivotAgg::PercentOfTotal if grand != 0.0 => Value::Number(self.sum / grand),
            PivotAgg::PercentOfTotal => Value::Number(0.0),
            _ => Value::Empty,
        }
    }
}

/// Arrondi à 12 chiffres significatifs : les sommes de décimaux ne
/// laissent pas de traces (`1209,5500000000002`).
fn tidy(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let digits = (12 - x.abs().log10().ceil() as i32).clamp(0, 15);
    let p = 10f64.powi(digits);
    let r = (x * p).round() / p;
    if r.is_finite() { r } else { x }
}

fn number_text(x: f64) -> String {
    crate::format::format_general(x)
}

fn group_key(v: &Value, shown: &str, g: PivotGroup, lang: Lang) -> Key {
    let blank = || Key {
        rank: 3,
        num: 0.0,
        label: loc(lang, "(vide)").to_string(),
    };
    if v.is_empty() {
        return blank();
    }
    match (g, v) {
        (PivotGroup::Month | PivotGroup::Quarter | PivotGroup::Year, Value::Number(x)) => {
            let Some((y, m, _)) = serial_to_ymd(*x) else {
                return blank();
            };
            match g {
                PivotGroup::Month => Key {
                    rank: 0,
                    num: f64::from(y) * 12.0 + f64::from(m),
                    label: format!("{} {y}", month_name(lang, m)),
                },
                PivotGroup::Quarter => {
                    let q = (m - 1) / 3 + 1;
                    Key {
                        rank: 0,
                        num: f64::from(y) * 4.0 + f64::from(q),
                        label: format!("{}{q} {y}", loc(lang, "T")),
                    }
                }
                _ => Key {
                    rank: 0,
                    num: f64::from(y),
                    label: y.to_string(),
                },
            }
        }
        (PivotGroup::Range { step }, Value::Number(x)) if step.is_finite() && step > 0.0 => {
            let lo = (x / step).floor() * step;
            let hi = lo + step;
            Key {
                rank: 0,
                num: lo,
                label: format!("{} – {}", number_text(lo), number_text(hi)),
            }
        }
        (_, Value::Number(x)) => Key {
            rank: 0,
            num: *x,
            label: shown.to_string(),
        },
        (_, Value::Text(_)) => Key {
            rank: 1,
            num: 0.0,
            label: shown.to_string(),
        },
        _ => Key {
            rank: 2,
            num: 0.0,
            label: shown.to_string(),
        },
    }
}

type KeyPath = Vec<Key>;

impl PivotTable {
    /// Nouvelle définition sur une source, résultat à écrire en `at`.
    pub fn new(source_sheet: &str, source: Range, at: CellRef) -> Self {
        Self {
            source_sheet: source_sheet.to_string(),
            source,
            at,
            ..Self::default()
        }
    }

    /// Les titres des champs (ligne de titres de la source ; lettre de la
    /// colonne si le titre est vide).
    pub fn field_names(&self, wb: &Workbook) -> Vec<String> {
        let Some(s) = wb.sheet_index(&self.source_sheet) else {
            return Vec::new();
        };
        (self.source.start.col..=self.source.end.col)
            .take(512)
            .map(|c| {
                let t = wb.display(s, CellRef::new(self.source.start.row, c));
                if t.trim().is_empty() {
                    crate::refs::col_name(c)
                } else {
                    t
                }
            })
            .collect()
    }

    /// Les valeurs distinctes d'un champ (pour les filtres), triées.
    pub fn field_values(&self, wb: &Workbook, col: u32) -> Vec<String> {
        let Some(s) = wb.sheet_index(&self.source_sheet) else {
            return Vec::new();
        };
        let c = self.source.start.col.saturating_add(col);
        let mut keys: BTreeMap<Key, ()> = BTreeMap::new();
        for r in self.data_rows() {
            let at = CellRef::new(r, c);
            let shown = wb.display(s, at);
            keys.insert(
                group_key(&wb.value(s, at), &shown, PivotGroup::None, Lang::Fr),
                (),
            );
            if keys.len() > MAX_PIVOT_KEYS {
                break;
            }
        }
        keys.into_keys()
            .map(|k| if k.rank == 3 { String::new() } else { k.label })
            .collect()
    }

    fn data_rows(&self) -> std::ops::RangeInclusive<u32> {
        let first = self.source.start.row.saturating_add(1);
        let last = self
            .source
            .end
            .row
            .min(first.saturating_add(MAX_SOURCE_ROWS));
        first..=last
    }

    /// Le champ est-il une colonne de dates (format date sur la plupart des
    /// valeurs) ? Sert à proposer le regroupement par mois.
    pub fn is_date_field(&self, wb: &Workbook, col: u32) -> bool {
        let Some(s) = wb.sheet_index(&self.source_sheet) else {
            return false;
        };
        let c = self.source.start.col.saturating_add(col);
        let (mut dates, mut others) = (0, 0);
        for r in self.data_rows().take(500) {
            let at = CellRef::new(r, c);
            match wb.value(s, at) {
                Value::Number(_) if wb.format(s, at).number == NumberFormat::Date => dates += 1,
                Value::Empty => {}
                _ => others += 1,
            }
        }
        dates > 0 && dates >= others
    }

    /// Calcule le résultat (sans l'écrire).
    pub fn compute(&self, wb: &Workbook, lang: Lang) -> Result<Vec<Vec<PivotCell>>, PivotError> {
        let s = wb
            .sheet_index(&self.source_sheet)
            .ok_or(PivotError::NoSource)?;
        if self.rows.is_empty() && self.values.is_empty() {
            return Err(PivotError::NoFields);
        }
        if self.source.rows() < 2 {
            return Err(PivotError::NoData);
        }
        let width = self.source.cols();
        let names = self.field_names(wb);
        let name_of = |c: u32| names.get(c as usize).cloned().unwrap_or_default();
        let rows_f: Vec<PivotField> = self
            .rows
            .iter()
            .filter(|f| f.col < width)
            .copied()
            .collect();
        let cols_f: Vec<PivotField> = self
            .cols
            .iter()
            .filter(|f| f.col < width)
            .copied()
            .collect();
        let vals: Vec<PivotValue> = self
            .values
            .iter()
            .filter(|v| v.col < width)
            .copied()
            .collect();
        // Sans champ de valeurs : on compte les lignes.
        let vals = if vals.is_empty() {
            vec![PivotValue {
                col: rows_f.first().map_or(0, |f| f.col),
                agg: PivotAgg::Count,
            }]
        } else {
            vals
        };
        let nv = vals.len();
        let c0 = self.source.start.col;
        let read = |r: u32, col: u32| {
            let at = CellRef::new(r, c0.saturating_add(col));
            (wb.value(s, at), at)
        };

        // Agrégation.
        let mut cells: BTreeMap<(KeyPath, KeyPath), Vec<Acc>> = BTreeMap::new();
        let mut row_keys: BTreeMap<KeyPath, ()> = BTreeMap::new();
        let mut col_keys: BTreeMap<KeyPath, ()> = BTreeMap::new();
        let mut any = false;
        'rows: for r in self.data_rows() {
            // Ligne entièrement vide : ignorée.
            if (0..width.min(512)).all(|c| read(r, c).0.is_empty()) {
                continue;
            }
            for f in &self.filters {
                if f.hidden.is_empty() || f.col >= width {
                    continue;
                }
                let (_, at) = read(r, f.col);
                if f.hidden.contains(&wb.display(s, at)) {
                    continue 'rows;
                }
            }
            any = true;
            let path = |fields: &[PivotField]| -> KeyPath {
                fields
                    .iter()
                    .map(|f| {
                        let (v, at) = read(r, f.col);
                        group_key(&v, &wb.display(s, at), f.group, lang)
                    })
                    .collect()
            };
            let rk = path(&rows_f);
            let ck = path(&cols_f);
            row_keys.insert(rk.clone(), ());
            col_keys.insert(ck.clone(), ());
            if row_keys.len() > MAX_PIVOT_KEYS || col_keys.len() > 200 {
                return Err(PivotError::TooLarge);
            }
            let accs = cells
                .entry((rk, ck))
                .or_insert_with(|| vec![Acc::default(); nv]);
            for (i, v) in vals.iter().enumerate() {
                accs[i].add(&read(r, v.col).0);
            }
        }
        if !any {
            return Err(PivotError::NoData);
        }
        let row_keys: Vec<KeyPath> = row_keys.into_keys().collect();
        let col_keys: Vec<KeyPath> = col_keys.into_keys().collect();

        // Totaux généraux par valeur (pour les pourcentages).
        let mut grand = vec![Acc::default(); nv];
        for accs in cells.values() {
            for (g, a) in grand.iter_mut().zip(accs) {
                g.merge(a);
            }
        }
        let grand_sums: Vec<f64> = grand.iter().map(|a| a.sum).collect();
        let fmt_of = |v: &PivotValue| match v.agg {
            PivotAgg::PercentOfTotal => NumberFormat::Percent(1),
            PivotAgg::Count => NumberFormat::General,
            _ => {
                let at = CellRef::new(
                    self.source.start.row.saturating_add(1),
                    c0.saturating_add(v.col),
                );
                match wb.format(s, at).number {
                    NumberFormat::Date | NumberFormat::Text => NumberFormat::General,
                    f => f,
                }
            }
        };
        let value_label = |v: &PivotValue| format!("{} – {}", v.agg.label(lang), name_of(v.col));

        // Colonnes du résultat : (clé de colonne ou None pour le total, valeur).
        let multi_cols = !cols_f.is_empty();
        let mut out_cols: Vec<(Option<&KeyPath>, usize)> = Vec::new();
        for ck in &col_keys {
            for i in 0..nv {
                out_cols.push((Some(ck), i));
            }
        }
        if multi_cols && self.totals {
            for i in 0..nv {
                out_cols.push((None, i));
            }
        }
        let label_cols = rows_f.len().max(1);

        let text = |t: String| Value::Text(t);
        let head = |v: Value| PivotCell {
            value: v,
            format: NumberFormat::General,
            bold: true,
            header: true,
        };
        let mut grid: Vec<Vec<PivotCell>> = Vec::new();
        // Ligne de titres.
        let mut title = Vec::new();
        for i in 0..label_cols {
            title.push(head(match rows_f.get(i) {
                Some(f) => text(name_of(f.col)),
                None => Value::Empty,
            }));
        }
        for (ck, i) in &out_cols {
            let vl = value_label(&vals[*i]);
            let label = match ck {
                Some(ck) => {
                    let k = ck
                        .iter()
                        .map(|k| k.label.clone())
                        .collect::<Vec<_>>()
                        .join(" – ");
                    if nv > 1 { format!("{k} – {vl}") } else { k }
                }
                None if nv > 1 => format!("{} – {vl}", loc(lang, "Total")),
                None => loc(lang, "Total").to_string(),
            };
            let label = if multi_cols { label } else { vl };
            title.push(head(text(label)));
        }
        grid.push(title);

        // Valeurs d'une ligne (clé de ligne, ou toutes si `None` ; préfixe pour
        // les sous-totaux).
        let row_values = |filter: &dyn Fn(&KeyPath) -> bool| -> Vec<PivotCell> {
            out_cols
                .iter()
                .map(|(ck, i)| {
                    let mut acc = Acc::default();
                    for ((rk, k), accs) in &cells {
                        if filter(rk) && ck.is_none_or(|c| c == k) {
                            acc.merge(&accs[*i]);
                        }
                    }
                    PivotCell {
                        value: acc.result(vals[*i].agg, grand_sums[*i]),
                        format: fmt_of(&vals[*i]),
                        bold: false,
                        header: false,
                    }
                })
                .collect()
        };

        let subtotals = self.subtotals && rows_f.len() >= 2;
        let mut i = 0;
        while i < row_keys.len() {
            let rk = &row_keys[i];
            let mut line: Vec<PivotCell> = (0..label_cols)
                .map(|j| PivotCell {
                    value: match rk.get(j) {
                        Some(k) if rows_f.len() < 2 || j > 0 || !subtotals => text(k.label.clone()),
                        Some(k) => {
                            // Premier champ : affiché une fois par groupe.
                            let first = i == 0
                                || row_keys[i - 1].first().map(|x| &x.label) != Some(&k.label);
                            if first {
                                text(k.label.clone())
                            } else {
                                Value::Empty
                            }
                        }
                        None => Value::Empty,
                    },
                    format: NumberFormat::General,
                    bold: j == 0 && rows_f.len() >= 2,
                    header: false,
                })
                .collect();
            line.extend(row_values(&|k| k == rk));
            grid.push(line);
            if grid.len() > MAX_PIVOT_KEYS * 2 {
                return Err(PivotError::TooLarge);
            }
            let next_differs = row_keys.get(i + 1).is_none_or(|n| n.first() != rk.first());
            if subtotals
                && next_differs
                && let Some(first) = rk.first()
            {
                let mut sub: Vec<PivotCell> = (0..label_cols)
                    .map(|j| PivotCell {
                        value: if j == 0 {
                            text(format!("{} {}", loc(lang, "Total"), first.label))
                        } else {
                            Value::Empty
                        },
                        format: NumberFormat::General,
                        bold: true,
                        header: false,
                    })
                    .collect();
                let mut cellsv = row_values(&|k| k.first() == Some(first));
                for c in &mut cellsv {
                    c.bold = true;
                }
                sub.extend(cellsv);
                grid.push(sub);
            }
            i += 1;
        }
        if self.totals && !rows_f.is_empty() {
            let mut tot: Vec<PivotCell> = (0..label_cols)
                .map(|j| {
                    head(if j == 0 {
                        text(loc(lang, "Total général").to_string())
                    } else {
                        Value::Empty
                    })
                })
                .collect();
            let mut cellsv = row_values(&|_| true);
            for c in &mut cellsv {
                c.bold = true;
                c.header = true;
            }
            tot.extend(cellsv);
            grid.push(tot);
        }
        Ok(grid)
    }
}

impl Workbook {
    /// Écrit un tableau croisé dans la feuille `sheet` (le résultat précédent
    /// du même tableau est effacé) et enregistre sa définition. Renvoie la
    /// plage écrite.
    pub fn write_pivot(
        &mut self,
        sheet: usize,
        mut pivot: PivotTable,
        lang: Lang,
    ) -> Result<Range, PivotError> {
        if sheet >= self.sheets.len() {
            return Err(PivotError::NoSource);
        }
        let grid = pivot.compute(self, lang)?;
        // Ancienne définition au même endroit : remplacée.
        let old = self.sheets[sheet]
            .pivots
            .iter()
            .position(|p| p.at == pivot.at);
        if let Some(i) = old {
            let prev = self.sheets[sheet].pivots.remove(i);
            self.clear_area(sheet, prev.at, prev.written);
        }
        let rows = grid.len() as u32;
        let cols = grid.iter().map(Vec::len).max().unwrap_or(0) as u32;
        let at = pivot.at;
        for (r, line) in grid.iter().enumerate() {
            for (c, cell) in line.iter().enumerate() {
                let Some(target) = at.offset(r as i64, c as i64) else {
                    continue;
                };
                let format = CellFormat {
                    number: cell.format,
                    bold: cell.bold,
                    fill: cell.header.then_some([221, 235, 247]),
                    ..CellFormat::default()
                };
                self.put_value(sheet, target, &cell.value, format);
            }
        }
        pivot.written = (rows, cols);
        self.sheets[sheet].pivots.push(pivot);
        self.invalidate();
        let end = CellRef::new(
            (at.row + rows.max(1) - 1).min(MAX_ROWS - 1),
            (at.col + cols.max(1) - 1).min(MAX_COLS - 1),
        );
        Ok(Range::new(at, end))
    }

    /// Recalcule et réécrit tous les tableaux croisés du classeur (après une
    /// modification de leurs données, ou à l'ouverture). Renvoie le nombre de
    /// tableaux mis à jour.
    pub fn refresh_pivots(&mut self, lang: Lang) -> usize {
        let mut n = 0;
        for s in 0..self.sheets.len() {
            let list = self.sheets[s].pivots.clone();
            for p in list {
                if self.write_pivot(s, p, lang).is_ok() {
                    n += 1;
                }
            }
        }
        n
    }

    /// Efface une zone (contenu et mise en forme).
    pub(crate) fn clear_area(&mut self, sheet: usize, at: CellRef, (rows, cols): (u32, u32)) {
        let Some(sh) = self.sheets.get_mut(sheet) else {
            return;
        };
        if rows == 0 || cols == 0 {
            return;
        }
        let end = CellRef::new(
            at.row.saturating_add(rows - 1).min(MAX_ROWS - 1),
            at.col.saturating_add(cols - 1).min(MAX_COLS - 1),
        );
        let area = Range::new(at, end);
        let keys: Vec<CellRef> = sh
            .cells
            .range(area.start..=area.end)
            .map(|(k, _)| *k)
            .filter(|k| area.contains(*k))
            .collect();
        for k in keys {
            sh.cells.remove(&k);
        }
        self.invalidate();
    }

    /// Écrit une valeur calculée comme saisie (nombre, texte protégé…), avec
    /// sa mise en forme.
    pub(crate) fn put_value(&mut self, sheet: usize, at: CellRef, v: &Value, format: CellFormat) {
        let input = match v {
            Value::Empty => String::new(),
            Value::Number(x) => crate::import::number_input(*x, format.number),
            Value::Text(t) => crate::import::text_input(t),
            Value::Bool(b) => crate::import::bool_input(*b),
            Value::Error(e) => format!("={}", e.code()),
        };
        if let Some(sh) = self.sheets.get_mut(sheet)
            && at.row < MAX_ROWS
            && at.col < MAX_COLS
        {
            sh.put(at, input, format);
        }
        self.invalidate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ventes() -> Workbook {
        let mut wb = Workbook::new();
        let rows = [
            ["Date", "Région", "Produit", "Ventes"],
            ["15/01/2026", "Nord", "Stylo", "10"],
            ["20/01/2026", "Sud", "Cahier", "20"],
            ["03/02/2026", "Nord", "Cahier", "30"],
            ["10/04/2026", "Est", "Stylo", "40"],
            ["11/04/2026", "Nord", "Stylo", "5"],
        ];
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                wb.set_input(0, CellRef::new(r as u32, c as u32), v);
            }
        }
        wb
    }

    fn src() -> Range {
        Range::parse("A1:D6").unwrap()
    }

    #[test]
    fn somme_par_region_avec_total() {
        let wb = ventes();
        let mut p = PivotTable::new("Feuille 1", src(), CellRef::new(0, 6));
        p.rows.push(PivotField {
            col: 1,
            group: PivotGroup::None,
        });
        p.values.push(PivotValue {
            col: 3,
            agg: PivotAgg::Sum,
        });
        let g = p.compute(&wb, Lang::Fr).unwrap();
        assert_eq!(g.len(), 5); // titres, Est, Nord, Sud, total
        assert_eq!(g[1][0].value, Value::Text("Est".into()));
        assert_eq!(g[2][1].value, Value::Number(45.0));
        assert_eq!(g[4][1].value, Value::Number(105.0));
        assert_eq!(g[0][1].value, Value::Text("Somme – Ventes".into()));
    }

    #[test]
    fn colonnes_trimestres_et_pourcentages() {
        let wb = ventes();
        let mut p = PivotTable::new("Feuille 1", src(), CellRef::new(0, 6));
        p.rows.push(PivotField {
            col: 2,
            group: PivotGroup::None,
        });
        p.cols.push(PivotField {
            col: 0,
            group: PivotGroup::Quarter,
        });
        p.values.push(PivotValue {
            col: 3,
            agg: PivotAgg::PercentOfTotal,
        });
        let g = p.compute(&wb, Lang::Fr).unwrap();
        assert_eq!(g[0][1].value, Value::Text("T1 2026".into()));
        assert_eq!(g[0][2].value, Value::Text("T2 2026".into()));
        // Cahier : 50 / 105 au T1.
        let Value::Number(x) = g[1][1].value else {
            panic!()
        };
        assert!((x - 50.0 / 105.0).abs() < 1e-9);
        assert_eq!(g[1][1].format, NumberFormat::Percent(1));
    }

    #[test]
    fn mois_sous_totaux_filtres_et_ecriture() {
        let mut wb = ventes();
        let mut p = PivotTable::new("Feuille 1", src(), CellRef::new(0, 6));
        p.rows.push(PivotField {
            col: 0,
            group: PivotGroup::Month,
        });
        p.rows.push(PivotField {
            col: 1,
            group: PivotGroup::None,
        });
        p.values.push(PivotValue {
            col: 3,
            agg: PivotAgg::Sum,
        });
        p.filters.push(PivotFilter {
            col: 2,
            hidden: vec!["Cahier".into()],
        });
        let r = wb.write_pivot(0, p, Lang::En).unwrap();
        assert_eq!(wb.display(0, CellRef::new(1, 6)), "January 2026");
        assert_eq!(wb.display(0, CellRef::new(1, 8)), "10");
        assert_eq!(wb.display(0, CellRef::new(2, 6)), "Total January 2026");
        assert_eq!(wb.sheets[0].pivots.len(), 1);
        // La source change : l'actualisation suit.
        wb.set_input(0, CellRef::new(1, 3), "100");
        assert_eq!(wb.refresh_pivots(Lang::En), 1);
        assert_eq!(wb.display(0, CellRef::new(1, 8)), "100");
        let last = wb.display(0, CellRef::new(r.end.row, 6));
        assert_eq!(last, "Grand total");
        // Le JSON garde la définition.
        let back = Workbook::from_json(&wb.to_json()).unwrap();
        assert_eq!(back.sheets[0].pivots, wb.sheets[0].pivots);
    }

    #[test]
    fn tranches_de_nombres_et_erreurs() {
        let wb = ventes();
        let mut p = PivotTable::new("Feuille 1", src(), CellRef::new(0, 6));
        p.rows.push(PivotField {
            col: 3,
            group: PivotGroup::Range { step: 25.0 },
        });
        let g = p.compute(&wb, Lang::Fr).unwrap();
        assert_eq!(g[1][0].value, Value::Text("0 – 25".into()));
        assert_eq!(g[1][1].value, Value::Number(3.0));
        assert_eq!(
            PivotTable::new("Nulle part", src(), CellRef::default()).compute(&wb, Lang::Fr),
            Err(PivotError::NoSource)
        );
        assert_eq!(
            PivotTable::new("Feuille 1", src(), CellRef::default()).compute(&wb, Lang::Fr),
            Err(PivotError::NoFields)
        );
        assert!(p.is_date_field(&wb, 0));
        assert!(!p.is_date_field(&wb, 1));
        assert_eq!(p.field_values(&wb, 1), vec!["Est", "Nord", "Sud"]);
    }

    #[test]
    fn jamais_de_panique() {
        let mut wb = ventes();
        for (cols, rows) in [(0, 0), (99, 99), (u32::MAX, 3)] {
            let mut p = PivotTable::new(
                "Feuille 1",
                Range::new(
                    CellRef::new(0, 0),
                    CellRef::new(rows, cols.min(MAX_COLS - 1)),
                ),
                CellRef::new(MAX_ROWS - 1, MAX_COLS - 1),
            );
            p.rows.push(PivotField {
                col: cols,
                group: PivotGroup::Range { step: -1.0 },
            });
            p.values.push(PivotValue {
                col: 1,
                agg: PivotAgg::Average,
            });
            let _ = wb.write_pivot(0, p, Lang::De);
        }
    }
}
