//! Ouvrir des `.odt` et des `.docx` : nos propres exports reviennent avec la même
//! structure, les documents écrits par LibreOffice s'ouvrent, et les archives
//! abîmées donnent une erreur en français, jamais une panique.

use varium_core::doc::{Para, Run};
use varium_core::import::importer;
use varium_core::{DocFont, Lang, PageSetup, Paper, Values, document, docx, export, odt};

/// La structure d'un document, en texte : un élément par paragraphe non vide.
pub fn structure(paras: &[Para]) -> Vec<String> {
    fn runs(rs: &[Run]) -> String {
        let mut out = String::new();
        let mut fus: Vec<(String, varium_core::doc::Style, Option<String>)> = Vec::new();
        for r in rs {
            let lien = r.lien().map(|(u, _)| u.to_string());
            let t = r.plain();
            match fus.last_mut() {
                Some(l) if l.1 == r.style() && l.2 == lien => l.0.push_str(&t),
                _ => fus.push((t, r.style(), lien)),
            }
        }
        for (t, s, lien) in fus {
            let mut t = t.replace('\u{200B}', "");
            if let Some(u) = lien {
                t = format!("[{t}]({u})");
            }
            if s.bold {
                t = format!("<b>{t}</b>");
            }
            if s.italic {
                t = format!("<i>{t}</i>");
            }
            if s.underline {
                t = format!("<u>{t}</u>");
            }
            if s.strike {
                t = format!("<s>{t}</s>");
            }
            out.push_str(&t);
        }
        out.trim().to_string()
    }
    paras
        .iter()
        .filter_map(|p| {
            Some(match p {
                Para::Text { align, runs: r } => {
                    let t = runs(r);
                    if t.is_empty() {
                        return None;
                    }
                    format!("P{align:?}:{t}")
                }
                Para::Heading { level, runs: r } => format!("H{level}:{}", runs(r)),
                Para::Item {
                    number,
                    depth,
                    runs: r,
                } => format!(
                    "L{depth}{}:{}",
                    number.map_or("•".into(), |n| n.to_string()),
                    runs(r)
                ),
                Para::Quote { runs: r } => format!("Q:{}", runs(r)),
                Para::Math { tex } => format!("M:{tex}"),
                Para::Table(t) => format!(
                    "T{}:{}",
                    if t.header { "h" } else { "" },
                    t.rows
                        .iter()
                        .map(|r| r.iter().map(|c| runs(c)).collect::<Vec<_>>().join("|"))
                        .collect::<Vec<_>>()
                        .join("/")
                ),
                Para::Rule => "HR".into(),
                Para::PageBreak => "PB".into(),
            })
        })
        .collect()
}

pub const EXEMPLE: &str = "# Titre principal
Texte avec **gras**, *italique*, __souligné__ et ~~barré~~, puis [un lien](https://varium.fr).

## Sous-titre {{client}}
- premier
  - imbriqué
- second
1. un
2. deux

> Une citation
-> Centré <-
-> À droite ->
| Nom | Prix |
|---|---:|
| Pomme | 2 € |
| Poire \\| fruit | 3 € |
---
\\newpage
### Après le saut
{{qr:https://varium.fr}}
Prix : 5 \\* 3 \\$, nom\\_de\\_fichier.";

fn page() -> PageSetup {
    PageSetup {
        paper: Paper::A5,
        landscape: true,
        margin_top: 15.0,
        margin_bottom: 18.0,
        margin_left: 22.0,
        margin_right: 12.5,
        font: DocFont::Sans,
        font_size: 11.0,
        typographie: false,
        langue: Lang::De,
        header: "Atelier Lumen".into(),
        footer: "Confidentiel".into(),
        page_numbers: true,
        ..PageSetup::default()
    }
}

fn valeurs() -> Values {
    let mut v = Values::default();
    v.set("client", "Marc");
    v
}

fn verifier_aller_retour(nom: &str, octets: &[u8]) {
    let p = page();
    let attendu = structure(&document(EXEMPLE, &valeurs()));
    let imp = importer(nom, octets).unwrap_or_else(|e| panic!("{nom} : {e}"));
    let relu = structure(&document(&imp.source, &Values::default()));
    assert_eq!(relu, attendu, "{nom} :\n{}", imp.source);
    assert_eq!(imp.page.paper, p.paper, "{nom}");
    assert_eq!(imp.page.landscape, p.landscape, "{nom}");
    for (a, b) in [
        (imp.page.margin_top, p.margin_top),
        (imp.page.margin_bottom, p.margin_bottom),
        (imp.page.margin_left, p.margin_left),
        (imp.page.margin_right, p.margin_right),
    ] {
        assert!((a - b).abs() < 0.2, "{nom} : marge {a} au lieu de {b}");
    }
    assert_eq!(imp.page.langue, Lang::De, "{nom}");
    assert_eq!(imp.page.font, DocFont::Sans, "{nom}");
    assert!((imp.page.font_size - 11.0).abs() < 0.1, "{nom}");
    assert_eq!(imp.page.header, "Atelier Lumen", "{nom}");
    assert_eq!(imp.page.footer, "Confidentiel", "{nom}");
    assert!(imp.page.page_numbers, "{nom}");
    assert_eq!(imp.titre, "Exemple", "{nom}");
    assert!(imp.remarques.is_empty(), "{nom} : {:?}", imp.remarques);
}

#[test]
fn nos_exports_odt_reviennent_a_l_identique() {
    let r = export::prepare(EXEMPLE, &valeurs(), &page());
    verifier_aller_retour("exemple.odt", &odt::odt("Exemple", &[r], &page()));
}

#[test]
fn nos_exports_docx_reviennent_a_l_identique() {
    let r = export::prepare(EXEMPLE, &valeurs(), &page());
    verifier_aller_retour("exemple.docx", &docx::docx("Exemple", &[r], &page()));
}

#[test]
fn le_texte_justifie_devient_la_mise_en_page() {
    let mut p = page();
    p.justify = true;
    let r = export::prepare("Un.\n\nDeux.\n\nTrois.", &Values::default(), &p);
    for (nom, octets) in [
        ("a.odt", odt::odt("x", std::slice::from_ref(&r), &p)),
        ("a.docx", docx::docx("x", std::slice::from_ref(&r), &p)),
    ] {
        let imp = importer(nom, &octets).unwrap();
        assert!(imp.page.justify, "{nom}");
        assert_eq!(imp.source, "Un.\n\nDeux.\n\nTrois.", "{nom}");
    }
}

fn fixture(nom: &str) -> (varium_core::import::Importe, Vec<String>) {
    let dossier = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let octets = std::fs::read(dossier.join(nom)).expect(nom);
    let imp = importer(nom, &octets).unwrap_or_else(|e| panic!("{nom} : {e}"));
    let s = structure(&document(&imp.source, &Values::default()));
    (imp, s)
}

/// Notre export HTML, ouvert puis enregistré par LibreOffice
/// (`soffice --headless --convert-to odt|docx libreoffice.html`).
#[test]
fn notre_html_enregistre_par_libreoffice() {
    for nom in ["libreoffice.odt", "libreoffice.docx"] {
        let (imp, s) = fixture(nom);
        let joint = s.join("\n");
        for attendu in [
            "H1:Titre principal",
            "H2:Sous-titre Marc",
            "<b>gras</b>",
            "<i>italique</i>",
            "<u>souligné</u>",
            "<s>barré</s>",
            "[un lien](https://varium.fr",
            "Q:Une citation",
            "PCenter:Centré",
            "PRight:À droite",
            "H3:Après le saut",
            "Pomme|2 €",
            "Poire | fruit",
            "nom_de_fichier",
            "L0•:premier",
            "L1•:imbriqué",
            "HR",
        ] {
            assert!(
                joint.contains(attendu),
                "{nom} : « {attendu} » absent de\n{joint}\n---\n{}",
                imp.source
            );
        }
        assert!(imp.page.landscape, "{nom}");
        assert!(
            (imp.page.margin_left - 22.0).abs() < 0.5,
            "{nom} : {}",
            imp.page.margin_left
        );
    }
}

/// Un document LibreOffice avec de vraies listes, une note de bas de page, un saut de
/// page et un tableau à en-tête (`libreoffice-listes.html` converti par `soffice`).
#[test]
fn listes_et_notes_de_libreoffice() {
    for nom in ["libreoffice-listes.odt", "libreoffice-listes.docx"] {
        let (imp, s) = fixture(nom);
        let joint = s.join("\n");
        for attendu in [
            "H1:Capitolo uno",
            "[indirizzo](mailto:info@varium.fr)",
            "L0•:primo",
            "L1•:annidato",
            "L0•:secondo",
            "L01:uno",
            "L02:due",
            "Mela|2 €",
            "PB",
            "PLeft:Dopo il salto",
            "PCenter:<b>Centrato</b>",
        ] {
            assert!(
                joint.contains(attendu),
                "{nom} : « {attendu} » absent de\n{joint}\n---\n{}",
                imp.source
            );
        }
        assert!(imp.source.contains("nota[^1]"), "{nom} :\n{}", imp.source);
        assert!(
            imp.source.contains("[^1]: Fonte: ISTAT."),
            "{nom} :\n{}",
            imp.source
        );
        assert_eq!(imp.page.langue, Lang::It, "{nom}");
        assert_eq!(imp.titre, "Liste di prova", "{nom}");
    }
}

#[test]
fn notes_de_bas_de_page_commentaires_et_images() {
    // Un ODT écrit à la main, comme LibreOffice les écrit.
    let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0">
<office:automatic-styles><style:style style:name="T1" style:family="text"><style:text-properties fo:font-weight="bold"/></style:style></office:automatic-styles>
<office:body><office:text>
<text:p>Une phrase<text:note text:id="ftn1" text:note-class="footnote"><text:note-citation>1</text:note-citation><text:note-body><text:p>La <text:span text:style-name="T1">note</text:span>.</text:p></text:note-body></text:note> et  une<text:s text:c="2"/>autre<office:annotation><dc:creator>Léa</dc:creator><dc:date>2026-11-02T10:00:00</dc:date><text:p>À vérifier</text:p></office:annotation>.</text:p>
<text:p><draw:frame draw:name="Image1"><draw:image/></draw:frame>Après l'image<text:line-break/>Ligne suivante</text:p>
</office:text></office:body></office:document-content>"#;
    let mut z = varium_core::zip::Zip::new();
    z.add("mimetype", b"application/vnd.oasis.opendocument.text");
    z.add("content.xml", content.as_bytes());
    let imp = importer("main.odt", &z.finish()).unwrap();
    assert_eq!(
        imp.source,
        "Une phrase[^1] et une  autre{>>Léa · 2026-11-02 : À vérifier<<}.\n{>>Image non importée<<}Après l'image\nLigne suivante\n\n[^1]: La **note**."
    );
    assert_eq!(imp.remarques.len(), 1);
    // Les commentaires et marques ne vont pas dans les exports.
    let r = export::prepare(&imp.source, &Values::default(), &PageSetup::default());
    let t = export::text(&[r]);
    assert!(!t.contains("Léa") && !t.contains("Image non"), "{t}");
}

#[test]
fn docx_listes_notes_et_modifications_suivies() {
    let document = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>
<w:p><w:pPr><w:pStyle w:val="Titre1"/></w:pPr><w:r><w:t>Chapitre</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>un</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>sous</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>deux</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Texte </w:t></w:r><w:ins w:author="Max" w:date="2026-11-04T09:00:00Z"><w:r><w:t>ajouté</w:t></w:r></w:ins><w:del w:author="Max" w:date="2026-11-04T09:00:00Z"><w:r><w:delText>retiré</w:delText></w:r></w:del><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>gras</w:t></w:r><w:r><w:rPr><w:b w:val="0"/></w:rPr><w:t xml:space="preserve"> normal</w:t></w:r><w:r><w:br w:type="page"/></w:r><w:r><w:t>Page deux</w:t></w:r></w:p>
<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr>
</w:body></w:document>"#;
    let styles = r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri"/><w:sz w:val="22"/><w:lang w:val="en-US"/></w:rPr></w:rPrDefault></w:docDefaults><w:style w:type="paragraph" w:styleId="Titre1"><w:name w:val="heading 1"/></w:style></w:styles>"#;
    let numbering = r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/></w:lvl><w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#;
    let notes = r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="0"><w:p/></w:footnote><w:footnote w:id="1"><w:p><w:r><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> Source : INSEE.</w:t></w:r></w:p></w:footnote></w:footnotes>"#;
    let mut z = varium_core::zip::Zip::new();
    z.add("word/document.xml", document.as_bytes());
    z.add("word/styles.xml", styles.as_bytes());
    z.add("word/numbering.xml", numbering.as_bytes());
    z.add("word/footnotes.xml", notes.as_bytes());
    let imp = importer("word.docx", &z.finish()).unwrap();
    assert_eq!(
        imp.source,
        "# Chapitre\n1. un\n  - sous\n2. deux\nTexte {++ajouté++}{>>Max · 2026-11-04<<}{--retiré--}{>>Max · 2026-11-04<<}[^1]**gras** normal\n\\newpage\nPage deux\n\n[^1]: Source : INSEE."
    );
    assert_eq!(imp.page.paper, Paper::Letter);
    assert_eq!(imp.page.langue, Lang::En);
    assert_eq!(imp.page.font, DocFont::Sans);
    assert!((imp.page.margin_left - 25.4).abs() < 0.1);
}

struct Alea(u64);
impl Alea {
    fn borne(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n.max(1) as u64) as usize
    }
}

#[test]
fn archives_tronquees_ou_corrompues_sans_panique() {
    let r = export::prepare(EXEMPLE, &valeurs(), &page());
    let mut a = Alea(0x9E37_79B9_7F4A_7C15);
    for (nom, octets) in [
        ("x.odt", odt::odt("x", std::slice::from_ref(&r), &page())),
        ("x.docx", docx::docx("x", std::slice::from_ref(&r), &page())),
    ] {
        // Tronqué à toutes les longueurs (par pas).
        for n in (0..octets.len()).step_by(97) {
            if let Err(e) = importer(nom, &octets[..n]) {
                assert!(!e.is_empty());
            }
        }
        // Octets modifiés au hasard.
        for _ in 0..400 {
            let mut o = octets.clone();
            for _ in 0..1 + a.borne(8) {
                let i = a.borne(o.len());
                o[i] = a.borne(256) as u8;
            }
            let _ = importer(nom, &o);
        }
    }
    // XML abîmé dans une archive valide.
    let mut z = varium_core::zip::Zip::new();
    z.add(
        "word/document.xml",
        b"<w:document><w:body><w:p><w:r><w:t>abc",
    );
    let imp = importer("t.docx", &z.finish()).unwrap();
    assert_eq!(imp.source, "abc");
    for xml in ["", "<", "<<<>>>", "<a x=\"></a>", "\u{0}\u{1}"] {
        let mut z = varium_core::zip::Zip::new();
        z.add("content.xml", xml.as_bytes());
        let _ = importer("t.odt", &z.finish());
    }
    // Imbrication très profonde.
    let mut profond = String::from("<office:document-content><office:body><office:text><text:p>");
    for _ in 0..600 {
        profond.push_str("<text:span>");
    }
    profond.push_str("fond</text:p></office:text></office:body></office:document-content>");
    let mut z = varium_core::zip::Zip::new();
    z.add("content.xml", profond.as_bytes());
    let _ = importer("t.odt", &z.finish());
}

/// Régénère `tests/fixtures/libreoffice.html`, à convertir ensuite avec
/// `soffice --headless --convert-to odt` (puis `docx`) :
/// `VARIUM_FIXTURES=1 cargo test -p varium-core --test import -- --ignored`.
#[test]
#[ignore]
fn ecrire_le_html_des_fixtures() {
    if std::env::var_os("VARIUM_FIXTURES").is_none() {
        return;
    }
    let r = export::prepare(EXEMPLE, &valeurs(), &page());
    let html = export::html("Exemple", &[r], &page());
    let dossier = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    std::fs::create_dir_all(&dossier).unwrap();
    std::fs::write(dossier.join("libreoffice.html"), html).unwrap();
}
