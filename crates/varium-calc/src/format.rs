//! Formats de cellule, affichage à la française et lecture des saisies.
//!
//! Les nombres s'affichent comme dans LibreOffice Calc en français : virgule
//! décimale, espace fine insécable (U+202F) entre les milliers, espace insécable
//! (U+00A0) avant `%` et `€`, dates au format `jj/mm/aaaa`.

use serde::{Deserialize, Serialize};

use crate::value::{CalcError, Value};

/// Séparateur des milliers (espace fine insécable).
pub const THOUSANDS_SEP: char = '\u{202F}';
/// Espace insécable placée avant `%` et `€`.
pub const NBSP: char = '\u{00A0}';

/// Alignement horizontal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HAlign {
    Left,
    Center,
    Right,
}

/// Format des nombres. Le paramètre donne le nombre de décimales.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NumberFormat {
    /// Standard : entiers sans décimales, sinon jusqu'à 10 décimales.
    #[default]
    General,
    /// `1 234,50` (séparateur des milliers).
    Number(u8),
    /// Euros : `1 234,50 €`.
    Currency(u8),
    /// Pourcentage : `12,5 %`.
    Percent(u8),
    /// Date : `15/03/2026`.
    Date,
    /// Texte : la saisie est gardée telle quelle, sans conversion en nombre.
    Text,
}

/// Mise en forme d'une cellule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct CellFormat {
    #[serde(skip_serializing_if = "is_general")]
    pub number: NumberFormat,
    #[serde(skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<HAlign>,
    /// Couleur de fond (RVB).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<[u8; 3]>,
    /// Couleur du texte (RVB).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
}

fn is_general(n: &NumberFormat) -> bool {
    *n == NumberFormat::General
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl CellFormat {
    pub fn is_default(&self) -> bool {
        *self == CellFormat::default()
    }
}

/// Au-delà, on n'affiche pas plus de décimales (évite des chaînes absurdes).
const MAX_DECIMALS: usize = 20;

/// Nombre au format « Standard ».
pub fn format_general(n: f64) -> String {
    if !n.is_finite() {
        return CalcError::Num.code().to_string();
    }
    if n == 0.0 {
        return "0".into();
    }
    let a = n.abs();
    if !(1e-10..1e15).contains(&a) {
        // Notation scientifique : 1,2345E+20.
        let s = format!("{n:.9E}");
        let (mant, exp) = s.split_once('E').unwrap_or((&s, "0"));
        let mant = trim_zeros(mant).replace('.', ",");
        let exp: i32 = exp.parse().unwrap_or(0);
        let sign = if exp < 0 { '-' } else { '+' };
        return format!("{mant}E{sign}{:02}", exp.abs());
    }
    let int_digits = if a >= 1.0 {
        a.log10().floor() as i32 + 1
    } else {
        0
    };
    let decimals = (15 - int_digits).clamp(0, 10) as usize;
    let s = format!("{n:.decimals$}");
    let s = trim_zeros(&s);
    let s = if s == "-0" { "0" } else { s };
    s.replace('.', ",")
}

fn trim_zeros(s: &str) -> &str {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        s
    }
}

/// Nombre à décimales fixes, à la française (`-1 234,50`).
pub(crate) fn format_fixed(n: f64, decimals: usize, grouping: bool) -> String {
    format_fixed_min(n, decimals, grouping, 1)
}

/// Comme [`format_fixed`], avec un nombre minimal de chiffres entiers (0 permis).
fn format_fixed_min(n: f64, decimals: usize, grouping: bool, min_int: usize) -> String {
    if !n.is_finite() {
        return CalcError::Num.code().to_string();
    }
    let decimals = decimals.min(MAX_DECIMALS);
    let s = format!("{:.decimals$}", n.abs());
    let (int, frac) = s.split_once('.').unwrap_or((&s, ""));
    let int = int.trim_start_matches('0');
    let int = if int.len() < min_int {
        format!("{}{int}", "0".repeat(min_int - int.len()))
    } else {
        int.to_string()
    };
    let mut out = String::with_capacity(int.len() + frac.len() + 8);
    if n < 0.0 && s.bytes().any(|b| (b'1'..=b'9').contains(&b)) {
        out.push('-');
    }
    let len = int.len();
    for (i, c) in int.chars().enumerate() {
        if grouping && i > 0 && (len - i) % 3 == 0 {
            out.push(THOUSANDS_SEP);
        }
        out.push(c);
    }
    if !frac.is_empty() {
        out.push(',');
        out.push_str(frac);
    }
    out
}

/// Texte affiché pour une valeur selon le format choisi.
pub fn format_value(v: &Value, f: NumberFormat) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Text(s) => s.clone(),
        Value::Bool(b) => bool_text(*b).into(),
        Value::Error(e) => e.code().into(),
        Value::Number(n) => format_number(*n, f),
    }
}

/// Nombre affiché selon le format choisi.
pub fn format_number(n: f64, f: NumberFormat) -> String {
    match f {
        NumberFormat::General | NumberFormat::Text => format_general(n),
        NumberFormat::Number(d) => format_fixed(n, d.into(), true),
        NumberFormat::Currency(d) => format!("{}{NBSP}€", format_fixed(n, d.into(), true)),
        NumberFormat::Percent(d) => format!("{}{NBSP}%", format_fixed(n * 100.0, d.into(), true)),
        NumberFormat::Date => match serial_to_ymd(n) {
            Some((y, m, d)) if (0..=9999).contains(&y) => format!("{d:02}/{m:02}/{y:04}"),
            _ => format_general(n),
        },
    }
}

pub(crate) fn bool_text(b: bool) -> &'static str {
    if b { "VRAI" } else { "FAUX" }
}

// ---------------------------------------------------------------- Dates

/// Jours depuis le 1er janvier 1970 (algorithme de H. Hinnant).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

/// Écart entre le 1970-01-01 et l'origine des tableurs (1899-12-30).
const UNIX_EPOCH_SERIAL: i64 = 25_569;

/// Numéro de série d'une date (le 30/12/1899 vaut 0, comme dans les tableurs).
/// Les mois et jours hors limites débordent sur les suivants (`DATE(2026;13;1)`).
pub fn date_serial(year: i32, month: i32, day: i32) -> f64 {
    let m0 = i64::from(month) - 1;
    let y = i64::from(year) + m0.div_euclid(12);
    let m = m0.rem_euclid(12) + 1;
    (days_from_civil(y, m, 1) + i64::from(day) - 1 + UNIX_EPOCH_SERIAL) as f64
}

/// Date (année, mois, jour) d'un numéro de série.
pub fn serial_to_ymd(serial: f64) -> Option<(i32, u32, u32)> {
    if !serial.is_finite() || serial.abs() > 1e8 {
        return None;
    }
    let (y, m, d) = civil_from_days(serial.floor() as i64 - UNIX_EPOCH_SERIAL);
    Some((i32::try_from(y).ok()?, m, d))
}

/// Numéro de série du jour d'aujourd'hui (UTC), sans dépendance externe.
pub fn today_serial() -> f64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (secs / 86_400) as f64 + UNIX_EPOCH_SERIAL as f64
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// Lit `15/03/2026`, `15/3/26` ou `2026-03-15`.
pub(crate) fn parse_date(s: &str) -> Option<f64> {
    let s = s.trim();
    let num = |p: &str, max: usize| -> Option<u32> {
        (!p.is_empty() && p.len() <= max && p.bytes().all(|b| b.is_ascii_digit()))
            .then(|| p.parse().ok())
            .flatten()
    };
    let (y, m, d) = if s.contains('/') {
        let mut it = s.split('/');
        let (d, m, y) = (it.next()?, it.next()?, it.next()?);
        if it.next().is_some() {
            return None;
        }
        let year = num(y, 4)?;
        let year = match y.len() {
            1 | 2 if year < 30 => 2000 + year,
            1 | 2 => 1900 + year,
            4 => year,
            _ => return None,
        };
        (year as i32, num(m, 2)?, num(d, 2)?)
    } else {
        let mut it = s.split('-');
        let (y, m, d) = (it.next()?, it.next()?, it.next()?);
        if it.next().is_some() || y.len() != 4 {
            return None;
        }
        (num(y, 4)? as i32, num(m, 2)?, num(d, 2)?)
    };
    if !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
        return None;
    }
    Some(date_serial(y, m as i32, d as i32))
}

// ---------------------------------------------------------------- Nombres saisis

fn is_group_space(c: char) -> bool {
    matches!(c, ' ' | NBSP | THOUSANDS_SEP | '\u{2009}' | '\'')
}

/// Lit un nombre écrit à la française (`1 234,5`) ou à l'anglaise (`1,234.5`, `12.5`).
/// Renvoie aussi le nombre de décimales écrites.
pub(crate) fn parse_number_dec(s: &str) -> Option<(f64, usize)> {
    let s = s.trim();
    let (neg, body) = match s.chars().next()? {
        '-' | '\u{2212}' => (true, &s[s.chars().next()?.len_utf8()..]),
        '+' => (false, &s[1..]),
        _ => (false, s),
    };
    let body = body.trim_start();
    let (mant, exp) = match body.find(['e', 'E']) {
        Some(i) => (&body[..i], Some(&body[i + 1..])),
        None => (body, None),
    };
    let exp: i32 = match exp {
        Some(e) => {
            let digits = e.strip_prefix(['+', '-']).unwrap_or(e);
            if digits.is_empty() || digits.len() > 4 || !digits.bytes().all(|b| b.is_ascii_digit())
            {
                return None;
            }
            e.parse().ok()?
        }
        None => 0,
    };
    if !mant
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == ',' || is_group_space(c))
    {
        return None;
    }
    let dots = mant.matches('.').count();
    let commas = mant.matches(',').count();
    let (dec, group): (Option<char>, Option<char>) = if dots > 0 && commas > 0 {
        let last_dot = mant.rfind('.')?;
        let last_comma = mant.rfind(',')?;
        if last_dot > last_comma {
            if dots > 1 {
                return None;
            }
            (Some('.'), Some(','))
        } else {
            if commas > 1 {
                return None;
            }
            (Some(','), Some('.'))
        }
    } else if commas == 1 {
        (Some(','), None)
    } else if commas > 1 {
        (None, Some(','))
    } else if dots == 1 {
        (Some('.'), None)
    } else if dots > 1 {
        (None, Some('.'))
    } else {
        (None, None)
    };
    let (int, frac) = match dec {
        Some(d) => mant.split_once(d)?,
        None => (mant, ""),
    };
    if !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let is_group = |c: char| is_group_space(c) || Some(c) == group;
    let int_digits: String = if int.chars().any(is_group) {
        let groups: Vec<&str> = int.split(is_group).collect();
        let first = groups.first()?;
        if first.is_empty()
            || first.len() > 3
            || groups[1..].iter().any(|g| g.len() != 3)
            || !groups.iter().all(|g| g.bytes().all(|b| b.is_ascii_digit()))
        {
            return None;
        }
        groups.concat()
    } else {
        if !int.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        int.to_string()
    };
    if int_digits.is_empty() && frac.is_empty() {
        return None;
    }
    let text = format!(
        "{}{}.{}e{exp}",
        if neg { "-" } else { "" },
        if int_digits.is_empty() {
            "0"
        } else {
            &int_digits
        },
        if frac.is_empty() { "0" } else { frac }
    );
    let n: f64 = text.parse().ok()?;
    n.is_finite().then_some((n, frac.len()))
}

/// Lit un nombre écrit à la française ou à l'anglaise.
pub fn parse_number(s: &str) -> Option<f64> {
    parse_number_dec(s).map(|(n, _)| n)
}

/// Valeur d'une saisie qui n'est pas une formule, avec le format qu'elle suggère
/// (`12,5 %` → 0,125 en pourcentage, `15/03/2026` → date…).
pub(crate) fn parse_literal(input: &str) -> (Value, Option<NumberFormat>) {
    if input.is_empty() {
        return (Value::Empty, None);
    }
    if let Some(rest) = input.strip_prefix('\'') {
        return (Value::Text(rest.to_string()), None);
    }
    let t = input.trim();
    if t.is_empty() {
        return (Value::Text(input.to_string()), None);
    }
    match t.to_uppercase().as_str() {
        "VRAI" | "TRUE" => return (Value::Bool(true), None),
        "FAUX" | "FALSE" => return (Value::Bool(false), None),
        _ => {}
    }
    if let Some(p) = t.strip_suffix('%')
        && let Some((n, dec)) = parse_number_dec(p)
    {
        let n = n / 100.0;
        if n.is_finite() {
            return (Value::Number(n), Some(NumberFormat::Percent(dec.min(15) as u8)));
        }
    }
    let money = t
        .strip_suffix('€')
        .or_else(|| t.strip_prefix('€'))
        .or_else(|| t.strip_suffix("EUR"));
    if let Some(m) = money
        && let Some((n, dec)) = parse_number_dec(m)
    {
        let dec = if dec <= 2 { 2 } else { dec.min(15) };
        return (Value::Number(n), Some(NumberFormat::Currency(dec as u8)));
    }
    if let Some(serial) = parse_date(t) {
        return (Value::Number(serial), Some(NumberFormat::Date));
    }
    if let Some(n) = parse_number(t) {
        return (Value::Number(n), None);
    }
    (Value::Text(input.to_string()), None)
}

// ---------------------------------------------------------------- Fonction TEXTE

const JOURS: [&str; 7] = [
    "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche",
];
const MOIS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

/// Découpe un motif en morceaux : texte littéral (guillemets, `\x`) ou code.
fn pattern_parts(pattern: &str) -> Vec<(bool, String)> {
    let mut parts: Vec<(bool, String)> = Vec::new();
    let mut push = |lit: bool, c: char| match parts.last_mut() {
        Some((l, s)) if *l == lit => s.push(c),
        _ => parts.push((lit, c.to_string())),
    };
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                for q in chars.by_ref() {
                    if q == '"' {
                        break;
                    }
                    push(true, q);
                }
            }
            '\\' => {
                if let Some(n) = chars.next() {
                    push(true, n);
                }
            }
            c => push(false, c),
        }
    }
    parts
}

fn is_date_pattern(code: &str) -> bool {
    let lower = code.to_lowercase();
    let has_digit_placeholder = lower.contains(['0', '#', '?']);
    lower.contains(['j', 'd', 'a', 'y']) || (lower.contains('m') && !has_digit_placeholder)
}

/// Mise en forme d'un nombre par un motif (fonction `TEXTE`) : `0,00`, `# ##0,00 €`,
/// `0 %`, `#,##0.00`, `jj/mm/aaaa`, `dddd d mmmm yyyy`…
pub(crate) fn text_format(n: f64, pattern: &str) -> String {
    let trimmed = pattern.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("standard")
        || trimmed.eq_ignore_ascii_case("general")
        || trimmed == "@"
    {
        return format_general(n);
    }
    let parts = pattern_parts(pattern);
    let code: String = parts
        .iter()
        .filter(|(lit, _)| !lit)
        .map(|(_, s)| s.as_str())
        .collect();
    if is_date_pattern(&code) {
        return date_pattern(n, &parts);
    }
    number_pattern(n, &parts)
}

fn date_pattern(n: f64, parts: &[(bool, String)]) -> String {
    let Some((y, m, d)) = serial_to_ymd(n) else {
        return format_general(n);
    };
    let weekday = (n.floor() as i64 + 5).rem_euclid(7) as usize; // 0 = lundi
    let mut out = String::new();
    for (lit, s) in parts {
        if *lit {
            out.push_str(s);
            continue;
        }
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;
        while let Some(&c) = chars.get(i) {
            let lc = c.to_ascii_lowercase();
            let mut run = 1;
            while chars.get(i + run).map(|x| x.to_ascii_lowercase()) == Some(lc) {
                run += 1;
            }
            match lc {
                'j' | 'd' => match run {
                    1 => out.push_str(&d.to_string()),
                    2 => out.push_str(&format!("{d:02}")),
                    3 => out.push_str(JOURS[weekday].get(..3).unwrap_or("")),
                    _ => out.push_str(JOURS[weekday]),
                },
                'm' => {
                    let name = MOIS[(m as usize).saturating_sub(1) % 12];
                    match run {
                        1 => out.push_str(&m.to_string()),
                        2 => out.push_str(&format!("{m:02}")),
                        3 => out.push_str(&name.chars().take(4).collect::<String>()),
                        _ => out.push_str(name),
                    }
                }
                'a' | 'y' => {
                    if run <= 2 {
                        out.push_str(&format!("{:02}", y.rem_euclid(100)));
                    } else {
                        out.push_str(&format!("{y:04}"));
                    }
                }
                _ => {
                    for _ in 0..run {
                        out.push(c);
                    }
                }
            }
            i += run;
        }
    }
    out
}

fn number_pattern(n: f64, parts: &[(bool, String)]) -> String {
    // Le cœur numérique va du premier au dernier caractère 0, # ou ? du code.
    let is_ph = |c: char| matches!(c, '0' | '#' | '?');
    let mut prefix = String::new();
    let mut core = String::new();
    let mut suffix = String::new();
    let mut tail = String::new(); // code lu après le dernier marqueur provisoire
    let mut seen_ph = false;
    let mut percent = false;
    for (lit, s) in parts {
        if *lit {
            if seen_ph {
                tail.push_str(s);
            } else {
                prefix.push_str(s);
            }
            continue;
        }
        for c in s.chars() {
            if c == '%' {
                percent = true;
            }
            if is_ph(c) {
                if seen_ph {
                    core.push_str(&tail);
                    tail.clear();
                }
                seen_ph = true;
                core.push(c);
            } else if seen_ph {
                tail.push(c);
            } else {
                prefix.push(c);
            }
        }
    }
    suffix.push_str(&tail);
    if !seen_ph {
        return format!("{prefix}{}", format_general(n));
    }
    // Séparateur décimal : le dernier des deux s'ils sont présents ; une virgule
    // seule suivie de trois marqueurs après un `#` est un séparateur de milliers
    // à l'anglaise (`#,##0`).
    let dot = core.rfind('.');
    let comma = core.rfind(',');
    let dec_pos = match (dot, comma) {
        (Some(d), Some(c)) => Some(d.max(c)),
        (Some(d), None) => Some(d),
        (None, Some(c)) => {
            let after = &core[c + 1..];
            let before = &core[..c];
            if after.len() == 3 && after.chars().all(is_ph) && before.ends_with('#') {
                None
            } else {
                Some(c)
            }
        }
        (None, None) => None,
    };
    let (int_part, frac_part) = match dec_pos {
        Some(p) => (&core[..p], &core[p + 1..]),
        None => (core.as_str(), ""),
    };
    let decimals = frac_part.chars().filter(|&c| is_ph(c)).count();
    let grouping = int_part.chars().any(|c| !is_ph(c));
    let min_int = int_part.chars().filter(|&c| c == '0').count();
    let value = if percent { n * 100.0 } else { n };
    let digits = format_fixed_min(value, decimals, grouping, min_int);
    format!("{prefix}{digits}{suffix}")
}

/// Traduit un motif de `TEXTE` français vers l'anglais (exports) : `0,00` → `0.00`,
/// `jj/mm/aaaa` → `dd/mm/yyyy`.
pub(crate) fn pattern_to_english(pattern: &str) -> String {
    let parts = pattern_parts(pattern);
    let code: String = parts
        .iter()
        .filter(|(lit, _)| !lit)
        .map(|(_, s)| s.as_str())
        .collect();
    let date = is_date_pattern(&code);
    let has_dot = code.contains('.');
    let mut out = String::new();
    for (lit, s) in &parts {
        if *lit {
            out.push('"');
            out.push_str(&s.replace('"', ""));
            out.push('"');
            continue;
        }
        for c in s.chars() {
            match c {
                'j' | 'J' if date => out.push('d'),
                'a' | 'A' if date => out.push('y'),
                ',' if !date && !has_dot => out.push('.'),
                ' ' | NBSP | THOUSANDS_SEP if !date => {
                    // Espace entre deux marqueurs : séparateur de milliers.
                    out.push(',');
                }
                c => out.push(c),
            }
        }
    }
    // Une espace avant `%` ou en fin de motif n'est pas un séparateur de milliers.
    out.replace(",%", " %")
        .trim_end_matches(',')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_standard() {
        assert_eq!(format_general(12.0), "12");
        assert_eq!(format_general(-3.0), "-3");
        assert_eq!(format_general(1234.5), "1234,5");
        assert_eq!(format_general(0.1 + 0.2), "0,3");
        assert_eq!(format_general(1.0 / 3.0), "0,3333333333");
        assert_eq!(format_general(123_456_789.123), "123456789,123");
        assert_eq!(format_general(1e20), "1E+20");
        assert_eq!(format_general(-1.5e-12), "-1,5E-12");
        assert_eq!(format_general(f64::NAN), "#NOMBRE!");
    }

    #[test]
    fn format_fixe_a_la_francaise() {
        assert_eq!(
            format_number(1234.5, NumberFormat::Number(2)),
            "1\u{202F}234,50"
        );
        assert_eq!(
            format_number(-1234567.891, NumberFormat::Number(1)),
            "-1\u{202F}234\u{202F}567,9"
        );
        assert_eq!(format_number(-0.001, NumberFormat::Number(2)), "0,00");
        assert_eq!(
            format_number(1234.5, NumberFormat::Currency(2)),
            "1\u{202F}234,50\u{A0}€"
        );
        assert_eq!(format_number(0.125, NumberFormat::Percent(1)), "12,5\u{A0}%");
        assert_eq!(format_number(0.125, NumberFormat::Percent(0)), "13\u{A0}%");
        assert_eq!(format_number(999.0, NumberFormat::Number(0)), "999");
    }

    #[test]
    fn dates_en_numeros_de_serie() {
        assert_eq!(date_serial(1899, 12, 30), 0.0);
        assert_eq!(date_serial(1900, 3, 1), 61.0);
        assert_eq!(date_serial(2026, 3, 15), 46096.0);
        assert_eq!(date_serial(2026, 13, 1), date_serial(2027, 1, 1));
        assert_eq!(date_serial(2026, 3, 0), date_serial(2026, 2, 28));
        assert_eq!(serial_to_ymd(46096.0), Some((2026, 3, 15)));
        assert_eq!(serial_to_ymd(46096.75), Some((2026, 3, 15)));
        assert_eq!(serial_to_ymd(f64::INFINITY), None);
        assert_eq!(format_number(46096.0, NumberFormat::Date), "15/03/2026");
        for s in [-1000.0, 0.0, 59.0, 60.0, 61.0, 36526.0, 2_958_465.0] {
            let (y, m, d) = serial_to_ymd(s).unwrap();
            assert_eq!(date_serial(y, m as i32, d as i32), s);
        }
    }

    #[test]
    fn lecture_des_dates() {
        assert_eq!(parse_date("15/03/2026"), Some(46096.0));
        assert_eq!(parse_date("15/3/26"), Some(46096.0));
        assert_eq!(parse_date("2026-03-15"), Some(46096.0));
        assert_eq!(parse_date("29/02/2025"), None);
        assert_eq!(parse_date("29/02/2024").is_some(), true);
        assert_eq!(parse_date("32/01/2026"), None);
        assert_eq!(parse_date("1/2"), None);
        assert_eq!(parse_date("aa/bb/cccc"), None);
    }

    #[test]
    fn lecture_des_nombres() {
        assert_eq!(parse_number("12"), Some(12.0));
        assert_eq!(parse_number("12,5"), Some(12.5));
        assert_eq!(parse_number("12.5"), Some(12.5));
        assert_eq!(parse_number("1 234,5"), Some(1234.5));
        assert_eq!(parse_number("1\u{202F}234,5"), Some(1234.5));
        assert_eq!(parse_number("1\u{A0}234\u{A0}567"), Some(1_234_567.0));
        assert_eq!(parse_number("1.234,5"), Some(1234.5));
        assert_eq!(parse_number("1,234.5"), Some(1234.5));
        assert_eq!(parse_number("1,234,567"), Some(1_234_567.0));
        assert_eq!(parse_number("-0,5"), Some(-0.5));
        assert_eq!(parse_number(",5"), Some(0.5));
        assert_eq!(parse_number("1,5E3"), Some(1500.0));
        assert_eq!(parse_number("1e-2"), Some(0.01));
        assert_eq!(parse_number("12 34"), None);
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("-"), None);
        assert_eq!(parse_number("1e999"), None);
        assert_eq!(parse_number("1.2.3,4"), None);
    }

    #[test]
    fn saisies_reconnues() {
        assert_eq!(parse_literal("12"), (Value::Number(12.0), None));
        assert_eq!(
            parse_literal("12,5 %"),
            (Value::Number(0.125), Some(NumberFormat::Percent(1)))
        );
        assert_eq!(
            parse_literal("1 234,50 €"),
            (Value::Number(1234.5), Some(NumberFormat::Currency(2)))
        );
        assert_eq!(
            parse_literal("15/03/2026"),
            (Value::Number(46096.0), Some(NumberFormat::Date))
        );
        assert_eq!(parse_literal("vrai"), (Value::Bool(true), None));
        assert_eq!(parse_literal("'12"), (Value::Text("12".into()), None));
        assert_eq!(parse_literal("Bonjour"), (Value::Text("Bonjour".into()), None));
        assert_eq!(parse_literal(""), (Value::Empty, None));
    }

    #[test]
    fn motifs_de_texte() {
        assert_eq!(text_format(1234.567, "0,00"), "1234,57");
        assert_eq!(text_format(1234.567, "0.00"), "1234,57");
        assert_eq!(text_format(1234.567, "# ##0,00"), "1\u{202F}234,57");
        assert_eq!(text_format(1234.567, "#,##0.00"), "1\u{202F}234,57");
        assert_eq!(text_format(1234.0, "#,##0"), "1\u{202F}234");
        assert_eq!(text_format(0.256, "0 %"), "26 %");
        assert_eq!(text_format(0.256, "0,0%"), "25,6%");
        assert_eq!(text_format(7.0, "000"), "007");
        assert_eq!(text_format(12.5, "0,00 €"), "12,50 €");
        assert_eq!(text_format(46096.0, "jj/mm/aaaa"), "15/03/2026");
        assert_eq!(text_format(46096.0, "dd/mm/yyyy"), "15/03/2026");
        assert_eq!(text_format(46096.0, "jjjj j mmmm aaaa"), "dimanche 15 mars 2026");
        assert_eq!(text_format(46096.0, "mmm aa"), "mars 26");
        assert_eq!(text_format(3.5, "Standard"), "3,5");
    }

    #[test]
    fn motifs_traduits() {
        assert_eq!(pattern_to_english("0,00"), "0.00");
        assert_eq!(pattern_to_english("# ##0,00"), "#,##0.00");
        assert_eq!(pattern_to_english("jj/mm/aaaa"), "dd/mm/yyyy");
        assert_eq!(pattern_to_english("0 %"), "0 %");
        assert_eq!(pattern_to_english("#,##0.00"), "#,##0.00");
    }
}
