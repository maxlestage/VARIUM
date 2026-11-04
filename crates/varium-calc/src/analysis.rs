//! Analyse rapide d'une sélection : forme des données (en-tête, étiquettes,
//! colonnes de nombres), totaux en un clic, mises en forme conditionnelles
//! usuelles et « Mettre sous forme de tableau ».

use crate::cond::{CondFormat, CondRule, CondStyle};
use crate::format::{CellFormat, NumberFormat};
use crate::refs::{CellRef, MAX_COLS, MAX_ROWS, Range, col_name};
use crate::value::Value;
use crate::workbook::Workbook;

/// Forme des données d'une plage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataShape {
    /// La première ligne est une ligne de titres.
    pub header: bool,
    /// La première colonne contient des étiquettes (texte).
    pub label_col: bool,
    /// Colonnes (de la feuille) qui contiennent des nombres.
    pub numeric_cols: Vec<u32>,
    /// Première et dernière lignes de données.
    pub first_row: u32,
    pub last_row: u32,
    /// Nombre de valeurs numériques.
    pub numbers: usize,
    /// Les étiquettes ressemblent à une suite temporelle (dates, années, mois).
    pub temporal: bool,
}

/// Totaux de l'analyse rapide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickTotal {
    /// Somme de chaque colonne, sur la ligne dessous.
    SumBelow,
    /// Somme de chaque ligne, dans la colonne à droite.
    SumRight,
    /// Moyenne de chaque colonne, dessous.
    AverageBelow,
    /// Nombre de valeurs de chaque colonne, dessous.
    CountBelow,
    /// Part de chaque valeur dans le total de sa colonne, à droite.
    PercentOfTotal,
}

/// Mises en forme conditionnelles de l'analyse rapide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickFormat {
    DataBars,
    ColorScale,
    Duplicates,
    Top10,
    /// Retire les règles posées sur la plage.
    Clear,
}

const MONTHS: [&str; 24] = [
    "janv", "fevr", "mars", "avr", "mai", "juin", "juil", "aout", "sept", "oct", "nov", "dec",
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

fn looks_temporal(labels: &[String], dates: bool) -> bool {
    if dates {
        return true;
    }
    if labels.len() < 2 {
        return false;
    }
    let fold = |s: &str| {
        s.to_lowercase()
            .replace(['é', 'è'], "e")
            .replace('û', "u")
            .replace('.', "")
    };
    let years = labels.iter().all(|l| {
        l.len() == 4
            && l.bytes().all(|b| b.is_ascii_digit())
            && (l.starts_with("19") || l.starts_with("20"))
    });
    let months = labels
        .iter()
        .all(|l| MONTHS.iter().any(|m| fold(l).starts_with(m)));
    let quarters = labels.iter().all(|l| {
        let u = l.to_uppercase();
        u.starts_with('T') || u.starts_with('Q')
    }) && labels.iter().all(|l| l.chars().any(|c| c.is_ascii_digit()));
    years || months || quarters
}

impl Workbook {
    /// Forme des données de la plage `r`.
    pub fn data_shape(&self, sheet: usize, r: Range) -> DataShape {
        let is_text = |row: u32, col: u32| {
            matches!(self.value(sheet, CellRef::new(row, col)), Value::Text(_))
        };
        let is_num = |row: u32, col: u32| {
            matches!(self.value(sheet, CellRef::new(row, col)), Value::Number(_))
        };
        let rows = r.start.row..=r.end.row.min(r.start.row + 100_000);
        let cols = r.start.col..=r.end.col.min(r.start.col + 1_000);
        let header = r.rows() > 1
            && cols.clone().any(|c| is_text(r.start.row, c))
            && cols
                .clone()
                .any(|c| (r.start.row + 1..=*rows.end()).any(|row| is_num(row, c)))
            && cols.clone().all(|c| !is_num(r.start.row, c));
        let first_row = if header { r.start.row + 1 } else { r.start.row };
        let last_row = *rows.end();
        let label_col = r.cols() > 1 && (first_row..=last_row).any(|row| is_text(row, r.start.col));
        let mut numeric_cols = Vec::new();
        let mut numbers = 0;
        for c in cols {
            if label_col && c == r.start.col {
                continue;
            }
            let n = (first_row..=last_row).filter(|&row| is_num(row, c)).count();
            if n > 0 {
                numeric_cols.push(c);
                numbers += n;
            }
        }
        let labels: Vec<String> = if label_col {
            (first_row..=last_row)
                .take(500)
                .map(|row| self.display(sheet, CellRef::new(row, r.start.col)))
                .collect()
        } else {
            Vec::new()
        };
        let dates = (label_col || numeric_cols.len() > 1)
            && (first_row..=last_row).take(500).all(|row| {
                self.format(sheet, CellRef::new(row, r.start.col)).number == NumberFormat::Date
            })
            && last_row > first_row;
        DataShape {
            header,
            label_col,
            numeric_cols,
            first_row,
            last_row,
            numbers,
            temporal: looks_temporal(&labels, dates),
        }
    }

    /// Ajoute des totaux à côté de la plage ; `label` est le titre écrit
    /// devant (« Total », « % du total »…). Renvoie la zone écrite.
    pub fn quick_total(
        &mut self,
        sheet: usize,
        r: Range,
        kind: QuickTotal,
        label: &str,
    ) -> Option<Range> {
        let shape = self.data_shape(sheet, r);
        if shape.numeric_cols.is_empty() || sheet >= self.sheets.len() {
            return None;
        }
        let (a, b) = (shape.first_row + 1, shape.last_row + 1);
        let bold = |wb: &mut Workbook, at: CellRef| {
            let mut f = wb.format(sheet, at);
            f.bold = true;
            wb.set_format(sheet, at, f);
        };
        match kind {
            QuickTotal::SumBelow | QuickTotal::AverageBelow | QuickTotal::CountBelow => {
                let row = shape.last_row + 1;
                if row >= MAX_ROWS {
                    return None;
                }
                let func = match kind {
                    QuickTotal::SumBelow => "SOMME",
                    QuickTotal::AverageBelow => "MOYENNE",
                    _ => "NB",
                };
                for &c in &shape.numeric_cols {
                    let col = col_name(c);
                    let at = CellRef::new(row, c);
                    self.set_input(sheet, at, &format!("={func}({col}{a}:{col}{b})"));
                    bold(self, at);
                }
                let label_at = CellRef::new(row, r.start.col);
                if !shape.numeric_cols.contains(&r.start.col)
                    && self.input(sheet, label_at).is_empty()
                {
                    self.set_input(sheet, label_at, label);
                    bold(self, label_at);
                }
                Some(Range::new(
                    CellRef::new(row, r.start.col),
                    CellRef::new(row, r.end.col),
                ))
            }
            QuickTotal::SumRight => {
                let col = r.end.col + 1;
                if col >= MAX_COLS {
                    return None;
                }
                let c0 = col_name(shape.numeric_cols[0]);
                let c1 = col_name(*shape.numeric_cols.last()?);
                for row in shape.first_row..=shape.last_row {
                    let n = row + 1;
                    let at = CellRef::new(row, col);
                    self.set_input(sheet, at, &format!("=SOMME({c0}{n}:{c1}{n})"));
                    bold(self, at);
                }
                if shape.header {
                    let at = CellRef::new(r.start.row, col);
                    self.set_input(sheet, at, label);
                    bold(self, at);
                }
                Some(Range::new(
                    CellRef::new(r.start.row, col),
                    CellRef::new(shape.last_row, col),
                ))
            }
            QuickTotal::PercentOfTotal => {
                let n = shape.numeric_cols.len() as u32;
                if r.end.col + n >= MAX_COLS {
                    return None;
                }
                for (k, &c) in shape.numeric_cols.iter().enumerate() {
                    let target = r.end.col + 1 + k as u32;
                    let src = col_name(c);
                    for row in shape.first_row..=shape.last_row {
                        let at = CellRef::new(row, target);
                        self.set_input(
                            sheet,
                            at,
                            &format!("={src}{}/SOMME({src}${a}:{src}${b})", row + 1),
                        );
                        let mut f = self.format(sheet, at);
                        f.number = NumberFormat::Percent(1);
                        self.set_format(sheet, at, f);
                    }
                    if shape.header {
                        let at = CellRef::new(r.start.row, target);
                        let head = self.display(sheet, CellRef::new(r.start.row, c));
                        let text = if n > 1 && !head.is_empty() {
                            format!("{label} ({head})")
                        } else {
                            label.to_string()
                        };
                        self.set_input(sheet, at, &text);
                        bold(self, at);
                    }
                }
                Some(Range::new(
                    CellRef::new(r.start.row, r.end.col + 1),
                    CellRef::new(shape.last_row, r.end.col + n),
                ))
            }
        }
    }

    /// Pose une mise en forme conditionnelle usuelle sur les données de la plage.
    pub fn quick_format(&mut self, sheet: usize, r: Range, kind: QuickFormat) {
        let shape = self.data_shape(sheet, r);
        let Some(sh) = self.sheets.get_mut(sheet) else {
            return;
        };
        let meets = |a: Range, b: Range| {
            a.start.row <= b.end.row
                && b.start.row <= a.end.row
                && a.start.col <= b.end.col
                && b.start.col <= a.end.col
        };
        if kind == QuickFormat::Clear {
            sh.cond_formats.retain(|cf| !meets(cf.range, r));
            return;
        }
        let numbers = match (shape.numeric_cols.first(), shape.numeric_cols.last()) {
            (Some(&a), Some(&b)) => Some(Range::new(
                CellRef::new(shape.first_row, a),
                CellRef::new(shape.last_row, b),
            )),
            _ => None,
        };
        let all = Range::new(
            CellRef::new(shape.first_row, r.start.col),
            CellRef::new(shape.last_row, r.end.col),
        );
        let (range, rule, style) = match kind {
            QuickFormat::DataBars => (
                numbers,
                CondRule::DataBar {
                    color: [0x63, 0x8E, 0xC6],
                },
                CondStyle::default(),
            ),
            QuickFormat::ColorScale => (
                numbers,
                CondRule::ColorScale {
                    min: [0xF8, 0x69, 0x6B],
                    mid: Some([0xFF, 0xEB, 0x84]),
                    max: [0x63, 0xBE, 0x7B],
                },
                CondStyle::default(),
            ),
            QuickFormat::Duplicates => (
                Some(all),
                CondRule::Duplicate { unique: false },
                CondStyle {
                    fill: Some([0xFF, 0xC7, 0xCE]),
                    color: Some([0x9C, 0x00, 0x06]),
                    ..CondStyle::default()
                },
            ),
            QuickFormat::Top10 => (
                numbers,
                CondRule::Top {
                    n: 10,
                    bottom: false,
                    percent: false,
                },
                CondStyle {
                    fill: Some([0xC6, 0xEF, 0xCE]),
                    color: Some([0x00, 0x61, 0x00]),
                    ..CondStyle::default()
                },
            ),
            QuickFormat::Clear => return,
        };
        if let Some(range) = range {
            sh.cond_formats.push(CondFormat { range, rule, style });
        }
    }

    /// « Mettre sous forme de tableau » : en-tête en gras sur fond coloré,
    /// lignes alternées et filtre automatique. Une cellule seule s'étend au
    /// tableau qui l'entoure. Renvoie la plage mise en forme.
    pub fn format_as_table(&mut self, sheet: usize, r: Range) -> Range {
        let r = if r.start == r.end {
            self.current_region(sheet, r.start)
        } else {
            r
        };
        let cols = r.start.col..=r.end.col.min(r.start.col + 1_000);
        for c in cols.clone() {
            let at = CellRef::new(r.start.row, c);
            let mut f = self.format(sheet, at);
            f.bold = true;
            f.fill = Some([0x2F, 0x5C, 0x8F]);
            f.color = Some([0xFF, 0xFF, 0xFF]);
            self.set_format(sheet, at, f);
        }
        for (i, row) in (r.start.row + 1..=r.end.row).take(100_000).enumerate() {
            for c in cols.clone() {
                let at = CellRef::new(row, c);
                let f = self.format(sheet, at);
                let banded = CellFormat {
                    fill: (i % 2 == 1).then_some([0xE4, 0xEC, 0xF7]),
                    ..f
                };
                self.set_format(sheet, at, banded);
            }
        }
        self.set_auto_filter(sheet, Some(r));
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Workbook {
        let mut wb = Workbook::new();
        let rows = [
            ["Mois", "Ventes", "Coûts"],
            ["Janvier", "10", "4"],
            ["Février", "20", "5"],
            ["Mars", "30", "6"],
        ];
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                wb.set_input(0, CellRef::new(r as u32, c as u32), v);
            }
        }
        wb
    }

    #[test]
    fn forme_des_donnees() {
        let wb = table();
        let s = wb.data_shape(0, Range::parse("A1:C4").unwrap());
        assert!(s.header && s.label_col && s.temporal);
        assert_eq!(s.numeric_cols, vec![1, 2]);
        assert_eq!((s.first_row, s.last_row, s.numbers), (1, 3, 6));
        let s = wb.data_shape(0, Range::parse("B2:B4").unwrap());
        assert!(!s.header && !s.label_col && !s.temporal);
    }

    #[test]
    fn totaux() {
        let mut wb = table();
        let r = Range::parse("A1:C4").unwrap();
        wb.quick_total(0, r, QuickTotal::SumBelow, "Total");
        assert_eq!(wb.input(0, CellRef::new(4, 1)), "=SOMME(B2:B4)");
        assert_eq!(wb.value(0, CellRef::new(4, 2)), Value::Number(15.0));
        assert_eq!(wb.input(0, CellRef::new(4, 0)), "Total");
        assert!(wb.format(0, CellRef::new(4, 1)).bold);
        let mut wb = table();
        wb.quick_total(0, r, QuickTotal::SumRight, "Total");
        assert_eq!(wb.input(0, CellRef::new(1, 3)), "=SOMME(B2:C2)");
        assert_eq!(wb.input(0, CellRef::new(0, 3)), "Total");
        let mut wb = table();
        wb.quick_total(0, r, QuickTotal::PercentOfTotal, "% du total");
        assert_eq!(wb.input(0, CellRef::new(1, 3)), "=B2/SOMME(B$2:B$4)");
        assert_eq!(wb.display(0, CellRef::new(3, 3)), "50,0\u{a0}%");
        assert_eq!(wb.input(0, CellRef::new(0, 4)), "% du total (Coûts)");
        let mut wb = table();
        wb.quick_total(0, r, QuickTotal::AverageBelow, "Moyenne");
        assert_eq!(wb.value(0, CellRef::new(4, 1)), Value::Number(20.0));
        assert!(
            wb.quick_total(0, Range::parse("A1:A4").unwrap(), QuickTotal::SumBelow, "x")
                .is_none()
        );
    }

    #[test]
    fn mises_en_forme_et_tableau() {
        let mut wb = table();
        let r = Range::parse("A1:C4").unwrap();
        wb.quick_format(0, r, QuickFormat::DataBars);
        assert_eq!(wb.sheets[0].cond_formats[0].range.name(), "B2:C4");
        wb.quick_format(0, r, QuickFormat::Duplicates);
        assert_eq!(wb.sheets[0].cond_formats[1].range.name(), "A2:C4");
        wb.quick_format(0, r, QuickFormat::Clear);
        assert!(wb.sheets[0].cond_formats.is_empty());
        let t = wb.format_as_table(0, Range::cell(CellRef::new(2, 1)));
        assert_eq!(t.name(), "A1:C4");
        assert!(wb.format(0, CellRef::new(0, 0)).bold);
        assert!(wb.format(0, CellRef::new(2, 0)).fill.is_some());
        assert!(wb.format(0, CellRef::new(1, 0)).fill.is_none());
        assert!(wb.sheets[0].auto_filter.is_some());
    }

    #[test]
    fn valeurs_en_double() {
        let mut wb = Workbook::new();
        for (i, v) in ["a", "B", "A", "c"].iter().enumerate() {
            wb.set_input(0, CellRef::new(i as u32, 0), v);
        }
        wb.quick_format(0, Range::parse("A1:A4").unwrap(), QuickFormat::Duplicates);
        let f = wb.cond_formatter(0);
        assert!(
            f.paint(CellRef::new(0, 0), &Value::Text("a".into()))
                .is_some()
        );
        assert!(
            f.paint(CellRef::new(2, 0), &Value::Text("A".into()))
                .is_some()
        );
        assert!(
            f.paint(CellRef::new(1, 0), &Value::Text("B".into()))
                .is_none()
        );
    }
}
