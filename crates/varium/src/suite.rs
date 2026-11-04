//! La suite Varium : trois applications dans une fenêtre, comme LibreOffice —
//! Documents (traitement de texte), Classeur (tableur) et Présentation (diaporamas).
//!
//! Par-dessus les modules : le centre de démarrage (accueil), la palette de commandes
//! (Ctrl+K), l'aide-mémoire des raccourcis (F1), l'ouverture par glisser-déposer et
//! les conversions d'un module à l'autre.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::tr::{self, n_, tr};
use egui::{Color32, Key, KeyboardShortcut, Modifiers, RichText, Vec2};
use serde::{Deserialize, Serialize};

use crate::accueil::{self, Accueil, Choix};
use crate::app::{Act, VariumApp};
use crate::apparence::{self, Apparence};
use crate::calc_ui::{CalcAction, CalcApp};
use crate::conversions;
use crate::depot::{self, Cible};
use crate::palette::{self, Cmd, Conversion, Entry, Palette};
use crate::raccourcis::{self, AideMemoire, Demande, Ligne};
use crate::slides_ui::{SlideAction, SlidesApp};

const MODULE_KEY: &str = "varium.module";
const RECENTS_KEY: &str = "varium.recents";
const PALETTE_KEY: &str = "varium.palette.recentes";
/// Fichiers récents de tous les modules.
const MAX_RECENTS: usize = 20;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Default)]
pub enum Module {
    #[default]
    Documents,
    Classeur,
    Presentation,
}

impl Module {
    pub const ALL: [Module; 3] = [Module::Documents, Module::Classeur, Module::Presentation];

    pub fn label(self) -> &'static str {
        match self {
            Module::Documents => tr("Documents"),
            Module::Classeur => tr("Classeur"),
            Module::Presentation => tr("Présentation"),
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Module::Documents => "📝",
            Module::Classeur => "📊",
            Module::Presentation => "🎞",
        }
    }

    pub fn color(self) -> Color32 {
        match self {
            Module::Documents => Color32::from_rgb(0x1F, 0x5F, 0xBF),
            Module::Classeur => Color32::from_rgb(0x1E, 0x8A, 0x4C),
            Module::Presentation => Color32::from_rgb(0xD0, 0x5A, 0x1E),
        }
    }
}

/// Raccourcis de la suite, valables dans tous les modules.
const PALETTE_KEYS: [KeyboardShortcut; 2] = [
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::P),
    KeyboardShortcut::new(Modifiers::COMMAND, Key::K),
];
const AIDE_KEYS: [KeyboardShortcut; 2] = [
    KeyboardShortcut::new(Modifiers::NONE, Key::F1),
    KeyboardShortcut::new(Modifiers::COMMAND, Key::Slash),
];

/// Un raccourci pris par la suite, qui n'atteint donc jamais les modules.
fn reserved(s: &KeyboardShortcut) -> bool {
    PALETTE_KEYS.contains(s) || AIDE_KEYS.contains(s)
}

/// Place un fichier en tête des récents (tous modules), sans doublon.
pub fn push_recent(list: &mut Vec<(Module, String)>, m: Module, path: &Path) {
    let p = path.display().to_string();
    list.retain(|(_, x)| *x != p);
    list.insert(0, (m, p));
    list.truncate(MAX_RECENTS);
}

/// Ce que défait le bouton « Annuler » d'une conversion.
#[derive(Clone)]
enum Undo {
    /// La présentation remplacée (et son fichier) revient.
    Deck(Option<PathBuf>),
    /// Le document revient à l'état d'avant ; on retourne au module d'origine.
    Doc(Module),
}

struct Toast {
    msg: String,
    at: f64,
    error: bool,
    undo: Option<Undo>,
}

pub struct Suite {
    doc: VariumApp,
    calc: CalcApp,
    slides: SlidesApp,
    module: Module,
    last_title: String,
    /// Langue de l'interface au dernier titre de fenêtre envoyé.
    title_lang: varium_core::Lang,
    /// Centre de démarrage affiché à la place du module.
    show_accueil: bool,
    accueil: Accueil,
    palette: Palette,
    aide: AideMemoire,
    apparence: Apparence,
    zoom_interface: f32,
    apparence_appliquee: Option<(Apparence, f32)>,
    /// Identifiants des commandes récemment lancées depuis la palette.
    palette_recents: Vec<String>,
    /// Fichiers récents des trois modules.
    recents: Vec<(Module, String)>,
    /// Fichier de chaque module à l'image précédente (pour les récents).
    seen_paths: [Option<PathBuf>; 3],
    toast: Option<Toast>,
    /// Exports de la suite (aide-mémoire en HTML).
    io: crate::io::Io,
    /// Fichiers choisis dans le dialogue « Ouvrir… » de l'accueil.
    picked: Arc<Mutex<Vec<PathBuf>>>,
    /// Fichiers survolés et module qui les ouvrira.
    hover: Option<(Vec<PathBuf>, Cible)>,
    /// Le widget qui avait le clavier avant la palette ou l'aide-mémoire (il le
    /// retrouve à leur fermeture).
    prev_focus: Option<egui::Id>,
    /// Dernière bascule de la palette ou de l'aide-mémoire (barre native et clavier).
    last_toggle: f64,
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
        let stored_module = cc
            .storage
            .and_then(|s| eframe::get_value::<Module>(s, MODULE_KEY));
        let first_launch = stored_module.is_none();
        let module = stored_module.unwrap_or_default();
        // La langue de l'interface, avant de construire les modules.
        let choice = cc
            .storage
            .and_then(|s| s.get_string(tr::STORAGE_KEY))
            .and_then(|code| tr::parse_choice(&code));
        tr::set_choice(choice);
        let au_demarrage = cc
            .storage
            .and_then(|s| eframe::get_value::<bool>(s, accueil::STORAGE_KEY))
            .unwrap_or(true);
        let recents = cc
            .storage
            .and_then(|s| eframe::get_value::<Vec<(Module, String)>>(s, RECENTS_KEY))
            .unwrap_or_default();
        let palette_recents = cc
            .storage
            .and_then(|s| eframe::get_value::<Vec<String>>(s, PALETTE_KEY))
            .unwrap_or_default();
        let apparence = cc
            .storage
            .and_then(|s| s.get_string(apparence::THEME_KEY))
            .and_then(|c| Apparence::depuis_code(&c))
            .unwrap_or_default();
        let zoom_interface = apparence::zoom_valide(
            cc.storage
                .and_then(|s| eframe::get_value::<f32>(s, apparence::ZOOM_KEY))
                .unwrap_or(1.0),
        );
        let file_arg = std::env::args_os().nth(1).is_some();
        #[allow(unused_mut)]
        let mut suite = Self {
            doc: VariumApp::new(cc),
            calc: CalcApp::new(cc.storage),
            slides: SlidesApp::new(cc.storage),
            module,
            last_title: String::new(),
            title_lang: tr::lang(),
            show_accueil: !file_arg && (first_launch || au_demarrage),
            accueil: Accueil::new(au_demarrage),
            palette: Palette::default(),
            aide: AideMemoire::default(),
            apparence,
            zoom_interface,
            apparence_appliquee: None,
            palette_recents,
            recents,
            seen_paths: [None, None, None],
            toast: None,
            io: crate::io::Io::default(),
            picked: Arc::default(),
            hover: None,
            last_toggle: f64::NEG_INFINITY,
            prev_focus: None,
            #[cfg(target_os = "macos")]
            native: None,
            #[cfg(target_os = "macos")]
            menu_events: crate::menubar::Events::install(&cc.egui_ctx),
            #[cfg(target_os = "macos")]
            menu_generation: 0,
        };
        if suite.show_accueil {
            // L'accueil remplace le choix de modèle du premier lancement.
            suite.doc.dismiss_start_dialog();
        }
        // Les documents récents connus du module Documents.
        for p in suite.doc.recent_files().iter().rev() {
            if !suite.recents.iter().any(|(_, x)| x == p) {
                suite.recents.push((Module::Documents, p.clone()));
            }
        }
        suite.recents.truncate(MAX_RECENTS);
        suite.seen_paths = suite.paths();
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
                MenuAct::Doc(a) => {
                    self.show_accueil = false;
                    self.doc.menu_act(ctx, a)
                }
                MenuAct::Calc(a) => {
                    self.show_accueil = false;
                    self.calc.perform_once(ctx, a)
                }
                MenuAct::Slides(a) => {
                    self.show_accueil = false;
                    self.slides.perform_once(ctx, a)
                }
                MenuAct::Module(m) => self.switch(m),
                MenuAct::UiLang(c) => tr::set_choice(c),
                MenuAct::Website => ctx.open_url(egui::OpenUrl::new_tab(
                    "https://maxlestage.github.io/VARIUM/",
                )),
                MenuAct::Palette => self.toggle_palette(ctx),
                MenuAct::Raccourcis => self.toggle_aide(ctx),
                MenuAct::Accueil => self.show_accueil = true,
                MenuAct::Convert(c) => self.convert(ctx, c),
            }
        }
    }

    fn switch(&mut self, m: Module) {
        self.show_accueil = false;
        if self.module != m {
            self.module = m;
            self.doc.refresh_window_title();
            self.last_title.clear();
        }
    }

    fn native_menu(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.native.is_some()
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    fn notify(&mut self, ctx: &egui::Context, msg: impl Into<String>, error: bool) {
        self.toast = Some(Toast {
            msg: msg.into(),
            at: ctx.input(|i| i.time),
            error,
            undo: None,
        });
    }

    fn paths(&self) -> [Option<PathBuf>; 3] {
        [
            self.doc.file_path(),
            self.calc.file_path(),
            self.slides.file_path(),
        ]
    }

    /// Un fichier enregistré ou ouvert dans un module rejoint les récents.
    fn track_recents(&mut self) {
        let now = self.paths();
        for (i, m) in Module::ALL.into_iter().enumerate() {
            if now[i] != self.seen_paths[i]
                && let Some(p) = &now[i]
            {
                push_recent(&mut self.recents, m, p);
            }
        }
        self.seen_paths = now;
    }

    fn toggle_palette(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if now - self.last_toggle < 0.3 {
            return;
        }
        self.last_toggle = now;
        if !self.palette.open && !self.aide.open {
            self.prev_focus = ctx.memory(|m| m.focused());
        }
        self.aide.open = false;
        self.palette.toggle();
    }

    fn toggle_aide(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if now - self.last_toggle < 0.3 {
            return;
        }
        self.last_toggle = now;
        if !self.palette.open && !self.aide.open {
            self.prev_focus = ctx.memory(|m| m.focused());
        }
        self.palette.open = false;
        self.aide.toggle();
    }

    // ---------------- Commandes ----------------

    /// Toutes les commandes : celles du module actif, puis celles de la suite.
    fn entries(&self, ctx: &egui::Context) -> Vec<Entry> {
        let keys = |s: Option<KeyboardShortcut>| {
            s.filter(|s| !reserved(s))
                .map(|s| ctx.format_shortcut(&s))
                .unwrap_or_default()
        };
        let mut out = Vec::new();
        // Les actions des trois modules : celles du module actif d'abord ; une action
        // d'un autre module y bascule avant de s'exécuter.
        let actif = if self.show_accueil {
            None
        } else {
            Some(self.module)
        };
        let mut ordre = vec![Module::Documents, Module::Classeur, Module::Presentation];
        if let Some(m) = actif {
            ordre.retain(|&x| x != m);
            ordre.insert(0, m);
        }
        for m in ordre {
            let ici = Some(m) == actif;
            let mut ajoute =
                |cmd: Cmd, id: String, fr: &'static str, g: &'static str, k: String| {
                    let mut e = Entry {
                        cmd,
                        id,
                        label: tr(fr).to_string(),
                        fr: fr.to_string(),
                        group: tr(g).to_string(),
                        group_fr: g.to_string(),
                        keys: k,
                    };
                    if !ici {
                        let (nom, nom_fr) = match m {
                            Module::Documents => (tr("Documents"), "Documents"),
                            Module::Classeur => (tr("Classeur"), "Classeur"),
                            Module::Presentation => (tr("Présentation"), "Présentation"),
                        };
                        e.group = format!("{nom} › {}", e.group);
                        e.group_fr = format!("{nom_fr} {}", e.group_fr);
                        // Les raccourcis ne valent que dans leur module.
                        e.keys.clear();
                    }
                    out.push(e);
                };
            match m {
                Module::Documents => {
                    for (a, fr, g, s) in VariumApp::palette_entries() {
                        ajoute(Cmd::Doc(a), format!("doc:{a:?}:{fr}"), fr, g, keys(s));
                    }
                }
                Module::Classeur => {
                    for (a, fr, g, s, _) in CalcApp::palette_entries() {
                        ajoute(Cmd::Calc(a), format!("calc:{a:?}:{fr}"), fr, g, keys(s));
                    }
                }
                Module::Presentation => {
                    for (a, fr, g, s, _) in SlidesApp::palette_entries() {
                        ajoute(Cmd::Slides(a), format!("slides:{a:?}:{fr}"), fr, g, keys(s));
                    }
                }
            }
        }
        let mut push = |cmd: Cmd, id: String, fr: &'static str, group: &'static str, k: String| {
            out.push(Entry {
                cmd,
                id,
                label: tr(fr).to_string(),
                fr: fr.to_string(),
                group: tr(group).to_string(),
                group_fr: group.to_string(),
                keys: k,
            });
        };
        let general = n_("Général");
        let modules = [
            (
                Module::Documents,
                n_("Aller au module Documents"),
                n_("Nouveau document"),
            ),
            (
                Module::Classeur,
                n_("Aller au module Classeur"),
                n_("Nouveau classeur"),
            ),
            (
                Module::Presentation,
                n_("Aller au module Présentation"),
                n_("Nouvelle présentation"),
            ),
        ];
        for (m, aller, nouveau) in modules {
            if m != self.module || self.show_accueil {
                push(
                    Cmd::Module(m),
                    format!("g:module:{m:?}"),
                    aller,
                    general,
                    String::new(),
                );
            }
            if m != self.module || self.show_accueil {
                push(
                    Cmd::New(m),
                    format!("g:new:{m:?}"),
                    nouveau,
                    general,
                    String::new(),
                );
            }
        }
        push(
            Cmd::OpenAny,
            "g:open".into(),
            n_("Ouvrir un fichier (tous formats)…"),
            general,
            String::new(),
        );
        push(
            Cmd::Accueil,
            "g:accueil".into(),
            n_("Accueil — centre de démarrage"),
            general,
            String::new(),
        );
        push(
            Cmd::Shortcuts,
            "g:raccourcis".into(),
            n_("Aide-mémoire des raccourcis"),
            n_("Aide"),
            keys(Some(AIDE_KEYS[0])),
        );
        for c in Conversion::ALL {
            push(
                Cmd::Convert(c),
                format!("g:convert:{c:?}"),
                c.label_fr(),
                n_("Convertir"),
                String::new(),
            );
        }
        // Langues et fichiers récents : libellés composés.
        let langue = n_("Langue de l'interface");
        for (c, name) in tr::choices() {
            out.push(Entry {
                cmd: Cmd::UiLang(c),
                id: format!("g:lang:{}", tr::choice_code(c)),
                label: format!("{} : {name}", tr(langue)),
                fr: format!(
                    "{langue} : {}",
                    c.map_or("Automatique (système)", |l| l.native())
                ),
                group: tr(general).to_string(),
                group_fr: general.to_string(),
                keys: String::new(),
            });
        }
        let recents = n_("Fichiers récents");
        for (m, path) in self.recents.iter().take(12) {
            let name = crate::files::short_name(path);
            out.push(Entry {
                cmd: Cmd::OpenRecent(*m, path.clone()),
                id: format!("g:recent:{path}"),
                label: format!("{} {name}", m.icon()),
                fr: name,
                group: format!("{} · {}", tr(recents), m.label()),
                group_fr: recents.to_string(),
                keys: String::new(),
            });
        }
        out
    }

    fn run(&mut self, ctx: &egui::Context, cmd: Cmd) {
        match cmd {
            Cmd::Doc(a) => {
                self.switch(Module::Documents);
                self.doc.run_act(a);
            }
            Cmd::Calc(a) => {
                self.switch(Module::Classeur);
                self.calc.perform(ctx, a);
            }
            Cmd::Slides(a) => {
                self.switch(Module::Presentation);
                self.slides.perform(ctx, a);
            }
            Cmd::Module(m) => self.switch(m),
            Cmd::Accueil => self.show_accueil = true,
            Cmd::Shortcuts => {
                self.aide.open = false;
                self.aide.toggle();
            }
            Cmd::UiLang(c) => tr::set_choice(c),
            Cmd::New(m) => {
                self.switch(m);
                match m {
                    Module::Documents => self.doc.run_act(Act::New),
                    Module::Classeur => self.calc.perform(ctx, CalcAction::New),
                    Module::Presentation => self.slides.perform(ctx, SlideAction::New),
                }
            }
            Cmd::OpenAny => depot::choisir(ctx, &self.picked),
            Cmd::OpenRecent(m, path) => self.open_in(ctx, m, Path::new(&path)),
            Cmd::Convert(c) => self.convert(ctx, c),
        }
        ctx.request_repaint();
    }

    /// Ouvre un fichier dans un module.
    fn open_in(&mut self, ctx: &egui::Context, m: Module, path: &Path) {
        if !path.is_file() {
            let p = path.display().to_string();
            self.recents.retain(|(_, x)| *x != p);
            self.notify(
                ctx,
                trf!("« {path} » est introuvable (déplacé ou supprimé)", path = p),
                true,
            );
            return;
        }
        self.switch(m);
        match m {
            Module::Documents => self.doc.open_file(ctx, path),
            Module::Classeur => self.calc.open_file(ctx, path),
            Module::Presentation => self.slides.open_file(ctx, path),
        }
        push_recent(&mut self.recents, m, path);
    }

    /// Ouvre un fichier dans le module qui lui convient ; `false` s'il est refusé.
    fn open_any(&mut self, ctx: &egui::Context, path: &Path) -> bool {
        match depot::route(&depot::nom(path), &depot::head(path)) {
            Cible::Module(m) => {
                self.open_in(ctx, m, path);
                true
            }
            c => {
                self.notify(ctx, depot::refus(c, &depot::nom(path)), true);
                false
            }
        }
    }

    // ---------------- Glisser-déposer ----------------

    fn drag_and_drop(&mut self, ctx: &egui::Context) {
        let hovered: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .hovered_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if hovered.is_empty() {
            self.hover = None;
        } else if self.hover.as_ref().is_none_or(|(p, _)| *p != hovered) {
            let c = depot::route(&depot::nom(&hovered[0]), &depot::head(&hovered[0]));
            self.hover = Some((hovered, c));
        }
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        if dropped.is_empty() {
            return;
        }
        self.hover = None;
        let mut images = false;
        for p in &dropped {
            match depot::route(&depot::nom(p), &depot::head(p)) {
                Cible::Image => images = true,
                Cible::Module(m) => self.open_in(ctx, m, p),
                c => self.notify(ctx, depot::refus(c, &depot::nom(p)), true),
            }
        }
        // Les images restent pour la Présentation, qui les pose sur la diapositive ;
        // le reste est déjà ouvert.
        ctx.input_mut(|i| {
            i.raw
                .dropped_files
                .retain(|f| depot::route(&depot::nom(f.path()), b"") == Cible::Image)
        });
        if images {
            self.switch(Module::Presentation);
        }
    }

    // ---------------- Conversions ----------------

    fn convert(&mut self, ctx: &egui::Context, c: Conversion) {
        let now = ctx.input(|i| i.time);
        let (msg, undo) = match c {
            Conversion::DocToDeck => {
                let (title, src, values) = self.doc.document_parts();
                let mut deck = varium_slides::deck_from_document(&src, &values);
                if deck.title.trim().is_empty() {
                    deck.title = title;
                }
                let n = deck.slides.len();
                let old = self.slides.replace_deck(ctx, deck);
                self.switch(Module::Presentation);
                (
                    trf!(
                        "Présentation créée depuis le document : {n} diapositive(s)",
                        n
                    ),
                    Undo::Deck(old),
                )
            }
            Conversion::CalcToDoc | Conversion::CalcToNewDoc => {
                let rows = self.calc.selection_values();
                let table = conversions::markup_table(&rows);
                if table.is_empty() {
                    self.notify(ctx, tr("La sélection du classeur est vide"), true);
                    return;
                }
                let n = table.lines().count().saturating_sub(2);
                if c == Conversion::CalcToDoc {
                    self.doc.insert_markup(&format!("\n{table}\n"));
                } else {
                    let title = self.calc.workbook().title.clone();
                    self.doc.new_document_with(&title, &format!("{table}\n"));
                }
                self.switch(Module::Documents);
                (
                    trf!("Tableau de {n} ligne(s) inséré dans le document", n),
                    Undo::Doc(Module::Classeur),
                )
            }
            Conversion::CalcToFields => {
                let rows = self.calc.selection_values();
                match conversions::field_pairs(&rows) {
                    Ok(pairs) => {
                        let n = self.doc.set_field_values(&pairs);
                        self.switch(Module::Documents);
                        (
                            trf!("{n} champ(s) du document rempli(s) depuis le classeur", n),
                            Undo::Doc(Module::Classeur),
                        )
                    }
                    Err(e) => {
                        self.notify(ctx, e, true);
                        return;
                    }
                }
            }
        };
        self.toast = Some(Toast {
            msg,
            at: now,
            error: false,
            undo: Some(undo),
        });
    }

    fn undo_conversion(&mut self, undo: Undo) {
        match undo {
            Undo::Deck(path) => {
                self.slides.undo_replace(path);
                self.switch(Module::Documents);
            }
            Undo::Doc(back) => {
                self.doc.undo_now();
                self.switch(back);
            }
        }
    }

    // ---------------- Aide-mémoire ----------------

    /// Les raccourcis du module actif et de la suite.
    fn shortcut_lines(&self, ctx: &egui::Context) -> Vec<Ligne> {
        let mut out: Vec<Ligne> = Vec::new();
        let mut add = |g: &'static str, l: &'static str, s: Option<KeyboardShortcut>| {
            if let Some(s) = s.filter(|s| !reserved(s)) {
                out.push((tr(g).into(), tr(l).into(), ctx.format_shortcut(&s)));
            }
        };
        match self.module {
            Module::Documents => {
                for (_, l, g, s) in VariumApp::palette_entries() {
                    add(g, l, s);
                }
            }
            Module::Classeur => {
                for (_, l, g, s, _) in CalcApp::palette_entries() {
                    add(g, l, s);
                }
            }
            Module::Presentation => {
                for (_, l, g, s, _) in SlidesApp::palette_entries() {
                    add(g, l, s);
                }
            }
        }
        // Regroupe par menu, dans l'ordre d'apparition.
        let mut groups: Vec<String> = Vec::new();
        for (g, _, _) in &out {
            if !groups.contains(g) {
                groups.push(g.clone());
            }
        }
        out.sort_by_key(|(g, _, _)| groups.iter().position(|x| x == g));
        let partout = tr("Partout dans Varium").to_string();
        let f = |s: &KeyboardShortcut| ctx.format_shortcut(s);
        out.push((
            partout.clone(),
            tr("Palette de commandes").into(),
            format!("{}  ·  {}", f(&PALETTE_KEYS[1]), f(&PALETTE_KEYS[0])),
        ));
        out.push((
            partout,
            tr("Aide-mémoire des raccourcis").into(),
            format!("{}  ·  {}", f(&AIDE_KEYS[0]), f(&AIDE_KEYS[1])),
        ));
        if self.module == Module::Presentation {
            let d = tr("Pendant le diaporama").to_string();
            for (l, k) in [
                (n_("Diapositive suivante"), n_("→, Espace, Entrée")),
                (n_("Diapositive précédente"), n_("←, Retour arrière")),
                (n_("Écran noir"), "B"),
                (n_("Grille des diapositives"), "G"),
                (n_("Pointeur laser (maintenir)"), "L"),
                (n_("Stylo (1 à 4 : couleur)"), "P"),
                (n_("Effacer l'encre"), "E"),
                (n_("Zoom sur une zone"), "Z"),
                (n_("Aller à la diapositive n"), n_("n puis Entrée")),
                (n_("Quitter le diaporama"), n_("Échap")),
            ] {
                out.push((d.clone(), tr(l).into(), tr(k).into()));
            }
        }
        if self.module == Module::Classeur {
            let d = tr("Grille").to_string();
            for (l, k) in [
                (n_("Modifier la cellule"), "F2"),
                (n_("Étendre la sélection"), n_("Maj+flèches")),
                (n_("Valider et descendre"), n_("Entrée")),
                (n_("Valider et aller à droite"), n_("Tab")),
            ] {
                out.push((d.clone(), tr(l).into(), tr(k).into()));
            }
        }
        out
    }

    fn aide_title(&self) -> String {
        trf!(
            "Raccourcis clavier — {module}",
            module = self.module.label()
        )
    }

    // ---------------- Interface ----------------

    /// Les raccourcis que la suite traite pour le Classeur et la Présentation (Nouveau,
    /// Ouvrir, Enregistrer…) quand il n'y a pas de barre native.
    fn module_shortcuts(&mut self, ctx: &egui::Context) {
        if self.native_menu() {
            return;
        }
        let modifiers = |s: &KeyboardShortcut| {
            let m = s.modifiers;
            m.command as u8 + m.shift as u8 + m.alt as u8
        };
        match self.module {
            Module::Documents => {}
            Module::Classeur => {
                let mut list: Vec<_> = CalcApp::palette_entries()
                    .into_iter()
                    .filter_map(|(a, _, _, s, suite)| s.filter(|_| suite).map(|s| (s, a)))
                    .collect();
                list.sort_by_key(|(s, _)| std::cmp::Reverse(modifiers(s)));
                for (s, a) in list {
                    if ctx.input_mut(|i| i.consume_shortcut(&s)) {
                        self.calc.perform_once(ctx, a);
                    }
                }
            }
            Module::Presentation => {
                let mut list: Vec<_> = SlidesApp::palette_entries()
                    .into_iter()
                    .filter_map(|(a, _, _, s, suite)| s.filter(|_| suite).map(|s| (s, a)))
                    .collect();
                list.sort_by_key(|(s, _)| std::cmp::Reverse(modifiers(s)));
                for (s, a) in list {
                    if ctx.input_mut(|i| i.consume_shortcut(&s)) {
                        self.slides.perform_once(ctx, a);
                    }
                }
            }
        }
    }

    fn switcher(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let accueil = egui::Button::new(RichText::new("🏠").size(14.0))
                .min_size(Vec2::new(30.0, 26.0))
                .corner_radius(13.0)
                .selected(self.show_accueil);
            if ui
                .add(accueil)
                .on_hover_text(tr("Accueil — centre de démarrage"))
                .clicked()
            {
                self.show_accueil = !self.show_accueil;
            }
            for m in Module::ALL {
                let on = self.module == m && !self.show_accueil;
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
            ui.add_space(8.0);
            ui.menu_button(format!("⇄ {}", tr("Convertir")), |ui| {
                for c in Conversion::ALL {
                    if ui.button(tr(c.label_fr())).clicked() {
                        self.convert(&ctx, c);
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text(tr("Passer le contenu d'un module à un autre"));
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
                ui.menu_button(format!("🎨 {}", self.apparence.nom()), |ui| {
                    ui.label(RichText::new(tr("Apparence")).strong());
                    ui.separator();
                    for a in Apparence::TOUTES {
                        if ui.radio(self.apparence == a, a.nom()).clicked() {
                            self.apparence = a;
                        }
                    }
                    ui.separator();
                    ui.label(RichText::new(tr("Taille de l'interface")).strong());
                    ui.horizontal(|ui| {
                        if ui.button("−").clicked() {
                            self.zoom_interface =
                                apparence::zoom_valide(self.zoom_interface - apparence::ZOOM_PAS);
                        }
                        ui.label(format!("{} %", (self.zoom_interface * 100.0).round()));
                        if ui.button("+").clicked() {
                            self.zoom_interface =
                                apparence::zoom_valide(self.zoom_interface + apparence::ZOOM_PAS);
                        }
                        if ui.button(tr("Taille normale")).clicked() {
                            self.zoom_interface = 1.0;
                        }
                    });
                })
                .response
                .on_hover_text(tr("Thème et taille de l'interface"));
                let k = ctx.format_shortcut(&PALETTE_KEYS[1]);
                if ui
                    .button(format!("🔍 {}", tr("Commandes")))
                    .on_hover_text(trf!("Palette de commandes ({k})", k))
                    .clicked()
                {
                    self.toggle_palette(&ctx);
                }
                if ui
                    .button("⌨")
                    .on_hover_text(trf!(
                        "Aide-mémoire des raccourcis ({k})",
                        k = ctx.format_shortcut(&AIDE_KEYS[0])
                    ))
                    .clicked()
                {
                    self.toggle_aide(&ctx);
                }
            });
        });
    }

    fn toast_ui(&mut self, ctx: &egui::Context) {
        let Some(t) = &self.toast else { return };
        let now = ctx.input(|i| i.time);
        let life = if t.undo.is_some() { 10.0 } else { 6.0 };
        if now - t.at > life {
            self.toast = None;
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
        let mut undo_clicked = false;
        let mut close = false;
        egui::Area::new(egui::Id::unique("varium-suite-toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -72.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(ctx.content_rect().width() - 40.0);
                    ui.horizontal(|ui| {
                        let color = if t.error {
                            ui.visuals().error_fg_color
                        } else {
                            ui.visuals().text_color()
                        };
                        ui.add(egui::Label::new(RichText::new(&t.msg).color(color)).wrap());
                        if t.undo.is_some() && ui.button(tr("Annuler")).clicked() {
                            undo_clicked = true;
                        }
                        if ui.small_button("✕").clicked() {
                            close = true;
                        }
                    });
                });
            });
        if undo_clicked && let Some(u) = self.toast.take().and_then(|t| t.undo) {
            self.undo_conversion(u);
            self.notify(ctx, tr("Conversion annulée"), false);
        } else if close {
            self.toast = None;
        }
    }

    /// Exports de la suite et fichiers choisis dans « Ouvrir… ».
    fn handle_io(&mut self, ctx: &egui::Context) {
        for ev in self.io.drain() {
            match ev {
                crate::io::Event::Saved { path, .. } => self.notify(
                    ctx,
                    trf!("Enregistré : {path}", path = path.display()),
                    false,
                ),
                crate::io::Event::Info(m) => self.notify(ctx, m, false),
                crate::io::Event::Error(e) => self.notify(ctx, e, true),
                crate::io::Event::Opened { .. } => {}
            }
        }
        let picked = std::mem::take(&mut *self.picked.lock().unwrap_or_else(|e| e.into_inner()));
        for p in picked {
            self.open_any(ctx, &p);
        }
    }

    fn accueil_choice(&mut self, ctx: &egui::Context, c: Choix) {
        match c {
            Choix::Nouveau(m) => self.run(ctx, Cmd::New(m)),
            Choix::Modele(i) => {
                self.switch(Module::Documents);
                self.doc.load_template(ctx, i);
            }
            Choix::ExempleClasseur => {
                self.switch(Module::Classeur);
                self.calc.perform(ctx, CalcAction::Sample);
            }
            Choix::ExemplePresentation => {
                self.switch(Module::Presentation);
                self.slides.perform(ctx, SlideAction::Sample);
            }
            Choix::Ouvrir => depot::choisir(ctx, &self.picked),
            Choix::Recent(m, p) => self.open_in(ctx, m, Path::new(&p)),
            Choix::Fermer => self.show_accueil = false,
        }
    }

    /// La palette et l'aide-mémoire, par-dessus le module.
    fn overlays(&mut self, ctx: &egui::Context) {
        let was_open = self.palette.open || self.aide.open;
        if self.palette.open {
            let entries = self.entries(ctx);
            if let Some(i) = self.palette.show(ctx, &entries, &self.palette_recents) {
                let e = entries[i].clone();
                palette::push_recent(&mut self.palette_recents, &e.id);
                self.run(ctx, e.cmd);
            }
        }
        if self.aide.open {
            let lines = self.shortcut_lines(ctx);
            let title = self.aide_title();
            match self.aide.show(ctx, &title, &lines) {
                Some(Demande::ExportHtml) => {
                    let name = format!(
                        "{}.html",
                        crate::files::short_name(&title).replace([' ', '—'], "-")
                    );
                    self.io.save(
                        ctx,
                        name,
                        "text/html;charset=utf-8",
                        raccourcis::html(&title, &lines).into_bytes(),
                        false,
                    );
                }
                Some(Demande::Imprimer) => {
                    self.io.print(
                        ctx,
                        "varium-raccourcis.html".into(),
                        raccourcis::html(&title, &lines),
                    );
                }
                None => {}
            }
        }
        // Le clavier reste aux fenêtres de la suite : les modules ne le voient pas.
        let open = self.palette.open || self.aide.open;
        if was_open
            && !open
            && let Some(id) = self.prev_focus.take()
        {
            ctx.memory_mut(|m| m.request_focus(id));
        }
        if open {
            ctx.input_mut(|i| {
                i.events
                    .retain(|e| !matches!(e, egui::Event::Key { .. } | egui::Event::Text(_)))
            });
        }
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
        if self.apparence_appliquee != Some((self.apparence, self.zoom_interface)) {
            apparence::appliquer(&ctx, self.apparence, self.zoom_interface);
            self.apparence_appliquee = Some((self.apparence, self.zoom_interface));
        }
        // Langue changée : les titres de fenêtre suivent.
        if self.title_lang != tr::lang() {
            self.title_lang = tr::lang();
            self.doc.refresh_window_title();
            self.last_title.clear();
        }
        // Fermer la fenêtre depuis le Classeur ou la Présentation : si le document a
        // des modifications, on revient sur lui pour la confirmation.
        if (self.module != Module::Documents || self.show_accueil)
            && ctx.input(|i| i.viewport().close_requested())
            && self.doc.has_unsaved_changes()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.show_accueil = false;
            self.module = Module::Documents;
            self.doc.refresh_window_title();
        }

        let presenting = self.module == Module::Presentation && self.slides.is_presenting();
        if !presenting {
            if PALETTE_KEYS
                .iter()
                .any(|s| ctx.input_mut(|i| i.consume_shortcut(s)))
            {
                self.toggle_palette(&ctx);
            }
            if AIDE_KEYS
                .iter()
                .any(|s| ctx.input_mut(|i| i.consume_shortcut(s)))
            {
                self.toggle_aide(&ctx);
            }
            if !self.show_accueil && !self.palette.open && !self.aide.open {
                self.module_shortcuts(&ctx);
            }
        }
        self.handle_io(&ctx);
        self.drag_and_drop(&ctx);
        self.track_recents();

        if !presenting {
            egui::Panel::top("suite-switcher")
                .frame(
                    egui::Frame::side_top_panel(ui.style())
                        .inner_margin(egui::Margin::symmetric(8, 5)),
                )
                .show(ui, |ui| self.switcher(ui));
            self.overlays(&ctx);
        }
        if self.show_accueil && !presenting {
            if let Some(c) = self.accueil.show(ui, &mut self.slides, &self.recents) {
                self.accueil_choice(&ctx, c);
            }
            let t = "Varium".to_string();
            if t != self.last_title {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(t.clone()));
                self.last_title = t;
                self.doc.refresh_window_title();
            }
        } else {
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
        if let Some((paths, cible)) = &self.hover {
            let text = depot::annonce(*cible, self.module, paths.len());
            depot::voile(&ctx, *cible, &text);
        }
        self.toast_ui(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.doc.save(storage);
        self.calc.save(storage);
        self.slides.save(storage);
        eframe::set_value(storage, MODULE_KEY, &self.module);
        eframe::set_value(storage, accueil::STORAGE_KEY, &self.accueil.au_demarrage);
        eframe::set_value(storage, RECENTS_KEY, &self.recents);
        eframe::set_value(storage, PALETTE_KEY, &self.palette_recents);
        storage.set_string(apparence::THEME_KEY, self.apparence.code().to_string());
        eframe::set_value(storage, apparence::ZOOM_KEY, &self.zoom_interface);
        storage.set_string(tr::STORAGE_KEY, tr::choice_code(tr::choice()).to_string());
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recents_de_tous_les_modules() {
        let mut l = Vec::new();
        for i in 0..25 {
            push_recent(&mut l, Module::Classeur, Path::new(&format!("/d/{i}.xlsx")));
        }
        assert_eq!(l.len(), MAX_RECENTS);
        push_recent(&mut l, Module::Documents, Path::new("/d/20.xlsx"));
        assert_eq!(l[0], (Module::Documents, "/d/20.xlsx".to_string()));
        assert_eq!(l.iter().filter(|(_, p)| p == "/d/20.xlsx").count(), 1);
    }

    #[test]
    fn raccourcis_reserves_a_la_suite() {
        assert!(reserved(&KeyboardShortcut::new(Modifiers::NONE, Key::F1)));
        assert!(reserved(&KeyboardShortcut::new(Modifiers::COMMAND, Key::K)));
        assert!(!reserved(&KeyboardShortcut::new(
            Modifiers::COMMAND,
            Key::S
        )));
        // Aucun raccourci d'un module n'est caché par ceux de la suite, sauf F1 du
        // guide de la syntaxe (le guide reste dans la palette et le menu Aide).
        let doc: Vec<_> = VariumApp::palette_entries()
            .into_iter()
            .filter_map(|(a, _, _, s)| s.filter(reserved).map(|_| a))
            .collect();
        assert_eq!(doc, [Act::Syntax]);
        assert!(
            CalcApp::palette_entries()
                .iter()
                .all(|e| e.3.is_none_or(|s| !reserved(&s)))
        );
        assert!(
            SlidesApp::palette_entries()
                .iter()
                .all(|e| e.3.is_none_or(|s| !reserved(&s)))
        );
    }

    #[test]
    fn identifiants_de_commandes_uniques() {
        let mut ids: Vec<String> = VariumApp::palette_entries()
            .iter()
            .map(|(a, fr, _, _)| format!("doc:{a:?}:{fr}"))
            .chain(
                CalcApp::palette_entries()
                    .iter()
                    .map(|(a, fr, ..)| format!("calc:{a:?}:{fr}")),
            )
            .chain(
                SlidesApp::palette_entries()
                    .iter()
                    .map(|(a, fr, ..)| format!("slides:{a:?}:{fr}")),
            )
            .collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }
}
