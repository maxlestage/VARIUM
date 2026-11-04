//! Filtres de mise en forme. Les nombres, montants et dates suivent la langue du
//! document (`crate::lang`), le français par défaut.

use crate::parse::Filter;

/// Espace fine insécable (séparateur de milliers en français).
pub const THIN_NBSP: char = '\u{202F}';
/// Espace insécable (avant le symbole monétaire).
pub const NBSP: char = '\u{00A0}';

/// Liste des filtres connus, pour l'aide de l'interface.
pub const KNOWN: &[(&str, &str)] = &[
    ("upper", "MAJUSCULES"),
    ("lower", "minuscules"),
    ("capitalize", "Première lettre en capitale"),
    ("title", "Chaque Mot En Capitale"),
    ("trim", "Supprime les espaces autour"),
    ("number:2", "1 234,57"),
    ("money · money:usd", "1 234,50 €"),
    ("date · date:full · date:short", "15 mars 2026"),
    ("default:\"texte\"", "Valeur si le champ est vide"),
    ("qr", "Code QR de la valeur"),
];

pub fn apply(value: &str, filters: &[Filter]) -> String {
    filters.iter().fold(value.to_string(), |v, f| {
        apply_one(&v, &f.name, f.arg.as_deref())
    })
}

pub fn apply_one(v: &str, name: &str, arg: Option<&str>) -> String {
    match name {
        "upper" => v.to_uppercase(),
        "lower" => v.to_lowercase(),
        "trim" => v.trim().to_string(),
        "capitalize" => capitalize(v),
        "title" => title(v),
        "number" => parse_number(v)
            .map(|n| format_number(n, arg.and_then(|a| a.parse().ok())))
            .unwrap_or_else(|| v.to_string()),
        "money" => parse_number(v)
            .map(|n| format_money(n, arg.unwrap_or("EUR")))
            .unwrap_or_else(|| v.to_string()),
        "date" => parse_date(v)
            .map(|d| format_date(d, arg.unwrap_or("long")))
            .unwrap_or_else(|| v.to_string()),
        // Le code QR est dessiné à partir de la valeur, qui reste inchangée.
        "qr" => v.to_string(),
        "default" => {
            if v.is_empty() {
                arg.unwrap_or_default().to_string()
            } else {
                v.to_string()
            }
        }
        _ => v.to_string(),
    }
}

fn capitalize(v: &str) -> String {
    let mut c = v.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

fn title(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut boundary = true;
    for ch in v.chars() {
        if boundary && ch.is_alphabetic() {
            out.extend(ch.to_uppercase());
        } else {
            out.push(ch);
        }
        boundary = ch.is_whitespace() || matches!(ch, '-' | '\'' | '’');
    }
    out
}

/// Nombre décimal : accepte `1234,5`, `1 234.5`, `-3`.
pub fn parse_number(v: &str) -> Option<f64> {
    let s: String = v
        .chars()
        .filter(|c| !c.is_whitespace() && *c != THIN_NBSP && *c != NBSP)
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    if s.is_empty() {
        return None;
    }
    s.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Format français : séparateur de milliers fin, virgule décimale.
/// `digits = None` garde jusqu'à 10 décimales significatives.
pub fn format_number(n: f64, digits: Option<usize>) -> String {
    let s = match digits {
        Some(d) => format!("{:.*}", d, n.abs()),
        None => {
            let s = format!("{:.10}", n.abs());
            let s = s.trim_end_matches('0').trim_end_matches('.');
            s.to_string()
        }
    };
    let (int, frac) = match s.split_once('.') {
        Some((i, f)) => (i.to_string(), Some(f.to_string())),
        None => (s, None),
    };
    let lang = crate::lang::current();
    let group = lang.group();
    let mut grouped = String::new();
    let len = int.len();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            grouped.push(group);
        }
        grouped.push(c);
    }
    let neg = n < 0.0
        && (grouped.chars().any(|c| c.is_ascii_digit() && c != '0')
            || frac.as_deref().is_some_and(|f| f.chars().any(|c| c != '0')));
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push_str(&grouped);
    if let Some(f) = frac {
        out.push(lang.decimal());
        out.push_str(&f);
    }
    out
}

pub fn format_money(n: f64, currency: &str) -> String {
    use crate::lang::Lang;
    let code = currency.to_uppercase();
    let lang = crate::lang::current();
    if lang != Lang::Fr {
        let (symbol, digits) = match code.as_str() {
            "EUR" => ("€", 2),
            "USD" => ("$", 2),
            "GBP" => ("£", 2),
            "CHF" => ("CHF", 2),
            "CAD" => ("CA$", 2),
            "JPY" => ("¥", 0),
            other => (other, 2),
        };
        let amount = format_number(n.abs(), Some(digits));
        let sign = if n < 0.0 && amount.chars().any(|c| c.is_ascii_digit() && c != '0') {
            "-"
        } else {
            ""
        };
        // En anglais, le symbole précède le montant ; ailleurs il le suit.
        return if lang == Lang::En {
            let sep = if symbol.chars().count() > 1 && symbol.chars().all(char::is_alphabetic) {
                "\u{a0}"
            } else {
                ""
            };
            format!("{sign}{symbol}{sep}{amount}")
        } else {
            format!("{sign}{amount}{NBSP}{symbol}")
        };
    }
    let (symbol, digits) = match code.as_str() {
        "EUR" => ("€", 2),
        "USD" => ("$US", 2),
        "GBP" => ("£GB", 2),
        "CHF" => ("CHF", 2),
        "CAD" => ("$CA", 2),
        "JPY" => ("JPY", 0),
        other => (other, 2),
    };
    format!("{}{NBSP}{symbol}", format_number(n, Some(digits)))
}

/// Date ISO `AAAA-MM-JJ` → (année, mois 1-12, jour).
pub fn parse_date(v: &str) -> Option<(i32, u32, u32)> {
    let v = v.trim();
    let mut it = v.splitn(3, '-');
    let (y, m, d) = (it.next()?, it.next()?, it.next()?);
    if y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let (y, m, d): (i32, u32, u32) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    ((1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m)).then_some((y, m, d))
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Jour de la semaine (0 = dimanche), algorithme de Sakamoto.
fn weekday(y: i32, m: u32, d: u32) -> usize {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    ((y + y / 4 - y / 100 + y / 400 + T[m as usize - 1] + d as i32).rem_euclid(7)) as usize
}

pub fn format_date((y, m, d): (i32, u32, u32), style: &str) -> String {
    use crate::lang::Lang;
    let lang = crate::lang::current();
    let (Some(month), Some(wd)) = (
        lang.months().get((m as usize).wrapping_sub(1)).copied(),
        lang.weekdays().get(weekday(y, m, d)).copied(),
    ) else {
        return format!("{y:04}-{m:02}-{d:02}");
    };
    if style == "short" {
        return match lang {
            Lang::De => format!("{d:02}.{m:02}.{y}"),
            _ => format!("{d:02}/{m:02}/{y}"),
        };
    }
    let full = style == "full";
    match lang {
        Lang::Fr => {
            let day = if d == 1 {
                "1er".to_string()
            } else {
                d.to_string()
            };
            if full {
                format!("{wd} {day} {month} {y}")
            } else {
                format!("{day} {month} {y}")
            }
        }
        Lang::En => {
            if full {
                format!("{wd} {d} {month} {y}")
            } else {
                format!("{d} {month} {y}")
            }
        }
        Lang::Es | Lang::Pt => {
            if full {
                format!("{wd}, {d} de {month} de {y}")
            } else {
                format!("{d} de {month} de {y}")
            }
        }
        Lang::De => {
            if full {
                format!("{wd}, {d}. {month} {y}")
            } else {
                format!("{d}. {month} {y}")
            }
        }
        Lang::It => {
            let day = if d == 1 {
                "1º".to_string()
            } else {
                d.to_string()
            };
            if full {
                format!("{wd} {day} {month} {y}")
            } else {
                format!("{day} {month} {y}")
            }
        }
    }
}
