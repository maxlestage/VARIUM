//! Les adresses du site : `/{langue}/` puis la rubrique, avec les mêmes noms dans
//! toutes les langues (`/fr/syntaxe/`, `/en/syntaxe/`…), si bien que les versions d'une
//! page ne diffèrent que par leur premier segment.
//!
//! | Adresse | Page |
//! |---|---|
//! | `/{langue}/` | l'accueil |
//! | `/{langue}/syntaxe/` | la référence de la syntaxe |
//! | `/{langue}/installer/` | les téléchargements |
//! | `/{langue}/essayer/` | la démo |
//!
//! `/` mène à la langue du navigateur ; les anciennes adresses (`/syntaxe/`…) mènent à
//! la version française.

use crate::i18n::Langue;

/// Une page du site, dans n'importe quelle langue.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rubrique {
    Accueil,
    Syntaxe,
    Installer,
    Essayer,
}

impl Rubrique {
    /// Les pages, dans l'ordre de la navigation.
    pub const ALL: [Rubrique; 4] = [
        Rubrique::Accueil,
        Rubrique::Syntaxe,
        Rubrique::Installer,
        Rubrique::Essayer,
    ];

    /// La fin de l'adresse, après la langue : `""`, `syntaxe/`…
    pub fn segment(self) -> &'static str {
        match self {
            Rubrique::Accueil => "",
            Rubrique::Syntaxe => "syntaxe/",
            Rubrique::Installer => "installer/",
            Rubrique::Essayer => "essayer/",
        }
    }

    /// L'adresse de la page dans `langue`, sans le chemin de base : `/en/syntaxe/`.
    /// Elle finit par `/`, comme les dossiers de l'export statique.
    pub fn chemin(self, langue: Langue) -> String {
        format!("/{}/{}", langue.code(), self.segment())
    }

    /// L'ancienne adresse, d'avant les langues : `/syntaxe/` (`/` pour l'accueil).
    pub fn ancien_chemin(self) -> String {
        format!("/{}", self.segment())
    }

    /// Le nom de la page dans la navigation de `langue`.
    pub fn nom(self, langue: Langue) -> &'static str {
        let i = Rubrique::ALL.iter().position(|r| *r == self).unwrap_or(0);
        langue.textes().nav[i]
    }

    /// L'adresse `x-default` : la page qui choisit la langue pour l'accueil (`/`), la
    /// version française sinon.
    pub fn chemin_par_defaut(self) -> String {
        match self {
            Rubrique::Accueil => "/".to_string(),
            _ => self.chemin(Langue::Fr),
        }
    }
}

/// La langue d'une adresse (sans la base) qui commence par `/{langue}/` ou vaut `/{langue}`.
pub fn langue_du_chemin(chemin: &str) -> Option<Langue> {
    let premier = chemin.trim_start_matches('/').split('/').next()?;
    Langue::depuis_code(premier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresses() {
        assert_eq!(Rubrique::Accueil.chemin(Langue::De), "/de/");
        assert_eq!(Rubrique::Syntaxe.chemin(Langue::En), "/en/syntaxe/");
        assert_eq!(Rubrique::Essayer.ancien_chemin(), "/essayer/");
        assert_eq!(Rubrique::Accueil.chemin_par_defaut(), "/");
        assert_eq!(Rubrique::Installer.chemin_par_defaut(), "/fr/installer/");
        assert_eq!(Rubrique::Syntaxe.nom(Langue::Es), "Sintaxis");
    }

    #[test]
    fn chaque_page_a_une_adresse_par_langue() {
        let mut chemins: Vec<String> = Langue::ALL
            .iter()
            .flat_map(|l| Rubrique::ALL.map(|r| r.chemin(*l)))
            .collect();
        let total = chemins.len();
        chemins.sort();
        chemins.dedup();
        assert_eq!(chemins.len(), total);
        assert_eq!(total, 6 * 4);
    }

    #[test]
    fn langue_d_une_adresse() {
        assert_eq!(langue_du_chemin("/it/syntaxe/"), Some(Langue::It));
        assert_eq!(langue_du_chemin("/pt"), Some(Langue::Pt));
        assert_eq!(langue_du_chemin("/syntaxe/"), None);
        assert_eq!(langue_du_chemin("/"), None);
        assert_eq!(langue_du_chemin("/english/"), None);
    }
}
