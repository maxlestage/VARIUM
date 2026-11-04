//! Exports : HTML (imprimable en PDF), LaTeX, texte. ODT et DOCX : voir `odt` et `docx`.
//!
//! Tous les exports partent du document mis en forme (`doc`) et de la mise en page
//! (`page`). Ils acceptent plusieurs documents à la suite (publipostage) : chacun
//! commence sur une nouvelle page.

use crate::doc::{self, Align, Para, Run, Style, Table};
use crate::page::{DocFont, PageSetup};
use crate::render::Values;

/// Un document prêt à exporter : son contenu, son en-tête et son pied de page.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Rendered {
    pub paras: Vec<Para>,
    pub header: Vec<Run>,
    pub footer: Vec<Run>,
    /// Mise en forme de chaque paragraphe (même longueur que `paras`, ou vide).
    pub formats: Vec<doc::ParaFormat>,
    /// Notes de bas de page : la note n° k (appel `note:k`) est à l'indice k − 1.
    pub notes: Vec<Vec<Run>>,
    /// Signatures manuscrites citées par le document, par nom.
    pub signatures: std::collections::BTreeMap<String, crate::signature::Signature>,
}

impl Rendered {
    /// La mise en forme du paragraphe `i`.
    pub fn format(&self, i: usize) -> doc::ParaFormat {
        self.formats.get(i).copied().unwrap_or_default()
    }

    /// Le contenu de la note n° `n` (à partir de 1).
    pub fn note(&self, n: usize) -> &[Run] {
        n.checked_sub(1)
            .and_then(|i| self.notes.get(i))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// Met en forme la source avec ses valeurs, en-tête et pied de page compris.
pub fn prepare(src: &str, values: &Values, page: &PageSetup) -> Rendered {
    // Dates, nombres et montants dans la langue du document.
    crate::lang::with_lang(page.langue, || {
        let src = &crate::critic::pour_export(src);
        let doc::Document {
            mut paras,
            formats,
            mut notes,
        } = doc::document_complet(src, values);
        let mut header = doc::inline(&page.header, values);
        let mut footer = doc::inline(&page.footer, values);
        if page.typographie {
            crate::typo::apply(&mut paras, page.langue);
            for runs in [&mut header, &mut footer]
                .into_iter()
                .chain(notes.iter_mut())
            {
                let mut p = [doc::Para::Quote {
                    runs: std::mem::take(runs),
                }];
                crate::typo::apply(&mut p, page.langue);
                if let [doc::Para::Quote { runs: r }] = p {
                    *runs = r;
                }
            }
        }
        // Signatures citées : intégrées au document, sinon celles de l'utilisateur.
        let mut signatures = std::collections::BTreeMap::new();
        let mut noter = |runs: &mut Vec<Run>| {
            for r in runs.iter() {
                if let Some(nom) = r.signature()
                    && !signatures.contains_key(nom)
                    && let Some(s) = crate::signature::trouver(nom, values)
                {
                    signatures.insert(nom.to_string(), s);
                }
            }
        };
        for p in &mut paras {
            p.pour_chaque_runs(&mut noter);
        }
        noter(&mut header);
        noter(&mut footer);
        Rendered {
            paras,
            header,
            footer,
            formats,
            notes,
            signatures,
        }
    })
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

/// Code QR en SVG, de `mm` millimètres de côté (taille de l'élément).
pub fn qr_svg(contenu: &str, mm: f32) -> String {
    match crate::qr::QrCode::encode(contenu) {
        Ok(q) => q.to_svg(2).replacen(
            "<svg ",
            &format!(
                "<svg class=\"qr\" width=\"{m}mm\" height=\"{m}mm\" role=\"img\" aria-label=\"{}\" ",
                xml_escape(contenu),
                m = num(mm)
            ),
            1,
        ),
        Err(e) => format!("<mark>{}</mark>", xml_escape(&e.to_string())),
    }
}

/// Contexte d'écriture HTML (et XHTML pour l'EPUB) d'un document.
pub(crate) struct Html<'a> {
    pub d: &'a Rendered,
    /// Rang du document (publipostage), pour des identifiants uniques.
    pub n: usize,
    /// XHTML : balises vides fermées (`<br/>`).
    pub xhtml: bool,
}

impl Html<'_> {
    fn br(&self) -> &'static str {
        if self.xhtml { "<br/>" } else { "<br>" }
    }

    /// Les morceaux en HTML : mise en forme, couleurs, liens, appels de note,
    /// formules en MathML, codes QR et signatures en SVG.
    pub fn runs(&self, runs: &[Run]) -> String {
        let mut out = String::new();
        for r in runs {
            let inner = match r {
                _ if r.qr().is_some() => qr_svg(r.qr().unwrap_or_default(), 20.0),
                _ if r.lien().is_some() => {
                    let (url, text) = r.lien().unwrap_or_default();
                    format!(
                        "<a href=\"{}\">{}</a>",
                        xml_escape(&doc::adresse_lien(url)),
                        xml_escape(text)
                    )
                }
                _ if r.note().is_some() => {
                    let k = r.note().unwrap_or_default();
                    let n = self.n;
                    out.push_str(&format!(
                        "<sup class=\"appel\"><a id=\"d{n}-a{k}\" href=\"#d{n}-n{k}\" role=\"doc-noteref\">{k}</a></sup>"
                    ));
                    continue;
                }
                _ if r.signature().is_some() => {
                    let nom = r.signature().unwrap_or_default();
                    match self.d.signatures.get(nom) {
                        Some(s) => s.svg(nom),
                        None => format!(
                            "<span class=\"sans-signature\">{}</span>",
                            xml_escape(if nom.is_empty() { "Signature" } else { nom })
                        ),
                    }
                }
                _ if r.sommaire().is_some() => {
                    let k = r.sommaire().unwrap_or_default();
                    format!(
                        "<a href=\"#d{}-t{k}\">{}</a>",
                        self.n,
                        xml_escape(&r.plain())
                    )
                }
                Run::Text { text, .. } => xml_escape(text).replace('\n', self.br()),
                Run::Value { name, text, .. } => format!(
                    "<span class=\"v\" data-var=\"{}\">{}</span>",
                    xml_escape(name),
                    xml_escape(text).replace('\n', self.br())
                ),
                Run::Missing { raw, .. } => format!("<mark>{}</mark>", xml_escape(raw)),
                Run::Math { tex, .. } => crate::formule::mathml(tex, false),
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
            let mut css = String::new();
            if let Some(c) = s.couleur {
                css.push_str(&format!("color:#{};", c.hex()));
            }
            if let Some(c) = s.surlignage {
                css.push_str(&format!("background-color:#{};", c.hex()));
            }
            if !css.is_empty() {
                wrapped = format!("<span style=\"{css}\">{wrapped}</span>");
            }
            out.push_str(&wrapped);
        }
        out
    }

    fn table(&self, t: &Table, style: &str) -> String {
        let mut s = format!("<table{style}>");
        for (i, row) in t.rows.iter().enumerate() {
            s.push_str("<tr>");
            let tag = if i == 0 && t.header { "th" } else { "td" };
            for col in 0..t.columns() {
                let cell = row.get(col).map(|c| self.runs(c)).unwrap_or_default();
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

    /// Le corps du document, paragraphe par paragraphe ; `saut` est écrit à chaque
    /// saut de page.
    pub fn paras(&self, out: &mut String, saut: &mut dyn FnMut(&mut String)) {
        let mut titres = 0usize;
        let br = self.br();
        for (i, p) in self.d.paras.iter().enumerate() {
            let st = style_para(&self.d.format(i));
            let html = match p {
                _ if p.qr().is_some() => {
                    let align = match p {
                        Para::Text { align, .. } => *align,
                        _ => Align::Left,
                    };
                    format!(
                        "<p{}{st}>{}</p>",
                        align_class(align),
                        qr_svg(p.qr().unwrap_or_default(), crate::qr::TAILLE_MM)
                    )
                }
                Para::Text { align, runs } if runs.is_empty() => {
                    format!("<p{}{st}>{br}</p>", align_class(*align))
                }
                Para::Text { align, runs } => {
                    format!("<p{}{st}>{}</p>", align_class(*align), self.runs(runs))
                }
                Para::Heading { level, runs } => {
                    titres += 1;
                    format!(
                        "<h{level} id=\"d{}-t{}\"{st}>{}</h{level}>",
                        self.n,
                        titres - 1,
                        self.runs(runs)
                    )
                }
                Para::Item { depth, runs, .. } if runs.iter().any(|r| r.sommaire().is_some()) => {
                    format!(
                        "<p class=\"toc\" style=\"margin-left:{}em\">{}</p>",
                        1.2 * *depth as f32,
                        self.runs(runs)
                    )
                }
                Para::Item {
                    number,
                    depth,
                    runs,
                } => {
                    let label = match number {
                        Some(n) => format!("{n}."),
                        None => ["•", "◦", "▪"][*depth as usize % 3].to_string(),
                    };
                    let f = self.d.format(i);
                    let gauche = f
                        .gauche
                        .map_or(String::new(), |g| format!(" + {}mm", num(g)));
                    format!(
                        "<p class=\"li\" style=\"margin-left:calc({}em{gauche})\"><span class=\"puce\">{label}</span>{}</p>",
                        1.6 * (*depth as f32 + 1.0),
                        self.runs(runs)
                    )
                }
                Para::Quote { runs } => format!("<blockquote{st}>{}</blockquote>", self.runs(runs)),
                Para::Math { tex } => format!(
                    "<div class=\"formule\"{st}>{}</div>",
                    crate::formule::mathml(tex, true)
                ),
                Para::Table(t) => self.table(t, &st),
                Para::Rule => if self.xhtml { "<hr/>" } else { "<hr>" }.to_string(),
                Para::PageBreak => {
                    saut(out);
                    continue;
                }
            };
            out.push_str(&html);
            out.push('\n');
        }
        out.push_str(&self.notes());
    }

    /// Les notes, en fin de document (notes de fin).
    pub fn notes(&self) -> String {
        if self.d.notes.is_empty() {
            return String::new();
        }
        let n = self.n;
        let mut s = format!(
            "<section class=\"notes\" role=\"doc-endnotes\">{}<ol>",
            if self.xhtml { "<hr/>" } else { "<hr>" }
        );
        for (i, c) in self.d.notes.iter().enumerate() {
            let k = i + 1;
            s.push_str(&format!(
                "<li id=\"d{n}-n{k}\" role=\"doc-endnote\">{} <a href=\"#d{n}-a{k}\" role=\"doc-backlink\">↩</a></li>",
                self.runs(c)
            ));
        }
        s.push_str("</ol></section>\n");
        s
    }
}

/// Mise en forme d'un paragraphe en CSS (attribut `style`, vide sans mise en forme).
pub(crate) fn style_para(f: &doc::ParaFormat) -> String {
    let mut css = String::new();
    if let Some(v) = f.gauche {
        css.push_str(&format!("margin-left:{}mm;", num(v)));
    }
    if let Some(v) = f.droite {
        css.push_str(&format!("margin-right:{}mm;", num(v)));
    }
    if let Some(v) = f.alinea {
        css.push_str(&format!("text-indent:{}mm;", num(v)));
    }
    match f.interligne {
        Some(doc::Interligne::Multiple(m)) => {
            css.push_str(&format!("line-height:{};", num(m * 1.15)))
        }
        Some(doc::Interligne::Exact(p)) => css.push_str(&format!("line-height:{}pt;", num(p))),
        None => {}
    }
    if let Some(v) = f.avant {
        css.push_str(&format!("margin-top:{}pt;", num(v)));
    }
    if let Some(v) = f.apres {
        css.push_str(&format!("margin-bottom:{}pt;", num(v)));
    }
    if css.is_empty() {
        css
    } else {
        format!(" style=\"{css}\"")
    }
}

fn align_class(a: Align) -> &'static str {
    match a {
        Align::Left => "",
        Align::Center => " class=\"c\"",
        Align::Right => " class=\"r\"",
        Align::Justify => " class=\"j\"",
    }
}

fn html_paras(d: &Rendered, n: usize, out: &mut String, header: &str, footer: &str) {
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
    let h = Html { d, n, xhtml: false };
    h.paras(out, &mut |out: &mut String| {
        close(out);
        open(out);
    });
    close(out);
}

fn css_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Une page HTML autonome, mise en page pour l'écran et pour l'impression (PDF).
/// Les formules sont écrites en MathML, les notes rassemblées en fin de document.
pub fn html(title: &str, docs: &[Rendered], page: &PageSetup) -> String {
    let mut body = String::new();
    for (n, d) in docs.iter().enumerate() {
        let h = Html { d, n, xhtml: false };
        let header = h.runs(&d.header);
        let footer = h.runs(&d.footer);
        html_paras(d, n, &mut body, &header, &footer);
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
    let lang = page.langue.code();
    let auteur = xml_escape(&page.auteur);
    let (w, h) = (num(w), num(h));
    format!(
        r#"<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="Varium">
<title>{title}</title>
<meta name="author" content="{auteur}">
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
  svg.qr {{ vertical-align: middle; }} a {{ color: #1f5fbf; }}
  svg.signature {{ vertical-align: bottom; }} .sans-signature {{ display: inline-block; min-width: 45mm; border-bottom: .5pt solid #888; color: #888; font-size: .8em; }}
  .formule {{ text-align: center; margin: .3em 0; }} math {{ font-size: 1.05em; }}
  sup.appel a {{ text-decoration: none; }} .notes {{ font-size: .88em; margin-top: 1.5em; }} .notes hr {{ width: 30%; margin-left: 0; }}
  .notes ol {{ padding-left: 1.4em; }} .notes a {{ text-decoration: none; }} .toc a {{ color: inherit; text-decoration: none; }}
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

/// Une signature en TikZ, à sa taille dans la page.
fn tex_signature(s: &crate::signature::Signature) -> String {
    let (l, h) = s.taille_mm();
    let mut o = format!(
        "\\begin{{tikzpicture}}[x=1mm,y=1mm,line cap=round,line join=round,line width={}mm]",
        num(s.epaisseur() * h / s.hauteur.max(1.0))
    );
    for t in s.a_l_echelle(l, h) {
        let pts: Vec<String> = t
            .iter()
            .map(|(x, y)| format!("({},{})", num(*x), num(h - y)))
            .collect();
        if pts.len() > 1 {
            o.push_str(&format!("\\draw {};", pts.join(" -- ")));
        }
    }
    o.push_str("\\end{tikzpicture}");
    o
}

fn tex_runs_doc(runs: &[Run], d: &Rendered) -> String {
    let mut out = String::new();
    for r in runs {
        if let Some(k) = r.note() {
            out.push_str(&format!("\\footnote{{{}}}", tex_runs_doc(d.note(k), d)));
            continue;
        }
        if let Some(nom) = r.signature() {
            match d.signatures.get(nom) {
                Some(s) => out.push_str(&tex_signature(s)),
                None => out.push_str("\\rule{45mm}{0.4pt}"),
            }
            continue;
        }
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
        if let Some(c) = st.couleur {
            s = format!("\\textcolor[HTML]{{{}}}{{{s}}}", c.hex().to_uppercase());
        }
        if let Some(c) = st.surlignage {
            s = format!(
                "{{\\setlength{{\\fboxsep}}{{1pt}}\\colorbox[HTML]{{{}}}{{{s}}}}}",
                c.hex().to_uppercase()
            );
        }
        out.push_str(&s);
    }
    out
}

fn tex_runs(runs: &[Run]) -> String {
    tex_runs_doc(runs, &Rendered::default())
}

/// Ouvre et ferme le groupe d'une mise en forme de paragraphe.
fn tex_format(f: &doc::ParaFormat, taille: f32) -> (String, String) {
    if f.est_vide() {
        return (String::new(), String::new());
    }
    let mut o = String::from("{");
    if let Some(v) = f.avant {
        o.push_str(&format!("\\vspace{{{}pt}}", num(v)));
    }
    if let Some(v) = f.gauche {
        o.push_str(&format!("\\setlength{{\\leftskip}}{{{}mm}}", num(v)));
    }
    if let Some(v) = f.droite {
        o.push_str(&format!("\\setlength{{\\rightskip}}{{{}mm}}", num(v)));
    }
    if let Some(v) = f.alinea {
        o.push_str(&format!(
            "\\setlength{{\\parindent}}{{{}mm}}\\indent",
            num(v)
        ));
    }
    match f.interligne {
        Some(doc::Interligne::Multiple(m)) => o.push_str(&format!("\\setstretch{{{}}}", num(m))),
        Some(doc::Interligne::Exact(p)) => o.push_str(&format!(
            "\\fontsize{{{}pt}}{{{}pt}}\\selectfont",
            num(taille),
            num(p)
        )),
        None => {}
    }
    o.push(' ');
    let mut c = String::from("\\par");
    if let Some(v) = f.apres {
        c.push_str(&format!("\\vspace{{{}pt}}", num(v)));
    }
    c.push('}');
    (o, c)
}

fn tex_paras(d: &Rendered, taille: f32, out: &mut String) {
    let mut sommaire_ecrit = false;
    let paras = &d.paras;
    for (i, p) in paras.iter().enumerate() {
        let est_sommaire = |p: &Para| matches!(p, Para::Item { runs, .. } if runs.iter().any(|r| r.sommaire().is_some()));
        // Table des matières : celle de LaTeX (titre et entrées générés par lui).
        if est_sommaire(p) {
            if !sommaire_ecrit {
                out.push_str("\\tableofcontents\n");
                sommaire_ecrit = true;
            }
            continue;
        }
        if paras.get(i + 1).is_some_and(est_sommaire) && matches!(p, Para::Text { .. }) {
            sommaire_ecrit = false;
            continue;
        }
        let (ouvre, ferme) = tex_format(&d.format(i), taille);
        out.push_str(&ouvre);
        match p {
            Para::Text { runs, .. } if runs.is_empty() => {
                out.push_str("\\vspace{\\baselineskip}\n")
            }
            Para::Text { align, runs } => {
                let body = tex_runs_doc(runs, d);
                out.push_str(&match align {
                    Align::Center => format!("{{\\centering {body}\\par}}\n"),
                    Align::Right => format!("{{\\raggedleft {body}\\par}}\n"),
                    _ => format!("{body}\\par\n"),
                });
            }
            Para::Heading { level, runs } => {
                let cmd =
                    ["section", "subsection", "subsubsection"][(*level as usize).clamp(1, 3) - 1];
                let t = tex_runs_doc(runs, d);
                out.push_str(&format!(
                    "\\{cmd}*{{{t}}}\\addcontentsline{{toc}}{{{cmd}}}{{{}}}\n",
                    tex_escape(&doc::plain(runs))
                ));
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
                    tex_runs_doc(runs, d)
                ));
            }
            Para::Quote { runs } => out.push_str(&format!(
                "\\begin{{quote}}\\itshape {}\\end{{quote}}\n",
                tex_runs_doc(runs, d)
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
                            let s = row.get(c).map(|r| tex_runs_doc(r, d)).unwrap_or_default();
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
        out.push_str(&ferme);
        if !ferme.is_empty() {
            out.push('\n');
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
        tex_paras(d, page.font_size, &mut body);
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
        "% Généré par Varium\n\\documentclass[{size}pt,{paper}{landscape}]{{article}}\n\\usepackage[utf8]{{inputenc}}\n\\usepackage[T1]{{fontenc}}\n\\usepackage[french]{{babel}}\n\\usepackage{{amsmath,amssymb}}\n\\usepackage{{xcolor,tikz}}\n{font}\n\\usepackage[top={}mm,bottom={}mm,left={}mm,right={}mm]{{geometry}}\n\\usepackage{{setspace}}\n\\setstretch{{{}}}\n\\usepackage[normalem]{{ulem}}\n\\usepackage{{fancyhdr,lastpage}}\n\\pagestyle{{fancy}}\n\\fancyhf{{}}\n\\renewcommand{{\\headrulewidth}}{{0pt}}\n\\chead{{{header}}}\n\\lfoot{{{footer}}}\n\\rfoot{{{numbers}}}\n\\setlength{{\\parindent}}{{0pt}}\n\\setlength{{\\parskip}}{{0pt}}\n\\title{{{}}}\n\\author{{{}}}\n\\date{{}}\n\n\\begin{{document}}\n{ragged}{}\n\\end{{document}}\n",
        num(page.margin_top),
        num(page.margin_bottom),
        num(page.margin_left),
        num(page.margin_right),
        num(page.line_spacing),
        tex_escape(title),
        tex_escape(&page.auteur),
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
