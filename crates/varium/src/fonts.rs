//! Polices du document : Liberation Serif et Sans, celles de LibreOffice
//! (métriques de Times New Roman et d'Arial). Licence SIL OFL 1.1 :
//! `assets/fonts/LICENSE-OFL.txt`.

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};
use varium_core::DocFont;

const FACES: [(&str, &[u8]); 6] = [
    (
        "serif",
        include_bytes!("../assets/fonts/LiberationSerif-Regular.ttf"),
    ),
    (
        "serif-b",
        include_bytes!("../assets/fonts/LiberationSerif-Bold.ttf"),
    ),
    (
        "serif-i",
        include_bytes!("../assets/fonts/LiberationSerif-Italic.ttf"),
    ),
    (
        "serif-bi",
        include_bytes!("../assets/fonts/LiberationSerif-BoldItalic.ttf"),
    ),
    (
        "sans",
        include_bytes!("../assets/fonts/LiberationSans-Regular.ttf"),
    ),
    (
        "sans-b",
        include_bytes!("../assets/fonts/LiberationSans-Bold.ttf"),
    ),
];

/// Ajoute les familles `doc-serif`, `doc-serif-b`, … aux polices d'egui. Les
/// caractères absents (symboles, émojis) viennent des polices d'egui.
pub fn install(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let fallback = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (name, bytes) in FACES {
        defs.font_data.insert(
            format!("liberation-{name}"),
            Arc::new(FontData::from_static(bytes)),
        );
        let mut chain = vec![format!("liberation-{name}")];
        chain.extend(fallback.iter().cloned());
        defs.families
            .insert(FontFamily::Name(format!("doc-{name}").into()), chain);
    }
    ctx.set_fonts(defs);
}

/// Possède-t-on une vraie police italique pour cette famille ?
pub fn has_italic(font: DocFont) -> bool {
    font == DocFont::Serif
}

/// La famille egui d'un style de caractères.
pub fn family(font: DocFont, bold: bool, italic: bool) -> FontFamily {
    let name = match (font, bold, italic) {
        (DocFont::Serif, false, false) => "doc-serif",
        (DocFont::Serif, true, false) => "doc-serif-b",
        (DocFont::Serif, false, true) => "doc-serif-i",
        (DocFont::Serif, true, true) => "doc-serif-bi",
        (DocFont::Sans, false, _) => "doc-sans",
        (DocFont::Sans, true, _) => "doc-sans-b",
    };
    FontFamily::Name(name.into())
}

/// Largeurs des polices du document pour l'export PDF. Liberation Sans n'a pas de
/// variante italique incorporée : l'oblique d'Helvetica a les mêmes largeurs.
pub fn pdf_fonts(font: DocFont) -> varium_core::pdf::PdfFonts {
    let face = |name: &str| {
        FACES
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, b)| *b)
            .unwrap_or(&[])
    };
    match font {
        DocFont::Serif => varium_core::pdf::PdfFonts::from_ttf(
            face("serif"),
            face("serif-b"),
            face("serif-i"),
            face("serif-bi"),
        ),
        DocFont::Sans => varium_core::pdf::PdfFonts::from_ttf(
            face("sans"),
            face("sans-b"),
            face("sans"),
            face("sans-b"),
        ),
    }
}
