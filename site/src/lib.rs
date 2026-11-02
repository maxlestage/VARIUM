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
//! | `GET /essayer`, `POST /essayer` | la démo : un modèle, des valeurs, le document rendu |
//! | `GET /syntaxe` | la référence : variables, filtres, formules, exports |
//! | `POST /api/rendu` | le moteur en JSON : `{source, valeurs}` → texte, HTML, variables |
//! | `GET /sitemap.xml`, `GET /robots.txt` | pour les moteurs de recherche |

mod demo;
pub use demo::LIMITE;
mod layout;
mod pages;

use paquets::prelude::*;

/// Adresse du dépôt.
pub const REPO: &str = "https://github.com/maxlestage/varium";

/// L'application complète. `main.rs` la lance, les tests l'interrogent avec `app().test()`.
pub fn app() -> App {
    let mut app = App::new();

    app.get("/", pages::accueil);
    app.get("/syntaxe", pages::syntaxe);
    app.get("/essayer", demo::page);
    app.post("/essayer", demo::soumettre);
    app.post("/api/rendu", demo::api);
    app.get("/robots.txt", pages::robots);
    app.get("/sitemap.xml", pages::sitemap);
    app.fallback(pages::introuvable);

    app
}
