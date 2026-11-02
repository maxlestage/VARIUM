//! Exports : texte, HTML (formules via KaTeX) et LaTeX.

use crate::render::{Block, Missing, Piece, Values, blocks, render, render_pieces};

pub fn to_text(src: &str, values: &Values) -> String {
    render(src, values, Missing::Keep)
}

fn html_escape(s: &str) -> String {
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

const KATEX: &str = "https://cdn.jsdelivr.net/npm/katex@0.16.11/dist";

/// Page HTML autonome. Les formules sont composées par KaTeX à l'ouverture.
pub fn to_html(title: &str, src: &str, values: &Values) -> String {
    let mut body = String::new();
    for block in blocks(render_pieces(src, values)) {
        match block {
            Block::Prose(pieces) => {
                for p in pieces {
                    match p {
                        Piece::Text(t) => body.push_str(&html_escape(&t)),
                        Piece::Value { name, text } => body.push_str(&format!(
                            "<span class=\"v\" data-var=\"{}\">{}</span>",
                            html_escape(&name),
                            html_escape(&text)
                        )),
                        Piece::Missing { raw, .. } => {
                            body.push_str(&format!("<mark>{}</mark>", html_escape(&raw)))
                        }
                    }
                }
            }
            Block::Math { display, pieces } => {
                let tex: String = pieces.iter().map(Piece::text).collect();
                let (o, c) = if display {
                    ("\\[", "\\]")
                } else {
                    ("\\(", "\\)")
                };
                body.push_str(&format!("{o}{}{c}", html_escape(&tex)));
            }
        }
    }
    let title = html_escape(title);
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
  body {{ background:#f3f1ec; margin:0; font:17px/1.6 Georgia, 'Times New Roman', serif; color:#1d1b18; }}
  main {{ max-width:42rem; margin:2rem auto; background:#fff; padding:3rem 3.5rem; white-space:pre-wrap;
         box-shadow:0 1px 3px rgba(0,0,0,.12); }}
  mark {{ background:#fde2e1; color:#a1201b; }}
  @media print {{ body {{ background:#fff; }} main {{ box-shadow:none; margin:0; padding:0; }} }}
</style>
</head>
<body>
<main>{body}</main>
</body>
</html>
"#
    )
}

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

/// Retours à la ligne simples → `\\`, lignes vides → nouveau paragraphe.
fn tex_lines(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' {
            if chars.peek() == Some(&'\n') {
                while chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push_str("\n\n");
            } else {
                out.push_str("\\\\\n");
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Document LaTeX complet, compilable avec pdflatex / lualatex / xelatex.
pub fn to_latex(title: &str, src: &str, values: &Values) -> String {
    let mut body = String::new();
    for block in blocks(render_pieces(src, values)) {
        match block {
            Block::Prose(pieces) => {
                let s: String = pieces
                    .iter()
                    .map(|p| match p {
                        Piece::Missing { raw, .. } => format!("\\fbox{{{}}}", tex_escape(raw)),
                        _ => tex_escape(p.text()),
                    })
                    .collect();
                body.push_str(&tex_lines(&s));
            }
            Block::Math { display, pieces } => {
                let tex: String = pieces
                    .iter()
                    .map(|p| match p {
                        Piece::Missing { raw, .. } => format!("\\text{{{}}}", tex_escape(raw)),
                        _ => p.text().to_string(),
                    })
                    .collect();
                if display {
                    body.push_str(&format!("\n\\[\n{}\n\\]\n", tex.trim()));
                } else {
                    body.push_str(&format!("${tex}$"));
                }
            }
        }
    }
    format!(
        "% Généré par Varium\n\\documentclass[11pt,a4paper]{{article}}\n\\usepackage[utf8]{{inputenc}}\n\\usepackage[T1]{{fontenc}}\n\\usepackage[french]{{babel}}\n\\usepackage{{amsmath,amssymb}}\n\\usepackage{{lmodern}}\n\\usepackage[margin=2.5cm]{{geometry}}\n\\setlength{{\\parindent}}{{0pt}}\n\\setlength{{\\parskip}}{{0.8em}}\n\\title{{{}}}\n\\date{{}}\n\n\\begin{{document}}\n{}\n\\end{{document}}\n",
        tex_escape(title),
        body.trim()
    )
}
