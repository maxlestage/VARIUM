//! Export OpenDocument Spreadsheet (`.ods`).
//!
//! Les formules sont écrites en OpenFormula (`of:=SUM([.A1:.A3])`) avec leur
//! valeur calculée (`office:value-type`, `office:value`…). Le fichier
//! `mimetype` vient en premier, non compressé, comme l'exige la norme.

use std::collections::HashMap;
use std::fmt::Write as _;

use varium_core::zip::Zip;

use crate::cond::{CompareOp, CondRule, CondStyle};
use crate::eval::{CellMap, is_formula};
use crate::format::{
    CellFormat, HAlign, NBSP, NumberFormat, format_value, parse_literal, serial_to_ymd,
};
use crate::refs::{CellRef, MAX_COLS, MAX_ROWS, quote_sheet_name};
use crate::rewrite::to_odf_formula;
use crate::value::Value;
use crate::workbook::{DEFAULT_COL_WIDTH, DEFAULT_ROW_HEIGHT, Sheet, Workbook};
use crate::xlsx::export_sheet_names;
use crate::xml::{esc, esc_text, hex, num};

const HEADER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";
const MIME: &str = "application/vnd.oasis.opendocument.spreadsheet";
const NAMESPACES: &str = concat!(
    " xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\"",
    " xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\"",
    " xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\"",
    " xmlns:table=\"urn:oasis:names:tc:opendocument:xmlns:table:1.0\"",
    " xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\"",
    " xmlns:number=\"urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0\"",
    " xmlns:of=\"urn:oasis:names:tc:opendocument:xmlns:of:1.2\"",
    " xmlns:meta=\"urn:oasis:names:tc:opendocument:xmlns:meta:1.0\"",
    " xmlns:dc=\"http://purl.org/dc/elements/1.1/\"",
    " xmlns:calcext=\"urn:org:documentfoundation:names:experimental:calc:xmlns:calcext:1.0\"",
    " office:version=\"1.3\""
);

/// Points d'écran → centimètres (96 points par pouce).
fn cm(px: f32) -> String {
    format!("{:.3}cm", f64::from(px) * 2.54 / 96.0)
}

#[derive(Default)]
struct Styles {
    data: Vec<NumberFormat>,
    cells: Vec<CellFormat>,
    cell_index: HashMap<CellFormat, usize>,
    cols: Vec<String>,
    rows: Vec<String>,
    /// Styles nommés des règles conditionnelles (`VariumCF1`…), dans styles.xml.
    conds: Vec<CondStyle>,
}

impl Styles {
    fn cell(&mut self, f: &CellFormat) -> Option<String> {
        if f.is_default() {
            return None;
        }
        let i = match self.cell_index.get(f) {
            Some(&i) => i,
            None => {
                let i = self.cells.len();
                self.cells.push(*f);
                self.cell_index.insert(*f, i);
                if f.number != NumberFormat::General && !self.data.contains(&f.number) {
                    self.data.push(f.number);
                }
                i
            }
        };
        Some(format!("ce{}", i + 1))
    }

    fn col(&mut self, width: String) -> String {
        let i = match self.cols.iter().position(|w| *w == width) {
            Some(i) => i,
            None => {
                self.cols.push(width);
                self.cols.len() - 1
            }
        };
        format!("co{}", i + 1)
    }

    fn row(&mut self, height: String) -> String {
        let i = match self.rows.iter().position(|h| *h == height) {
            Some(i) => i,
            None => {
                self.rows.push(height);
                self.rows.len() - 1
            }
        };
        format!("ro{}", i + 1)
    }

    fn cond(&mut self, c: CondStyle) -> String {
        let i = match self.conds.iter().position(|x| *x == c) {
            Some(i) => i,
            None => {
                self.conds.push(c);
                self.conds.len() - 1
            }
        };
        format!("VariumCF{}", i + 1)
    }

    /// Styles nommés des règles conditionnelles.
    fn cond_xml(&self) -> String {
        let mut x = String::new();
        for (i, c) in self.conds.iter().enumerate() {
            let _ = write!(
                x,
                "<style:style style:name=\"VariumCF{}\" style:family=\"table-cell\" style:parent-style-name=\"Default\">",
                i + 1
            );
            if let Some(f) = c.fill {
                let _ = write!(
                    x,
                    "<style:table-cell-properties fo:background-color=\"#{}\"/>",
                    hex(f).to_lowercase()
                );
            }
            if c.bold || c.italic || c.color.is_some() {
                x.push_str("<style:text-properties");
                if let Some(col) = c.color {
                    let _ = write!(x, " fo:color=\"#{}\"", hex(col).to_lowercase());
                }
                if c.bold {
                    x.push_str(" fo:font-weight=\"bold\" style:font-weight-asian=\"bold\" style:font-weight-complex=\"bold\"");
                }
                if c.italic {
                    x.push_str(" fo:font-style=\"italic\" style:font-style-asian=\"italic\" style:font-style-complex=\"italic\"");
                }
                x.push_str("/>");
            }
            x.push_str("</style:style>");
        }
        x
    }

    fn data_name(&self, n: NumberFormat) -> Option<String> {
        self.data
            .iter()
            .position(|d| *d == n)
            .map(|i| format!("N{}", i + 1))
    }

    fn xml(&self) -> String {
        let mut x = String::from("<office:automatic-styles>");
        for (i, n) in self.data.iter().enumerate() {
            let name = format!("N{}", i + 1);
            let number = |d: u8| {
                let d = d.min(15);
                format!(
                    "<number:number number:decimal-places=\"{d}\" number:min-decimal-places=\"{d}\" number:min-integer-digits=\"1\" number:grouping=\"true\"/>"
                )
            };
            match n {
                NumberFormat::Number(d) => {
                    let _ = write!(
                        x,
                        "<number:number-style style:name=\"{name}\">{}</number:number-style>",
                        number(*d)
                    );
                }
                NumberFormat::Currency(d) => {
                    let _ = write!(
                        x,
                        "<number:currency-style style:name=\"{name}\">{}<number:text>{NBSP}</number:text><number:currency-symbol number:language=\"fr\" number:country=\"FR\">€</number:currency-symbol></number:currency-style>",
                        number(*d)
                    );
                }
                NumberFormat::Percent(d) => {
                    let d = (*d).min(15);
                    let _ = write!(
                        x,
                        "<number:percentage-style style:name=\"{name}\"><number:number number:decimal-places=\"{d}\" number:min-decimal-places=\"{d}\" number:min-integer-digits=\"1\"/><number:text>{NBSP}%</number:text></number:percentage-style>"
                    );
                }
                NumberFormat::Date => {
                    let _ = write!(
                        x,
                        "<number:date-style style:name=\"{name}\"><number:day number:style=\"long\"/><number:text>/</number:text><number:month number:style=\"long\"/><number:text>/</number:text><number:year number:style=\"long\"/></number:date-style>"
                    );
                }
                NumberFormat::Text => {
                    let _ = write!(
                        x,
                        "<number:text-style style:name=\"{name}\"><number:text-content/></number:text-style>"
                    );
                }
                NumberFormat::General => {}
            }
        }
        for (i, w) in self.cols.iter().enumerate() {
            let _ = write!(
                x,
                "<style:style style:name=\"co{}\" style:family=\"table-column\"><style:table-column-properties fo:break-before=\"auto\" style:column-width=\"{w}\"/></style:style>",
                i + 1
            );
        }
        for (i, h) in self.rows.iter().enumerate() {
            let _ = write!(
                x,
                "<style:style style:name=\"ro{}\" style:family=\"table-row\"><style:table-row-properties style:row-height=\"{h}\" style:use-optimal-row-height=\"false\"/></style:style>",
                i + 1
            );
        }
        for (i, f) in self.cells.iter().enumerate() {
            let _ = write!(
                x,
                "<style:style style:name=\"ce{}\" style:family=\"table-cell\" style:parent-style-name=\"Default\"",
                i + 1
            );
            if let Some(d) = self.data_name(f.number) {
                let _ = write!(x, " style:data-style-name=\"{d}\"");
            }
            x.push('>');
            if f.fill.is_some() || f.align.is_some() {
                x.push_str("<style:table-cell-properties");
                if let Some(c) = f.fill {
                    let _ = write!(x, " fo:background-color=\"#{}\"", hex(c).to_lowercase());
                }
                if f.align.is_some() {
                    x.push_str(" style:text-align-source=\"fix\"");
                }
                x.push_str("/>");
            }
            if let Some(a) = f.align {
                let v = match a {
                    HAlign::Left => "start",
                    HAlign::Center => "center",
                    HAlign::Right => "end",
                };
                let _ = write!(x, "<style:paragraph-properties fo:text-align=\"{v}\"/>");
            }
            if f.bold || f.italic || f.color.is_some() {
                x.push_str("<style:text-properties");
                if f.bold {
                    x.push_str(" fo:font-weight=\"bold\" style:font-weight-asian=\"bold\" style:font-weight-complex=\"bold\"");
                }
                if f.italic {
                    x.push_str(" fo:font-style=\"italic\" style:font-style-asian=\"italic\" style:font-style-complex=\"italic\"");
                }
                if let Some(c) = f.color {
                    let _ = write!(x, " fo:color=\"#{}\"", hex(c).to_lowercase());
                }
                x.push_str("/>");
            }
            x.push_str("</style:style>");
        }
        x.push_str("</office:automatic-styles>");
        x
    }
}

/// Paragraphe `<text:p>` : espaces multiples, tabulations et sauts de ligne codés.
fn text_p(s: &str) -> String {
    let mut out = String::from("<text:p>");
    let mut spaces = 0usize;
    let flush = |out: &mut String, spaces: &mut usize, at_start: bool| {
        if *spaces == 0 {
            return;
        }
        let (literal, coded) = if at_start {
            (0, *spaces)
        } else {
            (1, *spaces - 1)
        };
        if literal == 1 {
            out.push(' ');
        }
        match coded {
            0 => {}
            1 => out.push_str("<text:s/>"),
            n => {
                let _ = write!(out, "<text:s text:c=\"{n}\"/>");
            }
        }
        *spaces = 0;
    };
    let mut at_start = true;
    for c in s.chars() {
        match c {
            ' ' => spaces += 1,
            '\n' => {
                flush(&mut out, &mut spaces, at_start);
                out.push_str("</text:p><text:p>");
                at_start = true;
                continue;
            }
            '\t' => {
                flush(&mut out, &mut spaces, at_start);
                out.push_str("<text:tab/>");
            }
            '\r' => {}
            c => {
                flush(&mut out, &mut spaces, at_start);
                out.push_str(&esc_text(&c.to_string()));
            }
        }
        if c != ' ' {
            at_start = false;
        }
    }
    flush(&mut out, &mut spaces, at_start);
    out.push_str("</text:p>");
    out
}

/// Attributs de valeur d'une cellule (type et valeur calculée).
fn value_attrs(v: &Value, f: NumberFormat, formula: bool) -> String {
    match v {
        Value::Number(n) => match f {
            NumberFormat::Percent(_) => {
                format!(
                    " office:value-type=\"percentage\" office:value=\"{}\"",
                    num(*n)
                )
            }
            NumberFormat::Currency(_) => format!(
                " office:value-type=\"currency\" office:currency=\"EUR\" office:value=\"{}\"",
                num(*n)
            ),
            NumberFormat::Date => match serial_to_ymd(*n) {
                Some((y, m, d)) if (1..=9999).contains(&y) => {
                    let frac = n - n.floor();
                    if frac > 0.0 {
                        let secs = (frac * 86_400.0).round() as u32;
                        format!(
                            " office:value-type=\"date\" office:date-value=\"{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}\"",
                            (secs / 3600).min(23),
                            secs / 60 % 60,
                            secs % 60
                        )
                    } else {
                        format!(
                            " office:value-type=\"date\" office:date-value=\"{y:04}-{m:02}-{d:02}\""
                        )
                    }
                }
                _ => format!(" office:value-type=\"float\" office:value=\"{}\"", num(*n)),
            },
            _ => format!(" office:value-type=\"float\" office:value=\"{}\"", num(*n)),
        },
        Value::Bool(b) => format!(" office:value-type=\"boolean\" office:boolean-value=\"{b}\""),
        Value::Text(t) if formula => format!(
            " office:value-type=\"string\" office:string-value=\"{}\"",
            esc(t)
        ),
        Value::Text(_) => " office:value-type=\"string\"".into(),
        Value::Error(e) => format!(
            " office:value-type=\"string\" office:string-value=\"{}\"",
            e.english_code()
        ),
        Value::Empty => String::new(),
    }
}

fn table_xml(sheet: &Sheet, name: &str, values: Option<&CellMap>, styles: &mut Styles) -> String {
    let mut x = String::new();
    let _ = write!(x, "<table:table table:name=\"{}\">", esc(name));
    let widths = sheet.clean_col_widths();
    let heights = sheet.clean_row_heights();
    // Colonnes : jusqu'à la dernière cellule (contenu ou mise en forme) ou largeur.
    let last_col = sheet
        .cells()
        .map(|(c, _)| c.col + 1)
        .chain(widths.keys().map(|c| c + 1))
        .max()
        .unwrap_or(1);
    let default_col = styles.col(cm(DEFAULT_COL_WIDTH));
    let mut col = 0;
    while col < last_col {
        let style = match widths.get(&col) {
            Some(w) => styles.col(cm(*w)),
            None => default_col.clone(),
        };
        let mut run = 1;
        while col + run < last_col && widths.get(&(col + run)) == widths.get(&col) {
            run += 1;
        }
        let _ = write!(x, "<table:table-column table:style-name=\"{style}\"");
        if run > 1 {
            let _ = write!(x, " table:number-columns-repeated=\"{run}\"");
        }
        x.push_str(" table:default-cell-style-name=\"Default\"/>");
        col += run;
    }
    let last_row = sheet
        .cells()
        .map(|(c, _)| c.row + 1)
        .chain(heights.keys().map(|r| r + 1))
        .max()
        .unwrap_or(0);
    let default_row = styles.row(cm(DEFAULT_ROW_HEIGHT));
    let mut row = 0;
    while row < last_row {
        let from = CellRef::new(row, 0);
        let to = CellRef::new(row, u32::MAX);
        let mut cells = sheet.cells.range(from..=to).peekable();
        let style = match heights.get(&row) {
            Some(h) => styles.row(cm(*h)),
            None => default_row.clone(),
        };
        if cells.peek().is_none() {
            // Suite de lignes vides de même hauteur.
            let mut run = 1;
            while row + run < last_row
                && heights.get(&(row + run)) == heights.get(&row)
                && sheet
                    .cells
                    .range(CellRef::new(row + run, 0)..=CellRef::new(row + run, u32::MAX))
                    .next()
                    .is_none()
            {
                run += 1;
            }
            let _ = write!(x, "<table:table-row table:style-name=\"{style}\"");
            if run > 1 {
                let _ = write!(x, " table:number-rows-repeated=\"{run}\"");
            }
            x.push_str("><table:table-cell/></table:table-row>");
            row += run;
            continue;
        }
        let _ = write!(x, "<table:table-row table:style-name=\"{style}\">");
        let mut next_col = 0;
        for (&at, cell) in cells {
            if at.col > next_col {
                let gap = at.col - next_col;
                if gap > 1 {
                    let _ = write!(
                        x,
                        "<table:table-cell table:number-columns-repeated=\"{gap}\"/>"
                    );
                } else {
                    x.push_str("<table:table-cell/>");
                }
            }
            next_col = at.col + 1;
            x.push_str("<table:table-cell");
            if let Some(s) = styles.cell(&cell.format) {
                let _ = write!(x, " table:style-name=\"{s}\"");
            }
            let formula = is_formula(&cell.input);
            if formula {
                let _ = write!(
                    x,
                    " table:formula=\"{}\"",
                    esc(&to_odf_formula(&cell.input))
                );
            }
            let value = values.and_then(|m| m.get(&at)).cloned().unwrap_or_default();
            x.push_str(&value_attrs(&value, cell.format.number, formula));
            if value.is_empty() {
                x.push_str("/>");
            } else {
                x.push('>');
                x.push_str(&text_p(&format_value(&value, cell.format.number)));
                x.push_str("</table:table-cell>");
            }
        }
        x.push_str("</table:table-row>");
        row += 1;
    }
    if last_row == 0 {
        x.push_str("<table:table-row><table:table-cell/></table:table-row>");
    }
    x.push_str(&cond_formats_xml(sheet, name, styles));
    x.push_str("</table:table>");
    x
}

/// Adresse ODF d'une cellule : `Feuille.A1` ou `'Feuille 2'.A1`.
fn odf_addr(sheet: &str, at: CellRef) -> String {
    format!("{}.{}", quote_sheet_name(sheet), at.name())
}

/// Constante d'une condition : nombre (point décimal) ou texte entre guillemets.
fn cond_operand(v: &str) -> String {
    match parse_literal(v.trim()).0 {
        Value::Number(n) => num(n),
        Value::Bool(b) => if b { "1" } else { "0" }.into(),
        _ => format!("\"{}\"", v.trim().replace('"', "\"\"")),
    }
}

/// `<calcext:conditional-formats>`, comme LibreOffice les écrit : les règles
/// consécutives sur la même plage sont regroupées (la première vérifiée l'emporte).
fn cond_formats_xml(sheet: &Sheet, name: &str, styles: &mut Styles) -> String {
    if sheet.cond_formats.is_empty() {
        return String::new();
    }
    let mut x = String::from("<calcext:conditional-formats>");
    let mut i = 0;
    let rules = &sheet.cond_formats;
    while i < rules.len() {
        let range = rules[i].range;
        let target = format!(
            "{}:{}",
            odf_addr(name, range.start),
            odf_addr(name, range.end)
        );
        let base = odf_addr(name, range.start);
        let _ = write!(
            x,
            "<calcext:conditional-format calcext:target-range-address=\"{}\">",
            esc(&target)
        );
        while i < rules.len() && rules[i].range == range {
            let cf = &rules[i];
            i += 1;
            let tl = format!("[.{}]", range.start.name());
            let q = |t: &str| format!("\"{}\"", t.replace('"', "\"\""));
            let value = match &cf.rule {
                CondRule::ColorScale { min, mid, max } => {
                    x.push_str("<calcext:color-scale>");
                    let _ = write!(
                        x,
                        "<calcext:color-scale-entry calcext:value=\"0\" calcext:type=\"minimum\" calcext:color=\"#{}\"/>",
                        hex(*min).to_lowercase()
                    );
                    if let Some(m) = mid {
                        let _ = write!(
                            x,
                            "<calcext:color-scale-entry calcext:value=\"50\" calcext:type=\"percentile\" calcext:color=\"#{}\"/>",
                            hex(*m).to_lowercase()
                        );
                    }
                    let _ = write!(
                        x,
                        "<calcext:color-scale-entry calcext:value=\"0\" calcext:type=\"maximum\" calcext:color=\"#{}\"/></calcext:color-scale>",
                        hex(*max).to_lowercase()
                    );
                    continue;
                }
                CondRule::DataBar { color } => {
                    let _ = write!(
                        x,
                        "<calcext:data-bar calcext:min-length=\"10\" calcext:max-length=\"90\" calcext:negative-color=\"#ff0000\" calcext:axis-position=\"none\" calcext:positive-color=\"#{}\" calcext:axis-color=\"#000000\"><calcext:formatting-entry calcext:value=\"0\" calcext:type=\"minimum\"/><calcext:formatting-entry calcext:value=\"0\" calcext:type=\"maximum\"/></calcext:data-bar>",
                        hex(*color).to_lowercase()
                    );
                    continue;
                }
                CondRule::Compare { op, value, value2 } => {
                    let a = cond_operand(value);
                    let b = cond_operand(value2);
                    match op {
                        CompareOp::Gt => format!(">{a}"),
                        CompareOp::Ge => format!(">={a}"),
                        CompareOp::Lt => format!("<{a}"),
                        CompareOp::Le => format!("<={a}"),
                        CompareOp::Eq => format!("={a}"),
                        CompareOp::Ne => format!("!={a}"),
                        CompareOp::Between => format!("between({a},{b})"),
                        CompareOp::NotBetween => format!("not-between({a},{b})"),
                    }
                }
                CondRule::Contains { text } => format!("contains-text({})", q(text)),
                CondRule::BeginsWith { text } => format!("begins-with({})", q(text)),
                CondRule::EndsWith { text } => format!("ends-with({})", q(text)),
                CondRule::Empty => format!("formula-is(LEN(TRIM({tl}))=0)"),
                CondRule::NotEmpty => format!("formula-is(LEN(TRIM({tl}))>0)"),
                CondRule::Error => "is-error".into(),
                CondRule::Top { n, bottom, percent } => format!(
                    "{}-{}({})",
                    if *bottom { "bottom" } else { "top" },
                    if *percent { "percent" } else { "elements" },
                    (*n).max(1)
                ),
                CondRule::Average { above: true } => "above-average".into(),
                CondRule::Average { above: false } => "below-average".into(),
            };
            let style = styles.cond(cf.style);
            let _ = write!(
                x,
                "<calcext:condition calcext:apply-style-name=\"{style}\" calcext:value=\"{}\" calcext:base-cell-address=\"{}\"/>",
                esc(&value),
                esc(&base)
            );
        }
        x.push_str("</calcext:conditional-format>");
    }
    x.push_str("</calcext:conditional-formats>");
    x
}

/// `settings.xml` : volets figés de chaque feuille (vue de LibreOffice).
fn settings_xml(wb: &Workbook, names: &[String]) -> String {
    let mut x = String::from(HEADER);
    x.push_str("<office:document-settings xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" xmlns:config=\"urn:oasis:names:tc:opendocument:xmlns:config:1.0\" office:version=\"1.3\"><office:settings>");
    x.push_str("<config:config-item-set config:name=\"ooo:view-settings\"><config:config-item-map-indexed config:name=\"Views\"><config:config-item-map-entry>");
    x.push_str("<config:config-item config:name=\"ViewId\" config:type=\"string\">view1</config:config-item>");
    x.push_str("<config:config-item-map-named config:name=\"Tables\">");
    let item = |x: &mut String, name: &str, kind: &str, v: u32| {
        let _ = write!(
            x,
            "<config:config-item config:name=\"{name}\" config:type=\"{kind}\">{v}</config:config-item>"
        );
    };
    for (sheet, name) in wb.sheets.iter().zip(names) {
        let rows = sheet.frozen_rows.min(MAX_ROWS - 1);
        let cols = sheet.frozen_cols.min(MAX_COLS - 1);
        let _ = write!(
            x,
            "<config:config-item-map-entry config:name=\"{}\">",
            esc(name)
        );
        item(&mut x, "CursorPositionX", "int", 0);
        item(&mut x, "CursorPositionY", "int", 0);
        item(
            &mut x,
            "HorizontalSplitMode",
            "short",
            if cols > 0 { 2 } else { 0 },
        );
        item(
            &mut x,
            "VerticalSplitMode",
            "short",
            if rows > 0 { 2 } else { 0 },
        );
        item(&mut x, "HorizontalSplitPosition", "int", cols);
        item(&mut x, "VerticalSplitPosition", "int", rows);
        let active = match (rows > 0, cols > 0) {
            (true, true) => 3,
            (true, false) => 2,
            (false, true) => 1,
            (false, false) => 2,
        };
        item(&mut x, "ActiveSplitRange", "short", active);
        item(&mut x, "PositionLeft", "int", 0);
        item(&mut x, "PositionRight", "int", cols);
        item(&mut x, "PositionTop", "int", 0);
        item(&mut x, "PositionBottom", "int", rows);
        x.push_str("</config:config-item-map-entry>");
    }
    x.push_str("</config:config-item-map-named>");
    if let Some(first) = names.first() {
        let _ = write!(
            x,
            "<config:config-item config:name=\"ActiveTable\" config:type=\"string\">{}</config:config-item>",
            esc(first)
        );
    }
    x.push_str(
        "</config:config-item-map-entry></config:config-item-map-indexed></config:config-item-set>",
    );
    x.push_str("</office:settings></office:document-settings>");
    x
}

pub(crate) fn to_ods(wb: &Workbook) -> Vec<u8> {
    let computed = wb.snapshot();
    let names = export_sheet_names(&wb.sheets);
    let mut styles = Styles::default();
    let tables: Vec<String> = wb
        .sheets
        .iter()
        .enumerate()
        .map(|(i, s)| table_xml(s, &names[i], computed.values.get(i), &mut styles))
        .collect();

    let mut content = String::from(HEADER);
    let _ = write!(content, "<office:document-content{NAMESPACES}>");
    content.push_str(&styles.xml());
    content.push_str("<office:body><office:spreadsheet>");
    // Comme Varium : critères sans casse, jokers * et ? (pas d'expressions
    // régulières), sur la cellule entière.
    content.push_str("<table:calculation-settings table:case-sensitive=\"false\" table:search-criteria-must-apply-to-whole-cell=\"true\" table:use-wildcards=\"true\" table:use-regular-expressions=\"false\" table:automatic-find-labels=\"false\"/>");
    for t in &tables {
        content.push_str(t);
    }
    content.push_str("</office:spreadsheet></office:body></office:document-content>");

    let mut st = String::from(HEADER);
    let _ = write!(st, "<office:document-styles{NAMESPACES}>");
    st.push_str("<office:styles>");
    st.push_str("<style:default-style style:family=\"table-cell\"><style:text-properties fo:language=\"fr\" fo:country=\"FR\"/></style:default-style>");
    st.push_str("<style:style style:name=\"Default\" style:family=\"table-cell\"/>");
    st.push_str(&styles.cond_xml());
    st.push_str("</office:styles></office:document-styles>");

    let mut meta = String::from(HEADER);
    let _ = write!(meta, "<office:document-meta{NAMESPACES}><office:meta>");
    let _ = write!(
        meta,
        "<meta:generator>Varium/{}</meta:generator><dc:title>{}</dc:title><dc:language>fr-FR</dc:language>",
        env!("CARGO_PKG_VERSION"),
        esc_text(&wb.title)
    );
    meta.push_str("</office:meta></office:document-meta>");

    let mut manifest = String::from(HEADER);
    manifest.push_str("<manifest:manifest xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\" manifest:version=\"1.3\">");
    let _ = write!(
        manifest,
        "<manifest:file-entry manifest:full-path=\"/\" manifest:version=\"1.3\" manifest:media-type=\"{MIME}\"/>"
    );
    for f in ["content.xml", "styles.xml", "meta.xml", "settings.xml"] {
        let _ = write!(
            manifest,
            "<manifest:file-entry manifest:full-path=\"{f}\" manifest:media-type=\"text/xml\"/>"
        );
    }
    manifest.push_str("</manifest:manifest>");

    let mut zip = Zip::new();
    zip.add("mimetype", MIME.as_bytes());
    zip.add("META-INF/manifest.xml", manifest.as_bytes());
    zip.add("content.xml", content.as_bytes());
    zip.add("styles.xml", st.as_bytes());
    zip.add("meta.xml", meta.as_bytes());
    zip.add("settings.xml", settings_xml(wb, &names).as_bytes());
    zip.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphes() {
        assert_eq!(text_p("a  b"), "<text:p>a <text:s/>b</text:p>");
        assert_eq!(text_p(" a"), "<text:p><text:s/>a</text:p>");
        assert_eq!(text_p("a\nb<"), "<text:p>a</text:p><text:p>b&lt;</text:p>");
        assert_eq!(
            text_p("a    b"),
            "<text:p>a <text:s text:c=\"3\"/>b</text:p>"
        );
    }

    #[test]
    fn attributs_de_valeur() {
        assert_eq!(
            value_attrs(&Value::Number(0.2), NumberFormat::Percent(0), false),
            " office:value-type=\"percentage\" office:value=\"0.2\""
        );
        assert!(
            value_attrs(&Value::Number(46096.0), NumberFormat::Date, false).contains("2026-03-15")
        );
        assert!(value_attrs(&Value::Bool(true), NumberFormat::General, true).contains("true"));
    }
}
