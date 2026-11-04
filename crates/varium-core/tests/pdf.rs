//! Tests de l'export PDF natif : structure du fichier, pagination, en-tête/pied de page,
//! polices incorporées, formules, robustesse sur des entrées absurdes.

use varium_core::export::{Rendered, prepare};
use varium_core::pdf::{Metrics, PdfFonts, pdf};
use varium_core::{PageSetup, Values};

fn polices() -> PdfFonts {
    let d = |f: &str| {
        std::fs::read(format!(
            "{}/../varium/assets/fonts/{f}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("police Liberation")
    };
    PdfFonts::from_ttf(
        &d("LiberationSerif-Regular.ttf"),
        &d("LiberationSerif-Bold.ttf"),
        &d("LiberationSerif-Italic.ttf"),
        &d("LiberationSerif-BoldItalic.ttf"),
    )
}

fn texte(octets: &[u8]) -> String {
    octets
        .iter()
        .map(|&b| if b < 0x80 { b as char } else { '?' })
        .collect()
}

/// Vérifie la structure : en-tête, table xref exacte, nombre de pages. Retourne ce nombre.
fn valider(o: &[u8]) -> usize {
    let s = texte(o);
    assert!(s.starts_with("%PDF-1.7"));
    assert!(s.trim_end().ends_with("%%EOF"));
    let sx = s.rfind("startxref\n").expect("startxref");
    let xref: usize = s[sx + 10..].lines().next().unwrap().parse().unwrap();
    assert!(s[xref..].starts_with("xref\n"));
    let mut lignes = s[xref..].lines().skip(1);
    let nb: usize = lignes
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    lignes.next(); // entrée libre
    for i in 1..nb {
        let off: usize = lignes.next().unwrap()[..10].parse().unwrap();
        assert!(
            s[off..].starts_with(&format!("{i} 0 obj\n")),
            "objet {i} mal placé"
        );
    }
    s.matches("/Type /Page ").count()
}

/// Les flux du fichier, décompressés et mis bout à bout.
fn flux(o: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while let Some(p) = find(&o[i..], b"stream\n") {
        let debut = i + p + 7;
        let Some(f) = find(&o[debut..], b"\nendstream") else {
            break;
        };
        let data = &o[debut..debut + f];
        match miniz_oxide::inflate::decompress_to_vec_zlib(data) {
            Ok(d) => out.push_str(&String::from_utf8_lossy(&d)),
            Err(_) => out.push_str(&String::from_utf8_lossy(data)),
        }
        out.push('\n');
        i = debut + f + 11;
    }
    out
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

/// Le texte des pages, tel que la mise en page le pose.
fn textes_mis_en_page(docs: &[Rendered], page: &PageSetup) -> Vec<String> {
    use varium_core::mise_en_page::{Cache, Element, Options, mettre_en_page};
    let f = polices();
    let m = mettre_en_page(docs, page, &Options::new(&f), &mut Cache::new());
    m.pages
        .iter()
        .map(|p| {
            let mut s = String::new();
            p.pour_chaque(|e| {
                if let Element::Texte(t) = e {
                    s.push_str(&t.texte);
                    s.push(' ');
                }
            });
            s
        })
        .collect()
}

fn rendu(src: &str, page: &PageSetup) -> Rendered {
    prepare(src, &Values::default(), page)
}

#[test]
fn polices_liberation_lues() {
    let d = std::fs::read(format!(
        "{}/../varium/assets/fonts/LiberationSerif-Regular.ttf",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let m = Metrics::from_ttf(&d).unwrap();
    // Largeurs de Times : espace 250, « i » 278, « M » 889 (millièmes d'em).
    assert!((m.width_of(' ') - 250.0).abs() < 1.0);
    assert!((m.width_of('i') - 278.0).abs() < 1.0);
    assert!((m.width_of('M') - 889.0).abs() < 1.0);
    assert!((m.width_of('é') - 444.0).abs() < 1.0);
    assert!(Metrics::from_ttf(b"pas une police").is_none());
    assert!(Metrics::from_ttf(&d[..40]).is_none());
    assert_eq!(
        PdfFonts::from_ttf(&[], &[], &[], &[]),
        PdfFonts::approximate()
    );
}

#[test]
fn document_simple_valide() {
    let page = PageSetup::default();
    let src = "# Titre\n\nBonjour **monde** et *vous*, __soulignés__ ~~barrés~~.\n- un\n- deux\n1. a\n> citation\n---\n| a | b |\n|---|---|\n| 1 | 2 |\n";
    let o = pdf("Essai é", &[rendu(src, &page)], &page, &polices());
    assert_eq!(valider(&o), 1);
}

#[test]
fn texte_extractible_avec_accents() {
    let page = PageSetup::default();
    let o = pdf(
        "t",
        &[rendu(
            "Élève « français » — œuvre € Ωμέγα $\\alpha + \\beta$",
            &page,
        )],
        &page,
        &polices(),
    );
    valider(&o);
    let s = texte(&o);
    // Polices incorporées en sous-ensemble, avec leur table ToUnicode.
    assert!(s.contains("/FontFile2"));
    assert!(s.contains("/CIDFontType2"));
    assert!(s.contains("/Identity-H"));
    let f = flux(&o);
    for c in ['É', 'œ', '€', 'Ω', 'α', 'β'] {
        assert!(
            f.contains(&format!("<{:04X}>", c as u32)),
            "{c} absent de ToUnicode"
        );
    }
    assert!(!f.contains("<003F>"), "aucun « ? » de remplacement");
}

#[test]
fn sans_police_repli_winansi() {
    let page = PageSetup::default();
    let o = pdf(
        "t",
        &[rendu("Élève œuvre €", &page)],
        &page,
        &PdfFonts::approximate(),
    );
    valider(&o);
    let s = texte(&o);
    assert!(s.contains("/WinAnsiEncoding"));
    let f = flux(&o);
    // « É » = 0xC9, « œ » = 0x9C, « € » = 0x80 en WinAnsi.
    assert!(f.contains("C9") && f.contains("9C") && f.contains("80"));
}

#[test]
fn flux_compresses_et_metadonnees() {
    let page = PageSetup {
        auteur: "Zoé Martin".into(),
        ..PageSetup::default()
    };
    let src = "Un texte répété. ".repeat(400);
    let o = pdf("Mon titre", &[rendu(&src, &page)], &page, &polices());
    valider(&o);
    let s = texte(&o);
    assert!(s.contains("/FlateDecode"));
    assert!(s.contains("/Lang (fr-FR)") || s.contains("/Lang (fr"));
    assert!(s.contains("/Author <FEFF"));
    assert!(s.contains("/Type /Metadata"));
    assert!(s.contains("<dc:creator><rdf:Seq><rdf:li>Zo"));
    assert!(o.len() < 60_000, "{} octets", o.len());
}

#[test]
fn formules_composees() {
    let page = PageSetup::default();
    let o = pdf(
        "t",
        &[rendu(
            "$$\\frac{a+b}{\\sqrt{x}} = \\sum_{i=1}^{n} x_i^2$$",
            &page,
        )],
        &page,
        &polices(),
    );
    valider(&o);
    let f = flux(&o);
    // Barre de fraction (rectangle plein) et signe radical (ligne brisée).
    assert!(f.contains(" re f"));
    assert!(f.contains("1 J 1 j"));
    assert!(f.contains(&format!("<{:04X}>", '∑' as u32)));
}

#[test]
fn sommaire_numerote_et_cliquable() {
    let page = PageSetup::default();
    let src = "\\sommaire\n# Un\ntexte\n\\newpage\n# Deux\n";
    let d = rendu(src, &page);
    let t = textes_mis_en_page(std::slice::from_ref(&d), &page);
    assert!(t[0].contains("Un") && t[0].contains(" 1 ") && t[0].contains(" 2 "));
    let o = pdf("t", &[d], &page, &polices());
    valider(&o);
    // Deux liens internes vers les titres, plus les deux signets.
    assert_eq!(texte(&o).matches("/Dest [").count(), 4);
}

#[test]
fn notes_et_signature() {
    let page = PageSetup::default();
    varium_core::signature::definir(
        "essai-pdf",
        varium_core::signature::Signature::depuis_traits(&[vec![
            (0.0, 0.0),
            (40.0, 20.0),
            (80.0, 5.0),
        ]])
        .unwrap(),
    );
    let src = "Texte[^1] signé :\n{{signature:essai-pdf}}\n[^1]: Une note.";
    let d = rendu(src, &page);
    assert_eq!(d.notes.len(), 1);
    assert!(d.signatures.contains_key("essai-pdf"));
    let t = textes_mis_en_page(std::slice::from_ref(&d), &page);
    assert!(t[0].contains("Une note."));
    let o = pdf("t", &[d], &page, &polices());
    valider(&o);
    assert!(flux(&o).contains("1 J 1 j"));
}

#[test]
fn longs_documents_paginent() {
    let page = PageSetup::default();
    let src = "Ligne de texte assez longue pour la mise en page.\n".repeat(400);
    let o = pdf("t", &[rendu(&src, &page)], &page, &polices());
    let n = valider(&o);
    assert!((6..=12).contains(&n), "pages : {n}");
}

#[test]
fn numeros_de_page_par_document() {
    let page = PageSetup {
        page_numbers: true,
        header: "En-tête".into(),
        footer: "Pied".into(),
        ..PageSetup::default()
    };
    let long = "Paragraphe.\n".repeat(150);
    let docs = vec![rendu("Court", &page), rendu(&long, &page)];
    let o = pdf("t", &docs, &page, &polices());
    let n = valider(&o);
    assert!(n >= 3);
    let t = textes_mis_en_page(&docs, &page);
    assert_eq!(t.len(), n);
    assert!(t[0].contains("Page 1 sur 1"));
    assert!(t[n - 1].contains(&format!("Page {} sur {}", n - 1, n - 1)));
    assert!(t[1].contains("Pied") && t[1].contains("En-tête"));
}

#[test]
fn saut_de_page_et_publipostage() {
    let page = PageSetup::default();
    let o = pdf(
        "t",
        &[rendu("a\n\\newpage\nb\n\\newpage\n\\newpage\nc", &page)],
        &page,
        &polices(),
    );
    // Deux sauts de suite laissent une page blanche, comme dans Writer.
    assert_eq!(valider(&o), 4);
}

#[test]
fn aucun_document_donne_une_page_vide() {
    let page = PageSetup::default();
    assert_eq!(valider(&pdf("t", &[], &page, &polices())), 1);
}

#[test]
fn mot_plus_large_que_la_page() {
    let page = PageSetup::default();
    let src = "x".repeat(5000);
    assert!(valider(&pdf("t", &[rendu(&src, &page)], &page, &polices())) >= 1);
}

#[test]
fn tableau_tres_haut_coupe_entre_pages() {
    let page = PageSetup::default();
    let cellule = "mot ".repeat(1500);
    let src = format!("| a | b |\n|---|---|\n| {cellule} | court |\n");
    let o = pdf("t", &[rendu(&src, &page)], &page, &polices());
    assert!(valider(&o) >= 2);
}

#[test]
fn mises_en_page_absurdes() {
    for (marge, taille, inter) in [
        (0.0, 4.0, 0.8),
        (500.0, 12.0, 1.15),
        (-5.0, 1000.0, 99.0),
        (f32::NAN, f32::NAN, f32::INFINITY),
    ] {
        let page = PageSetup {
            margin_top: marge,
            margin_bottom: marge,
            margin_left: marge,
            margin_right: marge,
            font_size: taille,
            line_spacing: inter,
            page_numbers: true,
            justify: true,
            ..PageSetup::default()
        };
        let src = "# T\nUn paragraphe justifié un peu long pour passer à la ligne plusieurs fois de suite.\n- a\n| a |\n|---|\n| b |\n";
        let o = pdf("t", &[rendu(src, &page)], &page, &polices());
        assert!(valider(&o) >= 1);
    }
}

#[test]
fn entrees_aleatoires_sans_panique() {
    let page = PageSetup::default();
    let f = PdfFonts::approximate();
    let morceaux = [
        "{{",
        "}}",
        "**",
        "*",
        "__",
        "~~",
        "# ",
        "- ",
        "1. ",
        "> ",
        "|",
        "|---|",
        "\\newpage",
        "\n",
        "é",
        "𝄞",
        "\u{202F}",
        "\t",
        " ",
        "mot",
        "-> ",
        " <-",
        "$x^2$",
        "$$\\frac{a}{b}$$",
    ];
    let mut e = 0x1234_5678_9abc_def0u64;
    for _ in 0..300 {
        let mut src = String::new();
        for _ in 0..60 {
            e ^= e >> 12;
            e ^= e << 25;
            e ^= e >> 27;
            let r = e.wrapping_mul(0x2545_F491_4F6C_DD1D);
            src.push_str(morceaux[(r % morceaux.len() as u64) as usize]);
        }
        let o = pdf("t", &[rendu(&src, &page)], &page, &f);
        assert!(valider(&o) >= 1, "source : {src:?}");
    }
}

#[test]
fn signets_des_titres() {
    let page = PageSetup::default();
    let src = "# Un\n\ntexte\n\n## Deux\n\n### Trois\n\n## Quatre\n\n# Cinq\n";
    let o = pdf("t", &[rendu(src, &page)], &page, &polices());
    valider(&o);
    let s = texte(&o);
    assert!(s.contains("/Outlines "));
    assert!(s.contains("/PageMode /UseOutlines"));
    assert_eq!(s.matches("/Dest [").count(), 5);
    // Racine : 5 signets au total ; « Un » a deux enfants, « Deux » un seul.
    assert!(s.contains("/Type /Outlines"));
    assert!(s.contains("/Count 5"));
    assert!(s.contains("/Count 2"));
    assert!(s.contains("/Count 1"));
}

#[test]
fn signets_sur_plusieurs_pages_et_sans_titre() {
    let page = PageSetup::default();
    let mut src = String::new();
    for i in 0..60 {
        src.push_str(&format!("# Titre {i}\n\nparagraphe\n\n\\saut\n\n"));
    }
    let o = pdf("t", &[rendu(&src, &page)], &page, &polices());
    let pages = valider(&o);
    assert!(pages >= 2);
    assert!(texte(&o).matches("/Dest [").count() >= 2);
    // Sans titre : aucun plan.
    let o = pdf("t", &[rendu("juste du texte", &page)], &page, &polices());
    valider(&o);
    assert!(!texte(&o).contains("/Outlines"));
}

#[test]
fn liens_cliquables() {
    let page = PageSetup::default();
    let src = "# Titre\n\nVoir https://exemple.fr/a(b), puis mailto:moi@exemple.fr. Et http:// seul, ou www.non.fr.\n";
    let o = pdf("t", &[rendu(src, &page)], &page, &polices());
    valider(&o);
    let s = texte(&o);
    assert_eq!(s.matches("/Subtype /Link").count(), 2);
    let hex = |t: &str| t.bytes().map(|b| format!("{b:02X}")).collect::<String>();
    assert!(
        s.contains(&format!("/URI <{}>", hex("https://exemple.fr/a(b)")))
            || s.contains(&format!("/URI <{}>", hex("https://exemple.fr/a(b")))
    );
    assert!(s.contains(&format!("/URI <{}>", hex("mailto:moi@exemple.fr"))));
    assert!(s.contains("/Annots ["));
    // Sans adresse : aucune annotation.
    let o = pdf("t", &[rendu("rien ici", &page)], &page, &polices());
    valider(&o);
    assert!(!texte(&o).contains("/Annots"));
}

#[test]
fn liens_avec_signets_et_plusieurs_pages() {
    let page = PageSetup::default();
    let mut src = String::new();
    for i in 0..40 {
        src.push_str(&format!("# T{i}\n\nhttps://exemple.fr/{i}\n\n\\saut\n\n"));
    }
    let o = pdf("t", &[rendu(&src, &page)], &page, &polices());
    valider(&o);
    assert_eq!(texte(&o).matches("/Subtype /Link").count(), 40);
}

/// Échantillon complet de tous les exports, écrit dans le dossier donné par
/// `VARIUM_ECHANTILLONS` (vérification à la main : pdftotext, pdffonts, LibreOffice).
#[test]
fn echantillons_des_exports() {
    let Ok(dossier) = std::env::var("VARIUM_ECHANTILLONS") else {
        return;
    };
    let page = PageSetup {
        page_numbers: true,
        header: "Varium — essai".into(),
        footer: "{{client}}".into(),
        auteur: "Zoé Martin".into(),
        ..PageSetup::default()
    };
    varium_core::signature::definir(
        "",
        varium_core::signature::Signature::depuis_traits(&[
            vec![
                (0.0, 30.0),
                (10.0, 0.0),
                (20.0, 35.0),
                (35.0, 5.0),
                (50.0, 30.0),
                (90.0, 10.0),
            ],
            vec![(20.0, 40.0), (85.0, 38.0)],
        ])
        .unwrap(),
    );
    let src = "\\sommaire\n# Introduction\n{: alinéa=1cm interligne=1,5 après=6pt} Un paragraphe avec un alinéa, un interligne d'une ligne et demie et une note de bas de page[^a]. Le texte continue assez longtemps pour passer à la ligne plusieurs fois et montrer l'interligne.\nDu texte {couleur:rouge}en rouge{/}, ==surligné== et {surlignage:vert}sur fond vert{/}, une note en ligne^[Note écrite dans le texte.] et des caractères variés : € œ Ω α → ∞ 😀 漢字.\n{: retrait=2cm droite=2cm couleur=bleu} Un paragraphe en retrait des deux côtés, en bleu, justifié ou non selon la mise en page du document.\n## Formules\nEn ligne : $E = mc^2$ et $\\frac{a}{b}$, puis centrée :\n$$\\int_0^1 x^2\\,dx = \\frac{1}{3} \\quad \\sum_{i=1}^{n} i = \\frac{n(n+1)}{2}$$\n$$\\sqrt[3]{x} + \\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}$$\n## Signature\nFait à Paris, le 4 novembre 2026.\n{{signature}}\n\\newpage\n# Annexe\n| Article | Prix |\n|---|---:|\n| Stylo | 2 € |\n[^a]: Le contenu de la note, avec **du gras**.\n";
    let mut v = Values::default();
    v.set("client", "Dupont");
    let d = vec![prepare(src, &v, &page)];
    let w = |n: &str, b: &[u8]| std::fs::write(format!("{dossier}/{n}"), b).unwrap();
    w("essai.pdf", &pdf("Essai Varium", &d, &page, &polices()));
    w(
        "essai.odt",
        &varium_core::odt::odt("Essai Varium", &d, &page),
    );
    w(
        "essai.docx",
        &varium_core::docx::docx("Essai Varium", &d, &page),
    );
    w(
        "essai.epub",
        &varium_core::epub::epub("Essai Varium", &d, &page),
    );
    w("essai.rtf", varium_core::rtf::rtf(&d, &page).as_bytes());
    w(
        "essai.html",
        varium_core::export::html("Essai Varium", &d, &page).as_bytes(),
    );
    w(
        "essai.tex",
        varium_core::export::latex("Essai Varium", &d, &page).as_bytes(),
    );
}
