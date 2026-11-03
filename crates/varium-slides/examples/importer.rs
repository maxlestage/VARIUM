//! Importe une présentation ODP ou PPTX et l'écrit en `.varium-slides.json` :
//! `cargo run -p varium-slides --example importer -- fichier.pptx [sortie.json]`

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let entree = args.next().ok_or("Indiquez un fichier .odp ou .pptx")?;
    let deck = varium_slides::Deck::open_bytes(&std::fs::read(&entree)?)?;
    match args.next() {
        Some(sortie) => std::fs::write(sortie, deck.to_json())?,
        None => println!("{}", deck.to_json()),
    }
    Ok(())
}
