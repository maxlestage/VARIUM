//! Sauvegarde de secours automatique (logique pure, testable hors egui).
//!
//! Tant que le document a des modifications non enregistrées, une copie est
//! écrite régulièrement : à côté du fichier ouvert (`.nom.varium-secours`) ou,
//! pour un document sans fichier, dans le dossier de données de Varium. Elle
//! est effacée dès que le document est enregistré ou abandonné volontairement ;
//! après un plantage ou une coupure de courant, elle reste et Varium propose
//! de la restaurer.

use std::path::{Path, PathBuf};
use std::time::Duration;
use varium_core::Project;

/// Intervalle entre deux sauvegardes de secours.
pub const INTERVALLE: Duration = Duration::from_secs(30);

const EXTENSION: &str = "varium-secours";

/// Dossier de données de Varium (sert aux documents sans fichier).
pub fn dossier_donnees() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| {
                let h = PathBuf::from(h);
                if cfg!(target_os = "macos") {
                    h.join("Library/Application Support")
                } else {
                    h.join(".local/share")
                }
            })
        })
        .unwrap_or_else(std::env::temp_dir);
    base.join("varium")
}

/// Fichier de secours d'un document : voisin du fichier s'il existe, sinon
/// `sans-titre.varium-secours` dans `dossier`.
pub fn chemin(document: Option<&Path>, dossier: &Path) -> PathBuf {
    match document.and_then(|d| Some((d.parent()?, d.file_name()?))) {
        Some((parent, nom)) => parent.join(format!(".{}.{EXTENSION}", nom.to_string_lossy())),
        None => dossier.join(format!("sans-titre.{EXTENSION}")),
    }
}

/// Écrit la sauvegarde de secours (atomiquement ; crée le dossier au besoin).
pub fn ecrire(fichier: &Path, projet: &Project) -> std::io::Result<()> {
    if let Some(dir) = fichier.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::io::write_atomic(fichier, projet.to_json().as_bytes())
}

/// Lit une sauvegarde ; `None` si absente ou illisible (jamais de panique).
pub fn lire(fichier: &Path) -> Option<Project> {
    let texte = std::fs::read_to_string(fichier).ok()?;
    Project::from_json(&texte).ok()
}

/// Supprime la sauvegarde (sans erreur si elle n'existe pas).
pub fn effacer(fichier: &Path) {
    let _ = std::fs::remove_file(fichier);
}

/// La sauvegarde est-elle plus récente que le document enregistré ?
/// (Sans fichier de document, toute sauvegarde compte.)
pub fn plus_recente(secours: &Path, document: Option<&Path>) -> bool {
    let Ok(ms) = std::fs::metadata(secours).and_then(|m| m.modified()) else {
        return false;
    };
    match document.and_then(|d| std::fs::metadata(d).and_then(|m| m.modified()).ok()) {
        Some(md) => ms > md,
        None => true,
    }
}

/// Faut-il écrire une sauvegarde maintenant ?
pub fn echeance(maintenant: f64, derniere: f64) -> bool {
    maintenant - derniere >= INTERVALLE.as_secs_f64()
}

#[cfg(test)]
mod tests {
    use super::*;
    use varium_core::Values;

    fn dossier_test(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("varium-secours-{nom}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn chemins() {
        let d = Path::new("/données");
        assert_eq!(
            chemin(Some(Path::new("/a/b/lettre.varium.json")), d),
            PathBuf::from("/a/b/.lettre.varium.json.varium-secours")
        );
        assert_eq!(
            chemin(None, d),
            PathBuf::from("/données/sans-titre.varium-secours")
        );
        // Chemin sans nom de fichier : repli sur le dossier de données.
        assert_eq!(chemin(Some(Path::new("/")), d), chemin(None, d));
    }

    #[test]
    fn aller_retour_et_effacement() {
        let dir = dossier_test("ar");
        let f = chemin(None, &dir.join("sous/dossier"));
        let p = Project::new("Titre", "Bonjour {{nom}} é😀", &Values::default());
        ecrire(&f, &p).unwrap();
        let lu = lire(&f).expect("lisible");
        assert_eq!(lu.title, "Titre");
        assert_eq!(lu.source, "Bonjour {{nom}} é😀");
        effacer(&f);
        assert!(lire(&f).is_none());
        effacer(&f); // idempotent
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn secours_corrompu_ignore() {
        let dir = dossier_test("corrompu");
        let f = dir.join("x.varium-secours");
        std::fs::write(&f, "{ pas du json").unwrap();
        assert!(lire(&f).is_none());
        std::fs::write(&f, [0xff, 0xfe, 0x00]).unwrap();
        assert!(lire(&f).is_none());
        assert!(lire(&dir.join("absent")).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fraicheur() {
        let dir = dossier_test("frais");
        let doc = dir.join("d.varium.json");
        let sec = chemin(Some(&doc), &dir);
        assert!(!plus_recente(&sec, Some(&doc)), "pas de secours");
        std::fs::write(&doc, "{}").unwrap();
        std::thread::sleep(Duration::from_millis(50));
        ecrire(&sec, &Project::new("t", "s", &Values::default())).unwrap();
        assert!(plus_recente(&sec, Some(&doc)));
        assert!(plus_recente(&sec, None));
        std::thread::sleep(Duration::from_millis(50));
        std::fs::write(&doc, "{}").unwrap();
        assert!(!plus_recente(&sec, Some(&doc)), "document plus récent");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn echeances() {
        assert!(!echeance(10.0, 0.0));
        assert!(echeance(30.0, 0.0));
        assert!(echeance(100.0, 70.0));
    }
}
