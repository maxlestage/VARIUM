//! Varium — « Un document. Toutes ses possibilités. »
//!
//! `cargo run -p varium --release`

// Sous Windows, pas de fenêtre de console derrière l'application.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod calc_ui;
mod dico;
mod editor;
mod files;
mod fonts;
mod io;
mod pageview;
mod secours;
mod slides_ui;
mod suite;
mod templates;

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Varium")
            .with_app_id("varium")
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([820.0, 520.0])
            .with_icon(std::sync::Arc::new(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png"))
                    .unwrap_or_default(),
            )),
        ..Default::default()
    };
    eframe::run_native(
        "Varium",
        options,
        Box::new(|cc| Ok(Box::new(suite::Suite::new(cc)))),
    )
}
