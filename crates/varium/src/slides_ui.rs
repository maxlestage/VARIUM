//! Module Présentation : des diaporamas à la manière d'Impress et de Keynote.
//! Vignettes, édition de la diapositive, dispositions, thèmes, transitions, notes,
//! {{champs}} partagés, mode présentation plein écran, exports PDF / ODP / HTML.
//! Le modèle et les exports viennent du moteur `varium-slides`.

use std::path::PathBuf;

use egui::text::LayoutJob;
use egui::{Align2, Color32, FontId, Key, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use varium_core::{DocFont, Para, Run, Values};
use varium_slides::{Aspect, Deck, Frame, Layout, Slide, Theme, Transition};

use crate::io::{Event, Io, OpenKind};

pub const STORAGE_KEY: &str = "varium.slides";

pub struct SlidesApp {
    deck: Deck,
    current: usize,
    /// Mode présentation : diapositive affichée et heure de son apparition.
    presenting: Option<(usize, f64)>,
    show_notes: bool,
    undo: Vec<Deck>,
    redo: Vec<Deck>,
    last_change: f64,
    path: Option<PathBuf>,
    io: Io,
    toast: Option<(String, f64, bool)>,
    new_layout: Layout,
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

impl SlidesApp {
    pub fn new(storage: Option<&dyn eframe::Storage>) -> Self {
        let deck = storage
            .and_then(|s| eframe::get_value::<Deck>(s, STORAGE_KEY))
            .unwrap_or_else(Deck::sample);
        Self {
            deck,
            current: 0,
            presenting: None,
            show_notes: false,
            undo: Vec::new(),
            redo: Vec::new(),
            last_change: 0.0,
            path: None,
            io: Io::default(),
            toast: None,
            new_layout: Layout::TitleContent,
        }
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.deck);
    }

    pub fn is_presenting(&self) -> bool {
        self.presenting.is_some()
    }

    pub fn title(&self) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.deck.title.clone());
        format!("{name} — Varium Présentation")
    }

    fn snapshot(&mut self, now: f64, coalesce: bool) {
        // La frappe continue ne crée qu'une étape d'annulation.
        if coalesce && now - self.last_change < 1.0 {
            self.last_change = now;
            return;
        }
        self.last_change = now;
        self.undo.push(self.deck.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn notify(&mut self, ctx: &egui::Context, msg: impl Into<String>, error: bool) {
        self.toast = Some((msg.into(), ctx.input(|i| i.time), error));
    }

    fn stem(&self) -> String {
        let s: String = self
            .deck
            .title
            .trim()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .collect();
        let s = s.trim_matches('-').to_string();
        if s.is_empty() { "presentation".into() } else { s }
    }

    fn handle_io(&mut self, ctx: &egui::Context) {
        for ev in self.io.drain() {
            match ev {
                Event::Opened {
                    kind: OpenKind::Deck,
                    name,
                    content,
                    path,
                } => match Deck::from_json(&content) {
                    Ok(d) => {
                        self.undo.push(std::mem::replace(&mut self.deck, d));
                        self.path = path;
                        self.current = 0;
                        self.notify(ctx, format!("« {name} » ouvert"), false);
                    }
                    Err(e) => self.notify(ctx, e, true),
                },
                Event::Opened { .. } => {}
                Event::Saved { path, project } => {
                    if project {
                        self.path = Some(path.clone());
                    }
                    self.notify(ctx, format!("Enregistré : {}", path.display()), false);
                }
                Event::Info(m) => self.notify(ctx, m, false),
                Event::Error(e) => self.notify(ctx, e, true),
            }
        }
    }

    fn save_file(&mut self, ctx: &egui::Context, as_new: bool) {
        let json = self.deck.to_json().into_bytes();
        match (&self.path, as_new) {
            (Some(p), false) => self.io.write_to(ctx, p.clone(), json),
            _ => self.io.save(
                ctx,
                format!("{}.varium-slides.json", self.stem()),
                "application/json",
                json,
                true,
            ),
        }
    }

    fn export(&mut self, ctx: &egui::Context, kind: &str) {
        let stem = self.stem();
        let (name, mime, bytes) = match kind {
            "pdf" => (format!("{stem}.pdf"), "application/pdf", self.deck.to_pdf()),
            "odp" => (
                format!("{stem}.odp"),
                "application/vnd.oasis.opendocument.presentation",
                self.deck.to_odp(),
            ),
            _ => (format!("{stem}.html"), "text/html", self.deck.to_html().into_bytes()),
        };
        self.io.save(ctx, name, mime, bytes, false);
    }

    fn start(&mut self, ctx: &egui::Context, from: usize) {
        let now = ctx.input(|i| i.time);
        self.presenting = Some((from.min(self.deck.slides.len().saturating_sub(1)), now));
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
    }

    fn stop(&mut self, ctx: &egui::Context) {
        if let Some((i, _)) = self.presenting.take() {
            self.current = i;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
    }

    // ---------------- Interface ----------------

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.handle_io(&ctx);
        if self.deck.slides.is_empty() {
            self.deck.add_slide(0, Layout::Title);
        }
        self.current = self.current.min(self.deck.slides.len() - 1);

        if self.presenting.is_some() {
            self.present(ui);
            return;
        }
        // Raccourcis
        if !ctx.egui_wants_keyboard_input() {
            let (f5, shift) = ctx.input(|i| (i.key_pressed(Key::F5), i.modifiers.shift));
            if f5 {
                let from = if shift { self.current } else { 0 };
                self.start(&ctx, from);
            }
            if ctx.input(|i| i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::PageDown)) {
                self.current = (self.current + 1).min(self.deck.slides.len() - 1);
            }
            if ctx.input(|i| i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::PageUp)) {
                self.current = self.current.saturating_sub(1);
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::M)) {
            self.add(&ctx);
        }

        egui::Panel::top("slides-menu").show(ui, |ui| {
            ui.add_space(2.0);
            egui::MenuBar::new().ui(ui, |ui| self.menus(ui));
        });
        egui::Panel::top("slides-tools").show(ui, |ui| {
            ui.add_space(3.0);
            self.toolbar(ui);
            ui.add_space(3.0);
        });
        egui::Panel::left("slides-thumbs")
            .resizable(true)
            .default_size(190.0)
            .size_range(130.0..=320.0)
            .show(ui, |ui| self.thumbnails(ui));
        egui::Panel::right("slides-inspector")
            .resizable(true)
            .default_size(280.0)
            .size_range(230.0..=420.0)
            .show(ui, |ui| self.inspector(ui));
        egui::CentralPanel::default().show(ui, |ui| self.editor(ui));
        self.toast_ui(&ctx);
    }

    fn add(&mut self, ctx: &egui::Context) {
        self.snapshot(ctx.input(|i| i.time), false);
        self.current = self.deck.add_slide(self.current, self.new_layout);
    }

    fn menus(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        ui.menu_button("Fichier", |ui| {
            if ui.button("Nouvelle présentation").clicked() {
                self.snapshot(now, false);
                self.deck = Deck::new();
                self.path = None;
                self.current = 0;
            }
            if ui.button("Présentation d'exemple").clicked() {
                self.snapshot(now, false);
                self.deck = Deck::sample();
                self.path = None;
                self.current = 0;
            }
            if ui.button("Ouvrir…").clicked() {
                self.io.open(&ctx, OpenKind::Deck);
            }
            ui.separator();
            if ui.button("Enregistrer").clicked() {
                self.save_file(&ctx, false);
            }
            if ui.button("Enregistrer sous…").clicked() {
                self.save_file(&ctx, true);
            }
            ui.menu_button("Exporter", |ui| {
                if ui.button("PDF (une page par diapositive)").clicked() {
                    self.export(&ctx, "pdf");
                }
                if ui.button("LibreOffice Impress (.odp)").clicked() {
                    self.export(&ctx, "odp");
                }
                if ui.button("Diaporama web (.html)").clicked() {
                    self.export(&ctx, "html");
                }
            });
        });
        ui.menu_button("Édition", |ui| {
            if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Annuler")).clicked()
                && let Some(prev) = self.undo.pop()
            {
                self.redo.push(std::mem::replace(&mut self.deck, prev));
            }
            if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Rétablir")).clicked()
                && let Some(next) = self.redo.pop()
            {
                self.undo.push(std::mem::replace(&mut self.deck, next));
            }
        });
        ui.menu_button("Diapositive", |ui| {
            if ui.add(egui::Button::new("Nouvelle diapositive").shortcut_text("Ctrl+M")).clicked() {
                self.add(&ctx);
            }
            if ui.button("Dupliquer").clicked() {
                self.snapshot(now, false);
                self.current = self.deck.duplicate(self.current);
            }
            if ui.add_enabled(self.deck.slides.len() > 1, egui::Button::new("Supprimer")).clicked() {
                self.snapshot(now, false);
                self.deck.remove(self.current);
            }
            ui.separator();
            if ui.button("Monter").clicked() && self.current > 0 {
                self.snapshot(now, false);
                self.deck.move_slide(self.current, self.current - 1);
                self.current -= 1;
            }
            if ui.button("Descendre").clicked() && self.current + 1 < self.deck.slides.len() {
                self.snapshot(now, false);
                self.deck.move_slide(self.current, self.current + 1);
                self.current += 1;
            }
        });
        ui.menu_button("Diaporama", |ui| {
            if ui.add(egui::Button::new("Depuis la première diapositive").shortcut_text("F5")).clicked() {
                self.start(&ctx, 0);
            }
            if ui.add(egui::Button::new("Depuis la diapositive actuelle").shortcut_text("Maj+F5")).clicked() {
                self.start(&ctx, self.current);
            }
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        ui.horizontal_wrapped(|ui| {
            if ui.button("➕ Diapositive").on_hover_text("Nouvelle diapositive (Ctrl+M)").clicked() {
                self.add(&ctx);
            }
            egui::ComboBox::from_id_salt("new-layout")
                .selected_text(self.new_layout.label())
                .width(150.0)
                .show_ui(ui, |ui| {
                    for l in Layout::ALL {
                        ui.selectable_value(&mut self.new_layout, l, l.label());
                    }
                });
            if ui.button("⧉").on_hover_text("Dupliquer").clicked() {
                self.snapshot(now, false);
                self.current = self.deck.duplicate(self.current);
            }
            if ui
                .add_enabled(self.deck.slides.len() > 1, egui::Button::new("🗑"))
                .on_hover_text("Supprimer")
                .clicked()
            {
                self.snapshot(now, false);
                self.deck.remove(self.current);
            }
            ui.separator();
            egui::ComboBox::from_id_salt("theme")
                .selected_text(format!("Thème : {}", self.deck.theme.label()))
                .show_ui(ui, |ui| {
                    for t in Theme::ALL {
                        if ui.selectable_label(self.deck.theme == t, t.label()).clicked() {
                            self.snapshot(now, false);
                            self.deck.theme = t;
                        }
                    }
                });
            ui.separator();
            ui.add(
                egui::TextEdit::singleline(&mut self.deck.title)
                    .hint_text("Titre de la présentation")
                    .desired_width(200.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(RichText::new("▶ Présenter").strong()).fill(Color32::from_rgb(0x1F, 0x5F, 0xBF)))
                    .on_hover_text("Lancer le diaporama (F5 ; Maj+F5 depuis la diapositive actuelle)")
                    .clicked()
                {
                    self.start(&ctx, self.current);
                }
            });
        });
    }

    fn thumbnails(&mut self, ui: &mut egui::Ui) {
        let values = self.deck.values();
        let ratio = self.deck.aspect.ratio();
        let mut select = None;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.add_space(6.0);
            let w = ui.available_width() - 30.0;
            for i in 0..self.deck.slides.len() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}", i + 1)).small().weak());
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, w / ratio), Sense::click());
                    render_slide(ui, rect, &self.deck, i, &values);
                    let stroke = if i == self.current {
                        Stroke::new(3.0, Color32::from_rgb(0x2F, 0x74, 0xD6))
                    } else {
                        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
                    };
                    ui.painter().rect_stroke(rect, 3.0, stroke, egui::StrokeKind::Outside);
                    if resp.clicked() {
                        select = Some(i);
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Présenter à partir d'ici").clicked() {
                            self.start(ui.ctx(), i);
                        }
                    });
                });
                ui.add_space(8.0);
            }
        });
        if let Some(i) = select {
            self.current = i;
        }
    }

    fn editor(&mut self, ui: &mut egui::Ui) {
        let now = ui.input(|i| i.time);
        let values = self.deck.values();
        let ratio = self.deck.aspect.ratio();
        // La diapositive, aussi grande que possible.
        let avail = ui.available_size();
        let h = (avail.y * 0.56).min(avail.x / ratio);
        let w = h * ratio;
        ui.vertical_centered(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
            ui.painter().rect_filled(rect.translate(Vec2::new(0.0, 3.0)).expand(1.0), 4.0, Color32::from_black_alpha(50));
            render_slide(ui, rect, &self.deck, self.current, &values);
        });
        ui.add_space(8.0);
        let i = self.current;
        let layout = self.deck.slides[i].layout;
        let bx = varium_slides::boxes(layout);
        let before = self.deck.slides[i].clone();
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let s = &mut self.deck.slides[i];
            egui::Grid::new("slide-form").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                if bx.title.is_some() {
                    ui.label("Titre");
                    ui.add(egui::TextEdit::singleline(&mut s.title).desired_width(f32::INFINITY).hint_text("Titre de la diapositive"));
                    ui.end_row();
                }
                if bx.subtitle.is_some() {
                    ui.label("Sous-titre");
                    ui.add(egui::TextEdit::singleline(&mut s.subtitle).desired_width(f32::INFINITY));
                    ui.end_row();
                }
                if bx.body.is_some() {
                    ui.label(if bx.body2.is_some() { "Colonne 1" } else { "Contenu" });
                    ui.add(
                        egui::TextEdit::multiline(&mut s.body)
                            .desired_width(f32::INFINITY)
                            .desired_rows(5)
                            .hint_text("- une puce\n- **gras**, *italique*, {{champ}}\n  - sous-puce"),
                    );
                    ui.end_row();
                }
                if bx.body2.is_some() {
                    ui.label("Colonne 2");
                    ui.add(egui::TextEdit::multiline(&mut s.body2).desired_width(f32::INFINITY).desired_rows(5));
                    ui.end_row();
                }
                ui.label("Notes");
                ui.add(
                    egui::TextEdit::multiline(&mut s.notes)
                        .desired_width(f32::INFINITY)
                        .desired_rows(3)
                        .hint_text("Notes de l'orateur (touche N pendant le diaporama)"),
                );
                ui.end_row();
            });
        });
        if self.deck.slides[i] != before {
            // Une seule étape d'annulation pour la frappe continue.
            let after = std::mem::replace(&mut self.deck.slides[i], before);
            self.snapshot(now, true);
            self.deck.slides[i] = after;
        }
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        let now = ui.input(|i| i.time);
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.add_space(4.0);
            ui.label(RichText::new("Diapositive").strong());
            let i = self.current;
            let mut layout = self.deck.slides[i].layout;
            egui::ComboBox::from_label("Disposition")
                .selected_text(layout.label())
                .show_ui(ui, |ui| {
                    for l in Layout::ALL {
                        ui.selectable_value(&mut layout, l, l.label());
                    }
                });
            if layout != self.deck.slides[i].layout {
                self.snapshot(now, false);
                self.deck.slides[i].layout = layout;
            }
            let mut tr = self.deck.slides[i].transition;
            egui::ComboBox::from_label("Transition")
                .selected_text(tr.label())
                .show_ui(ui, |ui| {
                    for t in Transition::ALL {
                        ui.selectable_value(&mut tr, t, t.label());
                    }
                });
            if tr != self.deck.slides[i].transition {
                self.snapshot(now, false);
                self.deck.slides[i].transition = tr;
            }
            ui.label("Fond");
            ui.horizontal_wrapped(|ui| {
                let theme_bg = self.deck.theme.palette().background;
                for (name, c) in [
                    ("Thème", None),
                    ("Blanc", Some([0xFF, 0xFF, 0xFF])),
                    ("Nuit", Some([0x14, 0x1B, 0x2D])),
                    ("Bleu", Some([0x1F, 0x5F, 0xBF])),
                    ("Sable", Some([0xF4, 0xEC, 0xDC])),
                    ("Vert", Some([0x1E, 0x5B, 0x46])),
                    ("Corail", Some([0xF2, 0x6B, 0x5B])),
                ] {
                    let fill = rgb(c.unwrap_or(theme_bg));
                    let on = self.deck.slides[i].background == c;
                    let (r, resp) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                    ui.painter().rect_filled(r, 4.0, fill);
                    ui.painter().rect_stroke(
                        r,
                        4.0,
                        Stroke::new(if on { 2.5 } else { 1.0 }, if on { Color32::from_rgb(0x2F, 0x74, 0xD6) } else { Color32::GRAY }),
                        egui::StrokeKind::Outside,
                    );
                    if resp.on_hover_text(name).clicked() {
                        self.snapshot(now, false);
                        self.deck.slides[i].background = c;
                    }
                }
            });

            ui.separator();
            ui.label(RichText::new("Présentation").strong());
            let mut aspect = self.deck.aspect;
            ui.horizontal(|ui| {
                ui.selectable_value(&mut aspect, Aspect::Large, "16:9");
                ui.selectable_value(&mut aspect, Aspect::Standard, "4:3");
            });
            if aspect != self.deck.aspect {
                self.snapshot(now, false);
                self.deck.aspect = aspect;
            }
            ui.add(egui::TextEdit::singleline(&mut self.deck.footer).hint_text("Pied de page (ex. {{entreprise}})"));
            ui.checkbox(&mut self.deck.slide_numbers, "Numéros de diapositive");

            ui.separator();
            ui.label(RichText::new("Champs").strong());
            ui.label(RichText::new("Écrivez {{nom}} dans une diapositive : la valeur s'affiche partout.").small().weak());
            let vars = self.deck.variables();
            if vars.is_empty() {
                ui.label(RichText::new("Aucun champ.").weak());
            }
            for v in vars {
                ui.label(RichText::new(&v).monospace());
                let value = self.deck.values.entry(v.clone()).or_default();
                ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
            }
            ui.separator();
            ui.label(RichText::new("Mise en forme du contenu").strong());
            for (k, d) in [
                ("- puce", "liste à puces (2 espaces = sous-puce)"),
                ("1. élément", "liste numérotée"),
                ("**gras** *italique*", "caractères"),
                ("> citation", "citation"),
                ("| a | b |", "tableau"),
                ("{{champ}}", "valeur partagée"),
                ("$x^2$", "formule"),
            ] {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(k).monospace().small());
                    ui.label(RichText::new(d).small().weak());
                });
            }
        });
    }

    // ---------------- Mode présentation ----------------

    fn present(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let Some((mut i, mut since)) = self.presenting else { return };
        let n = self.deck.slides.len();
        let now = ctx.input(|i| i.time);
        let (next, prev, esc, home, end, notes) = ctx.input(|inp| {
            (
                inp.key_pressed(Key::ArrowRight)
                    || inp.key_pressed(Key::Space)
                    || inp.key_pressed(Key::PageDown)
                    || inp.key_pressed(Key::Enter)
                    || inp.pointer.primary_clicked(),
                inp.key_pressed(Key::ArrowLeft) || inp.key_pressed(Key::PageUp) || inp.key_pressed(Key::Backspace),
                inp.key_pressed(Key::Escape),
                inp.key_pressed(Key::Home),
                inp.key_pressed(Key::End),
                inp.key_pressed(Key::N),
            )
        });
        if notes {
            self.show_notes = !self.show_notes;
        }
        if esc {
            self.stop(&ctx);
            return;
        }
        let target = if next {
            if i + 1 >= n {
                self.stop(&ctx);
                return;
            }
            i + 1
        } else if prev {
            i.saturating_sub(1)
        } else if home {
            0
        } else if end {
            n - 1
        } else {
            i
        };
        if target != i {
            i = target;
            since = now;
        }
        self.presenting = Some((i, since));

        let values = self.deck.values();
        let screen = ui.max_rect();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::BLACK))
            .show(ui, |ui| {
                let ratio = self.deck.aspect.ratio();
                let area = screen;
                let (w, h) = if area.width() / area.height() > ratio {
                    (area.height() * ratio, area.height())
                } else {
                    (area.width(), area.width() / ratio)
                };
                let mut rect = Rect::from_center_size(area.center(), Vec2::new(w, h));
                // Transition
                let t = ((now - since) / 0.45).clamp(0.0, 1.0) as f32;
                let ease = 1.0 - (1.0 - t).powi(3);
                let tr = self.deck.slides[i].transition;
                match tr {
                    Transition::Glissement => rect = rect.translate(Vec2::new((1.0 - ease) * w * 0.25, 0.0)),
                    Transition::Zoom => rect = Rect::from_center_size(rect.center(), rect.size() * (0.85 + 0.15 * ease)),
                    _ => {}
                }
                render_slide(ui, rect, &self.deck, i, &values);
                if tr != Transition::Aucune && t < 1.0 {
                    ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(((1.0 - ease) * 255.0) as u8));
                    ctx.request_repaint();
                }
                if self.show_notes {
                    let notes = self.deck.slides[i].notes.clone();
                    let text = if notes.trim().is_empty() { "(pas de notes)".to_string() } else { notes };
                    let galley = ui.painter().layout(
                        format!("{}/{} — {}", i + 1, n, text),
                        FontId::proportional(18.0),
                        Color32::WHITE,
                        screen.width() - 80.0,
                    );
                    let r = Rect::from_min_size(
                        Pos2::new(screen.left() + 30.0, screen.bottom() - galley.size().y - 40.0),
                        galley.size() + Vec2::splat(20.0),
                    );
                    ui.painter().rect_filled(r, 8.0, Color32::from_black_alpha(200));
                    ui.painter().galley(r.min + Vec2::splat(10.0), galley, Color32::WHITE);
                }
            });
    }

    fn toast_ui(&mut self, ctx: &egui::Context) {
        let Some((msg, at, error)) = self.toast.clone() else { return };
        if ctx.input(|i| i.time) - at > 4.0 {
            self.toast = None;
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
        egui::Area::new(egui::Id::unique("slides-toast"))
            .anchor(Align2::CENTER_BOTTOM, [0.0, -40.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let c = if error { Color32::from_rgb(0xD0, 0x30, 0x2A) } else { ui.visuals().text_color() };
                    ui.label(RichText::new(msg).color(c));
                });
            });
    }
}

// ---------------------------------------------------------------------------------
// Rendu d'une diapositive (vignettes, édition, diaporama)
// ---------------------------------------------------------------------------------

fn frame_rect(slide: Rect, f: Frame) -> Rect {
    Rect::from_min_size(
        slide.min + Vec2::new(f.x * slide.width(), f.y * slide.height()),
        Vec2::new(f.w * slide.width(), f.h * slide.height()),
    )
}

struct Ink {
    font: DocFont,
    color: Color32,
    accent: Color32,
    size: f32,
}

fn runs_job(runs: &[Run], ink: &Ink, bold: bool, italic: bool, size: f32, color: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    for r in runs {
        let st = r.style();
        let b = bold || st.bold;
        let it = italic || st.italic || matches!(r, Run::Math { .. });
        let mut f = egui::TextFormat {
            font_id: FontId::new(size, crate::fonts::family(ink.font, b, it)),
            color,
            italics: it && !crate::fonts::has_italic(ink.font),
            line_height: Some(size * 1.25),
            ..Default::default()
        };
        if st.underline {
            f.underline = Stroke::new((size / 18.0).max(1.0), color);
        }
        if st.strike {
            f.strikethrough = Stroke::new((size / 18.0).max(1.0), color);
        }
        let text = match r {
            Run::Missing { raw, .. } => {
                f.color = Color32::from_rgb(0xE0, 0x5A, 0x4F);
                raw.clone()
            }
            other => other.plain(),
        };
        job.append(&text, 0.0, f);
    }
    if job.is_empty() {
        job.append(" ", 0.0, egui::TextFormat { font_id: FontId::proportional(size), ..Default::default() });
    }
    job
}

/// Dessine le texte mis en forme d'une zone (puces, titres, citations, tableaux).
fn draw_body(ui: &egui::Ui, area: Rect, paras: &[Para], ink: &Ink, centered: bool) {
    let painter = ui.painter_at(area);
    let mut y = area.top();
    let s = ink.size;
    for p in paras {
        if y > area.bottom() {
            break;
        }
        let (mut job, indent, bullet) = match p {
            Para::Heading { runs, level } => {
                let k = [1.35, 1.2, 1.1][(*level as usize).clamp(1, 3) - 1];
                (runs_job(runs, ink, true, false, s * k, ink.color), 0.0, None)
            }
            Para::Item { number, depth, runs } => {
                let label = match number {
                    Some(n) => format!("{n}."),
                    None => ["•", "–", "◦"][*depth as usize % 3].to_string(),
                };
                let ind = s * 1.3 * (*depth as f32 + 1.0);
                (runs_job(runs, ink, false, false, s * if *depth > 0 { 0.9 } else { 1.0 }, ink.color), ind, Some(label))
            }
            Para::Quote { runs } => (runs_job(runs, ink, false, true, s * 1.1, ink.color), s, None),
            Para::Text { runs, .. } if runs.is_empty() => {
                y += s * 0.6;
                continue;
            }
            Para::Text { runs, .. } => (runs_job(runs, ink, false, false, s, ink.color), 0.0, None),
            Para::Math { .. } => {
                let f = egui::TextFormat {
                    font_id: FontId::new(s * 1.1, crate::fonts::family(ink.font, false, true)),
                    color: ink.color,
                    ..Default::default()
                };
                (LayoutJob::single_section(p.plain(), f), 0.0, None)
            }
            Para::Table(t) => {
                let cols = t.columns().max(1);
                let cw = area.width() / cols as f32;
                for (ri, row) in t.rows.iter().enumerate() {
                    let rh = s * 1.5;
                    for c in 0..cols {
                        let cell = Rect::from_min_size(Pos2::new(area.left() + cw * c as f32, y), Vec2::new(cw, rh));
                        let head = ri == 0 && t.header;
                        if head {
                            painter.rect_filled(cell, 0.0, ink.accent.gamma_multiply(0.25));
                        }
                        painter.rect_stroke(cell, 0.0, Stroke::new(1.0, ink.color.gamma_multiply(0.4)), egui::StrokeKind::Inside);
                        let empty = Vec::new();
                        let mut job = runs_job(row.get(c).unwrap_or(&empty), ink, head, false, s * 0.8, ink.color);
                        job.wrap.max_width = cw - 8.0;
                        let g = ui.ctx().fonts_mut(|f| f.layout_job(job));
                        painter.galley(Pos2::new(cell.left() + 4.0, cell.center().y - g.size().y / 2.0), g, ink.color);
                    }
                    y += rh;
                }
                y += s * 0.3;
                continue;
            }
            Para::Rule => {
                painter.line_segment([Pos2::new(area.left(), y + s * 0.3), Pos2::new(area.right(), y + s * 0.3)], Stroke::new(1.5, ink.accent));
                y += s * 0.7;
                continue;
            }
            Para::PageBreak => continue,
        };
        job.wrap.max_width = area.width() - indent;
        if centered {
            job.halign = egui::Align::Center;
        }
        let g = ui.ctx().fonts_mut(|f| f.layout_job(job));
        let x = if centered { area.center().x } else { area.left() + indent };
        if let Some(label) = bullet {
            painter.text(Pos2::new(area.left() + indent - s * 1.1, y), Align2::LEFT_TOP, label, FontId::new(s, crate::fonts::family(ink.font, true, false)), ink.accent);
        }
        if matches!(p, Para::Quote { .. }) {
            painter.line_segment([Pos2::new(area.left() + s * 0.3, y), Pos2::new(area.left() + s * 0.3, y + g.size().y)], Stroke::new(s * 0.12, ink.accent));
        }
        let h = g.size().y;
        painter.galley(Pos2::new(x, y), g, ink.color);
        y += h + s * 0.35;
    }
}

/// Dessine la diapositive `i` du diaporama dans `rect`.
pub fn render_slide(ui: &egui::Ui, rect: Rect, deck: &Deck, i: usize, values: &Values) {
    let Some(slide) = deck.slides.get(i) else { return };
    let pal = deck.theme.palette();
    let bg = rgb(slide.background.unwrap_or(pal.background));
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, bg);
    // Sur un fond personnalisé, le texte reste lisible.
    let dark_bg = (bg.r() as u32 + bg.g() as u32 + bg.b() as u32) < 380;
    let pal_dark = (pal.background[0] as u32 + pal.background[1] as u32 + pal.background[2] as u32) < 380;
    let (text, title) = if slide.background.is_some() && dark_bg != pal_dark {
        if dark_bg { (Color32::from_gray(240), Color32::WHITE) } else { (Color32::from_gray(30), Color32::from_gray(15)) }
    } else {
        (rgb(pal.text), rgb(pal.title))
    };
    let accent = rgb(pal.accent);
    let h = rect.height();
    let layout = slide.layout;
    let bx = varium_slides::boxes(layout);
    let title_font = if pal.serif_titles { DocFont::Serif } else { DocFont::Sans };
    let centered = matches!(layout, Layout::Title | Layout::Section | Layout::Quote);

    if let Some(f) = bx.title {
        let r = frame_rect(rect, f);
        let ink = Ink { font: title_font, color: title, accent, size: varium_slides::title_size(layout) * h };
        let runs = varium_core::doc::inline(&slide.title, values);
        let mut job = runs_job(&runs, &ink, true, false, ink.size, title);
        job.wrap.max_width = r.width();
        if centered {
            job.halign = egui::Align::Center;
        }
        let g = ui.ctx().fonts_mut(|fo| fo.layout_job(job));
        let x = if centered { r.center().x } else { r.left() };
        let y = if matches!(layout, Layout::Title | Layout::Section) { r.bottom() - g.size().y } else { r.top() };
        painter.with_clip_rect(rect).galley(Pos2::new(x, y.max(r.top())), g, title);
        if layout == Layout::Section {
            painter.line_segment(
                [Pos2::new(r.center().x - r.width() * 0.08, r.bottom() + h * 0.03), Pos2::new(r.center().x + r.width() * 0.08, r.bottom() + h * 0.03)],
                Stroke::new(h * 0.008, accent),
            );
        }
    }
    if let Some(f) = bx.subtitle {
        let r = frame_rect(rect, f);
        let ink = Ink { font: DocFont::Sans, color: text, accent, size: varium_slides::body_size(layout) * h * 1.05 };
        let runs = varium_core::doc::inline(&slide.subtitle, values);
        let mut job = runs_job(&runs, &ink, false, false, ink.size, text.gamma_multiply(0.85));
        job.wrap.max_width = r.width();
        if centered {
            job.halign = egui::Align::Center;
        }
        let g = ui.ctx().fonts_mut(|fo| fo.layout_job(job));
        let x = if centered { r.center().x } else { r.left() };
        painter.galley(Pos2::new(x, r.top()), g, text);
    }
    let body_ink = Ink {
        font: if layout == Layout::Quote { DocFont::Serif } else { DocFont::Sans },
        color: text,
        accent,
        size: varium_slides::body_size(layout) * h,
    };
    for (f, src) in [(bx.body, &slide.body), (bx.body2, &slide.body2)] {
        if let Some(f) = f {
            let paras = varium_core::document(src, values);
            draw_body(ui, frame_rect(rect, f), &paras, &body_ink, layout == Layout::Quote);
        }
    }
    // Pied de page et numéro
    let small = FontId::new(h * 0.028, crate::fonts::family(DocFont::Sans, false, false));
    let muted = rgb(pal.muted);
    if !deck.footer.trim().is_empty() {
        let foot = varium_core::doc::plain(&varium_core::doc::inline(&deck.footer, values));
        painter.text(Pos2::new(rect.left() + rect.width() * 0.05, rect.bottom() - h * 0.04), Align2::LEFT_BOTTOM, foot, small.clone(), muted);
    }
    if deck.slide_numbers {
        painter.text(
            Pos2::new(rect.right() - rect.width() * 0.05, rect.bottom() - h * 0.04),
            Align2::RIGHT_BOTTOM,
            format!("{}", i + 1),
            small,
            muted,
        );
    }
}

/// Une nouvelle diapositive est toujours valide (pour les tests).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_cadres_restent_dans_la_diapositive() {
        let r = Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0));
        for l in Layout::ALL {
            let b = varium_slides::boxes(l);
            for f in [b.title, b.subtitle, b.body, b.body2].into_iter().flatten() {
                let fr = frame_rect(r, f);
                assert!(r.expand(0.5).contains_rect(fr), "{l:?}");
            }
        }
        let _ = Slide::default();
    }
}
