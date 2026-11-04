//! Planning de Gantt : à partir d'une plage « tâche, début, fin » (ou
//! « tâche, début, durée »), une feuille avec un calendrier en jours ou en
//! semaines où les barres sont dessinées par mise en forme conditionnelle,
//! la colonne du jour marquée, et un graphique de Gantt.

use varium_core::Lang;

use crate::cond::{CompareOp, CondFormat, CondRule, CondStyle};
use crate::format::{CellFormat, HAlign, NumberFormat, serial_to_ymd};
use crate::i18n::{loc, month_name};
use crate::refs::{CellRef, MAX_COLS, Range, col_name, quote_sheet_name};
use crate::value::Value;
use crate::workbook::{SheetChart, Workbook};

/// Plus grand nombre de tâches lues.
pub const MAX_TASKS: usize = 500;
/// Au-delà de ce nombre de jours, le calendrier compte en semaines.
const MAX_DAYS: f64 = 92.0;
/// Plus grand nombre de colonnes du calendrier.
const MAX_CAL_COLS: u32 = 160;

/// Une tâche du planning.
#[derive(Clone, Debug, PartialEq)]
pub struct GanttTask {
    pub name: String,
    /// Début et fin (incluse), numéros de série.
    pub start: f64,
    pub end: f64,
}

/// Les colonnes reconnues d'une plage de planning (décalages depuis la
/// première colonne).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GanttColumns {
    pub task: u32,
    pub start: u32,
    /// Colonne de fin, ou de durée en jours.
    pub end: u32,
    pub end_is_duration: bool,
    /// La première ligne est une ligne de titres.
    pub header: bool,
}

/// Pourquoi le planning ne peut pas être créé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GanttError {
    /// Il faut une colonne de tâches, une de dates de début et une de fin ou
    /// de durée.
    Columns,
    /// Aucune tâche avec des dates valides.
    NoTasks,
}

impl GanttError {
    pub fn message(self, lang: Lang) -> &'static str {
        loc(
            lang,
            match self {
                GanttError::Columns => {
                    "Sélectionnez un tableau avec une colonne de tâches, une de dates de début et une de dates de fin (ou de durées)."
                }
                GanttError::NoTasks => "Aucune tâche avec des dates valides.",
            },
        )
    }
}

fn fold(s: &str) -> String {
    crate::workbook::collation_key(s)
}

fn is_any(h: &str, words: &[&str]) -> bool {
    let h = fold(h);
    words
        .iter()
        .any(|w| h.split(|c: char| !c.is_alphanumeric()).any(|x| x == *w))
}

const START_WORDS: [&str; 8] = [
    "debut",
    "start",
    "inicio",
    "beginn",
    "anfang",
    "inizio",
    "commencement",
    "begin",
];
const END_WORDS: [&str; 8] = [
    "fin", "end", "fim", "ende", "final", "echeance", "due", "termine",
];
const DURATION_WORDS: [&str; 10] = [
    "duree", "duration", "jours", "days", "duracion", "dauer", "tage", "durata", "giorni",
    "duracao",
];

impl Workbook {
    /// Reconnaît les colonnes d'une plage de planning : par les titres
    /// (« Début », « Fin », « Durée »… en six langues), sinon par le type
    /// des valeurs (texte, puis deux colonnes de dates ou une date et un
    /// nombre).
    pub fn gantt_columns(&self, sheet: usize, r: Range) -> Option<GanttColumns> {
        let cols = r.cols().min(64);
        if cols < 3 || r.rows() < 1 {
            return None;
        }
        let head = |c: u32| self.display(sheet, CellRef::new(r.start.row, r.start.col + c));
        let row1 = r.start.row + u32::from(r.rows() > 1);
        let kind = |c: u32| {
            let mut dates = 0;
            let mut nums = 0;
            let mut texts = 0;
            for row in row1..=r.end.row.min(row1 + 50) {
                let at = CellRef::new(row, r.start.col + c);
                match self.value(sheet, at) {
                    Value::Number(_) if self.format(sheet, at).number == NumberFormat::Date => {
                        dates += 1
                    }
                    Value::Number(_) => nums += 1,
                    Value::Text(_) => texts += 1,
                    _ => {}
                }
            }
            (dates, nums, texts)
        };
        let header = (0..cols).all(|c| {
            matches!(
                self.value(sheet, CellRef::new(r.start.row, r.start.col + c)),
                Value::Text(_) | Value::Empty
            )
        }) && r.rows() > 1;
        let by_name = |words: &[&str]| (0..cols).find(|&c| header && is_any(&head(c), words));
        let date_cols: Vec<u32> = (0..cols).filter(|&c| kind(c).0 > 0).collect();
        let start = by_name(&START_WORDS)
            .filter(|c| date_cols.contains(c))
            .or_else(|| date_cols.first().copied())?;
        let end_named = by_name(&END_WORDS).filter(|c| *c != start && date_cols.contains(c));
        let dur_named = by_name(&DURATION_WORDS).filter(|c| *c != start);
        let (end, end_is_duration) = match (end_named, dur_named) {
            (Some(e), _) => (e, false),
            (None, Some(d)) => (d, true),
            (None, None) => match date_cols.iter().find(|c| **c != start) {
                Some(e) => (*e, false),
                None => ((0..cols).find(|&c| c != start && kind(c).1 > 0)?, true),
            },
        };
        let task = (0..cols)
            .find(|&c| c != start && c != end && kind(c).2 > 0)
            .or_else(|| (0..cols).find(|&c| c != start && c != end))?;
        Some(GanttColumns {
            task,
            start,
            end,
            end_is_duration,
            header,
        })
    }

    /// Les tâches d'une plage de planning (dates invalides ignorées ; une fin
    /// avant le début est ramenée au début).
    pub fn gantt_tasks(&self, sheet: usize, r: Range) -> Vec<GanttTask> {
        let Some(g) = self.gantt_columns(sheet, r) else {
            return Vec::new();
        };
        let first = r.start.row + u32::from(g.header);
        let mut out = Vec::new();
        for row in first..=r.end.row {
            if out.len() >= MAX_TASKS {
                break;
            }
            let v = |c: u32| self.value(sheet, CellRef::new(row, r.start.col + c));
            let (Value::Number(start), Value::Number(e)) = (v(g.start), v(g.end)) else {
                continue;
            };
            if serial_to_ymd(start).is_none() || !e.is_finite() {
                continue;
            }
            let end = if g.end_is_duration {
                start + (e.clamp(1.0, 36_500.0) - 1.0).floor()
            } else {
                e
            };
            if serial_to_ymd(end).is_none() {
                continue;
            }
            out.push(GanttTask {
                name: self.display(sheet, CellRef::new(row, r.start.col + g.task)),
                start: start.floor(),
                end: end.max(start).floor(),
            });
        }
        out
    }

    /// Crée une feuille « Planning » à partir d'une plage de tâches : les
    /// tâches (reliées à la source par des formules), un calendrier dont les
    /// barres et la colonne du jour viennent de la mise en forme
    /// conditionnelle, et un graphique de Gantt. Renvoie l'index de la
    /// nouvelle feuille.
    pub fn make_gantt(&mut self, sheet: usize, r: Range, lang: Lang) -> Result<usize, GanttError> {
        let g = self.gantt_columns(sheet, r).ok_or(GanttError::Columns)?;
        let tasks = self.gantt_tasks(sheet, r);
        if tasks.is_empty() {
            return Err(GanttError::NoTasks);
        }
        let src_name = self.sheets[sheet].name.clone();
        let target = self.add_sheet();
        let mut name = loc(lang, "Planning").to_string();
        let mut k = 2;
        while !self.rename_sheet(target, &name) {
            name = format!("{} {k}", loc(lang, "Planning"));
            k += 1;
            if k > 999 {
                break;
            }
        }
        let prefix = format!("{}!", quote_sheet_name(&src_name));
        let first = r.start.row + u32::from(g.header);
        let rows: Vec<u32> = (first..=r.end.row)
            .filter(|row| {
                matches!(
                    self.value(sheet, CellRef::new(*row, r.start.col + g.start)),
                    Value::Number(_)
                )
            })
            .take(MAX_TASKS)
            .collect();
        let lo = tasks.iter().map(|t| t.start).fold(f64::INFINITY, f64::min);
        let hi = tasks
            .iter()
            .map(|t| t.end)
            .fold(f64::NEG_INFINITY, f64::max);
        let weeks = hi - lo + 1.0 > MAX_DAYS;
        // Le calendrier commence un lundi.
        let monday = |d: f64| d - (d - 2.0).rem_euclid(7.0);
        let cal_start = if weeks { monday(lo) } else { lo };
        let step = if weeks { 7.0 } else { 1.0 };
        let n_cols = (((hi - cal_start) / step).floor() as u32 + 1).clamp(1, MAX_CAL_COLS);
        let ctx = |wb: &mut Workbook, at: &str, input: &str| {
            if let Some(c) = CellRef::parse(at) {
                wb.set_input(target, c, input);
            }
        };
        ctx(
            self,
            "A1",
            &format!("{} — {src_name}", loc(lang, "Planning")),
        );
        ctx(self, "A2", loc(lang, "Début du calendrier"));
        self.put_value(
            target,
            CellRef::new(1, 2),
            &Value::Number(cal_start),
            CellFormat {
                number: NumberFormat::Date,
                ..CellFormat::default()
            },
        );
        ctx(
            self,
            "D2",
            if weeks {
                loc(lang, "Semaines")
            } else {
                loc(lang, "Jours")
            },
        );
        let head_row = 3u32;
        let heads = [
            loc(lang, "Tâche"),
            loc(lang, "Début"),
            loc(lang, "Fin"),
            loc(lang, "Durée (j)"),
        ];
        for (c, h) in heads.iter().enumerate() {
            self.set_input(target, CellRef::new(head_row, c as u32), h);
        }
        let col0 = 4u32;
        let mut last_month = None;
        for k in 0..n_cols {
            let col = col0 + k;
            if col >= MAX_COLS {
                break;
            }
            let date = cal_start + f64::from(k) * step;
            if let Some((y, m, _)) = serial_to_ymd(date)
                && last_month != Some((y, m))
            {
                last_month = Some((y, m));
                self.set_input(
                    target,
                    CellRef::new(head_row - 1, col),
                    &crate::import::text_input(&format!("{} {y}", month_name(lang, m))),
                );
            }
            let off = f64::from(k) * step;
            self.set_input(
                target,
                CellRef::new(head_row, col),
                &format!(
                    "=JOUR($C$2+{})",
                    crate::import::number_input(off, NumberFormat::General)
                ),
            );
            self.sheets[target].col_widths.insert(col, 24.0);
        }
        let last_col = (col0 + n_cols - 1).min(MAX_COLS - 1);
        for (i, src_row) in rows.iter().enumerate() {
            let row = head_row + 1 + i as u32;
            let n = row + 1;
            let src = |c: u32| format!("={prefix}{}{}", col_name(r.start.col + c), src_row + 1);
            self.set_input(target, CellRef::new(row, 0), &src(g.task));
            self.set_input(target, CellRef::new(row, 1), &src(g.start));
            if g.end_is_duration {
                self.set_input(
                    target,
                    CellRef::new(row, 2),
                    &format!(
                        "=B{n}+MAX(1;{}{}{})-1",
                        prefix,
                        col_name(r.start.col + g.end),
                        src_row + 1
                    ),
                );
            } else {
                self.set_input(target, CellRef::new(row, 2), &src(g.end));
            }
            self.set_input(target, CellRef::new(row, 3), &format!("=C{n}-B{n}+1"));
            for c in [1, 2] {
                let at = CellRef::new(row, c);
                let mut f = self.format(target, at);
                f.number = NumberFormat::Date;
                self.set_format(target, at, f);
            }
            for k in 0..n_cols {
                let col = col0 + k;
                if col >= MAX_COLS {
                    break;
                }
                let off = crate::import::number_input(f64::from(k) * step, NumberFormat::General);
                let day = format!("($C$2+{off})");
                let (in_bar, today) = if weeks {
                    (
                        format!("ET({day}<=$C{n};{day}+6>=$B{n})"),
                        format!("ET(AUJOURDHUI()>={day};AUJOURDHUI()<{day}+7)"),
                    )
                } else {
                    (
                        format!("ET({day}>=$B{n};{day}<=$C{n})"),
                        format!("{day}=AUJOURDHUI()"),
                    )
                };
                self.set_input(
                    target,
                    CellRef::new(row, col),
                    &format!("=SI({today};SI({in_bar};2;3);SI({in_bar};1;\"\"))"),
                );
            }
        }
        let last_row = head_row + rows.len() as u32;
        // Mise en forme : titres, barres, colonne du jour.
        let bold = |wb: &mut Workbook, at: CellRef, fill: Option<[u8; 3]>| {
            let mut f = wb.format(target, at);
            f.bold = true;
            f.fill = fill;
            f.align = Some(HAlign::Center);
            wb.set_format(target, at, f);
        };
        bold(self, CellRef::new(0, 0), None);
        for c in 0..=last_col {
            bold(self, CellRef::new(head_row, c), Some([221, 235, 247]));
        }
        self.sheets[target].col_widths.insert(0, 180.0);
        self.sheets[target].frozen_rows = head_row + 1;
        self.sheets[target].frozen_cols = col0;
        let cal = Range::new(
            CellRef::new(head_row + 1, col0),
            CellRef::new(last_row.max(head_row + 1), last_col),
        );
        let paint = |value: &str, fill: [u8; 3]| CondFormat {
            range: cal,
            rule: CondRule::Compare {
                op: CompareOp::Eq,
                value: value.into(),
                value2: String::new(),
            },
            style: CondStyle {
                fill: Some(fill),
                color: Some(fill),
                ..CondStyle::default()
            },
        };
        self.sheets[target].cond_formats = vec![
            paint("1", [0x5B, 0x8F, 0xD9]),
            paint("2", [0xC0, 0x39, 0x2B]),
            paint("3", [0xF8, 0xD7, 0xD3]),
        ];
        self.sheets[target].charts.push(SheetChart {
            kind: "gantt".into(),
            range: Range::new(CellRef::new(head_row, 0), CellRef::new(last_row, 2)),
            title: loc(lang, "Planning").to_string(),
        });
        self.invalidate();
        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projet(fin: &str) -> Workbook {
        let mut wb = Workbook::new();
        let rows = [
            ["Tâche", "Début", fin],
            ["Cadrage", "02/03/2026", "06/03/2026"],
            ["Maquettes", "09/03/2026", "20/03/2026"],
            ["Recette", "23/03/2026", "27/03/2026"],
        ];
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                wb.set_input(0, CellRef::new(r as u32, c as u32), v);
            }
        }
        wb
    }

    #[test]
    fn colonnes_et_taches() {
        let wb = projet("Fin");
        let r = Range::parse("A1:C4").unwrap();
        let g = wb.gantt_columns(0, r).unwrap();
        assert_eq!(
            (g.task, g.start, g.end, g.end_is_duration),
            (0, 1, 2, false)
        );
        let t = wb.gantt_tasks(0, r);
        assert_eq!(t.len(), 3);
        assert_eq!(t[1].end - t[1].start, 11.0);
    }

    #[test]
    fn duree_au_lieu_de_fin() {
        let mut wb = projet("Durée");
        for (r, d) in [(1, "5"), (2, "10"), (3, "3")] {
            wb.set_input(0, CellRef::new(r, 2), d);
        }
        let r = Range::parse("A1:C4").unwrap();
        let g = wb.gantt_columns(0, r).unwrap();
        assert!(g.end_is_duration);
        assert_eq!(
            wb.gantt_tasks(0, r)[0].end - wb.gantt_tasks(0, r)[0].start,
            4.0
        );
    }

    #[test]
    fn feuille_planning() {
        let mut wb = projet("Fin");
        wb.set_today(Some(crate::format::date_serial(2026, 3, 10)));
        let s = wb
            .make_gantt(0, Range::parse("A1:C4").unwrap(), Lang::Fr)
            .unwrap();
        assert_eq!(wb.sheets[s].name, "Planning");
        assert_eq!(wb.display(s, CellRef::new(4, 0)), "Cadrage");
        assert_eq!(wb.display(s, CellRef::new(4, 3)), "5");
        // 2 mars : début du calendrier ; la tâche 1 couvre les 5 premiers jours.
        assert_eq!(wb.display(s, CellRef::new(3, 4)), "2");
        assert_eq!(wb.value(s, CellRef::new(4, 4)), Value::Number(1.0));
        assert_eq!(wb.value(s, CellRef::new(4, 9)), Value::Text(String::new()));
        // Le 10 mars (aujourd'hui), la tâche 2 est en cours : 2 ; ailleurs : 3.
        assert_eq!(wb.value(s, CellRef::new(5, 12)), Value::Number(2.0));
        assert_eq!(wb.value(s, CellRef::new(4, 12)), Value::Number(3.0));
        assert_eq!(wb.sheets[s].charts[0].kind, "gantt");
        let cp = wb.cond_paint(s, CellRef::new(4, 4)).unwrap();
        assert!(cp.fill.is_some());
        // Un deuxième planning prend un autre nom.
        let s2 = wb
            .make_gantt(0, Range::parse("A1:C4").unwrap(), Lang::En)
            .unwrap();
        assert_ne!(wb.sheets[s2].name, wb.sheets[s].name);
    }

    #[test]
    fn long_projet_en_semaines_et_erreurs() {
        let mut wb = projet("Fin");
        wb.set_input(0, CellRef::new(3, 2), "30/12/2026");
        let s = wb
            .make_gantt(0, Range::parse("A1:C4").unwrap(), Lang::Fr)
            .unwrap();
        assert_eq!(wb.display(s, CellRef::new(1, 3)), "Semaines");
        let mut empty = Workbook::new();
        assert_eq!(
            empty.make_gantt(0, Range::parse("A1:C4").unwrap(), Lang::Fr),
            Err(GanttError::Columns)
        );
        for r in ["A1:A1", "A1:Z9", "B2:C3"] {
            let _ = wb.make_gantt(0, Range::parse(r).unwrap(), Lang::It);
        }
    }
}
