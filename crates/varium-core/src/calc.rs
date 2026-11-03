//! Champs calculés : `{{= quantité * prix_unitaire | money}}`.
//!
//! Une formule combine des nombres et des variables avec `+ - * /`, des parenthèses
//! et des pourcentages (`20%` vaut `0,2`). Les nombres s'écrivent à la française
//! (`12,5`) ou à l'anglaise (`12.5`). Les noms de variables utilisables dans une
//! formule sont faits de lettres, de chiffres, de `_` et de `.` (le `-` y est une
//! soustraction).
//!
//! Fonctions : `arrondi(x)` / `arrondi(x; 2)`, `min(a; b; …)`, `max(a; b; …)`,
//! `abs(x)`, `somme(a; b; …)`. Les arguments se séparent par `;`, comme dans un
//! tableur français.

use crate::filters;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
    LParen,
    RParen,
    Semi,
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.'
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() || c == filters::NBSP || c == filters::THIN_NBSP {
            i += 1;
        } else if c.is_ascii_digit()
            || ((c == ',' || c == '.')
                && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit())
                && !matches!(out.last(), Some(Tok::Ident(_))))
        {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || ((chars[i] == ',' || chars[i] == '.')
                        && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit())))
            {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            let n = filters::parse_number(&s).ok_or_else(|| format!("nombre invalide : {s}"))?;
            out.push(Tok::Num(n));
        } else if is_ident_char(c) {
            let start = i;
            while i < chars.len() && is_ident_char(chars[i]) {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
        } else {
            out.push(match c {
                '+' | '-' | '*' | '/' | '%' | '×' | '÷' => Tok::Op(match c {
                    '×' => '*',
                    '÷' => '/',
                    o => o,
                }),
                '(' => Tok::LParen,
                ')' => Tok::RParen,
                ';' => Tok::Semi,
                other => return Err(format!("caractère inattendu : « {other} »")),
            });
            i += 1;
        }
    }
    Ok(out)
}

/// Les variables utilisées par une formule, dans l'ordre (avec répétitions).
pub fn identifiers(formula: &str) -> Vec<String> {
    let Ok(toks) = tokenize(formula) else {
        return Vec::new();
    };
    toks.iter()
        .enumerate()
        .filter_map(|(i, t)| match t {
            // un identifiant suivi d'une parenthèse est une fonction
            Tok::Ident(name) if !matches!(toks.get(i + 1), Some(Tok::LParen)) => Some(name.clone()),
            _ => None,
        })
        .collect()
}

/// Renomme une variable dans une formule, sans toucher au reste.
pub fn rename_in(formula: &str, old: &str, new: &str) -> String {
    let chars: Vec<char> = formula.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if is_ident_char(chars[i]) && !chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && is_ident_char(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let is_call = chars[i..].iter().find(|c| !c.is_whitespace()) == Some(&'(');
            if word == old && !is_call {
                out.push_str(new);
            } else {
                out.push_str(&word);
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

struct Parser<'a, F: Fn(&str) -> Option<f64>> {
    toks: &'a [Tok],
    pos: usize,
    lookup: F,
}

impl<F: Fn(&str) -> Option<f64>> Parser<'_, F> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    // expr := term (('+'|'-') term)*
    fn expr(&mut self) -> Result<f64, String> {
        let mut v = self.term()?;
        while let Some(Tok::Op(o @ ('+' | '-'))) = self.peek().cloned() {
            self.pos += 1;
            let r = self.term()?;
            v = if o == '+' { v + r } else { v - r };
        }
        Ok(v)
    }

    // term := unary (('*'|'/') unary)*
    fn term(&mut self) -> Result<f64, String> {
        let mut v = self.unary()?;
        while let Some(Tok::Op(o @ ('*' | '/'))) = self.peek().cloned() {
            self.pos += 1;
            let r = self.unary()?;
            if o == '*' {
                v *= r;
            } else {
                if r == 0.0 {
                    return Err("division par zéro".into());
                }
                v /= r;
            }
        }
        Ok(v)
    }

    // unary := ('-'|'+') unary | postfix
    fn unary(&mut self) -> Result<f64, String> {
        match self.peek() {
            Some(Tok::Op('-')) => {
                self.pos += 1;
                Ok(-self.unary()?)
            }
            Some(Tok::Op('+')) => {
                self.pos += 1;
                self.unary()
            }
            _ => self.postfix(),
        }
    }

    // postfix := atom '%'*
    fn postfix(&mut self) -> Result<f64, String> {
        let mut v = self.atom()?;
        while let Some(Tok::Op('%')) = self.peek() {
            self.pos += 1;
            v /= 100.0;
        }
        Ok(v)
    }

    fn atom(&mut self) -> Result<f64, String> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(n),
            Some(Tok::LParen) => {
                let v = self.expr()?;
                match self.next() {
                    Some(Tok::RParen) => Ok(v),
                    _ => Err("parenthèse fermante attendue".into()),
                }
            }
            Some(Tok::Ident(name)) => {
                if let Some(Tok::LParen) = self.peek() {
                    self.pos += 1;
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Some(Tok::RParen)) {
                        loop {
                            args.push(self.expr()?);
                            match self.next() {
                                Some(Tok::Semi) => continue,
                                Some(Tok::RParen) => break,
                                _ => return Err("« ; » ou « ) » attendu".into()),
                            }
                        }
                    } else {
                        self.pos += 1;
                    }
                    call(&name, &args)
                } else {
                    (self.lookup)(&name).ok_or_else(|| format!("« {name} » n'a pas de valeur"))
                }
            }
            Some(t) => Err(format!("symbole inattendu : {t:?}")),
            None => Err("formule incomplète".into()),
        }
    }
}

fn call(name: &str, args: &[f64]) -> Result<f64, String> {
    let need = |n: usize| {
        if args.len() < n {
            Err(format!("{name} attend au moins {n} argument(s)"))
        } else {
            Ok(())
        }
    };
    match name.to_lowercase().as_str() {
        "arrondi" | "round" => {
            need(1)?;
            let d = args.get(1).copied().unwrap_or(0.0).clamp(0.0, 10.0) as i32;
            let f = 10f64.powi(d);
            Ok((args[0] * f).round() / f)
        }
        "abs" => {
            need(1)?;
            Ok(args[0].abs())
        }
        "min" => {
            need(1)?;
            Ok(args.iter().copied().fold(f64::INFINITY, f64::min))
        }
        "max" => {
            need(1)?;
            Ok(args.iter().copied().fold(f64::NEG_INFINITY, f64::max))
        }
        "somme" | "sum" => Ok(args.iter().sum()),
        other => Err(format!("fonction inconnue : {other}")),
    }
}

/// Évalue une formule ; `lookup` donne la valeur numérique d'une variable.
pub fn eval(formula: &str, lookup: impl Fn(&str) -> Option<f64>) -> Result<f64, String> {
    let toks = tokenize(formula)?;
    if toks.is_empty() {
        return Err("formule vide".into());
    }
    let mut p = Parser {
        toks: &toks,
        pos: 0,
        lookup,
    };
    let v = p.expr()?;
    if p.pos != toks.len() {
        return Err("symbole en trop dans la formule".into());
    }
    if !v.is_finite() {
        return Err("résultat non fini".into());
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(f: &str) -> Result<f64, String> {
        eval(f, |n| match n {
            "quantité" => Some(3.0),
            "prix" => Some(12.5),
            "total_ht" => Some(100.0),
            _ => None,
        })
    }

    #[test]
    fn operations() {
        assert_eq!(ev("quantité * prix"), Ok(37.5));
        assert_eq!(ev("1 + 2 * 3"), Ok(7.0));
        assert_eq!(ev("(1 + 2) * 3"), Ok(9.0));
        assert_eq!(ev("-2 + 5"), Ok(3.0));
        assert_eq!(ev("total_ht * 20%"), Ok(20.0));
        assert_eq!(ev("12,5 * 2"), Ok(25.0));
        assert_eq!(ev("10 / 4"), Ok(2.5));
        assert_eq!(ev("arrondi(10 / 3; 2)"), Ok(3.33));
        assert_eq!(ev("max(1; prix; 3)"), Ok(12.5));
        assert_eq!(ev("somme(1; 2; 3)"), Ok(6.0));
    }

    #[test]
    fn erreurs() {
        assert!(ev("inconnu * 2").is_err());
        assert!(ev("1 / 0").is_err());
        assert!(ev("(1 + 2").is_err());
        assert!(ev("1 2").is_err());
        assert!(ev("").is_err());
    }

    #[test]
    fn identifiants_et_renommage() {
        assert_eq!(identifiers("arrondi(a * b; 2) + a"), ["a", "b", "a"]);
        assert_eq!(rename_in("a*ab + a", "a", "x"), "x*ab + x");
        assert_eq!(rename_in("min(a; 2)", "min", "y"), "min(a; 2)");
    }
}
