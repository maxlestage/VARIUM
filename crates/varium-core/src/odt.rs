//! Export OpenDocument Texte (`.odt`), le format de LibreOffice Writer.

use crate::doc::{Align, Para, Run, Style, Table};
use crate::export::{Rendered, xml_escape};
use crate::page::PageSetup;
use crate::zip::Zip;

const NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" xmlns:dc="http://purl.org/dc/elements/1.1/" office:version="1.3""#;

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

fn runs_xml(runs: &[Run]) -> String {
    let mut out = String::new();
    for r in runs {
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
        let name = if missing {
            "Tmiss".to_string()
        } else if style.mask() != 0 {
            format!("T{}", style.mask())
        } else {
            String::new()
        };
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

/// Styles de caractères automatiques : T1 à T15 (combinaisons gras/italique/souligné/barré).
fn text_styles() -> String {
    let mut s = String::new();
    for m in 1u8..16 {
        let mut props = String::new();
        if m & 1 != 0 {
            props.push_str(" fo:font-weight=\"bold\" style:font-weight-asian=\"bold\" style:font-weight-complex=\"bold\"");
        }
        if m & 2 != 0 {
            props.push_str(" fo:font-style=\"italic\" style:font-style-asian=\"italic\" style:font-style-complex=\"italic\"");
        }
        if m & 4 != 0 {
            props.push_str(" style:text-underline-style=\"solid\" style:text-underline-width=\"auto\" style:text-underline-color=\"font-color\"");
        }
        if m & 8 != 0 {
            props.push_str(
                " style:text-line-through-style=\"solid\" style:text-line-through-type=\"single\"",
            );
        }
        s.push_str(&format!(
            "<style:style style:name=\"T{m}\" style:family=\"text\"><style:text-properties{props}/></style:style>"
        ));
    }
    s.push_str("<style:style style:name=\"Tmiss\" style:family=\"text\"><style:text-properties fo:color=\"#a1201b\" fo:background-color=\"#fde2e1\"/></style:style>");
    s
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
    for (name, a) in [
        ("Pl", Align::Left),
        ("Pc", Align::Center),
        ("Pr", Align::Right),
        ("Pj", Align::Justify),
    ] {
        s.push_str(&format!(
            "<style:style style:name=\"{name}\" style:family=\"paragraph\" style:parent-style-name=\"Standard\"><style:paragraph-properties fo:text-align=\"{}\"/></style:style>",
            align_attr(a)
        ));
    }
    for d in 0..6 {
        let left = 6.35 * (d as f32 + 1.0);
        s.push_str(&format!(
            "<style:style style:name=\"L{d}\" style:family=\"paragraph\" style:parent-style-name=\"Standard\"><style:paragraph-properties fo:margin-left=\"{}\" fo:text-indent=\"-6.35mm\"/></style:style>",
            mm(left)
        ));
    }
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
    s
}

fn table_xml(t: &Table, n: &mut usize) -> String {
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
            let cell = row.get(c).map(|r| runs_xml(r)).unwrap_or_default();
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

fn body_xml(docs: &[Rendered], page: &PageSetup) -> String {
    let default_style = if page.justify { "Pj" } else { "Pl" };
    let mut s = String::new();
    let mut tables = 0;
    for (i, d) in docs.iter().enumerate() {
        if i > 0 {
            s.push_str("<text:p text:style-name=\"PB\"/>");
        }
        for p in &d.paras {
            match p {
                Para::Text { align, runs } => {
                    let style = match align {
                        Align::Left => default_style,
                        Align::Center => "Pc",
                        Align::Right => "Pr",
                        Align::Justify => "Pj",
                    };
                    s.push_str(&format!("<text:p text:style-name=\"{style}\">{}</text:p>", runs_xml(runs)));
                }
                Para::Heading { level, runs } => s.push_str(&format!(
                    "<text:h text:style-name=\"Heading_20_{level}\" text:outline-level=\"{level}\">{}</text:h>",
                    runs_xml(runs)
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
                    s.push_str(&format!(
                        "<text:p text:style-name=\"L{}\">{}<text:tab/>{}</text:p>",
                        depth.min(&5),
                        xml_escape(&label),
                        runs_xml(runs)
                    ));
                }
                Para::Quote { runs } => s.push_str(&format!(
                    "<text:p text:style-name=\"Quotations\">{}</text:p>",
                    runs_xml(runs)
                )),
                Para::Math { .. } => s.push_str(&format!(
                    "<text:p text:style-name=\"Pc\"><text:span text:style-name=\"T2\">{}</text:span></text:p>",
                    odf_text(&p.plain())
                )),
                Para::Table(t) => s.push_str(&table_xml(t, &mut tables)),
                Para::Rule => s.push_str("<text:p text:style-name=\"Hr\"/>"),
                Para::PageBreak => s.push_str("<text:p text:style-name=\"PB\"/>"),
            }
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
    let header = first.map(|d| runs_xml(&d.header)).unwrap_or_default();
    let footer = first.map(|d| runs_xml(&d.footer)).unwrap_or_default();
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
<office:document-styles {NS}>{fonts}<office:styles><style:default-style style:family="paragraph"><style:paragraph-properties fo:hyphenation-ladder-count="no-limit" style:writing-mode="page"/><style:text-properties style:font-name="{font}" fo:font-size="{size}" fo:language="{lang}" fo:country="{country}"/></style:default-style><style:style style:name="Standard" style:family="paragraph" style:class="text"><style:paragraph-properties fo:line-height="{lh}"/><style:text-properties style:font-name="{font}" fo:font-size="{size}"/></style:style>{h1}{h2}{h3}<style:style style:name="Quotations" style:family="paragraph" style:parent-style-name="Standard" style:class="html"><style:paragraph-properties fo:margin-left="10mm" fo:margin-right="10mm" fo:margin-top="1mm" fo:margin-bottom="2mm" fo:padding-left="3mm" fo:border-left="1pt solid #999999"/><style:text-properties fo:font-style="italic"/></style:style><style:style style:name="Table_20_Contents" style:display-name="Table Contents" style:family="paragraph" style:parent-style-name="Standard" style:class="extra"/><style:style style:name="Header" style:family="paragraph" style:parent-style-name="Standard" style:class="extra"><style:paragraph-properties fo:text-align="center"/><style:text-properties fo:font-size="9pt" fo:color="#444444"/></style:style><style:style style:name="Footer" style:family="paragraph" style:parent-style-name="Standard" style:class="extra"><style:paragraph-properties><style:tab-stops><style:tab-stop style:position="{half}" style:type="center"/><style:tab-stop style:position="{text_w}" style:type="right"/></style:tab-stops></style:paragraph-properties><style:text-properties fo:font-size="9pt" fo:color="#444444"/></style:style></office:styles><office:automatic-styles>{ts}<style:page-layout style:name="pm1"><style:page-layout-properties fo:page-width="{w}" fo:page-height="{h}" style:print-orientation="{orientation}" fo:margin-top="{mt}" fo:margin-bottom="{mb}" fo:margin-left="{ml}" fo:margin-right="{mr}"/><style:header-style><style:header-footer-properties fo:min-height="0mm" fo:margin-bottom="4mm"/></style:header-style><style:footer-style><style:header-footer-properties fo:min-height="0mm" fo:margin-top="4mm"/></style:footer-style></style:page-layout></office:automatic-styles><office:master-styles><style:master-page style:name="Standard" style:page-layout-name="pm1">{header_xml}{footer_xml}</style:master-page></office:master-styles></office:document-styles>"##,
        fonts = font_decls(),
        h1 = heading(1, "160%"),
        h2 = heading(2, "135%"),
        h3 = heading(3, "115%"),
        ts = text_styles(),
        w = mm(w),
        h = mm(h),
        mt = mm(page.margin_top),
        mb = mm(page.margin_bottom),
        ml = mm(page.margin_left),
        mr = mm(page.margin_right),
        half = mm(page.text_width_mm() / 2.0),
        text_w = mm(page.text_width_mm()),
    )
}

/// Le fichier `.odt` (une archive ZIP), prêt à ouvrir dans LibreOffice Writer.
pub fn odt(title: &str, docs: &[Rendered], page: &PageSetup) -> Vec<u8> {
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content {NS}>{fonts}<office:automatic-styles>{ts}{ps}</office:automatic-styles><office:body><office:text>{body}</office:text></office:body></office:document-content>"#,
        fonts = font_decls(),
        ts = text_styles(),
        ps = para_styles(),
        body = body_xml(docs, page),
    );
    let meta = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-meta {NS}><office:meta><meta:generator>Varium</meta:generator><dc:title>{}</dc:title><dc:language>{}</dc:language></office:meta></office:document-meta>"#,
        xml_escape(title),
        page.langue.tag()
    );
    let manifest = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3"><manifest:file-entry manifest:full-path="/" manifest:version="1.3" manifest:media-type="application/vnd.oasis.opendocument.text"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/></manifest:manifest>"#;

    let mut zip = Zip::new();
    // `mimetype` doit être le premier fichier, non compressé.
    zip.add("mimetype", b"application/vnd.oasis.opendocument.text");
    zip.add("content.xml", content.as_bytes());
    zip.add("styles.xml", styles_xml(docs, page).as_bytes());
    zip.add("meta.xml", meta.as_bytes());
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
