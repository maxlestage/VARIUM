//! Analyse des formules en arbre syntaxique.
//!
//! Priorités (de la plus forte à la plus faible), comme dans LibreOffice Calc :
//! signe `-` unaire, `%` postfixé, `^`, `*` `/`, `+` `-`, `&`, comparaisons.
//! Les suites d'opérateurs de même priorité sont gardées « à plat » (pas d'arbre
//! profond pour `1+1+1+…`), et l'imbrication des parenthèses et des fonctions
//! est limitée : aucune saisie ne peut faire déborder la pile.

use crate::functions::{F, lookup};
use crate::lexer::{Kind, Op, Tok, tokenize};
use crate::refs::{CellRef, Range};
use crate::value::CalcError;

/// Longueur maximale d'une formule (en octets), comme Excel.
pub(crate) const MAX_FORMULA_LEN: usize = 8192;
/// Imbrication maximale des parenthèses et des appels de fonction.
pub(crate) const MAX_DEPTH: u32 = 64;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Expr {
    Num(f64),
    Str(String),
    Bool(bool),
    Err(CalcError),
    /// Argument omis : `SI(A1;;2)`.
    Missing,
    /// Référence (cellule seule ou plage) déjà rattachée à sa feuille.
    Ref {
        sheet: usize,
        range: Range,
    },
    Neg(Box<Expr>),
    /// `--x` : conversion en nombre.
    ToNum(Box<Expr>),
    /// `x%%` : division par 100 autant de fois qu'il y a de `%`.
    Percent(Box<Expr>, u32),
    /// Suite d'opérations de même priorité, évaluée de gauche à droite.
    Chain(Box<Expr>, Vec<(Op, Expr)>),
    /// Appel de fonction (`None` : fonction inconnue → `#NOM?`).
    Call(Option<F>, Vec<Expr>),
}

pub(crate) struct ParseError {
    pub message: String,
}

fn err<T>(message: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError {
        message: message.into(),
    })
}

struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
    depth: u32,
    current: usize,
    resolve: &'a dyn Fn(&str) -> Option<usize>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Kind> {
        self.toks.get(self.pos).map(|t| &t.kind)
    }

    fn peek_op(&self) -> Option<Op> {
        match self.peek() {
            Some(Kind::Op(op)) => Some(*op),
            _ => None,
        }
    }

    fn chain(
        &mut self,
        ops: &[Op],
        next: fn(&mut Self) -> Result<Expr, ParseError>,
    ) -> Result<Expr, ParseError> {
        let first = next(self)?;
        let mut rest = Vec::new();
        while let Some(op) = self.peek_op() {
            if !ops.contains(&op) {
                break;
            }
            self.pos += 1;
            rest.push((op, next(self)?));
        }
        Ok(if rest.is_empty() {
            first
        } else {
            Expr::Chain(Box::new(first), rest)
        })
    }

    fn comparison(&mut self) -> Result<Expr, ParseError> {
        self.chain(
            &[Op::Eq, Op::Ne, Op::Lt, Op::Gt, Op::Le, Op::Ge],
            Self::concat,
        )
    }

    fn concat(&mut self) -> Result<Expr, ParseError> {
        self.chain(&[Op::Concat], Self::additive)
    }

    fn additive(&mut self) -> Result<Expr, ParseError> {
        self.chain(&[Op::Add, Op::Sub], Self::term)
    }

    fn term(&mut self) -> Result<Expr, ParseError> {
        self.chain(&[Op::Mul, Op::Div], Self::power)
    }

    fn power(&mut self) -> Result<Expr, ParseError> {
        self.chain(&[Op::Pow], Self::unary)
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        let mut minus = 0u32;
        while let Some(op @ (Op::Add | Op::Sub)) = self.peek_op() {
            if op == Op::Sub {
                minus = minus.saturating_add(1);
            }
            self.pos += 1;
        }
        let e = self.postfix()?;
        Ok(match minus {
            0 => e,
            m if m % 2 == 1 => Expr::Neg(Box::new(e)),
            _ => Expr::ToNum(Box::new(e)),
        })
    }

    fn postfix(&mut self) -> Result<Expr, ParseError> {
        let e = self.primary()?;
        let mut n = 0u32;
        while self.peek_op() == Some(Op::Percent) {
            self.pos += 1;
            n = n.saturating_add(1);
        }
        Ok(if n == 0 {
            e
        } else {
            Expr::Percent(Box::new(e), n)
        })
    }

    fn enter(&mut self) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return err(format!(
                "Trop de parenthèses ou de fonctions imbriquées ({MAX_DEPTH} au plus)"
            ));
        }
        Ok(())
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let Some(tok) = self.toks.get(self.pos) else {
            return err("Formule incomplète : il manque une valeur à la fin");
        };
        self.pos += 1;
        Ok(match &tok.kind {
            Kind::Num(n) => Expr::Num(*n),
            Kind::Str(s) => Expr::Str(s.clone()),
            Kind::Bool(b) => Expr::Bool(*b),
            Kind::Err(e) => Expr::Err(*e),
            Kind::Name(_) => Expr::Err(CalcError::Name),
            Kind::Ref(r) => {
                let sheet = match &r.sheet {
                    None => Some(self.current),
                    Some(part) => (self.resolve)(&part.name),
                };
                match sheet {
                    Some(sheet) => {
                        let a: CellRef = r.start.cell();
                        let b = r.end.map(|e| e.cell()).unwrap_or(a);
                        Expr::Ref {
                            sheet,
                            range: Range::new(a, b),
                        }
                    }
                    None => Expr::Err(CalcError::Ref),
                }
            }
            Kind::Func(name) => {
                let f = lookup(name);
                if self.peek() != Some(&Kind::LParen) {
                    return err(format!("Parenthèse ouvrante attendue après {name}"));
                }
                self.pos += 1;
                self.enter()?;
                let args = self.arguments(name)?;
                self.depth -= 1;
                Expr::Call(f, args)
            }
            Kind::LParen => {
                self.enter()?;
                let e = self.comparison()?;
                if self.peek() != Some(&Kind::RParen) {
                    return err("Parenthèse fermante manquante");
                }
                self.pos += 1;
                self.depth -= 1;
                e
            }
            Kind::RParen => return err("Parenthèse fermante en trop, ou valeur manquante"),
            Kind::Semi => return err("Point-virgule inattendu (en dehors d'une fonction ?)"),
            Kind::Op(_) => return err("Opérateur inattendu : il manque une valeur"),
            Kind::Bad(s) if s.starts_with(',') => {
                return err("Le séparateur d'arguments est le point-virgule « ; »");
            }
            Kind::Bad(s) if s.starts_with('"') => return err("Guillemet fermant manquant"),
            Kind::Bad(s) if s.starts_with('\'') => {
                return err("Nom de feuille entre apostrophes mal fermé ou sans cellule");
            }
            Kind::Bad(s) => return err(format!("Caractère inattendu : « {s} »")),
        })
    }

    fn arguments(&mut self, name: &str) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if self.peek() == Some(&Kind::RParen) {
            self.pos += 1;
            return Ok(args);
        }
        loop {
            match self.peek() {
                Some(Kind::Semi | Kind::RParen) => args.push(Expr::Missing),
                _ => args.push(self.comparison()?),
            }
            match self.peek() {
                Some(Kind::Semi) => self.pos += 1,
                Some(Kind::RParen) => {
                    self.pos += 1;
                    return Ok(args);
                }
                Some(Kind::Bad(s)) if s.starts_with(',') => {
                    return err("Le séparateur d'arguments est le point-virgule « ; »");
                }
                None => return err(format!("Parenthèse fermante manquante pour {name}")),
                Some(_) => {
                    return err(format!(
                        "Opérateur ou « ; » manquant dans les arguments de {name}"
                    ));
                }
            }
        }
    }
}

/// Analyse une formule complète (commençant par `=`). `current` est la feuille
/// de la formule ; `resolve` donne l'index d'une feuille d'après son nom.
pub(crate) fn parse(
    input: &str,
    current: usize,
    resolve: &dyn Fn(&str) -> Option<usize>,
) -> Result<Expr, ParseError> {
    if input.len() > MAX_FORMULA_LEN {
        return err(format!(
            "Formule trop longue ({MAX_FORMULA_LEN} caractères au plus)"
        ));
    }
    let toks = tokenize(input);
    if toks.is_empty() {
        return err("Formule vide");
    }
    let mut p = Parser {
        toks: &toks,
        pos: 0,
        depth: 0,
        current,
        resolve,
    };
    let e = p.comparison()?;
    if p.pos < toks.len() {
        return match &toks[p.pos].kind {
            Kind::RParen => err("Parenthèse fermante en trop"),
            Kind::Bad(s) if s.starts_with(',') => {
                err("Le séparateur d'arguments est le point-virgule « ; »")
            }
            Kind::Bad(s) => err(format!("Caractère inattendu : « {s} »")),
            _ => err("Opérateur manquant entre deux valeurs"),
        };
    }
    Ok(e)
}

/// Vérifie la syntaxe d'une formule (toutes les feuilles sont supposées exister).
/// Renvoie un message d'erreur en français.
pub fn check_formula(input: &str) -> Result<(), String> {
    if !input.starts_with('=') {
        return Ok(());
    }
    parse(input, 0, &|_| Some(0))
        .map(|_| ())
        .map_err(|e| e.message)
}

/// Références (feuille, plage) contenues dans une expression.
pub(crate) fn collect_refs(e: &Expr, out: &mut Vec<(usize, Range)>) {
    match e {
        Expr::Ref { sheet, range } => out.push((*sheet, *range)),
        Expr::Neg(x) | Expr::ToNum(x) | Expr::Percent(x, _) => collect_refs(x, out),
        Expr::Chain(first, rest) => {
            collect_refs(first, out);
            for (_, x) in rest {
                collect_refs(x, out);
            }
        }
        Expr::Call(_, args) => {
            for a in args {
                collect_refs(a, out);
            }
        }
        Expr::Num(_) | Expr::Str(_) | Expr::Bool(_) | Expr::Err(_) | Expr::Missing => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<Expr, String> {
        parse(s, 0, &|n| (n.to_lowercase() == "feuille2").then_some(1)).map_err(|e| e.message)
    }

    #[test]
    fn priorites() {
        let e = p("=1+2*3").unwrap();
        let Expr::Chain(_, rest) = e else { panic!() };
        assert_eq!(rest.len(), 1);
        assert!(matches!(rest[0].1, Expr::Chain(..)));
        assert!(matches!(p("=-2^2").unwrap(), Expr::Chain(..)));
        assert!(matches!(p("=--A1").unwrap(), Expr::ToNum(_)));
        assert!(matches!(p("=---A1").unwrap(), Expr::Neg(_)));
        assert!(matches!(p("=50%%").unwrap(), Expr::Percent(_, 2)));
    }

    #[test]
    fn references_resolues() {
        let e = p("=Feuille2!B2").unwrap();
        assert_eq!(
            e,
            Expr::Ref {
                sheet: 1,
                range: Range::cell(CellRef::new(1, 1))
            }
        );
        assert_eq!(p("=Inconnue!B2").unwrap(), Expr::Err(CalcError::Ref));
    }

    #[test]
    fn erreurs_de_syntaxe() {
        for s in [
            "=1+",
            "=(1",
            "=1)",
            "=SOMME(1;2",
            "=SOMME(A1,B2)",
            "=1 2",
            "=\"abc",
            "=",
            "=;",
            "=1,2,3",
        ] {
            assert!(p(s).is_err(), "{s} devrait échouer");
        }
        assert!(p("=SOMME(A1,B2)").unwrap_err().contains("point-virgule"));
        // Comme dans LibreOffice en français : « 1,2 » est un nombre décimal.
        assert!(p("=SOMME(1,2)").is_ok());
        assert!(p("=SI(A1;;2)").is_ok());
        assert!(p("=PI()").is_ok());
        // Un nom inconnu n'est pas une erreur de syntaxe : il vaut #NOM?.
        assert_eq!(p("=SOMME").unwrap(), Expr::Err(CalcError::Name));
    }

    #[test]
    fn imbrication_limitee() {
        let ok = format!("={}1{}", "(".repeat(60), ")".repeat(60));
        assert!(p(&ok).is_ok());
        let trop = format!("={}1{}", "(".repeat(100), ")".repeat(100));
        assert!(p(&trop).is_err());
        let long = format!("=1{}", "+1".repeat(5000));
        assert!(p(&long).unwrap_err().contains("trop longue"));
        let plat = format!("=1{}", "+1".repeat(3000));
        assert!(p(&plat).is_ok());
    }
}
