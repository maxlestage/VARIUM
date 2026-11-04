//! Fichiers que le système demande d'ouvrir : arguments de la ligne de commande
//! (« Ouvrir avec », double-clic sous Windows et Linux grâce aux associations de
//! fichiers de l'installateur), et sous macOS l'événement Apple « open documents »
//! (double-clic dans le Finder, fichier glissé sur l'icône du Dock), que winit ne
//! transmet pas : un gestionnaire est installé à l'exécution avec objc2.
//!
//! Les chemins attendent dans une file ; la suite les prend à chaque image et les
//! ouvre dans le module qui convient (voir [`crate::depot::route`]).

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::depot::{self, Cible};
use crate::suite::Module;

/// Chemins en attente d'ouverture.
static EN_ATTENTE: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
/// Le contexte egui, pour réveiller l'interface quand un fichier arrive.
static CONTEXTE: OnceLock<egui::Context> = OnceLock::new();

/// Les fichiers donnés en arguments (les options, comme `-psn_…` de macOS, sont
/// ignorées ; seuls les fichiers existants comptent).
pub fn arguments() -> Vec<PathBuf> {
    fichiers(std::env::args_os().skip(1).map(PathBuf::from))
}

fn fichiers(args: impl Iterator<Item = PathBuf>) -> Vec<PathBuf> {
    args.filter(|p| !p.to_string_lossy().starts_with('-'))
        .filter(|p| p.is_file())
        .take(64)
        .collect()
}

/// Le premier argument, s'il revient au module Documents (c'est lui qui l'ouvre
/// à sa création) ; les autres fichiers passent par la suite.
pub fn document_arg() -> Option<PathBuf> {
    let p = arguments().into_iter().next()?;
    (depot::route(&depot::nom(&p), &depot::head(&p)) == Cible::Module(Module::Documents))
        .then_some(p)
}

/// Ajoute des chemins à ouvrir et réveille l'interface.
pub fn pousser(chemins: Vec<PathBuf>) {
    if chemins.is_empty() {
        return;
    }
    EN_ATTENTE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .extend(chemins);
    if let Some(ctx) = CONTEXTE.get() {
        ctx.request_repaint();
    }
}

/// Les chemins en attente (la file est vidée).
pub fn prendre() -> Vec<PathBuf> {
    std::mem::take(&mut *EN_ATTENTE.lock().unwrap_or_else(|e| e.into_inner()))
}

/// À la création de la suite : retient le contexte, met en file les arguments
/// que le module Documents n'ouvre pas lui-même, et (macOS) s'assure que le
/// gestionnaire « open documents » est en place.
pub fn installer(ctx: &egui::Context) {
    let _ = CONTEXTE.set(ctx.clone());
    let doc = document_arg();
    let autres: Vec<PathBuf> = arguments()
        .into_iter()
        .filter(|p| Some(p) != doc.as_ref())
        .collect();
    pousser(autres);
    #[cfg(target_os = "macos")]
    mac::installer_gestionnaire();
}

/// Avant le lancement de l'application (macOS) : le gestionnaire « open
/// documents » doit être posé pendant `applicationWillFinishLaunching`, avant
/// que le système n'envoie les fichiers du double-clic qui a lancé Varium.
pub fn avant_lancement() {
    #[cfg(target_os = "macos")]
    mac::observer_le_lancement();
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::CStr;
    use std::os::raw::c_char;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicPtr, Ordering};

    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::{ClassType, class, define_class, msg_send, sel};

    /// Codes Apple Event (quatre caractères).
    const K_CORE_EVENT_CLASS: u32 = u32::from_be_bytes(*b"aevt");
    const K_AE_OPEN_DOCUMENTS: u32 = u32::from_be_bytes(*b"odoc");
    const KEY_DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");
    const TYPE_FILE_URL: u32 = u32::from_be_bytes(*b"furl");

    define_class!(
        // SAFETY : NSObject n'impose rien aux sous-classes ; pas de Drop.
        #[unsafe(super(NSObject))]
        #[name = "VariumOuvreurDeDocuments"]
        struct Ouvreur;

        impl Ouvreur {
            /// Notification `NSApplicationWillFinishLaunchingNotification`.
            #[unsafe(method(applicationWillFinishLaunching:))]
            fn will_finish(&self, _notification: *mut AnyObject) {
                poser(self);
            }

            /// Événement « open documents » : la liste des fichiers.
            #[unsafe(method(ouvrirDocuments:reponse:))]
            fn ouvrir(&self, evenement: *mut AnyObject, _reponse: *mut AnyObject) {
                super::pousser(chemins(evenement));
            }
        }
    );

    /// L'unique gestionnaire, gardé vivant pendant toute l'exécution.
    static OUVREUR: AtomicPtr<Ouvreur> = AtomicPtr::new(std::ptr::null_mut());

    fn ouvreur() -> *mut Ouvreur {
        let p = OUVREUR.load(Ordering::Acquire);
        if !p.is_null() {
            return p;
        }
        // SAFETY : `new` d'une sous-classe de NSObject sans variables d'instance.
        let o: Retained<Ouvreur> = unsafe { msg_send![Ouvreur::class(), new] };
        let p = Retained::into_raw(o);
        match OUVREUR.compare_exchange(std::ptr::null_mut(), p, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => p,
            // Un autre fil l'a créé entre-temps : le nôtre reste simplement inutilisé.
            Err(autre) => autre,
        }
    }

    /// Installe le gestionnaire Apple Event (remplace celui d'AppKit, qui ignore
    /// les fichiers faute de méthode `application:openURLs:` chez winit).
    fn poser(o: &Ouvreur) {
        // SAFETY : messages Foundation documentés ; le gestionnaire vit jusqu'à la fin.
        unsafe {
            let gestionnaire: *mut AnyObject =
                msg_send![class!(NSAppleEventManager), sharedAppleEventManager];
            if gestionnaire.is_null() {
                return;
            }
            let _: () = msg_send![
                gestionnaire,
                setEventHandler: o,
                andSelector: sel!(ouvrirDocuments:reponse:),
                forEventClass: K_CORE_EVENT_CLASS,
                andEventID: K_AE_OPEN_DOCUMENTS
            ];
        }
    }

    pub fn installer_gestionnaire() {
        // SAFETY : pointeur issu de `Retained::into_raw`, jamais libéré.
        if let Some(o) = unsafe { ouvreur().as_ref() } {
            poser(o);
        }
    }

    /// Avant `NSApplication run` : observer `applicationWillFinishLaunching`.
    pub fn observer_le_lancement() {
        // SAFETY : pointeur valide (voir `ouvreur`) ; messages Foundation documentés.
        unsafe {
            let Some(o) = ouvreur().as_ref() else {
                return;
            };
            let centre: *mut AnyObject = msg_send![class!(NSNotificationCenter), defaultCenter];
            let nom: *mut AnyObject = msg_send![
                class!(NSString),
                stringWithUTF8String: c"NSApplicationWillFinishLaunchingNotification".as_ptr()
            ];
            if centre.is_null() || nom.is_null() {
                return;
            }
            let _: () = msg_send![
                centre,
                addObserver: o,
                selector: sel!(applicationWillFinishLaunching:),
                name: nom,
                object: std::ptr::null_mut::<AnyObject>()
            ];
        }
    }

    /// Texte d'un NSString.
    unsafe fn texte(s: *mut AnyObject) -> Option<String> {
        if s.is_null() {
            return None;
        }
        // SAFETY : `s` est un NSString ; UTF8String vit le temps de la copie.
        let p: *const c_char = unsafe { msg_send![s, UTF8String] };
        if p.is_null() {
            return None;
        }
        Some(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
    }

    /// Les fichiers d'un événement « open documents » : une liste de descripteurs
    /// d'URL de fichier.
    fn chemins(evenement: *mut AnyObject) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if evenement.is_null() {
            return out;
        }
        // SAFETY : `evenement` est le NSAppleEventDescriptor reçu d'AppKit.
        unsafe {
            let liste: *mut AnyObject =
                msg_send![evenement, paramDescriptorForKeyword: KEY_DIRECT_OBJECT];
            if liste.is_null() {
                return out;
            }
            let n: isize = msg_send![liste, numberOfItems];
            for i in 1..=n.clamp(0, 1000) {
                let d: *mut AnyObject = msg_send![liste, descriptorAtIndex: i];
                if d.is_null() {
                    continue;
                }
                let url: *mut AnyObject = msg_send![d, coerceToDescriptorType: TYPE_FILE_URL];
                if url.is_null() {
                    continue;
                }
                let s: *mut AnyObject = msg_send![url, stringValue];
                if let Some(chemin) = texte(s).and_then(|u| super::chemin_d_url(&u)) {
                    out.push(chemin);
                }
            }
        }
        out
    }
}

/// Le chemin d'une URL `file://` (encodage `%xx` décodé) ; `None` sinon.
pub fn chemin_d_url(url: &str) -> Option<PathBuf> {
    let reste = url.strip_prefix("file://")?;
    // `file://localhost/…` ou `file:///…`.
    let reste = reste.strip_prefix("localhost").unwrap_or(reste);
    if !reste.starts_with('/') {
        return None;
    }
    let b = reste.as_bytes();
    let mut octets = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(h) = b
                .get(i + 1..i + 3)
                .and_then(|h| std::str::from_utf8(h).ok())
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            octets.push(h);
            i += 3;
            continue;
        }
        octets.push(b[i]);
        i += 1;
    }
    let s = String::from_utf8(octets).ok()?;
    let s = s.trim_end_matches('/');
    (!s.is_empty()).then(|| PathBuf::from(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_de_fichiers() {
        assert_eq!(
            chemin_d_url("file:///Users/camille/Mes%20documents/Devis%20%C3%A9t%C3%A9.xlsx"),
            Some(PathBuf::from("/Users/camille/Mes documents/Devis été.xlsx"))
        );
        assert_eq!(
            chemin_d_url("file://localhost/tmp/a.odp"),
            Some(PathBuf::from("/tmp/a.odp"))
        );
        assert_eq!(chemin_d_url("https://exemple.fr/a"), None);
        assert_eq!(chemin_d_url("file://serveur/partage"), None);
        assert_eq!(chemin_d_url("file:///%"), Some(PathBuf::from("/%")));
        assert_eq!(chemin_d_url("file:///%ff%fe"), None);
    }

    #[test]
    fn file_d_attente() {
        pousser(vec![PathBuf::from("/a"), PathBuf::from("/b")]);
        pousser(Vec::new());
        let pris = prendre();
        assert!(pris.ends_with(&[PathBuf::from("/a"), PathBuf::from("/b")]));
        assert!(prendre().is_empty());
    }

    #[test]
    fn options_ignorees() {
        let dir = std::env::temp_dir();
        let f = dir.join("varium-ouverture-test.csv");
        std::fs::write(&f, "a;b\n").unwrap();
        let v = fichiers(
            [
                PathBuf::from("-psn_0_12345"),
                f.clone(),
                PathBuf::from("/inexistant/x.odp"),
            ]
            .into_iter(),
        );
        assert_eq!(v, [f.clone()]);
        let _ = std::fs::remove_file(f);
    }
}
