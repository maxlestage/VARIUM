//! La mise en page commune : en-tête, navigation, pied de page, balises SEO.

use paquets::prelude::*;

use crate::REPO;

/// Les entrées de la navigation : (chemin, libellé).
const NAV: [(&str, &str); 3] = [
    ("/", "Présentation"),
    ("/syntaxe", "Syntaxe"),
    ("/essayer", "Essayer"),
];

/// Origine publique du site : `SITE_URL`, sinon l'hôte de la requête.
pub fn origine(req: &Request) -> String {
    if let Ok(url) = std::env::var("SITE_URL") {
        return url.trim_end_matches('/').to_string();
    }
    let hote = req.header("host").unwrap_or("localhost:3000");
    let schema = if req.header("x-forwarded-proto") == Some("https") {
        "https"
    } else {
        "http"
    };
    format!("{schema}://{hote}")
}

/// Une page du site : `titre`, `description`, le chemin courant et son contenu.
pub fn page(req: &Request, titre: &str, description: &str, chemin: &str, contenu: Node) -> Page {
    let titre_complet = if chemin == "/" {
        "Varium · Un document. Toutes ses possibilités.".to_string()
    } else {
        format!("{titre} · Varium")
    };
    Page::new(titre_complet)
        .lang("fr")
        .description(description.to_string())
        .seo(
            Seo::new()
                .base_url(origine(req))
                .canonical(chemin.to_string())
                .site_name("Varium")
                .og_type("website")
                .locale("fr_FR"),
        )
        .head(r#"<link rel="icon" type="image/svg+xml" href="/favicon.svg"/>"#)
        .stylesheet("/site.css")
        .body(entete(chemin))
        .body(main().class("contenu").child(contenu))
        .body(pied())
}

fn entete(chemin: &str) -> Element {
    header()
        .class("entete")
        .child(
            a().class("marque")
                .href("/")
                .child(
                    img()
                        .attr("src", "/favicon.svg")
                        .attr("alt", "")
                        .attr("width", "28")
                        .attr("height", "28"),
                )
                .child(span().text("Varium")),
        )
        .child(nav().children(NAV.iter().map(|(href, libelle)| {
            let lien = a().href(href.to_string()).text(libelle.to_string());
            if *href == chemin {
                lien.class("actif").attr("aria-current", "page")
            } else {
                lien
            }
        })))
        .child(a().class("button ghost petit").href(REPO).text("GitHub"))
}

fn pied() -> Element {
    footer()
        .class("pied")
        .child(
            p().class("muted")
                .child(text(
                    "Varium, un éditeur de documents à variables écrit en Rust. ",
                ))
                .child(a().href(REPO).text("Code source")),
        )
        .child(
            p().class("muted")
                .child(text("Ce site est écrit avec "))
                .child(
                    a().href("https://github.com/maxlestage/Paquets")
                        .text("Paquets"),
                )
                .child(text(", le framework full-stack Rust.")),
        )
}
