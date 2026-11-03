//! Varium — « Un document. Toutes ses possibilités. »
//!
//! `cargo run -p varium --release`

// Sous Windows, pas de fenêtre de console derrière l'application.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod editor;
mod files;
mod fonts;
mod io;
mod pageview;
mod secours;
mod templates;

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Varium")
            .with_app_id("varium")
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([820.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Varium",
        options,
        Box::new(|cc| Ok(Box::new(app::VariumApp::new(cc)))),
    )
}
