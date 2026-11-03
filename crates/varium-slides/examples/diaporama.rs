//! Exporte une présentation dans un dossier, dans tous les formats :
//! `cargo run -p varium-slides --example diaporama -- /tmp/varium-slides [thème] [4:3] [fichier.json]`
//!
//! Sans fichier `.json`, c'est la présentation d'exemple qui est exportée.

use varium_slides::{Aspect, Deck, Theme};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dossier = args.next().unwrap_or_else(|| "varium-slides".into());
    let mut deck = Deck::sample();
    let (mut theme, mut aspect) = (None, None);
    for a in args {
        if a == "4:3" {
            aspect = Some(Aspect::Standard);
        } else if a.ends_with(".json") {
            deck = Deck::from_json(&std::fs::read_to_string(&a)?)?;
        } else if let Some(t) = Theme::from_id(&a) {
            theme = Some(t);
        } else {
            return Err(format!("Argument inconnu : « {a} »").into());
        }
    }
    deck.theme = theme.unwrap_or(deck.theme);
    deck.aspect = aspect.unwrap_or(deck.aspect);
    std::fs::create_dir_all(&dossier)?;
    let chemin = |ext: &str| format!("{dossier}/presentation.{ext}");
    std::fs::write(chemin("varium-slides.json"), deck.to_json())?;
    std::fs::write(chemin("html"), deck.to_html())?;
    std::fs::write(chemin("odp"), deck.to_odp())?;
    std::fs::write(chemin("pdf"), deck.to_pdf())?;
    println!("Exports écrits dans {dossier}");
    Ok(())
}
