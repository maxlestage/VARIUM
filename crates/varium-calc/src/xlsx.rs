//! Export Office Open XML (`.xlsx`).
//!
//! Chaque cellule à formule porte sa formule en anglais (`<f>SUM(A1:A3)</f>`) et
//! sa valeur calculée (`<v>`) ; les textes sont écrits en ligne
//! (`t="inlineStr"`). Le classeur demande un recalcul complet à l'ouverture.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;

use varium_core::zip::Zip;

use crate::cond::{CompareOp, CondRule, CondStyle};
use crate::eval::{CellMap, is_formula};
use crate::format::{CellFormat, HAlign, NumberFormat, parse_literal};
use crate::refs::{CellRef, MAX_COLS, MAX_ROWS, Range};
use crate::rewrite::to_xlsx_formula;
use crate::value::Value;
use crate::workbook::{MAX_SHEET_NAME, Sheet, Workbook};
use crate::xml::{esc, esc_text, hex, num};

const HEADER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// Table des styles : chaque mise en forme distincte devient un `xf`.
#[derive(Default)]
struct Styles {
    xfs: Vec<CellFormat>,
    index: HashMap<CellFormat, usize>,
    fonts: Vec<(bool, bool, Option<[u8; 3]>)>,
    fills: Vec<[u8; 3]>,
    numfmts: Vec<String>,
    /// Styles différentiels de la mise en forme conditionnelle.
    dxfs: Vec<CondStyle>,
}

impl Styles {
    fn new() -> Self {
        let mut s = Styles::default();
        s.fonts.push((false, false, None));
        s.xf(&CellFormat::default());
        s
    }

    fn xf(&mut self, f: &CellFormat) -> usize {
        if let Some(&i) = self.index.get(f) {
            return i;
        }
        let i = self.xfs.len();
        self.xfs.push(*f);
        self.index.insert(*f, i);
        let font = (f.bold, f.italic, f.color);
        if !self.fonts.contains(&font) {
            self.fonts.push(font);
        }
        if let Some(fill) = f.fill
            && !self.fills.contains(&fill)
        {
            self.fills.push(fill);
        }
        if let Some(code) = numfmt_code(f.number)
            && !self.numfmts.contains(&code)
        {
            self.numfmts.push(code);
        }
        i
    }

    fn dxf(&mut self, s: CondStyle) -> usize {
        match self.dxfs.iter().position(|d| *d == s) {
            Some(i) => i,
            None => {
                self.dxfs.push(s);
                self.dxfs.len() - 1
            }
        }
    }

    fn numfmt_id(&self, n: NumberFormat) -> usize {
        match n {
            NumberFormat::General => 0,
            NumberFormat::Text => 49,
            other => numfmt_code(other)
                .and_then(|c| self.numfmts.iter().position(|x| *x == c))
                .map(|p| 164 + p)
                .unwrap_or(0),
        }
    }

    fn xml(&self) -> String {
        let mut x = String::from(HEADER);
        let _ = write!(x, "<styleSheet xmlns=\"{NS_MAIN}\">");
        if !self.numfmts.is_empty() {
            let _ = write!(x, "<numFmts count=\"{}\">", self.numfmts.len());
            for (i, code) in self.numfmts.iter().enumerate() {
                let _ = write!(
                    x,
                    "<numFmt numFmtId=\"{}\" formatCode=\"{}\"/>",
                    164 + i,
                    esc(code)
                );
            }
            x.push_str("</numFmts>");
        }
        let _ = write!(x, "<fonts count=\"{}\">", self.fonts.len());
        for (b, i, color) in &self.fonts {
            x.push_str("<font>");
            if *b {
                x.push_str("<b/>");
            }
            if *i {
                x.push_str("<i/>");
            }
            x.push_str("<sz val=\"11\"/>");
            if let Some(c) = color {
                let _ = write!(x, "<color rgb=\"FF{}\"/>", hex(*c));
            }
            x.push_str("<name val=\"Calibri\"/><family val=\"2\"/></font>");
        }
        x.push_str("</fonts>");
        let _ = write!(
            x,
            "<fills count=\"{}\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill>",
            self.fills.len() + 2
        );
        for c in &self.fills {
            let _ = write!(
                x,
                "<fill><patternFill patternType=\"solid\"><fgColor rgb=\"FF{}\"/><bgColor indexed=\"64\"/></patternFill></fill>",
                hex(*c)
            );
        }
        x.push_str("</fills>");
        x.push_str("<borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>");
        x.push_str("<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>");
        let _ = write!(x, "<cellXfs count=\"{}\">", self.xfs.len());
        for f in &self.xfs {
            let numfmt = self.numfmt_id(f.number);
            let font = self
                .fonts
                .iter()
                .position(|x| *x == (f.bold, f.italic, f.color))
                .unwrap_or(0);
            let fill = f
                .fill
                .and_then(|c| self.fills.iter().position(|x| *x == c))
                .map(|p| p + 2)
                .unwrap_or(0);
            let _ = write!(
                x,
                "<xf numFmtId=\"{numfmt}\" fontId=\"{font}\" fillId=\"{fill}\" borderId=\"0\" xfId=\"0\""
            );
            if numfmt != 0 {
                x.push_str(" applyNumberFormat=\"1\"");
            }
            if font != 0 {
                x.push_str(" applyFont=\"1\"");
            }
            if fill != 0 {
                x.push_str(" applyFill=\"1\"");
            }
            if f.align.is_some() || f.wrap {
                x.push_str(" applyAlignment=\"1\"><alignment");
                if let Some(a) = f.align {
                    let h = match a {
                        HAlign::Left => "left",
                        HAlign::Center => "center",
                        HAlign::Right => "right",
                    };
                    let _ = write!(x, " horizontal=\"{h}\"");
                }
                if f.wrap {
                    x.push_str(" wrapText=\"1\"");
                }
                x.push_str("/></xf>");
            } else {
                x.push_str("/>");
            }
        }
        x.push_str("</cellXfs>");
        x.push_str("<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>");
        if !self.dxfs.is_empty() {
            let _ = write!(x, "<dxfs count=\"{}\">", self.dxfs.len());
            for d in &self.dxfs {
                x.push_str("<dxf>");
                if d.bold || d.italic || d.color.is_some() {
                    x.push_str("<font>");
                    if d.bold {
                        x.push_str("<b/>");
                    }
                    if d.italic {
                        x.push_str("<i/>");
                    }
                    if let Some(c) = d.color {
                        let _ = write!(x, "<color rgb=\"FF{}\"/>", hex(c));
                    }
                    x.push_str("</font>");
                }
                if let Some(c) = d.fill {
                    let _ = write!(
                        x,
                        "<fill><patternFill patternType=\"solid\"><fgColor rgb=\"FF{0}\"/><bgColor rgb=\"FF{0}\"/></patternFill></fill>",
                        hex(c)
                    );
                }
                x.push_str("</dxf>");
            }
            x.push_str("</dxfs>");
        }
        x.push_str("</styleSheet>");
        x
    }
}

/// Code de format Excel d'un format de nombre (`None` : format intégré).
fn numfmt_code(n: NumberFormat) -> Option<String> {
    let decimals = |d: u8| {
        let d = usize::from(d.min(15));
        if d == 0 {
            String::new()
        } else {
            format!(".{}", "0".repeat(d))
        }
    };
    match n {
        NumberFormat::General | NumberFormat::Text => None,
        NumberFormat::Number(d) => Some(format!("#,##0{}", decimals(d))),
        NumberFormat::Currency(d) => Some(format!("#,##0{}\\ \"€\"", decimals(d))),
        NumberFormat::Percent(d) => Some(format!("0{}%", decimals(d))),
        NumberFormat::Date => Some("dd/mm/yyyy".into()),
    }
}

/// Noms de feuilles acceptés par Excel (uniques, 31 caractères, sans `[]:*?/\`).
pub(crate) fn export_sheet_names(sheets: &[Sheet]) -> Vec<String> {
    let mut used: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (i, s) in sheets.iter().enumerate() {
        let mut name: String = s
            .name
            .chars()
            .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\') && !c.is_control())
            .take(MAX_SHEET_NAME)
            .collect();
        name = name.trim().trim_matches('\'').to_string();
        if name.is_empty() {
            name = format!("Feuille {}", i + 1);
        }
        let base = name.clone();
        let mut k = 2;
        while used.contains(&name.to_lowercase()) {
            let suffix = format!(" ({k})");
            let keep = MAX_SHEET_NAME.saturating_sub(suffix.chars().count());
            name = format!("{}{suffix}", base.chars().take(keep).collect::<String>());
            k += 1;
        }
        used.push(name.to_lowercase());
        out.push(name);
    }
    out
}

fn sheet_xml(
    sheet: &Sheet,
    values: Option<&CellMap>,
    styles: &mut Styles,
    selected: bool,
    filter: &FilterExport,
) -> String {
    let mut x = String::from(HEADER);
    let _ = write!(x, "<worksheet xmlns=\"{NS_MAIN}\" xmlns:r=\"{NS_REL}\">");
    let (rows, cols) = sheet.used_size();
    let dim = if rows == 0 || cols == 0 {
        "A1".to_string()
    } else {
        Range::new(CellRef::new(0, 0), CellRef::new(rows - 1, cols - 1)).name()
    };
    let _ = write!(x, "<dimension ref=\"{dim}\"/>");
    let _ = write!(
        x,
        "<sheetViews><sheetView workbookViewId=\"0\"{}>",
        if selected { " tabSelected=\"1\"" } else { "" }
    );
    x.push_str(&pane_xml(sheet.frozen_rows, sheet.frozen_cols));
    x.push_str("</sheetView></sheetViews>");
    x.push_str("<sheetFormatPr defaultRowHeight=\"15\"/>");
    let widths = sheet.clean_col_widths();
    let heights = sheet.clean_row_heights();
    if !widths.is_empty() {
        x.push_str("<cols>");
        for (c, w) in &widths {
            let chars = (f64::from(*w) / 7.0).clamp(0.5, 255.0);
            let _ = write!(
                x,
                "<col min=\"{0}\" max=\"{0}\" width=\"{1:.2}\" customWidth=\"1\"/>",
                c + 1,
                chars
            );
        }
        x.push_str("</cols>");
    }
    x.push_str("<sheetData>");
    let mut row_ids: BTreeSet<u32> = sheet.cells().map(|(c, _)| c.row).collect();
    row_ids.extend(heights.keys().copied());
    row_ids.extend(filter.hidden.iter().copied());
    for row in row_ids {
        let _ = write!(x, "<row r=\"{}\"", u64::from(row) + 1);
        if filter.hidden.contains(&row) {
            x.push_str(" hidden=\"1\"");
        }
        if let Some(h) = heights.get(&row) {
            let _ = write!(
                x,
                " ht=\"{:.2}\" customHeight=\"1\"",
                (f64::from(*h) * 0.75).clamp(1.0, 409.0)
            );
        }
        x.push('>');
        let from = CellRef::new(row, 0);
        let to = CellRef::new(row, u32::MAX);
        for (&at, cell) in sheet.cells.range(from..=to) {
            let s = styles.xf(&cell.format);
            let style = if s == 0 {
                String::new()
            } else {
                format!(" s=\"{s}\"")
            };
            let r = at.name();
            let value = values.and_then(|m| m.get(&at));
            if is_formula(&cell.input) {
                let f = esc_text(&to_xlsx_formula(&cell.input));
                match value {
                    Some(Value::Number(n)) => {
                        let _ = write!(x, "<c r=\"{r}\"{style}><f>{f}</f><v>{}</v></c>", num(*n));
                    }
                    Some(Value::Text(t)) => {
                        let _ = write!(
                            x,
                            "<c r=\"{r}\"{style} t=\"str\"><f>{f}</f><v>{}</v></c>",
                            esc_text(t)
                        );
                    }
                    Some(Value::Bool(b)) => {
                        let _ = write!(
                            x,
                            "<c r=\"{r}\"{style} t=\"b\"><f>{f}</f><v>{}</v></c>",
                            u8::from(*b)
                        );
                    }
                    Some(Value::Error(e)) => {
                        let _ = write!(
                            x,
                            "<c r=\"{r}\"{style} t=\"e\"><f>{f}</f><v>{}</v></c>",
                            e.english_code()
                        );
                    }
                    Some(Value::Empty) | None => {
                        let _ = write!(x, "<c r=\"{r}\"{style}><f>{f}</f></c>");
                    }
                }
                continue;
            }
            match value {
                Some(Value::Number(n)) => {
                    let _ = write!(x, "<c r=\"{r}\"{style}><v>{}</v></c>", num(*n));
                }
                Some(Value::Bool(b)) => {
                    let _ = write!(x, "<c r=\"{r}\"{style} t=\"b\"><v>{}</v></c>", u8::from(*b));
                }
                Some(Value::Text(t)) => {
                    let _ = write!(
                        x,
                        "<c r=\"{r}\"{style} t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
                        esc_text(t)
                    );
                }
                _ => {
                    let _ = write!(x, "<c r=\"{r}\"{style}/>");
                }
            }
        }
        x.push_str("</row>");
    }
    x.push_str("</sheetData>");
    x.push_str(&filter.xml);
    x.push_str(&cond_xml(sheet, styles));
    x.push_str("</worksheet>");
    x
}

/// Filtre automatique d'une feuille, prêt à écrire.
#[derive(Default)]
struct FilterExport {
    xml: String,
    hidden: BTreeSet<u32>,
}

/// Plage effective du filtre automatique (étendue aux lignes ajoutées).
fn filter_range(wb: &Workbook, i: usize, sheet: &Sheet) -> Option<Range> {
    let af = sheet.auto_filter.as_ref()?;
    let rows = af.data_rows(&wb.sheets[i]);
    Some(Range::new(
        af.range.start,
        CellRef::new((*rows.end()).max(af.range.start.row), af.range.end.col),
    ))
}

/// Valeur d'un critère de filtre : nombre à l'anglaise ou texte.
fn filter_val(v: &str) -> String {
    match parse_literal(v.trim()).0 {
        Value::Number(n) => num(n),
        _ => v.trim().to_string(),
    }
}

fn filter_export(wb: &Workbook, i: usize) -> FilterExport {
    use crate::filter::FilterCondition as C;
    let Some(sheet) = wb.sheets.get(i) else {
        return FilterExport::default();
    };
    let (Some(af), Some(range)) = (&sheet.auto_filter, filter_range(wb, i, sheet)) else {
        return FilterExport::default();
    };
    let mut x = format!("<autoFilter ref=\"{}\"", range.name());
    let columns: Vec<_> = af
        .columns
        .iter()
        .filter(|c| !c.is_empty() && (range.start.col..=range.end.col).contains(&c.col))
        .collect();
    if columns.is_empty() {
        x.push_str("/>");
    } else {
        x.push('>');
        for cf in columns {
            let _ = write!(x, "<filterColumn colId=\"{}\">", cf.col - range.start.col);
            if !cf.hidden.is_empty() {
                // Excel écrit les valeurs affichées (cochées).
                let shown: Vec<String> = wb
                    .filter_values(i, cf.col)
                    .into_iter()
                    .map(|(v, _)| v)
                    .filter(|v| !cf.hidden.contains(v))
                    .collect();
                let blank = shown.iter().any(String::is_empty);
                x.push_str(if blank {
                    "<filters blank=\"1\">"
                } else {
                    "<filters>"
                });
                for v in shown.iter().filter(|v| !v.is_empty()) {
                    let _ = write!(x, "<filter val=\"{}\"/>", esc(v));
                }
                x.push_str("</filters>");
            } else if let Some(cond) = &cf.condition {
                let custom = |pairs: &[(&str, String)], and: bool| {
                    let mut c = String::from(if and && pairs.len() > 1 {
                        "<customFilters and=\"1\">"
                    } else {
                        "<customFilters>"
                    });
                    for (op, v) in pairs {
                        let _ = write!(c, "<customFilter operator=\"{op}\" val=\"{}\"/>", esc(v));
                    }
                    c.push_str("</customFilters>");
                    c
                };
                let body = match cond {
                    C::Compare { op, value, value2 } => {
                        let (a, b) = (filter_val(value), filter_val(value2));
                        match op {
                            CompareOp::Gt => custom(&[("greaterThan", a)], true),
                            CompareOp::Ge => custom(&[("greaterThanOrEqual", a)], true),
                            CompareOp::Lt => custom(&[("lessThan", a)], true),
                            CompareOp::Le => custom(&[("lessThanOrEqual", a)], true),
                            CompareOp::Eq => custom(&[("equal", a)], true),
                            CompareOp::Ne => custom(&[("notEqual", a)], true),
                            CompareOp::Between => {
                                custom(&[("greaterThanOrEqual", a), ("lessThanOrEqual", b)], true)
                            }
                            CompareOp::NotBetween => {
                                custom(&[("lessThan", a), ("greaterThan", b)], false)
                            }
                        }
                    }
                    C::Contains { text } => custom(&[("equal", format!("*{text}*"))], true),
                    C::NotContains { text } => custom(&[("notEqual", format!("*{text}*"))], true),
                    C::BeginsWith { text } => custom(&[("equal", format!("{text}*"))], true),
                    C::EndsWith { text } => custom(&[("equal", format!("*{text}"))], true),
                    C::Top { n, bottom } => format!(
                        "<top10{} val=\"{}\"/>",
                        if *bottom { " top=\"0\"" } else { "" },
                        (*n).max(1)
                    ),
                    C::AboveAverage => "<dynamicFilter type=\"aboveAverage\"/>".into(),
                    C::BelowAverage => "<dynamicFilter type=\"belowAverage\"/>".into(),
                };
                x.push_str(&body);
            }
            x.push_str("</filterColumn>");
        }
        x.push_str("</autoFilter>");
    }
    FilterExport {
        xml: x,
        hidden: wb.filtered_rows(i),
    }
}

/// Volets figés : `<pane state="frozen"/>` et la sélection du volet actif.
fn pane_xml(rows: u32, cols: u32) -> String {
    let rows = rows.min(MAX_ROWS - 1);
    let cols = cols.min(MAX_COLS - 1);
    if rows == 0 && cols == 0 {
        return String::new();
    }
    let active = match (rows > 0, cols > 0) {
        (true, true) => "bottomRight",
        (true, false) => "bottomLeft",
        _ => "topRight",
    };
    let mut x = String::from("<pane");
    if cols > 0 {
        let _ = write!(x, " xSplit=\"{cols}\"");
    }
    if rows > 0 {
        let _ = write!(x, " ySplit=\"{rows}\"");
    }
    let top_left = CellRef::new(rows, cols).name();
    let _ = write!(
        x,
        " topLeftCell=\"{top_left}\" activePane=\"{active}\" state=\"frozen\"/><selection pane=\"{active}\" activeCell=\"{top_left}\" sqref=\"{top_left}\"/>"
    );
    x
}

/// Constante d'une règle pour une formule Excel : nombre ou texte entre guillemets.
fn rule_operand(v: &str) -> String {
    match parse_literal(v.trim()).0 {
        Value::Number(n) => num(n),
        Value::Bool(b) => if b { "TRUE" } else { "FALSE" }.into(),
        _ => format!("\"{}\"", v.trim().replace('"', "\"\"")),
    }
}

/// `<conditionalFormatting>` : une règle par plage, priorités dans l'ordre.
fn cond_xml(sheet: &Sheet, styles: &mut Styles) -> String {
    let mut x = String::new();
    for (i, cf) in sheet.cond_formats.iter().enumerate() {
        let priority = i + 1;
        let sqref = cf.range.name();
        let tl = cf.range.start.name();
        let dxf = if cf.rule.has_style() {
            format!(" dxfId=\"{}\"", styles.dxf(cf.style))
        } else {
            String::new()
        };
        let rule = match &cf.rule {
            CondRule::Compare { op, value, value2 } => {
                let operator = match op {
                    CompareOp::Gt => "greaterThan",
                    CompareOp::Ge => "greaterThanOrEqual",
                    CompareOp::Lt => "lessThan",
                    CompareOp::Le => "lessThanOrEqual",
                    CompareOp::Eq => "equal",
                    CompareOp::Ne => "notEqual",
                    CompareOp::Between => "between",
                    CompareOp::NotBetween => "notBetween",
                };
                let mut f = format!("<formula>{}</formula>", esc_text(&rule_operand(value)));
                if op.two_values() {
                    let _ = write!(f, "<formula>{}</formula>", esc_text(&rule_operand(value2)));
                }
                format!(
                    "<cfRule type=\"cellIs\"{dxf} priority=\"{priority}\" operator=\"{operator}\">{f}</cfRule>"
                )
            }
            CondRule::Contains { text } => {
                let q = text.replace('"', "\"\"");
                format!(
                    "<cfRule type=\"containsText\"{dxf} priority=\"{priority}\" operator=\"containsText\" text=\"{}\"><formula>{}</formula></cfRule>",
                    esc(text),
                    esc_text(&format!("NOT(ISERROR(SEARCH(\"{q}\",{tl})))"))
                )
            }
            CondRule::BeginsWith { text } => {
                let q = text.replace('"', "\"\"");
                format!(
                    "<cfRule type=\"beginsWith\"{dxf} priority=\"{priority}\" operator=\"beginsWith\" text=\"{}\"><formula>{}</formula></cfRule>",
                    esc(text),
                    esc_text(&format!("LEFT({tl},LEN(\"{q}\"))=\"{q}\""))
                )
            }
            CondRule::EndsWith { text } => {
                let q = text.replace('"', "\"\"");
                format!(
                    "<cfRule type=\"endsWith\"{dxf} priority=\"{priority}\" operator=\"endsWith\" text=\"{}\"><formula>{}</formula></cfRule>",
                    esc(text),
                    esc_text(&format!("RIGHT({tl},LEN(\"{q}\"))=\"{q}\""))
                )
            }
            CondRule::Empty => format!(
                "<cfRule type=\"containsBlanks\"{dxf} priority=\"{priority}\"><formula>LEN(TRIM({tl}))=0</formula></cfRule>"
            ),
            CondRule::NotEmpty => format!(
                "<cfRule type=\"notContainsBlanks\"{dxf} priority=\"{priority}\"><formula>LEN(TRIM({tl}))&gt;0</formula></cfRule>"
            ),
            CondRule::Error => format!(
                "<cfRule type=\"containsErrors\"{dxf} priority=\"{priority}\"><formula>ISERROR({tl})</formula></cfRule>"
            ),
            CondRule::Top { n, bottom, percent } => format!(
                "<cfRule type=\"top10\"{dxf} priority=\"{priority}\" rank=\"{}\"{}{}/>",
                (*n).max(1),
                if *bottom { " bottom=\"1\"" } else { "" },
                if *percent { " percent=\"1\"" } else { "" }
            ),
            CondRule::Average { above } => format!(
                "<cfRule type=\"aboveAverage\"{dxf} priority=\"{priority}\"{}/>",
                if *above { "" } else { " aboveAverage=\"0\"" }
            ),
            CondRule::ColorScale { min, mid, max } => {
                let mut c = String::from("<cfvo type=\"min\"/>");
                if mid.is_some() {
                    c.push_str("<cfvo type=\"percentile\" val=\"50\"/>");
                }
                c.push_str("<cfvo type=\"max\"/>");
                for col in [Some(*min), *mid, Some(*max)].into_iter().flatten() {
                    let _ = write!(c, "<color rgb=\"FF{}\"/>", hex(col));
                }
                format!(
                    "<cfRule type=\"colorScale\" priority=\"{priority}\"><colorScale>{c}</colorScale></cfRule>"
                )
            }
            CondRule::Duplicate { unique } => format!(
                "<cfRule type=\"{}\"{dxf} priority=\"{priority}\"/>",
                if *unique {
                    "uniqueValues"
                } else {
                    "duplicateValues"
                }
            ),
            CondRule::DataBar { color } => format!(
                "<cfRule type=\"dataBar\" priority=\"{priority}\"><dataBar><cfvo type=\"min\"/><cfvo type=\"max\"/><color rgb=\"FF{}\"/></dataBar></cfRule>",
                hex(*color)
            ),
        };
        let _ = write!(
            x,
            "<conditionalFormatting sqref=\"{sqref}\">{rule}</conditionalFormatting>"
        );
    }
    x
}

pub(crate) fn to_xlsx(wb: &Workbook) -> Vec<u8> {
    let computed = wb.snapshot();
    let mut styles = Styles::new();
    let sheets: Vec<String> = wb
        .sheets
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let filter = filter_export(wb, i);
            sheet_xml(s, computed.values.get(i), &mut styles, i == 0, &filter)
        })
        .collect();
    let names = export_sheet_names(&wb.sheets);
    let n = sheets.len();

    let mut types = String::from(HEADER);
    types
        .push_str("<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">");
    types.push_str("<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>");
    types.push_str("<Default Extension=\"xml\" ContentType=\"application/xml\"/>");
    types.push_str("<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>");
    for i in 1..=n {
        let _ = write!(
            types,
            "<Override PartName=\"/xl/worksheets/sheet{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
        );
    }
    types.push_str("<Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>");
    types.push_str("<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>");
    types.push_str("<Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>");
    types.push_str("</Types>");

    let mut rels = String::from(HEADER);
    rels.push_str(
        "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    );
    rels.push_str("<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>");
    rels.push_str("<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>");
    rels.push_str("<Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties\" Target=\"docProps/app.xml\"/>");
    rels.push_str("</Relationships>");

    let mut book = String::from(HEADER);
    let _ = write!(book, "<workbook xmlns=\"{NS_MAIN}\" xmlns:r=\"{NS_REL}\">");
    book.push_str("<bookViews><workbookView activeTab=\"0\"/></bookViews><sheets>");
    for (i, name) in names.iter().enumerate() {
        let _ = write!(
            book,
            "<sheet name=\"{}\" sheetId=\"{}\" r:id=\"rId{}\"/>",
            esc(name),
            i + 1,
            i + 1
        );
    }
    book.push_str("</sheets>");
    // Plage du filtre automatique, comme Excel l'écrit.
    let mut defined = String::new();
    for (i, (s, name)) in wb.sheets.iter().zip(&names).enumerate() {
        if let Some(r) = filter_range(wb, i, s) {
            let abs = |c: CellRef| format!("${}${}", crate::refs::col_name(c.col), c.row + 1);
            let _ = write!(
                defined,
                "<definedName name=\"_xlnm._FilterDatabase\" localSheetId=\"{i}\" hidden=\"1\">{}</definedName>",
                esc_text(&format!(
                    "{}!{}:{}",
                    crate::refs::always_quote_sheet_name(name),
                    abs(r.start),
                    abs(r.end)
                ))
            );
        }
    }
    if !defined.is_empty() {
        let _ = write!(book, "<definedNames>{defined}</definedNames>");
    }
    book.push_str("<calcPr calcId=\"191029\" fullCalcOnLoad=\"1\"/></workbook>");

    let mut book_rels = String::from(HEADER);
    book_rels.push_str(
        "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    );
    for i in 1..=n {
        let _ = write!(
            book_rels,
            "<Relationship Id=\"rId{i}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{i}.xml\"/>"
        );
    }
    let _ = write!(
        book_rels,
        "<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>",
        n + 1
    );
    book_rels.push_str("</Relationships>");

    let mut core = String::from(HEADER);
    core.push_str("<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">");
    let _ = write!(
        core,
        "<dc:title>{}</dc:title><dc:creator>Varium</dc:creator></cp:coreProperties>",
        esc_text(&wb.title)
    );

    let mut app = String::from(HEADER);
    app.push_str("<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\"><Application>Varium</Application></Properties>");

    let mut zip = Zip::new();
    zip.add("[Content_Types].xml", types.as_bytes());
    zip.add("_rels/.rels", rels.as_bytes());
    zip.add("docProps/core.xml", core.as_bytes());
    zip.add("docProps/app.xml", app.as_bytes());
    zip.add("xl/workbook.xml", book.as_bytes());
    zip.add("xl/_rels/workbook.xml.rels", book_rels.as_bytes());
    zip.add("xl/styles.xml", styles.xml().as_bytes());
    for (i, s) in sheets.iter().enumerate() {
        zip.add(&format!("xl/worksheets/sheet{}.xml", i + 1), s.as_bytes());
    }
    zip.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_de_format() {
        assert_eq!(numfmt_code(NumberFormat::Number(2)).unwrap(), "#,##0.00");
        assert_eq!(numfmt_code(NumberFormat::Percent(0)).unwrap(), "0%");
        assert_eq!(
            numfmt_code(NumberFormat::Currency(2)).unwrap(),
            "#,##0.00\\ \"€\""
        );
        assert_eq!(numfmt_code(NumberFormat::General), None);
    }

    #[test]
    fn noms_de_feuilles_exportes() {
        let sheets = vec![
            Sheet::new("A/B"),
            Sheet::new("ab"),
            Sheet::new(""),
            Sheet::new("x".repeat(40)),
        ];
        let names = export_sheet_names(&sheets);
        assert_eq!(names[0], "AB");
        assert_eq!(names[1], "ab (2)");
        assert_eq!(names[2], "Feuille 3");
        assert_eq!(names[3].chars().count(), 31);
    }
}
