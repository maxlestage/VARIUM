//! Formules en clair et erreurs expliquées.
//!
//! [`Workbook::formula_tree`] donne l'arbre d'une formule sous une forme
//! publique et simple ([`FormulaNode`]) : l'interface en tire une phrase dans
//! sa langue (« Somme de B4 à B8, multipliée par 2 »). [`Workbook::error_cause`]
//! cherche l'origine d'une erreur : la division par une cellule vide, le texte
//! dans un calcul, la cellule d'où vient une erreur propagée…

use crate::eval::{Probe, apply_op, is_formula, sheet_resolver};
use crate::format::{NumberFormat, format_value};
use crate::functions::{F, FunctionInfo, info, lookup};
use crate::lexer::{Kind, Op, tokenize};
use crate::parser::{Expr, check_formula, parse};
use crate::refs::{CellRef, Range, quote_sheet_name};
use crate::value::{CalcError, Value};
use crate::workbook::Workbook;

/// Opérateur binaire d'une formule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Concat,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

impl BinOp {
    fn from_op(op: Op) -> BinOp {
        match op {
            Op::Add => BinOp::Add,
            Op::Sub => BinOp::Sub,
            Op::Mul => BinOp::Mul,
            Op::Div => BinOp::Div,
            Op::Pow => BinOp::Pow,
            Op::Concat => BinOp::Concat,
            Op::Eq => BinOp::Eq,
            Op::Ne => BinOp::Ne,
            Op::Lt => BinOp::Lt,
            Op::Gt => BinOp::Gt,
            Op::Le => BinOp::Le,
            // `%` est postfixé : il n'apparaît pas entre deux valeurs.
            Op::Ge | Op::Percent => BinOp::Ge,
        }
    }

    /// Priorité (plus grand : lie plus fort).
    pub fn precedence(self) -> u8 {
        match self {
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 1,
            BinOp::Concat => 2,
            BinOp::Add | BinOp::Sub => 3,
            BinOp::Mul | BinOp::Div => 4,
            BinOp::Pow => 5,
        }
    }
}

/// Arbre d'une formule, pour l'expliquer en clair.
#[derive(Clone, Debug, PartialEq)]
pub enum FormulaNode {
    Number(f64),
    Text(String),
    Bool(bool),
    Error(CalcError),
    /// Argument omis.
    Missing,
    /// Une cellule (`sheet` : autre feuille que celle de la formule).
    Cell {
        sheet: Option<String>,
        cell: String,
    },
    /// Une plage de cellules.
    Range {
        sheet: Option<String>,
        from: String,
        to: String,
    },
    /// Signe moins.
    Neg(Box<FormulaNode>),
    /// Pourcentage (`x%`).
    Percent(Box<FormulaNode>),
    Binary(BinOp, Box<FormulaNode>, Box<FormulaNode>),
    /// Appel de fonction (`info` : `None` si la fonction est inconnue).
    Call {
        name: String,
        info: Option<&'static FunctionInfo>,
        args: Vec<FormulaNode>,
    },
}

/// Origine d'une erreur, à expliquer en clair.
#[derive(Clone, Debug, PartialEq)]
pub enum ErrorCause {
    /// Formule mal écrite (message en français du vérificateur).
    Syntax(String),
    /// Division par zéro ; `cell` : le diviseur, s'il s'agit d'une cellule.
    DivZero { cell: Option<String>, empty: bool },
    /// Moyenne sans aucun nombre.
    EmptyAverage,
    /// Du texte là où un nombre est attendu.
    Text { cell: Option<String>, text: String },
    /// Référence à une cellule ou une feuille supprimée.
    BadRef,
    /// Nom de fonction (ou nom) inconnu.
    UnknownName(String),
    /// La formule dépend d'elle-même.
    Circular,
    /// Valeur cherchée introuvable (`RECHERCHEV`, `EQUIV`…).
    NotFound(Option<String>),
    /// Nombre invalide ou trop grand.
    Num,
    /// L'erreur vient de cette cellule.
    FromCell { cell: String, error: CalcError },
    /// Autre erreur, sans plus de détail.
    Other(CalcError),
}

struct Names<'a> {
    current: usize,
    sheets: Vec<&'a str>,
    unknown: Vec<String>,
}

impl Names<'_> {
    fn sheet(&self, s: usize) -> Option<String> {
        (s != self.current).then(|| quote_sheet_name(self.sheets.get(s).copied().unwrap_or("?")))
    }

    fn cell(&self, s: usize, c: CellRef) -> String {
        match self.sheet(s) {
            Some(p) => format!("{p}!{}", c.name()),
            None => c.name(),
        }
    }

    fn first_unknown(&self) -> String {
        self.unknown.first().cloned().unwrap_or_default()
    }
}

/// Noms de fonctions et noms inconnus d'une formule, dans l'ordre.
fn unknown_names(formula: &str) -> Vec<String> {
    tokenize(formula)
        .into_iter()
        .filter_map(|t| match t.kind {
            Kind::Func(n) if lookup(&n).is_none() => Some(n),
            Kind::Name(n) => Some(n),
            _ => None,
        })
        .collect()
}

fn node(e: &Expr, names: &Names, unknown_i: &mut usize) -> FormulaNode {
    match e {
        Expr::Num(n) => FormulaNode::Number(*n),
        Expr::Str(s) => FormulaNode::Text(s.clone()),
        Expr::Bool(b) => FormulaNode::Bool(*b),
        Expr::Err(e) => FormulaNode::Error(*e),
        Expr::Missing => FormulaNode::Missing,
        Expr::Ref { sheet, range } => {
            if range.start == range.end {
                FormulaNode::Cell {
                    sheet: names.sheet(*sheet),
                    cell: range.start.name(),
                }
            } else {
                FormulaNode::Range {
                    sheet: names.sheet(*sheet),
                    from: range.start.name(),
                    to: range.end.name(),
                }
            }
        }
        Expr::Neg(x) => FormulaNode::Neg(Box::new(node(x, names, unknown_i))),
        Expr::ToNum(x) => node(x, names, unknown_i),
        Expr::Percent(x, k) => {
            let mut n = node(x, names, unknown_i);
            for _ in 0..(*k).min(8) {
                n = FormulaNode::Percent(Box::new(n));
            }
            n
        }
        Expr::Chain(first, rest) => {
            let mut acc = node(first, names, unknown_i);
            for (op, x) in rest {
                acc = FormulaNode::Binary(
                    BinOp::from_op(*op),
                    Box::new(acc),
                    Box::new(node(x, names, unknown_i)),
                );
            }
            acc
        }
        Expr::Call(f, args) => {
            let (name, fi) = match f {
                Some(id) => (info(*id).fr.to_string(), Some(info(*id))),
                None => {
                    let n = names.unknown.get(*unknown_i).cloned().unwrap_or_default();
                    *unknown_i += 1;
                    (n, None)
                }
            };
            FormulaNode::Call {
                name,
                info: fi,
                args: args.iter().map(|a| node(a, names, unknown_i)).collect(),
            }
        }
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => *n != 0.0,
        _ => false,
    }
}

/// Texte qui ne se lit pas comme un nombre.
fn non_numeric_text(v: &Value) -> Option<String> {
    match v {
        Value::Text(t) if crate::eval::to_num(v).is_err() => Some(t.clone()),
        _ => None,
    }
}

struct Explainer<'a> {
    probe: Probe<'a>,
    names: Names<'a>,
}

impl Explainer<'_> {
    fn ref_name(&self, e: &Expr) -> Option<String> {
        match e {
            Expr::Ref { sheet, range } if range.start == range.end => {
                Some(self.names.cell(*sheet, range.start))
            }
            _ => None,
        }
    }

    fn error_in_range(&self, sheet: usize, range: Range) -> Option<ErrorCause> {
        self.probe
            .first_error(sheet, range)
            .map(|(c, error)| ErrorCause::FromCell {
                cell: self.names.cell(sheet, c),
                error,
            })
    }

    /// Origine de l'erreur de `e` (`None` si `e` ne vaut pas une erreur).
    fn cause(&self, e: &Expr, depth: u32) -> Option<ErrorCause> {
        if depth > 80 {
            return None;
        }
        match e {
            Expr::Err(CalcError::Ref) => Some(ErrorCause::BadRef),
            Expr::Err(CalcError::Name) => Some(ErrorCause::UnknownName(self.names.first_unknown())),
            Expr::Err(err) => Some(ErrorCause::Other(*err)),
            Expr::Ref { sheet, range } => {
                if range.start == range.end {
                    match self.probe.cell(*sheet, range.start) {
                        Value::Error(error) => Some(ErrorCause::FromCell {
                            cell: self.names.cell(*sheet, range.start),
                            error,
                        }),
                        _ => None,
                    }
                } else {
                    match self.probe.scalar(e) {
                        Value::Error(err) => self
                            .error_in_range(*sheet, *range)
                            .or(Some(ErrorCause::Other(err))),
                        _ => None,
                    }
                }
            }
            Expr::Neg(x) | Expr::ToNum(x) | Expr::Percent(x, _) => {
                if let Some(c) = self.cause(x, depth + 1) {
                    return Some(c);
                }
                let v = self.probe.scalar(x);
                non_numeric_text(&v).map(|text| ErrorCause::Text {
                    cell: self.ref_name(x),
                    text,
                })
            }
            Expr::Chain(first, rest) => {
                let mut acc = self.probe.scalar(first);
                if let Value::Error(err) = acc {
                    return self
                        .cause(first, depth + 1)
                        .or(Some(ErrorCause::Other(err)));
                }
                let mut acc_expr: Option<&Expr> = Some(first);
                for (op, x) in rest {
                    let rhs = self.probe.scalar(x);
                    if let Value::Error(err) = rhs {
                        return self.cause(x, depth + 1).or(Some(ErrorCause::Other(err)));
                    }
                    let r = apply_op(*op, &acc, &rhs);
                    if let Value::Error(err) = r {
                        return Some(match err {
                            CalcError::Div0 if *op == Op::Div => ErrorCause::DivZero {
                                cell: self.ref_name(x),
                                empty: rhs.is_empty(),
                            },
                            CalcError::Value => {
                                if let Some(text) = non_numeric_text(&rhs) {
                                    ErrorCause::Text {
                                        cell: self.ref_name(x),
                                        text,
                                    }
                                } else if let Some(text) = non_numeric_text(&acc) {
                                    ErrorCause::Text {
                                        cell: acc_expr.and_then(|a| self.ref_name(a)),
                                        text,
                                    }
                                } else {
                                    ErrorCause::Other(err)
                                }
                            }
                            CalcError::Num => ErrorCause::Num,
                            other => ErrorCause::Other(other),
                        });
                    }
                    acc = r;
                    acc_expr = None;
                }
                None
            }
            Expr::Call(None, _) => Some(ErrorCause::UnknownName(self.names.first_unknown())),
            Expr::Call(Some(f), args) => {
                let Value::Error(err) = self.probe.scalar(e) else {
                    return None;
                };
                match f {
                    F::Si => {
                        let cond = args.first()?;
                        if let Some(c) = self.cause(cond, depth + 1) {
                            return Some(c);
                        }
                        let b = truthy(&self.probe.scalar(cond));
                        if let Some(branch) = args.get(if b { 1 } else { 2 })
                            && let Some(c) = self.cause(branch, depth + 1)
                        {
                            return Some(c);
                        }
                        return Some(ErrorCause::Other(err));
                    }
                    F::SiErreur => {
                        if let Some(c) = args.get(1).and_then(|a| self.cause(a, depth + 1)) {
                            return Some(c);
                        }
                        return Some(ErrorCause::Other(err));
                    }
                    _ => {}
                }
                for a in args {
                    let found = match a {
                        Expr::Ref { sheet, range } if range.start != range.end => self
                            .error_in_range(*sheet, *range)
                            .filter(|c| matches!(c, ErrorCause::FromCell { error, .. } if *error == err)),
                        _ => self.cause(a, depth + 1),
                    };
                    if found.is_some() {
                        return found;
                    }
                }
                Some(match (f, err) {
                    (F::Moyenne | F::MoyenneSi, CalcError::Div0) => ErrorCause::EmptyAverage,
                    (F::Mod, CalcError::Div0) => ErrorCause::DivZero {
                        cell: args.get(1).and_then(|a| self.ref_name(a)),
                        empty: args.get(1).is_some_and(|a| self.probe.scalar(a).is_empty()),
                    },
                    (_, CalcError::Div0) => ErrorCause::DivZero {
                        cell: None,
                        empty: false,
                    },
                    (_, CalcError::NA) => ErrorCause::NotFound(
                        args.first()
                            .map(|a| format_value(&self.probe.scalar(a), NumberFormat::General)),
                    ),
                    (_, CalcError::Num) => ErrorCause::Num,
                    (_, CalcError::Ref) => ErrorCause::BadRef,
                    (_, other) => ErrorCause::Other(other),
                })
            }
            _ => None,
        }
    }
}

impl Workbook {
    fn names_for(&self, sheet: usize, formula: &str) -> Names<'_> {
        Names {
            current: sheet,
            sheets: self.sheets.iter().map(|s| s.name.as_str()).collect(),
            unknown: unknown_names(formula),
        }
    }

    /// Arbre d'une formule (`=…`) écrite dans la feuille `sheet`, pour
    /// l'expliquer en clair ; `None` si ce n'est pas une formule valide.
    pub fn formula_tree(&self, sheet: usize, formula: &str) -> Option<FormulaNode> {
        if !is_formula(formula) {
            return None;
        }
        let names = sheet_resolver(&self.sheets);
        let resolve = |n: &str| names.get(&n.to_lowercase()).copied();
        let e = parse(formula, sheet, &resolve).ok()?;
        let n = self.names_for(sheet, formula);
        Some(node(&e, &n, &mut 0))
    }

    /// Calcule une formule comme si elle était dans la cellule `at`, sans rien
    /// modifier (aperçu de l'assistant de formule).
    pub fn evaluate(&self, sheet: usize, at: CellRef, formula: &str) -> Value {
        if !is_formula(formula) {
            return crate::format::parse_literal(formula).0;
        }
        let names = sheet_resolver(&self.sheets);
        let resolve = |n: &str| names.get(&n.to_lowercase()).copied();
        let Ok(e) = parse(formula, sheet, &resolve) else {
            return Value::Error(CalcError::Name);
        };
        let computed = self.computed();
        Probe::new(&self.sheets, &computed.values, at, self.today()).result(&e)
    }

    /// Origine de l'erreur d'une formule placée en `at` (`None` si elle ne
    /// donne pas d'erreur).
    pub fn formula_error_cause(
        &self,
        sheet: usize,
        at: CellRef,
        formula: &str,
    ) -> Option<ErrorCause> {
        if !is_formula(formula) {
            return match crate::format::parse_literal(formula).0 {
                Value::Error(e) => Some(ErrorCause::Other(e)),
                _ => None,
            };
        }
        if let Err(msg) = check_formula(formula) {
            return Some(ErrorCause::Syntax(msg));
        }
        let names = sheet_resolver(&self.sheets);
        let resolve = |n: &str| names.get(&n.to_lowercase()).copied();
        let e = parse(formula, sheet, &resolve).ok()?;
        let computed = self.computed();
        let probe = Probe::new(&self.sheets, &computed.values, at, self.today());
        let Value::Error(err) = probe.result(&e) else {
            return None;
        };
        let ex = Explainer {
            probe,
            names: self.names_for(sheet, formula),
        };
        ex.cause(&e, 0).or(Some(ErrorCause::Other(err)))
    }

    /// Origine de l'erreur d'une cellule (`None` si elle n'est pas en erreur).
    pub fn error_cause(&self, sheet: usize, at: CellRef) -> Option<ErrorCause> {
        let input = self.input(sheet, at).to_string();
        if let Err(msg) = check_formula(&input) {
            return Some(ErrorCause::Syntax(msg));
        }
        let Value::Error(err) = self.value(sheet, at) else {
            return None;
        };
        if err == CalcError::Circular {
            return Some(ErrorCause::Circular);
        }
        if !is_formula(&input) {
            return Some(ErrorCause::Other(err));
        }
        self.formula_error_cause(sheet, at, &input)
            .or(Some(ErrorCause::Other(err)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wb(cells: &[(&str, &str)]) -> Workbook {
        let mut wb = Workbook::new();
        for (at, input) in cells {
            wb.set_input(0, CellRef::parse(at).unwrap(), input);
        }
        wb
    }

    fn at(s: &str) -> CellRef {
        CellRef::parse(s).unwrap()
    }

    #[test]
    fn arbre_d_une_formule() {
        let w = wb(&[]);
        let t = w.formula_tree(0, "=SOMME(B4:B8)*2").unwrap();
        let FormulaNode::Binary(BinOp::Mul, a, b) = t else {
            panic!("{t:?}")
        };
        assert_eq!(*b, FormulaNode::Number(2.0));
        let FormulaNode::Call { name, info, args } = *a else {
            panic!()
        };
        assert_eq!(name, "SOMME");
        assert!(info.is_some());
        assert_eq!(
            args,
            vec![FormulaNode::Range {
                sheet: None,
                from: "B4".into(),
                to: "B8".into()
            }]
        );
        let mut w2 = Workbook::new();
        w2.add_sheet();
        assert_eq!(
            w2.formula_tree(0, "='Feuille 2'!A1+toto(1)"),
            Some(FormulaNode::Binary(
                BinOp::Add,
                Box::new(FormulaNode::Cell {
                    sheet: Some("'Feuille 2'".into()),
                    cell: "A1".into()
                }),
                Box::new(FormulaNode::Call {
                    name: "toto".into(),
                    info: None,
                    args: vec![FormulaNode::Number(1.0)]
                })
            ))
        );
        assert_eq!(w.formula_tree(0, "12"), None);
        assert_eq!(w.formula_tree(0, "=1+"), None);
        assert_eq!(
            w.formula_tree(0, "=-A1%"),
            Some(FormulaNode::Neg(Box::new(FormulaNode::Percent(Box::new(
                FormulaNode::Cell {
                    sheet: None,
                    cell: "A1".into()
                }
            )))))
        );
    }

    #[test]
    fn division_par_une_cellule_vide() {
        let w = wb(&[("A1", "10"), ("A2", "=A1/B4"), ("A3", "=A1/0"), ("C1", "0")]);
        assert_eq!(
            w.error_cause(0, at("A2")),
            Some(ErrorCause::DivZero {
                cell: Some("B4".into()),
                empty: true
            })
        );
        assert_eq!(
            w.error_cause(0, at("A3")),
            Some(ErrorCause::DivZero {
                cell: None,
                empty: false
            })
        );
        assert_eq!(
            w.formula_error_cause(0, at("D1"), "=A1/C1"),
            Some(ErrorCause::DivZero {
                cell: Some("C1".into()),
                empty: false
            })
        );
        assert_eq!(w.error_cause(0, at("A1")), None);
    }

    #[test]
    fn autres_causes() {
        let w = wb(&[
            ("A1", "abc"),
            ("A2", "=A1*2"),
            ("A3", "=SOMME(A2;1)"),
            ("A4", "=SOMME(B1:B5)"),
            ("B3", "=1/0"),
            ("A5", "=TRUC(1)"),
            ("A6", "=RECHERCHEV(\"zz\";A1:A2;1;FAUX)"),
            ("A7", "=A7+1"),
            ("A8", "=MOYENNE(C1:C3)"),
            ("A9", "=1+"),
            ("A10", "=RACINE(-1)"),
            ("A11", "=SI(1>0;A1+1;0)"),
        ]);
        assert_eq!(
            w.error_cause(0, at("A2")),
            Some(ErrorCause::Text {
                cell: Some("A1".into()),
                text: "abc".into()
            })
        );
        assert_eq!(
            w.error_cause(0, at("A3")),
            Some(ErrorCause::FromCell {
                cell: "A2".into(),
                error: CalcError::Value
            })
        );
        assert_eq!(
            w.error_cause(0, at("A4")),
            Some(ErrorCause::FromCell {
                cell: "B3".into(),
                error: CalcError::Div0
            })
        );
        assert_eq!(
            w.error_cause(0, at("A5")),
            Some(ErrorCause::UnknownName("TRUC".into()))
        );
        assert_eq!(
            w.error_cause(0, at("A6")),
            Some(ErrorCause::NotFound(Some("zz".into())))
        );
        assert_eq!(w.error_cause(0, at("A7")), Some(ErrorCause::Circular));
        assert_eq!(w.error_cause(0, at("A8")), Some(ErrorCause::EmptyAverage));
        assert!(matches!(
            w.error_cause(0, at("A9")),
            Some(ErrorCause::Syntax(_))
        ));
        assert_eq!(w.error_cause(0, at("A10")), Some(ErrorCause::Num));
        assert_eq!(
            w.error_cause(0, at("A11")),
            Some(ErrorCause::Text {
                cell: Some("A1".into()),
                text: "abc".into()
            })
        );
    }

    #[test]
    fn evaluation_a_la_demande() {
        let w = wb(&[("A1", "2"), ("A2", "3")]);
        assert_eq!(
            w.evaluate(0, at("B1"), "=SOMME(A1:A2)*2"),
            Value::Number(10.0)
        );
        assert_eq!(
            w.evaluate(0, at("B1"), "=1+"),
            Value::Error(CalcError::Name)
        );
        assert_eq!(w.evaluate(0, at("B1"), "5"), Value::Number(5.0));
        // Rien n'a changé dans le classeur.
        assert_eq!(w.input(0, at("B1")), "");
    }
}
