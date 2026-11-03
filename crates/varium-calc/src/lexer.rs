//! Découpage d'une formule en jetons.
//!
//! Les positions des jetons sont des octets dans la saisie complète (signe `=`
//! compris), ce qui permet de réécrire une formule (décalage des références,
//! traduction) sans toucher à sa mise en page.

use crate::refs::{Addr, parse_addr};
use crate::value::CalcError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Concat,
    Percent,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

/// Préfixe de feuille d'une référence.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SheetPart {
    /// Nom de la feuille (sans apostrophes).
    pub name: String,
    /// Position (octets) où commence l'adresse, juste après `!` ou `.`.
    pub cell_start: usize,
}

/// Référence à une cellule ou à une plage.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RefTok {
    pub sheet: Option<SheetPart>,
    pub start: Addr,
    pub end: Option<Addr>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Kind {
    Num(f64),
    Str(String),
    Bool(bool),
    Err(CalcError),
    Ref(RefTok),
    /// Nom de fonction (la parenthèse ouvrante est un jeton à part).
    Func(String),
    /// Nom inconnu (ni fonction, ni référence).
    Name(String),
    Op(Op),
    LParen,
    RParen,
    Semi,
    /// Caractère ou suite de caractères incompréhensible.
    Bad(String),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Tok {
    pub kind: Kind,
    pub start: usize,
    pub end: usize,
}

const ERROR_CODES: [&str; 11] = [
    "#DIV/0!", "#VALEUR!", "#VALUE!", "#REF!", "#NOM?", "#NAME?", "#N/A", "#NOMBRE!", "#NUM!",
    "#CIRC!", "#NULL!",
];

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.' || c == '$'
}

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    out: Vec<Tok>,
}

impl<'a> Lexer<'a> {
    fn peek(&self) -> Option<char> {
        self.src.get(self.pos..)?.chars().next()
    }

    fn peek_at(&self, byte_off: usize) -> Option<char> {
        self.src.get(self.pos + byte_off..)?.chars().next()
    }

    fn push(&mut self, kind: Kind, start: usize) {
        self.out.push(Tok {
            kind,
            start,
            end: self.pos,
        });
    }

    /// Avance tant que `f` est vrai ; renvoie le texte lu.
    fn take_while(&mut self, f: impl Fn(char) -> bool) -> &'a str {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if !f(c) {
                break;
            }
            self.pos += c.len_utf8();
        }
        &self.src[start..self.pos]
    }

    fn run(&mut self) {
        while let Some(c) = self.peek() {
            let start = self.pos;
            if c.is_whitespace() {
                self.pos += c.len_utf8();
                continue;
            }
            if c.is_ascii_digit()
                || ((c == '.' || c == ',') && self.peek_at(1).is_some_and(|n| n.is_ascii_digit()))
            {
                self.number(start);
            } else if c == '"' {
                self.string(start);
            } else if c == '\'' {
                self.quoted_sheet(start);
            } else if c == '#' {
                self.error_literal(start);
            } else if c.is_alphabetic() || c == '_' || c == '$' {
                self.identifier(start);
            } else {
                self.pos += c.len_utf8();
                let next = self.peek();
                let kind = match c {
                    '+' => Kind::Op(Op::Add),
                    '-' | '\u{2212}' => Kind::Op(Op::Sub),
                    '*' | '×' => Kind::Op(Op::Mul),
                    '/' | '÷' => Kind::Op(Op::Div),
                    '^' => Kind::Op(Op::Pow),
                    '&' => Kind::Op(Op::Concat),
                    '%' => Kind::Op(Op::Percent),
                    '=' => Kind::Op(Op::Eq),
                    '<' if next == Some('>') => {
                        self.pos += 1;
                        Kind::Op(Op::Ne)
                    }
                    '<' if next == Some('=') => {
                        self.pos += 1;
                        Kind::Op(Op::Le)
                    }
                    '>' if next == Some('=') => {
                        self.pos += 1;
                        Kind::Op(Op::Ge)
                    }
                    '<' => Kind::Op(Op::Lt),
                    '>' => Kind::Op(Op::Gt),
                    '≠' => Kind::Op(Op::Ne),
                    '≤' => Kind::Op(Op::Le),
                    '≥' => Kind::Op(Op::Ge),
                    '(' => Kind::LParen,
                    ')' => Kind::RParen,
                    ';' => Kind::Semi,
                    other => Kind::Bad(other.to_string()),
                };
                self.push(kind, start);
            }
        }
    }

    fn number(&mut self, start: usize) {
        self.take_while(|c| c.is_ascii_digit());
        if matches!(self.peek(), Some('.' | ','))
            && self.peek_at(1).is_some_and(|n| n.is_ascii_digit())
        {
            self.pos += 1;
            self.take_while(|c| c.is_ascii_digit());
        } else if matches!(self.peek(), Some('.' | ','))
            && !self.peek_at(1).is_some_and(|n| n.is_alphanumeric())
        {
            // « 12. » : séparateur final accepté.
            self.pos += 1;
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            let sign = matches!(self.peek_at(1), Some('+' | '-'));
            let d = if sign { 2 } else { 1 };
            if self.peek_at(d).is_some_and(|c| c.is_ascii_digit()) {
                self.pos += d;
                self.take_while(|c| c.is_ascii_digit());
            }
        }
        let text = self.src[start..self.pos].replace(',', ".");
        let text = text.trim_end_matches('.');
        let kind = match text.parse::<f64>() {
            Ok(n) if n.is_finite() => Kind::Num(n),
            _ => Kind::Bad(self.src[start..self.pos].to_string()),
        };
        self.push(kind, start);
    }

    fn string(&mut self, start: usize) {
        self.pos += 1;
        let mut s = String::new();
        loop {
            match self.peek() {
                None => {
                    let text = self.src[start..].to_string();
                    self.push(Kind::Bad(text), start);
                    return;
                }
                Some('"') => {
                    self.pos += 1;
                    if self.peek() == Some('"') {
                        self.pos += 1;
                        s.push('"');
                    } else {
                        break;
                    }
                }
                Some(c) => {
                    self.pos += c.len_utf8();
                    s.push(c);
                }
            }
        }
        self.push(Kind::Str(s), start);
    }

    fn quoted_sheet(&mut self, start: usize) {
        self.pos += 1;
        let mut name = String::new();
        loop {
            match self.peek() {
                None => {
                    let text = self.src[start..].to_string();
                    self.push(Kind::Bad(text), start);
                    return;
                }
                Some('\'') => {
                    self.pos += 1;
                    if self.peek() == Some('\'') {
                        self.pos += 1;
                        name.push('\'');
                    } else {
                        break;
                    }
                }
                Some(c) => {
                    self.pos += c.len_utf8();
                    name.push(c);
                }
            }
        }
        if matches!(self.peek(), Some('!' | '.')) {
            self.pos += 1;
            let cell_start = self.pos;
            if self.cell_or_range(start, Some(SheetPart { name, cell_start })) {
                return;
            }
        }
        let text = self.src[start..self.pos].to_string();
        self.push(Kind::Bad(text), start);
    }

    /// Lit une adresse ou une plage à la position courante. Faux si rien ne convient
    /// (la position est alors inchangée).
    fn cell_or_range(&mut self, start: usize, sheet: Option<SheetPart>) -> bool {
        let save = self.pos;
        let word = self.take_while(|c| c.is_ascii_alphanumeric() || c == '$');
        let Some(a) = parse_addr(word) else {
            self.pos = save;
            return false;
        };
        let end = self.range_end();
        self.push(
            Kind::Ref(RefTok {
                sheet,
                start: a,
                end,
            }),
            start,
        );
        true
    }

    /// Lit `:B7` (ou `:Feuille.B7`) après une adresse, si présent.
    fn range_end(&mut self) -> Option<Addr> {
        if self.peek() != Some(':') {
            return None;
        }
        let save = self.pos;
        self.pos += 1;
        let word = self.take_while(is_ident_char);
        let addr = parse_addr(word).or_else(|| {
            let (_, cell) = word.rsplit_once(['.', '!'])?;
            parse_addr(cell)
        });
        if addr.is_none() {
            self.pos = save;
        }
        addr
    }

    fn error_literal(&mut self, start: usize) {
        let rest = &self.src[start..];
        for code in ERROR_CODES {
            if rest
                .get(..code.len())
                .is_some_and(|p| p.eq_ignore_ascii_case(code))
            {
                self.pos += code.len();
                let e = CalcError::from_code(code).unwrap_or(CalcError::Value);
                self.push(Kind::Err(e), start);
                return;
            }
        }
        self.pos += 1;
        self.push(Kind::Bad("#".into()), start);
    }

    fn identifier(&mut self, start: usize) {
        let word = self.take_while(is_ident_char);
        // Feuille!A1
        if self.peek() == Some('!') {
            self.pos += 1;
            let cell_start = self.pos;
            let name = word.trim_start_matches('$').to_string();
            if self.cell_or_range(start, Some(SheetPart { name, cell_start })) {
                return;
            }
            let text = self.src[start..self.pos].to_string();
            self.push(Kind::Bad(text), start);
            return;
        }
        // Nom de fonction : suivi de « ( », espaces permis.
        let after_spaces = self.src[self.pos..].trim_start();
        if after_spaces.starts_with('(') {
            self.push(Kind::Func(word.to_string()), start);
            return;
        }
        if let Some(a) = parse_addr(word) {
            let end = self.range_end();
            self.push(
                Kind::Ref(RefTok {
                    sheet: None,
                    start: a,
                    end,
                }),
                start,
            );
            return;
        }
        // Style LibreOffice : Feuille2.A1
        if let Some((sheet, cell)) = word.rsplit_once('.')
            && !sheet.is_empty()
            && let Some(a) = parse_addr(cell)
        {
            let cell_start = start + sheet.len() + 1;
            let end = self.range_end();
            let name = sheet.trim_start_matches('$').to_string();
            self.push(
                Kind::Ref(RefTok {
                    sheet: Some(SheetPart { name, cell_start }),
                    start: a,
                    end,
                }),
                start,
            );
            return;
        }
        let kind = match word.to_uppercase().as_str() {
            "VRAI" | "TRUE" => Kind::Bool(true),
            "FAUX" | "FALSE" => Kind::Bool(false),
            _ => Kind::Name(word.to_string()),
        };
        self.push(kind, start);
    }
}

/// Découpe une formule (saisie complète, qui commence par `=`).
pub(crate) fn tokenize(input: &str) -> Vec<Tok> {
    let mut lx = Lexer {
        src: input,
        pos: usize::from(input.starts_with('=')),
        out: Vec::new(),
    };
    lx.run();
    lx.out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(s: &str) -> Vec<Kind> {
        tokenize(s).into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn jetons_simples() {
        let k = kinds("=SOMME(A1:B2; 1,5) & \"a\"\"b\"");
        assert!(matches!(&k[0], Kind::Func(f) if f == "SOMME"));
        assert_eq!(k[1], Kind::LParen);
        assert!(matches!(&k[2], Kind::Ref(r) if r.end.is_some()));
        assert_eq!(k[3], Kind::Semi);
        assert_eq!(k[4], Kind::Num(1.5));
        assert_eq!(k[5], Kind::RParen);
        assert_eq!(k[6], Kind::Op(Op::Concat));
        assert_eq!(k[7], Kind::Str("a\"b".into()));
    }

    #[test]
    fn references_de_feuille() {
        let k = kinds("='Feuille 2'!$A$1:B3+Feuille3!C4+Feuille4.D5");
        let Kind::Ref(r) = &k[0] else { panic!() };
        assert_eq!(r.sheet.as_ref().unwrap().name, "Feuille 2");
        assert!(r.start.abs_row && r.start.abs_col);
        assert!(r.end.is_some());
        let Kind::Ref(r) = &k[2] else { panic!() };
        assert_eq!(r.sheet.as_ref().unwrap().name, "Feuille3");
        let Kind::Ref(r) = &k[4] else { panic!() };
        assert_eq!(r.sheet.as_ref().unwrap().name, "Feuille4");
    }

    #[test]
    fn noms_et_erreurs() {
        let k = kinds("=NB.SI(A1;\">2\")+#REF!+toto+VRAI+LOG10(1)");
        assert!(matches!(&k[0], Kind::Func(f) if f == "NB.SI"));
        assert!(k.contains(&Kind::Err(CalcError::Ref)));
        assert!(k.contains(&Kind::Name("toto".into())));
        assert!(k.contains(&Kind::Bool(true)));
        assert!(k.iter().any(|x| matches!(x, Kind::Func(f) if f == "LOG10")));
    }

    #[test]
    fn caracteres_inconnus() {
        let k = kinds("=1,2,3");
        assert!(k.iter().any(|x| matches!(x, Kind::Bad(_))) || k.len() >= 2);
        let k = kinds("=\"abc");
        assert!(matches!(&k[0], Kind::Bad(_)));
        let k = kinds("='abc");
        assert!(matches!(&k[0], Kind::Bad(_)));
        assert!(kinds("=1 ? 2").iter().any(|x| matches!(x, Kind::Bad(_))));
    }
}
