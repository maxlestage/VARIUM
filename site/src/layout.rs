//! La mise en page commune : en-tête (navigation, langues), pied de page, balises SEO.

use paquets::prelude::*;

use crate::i18n::Langue;
use crate::routes::Rubrique;
use crate::{Options, REPO};

/// Une page du site dans `langue` : `rubrique` (`None` pour une page introuvable, qui
/// n'est pas indexée), son titre, sa description et son contenu.
///
/// La page porte `<html lang>`, son URL canonique, `og:locale` et un lien `hreflang`
/// vers chacune de ses versions, plus `x-default`.
pub fn page(
    req: &Request,
    langue: Langue,
    rubrique: Option<Rubrique>,
    titre: &str,
    description: &str,
    contenu: Node,
) -> Page {
    let o = Options::de(req);
    let t = langue.textes();
    let titre_complet = match rubrique {
        Some(Rubrique::Accueil) => format!("Varium · {}", t.accroche),
        _ => format!("{titre} · Varium"),
    };
    let mut seo = Seo::new()
        .base_url(o.origine(req))
        .site_name("Varium")
        .og_type("website")
        .locale(langue.locale())
        .image(crate::pwa::image(langue));
    match rubrique {
        Some(r) => {
            seo = seo.canonical(r.chemin(langue));
            for l in Langue::ALL {
                seo = seo.alternate(l.code(), r.chemin(l));
            }
            seo = seo.alternate("x-default", r.chemin_par_defaut());
        }
        None => seo = seo.noindex(true),
    }
    // L'adresse partagée : la page elle-même (l'accueil pour une page introuvable).
    let adresse = format!(
        "{}{}",
        o.origine(req),
        rubrique.unwrap_or(Rubrique::Accueil).chemin(langue)
    );
    Page::new(titre_complet)
        .lang(langue.code())
        .description(description.to_string())
        .seo(seo)
        .head(crate::pwa::balises(o, langue))
        .stylesheet(o.lien("/site.css"))
        .body(entete(o, langue, rubrique))
        .body(main().class("contenu").child(contenu))
        .body(crate::pwa::bloc_partage(&adresse, langue))
        .body(pied(langue))
}

/// Une page qui ne choisit pas de langue (l'aiguillage de `/`, les anciennes adresses,
/// l'introuvable de l'export) : en-tête réduit au logo, contenu dans toutes les langues.
pub fn page_neutre(req: &Request, titre: &str, description: &str, contenu: Node) -> Page {
    let o = Options::de(req);
    Page::new(titre.to_string())
        .lang(Langue::Fr.code())
        .description(description.to_string())
        .seo(
            Seo::new()
                .base_url(o.origine(req))
                .site_name("Varium")
                .og_type("website")
                .locale(Langue::Fr.locale())
                .image(crate::pwa::image(Langue::Fr)),
        )
        .head(crate::pwa::balises(o, Langue::Fr))
        .stylesheet(o.lien("/site.css"))
        .body(header().class("entete").child(marque(o, "/", "Varium")))
        .body(main().class("contenu").child(contenu))
        .body(
            footer().class("pied").child(
                p().class("muted")
                    .child(text("Varium · "))
                    .child(a().href(REPO).text("GitHub")),
            ),
        )
}

fn marque(o: &Options, chemin: &str, aria: &str) -> Element {
    a().class("marque")
        .href(o.lien(chemin))
        .attr("aria-label", aria.to_string())
        .child(
            img()
                .attr("src", o.lien("/favicon.svg"))
                .attr("alt", "")
                .attr("width", "28")
                .attr("height", "28"),
        )
        .child(span().text("Varium"))
}

fn entete(o: &Options, langue: Langue, rubrique: Option<Rubrique>) -> Element {
    let t = langue.textes();
    header()
        .class("entete")
        .child(marque(o, &Rubrique::Accueil.chemin(langue), t.accueil_aria))
        .child(
            nav()
                .class("principale")
                .attr("aria-label", t.nav_aria)
                .children(Rubrique::ALL.map(|r| {
                    let lien = a().href(o.lien(&r.chemin(langue))).text(r.nom(langue));
                    if Some(r) == rubrique {
                        lien.class("actif").attr("aria-current", "page")
                    } else {
                        lien
                    }
                })),
        )
        .child(
            div()
                .class("actions")
                .child(langues(o, langue, rubrique))
                .child(
                    a().class("button petit installer")
                        .href(o.lien_application(langue))
                        .text(t.installer_varium),
                )
                .child(a().class("button ghost petit").href(REPO).text("GitHub")),
        )
}

/// Le sélecteur de langue : un menu `<details>` (sans JavaScript) qui mène à la même
/// page dans chaque langue, ou à l'accueil depuis une page introuvable.
fn langues(o: &Options, langue: Langue, rubrique: Option<Rubrique>) -> Element {
    let t = langue.textes();
    let cible = rubrique.unwrap_or(Rubrique::Accueil);
    details()
        .class("langues")
        .child(
            summary()
                .attr("title", t.langue_choisir)
                .child(span().class("globe").attr("aria-hidden", "true").text("🌐"))
                .child(span().class("nom").text(langue.nom()))
                .child(
                    span()
                        .class("code")
                        .attr("aria-hidden", "true")
                        .text(langue.code().to_uppercase()),
                ),
        )
        .child(
            ul().attr("aria-label", t.langue_choisir)
                .children(Langue::ALL.map(|l| {
                    let lien = a()
                        .href(o.lien(&cible.chemin(l)))
                        .attr("hreflang", l.code())
                        .attr("lang", l.code())
                        .text(l.nom());
                    li().child(if l == langue {
                        lien.class("actif").attr("aria-current", "true")
                    } else {
                        lien
                    })
                })),
        )
}

fn pied(langue: Langue) -> Element {
    let t = langue.textes();
    let (avant, apres) = t
        .pied_paquets
        .split_once("{}")
        .unwrap_or((t.pied_paquets, ""));
    footer()
        .class("pied")
        .child(
            p().class("muted")
                .child(text(format!("{} ", t.pied)))
                .child(a().href(REPO).text(t.code_source)),
        )
        .child(
            p().class("muted")
                .child(text(avant.to_string()))
                .child(
                    a().href("https://github.com/maxlestage/Paquets")
                        .text("Paquets"),
                )
                .child(text(apres.to_string())),
        )
}
