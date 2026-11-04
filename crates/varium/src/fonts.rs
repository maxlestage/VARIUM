//! Polices du document : Liberation Serif et Sans, celles de LibreOffice
//! (métriques de Times New Roman et d'Arial). Licence SIL OFL 1.1 :
//! `assets/fonts/LICENSE-OFL.txt`.

use std::sync::{Arc, OnceLock};

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
    // Polices de la mise en page, prêtes pour la vue page et les exports.
    let _ = polices(DocFont::Serif);
    let _ = polices(DocFont::Sans);
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

/// Polices de secours d'egui (symboles, émojis), dans son ordre de recherche : la vue
/// page les emploie déjà pour les caractères absents de Liberation, le PDF les
/// incorpore aussi.
fn secours() -> Vec<Arc<varium_core::police::Face>> {
    static SECOURS: OnceLock<Vec<Arc<varium_core::police::Face>>> = OnceLock::new();
    SECOURS
        .get_or_init(|| {
            let defs = FontDefinitions::default();
            let noms = defs
                .families
                .get(&FontFamily::Proportional)
                .cloned()
                .unwrap_or_default();
            noms.iter()
                .filter_map(|n| defs.font_data.get(n))
                .filter_map(|d| varium_core::police::Face::lire((*d.font).as_ref()))
                .map(Arc::new)
                .collect()
        })
        .clone()
}

/// Les polices d'une famille pour la mise en page et le PDF (lues une seule fois).
pub fn polices(font: DocFont) -> Arc<varium_core::pdf::PdfFonts> {
    static SERIF: OnceLock<Arc<varium_core::pdf::PdfFonts>> = OnceLock::new();
    static SANS: OnceLock<Arc<varium_core::pdf::PdfFonts>> = OnceLock::new();
    let cellule = match font {
        DocFont::Serif => &SERIF,
        DocFont::Sans => &SANS,
    };
    cellule
        .get_or_init(|| {
            let face = |name: &str| {
                FACES
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, b)| *b)
                    .unwrap_or(&[])
            };
            // Liberation Sans n'a pas de variante italique incorporée : l'italique est
            // alors une inclinaison de la romaine.
            let p = match font {
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
            };
            let p = p.avec_secours_lues(secours());
            // Les exports ODT et DOCX s'en servent pour les numéros de page du sommaire.
            varium_core::mise_en_page::enregistrer_polices(font, p.clone());
            Arc::new(p)
        })
        .clone()
}

/// Polices du document pour l'export PDF (incorporées, avec les polices de secours).
pub fn pdf_fonts(font: DocFont) -> varium_core::pdf::PdfFonts {
    (*polices(font)).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_avec_polices_de_secours() {
        let f = pdf_fonts(DocFont::Sans);
        assert!(!f.secours.is_empty(), "polices de secours d'egui");
        // L'émoji vient d'une police de secours, pas du rectangle `.notdef`.
        let g = f.glyphe('😀', 0);
        assert!(g.police >= varium_core::pdf::SECOURS && g.gid != 0);
        let page = varium_core::PageSetup {
            font: DocFont::Sans,
            ..Default::default()
        };
        let d = varium_core::export::prepare("Bonjour 😀 *italique*", &Default::default(), &page);
        let o = varium_core::pdf::pdf("t", &[d], &page, &f);
        let s = String::from_utf8_lossy(&o);
        assert!(s.matches("/FontFile2").count() >= 2);
    }
}
