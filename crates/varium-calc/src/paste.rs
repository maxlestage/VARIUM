//! Collage intelligent : tableaux venus d'autres applications (tabulations,
//! CSV, points-virgules, HTML, Markdown) découpés en cellules, et valeurs
//! converties (nombres à la française ou à l'anglaise, pourcentages, montants,
//! dates) avec le format qui leur va. Aussi le « Collage spécial ».

use crate::eval::is_formula;
use crate::format::{
    CellFormat, NumberFormat, date_serial, parse_date, parse_literal, parse_number_dec,
    serial_to_ymd,
};
use crate::refs::{CellRef, MAX_COLS, MAX_ROWS, Range};
use crate::rewrite::{normalize_formula, shift_formula, transpose_formula};
use crate::value::Value;
use crate::workbook::{Clip, ClipCell, Workbook};

/// Nombre maximal de lignes et de colonnes lues dans un texte collé.
const MAX_PASTE_ROWS: usize = 100_000;
const MAX_PASTE_COLS: usize = 1_000;

/// Forme du texte collé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteSource {
    /// Valeurs séparées par des tabulations (tableurs, navigateurs).
    Tabs,
    /// Valeurs séparées par ce caractère (`;` ou `,`).
    Delimited(char),
    /// Tableau HTML (`<table>`).
    Html,
    /// Tableau Markdown ou Varium (`| a | b |`).
    Markdown,
    /// Une valeur par ligne.
    Lines,
}

/// Découpe un texte collé en lignes et cellules, et dit d'où il vient.
pub fn parse_table(text: &str) -> (PasteSource, Vec<Vec<String>>) {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let lower_has_table = text.len() < 64 * 1024 * 1024
        && text
            .as_bytes()
            .windows(6)
            .any(|w| w.eq_ignore_ascii_case(b"<table"));
    if lower_has_table {
        let rows = html_rows(&text);
        if !rows.is_empty() {
            return (PasteSource::Html, rows);
        }
    }
    if let Some(rows) = markdown_rows(&text) {
        return (PasteSource::Markdown, rows);
    }
    let body = text.strip_suffix('\n').unwrap_or(&text);
    if body.contains('\t') {
        return (PasteSource::Tabs, delimited(body, '\t'));
    }
    let lines: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).collect();
    for d in [';', ','] {
        if d == ',' && lines.len() < 2 {
            continue;
        }
        let counts: Vec<usize> = lines.iter().map(|l| delimited(l, d)[0].len()).collect();
        if !counts.is_empty() && counts.iter().all(|&c| c >= 2 && c == counts[0]) {
            return (PasteSource::Delimited(d), delimited(body, d));
        }
    }
    if !lines.is_empty() && lines.iter().all(|l| l.contains(';')) {
        return (PasteSource::Delimited(';'), delimited(body, ';'));
    }
    let rows = body
        .split('\n')
        .take(MAX_PASTE_ROWS)
        .map(|l| vec![l.to_string()])
        .collect();
    (PasteSource::Lines, rows)
}

/// Découpage avec guillemets à la CSV (`"a;b"`, `""` pour un guillemet, retours
/// à la ligne permis entre guillemets).
fn delimited(text: &str, d: char) -> Vec<Vec<String>> {
    let mut rows = vec![Vec::new()];
    let mut field = String::new();
    let mut quoted = false;
    let mut at_start = true;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if at_start => {
                quoted = true;
                at_start = false;
            }
            '\n' => {
                if let Some(r) = rows.last_mut() {
                    push_field(r, std::mem::take(&mut field));
                }
                if rows.len() >= MAX_PASTE_ROWS {
                    return rows;
                }
                rows.push(Vec::new());
                at_start = true;
            }
            c if c == d => {
                if let Some(r) = rows.last_mut() {
                    push_field(r, std::mem::take(&mut field));
                }
                at_start = true;
            }
            c => {
                if !(at_start && c == ' ' && d != '\t') {
                    at_start = false;
                }
                field.push(c);
            }
        }
    }
    if let Some(r) = rows.last_mut() {
        push_field(r, field);
    }
    rows
}

fn push_field(row: &mut Vec<String>, f: String) {
    if row.len() < MAX_PASTE_COLS {
        row.push(f);
    }
}

/// Tableau Markdown : au moins deux lignes `| … |`, la ligne `|---|` ignorée.
fn markdown_rows(text: &str) -> Option<Vec<Vec<String>>> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.len() < 2 || !lines.iter().all(|l| l.starts_with('|') && l.len() > 1) {
        return None;
    }
    let mut rows = Vec::new();
    for l in lines.into_iter().take(MAX_PASTE_ROWS) {
        let inner = l.strip_prefix('|').unwrap_or(l);
        let inner = inner.strip_suffix('|').unwrap_or(inner);
        let mut cells = Vec::new();
        let mut cur = String::new();
        let mut chars = inner.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\\' if chars.peek() == Some(&'|') => {
                    cur.push('|');
                    chars.next();
                }
                '|' => cells.push(std::mem::take(&mut cur).trim().to_string()),
                c => cur.push(c),
            }
        }
        cells.push(cur.trim().to_string());
        let separator = cells.iter().all(|c| {
            !c.is_empty() && c.contains('-') && c.chars().all(|ch| matches!(ch, '-' | ':' | ' '))
        });
        if !separator {
            cells.truncate(MAX_PASTE_COLS);
            rows.push(cells);
        }
    }
    (!rows.is_empty()).then_some(rows)
}

/// Décode les entités HTML usuelles.
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let end = tail[1..]
            .char_indices()
            .take(12)
            .find(|(_, c)| *c == ';')
            .map(|(j, _)| j + 1);
        let decoded = end.and_then(|e| {
            let name = &tail[1..e];
            let c = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{A0}'),
                "euro" => Some('€'),
                _ => {
                    let num = name.strip_prefix('#')?;
                    let code = match num.strip_prefix(['x', 'X']) {
                        Some(h) => u32::from_str_radix(h, 16).ok()?,
                        None => num.parse().ok()?,
                    };
                    char::from_u32(code)
                }
            };
            c.map(|c| (c, e))
        });
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &tail[e + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Valeur d'un attribut dans le texte d'une balise (`colspan="2"`).
fn attr_value(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(name) {
        let at = from + i;
        from = at + name.len();
        let before_ok = at == 0 || !lower.as_bytes()[at - 1].is_ascii_alphanumeric();
        let rest = lower[from..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let v = rest[1..].trim_start();
        let v = v.trim_start_matches(['"', '\'']);
        let end = v
            .find(|c: char| c == '"' || c == '\'' || c.is_whitespace() || c == '>')
            .unwrap_or(v.len());
        return Some(v[..end].to_string());
    }
    None
}

/// Lignes d'un tableau HTML : `<tr>`, `<td>` / `<th>` (avec `colspan`),
/// balises internes retirées, `<br>` changés en espaces.
fn html_rows(html: &str) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Option<Vec<String>> = None;
    let mut cell: Option<(String, usize)> = None;
    let mut skip_until: Option<&str> = None;
    let mut rest = html;
    loop {
        let Some(lt) = rest.find('<') else {
            if let Some((c, _)) = &mut cell {
                c.push_str(rest);
            }
            break;
        };
        let text = &rest[..lt];
        if skip_until.is_none()
            && let Some((c, _)) = &mut cell
        {
            c.push_str(text);
        }
        let after = &rest[lt + 1..];
        // Commentaires.
        if after.starts_with("!--") {
            match after.find("-->") {
                Some(e) => {
                    rest = &after[e + 3..];
                    continue;
                }
                None => break,
            }
        }
        let Some(gt) = after.find('>') else {
            break;
        };
        let tag = &after[..gt];
        rest = &after[gt + 1..];
        let closing = tag.starts_with('/');
        let name: String = tag
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        if let Some(end) = skip_until {
            if closing && name == end {
                skip_until = None;
            }
            continue;
        }
        let finish_cell = |row: &mut Option<Vec<String>>, cell: &mut Option<(String, usize)>| {
            if let Some((text, span)) = cell.take() {
                let r = row.get_or_insert_with(Vec::new);
                let clean = decode_entities(&text)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                let clean = clean.replace('\u{A0}', " ").trim().to_string();
                if r.len() < MAX_PASTE_COLS {
                    r.push(clean);
                }
                for _ in 1..span.min(MAX_PASTE_COLS) {
                    if r.len() < MAX_PASTE_COLS {
                        r.push(String::new());
                    }
                }
            }
        };
        match (name.as_str(), closing) {
            ("style" | "script" | "head" | "title", false) => {
                skip_until = Some(match name.as_str() {
                    "style" => "style",
                    "script" => "script",
                    "head" => "head",
                    _ => "title",
                });
            }
            ("tr", false) => {
                finish_cell(&mut row, &mut cell);
                if let Some(r) = row.take()
                    && rows.len() < MAX_PASTE_ROWS
                {
                    rows.push(r);
                }
                row = Some(Vec::new());
            }
            ("tr", true) | ("table", true) => {
                finish_cell(&mut row, &mut cell);
                if let Some(r) = row.take()
                    && rows.len() < MAX_PASTE_ROWS
                {
                    rows.push(r);
                }
            }
            ("td" | "th", false) => {
                finish_cell(&mut row, &mut cell);
                let span = attr_value(tag, "colspan")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(1)
                    .clamp(1, 1000);
                cell = Some((String::new(), span));
            }
            ("td" | "th", true) => finish_cell(&mut row, &mut cell),
            ("br" | "p" | "div" | "li", _) => {
                if let Some((c, _)) = &mut cell {
                    c.push(' ');
                }
            }
            _ => {}
        }
    }
    if let Some(r) = row.take()
        && !r.is_empty()
    {
        rows.push(r);
    }
    rows.retain(|r| !r.is_empty());
    rows
}

// ---------------------------------------------------------------- Valeurs

const MOIS_FR: [&str; 12] = [
    "janvier",
    "fevrier",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "aout",
    "septembre",
    "octobre",
    "novembre",
    "decembre",
];
const MOIS_EN: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'û' | 'ü' | 'ù' => 'u',
            'à' | 'â' => 'a',
            'ô' => 'o',
            'î' | 'ï' => 'i',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

/// Numéro du mois (1 à 12) d'un nom français ou anglais, entier ou abrégé
/// (`mars`, `févr.`, `Sept`, `Dec`).
fn month_number(word: &str) -> Option<u32> {
    let w = fold(word.trim_end_matches('.'));
    if w.len() < 3 {
        return None;
    }
    for table in [MOIS_FR, MOIS_EN] {
        if let Some(i) = table.iter().position(|m| *m == w) {
            return Some(i as u32 + 1);
        }
    }
    for table in [MOIS_FR, MOIS_EN] {
        let found: Vec<usize> = table
            .iter()
            .enumerate()
            .filter(|(_, m)| m.starts_with(&w))
            .map(|(i, _)| i)
            .collect();
        if let [i] = found[..] {
            return Some(i as u32 + 1);
        }
    }
    // « juil » et « juin » : abréviations ambiguës traitées par le préfixe ci-dessus.
    None
}

fn valid_date(y: i32, m: u32, d: u32) -> Option<f64> {
    if !(1..=12).contains(&m) || d == 0 || !(1000..=9999).contains(&y) {
        return None;
    }
    let serial = date_serial(y, m as i32, d as i32);
    // Le 31 février déborde sur mars : refusé.
    match serial_to_ymd(serial) {
        Some((yy, mm, dd)) if yy == y && mm == m && dd == d => Some(serial),
        _ => None,
    }
}

/// Dates écrites en toutes lettres (`3 mars 2026`, `1er avril 2026`,
/// `March 3, 2026`, `3 Mar 2026`) ou avec des points (`15.03.2026`).
fn parse_long_date(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.len() > 40 {
        return None;
    }
    // 15.03.2026
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        && parts[2].len() == 4
    {
        let d = parts[0].parse().ok()?;
        let m = parts[1].parse().ok()?;
        let y = parts[2].parse().ok()?;
        return valid_date(y, m, d);
    }
    let words: Vec<String> = s
        .replace(',', " ")
        .split_whitespace()
        .map(str::to_string)
        .collect();
    // Jour de la semaine en tête : « lundi 3 mars 2026 », « Monday, March 3, 2026 ».
    let words: Vec<&str> = match words.first().map(|w| fold(w)) {
        Some(w)
            if [
                "lundi",
                "mardi",
                "mercredi",
                "jeudi",
                "vendredi",
                "samedi",
                "dimanche",
                "monday",
                "tuesday",
                "wednesday",
                "thursday",
                "friday",
                "saturday",
                "sunday",
            ]
            .contains(&w.as_str()) =>
        {
            words[1..].iter().map(String::as_str).collect()
        }
        _ => words.iter().map(String::as_str).collect(),
    };
    let day = |w: &str| -> Option<u32> {
        let w = w
            .trim_end_matches("er")
            .trim_end_matches("st")
            .trim_end_matches("nd")
            .trim_end_matches("rd")
            .trim_end_matches("th");
        (w.len() <= 2 && !w.is_empty() && w.bytes().all(|b| b.is_ascii_digit()))
            .then(|| w.parse().ok())
            .flatten()
    };
    let year = |w: &str| -> Option<i32> {
        (w.len() == 4 && w.bytes().all(|b| b.is_ascii_digit()))
            .then(|| w.parse().ok())
            .flatten()
    };
    match words[..] {
        [d, m, y] => {
            if let (Some(d), Some(m), Some(y)) = (day(d), month_number(m), year(y)) {
                return valid_date(y, m, d);
            }
            if let (Some(m), Some(d), Some(y)) = (month_number(d), day(m), year(y)) {
                return valid_date(y, m, d);
            }
            None
        }
        _ => None,
    }
}

/// Le texte collé semble-t-il écrit à l'anglaise (point décimal) ?
fn english_decimals(fields: &[&str]) -> bool {
    let mut dot = 0;
    let mut comma = 0;
    for f in fields {
        let t = f
            .trim()
            .trim_start_matches(['$', '£', '-'])
            .trim_end_matches(['%', '€', '$']);
        let t = t.trim();
        if t.is_empty()
            || !t
                .bytes()
                .all(|b| b.is_ascii_digit() || b == b'.' || b == b',')
        {
            continue;
        }
        match (t.rfind('.'), t.rfind(',')) {
            (Some(d), Some(c)) if d > c => dot += 1,
            (Some(_), Some(_)) => comma += 1,
            (Some(d), None) if t.matches('.').count() == 1 && t.len() - d - 1 != 3 => dot += 1,
            (None, Some(c)) if t.matches(',').count() == 1 && t.len() - c - 1 != 3 => comma += 1,
            _ => {}
        }
    }
    dot > comma
}

fn number_input(n: f64) -> String {
    let s = format!("{n}");
    if s == "-0" { "0".into() } else { s }
}

/// Convertit une valeur collée en saisie Varium, avec le format qu'elle
/// suggère : `1 234,50 €` → 1234,5 monétaire, `$1,234.50` → 1234,5 à deux
/// décimales, `12,5 %` → 0,125 pourcentage, `3 mars 2026` → date. `english`
/// dit si le texte collé utilise le point décimal (`1,234` vaut alors 1234).
pub fn smart_input(raw: &str, english: bool) -> (String, NumberFormat) {
    let t = raw.trim_matches(|c: char| c.is_whitespace() || c == '\u{A0}' || c == '\u{202F}');
    if t.is_empty() {
        return (String::new(), NumberFormat::General);
    }
    if t.starts_with('=') || t.starts_with('\'') {
        return (t.to_string(), NumberFormat::General);
    }
    // Zéros en tête (codes postaux, numéros) : gardés comme texte.
    if t.len() > 1 && t.starts_with('0') && t.bytes().all(|b| b.is_ascii_digit()) {
        return (format!("'{t}"), NumberFormat::General);
    }
    // Négatif comptable : (1 234,50)
    let (neg, body) = match t.strip_prefix('(').and_then(|b| b.strip_suffix(')')) {
        Some(b) if b.chars().any(|c| c.is_ascii_digit()) => (true, b.trim()),
        _ => (false, t),
    };
    let sign = |n: f64| if neg { -n } else { n };
    let number = |s: &str| -> Option<(f64, usize)> {
        let s = s.trim();
        if english {
            let plain: String = s.chars().filter(|c| *c != ',').collect();
            if s.contains(',') && s.contains('.') && s.rfind(',') < s.rfind('.') {
                return parse_number_dec(&plain);
            }
            // 1,234 ou 12,345,678 : séparateurs des milliers.
            let groups: Vec<&str> = s.trim_start_matches('-').split(',').collect();
            if groups.len() > 1
                && !groups[0].is_empty()
                && groups[0].len() <= 3
                && groups[1..].iter().all(|g| g.len() == 3)
                && groups.iter().all(|g| g.bytes().all(|b| b.is_ascii_digit()))
            {
                return parse_number_dec(&plain);
            }
        }
        parse_number_dec(s)
    };
    // Pourcentages.
    if let Some(p) = body.strip_suffix('%')
        && let Some((n, dec)) = number(p)
    {
        let v = sign(n) / 100.0;
        if v.is_finite() {
            return (
                format!("{}%", number_input(sign(n))),
                NumberFormat::Percent(dec.min(15) as u8),
            );
        }
    }
    // Montants.
    let currencies: [(&str, bool); 7] = [
        ("€", true),
        ("EUR", true),
        ("US$", false),
        ("USD", false),
        ("$", false),
        ("£", false),
        ("GBP", false),
    ];
    for (sym, euro) in currencies {
        let stripped = body
            .strip_suffix(sym)
            .or_else(|| body.strip_prefix(sym))
            .or_else(|| {
                // -$5 ou -5 €
                body.strip_prefix('-')
                    .and_then(|b| b.trim_start().strip_prefix(sym))
                    .map(|_| "")
            });
        let Some(m) = stripped else { continue };
        let m = if m.is_empty() {
            // Cas « -$5 » : on retire le symbole après le signe.
            let b = body.strip_prefix('-').unwrap_or(body).trim_start();
            let rest = b.strip_prefix(sym).unwrap_or(b);
            let Some((n, dec)) = number(rest) else {
                continue;
            };
            return currency_input(-sign(n), dec, euro);
        } else {
            m
        };
        if let Some((n, dec)) = number(m) {
            return currency_input(sign(n), dec, euro);
        }
    }
    if let Some(serial) = parse_date(body).or_else(|| parse_long_date(body))
        && !neg
    {
        return (iso_date(serial), NumberFormat::Date);
    }
    if let Some((n, dec)) = number(body) {
        let fmt = if english && dec > 0 && body.contains(',') {
            NumberFormat::Number(dec.min(15) as u8)
        } else {
            NumberFormat::General
        };
        return (number_input(sign(n)), fmt);
    }
    // Le reste est du texte ; s'il serait lu autrement, une apostrophe le garde.
    let (v, _) = parse_literal(t);
    if matches!(v, Value::Text(_)) {
        (t.to_string(), NumberFormat::General)
    } else {
        (format!("'{t}"), NumberFormat::General)
    }
}

fn currency_input(n: f64, dec: usize, euro: bool) -> (String, NumberFormat) {
    let dec = if dec <= 2 { 2 } else { dec.min(15) } as u8;
    (
        number_input(n),
        if euro {
            NumberFormat::Currency(dec)
        } else {
            NumberFormat::Number(dec)
        },
    )
}

fn iso_date(serial: f64) -> String {
    match serial_to_ymd(serial) {
        Some((y, m, d)) => format!("{y:04}-{m:02}-{d:02}"),
        None => number_input(serial),
    }
}

impl Clip {
    /// Texte collé depuis une autre application : le tableau est découpé
    /// (tabulations, CSV, HTML, Markdown) et les valeurs converties (voir
    /// [`smart_input`]).
    pub fn from_external(text: &str) -> Clip {
        let (_, rows) = parse_table(text);
        let fields: Vec<&str> = rows
            .iter()
            .flat_map(|r| r.iter().map(String::as_str))
            .take(10_000)
            .collect();
        let english = english_decimals(&fields);
        let mut clip = Clip::default();
        for (r, row) in rows.iter().enumerate().take(MAX_ROWS as usize) {
            clip.rows = r as u32 + 1;
            for (c, field) in row.iter().enumerate().take(MAX_COLS as usize) {
                clip.cols = clip.cols.max(c as u32 + 1);
                let (input, number) = smart_input(field, english);
                if input.is_empty() {
                    continue;
                }
                clip.cells.push(ClipCell {
                    row: r as u32,
                    col: c as u32,
                    input,
                    format: CellFormat {
                        number,
                        ..CellFormat::default()
                    },
                    display: field.trim().to_string(),
                    value: Value::Empty,
                });
            }
        }
        clip
    }

    /// Le texte du presse-papiers du système vient-il de cette copie ?
    pub fn matches_text(&self, text: &str) -> bool {
        let norm = |s: &str| {
            s.replace("\r\n", "\n")
                .trim_end_matches('\n')
                .lines()
                .map(|l| l.trim_end_matches('\t').to_string())
                .collect::<Vec<_>>()
                .join("\n")
        };
        norm(&self.to_text()) == norm(text)
    }
}

/// Ce que colle le « Collage spécial ».
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PasteWhat {
    /// Contenu et mise en forme.
    #[default]
    All,
    /// Valeurs calculées seules (les formules deviennent leur résultat) ;
    /// la mise en forme de la destination est gardée, sauf le format des nombres.
    Values,
    /// Saisies (formules décalées), sans la mise en forme.
    Formulas,
    /// Mise en forme seule.
    Formats,
}

/// Options du « Collage spécial ».
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PasteOptions {
    pub what: PasteWhat,
    /// Les lignes deviennent des colonnes.
    pub transpose: bool,
    /// Les cellules vides de la copie ne remplacent pas la destination.
    pub skip_blanks: bool,
}

/// Saisie qui redonne une valeur calculée.
fn value_input(v: &Value) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Number(n) => number_input(*n),
        Value::Bool(b) => if *b { "VRAI" } else { "FAUX" }.into(),
        Value::Error(e) => e.code().into(),
        Value::Text(s) => {
            if s.is_empty() {
                String::new()
            } else if s.starts_with('=')
                || s.starts_with('\'')
                || !matches!(parse_literal(s).0, Value::Text(_))
            {
                format!("'{s}")
            } else {
                s.clone()
            }
        }
    }
}

impl Workbook {
    /// Collage spécial à partir de `at` : tout, valeurs seules, formules ou
    /// formats, transposé ou non. Renvoie la zone collée.
    pub fn paste_special(
        &mut self,
        sheet: usize,
        at: CellRef,
        clip: &Clip,
        opts: PasteOptions,
    ) -> Option<Range> {
        if sheet >= self.sheets.len() || clip.is_empty() {
            return None;
        }
        let (rows, cols) = if opts.transpose {
            (clip.cols, clip.rows)
        } else {
            (clip.rows, clip.cols)
        };
        let end = CellRef::new(
            (u64::from(at.row) + u64::from(rows) - 1).min(u64::from(MAX_ROWS) - 1) as u32,
            (u64::from(at.col) + u64::from(cols) - 1).min(u64::from(MAX_COLS) - 1) as u32,
        );
        let area = Range::new(at, end);
        let place = |c: &ClipCell| -> Option<CellRef> {
            let (r, k) = if opts.transpose {
                (c.col, c.row)
            } else {
                (c.row, c.col)
            };
            at.offset(i64::from(r), i64::from(k))
        };
        let by_target: std::collections::HashMap<CellRef, &ClipCell> = clip
            .cells
            .iter()
            .filter_map(|c| Some((place(c)?, c)))
            .collect();
        let values: Vec<(CellRef, Option<&ClipCell>)> = area
            .iter()
            .take(4_000_000)
            .map(|t| (t, by_target.get(&t).copied()))
            .collect();
        let computed_values = self.snapshot_values(sheet);
        for (target, src) in values {
            if src.is_none() && opts.skip_blanks {
                continue;
            }
            let old = self
                .sheets
                .get(sheet)
                .and_then(|s| s.cell(target))
                .cloned()
                .unwrap_or_default();
            let shifted = |c: &ClipCell| -> String {
                match clip.origin {
                    Some(o) if opts.transpose => {
                        let src = o.offset(i64::from(c.row), i64::from(c.col)).unwrap_or(o);
                        transpose_formula(&c.input, src, target)
                    }
                    Some(o) => {
                        let drow = i64::from(target.row) - i64::from(o.row) - i64::from(c.row);
                        let dcol = i64::from(target.col) - i64::from(o.col) - i64::from(c.col);
                        shift_formula(&c.input, drow as i32, dcol as i32)
                    }
                    None if is_formula(&c.input) => normalize_formula(&c.input),
                    None => c.input.clone(),
                }
            };
            let (input, format) = match (opts.what, src) {
                (PasteWhat::All, Some(c)) => (shifted(c), c.format),
                (PasteWhat::All, None) => (String::new(), CellFormat::default()),
                (PasteWhat::Formulas, Some(c)) => (shifted(c), old.format),
                (PasteWhat::Formulas | PasteWhat::Values, None) => (String::new(), old.format),
                (PasteWhat::Values, Some(c)) => {
                    let input = if clip.origin.is_some() {
                        let v = if c.value.is_empty() && !c.input.is_empty() {
                            // Copie sans valeur mémorisée : celle de la cellule source.
                            clip.origin
                                .and_then(|o| o.offset(i64::from(c.row), i64::from(c.col)))
                                .and_then(|p| computed_values.get(&p).cloned())
                                .unwrap_or_default()
                        } else {
                            c.value.clone()
                        };
                        value_input(&v)
                    } else if is_formula(&c.input) {
                        format!("'{}", c.input)
                    } else {
                        c.input.clone()
                    };
                    let mut f = old.format;
                    f.number = c.format.number;
                    (input, f)
                }
                (PasteWhat::Formats, Some(c)) => (old.input.clone(), c.format),
                (PasteWhat::Formats, None) => (old.input.clone(), CellFormat::default()),
            };
            if let Some(sh) = self.sheets.get_mut(sheet) {
                sh.put(target, input, format);
            }
        }
        self.invalidate();
        Some(area)
    }

    /// Valeurs calculées d'une feuille (copie), pour le collage des valeurs.
    fn snapshot_values(&self, sheet: usize) -> std::collections::HashMap<CellRef, Value> {
        self.sheets
            .get(sheet)
            .map(|s| {
                s.cells()
                    .take(200_000)
                    .map(|(c, _)| (c, self.value(sheet, c)))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(text: &str) -> Vec<Vec<String>> {
        parse_table(text).1
    }

    #[test]
    fn tabulations_et_csv() {
        let (src, rows) = parse_table("a\tb\n1\t2\n");
        assert_eq!(src, PasteSource::Tabs);
        assert_eq!(rows, vec![vec!["a", "b"], vec!["1", "2"]]);
        let (src, rows) = parse_table("nom;prix\npomme;1,5\n");
        assert_eq!(src, PasteSource::Delimited(';'));
        assert_eq!(rows[1], vec!["pomme", "1,5"]);
        let (src, rows) = parse_table("name,price\n\"Smith, J\",\"1,234.5\"\n");
        assert_eq!(src, PasteSource::Delimited(','));
        assert_eq!(rows[1], vec!["Smith, J", "1,234.5"]);
        // Une seule ligne avec une virgule : c'est une valeur (1,5), pas deux.
        assert_eq!(parse_table("1,5").0, PasteSource::Lines);
        assert_eq!(grid("un\ndeux"), vec![vec!["un"], vec!["deux"]]);
        assert_eq!(grid("\"a\nb\"\tc"), vec![vec!["a\nb", "c"]]);
    }

    #[test]
    fn tableaux_html() {
        let html = r#"<html><head><style>td{color:red}</style></head><body>
            <table><tr><th>Nom</th><th colspan="2">Prix &amp; taxe</th></tr>
            <tr><td><b>Pomme</b></td><td>1,50&nbsp;€</td><td>5<br>%</td></tr></table>"#;
        let (src, rows) = parse_table(html);
        assert_eq!(src, PasteSource::Html);
        assert_eq!(rows[0], vec!["Nom", "Prix & taxe", ""]);
        assert_eq!(rows[1], vec!["Pomme", "1,50 €", "5 %"]);
        // LibreOffice : balises en majuscules, cellules non fermées.
        let rows = grid("<TABLE><TR><TD>a<TD>b<TR><TD>c</TABLE>");
        assert_eq!(rows, vec![vec!["a", "b"], vec!["c"]]);
    }

    #[test]
    fn tableaux_markdown() {
        let md = "| Nom | Âge |\n|-----|:---:|\n| Léa | 31 |\n| a \\| b | 2 |";
        let (src, rows) = parse_table(md);
        assert_eq!(src, PasteSource::Markdown);
        assert_eq!(
            rows,
            vec![vec!["Nom", "Âge"], vec!["Léa", "31"], vec!["a | b", "2"]]
        );
    }

    #[test]
    fn valeurs_converties() {
        let fr = |s: &str| smart_input(s, false);
        assert_eq!(
            fr("1 234,50 €"),
            ("1234.5".into(), NumberFormat::Currency(2))
        );
        assert_eq!(fr("12,5 %"), ("12.5%".into(), NumberFormat::Percent(1)));
        assert_eq!(fr("15/03/2026"), ("2026-03-15".into(), NumberFormat::Date));
        assert_eq!(fr("2026-03-15"), ("2026-03-15".into(), NumberFormat::Date));
        assert_eq!(fr("3 mars 2026"), ("2026-03-03".into(), NumberFormat::Date));
        assert_eq!(
            fr("1er avril 2026"),
            ("2026-04-01".into(), NumberFormat::Date)
        );
        assert_eq!(
            fr("lundi 2 mars 2026"),
            ("2026-03-02".into(), NumberFormat::Date)
        );
        assert_eq!(
            fr("March 3, 2026"),
            ("2026-03-03".into(), NumberFormat::Date)
        );
        assert_eq!(fr("15.03.2026"), ("2026-03-15".into(), NumberFormat::Date));
        assert_eq!(fr("31 février 2026").1, NumberFormat::General);
        assert_eq!(fr("00123"), ("'00123".into(), NumberFormat::General));
        assert_eq!(fr("(12,5)"), ("-12.5".into(), NumberFormat::General));
        assert_eq!(fr("-12 €"), ("-12".into(), NumberFormat::Currency(2)));
        assert_eq!(fr("Bonjour"), ("Bonjour".into(), NumberFormat::General));
        assert_eq!(fr("=SOMME(A1:A2)").0, "=SOMME(A1:A2)");
        assert_eq!(fr("06 12 34 56 78").0, "06 12 34 56 78");
        assert_eq!(fr("").0, "");
        let en = |s: &str| smart_input(s, true);
        assert_eq!(en("$1,234.50"), ("1234.5".into(), NumberFormat::Number(2)));
        assert_eq!(en("1,234"), ("1234".into(), NumberFormat::General));
        assert_eq!(en("-$5"), ("-5".into(), NumberFormat::Number(2)));
        assert_eq!(en("3.5"), ("3.5".into(), NumberFormat::General));
    }

    #[test]
    fn collage_externe_dans_le_classeur() {
        let mut wb = Workbook::new();
        let clip = Clip::from_external("Produit\tPrix\tDate\nPomme\t1,50 €\t3 mars 2026\n");
        wb.paste(0, CellRef::new(0, 0), &clip);
        assert_eq!(wb.value(0, CellRef::new(1, 1)), Value::Number(1.5));
        assert_eq!(
            wb.format(0, CellRef::new(1, 1)).number,
            NumberFormat::Currency(2)
        );
        assert_eq!(wb.display(0, CellRef::new(1, 2)), "03/03/2026");
        // Anglais : le point décimal est détecté sur tout le texte.
        let clip = Clip::from_external("a,b\n1.5,\"1,234\"\n2.25,3\n");
        let mut wb = Workbook::new();
        wb.paste(0, CellRef::new(0, 0), &clip);
        assert_eq!(wb.value(0, CellRef::new(1, 1)), Value::Number(1234.0));
        assert_eq!(wb.value(0, CellRef::new(2, 0)), Value::Number(2.25));
    }

    #[test]
    fn collage_special() {
        let mut wb = Workbook::new();
        wb.set_input(0, CellRef::new(0, 0), "2");
        wb.set_input(0, CellRef::new(0, 1), "=A1*10");
        let mut f = wb.format(0, CellRef::new(0, 1));
        f.bold = true;
        wb.set_format(0, CellRef::new(0, 1), f);
        let clip = wb.copy_range(0, Range::parse("A1:B1").unwrap());
        assert!(clip.matches_text("2\t20\n"));
        // Valeurs seules : la formule devient 20.
        let opts = PasteOptions {
            what: PasteWhat::Values,
            ..PasteOptions::default()
        };
        wb.paste_special(0, CellRef::new(3, 0), &clip, opts);
        assert_eq!(wb.input(0, CellRef::new(3, 1)), "20");
        assert!(!wb.format(0, CellRef::new(3, 1)).bold);
        // Transposé : A1:B1 devient A6:A7, la formule décalée suit.
        let opts = PasteOptions {
            transpose: true,
            ..PasteOptions::default()
        };
        wb.paste_special(0, CellRef::new(5, 0), &clip, opts);
        assert_eq!(wb.input(0, CellRef::new(5, 0)), "2");
        assert_eq!(wb.input(0, CellRef::new(6, 0)), "=A6*10");
        wb.set_input(0, CellRef::new(20, 0), "=SOMME(B20:D20)");
        let clip2 = wb.copy_range(0, Range::parse("A21").unwrap());
        wb.paste_special(0, CellRef::new(22, 5), &clip2, opts);
        // Les décalages sont échangés : 1 ligne plus haut et 1 à 3 colonnes à
        // droite deviennent 1 colonne à gauche et 1 à 3 lignes plus bas.
        assert_eq!(wb.input(0, CellRef::new(22, 5)), "=SOMME(E24:E26)");
        assert_eq!(wb.value(0, CellRef::new(6, 0)), Value::Number(20.0));
        assert!(wb.format(0, CellRef::new(6, 0)).bold);
        // Formats seuls.
        wb.set_input(0, CellRef::new(9, 1), "x");
        let opts = PasteOptions {
            what: PasteWhat::Formats,
            ..PasteOptions::default()
        };
        wb.paste_special(0, CellRef::new(9, 0), &clip, opts);
        assert_eq!(wb.input(0, CellRef::new(9, 1)), "x");
        assert!(wb.format(0, CellRef::new(9, 1)).bold);
        // Formules sans mise en forme.
        let opts = PasteOptions {
            what: PasteWhat::Formulas,
            ..PasteOptions::default()
        };
        wb.paste_special(0, CellRef::new(12, 0), &clip, opts);
        assert_eq!(wb.input(0, CellRef::new(12, 1)), "=A13*10");
        assert!(!wb.format(0, CellRef::new(12, 1)).bold);
        // Du texte qui ressemble à un nombre reste du texte en valeurs seules.
        assert_eq!(value_input(&Value::Text("12".into())), "'12");
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    #[test]
    fn fuzz_sans_panique() {
        const PIECES: &[&str] = &[
            "<table",
            "<tr>",
            "<td colspan=\"9999999\">",
            "</td>",
            "<th>",
            "</tr>",
            "</table>",
            "<!--",
            "-->",
            "&amp;",
            "&#x1F600;",
            "&#99999999;",
            "&",
            ";",
            "|",
            "|---|",
            "\t",
            "\n",
            "\r\n",
            "\"",
            "\"\"",
            ",",
            ";",
            "1,5",
            "1.234,5",
            "€",
            "$",
            "%",
            "(",
            ")",
            "3 mars 2026",
            "31/02/2026",
            "0012",
            "é",
            "😀",
            "<",
            ">",
            "<br>",
            "<style>",
            "=",
            "'",
            " ",
            "-",
            "1e999",
            "9999999999999999999999",
        ];
        let mut rng = Rng(0xDEAD_BEEF_1234_5678);
        for _ in 0..3000 {
            let n = rng.next() % 16;
            let s: String = (0..n)
                .map(|_| PIECES[(rng.next() % PIECES.len() as u64) as usize])
                .collect();
            let clip = Clip::from_external(&s);
            let mut wb = Workbook::new();
            wb.paste(0, CellRef::new(0, 0), &clip);
            let _ = smart_input(&s, rng.next().is_multiple_of(2));
            let _ = wb.paste_special(
                0,
                CellRef::new(2, 2),
                &clip,
                PasteOptions {
                    what: PasteWhat::Values,
                    transpose: true,
                    skip_blanks: false,
                },
            );
        }
        // Octets arbitraires (UTF-8 valide).
        for _ in 0..500 {
            let bytes: Vec<u8> = (0..(rng.next() % 64)).map(|_| rng.next() as u8).collect();
            let s = String::from_utf8_lossy(&bytes);
            let _ = Clip::from_external(&s);
        }
    }
}
