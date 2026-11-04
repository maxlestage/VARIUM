//! Tests de l'export PDF natif : structure du fichier, pagination, en-tête/pied de page,
//! robustesse sur des entrées absurdes.

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
    assert!(s.starts_with("%PDF-1.4"));
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
        &[rendu("Élève « français » — œuvre €", &page)],
        &page,
        &polices(),
    );
    let s = texte(&o);
    // « É » = 0xC9, « œ » = 0x9C, « € » = 0x80 en WinAnsi.
    assert!(s.contains("C9"));
    assert!(s.contains("9C"));
    assert!(s.contains("80"));
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
    let s = texte(&o);
    // « Page 1 sur 1 » du premier document (texte codé en hexadécimal).
    let hex = |t: &str| t.bytes().map(|b| format!("{b:02X}")).collect::<String>();
    assert!(s.contains(&hex("Page 1 sur 1")));
    assert!(s.contains(&hex(&format!("Page {} sur {}", n - 1, n - 1))));
    assert!(s.contains(&hex("Pied")));
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
    assert_eq!(valider(&o), 3);
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
    assert!(
        s.contains("/URI (https://exemple.fr/a\\(b\\))")
            || s.contains("/URI (https://exemple.fr/a\\(b)")
    );
    assert!(s.contains("/URI (mailto:moi@exemple.fr)"));
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
