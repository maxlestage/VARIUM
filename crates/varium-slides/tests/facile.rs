//! Minutage, plan, embellissement et création depuis un texte, vus de l'extérieur.

use varium_slides::{Closing, Deck, HandoutNotes, HandoutOptions, HandoutPaper};

#[test]
fn minutage_dans_les_exports() {
    let mut deck = Deck::sample();
    let durees: Vec<f64> = (0..deck.slides.len()).map(|i| 5.0 + i as f64).collect();
    deck.set_timings(&durees);
    // Sans avance automatique, les exports n'en parlent pas.
    assert!(!deck.to_html().contains("data-duree=\""));
    let odp = String::from_utf8_lossy(&deck.to_odp()).into_owned();
    assert!(!odp.contains("transition-type=\"automatic\""));
    assert!(!Deck::from_odp(&deck.to_odp()).unwrap().auto_advance);
    deck.auto_advance = true;
    let html = deck.to_html();
    assert!(html.contains("data-duree=\"5\""));
    assert!(html.contains("setTimeout(suivante, duree * 1000)"));
    let odp = deck.to_odp();
    let relu = Deck::from_odp(&odp).unwrap();
    assert!(relu.auto_advance);
    assert_eq!(relu.slides[0].duration, Some(5.0));
    assert_eq!(relu.slides[3].duration, Some(8.0));
    assert_eq!(relu.slides[3].transition, deck.slides[3].transition);
}

#[test]
fn plan_puis_exports() {
    let deck = Deck::sample();
    let plan = deck.to_outline();
    let d = deck.apply_outline(&format!("{plan}\nConclusion\n\tUn dernier mot"));
    assert_eq!(d.slides.len(), deck.slides.len() + 1);
    assert_eq!(d.slides.last().unwrap().body, "- Un dernier mot");
    assert!(d.to_pdf().starts_with(b"%PDF"));
}

#[test]
fn depuis_un_texte_puis_embellir() {
    let d = Deck::from_text(
        "Bilan 2026\n\nVentes\n87 % des clients renouvellent.\n\nÉquipe :\n- 12 personnes\n- 3 recrutements",
        Closing::default(),
    );
    for i in 0..d.slides.len() {
        let p = d.design_suggestions(i);
        assert!((3..=6).contains(&p.len()));
    }
    let pdf = d.to_handout_pdf(HandoutOptions {
        per_page: 6,
        notes: HandoutNotes::Speaker,
        paper: HandoutPaper::Letter,
    });
    assert!(pdf.starts_with(b"%PDF"));
}
