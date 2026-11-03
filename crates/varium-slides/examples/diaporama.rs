//! Exporte la présentation d'exemple dans un dossier, dans tous les formats :
//! `cargo run -p varium-slides --example diaporama -- /tmp/varium-slides [thème] [4:3]`

use varium_slides::{Aspect, Deck, Theme};

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let dossier = args.next().unwrap_or_else(|| "varium-slides".into());
    let mut deck = Deck::sample();
    for a in args {
        if a == "4:3" {
            deck.aspect = Aspect::Standard;
        } else if let Some(t) = Theme::from_id(&a) {
            deck.theme = t;
        }
    }
    std::fs::create_dir_all(&dossier)?;
    let chemin = |ext: &str| format!("{dossier}/presentation.{ext}");
    std::fs::write(chemin("varium-slides.json"), deck.to_json())?;
    std::fs::write(chemin("html"), deck.to_html())?;
    std::fs::write(chemin("odp"), deck.to_odp())?;
    std::fs::write(chemin("pdf"), deck.to_pdf())?;
    println!("Exports écrits dans {dossier}");
    Ok(())
}
