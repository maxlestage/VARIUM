//! L'interface de Varium, sur le modèle de LibreOffice Writer : barre de menus,
//! barre d'outils de mise en forme, texte et page côte à côte, panneau des champs,
//! barre d'état. Sur un écran étroit (téléphone), les vues passent en onglets.

use egui::text::{CCursor, CharIndex};
use egui::text_selection::CCursorRange;
use egui::{Color32, Key, KeyboardShortcut, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use varium_core::export::{self, Rendered};
use varium_core::merge::{self, Sheet};
use varium_core::{DocFont, PageSetup, Paper, Project, Values, VarType, docx, filters, odt, stats};

use crate::editor::{self, LineStyle, Palette, Sel};
use crate::files;
use crate::io::{self, Event, Io, OpenKind};
use crate::pageview::{self, View};
use crate::secours;
use crate::templates;

const STORAGE_KEY: &str = "varium.project";
const PREFS_KEY: &str = "varium.prefs";
/// En dessous de cette largeur, l'interface passe en mode téléphone.
const MOBILE_WIDTH: f32 = 720.0;
/// Variable remplie automatiquement avec la date du jour.
const TODAY: &str = "aujourdhui";

// ---------------------------------------------------------------------------------
// Actions et raccourcis
// ---------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Act {
    New,
    Templates,
    NewBlank,
    Open,
    OpenRecent(usize),
    Save,
    SaveAs,
    Quit,
    Export(Format),
    Print,
    Merge,
    Undo,
    Redo,
    Find,
    Replace,
    Wrap(&'static str),
    Style(LineStyle),
    InsertField,
    InsertFormula,
    InsertToday,
    InsertPageBreak,
    InsertRule,
    InsertTable,
    InsertMath(bool),
    InsertText(&'static str),
    PageSetup,
    Stats,
    ClearValues,
    Shortcuts,
    Syntax,
    Languages,
    About,
    ZoomIn,
    ZoomOut,
    ZoomFit,
    Zoom(f32),
    ToggleHighlight,
    ToggleFields,
    ToggleToolbar,
    ToggleStatus,
    FullScreen,
    Focus,
    Layout(Layout),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Format {
    Pdf,
    Odt,
    Docx,
    Epub,
    Rtf,
    Html,
    Latex,
    Text,
    Model,
}

const fn sc(m: Modifiers, k: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(m, k)
}
const CMD: Modifiers = Modifiers::COMMAND;
const CMD_SHIFT: Modifiers = Modifiers::COMMAND.plus(Modifiers::SHIFT);

/// Raccourcis de LibreOffice Writer (les plus spécifiques d'abord).
const SHORTCUTS: &[(KeyboardShortcut, Act)] = &[
    (sc(CMD_SHIFT, Key::Z), Act::Redo),
    (sc(CMD_SHIFT, Key::F), Act::Focus),
    (sc(CMD_SHIFT, Key::N), Act::Templates),
    (sc(CMD_SHIFT, Key::S), Act::SaveAs),
    (sc(CMD_SHIFT, Key::V), Act::InsertField),
    (
        sc(Modifiers::SHIFT, Key::F12),
        Act::Style(LineStyle::Bullet),
    ),
    (sc(CMD, Key::N), Act::New),
    (sc(CMD, Key::O), Act::Open),
    (sc(CMD, Key::S), Act::Save),
    (sc(CMD, Key::P), Act::Print),
    (sc(CMD, Key::Z), Act::Undo),
    (sc(CMD, Key::Y), Act::Redo),
    (sc(CMD.plus(Modifiers::ALT), Key::F), Act::Replace),
    (sc(CMD, Key::F), Act::Find),
    (sc(CMD, Key::H), Act::Replace),
    (sc(CMD, Key::B), Act::Wrap("**")),
    (sc(CMD, Key::I), Act::Wrap("*")),
    (sc(CMD, Key::U), Act::Wrap("__")),
    (sc(CMD, Key::Num0), Act::Style(LineStyle::Body)),
    (sc(CMD, Key::Num1), Act::Style(LineStyle::Heading(1))),
    (sc(CMD, Key::Num2), Act::Style(LineStyle::Heading(2))),
    (sc(CMD, Key::Num3), Act::Style(LineStyle::Heading(3))),
    (sc(CMD, Key::E), Act::Style(LineStyle::Center)),
    (sc(CMD, Key::R), Act::Style(LineStyle::Right)),
    (sc(CMD, Key::L), Act::Style(LineStyle::Body)),
    (sc(CMD, Key::Enter), Act::InsertPageBreak),
    (
        sc(Modifiers::NONE, Key::F12),
        Act::Style(LineStyle::Numbered),
    ),
    (sc(Modifiers::NONE, Key::F1), Act::Syntax),
    (sc(Modifiers::NONE, Key::F11), Act::FullScreen),
];

fn shortcut_for(act: Act) -> Option<KeyboardShortcut> {
    SHORTCUTS.iter().find(|(_, a)| *a == act).map(|(s, _)| *s)
}

// ---------------------------------------------------------------------------------
// État
// ---------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
enum Layout {
    /// Texte et page côte à côte.
    Split,
    /// La page seule (lecture, impression).
    Page,
    /// Le texte seul.
    Text,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Text,
    Fields,
    Page,
}

/// Les onglets de l'inspecteur, à la manière de Pages.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
enum Inspector {
    Format,
    Document,
    Fields,
    Outline,
    Review,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Prefs {
    inspector: Inspector,
    layout: Layout,
    /// `None` : la page occupe la largeur disponible.
    zoom: Option<f32>,
    highlight: bool,
    fields_panel: bool,
    toolbar: bool,
    status_bar: bool,
    /// Documents récents, du plus récent au plus ancien.
    recent: Vec<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            inspector: Inspector::Fields,
            layout: Layout::Split,
            zoom: None,
            highlight: true,
            fields_panel: true,
            toolbar: true,
            status_bar: true,
            recent: Vec::new(),
        }
    }
}

#[derive(Clone, PartialEq)]
struct Snapshot {
    title: String,
    source: String,
    values: Values,
    page: PageSetup,
}

/// Annuler / rétablir : la frappe continue est regroupée en une seule étape.
struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    current: Snapshot,
    last_edit: f64,
    cut: bool,
}

enum Dialog {
    Templates,
    PageSetup,
    Merge,
    Table {
        rows: usize,
        cols: usize,
    },
    Field {
        name: String,
        ty: VarType,
    },
    Formula {
        expr: String,
    },
    Stats,
    Shortcuts,
    Syntax,
    Languages(varium_core::Lang),
    About,
    /// Modifications non enregistrées : que faire avant de continuer ?
    Confirm(Pending),
    /// Une sauvegarde de secours existe (arrêt brutal ?) : la restaurer ?
    Restore {
        project: Box<Project>,
        secours: std::path::PathBuf,
        document: Option<std::path::PathBuf>,
    },
}

/// Action différée jusqu'à la réponse à « Enregistrer les modifications ? ».
#[derive(Clone, PartialEq, Debug)]
enum Pending {
    New,
    Templates,
    Open,
    Recent(String),
    Quit,
}

enum SaveOutcome {
    Done,
    /// Le dialogue « Enregistrer sous » est ouvert.
    Asking,
    Failed,
}

#[derive(Default)]
struct Find {
    open: bool,
    replace: bool,
    query: String,
    with: String,
    case: bool,
    current: usize,
    focus: bool,
}

struct MergeState {
    file: String,
    sheet: Sheet,
    record: usize,
}

struct Rename {
    from: String,
    to: String,
}

pub struct VariumApp {
    title: String,
    source: String,
    values: Values,
    page: PageSetup,
    /// Sélection dans le texte (en caractères), même quand l'éditeur n'a pas le focus.
    sel: Sel,
    /// Sélection à appliquer à l'éditeur à la prochaine image.
    pending_sel: Option<Sel>,
    history: History,
    prefs: Prefs,
    tab: Tab,
    dialog: Option<Dialog>,
    find: Find,
    merge: Option<MergeState>,
    rename: Option<Rename>,
    toast: Option<(String, f64, bool)>,
    queue: Vec<Act>,
    io: Io,
    pages: usize,
    mobile_style: Option<bool>,
    /// Fichier du document ouvert (Ctrl+S le réécrit).
    path: Option<std::path::PathBuf>,
    /// État au dernier enregistrement (pour l'indicateur de modifications).
    saved: Snapshot,
    /// État envoyé au dialogue « Enregistrer sous » en cours.
    saving: Option<Snapshot>,
    /// À exécuter quand l'enregistrement demandé par la confirmation aboutit.
    after_save: Option<Pending>,
    last_window_title: String,
    /// Dernière action lancée (menu natif ou raccourci) et son heure, pour ne pas la doubler.
    last_act: Option<(Act, f64)>,
    /// Barre de menus native (macOS).
    #[cfg(target_os = "macos")]
    native: Option<native::NativeMenu>,
    allow_close: bool,
    /// Fichier de secours actuellement écrit (effacé dès que le document est propre).
    backup_file: Option<std::path::PathBuf>,
    last_backup: f64,
}

fn today() -> String {
    jiff::Zoned::now().date().to_string()
}

impl VariumApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::fonts::install(&cc.egui_ctx);
        let stored = cc
            .storage
            .and_then(|s| eframe::get_value::<Project>(s, STORAGE_KEY));
        let first_launch = stored.is_none();
        let project = stored.unwrap_or_else(templates::first);
        let prefs = cc
            .storage
            .and_then(|s| eframe::get_value::<Prefs>(s, PREFS_KEY))
            .unwrap_or_default();
        let snap = Snapshot {
            title: project.title.clone(),
            source: project.source.clone(),
            values: project.values(),
            page: project.page.clone(),
        };
        let mut app = Self {
            title: String::new(),
            source: String::new(),
            values: Values::default(),
            page: PageSetup::default(),
            sel: (0, 0),
            pending_sel: None,
            history: History {
                undo: Vec::new(),
                redo: Vec::new(),
                current: snap,
                last_edit: 0.0,
                cut: false,
            },
            prefs,
            tab: Tab::Text,
            dialog: None,
            find: Find::default(),
            merge: None,
            rename: None,
            toast: None,
            queue: Vec::new(),
            io: Io::default(),
            pages: 1,
            mobile_style: None,
            path: None,
            saved: Snapshot {
                title: String::new(),
                source: String::new(),
                values: Values::default(),
                page: PageSetup::default(),
            },
            saving: None,
            after_save: None,
            last_window_title: String::new(),
            last_act: None,
            #[cfg(target_os = "macos")]
            native: None,
            allow_close: false,
            backup_file: None,
            last_backup: f64::NEG_INFINITY,
        };
        app.restore(app.history.current.clone());
        app.saved = app.snapshot();
        #[cfg(target_os = "macos")]
        {
            app.native = native::NativeMenu::install();
        }
        // Document sans fichier interrompu par un plantage ?
        app.propose_secours(None);
        // Un fichier donné au lancement (« Ouvrir avec », glisser sur l'icône, terminal).
        if let Some(path) = std::env::args_os().nth(1).map(std::path::PathBuf::from)
            && path.is_file()
        {
            let kind = if path.extension().is_some_and(|e| e == "csv" || e == "tsv") {
                OpenKind::Data
            } else {
                OpenKind::Document
            };
            app.io.open_path(&cc.egui_ctx, kind, &path);
        } else if first_launch && app.dialog.is_none() {
            // Comme Pages : on commence par choisir un modèle.
            app.dialog = Some(Dialog::Templates);
        }
        app
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            title: self.title.clone(),
            source: self.source.clone(),
            values: self.values.clone(),
            page: self.page.clone(),
        }
    }

    fn restore(&mut self, s: Snapshot) {
        self.title = s.title;
        self.source = s.source;
        self.values = s.values;
        self.page = s.page;
        let n = self.source.chars().count();
        self.sel = (self.sel.0.min(n), self.sel.1.min(n));
        self.rename = None;
    }

    fn load(&mut self, p: Project) {
        self.history.cut = true;
        let values = p.values();
        self.restore(Snapshot {
            title: p.title,
            source: p.source,
            values,
            page: p.page,
        });
        self.sel = (0, 0);
        self.pending_sel = Some((0, 0));
        self.merge = None;
        self.path = None;
        self.saved = self.snapshot();
    }

    /// Propose de restaurer la sauvegarde de secours de `document` (ou du
    /// document sans fichier) si elle est plus récente et différente.
    fn propose_secours(&mut self, document: Option<&std::path::Path>) {
        let fichier = secours::chemin(document, &secours::dossier_donnees());
        if !secours::plus_recente(&fichier, document) {
            return;
        }
        match secours::lire(&fichier) {
            Some(p)
                if p.source != self.source
                    || p.title != self.title
                    || p.values() != self.values =>
            {
                self.dialog = Some(Dialog::Restore {
                    project: Box::new(p),
                    secours: fichier,
                    document: document.map(std::path::Path::to_path_buf),
                });
            }
            _ => secours::effacer(&fichier),
        }
    }

    /// Remplace le contenu par la sauvegarde ; le document reste « modifié ».
    fn restaurer(&mut self, p: Project, document: Option<std::path::PathBuf>) {
        let saved = self.saved.clone();
        self.load(p);
        self.saved = if document.is_some() {
            saved
        } else {
            Snapshot {
                title: String::new(),
                source: String::new(),
                values: Values::default(),
                page: PageSetup::default(),
            }
        };
        self.path = document;
    }

    /// Sauvegarde de secours : écrite régulièrement tant que le document est
    /// modifié, effacée dès qu'il est propre.
    fn tick_secours(&mut self, ctx: &egui::Context, now: f64) {
        if matches!(self.dialog, Some(Dialog::Restore { .. })) {
            return;
        }
        if !self.is_dirty() {
            if let Some(f) = self.backup_file.take() {
                secours::effacer(&f);
            }
            return;
        }
        let voulu = secours::chemin(self.path.as_deref(), &secours::dossier_donnees());
        let change = self.backup_file.as_ref() != Some(&voulu);
        if change || secours::echeance(now, self.last_backup) {
            if change && let Some(old) = self.backup_file.take() {
                secours::effacer(&old);
            }
            if secours::ecrire(&voulu, &self.project()).is_ok() {
                self.backup_file = Some(voulu);
            }
            self.last_backup = now;
        }
        ctx.request_repaint_after(secours::INTERVALLE);
    }

    /// Des modifications ne sont-elles pas enregistrées ?
    /// Le document a des modifications non enregistrées (pour la suite).
    pub fn has_unsaved_changes(&self) -> bool {
        self.is_dirty()
    }

    /// Force la remise à jour du titre de la fenêtre (retour depuis un autre module).
    pub fn refresh_window_title(&mut self) {
        self.last_window_title.clear();
    }

    fn is_dirty(&self) -> bool {
        self.saved.source != self.source
            || self.saved.title != self.title
            || self.saved.page != self.page
            || self.saved.values != self.values
    }

    /// Exécute `p` tout de suite, ou demande confirmation s'il y a des
    /// modifications non enregistrées.
    fn guard(&mut self, ctx: &egui::Context, p: Pending) {
        if self.is_dirty() {
            self.dialog = Some(Dialog::Confirm(p));
        } else {
            self.proceed(ctx, p);
        }
    }

    fn proceed(&mut self, ctx: &egui::Context, p: Pending) {
        match p {
            Pending::New => self.run(ctx, Act::NewBlank),
            Pending::Templates => self.dialog = Some(Dialog::Templates),
            Pending::Open => self.io.open(ctx, OpenKind::Document),
            Pending::Recent(path) => {
                let path = std::path::PathBuf::from(path);
                if path.exists() {
                    self.io.open_path(ctx, OpenKind::Document, &path);
                } else {
                    files::forget_recent(&mut self.prefs.recent, &path.display().to_string());
                    self.notify(
                        ctx,
                        format!(
                            "« {} » est introuvable (déplacé ou supprimé)",
                            path.display()
                        ),
                        true,
                    );
                }
            }
            Pending::Quit => {
                if let Some(f) = self.backup_file.take() {
                    secours::effacer(&f);
                }
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Enregistre le projet : réécrit le fichier courant, ou ouvre « Enregistrer sous ».
    fn save_project(&mut self, ctx: &egui::Context, as_new: bool) -> SaveOutcome {
        self.after_save = None;
        let bytes = self.project().to_json().into_bytes();
        let snap = self.snapshot();
        match (&self.path, as_new) {
            (Some(path), false) => match io::write_atomic(path, &bytes) {
                Ok(()) => {
                    let path = path.clone();
                    self.saved = snap;
                    files::push_recent(&mut self.prefs.recent, &path);
                    self.notify(ctx, format!("Enregistré : {}", path.display()), false);
                    SaveOutcome::Done
                }
                Err(e) => {
                    self.notify(ctx, format!("Écriture impossible : {e}"), true);
                    SaveOutcome::Failed
                }
            },
            _ => {
                self.saving = Some(snap);
                let name = format!("{}.varium.json", self.base_stem());
                self.io.save(ctx, name, "application/json", bytes, true);
                SaveOutcome::Asking
            }
        }
    }

    fn project(&self) -> Project {
        let mut p = Project::new(self.title.clone(), self.source.clone(), &self.values);
        p.page = self.page.clone();
        p
    }

    /// Enregistre l'état dans l'historique (appelé à chaque image).
    fn track(&mut self, now: f64) {
        if self.history.current.source == self.source
            && self.history.current.values == self.values
            && self.history.current.page == self.page
            && self.history.current.title == self.title
        {
            return;
        }
        let snap = self.snapshot();
        if self.history.cut || now - self.history.last_edit > 1.0 {
            let prev = std::mem::replace(&mut self.history.current, snap);
            self.history.undo.push(prev);
            if self.history.undo.len() > 300 {
                self.history.undo.remove(0);
            }
        } else {
            self.history.current = snap;
        }
        self.history.redo.clear();
        self.history.last_edit = now;
        self.history.cut = false;
    }

    fn undo(&mut self) {
        if let Some(prev) = self.history.undo.pop() {
            let cur = std::mem::replace(&mut self.history.current, prev.clone());
            self.history.redo.push(cur);
            self.restore(prev);
            self.history.cut = true;
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.history.redo.pop() {
            let cur = std::mem::replace(&mut self.history.current, next.clone());
            self.history.undo.push(cur);
            self.restore(next);
            self.history.cut = true;
        }
    }

    fn notify(&mut self, ctx: &egui::Context, msg: impl Into<String>, error: bool) {
        let now = ctx.input(|i| i.time);
        self.toast = Some((msg.into(), now, error));
    }

    fn variable_names(&self) -> Vec<String> {
        varium_core::variables(&self.source)
            .into_iter()
            .map(|v| v.name)
            .collect()
    }

    /// Les valeurs utilisées pour l'aperçu : celles du publipostage s'il est actif,
    /// et la date du jour pour `{{aujourdhui}}`.
    fn effective(&self) -> Values {
        let mut v = match &self.merge {
            Some(m) => m
                .sheet
                .values_for(m.record, &self.values, &self.variable_names()),
            None => self.values.clone(),
        };
        if v.get(TODAY).is_empty() {
            v.set(TODAY, today());
            v.types.entry(TODAY.into()).or_insert(VarType::Date);
        }
        v
    }

    /// Les documents à exporter : un seul, ou un par ligne du publipostage.
    fn rendered(&self) -> Vec<Rendered> {
        match &self.merge {
            Some(m) => {
                let names = self.variable_names();
                (0..m.sheet.rows.len())
                    .map(|i| {
                        let mut v = m.sheet.values_for(i, &self.values, &names);
                        if v.get(TODAY).is_empty() {
                            v.set(TODAY, today());
                            v.types.entry(TODAY.into()).or_insert(VarType::Date);
                        }
                        export::prepare(&self.source, &v, &self.page)
                    })
                    .collect()
            }
            None => vec![export::prepare(&self.source, &self.effective(), &self.page)],
        }
    }

    fn base_stem(&self) -> String {
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

    fn file_stem(&self) -> String {
        let s = self.base_stem();
        if self.merge.is_some() {
            format!("{s}-publipostage")
        } else {
            s
        }
    }

    // ---------------- Actions ----------------

    fn edit(&mut self, f: impl FnOnce(&mut String, Sel) -> Sel) {
        self.history.cut = true;
        let sel = f(&mut self.source, self.sel);
        self.sel = sel;
        self.pending_sel = Some(sel);
        self.tab = Tab::Text;
    }

    fn insert(&mut self, text: &str, select: Option<Sel>) {
        let text = text.to_string();
        self.edit(|s, sel| editor::insert(s, sel, &text, select));
    }

    /// Insère un bloc sur sa propre ligne.
    fn insert_block(&mut self, block: &str) {
        let b = editor::byte_of(&self.source, self.sel.0.min(self.sel.1));
        let before_nl = b == 0 || self.source[..b].ends_with('\n');
        let after_nl = self.source[b..].starts_with('\n') || b == self.source.len();
        let text = format!(
            "{}{block}{}",
            if before_nl { "" } else { "\n" },
            if after_nl { "" } else { "\n" }
        );
        self.insert(&text, None);
    }

    /// Ajoute une action, sauf si la même vient d'arriver par l'autre chemin
    /// (raccourci reçu à la fois par le menu natif et par egui).
    fn push_once(&mut self, act: Act, now: f64) {
        if let Some((last, t)) = self.last_act
            && last == act
            && now - t < 0.4
        {
            return;
        }
        self.last_act = Some((act, now));
        self.queue.push(act);
    }

    fn has_native_menu(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.native.is_some()
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    fn run(&mut self, ctx: &egui::Context, act: Act) {
        match act {
            Act::New => self.guard(ctx, Pending::New),
            Act::NewBlank => {
                self.load(Project::new("Sans titre", "", &Values::default()));
                self.notify(
                    ctx,
                    "Nouveau document · Ctrl+Z pour revenir au précédent",
                    false,
                );
            }
            Act::Templates => self.guard(ctx, Pending::Templates),
            Act::Open => self.guard(ctx, Pending::Open),
            Act::OpenRecent(i) => {
                if let Some(path) = self.prefs.recent.get(i).cloned() {
                    self.guard(ctx, Pending::Recent(path));
                }
            }
            Act::Save => {
                self.save_project(ctx, false);
            }
            Act::SaveAs => {
                self.save_project(ctx, true);
            }
            Act::Quit => self.guard(ctx, Pending::Quit),
            Act::Export(f) => self.export(ctx, f),
            Act::Print => {
                let html = export::html(&self.title, &self.rendered(), &self.page);
                self.io
                    .print(ctx, format!("{}.html", self.file_stem()), html);
            }
            Act::Merge => self.dialog = Some(Dialog::Merge),
            Act::Undo => self.undo(),
            Act::Redo => self.redo(),
            Act::Find | Act::Replace => {
                self.find.open = true;
                self.find.replace = act == Act::Replace || self.find.replace;
                self.find.focus = true;
                let (a, b) = self.sel;
                if a != b {
                    let (ba, bb) = (
                        editor::byte_of(&self.source, a.min(b)),
                        editor::byte_of(&self.source, a.max(b)),
                    );
                    let s = &self.source[ba..bb];
                    if !s.contains('\n') {
                        self.find.query = s.to_string();
                    }
                }
            }
            Act::Wrap(m) => self.edit(|s, sel| editor::toggle_wrap(s, sel, m)),
            Act::Style(st) => self.edit(|s, sel| editor::set_line_style(s, sel, st)),
            Act::InsertField => {
                self.dialog = Some(Dialog::Field {
                    name: String::new(),
                    ty: VarType::Text,
                })
            }
            Act::InsertFormula => {
                self.dialog = Some(Dialog::Formula {
                    expr: String::new(),
                })
            }
            Act::InsertToday => {
                self.values.types.insert(TODAY.into(), VarType::Date);
                self.insert(&format!("{{{{{TODAY}}}}}"), None);
            }
            Act::InsertPageBreak => self.insert_block("\\newpage"),
            Act::InsertRule => self.insert_block("---"),
            Act::InsertTable => self.dialog = Some(Dialog::Table { rows: 3, cols: 3 }),
            Act::InsertMath(false) => self.insert("$x^2$", Some((1, 4))),
            Act::InsertMath(true) => self.insert_block("$$ \\sum_{i=1}^{n} x_i $$"),
            Act::InsertText(t) => self.insert(t, None),
            Act::PageSetup => self.dialog = Some(Dialog::PageSetup),
            Act::Stats => self.dialog = Some(Dialog::Stats),
            Act::ClearValues => {
                self.history.cut = true;
                self.values.values.clear();
                self.notify(ctx, "Valeurs effacées · Ctrl+Z pour les retrouver", false);
            }
            Act::Shortcuts => self.dialog = Some(Dialog::Shortcuts),
            Act::Syntax => self.dialog = Some(Dialog::Syntax),
            Act::Languages => self.dialog = Some(Dialog::Languages(self.page.langue)),
            Act::About => self.dialog = Some(Dialog::About),
            Act::ZoomIn => self.prefs.zoom = Some((self.zoom_now() + 0.1).min(3.0)),
            Act::ZoomOut => self.prefs.zoom = Some((self.zoom_now() - 0.1).max(0.3)),
            Act::ZoomFit => self.prefs.zoom = None,
            Act::Zoom(z) => self.prefs.zoom = Some(z),
            Act::ToggleHighlight => self.prefs.highlight = !self.prefs.highlight,
            Act::ToggleFields => self.prefs.fields_panel = !self.prefs.fields_panel,
            Act::ToggleToolbar => self.prefs.toolbar = !self.prefs.toolbar,
            Act::ToggleStatus => self.prefs.status_bar = !self.prefs.status_bar,
            Act::FullScreen => {
                let on = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!on));
            }
            Act::Focus => {
                // Mode concentration : rien que la page et le texte.
                let on = self.prefs.toolbar || self.prefs.fields_panel || self.prefs.status_bar;
                self.prefs.toolbar = !on;
                self.prefs.fields_panel = !on;
                self.prefs.status_bar = !on;
            }
            Act::Layout(l) => self.prefs.layout = l,
        }
    }

    fn zoom_now(&self) -> f32 {
        self.prefs.zoom.unwrap_or(1.0)
    }

    fn export(&mut self, ctx: &egui::Context, f: Format) {
        let stem = self.file_stem();
        let docs = self.rendered();
        let (name, mime, bytes): (String, &'static str, Vec<u8>) = match f {
            Format::Pdf => (
                format!("{stem}.pdf"),
                "application/pdf",
                varium_core::pdf::pdf(
                    &self.title,
                    &docs,
                    &self.page,
                    &crate::fonts::pdf_fonts(self.page.font),
                ),
            ),
            Format::Odt => (
                format!("{stem}.odt"),
                "application/vnd.oasis.opendocument.text",
                odt::odt(&self.title, &docs, &self.page),
            ),
            Format::Docx => (
                format!("{stem}.docx"),
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                docx::docx(&self.title, &docs, &self.page),
            ),
            Format::Epub => (
                format!("{stem}.epub"),
                "application/epub+zip",
                varium_core::epub::epub(&self.title, &docs, &self.page),
            ),
            Format::Rtf => (
                format!("{stem}.rtf"),
                "application/rtf",
                varium_core::rtf::rtf(&docs, &self.page).into_bytes(),
            ),
            Format::Html => (
                format!("{stem}.html"),
                "text/html;charset=utf-8",
                export::html(&self.title, &docs, &self.page).into_bytes(),
            ),
            Format::Latex => (
                format!("{stem}.tex"),
                "application/x-tex;charset=utf-8",
                export::latex(&self.title, &docs, &self.page).into_bytes(),
            ),
            Format::Text => (
                format!("{stem}.txt"),
                "text/plain;charset=utf-8",
                export::text(&docs).into_bytes(),
            ),
            Format::Model => (
                format!("{stem}.modele.txt"),
                "text/plain;charset=utf-8",
                self.source.clone().into_bytes(),
            ),
        };
        self.io.save(ctx, name, mime, bytes, false);
    }

    fn handle_io(&mut self, ctx: &egui::Context) {
        for ev in self.io.drain() {
            match ev {
                // Classeurs et présentations ont leur propre module.
                Event::Opened {
                    kind: OpenKind::Workbook | OpenKind::Deck,
                    ..
                } => {}
                Event::Opened {
                    kind: OpenKind::Data,
                    name,
                    content,
                    ..
                } => match merge::parse_csv(&content) {
                    Ok(sheet) if sheet.rows.is_empty() => self.notify(
                        ctx,
                        format!("« {name} » ne contient aucune ligne de données"),
                        true,
                    ),
                    Ok(sheet) => {
                        let n = sheet.rows.len();
                        self.merge = Some(MergeState {
                            file: name.clone(),
                            sheet,
                            record: 0,
                        });
                        self.dialog = Some(Dialog::Merge);
                        self.notify(
                            ctx,
                            format!("{n} destinataire(s) chargé(s) depuis « {name} »"),
                            false,
                        );
                    }
                    Err(e) => self.notify(ctx, e, true),
                },
                Event::Opened {
                    kind: OpenKind::Document,
                    name,
                    content,
                    path,
                } => {
                    if files::is_project_file(&name) {
                        match Project::from_json(&content) {
                            Ok(p) => {
                                self.load(p);
                                if let Some(path) = path {
                                    files::push_recent(&mut self.prefs.recent, &path);
                                    self.propose_secours(Some(&path));
                                    self.path = Some(path);
                                }
                                self.notify(ctx, format!("« {name} » ouvert"), false);
                            }
                            Err(e) => self.notify(ctx, e, true),
                        }
                    } else {
                        let title = name
                            .rsplit_once('.')
                            .map_or(name.as_str(), |(s, _)| s)
                            .to_string();
                        let mut p = Project::new(title, content, &self.values);
                        p.page = self.page.clone();
                        self.load(p);
                        self.notify(
                            ctx,
                            format!("Modèle « {name} » ouvert, valeurs conservées"),
                            false,
                        );
                    }
                }
                Event::Saved { path, project } => {
                    self.notify(ctx, format!("Enregistré : {}", path.display()), false);
                    if project {
                        self.saved = self.saving.take().unwrap_or_else(|| self.snapshot());
                        files::push_recent(&mut self.prefs.recent, &path);
                        self.path = Some(path);
                        if let Some(p) = self.after_save.take() {
                            self.proceed(ctx, p);
                        }
                    }
                }
                Event::Info(msg) => self.notify(ctx, msg, false),
                Event::Error(e) => self.notify(ctx, e, true),
            }
        }
    }

    // ---------------- Menus ----------------

    fn item(&mut self, ui: &mut egui::Ui, label: &str, act: Act) {
        let mut b = egui::Button::new(label);
        if let Some(s) = shortcut_for(act) {
            b = b.shortcut_text(ui.ctx().format_shortcut(&s));
        }
        if ui.add(b).clicked() {
            self.queue.push(act);
        }
    }

    fn check(&mut self, ui: &mut egui::Ui, label: &str, on: bool, act: Act) {
        let mut b = egui::Button::new(format!("{}  {label}", if on { "☑" } else { "☐" }));
        if let Some(s) = shortcut_for(act) {
            b = b.shortcut_text(ui.ctx().format_shortcut(&s));
        }
        if ui.add(b).clicked() {
            self.queue.push(act);
        }
    }

    fn menus(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Fichier", |ui| {
            self.item(ui, "Nouveau", Act::New);
            self.item(ui, "Nouveau à partir d'un modèle…", Act::Templates);
            self.item(ui, "Ouvrir…", Act::Open);
            ui.add_enabled_ui(!self.prefs.recent.is_empty(), |ui| {
                ui.menu_button("Documents récents", |ui| {
                    for (i, p) in self.prefs.recent.clone().iter().enumerate() {
                        if ui.button(files::short_name(p)).on_hover_text(p).clicked() {
                            self.queue.push(Act::OpenRecent(i));
                            ui.close();
                        }
                    }
                });
            });
            ui.separator();
            self.item(ui, "Enregistrer", Act::Save);
            self.item(ui, "Enregistrer sous…", Act::SaveAs);
            ui.menu_button("Exporter", |ui| {
                self.item(ui, "PDF (.pdf)", Act::Export(Format::Pdf));
                self.item(
                    ui,
                    "OpenDocument Texte (.odt) — LibreOffice",
                    Act::Export(Format::Odt),
                );
                self.item(ui, "Word (.docx)", Act::Export(Format::Docx));
                self.item(
                    ui,
                    "Texte enrichi (.rtf) — Pages, TextEdit",
                    Act::Export(Format::Rtf),
                );
                self.item(ui, "Livre numérique (.epub)", Act::Export(Format::Epub));
                self.item(ui, "Page web (.html)", Act::Export(Format::Html));
                self.item(ui, "LaTeX (.tex)", Act::Export(Format::Latex));
                self.item(ui, "Texte brut (.txt)", Act::Export(Format::Text));
                self.item(ui, "Modèle avec ses {{champs}}", Act::Export(Format::Model));
            });
            self.item(ui, "Imprimer (via le navigateur)…", Act::Print);
            ui.separator();
            self.item(ui, "Publipostage…", Act::Merge);
            ui.separator();
            self.item(ui, "Quitter", Act::Quit);
        });
        ui.menu_button("Édition", |ui| {
            let (u, r) = (!self.history.undo.is_empty(), !self.history.redo.is_empty());
            ui.add_enabled_ui(u, |ui| self.item(ui, "Annuler", Act::Undo));
            ui.add_enabled_ui(r, |ui| self.item(ui, "Rétablir", Act::Redo));
            ui.separator();
            self.item(ui, "Rechercher…", Act::Find);
            self.item(ui, "Rechercher et remplacer…", Act::Replace);
            ui.separator();
            self.item(ui, "Effacer toutes les valeurs", Act::ClearValues);
        });
        ui.menu_button("Affichage", |ui| {
            let l = self.prefs.layout;
            self.check(
                ui,
                "Texte et page",
                l == Layout::Split,
                Act::Layout(Layout::Split),
            );
            self.check(
                ui,
                "Page seule",
                l == Layout::Page,
                Act::Layout(Layout::Page),
            );
            self.check(
                ui,
                "Texte seul",
                l == Layout::Text,
                Act::Layout(Layout::Text),
            );
            ui.separator();
            self.check(
                ui,
                "Panneau des champs",
                self.prefs.fields_panel,
                Act::ToggleFields,
            );
            self.check(ui, "Barre d'outils", self.prefs.toolbar, Act::ToggleToolbar);
            self.check(ui, "Barre d'état", self.prefs.status_bar, Act::ToggleStatus);
            ui.separator();
            self.item(ui, "Mode concentration", Act::Focus);
            self.item(ui, "Plein écran", Act::FullScreen);
            self.check(
                ui,
                "Trame de fond des champs",
                self.prefs.highlight,
                Act::ToggleHighlight,
            );
            ui.separator();
            ui.menu_button("Zoom", |ui| {
                self.item(ui, "Largeur de la page", Act::ZoomFit);
                for z in [0.5, 0.75, 1.0, 1.25, 1.5, 2.0] {
                    self.item(ui, &format!("{} %", (z * 100.0) as i32), Act::Zoom(z));
                }
            });
            ui.menu_button("Thème", egui::widgets::global_theme_preference_buttons);
        });
        ui.menu_button("Insertion", |ui| {
            self.item(ui, "Champ {{…}}…", Act::InsertField);
            self.item(ui, "Champ calculé {{= …}}…", Act::InsertFormula);
            self.item(ui, "Date du jour", Act::InsertToday);
            ui.separator();
            self.item(ui, "Tableau…", Act::InsertTable);
            self.item(ui, "Saut de page", Act::InsertPageBreak);
            self.item(ui, "Ligne horizontale", Act::InsertRule);
            ui.menu_button("Formule", |ui| {
                self.item(ui, "En ligne  $…$", Act::InsertMath(false));
                self.item(ui, "Centrée  $$…$$", Act::InsertMath(true));
            });
            ui.menu_button("Caractère spécial", |ui| {
                for (c, name) in SPECIAL {
                    self.item(ui, &format!("{c}   {name}"), Act::InsertText(c));
                }
            });
        });
        ui.menu_button("Format", |ui| {
            self.item(ui, "Gras", Act::Wrap("**"));
            self.item(ui, "Italique", Act::Wrap("*"));
            self.item(ui, "Souligné", Act::Wrap("__"));
            self.item(ui, "Barré", Act::Wrap("~~"));
            ui.separator();
            ui.menu_button("Styles de paragraphe", |ui| {
                for st in LineStyle::MENU {
                    self.item(ui, st.label(), Act::Style(st));
                }
            });
            ui.menu_button("Listes", |ui| {
                self.item(ui, "Liste à puces", Act::Style(LineStyle::Bullet));
                self.item(ui, "Liste numérotée", Act::Style(LineStyle::Numbered));
            });
            ui.menu_button("Alignement", |ui| {
                self.item(ui, "À gauche", Act::Style(LineStyle::Body));
                self.item(ui, "Centré", Act::Style(LineStyle::Center));
                self.item(ui, "À droite", Act::Style(LineStyle::Right));
            });
            ui.separator();
            self.item(
                ui,
                "Page… (format, marges, police, en-tête)",
                Act::PageSetup,
            );
        });
        ui.menu_button("Outils", |ui| {
            self.item(ui, "Publipostage…", Act::Merge);
            self.item(ui, "Nombre de mots", Act::Stats);
        });
        ui.menu_button("Aide", |ui| {
            self.item(ui, "Guide de la syntaxe", Act::Syntax);
            self.item(ui, "Écrire en plusieurs langues", Act::Languages);
            self.item(ui, "Raccourcis clavier", Act::Shortcuts);
            ui.hyperlink_to("Site de Varium", "https://maxlestage.github.io/VARIUM/");
            self.item(ui, "À propos de Varium", Act::About);
        });
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.label(RichText::new("{ }").monospace().strong().color(accent(ui)));
            ui.label(RichText::new("Varium").strong().size(15.0));
            ui.add_space(6.0);
            self.menus(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.title)
                        .hint_text("Titre du document")
                        .desired_width(220.0),
                );
            });
        });
    }

    // ---------------- Barre d'outils ----------------

    fn tool(&mut self, ui: &mut egui::Ui, icon: Icon, tip: &str, act: Act, on: bool) {
        let mut tip = tip.to_string();
        if let Some(s) = shortcut_for(act) {
            tip = format!("{tip} ({})", ui.ctx().format_shortcut(&s));
        }
        if icon_button(ui, icon, on).on_hover_text(tip).clicked() {
            self.queue.push(act);
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, mobile: bool) {
        let style = editor::line_style_at(&self.source, self.sel.1);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if !mobile {
                self.tool(ui, Icon::New, "Nouveau", Act::New, false);
                self.tool(ui, Icon::Open, "Ouvrir", Act::Open, false);
                self.tool(ui, Icon::Save, "Enregistrer (Ctrl+S)", Act::Save, false);
                sep(ui);
                self.tool(
                    ui,
                    Icon::Pdf,
                    "Exporter en PDF",
                    Act::Export(Format::Pdf),
                    false,
                );
                self.tool(
                    ui,
                    Icon::Odt,
                    "Exporter pour LibreOffice (.odt)",
                    Act::Export(Format::Odt),
                    false,
                );
                sep(ui);
                self.tool(ui, Icon::Undo, "Annuler", Act::Undo, false);
                self.tool(ui, Icon::Redo, "Rétablir", Act::Redo, false);
                self.tool(
                    ui,
                    Icon::Find,
                    "Rechercher et remplacer",
                    Act::Replace,
                    self.find.open,
                );
                sep(ui);
            }
            let mut chosen = style;
            egui::ComboBox::from_id_salt("para-style")
                .selected_text(match style {
                    LineStyle::Bullet
                    | LineStyle::Numbered
                    | LineStyle::Center
                    | LineStyle::Right => LineStyle::Body.label(),
                    s => s.label(),
                })
                .width(if mobile { 118.0 } else { 140.0 })
                .show_ui(ui, |ui| {
                    for st in LineStyle::MENU {
                        ui.selectable_value(&mut chosen, st, st.label());
                    }
                });
            if chosen != style {
                self.queue.push(Act::Style(chosen));
            }
            sep(ui);
            self.tool(ui, Icon::Bold, "Gras", Act::Wrap("**"), false);
            self.tool(ui, Icon::Italic, "Italique", Act::Wrap("*"), false);
            self.tool(ui, Icon::Underline, "Souligné", Act::Wrap("__"), false);
            self.tool(ui, Icon::Strike, "Barré", Act::Wrap("~~"), false);
            sep(ui);
            self.tool(
                ui,
                Icon::AlignLeft,
                "Aligner à gauche",
                Act::Style(LineStyle::Body),
                !matches!(style, LineStyle::Center | LineStyle::Right),
            );
            self.tool(
                ui,
                Icon::AlignCenter,
                "Centrer",
                Act::Style(LineStyle::Center),
                style == LineStyle::Center,
            );
            self.tool(
                ui,
                Icon::AlignRight,
                "Aligner à droite",
                Act::Style(LineStyle::Right),
                style == LineStyle::Right,
            );
            sep(ui);
            self.tool(
                ui,
                Icon::Bullets,
                "Liste à puces",
                Act::Style(LineStyle::Bullet),
                style == LineStyle::Bullet,
            );
            self.tool(
                ui,
                Icon::Numbers,
                "Liste numérotée",
                Act::Style(LineStyle::Numbered),
                style == LineStyle::Numbered,
            );
            sep(ui);
            self.tool(
                ui,
                Icon::Field,
                "Insérer un champ {{…}}",
                Act::InsertField,
                false,
            );
            self.tool(
                ui,
                Icon::Formula,
                "Insérer un champ calculé",
                Act::InsertFormula,
                false,
            );
            self.tool(
                ui,
                Icon::Table,
                "Insérer un tableau",
                Act::InsertTable,
                false,
            );
            self.tool(
                ui,
                Icon::PageBreak,
                "Saut de page",
                Act::InsertPageBreak,
                false,
            );
            if mobile {
                sep(ui);
                self.tool(
                    ui,
                    Icon::Find,
                    "Rechercher et remplacer",
                    Act::Replace,
                    self.find.open,
                );
            }
            if !mobile && self.has_native_menu() {
                sep(ui);
                ui.add(
                    egui::TextEdit::singleline(&mut self.title)
                        .hint_text("Titre du document")
                        .desired_width(200.0),
                );
            }
        });
    }

    // ---------------- Recherche ----------------

    fn find_bar(&mut self, ui: &mut egui::Ui) {
        let found = editor::find_all(&self.source, &self.find.query, self.find.case);
        let n = found.len();
        if self.find.current >= n {
            self.find.current = 0;
        }
        let mut go: Option<usize> = None;
        let mut replace_one = false;
        let mut replace_all = false;
        ui.horizontal_wrapped(|ui| {
            ui.label("🔍");
            let find_id = egui::Id::unique("varium-find");
            if self.find.focus {
                // Le texte proposé est sélectionné : la frappe le remplace.
                let mut st =
                    egui::text_edit::TextEditState::load(ui.ctx(), find_id).unwrap_or_default();
                st.cursor.set_char_range(Some(CCursorRange::two(
                    CCursor::new(CharIndex(0)),
                    CCursor::new(CharIndex(self.find.query.chars().count())),
                )));
                st.store(ui.ctx(), find_id);
            }
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.find.query)
                    .id(find_id)
                    .hint_text("Rechercher")
                    .desired_width(180.0),
            );
            if self.find.focus {
                r.request_focus();
                self.find.focus = false;
            }
            if r.changed() {
                self.find.current = 0;
                go = Some(0);
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && n > 0 {
                go = Some((self.find.current + 1) % n);
                r.request_focus();
            }
            ui.label(
                RichText::new(if self.find.query.is_empty() {
                    String::new()
                } else if n == 0 {
                    "aucun résultat".into()
                } else {
                    format!("{} sur {n}", self.find.current + 1)
                })
                .small()
                .weak(),
            );
            if ui
                .add_enabled(n > 0, egui::Button::new("⏶").small())
                .on_hover_text("Précédent")
                .clicked()
            {
                go = Some((self.find.current + n - 1) % n);
            }
            if ui
                .add_enabled(n > 0, egui::Button::new("⏷").small())
                .on_hover_text("Suivant")
                .clicked()
            {
                go = Some((self.find.current + 1) % n);
            }
            ui.checkbox(&mut self.find.case, "Respecter la casse");
            if self.find.replace {
                ui.separator();
                ui.add(
                    egui::TextEdit::singleline(&mut self.find.with)
                        .hint_text("Remplacer par")
                        .desired_width(160.0),
                );
                replace_one = ui
                    .add_enabled(n > 0, egui::Button::new("Remplacer"))
                    .clicked();
                replace_all = ui
                    .add_enabled(n > 0, egui::Button::new("Tout remplacer"))
                    .clicked();
            } else if ui.small_button("Remplacer…").clicked() {
                self.find.replace = true;
            }
            if ui
                .small_button("✕")
                .on_hover_text("Fermer (Échap)")
                .clicked()
                || ui.input(|i| i.key_pressed(Key::Escape))
            {
                self.find.open = false;
            }
        });

        if replace_all {
            let count = n;
            let with = self.find.with.clone();
            self.history.cut = true;
            for &(a, b) in found.iter().rev() {
                self.source.replace_range(a..b, &with);
            }
            let ctx = ui.ctx().clone();
            self.notify(&ctx, format!("{count} remplacement(s)"), false);
        } else if replace_one && let Some(&(a, b)) = found.get(self.find.current) {
            self.history.cut = true;
            self.source.replace_range(a..b, &self.find.with);
            go = Some(self.find.current);
        }
        if let Some(i) = go {
            let found = editor::find_all(&self.source, &self.find.query, self.find.case);
            if let Some(&(a, b)) = found.get(i) {
                self.find.current = i;
                let sel = (
                    editor::char_of(&self.source, a),
                    editor::char_of(&self.source, b),
                );
                self.sel = sel;
                self.pending_sel = Some(sel);
                self.tab = Tab::Text;
            }
        }
    }

    // ---------------- Texte ----------------

    fn text_pane(&mut self, ui: &mut egui::Ui, mobile: bool) {
        let id = egui::Id::unique("varium-source");
        if let Some((a, b)) = self.pending_sel.take() {
            let mut st = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
            st.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::new(CharIndex(a)),
                CCursor::new(CharIndex(b)),
            )));
            st.store(ui.ctx(), id);
            ui.memory_mut(|m| m.request_focus(id));
        }
        let pal = Palette::new(ui.visuals().dark_mode);
        let found = if self.find.open {
            editor::find_all(&self.source, &self.find.query, self.find.case)
        } else {
            Vec::new()
        };
        let font = self.page.font;
        let size = if mobile { 17.0 } else { 16.0 };
        // Remarques de la révision, soulignées dans le texte quand l'onglet est ouvert.
        let issues: Vec<(usize, usize)> = if self.prefs.inspector == Inspector::Review {
            varium_core::check::check(&self.source, self.page.langue)
                .into_iter()
                .map(|i| (i.start, i.end))
                .collect()
        } else {
            Vec::new()
        };
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
            let mut job = editor::highlight(buf.as_str(), &pal, font, size, &found, &issues);
            job.wrap.max_width = wrap;
            ui.ctx().fonts_mut(|f| f.layout_job(job))
        };
        let bg = if ui.visuals().dark_mode {
            Color32::from_rgb(0x22, 0x22, 0x24)
        } else {
            Color32::from_rgb(0xFF, 0xFF, 0xFE)
        };
        egui::ScrollArea::vertical()
            .id_salt("source-scroll")
            .auto_shrink(false)
            .show(ui, |ui| {
                egui::Frame::new()
                    .fill(bg)
                    .inner_margin(if mobile {
                        egui::Margin::symmetric(12, 12)
                    } else {
                        egui::Margin::symmetric(28, 22)
                    })
                    .stroke(Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
                    .show(ui, |ui| {
                        let rows = ((ui.available_height() - 40.0) / 22.0).max(8.0) as usize;
                        let out = egui::TextEdit::multiline(&mut self.source)
                            .id(id)
                            .frame(egui::Frame::NONE)
                            .hint_text("Écrivez votre document. {{prénom}} crée un champ, **gras**, # Titre, - liste…")
                            .desired_width(f32::INFINITY)
                            .desired_rows(rows)
                            .lock_focus(true)
                            .layouter(&mut layouter)
                            .show(ui);
                        if let Some(r) = out.cursor_range {
                            let rr = r.as_sorted_char_range();
                            self.sel = (rr.start.0, rr.end.0);
                        }
                    });
            });
    }

    // ---------------- Champs ----------------

    /// L'inspecteur : Format, Document, Champs et Plan, comme la barre latérale de Pages.
    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let w = (ui.available_width() - 8.0) / 5.0;
            for (tab, label) in [
                (Inspector::Format, "Format"),
                (Inspector::Document, "Document"),
                (Inspector::Fields, "Champs"),
                (Inspector::Outline, "Plan"),
                (Inspector::Review, "Révision"),
            ] {
                let b = egui::Button::selectable(
                    self.prefs.inspector == tab,
                    RichText::new(label).size(12.5),
                )
                .min_size(Vec2::new(w, 26.0));
                if ui.add(b).clicked() {
                    self.prefs.inspector = tab;
                }
            }
        });
        ui.separator();
        match self.prefs.inspector {
            Inspector::Fields => self.fields_pane(ui),
            Inspector::Format => {
                egui::ScrollArea::vertical()
                    .id_salt("insp-format")
                    .auto_shrink(false)
                    .show(ui, |ui| self.format_tab(ui));
            }
            Inspector::Document => {
                egui::ScrollArea::vertical()
                    .id_salt("insp-doc")
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Mise en page").strong());
                        page_form(ui, &mut self.page, "insp-page");
                        ui.add_space(8.0);
                        ui.label(RichText::new("Document").strong());
                        let eff = self.effective();
                        let st = stats(&varium_core::document(&self.source, &eff));
                        egui::Grid::new("insp-stats").num_columns(2).show(ui, |ui| {
                            for (k, v) in [
                                ("Pages", self.pages.to_string()),
                                ("Mots", st.words.to_string()),
                                ("Caractères", st.chars.to_string()),
                                ("Paragraphes", st.paragraphs.to_string()),
                            ] {
                                ui.label(RichText::new(k).weak());
                                ui.label(v);
                                ui.end_row();
                            }
                        });
                    });
            }
            Inspector::Outline => self.outline_tab(ui),
            Inspector::Review => self.review_tab(ui),
        }
    }

    fn format_tab(&mut self, ui: &mut egui::Ui) {
        let style = editor::line_style_at(&self.source, self.sel.1);
        ui.label(RichText::new("Style de paragraphe").strong());
        for st in [
            LineStyle::Heading(1),
            LineStyle::Heading(2),
            LineStyle::Heading(3),
            LineStyle::Body,
            LineStyle::Quote,
            LineStyle::Bullet,
            LineStyle::Numbered,
        ] {
            let (size, strong) = match st {
                LineStyle::Heading(1) => (20.0, true),
                LineStyle::Heading(2) => (17.0, true),
                LineStyle::Heading(_) => (15.0, true),
                _ => (14.0, false),
            };
            let mut t = RichText::new(st.label()).size(size);
            if strong {
                t = t.strong();
            }
            if st == LineStyle::Quote {
                t = t.italics();
            }
            let on = style == st
                || (st == LineStyle::Body && matches!(style, LineStyle::Center | LineStyle::Right));
            if ui
                .add(egui::Button::selectable(on, t).min_size(Vec2::new(ui.available_width(), 0.0)))
                .clicked()
            {
                self.queue.push(Act::Style(st));
            }
        }
        ui.add_space(8.0);
        ui.label(RichText::new("Police").strong());
        ui.horizontal(|ui| {
            self.tool(ui, Icon::Bold, "Gras", Act::Wrap("**"), false);
            self.tool(ui, Icon::Italic, "Italique", Act::Wrap("*"), false);
            self.tool(ui, Icon::Underline, "Souligné", Act::Wrap("__"), false);
            self.tool(ui, Icon::Strike, "Barré", Act::Wrap("~~"), false);
        });
        egui::ComboBox::from_id_salt("insp-font")
            .selected_text(self.page.font.label())
            .show_ui(ui, |ui| {
                for f in [DocFont::Serif, DocFont::Sans] {
                    ui.selectable_value(&mut self.page.font, f, f.label());
                }
            });
        ui.add(
            egui::DragValue::new(&mut self.page.font_size)
                .range(8.0..=24.0)
                .speed(0.5)
                .custom_formatter(|v, _| format!("{} pt", fr(v))),
        );
        ui.add_space(8.0);
        ui.label(RichText::new("Alignement").strong());
        ui.horizontal(|ui| {
            let side = !matches!(style, LineStyle::Center | LineStyle::Right);
            self.tool(
                ui,
                Icon::AlignLeft,
                "À gauche",
                Act::Style(LineStyle::Body),
                side,
            );
            self.tool(
                ui,
                Icon::AlignCenter,
                "Centré",
                Act::Style(LineStyle::Center),
                style == LineStyle::Center,
            );
            self.tool(
                ui,
                Icon::AlignRight,
                "À droite",
                Act::Style(LineStyle::Right),
                style == LineStyle::Right,
            );
            ui.checkbox(&mut self.page.justify, "Justifier");
        });
        ui.add_space(8.0);
        ui.label(RichText::new("Espacement").strong());
        ui.horizontal(|ui| {
            for (v, l) in [(1.0, "1"), (1.15, "1,15"), (1.5, "1,5"), (2.0, "2")] {
                if ui
                    .selectable_label((self.page.line_spacing - v).abs() < 0.01, l)
                    .clicked()
                {
                    self.page.line_spacing = v;
                }
            }
        });
        ui.add_space(8.0);
        ui.label(RichText::new("Insérer").strong());
        ui.horizontal_wrapped(|ui| {
            for (label, act) in [
                ("{ } Champ", Act::InsertField),
                ("= Champ calculé", Act::InsertFormula),
                ("📅 Date du jour", Act::InsertToday),
                ("▦ Tableau", Act::InsertTable),
                ("⤓ Saut de page", Act::InsertPageBreak),
                ("— Ligne", Act::InsertRule),
            ] {
                if ui.button(label).clicked() {
                    self.queue.push(act);
                }
            }
        });
        ui.add_space(8.0);
        ui.checkbox(
            &mut self.page.typographie,
            "Typographie française automatique",
        )
        .on_hover_text(
            "« guillemets », apostrophe ’, points de suspension …, espaces fines avant ; : ! ?",
        );
    }

    /// Révision : remarques d'écriture selon la langue du document.
    fn review_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Langue");
            egui::ComboBox::from_id_salt("review-lang")
                .selected_text(self.page.langue.native())
                .show_ui(ui, |ui| {
                    for l in varium_core::Lang::ALL {
                        ui.selectable_value(&mut self.page.langue, l, l.native());
                    }
                });
            if ui
                .small_button("Règles…")
                .on_hover_text("Les règles d'écriture de cette langue")
                .clicked()
            {
                self.queue.push(Act::Languages);
            }
        });
        let issues = varium_core::check::check(&self.source, self.page.langue);
        let fixable = issues.iter().filter(|i| i.replacement.is_some()).count();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(match issues.len() {
                    0 => "Aucune remarque ✔".to_string(),
                    1 => "1 remarque".to_string(),
                    n => format!("{n} remarques"),
                })
                .strong(),
            );
            if fixable > 0 && ui.button(format!("Tout corriger ({fixable})")).clicked() {
                self.history.cut = true;
                let n = varium_core::check::apply_all(&mut self.source, self.page.langue);
                let ctx = ui.ctx().clone();
                self.notify(&ctx, format!("{n} correction(s) appliquée(s)"), false);
            }
        });
        ui.label(
            RichText::new("Les remarques sont soulignées en rouge dans le texte. Les champs et les formules ne sont pas examinés.")
                .small()
                .weak(),
        );
        ui.separator();
        let mut apply: Option<varium_core::check::Issue> = None;
        let mut goto: Option<(usize, usize)> = None;
        egui::ScrollArea::vertical()
            .id_salt("review")
            .auto_shrink(false)
            .show(ui, |ui| {
                for (k, issue) in issues.iter().enumerate().take(300) {
                    egui::Frame::group(ui.style())
                        .inner_margin(6.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new(issue.kind.label()).small().color(err(ui)));
                                ui.label(&issue.message);
                            });
                            // Le contexte : quelques caractères autour de la remarque.
                            let a = self.source[..issue.start]
                                .char_indices()
                                .rev()
                                .nth(18)
                                .map_or(0, |(i, _)| i);
                            let b = self.source[issue.end..]
                                .char_indices()
                                .nth(18)
                                .map_or(self.source.len(), |(i, _)| issue.end + i);
                            let mut job = egui::text::LayoutJob::default();
                            let base = egui::TextFormat {
                                color: ui.visuals().weak_text_color(),
                                ..Default::default()
                            };
                            job.append(
                                &self.source[a..issue.start].replace('\n', "⏎"),
                                0.0,
                                base.clone(),
                            );
                            job.append(
                                &self.source[issue.start..issue.end]
                                    .replace(' ', "·")
                                    .replace('\n', "⏎"),
                                0.0,
                                egui::TextFormat {
                                    color: ui.visuals().text_color(),
                                    background: Color32::from_rgba_unmultiplied(
                                        0xE0, 0x3A, 0x30, 50,
                                    ),
                                    ..Default::default()
                                },
                            );
                            job.append(&self.source[issue.end..b].replace('\n', "⏎"), 0.0, base);
                            ui.label(job);
                            ui.horizontal(|ui| {
                                if let Some(rep) = &issue.replacement
                                    && ui
                                        .small_button(format!(
                                            "Corriger : « {} »",
                                            rep.replace(' ', "·")
                                        ))
                                        .clicked()
                                {
                                    apply = Some(issue.clone());
                                }
                                if ui.small_button("Aller").clicked() {
                                    goto = Some((issue.start, issue.end));
                                }
                            });
                        });
                    if k + 1 == 300 {
                        ui.label(RichText::new("… (300 premières remarques)").weak());
                    }
                }
            });
        if let Some(i) = apply {
            self.history.cut = true;
            varium_core::check::apply_fix(&mut self.source, &i);
        }
        if let Some((a, b)) = goto {
            let sel = (
                editor::char_of(&self.source, a),
                editor::char_of(&self.source, b),
            );
            self.sel = sel;
            self.pending_sel = Some(sel);
            self.tab = Tab::Text;
            if self.prefs.layout == Layout::Page {
                self.prefs.layout = Layout::Split;
            }
        }
    }

    fn outline_tab(&mut self, ui: &mut egui::Ui) {
        let plan = editor::outline(&self.source);
        let eff = self.effective();
        ui.label(
            RichText::new(format!(
                "{} page(s) · {} titre(s)",
                self.pages.max(1),
                plan.len()
            ))
            .weak(),
        );
        ui.add_space(4.0);
        if plan.is_empty() {
            ui.label(
                RichText::new("Pas encore de titre. Choisissez « Titre 1 » dans l'onglet Format, ou écrivez # au début d'une ligne.")
                    .weak(),
            );
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("insp-outline")
            .auto_shrink(false)
            .show(ui, |ui| {
                for h in plan {
                    ui.horizontal(|ui| {
                        ui.add_space(14.0 * f32::from(h.level.saturating_sub(1)));
                        // Le titre tel qu'il s'affiche, champs remplis.
                        let shown =
                            varium_core::doc::plain(&varium_core::doc::inline(&h.title, &eff));
                        let t = if h.level == 1 {
                            RichText::new(shown).strong()
                        } else {
                            RichText::new(shown)
                        };
                        if ui.link(t).on_hover_text("Aller à ce titre").clicked() {
                            self.sel = h.sel;
                            self.pending_sel = Some(h.sel);
                            self.tab = Tab::Text;
                            if self.prefs.layout == Layout::Page {
                                self.prefs.layout = Layout::Split;
                            }
                        }
                    });
                }
            });
    }

    fn fields_pane(&mut self, ui: &mut egui::Ui) {
        let vars = varium_core::variables(&self.source);
        let eff = self.effective();
        let filled = vars.iter().filter(|v| !eff.get(&v.name).is_empty()).count();

        ui.horizontal(|ui| {
            ui.heading("Champs");
            ui.label(
                RichText::new(vars.len().to_string())
                    .strong()
                    .color(accent(ui)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("+ Champ")
                    .on_hover_text("Insérer un champ dans le texte")
                    .clicked()
                {
                    self.queue.push(Act::InsertField);
                }
            });
        });
        if !vars.is_empty() {
            ui.add(
                egui::ProgressBar::new(filled as f32 / vars.len() as f32)
                    .text(format!("{filled} / {} renseignés", vars.len()))
                    .desired_height(14.0),
            );
        }
        if self.merge.is_some() {
            self.merge_banner(ui);
        }
        ui.add_space(6.0);

        let mut action: Option<VarAction> = None;
        egui::ScrollArea::vertical()
            .id_salt("vars-scroll")
            .auto_shrink(false)
            .show(ui, |ui| {
                if vars.is_empty() {
                    ui.label(
                        RichText::new("Aucun champ. Écrivez {{prénom}} dans le texte, ou utilisez « + Champ ».").weak(),
                    );
                }
                for info in &vars {
                    egui::Frame::group(ui.style()).inner_margin(8.0).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if let Some(a) = self.variable_card(ui, &info.name, info.count) {
                            action = Some(a);
                        }
                    });
                    ui.add_space(4.0);
                }

                let formulas = varium_core::formulas(&self.source);
                if !formulas.is_empty() {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Champs calculés").strong());
                    for f in formulas {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(format!("= {f}")).monospace().color(formula_color(ui)));
                            match eff.compute(&f) {
                                Ok(n) => ui.label(RichText::new(format!("→ {}", filters::format_number(n, None))).weak()),
                                Err(e) => ui.label(RichText::new(e).small().color(err(ui))),
                            };
                        });
                    }
                }

                ui.add_space(8.0);
                egui::CollapsingHeader::new("Filtres disponibles").show(ui, |ui| {
                    egui::Grid::new("filters").num_columns(2).striped(true).show(ui, |ui| {
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

        let ctx = ui.ctx().clone();
        match action {
            Some(VarAction::Rename { from, to }) => {
                match varium_core::rename_variable(&self.source, &from, &to) {
                    Ok(s) => {
                        self.history.cut = true;
                        self.source = s;
                        if let Some(v) = self.values.values.remove(&from) {
                            self.values.values.insert(to.clone(), v);
                        }
                        if let Some(t) = self.values.types.remove(&from) {
                            self.values.types.insert(to.clone(), t);
                        }
                        self.rename = None;
                        self.notify(
                            &ctx,
                            format!("« {from} » renommé en « {to} » partout"),
                            false,
                        );
                    }
                    Err(e) => self.notify(&ctx, e, true),
                }
            }
            Some(VarAction::Inline(name)) => {
                self.history.cut = true;
                self.source = varium_core::inline_variable(&self.source, &name, &self.effective());
                self.notify(&ctx, format!("« {name} » figé dans le texte"), false);
            }
            None => {}
        }
    }

    fn merge_banner(&mut self, ui: &mut egui::Ui) {
        let Some(m) = self.merge.as_mut() else { return };
        let n = m.sheet.rows.len();
        let mut stop = false;
        egui::Frame::new()
            .fill(accent(ui).gamma_multiply(0.15))
            .corner_radius(6.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("✉ Publipostage").strong());
                    if ui.small_button("◀").clicked() && m.record > 0 {
                        m.record -= 1;
                    }
                    ui.label(format!("{} / {n}", m.record + 1));
                    if ui.small_button("▶").clicked() && m.record + 1 < n {
                        m.record += 1;
                    }
                    stop = ui.small_button("Terminer").clicked();
                });
                ui.label(
                    RichText::new(format!("« {} » remplit les champs de même nom.", m.file))
                        .small()
                        .weak(),
                );
            });
        if stop {
            self.merge = None;
        }
    }

    fn variable_card(&mut self, ui: &mut egui::Ui, name: &str, count: usize) -> Option<VarAction> {
        let mut action = None;
        let mut ty = self.values.ty(name);
        let from_merge = self
            .merge
            .as_ref()
            .is_some_and(|m| m.sheet.column_for(name).is_some());

        ui.horizontal(|ui| {
            ui.label(RichText::new(name).monospace().strong().color(accent(ui)));
            if count > 1 {
                ui.label(RichText::new(format!("×{count}")).small().weak())
                    .on_hover_text(format!("{count} occurrences dans le document"));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let has_value = !self.values.get(name).is_empty();
                if ui
                    .add_enabled(has_value, egui::Button::new("⤓").small())
                    .on_hover_text("Figer : remplacer le champ par sa valeur dans le texte")
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
            });
        });

        if let Some(r) = self.rename.as_mut().filter(|r| r.from == name) {
            let mut commit = false;
            let mut cancel = false;
            ui.horizontal_wrapped(|ui| {
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
                    RichText::new(format!("« {to} » existe déjà : les deux seront fusionnés"))
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

        if from_merge {
            let v = self.effective().get(name).to_string();
            ui.label(RichText::new(format!("📄 {v}")).italics())
                .on_hover_text("Valeur lue dans le fichier du publipostage");
        } else {
            let value = self.values.values.entry(name.to_string()).or_default();
            value_editor(ui, name, ty, value);
        }

        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt(("type", name))
                .selected_text(ty.label())
                .width(104.0)
                .show_ui(ui, |ui| {
                    for t in VarType::ALL {
                        ui.selectable_value(&mut ty, t, t.label());
                    }
                });
            let value = self.values.get(name).to_string();
            if let Some(msg) = ty.validate(&value) {
                ui.label(RichText::new(msg).small().color(err(ui)));
            } else if name == TODAY && value.is_empty() {
                ui.label(RichText::new("date du jour, automatique").small().weak());
            } else if matches!(ty, VarType::Date | VarType::Number | VarType::Money)
                && !value.is_empty()
            {
                let shown = self.values.resolve(&varium_core::VarRef {
                    name: name.to_string(),
                    filters: vec![],
                    raw: String::new(),
                    range: 0..0,
                    formula: None,
                });
                if let Some(s) = shown {
                    ui.label(RichText::new(format!("→ {s}")).small().weak());
                }
            }
        });
        if ty != self.values.ty(name) {
            self.values.types.insert(name.to_string(), ty);
        }
        action
    }

    // ---------------- Page ----------------

    fn page_pane(&mut self, ui: &mut egui::Ui) {
        let eff = self.effective();
        let r = export::prepare(&self.source, &eff, &self.page);
        let desk = if ui.visuals().dark_mode {
            Color32::from_rgb(0x2B, 0x2B, 0x2E)
        } else {
            Color32::from_rgb(0xD9, 0xD6, 0xD0)
        };
        egui::Frame::new().fill(desk).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            egui::ScrollArea::both()
                .id_salt("page-scroll")
                .auto_shrink(false)
                .show(ui, |ui| {
                    let fit = ((ui.available_width() - 40.0) / pageview::page_width_px(&self.page))
                        .clamp(0.25, 3.0);
                    let mut zoom = self.prefs.zoom.unwrap_or(fit.min(1.6));
                    // Pincer (téléphone) ou Ctrl + molette (ordinateur) pour zoomer.
                    let delta = ui.input(|i| i.zoom_delta());
                    if (delta - 1.0).abs() > 0.001 && ui.rect_contains_pointer(ui.max_rect()) {
                        zoom = (zoom * delta).clamp(0.3, 3.0);
                        self.prefs.zoom = Some(zoom);
                    }
                    self.pages = pageview::show(
                        ui,
                        &View {
                            paras: &r.paras,
                            header: &r.header,
                            footer: &r.footer,
                            page: &self.page,
                            zoom,
                            highlight: self.prefs.highlight,
                        },
                    );
                });
        });
    }

    // ---------------- Barre d'état ----------------

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let eff = self.effective();
        let doc = varium_core::document(&self.source, &eff);
        let st = stats(&doc);
        let vars = varium_core::variables(&self.source);
        let filled = vars.iter().filter(|v| !eff.get(&v.name).is_empty()).count();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let pages = self.pages.max(1);
            ui.label(format!("{pages} page{}", if pages > 1 { "s" } else { "" }));
            ui.separator();
            ui.label(format!("{} mots, {} caractères", st.words, st.chars))
                .on_hover_text("Outils → Nombre de mots");
            ui.separator();
            ui.label(format!("{filled}/{} champs", vars.len()));
            if let Some(m) = &self.merge {
                ui.separator();
                ui.label(format!("✉ {}/{}", m.record + 1, m.sheet.rows.len()));
            }
            ui.separator();
            ui.label(match self.page.paper {
                Paper::A4 => "A4",
                Paper::A5 => "A5",
                Paper::Letter => "Lettre US",
                Paper::Legal => "Légal US",
            });
            ui.label(self.page.langue.label());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = match self.prefs.zoom {
                    None => "Largeur".to_string(),
                    Some(z) => format!("{} %", (z * 100.0).round() as i32),
                };
                if ui.small_button("+").clicked() {
                    self.queue.push(Act::ZoomIn);
                }
                let mut z = self.prefs.zoom.unwrap_or(1.0);
                if ui
                    .add(egui::Slider::new(&mut z, 0.3..=3.0).show_value(false))
                    .changed()
                {
                    self.prefs.zoom = Some(z);
                }
                if ui.small_button("−").clicked() {
                    self.queue.push(Act::ZoomOut);
                }
                if ui
                    .selectable_label(self.prefs.zoom.is_none(), label)
                    .on_hover_text("Ajuster à la largeur")
                    .clicked()
                {
                    self.queue.push(Act::ZoomFit);
                }
            });
        });
    }

    // ---------------- Dialogues ----------------

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
        let mut keep = true;
        let modal = egui::Modal::new(egui::Id::unique("varium-dialog")).show(ctx, |ui| {
            let max_w = (ctx.content_rect().width() - 40.0).clamp(260.0, 640.0);
            ui.set_max_width(max_w);
            match &mut dialog {
                Dialog::Templates => keep = self.templates_dialog(ui),
                Dialog::PageSetup => keep = self.page_dialog(ui),
                Dialog::Merge => keep = self.merge_dialog(ui),
                Dialog::Table { rows, cols } => {
                    ui.heading("Insérer un tableau");
                    egui::Grid::new("tbl").num_columns(2).show(ui, |ui| {
                        ui.label("Colonnes");
                        ui.add(egui::DragValue::new(cols).range(1..=12));
                        ui.end_row();
                        ui.label("Lignes (avec l'en-tête)");
                        ui.add(egui::DragValue::new(rows).range(2..=50));
                        ui.end_row();
                    });
                    ui.label(RichText::new("Écrivez dans les cellules entre les « | ».").small().weak());
                    ui.horizontal(|ui| {
                        if ui.button("Insérer").clicked() {
                            let t = editor::table_markup(*rows, *cols);
                            self.insert_block(t.trim_end());
                            keep = false;
                        }
                        if ui.button("Annuler").clicked() {
                            keep = false;
                        }
                    });
                }
                Dialog::Field { name, ty } => {
                    ui.heading("Insérer un champ");
                    ui.label("Le champ prend la même valeur partout où il apparaît.");
                    let r = ui.add(
                        egui::TextEdit::singleline(name)
                            .hint_text("prénom, montant, date_rdv…")
                            .desired_width(f32::INFINITY),
                    );
                    r.request_focus();
                    egui::ComboBox::from_label("Type")
                        .selected_text(ty.label())
                        .show_ui(ui, |ui| {
                            for t in VarType::ALL {
                                ui.selectable_value(ty, t, t.label());
                            }
                        });
                    let existing = varium_core::variables(&self.source);
                    if !existing.is_empty() {
                        ui.label(RichText::new("Ou reprendre un champ existant :").small().weak());
                        ui.horizontal_wrapped(|ui| {
                            for v in existing.iter().take(24) {
                                if ui.small_button(&v.name).clicked() {
                                    *name = v.name.clone();
                                    *ty = self.values.ty(&v.name);
                                }
                            }
                        });
                    }
                    let n = name.trim().to_string();
                    let valid = varium_core::is_valid_name(&n);
                    if !n.is_empty() && !valid {
                        ui.label(RichText::new("Lettres, chiffres, _ . - (sans espace)").small().color(err(ui)));
                    }
                    ui.horizontal(|ui| {
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        if (ui.add_enabled(valid, egui::Button::new("Insérer")).clicked() || (enter && valid)) && valid {
                            if *ty != VarType::Text {
                                self.values.types.insert(n.clone(), *ty);
                            }
                            self.insert(&format!("{{{{{n}}}}}"), None);
                            keep = false;
                        }
                        if ui.button("Annuler").clicked() {
                            keep = false;
                        }
                    });
                }
                Dialog::Formula { expr } => {
                    ui.heading("Insérer un champ calculé");
                    ui.label("Opérations + − × ÷, parenthèses, pourcentages (20%) et fonctions arrondi(x; 2), min, max, somme.");
                    let r = ui.add(
                        egui::TextEdit::singleline(expr)
                            .hint_text("quantité * prix_unitaire")
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY),
                    );
                    r.request_focus();
                    let names = self.variable_names();
                    if !names.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            for v in names.iter().take(24) {
                                if ui.small_button(v).clicked() {
                                    if !expr.is_empty() && !expr.ends_with(' ') {
                                        expr.push(' ');
                                    }
                                    expr.push_str(v);
                                }
                            }
                        });
                    }
                    let res = self.effective().compute(expr);
                    match &res {
                        Ok(n) => ui.label(format!("Résultat avec les valeurs actuelles : {}", filters::format_number(*n, None))),
                        Err(e) if !expr.trim().is_empty() => ui.label(RichText::new(e).color(warn(ui))),
                        _ => ui.label(""),
                    };
                    ui.horizontal(|ui| {
                        let ok = !expr.trim().is_empty();
                        if ui.add_enabled(ok, egui::Button::new("Insérer")).clicked() {
                            self.insert(&format!("{{{{= {}}}}}", expr.trim()), None);
                            keep = false;
                        }
                        if ui.add_enabled(ok, egui::Button::new("Insérer en euros")).clicked() {
                            self.insert(&format!("{{{{= {} | money}}}}", expr.trim()), None);
                            keep = false;
                        }
                        if ui.button("Annuler").clicked() {
                            keep = false;
                        }
                    });
                }
                Dialog::Stats => {
                    let eff = self.effective();
                    let st = stats(&varium_core::document(&self.source, &eff));
                    ui.heading("Nombre de mots");
                    egui::Grid::new("stats").num_columns(2).striped(true).show(ui, |ui| {
                        for (k, v) in [
                            ("Pages", self.pages.to_string()),
                            ("Mots", st.words.to_string()),
                            ("Caractères (espaces comprises)", st.chars.to_string()),
                            ("Paragraphes", st.paragraphs.to_string()),
                            ("Champs", varium_core::variables(&self.source).len().to_string()),
                            ("Champs calculés", varium_core::formulas(&self.source).len().to_string()),
                        ] {
                            ui.label(k);
                            ui.label(RichText::new(v).strong());
                            ui.end_row();
                        }
                    });
                    keep = !ui.button("Fermer").clicked();
                }
                Dialog::Shortcuts => {
                    ui.heading("Raccourcis clavier");
                    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                        egui::Grid::new("sc").num_columns(2).striped(true).show(ui, |ui| {
                            for (s, a) in SHORTCUTS {
                                ui.label(RichText::new(ctx.format_shortcut(s)).monospace());
                                ui.label(act_label(*a));
                                ui.end_row();
                            }
                        });
                    });
                    keep = !ui.button("Fermer").clicked();
                }
                Dialog::Languages(lang) => {
                    ui.heading("Écrire en plusieurs langues");
                    ui.label("Choisissez la langue du document (Document → Langue) : les dates, les nombres, les montants, la typographie automatique et la révision suivent ses règles.");
                    ui.horizontal_wrapped(|ui| {
                        for l in varium_core::Lang::ALL {
                            ui.selectable_value(lang, l, l.native());
                        }
                    });
                    ui.separator();
                    egui::Grid::new("lang-rules").num_columns(2).striped(true).show(ui, |ui| {
                        for (k, v) in lang.rules() {
                            ui.label(RichText::new(*k).strong());
                            ui.label(*v);
                            ui.end_row();
                        }
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button(format!("Écrire ce document en {}", lang.native())).clicked() {
                            self.page.langue = *lang;
                            keep = false;
                        }
                        if ui.button("Fermer").clicked() {
                            keep = false;
                        }
                    });
                }
                Dialog::Syntax => {
                    ui.heading("Guide de la syntaxe");
                    egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                        egui::Grid::new("syn").num_columns(2).striped(true).show(ui, |ui| {
                            for (k, v) in SYNTAX {
                                ui.label(RichText::new(*k).monospace());
                                ui.label(*v);
                                ui.end_row();
                            }
                        });
                    });
                    keep = !ui.button("Fermer").clicked();
                }
                Dialog::Confirm(p) => {
                    ui.heading("Enregistrer les modifications ?");
                    ui.label(format!(
                        "« {} » contient des modifications non enregistrées.",
                        self.title.trim()
                    ));
                    ui.label(RichText::new("Sans enregistrement, elles seront perdues.").weak());
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Enregistrer").clicked() {
                            keep = false;
                            match self.save_project(ctx, false) {
                                SaveOutcome::Done => self.proceed(ctx, p.clone()),
                                SaveOutcome::Asking => self.after_save = Some(p.clone()),
                                SaveOutcome::Failed => {}
                            }
                        }
                        if ui.button("Ne pas enregistrer").clicked() {
                            keep = false;
                            self.proceed(ctx, p.clone());
                        }
                        if ui.button("Annuler").clicked() {
                            keep = false;
                        }
                    });
                }
                Dialog::Restore {
                    project,
                    secours,
                    document,
                } => {
                    ui.heading("Restaurer la sauvegarde de secours ?");
                    ui.label(format!(
                        "Varium s'est arrêté sans que « {} » soit enregistré. Une sauvegarde automatique plus récente existe.",
                        if project.title.trim().is_empty() {
                            "le document"
                        } else {
                            project.title.trim()
                        }
                    ));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Restaurer").clicked() {
                            keep = false;
                            let p = (**project).clone();
                            self.restaurer(p, document.clone());
                            secours::effacer(secours);
                        }
                        if ui.button("Ignorer").clicked() {
                            keep = false;
                            secours::effacer(secours);
                        }
                    });
                }
                Dialog::About => {
                    ui.heading("Varium");
                    ui.label(RichText::new("Un document. Toutes ses possibilités.").italics());
                    ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    ui.label("Éditeur de documents à champs, écrit en Rust avec egui. Polices Liberation (licence SIL OFL 1.1).");
                    ui.hyperlink_to("Code source", "https://github.com/maxlestage/VARIUM");
                    ui.hyperlink_to("Site de Varium", "https://maxlestage.github.io/VARIUM/");
                    keep = !ui.button("Fermer").clicked();
                }
            }
        });
        if modal.should_close() {
            keep = false;
        }
        if keep && self.dialog.is_none() {
            self.dialog = Some(dialog);
        }
    }

    fn templates_dialog(&mut self, ui: &mut egui::Ui) -> bool {
        ui.heading("Nouveau à partir d'un modèle");
        ui.label(
            RichText::new("Le document actuel reste récupérable avec Annuler (Ctrl+Z).")
                .small()
                .weak(),
        );
        ui.add_space(6.0);
        let mut chosen = None;
        let cols = if ui.available_width() < 420.0 { 1 } else { 2 };
        egui::ScrollArea::vertical()
            .max_height(440.0)
            .show(ui, |ui| {
                let w = ((ui.available_width() - 8.0 * (cols as f32 - 1.0)) / cols as f32 - 22.0)
                    .max(180.0);
                for (row, chunk) in templates::ALL.chunks(cols).enumerate() {
                    ui.horizontal_top(|ui| {
                        for (k, t) in chunk.iter().enumerate() {
                            let r = egui::Frame::group(ui.style())
                                .inner_margin(10.0)
                                .show(ui, |ui| {
                                    ui.set_width(w);
                                    ui.set_min_height(40.0);
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(t.icon).size(22.0));
                                        ui.vertical(|ui| {
                                            ui.label(RichText::new(t.name).strong());
                                            ui.label(RichText::new(t.description).small().weak());
                                        });
                                    });
                                })
                                .response
                                .interact(Sense::click())
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            if r.clicked() {
                                chosen = Some(row * cols + k);
                            }
                        }
                    });
                }
            });
        ui.add_space(6.0);
        let mut keep = true;
        ui.horizontal(|ui| {
            if ui.button("Document vierge").clicked() {
                self.queue.push(Act::NewBlank);
                keep = false;
            }
            if ui.button("Annuler").clicked() {
                keep = false;
            }
        });
        if let Some(i) = chosen {
            self.load(templates::ALL[i].project());
            let ctx = ui.ctx().clone();
            self.notify(
                &ctx,
                format!("Modèle « {} » ouvert", templates::ALL[i].name),
                false,
            );
            return false;
        }
        keep
    }

    fn page_dialog(&mut self, ui: &mut egui::Ui) -> bool {
        ui.heading("Mise en page");
        page_form(ui, &mut self.page, "page");
        ui.add_space(6.0);
        let mut keep = true;
        ui.horizontal(|ui| {
            if ui.button("OK").clicked() {
                keep = false;
            }
            if ui.button("Par défaut").clicked() {
                self.page = PageSetup::default();
            }
        });
        keep
    }

    fn merge_dialog(&mut self, ui: &mut egui::Ui) -> bool {
        ui.heading("Publipostage");
        ui.label("Un document par ligne d'un tableau CSV (Excel, LibreOffice Calc, Google Sheets…). La première ligne donne les noms des colonnes : une colonne remplit le champ de même nom.");
        ui.add_space(4.0);
        let mut keep = true;
        let ctx = ui.ctx().clone();
        let names = self.variable_names();
        match &mut self.merge {
            None => {
                if ui.button("📂 Choisir un fichier CSV…").clicked() {
                    self.io.open(&ctx, OpenKind::Data);
                }
                ui.label(
                    RichText::new("Exemple : prénom;nom;ville")
                        .small()
                        .monospace()
                        .weak(),
                );
            }
            Some(m) => {
                let n = m.sheet.rows.len();
                ui.label(
                    RichText::new(format!(
                        "« {} » : {n} destinataire(s), {} colonne(s)",
                        m.file,
                        m.sheet.headers.len()
                    ))
                    .strong(),
                );
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        egui::Grid::new("cols")
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                for v in &names {
                                    ui.label(RichText::new(v).monospace());
                                    match m.sheet.column_for(v) {
                                        Some(c) => ui
                                            .label(format!("✔ colonne « {} »", m.sheet.headers[c])),
                                        None => {
                                            ui.label(RichText::new("valeur du document").weak())
                                        }
                                    };
                                    ui.end_row();
                                }
                            });
                    });
                ui.horizontal(|ui| {
                    ui.label("Aperçu :");
                    if ui.small_button("◀").clicked() && m.record > 0 {
                        m.record -= 1;
                    }
                    ui.label(format!("{} / {n}", m.record + 1));
                    if ui.small_button("▶").clicked() && m.record + 1 < n {
                        m.record += 1;
                    }
                });
                ui.separator();
                ui.label(format!(
                    "Créer les {n} documents dans un seul fichier, une page par destinataire :"
                ));
                ui.horizontal_wrapped(|ui| {
                    if ui.button("LibreOffice (.odt)").clicked() {
                        self.queue.push(Act::Export(Format::Odt));
                    }
                    if ui.button("Word (.docx)").clicked() {
                        self.queue.push(Act::Export(Format::Docx));
                    }
                    if ui.button("PDF").clicked() {
                        self.queue.push(Act::Export(Format::Pdf));
                    }
                    if ui.button("HTML").clicked() {
                        self.queue.push(Act::Export(Format::Html));
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Autre fichier…").clicked() {
                        self.io.open(&ctx, OpenKind::Data);
                    }
                    if ui.button("Terminer le publipostage").clicked() {
                        self.merge = None;
                        keep = false;
                    }
                });
            }
        }
        if ui.button("Fermer").clicked() {
            keep = false;
        }
        keep
    }

    fn toast(&mut self, ctx: &egui::Context, bottom: f32) {
        let Some((msg, at, error)) = self.toast.clone() else {
            return;
        };
        let now = ctx.input(|i| i.time);
        if now - at > 4.0 {
            self.toast = None;
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
        egui::Area::new(egui::Id::unique("varium-toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -bottom])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(ctx.content_rect().width() - 40.0);
                    let color = if error {
                        err(ui)
                    } else {
                        ui.visuals().text_color()
                    };
                    ui.label(RichText::new(msg).color(color));
                });
            });
    }

    fn apply_style(&mut self, ctx: &egui::Context, mobile: bool) {
        if self.mobile_style == Some(mobile) {
            return;
        }
        self.mobile_style = Some(mobile);
        ctx.all_styles_mut(|s| {
            if mobile {
                s.spacing.interact_size.y = 34.0;
                s.spacing.button_padding = Vec2::new(10.0, 6.0);
                s.spacing.item_spacing = Vec2::new(8.0, 8.0);
            } else {
                s.spacing.interact_size.y = 22.0;
                s.spacing.button_padding = Vec2::new(6.0, 3.0);
                s.spacing.item_spacing = Vec2::new(8.0, 4.0);
            }
        });
    }

    // ---------------- Dispositions ----------------

    fn desktop(&mut self, ui: &mut egui::Ui) {
        // Sur macOS, les menus sont dans la barre de l'écran : pas de barre dans la fenêtre.
        if !self.has_native_menu() {
            egui::Panel::top("menu").show(ui, |ui| {
                ui.add_space(2.0);
                self.menu_bar(ui);
                ui.add_space(1.0);
            });
        }
        if self.prefs.toolbar {
            egui::Panel::top("toolbar").show(ui, |ui| {
                ui.add_space(3.0);
                egui::ScrollArea::horizontal()
                    .id_salt("tools")
                    .show(ui, |ui| self.toolbar(ui, false));
                ui.add_space(3.0);
            });
        }
        if self.find.open {
            egui::Panel::top("find").show(ui, |ui| {
                ui.add_space(3.0);
                self.find_bar(ui);
                ui.add_space(3.0);
            });
        }
        if self.prefs.status_bar {
            egui::Panel::bottom("status").show(ui, |ui| {
                ui.add_space(2.0);
                self.status_bar(ui);
                ui.add_space(1.0);
            });
        }
        let width = ui.available_width();
        if self.prefs.fields_panel {
            egui::Panel::right("fields")
                .resizable(true)
                .size_range(240.0..=420.0)
                .default_size((width * 0.24).clamp(250.0, 360.0))
                .show(ui, |ui| {
                    ui.add_space(6.0);
                    self.inspector(ui);
                });
        }
        match self.prefs.layout {
            Layout::Split => {
                egui::Panel::left("text")
                    .resizable(true)
                    .default_size((width * 0.38).max(300.0))
                    .show(ui, |ui| {
                        ui.add_space(6.0);
                        self.text_pane(ui, false);
                    });
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| self.page_pane(ui));
            }
            Layout::Page => {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| self.page_pane(ui));
            }
            Layout::Text => {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.add_space(4.0);
                    self.text_pane(ui, false);
                });
            }
        }
    }

    fn mobile(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("m-top").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.menu_button(RichText::new("☰").size(20.0), |ui| {
                    ui.set_min_width(220.0);
                    self.menus(ui);
                });
                ui.label(RichText::new("{ }").monospace().strong().color(accent(ui)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, Icon::Redo, false)
                        .on_hover_text("Rétablir")
                        .clicked()
                    {
                        self.queue.push(Act::Redo);
                    }
                    if icon_button(ui, Icon::Undo, false)
                        .on_hover_text("Annuler")
                        .clicked()
                    {
                        self.queue.push(Act::Undo);
                    }
                    if icon_button(ui, Icon::Pdf, false)
                        .on_hover_text("Exporter en PDF")
                        .clicked()
                    {
                        self.queue.push(Act::Export(Format::Pdf));
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut self.title)
                            .hint_text("Titre")
                            .desired_width(ui.available_width()),
                    );
                });
            });
            ui.add_space(2.0);
        });
        if self.tab == Tab::Text {
            egui::Panel::top("m-tools").show(ui, |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("m-tools-scroll")
                    .show(ui, |ui| self.toolbar(ui, true));
                ui.add_space(2.0);
            });
        }
        if self.find.open {
            egui::Panel::top("m-find").show(ui, |ui| self.find_bar(ui));
        }

        let eff = self.effective();
        let vars = varium_core::variables(&self.source);
        let filled = vars.iter().filter(|v| !eff.get(&v.name).is_empty()).count();
        egui::Panel::bottom("m-tabs").show(ui, |ui| {
            ui.add_space(4.0);
            ui.columns(3, |cols| {
                let pages = self.pages.max(1);
                for (col, (tab, icon, label)) in cols.iter_mut().zip([
                    (Tab::Text, "✏", "Texte".to_string()),
                    (
                        Tab::Fields,
                        "{ }",
                        format!("Champs {filled}/{}", vars.len()),
                    ),
                    (
                        Tab::Page,
                        "📄",
                        format!("Page{} ({pages})", if pages > 1 { "s" } else { "" }),
                    ),
                ]) {
                    let on = self.tab == tab;
                    let text = RichText::new(format!("{icon}\n{label}")).size(13.0);
                    let b = egui::Button::selectable(on, text)
                        .min_size(Vec2::new(col.available_width(), 46.0));
                    if col.add(b).clicked() {
                        self.tab = tab;
                    }
                }
            });
            ui.add_space(2.0);
        });

        egui::CentralPanel::default()
            .frame(if self.tab == Tab::Page {
                egui::Frame::NONE
            } else {
                egui::Frame::central_panel(ui.style()).inner_margin(6.0)
            })
            .show(ui, |ui| match self.tab {
                Tab::Text => self.text_pane(ui, true),
                Tab::Fields => self.fields_pane(ui),
                Tab::Page => self.page_pane(ui),
            });
    }
}

enum VarAction {
    Rename { from: String, to: String },
    Inline(String),
}

impl eframe::App for VariumApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_io(&ctx);

        // Fermeture de la fenêtre : confirmation s'il reste des modifications.
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.is_dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.dialog.is_none() {
                self.dialog = Some(Dialog::Confirm(Pending::Quit));
            }
        }
        let wt = files::window_title(&self.title, self.path.as_deref(), self.is_dirty());
        if wt != self.last_window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(wt.clone()));
            self.last_window_title = wt;
        }

        // Raccourcis (avant les widgets, pour que l'éditeur ne les reçoive pas)
        let now = ctx.input(|i| i.time);
        if self.dialog.is_none() {
            for (s, act) in SHORTCUTS {
                if ctx.input_mut(|i| i.consume_shortcut(s)) {
                    self.push_once(*act, now);
                }
            }
        }
        // Menus natifs de macOS
        #[cfg(target_os = "macos")]
        if let Some(menu) = &self.native {
            for act in menu.poll() {
                self.push_once(act, now);
            }
        }

        let mobile = ctx.content_rect().width() < MOBILE_WIDTH;
        self.apply_style(&ctx, mobile);
        if mobile {
            self.mobile(ui);
        } else {
            self.desktop(ui);
        }
        self.dialogs(&ctx);

        for act in std::mem::take(&mut self.queue) {
            self.run(&ctx, act);
        }
        // Le nombre de pages sert aussi quand la page n'est pas affichée.
        if (mobile && self.tab != Tab::Page) || (!mobile && self.prefs.layout == Layout::Text) {
            let eff = self.effective();
            let r = export::prepare(&self.source, &eff, &self.page);
            self.pages = pageview::count_pages(
                &ctx,
                &View {
                    paras: &r.paras,
                    header: &r.header,
                    footer: &r.footer,
                    page: &self.page,
                    zoom: 1.0,
                    highlight: false,
                },
            );
        }
        let now = ctx.input(|i| i.time);
        self.track(now);
        self.tick_secours(&ctx, now);
        self.toast(&ctx, if mobile { 70.0 } else { 34.0 });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.project());
        eframe::set_value(storage, PREFS_KEY, &self.prefs);
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(5)
    }
}

/// Le formulaire de mise en page (dialogue « Page… » et onglet Document de l'inspecteur).
fn page_form(ui: &mut egui::Ui, p: &mut PageSetup, id: &str) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("Format");
            egui::ComboBox::from_id_salt((id, "paper"))
                .selected_text(p.paper.label())
                .show_ui(ui, |ui| {
                    for x in Paper::ALL {
                        ui.selectable_value(&mut p.paper, x, x.label());
                    }
                });
            ui.end_row();
            ui.label("Orientation");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut p.landscape, false, "Portrait");
                ui.selectable_value(&mut p.landscape, true, "Paysage");
            });
            ui.end_row();
            ui.label("Marges (mm)");
            ui.horizontal_wrapped(|ui| {
                for (l, v) in [
                    ("haut", &mut p.margin_top),
                    ("bas", &mut p.margin_bottom),
                    ("gauche", &mut p.margin_left),
                    ("droite", &mut p.margin_right),
                ] {
                    ui.label(RichText::new(l).small());
                    ui.add(
                        egui::DragValue::new(v)
                            .range(0.0..=60.0)
                            .speed(0.5)
                            .custom_formatter(|v, _| fr(v)),
                    );
                }
            });
            ui.end_row();
            ui.label("Police");
            egui::ComboBox::from_id_salt((id, "font"))
                .selected_text(p.font.label())
                .show_ui(ui, |ui| {
                    for f in [DocFont::Serif, DocFont::Sans] {
                        ui.selectable_value(&mut p.font, f, f.label());
                    }
                });
            ui.end_row();
            ui.label("Taille");
            ui.add(
                egui::DragValue::new(&mut p.font_size)
                    .range(8.0..=24.0)
                    .speed(0.5)
                    .custom_formatter(|v, _| format!("{} pt", fr(v))),
            );
            ui.end_row();
            ui.label("Interligne");
            ui.horizontal(|ui| {
                for (v, l) in [
                    (1.0, "simple"),
                    (1.15, "1,15"),
                    (1.5, "1,5"),
                    (2.0, "double"),
                ] {
                    if ui
                        .selectable_label((p.line_spacing - v).abs() < 0.01, l)
                        .clicked()
                    {
                        p.line_spacing = v;
                    }
                }
            });
            ui.end_row();
            ui.label("Texte");
            ui.checkbox(&mut p.justify, "Justifié");
            ui.end_row();
            ui.label("En-tête");
            ui.add(
                egui::TextEdit::singleline(&mut p.header)
                    .hint_text("ex. {{entreprise}} — Confidentiel"),
            );
            ui.end_row();
            ui.label("Pied de page");
            ui.add(
                egui::TextEdit::singleline(&mut p.footer)
                    .hint_text("ex. **{{entreprise}}** · SIRET {{siret}}"),
            );
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut p.page_numbers, "Numéros de page « Page 1 sur 3 »");
            ui.end_row();
            ui.label("Langue");
            egui::ComboBox::from_id_salt((id, "langue"))
                .selected_text(p.langue.label())
                .show_ui(ui, |ui| {
                    for l in varium_core::Lang::ALL {
                        ui.selectable_value(&mut p.langue, l, l.label());
                    }
                });
            ui.end_row();
            ui.label("Typographie");
            ui.checkbox(&mut p.typographie, "Automatique (selon la langue)")
                    .on_hover_text("« guillemets », apostrophe ’, points de suspension …, espaces fines avant ; : ! ?");
            ui.end_row();
        });
}

/// Nombre à la française : `12,5`, `20`.
fn fr(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0')
        .trim_end_matches('.')
        .replace('.', ",")
}

// ---------------------------------------------------------------------------------
// Saisie d'une valeur
// ---------------------------------------------------------------------------------

fn value_editor(ui: &mut egui::Ui, name: &str, ty: VarType, value: &mut String) {
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
                        *value = today();
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
}

// ---------------------------------------------------------------------------------
// Icônes de la barre d'outils, dessinées (nettes à toutes les tailles)
// ---------------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Icon {
    New,
    Open,
    Save,
    Pdf,
    Odt,
    Undo,
    Redo,
    Find,
    Bold,
    Italic,
    Underline,
    Strike,
    AlignLeft,
    AlignCenter,
    AlignRight,
    Bullets,
    Numbers,
    Field,
    Formula,
    Table,
    PageBreak,
}

fn sep(ui: &mut egui::Ui) {
    ui.add_space(3.0);
    let h = ui.spacing().interact_size.y;
    let (r, _) = ui.allocate_exact_size(Vec2::new(1.0, h), Sense::hover());
    ui.painter().line_segment(
        [r.center_top(), r.center_bottom()],
        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
    );
    ui.add_space(3.0);
}

fn icon_button(ui: &mut egui::Ui, icon: Icon, on: bool) -> egui::Response {
    let side = ui.spacing().interact_size.y.max(26.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    let v = ui.visuals();
    let bg = if on {
        v.selection.bg_fill.gamma_multiply(0.6)
    } else if resp.hovered() {
        v.widgets.hovered.weak_bg_fill
    } else {
        Color32::TRANSPARENT
    };
    let p = ui.painter();
    p.rect_filled(rect, 4.0, bg);
    let ink = v.text_color();
    let s = Stroke::new(1.4, ink);
    let c = rect.center();
    let u = side / 26.0; // unité
    let r = |x0: f32, y0: f32, x1: f32, y1: f32| {
        Rect::from_min_max(c + Vec2::new(x0, y0) * u, c + Vec2::new(x1, y1) * u)
    };
    let pt = |x: f32, y: f32| c + Vec2::new(x, y) * u;
    let lines = |p: &egui::Painter, rows: &[(f32, f32)]| {
        for (i, (x0, x1)) in rows.iter().enumerate() {
            let y = -6.0 + i as f32 * 4.0;
            p.line_segment([pt(*x0, y), pt(*x1, y)], s);
        }
    };
    let text = |p: &egui::Painter, t: &str, font: egui::FontId, color: Color32| {
        p.text(c, egui::Align2::CENTER_CENTER, t, font, color);
    };
    let serif = |bold: bool, italic: bool| {
        egui::FontId::new(16.0 * u, crate::fonts::family(DocFont::Serif, bold, italic))
    };
    match icon {
        Icon::New => {
            p.rect_stroke(r(-6.0, -8.0, 6.0, 8.0), 1.0, s, egui::StrokeKind::Middle);
            p.line_segment([pt(2.0, -8.0), pt(6.0, -4.0)], s);
        }
        Icon::Open => {
            p.rect_stroke(r(-8.0, -5.0, 8.0, 7.0), 1.0, s, egui::StrokeKind::Middle);
            p.line_segment([pt(-8.0, -5.0), pt(-4.0, -8.0)], s);
            p.line_segment([pt(-4.0, -8.0), pt(0.0, -5.0)], s);
        }
        Icon::Save => {
            p.rect_stroke(r(-7.0, -7.0, 7.0, 7.0), 1.5, s, egui::StrokeKind::Middle);
            p.rect_filled(r(-4.0, -7.0, 4.0, -2.0), 0.0, ink);
            p.rect_stroke(r(-4.0, 2.0, 4.0, 7.0), 0.0, s, egui::StrokeKind::Middle);
        }
        Icon::Pdf => {
            p.rect_filled(
                r(-9.0, -6.0, 9.0, 6.0),
                2.0,
                Color32::from_rgb(0xC6, 0x28, 0x28),
            );
            text(
                p,
                "PDF",
                egui::FontId::proportional(8.5 * u),
                Color32::WHITE,
            );
        }
        Icon::Odt => {
            p.rect_filled(
                r(-9.0, -6.0, 9.0, 6.0),
                2.0,
                Color32::from_rgb(0x1F, 0x5F, 0xBF),
            );
            text(
                p,
                "ODT",
                egui::FontId::proportional(8.0 * u),
                Color32::WHITE,
            );
        }
        Icon::Undo | Icon::Redo => {
            let dir = if matches!(icon, Icon::Undo) {
                1.0
            } else {
                -1.0
            };
            let pts: Vec<Pos2> = (0..=12)
                .map(|i| {
                    let a = std::f32::consts::PI * (0.15 + i as f32 / 12.0 * 0.95);
                    pt(dir * -a.cos() * 6.0, -a.sin() * 6.0 + 4.0)
                })
                .collect();
            p.add(egui::Shape::line(pts.clone(), s));
            let tip = pts[0];
            p.line_segment([tip, tip + Vec2::new(dir * 4.0, -1.0) * u], s);
            p.line_segment([tip, tip + Vec2::new(0.0, -4.5) * u], s);
        }
        Icon::Find => {
            p.circle_stroke(pt(-2.0, -2.0), 5.0 * u, s);
            p.line_segment([pt(2.0, 2.0), pt(7.0, 7.0)], Stroke::new(2.2, ink));
        }
        Icon::Bold => text(p, "G", serif(true, false), ink),
        Icon::Italic => text(p, "I", serif(false, true), ink),
        Icon::Underline => {
            text(p, "S", serif(false, false), ink);
            p.line_segment([pt(-5.0, 8.0), pt(5.0, 8.0)], s);
        }
        Icon::Strike => {
            text(p, "ab", serif(false, false), ink);
            p.line_segment([pt(-7.0, 0.5), pt(7.0, 0.5)], s);
        }
        Icon::AlignLeft => lines(p, &[(-8.0, 8.0), (-8.0, 3.0), (-8.0, 8.0), (-8.0, 1.0)]),
        Icon::AlignCenter => lines(p, &[(-8.0, 8.0), (-5.0, 5.0), (-8.0, 8.0), (-4.0, 4.0)]),
        Icon::AlignRight => lines(p, &[(-8.0, 8.0), (-3.0, 8.0), (-8.0, 8.0), (-1.0, 8.0)]),
        Icon::Bullets => {
            for i in 0..3 {
                let y = -5.0 + i as f32 * 5.0;
                p.circle_filled(pt(-6.0, y), 1.5 * u, ink);
                p.line_segment([pt(-2.0, y), pt(8.0, y)], s);
            }
        }
        Icon::Numbers => {
            for i in 0..3 {
                let y = -5.0 + i as f32 * 5.0;
                p.text(
                    pt(-6.0, y),
                    egui::Align2::CENTER_CENTER,
                    (i + 1).to_string(),
                    egui::FontId::proportional(7.0 * u),
                    ink,
                );
                p.line_segment([pt(-2.0, y), pt(8.0, y)], s);
            }
        }
        Icon::Field => text(p, "{ }", egui::FontId::monospace(12.0 * u), accent(ui)),
        Icon::Formula => text(
            p,
            "=×",
            egui::FontId::monospace(12.0 * u),
            formula_color(ui),
        ),
        Icon::Table => {
            let t = r(-8.0, -7.0, 8.0, 7.0);
            p.rect_stroke(t, 1.0, s, egui::StrokeKind::Middle);
            p.line_segment([pt(-8.0, -2.5), pt(8.0, -2.5)], s);
            p.line_segment([pt(-8.0, 2.5), pt(8.0, 2.5)], s);
            p.line_segment([pt(-2.5, -7.0), pt(-2.5, 7.0)], s);
            p.line_segment([pt(2.5, -7.0), pt(2.5, 7.0)], s);
        }
        Icon::PageBreak => {
            p.rect_stroke(r(-6.0, -9.0, 6.0, -2.0), 0.0, s, egui::StrokeKind::Middle);
            p.rect_stroke(r(-6.0, 2.0, 6.0, 9.0), 0.0, s, egui::StrokeKind::Middle);
            for i in 0..4 {
                let x = -8.0 + i as f32 * 4.5;
                p.line_segment([pt(x, 0.0), pt(x + 2.5, 0.0)], s);
            }
        }
    }
    resp
}

// ---------------------------------------------------------------------------------
// Couleurs, textes d'aide
// ---------------------------------------------------------------------------------

fn accent(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(0x8A, 0xB4, 0xF8)
    } else {
        Color32::from_rgb(0x1F, 0x5F, 0xBF)
    }
}
fn formula_color(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(0xD0, 0xA2, 0xFA)
    } else {
        Color32::from_rgb(0x7B, 0x3F, 0xB5)
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

const SPECIAL: [(&str, &str); 14] = [
    ("« ", "guillemet ouvrant"),
    (" »", "guillemet fermant"),
    ("—", "tiret cadratin"),
    ("–", "tiret demi-cadratin"),
    ("…", "points de suspension"),
    ("€", "euro"),
    ("°", "degré"),
    ("§", "paragraphe"),
    ("©", "copyright"),
    ("®", "marque déposée"),
    ("™", "marque"),
    ("✓", "coche"),
    ("→", "flèche"),
    ("\u{a0}", "espace insécable"),
];

const SYNTAX: &[(&str, &str)] = &[
    ("{{nom}}", "un champ : la même valeur partout"),
    (
        "{{nom | upper}}",
        "un filtre (upper, lower, capitalize, title, trim)",
    ),
    (
        "{{montant | money}}",
        "un montant : 12 500,00 € (money:usd…)",
    ),
    (
        "{{date | date:full}}",
        "une date : lundi 3 novembre 2026 (date, date:short)",
    ),
    ("{{x | default:\"…\"}}", "une valeur si le champ est vide"),
    (
        "{{= q * prix}}",
        "un champ calculé (+ − * / %, arrondi, min, max, somme)",
    ),
    ("{{aujourdhui}}", "la date du jour, automatiquement"),
    ("# Titre  ## Sous-titre", "titres (Ctrl+1, Ctrl+2, Ctrl+3)"),
    ("**gras**  *italique*", "caractères (Ctrl+B, Ctrl+I)"),
    ("__souligné__  ~~barré~~", "caractères (Ctrl+U)"),
    ("- élément   1. élément", "listes (Maj+F12, F12)"),
    ("> citation", "citation"),
    ("-> centré <-", "centré (Ctrl+E)"),
    ("-> à droite ->", "aligné à droite (Ctrl+R)"),
    ("| a | b |", "tableau ; |---|---:| sous l'en-tête"),
    ("---", "ligne horizontale"),
    ("\\newpage", "saut de page (Ctrl+Entrée)"),
    ("\\sommaire", "table des matières (liste des titres)"),
    ("$x^2$   $$ … $$", "formules LaTeX, en ligne ou centrées"),
    ("\\{{x}}  \\*  \\$", "caractères littéraux"),
];

fn act_label(a: Act) -> &'static str {
    match a {
        Act::New => "Nouveau document",
        Act::Templates => "Nouveau à partir d'un modèle",
        Act::Open => "Ouvrir",
        Act::Save => "Enregistrer",
        Act::SaveAs => "Enregistrer sous",
        Act::Export(Format::Odt) => "Exporter pour LibreOffice (.odt)",
        Act::Export(Format::Pdf) => "Exporter en PDF",
        Act::Export(_) => "Exporter",
        Act::Print => "Imprimer",
        Act::Undo => "Annuler",
        Act::Redo => "Rétablir",
        Act::Find => "Rechercher",
        Act::Replace => "Rechercher et remplacer",
        Act::Wrap("**") => "Gras",
        Act::Wrap("*") => "Italique",
        Act::Wrap("__") => "Souligné",
        Act::Wrap(_) => "Barré",
        Act::Style(LineStyle::Body) => "Corps de texte / aligner à gauche",
        Act::Style(LineStyle::Heading(1)) => "Titre 1",
        Act::Style(LineStyle::Heading(2)) => "Titre 2",
        Act::Style(LineStyle::Heading(_)) => "Titre 3",
        Act::Style(LineStyle::Center) => "Centrer",
        Act::Style(LineStyle::Right) => "Aligner à droite",
        Act::Style(LineStyle::Bullet) => "Liste à puces",
        Act::Style(LineStyle::Numbered) => "Liste numérotée",
        Act::Style(LineStyle::Quote) => "Citation",
        Act::InsertField => "Insérer un champ",
        Act::InsertPageBreak => "Saut de page",
        Act::Syntax => "Guide de la syntaxe",
        Act::Languages => "Écrire en plusieurs langues",
        Act::FullScreen => "Plein écran",
        Act::Focus => "Mode concentration",
        _ => "",
    }
}

// ---------------------------------------------------------------------------------
// Barre de menus native de macOS
// ---------------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod native {
    use muda::accelerator::Accelerator;
    use muda::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

    use super::{Act, Format, Layout};
    use crate::editor::LineStyle;

    /// Les menus de la barre de l'écran, et l'action de chaque élément.
    pub struct NativeMenu {
        _menu: Menu,
        actions: Vec<(String, Act)>,
    }

    struct Builder {
        actions: Vec<(String, Act)>,
    }

    impl Builder {
        fn item(&mut self, label: &str, act: Act, keys: Option<&str>) -> MenuItem {
            let id = format!("varium-{}", self.actions.len());
            self.actions.push((id.clone(), act));
            let accel = keys.and_then(|k| k.parse::<Accelerator>().ok());
            MenuItem::with_id(id, label, true, accel)
        }
    }

    impl NativeMenu {
        /// Installe les menus. `None` si macOS les refuse (on garde alors ceux d'egui).
        pub fn install() -> Option<Self> {
            let mut b = Builder {
                actions: Vec::new(),
            };
            let sep = PredefinedMenuItem::separator;

            let about = PredefinedMenuItem::about(
                Some("À propos de Varium"),
                Some(AboutMetadata {
                    name: Some("Varium".into()),
                    version: Some(env!("CARGO_PKG_VERSION").into()),
                    comments: Some("Un document. Toutes ses possibilités.".into()),
                    website: Some("https://maxlestage.github.io/VARIUM/".into()),
                    ..Default::default()
                }),
            );
            let quit = b.item("Quitter Varium", Act::Quit, Some("Cmd+Q"));
            let app_menu = Submenu::with_items(
                "Varium",
                true,
                &[
                    &about,
                    &sep(),
                    &PredefinedMenuItem::services(Some("Services")),
                    &sep(),
                    &PredefinedMenuItem::hide(Some("Masquer Varium")),
                    &PredefinedMenuItem::hide_others(Some("Masquer les autres")),
                    &PredefinedMenuItem::show_all(Some("Tout afficher")),
                    &sep(),
                    &quit,
                ],
            )
            .ok()?;

            let export = Submenu::with_items(
                "Exporter vers",
                true,
                &[
                    &b.item("PDF…", Act::Export(Format::Pdf), None),
                    &b.item("Word (.docx)…", Act::Export(Format::Docx), None),
                    &b.item("LibreOffice (.odt)…", Act::Export(Format::Odt), None),
                    &b.item("Texte enrichi (.rtf)…", Act::Export(Format::Rtf), None),
                    &b.item("EPUB…", Act::Export(Format::Epub), None),
                    &b.item("Page web (.html)…", Act::Export(Format::Html), None),
                    &b.item("LaTeX (.tex)…", Act::Export(Format::Latex), None),
                    &b.item("Texte brut (.txt)…", Act::Export(Format::Text), None),
                ],
            )
            .ok()?;
            let file = Submenu::with_items(
                "Fichier",
                true,
                &[
                    &b.item("Nouveau", Act::New, Some("Cmd+N")),
                    &b.item(
                        "Nouveau à partir d'un modèle…",
                        Act::Templates,
                        Some("Cmd+Shift+N"),
                    ),
                    &b.item("Ouvrir…", Act::Open, Some("Cmd+O")),
                    &sep(),
                    &b.item("Enregistrer", Act::Save, Some("Cmd+S")),
                    &b.item("Enregistrer sous…", Act::SaveAs, Some("Cmd+Shift+S")),
                    &sep(),
                    &export,
                    &b.item("Publipostage…", Act::Merge, None),
                    &sep(),
                    &b.item("Imprimer…", Act::Print, Some("Cmd+P")),
                ],
            )
            .ok()?;
            let edit = Submenu::with_items(
                "Édition",
                true,
                &[
                    &b.item("Annuler", Act::Undo, Some("Cmd+Z")),
                    &b.item("Rétablir", Act::Redo, Some("Cmd+Shift+Z")),
                    &sep(),
                    &b.item("Rechercher…", Act::Find, Some("Cmd+F")),
                    &b.item("Rechercher et remplacer…", Act::Replace, Some("Cmd+Alt+F")),
                    &sep(),
                    &b.item("Effacer toutes les valeurs", Act::ClearValues, None),
                ],
            )
            .ok()?;
            let insert = Submenu::with_items(
                "Insertion",
                true,
                &[
                    &b.item("Champ…", Act::InsertField, Some("Cmd+Shift+V")),
                    &b.item("Champ calculé…", Act::InsertFormula, None),
                    &b.item("Date du jour", Act::InsertToday, None),
                    &sep(),
                    &b.item("Tableau…", Act::InsertTable, None),
                    &b.item("Saut de page", Act::InsertPageBreak, Some("Cmd+Enter")),
                    &b.item("Ligne horizontale", Act::InsertRule, None),
                    &b.item("Formule", Act::InsertMath(false), None),
                ],
            )
            .ok()?;
            let format = Submenu::with_items(
                "Format",
                true,
                &[
                    &b.item("Gras", Act::Wrap("**"), Some("Cmd+B")),
                    &b.item("Italique", Act::Wrap("*"), Some("Cmd+I")),
                    &b.item("Souligné", Act::Wrap("__"), Some("Cmd+U")),
                    &b.item("Barré", Act::Wrap("~~"), None),
                    &sep(),
                    &b.item("Titre 1", Act::Style(LineStyle::Heading(1)), Some("Cmd+1")),
                    &b.item("Titre 2", Act::Style(LineStyle::Heading(2)), Some("Cmd+2")),
                    &b.item("Titre 3", Act::Style(LineStyle::Heading(3)), Some("Cmd+3")),
                    &b.item("Corps de texte", Act::Style(LineStyle::Body), Some("Cmd+0")),
                    &b.item("Citation", Act::Style(LineStyle::Quote), None),
                    &sep(),
                    &b.item("Liste à puces", Act::Style(LineStyle::Bullet), None),
                    &b.item("Liste numérotée", Act::Style(LineStyle::Numbered), None),
                    &sep(),
                    &b.item("Aligner à gauche", Act::Style(LineStyle::Body), None),
                    &b.item("Centrer", Act::Style(LineStyle::Center), Some("Cmd+E")),
                    &b.item(
                        "Aligner à droite",
                        Act::Style(LineStyle::Right),
                        Some("Cmd+R"),
                    ),
                    &sep(),
                    &b.item("Mise en page…", Act::PageSetup, None),
                ],
            )
            .ok()?;
            let view = Submenu::with_items(
                "Présentation",
                true,
                &[
                    &b.item("Texte et page", Act::Layout(Layout::Split), None),
                    &b.item("Page seule", Act::Layout(Layout::Page), None),
                    &b.item("Texte seul", Act::Layout(Layout::Text), None),
                    &sep(),
                    &b.item(
                        "Afficher / masquer l'inspecteur",
                        Act::ToggleFields,
                        Some("Cmd+Alt+I"),
                    ),
                    &b.item(
                        "Afficher / masquer la barre d'outils",
                        Act::ToggleToolbar,
                        Some("Cmd+Alt+T"),
                    ),
                    &b.item(
                        "Afficher / masquer la barre d'état",
                        Act::ToggleStatus,
                        None,
                    ),
                    &b.item("Trame de fond des champs", Act::ToggleHighlight, None),
                    &b.item("Mode concentration", Act::Focus, Some("Cmd+Shift+F")),
                    &sep(),
                    &b.item("Zoom avant", Act::ZoomIn, Some("Cmd+=")),
                    &b.item("Zoom arrière", Act::ZoomOut, Some("Cmd+-")),
                    &b.item("Largeur de la page", Act::ZoomFit, None),
                    &sep(),
                    &b.item("Plein écran", Act::FullScreen, Some("Ctrl+Cmd+F")),
                ],
            )
            .ok()?;
            let window = Submenu::with_items(
                "Fenêtre",
                true,
                &[
                    &PredefinedMenuItem::minimize(Some("Placer dans le Dock")),
                    &PredefinedMenuItem::maximize(Some("Zoom")),
                    &sep(),
                    &PredefinedMenuItem::bring_all_to_front(Some("Tout ramener au premier plan")),
                ],
            )
            .ok()?;
            let help = Submenu::with_items(
                "Aide",
                true,
                &[
                    &b.item("Guide de la syntaxe", Act::Syntax, None),
                    &b.item("Raccourcis clavier", Act::Shortcuts, None),
                    &b.item("Nombre de mots", Act::Stats, None),
                    &b.item("À propos de Varium", Act::About, None),
                ],
            )
            .ok()?;

            let menu = Menu::with_items(&[
                &app_menu, &file, &edit, &insert, &format, &view, &window, &help,
            ])
            .ok()?;
            menu.init_for_nsapp();
            window.set_as_windows_menu_for_nsapp();
            help.set_as_help_menu_for_nsapp();
            Some(Self {
                _menu: menu,
                actions: b.actions,
            })
        }

        /// Les actions choisies dans les menus depuis la dernière image.
        pub fn poll(&self) -> Vec<Act> {
            let mut out = Vec::new();
            while let Ok(ev) = MenuEvent::receiver().try_recv() {
                if let Some((_, act)) = self.actions.iter().find(|(id, _)| ev.id.0 == *id) {
                    out.push(*act);
                }
            }
            out
        }
    }
}
