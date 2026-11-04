//! Import Office Open XML (`.xlsx`).
//!
//! Sont lus : les feuilles et leurs noms, les valeurs (nombres, textes partagés
//! ou en ligne, valeurs logiques, erreurs, dates), les formules (traduites en
//! saisie Varium, formules partagées comprises), les formats de nombres
//! (approchés), gras, italique, couleurs, alignement, largeurs de colonnes,
//! hauteurs de lignes, volets figés et mise en forme conditionnelle. Un
//! fichier abîmé donne une erreur en français, jamais un plantage.

use std::collections::HashMap;

use varium_core::zip::ZipReader;

use crate::cond::{CompareOp, CondFormat, CondRule, CondStyle};
use crate::filter::{AutoFilter, ColumnFilter, FilterCondition};
use crate::format::{CellFormat, HAlign, NumberFormat};
use crate::import::{
    Filler, bool_input, builtin_number_format, error_input, expression_rule, finish, iso_date,
    number_format_from_code, number_input, parse_rgb, text_input, unique_sheet_names,
};
use crate::refs::{CellRef, MAX_COLS, MAX_ROWS, Range};
use crate::rewrite::{from_xlsx_formula, shift_formula};
use crate::workbook::{Sheet, Workbook};
use crate::xmltree::{Node, parse};

fn err(msg: impl std::fmt::Display) -> String {
    format!("Fichier Excel illisible : {msg}")
}

/// Résout la cible d'une relation par rapport au dossier de la partie source.
fn resolve(dir: &str, target: &str) -> String {
    let target = target.trim();
    if let Some(abs) = target.strip_prefix('/') {
        return abs.to_string();
    }
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for p in target.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    parts.join("/")
}

/// Relations d'une partie : identifiant → (type, chemin complet).
fn relations(zip: &ZipReader, part: &str) -> HashMap<String, (String, String)> {
    let (dir, file) = part.rsplit_once('/').unwrap_or(("", part));
    let rels_path = if dir.is_empty() {
        format!("_rels/{file}.rels")
    } else {
        format!("{dir}/_rels/{file}.rels")
    };
    let mut out = HashMap::new();
    let Some(text) = zip.text(&rels_path) else {
        return out;
    };
    let Ok(root) = parse(&text) else {
        return out;
    };
    for r in root.children_named("Relationship") {
        if let (Some(id), Some(target)) = (r.attr("Id"), r.attr("Target")) {
            if r.attr("TargetMode") == Some("External") {
                continue;
            }
            let kind = r.attr("Type").unwrap_or("").to_string();
            out.insert(id.to_string(), (kind, resolve(dir, target)));
        }
    }
    out
}

fn truthy(v: Option<&str>) -> bool {
    matches!(v.map(str::trim), Some("1" | "true" | "on"))
}

/// Valeur booléenne d'un élément comme `<b/>` ou `<b val="0"/>`.
fn flag(n: Option<&Node>) -> bool {
    match n {
        Some(n) => !matches!(n.attr("val"), Some("0" | "false" | "off")),
        None => false,
    }
}

/// Couleurs du thème Office par défaut, dans l'ordre des index de couleur
/// de thème d'Excel (clair 1, foncé 1, clair 2, foncé 2, accents 1 à 6,
/// lien hypertexte, lien visité).
const DEFAULT_THEME: [[u8; 3]; 12] = [
    [0xFF, 0xFF, 0xFF],
    [0x00, 0x00, 0x00],
    [0xE7, 0xE6, 0xE6],
    [0x44, 0x54, 0x6A],
    [0x44, 0x72, 0xC4],
    [0xED, 0x7D, 0x31],
    [0xA5, 0xA5, 0xA5],
    [0xFF, 0xC0, 0x00],
    [0x5B, 0x9B, 0xD5],
    [0x70, 0xAD, 0x47],
    [0x05, 0x63, 0xC1],
    [0x95, 0x4F, 0x72],
];

/// Palette des couleurs indexées (anciens fichiers Excel).
const INDEXED: [u32; 64] = [
    0x000000, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF, 0x000000,
    0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF, 0x800000, 0x008000,
    0x000080, 0x808000, 0x800080, 0x008080, 0xC0C0C0, 0x808080, 0x9999FF, 0x993366, 0xFFFFCC,
    0xCCFFFF, 0x660066, 0xFF8080, 0x0066CC, 0xCCCCFF, 0x000080, 0xFF00FF, 0xFFFF00, 0x00FFFF,
    0x800080, 0x800000, 0x008080, 0x0000FF, 0x00CCFF, 0xCCFFFF, 0xCCFFCC, 0xFFFF99, 0x99CCFF,
    0xFF99CC, 0xCC99FF, 0xFFCC99, 0x3366FF, 0x33CCCC, 0x99CC00, 0xFFCC00, 0xFF9900, 0xFF6600,
    0x666699, 0x969696, 0x003366, 0x339966, 0x003300, 0x333300, 0x993300, 0x993366, 0x333399,
    0x333333,
];

/// Couleurs du thème du classeur (`xl/theme/theme1.xml`), dans l'ordre des
/// index de couleur de thème ; le thème par défaut si elles manquent.
fn read_theme(text: Option<&str>) -> Vec<[u8; 3]> {
    let mut out = DEFAULT_THEME.to_vec();
    let Some(root) = text.and_then(|t| parse(t).ok()) else {
        return out;
    };
    fn find<'a>(n: &'a Node, name: &str, depth: u32) -> Option<&'a Node> {
        if n.is(name) {
            return Some(n);
        }
        if depth > 6 {
            return None;
        }
        n.elems().find_map(|c| find(c, name, depth + 1))
    }
    let Some(scheme) = find(&root, "clrScheme", 0) else {
        return out;
    };
    // Ordre du fichier : dk1, lt1, dk2, lt2, accent1…6, hlink, folHlink.
    let slots = [1, 0, 3, 2, 4, 5, 6, 7, 8, 9, 10, 11];
    for (i, c) in scheme.elems().take(12).enumerate() {
        let rgb = c
            .child("srgbClr")
            .and_then(|n| n.attr("val"))
            .or_else(|| c.child("sysClr").and_then(|n| n.attr("lastClr")))
            .and_then(parse_rgb);
        if let Some(rgb) = rgb {
            out[slots[i]] = rgb;
        }
    }
    out
}

/// Teinte d'Excel : la luminosité (TSL) est assombrie (teinte négative) ou
/// éclaircie (positive).
fn apply_tint(c: [u8; 3], tint: f64) -> [u8; 3] {
    if !tint.is_finite() || tint == 0.0 {
        return c;
    }
    let tint = tint.clamp(-1.0, 1.0);
    let [r, g, b] = c.map(|x| f64::from(x) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    let (h, sat) = if d == 0.0 {
        (0.0, 0.0)
    } else {
        let sat = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h / 6.0, sat)
    };
    let l = if tint < 0.0 {
        l * (1.0 + tint)
    } else {
        l * (1.0 - tint) + tint
    };
    let hue = |p: f64, q: f64, t: f64| {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let (r, g, b) = if sat == 0.0 {
        (l, l, l)
    } else {
        let q = if l < 0.5 {
            l * (1.0 + sat)
        } else {
            l + sat - l * sat
        };
        let p = 2.0 * l - q;
        (
            hue(p, q, h + 1.0 / 3.0),
            hue(p, q, h),
            hue(p, q, h - 1.0 / 3.0),
        )
    };
    [r, g, b].map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Couleur d'un élément `<color>` : RVB, couleur de thème (avec sa teinte)
/// ou couleur indexée.
fn color_of(n: Option<&Node>, theme: &[[u8; 3]]) -> Option<[u8; 3]> {
    let c = n?;
    let base = if let Some(rgb) = c.attr("rgb") {
        parse_rgb(rgb)?
    } else if let Some(t) = c.attr("theme") {
        let i: usize = t.trim().parse().ok()?;
        *theme.get(i).or_else(|| DEFAULT_THEME.get(i))?
    } else {
        let i: usize = c.attr("indexed")?.trim().parse().ok()?;
        let v = *INDEXED.get(i)?;
        [(v >> 16) as u8, (v >> 8) as u8, v as u8]
    };
    let tint = c.attr("tint").and_then(parse_f64).unwrap_or(0.0);
    Some(apply_tint(base, tint))
}

/// Feuille de styles : formats de cellule (`cellXfs`) et styles différentiels (`dxfs`).
#[derive(Default)]
struct Styles {
    xfs: Vec<CellFormat>,
    dxfs: Vec<CondStyle>,
    theme: Vec<[u8; 3]>,
}

fn read_styles(text: &str, theme: Vec<[u8; 3]>) -> Styles {
    let Ok(root) = parse(text) else {
        return Styles {
            theme,
            ..Styles::default()
        };
    };
    let th = theme.as_slice();
    let color_of = |n: Option<&Node>| color_of(n, th);
    let mut numfmts: HashMap<u32, NumberFormat> = HashMap::new();
    if let Some(nf) = root.child("numFmts") {
        for n in nf.children_named("numFmt") {
            if let (Some(id), Some(code)) = (
                n.attr("numFmtId").and_then(|v| v.trim().parse().ok()),
                n.attr("formatCode"),
            ) {
                numfmts.insert(id, number_format_from_code(code));
            }
        }
    }
    let fonts: Vec<(bool, bool, Option<[u8; 3]>)> = root
        .child("fonts")
        .map(|f| {
            f.children_named("font")
                .map(|n| {
                    (
                        flag(n.child("b")),
                        flag(n.child("i")),
                        color_of(n.child("color")),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let fills: Vec<Option<[u8; 3]>> = root
        .child("fills")
        .map(|f| {
            f.children_named("fill")
                .map(|n| {
                    let p = n.child("patternFill")?;
                    if p.attr("patternType") != Some("solid") {
                        return None;
                    }
                    color_of(p.child("fgColor")).or_else(|| color_of(p.child("bgColor")))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut styles = Styles::default();
    if let Some(xfs) = root.child("cellXfs") {
        for xf in xfs.children_named("xf") {
            let idx = |name: &str| xf.attr(name).and_then(|v| v.trim().parse::<usize>().ok());
            let mut f = CellFormat::default();
            if let Some(id) = idx("numFmtId") {
                let id = id as u32;
                f.number = numfmts
                    .get(&id)
                    .copied()
                    .unwrap_or_else(|| builtin_number_format(id));
            }
            if let Some(&(b, i, c)) = idx("fontId").and_then(|i| fonts.get(i)) {
                f.bold = b;
                f.italic = i;
                // Le noir explicite est la couleur par défaut.
                f.color = c.filter(|c| *c != [0, 0, 0]);
            }
            if let Some(&fill) = idx("fillId").and_then(|i| fills.get(i)) {
                f.fill = fill;
            }
            if let Some(a) = xf.child("alignment") {
                f.align = match a.attr("horizontal") {
                    Some("left") => Some(HAlign::Left),
                    Some("center" | "centerContinuous") => Some(HAlign::Center),
                    Some("right") => Some(HAlign::Right),
                    _ => None,
                };
                f.wrap = truthy(a.attr("wrapText"));
            }
            styles.xfs.push(f);
        }
    }
    if let Some(dxfs) = root.child("dxfs") {
        for d in dxfs.children_named("dxf") {
            let mut s = CondStyle::default();
            if let Some(font) = d.child("font") {
                s.bold = flag(font.child("b"));
                s.italic = flag(font.child("i"));
                s.color = color_of(font.child("color"));
            }
            if let Some(p) = d.child("fill").and_then(|f| f.child("patternFill")) {
                s.fill = color_of(p.child("bgColor")).or_else(|| color_of(p.child("fgColor")));
            }
            styles.dxfs.push(s);
        }
    }
    styles.theme = theme;
    styles
}

fn shared_strings(text: &str) -> Vec<String> {
    let Ok(root) = parse(text) else {
        return Vec::new();
    };
    root.children_named("si").map(rich_text).collect()
}

/// Texte d'un `<si>` ou `<is>` : `<t>` direct ou suites `<r><t>`, sans la
/// phonétique (`<rPh>`).
fn rich_text(n: &Node) -> String {
    let mut out = String::new();
    for c in n.elems() {
        if c.is("t") {
            out.push_str(&c.text());
        } else if c.is("r") {
            for t in c.children_named("t") {
                out.push_str(&t.text());
            }
        }
    }
    out
}

/// Lit une colonne (`ABC`) ou une adresse (`B12`) d'un attribut `r`.
fn cell_ref(r: &str) -> Option<CellRef> {
    CellRef::parse(r)
}

fn parse_f64(s: &str) -> Option<f64> {
    let v: f64 = s.trim().parse().ok()?;
    v.is_finite().then_some(v)
}

/// Saisie d'une constante de règle (`10`, `"abc"`) ; `None` pour une formule.
fn rule_constant(f: &str) -> Option<String> {
    let f = f.trim();
    if let Some(s) = f.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        return Some(s.replace("\"\"", "\""));
    }
    parse_f64(f).map(crate::format::format_general)
}

fn cond_rules(sheet_xml: &Node, styles: &Styles) -> Vec<CondFormat> {
    let mut found: Vec<(i64, usize, CondFormat)> = Vec::new();
    for cf in sheet_xml.children_named("conditionalFormatting") {
        let ranges: Vec<Range> = cf
            .attr("sqref")
            .unwrap_or("")
            .split_whitespace()
            .filter_map(Range::parse)
            .collect();
        for rule in cf.children_named("cfRule") {
            let priority = rule
                .attr("priority")
                .and_then(|p| p.trim().parse().ok())
                .unwrap_or(i64::MAX);
            let style = rule
                .attr("dxfId")
                .and_then(|d| d.trim().parse::<usize>().ok())
                .and_then(|d| styles.dxfs.get(d).copied())
                .unwrap_or_default();
            let formulas: Vec<String> = rule.children_named("formula").map(Node::text).collect();
            let text = rule.attr("text").unwrap_or("").to_string();
            let r = match rule.attr("type").unwrap_or("") {
                "cellIs" => {
                    let op = match rule.attr("operator").unwrap_or("") {
                        "greaterThan" => CompareOp::Gt,
                        "greaterThanOrEqual" => CompareOp::Ge,
                        "lessThan" => CompareOp::Lt,
                        "lessThanOrEqual" => CompareOp::Le,
                        "equal" => CompareOp::Eq,
                        "notEqual" => CompareOp::Ne,
                        "between" => CompareOp::Between,
                        "notBetween" => CompareOp::NotBetween,
                        _ => continue,
                    };
                    let Some(value) = formulas.first().and_then(|f| rule_constant(f)) else {
                        continue;
                    };
                    let value2 = if op.two_values() {
                        match formulas.get(1).and_then(|f| rule_constant(f)) {
                            Some(v) => v,
                            None => continue,
                        }
                    } else {
                        String::new()
                    };
                    CondRule::Compare { op, value, value2 }
                }
                "containsText" if rule.attr("operator") != Some("notContains") => {
                    CondRule::Contains { text }
                }
                "beginsWith" => CondRule::BeginsWith { text },
                "endsWith" => CondRule::EndsWith { text },
                "containsBlanks" => CondRule::Empty,
                "notContainsBlanks" => CondRule::NotEmpty,
                "containsErrors" => CondRule::Error,
                "duplicateValues" => CondRule::Duplicate { unique: false },
                "uniqueValues" => CondRule::Duplicate { unique: true },
                "expression" => match formulas.first().and_then(|f| expression_rule(f)) {
                    Some(r) => r,
                    None => continue,
                },
                "top10" => CondRule::Top {
                    n: rule
                        .attr("rank")
                        .and_then(|r| r.trim().parse().ok())
                        .unwrap_or(10),
                    bottom: truthy(rule.attr("bottom")),
                    percent: truthy(rule.attr("percent")),
                },
                "aboveAverage" => CondRule::Average {
                    above: rule.attr("aboveAverage").is_none_or(|v| truthy(Some(v))),
                },
                "colorScale" => {
                    let colors: Vec<[u8; 3]> = rule
                        .child("colorScale")
                        .map(|c| {
                            c.children_named("color")
                                .filter_map(|n| color_of(Some(n), &styles.theme))
                                .collect()
                        })
                        .unwrap_or_default();
                    match colors.as_slice() {
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
                    }
                }
                "dataBar" => CondRule::DataBar {
                    color: rule
                        .child("dataBar")
                        .and_then(|d| color_of(d.child("color"), &styles.theme))
                        .unwrap_or([0x63, 0x8E, 0xC6]),
                },
                _ => continue,
            };
            for range in &ranges {
                let order = found.len();
                found.push((
                    priority,
                    order,
                    CondFormat {
                        range: *range,
                        rule: r.clone(),
                        style,
                    },
                ));
            }
        }
    }
    found.sort_by_key(|(p, o, _)| (*p, *o));
    found.into_iter().map(|(_, _, cf)| cf).collect()
}

/// Formule partagée : formule de la cellule maîtresse (saisie Varium) et position.
struct SharedFormula {
    at: CellRef,
    input: String,
}

fn read_sheet(
    root: &Node,
    sheet: &mut Sheet,
    strings: &[String],
    styles: &Styles,
    date1904: bool,
    count: &mut usize,
) {
    // Colonnes
    if let Some(cols) = root.child("cols") {
        for c in cols.children_named("col") {
            let min: u32 = c
                .attr("min")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            let max: u32 = c
                .attr("max")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            let Some(w) = c.attr("width").and_then(parse_f64) else {
                continue;
            };
            if min == 0 || max < min || max - min > 255 || !truthy(c.attr("customWidth")) {
                continue;
            }
            let px = if truthy(c.attr("hidden")) {
                4.0
            } else {
                (w * 7.0).clamp(4.0, 4000.0) as f32
            };
            for col in min..=max.min(MAX_COLS) {
                sheet.col_widths.insert(col - 1, px);
            }
        }
    }
    // Volets figés
    if let Some(pane) = root
        .child("sheetViews")
        .and_then(|v| v.child("sheetView"))
        .and_then(|v| v.child("pane"))
        && matches!(pane.attr("state"), Some("frozen" | "frozenSplit"))
    {
        let n = |k: &str| {
            pane.attr(k)
                .and_then(parse_f64)
                .map_or(0, |v| v.clamp(0.0, 10_000.0) as u32)
        };
        sheet.frozen_cols = n("xSplit");
        sheet.frozen_rows = n("ySplit");
    }
    let mut fill = Filler { sheet, count };
    let mut shared: HashMap<String, SharedFormula> = HashMap::new();
    let Some(data) = root.child("sheetData") else {
        return;
    };
    let mut next_row = 0u32;
    for row in data.children_named("row") {
        let r = row
            .attr("r")
            .and_then(|v| v.trim().parse::<u32>().ok())
            .filter(|r| (1..=MAX_ROWS).contains(r))
            .map_or(next_row, |r| r - 1);
        next_row = r.saturating_add(1);
        if truthy(row.attr("customHeight"))
            && let Some(h) = row.attr("ht").and_then(parse_f64)
        {
            fill.sheet
                .row_heights
                .insert(r, (h / 0.75).clamp(2.0, 2000.0) as f32);
        }
        let mut next_col = 0u32;
        for c in row.children_named("c") {
            let at = c
                .attr("r")
                .and_then(cell_ref)
                .unwrap_or(CellRef::new(r, next_col));
            next_col = at.col.saturating_add(1);
            let format = c
                .attr("s")
                .and_then(|s| s.trim().parse::<usize>().ok())
                .and_then(|s| styles.xfs.get(s).copied())
                .unwrap_or_default();
            let v = c.child("v").map(Node::text);
            let t = c.attr("t").unwrap_or("n");
            // Formule
            let mut input = None;
            if let Some(f) = c.child("f") {
                let text = f.text();
                let si = f.attr("si").map(str::to_string);
                if !text.trim().is_empty() {
                    let conv = from_xlsx_formula(&text);
                    if f.attr("t") == Some("shared")
                        && let Some(si) = si
                    {
                        shared.insert(
                            si,
                            SharedFormula {
                                at,
                                input: conv.clone(),
                            },
                        );
                    }
                    input = Some(conv);
                } else if let Some(m) = si.and_then(|si| shared.get(&si)) {
                    let drow = i64::from(at.row) - i64::from(m.at.row);
                    let dcol = i64::from(at.col) - i64::from(m.at.col);
                    input = Some(shift_formula(&m.input, drow as i32, dcol as i32));
                }
            }
            let mut format = format;
            let input = match input {
                Some(f) => f,
                None => match t {
                    "s" => v
                        .and_then(|i| i.trim().parse::<usize>().ok())
                        .and_then(|i| strings.get(i))
                        .map(|s| text_input(s))
                        .unwrap_or_default(),
                    "inlineStr" => c
                        .child("is")
                        .map(|n| text_input(&rich_text(n)))
                        .unwrap_or_default(),
                    "str" => v.map(|s| text_input(&s)).unwrap_or_default(),
                    "b" => v.map(|s| bool_input(s.trim() == "1")).unwrap_or_default(),
                    "e" => v.map(|s| error_input(&s)).unwrap_or_default(),
                    "d" => match v.as_deref().and_then(iso_date) {
                        Some(serial) => {
                            if format.number == NumberFormat::General {
                                format.number = NumberFormat::Date;
                            }
                            number_input(serial, format.number)
                        }
                        None => String::new(),
                    },
                    _ => match v.as_deref().and_then(parse_f64) {
                        Some(mut n) => {
                            if date1904 && format.number == NumberFormat::Date {
                                n += 1462.0;
                            }
                            number_input(n, format.number)
                        }
                        None => String::new(),
                    },
                },
            };
            fill.put(at, input, format);
        }
    }
}

pub(crate) fn from_xlsx(title: &str, bytes: &[u8]) -> Result<Workbook, String> {
    let zip = ZipReader::new(bytes).map_err(err)?;
    // Partie principale : d'après _rels/.rels, sinon l'emplacement habituel.
    let root_rels = relations(&zip, "");
    let book_path = root_rels
        .values()
        .find(|(k, _)| k.ends_with("/officeDocument"))
        .map(|(_, p)| p.clone())
        .filter(|p| zip.get(p).is_some())
        .unwrap_or_else(|| "xl/workbook.xml".into());
    let book_text = zip.text(&book_path).ok_or_else(|| {
        err("le classeur (workbook.xml) est absent ; est-ce bien un fichier .xlsx ?")
    })?;
    let book = parse(&book_text).map_err(err)?;
    if !book.is("workbook") {
        return Err(err("le classeur (workbook.xml) n'est pas reconnu"));
    }
    let date1904 = truthy(book.child("workbookPr").and_then(|p| p.attr("date1904")));
    let rels = relations(&zip, &book_path);
    let book_dir = book_path.rsplit_once('/').map_or("", |(d, _)| d);
    let part = |suffix: &str, default: &str| -> Option<String> {
        rels.values()
            .find(|(k, _)| k.ends_with(suffix))
            .map(|(_, p)| p.clone())
            .or_else(|| Some(resolve(book_dir, default)))
            .and_then(|p| zip.text(&p))
    };
    let strings = part("/sharedStrings", "sharedStrings.xml")
        .map(|t| shared_strings(&t))
        .unwrap_or_default();
    let theme = read_theme(part("/theme", "theme/theme1.xml").as_deref());
    let styles = match part("/styles", "styles.xml") {
        Some(t) => read_styles(&t, theme),
        None => Styles {
            theme,
            ..Styles::default()
        },
    };

    let entries: Vec<(String, Option<String>)> = book
        .child("sheets")
        .map(|s| {
            s.children_named("sheet")
                .map(|n| {
                    let name = n.attr("name").unwrap_or("").to_string();
                    let path = n
                        .attr("r:id")
                        .or_else(|| n.attr("id"))
                        .and_then(|id| rels.get(id))
                        .map(|(_, p)| p.clone());
                    (name, path)
                })
                .collect()
        })
        .unwrap_or_default();
    if entries.is_empty() {
        return Err(err("aucune feuille trouvée"));
    }
    let names = unique_sheet_names(&entries.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>());
    let mut sheets = Vec::new();
    let mut count = 0usize;
    let mut pending: Vec<(usize, u32, Vec<String>)> = Vec::new();
    for (i, ((_, path), name)) in entries.iter().zip(names).enumerate() {
        let mut sheet = Sheet::new(name);
        // Les feuilles de graphique (chartsheet) restent vides.
        let path = path
            .clone()
            .unwrap_or_else(|| format!("xl/worksheets/sheet{}.xml", i + 1));
        if let Some(text) = zip.text(&path) {
            let root =
                parse(&text).map_err(|e| err(format!("feuille « {} » : {e}", sheet.name)))?;
            if root.is("worksheet") {
                read_sheet(&root, &mut sheet, &strings, &styles, date1904, &mut count);
                sheet.cond_formats = cond_rules(&root, &styles);
                let (af, lists) = auto_filter(&root);
                sheet.auto_filter = af;
                for (col, shown) in lists {
                    pending.push((i, col, shown));
                }
            }
        }
        sheets.push(sheet);
    }
    let mut wb = finish(title, sheets);
    // Listes de valeurs cochées : Varium retient les valeurs décochées.
    for (i, col, shown) in pending {
        let hidden: Vec<String> = wb
            .filter_values(i, col)
            .into_iter()
            .map(|(v, _)| v)
            .filter(|v| !shown.contains(v))
            .collect();
        wb.set_column_filter(
            i,
            ColumnFilter {
                col,
                hidden,
                condition: None,
            },
        );
    }
    Ok(wb)
}

/// `<autoFilter>` : la plage, les conditions, et les listes de valeurs
/// affichées (colonne, valeurs ; `""` pour les cellules vides).
fn auto_filter(root: &Node) -> (Option<AutoFilter>, Vec<(u32, Vec<String>)>) {
    let Some(node) = root.child("autoFilter") else {
        return (None, Vec::new());
    };
    let Some(range) = node.attr("ref").and_then(Range::parse) else {
        return (None, Vec::new());
    };
    let mut columns = Vec::new();
    let mut lists = Vec::new();
    for fc in node.children_named("filterColumn") {
        let Some(id) = fc.attr("colId").and_then(|v| v.trim().parse::<u32>().ok()) else {
            continue;
        };
        let col = range.start.col.saturating_add(id);
        if col > range.end.col {
            continue;
        }
        if let Some(f) = fc.child("filters") {
            let mut shown: Vec<String> = f
                .children_named("filter")
                .filter_map(|v| v.attr("val").map(str::to_string))
                .collect();
            if truthy(f.attr("blank")) {
                shown.push(String::new());
            }
            lists.push((col, shown));
            continue;
        }
        let condition = if let Some(t) = fc.child("top10") {
            Some(FilterCondition::Top {
                n: t.attr("val")
                    .and_then(parse_f64)
                    .map_or(10, |v| v.clamp(1.0, 1e6) as u32),
                bottom: t.attr("top").is_some_and(|v| !truthy(Some(v))),
            })
        } else if let Some(d) = fc.child("dynamicFilter") {
            match d.attr("type") {
                Some("aboveAverage") => Some(FilterCondition::AboveAverage),
                Some("belowAverage") => Some(FilterCondition::BelowAverage),
                _ => None,
            }
        } else if let Some(c) = fc.child("customFilters") {
            let items: Vec<(String, String)> = c
                .children_named("customFilter")
                .map(|f| {
                    (
                        f.attr("operator").unwrap_or("equal").to_string(),
                        f.attr("val").unwrap_or("").to_string(),
                    )
                })
                .collect();
            let local = |v: &str| v.replace('.', ",");
            match items.as_slice() {
                [(op, v)] => {
                    let star_start = v.starts_with('*');
                    let star_end = v.ends_with('*') && v.len() > 1;
                    let inner = v.trim_matches('*').to_string();
                    match (op.as_str(), star_start, star_end) {
                        ("equal", true, true) => Some(FilterCondition::Contains { text: inner }),
                        ("notEqual", true, true) => {
                            Some(FilterCondition::NotContains { text: inner })
                        }
                        ("equal", false, true) => Some(FilterCondition::BeginsWith { text: inner }),
                        ("equal", true, false) => Some(FilterCondition::EndsWith { text: inner }),
                        (op, _, _) => {
                            let op = match op {
                                "greaterThan" => CompareOp::Gt,
                                "greaterThanOrEqual" => CompareOp::Ge,
                                "lessThan" => CompareOp::Lt,
                                "lessThanOrEqual" => CompareOp::Le,
                                "notEqual" => CompareOp::Ne,
                                _ => CompareOp::Eq,
                            };
                            Some(FilterCondition::Compare {
                                op,
                                value: local(v),
                                value2: String::new(),
                            })
                        }
                    }
                }
                [(a, va), (b, vb)] if a == "greaterThanOrEqual" && b == "lessThanOrEqual" => {
                    Some(FilterCondition::Compare {
                        op: CompareOp::Between,
                        value: local(va),
                        value2: local(vb),
                    })
                }
                [(a, va), (b, vb)] if a == "lessThan" && b == "greaterThan" => {
                    Some(FilterCondition::Compare {
                        op: CompareOp::NotBetween,
                        value: local(va),
                        value2: local(vb),
                    })
                }
                _ => None,
            }
        } else {
            None
        };
        if condition.is_some() {
            columns.push(ColumnFilter {
                col,
                hidden: Vec::new(),
                condition,
            });
        }
    }
    (Some(AutoFilter { range, columns }), lists)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chemins_relatifs() {
        assert_eq!(
            resolve("xl", "worksheets/sheet1.xml"),
            "xl/worksheets/sheet1.xml"
        );
        assert_eq!(resolve("xl", "/xl/styles.xml"), "xl/styles.xml");
        assert_eq!(
            resolve("xl/worksheets", "../sharedStrings.xml"),
            "xl/sharedStrings.xml"
        );
        assert_eq!(resolve("", "xl/workbook.xml"), "xl/workbook.xml");
    }

    #[test]
    fn constantes_de_regles() {
        assert_eq!(rule_constant("10").as_deref(), Some("10"));
        assert_eq!(rule_constant("1.5").as_deref(), Some("1,5"));
        assert_eq!(rule_constant("\"a\"\"b\"").as_deref(), Some("a\"b"));
        assert_eq!(rule_constant("A1"), None);
    }

    #[test]
    fn couleurs_de_theme_et_teintes() {
        let theme_xml = r#"<a:theme xmlns:a="x"><a:themeElements><a:clrScheme name="T">
<a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
<a:dk2><a:srgbClr val="1F497D"/></a:dk2><a:lt2><a:srgbClr val="EEECE1"/></a:lt2>
<a:accent1><a:srgbClr val="4F81BD"/></a:accent1></a:clrScheme></a:themeElements></a:theme>"#;
        let theme = read_theme(Some(theme_xml));
        assert_eq!(theme[3], [0x1F, 0x49, 0x7D]);
        assert_eq!(theme[4], [0x4F, 0x81, 0xBD]);
        assert_eq!(theme[5], DEFAULT_THEME[5]);
        let node = |xml: &str| parse(xml).unwrap();
        assert_eq!(
            color_of(Some(&node(r#"<color theme="4"/>"#)), &theme),
            Some([0x4F, 0x81, 0xBD])
        );
        // Accent 1 éclairci à 80 % (fond bleu pâle d'Excel) : DCE6F1.
        let pale = color_of(
            Some(&node(r#"<color theme="4" tint="0.7999816888943144"/>"#)),
            &theme,
        )
        .unwrap();
        assert!(
            pale.iter()
                .zip([0xDC, 0xE6, 0xF1])
                .all(|(a, b)| a.abs_diff(b) <= 1),
            "{pale:?}"
        );
        // Assombri de 25 % : 366092.
        let dark = color_of(
            Some(&node(r#"<color theme="4" tint="-0.249977111117893"/>"#)),
            &theme,
        )
        .unwrap();
        assert!(
            dark.iter()
                .zip([0x36, 0x60, 0x92])
                .all(|(a, b)| a.abs_diff(b) <= 1),
            "{dark:?}"
        );
        assert_eq!(
            color_of(Some(&node(r#"<color indexed="10"/>"#)), &theme),
            Some([0xFF, 0, 0])
        );
        assert_eq!(
            color_of(Some(&node(r#"<color theme="99"/>"#)), &theme),
            None
        );
        assert_eq!(color_of(Some(&node(r#"<color auto="1"/>"#)), &theme), None);
        assert_eq!(read_theme(Some("pas du xml")), DEFAULT_THEME.to_vec());
    }
}
