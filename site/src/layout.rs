//! La mise en page commune : en-tête, navigation, pied de page, balises SEO.

use paquets::prelude::*;

use crate::{Options, REPO};

/// Les entrées de la navigation : (chemin, libellé). Les chemins finissent par `/`,
/// comme les dossiers de l'export statique : GitHub Pages n'a pas à rediriger.
const NAV: [(&str, &str); 4] = [
    ("/", "Présentation"),
    ("/syntaxe/", "Syntaxe"),
    ("/installer/", "Installer"),
    ("/essayer/", "Essayer"),
];

/// Une page du site : `titre`, `description`, le chemin courant (sans la base) et son
/// contenu.
pub fn page(req: &Request, titre: &str, description: &str, chemin: &str, contenu: Node) -> Page {
    let o = Options::de(req);
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
                .base_url(o.origine(req))
                .canonical(chemin.to_string())
                .site_name("Varium")
                .og_type("website")
                .locale("fr_FR"),
        )
        .head(format!(
            r#"<link rel="icon" type="image/svg+xml" href="{}"/>"#,
            o.lien("/favicon.svg")
        ))
        .stylesheet(o.lien("/site.css"))
        .body(entete(o, chemin))
        .body(main().class("contenu").child(contenu))
        .body(pied())
}

fn entete(o: &Options, chemin: &str) -> Element {
    header()
        .class("entete")
        .child(
            a().class("marque")
                .href(o.lien("/"))
                .child(
                    img()
                        .attr("src", o.lien("/favicon.svg"))
                        .attr("alt", "")
                        .attr("width", "28")
                        .attr("height", "28"),
                )
                .child(span().text("Varium")),
        )
        .child(nav().children(NAV.iter().map(|(href, libelle)| {
            let lien = a().href(o.lien(href)).text(libelle.to_string());
            if *href == chemin {
                lien.class("actif").attr("aria-current", "page")
            } else {
                lien
            }
        })))
        .child(
            div()
                .class("actions")
                .child(
                    a().class("button petit")
                        .href(o.lien_application())
                        .text("Installer Varium"),
                )
                .child(a().class("button ghost petit").href(REPO).text("GitHub")),
        )
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
