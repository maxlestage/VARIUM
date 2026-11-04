//! Historique des versions d'un document, comme Pages ou Time Machine.
//!
//! Chaque version est un instantané complet du projet (texte, valeurs, mise en page),
//! écrit dans un dossier propre au document. Les versions identiques à la précédente
//! ne sont pas réécrites. Un élagage garde l'historique raisonnable : toutes les
//! versions nommées, toutes celles des dernières 24 heures, puis une par heure
//! jusqu'à une semaine et une par jour au-delà, dans la limite de
//! [`MAX_VERSIONS`] versions et [`MAX_OCTETS`] octets.
//!
//! Le module ne lit pas l'horloge : l'heure (secondes depuis 1970) est donnée par
//! l'appelant, ce qui rend tout testable.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::project::Project;

/// Nombre maximal de versions non nommées gardées.
pub const MAX_VERSIONS: usize = 200;
/// Taille maximale de l'historique d'un document, en octets.
pub const MAX_OCTETS: u64 = 20_000_000;
/// Intervalle entre deux versions automatiques pendant l'écriture, en secondes.
pub const INTERVALLE_AUTO: u64 = 5 * 60;

const HEURE: u64 = 3600;
const JOUR: u64 = 24 * HEURE;

/// Pourquoi la version a été prise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Motif {
    /// À l'enregistrement du document.
    Enregistrement,
    /// Automatiquement, pendant l'écriture.
    Auto,
    /// Juste avant une opération importante (« Tout corriger », conversion…).
    Avant,
    /// Juste avant de restaurer une ancienne version.
    Restauration,
    /// Demandée par l'utilisateur (« Marquer cette version… »).
    Manuel,
}

/// Une version enregistrée.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Version {
    /// Secondes depuis le 1er janvier 1970 (UTC).
    pub horodatage: u64,
    pub motif: Motif,
    /// Précision du motif, ex. le nom de l'opération (« Tout corriger »).
    #[serde(default)]
    pub detail: String,
    /// Nom donné par l'utilisateur ; une version nommée n'est jamais élaguée.
    #[serde(default)]
    pub nom: Option<String>,
    /// Nombre de mots du texte.
    #[serde(default)]
    pub mots: usize,
    /// Empreinte du contenu (détecte les doublons).
    pub empreinte: u64,
    pub projet: Project,
}

impl Version {
    /// Identifiant stable, aussi nom de fichier (sans extension).
    pub fn id(&self) -> String {
        format!("{:012}-{:016x}", self.horodatage, self.empreinte)
    }
}

/// FNV-1a 64 bits : stable d'une exécution et d'une version de Rust à l'autre.
pub fn fnv(octets: &[u8]) -> u64 {
    octets.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x100_0000_01b3)
    })
}

/// Empreinte d'un projet (titre, texte, valeurs, types, mise en page).
pub fn empreinte(p: &Project) -> u64 {
    fnv(p.to_json().as_bytes())
}

/// Nombre de mots du texte du projet (valeurs des champs comprises).
pub fn mots(p: &Project) -> usize {
    crate::doc::stats(&crate::doc::document(&p.source, &p.values())).words
}

/// Dossier de l'historique d'un document : `base/historique/<empreinte du chemin>`,
/// ou `base/historique/sans-titre` pour un document jamais enregistré.
pub fn dossier(base: &Path, document: Option<&Path>) -> PathBuf {
    let racine = base.join("historique");
    match document {
        Some(d) => {
            let absolu = std::fs::canonicalize(d).unwrap_or_else(|_| d.to_path_buf());
            let nom: String = d
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '-' })
                .take(40)
                .collect();
            racine.join(format!(
                "{nom}-{:016x}",
                fnv(absolu.to_string_lossy().as_bytes())
            ))
        }
        None => racine.join("sans-titre"),
    }
}

/// L'historique d'un document : ses versions, de la plus ancienne à la plus récente.
#[derive(Debug, Clone, Default)]
pub struct Historique {
    pub dossier: PathBuf,
    pub versions: Vec<Version>,
}

impl Historique {
    /// Lit l'historique (dossier absent ou fichiers abîmés : ignorés, jamais d'erreur).
    pub fn ouvrir(dossier: &Path) -> Historique {
        let mut versions: Vec<Version> = std::fs::read_dir(dossier)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .filter_map(|t| serde_json::from_str::<Version>(&t).ok())
            .collect();
        versions.sort_by_key(|v| (v.horodatage, v.id()));
        Historique {
            dossier: dossier.to_path_buf(),
            versions,
        }
    }

    fn fichier(&self, v: &Version) -> PathBuf {
        self.dossier.join(format!("{}.json", v.id()))
    }

    fn ecrire(&self, v: &Version) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dossier)?;
        let f = self.fichier(v);
        let tmp = f.with_extension("tmp");
        std::fs::write(
            &tmp,
            serde_json::to_string(v).map_err(std::io::Error::other)?,
        )?;
        std::fs::rename(tmp, f)
    }

    /// La dernière version.
    pub fn derniere(&self) -> Option<&Version> {
        self.versions.last()
    }

    /// Ajoute une version, sauf si le contenu est identique à la dernière ; renvoie
    /// `true` si elle a été écrite. Élague ensuite.
    pub fn ajouter(
        &mut self,
        projet: &Project,
        motif: Motif,
        detail: &str,
        maintenant: u64,
    ) -> std::io::Result<bool> {
        let e = empreinte(projet);
        if self.derniere().is_some_and(|v| v.empreinte == e) {
            return Ok(false);
        }
        // Deux versions à la même seconde : la seconde suivante (ordre garanti).
        let horodatage = self
            .derniere()
            .map_or(maintenant, |v| maintenant.max(v.horodatage + 1));
        let v = Version {
            horodatage,
            motif,
            detail: detail.to_string(),
            nom: None,
            mots: mots(projet),
            empreinte: e,
            projet: projet.clone(),
        };
        self.ecrire(&v)?;
        self.versions.push(v);
        self.elaguer(maintenant);
        Ok(true)
    }

    /// Donne un nom à une version (vide : retire le nom).
    pub fn nommer(&mut self, id: &str, nom: &str) -> std::io::Result<()> {
        let Some(i) = self.versions.iter().position(|v| v.id() == id) else {
            return Ok(());
        };
        let nom = nom.trim();
        self.versions[i].nom = (!nom.is_empty()).then(|| nom.to_string());
        let v = self.versions[i].clone();
        self.ecrire(&v)
    }

    /// Marque l'état actuel : ajoute une version au besoin, puis la nomme.
    pub fn marquer(&mut self, projet: &Project, nom: &str, maintenant: u64) -> std::io::Result<()> {
        self.ajouter(projet, Motif::Manuel, "", maintenant)?;
        if let Some(id) = self.derniere().map(Version::id) {
            self.nommer(&id, nom)?;
        }
        Ok(())
    }

    /// Supprime une version.
    pub fn supprimer(&mut self, id: &str) {
        if let Some(i) = self.versions.iter().position(|v| v.id() == id) {
            let v = self.versions.remove(i);
            let _ = std::fs::remove_file(self.fichier(&v));
        }
    }

    /// Taille totale des fichiers de l'historique, en octets.
    pub fn octets(&self) -> u64 {
        self.versions
            .iter()
            .filter_map(|v| std::fs::metadata(self.fichier(v)).ok())
            .map(|m| m.len())
            .sum()
    }

    /// Applique la politique de conservation.
    pub fn elaguer(&mut self, maintenant: u64) {
        let garder = conserver(&self.versions, maintenant);
        let mut i = 0;
        let mut supprimees = Vec::new();
        self.versions.retain(|v| {
            let g = garder[i];
            i += 1;
            if !g {
                supprimees.push(v.clone());
            }
            g
        });
        for v in supprimees {
            let _ = std::fs::remove_file(self.fichier(&v));
        }
        // Limite de taille : les plus anciennes non nommées partent d'abord.
        let mut total = self.octets();
        while total > MAX_OCTETS {
            let Some(i) = self
                .versions
                .iter()
                .position(|v| v.nom.is_none())
                .filter(|&i| i + 1 < self.versions.len())
            else {
                break;
            };
            let taille = std::fs::metadata(self.fichier(&self.versions[i])).map_or(0, |m| m.len());
            let id = self.versions[i].id();
            self.supprimer(&id);
            total = total.saturating_sub(taille.max(1));
        }
    }
}

/// Quelles versions garder (même ordre que `versions`, de la plus ancienne à la plus
/// récente) : les nommées, la dernière, celles de moins de 24 h, puis la plus récente
/// de chaque heure jusqu'à 7 jours et de chaque jour au-delà ; au plus
/// [`MAX_VERSIONS`] non nommées.
pub fn conserver(versions: &[Version], maintenant: u64) -> Vec<bool> {
    let mut garder = vec![false; versions.len()];
    let mut creneaux_vus = std::collections::HashSet::new();
    let mut gardees = 0;
    for (i, v) in versions.iter().enumerate().rev() {
        let age = maintenant.saturating_sub(v.horodatage);
        let derniere = i + 1 == versions.len();
        if v.nom.is_some() || derniere {
            garder[i] = true;
            if v.nom.is_none() {
                gardees += 1;
            }
            continue;
        }
        let creneau = if age < JOUR {
            None
        } else if age < 7 * JOUR {
            Some((1, v.horodatage / HEURE))
        } else {
            Some((2, v.horodatage / JOUR))
        };
        let nouveau = creneau.is_none_or(|c| creneaux_vus.insert(c));
        if nouveau && gardees < MAX_VERSIONS {
            garder[i] = true;
            gardees += 1;
        }
    }
    garder
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Values;

    fn dossier_test(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("varium-versions-{nom}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn projet(s: &str) -> Project {
        Project::new("Titre", s, &Values::default())
    }

    #[test]
    fn ajout_dedoublonnage_et_relecture() {
        let d = dossier_test("ajout");
        let mut h = Historique::ouvrir(&d);
        assert!(h.versions.is_empty());
        assert!(
            h.ajouter(&projet("Un deux"), Motif::Auto, "", 1000)
                .unwrap()
        );
        assert!(
            !h.ajouter(&projet("Un deux"), Motif::Enregistrement, "", 1001)
                .unwrap()
        );
        assert!(
            h.ajouter(
                &projet("Un deux trois"),
                Motif::Avant,
                "Tout corriger",
                1000
            )
            .unwrap()
        );
        assert_eq!(h.versions.len(), 2);
        assert_eq!(h.versions[1].horodatage, 1001, "ordre strict");
        assert_eq!(h.versions[1].mots, 3);
        h.nommer(&h.versions[0].id(), "  Premier jet ").unwrap();
        let relu = Historique::ouvrir(&d);
        assert_eq!(relu.versions, h.versions);
        assert_eq!(relu.versions[0].nom.as_deref(), Some("Premier jet"));
        assert_eq!(relu.versions[1].detail, "Tout corriger");
        // Fichier abîmé : ignoré.
        std::fs::write(d.join("abime.json"), "{ pas du json").unwrap();
        assert_eq!(Historique::ouvrir(&d).versions.len(), 2);
        h.supprimer(&h.versions[0].id());
        assert_eq!(Historique::ouvrir(&d).versions.len(), 1);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn marquer_nomme_l_etat_actuel() {
        let d = dossier_test("marquer");
        let mut h = Historique::ouvrir(&d);
        h.ajouter(&projet("a"), Motif::Auto, "", 10).unwrap();
        h.marquer(&projet("a"), "Envoyée au client", 20).unwrap();
        assert_eq!(
            h.versions.len(),
            1,
            "même contenu : la version existante est nommée"
        );
        assert_eq!(h.versions[0].nom.as_deref(), Some("Envoyée au client"));
        h.marquer(&projet("b"), "Après relecture", 30).unwrap();
        assert_eq!(h.versions.len(), 2);
        assert_eq!(h.versions[1].motif, Motif::Manuel);
        let _ = std::fs::remove_dir_all(d);
    }

    fn v(horodatage: u64, nom: Option<&str>) -> Version {
        Version {
            horodatage,
            motif: Motif::Auto,
            detail: String::new(),
            nom: nom.map(str::to_string),
            mots: 0,
            empreinte: horodatage,
            projet: Project::default(),
        }
    }

    #[test]
    fn politique_de_conservation() {
        let maintenant = 100 * JOUR;
        let mut vs = Vec::new();
        // Il y a 30 jours : 10 versions le même jour, dont une nommée.
        for i in 0..10 {
            vs.push(v(70 * JOUR + i * 60, (i == 3).then_some("jalon")));
        }
        // Il y a 3 jours : 6 versions dans la même heure.
        for i in 0..6 {
            vs.push(v(97 * JOUR + i * 60, None));
        }
        // Aujourd'hui : 5 versions, toutes gardées.
        for i in 0..5 {
            vs.push(v(maintenant - 1000 + i * 60, None));
        }
        let g = conserver(&vs, maintenant);
        let gardees: Vec<u64> = vs
            .iter()
            .zip(&g)
            .filter(|(_, g)| **g)
            .map(|(v, _)| v.horodatage)
            .collect();
        assert_eq!(gardees.len(), 1 + 1 + 1 + 5);
        assert!(
            gardees.contains(&(70 * JOUR + 3 * 60)),
            "la version nommée reste"
        );
        assert!(
            gardees.contains(&(70 * JOUR + 9 * 60)),
            "la plus récente du jour"
        );
        assert!(
            gardees.contains(&(97 * JOUR + 5 * 60)),
            "la plus récente de l'heure"
        );
        // Plafond du nombre de versions.
        let beaucoup: Vec<Version> = (0..(MAX_VERSIONS as u64 + 50))
            .map(|i| v(maintenant - 5000 + i, None))
            .collect();
        let g = conserver(&beaucoup, maintenant);
        assert_eq!(g.iter().filter(|x| **x).count(), MAX_VERSIONS);
        assert!(g[g.len() - 1]);
    }

    #[test]
    fn dossiers_par_document() {
        let base = Path::new("/données");
        assert_eq!(dossier(base, None), base.join("historique/sans-titre"));
        let a = dossier(base, Some(Path::new("/a/lettre.varium.json")));
        let b = dossier(base, Some(Path::new("/b/lettre.varium.json")));
        assert_ne!(a, b);
        assert!(a.starts_with(base.join("historique")));
        assert!(
            a.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("lettre-varium-json-")
        );
        assert_eq!(fnv(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
