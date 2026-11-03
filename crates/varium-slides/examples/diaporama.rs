//! Exporte une présentation dans un dossier, dans tous les formats :
//! `cargo run -p varium-slides --example diaporama -- /tmp/varium-slides [thème] [4:3] [objets] [fichier.json]`
//!
//! `objets` ajoute une image (dégradé PNG transparent) et toutes les formes.
//!
//! Sans fichier `.json`, c'est la présentation d'exemple qui est exportée.

use varium_slides::{Aspect, Deck, ObjectKind, Theme};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dossier = args.next().unwrap_or_else(|| "varium-slides".into());
    let mut deck = Deck::sample();
    let (mut theme, mut aspect, mut objets) = (None, None, false);
    for a in args {
        if a == "4:3" {
            aspect = Some(Aspect::Standard);
        } else if a == "objets" {
            objets = true;
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
    if objets {
        illustrer(&mut deck)?;
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

/// Ajoute une image et toutes les formes à la diapositive « Démonstration ».
fn illustrer(deck: &mut Deck) -> Result<(), Box<dyn std::error::Error>> {
    let i = deck
        .slides
        .iter()
        .position(|s| s.title.starts_with("Démonstration"))
        .unwrap_or(0);
    let img = image::RgbaImage::from_fn(320, 180, |x, y| {
        image::Rgba([(x * 255 / 320) as u8, (y * 255 / 180) as u8, 180, (40 + x * 215 / 320) as u8])
    });
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)?;
    let k = deck.insert_image(i, &png)?;
    let o = &mut deck.slides[i].objects[k];
    o.frame = varium_slides::Frame::new(0.06, 0.3, 0.3, 0.3 * deck.aspect.ratio() * 180.0 / 320.0);
    for (n, kind) in ObjectKind::SHAPES.into_iter().enumerate() {
        let k = deck.insert_shape(i, kind);
        let o = &mut deck.slides[i].objects[k];
        o.frame = varium_slides::Frame::new(0.4 + 0.11 * n as f32, 0.35, 0.1, 0.3);
        if kind.is_closed() {
            o.text = format!("**{}** {{{{ville}}}}", kind.label());
            o.stroke = Some([255, 255, 255]);
            o.text_size = 0.03;
        } else {
            o.flip_v = n == 4;
        }
    }
    Ok(())
}
