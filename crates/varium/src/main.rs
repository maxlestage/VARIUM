//! Varium — « Un document. Toutes ses possibilités. »
//!
//! Bureau : `cargo run -p varium --release`
//! Web    : `trunk serve` dans `crates/varium`

mod app;
mod editor;
mod fonts;
mod io;
mod pageview;
mod templates;

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast as _;

    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .expect("pas de document");
        let canvas = document
            .get_element_by_id("varium_canvas")
            .expect("canvas #varium_canvas introuvable")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#varium_canvas n'est pas un canvas");

        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(app::VariumApp::new(cc)))),
            )
            .await;

        if let Some(loading) = document.get_element_by_id("loading") {
            match result {
                Ok(()) => loading.remove(),
                Err(e) => {
                    loading.set_inner_html(
                        "<p>Varium n'a pas pu démarrer. Consultez la console du navigateur.</p>",
                    );
                    panic!("échec du démarrage : {e:?}");
                }
            }
        }
    });
}
