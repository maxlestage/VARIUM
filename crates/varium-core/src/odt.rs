//! Export OpenDocument Texte (`.odt`), le format de LibreOffice Writer.

use crate::doc::{Align, Interligne, Para, ParaFormat, Run, Style, Table};
use crate::export::{Rendered, xml_escape};
use crate::page::PageSetup;
use crate::zip::Zip;

const NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" office:version="1.3""#;

fn mm(v: f32) -> String {
    format!("{v:.2}mm")
}

/// Texte ODF : les espaces multiples et les tabulations doivent être explicites.
fn odf_text(s: &str) -> String {
    let mut out = String::new();
    let mut spaces = 0usize;
    let mut at_start = true;
    let flush = |out: &mut String, spaces: &mut usize, at_start: bool| {
        if *spaces == 0 {
            return;
        }
        if at_start {
            out.push_str(&format!("<text:s text:c=\"{spaces}\"/>"));
        } else {
            out.push(' ');
            if *spaces > 1 {
                out.push_str(&format!("<text:s text:c=\"{}\"/>", *spaces - 1));
            }
        }
        *spaces = 0;
    };
    for c in s.chars() {
        match c {
            ' ' => spaces += 1,
            '\t' => {
                flush(&mut out, &mut spaces, at_start);
                out.push_str("<text:tab/>");
                at_start = false;
            }
            '\n' => {
                flush(&mut out, &mut spaces, at_start);
                out.push_str("<text:line-break/>");
                at_start = false;
            }
            _ => {
                flush(&mut out, &mut spaces, at_start);
                at_start = false;
                out.push_str(&xml_escape(c.encode_utf8(&mut [0; 4])));
            }
        }
    }
    flush(&mut out, &mut spaces, at_start);
    out
}

/// Images PNG (codes QR, signatures) et formules à joindre au fichier, styles
/// automatiques créés au fil de l'écriture.
#[derive(Default)]
struct Media {
    /// (nom dans `Pictures/`, octets).
    images: Vec<(String, Vec<u8>)>,
    /// Formules : contenu MathML de chaque objet `Object N/content.xml`.
    formules: Vec<String>,
    /// Styles de caractères : (clé, nom).
    textes: Vec<(String, String)>,
    /// Styles de paragraphe : (propriétés, parent, nom).
    paras: Vec<(String, String, String)>,
    notes: usize,
}

impl Media {
    /// Style de caractères d'une mise en forme (nom vide sans mise en forme).
    fn style_texte(&mut self, st: Style, manquant: bool) -> String {
        let mut props = String::new();
        if st.bold {
            props.push_str(" fo:font-weight=\"bold\" style:font-weight-asian=\"bold\" style:font-weight-complex=\"bold\"");
        }
        if st.italic {
            props.push_str(" fo:font-style=\"italic\" style:font-style-asian=\"italic\" style:font-style-complex=\"italic\"");
        }
        if st.underline {
            props.push_str(" style:text-underline-style=\"solid\" style:text-underline-width=\"auto\" style:text-underline-color=\"font-color\"");
        }
        if st.strike {
            props.push_str(
                " style:text-line-through-style=\"solid\" style:text-line-through-type=\"single\"",
            );
        }
        if manquant {
            props.push_str(" fo:color=\"#a1201b\" fo:background-color=\"#fde2e1\"");
        } else {
            if let Some(c) = st.couleur {
                props.push_str(&format!(" fo:color=\"#{}\"", c.hex()));
            }
            if let Some(c) = st.surlignage {
                props.push_str(&format!(" fo:background-color=\"#{}\"", c.hex()));
            }
        }
        if st.exposant {
            props.push_str(" style:text-position=\"super 58%\"");
        }
        if props.is_empty() {
            return String::new();
        }
        if let Some((_, n)) = self.textes.iter().find(|(k, _)| *k == props) {
            return n.clone();
        }
        let n = format!("T{}", self.textes.len() + 1);
        self.textes.push((props, n.clone()));
        n
    }

    /// Style de paragraphe : `parent` complété de l'alignement et de la mise en forme.
    fn style_para(&mut self, parent: &str, align: Option<Align>, f: &ParaFormat) -> String {
        let mut props = String::new();
        if let Some(a) = align {
            props.push_str(&format!(" fo:text-align=\"{}\"", align_attr(a)));
        }
        if let Some(v) = f.gauche {
            props.push_str(&format!(" fo:margin-left=\"{}\"", mm(v)));
        }
        if let Some(v) = f.droite {
            props.push_str(&format!(" fo:margin-right=\"{}\"", mm(v)));
        }
        if let Some(v) = f.alinea {
            props.push_str(&format!(" fo:text-indent=\"{}\"", mm(v)));
        }
        match f.interligne {
            Some(Interligne::Multiple(m)) => props.push_str(&format!(
                " fo:line-height=\"{}%\"",
                (m * 100.0).round() as i32
            )),
            Some(Interligne::Exact(p)) => {
                props.push_str(&format!(" fo:line-height=\"{:.1}pt\"", p))
            }
            None => {}
        }
        if let Some(v) = f.avant {
            props.push_str(&format!(" fo:margin-top=\"{:.1}pt\"", v));
        }
        if let Some(v) = f.apres {
            props.push_str(&format!(" fo:margin-bottom=\"{:.1}pt\"", v));
        }
        if props.is_empty() {
            return parent.to_string();
        }
        if let Some((_, _, n)) = self
            .paras
            .iter()
            .find(|(p, par, _)| *p == props && par == parent)
        {
            return n.clone();
        }
        let n = format!("P{}", self.paras.len() + 1);
        self.paras.push((props, parent.to_string(), n.clone()));
        n
    }

    fn styles_auto(&self) -> String {
        let mut s = String::new();
        for (props, n) in &self.textes {
            s.push_str(&format!(
                "<style:style style:name=\"{n}\" style:family=\"text\"><style:text-properties{props}/></style:style>"
            ));
        }
        for (props, parent, n) in &self.paras {
            s.push_str(&format!(
                "<style:style style:name=\"{n}\" style:family=\"paragraph\" style:parent-style-name=\"{parent}\"><style:paragraph-properties{props}/></style:style>"
            ));
        }
        s
    }
}

/// Une image PNG dans un cadre de `l` × `h` millimètres.
fn cadre_image(nom: &str, png: Vec<u8>, l: f32, h: f32, titre: &str, m: &mut Media) -> String {
    let fichier = format!("{nom}{}.png", m.images.len() + 1);
    m.images.push((fichier.clone(), png));
    format!(
        "<draw:frame draw:name=\"{}{n}\" text:anchor-type=\"as-char\" svg:width=\"{}\" svg:height=\"{}\" draw:z-index=\"0\"><draw:image xlink:href=\"Pictures/{fichier}\" xlink:type=\"simple\" xlink:show=\"embed\" xlink:actuate=\"onLoad\" draw:mime-type=\"image/png\"/><svg:title>{}</svg:title></draw:frame>",
        // « QR1 », « Signature1 » : noms relus par l'import.
        if nom == "qr" { "QR" } else { "Signature" },
        mm(l),
        mm(h),
        xml_escape(titre),
        n = m.images.len()
    )
}

/// Un code QR en image PNG dans un cadre de `cote` millimètres.
fn qr_frame(contenu: &str, cote: f32, m: &mut Media) -> String {
    let Ok(q) = crate::qr::QrCode::encode(contenu) else {
        return odf_text(contenu);
    };
    cadre_image("qr", q.to_png(8, 2), cote, cote, contenu, m)
}

/// Une formule : objet LibreOffice Math (MathML) de la taille de sa composition.
fn formule(tex: &str, centree: bool, page: &PageSetup, m: &mut Media) -> String {
    let taille = page.font_size * if centree { 1.1 } else { 1.0 };
    let (l, h, p) = crate::mise_en_page::mesure_formule(tex, centree, taille, page.font);
    m.formules.push(crate::formule::mathml(tex, centree));
    let n = m.formules.len();
    let pt = |v: f32| format!("{:.2}pt", v.max(1.0));
    format!(
        "<draw:frame draw:name=\"Formule{n}\" text:anchor-type=\"as-char\" svg:y=\"-{}\" svg:width=\"{}\" svg:height=\"{}\" draw:z-index=\"0\"><draw:object xlink:href=\"./Object {n}\" xlink:type=\"simple\" xlink:show=\"embed\" xlink:actuate=\"onLoad\"/><svg:desc>{}</svg:desc></draw:frame>",
        pt(h * 1.15),
        // LibreOffice Math espace un peu plus que Varium : cadre élargi.
        pt(l * 1.35 + 2.0),
        pt((h + p) * 1.2),
        xml_escape(tex.trim())
    )
}

/// Le texte des morceaux. Sans `d` (en-tête, pied de page) : codes QR, notes,
/// signatures et formules restent du texte.
fn runs_xml(runs: &[Run], d: Option<&Rendered>, page: &PageSetup, m: &mut Media) -> String {
    let mut out = String::new();
    for r in runs {
        if let (Some(q), Some(_)) = (r.qr(), d) {
            out.push_str(&qr_frame(q, 20.0, m));
            continue;
        }
        if let (Some(k), Some(d)) = (r.note(), d) {
            m.notes += 1;
            let style = m.style_para("Footnote", None, &ParaFormat::default());
            let contenu = runs_xml(d.note(k), None, page, m);
            out.push_str(&format!(
                "<text:note text:id=\"ftn{}\" text:note-class=\"footnote\"><text:note-citation>{k}</text:note-citation><text:note-body><text:p text:style-name=\"{style}\">{contenu}</text:p></text:note-body></text:note>",
                m.notes
            ));
            continue;
        }
        if let (Some(nom), Some(d)) = (r.signature(), d) {
            match d.signatures.get(nom) {
                Some(s) => {
                    let (l, h) = s.taille_mm();
                    out.push_str(&cadre_image("signature", s.png(200), l, h, nom, m));
                }
                None => out.push_str("____________________"),
            }
            continue;
        }
        if let (Run::Math { tex, .. }, Some(_)) = (r, d) {
            out.push_str(&formule(tex, false, page, m));
            continue;
        }
        if let Some((url, label)) = r.lien() {
            let st = m.style_texte(r.style(), false);
            let texte = if st.is_empty() {
                odf_text(label)
            } else {
                format!(
                    "<text:span text:style-name=\"{st}\">{}</text:span>",
                    odf_text(label)
                )
            };
            out.push_str(&format!(
                "<text:a xlink:type=\"simple\" xlink:href=\"{}\">{texte}</text:a>",
                xml_escape(&crate::doc::adresse_lien(url)),
            ));
            continue;
        }
        let (text, missing) = match r {
            Run::Text { text, .. } | Run::Value { text, .. } => (text.clone(), false),
            Run::Missing { raw, .. } => (raw.clone(), true),
            Run::Math { .. } => (r.plain(), false),
        };
        let mut style: Style = r.style();
        if matches!(r, Run::Math { .. }) {
            style.italic = true;
        }
        let body = odf_text(&text);
        let name = m.style_texte(style, missing);
        if name.is_empty() {
            out.push_str(&body);
        } else {
            out.push_str(&format!(
                "<text:span text:style-name=\"{name}\">{body}</text:span>"
            ));
        }
    }
    out
}

fn align_attr(a: Align) -> &'static str {
    match a {
        Align::Left => "start",
        Align::Center => "center",
        Align::Right => "end",
        Align::Justify => "justify",
    }
}

fn para_styles() -> String {
    let mut s = String::new();
    s.push_str("<style:style style:name=\"PB\" style:family=\"paragraph\" style:parent-style-name=\"Standard\"><style:paragraph-properties fo:break-before=\"page\"/></style:style>");
    s.push_str("<style:style style:name=\"Hr\" style:family=\"paragraph\" style:parent-style-name=\"Standard\"><style:paragraph-properties fo:padding-bottom=\"0.5mm\" fo:border-bottom=\"0.5pt solid #000000\" fo:margin-bottom=\"2mm\"/></style:style>");
    for (name, a, bold) in [
        ("Cl", Align::Left, false),
        ("Cc", Align::Center, false),
        ("Cr", Align::Right, false),
        ("Hl", Align::Left, true),
        ("Hc", Align::Center, true),
        ("Hr2", Align::Right, true),
    ] {
        let weight = if bold {
            "<style:text-properties fo:font-weight=\"bold\"/>"
        } else {
            ""
        };
        s.push_str(&format!(
            "<style:style style:name=\"{name}\" style:family=\"paragraph\" style:parent-style-name=\"Table_20_Contents\"><style:paragraph-properties fo:text-align=\"{}\"/>{weight}</style:style>",
            align_attr(a)
        ));
    }
    s.push_str("<style:style style:name=\"Tab\" style:family=\"table\"><style:table-properties style:width=\"100%\" table:align=\"margins\" fo:margin-top=\"1mm\" fo:margin-bottom=\"1mm\"/></style:style>");
    s.push_str("<style:style style:name=\"Cell\" style:family=\"table-cell\"><style:table-cell-properties fo:padding=\"1mm\" fo:border=\"0.5pt solid #000000\"/></style:style>");
    s.push_str("<style:style style:name=\"CellH\" style:family=\"table-cell\"><style:table-cell-properties fo:padding=\"1mm\" fo:border=\"0.5pt solid #000000\" fo:background-color=\"#f0eee9\"/></style:style>");
    s.push_str("<style:style style:name=\"Sect\" style:family=\"section\"><style:section-properties style:editable=\"false\"/></style:style>");
    s
}

fn table_xml(t: &Table, n: &mut usize, d: &Rendered, page: &PageSetup, m: &mut Media) -> String {
    *n += 1;
    let cols = t.columns().max(1);
    let mut s = format!(
        "<table:table table:name=\"Tableau{n}\" table:style-name=\"Tab\"><table:table-column table:number-columns-repeated=\"{cols}\"/>"
    );
    for (i, row) in t.rows.iter().enumerate() {
        let head = i == 0 && t.header;
        if head {
            s.push_str("<table:table-header-rows>");
        }
        s.push_str("<table:table-row>");
        for c in 0..cols {
            let style = match (head, t.align(c)) {
                (true, Align::Center) => "Hc",
                (true, Align::Right) => "Hr2",
                (true, _) => "Hl",
                (false, Align::Center) => "Cc",
                (false, Align::Right) => "Cr",
                (false, _) => "Cl",
            };
            let cell = row
                .get(c)
                .map(|r| runs_xml(r, Some(d), page, m))
                .unwrap_or_default();
            s.push_str(&format!(
                "<table:table-cell table:style-name=\"{}\" office:value-type=\"string\"><text:p text:style-name=\"{style}\">{cell}</text:p></table:table-cell>",
                if head { "CellH" } else { "Cell" }
            ));
        }
        s.push_str("</table:table-row>");
        if head {
            s.push_str("</table:table-header-rows>");
        }
    }
    s.push_str("</table:table>");
    s
}

/// Une table des matières LibreOffice (index régénérable), avec les numéros de page
/// calculés par la mise en page de Varium.
fn sommaire_xml(
    entrees: &[(u8, usize, String)],
    doc: usize,
    pages: &[usize],
    titre: &str,
    n: usize,
) -> String {
    let mut gabarits = String::new();
    for niveau in 1..=3 {
        gabarits.push_str(&format!(
            "<text:table-of-content-entry-template text:outline-level=\"{niveau}\" text:style-name=\"Contents_20_{niveau}\"><text:index-entry-link-start/><text:index-entry-text/><text:index-entry-tab-stop style:type=\"right\" style:leader-char=\".\"/><text:index-entry-page-number/><text:index-entry-link-end/></text:table-of-content-entry-template>"
        ));
    }
    let mut corps = format!(
        "<text:index-title text:style-name=\"Sect\" text:name=\"Sommaire{n}_Titre\"><text:p text:style-name=\"Contents_20_Heading\">{}</text:p></text:index-title>",
        xml_escape(titre)
    );
    for (niveau, k, texte) in entrees {
        let page = pages.get(*k).copied().unwrap_or(1);
        corps.push_str(&format!(
            "<text:p text:style-name=\"Contents_20_{}\"><text:a xlink:type=\"simple\" xlink:href=\"#titre-d{doc}-{k}\" text:style-name=\"Index_20_Link\" text:visited-style-name=\"Index_20_Link\">{}<text:tab/>{page}</text:a></text:p>",
            (*niveau).clamp(1, 3),
            odf_text(texte)
        ));
    }
    format!(
        "<text:table-of-content text:style-name=\"Sect\" text:protected=\"true\" text:name=\"Sommaire{n}\"><text:table-of-content-source text:outline-level=\"3\" text:use-index-marks=\"false\"><text:index-title-template text:style-name=\"Contents_20_Heading\">{}</text:index-title-template>{gabarits}</text:table-of-content-source><text:index-body>{corps}</text:index-body></text:table-of-content>",
        xml_escape(titre)
    )
}

fn body_xml(docs: &[Rendered], page: &PageSetup, m: &mut Media) -> String {
    let defaut = if page.justify {
        Align::Justify
    } else {
        Align::Left
    };
    let mut s = String::new();
    let mut tables = 0;
    let mut sommaires = 0;
    // Numéros de page des titres, seulement s'il y a une table des matières.
    let a_sommaire = docs.iter().any(|d| {
        d.paras
            .iter()
            .any(|p| p.runs().iter().any(|r| r.sommaire().is_some()))
    });
    let pages = if a_sommaire {
        crate::mise_en_page::pages_des_titres(docs, page)
    } else {
        Vec::new()
    };
    for (i, d) in docs.iter().enumerate() {
        if i > 0 {
            s.push_str("<text:p text:style-name=\"PB\"/>");
        }
        let mut titres = 0usize;
        let mut pi = 0;
        while pi < d.paras.len() {
            let p = &d.paras[pi];
            let f = d.format(pi);
            // Table des matières : le titre « Table des matières » et ses entrées.
            let est_entree = |p: &Para| match p {
                Para::Item { depth, runs, .. } => runs
                    .iter()
                    .find_map(Run::sommaire)
                    .map(|k| ((*depth + 1).min(3), k, crate::doc::plain(runs))),
                _ => None,
            };
            if matches!(p, Para::Text { .. })
                && let Some(premiere) = d.paras.get(pi + 1).and_then(est_entree)
            {
                let mut entrees = vec![premiere];
                let mut j = pi + 2;
                while let Some(e) = d.paras.get(j).and_then(est_entree) {
                    entrees.push(e);
                    j += 1;
                }
                sommaires += 1;
                s.push_str(&sommaire_xml(
                    &entrees,
                    i,
                    pages.get(i).map(Vec::as_slice).unwrap_or(&[]),
                    &p.plain(),
                    sommaires,
                ));
                pi = j;
                continue;
            }
            match p {
                Para::Text { align, runs } => {
                    let a = if *align == Align::Left {
                        defaut
                    } else {
                        *align
                    };
                    let style = m.style_para("Standard", Some(a), &f);
                    let contenu = match p.qr() {
                        Some(q) => qr_frame(q, crate::qr::TAILLE_MM, m),
                        None => runs_xml(runs, Some(d), page, m),
                    };
                    s.push_str(&format!(
                        "<text:p text:style-name=\"{style}\">{contenu}</text:p>"
                    ));
                }
                Para::Heading { level, runs } => {
                    let style = m.style_para(&format!("Heading_20_{level}"), None, &f);
                    let contenu = runs_xml(runs, Some(d), page, m);
                    s.push_str(&format!(
                        "<text:h text:style-name=\"{style}\" text:outline-level=\"{level}\"><text:bookmark-start text:name=\"titre-d{i}-{titres}\"/>{contenu}<text:bookmark-end text:name=\"titre-d{i}-{titres}\"/></text:h>"
                    ));
                    titres += 1;
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
                    let gauche = 6.35 * (*depth as f32 + 1.0) + f.gauche.unwrap_or(0.0);
                    let fl = ParaFormat {
                        gauche: Some(gauche),
                        alinea: Some(f.alinea.unwrap_or(0.0) - 6.35),
                        ..f
                    };
                    let style = m.style_para("Standard", None, &fl);
                    let contenu = runs_xml(runs, Some(d), page, m);
                    s.push_str(&format!(
                        "<text:p text:style-name=\"{style}\">{}<text:tab/>{contenu}</text:p>",
                        xml_escape(&label),
                    ));
                }
                Para::Quote { runs } => {
                    let style = m.style_para("Quotations", None, &f);
                    let contenu = runs_xml(runs, Some(d), page, m);
                    s.push_str(&format!(
                        "<text:p text:style-name=\"{style}\">{contenu}</text:p>"
                    ))
                }
                Para::Math { tex } => {
                    let style = m.style_para("Standard", Some(Align::Center), &f);
                    let contenu = formule(tex, true, page, m);
                    s.push_str(&format!(
                        "<text:p text:style-name=\"{style}\">{contenu}</text:p>"
                    ))
                }
                Para::Table(t) => s.push_str(&table_xml(t, &mut tables, d, page, m)),
                Para::Rule => s.push_str("<text:p text:style-name=\"Hr\"/>"),
                Para::PageBreak => s.push_str("<text:p text:style-name=\"PB\"/>"),
            }
            pi += 1;
        }
    }
    s
}

fn font_decls() -> &'static str {
    r#"<office:font-face-decls><style:font-face style:name="Liberation Serif" svg:font-family="'Liberation Serif'" style:font-family-generic="roman" style:font-pitch="variable"/><style:font-face style:name="Liberation Sans" svg:font-family="'Liberation Sans'" style:font-family-generic="swiss" style:font-pitch="variable"/></office:font-face-decls>"#
}

fn styles_xml(docs: &[Rendered], page: &PageSetup) -> String {
    let lang = page.langue.code();
    let country = page.langue.country();
    let font = page.font.family();
    let size = format!("{:.1}pt", page.font_size);
    let lh = format!("{}%", (page.line_spacing * 100.0).round() as i32);
    let (w, h) = page.size_mm();
    let orientation = if page.landscape {
        "landscape"
    } else {
        "portrait"
    };
    let first = docs.first();
    let mut m = Media::default();
    let header = first
        .map(|d| runs_xml(&d.header, None, page, &mut m))
        .unwrap_or_default();
    let footer = first
        .map(|d| runs_xml(&d.footer, None, page, &mut m))
        .unwrap_or_default();
    let numbers = if page.page_numbers {
        "Page <text:page-number text:select-page=\"current\">1</text:page-number> sur <text:page-count>1</text:page-count>"
    } else {
        ""
    };
    let header_xml = if header.is_empty() {
        String::new()
    } else {
        format!("<style:header><text:p text:style-name=\"Header\">{header}</text:p></style:header>")
    };
    let footer_xml = if footer.is_empty() && numbers.is_empty() {
        String::new()
    } else {
        let sep = if !footer.is_empty() && !numbers.is_empty() {
            "<text:tab/><text:tab/>"
        } else {
            ""
        };
        format!(
            "<style:footer><text:p text:style-name=\"Footer\">{footer}{sep}{numbers}</text:p></style:footer>"
        )
    };
    let heading = |n: u8, size: &str| {
        format!(
            "<style:style style:name=\"Heading_20_{n}\" style:display-name=\"Heading {n}\" style:family=\"paragraph\" style:parent-style-name=\"Standard\" style:next-style-name=\"Standard\" style:default-outline-level=\"{n}\" style:class=\"text\"><style:paragraph-properties fo:margin-top=\"4mm\" fo:margin-bottom=\"2mm\" fo:keep-with-next=\"always\" fo:text-align=\"start\"/><style:text-properties fo:font-size=\"{size}\" fo:font-weight=\"bold\" style:font-weight-asian=\"bold\" style:font-weight-complex=\"bold\"/></style:style>"
        )
    };
    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<office:document-styles {NS}>{fonts}<office:styles><style:default-style style:family="paragraph"><style:paragraph-properties fo:hyphenation-ladder-count="no-limit" style:writing-mode="page"/><style:text-properties style:font-name="{font}" fo:font-size="{size}" fo:language="{lang}" fo:country="{country}"/></style:default-style><style:style style:name="Standard" style:family="paragraph" style:class="text"><style:paragraph-properties fo:line-height="{lh}"/><style:text-properties style:font-name="{font}" fo:font-size="{size}"/></style:style>{h1}{h2}{h3}<style:style style:name="Quotations" style:family="paragraph" style:parent-style-name="Standard" style:class="html"><style:paragraph-properties fo:margin-left="10mm" fo:margin-right="10mm" fo:margin-top="1mm" fo:margin-bottom="2mm" fo:padding-left="3mm" fo:border-left="1pt solid #999999"/><style:text-properties fo:font-style="italic"/></style:style><style:style style:name="Table_20_Contents" style:display-name="Table Contents" style:family="paragraph" style:parent-style-name="Standard" style:class="extra"/><style:style style:name="Header" style:family="paragraph" style:parent-style-name="Standard" style:class="extra"><style:paragraph-properties fo:text-align="center"/><style:text-properties fo:font-size="9pt" fo:color="#444444"/></style:style><style:style style:name="Footer" style:family="paragraph" style:parent-style-name="Standard" style:class="extra"><style:paragraph-properties><style:tab-stops><style:tab-stop style:position="{half}" style:type="center"/><style:tab-stop style:position="{text_w}" style:type="right"/></style:tab-stops></style:paragraph-properties><style:text-properties fo:font-size="9pt" fo:color="#444444"/></style:style>{extra}</office:styles><office:automatic-styles>{ts}<style:page-layout style:name="pm1"><style:page-layout-properties fo:page-width="{w}" fo:page-height="{h}" style:print-orientation="{orientation}" fo:margin-top="{mt}" fo:margin-bottom="{mb}" fo:margin-left="{ml}" fo:margin-right="{mr}"/><style:header-style><style:header-footer-properties fo:min-height="0mm" fo:margin-bottom="4mm"/></style:header-style><style:footer-style><style:header-footer-properties fo:min-height="0mm" fo:margin-top="4mm"/></style:footer-style></style:page-layout></office:automatic-styles><office:master-styles><style:master-page style:name="Standard" style:page-layout-name="pm1">{header_xml}{footer_xml}</style:master-page></office:master-styles></office:document-styles>"##,
        fonts = font_decls(),
        h1 = heading(1, "160%"),
        h2 = heading(2, "135%"),
        h3 = heading(3, "115%"),
        ts = m.styles_auto(),
        w = mm(w),
        h = mm(h),
        mt = mm(page.margin_top),
        mb = mm(page.margin_bottom),
        ml = mm(page.margin_left),
        mr = mm(page.margin_right),
        half = mm(page.text_width_mm() / 2.0),
        text_w = mm(page.text_width_mm()),
        extra = styles_supplementaires(page),
    )
}

/// Styles des notes de bas de page et de la table des matières.
fn styles_supplementaires(page: &PageSetup) -> String {
    let tw = mm(page.text_width_mm());
    let mut s = String::from(
        "<style:style style:name=\"Footnote\" style:display-name=\"Footnote\" style:family=\"paragraph\" style:parent-style-name=\"Standard\" style:class=\"extra\"><style:paragraph-properties fo:margin-left=\"5mm\" fo:text-indent=\"-5mm\" fo:line-height=\"100%\"/><style:text-properties fo:font-size=\"83%\"/></style:style><style:style style:name=\"Contents_20_Heading\" style:display-name=\"Contents Heading\" style:family=\"paragraph\" style:parent-style-name=\"Standard\" style:class=\"index\"><style:paragraph-properties fo:margin-bottom=\"2mm\"/><style:text-properties fo:font-weight=\"bold\" style:font-weight-complex=\"bold\"/></style:style><style:style style:name=\"Index_20_Link\" style:display-name=\"Index Link\" style:family=\"text\"/>",
    );
    for n in 1..=3 {
        s.push_str(&format!(
            "<style:style style:name=\"Contents_20_{n}\" style:display-name=\"Contents {n}\" style:family=\"paragraph\" style:parent-style-name=\"Standard\" style:class=\"index\"><style:paragraph-properties fo:margin-left=\"{}\"><style:tab-stops><style:tab-stop style:position=\"{tw}\" style:type=\"right\" style:leader-style=\"dotted\" style:leader-text=\".\"/></style:tab-stops></style:paragraph-properties></style:style>",
            mm(5.0 * (n - 1) as f32)
        ));
    }
    s
}

/// Le fichier `.odt` (une archive ZIP), prêt à ouvrir dans LibreOffice Writer.
pub fn odt(title: &str, docs: &[Rendered], page: &PageSetup) -> Vec<u8> {
    let mut m = Media::default();
    let body = body_xml(docs, page, &mut m);
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content {NS}>{fonts}<office:automatic-styles>{ts}{ps}</office:automatic-styles><office:body><office:text>{body}</office:text></office:body></office:document-content>"#,
        fonts = font_decls(),
        ts = m.styles_auto(),
        ps = para_styles(),
    );
    let auteur = if page.auteur.trim().is_empty() {
        String::new()
    } else {
        format!(
            "<meta:initial-creator>{0}</meta:initial-creator><dc:creator>{0}</dc:creator>",
            xml_escape(page.auteur.trim())
        )
    };
    let meta = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-meta {NS}><office:meta><meta:generator>Varium</meta:generator><dc:title>{}</dc:title>{auteur}<dc:language>{}</dc:language></office:meta></office:document-meta>"#,
        xml_escape(title),
        page.langue.tag()
    );
    let mut fichiers: String = m
        .images
        .iter()
        .map(|(n, _)| format!("<manifest:file-entry manifest:full-path=\"Pictures/{n}\" manifest:media-type=\"image/png\"/>"))
        .collect();
    for n in 1..=m.formules.len() {
        fichiers.push_str(&format!("<manifest:file-entry manifest:full-path=\"Object {n}/\" manifest:version=\"1.3\" manifest:media-type=\"application/vnd.oasis.opendocument.formula\"/><manifest:file-entry manifest:full-path=\"Object {n}/content.xml\" manifest:media-type=\"text/xml\"/>"));
    }
    let manifest = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3"><manifest:file-entry manifest:full-path="/" manifest:version="1.3" manifest:media-type="application/vnd.oasis.opendocument.text"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/>{fichiers}</manifest:manifest>"#
    );

    let mut zip = Zip::new();
    // `mimetype` doit être le premier fichier, non compressé.
    zip.add("mimetype", b"application/vnd.oasis.opendocument.text");
    zip.add("content.xml", content.as_bytes());
    zip.add("styles.xml", styles_xml(docs, page).as_bytes());
    zip.add("meta.xml", meta.as_bytes());
    for (n, png) in &m.images {
        zip.add(&format!("Pictures/{n}"), png);
    }
    for (i, f) in m.formules.iter().enumerate() {
        zip.add(
            &format!("Object {}/content.xml", i + 1),
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{f}").as_bytes(),
        );
    }
    zip.add("META-INF/manifest.xml", manifest.as_bytes());
    zip.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn espaces_odf() {
        assert_eq!(odf_text("a  b"), "a <text:s text:c=\"1\"/>b");
        assert_eq!(odf_text(" a"), "<text:s text:c=\"1\"/>a");
        assert_eq!(odf_text("a<b"), "a&lt;b");
    }
}
