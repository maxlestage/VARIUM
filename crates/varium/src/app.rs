//! Interface egui de Varium : source, aperçu et panneau des variables.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Color32, FontFamily, FontId, Galley, Key, KeyboardShortcut, Modifiers, RichText, Stroke,
};
use varium_core::{
    Block, Piece, Project, Values, VarType, blocks, export, filters, render_pieces, tex,
};

use crate::io::{Event, Io};
use crate::sample;

const STORAGE_KEY: &str = "varium.project";
const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const INSERT_VAR: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::V);

struct Rename {
    from: String,
    to: String,
}

pub struct VariumApp {
    title: String,
    source: String,
    values: Values,
    /// Dernière position du curseur dans la source (en caractères).
    cursor: Option<usize>,
    highlight: bool,
    rename: Option<Rename>,
    toast: Option<(String, f64, bool)>,
    io: Io,
}

impl VariumApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let project = cc
            .storage
            .and_then(|s| eframe::get_value::<Project>(s, STORAGE_KEY))
            .unwrap_or_else(|| Project::new(sample::TITLE, sample::SOURCE, &sample::values()));
        let mut app = Self {
            title: String::new(),
            source: String::new(),
            values: Values::default(),
            cursor: None,
            highlight: true,
            rename: None,
            toast: None,
            io: Io::default(),
        };
        app.load(project);
        app
    }

    fn load(&mut self, p: Project) {
        self.values = p.values();
        self.title = p.title;
        self.source = p.source;
        self.cursor = None;
        self.rename = None;
    }

    fn project(&self) -> Project {
        Project::new(self.title.clone(), self.source.clone(), &self.values)
    }

    fn notify(&mut self, ui: &egui::Ui, msg: impl Into<String>, error: bool) {
        let now = ui.input(|i| i.time);
        self.toast = Some((msg.into(), now, error));
    }

    fn file_stem(&self) -> String {
        let s: String = self
            .title
            .trim()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let s = s.trim_matches('-').to_string();
        if s.is_empty() { "document".into() } else { s }
    }

    // ---------------- Actions ----------------

    fn save_project(&self, ui: &egui::Ui) {
        let name = format!("{}.varium.json", self.file_stem());
        self.io
            .save(ui.ctx(), name, "application/json", self.project().to_json());
    }

    fn export(&self, ui: &egui::Ui, kind: &str) {
        let stem = self.file_stem();
        let (name, mime, content) = match kind {
            "html" => (
                format!("{stem}.html"),
                "text/html",
                export::to_html(&self.title, &self.source, &self.values),
            ),
            "tex" => (
                format!("{stem}.tex"),
                "application/x-tex",
                export::to_latex(&self.title, &self.source, &self.values),
            ),
            "txt" => (
                format!("{stem}.txt"),
                "text/plain",
                export::to_text(&self.source, &self.values),
            ),
            _ => (
                format!("{stem}.modele.txt"),
                "text/plain",
                self.source.clone(),
            ),
        };
        self.io.save(ui.ctx(), name, mime, content);
    }

    fn insert_at_cursor(&mut self, text: &str) {
        let chars = self.source.chars().count();
        let at = self.cursor.unwrap_or(chars).min(chars);
        let byte = self
            .source
            .char_indices()
            .nth(at)
            .map_or(self.source.len(), |(i, _)| i);
        self.source.insert_str(byte, text);
        self.cursor = Some(at + text.chars().count());
    }

    fn handle_io(&mut self, ui: &egui::Ui) {
        for ev in self.io.drain() {
            match ev {
                Event::Opened { name, content } => {
                    let is_project = name.ends_with(".json") || name.ends_with(".varium");
                    if is_project {
                        match Project::from_json(&content) {
                            Ok(p) => {
                                self.load(p);
                                self.notify(ui, format!("Projet « {name} » ouvert"), false);
                            }
                            Err(e) => self.notify(ui, e, true),
                        }
                    } else {
                        let title = name
                            .rsplit_once('.')
                            .map_or(name.as_str(), |(s, _)| s)
                            .to_string();
                        self.load(Project::new(title, content, &self.values));
                        self.notify(
                            ui,
                            format!("Modèle « {name} » ouvert — valeurs conservées"),
                            false,
                        );
                    }
                }
                Event::Saved(path) => self.notify(ui, format!("Enregistré : {path}"), false),
                Event::Error(e) => self.notify(ui, e, true),
            }
        }
    }

    // ---------------- Interface ----------------

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.label(RichText::new("{ }").monospace().strong().color(accent(ui)));
            ui.label(RichText::new("Varium").strong().size(16.0));
            ui.separator();

            ui.menu_button("Fichier", |ui| {
                if ui.button("Nouveau document").clicked() {
                    self.load(Project::new("Sans titre", "", &Values::default()));
                }
                if ui.button("Document d'exemple").clicked() {
                    self.load(Project::new(
                        sample::TITLE,
                        sample::SOURCE,
                        &sample::values(),
                    ));
                }
                ui.separator();
                if ui
                    .add(
                        egui::Button::new("Ouvrir…").shortcut_text(ui.ctx().format_shortcut(&OPEN)),
                    )
                    .clicked()
                {
                    self.io.open(ui.ctx());
                }
                if ui
                    .add(
                        egui::Button::new("Enregistrer le projet…")
                            .shortcut_text(ui.ctx().format_shortcut(&SAVE)),
                    )
                    .clicked()
                {
                    self.save_project(ui);
                }
            });
            ui.menu_button("Exporter", |ui| {
                for (kind, label) in [
                    ("html", "HTML (formules KaTeX, imprimable en PDF)"),
                    ("tex", "LaTeX (.tex)"),
                    ("txt", "Texte (.txt)"),
                    ("model", "Modèle brut avec {{variables}}"),
                ] {
                    if ui.button(label).clicked() {
                        self.export(ui, kind);
                    }
                }
            });
            ui.menu_button("Insérer", |ui| {
                if ui
                    .add(
                        egui::Button::new("Variable {{…}}")
                            .shortcut_text(ui.ctx().format_shortcut(&INSERT_VAR)),
                    )
                    .clicked()
                {
                    self.insert_at_cursor("{{variable}}");
                }
                if ui.button("Formule en ligne $…$").clicked() {
                    self.insert_at_cursor("$x^2$");
                }
                if ui.button("Formule centrée $$…$$").clicked() {
                    self.insert_at_cursor("\n$$ \\sum_{i=1}^{n} x_i $$\n");
                }
            });
            ui.separator();
            ui.add(
                egui::TextEdit::singleline(&mut self.title)
                    .hint_text("Titre du document")
                    .desired_width(220.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
                ui.checkbox(&mut self.highlight, "Surligner les champs");
            });
        });
    }

    fn source_pane(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Source");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("∑ Formule").clicked() {
                    self.insert_at_cursor("$E = mc^2$");
                }
                if ui.small_button("+ Variable").clicked() {
                    self.insert_at_cursor("{{variable}}");
                }
            });
        });
        ui.label(
            RichText::new(
                "{{nom}} variable · {{montant|money}} filtre · $x^2$ formule · \\{{x}} littéral",
            )
            .small()
            .weak(),
        );
        ui.add_space(4.0);

        let dark = ui.visuals().dark_mode;
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap: f32| -> Arc<Galley> {
            let mut job = highlight_source(buf.as_str(), dark);
            job.wrap.max_width = wrap;
            ui.ctx().fonts_mut(|f| f.layout_job(job))
        };
        egui::ScrollArea::vertical()
            .id_salt("source-scroll")
            .show(ui, |ui| {
                let out = egui::TextEdit::multiline(&mut self.source)
                    .id_salt("source")
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(30)
                    .lock_focus(true)
                    .layouter(&mut layouter)
                    .show(ui);
                if let Some(range) = out.cursor_range {
                    self.cursor = Some(range.primary.index.0);
                }
            });
    }

    fn preview_pane(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Aperçu");
            let mut missing: Vec<String> = render_pieces(&self.source, &self.values)
                .into_iter()
                .filter_map(|p| match p {
                    Piece::Missing { name, .. } => Some(name),
                    _ => None,
                })
                .collect();
            missing.sort();
            missing.dedup();
            let missing = missing.len();
            if missing > 0 {
                ui.label(
                    RichText::new(format!("{missing} champ(s) à renseigner"))
                        .color(warn(ui))
                        .small(),
                );
            }
        });
        ui.add_space(4.0);

        let dark = ui.visuals().dark_mode;
        let (paper, ink) = if dark {
            (
                Color32::from_rgb(0x24, 0x23, 0x21),
                Color32::from_rgb(0xEC, 0xE8, 0xE1),
            )
        } else {
            (
                Color32::from_rgb(0xFF, 0xFE, 0xFB),
                Color32::from_rgb(0x1D, 0x1B, 0x18),
            )
        };
        let style = PreviewStyle {
            ink,
            dark,
            highlight: self.highlight,
        };
        let doc = blocks(render_pieces(&self.source, &self.values));

        egui::ScrollArea::vertical()
            .id_salt("preview-scroll")
            .show(ui, |ui| {
                let avail = ui.available_width();
                let margin = if avail < 520.0 { 20.0 } else { 48.0 };
                let outer = (avail - 24.0).clamp(200.0, 780.0);
                ui.horizontal(|ui| {
                    ui.add_space(((avail - outer) / 2.0).max(0.0));
                    egui::Frame::new()
                        .fill(paper)
                        .stroke(Stroke::new(
                            1.0,
                            ui.visuals().widgets.noninteractive.bg_stroke.color,
                        ))
                        .corner_radius(4.0)
                        .inner_margin(egui::Margin::symmetric(margin as i8, 40))
                        .shadow(egui::Shadow {
                            offset: [0, 2],
                            blur: 12,
                            spread: 0,
                            color: Color32::from_black_alpha(30),
                        })
                        .show(ui, |ui| {
                            let inner = outer - 2.0 * margin;
                            ui.set_width(inner);
                            ui.vertical(|ui| render_preview(ui, &doc, &style));
                        });
                });
                ui.add_space(24.0);
            });
    }

    fn variables_pane(&mut self, ui: &mut egui::Ui) {
        let vars = varium_core::variables(&self.source);
        let filled = vars
            .iter()
            .filter(|v| !self.values.get(&v.name).is_empty())
            .count();

        ui.horizontal(|ui| {
            ui.heading("Variables");
            ui.label(
                RichText::new(vars.len().to_string())
                    .strong()
                    .color(accent(ui)),
            );
        });
        if !vars.is_empty() {
            ui.add(
                egui::ProgressBar::new(filled as f32 / vars.len() as f32)
                    .text(format!("{filled} / {} renseignées", vars.len()))
                    .desired_height(14.0),
            );
        }
        ui.add_space(6.0);

        let mut action: Option<VarAction> = None;
        egui::ScrollArea::vertical()
            .id_salt("vars-scroll")
            .show(ui, |ui| {
                if vars.is_empty() {
                    ui.label(
                        RichText::new(
                            "Aucune variable. Écrivez {{prénom}} dans la source pour en créer une.",
                        )
                        .weak(),
                    );
                }
                for info in &vars {
                    egui::Frame::group(ui.style())
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            if let Some(a) = self.variable_card(ui, &info.name, info.count) {
                                action = Some(a);
                            }
                        });
                    ui.add_space(4.0);
                }

                ui.add_space(8.0);
                egui::CollapsingHeader::new("Filtres disponibles").show(ui, |ui| {
                    egui::Grid::new("filters")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            for (f, desc) in filters::KNOWN {
                                ui.label(RichText::new(*f).monospace());
                                ui.label(*desc);
                                ui.end_row();
                            }
                        });
                    ui.label(
                        RichText::new("Exemple : {{montant | money:usd}}, {{nom | trim | upper}}")
                            .small()
                            .weak(),
                    );
                });
            });

        match action {
            Some(VarAction::Rename { from, to }) => {
                match varium_core::rename_variable(&self.source, &from, &to) {
                    Ok(s) => {
                        self.source = s;
                        if let Some(v) = self.values.values.remove(&from) {
                            self.values.values.insert(to.clone(), v);
                        }
                        if let Some(t) = self.values.types.remove(&from) {
                            self.values.types.insert(to.clone(), t);
                        }
                        self.rename = None;
                        self.notify(
                            ui,
                            format!("« {from} » renommée en « {to} » partout"),
                            false,
                        );
                    }
                    Err(e) => self.notify(ui, e, true),
                }
            }
            Some(VarAction::Inline(name)) => {
                self.source = varium_core::inline_variable(&self.source, &name, &self.values);
                self.notify(ui, format!("« {name} » figée dans le texte"), false);
            }
            None => {}
        }
    }

    fn variable_card(&mut self, ui: &mut egui::Ui, name: &str, count: usize) -> Option<VarAction> {
        let mut action = None;
        let mut ty = self.values.ty(name);

        ui.horizontal(|ui| {
            ui.label(RichText::new(name).monospace().strong().color(accent(ui)));
            ui.label(RichText::new(format!("×{count}")).small().weak())
                .on_hover_text(format!("{count} occurrence(s) dans le document"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let has_value = !self.values.get(name).is_empty();
                if ui
                    .add_enabled(has_value, egui::Button::new("⤓").small())
                    .on_hover_text("Figer : remplacer la variable par sa valeur dans la source")
                    .clicked()
                {
                    action = Some(VarAction::Inline(name.to_string()));
                }
                if ui
                    .small_button("✏")
                    .on_hover_text("Renommer partout")
                    .clicked()
                {
                    self.rename = Some(Rename {
                        from: name.to_string(),
                        to: name.to_string(),
                    });
                }
                egui::ComboBox::from_id_salt(("type", name))
                    .selected_text(ty.label())
                    .width(96.0)
                    .show_ui(ui, |ui| {
                        for t in VarType::ALL {
                            ui.selectable_value(&mut ty, t, t.label());
                        }
                    });
            });
        });
        if ty != self.values.ty(name) {
            self.values.types.insert(name.to_string(), ty);
        }

        if let Some(r) = self.rename.as_mut().filter(|r| r.from == name) {
            let mut commit = false;
            let mut cancel = false;
            ui.horizontal(|ui| {
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut r.to)
                        .desired_width(140.0)
                        .font(egui::TextStyle::Monospace),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    commit = true;
                }
                commit |= ui.button("Renommer").clicked();
                cancel = ui.button("Annuler").clicked();
            });
            let to = r.to.trim().to_string();
            if !varium_core::is_valid_name(&to) {
                ui.label(
                    RichText::new("Nom invalide (lettres, chiffres, _ . -)")
                        .small()
                        .color(err(ui)),
                );
                commit = false;
            } else if to != name
                && varium_core::variables(&self.source)
                    .iter()
                    .any(|v| v.name == to)
            {
                ui.label(
                    RichText::new(format!("« {to} » existe déjà : les deux seront fusionnées"))
                        .small()
                        .color(warn(ui)),
                );
            }
            if cancel {
                self.rename = None;
            } else if commit && to != name {
                action = Some(VarAction::Rename {
                    from: name.to_string(),
                    to,
                });
            } else if commit {
                self.rename = None;
            }
        }

        let value = self.values.values.entry(name.to_string()).or_default();
        match ty {
            VarType::Date => {
                ui.horizontal(|ui| match value.parse::<jiff::civil::Date>() {
                    Ok(mut date) => {
                        let before = date;
                        ui.add(
                            egui_extras::DatePickerButton::new(&mut date)
                                .id_salt(name)
                                .format("%d/%m/%Y")
                                .calendar_week(false),
                        );
                        if date != before {
                            *value = date.to_string();
                        }
                        if ui.small_button("✕").on_hover_text("Effacer").clicked() {
                            value.clear();
                        }
                    }
                    Err(_) => {
                        ui.add(
                            egui::TextEdit::singleline(value)
                                .hint_text("AAAA-MM-JJ")
                                .desired_width(110.0),
                        );
                        if ui.button("📅 Aujourd'hui").clicked() {
                            *value = jiff::Zoned::now().date().to_string();
                        }
                    }
                });
            }
            VarType::Long => {
                ui.add(
                    egui::TextEdit::multiline(value)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
            }
            _ => {
                let hint = match ty {
                    VarType::Number => "ex. 1234,5",
                    VarType::Money => "ex. 12500",
                    VarType::Email => "nom@exemple.fr",
                    _ => "Valeur…",
                };
                ui.add(
                    egui::TextEdit::singleline(value)
                        .hint_text(hint)
                        .desired_width(f32::INFINITY),
                );
            }
        }

        if let Some(msg) = ty.validate(value) {
            ui.label(RichText::new(msg).small().color(err(ui)));
        } else if matches!(ty, VarType::Date | VarType::Number | VarType::Money)
            && !value.is_empty()
        {
            let shown = self.values.resolve(&varium_core::VarRef {
                name: name.to_string(),
                filters: vec![],
                raw: String::new(),
                range: 0..0,
            });
            if let Some(s) = shown {
                ui.label(RichText::new(format!("→ {s}")).small().weak());
            }
        }
        action
    }

    fn toast(&mut self, ui: &mut egui::Ui) {
        let Some((msg, at, error)) = self.toast.clone() else {
            return;
        };
        let now = ui.input(|i| i.time);
        if now - at > 3.5 {
            self.toast = None;
            return;
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(250));
        egui::Area::new(egui::Id::unique("varium-toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -24.0])
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let color = if error {
                        err(ui)
                    } else {
                        ui.visuals().text_color()
                    };
                    ui.label(RichText::new(msg).color(color));
                });
            });
    }
}

enum VarAction {
    Rename { from: String, to: String },
    Inline(String),
}

impl eframe::App for VariumApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_io(ui);

        if ui.input_mut(|i| i.consume_shortcut(&SAVE)) {
            self.save_project(ui);
        }
        if ui.input_mut(|i| i.consume_shortcut(&OPEN)) {
            self.io.open(ui.ctx());
        }
        if ui.input_mut(|i| i.consume_shortcut(&INSERT_VAR)) {
            self.insert_at_cursor("{{variable}}");
        }

        egui::Panel::top("menu").show(ui, |ui| {
            ui.add_space(2.0);
            self.menu_bar(ui);
            ui.add_space(2.0);
        });

        let width = ui.available_width();
        egui::Panel::right("variables")
            .resizable(true)
            .default_size((width * 0.24).clamp(260.0, 380.0))
            .show(ui, |ui| {
                ui.add_space(6.0);
                self.variables_pane(ui);
            });
        egui::Panel::left("source")
            .resizable(true)
            .default_size((width * 0.36).max(280.0))
            .show(ui, |ui| {
                ui.add_space(6.0);
                self.source_pane(ui);
            });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(2.0);
            self.preview_pane(ui);
        });

        self.toast(ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.project());
    }
}

// ---------------- Rendu ----------------

fn accent(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(0x8A, 0xB4, 0xF8)
    } else {
        Color32::from_rgb(0x1F, 0x5F, 0xBF)
    }
}
fn warn(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(0xF2, 0xB8, 0x4B)
    } else {
        Color32::from_rgb(0xA4, 0x5A, 0x00)
    }
}
fn err(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(0xF2, 0x8B, 0x82)
    } else {
        Color32::from_rgb(0xB3, 0x26, 0x1E)
    }
}

/// Coloration de la source : variables, formules, échappements.
fn highlight_source(src: &str, dark: bool) -> LayoutJob {
    let font = FontId::monospace(13.5);
    let (plain, var, filt, math, esc) = if dark {
        (
            Color32::from_rgb(0xDD, 0xDA, 0xD4),
            Color32::from_rgb(0x8A, 0xB4, 0xF8),
            Color32::from_rgb(0xC5, 0x8A, 0xF9),
            Color32::from_rgb(0x81, 0xC9, 0x95),
            Color32::from_gray(130),
        )
    } else {
        (
            Color32::from_rgb(0x22, 0x20, 0x1C),
            Color32::from_rgb(0x1F, 0x5F, 0xBF),
            Color32::from_rgb(0x7B, 0x3F, 0xB5),
            Color32::from_rgb(0x1E, 0x7A, 0x3C),
            Color32::from_gray(120),
        )
    };
    let var_bg = if dark {
        Color32::from_rgba_unmultiplied(0x8A, 0xB4, 0xF8, 28)
    } else {
        Color32::from_rgb(0xE6, 0xEF, 0xFC)
    };

    let mut job = LayoutJob::default();
    let fmt = |color: Color32, bg: Color32| TextFormat {
        font_id: font.clone(),
        color,
        background: bg,
        ..Default::default()
    };

    // Texte brut avec repérage des $…$ (approximatif mais suffisant pour la coloration).
    let mut in_math = false;
    let mut push_plain = |job: &mut LayoutJob, text: &str| {
        let mut rest = text;
        while !rest.is_empty() {
            match rest.find('$') {
                Some(p) => {
                    let escaped = p > 0 && rest.as_bytes()[p - 1] == b'\\';
                    let (head, tail) = rest.split_at(p);
                    job.append(
                        head,
                        0.0,
                        fmt(if in_math { math } else { plain }, Color32::TRANSPARENT),
                    );
                    let n = if tail.starts_with("$$") { 2 } else { 1 };
                    job.append(&tail[..n], 0.0, fmt(math, Color32::TRANSPARENT));
                    if !escaped {
                        in_math = !in_math;
                    }
                    rest = &tail[n..];
                }
                None => {
                    job.append(
                        rest,
                        0.0,
                        fmt(if in_math { math } else { plain }, Color32::TRANSPARENT),
                    );
                    break;
                }
            }
        }
    };

    let mut last = 0;
    for seg in varium_core::parse(src) {
        if let varium_core::Segment::Var(v) = seg {
            let before = &src[last..v.range.start];
            if let Some(stripped) = before.strip_suffix('\\') {
                push_plain(&mut job, stripped);
                job.append("\\", 0.0, fmt(esc, Color32::TRANSPARENT));
            } else {
                push_plain(&mut job, before);
            }
            match v.raw.find('|') {
                Some(p) => {
                    job.append(&v.raw[..p], 0.0, fmt(var, var_bg));
                    job.append(&v.raw[p..v.raw.len() - 2], 0.0, fmt(filt, var_bg));
                    job.append("}}", 0.0, fmt(var, var_bg));
                }
                None => job.append(&v.raw, 0.0, fmt(var, var_bg)),
            }
            last = v.range.end;
        }
    }
    push_plain(&mut job, &src[last..]);
    job
}

struct PreviewStyle {
    ink: Color32,
    dark: bool,
    highlight: bool,
}

impl PreviewStyle {
    fn text(&self) -> TextFormat {
        TextFormat {
            font_id: FontId::new(16.0, FontFamily::Proportional),
            color: self.ink,
            line_height: Some(24.0),
            ..Default::default()
        }
    }
    fn value(&self) -> TextFormat {
        let mut f = self.text();
        if self.highlight {
            f.background = if self.dark {
                Color32::from_rgba_unmultiplied(0x8A, 0xB4, 0xF8, 40)
            } else {
                Color32::from_rgb(0xE3, 0xEE, 0xFD)
            };
            f.underline = Stroke::new(
                1.0,
                if self.dark {
                    Color32::from_rgb(0x8A, 0xB4, 0xF8)
                } else {
                    Color32::from_rgb(0x1F, 0x5F, 0xBF)
                },
            );
        }
        f
    }
    fn missing(&self) -> TextFormat {
        let mut f = self.text();
        f.font_id = FontId::monospace(14.0);
        f.color = if self.dark {
            Color32::from_rgb(0xF2, 0x8B, 0x82)
        } else {
            Color32::from_rgb(0xB3, 0x26, 0x1E)
        };
        f.background = if self.dark {
            Color32::from_rgba_unmultiplied(0xF2, 0x8B, 0x82, 30)
        } else {
            Color32::from_rgb(0xFD, 0xE7, 0xE5)
        };
        f
    }
    fn math(&self, size: f32) -> TextFormat {
        let mut f = self.text();
        f.font_id = FontId::new(size, FontFamily::Proportional);
        f.italics = true;
        f.color = if self.dark {
            Color32::from_rgb(0xC5, 0xB4, 0xF9)
        } else {
            Color32::from_rgb(0x4A, 0x2D, 0x8F)
        };
        f
    }
}

fn render_preview(ui: &mut egui::Ui, doc: &[Block], st: &PreviewStyle) {
    let width = ui.available_width();
    let mut job = LayoutJob::default();

    let flush = |ui: &mut egui::Ui, job: &mut LayoutJob| {
        if job.is_empty() {
            return;
        }
        let mut j = std::mem::take(job);
        j.wrap.max_width = width;
        ui.label(j);
    };

    let is_display = |i: Option<&Block>| matches!(i, Some(Block::Math { display: true, .. }));
    for (bi, block) in doc.iter().enumerate() {
        match block {
            Block::Prose(pieces) => {
                let (after_display, before_display) = (
                    bi > 0 && is_display(doc.get(bi - 1)),
                    is_display(doc.get(bi + 1)),
                );
                for (pi, p) in pieces.iter().enumerate() {
                    match p {
                        Piece::Text(t) => {
                            // les sauts de ligne autour d'une formule centrée sont déjà matérialisés
                            let mut t = t.as_str();
                            if after_display && pi == 0 {
                                t = t.strip_prefix('\n').unwrap_or(t);
                            }
                            if before_display && pi + 1 == pieces.len() {
                                t = t.strip_suffix('\n').unwrap_or(t);
                            }
                            job.append(t, 0.0, st.text());
                        }
                        Piece::Value { text, .. } => job.append(text, 0.0, st.value()),
                        Piece::Missing { raw, .. } => job.append(raw, 0.0, st.missing()),
                    }
                }
            }
            Block::Math {
                display: false,
                pieces,
            } => {
                let tex: String = pieces.iter().map(Piece::text).collect();
                job.append(&tex::to_unicode(&tex), 0.0, st.math(16.0));
            }
            Block::Math {
                display: true,
                pieces,
            } => {
                flush(ui, &mut job);
                let tex: String = pieces.iter().map(Piece::text).collect();
                let mut m = LayoutJob::single_section(tex::to_unicode(tex.trim()), st.math(19.0));
                m.halign = egui::Align::Center;
                m.wrap.max_width = width;
                ui.add_space(4.0);
                ui.vertical_centered(|ui| ui.label(m));
                ui.add_space(4.0);
            }
        }
    }
    flush(ui, &mut job);
}
