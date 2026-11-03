//! Réécriture des formules : décalage des références (copier-coller, recopie),
//! insertion et suppression de lignes ou de colonnes, renommage de feuilles,
//! traduction en anglais (XLSX) et en OpenFormula (ODS).
//!
//! La réécriture se fait jeton par jeton : les espaces et l'écriture d'origine
//! (noms de fonctions en français, minuscules…) sont conservés.

use crate::format::{pattern_to_english, pattern_to_french};
use crate::functions::{F, info, lookup, odf_name, xlsx_name};
use crate::lexer::{Kind, RefTok, Tok, tokenize};
use crate::refs::{Addr, MAX_COLS, MAX_ROWS, always_quote_sheet_name};

const REF_ERROR: &str = "#REF!";

/// Recompose une formule en remplaçant certains jetons.
fn rewrite(input: &str, mut f: impl FnMut(&Tok, &str) -> Option<String>) -> String {
    if !input.starts_with('=') {
        return input.to_string();
    }
    let toks = tokenize(input);
    let mut out = String::with_capacity(input.len() + 8);
    out.push('=');
    let mut last = 1;
    for t in &toks {
        out.push_str(input.get(last..t.start).unwrap_or(""));
        let text = input.get(t.start..t.end).unwrap_or("");
        match f(t, text) {
            Some(new) => out.push_str(&new),
            None => out.push_str(text),
        }
        last = t.end;
    }
    out.push_str(input.get(last..).unwrap_or(""));
    out
}

/// Remplace les références d'une formule (`None` : inchangée).
pub(crate) fn rewrite_refs(
    input: &str,
    mut f: impl FnMut(&RefTok, &str) -> Option<String>,
) -> String {
    rewrite(input, |t, _| match &t.kind {
        Kind::Ref(r) => {
            let prefix = match &r.sheet {
                Some(p) => input.get(t.start..p.cell_start).unwrap_or(""),
                None => "",
            };
            f(r, prefix)
        }
        _ => None,
    })
}

/// Texte d'une référence : préfixe de feuille d'origine et adresses.
pub(crate) fn ref_text(prefix: &str, start: Addr, end: Option<Addr>) -> String {
    match end {
        Some(e) => format!("{prefix}{}:{}", start.text(), e.text()),
        None => format!("{prefix}{}", start.text()),
    }
}

fn shift_addr(a: Addr, drow: i64, dcol: i64) -> Option<Addr> {
    let row = if a.abs_row {
        i64::from(a.row)
    } else {
        i64::from(a.row) + drow
    };
    let col = if a.abs_col {
        i64::from(a.col)
    } else {
        i64::from(a.col) + dcol
    };
    if !(0..i64::from(MAX_ROWS)).contains(&row) || !(0..i64::from(MAX_COLS)).contains(&col) {
        return None;
    }
    Some(Addr {
        row: row as u32,
        col: col as u32,
        ..a
    })
}

/// Décale les références relatives d'une formule de `drow` lignes et `dcol`
/// colonnes (copier-coller, recopie vers le bas…). Les parties absolues (`$`)
/// ne bougent pas ; une référence qui sortirait de la feuille devient `#REF!`.
/// Une saisie qui n'est pas une formule est renvoyée telle quelle.
pub fn shift_formula(input: &str, drow: i32, dcol: i32) -> String {
    if (drow == 0 && dcol == 0) || !input.starts_with('=') {
        return input.to_string();
    }
    let (dr, dc) = (i64::from(drow), i64::from(dcol));
    rewrite_refs(input, |r, prefix| {
        let start = shift_addr(r.start, dr, dc);
        let end = r.end.map(|e| shift_addr(e, dr, dc));
        Some(match (start, end) {
            (Some(s), None) => ref_text(prefix, s, None),
            (Some(s), Some(Some(e))) => ref_text(prefix, s, Some(e)),
            _ => REF_ERROR.to_string(),
        })
    })
}

/// Insertion ou suppression de lignes ou de colonnes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Structural {
    pub rows: bool,
    pub insert: bool,
    pub at: u32,
    pub n: u32,
}

impl Structural {
    fn limit(&self) -> u32 {
        if self.rows { MAX_ROWS } else { MAX_COLS }
    }

    /// Nouvelle position d'une ligne (ou colonne) ; `None` si elle disparaît.
    pub fn map(&self, v: u32) -> Option<u32> {
        if v < self.at {
            return Some(v);
        }
        if self.insert {
            let nv = u64::from(v) + u64::from(self.n);
            (nv < u64::from(self.limit())).then_some(nv as u32)
        } else if u64::from(v) < u64::from(self.at) + u64::from(self.n) {
            None
        } else {
            Some(v - self.n)
        }
    }

    /// Nouvelles bornes d'un intervalle `lo..=hi` ; `None` s'il disparaît.
    pub(crate) fn map_span(&self, lo: u32, hi: u32) -> Option<(u32, u32)> {
        if self.insert {
            let nlo = self.map(lo)?;
            let nhi = self.map(hi).unwrap_or(self.limit() - 1);
            return Some((nlo, nhi));
        }
        let end_del = u64::from(self.at) + u64::from(self.n);
        if lo >= self.at && u64::from(hi) < end_del {
            return None;
        }
        let nlo = if lo < self.at {
            lo
        } else if u64::from(lo) < end_del {
            self.at
        } else {
            lo - self.n
        };
        let nhi = if hi < self.at {
            hi
        } else if u64::from(hi) < end_del {
            self.at - 1
        } else {
            hi - self.n
        };
        Some((nlo, nhi))
    }

    /// Adapte une référence ; `None` si elle devient `#REF!`.
    pub fn adjust(&self, r: &RefTok) -> Option<(Addr, Option<Addr>)> {
        let Some(end) = r.end else {
            let mut a = r.start;
            if self.rows {
                a.row = self.map(a.row)?;
            } else {
                a.col = self.map(a.col)?;
            }
            return Some((a, None));
        };
        let (mut s, mut e) = (r.start, end);
        if self.rows {
            if s.row > e.row {
                std::mem::swap(&mut s.row, &mut e.row);
                std::mem::swap(&mut s.abs_row, &mut e.abs_row);
            }
            let (lo, hi) = self.map_span(s.row, e.row)?;
            s.row = lo;
            e.row = hi;
        } else {
            if s.col > e.col {
                std::mem::swap(&mut s.col, &mut e.col);
                std::mem::swap(&mut s.abs_col, &mut e.abs_col);
            }
            let (lo, hi) = self.map_span(s.col, e.col)?;
            s.col = lo;
            e.col = hi;
        }
        Some((s, Some(e)))
    }
}

/// Adapte les références d'une formule à une insertion ou suppression sur la
/// feuille visée. `targets_sheet` dit si une référence (préfixe de feuille ou
/// non) désigne cette feuille.
pub(crate) fn adjust_formula(
    input: &str,
    edit: Structural,
    targets_sheet: impl Fn(Option<&str>) -> bool,
) -> String {
    rewrite_refs(input, |r, prefix| {
        if !targets_sheet(r.sheet.as_ref().map(|p| p.name.as_str())) {
            return None;
        }
        Some(match edit.adjust(r) {
            Some((s, e)) => ref_text(prefix, s, e),
            None => REF_ERROR.to_string(),
        })
    })
}

/// Majuscules pour les fonctions connues et les références (`=somme(a1)` →
/// `=SOMME(A1)`), comme le fait LibreOffice à la validation.
pub(crate) fn normalize_formula(input: &str) -> String {
    rewrite(input, |t, text| match &t.kind {
        Kind::Func(name) if lookup(name).is_some() => Some(name.to_uppercase()),
        Kind::Bool(_) => Some(text.to_uppercase()),
        Kind::Ref(r) => {
            let prefix = match &r.sheet {
                Some(p) => input.get(t.start..p.cell_start).unwrap_or(""),
                None => "",
            };
            Some(ref_text(prefix, r.start, r.end))
        }
        _ => None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dialect {
    /// Anglais lisible : `=SUM(A1,B2)`.
    English,
    /// Formule d'un fichier XLSX (préfixes `_xlfn.`).
    Xlsx,
    /// OpenFormula d'un fichier ODS : `of:=SUM([.A1];[.B2])`.
    Odf,
}

fn translate(formula: &str, dialect: Dialect) -> String {
    // Pile des appels en cours : (fonction, numéro de l'argument courant).
    let mut calls: Vec<(Option<F>, usize)> = Vec::new();
    let mut pending: Option<F> = None;
    rewrite(formula, |t, text| {
        let func = pending.take();
        match &t.kind {
            Kind::Func(name) => {
                let id = lookup(name);
                pending = id;
                id.map(|id| match dialect {
                    Dialect::English => info(id).en.to_string(),
                    Dialect::Xlsx => xlsx_name(id),
                    Dialect::Odf => odf_name(id),
                })
            }
            Kind::LParen => {
                calls.push((func, 0));
                None
            }
            Kind::RParen => {
                calls.pop();
                None
            }
            Kind::Semi => {
                if let Some(top) = calls.last_mut() {
                    top.1 += 1;
                }
                (dialect != Dialect::Odf).then(|| ",".to_string())
            }
            Kind::Num(_) => Some(text.replace(',', ".")),
            Kind::Bool(b) => Some(match (dialect, b) {
                (Dialect::Odf, true) => "TRUE()".into(),
                (Dialect::Odf, false) => "FALSE()".into(),
                (_, true) => "TRUE".into(),
                (_, false) => "FALSE".into(),
            }),
            Kind::Err(e) => Some(e.english_code().to_string()),
            Kind::Str(s) => {
                // Motif de TEXTE : 0,00 → 0.00, jj/mm/aaaa → dd/mm/yyyy.
                let is_pattern = matches!(calls.last(), Some((Some(F::Texte), 1)));
                is_pattern.then(|| format!("\"{}\"", pattern_to_english(s).replace('"', "\"\"")))
            }
            Kind::Ref(r) => Some(match dialect {
                Dialect::Odf => {
                    let sheet = match &r.sheet {
                        Some(p) => format!("${}", always_quote_sheet_name(&p.name)),
                        None => String::new(),
                    };
                    match r.end {
                        Some(e) => format!("[{sheet}.{}:.{}]", r.start.text(), e.text()),
                        None => format!("[{sheet}.{}]", r.start.text()),
                    }
                }
                _ => {
                    let prefix = match &r.sheet {
                        Some(p) => format!("{}!", always_quote_sheet_name(&p.name)),
                        None => String::new(),
                    };
                    ref_text(&prefix, r.start, r.end)
                }
            }),
            _ => None,
        }
    })
}

/// Traduit une formule française en anglais : `=SOMME(A1;B2)` → `=SUM(A1,B2)`.
/// Les noms de fonctions, les séparateurs, les décimales, `VRAI`/`FAUX` et les
/// codes d'erreur sont traduits ; les références de feuille sont mises entre
/// apostrophes (`'Feuille 2'!A1`). Une saisie sans `=` est renvoyée telle quelle.
pub fn translate_to_english(formula: &str) -> String {
    translate(formula, Dialect::English)
}

/// Formule pour l'attribut `<f>` d'un fichier XLSX (sans `=`).
pub(crate) fn to_xlsx_formula(formula: &str) -> String {
    let t = translate(formula, Dialect::Xlsx);
    t.strip_prefix('=').unwrap_or(&t).to_string()
}

/// Formule OpenFormula pour l'attribut `table:formula` d'un fichier ODS.
pub(crate) fn to_odf_formula(formula: &str) -> String {
    format!("of:{}", translate(formula, Dialect::Odf))
}

// ---------------------------------------------------------------- Imports

/// Remplace les colonnes entières (`A:B`, `$C:$C`) par des plages explicites
/// (`A$1:B$1048576`), que le moteur sait lire. `text` ne contient pas de chaîne.
fn expand_whole_columns(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let is_ident = |c: char| c.is_alphanumeric() || matches!(c, '_' | '.' | '$' | '!' | '\'');
    let col_word = |i: usize| -> Option<(usize, String)> {
        // Lit `$?[A-Z]{1,3}` à partir de i ; renvoie la fin et les lettres.
        let mut j = i;
        if chars.get(j) == Some(&'$') {
            j += 1;
        }
        let start = j;
        while chars.get(j).is_some_and(|c| c.is_ascii_alphabetic()) {
            j += 1;
        }
        let letters: String = chars[start..j].iter().collect();
        (j > start && j - start <= 3 && crate::refs::col_index(&letters).is_some())
            .then_some((j, letters))
    };
    let mut i = 0;
    while i < chars.len() {
        let prev_ok = i == 0 || !is_ident(chars[i - 1]);
        if prev_ok
            && let Some((j, a)) = col_word(i)
            && chars.get(j) == Some(&':')
            && let Some((k, b)) = col_word(j + 1)
            && !chars
                .get(k)
                .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '(' | '.' | '!'))
        {
            let _ = std::fmt::Write::write_fmt(
                &mut out,
                format_args!("{}$1:{}${MAX_ROWS}", a.to_uppercase(), b.to_uppercase()),
            );
            i = k;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Passe sur les morceaux hors chaînes (`"…"`) et hors noms de feuille entre
/// apostrophes (`'…'`), qui sont recopiés tels quels.
fn outside_strings(text: &str, mut f: impl FnMut(&str) -> String) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut chunk = String::new();
    let mut quote: Option<char> = None;
    for c in text.chars() {
        match quote {
            Some(q) => {
                out.push(c);
                if c == q {
                    quote = None;
                }
            }
            None if c == '"' || c == '\'' => {
                out.push_str(&f(&chunk));
                chunk.clear();
                out.push(c);
                quote = Some(c);
            }
            None => chunk.push(c),
        }
    }
    out.push_str(&f(&chunk));
    out
}

/// Écriture française des jetons : noms de fonctions, `VRAI`/`FAUX`, codes
/// d'erreur et virgule décimale.
fn to_french_tokens(formula: &str) -> String {
    let mut calls: Vec<(Option<F>, usize)> = Vec::new();
    let mut pending: Option<F> = None;
    rewrite(formula, |t, text| {
        let func = pending.take();
        match &t.kind {
            Kind::Func(name) => {
                let bare = strip_func_prefix(name);
                let id = lookup(bare).or_else(|| lookup(name));
                pending = id;
                match id {
                    Some(id) => Some(info(id).fr.to_string()),
                    None => (bare.len() != name.len()).then(|| bare.to_string()),
                }
            }
            Kind::LParen => {
                calls.push((func, 0));
                None
            }
            Kind::RParen => {
                calls.pop();
                None
            }
            Kind::Semi => {
                if let Some(top) = calls.last_mut() {
                    top.1 += 1;
                }
                None
            }
            Kind::Str(s) => {
                // Motif de TEXTE : #,##0.00 → # ##0,00, dd/mm/yyyy → jj/mm/aaaa.
                let is_pattern = matches!(calls.last(), Some((Some(F::Texte), 1)));
                is_pattern.then(|| format!("\"{}\"", pattern_to_french(s).replace('"', "\"\"")))
            }
            Kind::Ref(r) => {
                let prefix = match &r.sheet {
                    Some(p) => format!("{}!", crate::refs::quote_sheet_name(&p.name)),
                    None => String::new(),
                };
                Some(ref_text(&prefix, r.start, r.end))
            }
            Kind::Bool(true) => Some("VRAI".into()),
            Kind::Bool(false) => Some("FAUX".into()),
            Kind::Err(e) => Some(e.code().to_string()),
            Kind::Num(_) => Some(text.replace('.', ",")),
            _ => None,
        }
    })
}

/// Préfixes des fonctions récentes d'Excel (`_xlfn.`, `_xlws.`).
fn strip_func_prefix(name: &str) -> &str {
    let lower = name.to_ascii_lowercase();
    for p in ["_xlfn._xlws.", "_xlfn.", "_xlws."] {
        if lower.starts_with(p) {
            return &name[p.len()..];
        }
    }
    name
}

/// Formule d'un fichier XLSX (`SUM(A1,B2)`, sans `=`) → saisie Varium
/// (`=SOMME(A1;B2)`).
pub(crate) fn from_xlsx_formula(f: &str) -> String {
    let f = f.trim();
    let f = f.strip_prefix('=').unwrap_or(f);
    let body = outside_strings(f, |chunk| expand_whole_columns(&chunk.replace(',', ";")));
    to_french_tokens(&format!("={body}"))
}

/// Une référence OpenFormula entre crochets (`.A1`, `$'F 2'.$B$2:.C3`) → `'F 2'!$B$2:C3`.
fn odf_ref(inner: &str) -> String {
    // Découpe sur « : » hors apostrophes.
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in inner.chars() {
        match c {
            '\'' => {
                quoted = !quoted;
                cur.push(c);
            }
            ':' if !quoted => parts.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    parts.push(cur);
    let mut sheet: Option<String> = None;
    let mut addrs = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        let p = p.trim();
        // Nom de feuille éventuel avant le dernier « . » hors apostrophes.
        let mut split = None;
        let mut q = false;
        for (k, c) in p.char_indices() {
            match c {
                '\'' => q = !q,
                '.' if !q => split = Some(k),
                _ => {}
            }
        }
        let (sh, addr) = match split {
            Some(k) => (&p[..k], &p[k + 1..]),
            None => ("", p),
        };
        let sh = sh.trim_start_matches('$');
        if i == 0 && !sh.is_empty() {
            let name = sh
                .strip_prefix('\'')
                .and_then(|x| x.strip_suffix('\''))
                .map(|x| x.replace("''", "'"))
                .unwrap_or_else(|| sh.to_string());
            sheet = Some(name);
        }
        if addr.contains('#') || addr.is_empty() {
            return REF_ERROR.to_string();
        }
        addrs.push(addr.to_string());
    }
    let prefix = sheet
        .map(|s| format!("{}!", crate::refs::quote_sheet_name(&s)))
        .unwrap_or_default();
    let body = addrs.join(":");
    // Colonnes entières : [.A:.A].
    let body = if addrs.len() == 2
        && addrs.iter().all(|a| {
            a.trim_start_matches('$')
                .chars()
                .all(|c| c.is_ascii_alphabetic())
        }) {
        expand_whole_columns(&body)
    } else {
        body
    };
    format!("{prefix}{body}")
}

/// Formule OpenFormula d'un fichier ODS (`of:=SUM([.A1:.A3])`) → saisie Varium.
pub(crate) fn from_odf_formula(f: &str) -> String {
    let f = f.trim();
    let (ns, body) = match f.split_once(':') {
        Some((ns, rest))
            if !ns.is_empty()
                && ns.len() <= 8
                && ns.chars().all(|c| c.is_ascii_alphanumeric())
                && rest.starts_with('=') =>
        {
            (ns, rest)
        }
        _ => ("of", f),
    };
    if ns.eq_ignore_ascii_case("msoxl") {
        return from_xlsx_formula(body);
    }
    let body = body.strip_prefix('=').unwrap_or(body);
    let body = odf_refs(body);
    to_french_tokens(&format!("={body}"))
}

/// Remplace les références entre crochets (hors chaînes).
fn odf_refs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut in_str = false;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            in_str = !in_str;
        }
        if !in_str && c == '[' {
            // Fin du crochet, en sautant les apostrophes.
            let mut j = i + 1;
            let mut q = false;
            while j < chars.len() {
                match chars[j] {
                    '\'' => q = !q,
                    ']' if !q => break,
                    _ => {}
                }
                j += 1;
            }
            if j < chars.len() {
                let inner: String = chars[i + 1..j].iter().collect();
                out.push_str(&odf_ref(&inner));
                i = j + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decalage_relatif_et_absolu() {
        assert_eq!(shift_formula("=A1+$B$2+C$3+$D4", 2, 1), "=B3+$B$2+D$3+$D6");
        assert_eq!(shift_formula("=SOMME(A1:B2)", 1, 0), "=SOMME(A2:B3)");
        assert_eq!(shift_formula("=A1", -1, 0), "=#REF!");
        assert_eq!(shift_formula("=SOMME(A1:A3)", 0, -1), "=SOMME(#REF!)");
        assert_eq!(
            shift_formula("='Feuille 2'!A1*2", 1, 0),
            "='Feuille 2'!A2*2"
        );
        assert_eq!(shift_formula("=  a1 +  1", 1, 0), "=  A2 +  1");
        assert_eq!(shift_formula("12", 5, 5), "12");
        assert_eq!(shift_formula("=\"A1\"&A1", 1, 0), "=\"A1\"&A2");
    }

    #[test]
    fn insertion_et_suppression() {
        let ins = Structural {
            rows: true,
            insert: true,
            at: 2,
            n: 2,
        };
        let all = |_: Option<&str>| true;
        assert_eq!(
            adjust_formula("=A1+A3+SOMME(A1:A5)", ins, all),
            "=A1+A5+SOMME(A1:A7)"
        );
        let del = Structural {
            rows: true,
            insert: false,
            at: 1,
            n: 2,
        };
        assert_eq!(adjust_formula("=A1+A2+A4", del, all), "=A1+#REF!+A2");
        assert_eq!(adjust_formula("=SOMME(A1:A5)", del, all), "=SOMME(A1:A3)");
        assert_eq!(adjust_formula("=SOMME(A2:A3)", del, all), "=SOMME(#REF!)");
        assert_eq!(adjust_formula("=SOMME(A3:A6)", del, all), "=SOMME(A2:A4)");
        let delc = Structural {
            rows: false,
            insert: false,
            at: 0,
            n: 1,
        };
        assert_eq!(adjust_formula("=$B$1+A1", delc, all), "=$A$1+#REF!");
    }

    #[test]
    fn traduction_anglaise() {
        assert_eq!(translate_to_english("=SOMME(A1;B2)"), "=SUM(A1,B2)");
        assert_eq!(
            translate_to_english("=SI(A1>=1,5;\"oui\";FAUX)"),
            "=IF(A1>=1.5,\"oui\",FALSE)"
        );
        assert_eq!(
            translate_to_english("=Feuille2!A1+'Feuille 3'!B2:C3"),
            "='Feuille2'!A1+'Feuille 3'!B2:C3"
        );
        assert_eq!(
            translate_to_english("=texte(A1;\"0,00\")"),
            "=TEXT(A1,\"0.00\")"
        );
        assert_eq!(translate_to_english("=#VALEUR!"), "=#VALUE!");
        assert_eq!(translate_to_english("=inconnue(1;2)"), "=inconnue(1,2)");
        assert_eq!(translate_to_english("texte"), "texte");
        assert_eq!(
            to_xlsx_formula("=CONCAT(A1;\"x\")"),
            "_xlfn.CONCAT(A1,\"x\")"
        );
    }

    #[test]
    fn openformula() {
        assert_eq!(to_odf_formula("=SOMME(A1:A3)"), "of:=SUM([.A1:.A3])");
        assert_eq!(
            to_odf_formula("='Feuille 2'!$B$2*VRAI"),
            "of:=[$'Feuille 2'.$B$2]*TRUE()"
        );
        assert_eq!(to_odf_formula("=SI(A1;1,5;2)"), "of:=IF([.A1];1.5;2)");
    }

    #[test]
    fn formules_importees() {
        assert_eq!(from_xlsx_formula("SUM(A1,B2)"), "=SOMME(A1;B2)");
        assert_eq!(
            from_xlsx_formula("IF(A1>=1.5,\"a,b\",FALSE)"),
            "=SI(A1>=1,5;\"a,b\";FAUX)"
        );
        assert_eq!(
            from_xlsx_formula("'Feuille, 2'!A1+_xlfn.CONCAT(\"x\",B1)"),
            "='Feuille, 2'!A1+CONCAT(\"x\";B1)"
        );
        assert_eq!(from_xlsx_formula("SUM(A:A)"), "=SOMME(A$1:A$1048576)");
        assert_eq!(from_xlsx_formula("#DIV/0!"), "=#DIV/0!");
        assert_eq!(
            from_xlsx_formula("VLOOKUP(1,A1:B3,2,0)"),
            "=RECHERCHEV(1;A1:B3;2;0)"
        );
        assert_eq!(from_odf_formula("of:=SUM([.A1:.A3])"), "=SOMME(A1:A3)");
        assert_eq!(
            from_odf_formula("of:=[$'Feuille 2'.$B$2]*TRUE()"),
            "='Feuille 2'!$B$2*VRAI()"
        );
        assert_eq!(
            from_odf_formula("of:=IF([.A1];1.5;\"[x]\")"),
            "=SI(A1;1,5;\"[x]\")"
        );
        assert_eq!(
            from_odf_formula("of:=[$Feuille2.A1:.B2]"),
            "=Feuille2!A1:B2"
        );
        assert_eq!(from_odf_formula("of:=[.#REF!]+1"), "=#REF!+1");
        assert_eq!(from_odf_formula("msoxl:=SUM(A1,A2)"), "=SOMME(A1;A2)");
        // Aller-retour avec les exports.
        for f in [
            "=SOMME(A1:A3)*2",
            "=SI(A1>=1,5;\"oui\";FAUX)",
            "='Feuille 2'!$B$2&\"x\"",
            "=RECHERCHEV(A1;'L''autre'!A1:B3;2;0)",
        ] {
            assert_eq!(from_xlsx_formula(&to_xlsx_formula(f)), f, "xlsx {f}");
            let odf = to_odf_formula(f);
            let back = from_odf_formula(&odf);
            assert_eq!(
                back.replace("VRAI()", "VRAI").replace("FAUX()", "FAUX"),
                f,
                "ods {f}"
            );
        }
    }

    #[test]
    fn normalisation() {
        assert_eq!(
            normalize_formula("=somme(a1:b$2)+toto"),
            "=SOMME(A1:B$2)+toto"
        );
        assert_eq!(normalize_formula("=vrai"), "=VRAI");
    }
}
