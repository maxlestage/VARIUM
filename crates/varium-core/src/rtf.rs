//! Export RTF (texte enrichi), lisible par Pages, TextEdit, Word et LibreOffice.

use crate::doc::{Align, Couleur, Interligne, Para, ParaFormat, Run};
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

/// Table des couleurs : noir (1), rouge des champs manquants (2), puis celles du texte.
struct Couleurs(Vec<Couleur>);

impl Couleurs {
    fn indice(&mut self, c: Couleur) -> usize {
        match self.0.iter().position(|x| *x == c) {
            Some(i) => i + 3,
            None => {
                self.0.push(c);
                self.0.len() + 2
            }
        }
    }

    fn table(&self) -> String {
        let mut s = String::from("{\\colortbl;\\red0\\green0\\blue0;\\red161\\green32\\blue27;");
        for c in &self.0 {
            s.push_str(&format!("\\red{}\\green{}\\blue{};", c.r, c.g, c.b));
        }
        s.push('}');
        s
    }
}

/// Une image PNG en RTF, de `l` × `h` millimètres.
fn image(png: &[u8], l: f32, h: f32) -> String {
    let hex: String = png.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{{\\pict\\pngblip\\picwgoal{}\\pichgoal{} {hex}}}",
        tw(l),
        tw(h)
    )
}

fn runs(rs: &[Run], d: &Rendered, c: &mut Couleurs) -> String {
    let mut out = String::new();
    for r in rs {
        if let Some(k) = r.note() {
            out.push_str(&format!(
                "{{\\super\\chftn}}{{\\footnote\\pard\\plain{{\\super\\chftn}} {}}}",
                runs(d.note(k), d, c)
            ));
            continue;
        }
        if let Some(nom) = r.signature() {
            if let Some(s) = d.signatures.get(nom) {
                let (l, h) = s.taille_mm();
                out.push_str(&image(&s.png(120), l, h));
            } else {
                out.push_str("____________________");
            }
            continue;
        }
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
        } else if let Some(k) = st.couleur {
            open.push_str(&format!("\\cf{}", c.indice(k)));
        }
        if let Some(k) = st.surlignage {
            let i = c.indice(k);
            open.push_str(&format!("\\chcbpat{i}\\highlight{i}"));
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

/// Retraits, interligne et espacements d'un paragraphe (vide sans mise en forme).
fn format_para(f: &ParaFormat) -> String {
    let mut s = String::new();
    if let Some(v) = f.gauche {
        s.push_str(&format!("\\li{}", tw(v)));
    }
    if let Some(v) = f.droite {
        s.push_str(&format!("\\ri{}", tw(v)));
    }
    if let Some(v) = f.alinea {
        s.push_str(&format!("\\fi{}", tw(v)));
    }
    if let Some(v) = f.avant {
        s.push_str(&format!("\\sb{}", (v * 20.0).round() as i32));
    }
    if let Some(v) = f.apres {
        s.push_str(&format!("\\sa{}", (v * 20.0).round() as i32));
    }
    s
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
    let mut c = Couleurs(Vec::new());
    let vide = Rendered::default();
    // Interligne : celui du paragraphe, sinon celui du document.
    let interligne = |f: &ParaFormat| match f.interligne {
        Some(Interligne::Multiple(m)) => format!("\\sl{}\\slmult1", (240.0 * m).round() as i32),
        Some(Interligne::Exact(p)) => format!("\\sl-{}\\slmult0", (p * 20.0).round() as i32),
        None => format!("\\sl{line}\\slmult1"),
    };
    let para = |f: &ParaFormat, extra: &str, content: &str| {
        format!(
            "\\pard\\plain{}{extra}{}\\f0\\fs{size} {content}\\par\n",
            interligne(f),
            format_para(f)
        )
    };
    let text_w = tw(page.text_width_mm());
    let mut marges = String::new();
    if let Some(d) = docs.first() {
        if !d.header.is_empty() {
            marges.push_str(&format!(
                "{{\\header\\pard\\qc\\f0\\fs{} {}\\par}}\n",
                size * 3 / 4,
                runs(&d.header, &vide, &mut c)
            ));
        }
        if !d.footer.is_empty() || page.page_numbers {
            let numbers = if page.page_numbers {
                "\\tqr\\tx{w}\\tab Page {\\field{\\*\\fldinst PAGE}{\\fldrslt 1}} sur {\\field{\\*\\fldinst NUMPAGES}{\\fldrslt 1}}"
                    .replace("{w}", &text_w.to_string())
            } else {
                String::new()
            };
            marges.push_str(&format!(
                "{{\\footer\\pard\\f0\\fs{} {}{numbers}\\par}}\n",
                size * 3 / 4,
                runs(&d.footer, &vide, &mut c)
            ));
        }
    }
    let mut corps = String::new();
    for (i, d) in docs.iter().enumerate() {
        if i > 0 {
            corps.push_str("\\page\n");
        }
        for (pi, p) in d.paras.iter().enumerate() {
            let f = d.format(pi);
            match p {
                Para::Text { align: a, runs: r } => {
                    let al = if *a == Align::Left {
                        body_align
                    } else {
                        align(*a)
                    };
                    let contenu = runs(r, d, &mut c);
                    corps.push_str(&para(&f, al, &contenu));
                }
                Para::Heading { level, runs: r } => {
                    let hs =
                        (size as f32 * [1.6, 1.35, 1.15][(*level as usize).clamp(1, 3) - 1]) as i32;
                    corps.push_str(&format!(
                        "\\pard\\plain\\sb240\\sa120\\keepn\\outlinelevel{}{}\\f0\\fs{hs}\\b {}\\b0\\par\n",
                        (*level as i32 - 1).clamp(0, 8),
                        format_para(&f),
                        runs(r, d, &mut c)
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
                    let left = 360 * (*depth as i32 + 1) + f.gauche.map_or(0, tw);
                    let f2 = ParaFormat { gauche: None, ..f };
                    let contenu = runs(r, d, &mut c);
                    corps.push_str(&para(
                        &f2,
                        &format!("\\li{left}\\fi-360\\tx{left}"),
                        &format!("{label}\\tab {contenu}"),
                    ));
                }
                Para::Quote { runs: r } => {
                    let contenu = runs(r, d, &mut c);
                    corps.push_str(&para(&f, "\\li567\\ri567\\i", &contenu))
                }
                Para::Math { .. } => corps.push_str(&para(&f, "\\qc\\i", &esc(&p.plain()))),
                Para::Table(t) => {
                    let cols = t.columns().max(1) as i32;
                    for (ri, row) in t.rows.iter().enumerate() {
                        corps.push_str("\\trowd\\trgaph80");
                        for k in 1..=cols {
                            corps.push_str(&format!(
                                "\\clbrdrt\\brdrs\\clbrdrl\\brdrs\\clbrdrb\\brdrs\\clbrdrr\\brdrs\\cellx{}",
                                text_w * k / cols
                            ));
                        }
                        for k in 0..cols as usize {
                            let cell = row.get(k).map(|r| runs(r, d, &mut c)).unwrap_or_default();
                            let b = if ri == 0 && t.header { "\\b" } else { "" };
                            corps.push_str(&format!(
                                "\\pard\\plain\\intbl{}{b}\\f0\\fs{size} {cell}\\cell ",
                                align(t.align(k))
                            ));
                        }
                        corps.push_str("\\row\n");
                    }
                }
                Para::Rule => corps.push_str("\\pard\\brdrb\\brdrs\\brdrw10\\brsp20 \\par\n"),
                Para::PageBreak => corps.push_str("\\page\n"),
            }
        }
    }
    let auteur = if page.auteur.trim().is_empty() {
        String::new()
    } else {
        format!("{{\\info{{\\author {}}}}}", esc(page.auteur.trim()))
    };
    format!(
        "{{\\rtf1\\ansi\\ansicpg1252\\deff0{{\\fonttbl{{\\f0{font}}}}}{}{auteur}\n\\paperw{}\\paperh{}\\margt{}\\margb{}\\margl{}\\margr{}{}\n{marges}{corps}}}",
        c.table(),
        tw(w),
        tw(h),
        tw(page.margin_top),
        tw(page.margin_bottom),
        tw(page.margin_left),
        tw(page.margin_right),
        if page.landscape { "\\landscape" } else { "" },
    )
}
