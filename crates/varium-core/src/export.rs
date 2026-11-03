//! Exports : HTML (imprimable en PDF), LaTeX, texte. ODT et DOCX : voir `odt` et `docx`.
//!
//! Tous les exports partent du document mis en forme (`doc`) et de la mise en page
//! (`page`). Ils acceptent plusieurs documents à la suite (publipostage) : chacun
//! commence sur une nouvelle page.

use crate::doc::{self, Align, Para, Run, Style, Table};
use crate::page::{DocFont, PageSetup};
use crate::render::Values;

/// Un document prêt à exporter : son contenu, son en-tête et son pied de page.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub paras: Vec<Para>,
    pub header: Vec<Run>,
    pub footer: Vec<Run>,
}

/// Met en forme la source avec ses valeurs, en-tête et pied de page compris.
pub fn prepare(src: &str, values: &Values, page: &PageSetup) -> Rendered {
    Rendered {
        paras: doc::document(src, values),
        header: doc::inline(&page.header, values),
        footer: doc::inline(&page.footer, values),
    }
}

pub(crate) fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Nombre français avec virgule, pour les dimensions CSS (`12.5` → `12.5`).
fn num(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

// ---------------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------------

const KATEX: &str = "https://cdn.jsdelivr.net/npm/katex@0.16.11/dist";

fn html_runs(runs: &[Run]) -> String {
    let mut out = String::new();
    for r in runs {
        let inner = match r {
            Run::Text { text, .. } => xml_escape(text),
            Run::Value { name, text, .. } => format!(
                "<span class=\"v\" data-var=\"{}\">{}</span>",
                xml_escape(name),
                xml_escape(text)
            ),
            Run::Missing { raw, .. } => format!("<mark>{}</mark>", xml_escape(raw)),
            Run::Math { tex, .. } => format!("\\({}\\)", xml_escape(tex)),
        };
        let s = r.style();
        let mut wrapped = inner;
        if s.bold {
            wrapped = format!("<strong>{wrapped}</strong>");
        }
        if s.italic {
            wrapped = format!("<em>{wrapped}</em>");
        }
        if s.underline {
            wrapped = format!("<u>{wrapped}</u>");
        }
        if s.strike {
            wrapped = format!("<s>{wrapped}</s>");
        }
        out.push_str(&wrapped);
    }
    out
}

fn align_class(a: Align) -> &'static str {
    match a {
        Align::Left => "",
        Align::Center => " class=\"c\"",
        Align::Right => " class=\"r\"",
        Align::Justify => " class=\"j\"",
    }
}

fn html_table(t: &Table) -> String {
    let mut s = String::from("<table>");
    for (i, row) in t.rows.iter().enumerate() {
        s.push_str("<tr>");
        let tag = if i == 0 && t.header { "th" } else { "td" };
        for col in 0..t.columns() {
            let cell = row.get(col).map(|c| html_runs(c)).unwrap_or_default();
            s.push_str(&format!(
                "<{tag}{}>{cell}</{tag}>",
                align_class(t.align(col))
            ));
        }
        s.push_str("</tr>");
    }
    s.push_str("</table>");
    s
}

fn html_paras(paras: &[Para], out: &mut String, header: &str, footer: &str) {
    let open = |out: &mut String| {
        out.push_str("<section class=\"page\">");
        if !header.is_empty() {
            out.push_str(&format!("<div class=\"entete\">{header}</div>"));
        }
    };
    let close = |out: &mut String| {
        if !footer.is_empty() {
            out.push_str(&format!("<div class=\"pied\">{footer}</div>"));
        }
        out.push_str("</section>\n");
    };
    open(out);
    for p in paras {
        let html = match p {
            Para::Text { align, runs } if runs.is_empty() => {
                format!("<p{}><br></p>", align_class(*align))
            }
            Para::Text { align, runs } => {
                format!("<p{}>{}</p>", align_class(*align), html_runs(runs))
            }
            Para::Heading { level, runs } => format!("<h{level}>{}</h{level}>", html_runs(runs)),
            Para::Item {
                number,
                depth,
                runs,
            } => {
                let label = match number {
                    Some(n) => format!("{n}."),
                    None => ["•", "◦", "▪"][*depth as usize % 3].to_string(),
                };
                format!(
                    "<p class=\"li\" style=\"margin-left:{}em\"><span class=\"puce\">{label}</span>{}</p>",
                    1.6 * (*depth as f32 + 1.0),
                    html_runs(runs)
                )
            }
            Para::Quote { runs } => format!("<blockquote>{}</blockquote>", html_runs(runs)),
            Para::Math { tex } => format!("<p class=\"c\">\\[{}\\]</p>", xml_escape(tex.trim())),
            Para::Table(t) => html_table(t),
            Para::Rule => "<hr>".to_string(),
            Para::PageBreak => {
                close(out);
                open(out);
                continue;
            }
        };
        out.push_str(&html);
        out.push('\n');
    }
    close(out);
}

fn css_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Une page HTML autonome, mise en page pour l'écran et pour l'impression (PDF).
/// Les formules sont composées par KaTeX à l'ouverture.
pub fn html(title: &str, docs: &[Rendered], page: &PageSetup) -> String {
    let mut body = String::new();
    for d in docs {
        let header = html_runs(&d.header);
        let footer = html_runs(&d.footer);
        html_paras(&d.paras, &mut body, &header, &footer);
    }
    let (w, h) = page.size_mm();
    let first = docs.first();
    let header_txt = first.map(|d| doc::plain(&d.header)).unwrap_or_default();
    let footer_txt = first.map(|d| doc::plain(&d.footer)).unwrap_or_default();
    let mut margin_boxes = String::new();
    if !header_txt.is_empty() {
        margin_boxes.push_str(&format!(
            "@top-center {{ content: {}; }}",
            css_string(&header_txt)
        ));
    }
    if !footer_txt.is_empty() {
        margin_boxes.push_str(&format!(
            "@bottom-left {{ content: {}; }}",
            css_string(&footer_txt)
        ));
    }
    if page.page_numbers {
        margin_boxes.push_str(
            "@bottom-right { content: \"Page \" counter(page) \" sur \" counter(pages); }",
        );
    }
    let numbers_screen = if page.page_numbers {
        ".page { counter-increment: p; } .page::after { content: \"Page \" counter(p); position: absolute; right: var(--mr); bottom: 8mm; font-size: 9pt; color: #555; }"
    } else {
        ""
    };
    let title = xml_escape(title);
    let font = page.font.css();
    let size = num(page.font_size);
    let lh = num(page.line_spacing * 1.15);
    let (mt, mb, ml, mr) = (
        num(page.margin_top),
        num(page.margin_bottom),
        num(page.margin_left),
        num(page.margin_right),
    );
    let justify = if page.justify { "justify" } else { "left" };
    let orientation = if page.landscape {
        "landscape"
    } else {
        "portrait"
    };
    let paper = page.paper.css();
    let (w, h) = (num(w), num(h));
    format!(
        r#"<!doctype html>
<html lang="fr">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="Varium">
<title>{title}</title>
<link rel="stylesheet" href="{KATEX}/katex.min.css">
<script defer src="{KATEX}/katex.min.js"></script>
<script defer src="{KATEX}/contrib/auto-render.min.js" onload="renderMathInElement(document.body)"></script>
<style>
  @page {{ size: {paper} {orientation}; margin: {mt}mm {mr}mm {mb}mm {ml}mm; {margin_boxes} }}
  :root {{ --mr: {mr}mm; }}
  body {{ background: #e8e6e1; margin: 0; padding: 1px 0; color: #111; counter-reset: p;
         font-family: {font}; font-size: {size}pt; line-height: {lh}; text-align: {justify}; }}
  .page {{ position: relative; box-sizing: border-box; width: {w}mm; min-height: {h}mm; margin: 12mm auto;
          padding: {mt}mm {mr}mm {mb}mm {ml}mm; background: #fff; box-shadow: 0 1px 4px rgba(0,0,0,.18); }}
  .entete, .pied {{ position: absolute; left: {ml}mm; right: {mr}mm; font-size: .8em; color: #444; }}
  .entete {{ top: 8mm; }} .pied {{ bottom: 8mm; }}
  {numbers_screen}
  p, blockquote, td, th {{ margin: 0; white-space: pre-wrap; }}
  h1, h2, h3 {{ line-height: 1.25; margin: .5em 0 .25em; text-align: left; }}
  h1 {{ font-size: 1.65em; }} h2 {{ font-size: 1.35em; }} h3 {{ font-size: 1.12em; }}
  .c {{ text-align: center; }} .r {{ text-align: right; }} .j {{ text-align: justify; }}
  .li {{ text-indent: -1.2em; padding-left: 1.2em; }} .li .puce {{ display: inline-block; width: 1.2em; text-indent: 0; }}
  blockquote {{ margin: .3em 0 .3em 1.2em; padding-left: .9em; border-left: 3px solid #bbb; color: #333; font-style: italic; }}
  table {{ border-collapse: collapse; width: 100%; margin: .4em 0; }}
  th, td {{ border: .5pt solid #444; padding: .2em .45em; vertical-align: top; text-align: left; }}
  th {{ background: #f0eee9; }}
  th.c, td.c {{ text-align: center; }} th.r, td.r {{ text-align: right; }}
  hr {{ border: 0; border-top: .5pt solid #444; margin: .5em 0; }}
  mark {{ background: #fde2e1; color: #a1201b; }}
  @media print {{
    body {{ background: #fff; padding: 0; }}
    .page {{ width: auto; min-height: 0; margin: 0; padding: 0; box-shadow: none; break-after: page; }}
    .page:last-child {{ break-after: auto; }}
    .entete, .pied, .page::after {{ display: none; }}
  }}
  @media screen and (max-width: {w}mm) {{ .page {{ width: auto; min-height: 0; margin: 0 0 8px; padding: 6vw; }} }}
</style>
</head>
<body>
{body}</body>
</html>
"#
    )
}

/// Page HTML d'un seul document (compatibilité).
pub fn to_html(title: &str, src: &str, values: &Values) -> String {
    let page = PageSetup::default();
    html(title, &[prepare(src, values, &page)], &page)
}

// ---------------------------------------------------------------------------------
// LaTeX
// ---------------------------------------------------------------------------------

fn tex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '\u{202F}' => out.push_str("\\,"),
            '\u{00A0}' => out.push('~'),
            _ => out.push(c),
        }
    }
    out
}

fn tex_runs(runs: &[Run]) -> String {
    let mut out = String::new();
    for r in runs {
        let mut s = match r {
            Run::Text { text, .. } => tex_escape(text),
            // Une valeur sur plusieurs lignes (adresse…) garde ses retours à la ligne.
            Run::Value { text, .. } => tex_escape(text).replace('\n', "\\\\\n"),
            Run::Missing { raw, .. } => format!("\\fbox{{{}}}", tex_escape(raw)),
            Run::Math { tex, .. } => format!("${tex}$"),
        };
        let st: Style = r.style();
        if st.bold {
            s = format!("\\textbf{{{s}}}");
        }
        if st.italic {
            s = format!("\\textit{{{s}}}");
        }
        if st.underline {
            s = format!("\\uline{{{s}}}");
        }
        if st.strike {
            s = format!("\\sout{{{s}}}");
        }
        out.push_str(&s);
    }
    out
}

fn tex_paras(paras: &[Para], out: &mut String) {
    for p in paras {
        match p {
            Para::Text { runs, .. } if runs.is_empty() => {
                out.push_str("\\vspace{\\baselineskip}\n")
            }
            Para::Text { align, runs } => {
                let body = tex_runs(runs);
                out.push_str(&match align {
                    Align::Center => format!("{{\\centering {body}\\par}}\n"),
                    Align::Right => format!("{{\\raggedleft {body}\\par}}\n"),
                    _ => format!("{body}\\par\n"),
                });
            }
            Para::Heading { level, runs } => {
                let cmd = ["section", "subsection", "subsubsection"][(*level as usize - 1).min(2)];
                out.push_str(&format!("\\{cmd}*{{{}}}\n", tex_runs(runs)));
            }
            Para::Item {
                number,
                depth,
                runs,
            } => {
                let label = match number {
                    Some(n) => format!("{n}."),
                    None => "\\textbullet".into(),
                };
                let indent = 1.5 * (*depth as f32 + 1.0);
                out.push_str(&format!(
                    "\\par\\hangindent={}em\\hangafter=1\\noindent\\hspace*{{{}em}}\\makebox[1.5em][l]{{{label}}}{}\\par\n",
                    num(indent),
                    num(indent - 1.5),
                    tex_runs(runs)
                ));
            }
            Para::Quote { runs } => out.push_str(&format!(
                "\\begin{{quote}}\\itshape {}\\end{{quote}}\n",
                tex_runs(runs)
            )),
            Para::Math { tex } => out.push_str(&format!("\\[\n{}\n\\]\n", tex.trim())),
            Para::Table(t) => {
                let cols: String = (0..t.columns())
                    .map(|c| match t.align(c) {
                        Align::Center => "c|",
                        Align::Right => "r|",
                        _ => "l|",
                    })
                    .collect();
                out.push_str(&format!(
                    "\\par\\noindent\\begin{{tabular}}{{|{cols}}}\\hline\n"
                ));
                for (i, row) in t.rows.iter().enumerate() {
                    let cells: Vec<String> = (0..t.columns())
                        .map(|c| {
                            let s = row.get(c).map(|r| tex_runs(r)).unwrap_or_default();
                            if i == 0 && t.header {
                                format!("\\textbf{{{s}}}")
                            } else {
                                s
                            }
                        })
                        .collect();
                    out.push_str(&format!("{} \\\\ \\hline\n", cells.join(" & ")));
                }
                out.push_str("\\end{tabular}\\par\n");
            }
            Para::Rule => out.push_str("\\par\\noindent\\rule{\\linewidth}{0.4pt}\\par\n"),
            Para::PageBreak => out.push_str("\\newpage\n"),
        }
    }
}

/// Document LaTeX complet, compilable avec pdflatex, lualatex ou xelatex.
pub fn latex(title: &str, docs: &[Rendered], page: &PageSetup) -> String {
    let mut body = String::new();
    for (i, d) in docs.iter().enumerate() {
        if i > 0 {
            body.push_str("\\newpage\n");
        }
        tex_paras(&d.paras, &mut body);
    }
    let size = match page.font_size {
        s if s < 10.5 => 10,
        s if s < 11.5 => 11,
        _ => 12,
    };
    let paper = match page.paper {
        crate::page::Paper::A4 => "a4paper",
        crate::page::Paper::A5 => "a5paper",
        crate::page::Paper::Letter => "letterpaper",
        crate::page::Paper::Legal => "legalpaper",
    };
    let landscape = if page.landscape { ",landscape" } else { "" };
    let font = match page.font {
        DocFont::Serif => "\\usepackage{mathptmx}",
        DocFont::Sans => "\\usepackage{helvet}\n\\renewcommand{\\familydefault}{\\sfdefault}",
    };
    let ragged = if page.justify { "" } else { "\\raggedright\n" };
    let first = docs.first();
    let header = first.map(|d| tex_runs(&d.header)).unwrap_or_default();
    let footer = first.map(|d| tex_runs(&d.footer)).unwrap_or_default();
    let numbers = if page.page_numbers {
        "Page \\thepage{} sur \\pageref{LastPage}"
    } else {
        ""
    };
    format!(
        "% Généré par Varium\n\\documentclass[{size}pt,{paper}{landscape}]{{article}}\n\\usepackage[utf8]{{inputenc}}\n\\usepackage[T1]{{fontenc}}\n\\usepackage[french]{{babel}}\n\\usepackage{{amsmath,amssymb}}\n{font}\n\\usepackage[top={}mm,bottom={}mm,left={}mm,right={}mm]{{geometry}}\n\\usepackage{{setspace}}\n\\setstretch{{{}}}\n\\usepackage[normalem]{{ulem}}\n\\usepackage{{fancyhdr,lastpage}}\n\\pagestyle{{fancy}}\n\\fancyhf{{}}\n\\renewcommand{{\\headrulewidth}}{{0pt}}\n\\chead{{{header}}}\n\\lfoot{{{footer}}}\n\\rfoot{{{numbers}}}\n\\setlength{{\\parindent}}{{0pt}}\n\\setlength{{\\parskip}}{{0pt}}\n\\title{{{}}}\n\\date{{}}\n\n\\begin{{document}}\n{ragged}{}\n\\end{{document}}\n",
        num(page.margin_top),
        num(page.margin_bottom),
        num(page.margin_left),
        num(page.margin_right),
        num(page.line_spacing),
        tex_escape(title),
        body.trim()
    )
}

/// Document LaTeX d'une source (compatibilité).
pub fn to_latex(title: &str, src: &str, values: &Values) -> String {
    let page = PageSetup::default();
    latex(title, &[prepare(src, values, &page)], &page)
}

// ---------------------------------------------------------------------------------
// Texte
// ---------------------------------------------------------------------------------

/// Le document en texte brut (puces et numéros conservés, sauts de page en `\f`).
pub fn text(docs: &[Rendered]) -> String {
    let mut out = String::new();
    for (i, d) in docs.iter().enumerate() {
        if i > 0 {
            out.push('\u{000C}');
        }
        for p in &d.paras {
            match p {
                Para::Item {
                    number,
                    depth,
                    runs,
                } => {
                    out.push_str(&"  ".repeat(*depth as usize));
                    match number {
                        Some(n) => out.push_str(&format!("{n}. ")),
                        None => out.push_str("• "),
                    }
                    out.push_str(&doc::plain(runs));
                }
                Para::Quote { runs } => {
                    out.push_str("    ");
                    out.push_str(&doc::plain(runs));
                }
                Para::Rule => out.push_str(&"─".repeat(40)),
                Para::PageBreak => out.push('\u{000C}'),
                other => out.push_str(&other.plain()),
            }
            out.push('\n');
        }
    }
    out
}

/// Texte d'une source (compatibilité).
pub fn to_text(src: &str, values: &Values) -> String {
    let page = PageSetup::default();
    text(&[prepare(src, values, &page)])
}
