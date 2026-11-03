//! La configuration du site : chemin de base, adresses publiques, mode statique.
//!
//! Les options sont données à [`app_with`](crate::app_with), qui les range dans l'état de
//! l'`App` : chaque page les lit avec [`Options::de`]. Rien n'est global, si bien que
//! plusieurs configurations peuvent tourner dans le même processus (les tests).

use paquets::prelude::*;

/// L'origine de GitHub Pages pour le dépôt, utilisée par l'export quand `SITE_URL` manque.
const ORIGINE_PAGES: &str = "https://maxlestage.github.io";

/// L'application web publiée sur GitHub Pages : le lien du serveur quand `APP_URL` manque.
pub const APP_PUBLIQUE: &str = "https://maxlestage.github.io/VARIUM/app/";

/// La configuration du site.
///
/// ```
/// use varium_site::Options;
///
/// let o = Options::new().base("/VARIUM/");
/// assert_eq!(o.lien("/syntaxe/"), "/VARIUM/syntaxe/");
/// assert_eq!(o.lien_application(), "/VARIUM/app/");
/// ```
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Préfixe de chaque lien interne, sans `/` final : `""` ou `/VARIUM`.
    base: String,
    /// Adresse publique (origine et base), pour les URL absolues.
    site: Option<String>,
    /// Adresse de l'application web.
    application: Option<String>,
    /// Rendu pour un hôte statique : aucun formulaire envoyé au serveur.
    statique: bool,
}

impl Options {
    /// Aucun chemin de base, adresses tirées de chaque requête, mode serveur.
    pub fn new() -> Options {
        Options::default()
    }

    /// `BASE_PATH`, `SITE_URL` et `APP_URL`, quand ils sont définis et non vides.
    pub fn depuis_env() -> Options {
        let var = |nom: &str| std::env::var(nom).ok().filter(|v| !v.trim().is_empty());
        let mut o = Options::new();
        if let Some(base) = var("BASE_PATH") {
            o = o.base(&base);
        }
        o.site = var("SITE_URL");
        o.application = var("APP_URL");
        o
    }

    /// La configuration du serveur ([`Options::depuis_env`]) : sans `APP_URL`, le
    /// bouton « Ouvrir l'application » mène à l'application publiée sur GitHub Pages,
    /// que le serveur ne sert pas lui-même.
    pub fn serveur() -> Options {
        let o = Options::depuis_env();
        if o.application.is_some() {
            o
        } else {
            o.application(APP_PUBLIQUE)
        }
    }

    /// Le chemin de base : `"/VARIUM"`, `"VARIUM/"` et `"/VARIUM/"` donnent `/VARIUM` ;
    /// `""` et `"/"`, aucun préfixe.
    pub fn base(mut self, base: &str) -> Options {
        let base = base.trim().trim_matches('/');
        self.base = if base.is_empty() {
            String::new()
        } else {
            format!("/{base}")
        };
        self
    }

    /// L'adresse publique du site, base comprise (`https://maxlestage.github.io/VARIUM`) :
    /// URL canoniques, sitemap, robots.txt. Sans elle, l'adresse vient de chaque requête.
    pub fn site(mut self, url: impl Into<String>) -> Options {
        self.site = Some(url.into()).filter(|u| !u.trim().is_empty());
        self
    }

    /// L'adresse de l'application web, ouverte par « Ouvrir l'application ».
    pub fn application(mut self, url: impl Into<String>) -> Options {
        self.application = Some(url.into()).filter(|u| !u.trim().is_empty());
        self
    }

    /// Le rendu pour un hôte statique (GitHub Pages) : `/essayer` montre l'exemple en
    /// lecture seule et renvoie vers l'application, au lieu d'un formulaire.
    pub fn statique(mut self, statique: bool) -> Options {
        self.statique = statique;
        self
    }

    /// Les options rangées dans l'état de l'`App` par [`app_with`](crate::app_with).
    pub fn de(req: &Request) -> &'static Options {
        req.state::<Options>()
    }

    /// Le chemin de base, sans `/` final (vide par défaut).
    pub fn chemin_base(&self) -> &str {
        &self.base
    }

    /// `true` pour l'export statique.
    pub fn est_statique(&self) -> bool {
        self.statique
    }

    /// Les options de l'export : le mode statique et, sans adresse publique, celle de
    /// GitHub Pages derrière la base (`http://localhost:3000` sans base).
    pub fn pour_export(mut self) -> Options {
        self.statique = true;
        if self.site.is_none() {
            self.site = Some(if self.base.is_empty() {
                "http://localhost:3000".to_string()
            } else {
                format!("{ORIGINE_PAGES}{}", self.base)
            });
        }
        self
    }

    /// Un lien interne : `chemin` (qui commence par `/`) derrière le chemin de base. Tous
    /// les liens du site passent par ici : navigation, boutons, feuille de style, icône,
    /// formulaire, URL canoniques et sitemap.
    pub fn lien(&self, chemin: &str) -> String {
        format!("{}{chemin}", self.base)
    }

    /// L'adresse de l'application web : `APP_URL`, sinon `{base}/app/`.
    pub fn lien_application(&self) -> String {
        self.application
            .clone()
            .unwrap_or_else(|| self.lien("/app/"))
    }

    /// L'adresse publique du site, base comprise et sans `/` final : [`Options::site`],
    /// sinon le schéma et l'hôte de la requête suivis de la base.
    pub fn origine(&self, req: &Request) -> String {
        if let Some(url) = &self.site {
            let url = url.trim().trim_end_matches('/');
            return if url.ends_with(&self.base) {
                url.to_string()
            } else {
                format!("{url}{}", self.base)
            };
        }
        let hote = req.header("host").unwrap_or("localhost:3000");
        let schema = if req.header("x-forwarded-proto") == Some("https") {
            "https"
        } else {
            "http"
        };
        format!("{schema}://{hote}{}", self.base)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_la_base() {
        for base in ["/VARIUM", "VARIUM", "/VARIUM/", " VARIUM/ "] {
            assert_eq!(Options::new().base(base).chemin_base(), "/VARIUM");
        }
        for base in ["", "/", "  "] {
            assert_eq!(Options::new().base(base).chemin_base(), "");
        }
        assert_eq!(Options::new().lien("/"), "/");
        assert_eq!(Options::new().base("/VARIUM").lien("/"), "/VARIUM/");
    }

    #[test]
    fn application_par_defaut() {
        assert_eq!(Options::new().lien_application(), "/app/");
        assert_eq!(
            Options::new()
                .base("/VARIUM")
                .application("https://exemple.fr/app/")
                .lien_application(),
            "https://exemple.fr/app/"
        );
    }

    #[test]
    fn options_de_l_export() {
        let o = Options::new().base("/VARIUM").pour_export();
        assert!(o.est_statique());
        assert_eq!(
            o.site.as_deref(),
            Some("https://maxlestage.github.io/VARIUM")
        );
        let o = Options::new().site("https://exemple.fr/").pour_export();
        assert_eq!(o.site.as_deref(), Some("https://exemple.fr/"));
        assert_eq!(
            Options::new().pour_export().site.as_deref(),
            Some("http://localhost:3000")
        );
    }
}
