//! Logique pure des fichiers façon Writer : titre de fenêtre, documents récents,
//! état « modifié » et confirmation avant de remplacer le document (sans egui, donc
//! testable).

use crate::tr::tr;
use std::path::{Path, PathBuf};
use varium_core::{PageSetup, Values};

/// L'état d'un document à un instant (dernier enregistrement, annuler/rétablir).
#[derive(Clone, PartialEq, Default, Debug)]
pub struct Snapshot {
    pub title: String,
    pub source: String,
    pub values: Values,
    pub page: PageSetup,
    pub objectif: Option<u32>,
}

impl Snapshot {
    /// Le document affiché diffère-t-il de cet état ? (sans rien copier)
    pub fn differe(
        &self,
        title: &str,
        source: &str,
        values: &Values,
        page: &PageSetup,
        objectif: Option<u32>,
    ) -> bool {
        self.source != source
            || self.title != title
            || self.page != *page
            || self.values != *values
            || self.objectif != objectif
    }
}

/// Avant une action qui remplace le document (nouveau, ouvrir, quitter).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Garde<P> {
    /// Rien à perdre : l'action se fait tout de suite.
    Continuer(P),
    /// Des modifications ne sont pas enregistrées : « Enregistrer les modifications ? ».
    Confirmer(P),
}

pub fn garder<P>(modifie: bool, action: P) -> Garde<P> {
    if modifie {
        Garde::Confirmer(action)
    } else {
        Garde::Continuer(action)
    }
}

/// La réponse à « Enregistrer les modifications ? ».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reponse {
    Enregistrer,
    NePasEnregistrer,
    Annuler,
}

/// L'issue d'un enregistrement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    Fait,
    /// Le dialogue « Enregistrer sous » est ouvert : l'action attendra sa fin.
    Demande,
    Echec,
}

/// Ce que devient l'action différée après la réponse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Suite<P> {
    /// La faire maintenant.
    Faire(P),
    /// La faire quand « Enregistrer sous » aura abouti.
    ApresEnregistrement(P),
    /// L'abandonner (Annuler, ou enregistrement impossible).
    Abandonner,
}

/// Après la réponse à la confirmation ; `enregistrer` n'est appelé que si l'on a
/// choisi « Enregistrer ».
pub fn apres_confirmation<P>(
    reponse: Reponse,
    action: P,
    enregistrer: impl FnOnce() -> Issue,
) -> Suite<P> {
    match reponse {
        Reponse::Annuler => Suite::Abandonner,
        Reponse::NePasEnregistrer => Suite::Faire(action),
        Reponse::Enregistrer => match enregistrer() {
            Issue::Fait => Suite::Faire(action),
            Issue::Demande => Suite::ApresEnregistrement(action),
            Issue::Echec => Suite::Abandonner,
        },
    }
}

/// Nombre de documents récents conservés.
pub const MAX_RECENT: usize = 10;

/// Titre de la fenêtre : `* Titre — fichier — Varium` quand il y a des
/// modifications non enregistrées.
pub fn window_title(title: &str, path: Option<&Path>, dirty: bool) -> String {
    let title = title.trim();
    let name = path
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy());
    let mut s = String::new();
    if dirty {
        s.push_str("• ");
    }
    match (title.is_empty(), name) {
        (true, None) => s.push_str(tr("Sans titre")),
        (true, Some(n)) => s.push_str(&n),
        (false, None) => s.push_str(title),
        (false, Some(n)) => s.push_str(&format!("{title} — {n}")),
    }
    s.push_str(" — Varium");
    s
}

/// Place `path` en tête de la liste des récents, sans doublon.
pub fn push_recent(list: &mut Vec<String>, path: &Path) {
    let p = path.display().to_string();
    list.retain(|x| *x != p);
    list.insert(0, p);
    list.truncate(MAX_RECENT);
}

/// Retire un fichier devenu introuvable.
pub fn forget_recent(list: &mut Vec<String>, path: &str) {
    list.retain(|x| x != path);
}

/// Nom court d'un chemin pour les menus.
pub fn short_name(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned())
}

/// Le nom désigne-t-il un projet Varium (et non un modèle texte) ?
pub fn is_project_file(name: &str) -> bool {
    let n = name.to_lowercase();
    n.ends_with(".json") || n.ends_with(".varium")
}

/// Relit le projet d'un fichier connu (restauration au démarrage). `None` si
/// le fichier a disparu ou n'est pas lisible ; un projet abîmé est récupéré
/// au mieux, sans jamais toucher au fichier.
pub fn lire_projet(path: &Path) -> Option<varium_core::project::Project> {
    let texte = std::fs::read_to_string(path).ok()?;
    match varium_core::project::Project::from_json(&texte) {
        Ok(p) => Some(p),
        Err(_) => varium_core::project::Project::recuperer(&texte).map(|(p, _)| p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un document comme celui de l'application : le texte affiché, l'état enregistré,
    /// l'enregistrement en cours (« Enregistrer sous ») et l'action différée.
    struct Doc {
        courant: Snapshot,
        enregistre: Snapshot,
        en_cours: Option<Snapshot>,
        chemin: Option<PathBuf>,
        apres: Option<&'static str>,
        fait: Vec<&'static str>,
    }

    impl Doc {
        fn ouvrir(texte: &str, chemin: Option<&str>) -> Doc {
            let s = Snapshot {
                title: "Lettre".into(),
                source: texte.into(),
                ..Snapshot::default()
            };
            Doc {
                courant: s.clone(),
                enregistre: s,
                en_cours: None,
                chemin: chemin.map(PathBuf::from),
                apres: None,
                fait: Vec::new(),
            }
        }
        fn modifie(&self) -> bool {
            let c = &self.courant;
            self.enregistre
                .differe(&c.title, &c.source, &c.values, &c.page, c.objectif)
        }
        /// Ctrl+S : réécrit le fichier connu, sinon « Enregistrer sous ».
        fn enregistrer(&mut self) -> Issue {
            if self.chemin.is_some() {
                self.enregistre = self.courant.clone();
                Issue::Fait
            } else {
                self.en_cours = Some(self.courant.clone());
                Issue::Demande
            }
        }
        /// Fin du dialogue « Enregistrer sous ».
        fn enregistre_sous(&mut self, chemin: &str) {
            self.enregistre = self.en_cours.take().unwrap_or_else(|| self.courant.clone());
            self.chemin = Some(chemin.into());
            if let Some(a) = self.apres.take() {
                self.fait.push(a);
            }
        }
        fn demander(&mut self, action: &'static str) -> bool {
            match garder(self.modifie(), action) {
                Garde::Continuer(a) => {
                    self.fait.push(a);
                    false
                }
                Garde::Confirmer(_) => true,
            }
        }
        fn repondre(&mut self, r: Reponse, action: &'static str) {
            let mut issue = Issue::Echec;
            let suite = apres_confirmation(r, action, || {
                issue = self.enregistrer();
                issue
            });
            match suite {
                Suite::Faire(a) => self.fait.push(a),
                Suite::ApresEnregistrement(a) => self.apres = Some(a),
                Suite::Abandonner => {}
            }
        }
    }

    #[test]
    fn ouvrir_modifier_enregistrer() {
        let mut d = Doc::ouvrir("Bonjour", Some("/d/lettre.varium.json"));
        assert!(!d.modifie());
        d.courant.source.push_str(" Marc");
        assert!(d.modifie());
        // Revenir au texte d'origine : plus rien à enregistrer.
        d.courant.source = "Bonjour".into();
        assert!(!d.modifie());
        d.courant.values.set("nom", "Marc");
        assert!(d.modifie());
        assert_eq!(d.enregistrer(), Issue::Fait);
        assert!(!d.modifie());
        d.courant.page.landscape = true;
        assert!(d.modifie(), "la mise en page compte");
        d.enregistrer();
        d.courant.objectif = Some(300);
        assert!(d.modifie(), "l'objectif de mots compte");
    }

    #[test]
    fn fermer_ou_ouvrir_demande_confirmation_si_modifie() {
        let mut d = Doc::ouvrir("Texte", Some("/d/a.json"));
        assert!(!d.demander("quitter"), "propre : pas de question");
        assert_eq!(d.fait, ["quitter"]);
        d.courant.source = "Autre".into();
        assert!(d.demander("ouvrir"));
        // Annuler : rien ne se passe, le document reste modifié.
        d.repondre(Reponse::Annuler, "ouvrir");
        assert_eq!(d.fait, ["quitter"]);
        assert!(d.modifie());
        // Ne pas enregistrer : l'action se fait, les modifications sont perdues.
        d.repondre(Reponse::NePasEnregistrer, "ouvrir");
        assert_eq!(d.fait, ["quitter", "ouvrir"]);
        // Enregistrer un fichier connu : enregistré, puis l'action.
        d.repondre(Reponse::Enregistrer, "nouveau");
        assert!(!d.modifie());
        assert_eq!(d.fait.last(), Some(&"nouveau"));
    }

    #[test]
    fn enregistrer_sous_differe_l_action() {
        let mut d = Doc::ouvrir("", None);
        d.courant.source = "Nouveau texte".into();
        assert!(d.demander("quitter"));
        d.repondre(Reponse::Enregistrer, "quitter");
        assert!(d.fait.is_empty(), "attend la fin de « Enregistrer sous »");
        // On tape encore pendant que le dialogue est ouvert : c'est l'état envoyé qui
        // est enregistré, le document reste donc modifié.
        d.courant.source.push('!');
        d.enregistre_sous("/d/nouveau.varium.json");
        assert_eq!(d.fait, ["quitter"]);
        assert!(d.modifie());
        assert_eq!(
            d.chemin.as_deref(),
            Some(Path::new("/d/nouveau.varium.json"))
        );
        // Un enregistrement impossible abandonne l'action.
        let s = apres_confirmation(Reponse::Enregistrer, "x", || Issue::Echec);
        assert_eq!(s, Suite::Abandonner);
    }

    #[test]
    fn lire_projet_relit_recupere_ou_abandonne() {
        let dir = std::env::temp_dir().join(format!("varium-lire-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ok = dir.join("a.varium.json");
        let projet = varium_core::project::Project::new(
            "T",
            "Bonjour {{nom}}",
            &varium_core::render::Values::default(),
        );
        std::fs::write(&ok, projet.to_json()).unwrap();
        assert_eq!(lire_projet(&ok).unwrap().source, "Bonjour {{nom}}");
        let abime = dir.join("b.varium.json");
        let json = projet.to_json();
        std::fs::write(&abime, &json[..json.len() / 2]).unwrap();
        // Tronqué : récupéré ou abandonné, mais jamais de panique.
        let _ = lire_projet(&abime);
        assert!(lire_projet(&dir.join("absent.json")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn titre_de_fenetre() {
        assert_eq!(window_title("Lettre", None, false), "Lettre — Varium");
        assert_eq!(
            window_title("Lettre", Some(Path::new("/a/b/l.varium.json")), true),
            "• Lettre — l.varium.json — Varium"
        );
        assert_eq!(window_title("  ", None, false), "Sans titre — Varium");
        assert_eq!(
            window_title("", Some(Path::new("x.json")), false),
            "x.json — Varium"
        );
    }

    #[test]
    fn recents_sans_doublon_et_bornes() {
        let mut l = Vec::new();
        for i in 0..15 {
            push_recent(&mut l, Path::new(&format!("/d/{i}.json")));
        }
        assert_eq!(l.len(), MAX_RECENT);
        assert_eq!(l[0], "/d/14.json");
        push_recent(&mut l, Path::new("/d/10.json"));
        assert_eq!(l[0], "/d/10.json");
        assert_eq!(l.iter().filter(|x| *x == "/d/10.json").count(), 1);
        forget_recent(&mut l, "/d/10.json");
        assert!(!l.contains(&"/d/10.json".to_string()));
    }

    #[test]
    fn noms_courts_et_extensions() {
        assert_eq!(short_name("/a/b/c.json"), "c.json");
        assert_eq!(short_name(""), "");
        assert!(is_project_file("A.VARIUM.JSON"));
        assert!(!is_project_file("a.md"));
    }
}
