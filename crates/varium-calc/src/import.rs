//! Outils communs aux imports XLSX et ODS : saisies des cellules, formats de
//! nombres, noms de feuilles, limites de taille.

use crate::eval::is_formula;
use crate::format::{CellFormat, NumberFormat, date_serial, parse_literal, serial_to_ymd};
use crate::refs::{CellRef, MAX_COLS, MAX_ROWS};
use crate::rewrite::normalize_formula;
use crate::value::{CalcError, Value};
use crate::workbook::{MAX_SHEET_NAME, Sheet, Workbook};

/// Nombre maximal de cellules lues dans un fichier (au-delà, elles sont ignorées).
pub(crate) const MAX_IMPORT_CELLS: usize = 2_000_000;

/// Remplit une feuille en comptant les cellules.
pub(crate) struct Filler<'a> {
    pub sheet: &'a mut Sheet,
    pub count: &'a mut usize,
}

impl Filler<'_> {
    /// Place une cellule (saisie et format). Hors limites : ignorée.
    pub fn put(&mut self, at: CellRef, input: String, format: CellFormat) {
        if at.row >= MAX_ROWS || at.col >= MAX_COLS || *self.count >= MAX_IMPORT_CELLS {
            return;
        }
        let input = if is_formula(&input) {
            normalize_formula(&input)
        } else {
            input
        };
        if input.is_empty() && format.is_default() {
            return;
        }
        *self.count += 1;
        self.sheet.put(at, input, format);
    }
}

/// Saisie qui redonne exactement ce texte (apostrophe si le texte serait lu
/// comme un nombre, une date, une formule…).
pub(crate) fn text_input(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let literal_text = matches!(parse_literal(s).0, Value::Text(ref t) if t == s);
    if literal_text && !s.starts_with('=') && !s.starts_with('\'') {
        s.to_string()
    } else {
        format!("'{s}")
    }
}

/// Saisie d'un nombre : date `jj/mm/aaaa` pour un format date entier,
/// sinon le nombre à la française.
pub(crate) fn number_input(n: f64, format: NumberFormat) -> String {
    if !n.is_finite() {
        return "=#NOMBRE!".into();
    }
    if format == NumberFormat::Date
        && n.fract() == 0.0
        && let Some((y, m, d)) = serial_to_ymd(n)
        && (1900..=9999).contains(&y)
    {
        return format!("{d:02}/{m:02}/{y:04}");
    }
    let s = format!("{n}");
    s.replace('.', ",")
}

/// Saisie d'une valeur logique.
pub(crate) fn bool_input(b: bool) -> String {
    if b { "VRAI" } else { "FAUX" }.into()
}

/// Saisie d'une erreur constante (`#DIV/0!` → `=#DIV/0!`).
pub(crate) fn error_input(code: &str) -> String {
    match CalcError::from_code(code.trim()) {
        Some(e) => format!("={}", e.code()),
        None => text_input(code),
    }
}

/// Lit une date ISO (`2026-03-15` ou `2026-03-15T10:30:00`) en numéro de série.
pub(crate) fn iso_date(s: &str) -> Option<f64> {
    let s = s.trim();
    let (date, time) = match s.split_once('T') {
        Some((d, t)) => (d, Some(t)),
        None => (s, None),
    };
    let mut it = date.splitn(3, '-');
    let y: i32 = it.next()?.parse().ok()?;
    let m: i32 = it.next()?.parse().ok()?;
    let d: i32 = it.next()?.get(..2)?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || !(-9999..=99_999).contains(&y) {
        return None;
    }
    let mut serial = date_serial(y, m, d);
    if let Some(t) = time {
        let mut parts = t.trim_end_matches('Z').split(':');
        let h: f64 = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
        let mi: f64 = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
        let sec: f64 = parts
            .next()
            .and_then(|x| x.get(..2.min(x.len()))?.parse().ok())
            .unwrap_or(0.0);
        serial += (h * 3600.0 + mi * 60.0 + sec) / 86_400.0;
    }
    Some(serial)
}

/// Lit une durée ISO 8601 (`PT12H30M00S`) en fraction de jour.
pub(crate) fn iso_duration(s: &str) -> Option<f64> {
    let s = s.trim();
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s),
    };
    let s = s.strip_prefix('P')?;
    let mut days = 0.0;
    let mut num = String::new();
    let mut in_time = false;
    for c in s.chars() {
        match c {
            'T' => in_time = true,
            '0'..='9' | '.' | ',' => num.push(if c == ',' { '.' } else { c }),
            unit => {
                let v: f64 = num.parse().ok()?;
                num.clear();
                days += match (unit, in_time) {
                    ('D', _) => v,
                    ('H', true) => v / 24.0,
                    ('M', true) => v / 1440.0,
                    ('S', true) => v / 86_400.0,
                    _ => return None,
                };
            }
        }
    }
    Some(if neg { -days } else { days })
}

/// Format de nombre approché d'après un code de format Excel / LibreOffice.
pub(crate) fn number_format_from_code(code: &str) -> NumberFormat {
    let first = code.split(';').next().unwrap_or("");
    // Retire les textes entre guillemets, les crochets ([Red], [$€-40C]) et
    // les caractères échappés, en notant les symboles monétaires.
    let mut cleaned = String::new();
    let mut currency = false;
    let mut chars = first.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                for q in chars.by_ref() {
                    if q == '"' {
                        break;
                    }
                    if matches!(q, '€' | '$' | '£') {
                        currency = true;
                    }
                }
            }
            '[' => {
                let mut inner = String::new();
                for q in chars.by_ref() {
                    if q == ']' {
                        break;
                    }
                    inner.push(q);
                }
                if inner.starts_with('$') {
                    currency = true;
                }
            }
            '\\' => {
                if let Some(n) = chars.next()
                    && matches!(n, '€' | '$' | '£')
                {
                    currency = true;
                }
            }
            '€' | '$' | '£' => currency = true,
            '_' | '*' => {
                chars.next();
            }
            c => cleaned.push(c),
        }
    }
    let lower = cleaned.to_lowercase();
    if lower.trim() == "general" || lower.trim() == "standard" || lower.trim().is_empty() {
        return if currency {
            NumberFormat::Currency(2)
        } else {
            NumberFormat::General
        };
    }
    if lower.contains('@') && !lower.contains(['0', '#']) {
        return NumberFormat::Text;
    }
    let has_digits = lower.contains(['0', '#', '?']);
    let is_date = lower.contains(['d', 'y', 'j', 'a'])
        || (lower.contains('m') && !has_digits && !lower.contains(['h', 's']));
    if is_date && !lower.contains("e+") && !lower.contains("e-") {
        return NumberFormat::Date;
    }
    if !has_digits {
        return NumberFormat::General;
    }
    let decimals = match lower.find(['.', ',']) {
        // « # ##0,00 » ou « #,##0.00 » : le dernier séparateur suivi de chiffres.
        Some(_) => {
            let sep = lower.rfind(['.', ',']).unwrap_or(0);
            let after: String = lower[sep + 1..]
                .chars()
                .take_while(|c| matches!(c, '0' | '#' | '?'))
                .collect();
            // « #,##0 » : la virgule groupe les milliers (trois chiffres après).
            if after.len() == 3
                && lower[..sep].contains('#')
                && !lower[sep + 1..].contains(['.', ','])
                && lower[sep..].starts_with(',')
                && !lower.contains('.')
            {
                0
            } else {
                after.len()
            }
        }
        None => 0,
    }
    .min(15) as u8;
    if lower.contains('%') {
        NumberFormat::Percent(decimals)
    } else if currency {
        NumberFormat::Currency(decimals)
    } else if lower.contains('e') {
        NumberFormat::General
    } else {
        NumberFormat::Number(decimals)
    }
}

/// Formats intégrés d'Excel (numéros de 0 à 49).
pub(crate) fn builtin_number_format(id: u32) -> NumberFormat {
    match id {
        1 | 3 | 37 | 38 => NumberFormat::Number(0),
        2 | 4 | 39 | 40 => NumberFormat::Number(2),
        5 | 6 | 41 | 42 => NumberFormat::Currency(0),
        7 | 8 | 43 | 44 => NumberFormat::Currency(2),
        9 => NumberFormat::Percent(0),
        10 => NumberFormat::Percent(2),
        14..=17 | 22 | 27..=36 | 50..=58 => NumberFormat::Date,
        49 => NumberFormat::Text,
        _ => NumberFormat::General,
    }
}

/// Nettoie les noms de feuilles (caractères interdits, longueur, doublons).
pub(crate) fn unique_sheet_names(names: &[String]) -> Vec<String> {
    let mut used: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (i, n) in names.iter().enumerate() {
        let mut name = crate::workbook::clean_sheet_name(n);
        if name.is_empty() {
            name = format!("Feuille {}", i + 1);
        }
        let base = name.clone();
        let mut k = 2;
        while used.contains(&name.to_lowercase()) {
            let suffix = format!(" ({k})");
            let keep = MAX_SHEET_NAME.saturating_sub(suffix.chars().count());
            name = format!("{}{suffix}", base.chars().take(keep).collect::<String>());
            k += 1;
        }
        used.push(name.to_lowercase());
        out.push(name);
    }
    out
}

/// Classeur final : titre, feuilles (au moins une).
pub(crate) fn finish(title: &str, sheets: Vec<Sheet>) -> Workbook {
    let mut wb = Workbook::new();
    wb.title = if title.trim().is_empty() {
        "Sans titre".into()
    } else {
        title.trim().to_string()
    };
    if !sheets.is_empty() {
        wb.sheets = sheets;
    }
    wb.invalidate();
    wb
}

/// Couleur `RRGGBB` ou `AARRGGBB` (ou `#rrggbb`).
pub(crate) fn parse_rgb(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_start_matches('#');
    let s = match s.len() {
        8 => &s[2..],
        6 => s,
        _ => return None,
    };
    if !s.is_ascii() {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    Some([p(0)?, p(2)?, p(4)?])
}

/// Longueur ODF (`2.5cm`, `1in`, `12pt`, `30mm`, `40px`) en points d'écran.
pub(crate) fn length_px(s: &str) -> Option<f32> {
    let s = s.trim();
    let split = s
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(s.len());
    let v: f64 = s[..split].trim().parse().ok()?;
    let px = match &s[split..] {
        "cm" => v / 2.54 * 96.0,
        "mm" => v / 25.4 * 96.0,
        "in" | "inch" => v * 96.0,
        "pt" => v * 96.0 / 72.0,
        "pc" => v * 16.0,
        "px" | "" => v,
        _ => return None,
    };
    (px.is_finite() && px > 0.0).then_some(px as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_de_nombres() {
        use NumberFormat::*;
        assert_eq!(number_format_from_code("General"), General);
        assert_eq!(number_format_from_code("0.00"), Number(2));
        assert_eq!(number_format_from_code("#,##0"), Number(0));
        assert_eq!(number_format_from_code("#,##0.00"), Number(2));
        assert_eq!(number_format_from_code("# ##0,00"), Number(2));
        assert_eq!(number_format_from_code("0%"), Percent(0));
        assert_eq!(number_format_from_code("0.0%"), Percent(1));
        assert_eq!(number_format_from_code("#,##0.00\\ \"€\""), Currency(2));
        assert_eq!(number_format_from_code("[$€-40C]#,##0.00"), Currency(2));
        assert_eq!(number_format_from_code("#,##0.00 [$€-1];[RED]-#,##0.00 [$€-1]"), Currency(2));
        assert_eq!(number_format_from_code("dd/mm/yyyy"), Date);
        assert_eq!(number_format_from_code("DD/MM/YYYY"), Date);
        assert_eq!(number_format_from_code("mmm-yy"), Date);
        assert_eq!(number_format_from_code("hh:mm"), General);
        assert_eq!(number_format_from_code("@"), Text);
        assert_eq!(number_format_from_code("0.00E+00"), General);
        assert_eq!(builtin_number_format(14), Date);
        assert_eq!(builtin_number_format(10), Percent(2));
    }

    #[test]
    fn saisies() {
        assert_eq!(text_input("abc"), "abc");
        assert_eq!(text_input("12"), "'12");
        assert_eq!(text_input("=1"), "'=1");
        assert_eq!(text_input("vrai"), "'vrai");
        assert_eq!(number_input(1.5, NumberFormat::General), "1,5");
        assert_eq!(number_input(46096.0, NumberFormat::Date), "15/03/2026");
        assert_eq!(error_input("#DIV/0!"), "=#DIV/0!");
        assert_eq!(iso_date("2026-03-15"), Some(46096.0));
        assert_eq!(iso_date("2026-03-15T12:00:00"), Some(46096.5));
        assert_eq!(iso_duration("PT12H"), Some(0.5));
        assert_eq!(parse_rgb("FFFF0000"), Some([255, 0, 0]));
        assert_eq!(parse_rgb("#00ff00"), Some([0, 255, 0]));
        assert_eq!(length_px("2.54cm"), Some(96.0));
        assert_eq!(
            unique_sheet_names(&["a".into(), "A".into(), "".into()]),
            vec!["a".to_string(), "A (2)".into(), "Feuille 3".into()]
        );
    }
}
