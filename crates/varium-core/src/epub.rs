//! Export EPUB 3 (livre numérique), comme « Exporter vers → EPUB » dans Pages.
//! Chaque document (un par destinataire en publipostage) devient un chapitre.

use crate::doc::Para;
use crate::export::{Html, Rendered, xml_escape};
use crate::page::PageSetup;
use crate::zip::Zip;

fn chapter(title: &str, d: &Rendered, n: usize, lang: &str) -> String {
    let h = Html { d, n, xhtml: true };
    let mut body = String::new();
    h.paras(&mut body, &mut |o: &mut String| {
        o.push_str("<div class=\"saut\"></div>\n")
    });
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{lang}" lang="{lang}">
<head><meta charset="utf-8"/><title>{}</title><link rel="stylesheet" type="text/css" href="style.css"/></head>
<body>
{body}</body>
</html>
"#,
        xml_escape(title)
    )
}

/// Propriétés EPUB d'un chapitre : MathML et SVG (codes QR, signatures).
fn proprietes(c: &str) -> String {
    let mut p = Vec::new();
    if c.contains("<math") {
        p.push("mathml");
    }
    if c.contains("<svg") {
        p.push("svg");
    }
    if p.is_empty() {
        String::new()
    } else {
        format!(" properties=\"{}\"", p.join(" "))
    }
}

/// Le fichier `.epub` (une archive ZIP).
pub fn epub(title: &str, docs: &[Rendered], page: &PageSetup) -> Vec<u8> {
    let lang = page.langue.code();
    let auteur = xml_escape(if page.auteur.trim().is_empty() {
        "Varium"
    } else {
        page.auteur.trim()
    });
    let t = xml_escape(if title.trim().is_empty() {
        "Document Varium"
    } else {
        title
    });
    let id = format!(
        "urn:varium:{:016x}",
        docs.iter()
            .flat_map(|d| d.paras.iter().map(Para::plain))
            .fold(0xcbf29ce484222325u64, |h, s| {
                s.bytes()
                    .fold(h, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
            })
    );
    let chapters: Vec<String> = docs
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let name = if docs.len() == 1 {
                title.to_string()
            } else {
                format!("{title} ({})", i + 1)
            };
            chapter(&name, d, i, page.langue.code())
        })
        .collect();
    let manifest: String = chapters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            format!(
                "<item id=\"c{i}\" href=\"chapitre-{i}.xhtml\" media-type=\"application/xhtml+xml\"{}/>",
                proprietes(c)
            )
        })
        .collect();
    let spine: String = (0..chapters.len())
        .map(|i| format!("<itemref idref=\"c{i}\"/>"))
        .collect();
    let nav_items: String = (0..chapters.len())
        .map(|i| {
            let label = if chapters.len() == 1 {
                t.clone()
            } else {
                format!("{t} — {}", i + 1)
            };
            format!("<li><a href=\"chapitre-{i}.xhtml\">{label}</a></li>")
        })
        .collect();
    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id" xml:lang="{lang}">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">{id}</dc:identifier><dc:title>{t}</dc:title><dc:language>{lang}</dc:language><dc:creator>{auteur}</dc:creator><meta property="dcterms:modified">2026-01-01T00:00:00Z</meta></metadata>
<manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="css" href="style.css" media-type="text/css"/>{manifest}</manifest>
<spine>{spine}</spine>
</package>
"#
    );
    let nav = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{lang}" lang="{lang}">
<head><meta charset="utf-8"/><title>{t}</title></head>
<body><nav epub:type="toc"><h1>Sommaire</h1><ol>{nav_items}</ol></nav></body>
</html>
"#
    );
    let font = page.font.css();
    let css = format!(
        "body {{ font-family: {font}; line-height: {}; text-align: {}; }}\np {{ margin: 0; }}\n.c {{ text-align: center; }} .r {{ text-align: right; }} .j {{ text-align: justify; }}\nblockquote {{ margin: .4em 1.5em; font-style: italic; }}\ntable {{ border-collapse: collapse; width: 100%; }} th, td {{ border: 1px solid #555; padding: .2em .4em; }}\n.saut {{ page-break-after: always; }}\nmark {{ background: #fde2e1; }}\n.li .puce {{ display: inline-block; width: 1.2em; }}\n.formule {{ text-align: center; margin: .3em 0; }}\nsup.appel a, .notes a, .toc a {{ text-decoration: none; }}\n.notes {{ font-size: .9em; }}\n",
        page.line_spacing * 1.15,
        if page.justify { "justify" } else { "left" }
    );
    let container = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>
"#;
    let mut zip = Zip::new();
    zip.add("mimetype", b"application/epub+zip");
    zip.add("META-INF/container.xml", container.as_bytes());
    zip.add("OEBPS/content.opf", opf.as_bytes());
    zip.add("OEBPS/nav.xhtml", nav.as_bytes());
    zip.add("OEBPS/style.css", css.as_bytes());
    for (i, c) in chapters.iter().enumerate() {
        zip.add(&format!("OEBPS/chapitre-{i}.xhtml"), c.as_bytes());
    }
    zip.finish()
}
