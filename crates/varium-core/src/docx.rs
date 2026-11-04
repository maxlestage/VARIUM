//! Export Word (`.docx`, Office Open XML), lisible aussi par LibreOffice et Google Docs.

use crate::doc::{Align, Para, Run, Table};
use crate::export::{Rendered, xml_escape};
use crate::page::PageSetup;
use crate::zip::Zip;

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing""#;

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
    if st.underline {
        props.push_str("<w:u w:val=\"single\"/>");
    }
    if st.strike {
        props.push_str("<w:strike/>");
    }
    if missing {
        props.push_str("<w:color w:val=\"A1201B\"/><w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"FDE2E1\"/>");
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

/// Images (codes QR) et liens du corps du document, à déclarer dans les relations.
/// Sans `actif` (en-tête, pied de page), codes et liens restent du texte.
#[derive(Default)]
struct Medias {
    actif: bool,
    images: Vec<Vec<u8>>,
    liens: Vec<String>,
}

/// Un code QR en image PNG en ligne, de `cote` millimètres.
fn qr_xml(contenu: &str, cote: f32, m: &mut Medias) -> Option<String> {
    let q = crate::qr::QrCode::encode(contenu).ok()?;
    m.images.push(q.to_png(8, 2));
    let n = m.images.len();
    let emu = (cote * 36_000.0).round() as i64;
    Some(format!(
        r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{emu}" cy="{emu}"/><wp:docPr id="{n}" name="QR {n}" descr="{d}"/><wp:cNvGraphicFramePr><a:graphicFrameLocks xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" noChangeAspect="1"/></wp:cNvGraphicFramePr><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="{n}" name="qr{n}.png"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rIdQr{n}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{emu}" cy="{emu}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#,
        d = xml_escape(contenu)
    ))
}

fn runs_xml(runs: &[Run], m: &mut Medias) -> String {
    runs.iter()
        .map(|r| match r {
            _ if m.actif && r.qr().is_some() => {
                let q = r.qr().unwrap_or_default();
                qr_xml(q, 20.0, m).unwrap_or_else(|| run_xml(q, r, false))
            }
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
        })
        .collect()
}

fn jc(a: Align) -> &'static str {
    match a {
        Align::Left => "<w:jc w:val=\"left\"/>",
        Align::Center => "<w:jc w:val=\"center\"/>",
        Align::Right => "<w:jc w:val=\"right\"/>",
        Align::Justify => "<w:jc w:val=\"both\"/>",
    }
}

fn table_xml(t: &Table, text_width: i32, m: &mut Medias) -> String {
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
                runs_xml(&runs, m)
            ));
        }
        s.push_str("</w:tr>");
    }
    s.push_str("</w:tbl><w:p><w:pPr><w:spacing w:after=\"0\"/></w:pPr></w:p>");
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
    let mut s = String::new();
    for (i, d) in docs.iter().enumerate() {
        if i > 0 {
            s.push_str(page_break);
        }
        for p in &d.paras {
            match p {
                Para::Text { align, runs } => {
                    let a = if *align == Align::Left { default_jc } else { *align };
                    let contenu = match p.qr().and_then(|q| qr_xml(q, crate::qr::TAILLE_MM, m)) {
                        Some(x) => x,
                        None => runs_xml(runs, m),
                    };
                    s.push_str(&format!("<w:p><w:pPr>{}</w:pPr>{contenu}</w:p>", jc(a)));
                }
                Para::Heading { level, runs } => s.push_str(&format!(
                    "<w:p><w:pPr><w:pStyle w:val=\"Heading{level}\"/></w:pPr>{}</w:p>",
                    runs_xml(runs, m)
                )),
                Para::Item {
                    number,
                    depth,
                    runs,
                } => {
                    let label = match number {
                        Some(n) => format!("{n}."),
                        None => ["•", "◦", "▪"][*depth as usize % 3].to_string(),
                    };
                    let left = 360 * (*depth as i32 + 1);
                    s.push_str(&format!(
                        "<w:p><w:pPr><w:tabs><w:tab w:val=\"left\" w:pos=\"{left}\"/></w:tabs><w:ind w:left=\"{left}\" w:hanging=\"360\"/></w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r><w:r><w:tab/></w:r>{}</w:p>",
                        xml_escape(&label),
                        runs_xml(runs, m)
                    ));
                }
                Para::Quote { runs } => s.push_str(&format!(
                    "<w:p><w:pPr><w:pStyle w:val=\"Quote\"/></w:pPr>{}</w:p>",
                    runs_xml(runs, m)
                )),
                Para::Math { .. } => s.push_str(&format!(
                    "<w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:r><w:rPr><w:i/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
                    xml_escape(&p.plain())
                )),
                Para::Table(t) => s.push_str(&table_xml(t, text_width, m)),
                Para::Rule => s.push_str("<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"auto\"/></w:pBdr></w:pPr></w:p>"),
                Para::PageBreak => s.push_str(page_break),
            }
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
<w:styles {W}><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="{font}" w:hAnsi="{font}" w:eastAsia="{font}" w:cs="{font}"/><w:sz w:val="{sz}"/><w:szCs w:val="{sz}"/><w:lang w:val="{lang}"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="0" w:line="{line}" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>{h1}{h2}{h3}<w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:qFormat/><w:pPr><w:pBdr><w:left w:val="single" w:sz="8" w:space="8" w:color="999999"/></w:pBdr><w:spacing w:before="60" w:after="120"/><w:ind w:left="567" w:right="567"/></w:pPr><w:rPr><w:i/><w:iCs/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Header"><w:name w:val="header"/><w:basedOn w:val="Normal"/><w:pPr><w:jc w:val="center"/></w:pPr><w:rPr><w:color w:val="444444"/><w:sz w:val="18"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Footer"><w:name w:val="footer"/><w:basedOn w:val="Normal"/><w:pPr><w:tabs><w:tab w:val="right" w:pos="{tw}"/></w:tabs></w:pPr><w:rPr><w:color w:val="444444"/><w:sz w:val="18"/></w:rPr></w:style></w:styles>"#,
        h1 = heading(1, (sz as f32 * 1.6) as i32),
        h2 = heading(2, (sz as f32 * 1.35) as i32),
        h3 = heading(3, (sz as f32 * 1.15) as i32),
        tw = tw(page.text_width_mm()),
    )
}

/// Le fichier `.docx` (une archive ZIP).
pub fn docx(title: &str, docs: &[Rendered], page: &PageSetup) -> Vec<u8> {
    let first = docs.first();
    let mut sans = Medias::default();
    let header = first
        .map(|d| runs_xml(&d.header, &mut sans))
        .unwrap_or_default();
    let footer = first
        .map(|d| runs_xml(&d.footer, &mut sans))
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
    for n in 1..=medias.images.len() {
        rels.push_str(&format!("<Relationship Id=\"rIdQr{n}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/qr{n}.png\"/>"));
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
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>{}</dc:title><dc:creator>Varium</dc:creator><dc:language>{}</dc:language></cp:coreProperties>"#,
        xml_escape(title),
        page.langue.tag()
    );

    let mut zip = Zip::new();
    zip.add("[Content_Types].xml", content_types.as_bytes());
    zip.add("_rels/.rels", root_rels.as_bytes());
    zip.add("docProps/core.xml", core.as_bytes());
    zip.add("word/document.xml", document.as_bytes());
    zip.add("word/styles.xml", styles_xml(page).as_bytes());
    zip.add("word/_rels/document.xml.rels", rels.as_bytes());
    for (i, png) in medias.images.iter().enumerate() {
        zip.add(&format!("word/media/qr{}.png", i + 1), png);
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
