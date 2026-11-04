//! Import de relevés bancaires (CSV, OFX, QIF) : colonnes reconnues (date,
//! libellé, débit et crédit ou montant, solde), dates et montants à la
//! française ou à l'anglaise, opérations classées par mots-clés (règles
//! modifiables, enregistrées dans le classeur), puis un tableau des
//! opérations, une synthèse par catégorie et par mois, et un graphique.

use serde::{Deserialize, Serialize};
use varium_core::Lang;

use crate::cond::{CompareOp, CondFormat, CondRule, CondStyle};
use crate::format::{CellFormat, HAlign, NumberFormat, date_serial, serial_to_ymd};
use crate::i18n::{loc, month_name};
use crate::refs::{CellRef, Range, col_name, quote_sheet_name};
use crate::value::Value;
use crate::workbook::{SheetChart, Workbook, collation_key};

/// Plus grand nombre d'opérations lues.
pub const MAX_TRANSACTIONS: usize = 100_000;

/// Une opération du relevé.
#[derive(Clone, Debug, PartialEq)]
pub struct Transaction {
    /// Date (numéro de série).
    pub date: f64,
    pub label: String,
    /// Montant signé : négatif pour un débit.
    pub amount: f64,
    /// Solde après l'opération, s'il est connu.
    pub balance: Option<f64>,
    /// Catégorie donnée par le fichier (QIF), sinon vide.
    pub category: String,
}

/// Règle de classement : une opération dont le libellé contient `keyword`
/// (sans casse ni accents) va dans `category`. La règle au mot-clé le plus
/// long l'emporte.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CategoryRule {
    pub keyword: String,
    pub category: String,
}

/// Les catégories proposées, en français (traduites par [`loc`]).
pub const CATEGORIES: [&str; 12] = [
    "Courses",
    "Transport",
    "Logement",
    "Abonnements",
    "Salaire",
    "Santé",
    "Loisirs",
    "Restaurants",
    "Impôts",
    "Retraits",
    "Virements",
    "Autres",
];

const DEFAULT_KEYWORDS: [(&str, &[&str]); 11] = [
    (
        "Courses",
        &[
            "carrefour",
            "leclerc",
            "auchan",
            "lidl",
            "aldi",
            "intermarche",
            "monoprix",
            "franprix",
            "casino",
            "super u",
            "hyper u",
            "picard",
            "biocoop",
            "supermarche",
            "grocery",
            "tesco",
            "walmart",
            "mercadona",
            "rewe",
            "edeka",
            "esselunga",
            "continente",
            "pingo doce",
            "coop",
            "spar",
        ],
    ),
    (
        "Transport",
        &[
            "sncf",
            "ratp",
            "uber",
            "bolt",
            "blablacar",
            "totalenergies",
            "esso",
            "shell",
            "carburant",
            "essence",
            "fuel",
            "peage",
            "autoroute",
            "parking",
            "navigo",
            "taxi",
            "renfe",
            "trenitalia",
            "deutsche bahn",
            "train",
            "metro",
        ],
    ),
    (
        "Logement",
        &[
            "loyer",
            "rent",
            "edf",
            "engie",
            "electricite",
            "veolia",
            "assurance habitation",
            "syndic",
            "mortgage",
            "hypotheque",
            "credit immo",
            "alquiler",
            "miete",
            "affitto",
            "renda",
            "gaz",
        ],
    ),
    (
        "Abonnements",
        &[
            "netflix",
            "spotify",
            "deezer",
            "amazon prime",
            "disney",
            "canal+",
            "orange",
            "sfr",
            "bouygues",
            "free mobile",
            "free telecom",
            "sosh",
            "apple.com",
            "icloud",
            "youtube",
            "abonnement",
            "subscription",
            "prelevement mensuel",
        ],
    ),
    (
        "Salaire",
        &[
            "salaire",
            "salary",
            "paie",
            "payroll",
            "nomina",
            "gehalt",
            "stipendio",
            "salario",
        ],
    ),
    (
        "Santé",
        &[
            "pharmacie",
            "pharmacy",
            "medecin",
            "docteur",
            "doctor",
            "dentiste",
            "hopital",
            "hospital",
            "cpam",
            "ameli",
            "mutuelle",
            "optique",
            "farmacia",
            "apotheke",
        ],
    ),
    (
        "Loisirs",
        &[
            "cinema",
            "fnac",
            "decathlon",
            "steam",
            "playstation",
            "theatre",
            "concert",
            "musee",
            "airbnb",
            "booking",
            "hotel",
            "voyage",
        ],
    ),
    (
        "Restaurants",
        &[
            "restaurant",
            "mcdonald",
            "burger",
            "kfc",
            "deliveroo",
            "uber eats",
            "just eat",
            "boulangerie",
            "brasserie",
            "pizzeria",
            "starbucks",
        ],
    ),
    (
        "Impôts",
        &[
            "impot",
            "dgfip",
            "tresor public",
            "taxe",
            "finanzamt",
            "hacienda",
        ],
    ),
    ("Retraits", &["retrait", "dab", "atm", "withdrawal", "cash"]),
    (
        "Virements",
        &[
            "virement",
            "transfer",
            "transferencia",
            "uberweisung",
            "bonifico",
            "vir ",
        ],
    ),
];

/// Les règles par défaut, catégories dans la langue donnée.
pub fn default_rules(lang: Lang) -> Vec<CategoryRule> {
    DEFAULT_KEYWORDS
        .iter()
        .flat_map(|(cat, words)| {
            words.iter().map(move |w| CategoryRule {
                keyword: (*w).to_string(),
                category: loc(lang, cat).to_string(),
            })
        })
        .collect()
}

/// La catégorie d'un libellé d'après les règles (le mot-clé le plus long
/// trouvé l'emporte).
pub fn categorize(label: &str, rules: &[CategoryRule]) -> Option<String> {
    let l = format!(" {} ", collation_key(label));
    rules
        .iter()
        .filter(|r| !r.keyword.trim().is_empty() && !r.category.trim().is_empty())
        .filter(|r| l.contains(&collation_key(r.keyword.trim_start())))
        .max_by_key(|r| r.keyword.chars().count())
        .map(|r| r.category.trim().to_string())
}

// ---------------------------------------------------------------- Lecture

/// Ordre des dates écrites avec des barres ou des points (année à la fin).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DateOrder {
    Dmy,
    Mdy,
}

/// Forme d'une date : année en tête (`2026-03-15`, `20260315`) ou en fin
/// (`15/03/26`, avec le nombre de chiffres de l'année).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    YearFirst,
    YearLast(usize),
}

fn split_date(s: &str) -> Option<(u32, u32, u32, Shape)> {
    let s = s.trim().split([' ', 'T']).next()?;
    // AAAAMMJJ (OFX), éventuellement suivi de l'heure.
    if s.len() >= 8 && s.bytes().take(8).all(|b| b.is_ascii_digit()) && !s.contains(['/', '-', '.'])
    {
        let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<u32>().ok();
        return Some((n(0..4)?, n(4..6)?, n(6..8)?, Shape::YearFirst));
    }
    let parts: Vec<&str> = s.split(['/', '-', '.', '\'']).map(str::trim).collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty() || p.len() > 4) {
        return None;
    }
    let n: Vec<u32> = parts
        .iter()
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let shape = if parts[0].len() == 4 {
        Shape::YearFirst
    } else {
        Shape::YearLast(parts[2].len())
    };
    Some((n[0], n[1], n[2], shape))
}

fn full_year(y: u32, len: usize) -> i32 {
    match len {
        1 | 2 if y < 70 => 2000 + y as i32,
        1 | 2 => 1900 + y as i32,
        _ => y.min(9999) as i32,
    }
}

fn make_date(y: i32, m: u32, d: u32) -> Option<f64> {
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || !(1900..=2200).contains(&y) {
        return None;
    }
    let s = date_serial(y, m as i32, d as i32);
    // Pas de 31 février.
    serial_to_ymd(s).filter(|(_, mm, _)| *mm == m).map(|_| s)
}

fn parse_date_with(s: &str, order: DateOrder) -> Option<f64> {
    let (a, b, c, shape) = split_date(s)?;
    match (shape, order) {
        (Shape::YearFirst, _) => make_date(a.min(9999) as i32, b, c),
        (Shape::YearLast(len), DateOrder::Mdy) => make_date(full_year(c, len), a, b),
        (Shape::YearLast(len), _) => make_date(full_year(c, len), b, a),
    }
}

/// L'ordre des dates d'une colonne : jour en premier (à la française) sauf
/// si un mois dépasserait 12.
fn detect_order<'a>(samples: impl Iterator<Item = &'a str>, prefer_us: bool) -> DateOrder {
    let (mut first_big, mut second_big) = (false, false);
    for s in samples {
        if let Some((a, b, _, Shape::YearLast(_))) = split_date(s) {
            first_big |= a > 12;
            second_big |= b > 12;
        }
    }
    match (first_big, second_big) {
        (true, _) => DateOrder::Dmy,
        (false, true) => DateOrder::Mdy,
        _ if prefer_us => DateOrder::Mdy,
        _ => DateOrder::Dmy,
    }
}

/// Montant : `1 234,56`, `-1,234.56`, `12,50 €`, `(12.50)`, `12.50-`, `+30`.
fn parse_amount(s: &str) -> Option<f64> {
    let mut t: String = s
        .trim()
        .chars()
        .filter(|c| !c.is_alphabetic() && !matches!(c, '€' | '$' | '£' | '¥' | '"'))
        .collect();
    t = t.trim().to_string();
    if t.is_empty() {
        return None;
    }
    let mut neg = false;
    if t.starts_with('(') && t.ends_with(')') {
        neg = true;
        t = t[1..t.len() - 1].to_string();
    }
    if let Some(r) = t.strip_suffix('-') {
        neg = !neg;
        t = r.trim().to_string();
    }
    if let Some(r) = t.strip_prefix('+') {
        t = r.trim().to_string();
    }
    // « 1.234,56 » (allemand) ou « 1,234.56 » (anglais) : le dernier
    // séparateur est décimal.
    let last_comma = t.rfind(',');
    let last_dot = t.rfind('.');
    let normalized = match (last_comma, last_dot) {
        (Some(c), Some(d)) if c > d => t.replace('.', "").replace(',', "."),
        (Some(_), Some(_)) => t.replace(',', ""),
        (Some(c), None) if t.len() - c - 1 == 3 && t.matches(',').count() > 1 => t.replace(',', ""),
        (Some(_), None) => t.replace(',', "."),
        _ => t,
    };
    let normalized: String = normalized
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{a0}' && *c != '\u{202f}' && *c != '\'')
        .collect();
    let x: f64 = normalized.parse().ok()?;
    x.is_finite().then_some(if neg { -x } else { x })
}

fn has_word(h: &str, words: &[&str]) -> bool {
    let h = collation_key(h);
    words.iter().any(|w| h.contains(w))
}

const DATE_WORDS: [&str; 7] = [
    "date", "fecha", "datum", "data", "booking", "buchung", "jour",
];
const VALUE_DATE_WORDS: [&str; 4] = ["valeur", "value", "valuta", "wert"];
const LABEL_WORDS: [&str; 15] = [
    "libelle",
    "description",
    "label",
    "detail",
    "memo",
    "concepto",
    "verwendungszweck",
    "descrizione",
    "descricao",
    "payee",
    "beneficiaire",
    "intitule",
    "operation",
    "motivo",
    "text",
];
const DEBIT_WORDS: [&str; 8] = [
    "debit",
    "withdrawal",
    "cargo",
    "soll",
    "addebito",
    "money out",
    "paid out",
    "sortie",
];
const CREDIT_WORDS: [&str; 8] = [
    "credit",
    "deposit",
    "abono",
    "haben",
    "accredito",
    "money in",
    "paid in",
    "entree",
];
const AMOUNT_WORDS: [&str; 7] = [
    "montant", "amount", "importe", "betrag", "importo", "valor", "somme",
];
const BALANCE_WORDS: [&str; 3] = ["solde", "balance", "saldo"];

fn detect_separator(text: &str) -> char {
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(30)
        .collect();
    let mut best = (',', 0usize);
    for sep in [';', '\t', ',', '|'] {
        let counts: Vec<usize> = lines.iter().map(|l| l.matches(sep).count()).collect();
        let max = counts.iter().copied().max().unwrap_or(0);
        let rows = counts.iter().filter(|&&c| c == max && c > 0).count();
        let score = rows * max.min(20);
        if score > best.1 {
            best = (sep, score);
        }
    }
    best.0
}

fn parse_csv(text: &str) -> Result<Vec<Transaction>, String> {
    let sep = detect_separator(text);
    let recs = crate::csv::records(text, sep)?;
    let recs: Vec<Vec<String>> = recs
        .into_iter()
        .filter(|r| r.iter().any(|f| !f.trim().is_empty()))
        .take(MAX_TRANSACTIONS + 50)
        .collect();
    let width = recs.iter().map(Vec::len).max().unwrap_or(0).min(64);
    fn field(r: &[String], c: usize) -> &str {
        r.get(c).map(|s| s.trim()).unwrap_or("")
    }
    let is_date = |s: &str| {
        parse_date_with(s, DateOrder::Dmy)
            .or_else(|| parse_date_with(s, DateOrder::Mdy))
            .is_some()
    };
    // Première ligne de données : la première qui a une date.
    let first_data = recs
        .iter()
        .position(|r| (0..width).any(|c| is_date(field(r, c))))
        .ok_or_else(|| "Aucune date trouvée dans le relevé".to_string())?;
    let header: Vec<String> = match first_data.checked_sub(1) {
        Some(h) if (0..width).all(|c| !is_date(field(&recs[h], c))) => {
            (0..width).map(|c| field(&recs[h], c).to_string()).collect()
        }
        _ => vec![String::new(); width],
    };
    let data = &recs[first_data..];
    let named = |words: &[&str], except: &[usize]| {
        (0..width).find(|c| !except.contains(c) && has_word(&header[*c], words))
    };
    let date_cols: Vec<usize> = (0..width)
        .filter(|&c| {
            let n = data
                .iter()
                .take(50)
                .filter(|r| is_date(field(r, c)))
                .count();
            n * 2 >= data.len().min(50)
        })
        .collect();
    let date_col = date_cols
        .iter()
        .copied()
        .find(|c| has_word(&header[*c], &DATE_WORDS) && !has_word(&header[*c], &VALUE_DATE_WORDS))
        .or_else(|| date_cols.first().copied())
        .ok_or_else(|| "Aucune date trouvée dans le relevé".to_string())?;
    let numeric = |c: usize| {
        let filled: Vec<&str> = data
            .iter()
            .take(100)
            .map(|r| field(r, c))
            .filter(|s| !s.is_empty())
            .collect();
        !filled.is_empty()
            && filled.iter().filter(|s| parse_amount(s).is_some()).count() * 10 >= filled.len() * 9
            && !date_cols.contains(&c)
    };
    let num_cols: Vec<usize> = (0..width).filter(|&c| numeric(c)).collect();
    let pick = |words: &[&str], used: &[usize]| named(words, used).filter(|c| num_cols.contains(c));
    let mut used = vec![date_col];
    let balance = pick(&BALANCE_WORDS, &used);
    used.extend(balance);
    let debit = pick(&DEBIT_WORDS, &used);
    used.extend(debit);
    let credit = pick(&CREDIT_WORDS, &used);
    used.extend(credit);
    let mut amount = pick(&AMOUNT_WORDS, &used);
    let (mut debit, mut credit, mut balance) = (debit, credit, balance);
    if amount.is_none() && (debit.is_none() || credit.is_none()) {
        let rest: Vec<usize> = num_cols
            .iter()
            .copied()
            .filter(|c| !used.contains(c))
            .collect();
        match (rest.as_slice(), debit, credit) {
            ([a], None, None) => amount = Some(*a),
            ([a, b], None, None) => {
                // Deux colonnes rarement remplies ensemble : débit et crédit.
                let both = data
                    .iter()
                    .filter(|r| !field(r, *a).is_empty() && !field(r, *b).is_empty())
                    .count();
                if both * 4 < data.len() {
                    debit = Some(*a);
                    credit = Some(*b);
                } else {
                    amount = Some(*a);
                    balance = balance.or(Some(*b));
                }
            }
            ([a, b, c, ..], None, None) => {
                debit = Some(*a);
                credit = Some(*b);
                balance = balance.or(Some(*c));
            }
            ([a, ..], Some(_), None) => credit = Some(*a),
            ([a, ..], None, Some(_)) => debit = Some(*a),
            _ => {}
        }
    }
    if amount.is_none() && debit.is_none() && credit.is_none() {
        return Err("Colonne des montants introuvable dans le relevé".into());
    }
    let label_col = named(&LABEL_WORDS, &date_cols)
        .filter(|c| !num_cols.contains(c))
        .or_else(|| {
            (0..width)
                .filter(|c| !date_cols.contains(c) && !num_cols.contains(c))
                .max_by_key(|&c| {
                    data.iter()
                        .take(50)
                        .map(|r| field(r, c).len())
                        .sum::<usize>()
                })
        });
    let order = detect_order(data.iter().map(|r| field(r, date_col)), false);
    let mut out = Vec::new();
    for r in data {
        let Some(date) = parse_date_with(field(r, date_col), order) else {
            continue;
        };
        let amount = match amount {
            Some(c) => parse_amount(field(r, c)),
            None => {
                let d = debit.and_then(|c| parse_amount(field(r, c)));
                let cr = credit.and_then(|c| parse_amount(field(r, c)));
                match (d, cr) {
                    (None, None) => None,
                    (d, cr) => Some(cr.unwrap_or(0.0).abs() - d.unwrap_or(0.0).abs()),
                }
            }
        };
        let Some(amount) = amount else {
            continue;
        };
        out.push(Transaction {
            date,
            label: label_col
                .map(|c| field(r, c).to_string())
                .unwrap_or_default(),
            amount,
            balance: balance.and_then(|c| parse_amount(field(r, c))),
            category: String::new(),
        });
        if out.len() >= MAX_TRANSACTIONS {
            break;
        }
    }
    Ok(out)
}

/// Valeur d'une balise OFX (`<TRNAMT>-12.50`, fermée ou non).
fn ofx_tag<'a>(block: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let i = block.find(&open)? + open.len();
    let rest = &block[i..];
    let end = rest.find('<').unwrap_or(rest.len());
    let v = rest[..end].trim();
    (!v.is_empty()).then_some(v)
}

fn unescape_ofx(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
}

fn parse_ofx(text: &str) -> Result<Vec<Transaction>, String> {
    let upper = text.to_ascii_uppercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = upper[from..].find("<STMTTRN>") {
        let start = from + i + "<STMTTRN>".len();
        let end = upper[start..]
            .find("</STMTTRN>")
            .or_else(|| upper[start..].find("<STMTTRN>"))
            .map_or(upper.len(), |j| start + j);
        from = end;
        let block = &text[start..end];
        let ublock = &upper[start..end];
        // Les positions sont les mêmes dans le texte d'origine (majuscules
        // ASCII seulement) : la casse des libellés est gardée.
        let get = |tag: &str| {
            let open = format!("<{tag}>");
            let i = ublock.find(&open)? + open.len();
            let rest = block.get(i..)?;
            let v = rest[..rest.find('<').unwrap_or(rest.len())].trim();
            (!v.is_empty()).then_some(v)
        };
        let (Some(date), Some(amount)) = (
            get("DTPOSTED").and_then(|d| parse_date_with(d, DateOrder::Dmy)),
            get("TRNAMT").and_then(parse_amount),
        ) else {
            continue;
        };
        let name = get("NAME").unwrap_or("");
        let memo = get("MEMO").unwrap_or("");
        let label = match (name.is_empty(), memo.is_empty() || memo == name) {
            (false, false) => format!("{name} {memo}"),
            (false, true) => name.to_string(),
            _ => memo.to_string(),
        };
        out.push(Transaction {
            date,
            label: unescape_ofx(&label),
            amount,
            balance: None,
            category: String::new(),
        });
        if out.len() >= MAX_TRANSACTIONS || from >= upper.len() {
            break;
        }
    }
    if out.is_empty() {
        return Err("Aucune opération trouvée dans le fichier OFX".into());
    }
    // Solde final : on remonte les soldes intermédiaires.
    if let Some(bal) = upper
        .find("<LEDGERBAL>")
        .and_then(|i| ofx_tag(&upper[i..], "BALAMT"))
        .and_then(parse_amount)
    {
        out.sort_by(|a, b| a.date.total_cmp(&b.date));
        let mut running = bal;
        for t in out.iter_mut().rev() {
            t.balance = Some(running);
            running -= t.amount;
        }
    }
    Ok(out)
}

fn parse_qif(text: &str) -> Result<Vec<Transaction>, String> {
    struct Rec {
        date: String,
        amount: Option<f64>,
        payee: String,
        memo: String,
        category: String,
    }
    let mut recs = Vec::new();
    let mut cur = Rec {
        date: String::new(),
        amount: None,
        payee: String::new(),
        memo: String::new(),
        category: String::new(),
    };
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        let Some(code) = line.chars().next() else {
            continue;
        };
        let rest = line[code.len_utf8()..].trim();
        match code {
            '!' => {}
            'D' => cur.date = rest.to_string(),
            'T' | 'U' if cur.amount.is_none() => cur.amount = parse_amount(rest),
            'P' => cur.payee = rest.to_string(),
            'M' => cur.memo = rest.to_string(),
            'L' => cur.category = rest.trim_matches(['[', ']']).to_string(),
            '^' => {
                recs.push(std::mem::replace(
                    &mut cur,
                    Rec {
                        date: String::new(),
                        amount: None,
                        payee: String::new(),
                        memo: String::new(),
                        category: String::new(),
                    },
                ));
                if recs.len() > MAX_TRANSACTIONS {
                    break;
                }
            }
            _ => {}
        }
    }
    // Les fichiers QIF écrivent souvent les dates à l'américaine.
    let order = detect_order(recs.iter().map(|r| r.date.as_str()), true);
    let out: Vec<Transaction> = recs
        .into_iter()
        .filter_map(|r| {
            Some(Transaction {
                date: parse_date_with(&r.date, order)?,
                label: if r.payee.is_empty() { r.memo } else { r.payee },
                amount: r.amount?,
                balance: None,
                category: r.category,
            })
        })
        .collect();
    if out.is_empty() {
        return Err("Aucune opération trouvée dans le fichier QIF".into());
    }
    Ok(out)
}

/// Lit un relevé (format reconnu d'après le contenu : OFX, QIF ou CSV).
pub fn parse_statement(text: &str) -> Result<Vec<Transaction>, String> {
    let text = text.trim_start_matches('\u{FEFF}');
    let head: String = text
        .chars()
        .take(4000)
        .collect::<String>()
        .to_ascii_uppercase();
    if head.contains("<OFX>") || head.contains("OFXHEADER") || head.contains("<STMTTRN>") {
        parse_ofx(text)
    } else if head.trim_start().starts_with("!TYPE") || head.trim_start().starts_with("!ACCOUNT") {
        parse_qif(text)
    } else {
        parse_csv(text)
    }
}

/// Résultat d'un import : feuilles créées et nombre d'opérations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BankImport {
    pub operations: usize,
    pub summary: usize,
    pub count: usize,
}

/// Titres possibles de la colonne des catégories (six langues).
fn is_category_header(s: &str) -> bool {
    Lang::ALL.iter().any(|l| s == loc(*l, "Catégorie"))
}

impl Workbook {
    /// Les règles de classement en vigueur (celles du classeur, sinon celles
    /// par défaut).
    pub fn effective_bank_rules(&self, lang: Lang) -> Vec<CategoryRule> {
        if self.bank_rules.is_empty() {
            default_rules(lang)
        } else {
            self.bank_rules.clone()
        }
    }

    fn unique_sheet(&mut self, base: &str) -> usize {
        let s = self.add_sheet();
        let mut name = base.to_string();
        let mut k = 2;
        while !self.rename_sheet(s, &name) && k < 1000 {
            name = format!("{base} {k}");
            k += 1;
        }
        s
    }

    /// Importe un relevé bancaire dans deux nouvelles feuilles : les
    /// opérations (date, libellé, catégorie, montant, solde) et une synthèse
    /// par catégorie et par mois (formules), avec un graphique.
    pub fn import_bank_statement(&mut self, text: &str, lang: Lang) -> Result<BankImport, String> {
        let mut tx = parse_statement(text)?;
        if tx.is_empty() {
            return Err("Aucune opération trouvée dans le relevé".into());
        }
        tx.sort_by(|a, b| a.date.total_cmp(&b.date));
        if self.bank_rules.is_empty() {
            self.bank_rules = default_rules(lang);
        }
        let rules = self.bank_rules.clone();
        let other = loc(lang, "Autres");
        let ops = self.unique_sheet(loc(lang, "Opérations"));
        let has_balance = tx.iter().any(|t| t.balance.is_some());
        let heads = [
            loc(lang, "Date"),
            loc(lang, "Libellé"),
            loc(lang, "Catégorie"),
            loc(lang, "Montant"),
            loc(lang, "Solde"),
        ];
        let n_heads = if has_balance { 5 } else { 4 };
        let head_fmt = CellFormat {
            bold: true,
            fill: Some([221, 235, 247]),
            align: Some(HAlign::Center),
            ..CellFormat::default()
        };
        for (c, h) in heads.iter().take(n_heads).enumerate() {
            self.put_value(
                ops,
                CellRef::new(0, c as u32),
                &Value::Text((*h).into()),
                head_fmt,
            );
        }
        let date_fmt = CellFormat {
            number: NumberFormat::Date,
            ..CellFormat::default()
        };
        let money = CellFormat {
            number: NumberFormat::Number(2),
            ..CellFormat::default()
        };
        for (i, t) in tx.iter().enumerate() {
            let r = i as u32 + 1;
            let cat = if t.category.trim().is_empty() {
                categorize(&t.label, &rules)
                    .or_else(|| (t.amount > 0.0).then(|| loc(lang, "Virements").to_string()))
                    .unwrap_or_else(|| other.to_string())
            } else {
                t.category.trim().to_string()
            };
            self.put_value(ops, CellRef::new(r, 0), &Value::Number(t.date), date_fmt);
            self.put_value(
                ops,
                CellRef::new(r, 1),
                &Value::Text(t.label.clone()),
                CellFormat::default(),
            );
            self.put_value(
                ops,
                CellRef::new(r, 2),
                &Value::Text(cat),
                CellFormat::default(),
            );
            self.put_value(ops, CellRef::new(r, 3), &Value::Number(t.amount), money);
            if let Some(b) = t.balance {
                self.put_value(ops, CellRef::new(r, 4), &Value::Number(b), money);
            }
        }
        let last = tx.len() as u32 + 1;
        let sh = &mut self.sheets[ops];
        sh.col_widths.insert(0, 96.0);
        sh.col_widths.insert(1, 280.0);
        sh.col_widths.insert(2, 120.0);
        sh.frozen_rows = 1;
        sh.cond_formats.push(CondFormat {
            range: Range::new(CellRef::new(1, 3), CellRef::new(last - 1, 3)),
            rule: CondRule::Compare {
                op: CompareOp::Lt,
                value: "0".into(),
                value2: String::new(),
            },
            style: CondStyle {
                color: Some([0xC0, 0x28, 0x20]),
                ..CondStyle::default()
            },
        });
        self.set_auto_filter(
            ops,
            Some(Range::new(
                CellRef::new(0, 0),
                CellRef::new(last - 1, n_heads as u32 - 1),
            )),
        );
        let summary = self.write_bank_summary(ops, lang);
        self.invalidate();
        Ok(BankImport {
            operations: ops,
            summary,
            count: tx.len(),
        })
    }

    /// Feuille de synthèse : une ligne par catégorie, le total puis une
    /// colonne par mois (formules `SOMME.SI.ENS` sur la feuille des
    /// opérations) ; graphique des totaux par catégorie.
    fn write_bank_summary(&mut self, ops: usize, lang: Lang) -> usize {
        let (rows, _) = self.used_size(ops);
        let last = rows.max(2);
        let mut cats: Vec<String> = Vec::new();
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for r in 1..last {
            let c = self.display(ops, CellRef::new(r, 2));
            if !c.is_empty() && !cats.contains(&c) {
                cats.push(c);
            }
            if let Value::Number(d) = self.value(ops, CellRef::new(r, 0)) {
                lo = lo.min(d);
                hi = hi.max(d);
            }
        }
        cats.sort_by_key(|c| collation_key(c));
        let s = self.unique_sheet(loc(lang, "Synthèse"));
        let src = quote_sheet_name(&self.sheets[ops].name);
        let col = |c: &str| format!("{src}!${c}$2:${c}${last}");
        let head_fmt = CellFormat {
            bold: true,
            fill: Some([221, 235, 247]),
            align: Some(HAlign::Center),
            ..CellFormat::default()
        };
        let title = format!("{} — {}", loc(lang, "Synthèse"), self.sheets[ops].name);
        self.put_value(
            s,
            CellRef::new(0, 0),
            &Value::Text(title),
            CellFormat {
                bold: true,
                ..CellFormat::default()
            },
        );
        self.put_value(
            s,
            CellRef::new(2, 0),
            &Value::Text(loc(lang, "Catégorie").into()),
            head_fmt,
        );
        self.put_value(
            s,
            CellRef::new(2, 1),
            &Value::Text(loc(lang, "Total").into()),
            head_fmt,
        );
        let mut months: Vec<(i32, u32)> = Vec::new();
        if let (Some((y0, m0, _)), Some((y1, m1, _))) = (serial_to_ymd(lo), serial_to_ymd(hi)) {
            let (mut y, mut m) = (y0, m0);
            while (y, m) <= (y1, m1) && months.len() < 120 {
                months.push((y, m));
                m += 1;
                if m > 12 {
                    m = 1;
                    y += 1;
                }
            }
        }
        for (k, (y, m)) in months.iter().enumerate() {
            self.put_value(
                s,
                CellRef::new(2, 2 + k as u32),
                &Value::Text(format!("{} {y}", month_name(lang, *m))),
                head_fmt,
            );
        }
        let money = CellFormat {
            number: NumberFormat::Number(2),
            ..CellFormat::default()
        };
        let n = cats.len() as u32;
        for (i, cat) in cats.iter().enumerate() {
            let r = 3 + i as u32;
            self.put_value(
                s,
                CellRef::new(r, 0),
                &Value::Text(cat.clone()),
                CellFormat::default(),
            );
            let row = r + 1;
            self.set_input(
                s,
                CellRef::new(r, 1),
                &format!("=SOMME.SI({};$A{row};{})", col("C"), col("D")),
            );
            self.set_format(
                s,
                CellRef::new(r, 1),
                CellFormat {
                    bold: true,
                    ..money
                },
            );
            for (k, (y, m)) in months.iter().enumerate() {
                let start = format!("DATE({y};{m};1)");
                let end = format!("DATE({y};{};1)", m + 1);
                self.set_input(
                    s,
                    CellRef::new(r, 2 + k as u32),
                    &format!(
                        "=SOMME.SI.ENS({};{};$A{row};{};\">=\"&{start};{};\"<\"&{end})",
                        col("D"),
                        col("C"),
                        col("A"),
                        col("A")
                    ),
                );
                self.set_format(s, CellRef::new(r, 2 + k as u32), money);
            }
        }
        let total_row = 3 + n;
        self.put_value(
            s,
            CellRef::new(total_row, 0),
            &Value::Text(loc(lang, "Total").into()),
            head_fmt,
        );
        for c in 1..2 + months.len() as u32 {
            let l = col_name(c);
            self.set_input(
                s,
                CellRef::new(total_row, c),
                &format!("=SOMME({l}4:{l}{total_row})"),
            );
            self.set_format(
                s,
                CellRef::new(total_row, c),
                CellFormat {
                    bold: true,
                    fill: head_fmt.fill,
                    ..money
                },
            );
        }
        let sh = &mut self.sheets[s];
        sh.col_widths.insert(0, 150.0);
        sh.frozen_rows = 3;
        sh.frozen_cols = 1;
        sh.cond_formats.push(CondFormat {
            range: Range::new(
                CellRef::new(3, 1),
                CellRef::new(total_row, 1 + months.len() as u32),
            ),
            rule: CondRule::Compare {
                op: CompareOp::Lt,
                value: "0".into(),
                value2: String::new(),
            },
            style: CondStyle {
                color: Some([0xC0, 0x28, 0x20]),
                ..CondStyle::default()
            },
        });
        if n > 0 {
            sh.charts.push(SheetChart {
                kind: "barres-h".into(),
                range: Range::new(CellRef::new(2, 0), CellRef::new(2 + n, 1)),
                title: loc(lang, "Total par catégorie").to_string(),
            });
        }
        s
    }

    /// Reclasse les opérations d'une feuille importée (colonne « Catégorie »
    /// à côté du libellé) avec les règles du classeur. Renvoie le nombre
    /// d'opérations dont la catégorie a changé.
    pub fn recategorize(&mut self, sheet: usize, lang: Lang) -> usize {
        let Some(sh) = self.sheets.get(sheet) else {
            return 0;
        };
        let (rows, cols) = sh.used_size();
        let Some(cat_col) = (0..cols.min(64))
            .find(|c| is_category_header(&self.display(sheet, CellRef::new(0, *c))))
        else {
            return 0;
        };
        if cat_col == 0 {
            return 0;
        }
        let rules = self.effective_bank_rules(lang);
        let mut changed = 0;
        for r in 1..rows {
            let label = self.display(sheet, CellRef::new(r, cat_col - 1));
            if let Some(c) = categorize(&label, &rules)
                && c != self.display(sheet, CellRef::new(r, cat_col))
            {
                let f = self.format(sheet, CellRef::new(r, cat_col));
                self.put_value(sheet, CellRef::new(r, cat_col), &Value::Text(c), f);
                changed += 1;
            }
        }
        changed
    }

    /// Un classeur à partir d'un relevé bancaire (fichier OFX, QIF ou CSV
    /// ouvert directement).
    pub fn from_bank_statement(title: &str, text: &str, lang: Lang) -> Result<Workbook, String> {
        let mut wb = Workbook::new();
        if !title.trim().is_empty() {
            wb.title = title.trim().to_string();
        }
        wb.import_bank_statement(text, lang)?;
        wb.remove_sheet(0);
        Ok(wb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV_FR: &str = "Compte courant n° 123;;;;\n\
Date opération;Date valeur;Libellé;Débit;Crédit\n\
03/01/2026;03/01/2026;CB CARREFOUR MARKET;45,20;\n\
05/01/2026;05/01/2026;PRLV NETFLIX.COM;13,49;\n\
28/01/2026;28/01/2026;VIR SALAIRE JANVIER;;2 150,00\n\
02/02/2026;02/02/2026;CB UBER EATS PARIS;23,90;\n\
14/02/2026;14/02/2026;CB PHARMACIE DU CENTRE;12,00;\n";

    #[test]
    fn csv_a_la_francaise_debit_credit() {
        let t = parse_statement(CSV_FR).unwrap();
        assert_eq!(t.len(), 5);
        assert_eq!(t[0].label, "CB CARREFOUR MARKET");
        assert_eq!(t[0].amount, -45.2);
        assert_eq!(t[2].amount, 2150.0);
        assert_eq!(serial_to_ymd(t[3].date), Some((2026, 2, 2)));
    }

    #[test]
    fn csv_a_l_anglaise_montant_et_solde() {
        let csv = "Date,Description,Amount,Balance\n\
01/13/2026,TESCO STORES,-32.10,1200.00\n\
01/15/2026,SPOTIFY,-9.99,1190.01\n\
01/31/2026,PAYROLL ACME,\"2,500.00\",3690.01\n";
        let t = parse_statement(csv).unwrap();
        assert_eq!(t.len(), 3);
        assert_eq!(serial_to_ymd(t[0].date), Some((2026, 1, 13)));
        assert_eq!(t[2].amount, 2500.0);
        assert_eq!(t[1].balance, Some(1190.01));
    }

    #[test]
    fn ofx_et_qif() {
        let ofx = "OFXHEADER:100\nDATA:OFXSGML\n<OFX><BANKMSGSRSV1><STMTTRNRS><STMTRS><BANKTRANLIST>\n\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20260105120000[+1:CET]<TRNAMT>-60.00<FITID>1<NAME>SNCF INTERNET<MEMO>Billet</STMTTRN>\n\
<STMTTRN><TRNTYPE>CREDIT<DTPOSTED>20260110<TRNAMT>100.00<FITID>2<NAME>VIREMENT M. DUPONT</STMTTRN>\n\
</BANKTRANLIST><LEDGERBAL><BALAMT>540.00<DTASOF>20260131</LEDGERBAL></STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>";
        let t = parse_statement(ofx).unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].label, "SNCF INTERNET Billet");
        assert_eq!(t[1].balance, Some(540.0));
        assert_eq!(t[0].balance, Some(440.0));
        let qif = "!Type:Bank\nD01/31/2026\nT-1,234.56\nPLOYER RESIDENCE\nLLogement\n^\nD2/3'26\nT45.00\nPREMBOURSEMENT CPAM\n^\n";
        let q = parse_statement(qif).unwrap();
        assert_eq!(q.len(), 2);
        assert_eq!(q[0].amount, -1234.56);
        assert_eq!(q[0].category, "Logement");
        assert_eq!(serial_to_ymd(q[1].date), Some((2026, 2, 3)));
    }

    #[test]
    fn categories_par_mots_cles() {
        let rules = default_rules(Lang::Fr);
        assert_eq!(
            categorize("CB UBER EATS PARIS", &rules).as_deref(),
            Some("Restaurants")
        );
        assert_eq!(categorize("UBER BV", &rules).as_deref(), Some("Transport"));
        assert_eq!(categorize("Pharmacie", &rules).as_deref(), Some("Santé"));
        assert_eq!(categorize("rien de connu", &rules), None);
        let en = default_rules(Lang::En);
        assert_eq!(categorize("NETFLIX", &en).as_deref(), Some("Subscriptions"));
    }

    #[test]
    fn import_complet() {
        let mut wb = Workbook::new();
        let r = wb.import_bank_statement(CSV_FR, Lang::Fr).unwrap();
        assert_eq!(r.count, 5);
        assert_eq!(wb.sheets[r.operations].name, "Opérations");
        assert_eq!(wb.display(r.operations, CellRef::new(1, 2)), "Courses");
        assert_eq!(wb.display(r.operations, CellRef::new(3, 2)), "Salaire");
        assert_eq!(wb.display(r.operations, CellRef::new(1, 0)), "03/01/2026");
        // Synthèse : Courses, total puis janvier, février.
        let s = r.summary;
        assert_eq!(wb.display(s, CellRef::new(2, 2)), "janvier 2026");
        let row = (3..10)
            .find(|&r| wb.display(s, CellRef::new(r, 0)) == "Courses")
            .unwrap();
        assert_eq!(wb.display(s, CellRef::new(row, 1)), "-45,20");
        assert_eq!(wb.display(s, CellRef::new(row, 2)), "-45,20");
        assert_eq!(wb.sheets[s].charts.len(), 1);
        assert!(!wb.bank_rules.is_empty());
        // Règle ajoutée par l'utilisateur, puis reclassement.
        wb.bank_rules.push(CategoryRule {
            keyword: "carrefour market".into(),
            category: "Supermarché".into(),
        });
        assert_eq!(wb.recategorize(r.operations, Lang::Fr), 1);
        assert_eq!(wb.display(r.operations, CellRef::new(1, 2)), "Supermarché");
        let back = Workbook::from_json(&wb.to_json()).unwrap();
        assert_eq!(back.bank_rules, wb.bank_rules);
        let only = Workbook::from_bank_statement("Relevé", CSV_FR, Lang::En).unwrap();
        assert_eq!(only.sheets.len(), 2);
        assert_eq!(only.sheets[0].name, "Transactions");
    }

    #[test]
    fn montants_et_dates() {
        assert_eq!(parse_amount("1 234,56 €"), Some(1234.56));
        assert_eq!(parse_amount("1.234,56"), Some(1234.56));
        assert_eq!(parse_amount("-1,234.56"), Some(-1234.56));
        assert_eq!(parse_amount("(12.50)"), Some(-12.5));
        assert_eq!(parse_amount("12,50-"), Some(-12.5));
        assert_eq!(parse_amount("+30"), Some(30.0));
        assert_eq!(parse_amount("1,234,567"), Some(1_234_567.0));
        assert_eq!(parse_amount("abc"), None);
        assert_eq!(parse_date_with("31/02/2026", DateOrder::Dmy), None);
        assert!(parse_date_with("2026-02-28", DateOrder::Mdy).is_some());
        assert!(parse_date_with("15.03.2026", DateOrder::Dmy).is_some());
    }

    #[test]
    fn fuzz_des_releves() {
        let bases = [
            CSV_FR,
            "!Type:Bank\nD\nT\n^",
            "<OFX><STMTTRN><DTPOSTED>2026<TRNAMT>x",
            "a;b\n1;2",
        ];
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for base in bases {
            let bytes: Vec<char> = base.chars().collect();
            for _ in 0..400 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let cut = (seed as usize) % (bytes.len() + 1);
                let mut s: String = bytes[..cut].iter().collect();
                let junk = [
                    "\"",
                    ";",
                    "\n",
                    "<STMTTRN>",
                    "^",
                    "D",
                    "/",
                    "-",
                    "💥",
                    ",",
                    "(",
                    "99/99/9999",
                ];
                s.push_str(junk[(seed >> 20) as usize % junk.len()]);
                s.extend(bytes[cut..].iter());
                let _ = parse_statement(&s);
                let mut wb = Workbook::new();
                let _ = wb.import_bank_statement(&s, Lang::Fr);
            }
        }
        assert!(parse_statement("").is_err());
        assert!(parse_statement("bonjour").is_err());
    }
}
