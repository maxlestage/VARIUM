//! L'interface de Varium, sur le modèle de LibreOffice Writer : barre de menus,
//! barre d'outils de mise en forme, texte et page côte à côte, panneau des champs,
//! barre d'état. Sur un écran étroit (téléphone), les vues passent en onglets.

use crate::labels;
use crate::tr::{n_, tr};
use egui::text::{CCursor, CharIndex};
use egui::text_selection::CCursorRange;
use egui::{Color32, Key, KeyboardShortcut, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use varium_core::export::{self, Rendered};
use varium_core::merge::{self, Sheet};
use varium_core::{DocFont, PageSetup, Paper, Project, Values, VarType, docx, filters, odt, stats};

use crate::dico;
use crate::editor::{self, LineStyle, Palette, Sel};
use crate::files;
use crate::io::{self, Event, Io, OpenKind};
use crate::pageview::{self, View};
use crate::secours;
use crate::templates;

// Outils d'écriture du module Documents (sous-modules de `app`, fichiers voisins).
#[path = "doc_detection.rs"]
mod detection;
#[path = "doc_ecriture.rs"]
mod ecriture;
#[path = "doc_formulaire.rs"]
mod formulaire;
#[path = "doc_historique.rs"]
mod historique;
#[path = "doc_import.rs"]
mod import;
#[path = "doc_mise_en_forme.rs"]
mod mise_en_forme;
#[path = "doc_modeles.rs"]
mod modeles;
#[path = "doc_revision.rs"]
mod revision;
#[path = "doc_voix.rs"]
mod voix;

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
pub enum Act {
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
    /// Historique des versions.
    History,
    /// « Marquer cette version… »
    MarkVersion,
    /// Objectif de mots.
    Goal,
    /// Coller sans mise en forme (Ctrl+Maj+V).
    PastePlain,
    InsertQr,
    /// « Remplir comme un formulaire ».
    FillForm,
    /// « Détecter les champs… » dans un document déjà écrit.
    DetectFields,
    ToggleAutocorrect,
    /// Défilement « machine à écrire » du mode concentration.
    ToggleTypewriter,
    /// « Suivre les modifications » (révision).
    ToggleSuivi,
    /// Commenter la sélection.
    Commenter,
    ToutAccepter,
    ToutRefuser,
    /// L'onglet « Modifications » de l'inspecteur.
    Revision,
    /// « Créer un modèle à partir de ce document ».
    DecouperModele,
    /// « Lire à voix haute » (Pause / Reprendre pendant la lecture).
    LireVoix,
    ArreterVoix,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Format {
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
    (sc(CMD_SHIFT, Key::V), Act::PastePlain),
    (sc(CMD_SHIFT, Key::H), Act::History),
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
    (sc(Modifiers::NONE, Key::F2), Act::InsertField),
    (sc(Modifiers::NONE, Key::F11), Act::Focus),
];

fn shortcut_for(act: Act) -> Option<KeyboardShortcut> {
    SHORTCUTS.iter().find(|(_, a)| *a == act).map(|(s, _)| *s)
}

// ---------------------------------------------------------------------------------
// État
// ---------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum Layout {
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
    /// Commentaires et suivi des modifications.
    Suivi,
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
    /// Souligner les fautes d'orthographe pendant la frappe.
    spell_live: bool,
    /// Fichier ouvert à la fermeture, rouvert au démarrage.
    dernier_fichier: Option<String>,
    /// Correction automatique pendant la frappe (Outils).
    correction_auto: bool,
    /// Mode concentration : la ligne en cours reste au milieu de l'écran.
    machine_a_ecrire: bool,
    /// Nom signé sur les modifications suivies et les commentaires (vide : celui de
    /// la session).
    auteur: String,
    /// « Suivre les modifications ».
    suivi: bool,
    /// Les exports gardent les marques de révision (sinon : version finale).
    export_marques: bool,
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
            spell_live: true,
            dernier_fichier: None,
            correction_auto: true,
            machine_a_ecrire: true,
            auteur: String::new(),
            suivi: false,
            export_marques: false,
        }
    }
}

/// L'état d'un document (dernier enregistrement, annuler/rétablir) : voir `files`.
use crate::files::Snapshot;

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
    /// Objectif de mots (saisie en cours).
    Objectif(String),
    /// « Marquer cette version… » (nom en cours).
    Marquer(String),
    Qr(formulaire::QrSaisie),
    /// « Détecter les champs… » (propositions cochées).
    Detection(detection::DetectionSaisie),
    /// « Commenter la sélection » (texte du commentaire).
    Commentaire(String),
    /// « Créer un modèle à partir de ce document ».
    Decouper(Box<modeles::Decoupe>),
    /// « Nouveau depuis un modèle » : les questions une à une.
    Assistant(Box<modeles::Assistant>),
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

/// Menu contextuel d'un mot mal orthographié (clic droit dans le texte).
struct SpellMenu {
    start: usize,
    end: usize,
    word: String,
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
    /// Les menus sont dans la barre native de macOS (pas de barre dans la fenêtre).
    pub native_menu: bool,
    allow_close: bool,
    /// Fichier de secours actuellement écrit (effacé dès que le document est propre).
    backup_file: Option<std::path::PathBuf>,
    last_backup: f64,
    /// Dictionnaires orthographiques, dictionnaire personnel et mots ignorés.
    dicos: dico::Dicos,
    spell_menu: Option<SpellMenu>,
    /// Objectif de mots du document.
    objectif: Option<u32>,
    /// Outils d'écriture : concentration, formulaire, historique des versions…
    outils: ecriture::Outils,
    /// Badge « champs possibles » et lecture à voix haute.
    etat: revision::Etat,
}

fn today() -> String {
    jiff::Zoned::now().date().to_string()
}

impl VariumApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::fonts::install(&cc.egui_ctx);
        mise_en_forme::charger_signatures();
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
            objectif: project.objectif,
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
                objectif: None,
            },
            saving: None,
            after_save: None,
            last_window_title: String::new(),
            last_act: None,
            native_menu: false,
            allow_close: false,
            backup_file: None,
            last_backup: f64::NEG_INFINITY,
            dicos: dico::Dicos::new(),
            spell_menu: None,
            objectif: None,
            outils: ecriture::Outils::default(),
            etat: revision::Etat::default(),
        };
        app.restore(app.history.current.clone());
        app.saved = app.snapshot();
        // Le document était lié à un fichier : on le retrouve (Ctrl+S le
        // réécrit) ; s'il diffère de l'état restauré, il est « modifié ».
        if !first_launch
            && std::env::args_os().nth(1).is_none()
            && let Some(chemin) = app
                .prefs
                .dernier_fichier
                .clone()
                .map(std::path::PathBuf::from)
            && let Some(sur_disque) = files::lire_projet(&chemin)
        {
            let valeurs = sur_disque.values();
            app.saved = Snapshot {
                title: sur_disque.title,
                source: sur_disque.source,
                values: valeurs,
                page: sur_disque.page,
                objectif: sur_disque.objectif,
            };
            app.path = Some(chemin);
        }
        // Document interrompu par un plantage ?
        let lie = app.path.clone();
        app.propose_secours(lie.as_deref());
        // Un fichier donné au lancement (« Ouvrir avec », glisser sur l'icône, terminal).
        if let Some(path) = crate::ouverture::document_arg()
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
            objectif: self.objectif,
        }
    }

    fn restore(&mut self, s: Snapshot) {
        self.title = s.title;
        self.source = s.source;
        self.values = s.values;
        self.page = s.page;
        self.objectif = s.objectif;
        let n = self.source.chars().count();
        self.sel = (self.sel.0.min(n), self.sel.1.min(n));
        self.rename = None;
    }

    fn load(&mut self, p: Project) {
        // Le document remplacé garde une version dans son historique.
        if self.is_dirty() {
            self.prendre_version(
                varium_core::versions::Motif::Avant,
                "Remplacement du document",
            );
        }
        self.history.cut = true;
        let values = p.values();
        self.restore(Snapshot {
            title: p.title,
            source: p.source,
            values,
            page: p.page,
            objectif: p.objectif,
        });
        self.outils.mots_depart = None;
        self.outils.formulaire = false;
        self.sel = (0, 0);
        self.pending_sel = Some((0, 0));
        self.merge = None;
        self.path = None;
        self.saved = self.snapshot();
        // Les mots ignorés valent pour un document.
        self.dicos.clear_ignored();
        self.spell_menu = None;
        self.etat.badge = None;
        self.etat.voix.arreter();
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
                objectif: None,
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
        self.saved.differe(
            &self.title,
            &self.source,
            &self.values,
            &self.page,
            self.objectif,
        )
    }

    /// Exécute `p` tout de suite, ou demande confirmation s'il y a des
    /// modifications non enregistrées.
    fn guard(&mut self, ctx: &egui::Context, p: Pending) {
        match files::garder(self.is_dirty(), p) {
            files::Garde::Confirmer(p) => self.dialog = Some(Dialog::Confirm(p)),
            files::Garde::Continuer(p) => self.proceed(ctx, p),
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
                        trf!(
                            "« {path} » est introuvable (déplacé ou supprimé)",
                            path = path.display()
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
                    self.prendre_version(varium_core::versions::Motif::Enregistrement, "");
                    files::push_recent(&mut self.prefs.recent, &path);
                    self.notify(
                        ctx,
                        trf!("Enregistré : {path}", path = path.display()),
                        false,
                    );
                    SaveOutcome::Done
                }
                Err(e) => {
                    self.notify(ctx, trf!("Écriture impossible : {e}", e), true);
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
        p.objectif = self.objectif;
        p
    }

    /// Enregistre l'état dans l'historique (appelé à chaque image).
    fn track(&mut self, now: f64) {
        if self.history.current.source == self.source
            && self.history.current.values == self.values
            && self.history.current.page == self.page
            && self.history.current.title == self.title
            && self.history.current.objectif == self.objectif
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
        varium_core::critic::avec_vue(self.vue_export(), || self.rendered_sans_vue())
    }

    fn rendered_sans_vue(&self) -> Vec<Rendered> {
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
        self.native_menu
    }

    /// Une action choisie dans la barre de menus native.
    #[cfg(target_os = "macos")]
    pub fn menu_act(&mut self, ctx: &egui::Context, act: Act) {
        let now = ctx.input(|i| i.time);
        self.push_once(act, now);
    }

    fn run(&mut self, ctx: &egui::Context, act: Act) {
        match act {
            Act::New => self.guard(ctx, Pending::New),
            Act::NewBlank => {
                self.load(Project::new(tr("Sans titre"), "", &Values::default()));
                self.notify(
                    ctx,
                    tr("Nouveau document · Ctrl+Z pour revenir au précédent"),
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
                self.prendre_version(
                    varium_core::versions::Motif::Avant,
                    "Effacer toutes les valeurs",
                );
                self.history.cut = true;
                self.values.values.clear();
                self.notify(
                    ctx,
                    tr("Valeurs effacées · Ctrl+Z pour les retrouver"),
                    false,
                );
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
                // Mode concentration : rien que le texte, au centre (Échap pour sortir).
                self.outils.concentration = !self.outils.concentration;
                if self.outils.concentration {
                    self.outils.formulaire = false;
                    self.pending_sel = Some(self.sel);
                    self.notify(ctx, tr("Mode concentration · Échap pour en sortir"), false);
                }
            }
            Act::Layout(l) => self.prefs.layout = l,
            Act::History => self.ouvrir_historique(),
            Act::MarkVersion => self.dialog = Some(Dialog::Marquer(String::new())),
            Act::Goal => self.ouvrir_objectif(),
            Act::PastePlain => self.coller_brut(ctx),
            Act::InsertQr => self.dialog = Some(Dialog::Qr(formulaire::QrSaisie::default())),
            Act::DetectFields => {
                self.dialog = Some(Dialog::Detection(detection::DetectionSaisie::analyser(
                    &self.source,
                )));
            }
            Act::FillForm => {
                self.outils.formulaire = !self.outils.formulaire;
                if self.outils.formulaire {
                    self.outils.concentration = false;
                    self.outils.formulaire_focus = true;
                }
            }
            Act::ToggleAutocorrect => {
                self.prefs.correction_auto = !self.prefs.correction_auto;
                self.notify(
                    ctx,
                    if self.prefs.correction_auto {
                        tr("Correction automatique activée")
                    } else {
                        tr("Correction automatique désactivée")
                    },
                    false,
                );
            }
            Act::ToggleTypewriter => self.prefs.machine_a_ecrire = !self.prefs.machine_a_ecrire,
            Act::ToggleSuivi => self.basculer_suivi(ctx),
            Act::Commenter => self.ouvrir_commentaire(),
            Act::ToutAccepter => self.decider_tout(ctx, true),
            Act::ToutRefuser => self.decider_tout(ctx, false),
            Act::Revision => {
                self.prefs.inspector = Inspector::Suivi;
                self.prefs.fields_panel = true;
            }
            Act::DecouperModele => self.ouvrir_decoupe(),
            Act::LireVoix => self.lire_voix(ctx),
            Act::ArreterVoix => self.etat.voix.arreter(),
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
                        trf!("« {name} » ne contient aucune ligne de données", name),
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
                            trf!("{n} destinataire(s) chargé(s) depuis « {name} »", n, name),
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
                    if io::is_office_text(&name) || content.starts_with("PK\u{3}\u{4}") {
                        self.ouvrir_bureautique(ctx, &name, &content, path);
                    } else if files::is_project_file(&name) {
                        match Project::from_json(&content) {
                            Ok(p) => {
                                self.load(p);
                                let source = self.source.clone();
                                self.signaler_champs(&source);
                                if let Some(path) = path {
                                    files::push_recent(&mut self.prefs.recent, &path);
                                    self.propose_secours(Some(&path));
                                    self.path = Some(path);
                                }
                                self.notify(ctx, trf!("« {name} » ouvert", name), false);
                            }
                            Err(e) => {
                                // Projet abîmé : on récupère ce qu'on peut, sans associer
                                // le fichier d'origine (Enregistrer demandera un nouveau nom).
                                if let Some((p, notes)) = Project::recuperer(&content) {
                                    self.load(p);
                                    let detail = notes.join(" ; ");
                                    self.notify(
                                        ctx,
                                        trf!(
                                            "« {name} » est abîmé : contenu récupéré ({detail}). Enregistrez-le sous un nouveau nom.",
                                            name,
                                            detail
                                        ),
                                        true,
                                    );
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
                                        trf!(
                                            "« {name} » est illisible ({e}) : texte brut ouvert",
                                            name,
                                            e
                                        ),
                                        true,
                                    );
                                }
                            }
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
                            trf!("Modèle « {name} » ouvert, valeurs conservées", name),
                            false,
                        );
                    }
                }
                Event::Saved { path, project } => {
                    self.notify(
                        ctx,
                        trf!("Enregistré : {path}", path = path.display()),
                        false,
                    );
                    if project {
                        self.saved = self.saving.take().unwrap_or_else(|| self.snapshot());
                        files::push_recent(&mut self.prefs.recent, &path);
                        self.path = Some(path);
                        self.prendre_version(varium_core::versions::Motif::Enregistrement, "");
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
        ui.menu_button(tr("Fichier"), |ui| {
            self.item(ui, tr("Nouveau"), Act::New);
            self.item(ui, tr("Nouveau à partir d'un modèle…"), Act::Templates);
            self.item(
                ui,
                tr("Créer un modèle à partir de ce document…"),
                Act::DecouperModele,
            );
            self.item(ui, tr("Ouvrir…"), Act::Open);
            ui.add_enabled_ui(!self.prefs.recent.is_empty(), |ui| {
                ui.menu_button(tr("Documents récents"), |ui| {
                    for (i, p) in self.prefs.recent.clone().iter().enumerate() {
                        if ui.button(files::short_name(p)).on_hover_text(p).clicked() {
                            self.queue.push(Act::OpenRecent(i));
                            ui.close();
                        }
                    }
                });
            });
            ui.separator();
            self.item(ui, tr("Enregistrer"), Act::Save);
            self.item(ui, tr("Enregistrer sous…"), Act::SaveAs);
            ui.menu_button(tr("Exporter"), |ui| {
                self.item(ui, tr("PDF (.pdf)"), Act::Export(Format::Pdf));
                self.item(
                    ui,
                    tr("OpenDocument Texte (.odt) — LibreOffice"),
                    Act::Export(Format::Odt),
                );
                self.item(ui, tr("Word (.docx)"), Act::Export(Format::Docx));
                self.item(
                    ui,
                    tr("Texte enrichi (.rtf) — Pages, TextEdit"),
                    Act::Export(Format::Rtf),
                );
                self.item(ui, tr("Livre numérique (.epub)"), Act::Export(Format::Epub));
                self.item(ui, tr("Page web (.html)"), Act::Export(Format::Html));
                self.item(ui, tr("LaTeX (.tex)"), Act::Export(Format::Latex));
                self.item(ui, tr("Texte brut (.txt)"), Act::Export(Format::Text));
                self.item(
                    ui,
                    tr("Modèle avec ses {{champs}}"),
                    Act::Export(Format::Model),
                );
            });
            self.item(ui, tr("Imprimer (via le navigateur)…"), Act::Print);
            ui.separator();
            self.item(ui, tr("Historique des versions…"), Act::History);
            self.item(ui, tr("Marquer cette version…"), Act::MarkVersion);
            ui.separator();
            self.item(ui, tr("Publipostage…"), Act::Merge);
            ui.separator();
            self.item(ui, tr("Quitter"), Act::Quit);
        });
        ui.menu_button(tr("Édition"), |ui| {
            let (u, r) = (!self.history.undo.is_empty(), !self.history.redo.is_empty());
            ui.add_enabled_ui(u, |ui| self.item(ui, tr("Annuler"), Act::Undo));
            ui.add_enabled_ui(r, |ui| self.item(ui, tr("Rétablir"), Act::Redo));
            ui.separator();
            self.item(ui, tr("Coller sans mise en forme"), Act::PastePlain);
            ui.separator();
            self.item(ui, tr("Rechercher…"), Act::Find);
            self.item(ui, tr("Rechercher et remplacer…"), Act::Replace);
            ui.separator();
            self.item(ui, tr("Effacer toutes les valeurs"), Act::ClearValues);
        });
        ui.menu_button(tr("Affichage"), |ui| {
            let l = self.prefs.layout;
            self.check(
                ui,
                tr("Texte et page"),
                l == Layout::Split,
                Act::Layout(Layout::Split),
            );
            self.check(
                ui,
                tr("Page seule"),
                l == Layout::Page,
                Act::Layout(Layout::Page),
            );
            self.check(
                ui,
                tr("Texte seul"),
                l == Layout::Text,
                Act::Layout(Layout::Text),
            );
            ui.separator();
            self.check(
                ui,
                tr("Panneau des champs"),
                self.prefs.fields_panel,
                Act::ToggleFields,
            );
            self.check(
                ui,
                tr("Barre d'outils"),
                self.prefs.toolbar,
                Act::ToggleToolbar,
            );
            self.check(
                ui,
                tr("Barre d'état"),
                self.prefs.status_bar,
                Act::ToggleStatus,
            );
            ui.separator();
            self.check(
                ui,
                tr("Mode concentration"),
                self.outils.concentration,
                Act::Focus,
            );
            self.check(
                ui,
                tr("Défilement machine à écrire"),
                self.prefs.machine_a_ecrire,
                Act::ToggleTypewriter,
            );
            self.item(ui, tr("Objectif de mots…"), Act::Goal);
            self.item(ui, tr("Plein écran"), Act::FullScreen);
            self.check(
                ui,
                tr("Trame de fond des champs"),
                self.prefs.highlight,
                Act::ToggleHighlight,
            );
            ui.separator();
            ui.menu_button(tr("Zoom"), |ui| {
                self.item(ui, tr("Largeur de la page"), Act::ZoomFit);
                for z in [0.5, 0.75, 1.0, 1.25, 1.5, 2.0] {
                    self.item(ui, &format!("{} %", (z * 100.0) as i32), Act::Zoom(z));
                }
            });
            pageview::case_vignettes(ui);
            ui.menu_button(tr("Thème"), egui::widgets::global_theme_preference_buttons);
            crate::tr::menu(ui);
        });
        ui.menu_button(tr("Insertion"), |ui| {
            self.item(ui, tr("Champ {{…}}…"), Act::InsertField);
            self.item(ui, tr("Champ calculé {{= …}}…"), Act::InsertFormula);
            self.item(ui, tr("Date du jour"), Act::InsertToday);
            self.item(ui, tr("Code QR…"), Act::InsertQr);
            if ui.button(tr("Signature…")).clicked() {
                mise_en_forme::ouvrir_signature(ui.ctx());
                ui.close();
            }
            ui.separator();
            self.item(ui, tr("Tableau…"), Act::InsertTable);
            self.item(ui, tr("Saut de page"), Act::InsertPageBreak);
            self.item(ui, tr("Ligne horizontale"), Act::InsertRule);
            ui.menu_button(tr("Formule"), |ui| {
                self.item(ui, tr("En ligne  $…$"), Act::InsertMath(false));
                self.item(ui, tr("Centrée  $$…$$"), Act::InsertMath(true));
            });
            ui.menu_button(tr("Caractère spécial"), |ui| {
                for (c, name) in SPECIAL {
                    self.item(ui, &format!("{c}   {}", tr(name)), Act::InsertText(c));
                }
            });
        });
        ui.menu_button(tr("Format"), |ui| {
            self.item(ui, tr("Gras"), Act::Wrap("**"));
            self.item(ui, tr("Italique"), Act::Wrap("*"));
            self.item(ui, tr("Souligné"), Act::Wrap("__"));
            self.item(ui, tr("Barré"), Act::Wrap("~~"));
            ui.separator();
            ui.menu_button(tr("Styles de paragraphe"), |ui| {
                for st in LineStyle::MENU {
                    self.item(ui, st.label(), Act::Style(st));
                }
            });
            ui.menu_button(tr("Listes"), |ui| {
                self.item(ui, tr("Liste à puces"), Act::Style(LineStyle::Bullet));
                self.item(ui, tr("Liste numérotée"), Act::Style(LineStyle::Numbered));
            });
            ui.menu_button(tr("Alignement"), |ui| {
                self.item(ui, tr("À gauche"), Act::Style(LineStyle::Body));
                self.item(ui, tr("Centré"), Act::Style(LineStyle::Center));
                self.item(ui, tr("À droite"), Act::Style(LineStyle::Right));
            });
            ui.separator();
            self.item(
                ui,
                tr("Page… (format, marges, police, en-tête)"),
                Act::PageSetup,
            );
        });
        ui.menu_button(tr("Outils"), |ui| {
            self.check(
                ui,
                tr("Remplir comme un formulaire"),
                self.outils.formulaire,
                Act::FillForm,
            );
            self.item(ui, tr("Détecter les champs…"), Act::DetectFields);
            self.item(ui, tr("Publipostage…"), Act::Merge);
            ui.separator();
            ui.menu_button(tr("Révision"), |ui| {
                self.check(
                    ui,
                    tr("Suivre les modifications"),
                    self.prefs.suivi,
                    Act::ToggleSuivi,
                );
                self.item(ui, tr("Commenter la sélection…"), Act::Commenter);
                self.item(ui, tr("Commentaires et modifications"), Act::Revision);
                ui.separator();
                self.item(ui, tr("Tout accepter"), Act::ToutAccepter);
                self.item(ui, tr("Tout refuser"), Act::ToutRefuser);
            });
            let lecture = self.etat.voix.etat != voix::EtatLecture::Arret;
            self.item(
                ui,
                if lecture {
                    tr("Pause / reprendre la lecture")
                } else {
                    tr("Lire à voix haute")
                },
                Act::LireVoix,
            );
            if lecture {
                self.item(ui, tr("Arrêter la lecture"), Act::ArreterVoix);
            }
            ui.separator();
            self.check(
                ui,
                tr("Correction automatique pendant la frappe"),
                self.prefs.correction_auto,
                Act::ToggleAutocorrect,
            );
            self.item(ui, tr("Nombre de mots"), Act::Stats);
            self.item(ui, tr("Objectif de mots…"), Act::Goal);
            ui.separator();
            self.item(ui, tr("Historique des versions…"), Act::History);
        });
        ui.menu_button(tr("Aide"), |ui| {
            self.item(ui, tr("Guide de la syntaxe"), Act::Syntax);
            self.item(ui, tr("Écrire en plusieurs langues"), Act::Languages);
            self.item(ui, tr("Raccourcis clavier"), Act::Shortcuts);
            ui.hyperlink_to(tr("Site de Varium"), "https://maxlestage.github.io/VARIUM/");
            self.item(ui, tr("À propos de Varium"), Act::About);
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
                        .hint_text(tr("Titre du document"))
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
                self.tool(ui, Icon::New, tr("Nouveau"), Act::New, false);
                self.tool(ui, Icon::Open, tr("Ouvrir"), Act::Open, false);
                self.tool(ui, Icon::Save, tr("Enregistrer (Ctrl+S)"), Act::Save, false);
                sep(ui);
                self.tool(
                    ui,
                    Icon::Pdf,
                    tr("Exporter en PDF"),
                    Act::Export(Format::Pdf),
                    false,
                );
                self.tool(
                    ui,
                    Icon::Odt,
                    tr("Exporter pour LibreOffice (.odt)"),
                    Act::Export(Format::Odt),
                    false,
                );
                sep(ui);
                self.tool(ui, Icon::Undo, tr("Annuler"), Act::Undo, false);
                self.tool(ui, Icon::Redo, tr("Rétablir"), Act::Redo, false);
                self.tool(
                    ui,
                    Icon::Find,
                    tr("Rechercher et remplacer"),
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
            self.tool(ui, Icon::Bold, tr("Gras"), Act::Wrap("**"), false);
            self.tool(ui, Icon::Italic, tr("Italique"), Act::Wrap("*"), false);
            self.tool(ui, Icon::Underline, tr("Souligné"), Act::Wrap("__"), false);
            self.tool(ui, Icon::Strike, tr("Barré"), Act::Wrap("~~"), false);
            sep(ui);
            self.tool(
                ui,
                Icon::AlignLeft,
                tr("Aligner à gauche"),
                Act::Style(LineStyle::Body),
                !matches!(style, LineStyle::Center | LineStyle::Right),
            );
            self.tool(
                ui,
                Icon::AlignCenter,
                tr("Centrer"),
                Act::Style(LineStyle::Center),
                style == LineStyle::Center,
            );
            self.tool(
                ui,
                Icon::AlignRight,
                tr("Aligner à droite"),
                Act::Style(LineStyle::Right),
                style == LineStyle::Right,
            );
            sep(ui);
            self.tool(
                ui,
                Icon::Bullets,
                tr("Liste à puces"),
                Act::Style(LineStyle::Bullet),
                style == LineStyle::Bullet,
            );
            self.tool(
                ui,
                Icon::Numbers,
                tr("Liste numérotée"),
                Act::Style(LineStyle::Numbered),
                style == LineStyle::Numbered,
            );
            sep(ui);
            self.tool(
                ui,
                Icon::Field,
                tr("Insérer un champ {{…}}"),
                Act::InsertField,
                false,
            );
            self.tool(
                ui,
                Icon::Formula,
                tr("Insérer un champ calculé"),
                Act::InsertFormula,
                false,
            );
            self.tool(
                ui,
                Icon::Table,
                tr("Insérer un tableau"),
                Act::InsertTable,
                false,
            );
            self.tool(
                ui,
                Icon::PageBreak,
                tr("Saut de page"),
                Act::InsertPageBreak,
                false,
            );
            if mobile {
                sep(ui);
                self.tool(
                    ui,
                    Icon::Find,
                    tr("Rechercher et remplacer"),
                    Act::Replace,
                    self.find.open,
                );
            }
            if !mobile && self.has_native_menu() {
                sep(ui);
                ui.add(
                    egui::TextEdit::singleline(&mut self.title)
                        .hint_text(tr("Titre du document"))
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
                    .hint_text(tr("Rechercher"))
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
                    tr("aucun résultat").into()
                } else {
                    trf!("{i} sur {n}", i = self.find.current + 1, n)
                })
                .small()
                .weak(),
            );
            if ui
                .add_enabled(n > 0, egui::Button::new("⏶").small())
                .on_hover_text(tr("Précédent"))
                .clicked()
            {
                go = Some((self.find.current + n - 1) % n);
            }
            if ui
                .add_enabled(n > 0, egui::Button::new("⏷").small())
                .on_hover_text(tr("Suivant"))
                .clicked()
            {
                go = Some((self.find.current + 1) % n);
            }
            ui.checkbox(&mut self.find.case, tr("Respecter la casse"));
            if self.find.replace {
                ui.separator();
                ui.add(
                    egui::TextEdit::singleline(&mut self.find.with)
                        .hint_text(tr("Remplacer par"))
                        .desired_width(160.0),
                );
                replace_one = ui
                    .add_enabled(n > 0, egui::Button::new(tr("Remplacer")))
                    .clicked();
                replace_all = ui
                    .add_enabled(n > 0, egui::Button::new(tr("Tout remplacer")))
                    .clicked();
            } else if ui.small_button(tr("Remplacer…")).clicked() {
                self.find.replace = true;
            }
            if ui
                .small_button("✕")
                .on_hover_text(tr("Fermer (Échap)"))
                .clicked()
                || ui.input(|i| i.key_pressed(Key::Escape))
            {
                self.find.open = false;
            }
        });

        if replace_all {
            let count = n;
            let with = self.find.with.clone();
            self.prendre_version(varium_core::versions::Motif::Avant, "Tout remplacer");
            self.history.cut = true;
            for &(a, b) in found.iter().rev() {
                self.source.replace_range(a..b, &with);
            }
            let ctx = ui.ctx().clone();
            self.notify(&ctx, trf!("{count} remplacement(s)", count), false);
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
        self.text_pane_ex(ui, mobile, false);
    }

    /// L'éditeur. `focus` : mode concentration (texte plus grand, paragraphes autres
    /// que celui du curseur estompés, défilement « machine à écrire »).
    fn text_pane_ex(&mut self, ui: &mut egui::Ui, mobile: bool, focus: bool) {
        let id = egui::Id::unique("varium-source");
        let ctx = ui.ctx().clone();
        // Collage intelligent : le texte collé (Ctrl+V) est converti avant que
        // l'éditeur l'insère, à sa place parmi les autres frappes.
        if ui.memory(|m| m.has_focus(id)) {
            let mut colles: Vec<String> = Vec::new();
            ui.input_mut(|i| {
                for e in &mut i.events {
                    if let egui::Event::Paste(t) = e {
                        colles.push(std::mem::take(t));
                    }
                }
            });
            if !colles.is_empty() {
                let convertis: Vec<String> =
                    colles.into_iter().map(|t| self.coller(&ctx, t)).collect();
                // Des dates, montants, e-mails dans le texte collé : le badge les signale.
                self.signaler_champs(&convertis.join("\n"));
                let mut it = convertis.into_iter();
                ui.input_mut(|i| {
                    for e in &mut i.events {
                        if let egui::Event::Paste(t) = e {
                            *t = it.next().unwrap_or_default();
                        }
                    }
                });
                // Le collage est une étape d'annulation à lui seul.
                self.history.cut = true;
            }
        }
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
        let mut found = if self.find.open {
            editor::find_all(&self.source, &self.find.query, self.find.case)
        } else {
            Vec::new()
        };
        // La phrase lue à voix haute est surlignée.
        if let Some(r) = self.etat.voix.suivre() {
            found.push((
                editor::byte_of(&self.source, r.start),
                editor::byte_of(&self.source, r.end),
            ));
        }
        let font = self.page.font;
        let size = if focus {
            19.0
        } else if mobile {
            17.0
        } else {
            16.0
        };
        // Remarques de la révision, soulignées dans le texte quand l'onglet est ouvert.
        let issues: Vec<(usize, usize)> = if self.prefs.inspector == Inspector::Review && !focus {
            varium_core::check::check(&self.source, self.page.langue)
                .into_iter()
                .map(|i| (i.start, i.end))
                .collect()
        } else {
            Vec::new()
        };
        // Paragraphe du curseur (octets), mis en avant en mode concentration.
        let courant = focus.then(|| {
            let b = editor::byte_of(&self.source, self.sel.1);
            let debut = self.source[..b].rfind('\n').map_or(0, |i| i + 1);
            let fin = self.source[b..]
                .find('\n')
                .map_or(self.source.len(), |i| b + i);
            debut..fin
        });
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
            let mut job = editor::highlight(buf.as_str(), &pal, font, size, &found, &issues);
            if let Some(c) = courant.clone()
                && c.end <= buf.as_str().len()
            {
                Self::estomper(&mut job, c);
            }
            job.wrap.max_width = wrap;
            ui.ctx().fonts_mut(|f| f.layout_job(job))
        };
        let bg = if focus {
            Color32::TRANSPARENT
        } else if ui.visuals().dark_mode {
            Color32::from_rgb(0x22, 0x22, 0x24)
        } else {
            Color32::from_rgb(0xFF, 0xFF, 0xFE)
        };
        let avant = self.source.clone();
        let sel_avant = self.sel;
        let hauteur = ui.available_height();
        egui::ScrollArea::vertical()
            .id_salt("source-scroll")
            .auto_shrink(false)
            .show(ui, |ui| {
                // Machine à écrire : de la marge en haut et en bas pour que la première et
                // la dernière ligne puissent venir au milieu.
                if focus && self.prefs.machine_a_ecrire {
                    ui.add_space(hauteur * 0.45);
                }
                egui::Frame::new()
                    .fill(bg)
                    .inner_margin(if mobile {
                        egui::Margin::symmetric(12, 12)
                    } else if focus {
                        egui::Margin::symmetric(8, 40)
                    } else {
                        egui::Margin::symmetric(28, 22)
                    })
                    .stroke(if focus {
                        Stroke::NONE
                    } else {
                        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
                    })
                    .show(ui, |ui| {
                        let rows = ((ui.available_height() - 40.0) / 22.0).max(8.0) as usize;
                        let out = egui::TextEdit::multiline(&mut self.source)
                            .id(id)
                            .frame(egui::Frame::NONE)
                            .hint_text(tr("Écrivez votre document. {{prénom}} crée un champ, **gras**, # Titre, - liste…"))
                            .desired_width(f32::INFINITY)
                            .desired_rows(rows)
                            .lock_focus(true)
                            .layouter(&mut layouter)
                            .show(ui);
                        if let Some(r) = out.cursor_range {
                            let rr = r.as_sorted_char_range();
                            self.sel = (rr.start.0, rr.end.0);
                            // Machine à écrire : la ligne du curseur reste au milieu.
                            let bouge = self.outils.dernier_curseur != Some(rr.end.0);
                            if focus && self.prefs.machine_a_ecrire && (bouge || out.response.changed()) {
                                let pos = out.galley.pos_from_cursor(r.primary);
                                let rect = pos.translate(out.galley_pos.to_vec2());
                                ui.scroll_to_rect(rect, Some(egui::Align::Center));
                            }
                            self.outils.dernier_curseur = Some(rr.end.0);
                        }
                        if out.response.changed() {
                            let now = ui.input(|i| i.time);
                            self.corriger_frappe(&avant, now);
                            self.suivre_frappe(&avant, sel_avant.1);
                        }
                        if !focus {
                            self.spell_overlay(ui, &out);
                        }
                    });
                if focus && self.prefs.machine_a_ecrire {
                    ui.add_space(hauteur * 0.45);
                }
            });
    }

    /// Orthographe dans l'éditeur : mots inconnus soulignés d'une vague rouge
    /// (les remarques typographiques gardent leur trait droit), et menu du clic droit.
    fn spell_overlay(&mut self, ui: &mut egui::Ui, out: &egui::text_edit::TextEditOutput) {
        if !(self.prefs.spell_live || self.prefs.inspector == Inspector::Review) {
            self.spell_menu = None;
            return;
        }
        let ctx = ui.ctx().clone();
        let lang = self.page.langue;
        // Pas de dictionnaire chargé (chargement en cours ?) : rien à souligner.
        let Some(ranges) = self.dicos.misspelled(&self.source, lang, &ctx) else {
            return;
        };
        let chars = editor::chars_of(&self.source, ranges.iter().flat_map(|r| [r.start, r.end]));
        let painter = ui
            .painter()
            .with_clip_rect(out.text_clip_rect.intersect(ui.clip_rect()));
        let color = Color32::from_rgb(0xE0, 0x3A, 0x30);
        let at = |c: usize| {
            out.galley
                .pos_from_cursor(CCursor::new(CharIndex(c)))
                .translate(out.galley_pos.to_vec2())
        };
        for pair in chars.chunks(2) {
            let (a, b) = (at(pair[0]), at(pair[1]));
            if (a.min.y - b.min.y).abs() < 1.0 {
                editor::paint_wave(&painter, a.min.x, b.min.x, a.max.y - 1.0, color);
            } else {
                let right = out.galley_pos.x + out.galley.rect.max.x;
                let left = out.galley_pos.x + out.galley.rect.min.x;
                editor::paint_wave(&painter, a.min.x, right, a.max.y - 1.0, color);
                editor::paint_wave(&painter, left, b.min.x, b.max.y - 1.0, color);
            }
        }
        let resp = &out.response.response;
        if resp.secondary_clicked()
            && let Some(pos) = resp.interact_pointer_pos()
        {
            let c = out.galley.cursor_from_pos(pos - out.galley_pos).index.0;
            let byte = editor::byte_of(&self.source, c);
            self.spell_menu = ranges
                .iter()
                .find(|r| r.start <= byte && byte <= r.end)
                .map(|r| SpellMenu {
                    start: r.start,
                    end: r.end,
                    word: self.source[r.clone()].to_string(),
                });
        }
        if self.spell_menu.is_some() {
            let mut action = None;
            let shown = resp.context_menu(|ui| action = self.spell_menu_ui(ui));
            if shown.is_none() || action.is_some() {
                self.spell_menu = None;
            }
            if let Some(a) = action {
                self.apply_spell(a);
            }
        }
    }

    fn spell_menu_ui(&mut self, ui: &mut egui::Ui) -> Option<SpellAction> {
        let m = self.spell_menu.as_ref()?;
        let (start, end, word) = (m.start, m.end, m.word.clone());
        let ctx = ui.ctx().clone();
        let mut action = None;
        ui.label(RichText::new(&word).strong().color(err(ui)));
        match self
            .dicos
            .suggestions(self.page.langue, &word, &ctx)
            .map(<[String]>::to_vec)
        {
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(tr("Suggestions…")).weak());
                });
            }
            Some(list) if list.is_empty() => {
                ui.label(RichText::new(tr("Aucune suggestion")).weak());
            }
            Some(list) => {
                for sug in list {
                    if ui.button(RichText::new(&sug).strong()).clicked() {
                        action = Some(SpellAction::Replace {
                            start,
                            end,
                            word: word.clone(),
                            with: sug,
                        });
                    }
                }
            }
        }
        ui.separator();
        if ui.button(tr("Ajouter au dictionnaire")).clicked() {
            action = Some(SpellAction::Add(word.clone()));
        }
        if ui
            .button(tr("Ignorer"))
            .on_hover_text(tr("Ne plus signaler ce mot dans ce document"))
            .clicked()
        {
            action = Some(SpellAction::Ignore(word));
        }
        if action.is_some() {
            ui.close();
        }
        action
    }

    fn apply_spell(&mut self, a: SpellAction) {
        match a {
            SpellAction::Replace {
                start,
                end,
                word,
                with,
            } => {
                if self.source.get(start..end) == Some(word.as_str()) {
                    self.history.cut = true;
                    self.source.replace_range(start..end, &with);
                }
            }
            SpellAction::Add(w) => self.dicos.add_personal(&w),
            SpellAction::Ignore(w) => self.dicos.ignore(&w),
        }
    }

    // ---------------- Champs ----------------

    /// L'inspecteur : Format, Document, Champs et Plan, comme la barre latérale de Pages.
    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let w = (ui.available_width() - 10.0) / 6.0;
            for (tab, label) in [
                (Inspector::Format, tr("Format")),
                (Inspector::Document, tr("Document")),
                (Inspector::Fields, tr("Champs")),
                (Inspector::Outline, tr("Plan")),
                (Inspector::Review, tr("Révision")),
                (Inspector::Suivi, tr("Modifications")),
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
                        ui.label(RichText::new(tr("Mise en page")).strong());
                        page_form(ui, &mut self.page, "insp-page");
                        ui.horizontal(|ui| {
                            ui.label(tr("Auteur"));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.page.auteur)
                                    .hint_text(tr("pour les métadonnées des exports")),
                            );
                        });
                        ui.add_space(8.0);
                        ui.label(RichText::new(tr("Document")).strong());
                        let eff = self.effective();
                        let st = stats(&varium_core::document(&self.source, &eff));
                        egui::Grid::new("insp-stats").num_columns(2).show(ui, |ui| {
                            for (k, v) in [
                                (tr("Pages"), self.pages.to_string()),
                                (tr("Mots"), st.words.to_string()),
                                (tr("Caractères"), st.chars.to_string()),
                                (tr("Paragraphes"), st.paragraphs.to_string()),
                            ] {
                                ui.label(RichText::new(k).weak());
                                ui.label(v);
                                ui.end_row();
                            }
                        });
                        self.statistiques_ecriture(ui);
                    });
            }
            Inspector::Outline => self.outline_tab(ui),
            Inspector::Review => self.review_tab(ui),
            Inspector::Suivi => self.revision_tab(ui),
        }
    }

    fn format_tab(&mut self, ui: &mut egui::Ui) {
        let style = editor::line_style_at(&self.source, self.sel.1);
        ui.label(RichText::new(tr("Style de paragraphe")).strong());
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
        ui.label(RichText::new(tr("Police")).strong());
        ui.horizontal(|ui| {
            self.tool(ui, Icon::Bold, tr("Gras"), Act::Wrap("**"), false);
            self.tool(ui, Icon::Italic, tr("Italique"), Act::Wrap("*"), false);
            self.tool(ui, Icon::Underline, tr("Souligné"), Act::Wrap("__"), false);
            self.tool(ui, Icon::Strike, tr("Barré"), Act::Wrap("~~"), false);
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
        ui.label(RichText::new(tr("Alignement")).strong());
        ui.horizontal(|ui| {
            let side = !matches!(style, LineStyle::Center | LineStyle::Right);
            self.tool(
                ui,
                Icon::AlignLeft,
                tr("À gauche"),
                Act::Style(LineStyle::Body),
                side,
            );
            self.tool(
                ui,
                Icon::AlignCenter,
                tr("Centré"),
                Act::Style(LineStyle::Center),
                style == LineStyle::Center,
            );
            self.tool(
                ui,
                Icon::AlignRight,
                tr("À droite"),
                Act::Style(LineStyle::Right),
                style == LineStyle::Right,
            );
            ui.checkbox(&mut self.page.justify, tr("Justifier"));
        });
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Espacement")).strong());
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
        self.format_paragraphe(ui);
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Insérer")).strong());
        ui.horizontal_wrapped(|ui| {
            for (label, act) in [
                (tr("{ } Champ"), Act::InsertField),
                (tr("= Champ calculé"), Act::InsertFormula),
                (tr("📅 Date du jour"), Act::InsertToday),
                (tr("▦ Tableau"), Act::InsertTable),
                (tr("⤓ Saut de page"), Act::InsertPageBreak),
                (tr("— Ligne"), Act::InsertRule),
            ] {
                if ui.button(label).clicked() {
                    self.queue.push(act);
                }
            }
        });
        ui.add_space(8.0);
        ui.checkbox(
            &mut self.page.typographie,
            tr("Typographie française automatique"),
        )
        .on_hover_text(tr(
            "« guillemets », apostrophe ’, points de suspension …, espaces fines avant ; : ! ?",
        ));
    }

    /// Révision : remarques d'écriture selon la langue du document.
    fn review_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(tr("Langue"));
            egui::ComboBox::from_id_salt("review-lang")
                .selected_text(self.page.langue.native())
                .show_ui(ui, |ui| {
                    for l in varium_core::Lang::ALL {
                        ui.selectable_value(&mut self.page.langue, l, l.native());
                    }
                });
            if ui
                .small_button(tr("Règles…"))
                .on_hover_text(tr("Les règles d'écriture de cette langue"))
                .clicked()
            {
                self.queue.push(Act::Languages);
            }
        });
        self.spelling_section(ui);
        ui.separator();
        let issues = varium_core::check::check(&self.source, self.page.langue);
        let fixable = issues.iter().filter(|i| i.replacement.is_some()).count();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(match issues.len() {
                    0 => tr("Aucune remarque ✔").to_string(),
                    1 => tr("1 remarque").to_string(),
                    n => trf!("{n} remarques", n),
                })
                .strong(),
            );
            if fixable > 0
                && ui
                    .button(trf!("Tout corriger ({fixable})", fixable))
                    .clicked()
            {
                self.prendre_version(varium_core::versions::Motif::Avant, "Tout corriger");
                self.history.cut = true;
                let n = varium_core::check::apply_all(&mut self.source, self.page.langue);
                let ctx = ui.ctx().clone();
                self.notify(&ctx, trf!("{n} correction(s) appliquée(s)", n), false);
            }
        });
        ui.label(
            RichText::new(tr("Les remarques sont soulignées en rouge dans le texte. Les champs et les formules ne sont pas examinés."))
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
                                ui.label(
                                    RichText::new(labels::issue(issue.kind))
                                        .small()
                                        .color(err(ui)),
                                );
                                ui.label(issue.message_in(crate::tr::lang()));
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
                                        .small_button(trf!(
                                            "Corriger : « {v} »",
                                            v = rep.replace(' ', "·")
                                        ))
                                        .clicked()
                                {
                                    apply = Some(issue.clone());
                                }
                                if ui.small_button(tr("Aller")).clicked() {
                                    goto = Some((issue.start, issue.end));
                                }
                            });
                        });
                    if k + 1 == 300 {
                        ui.label(RichText::new(tr("… (300 premières remarques)")).weak());
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

    /// Révision → Orthographe : état du dictionnaire de la langue, mots inconnus
    /// avec leurs suggestions, dictionnaire personnel.
    fn spelling_section(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let lang = self.page.langue;
        ui.add_space(4.0);
        ui.label(RichText::new(tr("Orthographe")).strong());
        let installed = self.dicos.installed(lang).cloned();
        let etat = self.dicos.etat(lang);
        if matches!(etat, dico::Etat::Installe) {
            // Le chargement démarre dès que l'onglet est ouvert.
            self.dicos.dict(lang, &ctx);
        }
        let progress = self.dicos.download_progress(lang);
        match (&etat, &progress) {
            (_, Some(p)) => {
                ui.horizontal_wrapped(|ui| {
                    ui.spinner();
                    ui.label(if p.steps == 0 {
                        tr("Téléchargement…").to_string()
                    } else {
                        trf!(
                            "Téléchargement {step}/{steps} : {file} ({v})",
                            step = p.step,
                            steps = p.steps,
                            file = p.file,
                            v = human_size(p.bytes)
                        )
                    });
                });
            }
            (dico::Etat::Absent, None) => {
                ui.label(
                    RichText::new(trf!(
                        "Aucun dictionnaire {lang} sur cet ordinateur.",
                        lang = labels::lang_inline(lang)
                    ))
                    .color(warn(ui)),
                );
                let r = dico::remote(lang);
                ui.label(
                    RichText::new(trf!("« Télécharger » l'obtient des dictionnaires de LibreOffice ({locale}, licence {licence_name}).", locale = r.locale, licence_name = tr(r.licence_name)))
                    .small()
                    .weak(),
                );
            }
            (dico::Etat::Installe | dico::Etat::Chargement, None) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(tr("Chargement du dictionnaire…"));
                });
            }
            (dico::Etat::Pret, None) => {
                if let Some(d) = &installed {
                    ui.label(trf!("✔ Dictionnaire {locale}", locale = d.locale))
                        .on_hover_text(d.dic.display().to_string());
                }
            }
            (dico::Etat::Erreur(e), None) => {
                ui.label(RichText::new(e).color(err(ui)));
            }
        }
        if let Some(d) = &installed {
            let origine = if d.own {
                tr("dossier de Varium")
            } else {
                tr("installé sur l'ordinateur")
            };
            let lic = d.licence_name.as_deref().unwrap_or(tr("non précisée"));
            egui::CollapsingHeader::new(RichText::new(trf!("Licence : {lic}", lic)).small().weak())
                .id_salt("spell-licence")
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!("{} ({origine}) : {}", d.locale, d.aff.display()))
                            .small()
                            .weak(),
                    );
                    match d.licence.as_deref().and_then(|p| std::fs::read(p).ok()) {
                        Some(bytes) => {
                            let text = dico::decode(&bytes, "");
                            egui::ScrollArea::vertical()
                                .id_salt("spell-licence-text")
                                .max_height(140.0)
                                .show(ui, |ui| {
                                    ui.label(RichText::new(text).small().monospace());
                                });
                        }
                        None => {
                            ui.label(
                                RichText::new(tr(
                                    "Pas de fichier de licence à côté du dictionnaire.",
                                ))
                                .small()
                                .weak(),
                            );
                        }
                    }
                });
        }
        ui.horizontal_wrapped(|ui| {
            let own = installed.as_ref().is_some_and(|d| d.own);
            if progress.is_none() && !own {
                let r = dico::remote(lang);
                if ui
                    .button(tr("⤓ Télécharger"))
                    .on_hover_text(trf!("Dictionnaire {locale} des dictionnaires de LibreOffice (licence {licence_name}), enregistré dans le dossier de Varium", locale = r.locale, licence_name = tr(r.licence_name)))
                    .clicked()
                {
                    self.dicos.download(lang, &ctx);
                }
            }
            if ui
                .button(tr("Importer…"))
                .on_hover_text(trf!("Un dictionnaire Hunspell (.aff et .dic) ou une extension LibreOffice (.oxt), copié dans {own_dir}", own_dir = self.dicos.own_dir().display()))
                .clicked()
            {
                self.dicos.import_dialog(&ctx);
            }
        });
        ui.checkbox(&mut self.prefs.spell_live, tr("Vérifier pendant la frappe"))
            .on_hover_text(tr(
                "Souligner les mots inconnus d'une vague rouge, même quand cet onglet est fermé",
            ));

        let Some(ranges) = self.dicos.misspelled(&self.source, lang, &ctx) else {
            return;
        };
        ui.label(
            RichText::new(match ranges.len() {
                0 => tr("Aucune faute d'orthographe ✔").to_string(),
                1 => tr("1 mot inconnu").to_string(),
                n => trf!("{n} mots inconnus", n),
            })
            .strong(),
        );
        let mut action: Option<SpellAction> = None;
        let mut goto: Option<(usize, usize)> = None;
        egui::ScrollArea::vertical()
            .id_salt("spell-list")
            .max_height(260.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for r in ranges.iter().take(100) {
                    let word = self.source[r.clone()].to_string();
                    egui::Frame::group(ui.style())
                        .inner_margin(6.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal_wrapped(|ui| {
                                if ui
                                    .link(RichText::new(&word).strong().color(err(ui)))
                                    .on_hover_text(tr("Aller au mot"))
                                    .clicked()
                                {
                                    goto = Some((r.start, r.end));
                                }
                                match self
                                    .dicos
                                    .suggestions(lang, &word, &ctx)
                                    .map(<[String]>::to_vec)
                                {
                                    None => {
                                        ui.spinner();
                                    }
                                    Some(list) if list.is_empty() => {
                                        ui.label(RichText::new(tr("pas de suggestion")).weak());
                                    }
                                    Some(list) => {
                                        ui.label(RichText::new("→").weak());
                                        for sug in list.into_iter().take(4) {
                                            if ui
                                                .small_button(&sug)
                                                .on_hover_text(tr("Remplacer"))
                                                .clicked()
                                            {
                                                action = Some(SpellAction::Replace {
                                                    start: r.start,
                                                    end: r.end,
                                                    word: word.clone(),
                                                    with: sug,
                                                });
                                            }
                                        }
                                    }
                                }
                            });
                            ui.horizontal(|ui| {
                                if ui.small_button(tr("Ajouter au dictionnaire")).clicked() {
                                    action = Some(SpellAction::Add(word.clone()));
                                }
                                if ui
                                    .small_button(tr("Ignorer"))
                                    .on_hover_text(tr("Ne plus signaler ce mot dans ce document"))
                                    .clicked()
                                {
                                    action = Some(SpellAction::Ignore(word.clone()));
                                }
                            });
                        });
                }
                if ranges.len() > 100 {
                    ui.label(RichText::new(tr("… (100 premiers mots)")).weak());
                }
            });
        if let Some(a) = action {
            self.apply_spell(a);
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
        let n = self.dicos.personal.len();
        let mut remove = None;
        egui::CollapsingHeader::new(if n > 1 {
            trf!("Dictionnaire personnel ({n} mots)", n)
        } else {
            trf!("Dictionnaire personnel ({n} mot)", n)
        })
        .id_salt("spell-personal")
        .show(ui, |ui| {
            if n == 0 {
                ui.label(
                    RichText::new(tr(
                        "« Ajouter au dictionnaire » y range un mot, pour tous les documents.",
                    ))
                    .small()
                    .weak(),
                );
            }
            for w in &self.dicos.personal {
                ui.horizontal(|ui| {
                    if ui.small_button("✖").on_hover_text(tr("Retirer")).clicked() {
                        remove = Some(w.clone());
                    }
                    ui.label(w);
                });
            }
            if !self.dicos.ignored.is_empty() {
                ui.label(
                    RichText::new(trf!(
                        "Ignorés dans ce document : {v}",
                        v = self
                            .dicos
                            .ignored
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .small()
                    .weak(),
                );
            }
        });
        if let Some(w) = remove {
            self.dicos.remove_personal(&w);
        }
    }

    fn outline_tab(&mut self, ui: &mut egui::Ui) {
        let plan = editor::outline(&self.source);
        let eff = self.effective();
        ui.label(
            RichText::new(trf!(
                "{v} page(s) · {plan} titre(s)",
                v = self.pages.max(1),
                plan = plan.len()
            ))
            .weak(),
        );
        ui.add_space(4.0);
        if plan.is_empty() {
            ui.label(
                RichText::new(tr("Pas encore de titre. Choisissez « Titre 1 » dans l'onglet Format, ou écrivez # au début d'une ligne."))
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
                        if ui.link(t).on_hover_text(tr("Aller à ce titre")).clicked() {
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
            ui.heading(tr("Champs"));
            ui.label(
                RichText::new(vars.len().to_string())
                    .strong()
                    .color(accent(ui)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button(tr("+ Champ"))
                    .on_hover_text(tr("Insérer un champ dans le texte"))
                    .clicked()
                {
                    self.queue.push(Act::InsertField);
                }
            });
        });
        if !vars.is_empty() {
            ui.add(
                egui::ProgressBar::new(filled as f32 / vars.len() as f32)
                    .text(trf!(
                        "{filled} / {vars} renseignés",
                        filled,
                        vars = vars.len()
                    ))
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
                        RichText::new(tr("Aucun champ. Écrivez {{prénom}} dans le texte, ou utilisez « + Champ ».")).weak(),
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
                    ui.label(RichText::new(tr("Champs calculés")).strong());
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
                egui::CollapsingHeader::new(tr("Filtres disponibles")).show(ui, |ui| {
                    egui::Grid::new("filters").num_columns(2).striped(true).show(ui, |ui| {
                        for (f, desc) in filters::KNOWN {
                            ui.label(RichText::new(*f).monospace());
                            ui.label(*desc);
                            ui.end_row();
                        }
                    });
                    ui.label(
                        RichText::new(tr("Exemple : {{montant | money:usd}}, {{nom | trim | upper}}"))
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
                        self.prendre_version(
                            varium_core::versions::Motif::Avant,
                            "Renommer partout",
                        );
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
                            trf!("« {from} » renommé en « {to} » partout", from, to),
                            false,
                        );
                    }
                    Err(e) => self.notify(&ctx, e, true),
                }
            }
            Some(VarAction::Inline(name)) => {
                self.prendre_version(varium_core::versions::Motif::Avant, "Figer un champ");
                self.history.cut = true;
                self.source = varium_core::inline_variable(&self.source, &name, &self.effective());
                self.notify(&ctx, trf!("« {name} » figé dans le texte", name), false);
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
                    ui.label(RichText::new(tr("✉ Publipostage")).strong());
                    if ui.small_button("◀").clicked() && m.record > 0 {
                        m.record -= 1;
                    }
                    ui.label(format!("{} / {n}", m.record + 1));
                    if ui.small_button("▶").clicked() && m.record + 1 < n {
                        m.record += 1;
                    }
                    stop = ui.small_button(tr("Terminer")).clicked();
                });
                ui.label(
                    RichText::new(trf!(
                        "« {file} » remplit les champs de même nom.",
                        file = m.file
                    ))
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
                    .on_hover_text(trf!("{count} occurrences dans le document", count));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let has_value = !self.values.get(name).is_empty();
                if ui
                    .add_enabled(has_value, egui::Button::new("⤓").small())
                    .on_hover_text(tr("Figer : remplacer le champ par sa valeur dans le texte"))
                    .clicked()
                {
                    action = Some(VarAction::Inline(name.to_string()));
                }
                if ui
                    .small_button("✏")
                    .on_hover_text(tr("Renommer partout"))
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
                commit |= ui.button(tr("Renommer")).clicked();
                cancel = ui.button(tr("Annuler")).clicked();
            });
            let to = r.to.trim().to_string();
            if !varium_core::is_valid_name(&to) {
                ui.label(
                    RichText::new(tr("Nom invalide (lettres, chiffres, _ . -)"))
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
                    RichText::new(trf!("« {to} » existe déjà : les deux seront fusionnés", to))
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
                .on_hover_text(tr("Valeur lue dans le fichier du publipostage"));
        } else {
            let value = self.values.values.entry(name.to_string()).or_default();
            value_editor(ui, name, ty, value);
        }

        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt(("type", name))
                .selected_text(labels::var_type(ty))
                .width(104.0)
                .show_ui(ui, |ui| {
                    for t in VarType::ALL {
                        ui.selectable_value(&mut ty, t, labels::var_type(t));
                    }
                });
            let value = self.values.get(name).to_string();
            if let Some(msg) = ty.validate(&value) {
                ui.label(RichText::new(msg).small().color(err(ui)));
            } else if name == TODAY && value.is_empty() {
                ui.label(
                    RichText::new(tr("date du jour, automatique"))
                        .small()
                        .weak(),
                );
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
        let vue = self.vue_page();
        let r = pageview::preparer(&self.source, &eff, &self.page, &format!("{vue:?}"), || {
            varium_core::critic::avec_vue(vue, || export::prepare(&self.source, &eff, &self.page))
        });
        let view = View {
            rendu: &r,
            page: &self.page,
            zoom: self.prefs.zoom.unwrap_or(1.0),
            highlight: self.prefs.highlight,
        };
        // Vignettes des pages, à gauche (Affichage → Vignettes des pages).
        if pageview::vignettes_visibles(ui.ctx()) {
            egui::Panel::left("vignettes-pages")
                .resizable(true)
                .size_range(110.0..=260.0)
                .default_size(150.0)
                .show(ui, |ui| pageview::vignettes(ui, &view));
        }
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
                    self.pages = pageview::show(ui, &View { zoom, ..view });
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
            ui.label(if pages > 1 {
                trf!("{pages} pages", pages)
            } else {
                trf!("{pages} page", pages)
            });
            ui.separator();
            ui.label(trf!(
                "{words} mots, {chars} caractères",
                words = st.words,
                chars = st.chars
            ))
            .on_hover_text(tr("Outils → Nombre de mots"));
            self.objectif_statut(ui, st.words);
            ui.separator();
            ui.label(trf!("{filled}/{vars} champs", filled, vars = vars.len()));
            self.badge_statut(ui);
            self.voix_statut(ui);
            if self.prefs.suivi {
                ui.separator();
                if ui
                    .small_button(RichText::new(tr("✎ Suivi des modifications")).color(accent(ui)))
                    .on_hover_text(tr(
                        "Ce que vous tapez est marqué ; cliquez pour ouvrir le panneau",
                    ))
                    .clicked()
                {
                    self.queue.push(Act::Revision);
                }
            }
            if let Some(m) = &self.merge {
                ui.separator();
                ui.label(format!("✉ {}/{}", m.record + 1, m.sheet.rows.len()));
            }
            ui.separator();
            ui.label(match self.page.paper {
                Paper::A4 => "A4",
                Paper::A5 => "A5",
                Paper::Letter => tr("Lettre US"),
                Paper::Legal => tr("Légal US"),
            });
            ui.label(labels::lang(self.page.langue));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = match self.prefs.zoom {
                    None => tr("Largeur").to_string(),
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
                    .on_hover_text(tr("Ajuster à la largeur"))
                    .clicked()
                {
                    self.queue.push(Act::ZoomFit);
                }
            });
        });
    }

    // ---------------- Dialogues ----------------

    fn dialogs(&mut self, ctx: &egui::Context) {
        self.signature_fenetre(ctx);
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
                    ui.heading(tr("Insérer un tableau"));
                    egui::Grid::new("tbl").num_columns(2).show(ui, |ui| {
                        ui.label(tr("Colonnes"));
                        ui.add(egui::DragValue::new(cols).range(1..=12));
                        ui.end_row();
                        ui.label(tr("Lignes (avec l'en-tête)"));
                        ui.add(egui::DragValue::new(rows).range(2..=50));
                        ui.end_row();
                    });
                    ui.label(RichText::new(tr("Écrivez dans les cellules entre les « | ».")).small().weak());
                    ui.horizontal(|ui| {
                        if ui.button(tr("Insérer")).clicked() {
                            let t = editor::table_markup(*rows, *cols);
                            self.insert_block(t.trim_end());
                            keep = false;
                        }
                        if ui.button(tr("Annuler")).clicked() {
                            keep = false;
                        }
                    });
                }
                Dialog::Field { name, ty } => {
                    ui.heading(tr("Insérer un champ"));
                    ui.label(tr("Le champ prend la même valeur partout où il apparaît."));
                    let r = ui.add(
                        egui::TextEdit::singleline(name)
                            .hint_text(tr("prénom, montant, date_rdv…"))
                            .desired_width(f32::INFINITY),
                    );
                    r.request_focus();
                    egui::ComboBox::from_label(tr("Type"))
                        .selected_text(labels::var_type(*ty))
                        .show_ui(ui, |ui| {
                            for t in VarType::ALL {
                                ui.selectable_value(ty, t, labels::var_type(t));
                            }
                        });
                    let existing = varium_core::variables(&self.source);
                    if !existing.is_empty() {
                        ui.label(RichText::new(tr("Ou reprendre un champ existant :")).small().weak());
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
                        ui.label(RichText::new(tr("Lettres, chiffres, _ . - (sans espace)")).small().color(err(ui)));
                    }
                    ui.horizontal(|ui| {
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        if (ui.add_enabled(valid, egui::Button::new(tr("Insérer"))).clicked() || (enter && valid)) && valid {
                            if *ty != VarType::Text {
                                self.values.types.insert(n.clone(), *ty);
                            }
                            self.insert(&format!("{{{{{n}}}}}"), None);
                            keep = false;
                        }
                        if ui.button(tr("Annuler")).clicked() {
                            keep = false;
                        }
                    });
                }
                Dialog::Formula { expr } => {
                    ui.heading(tr("Insérer un champ calculé"));
                    ui.label(tr("Opérations + − × ÷, parenthèses, pourcentages (20%) et fonctions arrondi(x; 2), min, max, somme."));
                    let r = ui.add(
                        egui::TextEdit::singleline(expr)
                            .hint_text(tr("quantité * prix_unitaire"))
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
                        Ok(n) => ui.label(trf!("Résultat avec les valeurs actuelles : {v}", v = filters::format_number(*n, None))),
                        Err(e) if !expr.trim().is_empty() => ui.label(RichText::new(e).color(warn(ui))),
                        _ => ui.label(""),
                    };
                    ui.horizontal(|ui| {
                        let ok = !expr.trim().is_empty();
                        if ui.add_enabled(ok, egui::Button::new(tr("Insérer"))).clicked() {
                            self.insert(&format!("{{{{= {}}}}}", expr.trim()), None);
                            keep = false;
                        }
                        if ui.add_enabled(ok, egui::Button::new(tr("Insérer en euros"))).clicked() {
                            self.insert(&format!("{{{{= {} | money}}}}", expr.trim()), None);
                            keep = false;
                        }
                        if ui.button(tr("Annuler")).clicked() {
                            keep = false;
                        }
                    });
                }
                Dialog::Stats => {
                    let eff = self.effective();
                    let st = stats(&varium_core::document(&self.source, &eff));
                    ui.heading(tr("Nombre de mots"));
                    egui::Grid::new("stats").num_columns(2).striped(true).show(ui, |ui| {
                        for (k, v) in [
                            (tr("Pages"), self.pages.to_string()),
                            (tr("Mots"), st.words.to_string()),
                            (tr("Caractères (espaces comprises)"), st.chars.to_string()),
                            (tr("Paragraphes"), st.paragraphs.to_string()),
                            (tr("Champs"), varium_core::variables(&self.source).len().to_string()),
                            (tr("Champs calculés"), varium_core::formulas(&self.source).len().to_string()),
                        ] {
                            ui.label(k);
                            ui.label(RichText::new(v).strong());
                            ui.end_row();
                        }
                    });
                    keep = !ui.button(tr("Fermer")).clicked();
                }
                Dialog::Shortcuts => {
                    ui.heading(tr("Raccourcis clavier"));
                    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                        egui::Grid::new("sc").num_columns(2).striped(true).show(ui, |ui| {
                            for (s, a) in SHORTCUTS {
                                ui.label(RichText::new(ctx.format_shortcut(s)).monospace());
                                ui.label(act_label(*a));
                                ui.end_row();
                            }
                        });
                    });
                    keep = !ui.button(tr("Fermer")).clicked();
                }
                Dialog::Languages(lang) => {
                    ui.heading(tr("Écrire en plusieurs langues"));
                    ui.label(tr("Choisissez la langue du document (Document → Langue) : les dates, les nombres, les montants, la typographie automatique et la révision suivent ses règles."));
                    ui.label(RichText::new(tr("Orthographe : Varium vérifie les mots avec un dictionnaire Hunspell, le format de LibreOffice. Il emploie ceux déjà installés sur l'ordinateur (LibreOffice, dictionnaires du système) ; sinon, Révision → Orthographe propose de télécharger celui de la langue depuis les dictionnaires de LibreOffice (chacun avec sa licence libre, affichée) ou d'en importer un (.aff et .dic, ou extension .oxt). Les mots inconnus sont soulignés d'une vague rouge ; un clic droit propose des corrections, « Ajouter au dictionnaire » (dictionnaire personnel, pour tous les documents) ou « Ignorer » (pour ce document).")));
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
                        if ui.button(trf!("Écrire ce document en {native}", native = lang.native())).clicked() {
                            self.page.langue = *lang;
                            keep = false;
                        }
                        if ui.button(tr("Fermer")).clicked() {
                            keep = false;
                        }
                    });
                }
                Dialog::Syntax => {
                    ui.heading(tr("Guide de la syntaxe"));
                    egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                        egui::Grid::new("syn").num_columns(2).striped(true).show(ui, |ui| {
                            for (k, v) in SYNTAX {
                                ui.label(RichText::new(tr(k)).monospace());
                                ui.label(tr(v));
                                ui.end_row();
                            }
                        });
                    });
                    keep = !ui.button(tr("Fermer")).clicked();
                }
                Dialog::Confirm(p) => {
                    ui.heading(tr("Enregistrer les modifications ?"));
                    ui.label(trf!("« {title} » contient des modifications non enregistrées.", title = self.title.trim()));
                    ui.label(RichText::new(tr("Sans enregistrement, elles seront perdues.")).weak());
                    ui.add_space(6.0);
                    let mut reponse = None;
                    ui.horizontal(|ui| {
                        if ui.button(tr("Enregistrer")).clicked() {
                            reponse = Some(files::Reponse::Enregistrer);
                        }
                        if ui.button(tr("Ne pas enregistrer")).clicked() {
                            reponse = Some(files::Reponse::NePasEnregistrer);
                        }
                        if ui.button(tr("Annuler")).clicked() {
                            reponse = Some(files::Reponse::Annuler);
                        }
                    });
                    if let Some(r) = reponse {
                        keep = false;
                        let suite = files::apres_confirmation(r, p.clone(), || {
                            match self.save_project(ctx, false) {
                                SaveOutcome::Done => files::Issue::Fait,
                                SaveOutcome::Asking => files::Issue::Demande,
                                SaveOutcome::Failed => files::Issue::Echec,
                            }
                        });
                        match suite {
                            files::Suite::Faire(p) => self.proceed(ctx, p),
                            files::Suite::ApresEnregistrement(p) => self.after_save = Some(p),
                            files::Suite::Abandonner => {}
                        }
                    }
                }
                Dialog::Restore {
                    project,
                    secours,
                    document,
                } => {
                    ui.heading(tr("Restaurer la sauvegarde de secours ?"));
                    let title = if project.title.trim().is_empty() {
                        tr("le document")
                    } else {
                        project.title.trim()
                    };
                    ui.label(trf!("Varium s'est arrêté sans que « {title} » soit enregistré. Une sauvegarde automatique plus récente existe.", title));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr("Restaurer")).clicked() {
                            keep = false;
                            let p = (**project).clone();
                            self.restaurer(p, document.clone());
                            secours::effacer(secours);
                        }
                        if ui.button(tr("Ignorer")).clicked() {
                            keep = false;
                            secours::effacer(secours);
                        }
                    });
                }
                Dialog::Objectif(saisie) => keep = self.objectif_dialogue(ui, saisie),
                Dialog::Marquer(nom) => keep = self.marquer_dialogue(ui, nom),
                Dialog::Qr(saisie) => keep = self.qr_dialogue(ui, saisie),
                Dialog::Detection(saisie) => keep = self.detection_dialogue(ui, saisie),
                Dialog::Commentaire(texte) => keep = self.commentaire_dialogue(ui, texte),
                Dialog::Decouper(d) => keep = self.decoupe_dialogue(ui, d),
                Dialog::Assistant(a) => keep = self.assistant_dialogue(ui, a),
                Dialog::About => {
                    ui.heading("Varium");
                    ui.label(RichText::new(tr("Un document. Toutes ses possibilités.")).italics());
                    ui.label(trf!("Version {v}", v = env!("CARGO_PKG_VERSION")));
                    ui.label(tr("Éditeur de documents à champs, écrit en Rust avec egui. Polices Liberation (licence SIL OFL 1.1)."));
                    ui.hyperlink_to(tr("Code source"), "https://github.com/maxlestage/VARIUM");
                    ui.hyperlink_to(tr("Site de Varium"), "https://maxlestage.github.io/VARIUM/");
                    keep = !ui.button(tr("Fermer")).clicked();
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
        ui.heading(tr("Nouveau à partir d'un modèle"));
        ui.label(
            RichText::new(tr(
                "Le document actuel reste récupérable avec Annuler (Ctrl+Z).",
            ))
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
                                            ui.label(RichText::new(tr(t.name)).strong());
                                            ui.label(
                                                RichText::new(tr(t.description)).small().weak(),
                                            );
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
        let mut intelligent = None;
        self.modeles_intelligents(ui, &mut intelligent);
        if let Some(m) = intelligent {
            let ctx = ui.ctx().clone();
            self.ouvrir_modele_intelligent(&ctx, m);
            return matches!(self.dialog, Some(Dialog::Templates));
        }
        ui.add_space(6.0);
        let mut keep = true;
        ui.horizontal(|ui| {
            if ui.button(tr("Document vierge")).clicked() {
                self.queue.push(Act::NewBlank);
                keep = false;
            }
            if ui.button(tr("Annuler")).clicked() {
                keep = false;
            }
        });
        if let Some(i) = chosen {
            self.load(templates::ALL[i].project());
            let ctx = ui.ctx().clone();
            self.notify(
                &ctx,
                trf!(
                    "Modèle « {name} » ouvert",
                    name = tr(templates::ALL[i].name)
                ),
                false,
            );
            return false;
        }
        keep
    }

    fn page_dialog(&mut self, ui: &mut egui::Ui) -> bool {
        ui.heading(tr("Mise en page"));
        page_form(ui, &mut self.page, "page");
        ui.add_space(6.0);
        let mut keep = true;
        ui.horizontal(|ui| {
            if ui.button("OK").clicked() {
                keep = false;
            }
            if ui.button(tr("Par défaut")).clicked() {
                self.page = PageSetup::default();
            }
        });
        keep
    }

    fn merge_dialog(&mut self, ui: &mut egui::Ui) -> bool {
        ui.heading(tr("Publipostage"));
        ui.label(tr("Un document par ligne d'un tableau CSV (Excel, LibreOffice Calc, Google Sheets…). La première ligne donne les noms des colonnes : une colonne remplit le champ de même nom."));
        ui.add_space(4.0);
        let mut keep = true;
        let ctx = ui.ctx().clone();
        let names = self.variable_names();
        match &mut self.merge {
            None => {
                if ui.button(tr("📂 Choisir un fichier CSV…")).clicked() {
                    self.io.open(&ctx, OpenKind::Data);
                }
                ui.label(
                    RichText::new(tr("Exemple : prénom;nom;ville"))
                        .small()
                        .monospace()
                        .weak(),
                );
            }
            Some(m) => {
                let n = m.sheet.rows.len();
                ui.label(
                    RichText::new(trf!(
                        "« {file} » : {n} destinataire(s), {headers} colonne(s)",
                        file = m.file,
                        n,
                        headers = m.sheet.headers.len()
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
                                        Some(c) => ui.label(trf!(
                                            "✔ colonne « {v} »",
                                            v = m.sheet.headers[c]
                                        )),
                                        None => {
                                            ui.label(RichText::new(tr("valeur du document")).weak())
                                        }
                                    };
                                    ui.end_row();
                                }
                            });
                    });
                ui.horizontal(|ui| {
                    ui.label(tr("Aperçu :"));
                    if ui.small_button("◀").clicked() && m.record > 0 {
                        m.record -= 1;
                    }
                    ui.label(format!("{} / {n}", m.record + 1));
                    if ui.small_button("▶").clicked() && m.record + 1 < n {
                        m.record += 1;
                    }
                });
                ui.separator();
                ui.label(trf!(
                    "Créer les {n} documents dans un seul fichier, une page par destinataire :",
                    n
                ));
                ui.horizontal_wrapped(|ui| {
                    if ui.button(tr("LibreOffice (.odt)")).clicked() {
                        self.queue.push(Act::Export(Format::Odt));
                    }
                    if ui.button(tr("Word (.docx)")).clicked() {
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
                    if ui.button(tr("Autre fichier…")).clicked() {
                        self.io.open(&ctx, OpenKind::Data);
                    }
                    if ui.button(tr("Terminer le publipostage")).clicked() {
                        self.merge = None;
                        keep = false;
                    }
                });
            }
        }
        if ui.button(tr("Fermer")).clicked() {
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
        // Mode concentration : ni menus, ni barres, ni panneaux.
        if self.outils.concentration {
            self.concentration_ui(ui);
            return;
        }
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
        if self.outils.formulaire {
            self.formulaire_ui(ui);
            return;
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
                        .on_hover_text(tr("Rétablir"))
                        .clicked()
                    {
                        self.queue.push(Act::Redo);
                    }
                    if icon_button(ui, Icon::Undo, false)
                        .on_hover_text(tr("Annuler"))
                        .clicked()
                    {
                        self.queue.push(Act::Undo);
                    }
                    if icon_button(ui, Icon::Pdf, false)
                        .on_hover_text(tr("Exporter en PDF"))
                        .clicked()
                    {
                        self.queue.push(Act::Export(Format::Pdf));
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut self.title)
                            .hint_text(tr("Titre"))
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
                    (Tab::Text, "✏", tr("Texte").to_string()),
                    (
                        Tab::Fields,
                        "{ }",
                        trf!("Champs {filled}/{vars}", filled, vars = vars.len()),
                    ),
                    (
                        Tab::Page,
                        "📄",
                        if pages > 1 {
                            trf!("Pages ({pages})", pages)
                        } else {
                            trf!("Page ({pages})", pages)
                        },
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

enum SpellAction {
    Replace {
        start: usize,
        end: usize,
        word: String,
        with: String,
    },
    Add(String),
    Ignore(String),
}

enum VarAction {
    Rename { from: String, to: String },
    Inline(String),
}

impl eframe::App for VariumApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_io(&ctx);
        self.dicos.poll(&ctx);
        for (msg, error) in std::mem::take(&mut self.dicos.messages) {
            self.notify(&ctx, msg, error);
        }

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

        let mobile = ctx.content_rect().width() < MOBILE_WIDTH;
        self.apply_style(&ctx, mobile);
        if mobile {
            self.mobile(ui);
        } else {
            self.desktop(ui);
        }
        self.dialogs(&ctx);
        self.historique_panneau(&ctx);

        for act in std::mem::take(&mut self.queue) {
            self.run(&ctx, act);
        }
        // Le nombre de pages sert aussi quand la page n'est pas affichée.
        if (mobile && self.tab != Tab::Page) || (!mobile && self.prefs.layout == Layout::Text) {
            let eff = self.effective();
            let r = pageview::preparer(&self.source, &eff, &self.page, "", || {
                export::prepare(&self.source, &eff, &self.page)
            });
            self.pages = pageview::count_pages(
                &ctx,
                &View {
                    rendu: &r,
                    page: &self.page,
                    zoom: 1.0,
                    highlight: false,
                },
            );
        }
        if self.etat.voix.etat != voix::EtatLecture::Arret {
            if self.prefs.layout == Layout::Page && !mobile {
                // L'éditeur n'est pas affiché : la lecture avance quand même.
                let _ = self.etat.voix.suivre();
            }
            if let Some(m) = self.etat.voix.message.take() {
                self.notify(&ctx, m, true);
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        } else if let Some(m) = self.etat.voix.message.take() {
            self.notify(&ctx, m, true);
        }
        let now = ctx.input(|i| i.time);
        self.track(now);
        self.tick_secours(&ctx, now);
        self.versions_tick(&ctx, now);
        self.toast(&ctx, if mobile { 70.0 } else { 34.0 });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.project());
        self.prefs.dernier_fichier = self.path.as_ref().map(|p| p.display().to_string());
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
            ui.label(tr("Format"));
            egui::ComboBox::from_id_salt((id, "paper"))
                .selected_text(labels::paper(p.paper))
                .show_ui(ui, |ui| {
                    for x in Paper::ALL {
                        ui.selectable_value(&mut p.paper, x, labels::paper(x));
                    }
                });
            ui.end_row();
            ui.label(tr("Orientation"));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut p.landscape, false, tr("Portrait"));
                ui.selectable_value(&mut p.landscape, true, tr("Paysage"));
            });
            ui.end_row();
            ui.label(tr("Marges (mm)"));
            ui.horizontal_wrapped(|ui| {
                for (l, v) in [
                    (tr("haut"), &mut p.margin_top),
                    (tr("bas"), &mut p.margin_bottom),
                    (tr("gauche"), &mut p.margin_left),
                    (tr("droite"), &mut p.margin_right),
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
            ui.label(tr("Police"));
            egui::ComboBox::from_id_salt((id, "font"))
                .selected_text(p.font.label())
                .show_ui(ui, |ui| {
                    for f in [DocFont::Serif, DocFont::Sans] {
                        ui.selectable_value(&mut p.font, f, f.label());
                    }
                });
            ui.end_row();
            ui.label(tr("Taille"));
            ui.add(
                egui::DragValue::new(&mut p.font_size)
                    .range(8.0..=24.0)
                    .speed(0.5)
                    .custom_formatter(|v, _| format!("{} pt", fr(v))),
            );
            ui.end_row();
            ui.label(tr("Interligne"));
            ui.horizontal(|ui| {
                for (v, l) in [
                    (1.0, tr("simple")),
                    (1.15, "1,15"),
                    (1.5, "1,5"),
                    (2.0, tr("double")),
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
            ui.label(tr("Texte"));
            ui.checkbox(&mut p.justify, tr("Justifié"));
            ui.end_row();
            ui.label(tr("En-tête"));
            ui.add(
                egui::TextEdit::singleline(&mut p.header)
                    .hint_text(tr("ex. {{entreprise}} — Confidentiel")),
            );
            ui.end_row();
            ui.label(tr("Pied de page"));
            ui.add(
                egui::TextEdit::singleline(&mut p.footer)
                    .hint_text(tr("ex. **{{entreprise}}** · SIRET {{siret}}")),
            );
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut p.page_numbers, tr("Numéros de page « Page 1 sur 3 »"));
            ui.end_row();
            ui.label(tr("Langue"));
            egui::ComboBox::from_id_salt((id, "langue"))
                .selected_text(labels::lang(p.langue))
                .show_ui(ui, |ui| {
                    for l in varium_core::Lang::ALL {
                        ui.selectable_value(&mut p.langue, l, labels::lang(l));
                    }
                });
            ui.end_row();
            ui.label(tr("Typographie"));
            ui.checkbox(&mut p.typographie, tr("Automatique (selon la langue)"))
                    .on_hover_text(tr("« guillemets », apostrophe ’, points de suspension …, espaces fines avant ; : ! ?"));
            ui.end_row();
        });
}

/// Nombre à la française : `12,5`, `20`.
/// Taille lisible : « 1,2 Mo ».
fn human_size(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        trf!("{v} Mo", v = fr((bytes as f64 / 100_000.0).round() / 10.0))
    } else {
        trf!("{n} ko", n = bytes.div_ceil(1000))
    }
}

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
                    if ui.small_button("✕").on_hover_text(tr("Effacer")).clicked() {
                        value.clear();
                    }
                }
                Err(_) => {
                    ui.add(
                        egui::TextEdit::singleline(value)
                            .hint_text("AAAA-MM-JJ")
                            .desired_width(110.0),
                    );
                    if ui.button(tr("📅 Aujourd'hui")).clicked() {
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
                VarType::Number => tr("ex. 1234,5"),
                VarType::Money => tr("ex. 12500"),
                VarType::Email => tr("nom@exemple.fr"),
                _ => tr("Valeur…"),
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
        Icon::Bold => text(p, tr("G"), serif(true, false), ink),
        Icon::Italic => text(p, tr("I"), serif(false, true), ink),
        Icon::Underline => {
            text(p, tr("S"), serif(false, false), ink);
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
    ("« ", n_("guillemet ouvrant")),
    (" »", n_("guillemet fermant")),
    ("—", n_("tiret cadratin")),
    ("–", n_("tiret demi-cadratin")),
    ("…", n_("points de suspension")),
    ("€", n_("euro")),
    ("°", n_("degré")),
    ("§", n_("paragraphe")),
    ("©", n_("copyright")),
    ("®", n_("marque déposée")),
    ("™", n_("marque")),
    ("✓", n_("coche")),
    ("→", n_("flèche")),
    ("\u{a0}", n_("espace insécable")),
];

const SYNTAX: &[(&str, &str)] = &[
    (n_("{{nom}}"), n_("un champ : la même valeur partout")),
    (
        n_("{{nom | upper}}"),
        n_("un filtre (upper, lower, capitalize, title, trim)"),
    ),
    (
        n_("{{montant | money}}"),
        n_("un montant : 12 500,00 € (money:usd…)"),
    ),
    (
        "{{date | date:full}}",
        n_("une date : lundi 3 novembre 2026 (date, date:short)"),
    ),
    (
        "{{x | default:\"…\"}}",
        n_("une valeur si le champ est vide"),
    ),
    (
        n_("{{= q * prix}}"),
        n_("un champ calculé (+ − * / %, arrondi, min, max, somme)"),
    ),
    ("{{aujourdhui}}", n_("la date du jour, automatiquement")),
    (
        n_("{{qr:https://exemple.fr}}"),
        n_("un code QR (sur sa ligne) ; {{site | qr}} pour la valeur d'un champ"),
    ),
    (
        n_("[texte](https://exemple.fr)"),
        n_("un lien (Ctrl+V d'une adresse sur une sélection)"),
    ),
    (
        n_("# Titre  ## Sous-titre"),
        n_("titres (Ctrl+1, Ctrl+2, Ctrl+3)"),
    ),
    (
        n_("**gras**  *italique*"),
        n_("caractères (Ctrl+B, Ctrl+I)"),
    ),
    (n_("__souligné__  ~~barré~~"), n_("caractères (Ctrl+U)")),
    (n_("- élément   1. élément"), n_("listes (Maj+F12, F12)")),
    (n_("> citation"), n_("citation")),
    (n_("-> centré <-"), n_("centré (Ctrl+E)")),
    (n_("-> à droite ->"), n_("aligné à droite (Ctrl+R)")),
    ("| a | b |", n_("tableau ; |---|---:| sous l'en-tête")),
    ("---", n_("ligne horizontale")),
    ("\\newpage", n_("saut de page (Ctrl+Entrée)")),
    ("\\sommaire", n_("table des matières (liste des titres)")),
    (
        "$x^2$   $$ … $$",
        n_("formules LaTeX, en ligne ou centrées"),
    ),
    ("\\{{x}}  \\*  \\$", n_("caractères littéraux")),
    (
        "{++ajout++}  {--retrait--}",
        n_("modification suivie (Outils → Révision)"),
    ),
    ("{~~ancien~>nouveau~~}", n_("remplacement proposé")),
    (
        "{==texte==}{>>remarque<<}",
        n_("commentaire sur un passage (absent des exports)"),
    ),
];

fn act_label(a: Act) -> &'static str {
    match a {
        Act::New => tr("Nouveau document"),
        Act::Templates => tr("Nouveau à partir d'un modèle"),
        Act::Open => tr("Ouvrir"),
        Act::Save => tr("Enregistrer"),
        Act::SaveAs => tr("Enregistrer sous"),
        Act::Export(Format::Odt) => tr("Exporter pour LibreOffice (.odt)"),
        Act::Export(Format::Pdf) => tr("Exporter en PDF"),
        Act::Export(_) => tr("Exporter"),
        Act::Print => tr("Imprimer"),
        Act::Undo => tr("Annuler"),
        Act::Redo => tr("Rétablir"),
        Act::Find => tr("Rechercher"),
        Act::Replace => tr("Rechercher et remplacer"),
        Act::Wrap("**") => tr("Gras"),
        Act::Wrap("*") => tr("Italique"),
        Act::Wrap("__") => tr("Souligné"),
        Act::Wrap(_) => tr("Barré"),
        Act::Style(LineStyle::Body) => tr("Corps de texte / aligner à gauche"),
        Act::Style(LineStyle::Heading(1)) => tr("Titre 1"),
        Act::Style(LineStyle::Heading(2)) => tr("Titre 2"),
        Act::Style(LineStyle::Heading(_)) => tr("Titre 3"),
        Act::Style(LineStyle::Center) => tr("Centrer"),
        Act::Style(LineStyle::Right) => tr("Aligner à droite"),
        Act::Style(LineStyle::Bullet) => tr("Liste à puces"),
        Act::Style(LineStyle::Numbered) => tr("Liste numérotée"),
        Act::Style(LineStyle::Quote) => tr("Citation"),
        Act::InsertField => tr("Insérer un champ"),
        Act::InsertPageBreak => tr("Saut de page"),
        Act::Syntax => tr("Guide de la syntaxe"),
        Act::Languages => tr("Écrire en plusieurs langues"),
        Act::FullScreen => tr("Plein écran"),
        Act::Focus => tr("Mode concentration"),
        Act::PastePlain => tr("Coller sans mise en forme"),
        Act::History => tr("Historique des versions"),
        _ => "",
    }
}

// ---------------------------------------------------------------------------------
// Barre de menus native de macOS
// ---------------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub fn native_menus(b: &mut crate::menubar::Builder) -> muda::Result<Vec<muda::Submenu>> {
    use crate::menubar::{clipboard_items, sep};
    use muda::Submenu;

    let [cut, copy, paste, select_all] = clipboard_items();
    let export = Submenu::with_items(
        tr("Exporter vers"),
        true,
        &[
            &b.item("PDF…", Act::Export(Format::Pdf), None),
            &b.item(tr("Word (.docx)…"), Act::Export(Format::Docx), None),
            &b.item(tr("LibreOffice (.odt)…"), Act::Export(Format::Odt), None),
            &b.item(tr("Texte enrichi (.rtf)…"), Act::Export(Format::Rtf), None),
            &b.item("EPUB…", Act::Export(Format::Epub), None),
            &b.item(tr("Page web (.html)…"), Act::Export(Format::Html), None),
            &b.item(tr("LaTeX (.tex)…"), Act::Export(Format::Latex), None),
            &b.item(tr("Texte brut (.txt)…"), Act::Export(Format::Text), None),
        ],
    )?;
    let file = Submenu::with_items(
        tr("Fichier"),
        true,
        &[
            &b.item(tr("Nouveau"), Act::New, Some("Cmd+N")),
            &b.item(
                tr("Nouveau à partir d'un modèle…"),
                Act::Templates,
                Some("Cmd+Shift+N"),
            ),
            &b.item(tr("Ouvrir…"), Act::Open, Some("Cmd+O")),
            &sep(),
            &b.item(tr("Enregistrer"), Act::Save, Some("Cmd+S")),
            &b.item(tr("Enregistrer sous…"), Act::SaveAs, Some("Cmd+Shift+S")),
            &sep(),
            &export,
            &b.item(tr("Publipostage…"), Act::Merge, None),
            &sep(),
            &b.item(
                tr("Historique des versions…"),
                Act::History,
                Some("Cmd+Shift+H"),
            ),
            &b.item(tr("Marquer cette version…"), Act::MarkVersion, None),
            &sep(),
            &b.item(tr("Imprimer…"), Act::Print, Some("Cmd+P")),
        ],
    )?;
    let edit = Submenu::with_items(
        tr("Édition"),
        true,
        &[
            &b.item(tr("Annuler"), Act::Undo, Some("Cmd+Z")),
            &b.item(tr("Rétablir"), Act::Redo, Some("Cmd+Shift+Z")),
            &sep(),
            &cut,
            &copy,
            &paste,
            &b.item(
                tr("Coller sans mise en forme"),
                Act::PastePlain,
                Some("Cmd+Shift+V"),
            ),
            &select_all,
            &sep(),
            &b.item(tr("Rechercher…"), Act::Find, Some("Cmd+F")),
            &b.item(
                tr("Rechercher et remplacer…"),
                Act::Replace,
                Some("Cmd+Alt+F"),
            ),
            &sep(),
            &b.item(tr("Effacer toutes les valeurs"), Act::ClearValues, None),
        ],
    )?;
    let insert = Submenu::with_items(
        tr("Insertion"),
        true,
        &[
            &b.item(tr("Champ…"), Act::InsertField, Some("F2")),
            &b.item(tr("Champ calculé…"), Act::InsertFormula, None),
            &b.item(tr("Date du jour"), Act::InsertToday, None),
            &sep(),
            &b.item(tr("Tableau…"), Act::InsertTable, None),
            &b.item(tr("Saut de page"), Act::InsertPageBreak, Some("Cmd+Enter")),
            &b.item(tr("Ligne horizontale"), Act::InsertRule, None),
            &b.item(tr("Formule"), Act::InsertMath(false), None),
            &b.item(tr("Code QR…"), Act::InsertQr, None),
        ],
    )?;
    let format = Submenu::with_items(
        tr("Format"),
        true,
        &[
            &b.item(tr("Gras"), Act::Wrap("**"), Some("Cmd+B")),
            &b.item(tr("Italique"), Act::Wrap("*"), Some("Cmd+I")),
            &b.item(tr("Souligné"), Act::Wrap("__"), Some("Cmd+U")),
            &b.item(tr("Barré"), Act::Wrap("~~"), None),
            &sep(),
            &b.item(
                tr("Titre 1"),
                Act::Style(LineStyle::Heading(1)),
                Some("Cmd+1"),
            ),
            &b.item(
                tr("Titre 2"),
                Act::Style(LineStyle::Heading(2)),
                Some("Cmd+2"),
            ),
            &b.item(
                tr("Titre 3"),
                Act::Style(LineStyle::Heading(3)),
                Some("Cmd+3"),
            ),
            &b.item(
                tr("Corps de texte"),
                Act::Style(LineStyle::Body),
                Some("Cmd+0"),
            ),
            &b.item(tr("Citation"), Act::Style(LineStyle::Quote), None),
            &sep(),
            &b.item(tr("Liste à puces"), Act::Style(LineStyle::Bullet), None),
            &b.item(tr("Liste numérotée"), Act::Style(LineStyle::Numbered), None),
            &sep(),
            &b.item(tr("Aligner à gauche"), Act::Style(LineStyle::Body), None),
            &b.item(tr("Centrer"), Act::Style(LineStyle::Center), Some("Cmd+E")),
            &b.item(
                tr("Aligner à droite"),
                Act::Style(LineStyle::Right),
                Some("Cmd+R"),
            ),
            &sep(),
            &b.item(tr("Mise en page…"), Act::PageSetup, None),
        ],
    )?;
    let view = Submenu::with_items(
        tr("Affichage"),
        true,
        &[
            &b.item(tr("Texte et page"), Act::Layout(Layout::Split), None),
            &b.item(tr("Page seule"), Act::Layout(Layout::Page), None),
            &b.item(tr("Texte seul"), Act::Layout(Layout::Text), None),
            &sep(),
            &b.item(
                tr("Afficher / masquer l'inspecteur"),
                Act::ToggleFields,
                Some("Cmd+Alt+I"),
            ),
            &b.item(
                tr("Afficher / masquer la barre d'outils"),
                Act::ToggleToolbar,
                Some("Cmd+Alt+T"),
            ),
            &b.item(
                tr("Afficher / masquer la barre d'état"),
                Act::ToggleStatus,
                None,
            ),
            &b.item(tr("Trame de fond des champs"), Act::ToggleHighlight, None),
            &b.item(tr("Mode concentration"), Act::Focus, Some("Cmd+Shift+F")),
            &b.item(
                tr("Défilement machine à écrire"),
                Act::ToggleTypewriter,
                None,
            ),
            &b.item(tr("Objectif de mots…"), Act::Goal, None),
            &b.item(tr("Remplir comme un formulaire"), Act::FillForm, None),
            &b.item(tr("Détecter les champs…"), Act::DetectFields, None),
            &b.item(
                tr("Créer un modèle à partir de ce document…"),
                Act::DecouperModele,
                None,
            ),
            &b.item(tr("Suivre les modifications"), Act::ToggleSuivi, None),
            &b.item(tr("Commenter la sélection…"), Act::Commenter, None),
            &b.item(tr("Commentaires et modifications"), Act::Revision, None),
            &b.item(tr("Lire à voix haute"), Act::LireVoix, None),
            &b.item(tr("Arrêter la lecture"), Act::ArreterVoix, None),
            &b.item(
                tr("Correction automatique pendant la frappe"),
                Act::ToggleAutocorrect,
                None,
            ),
            &sep(),
            &b.item(tr("Zoom avant"), Act::ZoomIn, Some("Cmd+=")),
            &b.item(tr("Zoom arrière"), Act::ZoomOut, Some("Cmd+-")),
            &b.item(tr("Largeur de la page"), Act::ZoomFit, None),
            &sep(),
            &b.item(tr("Plein écran"), Act::FullScreen, Some("Ctrl+Cmd+F")),
        ],
    )?;
    let help = Submenu::with_items(
        tr("Aide"),
        true,
        &[
            &b.item(tr("Guide de la syntaxe"), Act::Syntax, None),
            &b.item(tr("Écrire en plusieurs langues"), Act::Languages, None),
            &b.item(tr("Raccourcis clavier"), Act::Shortcuts, None),
            &b.item(tr("Nombre de mots"), Act::Stats, None),
            &sep(),
            &b.item(tr("Site de Varium"), crate::menubar::MenuAct::Website, None),
            &b.item(tr("À propos de Varium"), Act::About, None),
        ],
    )?;

    Ok(vec![file, edit, insert, format, view, help])
}

// ---------------------------------------------------------------------------------
// Pour la suite : palette de commandes, aide-mémoire, accueil et conversions
// ---------------------------------------------------------------------------------

impl VariumApp {
    /// Les commandes du module : (action, libellé en français, groupe en français,
    /// raccourci). Les libellés passent par `tr` à l'affichage.
    pub fn palette_entries() -> Vec<(Act, &'static str, &'static str, Option<KeyboardShortcut>)> {
        let fichier = n_("Fichier");
        let edition = n_("Édition");
        let insertion = n_("Insertion");
        let format = n_("Format");
        let affichage = n_("Affichage");
        let aide = n_("Aide");
        let list: [(Act, &'static str, &'static str); 68] = [
            (Act::New, n_("Nouveau document"), fichier),
            (Act::Templates, n_("Nouveau à partir d'un modèle…"), fichier),
            (Act::Open, n_("Ouvrir…"), fichier),
            (Act::Save, n_("Enregistrer"), fichier),
            (Act::SaveAs, n_("Enregistrer sous…"), fichier),
            (Act::Export(Format::Pdf), n_("Exporter en PDF"), fichier),
            (Act::Export(Format::Docx), n_("Word (.docx)…"), fichier),
            (Act::Export(Format::Odt), n_("LibreOffice (.odt)…"), fichier),
            (
                Act::Export(Format::Rtf),
                n_("Texte enrichi (.rtf)…"),
                fichier,
            ),
            (Act::Export(Format::Epub), n_("Exporter en EPUB"), fichier),
            (Act::Export(Format::Html), n_("Page web (.html)…"), fichier),
            (Act::Export(Format::Latex), n_("LaTeX (.tex)…"), fichier),
            (Act::Export(Format::Text), n_("Texte brut (.txt)…"), fichier),
            (Act::Merge, n_("Publipostage…"), fichier),
            (Act::Print, n_("Imprimer…"), fichier),
            (Act::Quit, n_("Quitter Varium"), fichier),
            (Act::Undo, n_("Annuler"), edition),
            (Act::Redo, n_("Rétablir"), edition),
            (Act::Find, n_("Rechercher…"), edition),
            (Act::Replace, n_("Rechercher et remplacer…"), edition),
            (Act::ClearValues, n_("Effacer toutes les valeurs"), edition),
            (Act::InsertField, n_("Champ…"), insertion),
            (Act::InsertFormula, n_("Champ calculé…"), insertion),
            (Act::InsertToday, n_("Date du jour"), insertion),
            (Act::InsertTable, n_("Tableau…"), insertion),
            (Act::InsertPageBreak, n_("Saut de page"), insertion),
            (Act::InsertRule, n_("Ligne horizontale"), insertion),
            (Act::InsertMath(false), n_("Formule"), insertion),
            (Act::InsertMath(true), n_("Formule centrée"), insertion),
            (Act::Wrap("**"), n_("Gras"), format),
            (Act::Wrap("*"), n_("Italique"), format),
            (Act::Wrap("__"), n_("Souligné"), format),
            (Act::Wrap("~~"), n_("Barré"), format),
            (Act::Style(LineStyle::Heading(1)), n_("Titre 1"), format),
            (Act::Style(LineStyle::Heading(2)), n_("Titre 2"), format),
            (Act::Style(LineStyle::Heading(3)), n_("Titre 3"), format),
            (Act::Style(LineStyle::Body), n_("Corps de texte"), format),
            (Act::Style(LineStyle::Quote), n_("Citation"), format),
            (Act::Style(LineStyle::Bullet), n_("Liste à puces"), format),
            (
                Act::Style(LineStyle::Numbered),
                n_("Liste numérotée"),
                format,
            ),
            (Act::Style(LineStyle::Center), n_("Centrer"), format),
            (Act::Style(LineStyle::Right), n_("Aligner à droite"), format),
            (Act::PageSetup, n_("Mise en page…"), format),
            (Act::Layout(Layout::Split), n_("Texte et page"), affichage),
            (Act::Layout(Layout::Page), n_("Page seule"), affichage),
            (Act::Layout(Layout::Text), n_("Texte seul"), affichage),
            (
                Act::ToggleFields,
                n_("Afficher / masquer l'inspecteur"),
                affichage,
            ),
            (
                Act::ToggleToolbar,
                n_("Afficher / masquer la barre d'outils"),
                affichage,
            ),
            (
                Act::ToggleStatus,
                n_("Afficher / masquer la barre d'état"),
                affichage,
            ),
            (
                Act::ToggleHighlight,
                n_("Trame de fond des champs"),
                affichage,
            ),
            (Act::Focus, n_("Mode concentration"), affichage),
            (Act::ZoomIn, n_("Zoom avant"), affichage),
            (Act::ZoomOut, n_("Zoom arrière"), affichage),
            (Act::ZoomFit, n_("Largeur de la page"), affichage),
            (Act::FullScreen, n_("Plein écran"), affichage),
            (Act::Syntax, n_("Guide de la syntaxe"), aide),
            (Act::Languages, n_("Écrire en plusieurs langues"), aide),
            (Act::Stats, n_("Nombre de mots"), aide),
            (Act::About, n_("À propos de Varium"), aide),
            (Act::PastePlain, n_("Coller sans mise en forme"), edition),
            (Act::History, n_("Historique des versions…"), fichier),
            (Act::MarkVersion, n_("Marquer cette version…"), fichier),
            (Act::InsertQr, n_("Code QR…"), insertion),
            (
                Act::ToggleTypewriter,
                n_("Défilement machine à écrire"),
                affichage,
            ),
            (Act::Goal, n_("Objectif de mots…"), affichage),
            (Act::DetectFields, n_("Détecter les champs…"), n_("Outils")),
            (
                Act::FillForm,
                n_("Remplir comme un formulaire"),
                n_("Outils"),
            ),
            (
                Act::ToggleAutocorrect,
                n_("Correction automatique pendant la frappe"),
                n_("Outils"),
            ),
        ];
        let outils = n_("Outils");
        let revision: [(Act, &'static str, &'static str); 8] = [
            (
                Act::DecouperModele,
                n_("Créer un modèle à partir de ce document…"),
                fichier,
            ),
            (Act::ToggleSuivi, n_("Suivre les modifications"), outils),
            (Act::Commenter, n_("Commenter la sélection…"), outils),
            (Act::Revision, n_("Commentaires et modifications"), outils),
            (Act::ToutAccepter, n_("Tout accepter"), outils),
            (Act::ToutRefuser, n_("Tout refuser"), outils),
            (Act::LireVoix, n_("Lire à voix haute"), outils),
            (Act::ArreterVoix, n_("Arrêter la lecture"), outils),
        ];
        list.into_iter()
            .chain(revision)
            .map(|(a, l, g)| (a, l, g, shortcut_for(a)))
            .collect()
    }

    /// Lance une action à la prochaine image du module (palette de commandes).
    pub fn run_act(&mut self, act: Act) {
        self.queue.push(act);
    }

    /// Les documents récents, du plus récent au plus ancien.
    pub fn recent_files(&self) -> Vec<String> {
        self.prefs.recent.clone()
    }

    /// Le fichier du document ouvert.
    pub fn file_path(&self) -> Option<std::path::PathBuf> {
        self.path.clone()
    }

    /// Ouvre un fichier (récent, glissé-déposé), après confirmation s'il reste des
    /// modifications non enregistrées.
    pub fn open_file(&mut self, ctx: &egui::Context, path: &std::path::Path) {
        self.guard(ctx, Pending::Recent(path.display().to_string()));
    }

    /// Titre, source et valeurs des champs du document (conversions).
    pub fn document_parts(&self) -> (String, String, std::collections::BTreeMap<String, String>) {
        (
            self.title.clone(),
            self.source.clone(),
            self.values.values.clone(),
        )
    }

    /// Insère un bloc (tableau…) sur sa propre ligne, au curseur.
    pub fn insert_markup(&mut self, block: &str) {
        self.history.cut = true;
        self.insert_block(block);
        self.history.cut = true;
    }

    /// Remplace le document par un nouveau (Ctrl+Z ramène le précédent).
    pub fn new_document_with(&mut self, title: &str, source: &str) {
        let mut p = Project::new(title, source, &self.values);
        p.page = self.page.clone();
        self.load(p);
    }

    /// Donne leurs valeurs à des champs ; renvoie le nombre de champs remplis.
    pub fn set_field_values(&mut self, pairs: &[(String, String)]) -> usize {
        self.history.cut = true;
        for (k, v) in pairs {
            self.values.set(k.clone(), v.clone());
        }
        pairs.len()
    }

    /// Annule tout de suite la dernière modification (bouton « Annuler » d'une
    /// conversion).
    pub fn undo_now(&mut self) {
        let now = self.snapshot();
        if now.source != self.history.current.source
            || now.values != self.history.current.values
            || now.title != self.history.current.title
        {
            // La modification n'est pas encore dans l'historique.
            let prev = std::mem::replace(&mut self.history.current, now);
            self.history.undo.push(prev);
        }
        self.undo();
    }

    /// Ferme le choix de modèle du premier lancement (l'accueil le remplace).
    pub fn dismiss_start_dialog(&mut self) {
        if matches!(self.dialog, Some(Dialog::Templates)) {
            self.dialog = None;
        }
    }

    /// Charge un modèle de document (accueil).
    pub fn load_template(&mut self, ctx: &egui::Context, i: usize) {
        // Au-delà des modèles classiques : les modèles intelligents (lettres types,
        // modèles de l'utilisateur), qui posent leurs questions.
        if let Some(k) = i.checked_sub(templates::ALL.len()) {
            if let Some(m) = templates::intelligents().into_iter().nth(k) {
                self.ouvrir_modele_intelligent(ctx, m.modele);
            }
            return;
        }
        if let Some(t) = templates::ALL.get(i) {
            self.load(t.project());
            self.notify(
                ctx,
                trf!("Modèle « {name} » ouvert", name = tr(t.name)),
                false,
            );
        }
    }
}
