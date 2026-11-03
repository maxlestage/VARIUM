//! La suite Varium : trois applications dans une fenêtre, comme LibreOffice —
//! Documents (traitement de texte), Classeur (tableur) et Présentation (diaporamas).

use crate::tr::{self, tr};
use egui::{Color32, RichText, Vec2};
use serde::{Deserialize, Serialize};

use crate::app::VariumApp;
use crate::calc_ui::CalcApp;
use crate::slides_ui::SlidesApp;

const MODULE_KEY: &str = "varium.module";

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Default)]
pub enum Module {
    #[default]
    Documents,
    Classeur,
    Presentation,
}

impl Module {
    const ALL: [Module; 3] = [Module::Documents, Module::Classeur, Module::Presentation];

    fn label(self) -> &'static str {
        match self {
            Module::Documents => tr("Documents"),
            Module::Classeur => tr("Classeur"),
            Module::Presentation => tr("Présentation"),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Module::Documents => "📝",
            Module::Classeur => "📊",
            Module::Presentation => "🎞",
        }
    }

    fn color(self) -> Color32 {
        match self {
            Module::Documents => Color32::from_rgb(0x1F, 0x5F, 0xBF),
            Module::Classeur => Color32::from_rgb(0x1E, 0x8A, 0x4C),
            Module::Presentation => Color32::from_rgb(0xD0, 0x5A, 0x1E),
        }
    }
}

pub struct Suite {
    doc: VariumApp,
    calc: CalcApp,
    slides: SlidesApp,
    module: Module,
    last_title: String,
    /// Langue de l'interface au dernier titre de fenêtre envoyé.
    title_lang: varium_core::Lang,
    /// Barre de menus native de macOS (reconstruite quand le module ou la langue change).
    #[cfg(target_os = "macos")]
    native: Option<crate::menubar::NativeBar>,
    #[cfg(target_os = "macos")]
    menu_events: crate::menubar::Events,
    #[cfg(target_os = "macos")]
    menu_generation: u32,
}

impl Suite {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let module = cc
            .storage
            .and_then(|s| eframe::get_value::<Module>(s, MODULE_KEY))
            .unwrap_or_default();
        // La langue de l'interface, avant de construire les modules.
        let choice = cc
            .storage
            .and_then(|s| s.get_string(tr::STORAGE_KEY))
            .and_then(|code| tr::parse_choice(&code));
        tr::set_choice(choice);
        #[allow(unused_mut)]
        let mut suite = Self {
            doc: VariumApp::new(cc),
            calc: CalcApp::new(cc.storage),
            slides: SlidesApp::new(cc.storage),
            module,
            last_title: String::new(),
            title_lang: tr::lang(),
            #[cfg(target_os = "macos")]
            native: None,
            #[cfg(target_os = "macos")]
            menu_events: crate::menubar::Events::install(&cc.egui_ctx),
            #[cfg(target_os = "macos")]
            menu_generation: 0,
        };
        #[cfg(target_os = "macos")]
        suite.update_native_menu();
        suite
    }

    /// Installe ou reconstruit la barre native selon le module et la langue.
    #[cfg(target_os = "macos")]
    fn update_native_menu(&mut self) {
        if self.native.as_ref().is_some_and(|n| !n.stale(self.module)) {
            return;
        }
        self.menu_generation += 1;
        self.native = crate::menubar::NativeBar::install(self.module, self.menu_generation);
        let on = self.native.is_some();
        self.doc.native_menu = on;
        self.calc.native_menu = on;
        self.slides.native_menu = on;
    }

    /// Les actions choisies dans la barre native depuis la dernière image.
    #[cfg(target_os = "macos")]
    fn native_actions(&mut self, ctx: &egui::Context) {
        use crate::menubar::MenuAct;
        let Some(bar) = &self.native else { return };
        for act in bar.poll(&self.menu_events) {
            match act {
                MenuAct::Doc(crate::app::Act::Quit) if self.module != Module::Documents => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                MenuAct::Doc(a) => self.doc.menu_act(ctx, a),
                MenuAct::Calc(a) => self.calc.perform_once(ctx, a),
                MenuAct::Slides(a) => self.slides.perform_once(ctx, a),
                MenuAct::Module(m) => self.switch(m),
                MenuAct::UiLang(c) => tr::set_choice(c),
                MenuAct::Website => ctx.open_url(egui::OpenUrl::new_tab(
                    "https://maxlestage.github.io/VARIUM/",
                )),
            }
        }
    }

    fn switch(&mut self, m: Module) {
        if self.module != m {
            self.module = m;
            self.doc.refresh_window_title();
            self.last_title.clear();
        }
    }

    fn switcher(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for m in Module::ALL {
                let on = self.module == m;
                let text = RichText::new(format!("{}  {}", m.icon(), m.label())).size(13.5);
                let text = if on {
                    text.strong().color(Color32::WHITE)
                } else {
                    text
                };
                let mut b = egui::Button::new(text)
                    .min_size(Vec2::new(128.0, 26.0))
                    .corner_radius(13.0);
                if on {
                    b = b.fill(m.color());
                }
                if ui.add(b).clicked() {
                    self.switch(m);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let current = tr::choice();
                let label = current.map_or(tr("Automatique"), |l| l.native());
                ui.menu_button(format!("🌐 {label}"), |ui| {
                    ui.label(RichText::new(tr("Langue de l'interface")).strong());
                    ui.separator();
                    for (c, name) in tr::choices() {
                        if ui.radio(current == c, name).clicked() {
                            tr::set_choice(c);
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text(tr("Langue de l'interface"));
            });
        });
    }
}

impl eframe::App for Suite {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        #[cfg(target_os = "macos")]
        {
            self.update_native_menu();
            self.native_actions(&ctx);
        }
        // Langue changée : les titres de fenêtre suivent.
        if self.title_lang != tr::lang() {
            self.title_lang = tr::lang();
            self.doc.refresh_window_title();
            self.last_title.clear();
        }
        // Fermer la fenêtre depuis le Classeur ou la Présentation : si le document a
        // des modifications, on revient sur lui pour la confirmation.
        if self.module != Module::Documents
            && ctx.input(|i| i.viewport().close_requested())
            && self.doc.has_unsaved_changes()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.module = Module::Documents;
            self.doc.refresh_window_title();
        }

        let presenting = self.module == Module::Presentation && self.slides.is_presenting();
        if !presenting {
            egui::Panel::top("suite-switcher")
                .frame(
                    egui::Frame::side_top_panel(ui.style())
                        .inner_margin(egui::Margin::symmetric(8, 5)),
                )
                .show(ui, |ui| self.switcher(ui));
        }
        match self.module {
            Module::Documents => self.doc.ui(ui, frame),
            Module::Classeur => {
                self.calc.ui(ui);
                let t = self.calc.title();
                if t != self.last_title {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Title(t.clone()));
                    self.last_title = t;
                }
            }
            Module::Presentation => {
                self.slides.ui(ui);
                let t = self.slides.title();
                if t != self.last_title {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Title(t.clone()));
                    self.last_title = t;
                }
            }
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.doc.save(storage);
        self.calc.save(storage);
        self.slides.save(storage);
        eframe::set_value(storage, MODULE_KEY, &self.module);
        storage.set_string(tr::STORAGE_KEY, tr::choice_code(tr::choice()).to_string());
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(5)
    }
}
