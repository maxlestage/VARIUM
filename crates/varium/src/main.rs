//! Varium — « Un document. Toutes ses possibilités. »
//!
//! `cargo run -p varium --release`

// Sous Windows, pas de fenêtre de console derrière l'application.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[macro_use]
mod tr;

mod accueil;
mod app;
mod apparence;
mod calc_ui;
mod conversions;
mod depot;
mod dico;
mod editor;
mod files;
mod fonts;
mod io;
mod labels;
#[cfg(target_os = "macos")]
mod menubar;
mod ouverture;
mod pageview;
mod palette;
mod raccourcis;
mod secours;
mod slides_ui;
mod suite;
mod templates;

fn main() -> eframe::Result {
    env_logger::init();
    // macOS : recevoir les fichiers ouverts depuis le Finder (événement Apple).
    ouverture::avant_lancement();
    let viewport = egui::ViewportBuilder::default()
        .with_title("Varium")
        .with_app_id("varium")
        .with_inner_size([1360.0, 860.0])
        .with_min_inner_size([820.0, 520.0])
        .with_icon(std::sync::Arc::new(
            eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png"))
                .unwrap_or_default(),
        ));
    // macOS : barre de titre unifiée, comme Pages — le contenu passe sous une barre
    // transparente et le sélecteur de modules s'y dessine.
    #[cfg(target_os = "macos")]
    let viewport = viewport
        .with_fullsize_content_view(true)
        .with_titlebar_shown(false)
        .with_title_shown(false);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Varium",
        options,
        Box::new(|cc| Ok(Box::new(suite::Suite::new(cc)))),
    )
}
