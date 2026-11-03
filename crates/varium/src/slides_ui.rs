//! Module Présentation : des diaporamas à la manière d'Impress et de Keynote.
//! Vignettes, édition de la diapositive, dispositions, thèmes, transitions, notes,
//! {{champs}} partagés, images et formes manipulables à la souris, mode
//! présentation plein écran, mode présentateur sur deuxième écran, import ODP /
//! PPTX et exports PDF / ODP / HTML.
//! Le modèle et les exports viennent du moteur `varium-slides`.

use crate::labels;
use crate::tr::tr;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use egui::text::LayoutJob;
use egui::{Align2, Color32, CursorIcon, FontId, Key, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use varium_core::{DocFont, Para, Run, Values};
use varium_slides::{
    Aspect, Deck, Frame, Layout, Object, ObjectKind, Theme, Transition, arrow_head,
};

use crate::io::{Event, Io, OpenKind};

pub const STORAGE_KEY: &str = "varium.slides";

/// Fenêtre du public en mode présentateur.
const AUDIENCE_ID: &str = "varium-diaporama-public";

/// Les commandes de la Présentation, communes aux menus egui, à la barre native de
/// macOS et au clavier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlideAction {
    New,
    Sample,
    Open,
    Save,
    SaveAs,
    ExportPdf,
    ExportOdp,
    ExportHtml,
    Undo,
    Redo,
    DuplicateObject,
    DeleteObject,
    InsertImage,
    InsertShape(ObjectKind),
    NewSlide,
    DuplicateSlide,
    DeleteSlide,
    MoveUp,
    MoveDown,
    StartFirst,
    StartCurrent,
    Presenter,
}

pub struct SlidesApp {
    deck: Deck,
    current: usize,
    /// Mode présentation : diapositive affichée et heure de son apparition.
    presenting: Option<(usize, f64)>,
    /// Mode présentateur (deuxième fenêtre pour le public).
    presenter: Option<Presenter>,
    /// Écran noir (touche B) pendant le diaporama.
    black: bool,
    show_notes: bool,
    undo: Vec<Deck>,
    redo: Vec<Deck>,
    last_change: f64,
    path: Option<PathBuf>,
    io: Io,
    toast: Option<(String, f64, bool)>,
    new_layout: Layout,
    /// Objet sélectionné : (diapositive, position dans `objects`).
    selected: Option<(usize, usize)>,
    /// Manipulation à la souris en cours.
    drag: Option<Drag>,
    textures: Textures,
    /// Images choisies dans le dialogue « Insertion → Image… » : (nom, octets).
    picked: Picked,
    /// Dernière action du clavier ou du menu natif, pour ne pas la doubler.
    last_act: Option<(SlideAction, f64)>,
    /// Les menus sont dans la barre native de macOS (pas de barre dans la fenêtre).
    pub native_menu: bool,
}

/// Fichiers choisis dans un dialogue : (nom, octets).
type Picked = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

/// Mode présentateur : chronomètre, taille des notes, plein écran du public.
struct Presenter {
    started: f64,
    /// Heure de la mise en pause, si le chronomètre est arrêté.
    paused: Option<f64>,
    /// Durée totale des pauses passées.
    paused_total: f64,
    notes_size: f32,
}

impl Presenter {
    fn elapsed(&self, now: f64) -> f64 {
        (self.paused.unwrap_or(now) - self.started - self.paused_total).max(0.0)
    }
}

/// Ce que l'on fait d'un objet en le tirant.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Grip {
    Move,
    /// Poignée de redimensionnement : 0 en haut à gauche, puis dans le sens des
    /// aiguilles d'une montre (8 poignées).
    Handle(usize),
    /// Extrémité d'une ligne : départ (`false`) ou arrivée (`true`).
    End(bool),
}

struct Drag {
    slide: usize,
    object: usize,
    grip: Grip,
    origin: Pos2,
    start: Object,
}

/// Commandes du diaporama, venues du clavier ou de la souris d'une des fenêtres.
#[derive(Clone, Copy, PartialEq)]
enum Nav {
    Next,
    Prev,
    First,
    Last,
    Black,
    Stop,
}

/// Textures des images de la présentation, par identifiant de média.
#[derive(Default)]
pub struct Textures {
    map: HashMap<String, Option<egui::TextureHandle>>,
}

impl Textures {
    fn get(&mut self, ctx: &egui::Context, deck: &Deck, id: &str) -> Option<egui::TextureId> {
        if let Some(t) = self.map.get(id) {
            return t.as_ref().map(|t| t.id());
        }
        let tex = deck.media.get(id).and_then(|m| {
            let img = image::load_from_memory(&m.data).ok()?.to_rgba8();
            let size = [img.width() as usize, img.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
            Some(ctx.load_texture(format!("slides-{id}"), color, egui::TextureOptions::LINEAR))
        });
        let id_tex = tex.as_ref().map(|t| t.id());
        self.map.insert(id.to_string(), tex);
        id_tex
    }
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Exécuteur minimal pour le dialogue natif asynchrone de rfd (comme `io`).
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};

    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    let mut fut = std::pin::pin!(fut);
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}

/// Le fichier est une image (d'après son extension ou ses premiers octets).
fn is_image(name: &str, bytes: &[u8]) -> bool {
    let n = name.to_lowercase();
    n.ends_with(".png")
        || n.ends_with(".jpg")
        || n.ends_with(".jpeg")
        || bytes.starts_with(b"\x89PNG")
        || bytes.starts_with(&[0xFF, 0xD8, 0xFF])
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
            presenter: None,
            black: false,
            show_notes: false,
            undo: Vec::new(),
            redo: Vec::new(),
            last_change: 0.0,
            path: None,
            io: Io::default(),
            toast: None,
            new_layout: Layout::TitleContent,
            selected: None,
            drag: None,
            textures: Textures::default(),
            picked: Arc::default(),
            last_act: None,
            native_menu: false,
        }
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        let mut deck = self.deck.clone();
        deck.prune_media();
        eframe::set_value(storage, STORAGE_KEY, &deck);
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
        trf!("{name} — Varium Présentation", name)
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

    fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.deck, prev));
            self.last_change = 0.0;
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.deck, next));
            self.last_change = 0.0;
        }
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
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let s = s.trim_matches('-').to_string();
        if s.is_empty() {
            "presentation".into()
        } else {
            s
        }
    }

    /// Remplace la présentation par celle d'un fichier ouvert (JSON, ODP, PPTX).
    fn load_bytes(&mut self, ctx: &egui::Context, name: &str, bytes: &[u8], path: Option<PathBuf>) {
        match Deck::open_bytes(bytes) {
            Ok(mut d) => {
                let lower = name.to_lowercase();
                let imported = lower.ends_with(".odp") || lower.ends_with(".pptx");
                if d.title.trim().is_empty() {
                    d.title = name
                        .rsplit_once('.')
                        .map_or(name, |(s, _)| s)
                        .trim_end_matches(".varium-slides")
                        .to_string();
                }
                self.undo.push(std::mem::replace(&mut self.deck, d));
                // Un ODP ou un PPTX importé s'enregistre en présentation Varium :
                // le fichier d'origine n'est jamais écrasé.
                self.path = if imported { None } else { path };
                self.current = 0;
                self.selected = None;
                self.textures = Textures::default();
                let n = self.deck.slides.len();
                self.notify(
                    ctx,
                    if imported {
                        trf!("« {name} » importé ({n} diapositive(s))", name, n)
                    } else {
                        trf!("« {name} » ouvert", name)
                    },
                    false,
                );
            }
            Err(e) => self.notify(ctx, format!("« {name} » : {e}"), true),
        }
    }

    fn handle_io(&mut self, ctx: &egui::Context) {
        for ev in self.io.drain() {
            match ev {
                Event::Opened {
                    kind: OpenKind::Deck,
                    name,
                    content,
                    path,
                } => {
                    // Les fichiers ODP et PPTX arrivent octet pour caractère (voir `io`).
                    let lower = name.to_lowercase();
                    let bytes: Vec<u8> = if lower.ends_with(".odp")
                        || lower.ends_with(".pptx")
                        || content.starts_with("PK\u{3}\u{4}")
                    {
                        content.chars().map(|c| c as u32 as u8).collect()
                    } else {
                        content.into_bytes()
                    };
                    self.load_bytes(ctx, &name, &bytes, path);
                }
                Event::Opened { .. } => {}
                Event::Saved { path, project } => {
                    if project {
                        self.path = Some(path.clone());
                    }
                    self.notify(
                        ctx,
                        trf!("Enregistré : {path}", path = path.display()),
                        false,
                    );
                }
                Event::Info(m) => self.notify(ctx, m, false),
                Event::Error(e) => self.notify(ctx, e, true),
            }
        }
        let picked = std::mem::take(&mut *self.picked.lock().unwrap_or_else(|e| e.into_inner()));
        for (name, bytes) in picked {
            self.insert_image(ctx, &name, &bytes, None);
        }
    }

    fn save_file(&mut self, ctx: &egui::Context, as_new: bool) {
        self.deck.prune_media();
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
            _ => (
                format!("{stem}.html"),
                "text/html",
                self.deck.to_html().into_bytes(),
            ),
        };
        self.io.save(ctx, name, mime, bytes, false);
    }

    /// Ouvre le dialogue de choix d'une image ; elle est insérée à son retour.
    fn pick_image(&self, ctx: &egui::Context) {
        let inbox = self.picked.clone();
        let ctx = ctx.clone();
        let dialog = rfd::AsyncFileDialog::new()
            .set_title(tr("Insérer une image"))
            .add_filter(tr("Image PNG ou JPEG"), &["png", "jpg", "jpeg"]);
        std::thread::spawn(move || {
            block_on(async move {
                let Some(file) = dialog.pick_file().await else {
                    return;
                };
                let name = file.file_name();
                let bytes = file.read().await;
                inbox
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push((name, bytes));
                ctx.request_repaint();
            })
        });
    }

    /// Insère une image sur la diapositive courante, centrée sur `at` (fractions
    /// de la diapositive) si c'est un dépôt à la souris.
    fn insert_image(
        &mut self,
        ctx: &egui::Context,
        name: &str,
        bytes: &[u8],
        at: Option<(f32, f32)>,
    ) {
        let now = ctx.input(|i| i.time);
        let before = self.deck.clone();
        match self.deck.insert_image(self.current, bytes) {
            Ok(k) => {
                self.undo.push(before);
                self.redo.clear();
                self.last_change = now;
                let s = &mut self.deck.slides[self.current];
                if let (Some((x, y)), Some(o)) = (at, s.objects.get_mut(k)) {
                    o.frame.x =
                        (x - o.frame.w / 2.0).clamp(-o.frame.w / 2.0, 1.0 - o.frame.w / 2.0);
                    o.frame.y =
                        (y - o.frame.h / 2.0).clamp(-o.frame.h / 2.0, 1.0 - o.frame.h / 2.0);
                }
                self.selected = Some((self.current, k));
                self.notify(ctx, trf!("Image « {name} » insérée", name), false);
            }
            Err(e) => self.notify(ctx, format!("« {name} » : {e}"), true),
        }
    }

    fn insert_shape(&mut self, ctx: &egui::Context, kind: ObjectKind) {
        self.snapshot(ctx.input(|i| i.time), false);
        let k = self.deck.insert_shape(self.current, kind);
        self.selected = Some((self.current, k));
    }

    /// Fichiers déposés sur la fenêtre : images insérées, présentations ouvertes.
    fn dropped_files(&mut self, ctx: &egui::Context, canvas: Option<Rect>) {
        let files = ctx.input(|i| i.raw.dropped_files.clone());
        if files.is_empty() {
            return;
        }
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        for f in files {
            let path = f.path().to_path_buf();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| tr("fichier").into());
            let bytes = match f.bytes() {
                Ok(b) => b,
                Err(e) => {
                    self.notify(ctx, trf!("« {name} » illisible : {e}", name, e), true);
                    continue;
                }
            };
            if is_image(&name, &bytes) {
                let at = match (canvas, pointer) {
                    (Some(r), Some(p)) if r.contains(p) => {
                        Some(((p.x - r.left()) / r.width(), (p.y - r.top()) / r.height()))
                    }
                    _ => None,
                };
                self.insert_image(ctx, &name, &bytes, at);
            } else {
                let path = (!path.as_os_str().is_empty()).then_some(path);
                self.load_bytes(ctx, &name, &bytes, path);
            }
        }
    }

    fn start(&mut self, ctx: &egui::Context, from: usize) {
        let now = ctx.input(|i| i.time);
        self.presenting = Some((from.min(self.deck.slides.len().saturating_sub(1)), now));
        self.black = false;
        self.drag = None;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
    }

    /// Mode présentateur : le public dans une seconde fenêtre, l'orateur ici.
    fn start_presenter(&mut self, ctx: &egui::Context, from: usize) {
        let now = ctx.input(|i| i.time);
        self.presenting = Some((from.min(self.deck.slides.len().saturating_sub(1)), now));
        self.presenter = Some(Presenter {
            started: now,
            paused: None,
            paused_total: 0.0,
            notes_size: 20.0,
        });
        self.black = false;
        self.drag = None;
    }

    fn stop(&mut self, ctx: &egui::Context) {
        if let Some((i, _)) = self.presenting.take() {
            self.current = i;
        }
        if self.presenter.take().is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
        self.black = false;
    }

    // ---------------- Interface ----------------

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.handle_io(&ctx);
        if self.deck.slides.is_empty() {
            self.deck.add_slide(0, Layout::Title);
        }
        self.current = self.current.min(self.deck.slides.len() - 1);
        // L'objet sélectionné doit exister sur la diapositive courante.
        if let Some((s, k)) = self.selected
            && (s != self.current || k >= self.deck.slides[s].objects.len())
        {
            self.selected = None;
        }

        if self.presenter.is_some() {
            self.presenter_ui(ui);
            return;
        }
        if self.presenting.is_some() {
            self.present(ui);
            return;
        }
        // Raccourcis
        let now = ctx.input(|i| i.time);
        if !ctx.egui_wants_keyboard_input() {
            let (f5, shift) = ctx.input(|i| (i.key_pressed(Key::F5), i.modifiers.shift));
            if f5 {
                let from = if shift { self.current } else { 0 };
                self.start(&ctx, from);
            }
            if ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::Z)
                    || i.consume_key(egui::Modifiers::COMMAND, Key::Y)
            }) {
                self.perform_once(&ctx, SlideAction::Redo);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::Z)) {
                self.perform_once(&ctx, SlideAction::Undo);
            }
            match self.selected {
                Some((s, k)) => self.object_keys(&ctx, s, k, now),
                None => {
                    if ctx.input(|i| i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::PageDown))
                    {
                        self.current = (self.current + 1).min(self.deck.slides.len() - 1);
                    }
                    if ctx.input(|i| i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::PageUp)) {
                        self.current = self.current.saturating_sub(1);
                    }
                }
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::M)) {
            self.perform_once(&ctx, SlideAction::NewSlide);
        }

        if !self.native_menu {
            egui::Panel::top("slides-menu").show(ui, |ui| {
                ui.add_space(2.0);
                egui::MenuBar::new().ui(ui, |ui| self.menus(ui));
            });
        }
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
        let canvas = egui::CentralPanel::default()
            .show(ui, |ui| self.editor(ui))
            .inner;
        self.dropped_files(&ctx, Some(canvas));
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            ui.painter().rect_filled(
                canvas,
                4.0,
                Color32::from_rgba_unmultiplied(0x2F, 0x74, 0xD6, 60),
            );
            ui.painter().text(
                canvas.center(),
                Align2::CENTER_CENTER,
                tr("Déposez une image (PNG, JPEG) ou une présentation (ODP, PPTX)"),
                FontId::proportional(18.0),
                Color32::WHITE,
            );
        }
        self.toast_ui(&ctx);
    }

    /// Clavier sur l'objet sélectionné : Suppr, flèches, Ctrl+D, Échap.
    fn object_keys(&mut self, ctx: &egui::Context, s: usize, k: usize, now: f64) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::D)) {
            self.perform_once(ctx, SlideAction::DuplicateObject);
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
            self.snapshot(now, false);
            self.deck.slides[s].remove_object(k);
            self.selected = None;
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.selected = None;
            return;
        }
        let (dx, dy) = ctx.input(|i| {
            let pas = if i.modifiers.shift { 0.02 } else { 0.004 };
            let mut d = (0.0, 0.0);
            if i.key_pressed(Key::ArrowLeft) {
                d.0 -= pas;
            }
            if i.key_pressed(Key::ArrowRight) {
                d.0 += pas;
            }
            if i.key_pressed(Key::ArrowUp) {
                d.1 -= pas;
            }
            if i.key_pressed(Key::ArrowDown) {
                d.1 += pas;
            }
            d
        });
        if dx != 0.0 || dy != 0.0 {
            self.snapshot(now, true);
            let o = &mut self.deck.slides[s].objects[k];
            o.frame.x += dx;
            o.frame.y += dy;
        }
    }

    fn add(&mut self, ctx: &egui::Context) {
        self.snapshot(ctx.input(|i| i.time), false);
        self.current = self.deck.add_slide(self.current, self.new_layout);
    }

    /// Un élément de menu egui qui lance une action.
    fn entry(&mut self, ui: &mut egui::Ui, enabled: bool, label: &str, keys: &str, a: SlideAction) {
        let b = egui::Button::new(label).shortcut_text(keys);
        if ui.add_enabled(enabled, b).clicked() {
            let ctx = ui.ctx().clone();
            self.perform(&ctx, a);
        }
    }

    fn menus(&mut self, ui: &mut egui::Ui) {
        use SlideAction as A;
        ui.menu_button(tr("Fichier"), |ui| {
            self.entry(ui, true, tr("Nouvelle présentation"), "", A::New);
            self.entry(ui, true, tr("Présentation d'exemple"), "", A::Sample);
            if ui
                .button(tr("Ouvrir…"))
                .on_hover_text(tr(
                    "Présentation Varium (.json), LibreOffice Impress (.odp) ou PowerPoint (.pptx)",
                ))
                .clicked()
            {
                let ctx = ui.ctx().clone();
                self.perform(&ctx, A::Open);
            }
            ui.separator();
            self.entry(ui, true, tr("Enregistrer"), "", A::Save);
            self.entry(ui, true, tr("Enregistrer sous…"), "", A::SaveAs);
            ui.menu_button(tr("Exporter"), |ui| {
                self.entry(
                    ui,
                    true,
                    tr("PDF (une page par diapositive)"),
                    "",
                    A::ExportPdf,
                );
                self.entry(ui, true, tr("LibreOffice Impress (.odp)"), "", A::ExportOdp);
                self.entry(ui, true, tr("Diaporama web (.html)"), "", A::ExportHtml);
            });
        });
        ui.menu_button(tr("Édition"), |ui| {
            self.entry(
                ui,
                !self.undo.is_empty(),
                tr("Annuler"),
                tr("Ctrl+Z"),
                A::Undo,
            );
            self.entry(
                ui,
                !self.redo.is_empty(),
                tr("Rétablir"),
                tr("Ctrl+Y"),
                A::Redo,
            );
            if self.selected.is_some() {
                ui.separator();
                self.entry(
                    ui,
                    true,
                    tr("Dupliquer l'objet"),
                    tr("Ctrl+D"),
                    A::DuplicateObject,
                );
                self.entry(
                    ui,
                    true,
                    tr("Supprimer l'objet"),
                    tr("Suppr"),
                    A::DeleteObject,
                );
            }
        });
        ui.menu_button(tr("Insertion"), |ui| {
            self.entry(ui, true, tr("🖼 Image…"), "", A::InsertImage);
            ui.separator();
            for kind in ObjectKind::SHAPES {
                self.entry(ui, true, &shape_label(kind), "", A::InsertShape(kind));
            }
        });
        ui.menu_button(tr("Diapositive"), |ui| {
            self.entry(
                ui,
                true,
                tr("Nouvelle diapositive"),
                tr("Ctrl+M"),
                A::NewSlide,
            );
            self.entry(ui, true, tr("Dupliquer"), "", A::DuplicateSlide);
            let several = self.deck.slides.len() > 1;
            self.entry(ui, several, tr("Supprimer"), "", A::DeleteSlide);
            ui.separator();
            self.entry(ui, self.current > 0, tr("Monter"), "", A::MoveUp);
            let last = self.current + 1 >= self.deck.slides.len();
            self.entry(ui, !last, tr("Descendre"), "", A::MoveDown);
        });
        ui.menu_button(tr("Diaporama"), |ui| {
            self.entry(ui, true, tr("Depuis la première diapositive"), "F5", A::StartFirst);
            self.entry(ui, true, tr("Depuis la diapositive actuelle"), tr("Maj+F5"), A::StartCurrent);
            ui.separator();
            if ui
                .button(tr("Mode présentateur"))
                .on_hover_text(tr(
                    "Une fenêtre pour le public (à glisser sur le projecteur, F pour le plein écran), \
                     les notes, la suivante et le chronomètre ici",
                ))
                .clicked()
            {
                let ctx = ui.ctx().clone();
                self.perform(&ctx, A::Presenter);
            }
        });
        ui.menu_button(tr("Aide"), |ui| {
            crate::tr::menu(ui);
        });
    }

    /// Lance une action des menus (egui ou barre native de macOS) ou du clavier.
    pub fn perform(&mut self, ctx: &egui::Context, a: SlideAction) {
        use SlideAction as A;
        if self.presenting.is_some() {
            return;
        }
        let now = ctx.input(|i| i.time);
        match a {
            A::New => {
                self.snapshot(now, false);
                self.deck = Deck::new();
                self.path = None;
                self.current = 0;
            }
            A::Sample => {
                self.snapshot(now, false);
                self.deck = Deck::sample();
                self.path = None;
                self.current = 0;
            }
            A::Open => self.io.open(ctx, OpenKind::Deck),
            A::Save => self.save_file(ctx, false),
            A::SaveAs => self.save_file(ctx, true),
            A::ExportPdf => self.export(ctx, "pdf"),
            A::ExportOdp => self.export(ctx, "odp"),
            A::ExportHtml => self.export(ctx, "html"),
            A::Undo => self.undo(),
            A::Redo => self.redo(),
            A::DuplicateObject => {
                if let Some((s, k)) = self.selected {
                    self.snapshot(now, false);
                    self.selected = self.deck.slides[s].duplicate_object(k).map(|k| (s, k));
                }
            }
            A::DeleteObject => {
                if let Some((s, k)) = self.selected {
                    self.snapshot(now, false);
                    self.deck.slides[s].remove_object(k);
                    self.selected = None;
                }
            }
            A::InsertImage => self.pick_image(ctx),
            A::InsertShape(kind) => self.insert_shape(ctx, kind),
            A::NewSlide => self.add(ctx),
            A::DuplicateSlide => {
                self.snapshot(now, false);
                self.current = self.deck.duplicate(self.current);
            }
            A::DeleteSlide => {
                if self.deck.slides.len() > 1 {
                    self.snapshot(now, false);
                    self.deck.remove(self.current);
                }
            }
            A::MoveUp => {
                if self.current > 0 {
                    self.snapshot(now, false);
                    self.deck.move_slide(self.current, self.current - 1);
                    self.current -= 1;
                }
            }
            A::MoveDown => {
                if self.current + 1 < self.deck.slides.len() {
                    self.snapshot(now, false);
                    self.deck.move_slide(self.current, self.current + 1);
                    self.current += 1;
                }
            }
            A::StartFirst => self.start(ctx, 0),
            A::StartCurrent => self.start(ctx, self.current),
            A::Presenter => self.start_presenter(ctx, self.current),
        }
    }

    /// Une action venue du clavier ou de la barre native : la même touche peut arriver
    /// par les deux chemins, on ne l'exécute qu'une fois.
    pub fn perform_once(&mut self, ctx: &egui::Context, a: SlideAction) {
        let now = ctx.input(|i| i.time);
        if let Some((last, t)) = self.last_act
            && last == a
            && now - t < 0.4
        {
            return;
        }
        if ctx.egui_wants_keyboard_input() && matches!(a, SlideAction::Undo | SlideAction::Redo) {
            return;
        }
        self.last_act = Some((a, now));
        self.perform(ctx, a);
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(tr("➕ Diapositive"))
                .on_hover_text(tr("Nouvelle diapositive (Ctrl+M)"))
                .clicked()
            {
                self.add(&ctx);
            }
            egui::ComboBox::from_id_salt("new-layout")
                .selected_text(labels::layout(self.new_layout))
                .width(150.0)
                .show_ui(ui, |ui| {
                    for l in Layout::ALL {
                        ui.selectable_value(&mut self.new_layout, l, labels::layout(l));
                    }
                });
            if ui.button("⧉").on_hover_text(tr("Dupliquer")).clicked() {
                self.snapshot(now, false);
                self.current = self.deck.duplicate(self.current);
            }
            if ui
                .add_enabled(self.deck.slides.len() > 1, egui::Button::new("🗑"))
                .on_hover_text(tr("Supprimer"))
                .clicked()
            {
                self.snapshot(now, false);
                self.deck.remove(self.current);
            }
            ui.separator();
            if ui
                .button(tr("🖼 Image"))
                .on_hover_text(tr(
                    "Insérer une image PNG ou JPEG (ou déposez-la sur la diapositive)",
                ))
                .clicked()
            {
                self.pick_image(&ctx);
            }
            ui.menu_button(tr("⬟ Forme"), |ui| {
                for kind in ObjectKind::SHAPES {
                    if ui.button(shape_label(kind)).clicked() {
                        self.insert_shape(&ctx, kind);
                    }
                }
            });
            ui.separator();
            egui::ComboBox::from_id_salt("theme")
                .selected_text(trf!(
                    "Thème : {theme}",
                    theme = labels::theme(self.deck.theme)
                ))
                .show_ui(ui, |ui| {
                    for t in Theme::ALL {
                        if ui
                            .selectable_label(self.deck.theme == t, labels::theme(t))
                            .clicked()
                        {
                            self.snapshot(now, false);
                            self.deck.theme = t;
                        }
                    }
                });
            ui.separator();
            ui.add(
                egui::TextEdit::singleline(&mut self.deck.title)
                    .hint_text(tr("Titre de la présentation"))
                    .desired_width(200.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new(tr("▶ Présenter")).strong())
                            .fill(Color32::from_rgb(0x1F, 0x5F, 0xBF)),
                    )
                    .on_hover_text(tr(
                        "Lancer le diaporama (F5 ; Maj+F5 depuis la diapositive actuelle)",
                    ))
                    .clicked()
                {
                    self.start(&ctx, self.current);
                }
                if ui
                    .button(tr("🖥 Présentateur"))
                    .on_hover_text(tr("Mode présentateur sur deuxième écran"))
                    .clicked()
                {
                    self.start_presenter(&ctx, self.current);
                }
            });
        });
    }

    fn thumbnails(&mut self, ui: &mut egui::Ui) {
        let values = self.deck.values();
        let ratio = self.deck.aspect.ratio();
        let mut select = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.add_space(6.0);
                let w = ui.available_width() - 30.0;
                for i in 0..self.deck.slides.len() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}", i + 1)).small().weak());
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(w, w / ratio), Sense::click());
                        if ui.is_rect_visible(rect) {
                            render_slide(ui, rect, &self.deck, i, &values, &mut self.textures);
                        }
                        let stroke = if i == self.current {
                            Stroke::new(3.0, Color32::from_rgb(0x2F, 0x74, 0xD6))
                        } else {
                            Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
                        };
                        ui.painter()
                            .rect_stroke(rect, 3.0, stroke, egui::StrokeKind::Outside);
                        if resp.clicked() {
                            select = Some(i);
                        }
                        resp.context_menu(|ui| {
                            if ui.button(tr("Présenter à partir d'ici")).clicked() {
                                self.start(ui.ctx(), i);
                            }
                            if ui.button(tr("Mode présentateur à partir d'ici")).clicked() {
                                self.start_presenter(ui.ctx(), i);
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

    /// La diapositive en grand, manipulable, puis le formulaire de ses textes.
    /// Renvoie le rectangle de la diapositive (pour les fichiers déposés).
    fn editor(&mut self, ui: &mut egui::Ui) -> Rect {
        let now = ui.input(|i| i.time);
        let values = self.deck.values();
        let ratio = self.deck.aspect.ratio();
        // La diapositive, aussi grande que possible.
        let avail = ui.available_size();
        let h = (avail.y * 0.56).min(avail.x / ratio);
        let w = h * ratio;
        let mut canvas = Rect::NOTHING;
        ui.vertical_centered(|ui| {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click_and_drag());
            canvas = rect;
            ui.painter().rect_filled(
                rect.translate(Vec2::new(0.0, 3.0)).expand(1.0),
                4.0,
                Color32::from_black_alpha(50),
            );
            render_slide(
                ui,
                rect,
                &self.deck,
                self.current,
                &values,
                &mut self.textures,
            );
            self.canvas_interact(ui, rect, &resp, now);
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
                    ui.label(tr("Titre"));
                    ui.add(egui::TextEdit::singleline(&mut s.title).desired_width(f32::INFINITY).hint_text(tr("Titre de la diapositive")));
                    ui.end_row();
                }
                if bx.subtitle.is_some() {
                    ui.label(tr("Sous-titre"));
                    ui.add(
                        egui::TextEdit::multiline(&mut s.subtitle)
                            .desired_width(f32::INFINITY)
                            .desired_rows(2),
                    );
                    ui.end_row();
                }
                if bx.body.is_some() {
                    ui.label(if bx.body2.is_some() { tr("Colonne 1") } else { tr("Contenu") });
                    ui.add(
                        egui::TextEdit::multiline(&mut s.body)
                            .desired_width(f32::INFINITY)
                            .desired_rows(5)
                            .hint_text(tr("- une puce\n- **gras**, *italique*, {{champ}}\n  - sous-puce")),
                    );
                    ui.end_row();
                }
                if bx.body2.is_some() {
                    ui.label(tr("Colonne 2"));
                    ui.add(egui::TextEdit::multiline(&mut s.body2).desired_width(f32::INFINITY).desired_rows(5));
                    ui.end_row();
                }
                ui.label(tr("Notes"));
                ui.add(
                    egui::TextEdit::multiline(&mut s.notes)
                        .desired_width(f32::INFINITY)
                        .desired_rows(3)
                        .hint_text(tr("Notes de l'orateur (touche N pendant le diaporama)")),
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
        canvas
    }

    /// Sélection, déplacement et redimensionnement des objets à la souris.
    fn canvas_interact(&mut self, ui: &egui::Ui, rect: Rect, resp: &egui::Response, now: f64) {
        let slide = self.current;
        let shift = ui.input(|i| i.modifiers.shift);
        let hover = resp.hover_pos();
        let objects = &self.deck.slides[slide].objects;
        let selected = self.selected.map(|(_, k)| k);

        // Curseur au survol.
        if self.drag.is_none()
            && let Some(p) = hover
        {
            if let Some(grip) = selected.and_then(|k| grip_at(rect, &objects[k], p)) {
                ui.ctx().set_cursor_icon(grip_cursor(grip));
            } else if hit_object(rect, objects, p).is_some() {
                ui.ctx().set_cursor_icon(CursorIcon::Move);
            }
        }

        if resp.drag_started()
            && let Some(p) = ui.input(|i| i.pointer.press_origin())
        {
            let grip = selected.and_then(|k| grip_at(rect, &objects[k], p).map(|g| (k, g)));
            let target = grip.or_else(|| hit_object(rect, objects, p).map(|k| (k, Grip::Move)));
            match target {
                Some((k, grip)) => {
                    let start = objects[k].clone();
                    self.snapshot(now, false);
                    self.selected = Some((slide, k));
                    self.drag = Some(Drag {
                        slide,
                        object: k,
                        grip,
                        origin: p,
                        start,
                    });
                }
                None => self.selected = None,
            }
        } else if resp.clicked()
            && let Some(p) = resp.interact_pointer_pos()
        {
            self.selected = hit_object(rect, objects, p).map(|k| (slide, k));
        }

        if let Some(d) = &self.drag {
            if resp.dragged()
                && let Some(p) = resp.interact_pointer_pos()
                && d.slide == slide
                && d.object < self.deck.slides[slide].objects.len()
            {
                let dx = (p.x - d.origin.x) / rect.width();
                let dy = (p.y - d.origin.y) / rect.height();
                let o = apply_drag(&d.start, d.grip, dx, dy, shift);
                ui.ctx().set_cursor_icon(grip_cursor(d.grip));
                self.deck.slides[slide].objects[d.object] = o;
            }
            if !resp.dragged() || resp.drag_stopped() {
                // Un simple clic sans déplacement ne laisse pas d'étape vide.
                if self.undo.last().is_some_and(|u| *u == self.deck) {
                    self.undo.pop();
                }
                self.drag = None;
            }
        }

        // Cadre de sélection et poignées.
        if let Some((s, k)) = self.selected
            && s == slide
            && let Some(o) = self.deck.slides[slide].objects.get(k)
        {
            let painter = ui.painter_at(rect.expand(8.0));
            let blue = Color32::from_rgb(0x2F, 0x74, 0xD6);
            if o.kind.is_line() {
                let (a, b) = line_ends(rect, o);
                painter.line_segment([a, b], Stroke::new(1.0, blue));
            } else {
                painter.rect_stroke(
                    frame_rect(rect, o.frame),
                    0.0,
                    Stroke::new(1.0, blue),
                    egui::StrokeKind::Outside,
                );
            }
            for p in handles(rect, o) {
                let r = Rect::from_center_size(p, Vec2::splat(8.0));
                painter.rect(
                    r,
                    1.0,
                    Color32::WHITE,
                    Stroke::new(1.0, blue),
                    egui::StrokeKind::Middle,
                );
            }
        }
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        let now = ui.input(|i| i.time);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.add_space(4.0);
                if let Some((s, k)) = self.selected {
                    self.object_inspector(ui, s, k, now);
                    ui.separator();
                }
                ui.label(RichText::new(tr("Diapositive")).strong());
                let i = self.current;
                let mut layout = self.deck.slides[i].layout;
                egui::ComboBox::from_label(tr("Disposition"))
                    .selected_text(labels::layout(layout))
                    .show_ui(ui, |ui| {
                        for l in Layout::ALL {
                            ui.selectable_value(&mut layout, l, labels::layout(l));
                        }
                    });
                if layout != self.deck.slides[i].layout {
                    self.snapshot(now, false);
                    self.deck.slides[i].layout = layout;
                }
                let mut trans = self.deck.slides[i].transition;
                egui::ComboBox::from_label(tr("Transition"))
                    .selected_text(labels::transition(trans))
                    .show_ui(ui, |ui| {
                        for t in Transition::ALL {
                            ui.selectable_value(&mut trans, t, labels::transition(t));
                        }
                    });
                if trans != self.deck.slides[i].transition {
                    self.snapshot(now, false);
                    self.deck.slides[i].transition = trans;
                }
                ui.label(tr("Fond"));
                ui.horizontal_wrapped(|ui| {
                    let theme_bg = self.deck.theme.palette().background;
                    for (name, c) in [
                        (tr("Thème"), None),
                        (tr("Blanc"), Some([0xFF, 0xFF, 0xFF])),
                        (tr("Nuit"), Some([0x14, 0x1B, 0x2D])),
                        (tr("Bleu"), Some([0x1F, 0x5F, 0xBF])),
                        (tr("Sable"), Some([0xF4, 0xEC, 0xDC])),
                        (tr("Vert"), Some([0x1E, 0x5B, 0x46])),
                        (tr("Corail"), Some([0xF2, 0x6B, 0x5B])),
                    ] {
                        let fill = rgb(c.unwrap_or(theme_bg));
                        let on = self.deck.slides[i].background == c;
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                        ui.painter().rect_filled(r, 4.0, fill);
                        ui.painter().rect_stroke(
                            r,
                            4.0,
                            Stroke::new(
                                if on { 2.5 } else { 1.0 },
                                if on {
                                    Color32::from_rgb(0x2F, 0x74, 0xD6)
                                } else {
                                    Color32::GRAY
                                },
                            ),
                            egui::StrokeKind::Outside,
                        );
                        if resp.on_hover_text(name).clicked() {
                            self.snapshot(now, false);
                            self.deck.slides[i].background = c;
                        }
                    }
                });
                let n = self.deck.slides[i].objects.len();
                if n > 0 {
                    ui.label(
                        RichText::new(trf!(
                            "{n} objet(s) : cliquez sur la diapositive pour en sélectionner un.",
                            n
                        ))
                        .small()
                        .weak(),
                    );
                }

                ui.separator();
                ui.label(RichText::new(tr("Présentation")).strong());
                let mut aspect = self.deck.aspect;
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut aspect, Aspect::Large, "16:9");
                    ui.selectable_value(&mut aspect, Aspect::Standard, "4:3");
                });
                if aspect != self.deck.aspect {
                    self.snapshot(now, false);
                    self.deck.aspect = aspect;
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.deck.footer)
                        .hint_text(tr("Pied de page (ex. {{entreprise}})")),
                );
                ui.checkbox(&mut self.deck.slide_numbers, tr("Numéros de diapositive"));

                ui.separator();
                ui.label(RichText::new(tr("Champs")).strong());
                ui.label(
                    RichText::new(tr(
                        "Écrivez {{nom}} dans une diapositive : la valeur s'affiche partout.",
                    ))
                    .small()
                    .weak(),
                );
                let vars = self.deck.variables();
                if vars.is_empty() {
                    ui.label(RichText::new(tr("Aucun champ.")).weak());
                }
                for v in vars {
                    ui.label(RichText::new(&v).monospace());
                    let value = self.deck.values.entry(v.clone()).or_default();
                    ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
                }
                ui.separator();
                ui.label(RichText::new(tr("Mise en forme du contenu")).strong());
                for (k, d) in [
                    (tr("- puce"), tr("liste à puces (2 espaces = sous-puce)")),
                    (tr("1. élément"), tr("liste numérotée")),
                    (tr("**gras** *italique*"), tr("caractères")),
                    (tr("> citation"), tr("citation")),
                    ("| a | b |", tr("tableau")),
                    (tr("{{champ}}"), tr("valeur partagée")),
                    ("$x^2$", tr("formule")),
                ] {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(k).monospace().small());
                        ui.label(RichText::new(d).small().weak());
                    });
                }
            });
    }

    /// Réglages de l'objet sélectionné : position, taille, couleurs, texte, ordre.
    fn object_inspector(&mut self, ui: &mut egui::Ui, s: usize, k: usize, now: f64) {
        let Some(before) = self.deck.slides[s].objects.get(k).cloned() else {
            return;
        };
        let ratio = self.deck.media.get(&before.media).map(|m| m.ratio());
        let slide_ratio = self.deck.aspect.ratio();
        let mut o = before.clone();
        ui.label(RichText::new(trf!("Objet : {kind}", kind = labels::object(o.kind))).strong());
        egui::Grid::new("object-geometry")
            .num_columns(4)
            .spacing([6.0, 4.0])
            .show(ui, |ui| {
                let pc = |ui: &mut egui::Ui, label: &str, v: &mut f32| {
                    ui.label(label);
                    let mut x = *v * 100.0;
                    if ui
                        .add(
                            egui::DragValue::new(&mut x)
                                .speed(0.2)
                                .range(-100.0..=200.0)
                                .max_decimals(1)
                                .suffix(" %"),
                        )
                        .changed()
                    {
                        *v = x / 100.0;
                    }
                };
                pc(ui, "X", &mut o.frame.x);
                pc(ui, "Y", &mut o.frame.y);
                ui.end_row();
                pc(ui, tr("Largeur"), &mut o.frame.w);
                pc(ui, tr("Hauteur"), &mut o.frame.h);
                ui.end_row();
            });
        o.frame.w = o.frame.w.max(0.0);
        o.frame.h = o.frame.h.max(0.0);
        if let Some(r) = ratio
            && ui
                .button(tr("Proportions d'origine"))
                .on_hover_text(tr(
                    "Rétablit les proportions de l'image (largeur conservée)",
                ))
                .clicked()
        {
            o.frame.h = o.frame.w * slide_ratio / r;
        }
        if o.kind != ObjectKind::Image {
            if o.kind.is_closed() {
                ui.horizontal(|ui| {
                    let mut on = o.fill.is_some();
                    ui.checkbox(&mut on, tr("Fond"));
                    let mut c = o.fill.unwrap_or([0x2F, 0x74, 0xD6]);
                    if on {
                        egui::color_picker::color_edit_button_srgb(ui, &mut c);
                    }
                    o.fill = on.then_some(c);
                });
            }
            ui.horizontal(|ui| {
                let mut on = o.stroke.is_some();
                if o.kind.is_line() {
                    on = true;
                    ui.label(tr("Trait"));
                } else {
                    ui.checkbox(&mut on, tr("Trait"));
                }
                let mut c = o.stroke.unwrap_or([0x20, 0x20, 0x20]);
                if on {
                    egui::color_picker::color_edit_button_srgb(ui, &mut c);
                    ui.add(
                        egui::DragValue::new(&mut o.stroke_width)
                            .speed(0.1)
                            .range(0.0..=40.0)
                            .max_decimals(1)
                            .suffix(" pt"),
                    );
                }
                o.stroke = on.then_some(c);
            });
            if o.kind.is_line() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut o.flip_h, tr("Inverser ↔"));
                    ui.checkbox(&mut o.flip_v, tr("Inverser ↕"));
                });
            } else {
                ui.label(tr("Texte"));
                ui.add(
                    egui::TextEdit::multiline(&mut o.text)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY)
                        .hint_text(tr("Texte centré (**gras**, {{champ}}…)")),
                );
                ui.horizontal(|ui| {
                    let mut size = o.text_size * 100.0;
                    ui.label(tr("Taille"));
                    if ui
                        .add(
                            egui::DragValue::new(&mut size)
                                .speed(0.05)
                                .range(0.5..=40.0)
                                .max_decimals(1),
                        )
                        .changed()
                    {
                        o.text_size = size / 100.0;
                    }
                    let mut auto = o.text_color.is_none();
                    ui.checkbox(&mut auto, tr("Couleur auto"));
                    let pal = self.deck.slide_palette(s);
                    let mut c = o.text_color.unwrap_or_else(|| o.text_ink(&pal));
                    if !auto {
                        egui::color_picker::color_edit_button_srgb(ui, &mut c);
                    }
                    o.text_color = (!auto).then_some(c);
                });
            }
        }
        if o != before {
            self.snapshot(now, true);
            self.deck.slides[s].objects[k] = o;
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Premier plan")).clicked() {
                self.snapshot(now, false);
                let k = self.deck.slides[s].bring_to_front(k);
                self.selected = Some((s, k));
            }
            if ui.button(tr("Arrière-plan")).clicked() {
                self.snapshot(now, false);
                let k = self.deck.slides[s].send_to_back(k);
                self.selected = Some((s, k));
            }
            if ui
                .button(tr("Dupliquer"))
                .on_hover_text(tr("Ctrl+D"))
                .clicked()
            {
                self.snapshot(now, false);
                self.selected = self.deck.slides[s].duplicate_object(k).map(|k| (s, k));
            }
            if ui
                .button(tr("🗑 Supprimer"))
                .on_hover_text(tr("Suppr"))
                .clicked()
            {
                self.snapshot(now, false);
                self.deck.slides[s].remove_object(k);
                self.selected = None;
            }
        });
        ui.label(
            RichText::new(tr("Glisser pour déplacer, poignées pour redimensionner (Maj : proportions), flèches pour ajuster."))
                .small()
                .weak(),
        );
    }

    // ---------------- Mode présentation ----------------

    /// Commandes du diaporama lues dans l'entrée d'une fenêtre.
    fn read_nav(ctx: &egui::Context, click_advances: bool) -> Vec<Nav> {
        ctx.input(|inp| {
            let mut v = Vec::new();
            if inp.key_pressed(Key::ArrowRight)
                || inp.key_pressed(Key::ArrowDown)
                || inp.key_pressed(Key::Space)
                || inp.key_pressed(Key::PageDown)
                || inp.key_pressed(Key::Enter)
                || (click_advances && inp.pointer.primary_clicked())
            {
                v.push(Nav::Next);
            }
            if inp.key_pressed(Key::ArrowLeft)
                || inp.key_pressed(Key::ArrowUp)
                || inp.key_pressed(Key::PageUp)
                || inp.key_pressed(Key::Backspace)
            {
                v.push(Nav::Prev);
            }
            if inp.key_pressed(Key::Home) {
                v.push(Nav::First);
            }
            if inp.key_pressed(Key::End) {
                v.push(Nav::Last);
            }
            if inp.key_pressed(Key::B) || inp.key_pressed(Key::Period) {
                v.push(Nav::Black);
            }
            if inp.key_pressed(Key::Escape) {
                v.push(Nav::Stop);
            }
            v
        })
    }

    /// Applique une commande ; renvoie `false` si le diaporama est terminé.
    fn apply_nav(&mut self, ctx: &egui::Context, nav: Nav, end_after_last: bool) -> bool {
        let Some((i, _)) = self.presenting else {
            return false;
        };
        let n = self.deck.slides.len();
        let now = ctx.input(|i| i.time);
        let target = match nav {
            Nav::Stop => {
                self.stop(ctx);
                return false;
            }
            Nav::Black => {
                self.black = !self.black;
                return true;
            }
            Nav::Next if i + 1 >= n => {
                if end_after_last {
                    self.stop(ctx);
                    return false;
                }
                i
            }
            Nav::Next => i + 1,
            Nav::Prev => i.saturating_sub(1),
            Nav::First => 0,
            Nav::Last => n - 1,
        };
        if target != i {
            self.black = false;
            self.presenting = Some((target, now));
        }
        true
    }

    fn present(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.key_pressed(Key::N)) {
            self.show_notes = !self.show_notes;
        }
        for nav in Self::read_nav(&ctx, true) {
            if !self.apply_nav(&ctx, nav, true) {
                return;
            }
        }
        let Some((i, since)) = self.presenting else {
            return;
        };
        let n = self.deck.slides.len();
        let screen = ui.max_rect();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::BLACK))
            .show(ui, |ui| {
                show_slide(
                    ui,
                    screen,
                    &self.deck,
                    i,
                    since,
                    self.black,
                    &mut self.textures,
                );
                if self.show_notes {
                    let notes = self.deck.slides[i].notes.clone();
                    let text = if notes.trim().is_empty() {
                        tr("(pas de notes)").to_string()
                    } else {
                        notes
                    };
                    let galley = ui.painter().layout(
                        format!("{}/{} — {}", i + 1, n, text),
                        FontId::proportional(18.0),
                        Color32::WHITE,
                        screen.width() - 80.0,
                    );
                    let r = Rect::from_min_size(
                        Pos2::new(
                            screen.left() + 30.0,
                            screen.bottom() - galley.size().y - 40.0,
                        ),
                        galley.size() + Vec2::splat(20.0),
                    );
                    ui.painter()
                        .rect_filled(r, 8.0, Color32::from_black_alpha(200));
                    ui.painter()
                        .galley(r.min + Vec2::splat(10.0), galley, Color32::WHITE);
                }
            });
    }

    // ---------------- Mode présentateur ----------------

    fn presenter_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        // Fermer la fenêtre principale termine la présentation (sans quitter).
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.stop(&ctx);
            return;
        }
        let mut navs = if ctx.egui_wants_keyboard_input() {
            Vec::new()
        } else {
            Self::read_nav(&ctx, false)
        };

        // Fenêtre du public.
        let builder = egui::ViewportBuilder::default()
            .with_title(tr("Varium — Diaporama (public)"))
            .with_inner_size([960.0, 540.0])
            .with_min_inner_size([320.0, 180.0]);
        let audience = ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of(AUDIENCE_ID),
            builder,
            |ui, _class| self.audience_ui(ui),
        );
        navs.extend(audience);

        for nav in navs {
            if !self.apply_nav(&ctx, nav, false) {
                return;
            }
        }
        let Some((i, _)) = self.presenting else {
            return;
        };
        let n = self.deck.slides.len();
        let values = self.deck.values();
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        egui::Panel::top("presenter-bar").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} / {}", i + 1, n))
                        .size(22.0)
                        .strong(),
                );
                ui.separator();
                let (elapsed, paused) = match &self.presenter {
                    Some(p) => (p.elapsed(now), p.paused.is_some()),
                    None => (0.0, false),
                };
                let t = elapsed as u64;
                ui.label(
                    RichText::new(format!(
                        "⏱ {:02}:{:02}:{:02}",
                        t / 3600,
                        t / 60 % 60,
                        t % 60
                    ))
                    .size(22.0)
                    .monospace(),
                );
                if ui
                    .button(if paused {
                        tr("▶ Reprendre")
                    } else {
                        tr("⏸ Pause")
                    })
                    .clicked()
                    && let Some(p) = &mut self.presenter
                {
                    match p.paused.take() {
                        Some(at) => p.paused_total += now - at,
                        None => p.paused = Some(now),
                    }
                }
                if ui.button(tr("↺ Remettre à zéro")).clicked()
                    && let Some(p) = &mut self.presenter
                {
                    p.started = now;
                    p.paused_total = 0.0;
                    p.paused = p.paused.map(|_| now);
                }
                ui.separator();
                let clock = jiff::Zoned::now().strftime("%H:%M").to_string();
                ui.label(RichText::new(format!("🕒 {clock}")).size(22.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new(tr("Terminer (Échap)")).strong())
                                .fill(Color32::from_rgb(0xB0, 0x30, 0x2A)),
                        )
                        .clicked()
                    {
                        self.stop(&ctx);
                    }
                    let black = if self.black {
                        tr("Rallumer l'écran (B)")
                    } else {
                        tr("Écran noir (B)")
                    };
                    if ui.button(black).clicked() {
                        self.black = !self.black;
                    }
                    if ui.button(tr("Suivante ▶")).clicked() {
                        self.apply_nav(&ctx, Nav::Next, false);
                    }
                    if ui.button(tr("◀ Précédente")).clicked() {
                        self.apply_nav(&ctx, Nav::Prev, false);
                    }
                });
            });
            ui.add_space(4.0);
        });
        if self.presenting.is_none() {
            return;
        }
        let ratio = self.deck.aspect.ratio();
        egui::Panel::right("presenter-side")
            .resizable(true)
            .default_size(ui.available_width() * 0.36)
            .show(ui, |ui| {
                ui.label(RichText::new(tr("Diapositive suivante")).strong());
                let w = ui.available_width();
                let (r, _) = ui.allocate_exact_size(Vec2::new(w, w / ratio), Sense::hover());
                if i + 1 < n {
                    render_slide(ui, r, &self.deck, i + 1, &values, &mut self.textures);
                } else {
                    ui.painter().rect_filled(r, 2.0, Color32::from_gray(30));
                    ui.painter().text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        tr("Fin du diaporama"),
                        FontId::proportional(16.0),
                        Color32::GRAY,
                    );
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("Notes")).strong());
                    if let Some(p) = &mut self.presenter {
                        if ui
                            .small_button("A−")
                            .on_hover_text(tr("Texte plus petit"))
                            .clicked()
                        {
                            p.notes_size = (p.notes_size - 2.0).max(10.0);
                        }
                        if ui
                            .small_button("A+")
                            .on_hover_text(tr("Texte plus grand"))
                            .clicked()
                        {
                            p.notes_size = (p.notes_size + 2.0).min(64.0);
                        }
                    }
                });
                let size = self.presenter.as_ref().map_or(20.0, |p| p.notes_size);
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        notes_ui(ui, &self.deck.slides[i].notes, &values, size)
                    });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_gray(18)))
            .show(ui, |ui| {
                let area = ui.max_rect().shrink(16.0);
                let (w, h) = if area.width() / area.height() > ratio {
                    (area.height() * ratio, area.height())
                } else {
                    (area.width(), area.width() / ratio)
                };
                let r = Rect::from_center_size(area.center(), Vec2::new(w, h));
                render_slide(ui, r, &self.deck, i, &values, &mut self.textures);
                if self.black {
                    ui.painter()
                        .rect_filled(r, 2.0, Color32::from_black_alpha(190));
                    ui.painter().text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        tr("Écran noir pour le public"),
                        FontId::proportional(20.0),
                        Color32::WHITE,
                    );
                }
            });
    }

    /// La fenêtre du public : la diapositive seule, avec ses transitions.
    fn audience_ui(&mut self, ui: &mut egui::Ui) -> Vec<Nav> {
        let ctx = ui.ctx().clone();
        let mut navs = Self::read_nav(&ctx, true);
        if ctx.input(|i| i.viewport().close_requested()) {
            navs.push(Nav::Stop);
        }
        let toggle = ctx.input(|i| {
            i.key_pressed(Key::F)
                || i.key_pressed(Key::F11)
                || i.pointer
                    .button_double_clicked(egui::PointerButton::Primary)
        });
        if toggle {
            let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fs));
            // Le double-clic ne doit pas faire avancer deux fois.
            navs.retain(|n| *n != Nav::Next);
        }
        let Some((i, since)) = self.presenting else {
            return navs;
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::BLACK))
            .show(ui, |ui| {
                let screen = ui.max_rect();
                show_slide(
                    ui,
                    screen,
                    &self.deck,
                    i,
                    since,
                    self.black,
                    &mut self.textures,
                );
            });
        navs
    }

    fn toast_ui(&mut self, ctx: &egui::Context) {
        let Some((msg, at, error)) = self.toast.clone() else {
            return;
        };
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
                    let c = if error {
                        Color32::from_rgb(0xD0, 0x30, 0x2A)
                    } else {
                        ui.visuals().text_color()
                    };
                    ui.label(RichText::new(msg).color(c));
                });
            });
    }
}

fn shape_label(kind: ObjectKind) -> String {
    let icon = match kind {
        ObjectKind::Rect => "▭",
        ObjectKind::RoundRect => "▢",
        ObjectKind::Ellipse => "◯",
        ObjectKind::Line => "╱",
        ObjectKind::Arrow => "➚",
        ObjectKind::Image => "🖼",
    };
    format!("{icon} {}", labels::object(kind))
}

/// Les notes de l'orateur, mises en forme, à la taille choisie.
fn notes_ui(ui: &mut egui::Ui, notes: &str, values: &Values, size: f32) {
    let paras = varium_core::document(notes, values);
    if paras.is_empty() {
        ui.label(
            RichText::new(tr("Aucune note pour cette diapositive."))
                .italics()
                .weak()
                .size(size),
        );
        return;
    }
    let color = ui.visuals().text_color();
    let ink = Ink {
        font: DocFont::Sans,
        color,
        accent: color,
        size,
    };
    for p in &paras {
        let (runs, prefix, bold) = match p {
            Para::Item {
                number,
                depth,
                runs,
            } => (
                runs.clone(),
                format!(
                    "{}{} ",
                    "    ".repeat(*depth as usize),
                    match number {
                        Some(n) => format!("{n}."),
                        None => "•".into(),
                    }
                ),
                false,
            ),
            Para::Heading { runs, .. } => (runs.clone(), String::new(), true),
            Para::Text { runs, .. } | Para::Quote { runs } => (runs.clone(), String::new(), false),
            other => (
                vec![Run::Text {
                    text: other.plain(),
                    style: Default::default(),
                }],
                String::new(),
                false,
            ),
        };
        let mut all = Vec::with_capacity(runs.len() + 1);
        if !prefix.is_empty() {
            all.push(Run::Text {
                text: prefix,
                style: Default::default(),
            });
        }
        all.extend(runs);
        let mut job = runs_job(&all, &ink, bold, false, size, color);
        job.wrap.max_width = ui.available_width();
        ui.label(job);
        ui.add_space(size * 0.3);
    }
}

/// La diapositive `i` plein cadre dans `screen`, avec la transition qui la fait
/// apparaître depuis `since`, ou un écran noir.
fn show_slide(
    ui: &egui::Ui,
    screen: Rect,
    deck: &Deck,
    i: usize,
    since: f64,
    black: bool,
    textures: &mut Textures,
) {
    ui.painter().rect_filled(screen, 0.0, Color32::BLACK);
    if black {
        return;
    }
    let now = ui.input(|inp| inp.time);
    let values = deck.values();
    let ratio = deck.aspect.ratio();
    let (w, h) = if screen.width() / screen.height() > ratio {
        (screen.height() * ratio, screen.height())
    } else {
        (screen.width(), screen.width() / ratio)
    };
    let mut rect = Rect::from_center_size(screen.center(), Vec2::new(w, h));
    let t = ((now - since) / 0.45).clamp(0.0, 1.0) as f32;
    let ease = 1.0 - (1.0 - t).powi(3);
    let tr = deck.slides.get(i).map(|s| s.transition).unwrap_or_default();
    match tr {
        Transition::Glissement => rect = rect.translate(Vec2::new((1.0 - ease) * w * 0.25, 0.0)),
        Transition::Zoom => {
            rect = Rect::from_center_size(rect.center(), rect.size() * (0.85 + 0.15 * ease))
        }
        _ => {}
    }
    render_slide(ui, rect, deck, i, &values, textures);
    if tr != Transition::Aucune && t < 1.0 {
        ui.painter().rect_filled(
            screen,
            0.0,
            Color32::from_black_alpha(((1.0 - ease) * 255.0) as u8),
        );
        ui.ctx().request_repaint();
    }
}

// ---------------------------------------------------------------------------------
// Manipulation des objets
// ---------------------------------------------------------------------------------

/// Extrémités d'une ligne à l'écran.
fn line_ends(slide: Rect, o: &Object) -> (Pos2, Pos2) {
    let ((x1, y1), (x2, y2)) = o.line_points();
    let p = |x: f32, y: f32| {
        Pos2::new(
            slide.left() + x * slide.width(),
            slide.top() + y * slide.height(),
        )
    };
    (p(x1, y1), p(x2, y2))
}

/// Les poignées d'un objet : 8 autour du cadre, ou les 2 extrémités d'une ligne.
fn handles(slide: Rect, o: &Object) -> Vec<Pos2> {
    if o.kind.is_line() {
        let (a, b) = line_ends(slide, o);
        return vec![a, b];
    }
    let r = frame_rect(slide, o.frame);
    vec![
        r.left_top(),
        r.center_top(),
        r.right_top(),
        r.right_center(),
        r.right_bottom(),
        r.center_bottom(),
        r.left_bottom(),
        r.left_center(),
    ]
}

/// La poignée de l'objet sous le pointeur.
fn grip_at(slide: Rect, o: &Object, p: Pos2) -> Option<Grip> {
    let hs = handles(slide, o);
    let k = hs.iter().position(|h| h.distance(p) <= 7.0)?;
    Some(if o.kind.is_line() {
        Grip::End(k == 1)
    } else {
        Grip::Handle(k)
    })
}

fn grip_cursor(g: Grip) -> CursorIcon {
    match g {
        Grip::Move => CursorIcon::Grabbing,
        Grip::End(_) => CursorIcon::Crosshair,
        Grip::Handle(0 | 4) => CursorIcon::ResizeNwSe,
        Grip::Handle(2 | 6) => CursorIcon::ResizeNeSw,
        Grip::Handle(1 | 5) => CursorIcon::ResizeVertical,
        Grip::Handle(_) => CursorIcon::ResizeHorizontal,
    }
}

/// Distance d'un point à un segment.
fn segment_distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let l2 = ab.length_sq();
    if l2 < 1e-6 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// L'objet le plus en avant sous le pointeur.
fn hit_object(slide: Rect, objects: &[Object], p: Pos2) -> Option<usize> {
    objects.iter().enumerate().rev().find_map(|(k, o)| {
        let hit = if o.kind.is_line() {
            let (a, b) = line_ends(slide, o);
            let sw = o.stroke_width * slide.width() / 720.0;
            segment_distance(p, a, b) <= (sw / 2.0 + 5.0)
        } else {
            frame_rect(slide, o.frame).expand(2.0).contains(p)
        };
        hit.then_some(k)
    })
}

/// L'objet après un glissement de `(dx, dy)` (fractions de la diapositive) depuis
/// son état `start`. Maj conserve les proportions.
fn apply_drag(start: &Object, grip: Grip, dx: f32, dy: f32, keep_ratio: bool) -> Object {
    let mut o = start.clone();
    let f = start.frame;
    const MIN: f32 = 0.01;
    match grip {
        Grip::Move => {
            o.frame.x = f.x + dx;
            o.frame.y = f.y + dy;
        }
        Grip::End(arrivee) => {
            let ((x1, y1), (x2, y2)) = start.line_points();
            let ((x1, y1), (x2, y2)) = if arrivee {
                ((x1, y1), (x2 + dx, y2 + dy))
            } else {
                ((x1 + dx, y1 + dy), (x2, y2))
            };
            o.frame = Frame::new(x1.min(x2), y1.min(y2), (x2 - x1).abs(), (y2 - y1).abs());
            o.flip_h = x1 > x2;
            o.flip_v = y1 > y2;
        }
        Grip::Handle(h) => {
            let (mut l, mut t, mut r, mut b) = (f.x, f.y, f.right(), f.bottom());
            let gauche = matches!(h, 0 | 6 | 7);
            let droite = matches!(h, 2..=4);
            let haut = matches!(h, 0..=2);
            let bas = matches!(h, 4..=6);
            if gauche {
                l = (l + dx).min(r - MIN);
            }
            if droite {
                r = (r + dx).max(l + MIN);
            }
            if haut {
                t = (t + dy).min(b - MIN);
            }
            if bas {
                b = (b + dy).max(t + MIN);
            }
            let (mut w, mut hh) = (r - l, b - t);
            if keep_ratio && f.w > 0.0 && f.h > 0.0 {
                let ratio = f.w / f.h;
                if (gauche || droite) && (haut || bas) {
                    let s = (w / f.w).max(hh / f.h);
                    w = f.w * s;
                    hh = f.h * s;
                } else if gauche || droite {
                    hh = w / ratio;
                } else {
                    w = hh * ratio;
                }
                // Le côté opposé à la poignée reste en place (le centre pour les
                // poignées des côtés).
                l = if gauche {
                    f.right() - w
                } else if droite {
                    f.x
                } else {
                    f.x + (f.w - w) / 2.0
                };
                t = if haut {
                    f.bottom() - hh
                } else if bas {
                    f.y
                } else {
                    f.y + (f.h - hh) / 2.0
                };
            }
            o.frame = Frame::new(l, t, w, hh);
        }
    }
    o
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

fn runs_job(
    runs: &[Run],
    ink: &Ink,
    bold: bool,
    italic: bool,
    size: f32,
    color: Color32,
) -> LayoutJob {
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
        job.append(
            " ",
            0.0,
            egui::TextFormat {
                font_id: FontId::proportional(size),
                ..Default::default()
            },
        );
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
                (
                    runs_job(runs, ink, true, false, s * k, ink.color),
                    0.0,
                    None,
                )
            }
            Para::Item {
                number,
                depth,
                runs,
            } => {
                let label = match number {
                    Some(n) => format!("{n}."),
                    None => ["•", "–", "◦"][*depth as usize % 3].to_string(),
                };
                let ind = s * 1.3 * (*depth as f32 + 1.0);
                (
                    runs_job(
                        runs,
                        ink,
                        false,
                        false,
                        s * if *depth > 0 { 0.9 } else { 1.0 },
                        ink.color,
                    ),
                    ind,
                    Some(label),
                )
            }
            Para::Quote { runs } => (
                runs_job(runs, ink, false, true, s * 1.1, ink.color),
                s,
                None,
            ),
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
                        let cell = Rect::from_min_size(
                            Pos2::new(area.left() + cw * c as f32, y),
                            Vec2::new(cw, rh),
                        );
                        let head = ri == 0 && t.header;
                        if head {
                            painter.rect_filled(cell, 0.0, ink.accent.gamma_multiply(0.25));
                        }
                        painter.rect_stroke(
                            cell,
                            0.0,
                            Stroke::new(1.0, ink.color.gamma_multiply(0.4)),
                            egui::StrokeKind::Inside,
                        );
                        let empty = Vec::new();
                        let mut job = runs_job(
                            row.get(c).unwrap_or(&empty),
                            ink,
                            head,
                            false,
                            s * 0.8,
                            ink.color,
                        );
                        job.wrap.max_width = cw - 8.0;
                        let g = ui.ctx().fonts_mut(|f| f.layout_job(job));
                        painter.galley(
                            Pos2::new(cell.left() + 4.0, cell.center().y - g.size().y / 2.0),
                            g,
                            ink.color,
                        );
                    }
                    y += rh;
                }
                y += s * 0.3;
                continue;
            }
            Para::Rule => {
                painter.line_segment(
                    [
                        Pos2::new(area.left(), y + s * 0.3),
                        Pos2::new(area.right(), y + s * 0.3),
                    ],
                    Stroke::new(1.5, ink.accent),
                );
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
        let x = if centered {
            area.center().x
        } else {
            area.left() + indent
        };
        if let Some(label) = bullet {
            painter.text(
                Pos2::new(area.left() + indent - s * 1.1, y),
                Align2::LEFT_TOP,
                label,
                FontId::new(s, crate::fonts::family(ink.font, true, false)),
                ink.accent,
            );
        }
        if matches!(p, Para::Quote { .. }) {
            painter.line_segment(
                [
                    Pos2::new(area.left() + s * 0.3, y),
                    Pos2::new(area.left() + s * 0.3, y + g.size().y),
                ],
                Stroke::new(s * 0.12, ink.accent),
            );
        }
        let h = g.size().y;
        painter.galley(Pos2::new(x, y), g, ink.color);
        y += h + s * 0.35;
    }
}

/// Dessine un objet (image ou forme) de la diapositive.
fn draw_object(
    ui: &egui::Ui,
    slide: Rect,
    deck: &Deck,
    o: &Object,
    pal: &varium_slides::Palette,
    values: &Values,
    textures: &mut Textures,
) {
    let painter = ui.painter_at(slide);
    let r = frame_rect(slide, o.frame);
    // Épaisseurs en points d'une diapositive de 720 pt de large.
    let k = slide.width() / 720.0;
    let stroke = match o.stroke {
        Some(c) if o.stroke_width > 0.0 => Stroke::new((o.stroke_width * k).max(0.5), rgb(c)),
        _ => Stroke::NONE,
    };
    let fill = o.fill.map(rgb).unwrap_or(Color32::TRANSPARENT);
    match o.kind {
        ObjectKind::Image => match textures.get(ui.ctx(), deck, &o.media) {
            Some(tex) => {
                painter.image(
                    tex,
                    r,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => {
                painter.rect(
                    r,
                    0.0,
                    Color32::from_gray(200),
                    Stroke::new(1.0, Color32::GRAY),
                    egui::StrokeKind::Inside,
                );
                painter.text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    tr("Image absente"),
                    FontId::proportional((r.height() * 0.15).clamp(6.0, 16.0)),
                    Color32::DARK_GRAY,
                );
            }
        },
        ObjectKind::Rect | ObjectKind::RoundRect => {
            let radius = if o.kind == ObjectKind::RoundRect {
                (r.width().min(r.height()) * Object::CORNER).min(255.0)
            } else {
                0.0
            };
            painter.rect(r, radius, fill, stroke, egui::StrokeKind::Middle);
        }
        ObjectKind::Ellipse => {
            painter.add(egui::Shape::Ellipse(egui::epaint::EllipseShape {
                center: r.center(),
                radius: r.size() / 2.0,
                fill,
                stroke,
                angle: 0.0,
            }));
        }
        ObjectKind::Line | ObjectKind::Arrow => {
            let c = rgb(o.stroke.unwrap_or(pal.text));
            let w = (o.stroke_width * k).max(1.0);
            let (a, b) = line_ends(slide, o);
            if o.kind == ObjectKind::Arrow {
                let (fin, tri) = arrow_head((a.x, a.y), (b.x, b.y), w);
                painter.line_segment([a, Pos2::new(fin.0, fin.1)], Stroke::new(w, c));
                painter.add(egui::Shape::convex_polygon(
                    tri.iter().map(|p| Pos2::new(p.0, p.1)).collect(),
                    c,
                    Stroke::NONE,
                ));
            } else {
                painter.line_segment([a, b], Stroke::new(w, c));
            }
        }
    }
    if o.kind.is_closed() && !o.text.trim().is_empty() {
        let lines = varium_slides::text_lines(&o.text, values);
        let color = rgb(o.text_ink(pal));
        let size = o.text_size * slide.height();
        let ink = Ink {
            font: DocFont::Sans,
            color,
            accent: color,
            size,
        };
        let inner = r.shrink2(Vec2::new(slide.width() * 0.012, slide.width() * 0.012));
        let galleys: Vec<_> = lines
            .iter()
            .map(|l| {
                let mut job = runs_job(l, &ink, false, false, size, color);
                job.wrap.max_width = inner.width().max(1.0);
                job.halign = egui::Align::Center;
                ui.ctx().fonts_mut(|f| f.layout_job(job))
            })
            .collect();
        let total: f32 = galleys.iter().map(|g| g.size().y).sum();
        let mut y = inner.center().y - total / 2.0;
        let clip = painter.with_clip_rect(r.intersect(slide));
        for g in galleys {
            let h = g.size().y;
            clip.galley(Pos2::new(inner.center().x, y), g, color);
            y += h;
        }
    }
}

/// Dessine la diapositive `i` du diaporama dans `rect`.
pub fn render_slide(
    ui: &egui::Ui,
    rect: Rect,
    deck: &Deck,
    i: usize,
    values: &Values,
    textures: &mut Textures,
) {
    let Some(slide) = deck.slides.get(i) else {
        return;
    };
    // La palette de la diapositive : sur un fond personnalisé, le moteur ajuste les
    // couleurs du texte pour qu'il reste lisible.
    let pal = deck.slide_palette(i);
    let bg = rgb(pal.background);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, bg);
    let (text, title) = (rgb(pal.text), rgb(pal.title));
    let accent = rgb(pal.accent);
    let h = rect.height();
    let layout = slide.layout;
    let bx = varium_slides::boxes(layout);
    let title_font = if pal.serif_titles {
        DocFont::Serif
    } else {
        DocFont::Sans
    };
    let centered = matches!(layout, Layout::Title | Layout::Section | Layout::Quote);

    if let Some(f) = bx.title {
        let r = frame_rect(rect, f);
        let ink = Ink {
            font: title_font,
            color: title,
            accent,
            size: varium_slides::title_size(layout) * h,
        };
        let runs = varium_core::doc::inline(&slide.title, values);
        let mut job = runs_job(&runs, &ink, true, false, ink.size, title);
        job.wrap.max_width = r.width();
        if centered {
            job.halign = egui::Align::Center;
        }
        let g = ui.ctx().fonts_mut(|fo| fo.layout_job(job));
        let x = if centered { r.center().x } else { r.left() };
        let y = if matches!(layout, Layout::Title | Layout::Section) {
            r.bottom() - g.size().y
        } else {
            r.top()
        };
        painter
            .with_clip_rect(rect)
            .galley(Pos2::new(x, y.max(r.top())), g, title);
        if layout == Layout::Section {
            painter.line_segment(
                [
                    Pos2::new(r.center().x - r.width() * 0.08, r.bottom() + h * 0.03),
                    Pos2::new(r.center().x + r.width() * 0.08, r.bottom() + h * 0.03),
                ],
                Stroke::new(h * 0.008, accent),
            );
        }
    }
    if let Some(f) = bx.subtitle {
        let r = frame_rect(rect, f);
        let ink = Ink {
            font: DocFont::Sans,
            color: text,
            accent,
            size: varium_slides::subtitle_size(layout) * h,
        };
        let runs = varium_core::doc::inline(&slide.subtitle, values);
        let mut job = runs_job(
            &runs,
            &ink,
            false,
            false,
            ink.size,
            text.gamma_multiply(0.85),
        );
        job.wrap.max_width = r.width();
        if centered {
            job.halign = egui::Align::Center;
        }
        let g = ui.ctx().fonts_mut(|fo| fo.layout_job(job));
        let x = if centered { r.center().x } else { r.left() };
        painter.galley(Pos2::new(x, r.top()), g, text);
    }
    let body_ink = Ink {
        font: if layout == Layout::Quote {
            DocFont::Serif
        } else {
            DocFont::Sans
        },
        color: text,
        accent,
        size: varium_slides::body_size(layout) * h,
    };
    for (f, src) in [(bx.body, &slide.body), (bx.body2, &slide.body2)] {
        if let Some(f) = f {
            let paras = varium_core::document(src, values);
            draw_body(
                ui,
                frame_rect(rect, f),
                &paras,
                &body_ink,
                layout == Layout::Quote,
            );
        }
    }
    // Images et formes, au-dessus du texte, dans leur ordre.
    for o in &slide.objects {
        draw_object(ui, rect, deck, o, &pal, values, textures);
    }
    // Pied de page et numéro, aux places que le moteur leur donne dans les exports.
    if !bx.footer {
        return;
    }
    let small = FontId::new(
        h * varium_slides::FOOTER_SIZE,
        crate::fonts::family(DocFont::Sans, false, false),
    );
    let muted = rgb(pal.muted);
    if !deck.footer.trim().is_empty() {
        let f = frame_rect(rect, varium_slides::footer_frame());
        let foot = varium_core::doc::plain(&varium_core::doc::inline(&deck.footer, values));
        painter.text(
            f.left_center(),
            Align2::LEFT_CENTER,
            foot,
            small.clone(),
            muted,
        );
    }
    if deck.slide_numbers {
        let f = frame_rect(rect, varium_slides::number_frame());
        painter.text(
            f.right_center(),
            Align2::RIGHT_CENTER,
            format!("{}", i + 1),
            small,
            muted,
        );
    }
}

/// Les menus de la Présentation dans la barre native de macOS (le dernier est « Aide »).
#[cfg(target_os = "macos")]
pub fn native_menus(b: &mut crate::menubar::Builder) -> muda::Result<Vec<muda::Submenu>> {
    use crate::menubar::{MenuAct, clipboard_items, sep};
    use SlideAction as A;
    use muda::{IsMenuItem, Submenu};

    let export = Submenu::with_items(
        tr("Exporter"),
        true,
        &[
            &b.item(tr("PDF (une page par diapositive)"), A::ExportPdf, None),
            &b.item(tr("LibreOffice Impress (.odp)"), A::ExportOdp, None),
            &b.item(tr("Diaporama web (.html)"), A::ExportHtml, None),
        ],
    )?;
    let file = Submenu::with_items(
        tr("Fichier"),
        true,
        &[
            &b.item(tr("Nouvelle présentation"), A::New, Some("Cmd+N")),
            &b.item(tr("Présentation d'exemple"), A::Sample, None),
            &b.item(tr("Ouvrir…"), A::Open, Some("Cmd+O")),
            &sep(),
            &b.item(tr("Enregistrer"), A::Save, Some("Cmd+S")),
            &b.item(tr("Enregistrer sous…"), A::SaveAs, Some("Cmd+Shift+S")),
            &sep(),
            &export,
        ],
    )?;
    let [cut, copy, paste, select_all] = clipboard_items();
    let edit = Submenu::with_items(
        tr("Édition"),
        true,
        &[
            &b.item(tr("Annuler"), A::Undo, Some("Cmd+Z")),
            &b.item(tr("Rétablir"), A::Redo, Some("Cmd+Shift+Z")),
            &sep(),
            &cut,
            &copy,
            &paste,
            &select_all,
            &sep(),
            &b.item(tr("Dupliquer l'objet"), A::DuplicateObject, Some("Cmd+D")),
            &b.item(tr("Supprimer l'objet"), A::DeleteObject, None),
        ],
    )?;
    let shapes: Vec<_> = ObjectKind::SHAPES
        .into_iter()
        .map(|k| b.item(&shape_label(k), A::InsertShape(k), None))
        .collect();
    let insert = Submenu::with_items(
        tr("Insertion"),
        true,
        &[
            &b.item(tr("🖼 Image…"), A::InsertImage, Some("Cmd+Shift+I")),
            &sep(),
        ],
    )?;
    insert.append_items(
        &shapes
            .iter()
            .map(|i| i as &dyn IsMenuItem)
            .collect::<Vec<_>>(),
    )?;
    let slide = Submenu::with_items(
        tr("Diapositive"),
        true,
        &[
            &b.item(tr("Nouvelle diapositive"), A::NewSlide, Some("Cmd+Shift+N")),
            &b.item(tr("Dupliquer"), A::DuplicateSlide, None),
            &b.item(tr("Supprimer"), A::DeleteSlide, None),
            &sep(),
            &b.item(tr("Monter"), A::MoveUp, None),
            &b.item(tr("Descendre"), A::MoveDown, None),
        ],
    )?;
    let show = Submenu::with_items(
        tr("Diaporama"),
        true,
        &[
            &b.item(
                tr("Depuis la première diapositive"),
                A::StartFirst,
                Some("Cmd+Shift+Enter"),
            ),
            &b.item(
                tr("Depuis la diapositive actuelle"),
                A::StartCurrent,
                Some("Cmd+Enter"),
            ),
            &sep(),
            &b.item(tr("Mode présentateur"), A::Presenter, Some("Cmd+Alt+Enter")),
        ],
    )?;
    let help = Submenu::with_items(
        tr("Aide"),
        true,
        &[&b.item(tr("Site de Varium"), MenuAct::Website, None)],
    )?;
    Ok(vec![file, edit, insert, slide, show, help])
}

#[cfg(test)]
mod tests {
    use super::*;
    use varium_slides::Slide;

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

    fn carre() -> Object {
        Object {
            frame: Frame::new(0.2, 0.2, 0.2, 0.4),
            ..Object::default()
        }
    }

    #[test]
    fn deplacer_et_redimensionner() {
        let o = apply_drag(&carre(), Grip::Move, 0.1, -0.05, false);
        assert!((o.frame.x - 0.3).abs() < 1e-6 && (o.frame.y - 0.15).abs() < 1e-6);
        // Poignée en bas à droite : le coin opposé reste en place.
        let o = apply_drag(&carre(), Grip::Handle(4), 0.1, 0.1, false);
        assert_eq!((o.frame.x, o.frame.y), (0.2, 0.2));
        assert!((o.frame.w - 0.3).abs() < 1e-6 && (o.frame.h - 0.5).abs() < 1e-6);
        // Poignée de gauche tirée au-delà du bord droit : jamais négatif.
        let o = apply_drag(&carre(), Grip::Handle(7), 0.9, 0.0, false);
        assert!(o.frame.w >= 0.01 - 1e-6);
        // Maj : proportions conservées.
        let o = apply_drag(&carre(), Grip::Handle(4), 0.2, 0.0, true);
        assert!((o.frame.w / o.frame.h - 0.5).abs() < 1e-5);
        assert_eq!((o.frame.x, o.frame.y), (0.2, 0.2));
        let o = apply_drag(&carre(), Grip::Handle(0), -0.1, -0.1, true);
        assert!((o.frame.w / o.frame.h - 0.5).abs() < 1e-5);
        assert!((o.frame.right() - 0.4).abs() < 1e-5 && (o.frame.bottom() - 0.6).abs() < 1e-5);
    }

    #[test]
    fn extremites_de_ligne() {
        let ligne = Object {
            kind: ObjectKind::Arrow,
            frame: Frame::new(0.1, 0.1, 0.4, 0.2),
            ..Object::default()
        };
        // L'arrivée passe à gauche et au-dessus du départ : la flèche s'inverse.
        let o = apply_drag(&ligne, Grip::End(true), -0.6, -0.3, false);
        assert!(o.flip_h && o.flip_v);
        let ((x1, y1), (x2, y2)) = o.line_points();
        assert!((x1 - 0.1).abs() < 1e-6 && (y1 - 0.1).abs() < 1e-6);
        assert!((x2 + 0.1).abs() < 1e-6 && (y2 - 0.0).abs() < 1e-6);
    }

    #[test]
    fn selection_au_pointeur() {
        let r = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 500.0));
        let objets = vec![
            carre(),
            Object {
                frame: Frame::new(0.3, 0.3, 0.3, 0.3),
                ..Object::default()
            },
            Object {
                kind: ObjectKind::Line,
                frame: Frame::new(0.0, 0.9, 1.0, 0.0),
                ..Object::default()
            },
        ];
        // Le plus en avant d'abord.
        assert_eq!(hit_object(r, &objets, Pos2::new(350.0, 200.0)), Some(1));
        assert_eq!(hit_object(r, &objets, Pos2::new(250.0, 120.0)), Some(0));
        assert_eq!(hit_object(r, &objets, Pos2::new(500.0, 452.0)), Some(2));
        assert_eq!(hit_object(r, &objets, Pos2::new(900.0, 50.0)), None);
        assert_eq!(
            grip_at(r, &objets[0], Pos2::new(400.0, 300.0)),
            Some(Grip::Handle(4))
        );
    }

    #[test]
    fn chronometre_avec_pause() {
        let mut p = Presenter {
            started: 10.0,
            paused: None,
            paused_total: 0.0,
            notes_size: 20.0,
        };
        assert_eq!(p.elapsed(70.0), 60.0);
        p.paused = Some(70.0);
        assert_eq!(p.elapsed(100.0), 60.0);
        p.paused_total += 100.0 - 70.0;
        p.paused = None;
        assert_eq!(p.elapsed(110.0), 70.0);
    }
}
