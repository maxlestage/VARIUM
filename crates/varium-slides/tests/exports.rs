//! Exports HTML, ODP et PDF : contenu, échappement, structure des fichiers.

mod commun;

use commun::{compte, xml_bien_forme, zip_entries, zip_text};
use varium_slides::{Aspect, Deck, Layout, Slide, Theme, Transition};

/// Une présentation qui contient de quoi piéger les exports.
fn piegee() -> Deck {
    let mut deck = Deck::sample();
    deck.slides.push(Slide {
        layout: Layout::TitleContent,
        title: "<script>alert('titre')</script> & « Été »".into(),
        body: "- <b>gras ?</b> \"guillemets\" & 'apostrophes'\n- {{absent}}\n</section></div>".into(),
        notes: "<script>alert(1)</script> note secrète".into(),
        transition: Transition::Zoom,
        ..Slide::default()
    });
    deck.footer = "{{entreprise}} </footer><script>".into();
    deck
}

// ---------------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------------

#[test]
fn html_contient_chaque_diapositive_et_ses_notes() {
    let deck = Deck::sample();
    let html = deck.to_html();
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("<html lang=\"fr\">"));
    assert!(html.contains("<title>Varium — la suite bureautique</title>"));
    assert_eq!(compte(&html, "<section class=\"slide"), deck.slides.len());
    for (i, s) in deck.slides.iter().enumerate() {
        assert!(html.contains(&format!("id=\"d{}\"", i + 1)));
        assert!(html.contains(&format!("data-t=\"{}\"", s.transition.id())));
        assert!(html.contains(&format!("class=\"slide d-{}\"", s.layout.id())));
    }
    for titre in ["Pourquoi Varium ?", "Documents et classeurs", "Démonstration en direct"] {
        assert!(html.contains(titre), "titre {titre}");
    }
    // Notes de l'orateur présentes mais cachées.
    assert!(html.contains("<aside class=\"notes\" hidden>"));
    assert!(html.contains("Laisser un temps de silence après la citation."));
    assert!(html.contains(
        "Remercier l&#39;équipe de <span class=\"champ\" data-champ=\"entreprise\">Atelier Varium</span>."
    ));
}

#[test]
fn html_remplit_les_champs_et_rend_la_syntaxe() {
    let html = Deck::sample().to_html();
    assert!(html.contains("Présentée par <span class=\"champ\" data-champ=\"orateur\">Camille Martin</span>"));
    assert!(!html.contains("{{orateur}}"));
    assert!(html.contains("<strong>toutes ses possibilités</strong>"));
    assert!(html.contains("<table>") && html.contains("<th>Logiciel</th>"));
    assert!(html.contains("<div class=\"li\" style=\"--d:1\">"), "puce imbriquée");
    assert!(html.contains("<u>souligné</u>"));
    // Les accolades échappées restent visibles, littérales.
    assert!(html.contains("{{client}}"));
    // Pied de page et numéros.
    assert!(html.contains("<footer class=\"pied\""));
    assert!(html.contains(">8</div>"));
    // Un champ sans valeur reste visible et repérable.
    let mut deck = Deck::sample();
    deck.values.remove("orateur");
    let html = deck.to_html();
    assert!(html.contains("<mark class=\"manque\""));
    assert!(html.contains("{{orateur}}"));
}

#[test]
fn html_echappe_tout_le_texte() {
    let html = piegee().to_html();
    // Un seul script : celui du diaporama.
    assert_eq!(compte(&html, "<script>"), 1);
    assert_eq!(compte(&html, "</script>"), 1);
    assert!(html.contains("&lt;script&gt;alert(&#39;titre&#39;)&lt;/script&gt; &amp; « Été »"));
    assert!(html.contains("&lt;b&gt;gras ?&lt;/b&gt; &quot;guillemets&quot;"));
    assert!(html.contains("&lt;/section&gt;&lt;/div&gt;"));
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt; note secrète"));
    assert!(!html.contains("<b>"));
    assert_eq!(compte(&html, "<section "), compte(&html, "</section>"));
}

#[test]
fn html_diaporama_complet() {
    let html = Deck::sample().to_html();
    // Clavier, notes, plein écran, impression.
    for motif in [
        "ArrowRight",
        "ArrowLeft",
        "\"Home\"",
        "\"End\"",
        "\"Escape\"",
        "case \" \"",
        "\"n\"",
        "touchend",
        "@page{size:254mm 142.875mm;margin:0}",
        "break-after:page",
        "@keyframes v-fondu",
        "@keyframes v-zoom",
        "@keyframes v-glisse",
        "prefers-reduced-motion",
    ] {
        assert!(html.contains(motif), "{motif} absent");
    }
    // Aucune ressource externe.
    assert!(!html.contains("http://") && !html.contains("https://"));
    assert!(!html.contains("<link") && !html.contains("src="));
    // Couleurs du thème Océan.
    assert!(html.contains("--bg:#0b2540"));
    let mut deck = Deck::sample();
    deck.aspect = Aspect::Standard;
    deck.theme = Theme::Elegant;
    let html = deck.to_html();
    assert!(html.contains("@page{size:254mm 190.5mm;margin:0}"));
    assert!(html.contains("--titres:var(--serif)"));
}

#[test]
fn html_sans_numero_ni_pied() {
    let mut deck = Deck::sample();
    deck.slide_numbers = false;
    deck.footer.clear();
    let html = deck.to_html();
    assert!(!html.contains("class=\"num\""));
    assert!(!html.contains("<footer"));
    // La diapositive de titre n'a jamais de pied de page.
    let mut deck = Deck::sample();
    deck.slides.truncate(1);
    assert!(!deck.to_html().contains("<footer"));
}

// ---------------------------------------------------------------------------------
// ODP
// ---------------------------------------------------------------------------------

#[test]
fn odp_structure_opendocument() {
    let deck = piegee();
    let odp = deck.to_odp();
    // `mimetype` en premier, stocké, sans champ extra : lisible à l'octet 38.
    assert_eq!(&odp[..4], b"PK\x03\x04");
    assert_eq!(&odp[30..38], b"mimetype");
    assert_eq!(&odp[38..85], b"application/vnd.oasis.opendocument.presentation");
    let noms: Vec<String> = zip_entries(&odp).into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        noms,
        [
            "mimetype",
            "content.xml",
            "styles.xml",
            "meta.xml",
            "META-INF/manifest.xml"
        ]
    );
    for nom in &noms[1..] {
        let xml = zip_text(&odp, nom);
        if let Err(e) = xml_bien_forme(&xml) {
            panic!("{nom} mal formé : {e}");
        }
    }
    let manifest = zip_text(&odp, "META-INF/manifest.xml");
    assert!(manifest.contains("application/vnd.oasis.opendocument.presentation"));
}

#[test]
fn odp_une_page_par_diapositive() {
    let deck = piegee();
    let content = zip_text(&deck.to_odp(), "content.xml");
    assert_eq!(compte(&content, "<draw:page "), deck.slides.len());
    assert_eq!(compte(&content, "<presentation:notes "), deck.slides.len());
    assert!(content.contains("presentation:class=\"title\""));
    assert!(content.contains("presentation:class=\"notes\""));
    assert!(content.contains("Pourquoi Varium ?"));
    assert!(content.contains("Camille Martin"), "champ rempli");
    assert!(content.contains("Laisser un temps de silence après la citation."));
    assert!(content.contains("<text:list "), "puces en listes");
    assert!(content.contains("draw:fill-color=\"#0b2540\""), "fond du thème");
    assert!(content.contains("smil:type=\"fade\""), "transition fondu");
    assert!(content.contains("<text:page-number"));
    assert!(content.contains("&lt;script&gt;alert(1)&lt;/script&gt; note secrète"));
    assert!(!content.contains("<script>"));
    let styles = zip_text(&deck.to_odp(), "styles.xml");
    assert!(styles.contains("fo:page-width=\"25.4cm\""));
    assert!(styles.contains("fo:page-height=\"14.287cm\"") || styles.contains("fo:page-height=\"14.288cm\""));
    let meta = zip_text(&deck.to_odp(), "meta.xml");
    assert!(meta.contains("<dc:title>Varium — la suite bureautique</dc:title>"));
}

#[test]
fn odp_fond_propre_et_format_standard() {
    let mut deck = Deck::sample();
    deck.aspect = Aspect::Standard;
    deck.slides[2].background = Some([200, 30, 30]);
    let odp = deck.to_odp();
    let content = zip_text(&odp, "content.xml");
    assert!(content.contains("draw:fill-color=\"#c81e1e\""));
    assert!(zip_text(&odp, "styles.xml").contains("fo:page-height=\"19.05cm\""));
}

// ---------------------------------------------------------------------------------
// PDF
// ---------------------------------------------------------------------------------

fn texte_pdf(pdf: &[u8]) -> String {
    String::from_utf8_lossy(pdf).into_owned()
}

#[test]
fn pdf_structure() {
    let deck = Deck::sample();
    let pdf = deck.to_pdf();
    assert!(pdf.starts_with(b"%PDF-1.4\n"));
    assert!(pdf.ends_with(b"%%EOF\n"));
    let t = texte_pdf(&pdf);
    let n = deck.slides.len();
    assert_eq!(compte(&t, "/Type /Page "), n);
    assert!(t.contains(&format!("/Count {n} ")));
    assert!(t.contains("/MediaBox [0 0 720 405]"));
    // La table de références pointe sur chaque objet (positions en octets).
    let xref: usize = t
        .rsplit("startxref\n")
        .next()
        .and_then(|s| s.lines().next())
        .and_then(|s| s.parse().ok())
        .expect("startxref");
    assert_eq!(&pdf[xref..xref + 4], b"xref");
    let table = std::str::from_utf8(&pdf[xref..]).expect("table ASCII");
    let lignes: Vec<&str> = table.lines().collect();
    let total: usize = lignes[1].split(' ').nth(1).unwrap().parse().unwrap();
    assert_eq!(lignes.len(), 2 + total + 5, "xref, 0 N, N entrées, trailer, startxref, %%EOF");
    for (k, ligne) in lignes[3..2 + total].iter().enumerate() {
        let pos: usize = ligne[..10].parse().unwrap();
        let attendu = format!("{} 0 obj", k + 1);
        assert_eq!(&pdf[pos..pos + attendu.len()], attendu.as_bytes(), "objet {}", k + 1);
    }
    for police in ["/Helvetica-Bold", "/Times-Italic", "/WinAnsiEncoding"] {
        assert!(t.contains(police));
    }
    // Format 4:3.
    let mut deck = Deck::sample();
    deck.aspect = Aspect::Standard;
    assert!(texte_pdf(&deck.to_pdf()).contains("/MediaBox [0 0 720 540]"));
}

#[test]
fn pdf_accents_codes_en_winansi() {
    let mut deck = Deck::new();
    deck.title = "Présentation d'été".into();
    deck.slides[0].title = "Été".into();
    deck.slides[0].subtitle = "Coût : 5 € — « œuvre »".into();
    let pdf = texte_pdf(&deck.to_pdf());
    assert!(pdf.contains("<C974E9>"), "« Été » en WinAnsi");
    // û = FB, € = 80, — = 97, « = AB, œ = 9C, » = BB.
    assert!(
        pdf.contains("<436FFB74203A20352080209720AB209C7576726520BB>"),
        "« Coût : 5 € — « œuvre » » en WinAnsi"
    );
    // Titre du document en UTF-16 dans les métadonnées.
    assert!(pdf.contains("/Title <FEFF0050007200E9"));
}

#[test]
fn pdf_page_par_diapositive_meme_vide() {
    let vide = Deck {
        slides: Vec::new(),
        ..Deck::new()
    };
    let t = texte_pdf(&vide.to_pdf());
    assert_eq!(compte(&t, "/Type /Page "), 1);
    let mut deck = Deck::new();
    for i in 0..30 {
        deck.add_slide(i, Layout::ALL[i % Layout::ALL.len()]);
    }
    let t = texte_pdf(&deck.to_pdf());
    assert_eq!(compte(&t, "/Type /Page "), 31);
}

#[test]
fn pdf_texte_trop_long_reduit_et_coupe() {
    let mut deck = Deck::new();
    deck.slides[0] = Slide {
        layout: Layout::TitleContent,
        title: "Titre".into(),
        body: "- ligne de contenu assez longue pour remplir\n".repeat(400),
        ..Slide::default()
    };
    let pdf = deck.to_pdf();
    let t = texte_pdf(&pdf);
    // Le texte est réduit puis coupé : la page reste raisonnable.
    assert!(pdf.len() < 200_000, "PDF de {} octets", pdf.len());
    assert!(t.contains(" Tf "));
}

#[test]
fn exports_d_une_presentation_vide() {
    let vide = Deck {
        slides: Vec::new(),
        title: String::new(),
        ..Deck::new()
    };
    assert!(vide.to_html().contains("<title>Présentation</title>"));
    assert_eq!(compte(&vide.to_html(), "<section class=\"slide"), 1);
    let content = zip_text(&vide.to_odp(), "content.xml");
    assert_eq!(compte(&content, "<draw:page "), 1);
    assert!(xml_bien_forme(&content).is_ok());
}

#[test]
fn verificateur_xml_refuse_les_erreurs() {
    assert!(xml_bien_forme("<a><b x=\"1\"/>t &amp; u</a>").is_ok());
    assert!(xml_bien_forme("<a><b></a>").is_err());
    assert!(xml_bien_forme("<a x=1/>").is_err());
    assert!(xml_bien_forme("<a>&nbsp;</a>").is_err());
    assert!(xml_bien_forme("<a>1 < 2</a>").is_err());
    assert!(xml_bien_forme("<a/><b/>").is_err());
    assert!(xml_bien_forme("<a x=\"1\" x=\"2\"/>").is_err());
}
