//! Export RTF (texte enrichi), lisible par Pages, TextEdit, Word et LibreOffice.

use crate::doc::{Align, Para, Run};
use crate::export::Rendered;
use crate::page::{DocFont, PageSetup};

/// Texte RTF : caractères spéciaux échappés, non-ASCII en `\uN?`.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\line "),
            '\t' => out.push_str("\\tab "),
            c if c.is_ascii() => out.push(c),
            c => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{}?", *unit as i16));
                }
            }
        }
    }
    out
}

fn runs(rs: &[Run]) -> String {
    let mut out = String::new();
    for r in rs {
        let st = r.style();
        let italic = st.italic || matches!(r, Run::Math { .. });
        let mut open = String::from("{");
        if st.bold {
            open.push_str("\\b");
        }
        if italic {
            open.push_str("\\i");
        }
        if st.underline {
            open.push_str("\\ul");
        }
        if st.strike {
            open.push_str("\\strike");
        }
        if matches!(r, Run::Missing { .. }) {
            open.push_str("\\cf2");
        }
        if open.len() > 1 {
            open.push(' ');
        }
        let text = match r {
            Run::Text { text, .. } | Run::Value { text, .. } => text.clone(),
            Run::Missing { raw, .. } => raw.clone(),
            Run::Math { .. } => r.plain(),
        };
        out.push_str(&format!("{open}{}}}", esc(&text)));
    }
    out
}

fn align(a: Align) -> &'static str {
    match a {
        Align::Left => "\\ql",
        Align::Center => "\\qc",
        Align::Right => "\\qr",
        Align::Justify => "\\qj",
    }
}

/// Millimètres → twips.
fn tw(mm: f32) -> i32 {
    (mm * 56.692_9).round() as i32
}

/// Le document RTF.
pub fn rtf(docs: &[Rendered], page: &PageSetup) -> String {
    let size = (page.font_size * 2.0).round() as i32;
    let line = (240.0 * page.line_spacing).round() as i32;
    let body_align = if page.justify { "\\qj" } else { "\\ql" };
    let (w, h) = page.size_mm();
    let font = match page.font {
        DocFont::Serif => "\\froman Liberation Serif;",
        DocFont::Sans => "\\fswiss Liberation Sans;",
    };
    let mut s = format!(
        "{{\\rtf1\\ansi\\ansicpg1252\\deff0{{\\fonttbl{{\\f0{font}}}}}{{\\colortbl;\\red0\\green0\\blue0;\\red161\\green32\\blue27;}}\n\\paperw{}\\paperh{}\\margt{}\\margb{}\\margl{}\\margr{}{}\n",
        tw(w),
        tw(h),
        tw(page.margin_top),
        tw(page.margin_bottom),
        tw(page.margin_left),
        tw(page.margin_right),
        if page.landscape { "\\landscape" } else { "" },
    );
    let para = |extra: &str, content: &str| {
        format!("\\pard\\sl{line}\\slmult1{extra}\\f0\\fs{size} {content}\\par\n")
    };
    let text_w = tw(page.text_width_mm());
    if let Some(d) = docs.first() {
        if !d.header.is_empty() {
            s.push_str(&format!(
                "{{\\header\\pard\\qc\\f0\\fs{} {}\\par}}\n",
                size * 3 / 4,
                runs(&d.header)
            ));
        }
        if !d.footer.is_empty() || page.page_numbers {
            let numbers = if page.page_numbers {
                "\\tqr\\tx{w}\\tab Page {\\field{\\*\\fldinst PAGE}{\\fldrslt 1}} sur {\\field{\\*\\fldinst NUMPAGES}{\\fldrslt 1}}"
                    .replace("{w}", &text_w.to_string())
            } else {
                String::new()
            };
            s.push_str(&format!(
                "{{\\footer\\pard\\f0\\fs{} {}{numbers}\\par}}\n",
                size * 3 / 4,
                runs(&d.footer)
            ));
        }
    }
    for (i, d) in docs.iter().enumerate() {
        if i > 0 {
            s.push_str("\\page\n");
        }
        for p in &d.paras {
            match p {
                Para::Text { align: a, runs: r } => {
                    let al = if *a == Align::Left {
                        body_align
                    } else {
                        align(*a)
                    };
                    s.push_str(&para(al, &runs(r)));
                }
                Para::Heading { level, runs: r } => {
                    let hs =
                        (size as f32 * [1.6, 1.35, 1.15][(*level as usize).clamp(1, 3) - 1]) as i32;
                    s.push_str(&format!(
                        "\\pard\\sb240\\sa120\\keepn\\f0\\fs{hs}\\b {}\\b0\\par\n",
                        runs(r)
                    ));
                }
                Para::Item {
                    number,
                    depth,
                    runs: r,
                } => {
                    let label = match number {
                        Some(n) => format!("{n}."),
                        None => "\\u8226?".into(),
                    };
                    let left = 360 * (*depth as i32 + 1);
                    s.push_str(&para(
                        &format!("\\li{left}\\fi-360\\tx{left}"),
                        &format!("{label}\\tab {}", runs(r)),
                    ));
                }
                Para::Quote { runs: r } => s.push_str(&para("\\li567\\ri567\\i", &runs(r))),
                Para::Math { .. } => s.push_str(&para("\\qc\\i", &esc(&p.plain()))),
                Para::Table(t) => {
                    let cols = t.columns().max(1) as i32;
                    for (ri, row) in t.rows.iter().enumerate() {
                        s.push_str("\\trowd\\trgaph80");
                        for c in 1..=cols {
                            s.push_str(&format!(
                                "\\clbrdrt\\brdrs\\clbrdrl\\brdrs\\clbrdrb\\brdrs\\clbrdrr\\brdrs\\cellx{}",
                                text_w * c / cols
                            ));
                        }
                        for c in 0..cols as usize {
                            let cell = row.get(c).map(|r| runs(r)).unwrap_or_default();
                            let b = if ri == 0 && t.header { "\\b" } else { "" };
                            s.push_str(&format!(
                                "\\pard\\intbl{}{b}\\f0\\fs{size} {cell}\\cell ",
                                align(t.align(c))
                            ));
                        }
                        s.push_str("\\row\n");
                    }
                }
                Para::Rule => s.push_str("\\pard\\brdrb\\brdrs\\brdrw10\\brsp20 \\par\n"),
                Para::PageBreak => s.push_str("\\page\n"),
            }
        }
    }
    s.push('}');
    s
}
