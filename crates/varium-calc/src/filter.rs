//! Filtre automatique : boutons dans la ligne d'en-tête d'un tableau, liste
//! des valeurs à cocher, conditions sur les nombres et les textes, tri depuis
//! l'en-tête. Les lignes filtrées sont masquées dans la grille, et le filtre est
//! enregistré dans le classeur (JSON), en XLSX (`<autoFilter>`) et en ODS
//! (`table:database-range`).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::cond::{CompareOp, compare_matches};
use crate::format::format_general;
use crate::refs::{CellRef, MAX_ROWS, Range};
use crate::value::Value;
use crate::workbook::{Sheet, Workbook};

/// Filtre automatique d'une feuille.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoFilter {
    /// Ligne d'en-tête et données (la fin s'étend aux lignes ajoutées dessous).
    #[serde(with = "crate::cond::range_serde")]
    pub range: Range,
    /// Filtres des colonnes (colonnes de la feuille).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<ColumnFilter>,
}

/// Filtre d'une colonne.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnFilter {
    /// Colonne de la feuille.
    pub col: u32,
    /// Valeurs affichées décochées (`""` : cellules vides).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<String>,
    /// Condition à vérifier en plus.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<FilterCondition>,
}

impl ColumnFilter {
    /// Le filtre ne retire rien ?
    pub fn is_empty(&self) -> bool {
        self.hidden.is_empty() && self.condition.is_none()
    }
}

/// Condition d'un filtre de colonne.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FilterCondition {
    /// Comparaison à une ou deux valeurs (nombres, dates ou textes).
    Compare {
        op: CompareOp,
        value: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        value2: String,
    },
    /// Le texte contient… (sans casse).
    Contains {
        text: String,
    },
    /// Le texte ne contient pas…
    NotContains {
        text: String,
    },
    BeginsWith {
        text: String,
    },
    EndsWith {
        text: String,
    },
    /// Les N plus grandes (ou plus petites) valeurs.
    Top {
        n: u32,
        #[serde(default)]
        bottom: bool,
    },
    AboveAverage,
    BelowAverage,
}

fn lower_text(v: &Value, display: &str) -> String {
    match v {
        Value::Number(n) if display.is_empty() => format_general(*n).to_lowercase(),
        _ => display.to_lowercase(),
    }
}

/// Statistiques d'une colonne utiles aux conditions (seuil, moyenne).
struct ColStats {
    threshold: Option<f64>,
    average: Option<f64>,
}

impl FilterCondition {
    fn test(&self, v: &Value, display: &str, stats: &ColStats) -> bool {
        let text = || lower_text(v, display);
        match self {
            FilterCondition::Compare { op, value, value2 } => {
                compare_matches(*op, value, value2, v)
            }
            FilterCondition::Contains { text: t } => text().contains(&t.to_lowercase()),
            FilterCondition::NotContains { text: t } => !text().contains(&t.to_lowercase()),
            FilterCondition::BeginsWith { text: t } => text().starts_with(&t.to_lowercase()),
            FilterCondition::EndsWith { text: t } => text().ends_with(&t.to_lowercase()),
            FilterCondition::Top { bottom, .. } => match (v, stats.threshold) {
                (Value::Number(x), Some(t)) => {
                    if *bottom {
                        *x <= t
                    } else {
                        *x >= t
                    }
                }
                _ => false,
            },
            FilterCondition::AboveAverage => {
                matches!((v, stats.average), (Value::Number(x), Some(m)) if *x > m)
            }
            FilterCondition::BelowAverage => {
                matches!((v, stats.average), (Value::Number(x), Some(m)) if *x < m)
            }
        }
    }
}

impl AutoFilter {
    /// Lignes de données (sans l'en-tête), étendues aux lignes remplies juste
    /// dessous.
    pub fn data_rows(&self, sheet: &Sheet) -> std::ops::RangeInclusive<u32> {
        let r = self.range;
        let mut end = r.end.row;
        let filled = |row: u32| {
            (r.start.col..=r.end.col).any(|c| {
                sheet
                    .cell(CellRef::new(row, c))
                    .is_some_and(|cell| !cell.input.is_empty())
            })
        };
        while end + 1 < MAX_ROWS && end < r.end.row + 100_000 && filled(end + 1) {
            end += 1;
        }
        (r.start.row + 1)..=end
    }

    /// Le filtre d'une colonne (vide s'il n'y en a pas).
    pub fn column(&self, col: u32) -> ColumnFilter {
        self.columns
            .iter()
            .find(|c| c.col == col)
            .cloned()
            .unwrap_or(ColumnFilter {
                col,
                ..ColumnFilter::default()
            })
    }

    /// Une colonne a-t-elle un filtre actif ?
    pub fn is_active(&self, col: u32) -> bool {
        self.columns.iter().any(|c| c.col == col && !c.is_empty())
    }
}

impl Workbook {
    /// Zone de cellules remplies autour de `at`, sans ligne ni colonne vide
    /// (comme Ctrl+* dans les tableurs). Une cellule seule si tout est vide.
    pub fn current_region(&self, sheet: usize, at: CellRef) -> Range {
        let Some(sh) = self.sheets.get(sheet) else {
            return Range::cell(at);
        };
        let filled = |r: u32, c: u32| {
            sh.cell(CellRef::new(r, c))
                .is_some_and(|cell| !cell.input.is_empty())
        };
        let (mut r0, mut r1, mut c0, mut c1) = (at.row, at.row, at.col, at.col);
        // On étend tant qu'une ligne ou colonne voisine touche la zone.
        for _ in 0..10_000 {
            let mut grown = false;
            if r0 > 0 && (c0.saturating_sub(1)..=c1 + 1).any(|c| filled(r0 - 1, c)) {
                r0 -= 1;
                grown = true;
            }
            if r1 + 1 < MAX_ROWS && (c0.saturating_sub(1)..=c1 + 1).any(|c| filled(r1 + 1, c)) {
                r1 += 1;
                grown = true;
            }
            if c0 > 0 && (r0.saturating_sub(1)..=r1 + 1).any(|r| filled(r, c0 - 1)) {
                c0 -= 1;
                grown = true;
            }
            if c1 + 1 < crate::refs::MAX_COLS
                && (r0.saturating_sub(1)..=r1 + 1).any(|r| filled(r, c1 + 1))
            {
                c1 += 1;
                grown = true;
            }
            if !grown {
                break;
            }
        }
        Range::new(CellRef::new(r0, c0), CellRef::new(r1, c1))
    }

    /// Pose (ou retire, avec `None`) le filtre automatique. Une plage d'une
    /// seule cellule s'étend au tableau qui l'entoure.
    pub fn set_auto_filter(&mut self, sheet: usize, range: Option<Range>) {
        let range = range.map(|r| {
            if r.start == r.end {
                self.current_region(sheet, r.start)
            } else {
                r
            }
        });
        if let Some(sh) = self.sheets.get_mut(sheet) {
            sh.auto_filter = range.map(|range| AutoFilter {
                range,
                columns: Vec::new(),
            });
        }
    }

    /// Remplace le filtre d'une colonne (un filtre vide le retire).
    pub fn set_column_filter(&mut self, sheet: usize, filter: ColumnFilter) {
        let Some(af) = self
            .sheets
            .get_mut(sheet)
            .and_then(|s| s.auto_filter.as_mut())
        else {
            return;
        };
        af.columns.retain(|c| c.col != filter.col);
        if !filter.is_empty() {
            af.columns.push(filter);
            af.columns.sort_by_key(|c| c.col);
        }
    }

    /// Valeurs affichées d'une colonne du filtre, triées, avec leur nombre
    /// (`""` pour les cellules vides).
    pub fn filter_values(&self, sheet: usize, col: u32) -> Vec<(String, usize)> {
        let Some(sh) = self.sheets.get(sheet) else {
            return Vec::new();
        };
        let Some(af) = &sh.auto_filter else {
            return Vec::new();
        };
        let mut counts: BTreeMap<String, (Value, usize)> = BTreeMap::new();
        for row in af.data_rows(sh) {
            let at = CellRef::new(row, col);
            let d = self.display(sheet, at);
            let e = counts
                .entry(d)
                .or_insert_with(|| (self.value(sheet, at), 0));
            e.1 += 1;
        }
        let mut out: Vec<(String, Value, usize)> =
            counts.into_iter().map(|(d, (v, n))| (d, v, n)).collect();
        out.sort_by(|a, b| crate::workbook::sort_order(&a.1, &b.1).then(a.0.cmp(&b.0)));
        out.into_iter().map(|(d, _, n)| (d, n)).collect()
    }

    /// Lignes masquées par le filtre automatique.
    pub fn filtered_rows(&self, sheet: usize) -> BTreeSet<u32> {
        let mut hidden = BTreeSet::new();
        let Some(sh) = self.sheets.get(sheet) else {
            return hidden;
        };
        let Some(af) = &sh.auto_filter else {
            return hidden;
        };
        let rows = af.data_rows(sh);
        for cf in af.columns.iter().filter(|c| !c.is_empty()) {
            let hidden_values: BTreeSet<&str> = cf.hidden.iter().map(String::as_str).collect();
            let mut nums: Vec<f64> = Vec::new();
            if matches!(
                cf.condition,
                Some(
                    FilterCondition::Top { .. }
                        | FilterCondition::AboveAverage
                        | FilterCondition::BelowAverage
                )
            ) {
                for row in rows.clone() {
                    if let Value::Number(x) = self.value(sheet, CellRef::new(row, cf.col))
                        && x.is_finite()
                    {
                        nums.push(x);
                    }
                }
            }
            let threshold = match &cf.condition {
                Some(FilterCondition::Top { n, bottom }) if !nums.is_empty() && *n > 0 => {
                    let mut v = nums.clone();
                    v.sort_by(|a, b| {
                        if *bottom {
                            a.total_cmp(b)
                        } else {
                            b.total_cmp(a)
                        }
                    });
                    Some(v[(*n as usize).min(v.len()) - 1])
                }
                _ => None,
            };
            let stats = ColStats {
                threshold,
                average: (!nums.is_empty()).then(|| nums.iter().sum::<f64>() / nums.len() as f64),
            };
            for row in rows.clone() {
                if hidden.contains(&row) {
                    continue;
                }
                let at = CellRef::new(row, cf.col);
                let d = self.display(sheet, at);
                let mut keep = !hidden_values.contains(d.as_str());
                if keep && let Some(cond) = &cf.condition {
                    keep = cond.test(&self.value(sheet, at), &d, &stats);
                }
                if !keep {
                    hidden.insert(row);
                }
            }
        }
        hidden
    }

    /// Trie les données du filtre automatique selon une colonne.
    pub fn sort_filter(&mut self, sheet: usize, col: u32, ascending: bool) {
        let Some((range, rows)) = self.sheets.get(sheet).and_then(|sh| {
            let af = sh.auto_filter.as_ref()?;
            Some((af.range, af.data_rows(sh)))
        }) else {
            return;
        };
        if rows.is_empty() {
            return;
        }
        let data = Range::new(
            CellRef::new(*rows.start(), range.start.col),
            CellRef::new(*rows.end(), range.end.col),
        );
        self.sort_rows(sheet, data, col, ascending);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Workbook {
        let mut wb = Workbook::new();
        let rows = [
            ["Ville", "Ventes"],
            ["Paris", "120"],
            ["Lyon", "80"],
            ["Paris", "45"],
            ["Nice", ""],
            ["Lille", "300"],
        ];
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                wb.set_input(0, CellRef::new(r as u32, c as u32), v);
            }
        }
        wb
    }

    #[test]
    fn region_et_pose_du_filtre() {
        let mut wb = table();
        assert_eq!(wb.current_region(0, CellRef::new(2, 1)).name(), "A1:B6");
        assert_eq!(wb.current_region(0, CellRef::new(20, 5)).name(), "F21");
        wb.set_auto_filter(0, Some(Range::cell(CellRef::new(0, 0))));
        let af = wb.sheets[0].auto_filter.clone().unwrap();
        assert_eq!(af.range.name(), "A1:B6");
        // Une ligne ajoutée dessous fait partie du filtre.
        wb.set_input(0, CellRef::new(6, 0), "Brest");
        assert_eq!(af.data_rows(&wb.sheets[0]), 1..=6);
        assert_eq!(
            wb.filter_values(0, 0),
            vec![
                ("Brest".to_string(), 1),
                ("Lille".to_string(), 1),
                ("Lyon".to_string(), 1),
                ("Nice".to_string(), 1),
                ("Paris".to_string(), 2)
            ]
        );
        assert_eq!(wb.filter_values(0, 1)[0], ("45".to_string(), 1));
    }

    #[test]
    fn valeurs_et_conditions() {
        let mut wb = table();
        wb.set_auto_filter(0, Some(Range::parse("A1:B6").unwrap()));
        wb.set_column_filter(
            0,
            ColumnFilter {
                col: 0,
                hidden: vec!["Paris".into()],
                condition: None,
            },
        );
        assert_eq!(wb.filtered_rows(0), BTreeSet::from([1, 3]));
        wb.set_column_filter(
            0,
            ColumnFilter {
                col: 0,
                ..Default::default()
            },
        );
        assert!(wb.filtered_rows(0).is_empty());
        assert!(
            wb.sheets[0]
                .auto_filter
                .as_ref()
                .unwrap()
                .columns
                .is_empty()
        );
        wb.set_column_filter(
            0,
            ColumnFilter {
                col: 1,
                hidden: vec![],
                condition: Some(FilterCondition::Compare {
                    op: CompareOp::Gt,
                    value: "100".into(),
                    value2: String::new(),
                }),
            },
        );
        assert_eq!(wb.filtered_rows(0), BTreeSet::from([2, 3, 4]));
        wb.set_column_filter(
            0,
            ColumnFilter {
                col: 1,
                hidden: vec![],
                condition: Some(FilterCondition::Top {
                    n: 2,
                    bottom: false,
                }),
            },
        );
        assert_eq!(wb.filtered_rows(0), BTreeSet::from([2, 3, 4]));
        wb.set_column_filter(
            0,
            ColumnFilter {
                col: 0,
                hidden: vec![],
                condition: Some(FilterCondition::Contains { text: "LI".into() }),
            },
        );
        assert_eq!(wb.filtered_rows(0), BTreeSet::from([1, 2, 3, 4]));
    }

    #[test]
    fn tri_et_lignes_inserees() {
        let mut wb = table();
        wb.set_auto_filter(0, Some(Range::parse("A1:B6").unwrap()));
        wb.sort_filter(0, 1, false);
        assert_eq!(wb.input(0, CellRef::new(0, 0)), "Ville");
        assert_eq!(wb.input(0, CellRef::new(1, 0)), "Lille");
        // Les cellules vides restent à la fin.
        assert_eq!(wb.input(0, CellRef::new(5, 0)), "Nice");
        wb.insert_rows(0, 0, 2);
        assert_eq!(
            wb.sheets[0].auto_filter.as_ref().unwrap().range.name(),
            "A3:B8"
        );
        wb.delete_rows(0, 2, 1);
        assert!(wb.sheets[0].auto_filter.is_none());
    }

    #[test]
    fn exports_xlsx_et_ods() {
        let mut wb = table();
        wb.set_auto_filter(0, Some(Range::parse("A1:B6").unwrap()));
        wb.set_column_filter(
            0,
            ColumnFilter {
                col: 0,
                hidden: vec!["Lyon".into()],
                condition: None,
            },
        );
        wb.set_column_filter(
            0,
            ColumnFilter {
                col: 1,
                hidden: vec![],
                condition: Some(FilterCondition::Compare {
                    op: CompareOp::Ge,
                    value: "50".into(),
                    value2: String::new(),
                }),
            },
        );
        let hidden = wb.filtered_rows(0);
        assert_eq!(hidden, BTreeSet::from([2, 3, 4]));
        let back = Workbook::from_xlsx("t", &wb.to_xlsx()).unwrap();
        assert_eq!(back.sheets[0].auto_filter, wb.sheets[0].auto_filter);
        assert_eq!(back.filtered_rows(0), hidden);
        let ods = wb.to_ods();
        let zip = varium_core::zip::ZipReader::new(&ods).unwrap();
        let content = zip.text("content.xml").unwrap();
        assert!(content.contains("table:display-filter-buttons=\"true\""));
        assert!(content.contains("<table:filter-set-item table:value=\"Paris\"/>"));
        assert!(content.contains("table:operator=\"&gt;=\""));
        assert!(content.contains("table:visibility=\"filter\""));
    }

    #[test]
    fn json_compatible() {
        let mut wb = table();
        let old = wb.to_json();
        assert!(!old.contains("auto_filter"));
        wb.set_auto_filter(0, Some(Range::parse("A1:B6").unwrap()));
        wb.set_column_filter(
            0,
            ColumnFilter {
                col: 0,
                hidden: vec!["Lyon".into()],
                condition: Some(FilterCondition::BeginsWith { text: "p".into() }),
            },
        );
        let back = Workbook::from_json(&wb.to_json()).unwrap();
        assert_eq!(back.sheets[0].auto_filter, wb.sheets[0].auto_filter);
        assert!(
            Workbook::from_json(&old).unwrap().sheets[0]
                .auto_filter
                .is_none()
        );
    }
}
