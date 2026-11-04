//! Barre de menus native de macOS (en haut de l'écran), comme Pages, Numbers et
//! Keynote : elle suit le module actif (Documents, Classeur, Présentation) et la langue
//! de l'interface, et elle est reconstruite quand l'un ou l'autre change.

use std::sync::mpsc::{Receiver, channel};

use muda::accelerator::Accelerator;
use muda::{AboutMetadata, CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use varium_core::Lang;

use crate::calc_ui::CalcAction;
use crate::slides_ui::SlideAction;
use crate::suite::Module;
use crate::tr::{self, tr};

/// Ce que fait un élément de menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuAct {
    Doc(crate::app::Act),
    Calc(CalcAction),
    Slides(SlideAction),
    Module(Module),
    UiLang(Option<Lang>),
    Website,
    /// Palette de commandes (⌘K).
    Palette,
    /// Aide-mémoire des raccourcis.
    Raccourcis,
    /// Centre de démarrage.
    Accueil,
    Convert(crate::palette::Conversion),
}

/// Crée les éléments et retient l'action de chacun.
pub struct Builder {
    prefix: String,
    actions: Vec<(String, MenuAct)>,
}

impl Builder {
    fn id(&mut self, act: MenuAct) -> String {
        let id = format!("{}-{}", self.prefix, self.actions.len());
        self.actions.push((id.clone(), act));
        id
    }

    /// Un élément ; `keys` au format de muda (`Cmd+Shift+S`).
    pub fn item(&mut self, label: &str, act: impl Into<MenuAct>, keys: Option<&str>) -> MenuItem {
        let id = self.id(act.into());
        let accel = keys.and_then(|k| k.parse::<Accelerator>().ok());
        MenuItem::with_id(id, label, true, accel)
    }

    /// Un élément coché ou non.
    pub fn check(&mut self, label: &str, act: impl Into<MenuAct>, on: bool) -> CheckMenuItem {
        let id = self.id(act.into());
        CheckMenuItem::with_id(id, label, true, on, None)
    }
}

pub fn sep() -> PredefinedMenuItem {
    PredefinedMenuItem::separator()
}

/// Couper, Copier, Coller, Tout sélectionner : les éléments standard de macOS, qui
/// laissent le raccourci aux champs de texte d'egui.
pub fn clipboard_items() -> [PredefinedMenuItem; 4] {
    [
        PredefinedMenuItem::cut(Some(tr("Couper"))),
        PredefinedMenuItem::copy(Some(tr("Copier"))),
        PredefinedMenuItem::paste(Some(tr("Coller"))),
        PredefinedMenuItem::select_all(Some(tr("Tout sélectionner"))),
    ]
}

/// La barre installée.
pub struct NativeBar {
    _menu: Menu,
    actions: Vec<(String, MenuAct)>,
    pub module: Module,
    pub lang: Lang,
    pub choice: Option<Lang>,
}

/// Reçoit les clics de menu et réveille egui (sinon l'action attendrait le
/// prochain mouvement de souris).
pub struct Events(Receiver<MenuEvent>);

impl Events {
    pub fn install(ctx: &egui::Context) -> Self {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |e| {
            let _ = tx.send(e);
            ctx.request_repaint();
        }));
        Self(rx)
    }
}

impl NativeBar {
    /// Construit et installe la barre du module ; `None` si macOS la refuse.
    pub fn install(module: Module, generation: u32) -> Option<Self> {
        let mut b = Builder {
            prefix: format!("varium-{generation}"),
            actions: Vec::new(),
        };
        let menus = match module {
            Module::Documents => crate::app::native_menus(&mut b),
            Module::Classeur => crate::calc_ui::native_menus(&mut b),
            Module::Presentation => crate::slides_ui::native_menus(&mut b),
        }
        .ok()?;
        let (help, menus) = menus.split_last()?;

        let about = PredefinedMenuItem::about(
            Some(tr("À propos de Varium")),
            Some(AboutMetadata {
                name: Some("Varium".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
                comments: Some(tr("Un document. Toutes ses possibilités.").into()),
                website: Some("https://maxlestage.github.io/VARIUM/".into()),
                ..Default::default()
            }),
        );
        let choice = tr::choice();
        let langs: Vec<CheckMenuItem> = tr::choices()
            .map(|(c, name)| b.check(name, MenuAct::UiLang(c), c == choice))
            .collect();
        let lang_refs: Vec<&dyn muda::IsMenuItem> =
            langs.iter().map(|i| i as &dyn muda::IsMenuItem).collect();
        let lang_menu = Submenu::with_items(tr("Langue de l'interface"), true, &lang_refs).ok()?;
        let quit = b.item(tr("Quitter Varium"), crate::app::Act::Quit, Some("Cmd+Q"));
        let app_menu = Submenu::with_items(
            "Varium",
            true,
            &[
                &about,
                &sep(),
                &lang_menu,
                &sep(),
                &PredefinedMenuItem::services(Some(tr("Services"))),
                &sep(),
                &PredefinedMenuItem::hide(Some(tr("Masquer Varium"))),
                &PredefinedMenuItem::hide_others(Some(tr("Masquer les autres"))),
                &PredefinedMenuItem::show_all(Some(tr("Tout afficher"))),
                &sep(),
                &quit,
            ],
        )
        .ok()?;
        let modules = [
            (Module::Documents, tr("Documents")),
            (Module::Classeur, tr("Classeur")),
            (Module::Presentation, tr("Présentation")),
        ]
        .map(|(m, name)| b.check(name, MenuAct::Module(m), m == module));
        let accueil = b.item(tr("Accueil — centre de démarrage"), MenuAct::Accueil, None);
        let palette = b.item(tr("Palette de commandes"), MenuAct::Palette, Some("Cmd+K"));
        let raccourcis = b.item(
            tr("Aide-mémoire des raccourcis"),
            MenuAct::Raccourcis,
            Some("Cmd+/"),
        );
        let conversions: Vec<MenuItem> = crate::palette::Conversion::ALL
            .into_iter()
            .map(|c| b.item(tr(c.label_fr()), MenuAct::Convert(c), None))
            .collect();
        let conversion_refs: Vec<&dyn muda::IsMenuItem> = conversions
            .iter()
            .map(|i| i as &dyn muda::IsMenuItem)
            .collect();
        let convert = Submenu::with_items(tr("Convertir"), true, &conversion_refs).ok()?;
        let window = Submenu::with_items(
            tr("Fenêtre"),
            true,
            &[
                &PredefinedMenuItem::minimize(Some(tr("Placer dans le Dock"))),
                &PredefinedMenuItem::maximize(Some(tr("Zoom"))),
                &sep(),
                &modules[0],
                &modules[1],
                &modules[2],
                &sep(),
                &accueil,
                &palette,
                &raccourcis,
                &convert,
                &sep(),
                &PredefinedMenuItem::bring_all_to_front(Some(tr("Tout ramener au premier plan"))),
            ],
        )
        .ok()?;

        let menu = Menu::new();
        menu.append(&app_menu).ok()?;
        for m in menus {
            menu.append(m).ok()?;
        }
        menu.append(&window).ok()?;
        menu.append(help).ok()?;
        menu.init_for_nsapp();
        window.set_as_windows_menu_for_nsapp();
        help.set_as_help_menu_for_nsapp();
        Some(Self {
            _menu: menu,
            actions: b.actions,
            module,
            lang: tr::lang(),
            choice,
        })
    }

    /// Faut-il reconstruire la barre (module ou langue changés) ?
    pub fn stale(&self, module: Module) -> bool {
        self.module != module || self.lang != tr::lang() || self.choice != tr::choice()
    }

    /// Les actions choisies depuis la dernière image.
    pub fn poll(&self, events: &Events) -> Vec<MenuAct> {
        let mut out = Vec::new();
        while let Ok(ev) = events.0.try_recv() {
            if let Some((_, act)) = self.actions.iter().find(|(id, _)| ev.id.0 == *id) {
                out.push(*act);
            }
        }
        out
    }
}

impl From<crate::app::Act> for MenuAct {
    fn from(a: crate::app::Act) -> Self {
        MenuAct::Doc(a)
    }
}

impl From<CalcAction> for MenuAct {
    fn from(a: CalcAction) -> Self {
        MenuAct::Calc(a)
    }
}

impl From<SlideAction> for MenuAct {
    fn from(a: SlideAction) -> Self {
        MenuAct::Slides(a)
    }
}
