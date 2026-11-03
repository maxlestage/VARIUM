//! Mise en forme conditionnelle : règles posées sur une plage d'une feuille.
//!
//! # Ordre des règles
//!
//! Les règles d'une feuille sont lues dans l'ordre de la liste, comme dans
//! LibreOffice Calc :
//!
//! - pour les règles de **style** (comparaisons, texte, vide, erreur, N premiers,
//!   moyenne), **la première règle vérifiée l'emporte** : les suivantes sont
//!   ignorées pour cette cellule ;
//! - une **échelle de couleurs** donne le fond de la cellule (la première
//!   échelle applicable compte), sauf si une règle de style vérifiée a un fond :
//!   celui-ci l'emporte toujours ;
//! - une **barre de données** s'ajoute à ce qui précède (la première barre
//!   applicable est dessinée).
//!
//! Les comparaisons ne concernent que les cellules non vides : une valeur
//! numérique (`12`, `1,5`, `15/03/2026`, `20 %`) se compare aux nombres, un texte
//! aux textes (sans tenir compte de la casse).

use serde::{Deserialize, Serialize};

use crate::format::{format_general, parse_literal};
use crate::refs::{CellRef, Range};
use crate::value::Value;
use crate::workbook::Sheet;

/// Opérateur de comparaison d'une règle « valeur de la cellule ».
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompareOp {
    Gt,
    Ge,
    Lt,
    Le,
    Eq,
    Ne,
    Between,
    NotBetween,
}

impl CompareOp {
    /// Tous les opérateurs, dans l'ordre des menus.
    pub const ALL: [CompareOp; 8] = [
        CompareOp::Gt,
        CompareOp::Ge,
        CompareOp::Lt,
        CompareOp::Le,
        CompareOp::Eq,
        CompareOp::Ne,
        CompareOp::Between,
        CompareOp::NotBetween,
    ];

    /// Libellé français.
    pub fn label(self) -> &'static str {
        match self {
            CompareOp::Gt => "supérieure à (>)",
            CompareOp::Ge => "supérieure ou égale à (≥)",
            CompareOp::Lt => "inférieure à (<)",
            CompareOp::Le => "inférieure ou égale à (≤)",
            CompareOp::Eq => "égale à (=)",
            CompareOp::Ne => "différente de (≠)",
            CompareOp::Between => "comprise entre",
            CompareOp::NotBetween => "non comprise entre",
        }
    }

    /// L'opérateur a-t-il deux bornes ?
    pub fn two_values(self) -> bool {
        matches!(self, CompareOp::Between | CompareOp::NotBetween)
    }
}

/// Condition d'une règle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CondRule {
    /// La valeur de la cellule comparée à une ou deux valeurs saisies.
    Compare {
        op: CompareOp,
        value: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        value2: String,
    },
    /// Le texte contient… (sans casse).
    Contains { text: String },
    /// Le texte commence par… (sans casse).
    BeginsWith { text: String },
    /// Le texte finit par… (sans casse).
    EndsWith { text: String },
    /// Cellule vide (ou texte vide).
    Empty,
    /// Cellule non vide.
    NotEmpty,
    /// Valeur d'erreur (`#DIV/0!`…).
    Error,
    /// Les N plus grandes (ou plus petites) valeurs de la plage, ou N %.
    Top {
        n: u32,
        #[serde(default)]
        bottom: bool,
        #[serde(default)]
        percent: bool,
    },
    /// Au-dessus (ou au-dessous) de la moyenne de la plage.
    Average { above: bool },
    /// Échelle de 2 ou 3 couleurs (minimum, médiane, maximum).
    ColorScale {
        min: [u8; 3],
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mid: Option<[u8; 3]>,
        max: [u8; 3],
    },
    /// Barre de données proportionnelle à la valeur.
    DataBar { color: [u8; 3] },
}

impl CondRule {
    /// La règle donne-t-elle un style (et non une échelle ou une barre) ?
    pub fn has_style(&self) -> bool {
        !matches!(self, CondRule::ColorScale { .. } | CondRule::DataBar { .. })
    }

    /// Description courte, en français.
    pub fn describe(&self) -> String {
        match self {
            CondRule::Compare { op, value, value2 } => {
                if op.two_values() {
                    format!("Valeur {} {value} et {value2}", op.label())
                } else {
                    format!("Valeur {} {value}", op.label())
                }
            }
            CondRule::Contains { text } => format!("Le texte contient « {text} »"),
            CondRule::BeginsWith { text } => format!("Le texte commence par « {text} »"),
            CondRule::EndsWith { text } => format!("Le texte finit par « {text} »"),
            CondRule::Empty => "Cellule vide".into(),
            CondRule::NotEmpty => "Cellule non vide".into(),
            CondRule::Error => "Valeur d'erreur".into(),
            CondRule::Top { n, bottom, percent } => format!(
                "{} {n}{}",
                if *bottom {
                    "Les plus petites :"
                } else {
                    "Les plus grandes :"
                },
                if *percent { " %" } else { "" }
            ),
            CondRule::Average { above: true } => "Au-dessus de la moyenne".into(),
            CondRule::Average { above: false } => "Au-dessous de la moyenne".into(),
            CondRule::ColorScale { mid: Some(_), .. } => "Échelle de trois couleurs".into(),
            CondRule::ColorScale { .. } => "Échelle de deux couleurs".into(),
            CondRule::DataBar { .. } => "Barre de données".into(),
        }
    }
}

/// Style appliqué quand la condition est vraie.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct CondStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<[u8; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    #[serde(skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub italic: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl CondStyle {
    pub fn is_default(&self) -> bool {
        *self == CondStyle::default()
    }
}

/// Une règle de mise en forme conditionnelle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CondFormat {
    #[serde(with = "range_serde")]
    pub range: Range,
    pub rule: CondRule,
    #[serde(default, skip_serializing_if = "CondStyle::is_default")]
    pub style: CondStyle,
}

mod range_serde {
    use super::Range;
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(r: &Range, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&r.name())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Range, D::Error> {
        let s = String::deserialize(d)?;
        Range::parse(&s).ok_or_else(|| D::Error::custom(format!("plage invalide : {s}")))
    }
}

/// Ce que la mise en forme conditionnelle donne à une cellule.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CondPaint {
    pub fill: Option<[u8; 3]>,
    pub color: Option<[u8; 3]>,
    pub bold: bool,
    pub italic: bool,
    /// Barre de données : longueur (0 à 1) et couleur.
    pub bar: Option<(f32, [u8; 3])>,
}

/// Statistiques d'une plage, calculées une fois pour toutes les cellules.
#[derive(Clone, Copy, Debug)]
enum Stats {
    None,
    /// Seuil des N premiers (ou derniers) ; `None` si la plage n'a aucun nombre.
    Threshold(Option<f64>),
    Average(Option<f64>),
    Scale {
        lo: f64,
        mid: f64,
        hi: f64,
    },
    Empty,
}

/// Valeur saisie dans une règle : nombre (ou date, pourcentage) ou texte.
#[derive(Clone, Debug)]
enum Operand {
    Number(f64),
    Text(String),
}

fn operand(s: &str) -> Operand {
    match parse_literal(s.trim()).0 {
        Value::Number(n) => Operand::Number(n),
        Value::Bool(b) => Operand::Number(if b { 1.0 } else { 0.0 }),
        _ => Operand::Text(s.trim().to_lowercase()),
    }
}

struct Prepared {
    range: Range,
    rule: CondRule,
    style: CondStyle,
    stats: Stats,
    operands: (Operand, Operand),
}

/// Applique les règles d'une feuille ; préparé une fois (statistiques des
/// plages), puis interrogé cellule par cellule.
///
/// Ordre : la première règle de style vérifiée l'emporte ; la première échelle
/// de couleurs applicable donne le fond, sauf si cette règle de style a un fond ;
/// la première barre de données s'ajoute.
pub struct CondFormatter {
    rules: Vec<Prepared>,
}

/// Valeurs numériques présentes dans une plage.
fn numbers_in(sheet: &Sheet, range: Range, value: &dyn Fn(CellRef) -> Value) -> Vec<f64> {
    let mut out = Vec::new();
    for (at, _) in sheet.cells.range(range.start..=range.end) {
        if !range.contains(*at) {
            continue;
        }
        if let Value::Number(n) = value(*at)
            && n.is_finite()
        {
            out.push(n);
        }
    }
    out
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let pos = (sorted.len() - 1) as f64 * p.clamp(0.0, 1.0);
    let i = pos.floor() as usize;
    let j = (i + 1).min(sorted.len() - 1);
    sorted[i] + (sorted[j] - sorted[i]) * (pos - i as f64)
}

impl CondFormatter {
    pub(crate) fn new(sheet: &Sheet, value: &dyn Fn(CellRef) -> Value) -> Self {
        let rules = sheet
            .cond_formats
            .iter()
            .map(|cf| {
                let stats = match &cf.rule {
                    CondRule::Top { n, bottom, percent } => {
                        let mut v = numbers_in(sheet, cf.range, value);
                        v.sort_by(|a, b| b.total_cmp(a));
                        if *bottom {
                            v.reverse();
                        }
                        let count = if *percent {
                            ((v.len() as f64 * f64::from((*n).min(100)) / 100.0).floor() as usize)
                                .max(1)
                        } else {
                            *n as usize
                        };
                        if v.is_empty() || count == 0 {
                            Stats::Threshold(None)
                        } else {
                            Stats::Threshold(Some(v[count.min(v.len()) - 1]))
                        }
                    }
                    CondRule::Average { .. } => {
                        let v = numbers_in(sheet, cf.range, value);
                        if v.is_empty() {
                            Stats::Average(None)
                        } else {
                            Stats::Average(Some(v.iter().sum::<f64>() / v.len() as f64))
                        }
                    }
                    CondRule::ColorScale { .. } | CondRule::DataBar { .. } => {
                        let mut v = numbers_in(sheet, cf.range, value);
                        if v.is_empty() {
                            Stats::Empty
                        } else {
                            v.sort_by(f64::total_cmp);
                            Stats::Scale {
                                lo: v[0],
                                mid: percentile(&v, 0.5),
                                hi: v[v.len() - 1],
                            }
                        }
                    }
                    _ => Stats::None,
                };
                let operands = match &cf.rule {
                    CondRule::Compare { value, value2, .. } => (operand(value), operand(value2)),
                    _ => (Operand::Text(String::new()), Operand::Text(String::new())),
                };
                Prepared {
                    range: cf.range,
                    rule: cf.rule.clone(),
                    style: cf.style,
                    stats,
                    operands,
                }
            })
            .collect();
        CondFormatter { rules }
    }

    /// Aucune règle sur la feuille ?
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Mise en forme conditionnelle d'une cellule (`None` : aucune règle ne s'applique).
    pub fn paint(&self, at: CellRef, v: &Value) -> Option<CondPaint> {
        let mut out = CondPaint::default();
        let mut any = false;
        let mut styled = false;
        for p in &self.rules {
            if !p.range.contains(at) {
                continue;
            }
            match (&p.rule, p.stats) {
                (CondRule::ColorScale { min, mid, max }, Stats::Scale { lo, mid: m, hi }) => {
                    if out.fill.is_none()
                        && let Value::Number(x) = v
                    {
                        out.fill = Some(scale_color(*x, lo, m, hi, *min, *mid, *max));
                        any = true;
                    }
                }
                (CondRule::DataBar { color }, Stats::Scale { lo, hi, .. }) => {
                    if out.bar.is_none()
                        && let Value::Number(x) = v
                    {
                        let base = lo.min(0.0);
                        let span = hi.max(0.0) - base;
                        let frac = if span > 0.0 {
                            ((x - base) / span).clamp(0.0, 1.0)
                        } else {
                            1.0
                        };
                        out.bar = Some((frac as f32, *color));
                        any = true;
                    }
                }
                (rule, stats)
                    if !styled && rule.has_style() && matches(rule, stats, &p.operands, v) =>
                {
                    styled = true;
                    any = true;
                    if p.style.fill.is_some() {
                        out.fill = p.style.fill;
                    }
                    out.color = p.style.color;
                    out.bold = p.style.bold;
                    out.italic = p.style.italic;
                }
                _ => {}
            }
        }
        any.then_some(out)
    }
}

/// Texte d'une valeur pour les règles de texte (en minuscules).
fn value_text(v: &Value) -> Option<String> {
    match v {
        Value::Text(s) => Some(s.to_lowercase()),
        Value::Number(n) => Some(format_general(*n).to_lowercase()),
        Value::Bool(b) => Some(crate::format::bool_text(*b).to_lowercase()),
        Value::Error(e) => Some(e.code().to_lowercase()),
        Value::Empty => None,
    }
}

fn compare(v: &Value, o: &Operand) -> Option<std::cmp::Ordering> {
    match (v, o) {
        (Value::Number(x), Operand::Number(y)) => x.partial_cmp(y),
        (Value::Bool(b), Operand::Number(y)) => (if *b { 1.0 } else { 0.0 }).partial_cmp(y),
        (Value::Text(s), Operand::Text(t)) => Some(s.to_lowercase().cmp(t)),
        _ => None,
    }
}

fn matches(rule: &CondRule, stats: Stats, ops: &(Operand, Operand), v: &Value) -> bool {
    use std::cmp::Ordering::{Equal, Greater, Less};
    let empty = match v {
        Value::Empty => true,
        Value::Text(s) => s.is_empty(),
        _ => false,
    };
    match rule {
        CondRule::Compare { op, .. } => {
            if empty {
                return false;
            }
            let Some(a) = compare(v, &ops.0) else {
                return *op == CompareOp::Ne || *op == CompareOp::NotBetween;
            };
            match op {
                CompareOp::Gt => a == Greater,
                CompareOp::Ge => a != Less,
                CompareOp::Lt => a == Less,
                CompareOp::Le => a != Greater,
                CompareOp::Eq => a == Equal,
                CompareOp::Ne => a != Equal,
                CompareOp::Between | CompareOp::NotBetween => {
                    let Some(b) = compare(v, &ops.1) else {
                        return *op == CompareOp::NotBetween;
                    };
                    // Bornes dans n'importe quel ordre.
                    let inside = (a != Less && b != Greater) || (a != Greater && b != Less);
                    inside == (*op == CompareOp::Between)
                }
            }
        }
        CondRule::Contains { text } => {
            value_text(v).is_some_and(|s| s.contains(&text.to_lowercase()))
        }
        CondRule::BeginsWith { text } => {
            value_text(v).is_some_and(|s| s.starts_with(&text.to_lowercase()))
        }
        CondRule::EndsWith { text } => {
            value_text(v).is_some_and(|s| s.ends_with(&text.to_lowercase()))
        }
        CondRule::Empty => empty,
        CondRule::NotEmpty => !empty,
        CondRule::Error => v.is_error(),
        CondRule::Top { bottom, .. } => match (v, stats) {
            (Value::Number(x), Stats::Threshold(Some(t))) => {
                if *bottom {
                    *x <= t
                } else {
                    *x >= t
                }
            }
            _ => false,
        },
        CondRule::Average { above } => match (v, stats) {
            (Value::Number(x), Stats::Average(Some(m))) => {
                if *above {
                    *x > m
                } else {
                    *x < m
                }
            }
            _ => false,
        },
        CondRule::ColorScale { .. } | CondRule::DataBar { .. } => false,
    }
}

fn mix(a: [u8; 3], b: [u8; 3], t: f64) -> [u8; 3] {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let m = |x: u8, y: u8| (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

fn scale_color(
    x: f64,
    lo: f64,
    mid_v: f64,
    hi: f64,
    min: [u8; 3],
    mid: Option<[u8; 3]>,
    max: [u8; 3],
) -> [u8; 3] {
    let frac = |a: f64, b: f64| if b > a { (x - a) / (b - a) } else { 1.0 };
    match mid {
        Some(m) if x <= mid_v => mix(min, m, frac(lo, mid_v)),
        Some(m) => mix(m, max, frac(mid_v, hi)),
        None => mix(min, max, frac(lo, hi)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbook::Workbook;

    fn at(s: &str) -> CellRef {
        CellRef::parse(s).unwrap()
    }

    fn wb_with(values: &[&str]) -> Workbook {
        let mut wb = Workbook::new();
        for (i, v) in values.iter().enumerate() {
            wb.set_input(0, CellRef::new(i as u32, 0), v);
        }
        wb
    }

    fn rule(wb: &mut Workbook, rule: CondRule) {
        wb.sheets[0].cond_formats.push(CondFormat {
            range: Range::parse("A1:A10").unwrap(),
            rule,
            style: CondStyle {
                bold: true,
                ..CondStyle::default()
            },
        });
    }

    fn bold(wb: &Workbook, cell: &str) -> bool {
        wb.cond_paint(0, at(cell)).is_some_and(|p| p.bold)
    }

    #[test]
    fn comparaisons() {
        let mut wb = wb_with(&["5", "10", "15", "abc", "", "=1/0"]);
        rule(
            &mut wb,
            CondRule::Compare {
                op: CompareOp::Gt,
                value: "9,5".into(),
                value2: String::new(),
            },
        );
        assert!(!bold(&wb, "A1"));
        assert!(bold(&wb, "A2"));
        assert!(bold(&wb, "A3"));
        assert!(!bold(&wb, "A4"));
        assert!(!bold(&wb, "A5"));
        wb.sheets[0].cond_formats.clear();
        rule(
            &mut wb,
            CondRule::Compare {
                op: CompareOp::Between,
                value: "12".into(),
                value2: "4".into(),
            },
        );
        assert!(bold(&wb, "A1"));
        assert!(bold(&wb, "A2"));
        assert!(!bold(&wb, "A3"));
        wb.sheets[0].cond_formats.clear();
        rule(
            &mut wb,
            CondRule::Compare {
                op: CompareOp::Eq,
                value: "ABC".into(),
                value2: String::new(),
            },
        );
        assert!(bold(&wb, "A4"));
        assert!(!bold(&wb, "A1"));
    }

    #[test]
    fn textes_vides_erreurs() {
        let mut wb = wb_with(&["Bonjour", "au revoir", "", "=1/0"]);
        rule(
            &mut wb,
            CondRule::Contains {
                text: "JOUR".into(),
            },
        );
        assert!(bold(&wb, "A1"));
        assert!(!bold(&wb, "A2"));
        wb.sheets[0].cond_formats.clear();
        rule(&mut wb, CondRule::BeginsWith { text: "au".into() });
        assert!(bold(&wb, "A2"));
        wb.sheets[0].cond_formats.clear();
        rule(&mut wb, CondRule::Empty);
        assert!(bold(&wb, "A3"));
        assert!(!bold(&wb, "A1"));
        wb.sheets[0].cond_formats.clear();
        rule(&mut wb, CondRule::Error);
        assert!(bold(&wb, "A4"));
        assert!(!bold(&wb, "A1"));
    }

    #[test]
    fn premiers_et_moyenne() {
        let mut wb = wb_with(&["1", "2", "3", "4", "5"]);
        rule(
            &mut wb,
            CondRule::Top {
                n: 2,
                bottom: false,
                percent: false,
            },
        );
        assert!(bold(&wb, "A5") && bold(&wb, "A4") && !bold(&wb, "A3"));
        wb.sheets[0].cond_formats.clear();
        rule(
            &mut wb,
            CondRule::Top {
                n: 1,
                bottom: true,
                percent: false,
            },
        );
        assert!(bold(&wb, "A1") && !bold(&wb, "A2"));
        wb.sheets[0].cond_formats.clear();
        rule(&mut wb, CondRule::Average { above: true });
        assert!(bold(&wb, "A4") && !bold(&wb, "A3"));
    }

    #[test]
    fn premiere_regle_gagnante_et_echelles() {
        let mut wb = wb_with(&["0", "50", "100"]);
        wb.sheets[0].cond_formats.push(CondFormat {
            range: Range::parse("A1:A3").unwrap(),
            rule: CondRule::Compare {
                op: CompareOp::Ge,
                value: "100".into(),
                value2: String::new(),
            },
            style: CondStyle {
                color: Some([255, 0, 0]),
                ..CondStyle::default()
            },
        });
        wb.sheets[0].cond_formats.push(CondFormat {
            range: Range::parse("A1:A3").unwrap(),
            rule: CondRule::NotEmpty,
            style: CondStyle {
                color: Some([0, 0, 255]),
                ..CondStyle::default()
            },
        });
        wb.sheets[0].cond_formats.push(CondFormat {
            range: Range::parse("A1:A3").unwrap(),
            rule: CondRule::ColorScale {
                min: [0, 0, 0],
                mid: Some([100, 100, 100]),
                max: [200, 200, 200],
            },
            style: CondStyle::default(),
        });
        wb.sheets[0].cond_formats.push(CondFormat {
            range: Range::parse("A1:A3").unwrap(),
            rule: CondRule::DataBar { color: [1, 2, 3] },
            style: CondStyle::default(),
        });
        let p = wb.cond_paint(0, at("A3")).unwrap();
        assert_eq!(p.color, Some([255, 0, 0]));
        assert_eq!(p.fill, Some([200, 200, 200]));
        assert_eq!(p.bar, Some((1.0, [1, 2, 3])));
        let p = wb.cond_paint(0, at("A2")).unwrap();
        assert_eq!(p.color, Some([0, 0, 255]));
        assert_eq!(p.fill, Some([100, 100, 100]));
        assert_eq!(p.bar.map(|b| b.0), Some(0.5));
        assert!(wb.cond_paint(0, at("B1")).is_none());
    }

    #[test]
    fn plages_suivent_les_lignes_et_colonnes() {
        let mut wb = wb_with(&["1"]);
        rule(&mut wb, CondRule::NotEmpty);
        wb.insert_rows(0, 0, 2);
        assert_eq!(
            wb.sheets[0].cond_formats[0].range,
            Range::parse("A3:A12").unwrap()
        );
        wb.delete_rows(0, 5, 3);
        assert_eq!(
            wb.sheets[0].cond_formats[0].range,
            Range::parse("A3:A9").unwrap()
        );
        wb.insert_cols(0, 0, 1);
        assert_eq!(
            wb.sheets[0].cond_formats[0].range,
            Range::parse("B3:B9").unwrap()
        );
        wb.delete_cols(0, 1, 1);
        assert!(wb.sheets[0].cond_formats.is_empty());
    }

    #[test]
    fn json_compatible() {
        let mut wb = wb_with(&["1"]);
        rule(&mut wb, CondRule::NotEmpty);
        wb.sheets[0].frozen_rows = 1;
        let json = wb.to_json();
        assert!(json.contains("\"A1:A10\""));
        let back = Workbook::from_json(&json).unwrap();
        assert_eq!(back.sheets[0].cond_formats, wb.sheets[0].cond_formats);
        assert_eq!(back.sheets[0].frozen_rows, 1);
        // Un ancien fichier, sans règles ni volets, s'ouvre toujours.
        let old = r#"{"format":"varium-calc","version":1,"title":"x","sheets":[{"name":"F","cells":{"A1":{"input":"1"}}}]}"#;
        let wb = Workbook::from_json(old).unwrap();
        assert!(wb.sheets[0].cond_formats.is_empty());
        assert_eq!(wb.sheets[0].frozen_rows, 0);
    }
}
