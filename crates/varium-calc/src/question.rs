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

use crate::refs::{CellRef, MAX_COLS, MAX_ROWS, col_index, col_name};
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
        let mut rows = vec![vec![self.group_header.clone().unwrap_or_default(), title]];
        for (g, f) in &self.groups {
            rows.push(vec![safe_text(g), f.clone()]);
        }
        if let Some(t) = &self.total {
            rows.push(vec![total_label.to_string(), t.clone()]);
        }
        rows
    }
}

/// Un texte qui ressemblerait à une formule est protégé par une apostrophe.
fn safe_text(s: &str) -> String {
    if s.starts_with('=') || s.starts_with('\'') {
        format!("'{s}")
    } else {
        s.to_string()
    }
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
        let row0: Vec<Value> = (0..cols)
            .map(|c| self.value(sheet, CellRef::new(0, c)))
            .collect();
        let filled = row0.iter().filter(|v| !v.is_empty()).count();
        let header = rows >= 2
            && filled > 0
            && row0
                .iter()
                .all(|v| matches!(v, Value::Empty | Value::Text(_)));
        let first_row = u32::from(header);
        let columns = (0..cols)
            .filter(|&c| {
                (first_row..rows).any(|r| !self.value(sheet, CellRef::new(r, c)).is_empty())
            })
            .map(|c| {
                let title = if header {
                    let t = self.display(sheet, CellRef::new(0, c));
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

    /// Traduit `question` en formules, d'après les données de `sheet`.
    pub fn ask(&self, sheet: usize, question: &str) -> Result<Answer, QuestionError> {
        let question = question.trim();
        if question.is_empty() {
            return Err(QuestionError::Empty);
        }
        let table = self.question_table(sheet).ok_or(QuestionError::NoData)?;
        let ws = words(&question.chars().take(300).collect::<String>());
        let (operation, _) = detect_operation(&ws).ok_or(QuestionError::NoOperation)?;
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
            .filter(|c| Some(*c) != group_col && self.column_is_numeric(sheet, &table, *c))
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
        for (i, row) in cells.iter_mut().enumerate() {
            if !grouped || i == 0 {
                continue;
            }
            let at_row = at.row + i as u32 + 1;
            let col = col_name(at.col);
            if (i as u32) <= n_groups {
                row[1] = row[1].replace("{G}", &format!("{col}{at_row}"));
            } else {
                let value_col = col_name(at.col + 1);
                row[1] = format!(
                    "=SOMME({value_col}{}:{value_col}{})",
                    at.row + 2,
                    at_row - 1
                );
            }
        }
        let width = cells.iter().map(Vec::len).max().unwrap_or(0) as u32;
        for (i, row) in cells.iter().enumerate() {
            for (j, text) in row.iter().enumerate() {
                let cell = CellRef::new(at.row + i as u32, at.col + j as u32);
                self.set_input(sheet, cell, text);
                if (grouped && i == 0) || (!grouped && j == 0) {
                    let mut f = self.format(sheet, cell);
                    f.bold = true;
                    self.set_format(sheet, cell, f);
                }
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
        assert!(matches!(
            wb.ask(0, "total par mois").unwrap_err(),
            QuestionError::NoColumn { .. }
        ));
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
}
