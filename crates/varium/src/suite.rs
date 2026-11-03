//! La suite Varium : trois applications dans une fenêtre, comme LibreOffice —
//! Documents (traitement de texte), Classeur (tableur) et Présentation (diaporamas).

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
            Module::Documents => "Documents",
            Module::Classeur => "Classeur",
            Module::Presentation => "Présentation",
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
}

impl Suite {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let module = cc
            .storage
            .and_then(|s| eframe::get_value::<Module>(s, MODULE_KEY))
            .unwrap_or_default();
        Self {
            doc: VariumApp::new(cc),
            calc: CalcApp::new(cc.storage),
            slides: SlidesApp::new(cc.storage),
            module,
            last_title: String::new(),
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
                if ui.add(b).clicked() && !on {
                    self.module = m;
                    self.doc.refresh_window_title();
                    self.last_title.clear();
                }
            }
        });
    }
}

impl eframe::App for Suite {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
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
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(5)
    }
}
