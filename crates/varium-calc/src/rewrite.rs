//! Réécriture des formules : décalage des références (copier-coller, recopie),
//! insertion et suppression de lignes ou de colonnes, renommage de feuilles,
//! traduction en anglais (XLSX) et en OpenFormula (ODS).
//!
//! La réécriture se fait jeton par jeton : les espaces et l'écriture d'origine
//! (noms de fonctions en français, minuscules…) sont conservés.

use crate::format::pattern_to_english;
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
    fn map_span(&self, lo: u32, hi: u32) -> Option<(u32, u32)> {
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
    fn normalisation() {
        assert_eq!(
            normalize_formula("=somme(a1:b$2)+toto"),
            "=SOMME(A1:B$2)+toto"
        );
        assert_eq!(normalize_formula("=vrai"), "=VRAI");
    }
}
