//! Logique pure des fichiers façon Writer : titre de fenêtre, documents récents.

use crate::tr::tr;
use std::path::{Path, PathBuf};

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
