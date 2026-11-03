//! # Le site de Varium
//!
//! Le site de présentation de [Varium](https://github.com/maxlestage/varium), écrit
//! entièrement avec [Paquets](https://github.com/maxlestage/Paquets) : un `App`, des
//! `Page` rendues par le serveur (balises SEO, thème, Content-Security-Policy) et une
//! démo qui fait tourner le vrai moteur `varium-core` côté serveur, sans JavaScript.
//!
//! | Route | Contenu |
//! |---|---|
//! | `GET /` | la page d'accueil |
//! | `GET /essayer/`, `POST /essayer/` | la démo : un modèle, des valeurs, le document rendu |
//! | `GET /syntaxe/` | la référence : variables, filtres, formules, exports |
//! | `POST /api/rendu` | le moteur en JSON : `{source, valeurs}` → texte, HTML, variables |
//! | `GET /sitemap.xml`, `GET /robots.txt` | pour les moteurs de recherche |
//!
//! Les routes répondent avec ou sans `/` final. Avec un chemin de base (`BASE_PATH`,
//! voir [`Options`]), elles répondent aussi derrière lui : `/VARIUM/syntaxe/`.
//!
//! [`export`] écrit le site en fichiers statiques, pour GitHub Pages.

mod demo;
pub use demo::LIMITE;
pub mod export;
mod layout;
mod options;
pub use options::Options;
mod pages;

use paquets::prelude::*;
use paquets::vitesse::http::Uri;

/// Adresse du dépôt.
pub const REPO: &str = "https://github.com/maxlestage/varium";

/// Le serveur, configuré par l'environnement (voir [`Options::serveur`]). `main.rs` le
/// lance, les tests l'interrogent avec `app().test()`.
pub fn app() -> App {
    app_with(Options::serveur())
}

/// L'application complète, avec ces options.
pub fn app_with(options: Options) -> App {
    let mut app = App::new();

    if !options.chemin_base().is_empty() {
        // `/VARIUM/syntaxe/` est servi comme `/syntaxe/`, fichiers de `public/` compris :
        // le serveur répond aux liens qu'il écrit, comme GitHub Pages.
        let base = options.chemin_base().to_string();
        app.middleware(move |mut req: Request, next: Next| {
            if let Some(uri) = sans_base(&base, &req) {
                req.set_uri(uri);
            }
            next.run(req)
        });
    }
    app.state(options);

    app.get("/", pages::accueil);
    app.get("/syntaxe", pages::syntaxe);
    app.get("/essayer", demo::page);
    app.get("/installer", pages::installer);
    app.post("/essayer", demo::soumettre);
    app.post("/api/rendu", demo::api);
    app.get("/robots.txt", pages::robots);
    app.get("/sitemap.xml", pages::sitemap);
    app.fallback(pages::introuvable);

    app
}

/// L'adresse de la requête sans le chemin de base, quand elle commence par lui.
fn sans_base(base: &str, req: &Request) -> Option<Uri> {
    let reste = req.path().strip_prefix(base)?;
    if !(reste.is_empty() || reste.starts_with('/')) {
        return None;
    }
    let chemin = if reste.is_empty() { "/" } else { reste };
    let cible = match req.uri().query() {
        Some(q) => format!("{chemin}?{q}"),
        None => chemin.to_string(),
    };
    cible.parse().ok()
}
