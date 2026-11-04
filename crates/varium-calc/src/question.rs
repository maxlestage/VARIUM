//! « Poser une question » : une question en langage simple sur les données
//! (« total des ventes par région », « moyenne de B », « combien de clients »)
//! est traduite par des règles en formules du Classeur.
//!
//! Les mots-clés sont reconnus en français, anglais, espagnol, allemand,
//! italien et portugais, sans tenir compte de la casse ni des accents. Les
//! colonnes se désignent par le titre de la première ligne (« ventes »,
//! « chiffre d'affaires ») ou par leur lettre (« colonne B », « de B »).
//! Aucune saisie ne peut faire paniquer : une question incomprise donne une
//! [`QuestionError`] que l'interface explique.

use varium_core::Lang;

use crate::format::{NumberFormat, date_serial, serial_to_ymd};
use crate::i18n::{loc, month_name};
use crate::refs::{CellRef, MAX_COLS, MAX_ROWS, Range, col_index, col_name};
use crate::value::Value;
use crate::workbook::Workbook;

/// Ce qu'on demande de calculer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Sum,
    Average,
    Count,
    Max,
    Min,
    Median,
}

/// Pourquoi une question n'a pas pu être traduite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuestionError {
    /// La question est vide.
    Empty,
    /// La feuille ne contient pas de données.
    NoData,
    /// Aucune opération reconnue (total, moyenne, nombre, maximum…).
    NoOperation,
    /// Aucune colonne reconnue ; `columns` liste les colonnes utilisables.
    NoColumn { columns: Vec<String> },
    /// Le regroupement demandé n'est pas possible pour cette opération
    /// (seuls le total, la moyenne et le nombre se calculent par groupe).
    GroupedUnsupported,
    /// Trop de valeurs différentes pour un regroupement lisible.
    TooManyGroups,
    /// Un regroupement par période demande une colonne de dates.
    NoDateColumn,
}

/// Réponse : une formule, ou un petit tableau regroupé.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    pub operation: Operation,
    /// Titre de la colonne calculée (ou sa lettre).
    pub value_header: Option<String>,
    /// Titre de la colonne de regroupement.
    pub group_header: Option<String>,
    /// Formule d'un résultat unique.
    pub formula: Option<String>,
    /// Pour un regroupement : (valeur du groupe, formule).
    pub groups: Vec<(String, String)>,
    /// Formule de la ligne « Total » d'un regroupement (total ou nombre).
    pub total: Option<String>,
    /// Colonne supplémentaire (« Évolution ») : titre et une formule par
    /// groupe, affichées en pourcentage.
    pub extra: Option<(String, Vec<String>)>,
    /// Lignes de groupe (index) affichées en pourcentage.
    pub percent_rows: Vec<usize>,
    /// Nombre de groupes à reprendre dans un graphique (0 : pas de graphique).
    pub chart_rows: usize,
    /// Regroupement par période, s'il y en a un.
    pub period: Option<Period>,
}

/// Période d'un regroupement par dates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Period {
    Week,
    Month,
    Quarter,
    Year,
}

/// Plus grand nombre de groupes proposés.
pub const MAX_GROUPS: usize = 200;

impl Answer {
    /// Le tableau de saisies à écrire, ligne par ligne. `op_label` est le nom
    /// traduit de l'opération (« Total »), `total_label` celui de la ligne de
    /// total.
    pub fn cells(&self, op_label: &str, total_label: &str) -> Vec<Vec<String>> {
        let title = match &self.value_header {
            Some(h) => format!("{op_label} {h}"),
            None => op_label.to_string(),
        };
        if let Some(f) = &self.formula {
            return vec![vec![title, f.clone()]];
        }
        let mut head = vec![self.group_header.clone().unwrap_or_default(), title];
        if let Some((h, _)) = &self.extra {
            head.push(h.clone());
        }
        let mut rows = vec![head];
        for (i, (g, f)) in self.groups.iter().enumerate() {
            let mut row = vec![safe_text(g), f.clone()];
            if let Some((_, extra)) = &self.extra {
                row.push(extra.get(i).cloned().unwrap_or_default());
            }
            rows.push(row);
        }
        if let Some(t) = &self.total {
            rows.push(vec![total_label.to_string(), t.clone()]);
        }
        rows
    }

    /// Plage à reprendre dans un graphique quand la réponse est écrite à
    /// partir de `at` (titres, étiquettes et valeurs).
    pub fn chart_range(&self, at: CellRef) -> Option<Range> {
        if self.chart_rows == 0 || self.formula.is_some() {
            return None;
        }
        let end = CellRef::new(
            at.row
                .saturating_add(self.chart_rows as u32)
                .min(MAX_ROWS - 1),
            at.col.saturating_add(1).min(MAX_COLS - 1),
        );
        Some(Range::new(at, end))
    }
}

/// Un texte qui serait lu comme une formule, un nombre ou une date est
/// protégé par une apostrophe.
fn safe_text(s: &str) -> String {
    crate::import::text_input(s)
}

/// Minuscules sans accents, ponctuation remplacée par des espaces.
fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'é' | 'è' | 'ê' | 'ë' | 'á' | 'à' | 'â' | 'ä' | 'ã' => match c {
                'é' | 'è' | 'ê' | 'ë' => 'e',
                _ => 'a',
            },
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'ß' => 's',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        };
        out.push(c);
    }
    out
}

/// Mots d'une chaîne pliée, sans le « s » final des pluriels.
fn words(s: &str) -> Vec<String> {
    fold(s)
        .split_whitespace()
        .map(|w| {
            if w.chars().count() > 3 && w.ends_with('s') {
                w[..w.len() - 1].to_string()
            } else {
                w.to_string()
            }
        })
        .collect()
}

/// Position du premier endroit où `needle` apparaît comme suite de mots.
fn find_seq(hay: &[String], needle: &[String]) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| hay[i..i + needle.len()] == *needle)
}

const OPERATIONS: [(Operation, &[&str]); 6] = [
    (Operation::Median, &["mediane", "median", "mediana"]),
    (
        Operation::Average,
        &[
            "moyenne",
            "average",
            "mean",
            "promedio",
            "media",
            "durchschnitt",
            "mittelwert",
        ],
    ),
    (
        Operation::Max,
        &[
            "maximum",
            "max",
            "plus grand",
            "plus grande",
            "plus eleve",
            "plus haut",
            "highest",
            "largest",
            "biggest",
            "maximo",
            "maior",
            "massimo",
            "hochste",
            "grosste",
        ],
    ),
    (
        Operation::Min,
        &[
            "minimum",
            "min",
            "plus petit",
            "plus petite",
            "plus bas",
            "lowest",
            "smallest",
            "minimo",
            "menor",
            "kleinste",
            "niedrigste",
            "piu piccolo",
        ],
    ),
    (
        Operation::Count,
        &[
            "combien",
            "nombre",
            "nb",
            "count",
            "how many",
            "cuantos",
            "cuantas",
            "numero de",
            "anzahl",
            "quanti",
            "quante",
            "quantos",
            "quantas",
        ],
    ),
    (
        Operation::Sum,
        &[
            "total",
            "somme",
            "sum",
            "suma",
            "summe",
            "somma",
            "soma",
            "totale",
            "gesamt",
            "addition",
            "additionne",
        ],
    ),
];

const GROUP_WORDS: [&str; 8] = ["par", "by", "per", "por", "pro", "nach", "je", "para"];
const COLUMN_WORDS: [&str; 7] = [
    "colonne", "column", "columna", "spalte", "coluna", "col", "colonna",
];

fn detect_operation(ws: &[String]) -> Option<(Operation, usize)> {
    OPERATIONS
        .iter()
        .filter_map(|(op, names)| {
            names
                .iter()
                .filter_map(|n| find_seq(ws, &words(n)).map(|p| (*op, p)))
                .min_by_key(|&(_, p)| p)
        })
        .min_by_key(|&(_, p)| p)
}

impl Answer {
    fn empty(operation: Operation) -> Self {
        Answer {
            operation,
            value_header: None,
            group_header: None,
            formula: None,
            groups: Vec::new(),
            total: None,
            extra: None,
            percent_rows: Vec::new(),
            chart_rows: 0,
            period: None,
        }
    }
}

// Mots pliés par [`words`] (« mois » devient « moi », « semaines » « semaine »).
const WEEK_WORDS: [&str; 5] = ["semaine", "week", "semana", "woche", "settimana"];
const MONTH_WORDS: [&str; 6] = ["moi", "month", "mes", "mese", "monat", "monate"];
const QUARTER_WORDS: [&str; 4] = ["trimestre", "quarter", "quartal", "quartale"];
const YEAR_WORDS: [&str; 8] = ["annee", "an", "ans", "year", "ano", "jahr", "jahre", "anno"];
const TOP_WORDS: [&str; 10] = [
    "top", "meilleur", "best", "mejor", "mejore", "beste", "besten", "migliori", "melhore",
    "premier",
];
const EVOLUTION_WORDS: [&str; 10] = [
    "evolution",
    "evolucion",
    "evolucao",
    "evoluzione",
    "entwicklung",
    "trend",
    "tendance",
    "progression",
    "croissance",
    "growth",
];

/// Formes particulières de question : période, classement, évolution,
/// comparaison d'années.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Special {
    period: Option<Period>,
    top: Option<usize>,
    evolution: bool,
    years: Option<(i32, i32)>,
}

impl Special {
    fn detect(ws: &[String]) -> Special {
        let has = |list: &[&str]| ws.iter().any(|w| list.contains(&w.as_str()));
        // « par mois », « chaque semaine » : un mot de période après un mot de
        // regroupement ; ou un adjectif (« mensuel »).
        let after_group = |list: &[&str]| {
            ws.windows(2).any(|p| {
                (GROUP_WORDS.contains(&p[0].as_str())
                    || matches!(p[0].as_str(), "chaque" | "each" | "every" | "cada" | "ogni"))
                    && list.contains(&p[1].as_str())
            })
        };
        let adjective = ws.iter().find_map(|w| match w.as_str() {
            "mensuel" | "mensuelle" | "monthly" | "mensual" | "mensile" | "monatlich"
            | "mensal" => Some(Period::Month),
            "hebdomadaire" | "weekly" | "semanal" | "settimanale" | "wochentlich" => {
                Some(Period::Week)
            }
            "trimestriel" | "trimestrielle" | "quarterly" | "trimestral" | "trimestrale"
            | "vierteljahrlich" => Some(Period::Quarter),
            "annuel" | "annuelle" | "yearly" | "annual" | "anual" | "annuale" | "jahrlich" => {
                Some(Period::Year)
            }
            _ => None,
        });
        let period = if after_group(&WEEK_WORDS) {
            Some(Period::Week)
        } else if after_group(&MONTH_WORDS) {
            Some(Period::Month)
        } else if after_group(&QUARTER_WORDS) {
            Some(Period::Quarter)
        } else if after_group(&YEAR_WORDS) {
            Some(Period::Year)
        } else {
            adjective
        };
        let top = ws.iter().enumerate().find_map(|(i, w)| {
            if !TOP_WORDS.contains(&w.as_str()) {
                return None;
            }
            let n = ws
                .get(i + 1)
                .and_then(|n| n.parse::<usize>().ok())
                .or_else(|| i.checked_sub(1).and_then(|j| ws[j].parse::<usize>().ok()))
                .unwrap_or(if w == "top" { 5 } else { 0 });
            (n > 0).then_some(n.min(MAX_GROUPS))
        });
        let mut years: Vec<i32> = ws
            .iter()
            .filter_map(|w| w.parse::<i32>().ok())
            .filter(|y| (1900..=2200).contains(y))
            .collect();
        years.dedup();
        let years = match years.as_slice() {
            [a, b, ..] if a != b => Some((*a, *b)),
            _ => None,
        };
        Special {
            period,
            top,
            evolution: has(&EVOLUTION_WORDS),
            years,
        }
    }

    fn any(&self) -> bool {
        self.period.is_some() || self.top.is_some() || self.evolution || self.years.is_some()
    }
}

/// Numéro de semaine ISO 8601 et son année.
fn iso_week(serial: f64) -> (i32, u32) {
    let day = serial.floor();
    // Le 1er janvier 1900 (série 2) était un lundi.
    let weekday = (day - 2.0).rem_euclid(7.0); // 0 = lundi
    let thursday = day - weekday + 3.0;
    let (y, _, _) = serial_to_ymd(thursday).unwrap_or((1970, 1, 1));
    let first = date_serial(y, 1, 1);
    (y, ((thursday - first) / 7.0).floor() as u32 + 1)
}

/// Début et fin (exclue) de la période qui contient la date, avec son
/// libellé.
fn period_bounds(p: Period, serial: f64, lang: Lang) -> (f64, f64, String) {
    let (y, m, _) = serial_to_ymd(serial).unwrap_or((1970, 1, 1));
    match p {
        Period::Week => {
            let day = serial.floor();
            let start = day - (day - 2.0).rem_euclid(7.0);
            let (wy, w) = iso_week(day);
            (start, start + 7.0, format!("{}{w:02} {wy}", loc(lang, "S")))
        }
        Period::Month => (
            date_serial(y, m as i32, 1),
            date_serial(y, m as i32 + 1, 1),
            format!("{} {y}", month_name(lang, m)),
        ),
        Period::Quarter => {
            let q = (m - 1) / 3;
            (
                date_serial(y, (q * 3 + 1) as i32, 1),
                date_serial(y, (q * 3 + 4) as i32, 1),
                format!("{}{} {y}", loc(lang, "T"), q + 1),
            )
        }
        Period::Year => (
            date_serial(y, 1, 1),
            date_serial(y + 1, 1, 1),
            y.to_string(),
        ),
    }
}

/// Une date en formule (`DATE(2026;3;1)`).
fn date_formula(serial: f64) -> String {
    match serial_to_ymd(serial) {
        Some((y, m, d)) => format!("DATE({y};{m};{d})"),
        None => format!("{serial}"),
    }
}

/// Les colonnes de données : titre éventuel et position.
struct Table {
    first_row: u32,
    last_row: u32,
    /// (index de colonne, titre).
    columns: Vec<(u32, Option<String>)>,
}

impl Workbook {
    fn question_table(&self, sheet: usize) -> Option<Table> {
        if sheet >= self.sheets.len() {
            return None;
        }
        let (rows, cols) = self.used_size(sheet);
        if rows == 0 || cols == 0 {
            return None;
        }
        let rows = rows.min(100_000);
        let cols = cols.min(1_000);
        // Les lignes de titre du haut (une seule cellule remplie, ou vides)
        // sont sautées : le tableau commence à la première ligne qui a au
        // moins deux cellules.
        let filled_in = |r: u32| {
            (0..cols)
                .filter(|&c| !self.value(sheet, CellRef::new(r, c)).is_empty())
                .take(2)
                .count()
        };
        let top = (0..rows.min(30)).find(|&r| filled_in(r) >= 2).unwrap_or(0);
        let row0: Vec<Value> = (0..cols)
            .map(|c| self.value(sheet, CellRef::new(top, c)))
            .collect();
        let filled = row0.iter().filter(|v| !v.is_empty()).count();
        let header = rows >= top + 2
            && filled > 0
            && row0
                .iter()
                .all(|v| matches!(v, Value::Empty | Value::Text(_)));
        let first_row = top + u32::from(header);
        let columns = (0..cols)
            .filter(|&c| {
                (first_row..rows).any(|r| !self.value(sheet, CellRef::new(r, c)).is_empty())
            })
            .map(|c| {
                let title = if header {
                    let t = self.display(sheet, CellRef::new(top, c));
                    (!t.trim().is_empty()).then_some(t)
                } else {
                    None
                };
                (c, title)
            })
            .collect();
        Some(Table {
            first_row,
            last_row: rows - 1,
            columns,
        })
    }

    fn column_is_numeric(&self, sheet: usize, t: &Table, col: u32) -> bool {
        let (mut n, mut s) = (0, 0);
        for r in t.first_row..=t.last_row {
            match self.value(sheet, CellRef::new(r, col)) {
                Value::Number(_) => n += 1,
                Value::Text(_) => s += 1,
                _ => {}
            }
        }
        n > 0 && n >= s
    }

    fn column_is_date(&self, sheet: usize, t: &Table, col: u32) -> bool {
        let (mut d, mut other) = (0, 0);
        for r in t.first_row..=t.last_row.min(t.first_row + 500) {
            let at = CellRef::new(r, col);
            match self.value(sheet, at) {
                Value::Number(_) if self.format(sheet, at).number == NumberFormat::Date => d += 1,
                Value::Empty => {}
                _ => other += 1,
            }
        }
        d > 0 && d >= other
    }

    /// Colonnes citées dans la question (titre, « colonne B » ou lettre
    /// isolée), dans l'ordre où elles apparaissent.
    fn mentioned_columns(&self, t: &Table, ws: &[String], question: &str) -> Vec<u32> {
        let mut found: Vec<(usize, usize, u32)> = t
            .columns
            .iter()
            .filter_map(|(c, title)| {
                let tw = words(title.as_ref()?);
                find_seq(ws, &tw).map(|p| (p, usize::MAX - tw.len(), *c))
            })
            .collect();
        found.sort();
        let mut out: Vec<u32> = Vec::new();
        for (_, _, c) in found {
            if !out.contains(&c) {
                out.push(c);
            }
        }
        if let Some(i) = ws.iter().position(|w| COLUMN_WORDS.contains(&w.as_str()))
            && let Some(letter) = ws.get(i + 1)
            && letter.len() <= 2
            && let Some(c) = col_index(&letter.to_uppercase())
            && t.columns.iter().any(|(cc, _)| *cc == c)
            && !out.contains(&c)
        {
            out.push(c);
        }
        for w in question.split(|c: char| !c.is_alphanumeric()) {
            if w.len() == 1
                && w.chars().all(|c| c.is_ascii_uppercase())
                && let Some(c) = col_index(w)
                && t.columns.iter().any(|(cc, _)| *cc == c)
                && !out.contains(&c)
            {
                out.push(c);
            }
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn ask_special(
        &self,
        sheet: usize,
        table: &Table,
        ws: &[String],
        question: &str,
        operation: Operation,
        sp: Special,
        lang: Lang,
    ) -> Result<Answer, QuestionError> {
        let all_columns = || QuestionError::NoColumn {
            columns: table
                .columns
                .iter()
                .map(|(c, t)| t.clone().unwrap_or_else(|| col_name(*c)))
                .collect(),
        };
        let title_of = |c: u32| {
            table
                .columns
                .iter()
                .find(|(cc, _)| *cc == c)
                .and_then(|(_, t)| t.clone())
                .unwrap_or_else(|| col_name(c))
        };
        let mentioned = self.mentioned_columns(table, ws, question);
        let dates: Vec<u32> = table
            .columns
            .iter()
            .map(|(c, _)| *c)
            .filter(|c| self.column_is_date(sheet, table, *c))
            .collect();
        let numbers: Vec<u32> = table
            .columns
            .iter()
            .map(|(c, _)| *c)
            .filter(|c| !dates.contains(c) && self.column_is_numeric(sheet, table, *c))
            .collect();
        let value_col = mentioned
            .iter()
            .copied()
            .find(|c| numbers.contains(c))
            .or_else(|| (numbers.len() == 1).then(|| numbers[0]));
        if operation != Operation::Count && value_col.is_none() {
            return Err(all_columns());
        }
        let (a, b) = (table.first_row + 1, table.last_row + 1);
        let abs = |c: u32| {
            let n = col_name(c);
            format!("${n}${a}:${n}${b}")
        };
        let mut answer = Answer::empty(operation);
        answer.value_header = value_col.map(title_of);

        // Classement : les N plus grands groupes d'une colonne de texte.
        if let Some(n) = sp.top
            && sp.period.is_none()
        {
            if matches!(
                operation,
                Operation::Max | Operation::Min | Operation::Median
            ) {
                return Err(QuestionError::GroupedUnsupported);
            }
            let g = mentioned
                .iter()
                .copied()
                .find(|c| Some(*c) != value_col && !numbers.contains(c) && !dates.contains(c))
                .or_else(|| {
                    table.columns.iter().map(|(c, _)| *c).find(|c| {
                        Some(*c) != value_col && !numbers.contains(c) && !dates.contains(c)
                    })
                })
                .ok_or_else(all_columns)?;
            let mut totals: Vec<(String, f64, usize)> = Vec::new();
            for r in table.first_row..=table.last_row {
                let key = self.display(sheet, CellRef::new(r, g));
                if key.trim().is_empty() {
                    continue;
                }
                let x = value_col
                    .and_then(|c| match self.value(sheet, CellRef::new(r, c)) {
                        Value::Number(x) if x.is_finite() => Some(x),
                        _ => None,
                    })
                    .unwrap_or(0.0);
                match totals.iter_mut().find(|(k, _, _)| *k == key) {
                    Some(e) => {
                        e.1 += x;
                        e.2 += 1;
                    }
                    None => {
                        if totals.len() >= 10_000 {
                            return Err(QuestionError::TooManyGroups);
                        }
                        totals.push((key, x, 1));
                    }
                }
            }
            if totals.is_empty() {
                return Err(QuestionError::NoData);
            }
            let score = |e: &(String, f64, usize)| match (operation, value_col) {
                (Operation::Average, Some(_)) => e.1 / e.2 as f64,
                (Operation::Count, _) | (_, None) => e.2 as f64,
                _ => e.1,
            };
            totals.sort_by(|x, y| score(y).total_cmp(&score(x)));
            totals.truncate(n);
            answer.groups = totals
                .into_iter()
                .map(|(k, _, _)| {
                    let f = match (operation, value_col) {
                        (Operation::Average, Some(c)) => {
                            format!("=MOYENNE.SI({};{{G}};{})", abs(g), abs(c))
                        }
                        (Operation::Count, _) | (_, None) => format!("=NB.SI({};{{G}})", abs(g)),
                        (_, Some(c)) => format!("=SOMME.SI({};{{G}};{})", abs(g), abs(c)),
                    };
                    (k, f)
                })
                .collect();
            answer.group_header = Some(title_of(g));
            answer.chart_rows = answer.groups.len();
            if operation == Operation::Count || value_col.is_none() {
                answer.operation = Operation::Count;
                answer.value_header = None;
            }
            return Ok(answer);
        }

        // Périodes, évolution, comparaison d'années : une colonne de dates.
        let d = mentioned
            .iter()
            .copied()
            .find(|c| dates.contains(c))
            .or_else(|| dates.first().copied())
            .ok_or(QuestionError::NoDateColumn)?;
        if matches!(
            operation,
            Operation::Max | Operation::Min | Operation::Median
        ) {
            return Err(QuestionError::GroupedUnsupported);
        }
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for r in table.first_row..=table.last_row {
            if let Value::Number(x) = self.value(sheet, CellRef::new(r, d))
                && serial_to_ymd(x).is_some()
            {
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
        if !lo.is_finite() {
            return Err(QuestionError::NoData);
        }
        let mut periods: Vec<(f64, f64, String)> = Vec::new();
        if let (Some((y1, y2)), None) = (sp.years, sp.period) {
            for y in [y1, y2] {
                periods.push((
                    date_serial(y, 1, 1),
                    date_serial(y + 1, 1, 1),
                    y.to_string(),
                ));
            }
        } else {
            let p = sp.period.unwrap_or(Period::Month);
            answer.period = Some(p);
            let mut cur = lo;
            while cur <= hi {
                let (start, end, label) = period_bounds(p, cur, lang);
                if periods.len() >= MAX_GROUPS {
                    return Err(QuestionError::TooManyGroups);
                }
                periods.push((start, end, label));
                if end <= cur {
                    break;
                }
                cur = end;
            }
        }
        let crit = |start: f64, end: f64| {
            format!(
                "{dr};\">=\"&{s};{dr};\"<\"&{e}",
                dr = abs(d),
                s = date_formula(start),
                e = date_formula(end)
            )
        };
        answer.groups = periods
            .iter()
            .map(|(start, end, label)| {
                let c = crit(*start, *end);
                let f = match (operation, value_col) {
                    (Operation::Average, Some(v)) => format!(
                        "=SI(NB.SI.ENS({c})=0;\"\";SOMME.SI.ENS({vr};{c})/NB.SI.ENS({c}))",
                        vr = abs(v)
                    ),
                    (Operation::Count, _) | (_, None) => format!("=NB.SI.ENS({c})"),
                    (_, Some(v)) => format!("=SOMME.SI.ENS({};{c})", abs(v)),
                };
                (label.clone(), f)
            })
            .collect();
        if operation == Operation::Count || value_col.is_none() {
            answer.operation = Operation::Count;
            answer.value_header = None;
        }
        answer.group_header = Some(title_of(d));
        answer.chart_rows = answer.groups.len();
        if sp.years.is_some() && sp.period.is_none() {
            answer
                .groups
                .push((loc(lang, "Écart").to_string(), "={V-1}-{V-2}".to_string()));
            answer.groups.push((
                loc(lang, "Évolution").to_string(),
                "=SI({V-3}=0;\"\";{V-2}/{V-3}-1)".to_string(),
            ));
            answer.percent_rows = vec![3];
            answer.chart_rows = 2;
        } else if sp.evolution {
            let n = answer.groups.len();
            let extra = (0..n)
                .map(|i| {
                    if i == 0 {
                        String::new()
                    } else {
                        "=SI(OU({V-1}=\"\";{V-1}=0);\"\";{V}/{V-1}-1)".to_string()
                    }
                })
                .collect();
            answer.extra = Some((loc(lang, "Évolution").to_string(), extra));
        } else if operation != Operation::Average {
            answer.total = Some(String::new());
        }
        Ok(answer)
    }

    /// Traduit `question` en formules, d'après les données de `sheet`
    /// (libellés des périodes en français).
    pub fn ask(&self, sheet: usize, question: &str) -> Result<Answer, QuestionError> {
        self.ask_in(sheet, question, Lang::Fr)
    }

    /// Comme [`Workbook::ask`], avec les libellés (mois, « Évolution »…) dans
    /// la langue donnée. Reconnaît aussi les périodes (« total par mois », « par
    /// trimestre », « par semaine », « par année »), les classements (« top 5
    /// des produits »), l'évolution (« évolution des ventes ») et les
    /// comparaisons d'années (« ventes 2025 vs 2026 »).
    pub fn ask_in(
        &self,
        sheet: usize,
        question: &str,
        lang: Lang,
    ) -> Result<Answer, QuestionError> {
        let question = question.trim();
        if question.is_empty() {
            return Err(QuestionError::Empty);
        }
        let table = self.question_table(sheet).ok_or(QuestionError::NoData)?;
        let ws = words(&question.chars().take(300).collect::<String>());
        let special = Special::detect(&ws);
        let operation = match (detect_operation(&ws), special.any()) {
            (Some((op, _)), _) => op,
            (None, true) => Operation::Sum,
            (None, false) => return Err(QuestionError::NoOperation),
        };
        if special.any() {
            return self.ask_special(sheet, &table, &ws, question, operation, special, lang);
        }
        let all_columns = || QuestionError::NoColumn {
            columns: table
                .columns
                .iter()
                .map(|(c, t)| t.clone().unwrap_or_else(|| col_name(*c)))
                .collect(),
        };

        // Mot de regroupement : « par », « by »…
        let group_at = ws
            .iter()
            .position(|w| GROUP_WORDS.contains(&w.as_str()))
            .filter(|&p| p > 0 && p + 1 < ws.len());
        let (before, after) = match group_at {
            Some(p) => (&ws[..p], &ws[p + 1..]),
            None => (&ws[..], &ws[..0]),
        };
        let original: Vec<&str> = question
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();

        let find_column = |part: &[String], raw_part_has_letters: bool| -> Option<u32> {
            // 1. Titre de colonne (le plus long l'emporte).
            let by_title = table
                .columns
                .iter()
                .filter_map(|(c, t)| {
                    let t = t.as_ref()?;
                    let tw = words(t);
                    find_seq(part, &tw).map(|_| (*c, tw.len()))
                })
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
                .map(|(c, _)| c);
            if by_title.is_some() {
                return by_title;
            }
            // 2. « colonne B ».
            if let Some(i) = part.iter().position(|w| COLUMN_WORDS.contains(&w.as_str()))
                && let Some(letter) = part.get(i + 1)
                && letter.len() <= 2
                && let Some(c) = col_index(&letter.to_uppercase())
                && table.columns.iter().any(|(cc, _)| *cc == c)
            {
                return Some(c);
            }
            // 3. Une lettre majuscule isolée, dans la question d'origine.
            if raw_part_has_letters {
                return original
                    .iter()
                    .filter(|w| w.len() == 1 && w.chars().all(|c| c.is_ascii_uppercase()))
                    .filter_map(|w| col_index(w))
                    .find(|c| table.columns.iter().any(|(cc, _)| cc == c));
            }
            None
        };

        let value_col = find_column(before, group_at.is_none());
        let group_col = if group_at.is_some() {
            find_column(after, true).filter(|g| Some(*g) != value_col)
        } else {
            None
        };
        if group_at.is_some() && group_col.is_none() {
            return Err(all_columns());
        }

        // Colonne calculée : celle citée, sinon la seule colonne de nombres.
        let numeric: Vec<u32> = table
            .columns
            .iter()
            .map(|(c, _)| *c)
            .filter(|c| {
                Some(*c) != group_col
                    && self.column_is_numeric(sheet, &table, *c)
                    && !self.column_is_date(sheet, &table, *c)
            })
            .collect();
        let value_col = match (value_col, operation) {
            (Some(c), _) => Some(c),
            (None, Operation::Count) => None,
            (None, _) if numeric.len() == 1 => Some(numeric[0]),
            (None, _) => return Err(all_columns()),
        };
        let title_of = |c: u32| {
            table
                .columns
                .iter()
                .find(|(cc, _)| *cc == c)
                .and_then(|(_, t)| t.clone())
                .unwrap_or_else(|| col_name(c))
        };
        let (a, b) = (table.first_row + 1, table.last_row + 1);
        let range = |c: u32| format!("{}{a}:{}{b}", col_name(c), col_name(c));
        let abs_range = |c: u32| {
            let n = col_name(c);
            format!("${n}${a}:${n}${b}")
        };
        let func = |op: Operation| match op {
            Operation::Sum => "SOMME",
            Operation::Average => "MOYENNE",
            Operation::Count => "NBVAL",
            Operation::Max => "MAX",
            Operation::Min => "MIN",
            Operation::Median => "MEDIANE",
        };

        let Some(g) = group_col else {
            // Résultat unique. Sans colonne citée, « combien » compte les lignes.
            let formula = match value_col {
                Some(c) => format!("={}({})", func(operation), range(c)),
                None => {
                    let first = table.columns.first().map(|(c, _)| *c).unwrap_or(0);
                    format!("=NBVAL({})", range(first))
                }
            };
            return Ok(Answer {
                operation,
                value_header: value_col.map(title_of),
                group_header: None,
                formula: Some(formula),
                groups: Vec::new(),
                total: None,
                ..Answer::empty(operation)
            });
        };

        if matches!(
            operation,
            Operation::Max | Operation::Min | Operation::Median
        ) {
            return Err(QuestionError::GroupedUnsupported);
        }
        // Valeurs distinctes de la colonne de regroupement, dans l'ordre.
        let mut seen: Vec<String> = Vec::new();
        for r in table.first_row..=table.last_row {
            let v = self.display(sheet, CellRef::new(r, g));
            if v.trim().is_empty() || seen.contains(&v) {
                continue;
            }
            if seen.len() >= MAX_GROUPS {
                return Err(QuestionError::TooManyGroups);
            }
            seen.push(v);
        }
        if seen.is_empty() {
            return Err(QuestionError::NoData);
        }
        // Les formules renvoient à la cellule du groupe : le tableau est écrit
        // à partir de `at`, la ligne i + 1 portant le groupe i (voir
        // [`Workbook::apply_answer`], qui réécrit les références).
        let n_groups = seen.len();
        let groups = seen
            .into_iter()
            .map(|v| {
                let formula = match (operation, value_col) {
                    (Operation::Sum, Some(c)) => {
                        format!("=SOMME.SI({};{{G}};{})", abs_range(g), abs_range(c))
                    }
                    (Operation::Average, Some(c)) => {
                        format!("=MOYENNE.SI({};{{G}};{})", abs_range(g), abs_range(c))
                    }
                    _ => format!("=NB.SI({};{{G}})", abs_range(g)),
                };
                (v, formula)
            })
            .collect();
        let counts =
            !matches!(operation, Operation::Sum | Operation::Average) || value_col.is_none();
        Ok(Answer {
            operation: if counts { Operation::Count } else { operation },
            value_header: if counts {
                None
            } else {
                value_col.map(title_of)
            },
            group_header: Some(title_of(g)),
            formula: None,
            groups,
            total: match (counts, operation) {
                (_, Operation::Average) if !counts => None,
                _ => Some(String::new()),
            },
            chart_rows: n_groups,
            ..Answer::empty(operation)
        })
    }

    /// Écrit la réponse à partir de `at` (titres en gras) et renvoie le
    /// nombre de lignes et de colonnes écrites. Les formules de groupe
    /// (`{G}`) et de total sont complétées avec les vraies cellules.
    pub fn apply_answer(
        &mut self,
        sheet: usize,
        at: CellRef,
        answer: &Answer,
        op_label: &str,
        total_label: &str,
    ) -> (u32, u32) {
        let mut cells = answer.cells(op_label, total_label);
        let grouped = answer.formula.is_none();
        let n_groups = answer.groups.len() as u32;
        let col = col_name(at.col);
        let value_col = col_name(at.col.saturating_add(1));
        for (i, row) in cells.iter_mut().enumerate() {
            if !grouped || i == 0 {
                continue;
            }
            let at_row = u64::from(at.row) + i as u64 + 1;
            if (i as u32) <= n_groups {
                let v = |d: u64| format!("{value_col}{}", at_row.saturating_sub(d));
                for f in row.iter_mut().skip(1) {
                    *f = f
                        .replace("{G}", &format!("{col}{at_row}"))
                        .replace("{V-1}", &v(1))
                        .replace("{V-2}", &v(2))
                        .replace("{V-3}", &v(3))
                        .replace("{V}", &v(0));
                }
            } else {
                row[1] = format!(
                    "=SOMME({value_col}{}:{value_col}{})",
                    u64::from(at.row) + 2,
                    at_row - 1
                );
            }
        }
        let width = cells.iter().map(Vec::len).max().unwrap_or(0) as u32;
        for (i, row) in cells.iter().enumerate() {
            for (j, text) in row.iter().enumerate() {
                let Some(cell) = at.offset(i as i64, j as i64) else {
                    continue;
                };
                self.set_input(sheet, cell, text);
                let mut f = self.format(sheet, cell);
                if (grouped && i == 0) || (!grouped && j == 0) {
                    f.bold = true;
                }
                let percent = grouped
                    && i > 0
                    && ((j == 2 && answer.extra.is_some())
                        || (j == 1 && answer.percent_rows.contains(&(i - 1))));
                if percent {
                    f.number = NumberFormat::Percent(1);
                }
                self.set_format(sheet, cell, f);
            }
        }
        (cells.len() as u32, width)
    }

    /// Premier emplacement libre pour un bloc de `rows` × `cols` : `from` si
    /// la zone est vide, sinon sous les données (une ligne vide d'écart).
    pub fn free_spot(&self, sheet: usize, from: CellRef, rows: u32, cols: u32) -> CellRef {
        let free = |at: CellRef| {
            at.row.saturating_add(rows) <= MAX_ROWS
                && at.col.saturating_add(cols) <= MAX_COLS
                && (0..rows).all(|r| {
                    (0..cols).all(|c| {
                        self.input(sheet, CellRef::new(at.row + r, at.col + c))
                            .is_empty()
                    })
                })
        };
        if free(from) {
            return from;
        }
        let (used_rows, _) = self.used_size(sheet);
        let below = CellRef::new(used_rows + 1, from.col.min(MAX_COLS.saturating_sub(cols)));
        if free(below) { below } else { from }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ventes() -> Workbook {
        let mut wb = Workbook::new();
        let rows = [
            ["Région", "Produit", "Ventes"],
            ["Nord", "Stylo", "10"],
            ["Sud", "Cahier", "20"],
            ["Nord", "Cahier", "30"],
            ["Est", "Stylo", "40"],
        ];
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                wb.set_input(0, CellRef::new(r as u32, c as u32), v);
            }
        }
        wb
    }

    #[test]
    fn total_simple() {
        let wb = ventes();
        let a = wb.ask(0, "Quel est le total des ventes ?").unwrap();
        assert_eq!(a.operation, Operation::Sum);
        assert_eq!(a.formula.as_deref(), Some("=SOMME(C2:C5)"));
        assert_eq!(a.value_header.as_deref(), Some("Ventes"));
    }

    #[test]
    fn moyenne_par_lettre_et_sans_colonne() {
        let wb = ventes();
        assert_eq!(
            wb.ask(0, "moyenne de C").unwrap().formula.as_deref(),
            Some("=MOYENNE(C2:C5)")
        );
        // Une seule colonne de nombres : elle est devinée.
        assert_eq!(
            wb.ask(0, "average").unwrap().formula.as_deref(),
            Some("=MOYENNE(C2:C5)")
        );
        assert_eq!(
            wb.ask(0, "médiane des ventes").unwrap().formula.as_deref(),
            Some("=MEDIANE(C2:C5)")
        );
    }

    #[test]
    fn six_langues() {
        let wb = ventes();
        for q in [
            "total ventes",
            "sum of ventes",
            "suma de ventes",
            "Summe ventes",
            "somma ventes",
            "soma das ventes",
        ] {
            assert_eq!(
                wb.ask(0, q).unwrap().formula.as_deref(),
                Some("=SOMME(C2:C5)"),
                "{q}"
            );
        }
    }

    #[test]
    fn maximum_minimum_et_compte() {
        let wb = ventes();
        assert_eq!(
            wb.ask(0, "la plus grande vente: maximum des ventes")
                .unwrap()
                .formula
                .as_deref(),
            Some("=MAX(C2:C5)")
        );
        assert_eq!(
            wb.ask(0, "minimum ventes").unwrap().formula.as_deref(),
            Some("=MIN(C2:C5)")
        );
        assert_eq!(
            wb.ask(0, "combien de produits").unwrap().formula.as_deref(),
            Some("=NBVAL(B2:B5)")
        );
        assert_eq!(
            wb.ask(0, "combien de lignes").unwrap().formula.as_deref(),
            Some("=NBVAL(A2:A5)")
        );
    }

    #[test]
    fn regroupement_et_ecriture() {
        let mut wb = ventes();
        let a = wb.ask(0, "total des ventes par région").unwrap();
        assert_eq!(a.group_header.as_deref(), Some("Région"));
        assert_eq!(a.groups.len(), 3);
        let at = wb.free_spot(0, CellRef::new(0, 4), 5, 2);
        assert_eq!(at, CellRef::new(0, 4));
        let (rows, cols) = wb.apply_answer(0, at, &a, "Total", "Total");
        assert_eq!((rows, cols), (5, 2));
        // Nord = 10 + 30, Sud = 20, Est = 40, total = 100.
        assert_eq!(wb.display(0, CellRef::new(1, 4)), "Nord");
        assert_eq!(wb.display(0, CellRef::new(1, 5)), "40");
        assert_eq!(wb.display(0, CellRef::new(2, 5)), "20");
        assert_eq!(wb.display(0, CellRef::new(3, 5)), "40");
        assert_eq!(wb.display(0, CellRef::new(4, 5)), "100");
    }

    #[test]
    fn nombre_par_groupe_et_moyenne_par_groupe() {
        let mut wb = ventes();
        let a = wb.ask(0, "combien par produit").unwrap();
        wb.apply_answer(0, CellRef::new(0, 4), &a, "Nombre", "Total");
        assert_eq!(wb.display(0, CellRef::new(1, 4)), "Stylo");
        assert_eq!(wb.display(0, CellRef::new(1, 5)), "2");
        let b = wb.ask(0, "average ventes by produit").unwrap();
        assert!(b.total.is_none());
        wb.apply_answer(0, CellRef::new(0, 8), &b, "Moyenne", "Total");
        assert_eq!(wb.display(0, CellRef::new(1, 9)), "25");
    }

    #[test]
    fn erreurs_expliquees() {
        let wb = ventes();
        assert_eq!(wb.ask(0, "  ").unwrap_err(), QuestionError::Empty);
        assert_eq!(
            wb.ask(0, "bonjour").unwrap_err(),
            QuestionError::NoOperation
        );
        assert_eq!(
            wb.ask(0, "maximum par région").unwrap_err(),
            QuestionError::GroupedUnsupported
        );
        assert_eq!(
            wb.ask(0, "total par mois").unwrap_err(),
            QuestionError::NoDateColumn
        );
        assert_eq!(
            Workbook::new().ask(0, "total").unwrap_err(),
            QuestionError::NoData
        );
        assert_eq!(wb.ask(9, "total").unwrap_err(), QuestionError::NoData);
    }

    #[test]
    fn deux_colonnes_de_nombres_sans_choix() {
        let mut wb = ventes();
        for r in 1..5 {
            wb.set_input(0, CellRef::new(r, 3), "1");
        }
        wb.set_input(0, CellRef::new(0, 3), "Quantité");
        assert!(matches!(
            wb.ask(0, "total").unwrap_err(),
            QuestionError::NoColumn { .. }
        ));
        assert_eq!(
            wb.ask(0, "total quantité").unwrap().formula.as_deref(),
            Some("=SOMME(D2:D5)")
        );
    }

    #[test]
    fn emplacement_libre_evite_les_donnees() {
        let wb = ventes();
        let at = wb.free_spot(0, CellRef::new(1, 1), 2, 2);
        assert_eq!(at, CellRef::new(6, 1));
    }

    #[test]
    fn jamais_de_panique() {
        let wb = ventes();
        for q in [
            "par",
            "total par",
            "par total",
            "colonne",
            "total colonne Z",
            "total de Z",
            "💥 total 💥",
            &"total ".repeat(500),
        ] {
            let _ = wb.ask(0, q);
        }
    }
    fn journal() -> Workbook {
        let mut wb = Workbook::new();
        let rows = [
            ["Date", "Catégorie", "Ventes"],
            ["05/01/2025", "Livres", "100"],
            ["20/02/2025", "Jeux", "50"],
            ["10/01/2026", "Livres", "120"],
            ["15/01/2026", "Jeux", "30"],
            ["03/03/2026", "Musique", "80"],
            ["28/03/2026", "Livres", "10"],
        ];
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                wb.set_input(0, CellRef::new(r as u32, c as u32), v);
            }
        }
        wb
    }

    fn written(wb: &Workbook, at: CellRef, rows: u32, cols: u32) -> Vec<Vec<String>> {
        (0..rows)
            .map(|r| {
                (0..cols)
                    .map(|c| wb.display(0, CellRef::new(at.row + r, at.col + c)))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn total_par_mois_et_par_trimestre() {
        let mut wb = journal();
        let a = wb.ask_in(0, "total des ventes par mois", Lang::Fr).unwrap();
        assert_eq!(a.period, Some(Period::Month));
        // Janvier 2025 à mars 2026 : 15 mois.
        assert_eq!(a.groups.len(), 15);
        assert_eq!(a.chart_rows, 15);
        let at = CellRef::new(0, 5);
        let (rows, cols) = wb.apply_answer(0, at, &a, "Total", "Total");
        let t = written(&wb, at, rows, cols);
        assert_eq!(t[1], ["janvier 2025", "100"]);
        assert_eq!(t[13], ["janvier 2026", "150"]);
        assert_eq!(t[16], ["Total", "390"]);
        let wb = journal();
        let q = wb.ask_in(0, "sales by quarter", Lang::En).unwrap();
        assert_eq!(q.groups[0].0, "Q1 2025");
        assert_eq!(
            wb.ask_in(0, "Summe pro Jahr", Lang::De)
                .unwrap()
                .groups
                .len(),
            2
        );
        let w = wb.ask_in(0, "total par semaine", Lang::Fr).unwrap();
        assert_eq!(w.groups[0].0, "S01 2025");
        assert_eq!(
            wb.ask_in(0, "ventes mensuelles", Lang::Fr).unwrap().period,
            Some(Period::Month)
        );
        assert_eq!(
            wb.ask_in(0, "promedio por año", Lang::Es)
                .unwrap()
                .operation,
            Operation::Average
        );
    }

    #[test]
    fn top_evolution_et_comparaison() {
        let mut wb = journal();
        let top = wb.ask(0, "top 2 des catégories").unwrap();
        assert_eq!(top.groups.len(), 2);
        assert_eq!(top.groups[0].0, "Livres");
        assert_eq!(top.groups[1].0, "Jeux");
        let at = CellRef::new(0, 5);
        wb.apply_answer(0, at, &top, "Total", "Total");
        assert_eq!(wb.display(0, CellRef::new(1, 6)), "230");
        assert!(top.total.is_none());

        let mut wb = journal();
        let e = wb.ask(0, "évolution des ventes par année").unwrap();
        assert!(e.extra.is_some());
        let (rows, cols) = wb.apply_answer(0, at, &e, "Total", "Total");
        let t = written(&wb, at, rows, cols);
        assert_eq!(t[0][2], "Évolution");
        assert_eq!(t[1], ["2025", "150", ""]);
        assert_eq!(t[2], ["2026", "240", "60,0\u{a0}%"]);

        let mut wb = journal();
        let c = wb.ask(0, "ventes 2025 vs 2026").unwrap();
        assert_eq!(c.chart_rows, 2);
        let (rows, cols) = wb.apply_answer(0, at, &c, "Total", "Total");
        let t = written(&wb, at, rows, cols);
        assert_eq!(t[1], ["2025", "150"]);
        assert_eq!(t[2], ["2026", "240"]);
        assert_eq!(t[3], ["Écart", "90"]);
        assert_eq!(t[4], ["Évolution", "60,0\u{a0}%"]);
        assert_eq!(c.chart_range(at), Some(Range::new(at, CellRef::new(2, 6))));
        assert_eq!(
            journal()
                .ask(0, "moyenne par catégorie")
                .unwrap()
                .groups
                .len(),
            3
        );
    }

    #[test]
    fn fuzz_des_questions() {
        let wb = journal();
        let mots = [
            "total",
            "par",
            "mois",
            "top",
            "5",
            "2025",
            "vs",
            "2026",
            "évolution",
            "semaine",
            "moyenne",
            "Ventes",
            "B",
            "colonne",
            "max",
            "combien",
            "trimestre",
            "💥",
            "",
            "année",
            "9999999999",
            "0",
            "top0",
            "by",
            "week",
        ];
        let mut seed = 0x2545_f491u64;
        for _ in 0..3000 {
            let mut q = String::new();
            for _ in 0..(seed % 7 + 1) {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                q.push_str(mots[(seed % mots.len() as u64) as usize]);
                q.push(' ');
            }
            for l in Lang::ALL {
                let _ = wb.ask_in(0, &q, l);
            }
        }
    }

    #[test]
    fn titre_au_dessus_du_tableau() {
        let mut wb = journal();
        wb.insert_rows(0, 0, 2);
        wb.set_input(0, CellRef::new(0, 0), "Journal des ventes");
        let a = wb.ask(0, "total des ventes").unwrap();
        assert_eq!(a.formula.as_deref(), Some("=SOMME(C4:C9)"));
        assert_eq!(wb.ask(0, "total par mois").unwrap().groups.len(), 15);
    }
}
