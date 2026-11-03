//! # Le site de Varium
//!
//! Le site de présentation de [Varium](https://github.com/maxlestage/varium), écrit
//! entièrement avec [Paquets](https://github.com/maxlestage/Paquets) : un `App`, des
//! `Page` rendues par le serveur (balises SEO, thème, Content-Security-Policy) et une
//! démo qui fait tourner le vrai moteur `varium-core` côté serveur, sans JavaScript.
//!
//! Le site parle six langues ([`Langue`]) : français (la source), anglais, espagnol,
//! allemand, italien et portugais. Chaque page existe dans chacune, sous `/{langue}/`.
//!
//! | Route | Contenu |
//! |---|---|
//! | `GET /` | redirige vers `/{langue}/`, d'après `Accept-Language` (français sinon) |
//! | `GET /{langue}/` | la page d'accueil |
//! | `GET /{langue}/syntaxe/` | la référence : variables, filtres, formules, Classeur, exports |
//! | `GET /{langue}/installer/` | télécharger l'application de bureau |
//! | `GET /{langue}/essayer/`, `POST /{langue}/essayer/` | la démo : un modèle, des valeurs, le document rendu |
//! | `GET /syntaxe/`, `/essayer/`, `/installer/` | les anciennes adresses : redirigent vers `/fr/…` |
//! | `POST /api/rendu` | le moteur en JSON : `{source, valeurs}` → texte, HTML, variables |
//! | `GET /sitemap.xml`, `GET /robots.txt` | pour les moteurs de recherche |
//!
//! Les routes répondent avec ou sans `/` final. Avec un chemin de base (`BASE_PATH`,
//! voir [`Options`]), elles répondent aussi derrière lui : `/VARIUM/fr/syntaxe/`.
//!
//! [`export`] écrit le site en fichiers statiques, pour GitHub Pages.

mod demo;
pub use demo::LIMITE;
pub mod export;
pub mod i18n;
pub use i18n::Langue;
mod layout;
mod options;
pub use options::Options;
mod pages;
pub mod routes;
pub use routes::Rubrique;

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

    app.get("/", pages::racine);
    for langue in Langue::ALL {
        let l = langue.code();
        app.get(&format!("/{l}"), move |req| pages::accueil(req, langue));
        app.get(&format!("/{l}/syntaxe"), move |req| {
            pages::syntaxe(req, langue)
        });
        app.get(&format!("/{l}/installer"), move |req| {
            pages::installer(req, langue)
        });
        app.get(&format!("/{l}/essayer"), move |req| demo::page(req, langue));
        app.post(&format!("/{l}/essayer"), move |req| {
            demo::soumettre(req, langue)
        });
    }
    // Les adresses d'avant les langues, en français.
    for rubrique in [Rubrique::Syntaxe, Rubrique::Installer, Rubrique::Essayer] {
        app.get(&rubrique.ancien_chemin(), move |req| {
            pages::ancienne(req, rubrique)
        });
    }
    app.post("/essayer", |req| demo::soumettre(req, Langue::Fr));
    app.post("/api/rendu", demo::api);
    app.get("/robots.txt", pages::robots);
    app.get("/sitemap.xml", pages::sitemap);
    app.fallback(pages::introuvable_partout);

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
