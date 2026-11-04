//! Export Word (`.docx`, Office Open XML), lisible aussi par LibreOffice et Google Docs.

use crate::doc::{Align, Interligne, Para, ParaFormat, Run, Table};
use crate::export::{Rendered, xml_escape};
use crate::page::PageSetup;
use crate::zip::Zip;

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math""#;

/// Millimètres → vingtièmes de point (twips).
fn tw(mm: f32) -> i32 {
    (mm * 56.692_9).round() as i32
}

fn run_xml(text: &str, r: &Run, missing: bool) -> String {
    let st = r.style();
    let mut props = String::new();
    if st.bold {
        props.push_str("<w:b/><w:bCs/>");
    }
    if st.italic || matches!(r, Run::Math { .. }) {
        props.push_str("<w:i/><w:iCs/>");
    }
    if st.strike {
        props.push_str("<w:strike/>");
    }
    if missing {
        props.push_str("<w:color w:val=\"A1201B\"/>");
    } else if let Some(c) = st.couleur {
        props.push_str(&format!("<w:color w:val=\"{}\"/>", c.hex().to_uppercase()));
    }
    if st.underline {
        props.push_str("<w:u w:val=\"single\"/>");
    }
    if missing {
        props.push_str("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"FDE2E1\"/>");
    } else if let Some(c) = st.surlignage {
        props.push_str(&format!(
            "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>",
            c.hex().to_uppercase()
        ));
    }
    if st.exposant {
        props.push_str("<w:vertAlign w:val=\"superscript\"/>");
    }
    let rpr = if props.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{props}</w:rPr>")
    };
    // Les retours à la ligne (valeurs longues) deviennent des sauts de ligne.
    let parts: Vec<String> = text
        .split('\n')
        .map(|p| format!("<w:t xml:space=\"preserve\">{}</w:t>", xml_escape(p)))
        .collect();
    format!("<w:r>{rpr}{}</w:r>", parts.join("<w:br/>"))
}

/// Images (codes QR, signatures), liens et notes du corps du document, à déclarer
/// dans les relations. Sans `actif` (en-tête, pied de page), codes, liens, notes et
/// signatures restent du texte.
#[derive(Default)]
struct Medias {
    actif: bool,
    /// (nom du fichier dans `word/media/`, octets).
    images: Vec<(String, Vec<u8>)>,
    liens: Vec<String>,
    /// Contenu des notes de bas de page (`footnotes.xml`), numérotées à partir de 1.
    notes: Vec<String>,
    signets: usize,
}

/// Une image PNG en ligne, de `l` × `h` millimètres.
fn image_xml(nom: &str, png: Vec<u8>, l: f32, h: f32, descr: &str, m: &mut Medias) -> String {
    let fichier = format!("{nom}{}.png", m.images.len() + 1);
    m.images.push((fichier.clone(), png));
    let n = m.images.len();
    let (cx, cy) = ((l * 36_000.0).round() as i64, (h * 36_000.0).round() as i64);
    format!(
        r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{n}" name="{affiche} {n}" descr="{d}"/><wp:cNvGraphicFramePr><a:graphicFrameLocks xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" noChangeAspect="1"/></wp:cNvGraphicFramePr><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="{n}" name="{fichier}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rIdImg{n}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#,
        d = xml_escape(descr),
        // « QR 1 », « Signature 1 » : noms relus par l'import.
        affiche = if nom == "qr" { "QR" } else { "Signature" }
    )
}

/// Un code QR en image PNG en ligne, de `cote` millimètres.
fn qr_xml(contenu: &str, cote: f32, m: &mut Medias) -> Option<String> {
    let q = crate::qr::QrCode::encode(contenu).ok()?;
    Some(image_xml("qr", q.to_png(8, 2), cote, cote, contenu, m))
}

fn runs_xml(runs: &[Run], d: &Rendered, m: &mut Medias) -> String {
    let mut out = String::new();
    for r in runs {
        let x = match r {
            _ if m.actif && r.qr().is_some() => {
                let q = r.qr().unwrap_or_default();
                qr_xml(q, 20.0, m).unwrap_or_else(|| run_xml(q, r, false))
            }
            _ if m.actif && r.note().is_some() => {
                let k = r.note().unwrap_or_default();
                let contenu = runs_xml(d.note(k), d, m);
                m.notes.push(contenu);
                format!(
                    "<w:r><w:rPr><w:rStyle w:val=\"FootnoteReference\"/></w:rPr><w:footnoteReference w:id=\"{}\"/></w:r>",
                    m.notes.len()
                )
            }
            _ if m.actif && r.signature().is_some() => {
                let nom = r.signature().unwrap_or_default();
                match d.signatures.get(nom) {
                    Some(s) => {
                        let (l, h) = s.taille_mm();
                        image_xml("signature", s.png(200), l, h, nom, m)
                    }
                    None => run_xml("____________________", r, false),
                }
            }
            Run::Math { tex, .. } if m.actif => crate::formule::omml_formule(tex, false),
            _ if m.actif && r.lien().is_some() => {
                let (url, label) = r.lien().unwrap_or_default();
                m.liens.push(crate::doc::adresse_lien(url));
                let n = m.liens.len();
                let mut x = run_xml(label, r, false);
                x = x.replacen(
                    "<w:r>",
                    "<w:r><w:rPr><w:color w:val=\"1F5FBF\"/><w:u w:val=\"single\"/></w:rPr>",
                    1,
                );
                // Un seul <w:rPr> : on fusionne si le morceau en avait déjà un.
                x = x.replacen("</w:rPr><w:rPr>", "", 1);
                format!("<w:hyperlink r:id=\"rIdLien{n}\" w:history=\"1\">{x}</w:hyperlink>")
            }
            Run::Text { text, .. } | Run::Value { text, .. } => run_xml(text, r, false),
            Run::Missing { raw, .. } => run_xml(raw, r, true),
            Run::Math { .. } => run_xml(&r.plain(), r, false),
        };
        out.push_str(&x);
    }
    out
}

fn jc(a: Align) -> &'static str {
    match a {
        Align::Left => "<w:jc w:val=\"left\"/>",
        Align::Center => "<w:jc w:val=\"center\"/>",
        Align::Right => "<w:jc w:val=\"right\"/>",
        Align::Justify => "<w:jc w:val=\"both\"/>",
    }
}

/// Espacements, interligne et retraits d'un paragraphe (`w:spacing`, `w:ind`).
fn format_xml(f: &ParaFormat) -> String {
    let mut s = String::new();
    let mut sp = String::new();
    if let Some(v) = f.avant {
        sp.push_str(&format!(" w:before=\"{}\"", (v * 20.0).round() as i32));
    }
    if let Some(v) = f.apres {
        sp.push_str(&format!(" w:after=\"{}\"", (v * 20.0).round() as i32));
    }
    match f.interligne {
        Some(Interligne::Multiple(m)) => sp.push_str(&format!(
            " w:line=\"{}\" w:lineRule=\"auto\"",
            (240.0 * m).round() as i32
        )),
        Some(Interligne::Exact(p)) => sp.push_str(&format!(
            " w:line=\"{}\" w:lineRule=\"exact\"",
            (p * 20.0).round() as i32
        )),
        None => {}
    }
    if !sp.is_empty() {
        s.push_str(&format!("<w:spacing{sp}/>"));
    }
    let mut ind = String::new();
    if let Some(v) = f.gauche {
        ind.push_str(&format!(" w:left=\"{}\"", tw(v)));
    }
    if let Some(v) = f.droite {
        ind.push_str(&format!(" w:right=\"{}\"", tw(v)));
    }
    match f.alinea {
        Some(v) if v < 0.0 => ind.push_str(&format!(" w:hanging=\"{}\"", tw(-v))),
        Some(v) => ind.push_str(&format!(" w:firstLine=\"{}\"", tw(v))),
        None => {}
    }
    if !ind.is_empty() {
        s.push_str(&format!("<w:ind{ind}/>"));
    }
    s
}

fn table_xml(t: &Table, text_width: i32, d: &Rendered, m: &mut Medias) -> String {
    let cols = t.columns().max(1);
    let col_w = text_width / cols as i32;
    let border = "w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"444444\"";
    let mut s = format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"{text_width}\" w:type=\"dxa\"/><w:tblBorders><w:top {border}/><w:left {border}/><w:bottom {border}/><w:right {border}/><w:insideH {border}/><w:insideV {border}/></w:tblBorders><w:tblCellMar><w:left w:w=\"80\" w:type=\"dxa\"/><w:right w:w=\"80\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr><w:tblGrid>"
    );
    for _ in 0..cols {
        s.push_str(&format!("<w:gridCol w:w=\"{col_w}\"/>"));
    }
    s.push_str("</w:tblGrid>");
    for (i, row) in t.rows.iter().enumerate() {
        let head = i == 0 && t.header;
        s.push_str(if head {
            "<w:tr><w:trPr><w:tblHeader/></w:trPr>"
        } else {
            "<w:tr>"
        });
        for c in 0..cols {
            let mut runs = row.get(c).cloned().unwrap_or_default();
            if head {
                for r in &mut runs {
                    r.style_mut().bold = true;
                }
            }
            let shade = if head {
                "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"F0EEE9\"/>"
            } else {
                ""
            };
            s.push_str(&format!(
                "<w:tc><w:tcPr><w:tcW w:w=\"{col_w}\" w:type=\"dxa\"/>{shade}</w:tcPr><w:p><w:pPr><w:spacing w:after=\"0\"/>{}</w:pPr>{}</w:p></w:tc>",
                jc(t.align(c)),
                runs_xml(&runs, d, m)
            ));
        }
        s.push_str("</w:tr>");
    }
    s.push_str("</w:tbl><w:p><w:pPr><w:spacing w:after=\"0\"/></w:pPr></w:p>");
    s
}

/// Table des matières : un vrai champ TOC de Word (mis à jour à l'ouverture), dont
/// le résultat affiché porte les numéros de page calculés par Varium.
fn sommaire_xml(
    titre: &str,
    entrees: &[(u8, usize, String)],
    doc: usize,
    pages: &[usize],
) -> String {
    let mut s = format!(
        "<w:sdt><w:sdtPr><w:docPartObj><w:docPartGallery w:val=\"Table of Contents\"/><w:docPartUnique/></w:docPartObj></w:sdtPr><w:sdtContent><w:p><w:pPr><w:pStyle w:val=\"TOCHeading\"/></w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        xml_escape(titre)
    );
    let n = entrees.len();
    for (i, (niveau, k, texte)) in entrees.iter().enumerate() {
        let page = pages.get(*k).copied().unwrap_or(1);
        let debut = if i == 0 {
            "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\"> TOC \\o \"1-3\" \\h \\z \\u </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>"
        } else {
            ""
        };
        let fin = if i + 1 == n {
            "<w:r><w:fldChar w:fldCharType=\"end\"/></w:r>"
        } else {
            ""
        };
        s.push_str(&format!(
            "<w:p><w:pPr><w:pStyle w:val=\"TOC{}\"/></w:pPr>{debut}<w:hyperlink w:anchor=\"_Toc_d{doc}_{k}\" w:history=\"1\"><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>{page}</w:t></w:r></w:hyperlink>{fin}</w:p>",
            (*niveau).clamp(1, 3),
            xml_escape(texte)
        ));
    }
    s.push_str("</w:sdtContent></w:sdt>");
    s
}

fn body_xml(docs: &[Rendered], page: &PageSetup, m: &mut Medias) -> String {
    let default_jc = if page.justify {
        Align::Justify
    } else {
        Align::Left
    };
    let text_width = tw(page.text_width_mm());
    let page_break = "<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>";
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
    let mut s = String::new();
    for (i, d) in docs.iter().enumerate() {
        if i > 0 {
            s.push_str(page_break);
        }
        let mut titres = 0usize;
        let mut pi = 0;
        while pi < d.paras.len() {
            let p = &d.paras[pi];
            let f = d.format(pi);
            let fx = format_xml(&f);
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
                s.push_str(&sommaire_xml(
                    &p.plain(),
                    &entrees,
                    i,
                    pages.get(i).map(Vec::as_slice).unwrap_or(&[]),
                ));
                pi = j;
                continue;
            }
            match p {
                Para::Text { align, runs } => {
                    let a = if *align == Align::Left { default_jc } else { *align };
                    let contenu = match p.qr().and_then(|q| qr_xml(q, crate::qr::TAILLE_MM, m)) {
                        Some(x) => x,
                        None => runs_xml(runs, d, m),
                    };
                    s.push_str(&format!("<w:p><w:pPr>{fx}{}</w:pPr>{contenu}</w:p>", jc(a)));
                }
                Para::Heading { level, runs } => {
                    m.signets += 1;
                    let id = m.signets;
                    s.push_str(&format!(
                        "<w:p><w:pPr><w:pStyle w:val=\"Heading{level}\"/>{fx}</w:pPr><w:bookmarkStart w:id=\"{id}\" w:name=\"_Toc_d{i}_{titres}\"/>{}<w:bookmarkEnd w:id=\"{id}\"/></w:p>",
                        runs_xml(runs, d, m)
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
                    let left = 360 * (*depth as i32 + 1) + f.gauche.map_or(0, tw);
                    let f2 = ParaFormat {
                        gauche: None,
                        alinea: None,
                        ..f
                    };
                    s.push_str(&format!(
                        "<w:p><w:pPr><w:tabs><w:tab w:val=\"left\" w:pos=\"{left}\"/></w:tabs>{}<w:ind w:left=\"{left}\" w:hanging=\"360\"/></w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r><w:r><w:tab/></w:r>{}</w:p>",
                        format_xml(&f2),
                        xml_escape(&label),
                        runs_xml(runs, d, m)
                    ));
                }
                Para::Quote { runs } => s.push_str(&format!(
                    "<w:p><w:pPr><w:pStyle w:val=\"Quote\"/>{fx}</w:pPr>{}</w:p>",
                    runs_xml(runs, d, m)
                )),
                Para::Math { tex } => s.push_str(&format!(
                    "<w:p><w:pPr><w:jc w:val=\"center\"/>{fx}</w:pPr>{}</w:p>",
                    crate::formule::omml_formule(tex, true)
                )),
                Para::Table(t) => s.push_str(&table_xml(t, text_width, d, m)),
                Para::Rule => s.push_str("<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"auto\"/></w:pBdr></w:pPr></w:p>"),
                Para::PageBreak => s.push_str(page_break),
            }
            pi += 1;
        }
    }
    s
}

fn styles_xml(page: &PageSetup) -> String {
    let lang = page.langue.tag();
    let font = page.font.family();
    let sz = (page.font_size * 2.0).round() as i32;
    let line = (240.0 * page.line_spacing).round() as i32;
    let heading = |n: u8, size: i32| {
        format!(
            "<w:style w:type=\"paragraph\" w:styleId=\"Heading{n}\"><w:name w:val=\"heading {n}\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before=\"240\" w:after=\"120\"/><w:outlineLvl w:val=\"{}\"/></w:pPr><w:rPr><w:b/><w:bCs/><w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/></w:rPr></w:style>",
            n - 1
        )
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles {W}><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="{font}" w:hAnsi="{font}" w:eastAsia="{font}" w:cs="{font}"/><w:sz w:val="{sz}"/><w:szCs w:val="{sz}"/><w:lang w:val="{lang}"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="0" w:line="{line}" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>{h1}{h2}{h3}<w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:qFormat/><w:pPr><w:pBdr><w:left w:val="single" w:sz="8" w:space="8" w:color="999999"/></w:pBdr><w:spacing w:before="60" w:after="120"/><w:ind w:left="567" w:right="567"/></w:pPr><w:rPr><w:i/><w:iCs/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Header"><w:name w:val="header"/><w:basedOn w:val="Normal"/><w:pPr><w:jc w:val="center"/></w:pPr><w:rPr><w:color w:val="444444"/><w:sz w:val="18"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Footer"><w:name w:val="footer"/><w:basedOn w:val="Normal"/><w:pPr><w:tabs><w:tab w:val="right" w:pos="{tw}"/></w:tabs></w:pPr><w:rPr><w:color w:val="444444"/><w:sz w:val="18"/></w:rPr></w:style>{extra}</w:styles>"#,
        h1 = heading(1, (sz as f32 * 1.6) as i32),
        h2 = heading(2, (sz as f32 * 1.35) as i32),
        h3 = heading(3, (sz as f32 * 1.15) as i32),
        tw = tw(page.text_width_mm()),
        extra = styles_supplementaires(page),
    )
}

/// Styles des notes de bas de page et de la table des matières.
fn styles_supplementaires(page: &PageSetup) -> String {
    let tw = tw(page.text_width_mm());
    let mut s = String::from(
        r#"<w:style w:type="paragraph" w:styleId="FootnoteText"><w:name w:val="footnote text"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:sz w:val="18"/><w:szCs w:val="18"/></w:rPr></w:style><w:style w:type="character" w:styleId="FootnoteReference"><w:name w:val="footnote reference"/><w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="TOCHeading"><w:name w:val="TOC Heading"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="120"/></w:pPr><w:rPr><w:b/><w:bCs/></w:rPr></w:style>"#,
    );
    for n in 1..=3 {
        s.push_str(&format!(
            r#"<w:style w:type="paragraph" w:styleId="TOC{n}"><w:name w:val="toc {n}"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:pPr><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="{tw}"/></w:tabs><w:ind w:left="{}"/></w:pPr></w:style>"#,
            280 * (n - 1)
        ));
    }
    s
}

/// Le fichier `.docx` (une archive ZIP).
pub fn docx(title: &str, docs: &[Rendered], page: &PageSetup) -> Vec<u8> {
    let first = docs.first();
    let mut sans = Medias::default();
    let header = first
        .map(|d| runs_xml(&d.header, d, &mut sans))
        .unwrap_or_default();
    let footer = first
        .map(|d| runs_xml(&d.footer, d, &mut sans))
        .unwrap_or_default();
    let mut medias = Medias {
        actif: true,
        ..Medias::default()
    };
    let body = body_xml(docs, page, &mut medias);
    let numbers = if page.page_numbers {
        "<w:r><w:t xml:space=\"preserve\">Page </w:t></w:r><w:fldSimple w:instr=\"PAGE\"><w:r><w:t>1</w:t></w:r></w:fldSimple><w:r><w:t xml:space=\"preserve\"> sur </w:t></w:r><w:fldSimple w:instr=\"NUMPAGES\"><w:r><w:t>1</w:t></w:r></w:fldSimple>"
    } else {
        ""
    };
    let has_header = !header.is_empty();
    let has_footer = !footer.is_empty() || page.page_numbers;

    let (w, h) = page.size_mm();
    let orient = if page.landscape {
        " w:orient=\"landscape\""
    } else {
        ""
    };
    let mut refs = String::new();
    if has_header {
        refs.push_str("<w:headerReference w:type=\"default\" r:id=\"rIdHeader\"/>");
    }
    if has_footer {
        refs.push_str("<w:footerReference w:type=\"default\" r:id=\"rIdFooter\"/>");
    }
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document {W}><w:body>{body}<w:sectPr>{refs}<w:pgSz w:w="{w}" w:h="{h}"{orient}/><w:pgMar w:top="{mt}" w:right="{mr}" w:bottom="{mb}" w:left="{ml}" w:header="425" w:footer="425" w:gutter="0"/></w:sectPr></w:body></w:document>"#,
        w = tw(w),
        h = tw(h),
        mt = tw(page.margin_top),
        mr = tw(page.margin_right),
        mb = tw(page.margin_bottom),
        ml = tw(page.margin_left),
    );

    let mut rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#,
    );
    let mut overrides = String::new();
    if has_header {
        rels.push_str(r#"<Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>"#);
        overrides.push_str(r#"<Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>"#);
    }
    if has_footer {
        rels.push_str(r#"<Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>"#);
        overrides.push_str(r#"<Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>"#);
    }
    for (i, (nom, _)) in medias.images.iter().enumerate() {
        rels.push_str(&format!("<Relationship Id=\"rIdImg{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/{nom}\"/>", i + 1));
    }
    let has_notes = !medias.notes.is_empty();
    if has_notes {
        rels.push_str(r#"<Relationship Id="rIdNotes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/>"#);
        overrides.push_str(r#"<Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/>"#);
    }
    for (i, url) in medias.liens.iter().enumerate() {
        rels.push_str(&format!("<Relationship Id=\"rIdLien{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink\" Target=\"{}\" TargetMode=\"External\"/>", i + 1, xml_escape(url)));
    }
    rels.push_str("</Relationships>");

    let content_types = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>{overrides}</Types>"#
    );
    let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#;
    let core = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>{}</dc:title><dc:creator>{}</dc:creator><dc:language>{}</dc:language></cp:coreProperties>"#,
        xml_escape(title),
        xml_escape(if page.auteur.trim().is_empty() {
            "Varium"
        } else {
            page.auteur.trim()
        }),
        page.langue.tag()
    );

    let mut zip = Zip::new();
    zip.add("[Content_Types].xml", content_types.as_bytes());
    zip.add("_rels/.rels", root_rels.as_bytes());
    zip.add("docProps/core.xml", core.as_bytes());
    zip.add("word/document.xml", document.as_bytes());
    zip.add("word/styles.xml", styles_xml(page).as_bytes());
    zip.add("word/_rels/document.xml.rels", rels.as_bytes());
    for (nom, png) in &medias.images {
        zip.add(&format!("word/media/{nom}"), png);
    }
    if has_notes {
        let mut x = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes {W}><w:footnote w:type="separator" w:id="-1"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>"#
        );
        for (i, contenu) in medias.notes.iter().enumerate() {
            x.push_str(&format!(
                r#"<w:footnote w:id="{}"><w:p><w:pPr><w:pStyle w:val="FootnoteText"/></w:pPr><w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> </w:t></w:r>{contenu}</w:p></w:footnote>"#,
                i + 1
            ));
        }
        x.push_str("</w:footnotes>");
        zip.add("word/footnotes.xml", x.as_bytes());
    }
    if has_header {
        zip.add(
            "word/header1.xml",
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr {W}><w:p><w:pPr><w:pStyle w:val="Header"/></w:pPr>{header}</w:p></w:hdr>"#
            )
            .as_bytes(),
        );
    }
    if has_footer {
        let sep = if !footer.is_empty() && !numbers.is_empty() {
            "<w:r><w:tab/></w:r>"
        } else {
            ""
        };
        zip.add(
            "word/footer1.xml",
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:ftr {W}><w:p><w:pPr><w:pStyle w:val="Footer"/></w:pPr>{footer}{sep}{numbers}</w:p></w:ftr>"#
            )
            .as_bytes(),
        );
    }
    zip.finish()
}
