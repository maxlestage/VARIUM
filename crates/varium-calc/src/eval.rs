//! Évaluation des formules et recalcul du classeur.
//!
//! Le recalcul analyse toutes les formules, puis parcourt le graphe des
//! dépendances avec l'algorithme de Tarjan (version itérative, sans récursion) :
//! chaque formule est évaluée après celles dont elle dépend, et les cellules
//! prises dans un cycle reçoivent `#CIRC!`. Les successeurs d'une formule sont
//! énumérés à la volée (pas de liste d'arêtes en mémoire), si bien qu'une plage
//! comme `SOMME(A$1:A5000)` ne coûte pas de mémoire supplémentaire.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use crate::convert::RatesTable;
use crate::format::{self, format_general, parse_literal, text_format};
use crate::functions::F;
use crate::lexer::Op;
use crate::parser::{Expr, collect_refs, parse};
use crate::refs::{CellRef, Range};
use crate::value::{CalcError, Value};
use crate::workbook::{Cell, Sheet};

/// Taille maximale d'un texte calculé (en octets) : au-delà, `#VALEUR!`.
pub(crate) const MAX_TEXT: usize = 128 * 1024;

/// Hachage rapide pour les positions de cellules (type FxHash).
#[derive(Default, Clone, Copy)]
pub(crate) struct CellHasher(u64);

impl Hasher for CellHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(u64::from(b));
        }
    }
    fn write_u32(&mut self, i: u32) {
        self.write_u64(u64::from(i));
    }
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

pub(crate) type CellMap = HashMap<CellRef, Value, BuildHasherDefault<CellHasher>>;

/// Résultat d'un recalcul complet.
#[derive(Debug, Default)]
pub(crate) struct Computed {
    /// Valeurs de toutes les cellules non vides, feuille par feuille.
    pub values: Vec<CellMap>,
}

static EMPTY: Value = Value::Empty;

/// La saisie est-elle une formule ?
pub(crate) fn is_formula(input: &str) -> bool {
    input.len() > 1 && input.starts_with('=')
}

/// Valeur d'une cellule qui ne contient pas de formule.
pub(crate) fn constant_value(cell: &Cell) -> Value {
    if cell.input.is_empty() {
        return Value::Empty;
    }
    if cell.format.number == format::NumberFormat::Text {
        let s = cell.input.strip_prefix('\'').unwrap_or(&cell.input);
        return Value::Text(s.to_string());
    }
    parse_literal(&cell.input).0
}

/// Index des feuilles par nom (casse ignorée).
pub(crate) fn sheet_resolver(sheets: &[Sheet]) -> HashMap<String, usize> {
    let mut names = HashMap::new();
    for (i, s) in sheets.iter().enumerate() {
        names.entry(s.name.to_lowercase()).or_insert(i);
    }
    names
}

enum Cursor {
    Start,
    /// Parcours des cases de la plage (petite plage).
    Cells {
        row: u32,
        col: u32,
    },
    /// Parcours des formules de la feuille comprises dans les lignes de la plage.
    Scan {
        pos: usize,
        end: usize,
    },
}

struct Frame {
    node: usize,
    dep: usize,
    cursor: Cursor,
}

impl Frame {
    fn new(node: usize) -> Self {
        Self {
            node,
            dep: 0,
            cursor: Cursor::Start,
        }
    }
}

struct Graph<'a> {
    deps: &'a [Vec<(usize, Range)>],
    /// Positions des formules de chaque feuille, dans l'ordre de lecture.
    fpos: &'a [Vec<CellRef>],
    offsets: &'a [usize],
}

impl Graph<'_> {
    /// Prochaine formule dont dépend le nœud du cadre.
    fn next(&self, f: &mut Frame) -> Option<usize> {
        let deps = self.deps.get(f.node)?;
        loop {
            let &(s, r) = deps.get(f.dep)?;
            let list = &self.fpos[s];
            match f.cursor {
                Cursor::Start => {
                    let lo = list.partition_point(|c| c.row < r.start.row);
                    let hi = list.partition_point(|c| c.row <= r.end.row);
                    // Parcourir la liste coûte une comparaison par formule ;
                    // tester chaque case coûte une recherche dichotomique.
                    f.cursor = if ((hi - lo) as u64) <= r.area().saturating_mul(16) {
                        Cursor::Scan { pos: lo, end: hi }
                    } else {
                        Cursor::Cells {
                            row: r.start.row,
                            col: r.start.col,
                        }
                    };
                }
                Cursor::Scan { pos, end } => {
                    if pos >= end {
                        f.dep += 1;
                        f.cursor = Cursor::Start;
                        continue;
                    }
                    f.cursor = Cursor::Scan { pos: pos + 1, end };
                    if let Some(c) = list.get(pos)
                        && (r.start.col..=r.end.col).contains(&c.col)
                    {
                        return Some(self.offsets[s] + pos);
                    }
                }
                Cursor::Cells { row, col } => {
                    if row > r.end.row {
                        f.dep += 1;
                        f.cursor = Cursor::Start;
                        continue;
                    }
                    f.cursor = if col >= r.end.col {
                        Cursor::Cells {
                            row: row.saturating_add(1),
                            col: r.start.col,
                        }
                    } else {
                        Cursor::Cells { row, col: col + 1 }
                    };
                    if let Ok(i) = list.binary_search(&CellRef::new(row, col)) {
                        return Some(self.offsets[s] + i);
                    }
                }
            }
        }
    }
}

/// Recalcule toutes les cellules du classeur.
pub(crate) fn recalc(sheets: &[Sheet], today: f64, rates: &RatesTable) -> Computed {
    let names = sheet_resolver(sheets);
    let resolve = |n: &str| names.get(&n.to_lowercase()).copied();

    let mut values: Vec<CellMap> = Vec::with_capacity(sheets.len());
    let mut extents = Vec::with_capacity(sheets.len());
    let mut fpos: Vec<Vec<CellRef>> = Vec::with_capacity(sheets.len());
    let mut offsets = Vec::with_capacity(sheets.len());
    let mut nodes: Vec<(usize, CellRef)> = Vec::new();
    let mut exprs: Vec<Result<Expr, CalcError>> = Vec::new();
    let mut deps: Vec<Vec<(usize, Range)>> = Vec::new();

    for (s, sheet) in sheets.iter().enumerate() {
        let mut map = CellMap::default();
        map.reserve(sheet.cells.len());
        let mut list = Vec::new();
        offsets.push(nodes.len());
        for (c, cell) in &sheet.cells {
            if is_formula(&cell.input) {
                list.push(*c);
                nodes.push((s, *c));
                let parsed = parse(&cell.input, s, &resolve).map_err(|_| CalcError::Name);
                let mut d = Vec::new();
                if let Ok(e) = &parsed {
                    collect_refs(e, &mut d);
                }
                exprs.push(parsed);
                deps.push(d);
            } else {
                let v = constant_value(cell);
                if !v.is_empty() {
                    map.insert(*c, v);
                }
            }
        }
        extents.push(sheet.used_size());
        values.push(map);
        fpos.push(list);
    }

    let graph = Graph {
        deps: &deps,
        fpos: &fpos,
        offsets: &offsets,
    };
    let n = nodes.len();
    const UNSEEN: u32 = u32::MAX;
    let mut index = vec![UNSEEN; n];
    let mut low = vec![0u32; n];
    let mut on_stack = vec![false; n];
    let mut self_loop = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut calls: Vec<Frame> = Vec::new();
    let mut counter = 0u32;

    for root in 0..n {
        if index[root] != UNSEEN {
            continue;
        }
        index[root] = counter;
        low[root] = counter;
        counter += 1;
        stack.push(root);
        on_stack[root] = true;
        calls.push(Frame::new(root));
        while let Some(frame) = calls.last_mut() {
            let v = frame.node;
            if let Some(w) = graph.next(frame) {
                if w == v {
                    self_loop[v] = true;
                } else if index[w] == UNSEEN {
                    index[w] = counter;
                    low[w] = counter;
                    counter += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    calls.push(Frame::new(w));
                } else if on_stack[w] {
                    low[v] = low[v].min(index[w]);
                }
                continue;
            }
            calls.pop();
            if let Some(parent) = calls.last() {
                let p = parent.node;
                low[p] = low[p].min(low[v]);
            }
            if low[v] != index[v] {
                continue;
            }
            // Composante fortement connexe complète : on l'évalue.
            let mut scc = Vec::new();
            while let Some(w) = stack.pop() {
                on_stack[w] = false;
                scc.push(w);
                if w == v {
                    break;
                }
            }
            if scc.len() == 1 && !self_loop[v] {
                let (s, at) = nodes[v];
                let result = match &exprs[v] {
                    Ok(e) => {
                        let ctx = Ctx {
                            sheets,
                            values: &values,
                            extents: &extents,
                            at,
                            today,
                            rates,
                        };
                        ctx.result(e)
                    }
                    Err(e) => Value::Error(*e),
                };
                values[s].insert(at, result);
            } else {
                for w in scc {
                    let (s, at) = nodes[w];
                    values[s].insert(at, Value::Error(CalcError::Circular));
                }
            }
        }
    }
    Computed { values }
}

/// Évaluation à la demande d'expressions déjà analysées, avec les valeurs d'un
/// recalcul (assistant de formule, explication des erreurs).
pub(crate) struct Probe<'a> {
    sheets: &'a [Sheet],
    values: &'a [CellMap],
    extents: Vec<(u32, u32)>,
    at: CellRef,
    today: f64,
    rates: &'a RatesTable,
}

impl<'a> Probe<'a> {
    pub(crate) fn new(
        sheets: &'a [Sheet],
        values: &'a [CellMap],
        at: CellRef,
        today: f64,
        rates: &'a RatesTable,
    ) -> Self {
        Self {
            sheets,
            values,
            extents: sheets.iter().map(Sheet::used_size).collect(),
            at,
            today,
            rates,
        }
    }

    fn ctx(&self) -> Ctx<'_> {
        Ctx {
            sheets: self.sheets,
            values: self.values,
            extents: &self.extents,
            at: self.at,
            today: self.today,
            rates: self.rates,
        }
    }

    /// Valeur finale, comme celle d'une cellule.
    pub(crate) fn result(&self, e: &Expr) -> Value {
        self.ctx().result(e)
    }

    /// Valeur d'une sous-expression (une plage est réduite par intersection).
    pub(crate) fn scalar(&self, e: &Expr) -> Value {
        self.ctx().scalar(e)
    }

    /// Valeur d'une cellule.
    pub(crate) fn cell(&self, s: usize, c: CellRef) -> Value {
        self.values
            .get(s)
            .and_then(|m| m.get(&c))
            .cloned()
            .unwrap_or_default()
    }

    /// Première cellule en erreur d'une plage.
    pub(crate) fn first_error(&self, s: usize, r: Range) -> Option<(CellRef, CalcError)> {
        let mut found = None;
        let _ = self.ctx().each_in(s, r, |c, v| {
            if let Value::Error(e) = v {
                found = Some((c, *e));
                return Err(*e);
            }
            Ok(())
        });
        found
    }
}

/// Opération entre deux valeurs (`+`, `/`, `&`…).
pub(crate) fn apply_op(op: Op, a: &Value, b: &Value) -> Value {
    binop(op, a, b)
}

// ---------------------------------------------------------------- Évaluation

/// Résultat intermédiaire : une valeur ou une référence.
#[derive(Clone, Debug)]
enum Val {
    V(Value),
    R(usize, Range),
}

type Res<T> = Result<T, CalcError>;

/// Contexte d'évaluation d'une formule.
struct Ctx<'a> {
    sheets: &'a [Sheet],
    values: &'a [CellMap],
    extents: &'a [(u32, u32)],
    at: CellRef,
    today: f64,
    rates: &'a RatesTable,
}

fn num_result(n: f64) -> Value {
    if n.is_finite() {
        Value::Number(if n == 0.0 { 0.0 } else { n })
    } else {
        Value::Error(CalcError::Num)
    }
}

fn text_result(s: String) -> Value {
    if s.len() > MAX_TEXT {
        Value::Error(CalcError::Value)
    } else {
        Value::Text(s)
    }
}

/// Conversion en nombre (le texte numérique est accepté, sinon `#VALEUR!`).
pub(crate) fn to_num(v: &Value) -> Res<f64> {
    match v {
        Value::Number(n) => Ok(*n),
        Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        Value::Empty => Ok(0.0),
        Value::Text(s) => match parse_literal(s.trim()).0 {
            Value::Number(n) => Ok(n),
            _ => Err(CalcError::Value),
        },
        Value::Error(e) => Err(*e),
    }
}

pub(crate) fn to_text(v: &Value) -> Res<String> {
    match v {
        Value::Number(n) => Ok(format_general(*n)),
        Value::Bool(b) => Ok(format::bool_text(*b).into()),
        Value::Empty => Ok(String::new()),
        Value::Text(s) => Ok(s.clone()),
        Value::Error(e) => Err(*e),
    }
}

fn to_bool(v: &Value) -> Res<bool> {
    match v {
        Value::Number(n) => Ok(*n != 0.0),
        Value::Bool(b) => Ok(*b),
        Value::Empty => Ok(false),
        Value::Text(s) => match s.trim().to_uppercase().as_str() {
            "VRAI" | "TRUE" => Ok(true),
            "FAUX" | "FALSE" => Ok(false),
            _ => Err(CalcError::Value),
        },
        Value::Error(e) => Err(*e),
    }
}

/// Égalité « à 15 chiffres » des tableurs (0,1 + 0,2 = 0,3).
fn approx_eq(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= a.abs().max(b.abs()) * 1e-15
}

fn cmp_num(a: f64, b: f64) -> Ordering {
    if approx_eq(a, b) {
        Ordering::Equal
    } else {
        a.partial_cmp(&b).unwrap_or(Ordering::Equal)
    }
}

fn cmp_text(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

fn type_rank(v: &Value) -> u8 {
    match v {
        Value::Number(_) | Value::Empty => 0,
        Value::Text(_) => 1,
        Value::Bool(_) => 2,
        Value::Error(_) => 3,
    }
}

/// Comparaison des tableurs : nombres < textes < valeurs logiques ; une cellule
/// vide vaut 0, "" ou FAUX selon l'autre opérande ; texte sans casse.
fn compare(a: &Value, b: &Value) -> Res<Ordering> {
    use Value::*;
    Ok(match (a, b) {
        (Error(e), _) | (_, Error(e)) => return Err(*e),
        (Empty, Empty) => Ordering::Equal,
        (Number(x), Number(y)) => cmp_num(*x, *y),
        (Empty, Number(y)) => cmp_num(0.0, *y),
        (Number(x), Empty) => cmp_num(*x, 0.0),
        (Text(x), Text(y)) => cmp_text(x, y),
        (Empty, Text(y)) => cmp_text("", y),
        (Text(x), Empty) => cmp_text(x, ""),
        (Bool(x), Bool(y)) => x.cmp(y),
        (Empty, Bool(y)) => false.cmp(y),
        (Bool(x), Empty) => x.cmp(&false),
        _ => type_rank(a).cmp(&type_rank(b)),
    })
}

fn wildcard_match(pattern: &[char], text: &[char]) -> bool {
    // Parcours glouton avec retour sur la dernière étoile.
    let (mut p, mut t) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        match pattern.get(p) {
            Some('~') if pattern.get(p + 1) == Some(&text[t]) => {
                p += 2;
                t += 1;
            }
            Some('*') => {
                star = Some((p, t));
                p += 1;
            }
            Some('?') => {
                p += 1;
                t += 1;
            }
            Some(&c) if c != '~' && c == text[t] => {
                p += 1;
                t += 1;
            }
            _ => match star {
                Some((sp, st)) => {
                    p = sp + 1;
                    t = st + 1;
                    star = Some((sp, st + 1));
                }
                None => return false,
            },
        }
    }
    while pattern.get(p) == Some(&'*') {
        p += 1;
    }
    p == pattern.len()
}

fn lower_chars(s: &str) -> Vec<char> {
    s.chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect()
}

/// Égalité de recherche (RECHERCHEV, EQUIV) : textes sans casse avec jokers.
fn lookup_eq(needle: &Value, v: &Value) -> bool {
    match (needle, v) {
        (Value::Text(n), Value::Text(t)) => {
            if n.contains(['*', '?']) {
                wildcard_match(&lower_chars(n), &lower_chars(t))
            } else {
                n.to_lowercase() == t.to_lowercase()
            }
        }
        (Value::Number(a), Value::Number(b)) => approx_eq(*a, *b),
        (Value::Bool(a), Value::Bool(b)) => a == b,
        _ => false,
    }
}

/// Critère de SOMME.SI, NB.SI… : `">10"`, `"<=5"`, `"<>x"`, `"texte"`, `5`.
struct Criterion {
    op: Op,
    target: Target,
}

enum Target {
    Num(f64),
    Text(Vec<char>),
    Bool(bool),
    /// Critère vide (`""`, `"="`, `"<>"`) : cellules vides.
    Blank,
}

impl Criterion {
    fn new(v: &Value) -> Res<Self> {
        Ok(match v {
            Value::Number(n) => Criterion {
                op: Op::Eq,
                target: Target::Num(*n),
            },
            Value::Bool(b) => Criterion {
                op: Op::Eq,
                target: Target::Bool(*b),
            },
            Value::Empty => Criterion {
                op: Op::Eq,
                target: Target::Blank,
            },
            Value::Error(e) => return Err(*e),
            Value::Text(s) => {
                let (op, rest) = [
                    ("<>", Op::Ne),
                    ("<=", Op::Le),
                    (">=", Op::Ge),
                    ("<", Op::Lt),
                    (">", Op::Gt),
                    ("=", Op::Eq),
                ]
                .iter()
                .find_map(|(p, op)| s.strip_prefix(p).map(|r| (*op, r)))
                .unwrap_or((Op::Eq, s.as_str()));
                let target = if rest.is_empty() {
                    Target::Blank
                } else if let Value::Number(n) = parse_literal(rest.trim()).0 {
                    Target::Num(n)
                } else {
                    match rest.trim().to_uppercase().as_str() {
                        "VRAI" | "TRUE" => Target::Bool(true),
                        "FAUX" | "FALSE" => Target::Bool(false),
                        _ => Target::Text(lower_chars(rest)),
                    }
                };
                Criterion { op, target }
            }
        })
    }

    fn test(&self, v: &Value) -> bool {
        let ord_ok = |o: Ordering| match self.op {
            Op::Eq => o == Ordering::Equal,
            Op::Ne => o != Ordering::Equal,
            Op::Lt => o == Ordering::Less,
            Op::Le => o != Ordering::Greater,
            Op::Gt => o == Ordering::Greater,
            Op::Ge => o != Ordering::Less,
            _ => false,
        };
        let blank = matches!(v, Value::Empty) || matches!(v, Value::Text(t) if t.is_empty());
        match &self.target {
            Target::Blank => match self.op {
                Op::Ne => !blank,
                _ => blank,
            },
            Target::Num(x) => match v {
                Value::Number(n) => ord_ok(cmp_num(*n, *x)),
                _ => self.op == Op::Ne,
            },
            Target::Bool(b) => match v {
                Value::Bool(x) => ord_ok(x.cmp(b)),
                _ => self.op == Op::Ne,
            },
            Target::Text(p) => match v {
                Value::Text(t) => match self.op {
                    Op::Eq | Op::Ne => {
                        let eq = wildcard_match(p, &lower_chars(t));
                        (self.op == Op::Eq) == eq
                    }
                    _ => ord_ok(lower_chars(t).cmp(p)),
                },
                _ => self.op == Op::Ne,
            },
        }
    }
}

fn round_with(x: f64, digits: f64, mode: fn(f64) -> f64) -> f64 {
    let d = digits.trunc().clamp(-400.0, 400.0) as i32;
    if d < -308 {
        return 0.0;
    }
    format::round_decimal(x, d, mode)
}

fn round_half_away(v: f64) -> f64 {
    v.round()
}

impl Ctx<'_> {
    fn get(&self, s: usize, c: CellRef) -> &Value {
        self.values.get(s).and_then(|m| m.get(&c)).unwrap_or(&EMPTY)
    }

    /// Valeur finale d'une formule.
    fn result(&self, e: &Expr) -> Value {
        match self.deref(self.eval(e)) {
            Value::Empty => Value::Number(0.0),
            Value::Number(n) => num_result(n),
            v => v,
        }
    }

    /// Valeur d'un résultat ; une plage est réduite par intersection implicite.
    fn deref(&self, v: Val) -> Value {
        match v {
            Val::V(v) => v,
            Val::R(s, r) => {
                if r.start == r.end {
                    return self.get(s, r.start).clone();
                }
                if r.cols() == 1 && (r.start.row..=r.end.row).contains(&self.at.row) {
                    return self.get(s, CellRef::new(self.at.row, r.start.col)).clone();
                }
                if r.rows() == 1 && (r.start.col..=r.end.col).contains(&self.at.col) {
                    return self.get(s, CellRef::new(r.start.row, self.at.col)).clone();
                }
                Value::Error(CalcError::Value)
            }
        }
    }

    fn scalar(&self, e: &Expr) -> Value {
        self.deref(self.eval(e))
    }

    fn num(&self, e: &Expr) -> Res<f64> {
        to_num(&self.scalar(e))
    }

    fn text(&self, e: &Expr) -> Res<String> {
        to_text(&self.scalar(e))
    }

    fn boolean(&self, e: &Expr) -> Res<bool> {
        to_bool(&self.scalar(e))
    }

    fn range(&self, e: &Expr) -> Res<(usize, Range)> {
        match self.eval(e) {
            Val::R(s, r) => Ok((s, r)),
            Val::V(Value::Error(e)) => Err(e),
            Val::V(_) => Err(CalcError::Value),
        }
    }

    /// Argument numérique facultatif.
    fn opt_num(&self, args: &[Expr], i: usize, default: f64) -> Res<f64> {
        match args.get(i) {
            None | Some(Expr::Missing) => Ok(default),
            Some(e) => self.num(e),
        }
    }

    /// Parcourt les cellules non vides d'une plage, ligne par ligne.
    fn each_in(
        &self,
        s: usize,
        r: Range,
        mut f: impl FnMut(CellRef, &Value) -> Res<()>,
    ) -> Res<()> {
        let (Some(map), Some(&(rows, cols)), Some(sheet)) =
            (self.values.get(s), self.extents.get(s), self.sheets.get(s))
        else {
            return Ok(());
        };
        if rows == 0 || cols == 0 || r.start.row >= rows || r.start.col >= cols {
            return Ok(());
        }
        let r1 = r.end.row.min(rows - 1);
        let c1 = r.end.col.min(cols - 1);
        let area = u64::from(r1 - r.start.row + 1) * u64::from(c1 - r.start.col + 1);
        if area <= 2 * map.len() as u64 + 16 {
            for row in r.start.row..=r1 {
                for col in r.start.col..=c1 {
                    let c = CellRef::new(row, col);
                    if let Some(v) = map.get(&c) {
                        f(c, v)?;
                    }
                }
            }
        } else {
            let from = CellRef::new(r.start.row, r.start.col);
            let to = CellRef::new(r1, c1);
            for (c, _) in sheet.cells.range(from..=to) {
                if (r.start.col..=c1).contains(&c.col)
                    && let Some(v) = map.get(c)
                {
                    f(*c, v)?;
                }
            }
        }
        Ok(())
    }

    /// Nombre de lignes et de colonnes à parcourir dans des plages de même forme
    /// (au-delà, toutes les cellules sont vides).
    fn useful_shape(&self, anchors: &[(usize, CellRef)], rows: u32, cols: u32) -> (u32, u32) {
        let mut ur = 0;
        let mut uc = 0;
        for (s, a) in anchors {
            if let Some(&(er, ec)) = self.extents.get(*s) {
                ur = ur.max(er.saturating_sub(a.row));
                uc = uc.max(ec.saturating_sub(a.col));
            }
        }
        (ur.min(rows), uc.min(cols))
    }

    /// Appelle `f` pour chaque valeur des arguments : `true` si elle vient d'une plage.
    fn each_value(&self, args: &[Expr], mut f: impl FnMut(&Value, bool) -> Res<()>) -> Res<()> {
        for a in args {
            match self.eval(a) {
                Val::R(s, r) => self.each_in(s, r, |_, v| f(v, true))?,
                Val::V(v) => f(&v, false)?,
            }
        }
        Ok(())
    }

    /// Nombres des arguments, à la manière de SOMME : dans les plages, texte et
    /// valeurs logiques sont ignorés ; en argument direct, ils sont convertis.
    fn each_number(&self, args: &[Expr], mut f: impl FnMut(f64)) -> Res<()> {
        self.each_value(args, |v, in_range| {
            match v {
                Value::Number(n) => f(*n),
                Value::Error(e) => return Err(*e),
                Value::Empty => {}
                _ if in_range => {}
                other => f(to_num(other)?),
            }
            Ok(())
        })
    }

    fn numbers(&self, args: &[Expr]) -> Res<Vec<f64>> {
        let mut out = Vec::new();
        self.each_number(args, |n| out.push(n))?;
        Ok(out)
    }

    fn eval(&self, e: &Expr) -> Val {
        match e {
            Expr::Num(n) => Val::V(Value::Number(*n)),
            Expr::Str(s) => Val::V(Value::Text(s.clone())),
            Expr::Bool(b) => Val::V(Value::Bool(*b)),
            Expr::Err(e) => Val::V(Value::Error(*e)),
            Expr::Missing => Val::V(Value::Empty),
            Expr::Ref { sheet, range } => Val::R(*sheet, *range),
            Expr::Neg(x) => Val::V(match self.num(x) {
                Ok(n) => num_result(-n),
                Err(e) => Value::Error(e),
            }),
            Expr::ToNum(x) => Val::V(match self.num(x) {
                Ok(n) => num_result(n),
                Err(e) => Value::Error(e),
            }),
            Expr::Percent(x, k) => Val::V(match self.num(x) {
                Ok(n) => num_result(n / 100f64.powi((*k).min(400) as i32)),
                Err(e) => Value::Error(e),
            }),
            Expr::Chain(first, rest) => {
                let mut acc = self.scalar(first);
                for (op, x) in rest {
                    if acc.is_error() {
                        break;
                    }
                    let rhs = self.scalar(x);
                    acc = binop(*op, &acc, &rhs);
                }
                Val::V(acc)
            }
            Expr::Call(f, args) => match f {
                None => Val::V(Value::Error(CalcError::Name)),
                Some(f) => match self.call(*f, args) {
                    Ok(v) => v,
                    Err(e) => Val::V(Value::Error(e)),
                },
            },
        }
    }

    fn call(&self, f: F, args: &[Expr]) -> Res<Val> {
        let arity = |min: usize, max: usize| {
            if (min..=max).contains(&args.len()) {
                Ok(())
            } else {
                Err(CalcError::Value)
            }
        };
        let v = |x: Value| Ok(Val::V(x));
        let n = |x: f64| Ok(Val::V(num_result(x)));
        let a = |i: usize| args.get(i).unwrap_or(&Expr::Missing);
        match f {
            F::Somme => {
                arity(1, 255)?;
                let mut t = 0.0;
                self.each_number(args, |x| t += x)?;
                n(t)
            }
            F::Moyenne => {
                arity(1, 255)?;
                let (mut t, mut c) = (0.0, 0usize);
                self.each_number(args, |x| {
                    t += x;
                    c += 1;
                })?;
                if c == 0 {
                    return Err(CalcError::Div0);
                }
                n(t / c as f64)
            }
            F::Min | F::Max => {
                arity(1, 255)?;
                let mut best: Option<f64> = None;
                self.each_number(args, |x| {
                    best = Some(match best {
                        None => x,
                        Some(b) if f == F::Min => b.min(x),
                        Some(b) => b.max(x),
                    });
                })?;
                n(best.unwrap_or(0.0))
            }
            F::Produit => {
                arity(1, 255)?;
                let mut p = 1.0;
                let mut any = false;
                self.each_number(args, |x| {
                    p *= x;
                    any = true;
                })?;
                n(if any { p } else { 0.0 })
            }
            F::Mediane => {
                arity(1, 255)?;
                let mut xs = self.numbers(args)?;
                if xs.is_empty() {
                    return Err(CalcError::Num);
                }
                xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
                let m = xs.len() / 2;
                n(if xs.len() % 2 == 1 {
                    xs[m]
                } else {
                    (xs[m - 1] + xs[m]) / 2.0
                })
            }
            F::Ecartype => {
                arity(1, 255)?;
                let xs = self.numbers(args)?;
                if xs.len() < 2 {
                    return Err(CalcError::Div0);
                }
                let mean = xs.iter().sum::<f64>() / xs.len() as f64;
                let var =
                    xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (xs.len() - 1) as f64;
                n(var.sqrt())
            }
            F::Nb => {
                arity(1, 255)?;
                let mut c = 0usize;
                self.each_value(args, |x, in_range| {
                    match x {
                        Value::Number(_) => c += 1,
                        Value::Bool(_) | Value::Text(_) if !in_range && to_num(x).is_ok() => {
                            c += 1;
                        }
                        _ => {}
                    }
                    Ok(())
                })?;
                n(c as f64)
            }
            F::NbVal => {
                arity(1, 255)?;
                let mut c = 0usize;
                for arg in args {
                    match self.eval(arg) {
                        Val::R(s, r) => self.each_in(s, r, |_, _| {
                            c += 1;
                            Ok(())
                        })?,
                        Val::V(Value::Empty) => {}
                        Val::V(_) => c += 1,
                    }
                }
                n(c as f64)
            }
            F::NbVide => {
                arity(1, 1)?;
                let (s, r) = self.range(a(0))?;
                let mut filled = 0u64;
                self.each_in(s, r, |_, x| {
                    if !matches!(x, Value::Text(t) if t.is_empty()) {
                        filled += 1;
                    }
                    Ok(())
                })?;
                n((r.area() - filled) as f64)
            }
            F::Si => {
                arity(1, 3)?;
                let cond = self.boolean(a(0))?;
                let branch = if cond { 1 } else { 2 };
                match args.get(branch) {
                    Some(e) => Ok(self.eval(e)),
                    None => v(Value::Bool(cond)),
                }
            }
            F::SiErreur => {
                arity(2, 2)?;
                let first = self.eval(a(0));
                let value = self.deref(first.clone());
                if value.is_error() {
                    Ok(self.eval(a(1)))
                } else {
                    Ok(first)
                }
            }
            F::Et | F::Ou => {
                arity(1, 255)?;
                let mut acc: Option<bool> = None;
                self.each_value(args, |x, in_range| {
                    let b = match x {
                        Value::Error(e) => return Err(*e),
                        Value::Empty => return Ok(()),
                        Value::Text(_) if in_range => return Ok(()),
                        other => to_bool(other)?,
                    };
                    acc = Some(match (acc, f) {
                        (None, _) => b,
                        (Some(p), F::Et) => p && b,
                        (Some(p), _) => p || b,
                    });
                    Ok(())
                })?;
                v(Value::Bool(acc.ok_or(CalcError::Value)?))
            }
            F::Non => {
                arity(1, 1)?;
                v(Value::Bool(!self.boolean(a(0))?))
            }
            F::Vrai | F::Faux => {
                arity(0, 0)?;
                v(Value::Bool(f == F::Vrai))
            }
            F::Arrondi | F::ArrondiSup | F::ArrondiInf => {
                arity(1, 2)?;
                let x = self.num(a(0))?;
                let d = self.opt_num(args, 1, 0.0)?;
                let mode: fn(f64) -> f64 = match f {
                    F::Arrondi => round_half_away,
                    F::ArrondiSup => f64::ceil,
                    _ => f64::floor,
                };
                n(round_with(x, d, mode))
            }
            F::Ent => {
                arity(1, 1)?;
                n(self.num(a(0))?.floor())
            }
            F::Abs => {
                arity(1, 1)?;
                n(self.num(a(0))?.abs())
            }
            F::Signe => {
                arity(1, 1)?;
                let x = self.num(a(0))?;
                n(if x > 0.0 {
                    1.0
                } else if x < 0.0 {
                    -1.0
                } else {
                    0.0
                })
            }
            F::Racine => {
                arity(1, 1)?;
                let x = self.num(a(0))?;
                if x < 0.0 {
                    return Err(CalcError::Num);
                }
                n(x.sqrt())
            }
            F::Exp => {
                arity(1, 1)?;
                n(self.num(a(0))?.exp())
            }
            F::Ln | F::Log10 => {
                arity(1, 1)?;
                let x = self.num(a(0))?;
                if x <= 0.0 {
                    return Err(CalcError::Num);
                }
                n(if f == F::Ln { x.ln() } else { x.log10() })
            }
            F::Puissance => {
                arity(2, 2)?;
                Ok(Val::V(power(self.num(a(0))?, self.num(a(1))?)))
            }
            F::Mod => {
                arity(2, 2)?;
                let x = self.num(a(0))?;
                let d = self.num(a(1))?;
                if d == 0.0 {
                    return Err(CalcError::Div0);
                }
                n(x - d * (x / d).floor())
            }
            F::Pi => {
                arity(0, 0)?;
                n(std::f64::consts::PI)
            }
            F::SommeSi | F::MoyenneSi => {
                arity(2, 3)?;
                let (cs, cr) = self.range(a(0))?;
                let crit = Criterion::new(&self.scalar(a(1)))?;
                let (ss, sr) = match args.get(2) {
                    Some(e) if !matches!(e, Expr::Missing) => self.range(e)?,
                    _ => (cs, cr),
                };
                let (rows, cols) =
                    self.useful_shape(&[(cs, cr.start), (ss, sr.start)], cr.rows(), cr.cols());
                let (mut t, mut c) = (0.0, 0usize);
                for i in 0..rows {
                    for j in 0..cols {
                        let cv = self.get(cs, CellRef::new(cr.start.row + i, cr.start.col + j));
                        if !crit.test(cv) {
                            continue;
                        }
                        let target = CellRef::new(sr.start.row + i, sr.start.col + j);
                        match self.get(ss, target) {
                            Value::Number(x) => {
                                t += x;
                                c += 1;
                            }
                            Value::Error(e) => return Err(*e),
                            _ => {}
                        }
                    }
                }
                if f == F::SommeSi {
                    n(t)
                } else if c == 0 {
                    Err(CalcError::Div0)
                } else {
                    n(t / c as f64)
                }
            }
            F::NbSi => {
                arity(2, 2)?;
                let (s, r) = self.range(a(0))?;
                let crit = Criterion::new(&self.scalar(a(1)))?;
                let (rows, cols) = self.useful_shape(&[(s, r.start)], r.rows(), r.cols());
                let mut c = 0u64;
                for i in 0..rows {
                    for j in 0..cols {
                        let cv = self.get(s, CellRef::new(r.start.row + i, r.start.col + j));
                        if crit.test(cv) {
                            c += 1;
                        }
                    }
                }
                if crit.test(&Value::Empty) {
                    c += r.area() - u64::from(rows) * u64::from(cols);
                }
                n(c as f64)
            }
            F::SommeSiEns | F::NbSiEns => {
                let sum = f == F::SommeSiEns;
                let first = usize::from(sum);
                if args.len() < first + 2 || !(args.len() - first).is_multiple_of(2) {
                    return Err(CalcError::Value);
                }
                let mut pairs = Vec::new();
                let mut anchors = Vec::new();
                let mut shape: Option<(u32, u32)> = None;
                let sum_range = if sum {
                    let (s, r) = self.range(a(0))?;
                    anchors.push((s, r.start));
                    shape = Some((r.rows(), r.cols()));
                    Some((s, r))
                } else {
                    None
                };
                for k in (first..args.len()).step_by(2) {
                    let (s, r) = self.range(a(k))?;
                    let crit = Criterion::new(&self.scalar(a(k + 1)))?;
                    match shape {
                        Some(sh) if sh != (r.rows(), r.cols()) => return Err(CalcError::Value),
                        _ => shape = Some((r.rows(), r.cols())),
                    }
                    anchors.push((s, r.start));
                    pairs.push((s, r, crit));
                }
                let (rows, cols) = shape.unwrap_or((0, 0));
                let (ur, uc) = self.useful_shape(&anchors, rows, cols);
                let (mut t, mut c) = (0.0, 0u64);
                for i in 0..ur {
                    for j in 0..uc {
                        let ok = pairs.iter().all(|(s, r, crit)| {
                            crit.test(self.get(*s, CellRef::new(r.start.row + i, r.start.col + j)))
                        });
                        if !ok {
                            continue;
                        }
                        c += 1;
                        if let Some((s, r)) = sum_range {
                            match self.get(s, CellRef::new(r.start.row + i, r.start.col + j)) {
                                Value::Number(x) => t += x,
                                Value::Error(e) => return Err(*e),
                                _ => {}
                            }
                        }
                    }
                }
                if sum {
                    n(t)
                } else {
                    if pairs.iter().all(|(_, _, crit)| crit.test(&Value::Empty)) {
                        c += u64::from(rows) * u64::from(cols) - u64::from(ur) * u64::from(uc);
                    }
                    n(c as f64)
                }
            }
            F::SommeProd => {
                arity(1, 255)?;
                let mut ranges = Vec::new();
                for e in args {
                    ranges.push(self.range(e)?);
                }
                let (_, r0) = ranges[0];
                if ranges
                    .iter()
                    .any(|(_, r)| (r.rows(), r.cols()) != (r0.rows(), r0.cols()))
                {
                    return Err(CalcError::Value);
                }
                let anchors: Vec<_> = ranges.iter().map(|(s, r)| (*s, r.start)).collect();
                let (ur, uc) = self.useful_shape(&anchors, r0.rows(), r0.cols());
                let mut t = 0.0;
                for i in 0..ur {
                    for j in 0..uc {
                        let mut p = 1.0;
                        for (s, r) in &ranges {
                            match self.get(*s, CellRef::new(r.start.row + i, r.start.col + j)) {
                                Value::Number(x) => p *= x,
                                Value::Error(e) => return Err(*e),
                                _ => p = 0.0,
                            }
                        }
                        t += p;
                    }
                }
                n(t)
            }
            F::RechercheV => {
                arity(3, 4)?;
                let needle = self.scalar(a(0));
                if let Value::Error(e) = needle {
                    return Err(e);
                }
                let (s, r) = self.range(a(1))?;
                let col = self.num(a(2))?.trunc();
                if col < 1.0 {
                    return Err(CalcError::Value);
                }
                if col > f64::from(r.cols()) {
                    return Err(CalcError::Ref);
                }
                let sorted = match args.get(3) {
                    None | Some(Expr::Missing) => true,
                    Some(e) => self.boolean(e)?,
                };
                let (rows, _) = self.useful_shape(&[(s, r.start)], r.rows(), 1);
                let key = |i: u32| self.get(s, CellRef::new(r.start.row + i, r.start.col));
                let found = if sorted {
                    approx_position(&needle, rows, key)
                } else {
                    (0..rows).find(|&i| lookup_eq(&needle, key(i)))
                };
                let i = found.ok_or(CalcError::NA)?;
                let at = CellRef::new(r.start.row + i, r.start.col + col as u32 - 1);
                Ok(Val::R(s, Range::cell(at)))
            }
            F::Index => {
                arity(1, 3)?;
                let (s, r) = match self.eval(a(0)) {
                    Val::R(s, r) => (s, r),
                    Val::V(x) => {
                        let row = self.opt_num(args, 1, 1.0)?;
                        let col = self.opt_num(args, 2, 1.0)?;
                        return if row <= 1.0 && col <= 1.0 {
                            v(x)
                        } else {
                            Err(CalcError::Ref)
                        };
                    }
                };
                let mut row = self.opt_num(args, 1, 0.0)?.trunc();
                let mut col = self.opt_num(args, 2, 0.0)?.trunc();
                if args.len() <= 2 && r.rows() == 1 && r.cols() > 1 {
                    // INDEX(A1:E1; 3) : position dans la ligne.
                    col = row;
                    row = 0.0;
                }
                if row < 0.0 || col < 0.0 || row > f64::from(r.rows()) || col > f64::from(r.cols())
                {
                    return Err(CalcError::Ref);
                }
                let (row, col) = (row as u32, col as u32);
                let (r0, r1) = if row == 0 {
                    (r.start.row, r.end.row)
                } else {
                    (r.start.row + row - 1, r.start.row + row - 1)
                };
                let (c0, c1) = if col == 0 {
                    (r.start.col, r.end.col)
                } else {
                    (r.start.col + col - 1, r.start.col + col - 1)
                };
                Ok(Val::R(
                    s,
                    Range::new(CellRef::new(r0, c0), CellRef::new(r1, c1)),
                ))
            }
            F::Equiv => {
                arity(2, 3)?;
                let needle = self.scalar(a(0));
                if let Value::Error(e) = needle {
                    return Err(e);
                }
                let (s, r) = self.range(a(1))?;
                if r.rows() > 1 && r.cols() > 1 {
                    return Err(CalcError::NA);
                }
                let kind = self.opt_num(args, 2, 1.0)?;
                let len = r.rows().max(r.cols());
                let (ur, uc) = self.useful_shape(&[(s, r.start)], r.rows(), r.cols());
                let useful = if r.rows() > 1 { ur } else { uc };
                let item = |i: u32| {
                    if r.rows() > 1 {
                        self.get(s, CellRef::new(r.start.row + i, r.start.col))
                    } else {
                        self.get(s, CellRef::new(r.start.row, r.start.col + i))
                    }
                };
                let found = if kind == 0.0 {
                    (0..useful.min(len)).find(|&i| lookup_eq(&needle, item(i)))
                } else if kind > 0.0 {
                    approx_position(&needle, useful.min(len), item)
                } else {
                    // Ordre décroissant : la plus petite valeur supérieure ou égale.
                    let mut best = None;
                    for i in 0..useful.min(len) {
                        let x = item(i);
                        if type_rank(x) != type_rank(&needle) || x.is_empty() {
                            continue;
                        }
                        match compare(x, &needle) {
                            Ok(Ordering::Less) => break,
                            Ok(_) => best = Some(i),
                            Err(_) => {}
                        }
                    }
                    best
                };
                n(f64::from(found.ok_or(CalcError::NA)? + 1))
            }
            F::Concat | F::Concatener => {
                let mut out = String::new();
                self.each_value(args, |x, _| {
                    out.push_str(&to_text(x)?);
                    if out.len() > MAX_TEXT {
                        return Err(CalcError::Value);
                    }
                    Ok(())
                })?;
                v(text_result(out))
            }
            F::Gauche | F::Droite => {
                arity(1, 2)?;
                let t = self.text(a(0))?;
                let k = self.opt_num(args, 1, 1.0)?;
                if k < 0.0 {
                    return Err(CalcError::Value);
                }
                let k = k.min(1e9) as usize;
                let len = t.chars().count();
                let s: String = if f == F::Gauche {
                    t.chars().take(k).collect()
                } else {
                    t.chars().skip(len.saturating_sub(k)).collect()
                };
                v(Value::Text(s))
            }
            F::Stxt => {
                arity(3, 3)?;
                let t = self.text(a(0))?;
                let start = self.num(a(1))?;
                let k = self.num(a(2))?;
                if start < 1.0 || k < 0.0 {
                    return Err(CalcError::Value);
                }
                let s: String = t
                    .chars()
                    .skip(start.min(1e9) as usize - 1)
                    .take(k.min(1e9) as usize)
                    .collect();
                v(Value::Text(s))
            }
            F::NbCar => {
                arity(1, 1)?;
                n(self.text(a(0))?.chars().count() as f64)
            }
            F::Majuscule => {
                arity(1, 1)?;
                v(text_result(self.text(a(0))?.to_uppercase()))
            }
            F::Minuscule => {
                arity(1, 1)?;
                v(text_result(self.text(a(0))?.to_lowercase()))
            }
            F::SupprEspace => {
                arity(1, 1)?;
                let t = self.text(a(0))?;
                let s = t.split(' ').filter(|w| !w.is_empty()).collect::<Vec<_>>();
                v(Value::Text(s.join(" ")))
            }
            F::Texte => {
                arity(2, 2)?;
                let x = self.scalar(a(0));
                let pattern = self.text(a(1))?;
                let num = match &x {
                    Value::Error(e) => return Err(*e),
                    Value::Text(t) => match parse_literal(t.trim()).0 {
                        Value::Number(n) => Some(n),
                        _ => None,
                    },
                    other => to_num(other).ok(),
                };
                match num {
                    Some(nb) => v(text_result(text_format(nb, &pattern))),
                    None => v(x),
                }
            }
            F::Trouve | F::Cherche => {
                arity(2, 3)?;
                let needle = self.text(a(0))?;
                let hay = self.text(a(1))?;
                let start = self.opt_num(args, 2, 1.0)?;
                if start < 1.0 {
                    return Err(CalcError::Value);
                }
                let fold = |s: &str| -> Vec<char> {
                    if f == F::Cherche {
                        lower_chars(s)
                    } else {
                        s.chars().collect()
                    }
                };
                let h = fold(&hay);
                let nd = fold(&needle);
                let from = (start.min(1e9) as usize) - 1;
                if from > h.len() {
                    return Err(CalcError::Value);
                }
                if nd.is_empty() {
                    return n((from + 1) as f64);
                }
                let pos = (from..h.len())
                    .find(|&i| h.get(i..i + nd.len()) == Some(&nd[..]))
                    .ok_or(CalcError::Value)?;
                n((pos + 1) as f64)
            }
            F::Substitue => {
                arity(3, 4)?;
                let t = self.text(a(0))?;
                let old = self.text(a(1))?;
                let new = self.text(a(2))?;
                if old.is_empty() {
                    return v(Value::Text(t));
                }
                let out = match args.get(3) {
                    None | Some(Expr::Missing) => {
                        let count = t.matches(&old).count();
                        let grow = count.saturating_mul(new.len());
                        if t.len().saturating_add(grow) > MAX_TEXT {
                            return Err(CalcError::Value);
                        }
                        t.replace(&old, &new)
                    }
                    Some(e) => {
                        let k = self.num(e)?;
                        if k < 1.0 {
                            return Err(CalcError::Value);
                        }
                        match t.match_indices(&old).nth(k.min(1e9) as usize - 1) {
                            Some((i, _)) => {
                                format!("{}{}{}", &t[..i], new, &t[i + old.len()..])
                            }
                            None => t,
                        }
                    }
                };
                v(text_result(out))
            }
            F::Aujourdhui => {
                arity(0, 0)?;
                n(self.today.floor())
            }
            F::Date => {
                arity(3, 3)?;
                let mut y = self.num(a(0))?.trunc();
                let m = self.num(a(1))?.trunc();
                let d = self.num(a(2))?.trunc();
                if (0.0..30.0).contains(&y) {
                    y += 2000.0;
                } else if (30.0..100.0).contains(&y) {
                    y += 1900.0;
                }
                if !(0.0..=9999.0).contains(&y) || m.abs() > 1e6 || d.abs() > 1e8 {
                    return Err(CalcError::Num);
                }
                let serial = format::date_serial(y as i32, m as i32, d as i32);
                if serial < 0.0 {
                    return Err(CalcError::Num);
                }
                n(serial)
            }
            F::Annee | F::Mois | F::Jour => {
                arity(1, 1)?;
                let x = self.num(a(0))?;
                let (y, m, d) = format::serial_to_ymd(x).ok_or(CalcError::Num)?;
                n(match f {
                    F::Annee => f64::from(y),
                    F::Mois => f64::from(m),
                    _ => f64::from(d),
                })
            }
            F::Vpm => {
                arity(3, 5)?;
                let rate = self.num(a(0))?;
                let nper = self.num(a(1))?;
                let pv = self.num(a(2))?;
                let fv = self.opt_num(args, 3, 0.0)?;
                let kind = if self.opt_num(args, 4, 0.0)? != 0.0 {
                    1.0
                } else {
                    0.0
                };
                if nper == 0.0 {
                    return Err(CalcError::Num);
                }
                if rate == 0.0 {
                    return n(-(pv + fv) / nper);
                }
                let g = (1.0 + rate).powf(nper);
                n(-(rate * (fv + pv * g)) / ((1.0 + rate * kind) * (g - 1.0)))
            }
            F::EstVide => {
                arity(1, 1)?;
                v(Value::Bool(self.scalar(a(0)).is_empty()))
            }
            F::EstNum => {
                arity(1, 1)?;
                v(Value::Bool(matches!(self.scalar(a(0)), Value::Number(_))))
            }
            F::EstTexte => {
                arity(1, 1)?;
                v(Value::Bool(matches!(self.scalar(a(0)), Value::Text(_))))
            }
            F::EstErreur => {
                arity(1, 1)?;
                v(Value::Bool(self.scalar(a(0)).is_error()))
            }
            F::Na => {
                arity(0, 0)?;
                Err(CalcError::NA)
            }
            F::Convertir => {
                arity(3, 3)?;
                let x = self.num(a(0))?;
                let from = self.text(a(1))?;
                let to = self.text(a(2))?;
                n(crate::convert::convert(x, &from, &to).ok_or(CalcError::NA)?)
            }
            F::Devise => {
                arity(3, 4)?;
                let x = self.num(a(0))?;
                let from = self.text(a(1))?;
                let to = self.text(a(2))?;
                let date = match a(3) {
                    Expr::Missing => None,
                    e => Some(self.num(e)?),
                };
                n(self
                    .rates
                    .convert(x, &from, &to, date)
                    .ok_or(CalcError::NA)?)
            }
        }
    }
}

/// Recherche approchée dans une liste triée : dernière position dont la valeur
/// (du même type) est inférieure ou égale à celle cherchée.
fn approx_position<'v>(needle: &Value, len: u32, item: impl Fn(u32) -> &'v Value) -> Option<u32> {
    let mut best = None;
    for i in 0..len {
        let x = item(i);
        if x.is_empty() || type_rank(x) != type_rank(needle) {
            continue;
        }
        match compare(x, needle) {
            Ok(Ordering::Greater) => break,
            Ok(_) => best = Some(i),
            Err(_) => {}
        }
    }
    best
}

fn power(x: f64, y: f64) -> Value {
    if x == 0.0 && y == 0.0 {
        return Value::Number(1.0);
    }
    if x == 0.0 && y < 0.0 {
        return Value::Error(CalcError::Div0);
    }
    num_result(x.powf(y))
}

fn binop(op: Op, a: &Value, b: &Value) -> Value {
    if let Value::Error(e) = a {
        return Value::Error(*e);
    }
    let arith = |f: fn(f64, f64) -> Value| match (to_num(a), to_num(b)) {
        (Ok(x), Ok(y)) => f(x, y),
        (Err(e), _) | (_, Err(e)) => Value::Error(e),
    };
    match op {
        Op::Add => arith(|x, y| num_result(x + y)),
        Op::Sub => arith(|x, y| num_result(x - y)),
        Op::Mul => arith(|x, y| num_result(x * y)),
        Op::Div => arith(|x, y| {
            if y == 0.0 {
                Value::Error(CalcError::Div0)
            } else {
                num_result(x / y)
            }
        }),
        Op::Pow => arith(power),
        Op::Concat => match (to_text(a), to_text(b)) {
            (Ok(x), Ok(y)) => {
                if x.len() + y.len() > MAX_TEXT {
                    Value::Error(CalcError::Value)
                } else {
                    Value::Text(x + &y)
                }
            }
            (Err(e), _) | (_, Err(e)) => Value::Error(e),
        },
        Op::Eq | Op::Ne | Op::Lt | Op::Gt | Op::Le | Op::Ge => match compare(a, b) {
            Ok(o) => Value::Bool(match op {
                Op::Eq => o == Ordering::Equal,
                Op::Ne => o != Ordering::Equal,
                Op::Lt => o == Ordering::Less,
                Op::Gt => o == Ordering::Greater,
                Op::Le => o != Ordering::Greater,
                _ => o != Ordering::Less,
            }),
            Err(e) => Value::Error(e),
        },
        Op::Percent => Value::Error(CalcError::Value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jokers() {
        let m = |p: &str, t: &str| wildcard_match(&lower_chars(p), &lower_chars(t));
        assert!(m("a*", "abc"));
        assert!(m("*c", "abc"));
        assert!(m("a?c", "abc"));
        assert!(!m("a?c", "abbc"));
        assert!(m("*", ""));
        assert!(m("a*b*c", "axxbyyc"));
        assert!(!m("a*b*c", "axxbyy"));
        assert!(m("~*", "*"));
        assert!(!m("~*", "a"));
    }

    #[test]
    fn arrondis() {
        assert_eq!(round_with(2.675, 2.0, round_half_away), 2.68);
        assert_eq!(round_with(-2.5, 0.0, round_half_away), -3.0);
        assert_eq!(round_with(1234.0, -2.0, round_half_away), 1200.0);
        assert_eq!(round_with(1.231, 2.0, f64::ceil), 1.24);
        assert_eq!(round_with(-1.239, 2.0, f64::floor), -1.23);
        assert_eq!(round_with(1.5, 400.0, round_half_away), 1.5);
        assert_eq!(round_with(1.5, -400.0, round_half_away), 0.0);
    }

    #[test]
    fn comparaisons() {
        assert_eq!(
            compare(&Value::Text("abc".into()), &Value::Text("ABC".into())),
            Ok(Ordering::Equal)
        );
        assert_eq!(
            compare(&Value::Number(1e9), &Value::Text("a".into())),
            Ok(Ordering::Less)
        );
        assert_eq!(
            compare(&Value::Empty, &Value::Number(0.0)),
            Ok(Ordering::Equal)
        );
        assert_eq!(
            compare(&Value::Number(0.1 + 0.2), &Value::Number(0.3)),
            Ok(Ordering::Equal)
        );
    }
}
