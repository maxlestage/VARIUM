//! Import OpenDocument Spreadsheet (`.ods`).
//!
//! Sont lus : les feuilles, les valeurs (`office:value-type`), les formules
//! OpenFormula (traduites en saisie Varium), les styles de cellule (formats de
//! nombres approchés, gras, italique, couleurs, fond, alignement), les
//! largeurs de colonnes, les hauteurs de lignes, la mise en forme
//! conditionnelle (`calcext:conditional-formats`, comme LibreOffice l'écrit)
//! et les volets figés (`settings.xml`). Un fichier abîmé donne une erreur en
//! français, jamais un plantage.

use std::collections::HashMap;

use varium_core::zip::ZipReader;

use crate::cond::{CompareOp, CondFormat, CondRule, CondStyle};
use crate::format::{CellFormat, HAlign, NumberFormat, format_general};
use crate::import::{
    Filler, MAX_IMPORT_CELLS, bool_input, error_input, expression_rule, finish, iso_date,
    iso_duration, length_px, number_input, parse_rgb, text_input, unique_sheet_names,
};
use crate::refs::{CellRef, MAX_COLS, MAX_ROWS, Range};
use crate::rewrite::from_odf_formula;
use crate::workbook::{DEFAULT_COL_WIDTH, Sheet, Workbook};
use crate::xmltree::{Child, Node, parse};

fn err(msg: impl std::fmt::Display) -> String {
    format!("Fichier OpenDocument illisible : {msg}")
}

/// Style (automatique ou nommé), avant résolution des parents.
#[derive(Clone, Debug, Default)]
struct RawStyle {
    parent: Option<String>,
    data_style: Option<String>,
    fill: Option<Option<[u8; 3]>>,
    color: Option<[u8; 3]>,
    bold: Option<bool>,
    italic: Option<bool>,
    align: Option<Option<HAlign>>,
    wrap: Option<bool>,
    col_width: Option<f32>,
    row_height: Option<f32>,
    optimal_height: bool,
}

#[derive(Default)]
struct Styles {
    styles: HashMap<String, RawStyle>,
    data: HashMap<String, NumberFormat>,
}

fn data_style(n: &Node) -> Option<NumberFormat> {
    let decimals = || {
        n.child("number:number")
            .and_then(|x| x.attr("number:decimal-places"))
            .and_then(|d| d.trim().parse::<u8>().ok())
            .unwrap_or(0)
            .min(15)
    };
    Some(match n.name.as_str() {
        "number:number-style" => {
            let num = n.child("number:number");
            let grouping = num.and_then(|x| x.attr("number:grouping")) == Some("true");
            let d = decimals();
            let money = n
                .children_named("number:text")
                .any(|t| t.text().contains(['€', '$', '£']));
            if num.is_none() || n.child("number:scientific-number").is_some() {
                NumberFormat::General
            } else if money {
                NumberFormat::Currency(d)
            } else if grouping || d > 0 {
                NumberFormat::Number(d)
            } else {
                NumberFormat::General
            }
        }
        "number:currency-style" => NumberFormat::Currency(decimals()),
        "number:percentage-style" => NumberFormat::Percent(decimals()),
        "number:date-style" => NumberFormat::Date,
        "number:text-style" => NumberFormat::Text,
        "number:time-style" | "number:boolean-style" => NumberFormat::General,
        _ => return None,
    })
}

fn read_styles_into(container: &Node, out: &mut Styles) {
    for n in container.elems() {
        if let Some(f) = data_style(n) {
            if let Some(name) = n.attr("style:name") {
                out.data.insert(name.to_string(), f);
            }
            continue;
        }
        if !n.is("style:style") {
            continue;
        }
        let Some(name) = n.attr("style:name") else {
            continue;
        };
        let mut s = RawStyle {
            parent: n.attr("style:parent-style-name").map(str::to_string),
            data_style: n.attr("style:data-style-name").map(str::to_string),
            ..RawStyle::default()
        };
        if let Some(p) = n.child("style:table-cell-properties") {
            if let Some(bg) = p.attr("fo:background-color") {
                s.fill = Some(parse_rgb(bg));
            }
            if let Some(w) = p.attr("fo:wrap-option") {
                s.wrap = Some(w == "wrap");
            }
        }
        if let Some(p) = n.child("style:paragraph-properties")
            && let Some(a) = p.attr("fo:text-align")
        {
            s.align = Some(match a {
                "start" | "left" => Some(HAlign::Left),
                "center" => Some(HAlign::Center),
                "end" | "right" => Some(HAlign::Right),
                _ => None,
            });
        }
        if let Some(t) = n.child("style:text-properties") {
            if let Some(w) = t.attr("fo:font-weight") {
                s.bold = Some(w == "bold" || w.parse::<u32>().is_ok_and(|v| v >= 600));
            }
            if let Some(i) = t.attr("fo:font-style") {
                s.italic = Some(i == "italic" || i == "oblique");
            }
            s.color = t.attr("fo:color").and_then(parse_rgb);
        }
        if let Some(c) = n.child("style:table-column-properties") {
            s.col_width = c.attr("style:column-width").and_then(length_px);
        }
        if let Some(r) = n.child("style:table-row-properties") {
            s.row_height = r.attr("style:row-height").and_then(length_px);
            s.optimal_height = r.attr("style:use-optimal-row-height") == Some("true");
        }
        if let Some(d) = n.attr("style:display-name")
            && d != name
        {
            out.styles.entry(d.to_string()).or_insert_with(|| s.clone());
        }
        out.styles.insert(name.to_string(), s);
    }
}

impl Styles {
    /// Mise en forme d'un style de cellule, parents compris.
    fn cell_format(&self, name: &str) -> CellFormat {
        let mut chain = Vec::new();
        let mut cur = Some(name.to_string());
        while let Some(n) = cur {
            if chain.len() > 16 {
                break;
            }
            let Some(s) = self.styles.get(&n) else {
                break;
            };
            chain.push(s);
            cur = s.parent.clone();
        }
        let mut f = CellFormat::default();
        for s in chain.iter().rev() {
            if let Some(d) = s.data_style.as_ref().and_then(|d| self.data.get(d)) {
                f.number = *d;
            }
            if let Some(fill) = s.fill {
                f.fill = fill;
            }
            if let Some(c) = s.color {
                f.color = Some(c).filter(|c| *c != [0, 0, 0]);
            }
            if let Some(b) = s.bold {
                f.bold = b;
            }
            if let Some(i) = s.italic {
                f.italic = i;
            }
            if let Some(a) = s.align {
                f.align = a;
            }
            if let Some(w) = s.wrap {
                f.wrap = w;
            }
        }
        f
    }

    fn cond_style(&self, name: &str) -> CondStyle {
        let f = self.cell_format(name);
        CondStyle {
            fill: f.fill,
            color: f.color,
            bold: f.bold,
            italic: f.italic,
        }
    }
}

/// Texte d'une cellule : paragraphes `<text:p>` séparés par des sauts de ligne.
fn cell_text(cell: &Node) -> String {
    let mut paras = Vec::new();
    for p in cell.elems().filter(|n| n.is("text:p") || n.is("text:h")) {
        let mut s = String::new();
        para_text(p, &mut s, 0);
        paras.push(s);
    }
    paras.join("\n")
}

fn para_text(n: &Node, out: &mut String, depth: usize) {
    if depth > 32 {
        return;
    }
    for c in &n.children {
        match c {
            Child::Text(t) => out.push_str(t),
            Child::Elem(e) => match e.name.as_str() {
                "text:s" => {
                    let k: usize = e
                        .attr("text:c")
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(1)
                        .min(1000);
                    out.extend(std::iter::repeat_n(' ', k));
                }
                "text:tab" => out.push('\t'),
                "text:line-break" => out.push('\n'),
                "office:annotation" | "text:note" => {}
                _ => para_text(e, out, depth + 1),
            },
        }
    }
}

fn repeat(n: &Node, attr: &str) -> u32 {
    n.attr(attr)
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(1)
        .clamp(1, MAX_ROWS)
}

/// Saisie et format d'une cellule ODS.
fn cell_input(cell: &Node, mut format: CellFormat) -> Option<(String, CellFormat)> {
    let kind = cell
        .attr("office:value-type")
        .or_else(|| cell.attr("calcext:value-type"));
    if let Some(f) = cell.attr("table:formula") {
        return Some((from_odf_formula(f), format));
    }
    let input = match kind {
        Some("float") => {
            let n = cell.attr("office:value")?.trim().parse::<f64>().ok()?;
            number_input(n, format.number)
        }
        Some("percentage") => {
            let n = cell.attr("office:value")?.trim().parse::<f64>().ok()?;
            if !matches!(format.number, NumberFormat::Percent(_)) {
                format.number = NumberFormat::Percent(0);
            }
            number_input(n, format.number)
        }
        Some("currency") => {
            let n = cell.attr("office:value")?.trim().parse::<f64>().ok()?;
            if !matches!(format.number, NumberFormat::Currency(_)) {
                format.number = NumberFormat::Currency(2);
            }
            number_input(n, format.number)
        }
        Some("date") => {
            let n = iso_date(cell.attr("office:date-value")?)?;
            if format.number == NumberFormat::General || n.fract() == 0.0 {
                format.number = NumberFormat::Date;
            }
            number_input(n, format.number)
        }
        Some("time") => {
            let n = iso_duration(cell.attr("office:time-value")?)?;
            number_input(n, format.number)
        }
        Some("boolean") => bool_input(cell.attr("office:boolean-value")? == "true"),
        Some("error") => error_input(&cell_text(cell)),
        Some("string") => {
            let t = match cell.attr("office:string-value") {
                Some(v) => v.to_string(),
                None => cell_text(cell),
            };
            text_input(&t)
        }
        _ => {
            // Sans type : texte éventuel (certains producteurs l'omettent).
            let t = cell_text(cell);
            if t.is_empty() {
                String::new()
            } else {
                text_input(&t)
            }
        }
    };
    Some((input, format))
}

/// Lignes d'un tableau, y compris dans les groupes et en-têtes.
fn collect_rows<'a>(n: &'a Node, out: &mut Vec<&'a Node>, depth: usize) {
    if depth > 16 {
        return;
    }
    for c in n.elems() {
        match c.name.as_str() {
            "table:table-row" => out.push(c),
            "table:table-row-group" | "table:table-header-rows" | "table:table-rows" => {
                collect_rows(c, out, depth + 1)
            }
            _ => {}
        }
    }
}

fn collect_cols<'a>(n: &'a Node, out: &mut Vec<&'a Node>, depth: usize) {
    if depth > 16 {
        return;
    }
    for c in n.elems() {
        match c.name.as_str() {
            "table:table-column" => out.push(c),
            "table:table-column-group" | "table:table-header-columns" | "table:table-columns" => {
                collect_cols(c, out, depth + 1)
            }
            _ => {}
        }
    }
}

/// Adresse ODF (`Feuille.A1`, `'Feuille 2'.$A$1`, `$Feuille.A1`) → cellule.
fn odf_cell(s: &str) -> Option<CellRef> {
    let s = s.trim();
    let mut split = None;
    let mut q = false;
    for (i, c) in s.char_indices() {
        match c {
            '\'' => q = !q,
            '.' if !q => split = Some(i),
            _ => {}
        }
    }
    let addr = match split {
        Some(i) => &s[i + 1..],
        None => s,
    };
    CellRef::parse(&addr.replace('$', ""))
}

/// Plages d'une adresse cible (`F.A1:F.B5 F.D1:F.D3`).
fn odf_ranges(s: &str) -> Vec<Range> {
    let mut out = Vec::new();
    // Les noms entre apostrophes peuvent contenir des espaces.
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut q = false;
    for c in s.chars() {
        match c {
            '\'' => {
                q = !q;
                cur.push(c);
            }
            ' ' if !q => parts.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    parts.push(cur);
    for p in parts.iter().filter(|p| !p.trim().is_empty()) {
        let mut q = false;
        let mut colon = None;
        for (i, c) in p.char_indices() {
            match c {
                '\'' => q = !q,
                ':' if !q => colon = Some(i),
                _ => {}
            }
        }
        let r = match colon {
            Some(i) => odf_cell(&p[..i])
                .zip(odf_cell(&p[i + 1..]))
                .map(|(a, b)| Range::new(a, b)),
            None => odf_cell(p).map(Range::cell),
        };
        out.extend(r);
    }
    out
}

/// Arguments entre parenthèses d'une condition : `between(1,5)` → `["1", "5"]`.
fn cond_args(s: &str) -> Vec<String> {
    let inner = s
        .find('(')
        .and_then(|i| s.rfind(')').filter(|j| *j > i).map(|j| &s[i + 1..j]))
        .unwrap_or("");
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut depth = 0;
    for c in inner.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                cur.push(c);
            }
            '(' if !quoted => {
                depth += 1;
                cur.push(c);
            }
            ')' if !quoted => {
                depth -= 1;
                cur.push(c);
            }
            ',' | ';' if !quoted && depth == 0 => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// Constante d'une condition (`10`, `"abc"`) ; `None` pour une formule.
fn cond_constant(s: &str) -> Option<String> {
    let s = s.trim();
    if let Some(t) = s.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        return Some(t.replace("\"\"", "\""));
    }
    let n: f64 = s.parse().ok()?;
    n.is_finite().then(|| format_general(n))
}

fn cond_rule(value: &str) -> Option<CondRule> {
    let v = value.trim();
    let lower = v.to_ascii_lowercase();
    let unquote = |a: &str| cond_constant(a).unwrap_or_else(|| a.trim().to_string());
    let args = || cond_args(v);
    let n_arg = || {
        args()
            .first()
            .and_then(|a| a.trim().parse::<f64>().ok())
            .map(|n| n.clamp(1.0, 1e6) as u32)
            .unwrap_or(10)
    };
    let compare = |op: CompareOp, rest: &str| {
        cond_constant(rest).map(|value| CondRule::Compare {
            op,
            value,
            value2: String::new(),
        })
    };
    if lower.starts_with("between(") || lower.starts_with("not-between(") {
        let a = args();
        let (x, y) = (cond_constant(a.first()?)?, cond_constant(a.get(1)?)?);
        let op = if lower.starts_with("not") {
            CompareOp::NotBetween
        } else {
            CompareOp::Between
        };
        return Some(CondRule::Compare {
            op,
            value: x,
            value2: y,
        });
    }
    if lower.starts_with("contains-text(") {
        return Some(CondRule::Contains {
            text: unquote(args().first()?),
        });
    }
    if lower.starts_with("begins-with(") {
        return Some(CondRule::BeginsWith {
            text: unquote(args().first()?),
        });
    }
    if lower.starts_with("ends-with(") {
        return Some(CondRule::EndsWith {
            text: unquote(args().first()?),
        });
    }
    if lower.starts_with("formula-is(") {
        let inner = v.find('(').map_or("", |i| &v[i + 1..]);
        return expression_rule(inner.strip_suffix(')').unwrap_or(inner));
    }
    match lower.as_str() {
        "is-error" => return Some(CondRule::Error),
        "duplicate" => return Some(CondRule::Duplicate { unique: false }),
        "unique" => return Some(CondRule::Duplicate { unique: true }),
        "above-average" | "above-equal-average" => {
            return Some(CondRule::Average { above: true });
        }
        "below-average" | "below-equal-average" => {
            return Some(CondRule::Average { above: false });
        }
        _ => {}
    }
    for (prefix, bottom, percent) in [
        ("top-elements(", false, false),
        ("bottom-elements(", true, false),
        ("top-percent(", false, true),
        ("bottom-percent(", true, true),
    ] {
        if lower.starts_with(prefix) {
            return Some(CondRule::Top {
                n: n_arg(),
                bottom,
                percent,
            });
        }
    }
    for (prefix, op) in [
        (">=", CompareOp::Ge),
        ("<=", CompareOp::Le),
        ("!=", CompareOp::Ne),
        ("<>", CompareOp::Ne),
        (">", CompareOp::Gt),
        ("<", CompareOp::Lt),
        ("=", CompareOp::Eq),
    ] {
        if let Some(rest) = v.strip_prefix(prefix) {
            return compare(op, rest);
        }
    }
    None
}

fn read_cond_formats(table: &Node, styles: &Styles) -> Vec<CondFormat> {
    let mut out = Vec::new();
    let Some(cfs) = table.child("calcext:conditional-formats") else {
        return out;
    };
    for cf in cfs.children_named("calcext:conditional-format") {
        let ranges = odf_ranges(cf.attr("calcext:target-range-address").unwrap_or(""));
        for entry in cf.elems() {
            let (rule, style) = match entry.name.as_str() {
                "calcext:condition" => {
                    let Some(rule) = entry.attr("calcext:value").and_then(cond_rule) else {
                        continue;
                    };
                    let style = entry
                        .attr("calcext:apply-style-name")
                        .map(|s| styles.cond_style(s))
                        .unwrap_or_default();
                    (rule, style)
                }
                "calcext:color-scale" => {
                    let colors: Vec<[u8; 3]> = entry
                        .children_named("calcext:color-scale-entry")
                        .filter_map(|e| e.attr("calcext:color").and_then(parse_rgb))
                        .collect();
                    let rule = match colors.as_slice() {
                        [a, b] => CondRule::ColorScale {
                            min: *a,
                            mid: None,
                            max: *b,
                        },
                        [a, m, b, ..] => CondRule::ColorScale {
                            min: *a,
                            mid: Some(*m),
                            max: *b,
                        },
                        _ => continue,
                    };
                    (rule, CondStyle::default())
                }
                "calcext:data-bar" => (
                    CondRule::DataBar {
                        color: entry
                            .attr("calcext:positive-color")
                            .and_then(parse_rgb)
                            .unwrap_or([0x63, 0x8E, 0xC6]),
                    },
                    CondStyle::default(),
                ),
                _ => continue,
            };
            for r in &ranges {
                out.push(CondFormat {
                    range: *r,
                    rule: rule.clone(),
                    style,
                });
            }
        }
    }
    out
}

fn read_table(table: &Node, sheet: &mut Sheet, styles: &Styles, count: &mut usize) {
    // Colonnes : largeurs et style de cellule par défaut.
    let mut cols = Vec::new();
    collect_cols(table, &mut cols, 0);
    let mut col_styles: Vec<Option<String>> = Vec::new();
    let mut rest_style: Option<String> = None;
    let mut col = 0u32;
    for c in cols {
        let n = repeat(c, "table:number-columns-repeated").min(MAX_COLS);
        let width = c
            .attr("table:style-name")
            .and_then(|s| styles.styles.get(s))
            .and_then(|s| s.col_width);
        let default = c.attr("table:default-cell-style-name").map(str::to_string);
        if n > 1024 {
            rest_style = default.filter(|d| d != "Default");
            break;
        }
        for _ in 0..n {
            if col >= MAX_COLS {
                break;
            }
            if let Some(w) = width
                && (w - DEFAULT_COL_WIDTH).abs() > 0.5
            {
                sheet.col_widths.insert(col, w.clamp(4.0, 4000.0));
            }
            col_styles.push(default.clone());
            col += 1;
        }
    }
    let style_of_col = |c: u32| -> Option<&str> {
        col_styles
            .get(c as usize)
            .cloned()
            .unwrap_or_else(|| rest_style.clone())
            .and_then(|s| (s != "Default").then_some(s))
            .and_then(|s| styles.styles.get_key_value(&s).map(|(k, _)| k.as_str()))
    };
    let mut fill = Filler { sheet, count };
    let mut rows = Vec::new();
    collect_rows(table, &mut rows, 0);
    let mut row = 0u32;
    for r in rows {
        if row >= MAX_ROWS || *fill.count >= MAX_IMPORT_CELLS {
            break;
        }
        let nrows = repeat(r, "table:number-rows-repeated");
        let height = r
            .attr("table:style-name")
            .and_then(|s| styles.styles.get(s))
            .filter(|s| !s.optimal_height)
            .and_then(|s| s.row_height);
        // Lignes remplies : une seule passe, recopiée si la ligne est répétée.
        let mut cells: Vec<(u32, String, CellFormat)> = Vec::new();
        let mut c = 0u32;
        for cell in r.elems() {
            if c >= MAX_COLS {
                break;
            }
            let ncols = repeat(cell, "table:number-columns-repeated").min(MAX_COLS);
            if cell.is("table:table-cell") {
                let style = cell.attr("table:style-name").or_else(|| style_of_col(c));
                let format = style.map(|s| styles.cell_format(s)).unwrap_or_default();
                if let Some((input, format)) = cell_input(cell, format)
                    && !input.is_empty()
                {
                    for k in 0..ncols.min(MAX_COLS - c) {
                        if cells.len() >= MAX_IMPORT_CELLS {
                            break;
                        }
                        cells.push((c + k, input.clone(), format));
                    }
                }
            }
            c = c.saturating_add(ncols);
        }
        let reps = if cells.is_empty() { 0 } else { nrows };
        for k in 0..reps.min(MAX_ROWS - row) {
            if *fill.count >= MAX_IMPORT_CELLS {
                break;
            }
            for (col, input, format) in &cells {
                // Les formules d'une ligne répétée sont décalées comme une recopie.
                let input = if k > 0 {
                    crate::rewrite::shift_formula(input, k as i32, 0)
                } else {
                    input.clone()
                };
                fill.put(CellRef::new(row + k, *col), input, *format);
            }
        }
        if let Some(h) = height
            && nrows <= 1024
        {
            for k in 0..nrows.min(MAX_ROWS - row) {
                fill.sheet.row_heights.insert(row + k, h.clamp(2.0, 2000.0));
            }
        }
        row = row.saturating_add(nrows);
    }
}

/// Volets figés lus dans `settings.xml` : nom de feuille → (lignes, colonnes).
fn frozen_panes(text: &str) -> HashMap<String, (u32, u32)> {
    let mut out = HashMap::new();
    let Ok(root) = parse(text) else {
        return out;
    };
    let mut stack = vec![&root];
    while let Some(n) = stack.pop() {
        if n.is("config:config-item-map-named") && n.attr("config:name") == Some("Tables") {
            for entry in n.children_named("config:config-item-map-entry") {
                let Some(name) = entry.attr("config:name") else {
                    continue;
                };
                let item = |k: &str| -> u32 {
                    entry
                        .children_named("config:config-item")
                        .find(|i| i.attr("config:name") == Some(k))
                        .and_then(|i| i.text().trim().parse::<u32>().ok())
                        .unwrap_or(0)
                };
                let cols = if item("HorizontalSplitMode") == 2 {
                    item("HorizontalSplitPosition")
                } else {
                    0
                };
                let rows = if item("VerticalSplitMode") == 2 {
                    item("VerticalSplitPosition")
                } else {
                    0
                };
                out.insert(name.to_string(), (rows.min(10_000), cols.min(10_000)));
            }
            continue;
        }
        stack.extend(n.elems());
    }
    out
}

pub(crate) fn from_ods(title: &str, bytes: &[u8]) -> Result<Workbook, String> {
    let zip = ZipReader::new(bytes).map_err(err)?;
    if let Some(mime) = zip.text("mimetype")
        && !mime
            .trim()
            .starts_with("application/vnd.oasis.opendocument.spreadsheet")
    {
        return Err(err(format!(
            "ce n'est pas un classeur ({})",
            mime.trim().chars().take(80).collect::<String>()
        )));
    }
    let content = zip.text("content.xml").ok_or_else(|| {
        err("le contenu (content.xml) est absent ; est-ce bien un fichier .ods ?")
    })?;
    let root = parse(&content).map_err(err)?;
    let mut styles = Styles::default();
    if let Some(text) = zip.text("styles.xml")
        && let Ok(st) = parse(&text)
    {
        for part in ["office:styles", "office:automatic-styles"] {
            if let Some(c) = st.child(part) {
                read_styles_into(c, &mut styles);
            }
        }
    }
    for part in ["office:styles", "office:automatic-styles"] {
        if let Some(c) = root.child(part) {
            read_styles_into(c, &mut styles);
        }
    }
    let spreadsheet = root
        .child("office:body")
        .and_then(|b| b.child("office:spreadsheet"))
        .ok_or_else(|| err("aucun classeur dans le document"))?;
    let tables: Vec<&Node> = spreadsheet.children_named("table:table").collect();
    if tables.is_empty() {
        return Err(err("aucune feuille trouvée"));
    }
    let raw_names: Vec<String> = tables
        .iter()
        .map(|t| t.attr("table:name").unwrap_or("").to_string())
        .collect();
    let names = unique_sheet_names(&raw_names);
    let panes = zip
        .text("settings.xml")
        .map(|t| frozen_panes(&t))
        .unwrap_or_default();
    let mut sheets = Vec::new();
    let mut count = 0usize;
    for ((table, name), raw) in tables.iter().zip(names).zip(&raw_names) {
        let mut sheet = Sheet::new(name);
        read_table(table, &mut sheet, &styles, &mut count);
        sheet.cond_formats = read_cond_formats(table, &styles);
        if let Some(&(rows, cols)) = panes.get(raw) {
            sheet.frozen_rows = rows;
            sheet.frozen_cols = cols;
        }
        sheets.push(sheet);
    }
    Ok(finish(title, sheets))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditions_libreoffice() {
        assert_eq!(
            cond_rule("between(100,400)"),
            Some(CondRule::Compare {
                op: CompareOp::Between,
                value: "100".into(),
                value2: "400".into()
            })
        );
        assert_eq!(
            cond_rule(">=1.5"),
            Some(CondRule::Compare {
                op: CompareOp::Ge,
                value: "1,5".into(),
                value2: String::new()
            })
        );
        assert_eq!(
            cond_rule("=\"Loyer\""),
            Some(CondRule::Compare {
                op: CompareOp::Eq,
                value: "Loyer".into(),
                value2: String::new()
            })
        );
        assert_eq!(
            cond_rule("contains-text(\"a,b\")"),
            Some(CondRule::Contains { text: "a,b".into() })
        );
        assert_eq!(
            cond_rule("bottom-percent(10)"),
            Some(CondRule::Top {
                n: 10,
                bottom: true,
                percent: true
            })
        );
        assert_eq!(
            cond_rule("formula-is(LEN(TRIM([.F1]))=0)"),
            Some(CondRule::Empty)
        );
        assert_eq!(cond_rule("is-error"), Some(CondRule::Error));
        assert_eq!(cond_rule(">[.A1]"), None);
        assert_eq!(
            cond_rule("duplicate"),
            Some(CondRule::Duplicate { unique: false })
        );
    }

    #[test]
    fn adresses() {
        assert_eq!(
            odf_ranges("Budget.B4:Budget.B8"),
            vec![Range::parse("B4:B8").unwrap()]
        );
        assert_eq!(
            odf_ranges("'Feuille 1'.A1:'Feuille 1'.$B$2 F.C3"),
            vec![Range::parse("A1:B2").unwrap(), Range::parse("C3").unwrap()]
        );
    }
}
