//! Les pages : accueil, syntaxe, installation, introuvable, l'aiguillage vers une langue
//! (`/` et les anciennes adresses), et les fichiers pour les moteurs de recherche.

use paquets::prelude::*;
use varium_core::{Values, VarType, filters};

use crate::demo::{document, libelle_type, source_coloree};
use crate::i18n::{Langue, Textes, langue_preferee, remplir};
use crate::routes::{Rubrique, langue_du_chemin};
use crate::{Options, REPO, layout};

fn code_bloc(code_source: &str) -> Element {
    pre().child(code().text(code_source.to_string()))
}

fn carte(icone: &str, titre: &str, texte: &str) -> Element {
    article()
        .class("card fonction")
        .child(
            span()
                .class("icone")
                .attr("aria-hidden", "true")
                .text(icone.to_string()),
        )
        .child(h3().text(titre.to_string()))
        .child(p().class("muted").text(texte.to_string()))
}

/// Un texte avec un `{}` remplacé par un élément (un lien, en général).
fn a_trou(modele: &str, element: Element) -> Vec<Node> {
    let (avant, apres) = modele.split_once("{}").unwrap_or((modele, ""));
    vec![
        text(avant.to_string()),
        element.into(),
        text(apres.to_string()),
    ]
}

/// La description d'une page, pour `<meta name="description">`.
fn description(rubrique: Rubrique, langue: Langue, statique: bool) -> &'static str {
    let t = langue.textes();
    match rubrique {
        Rubrique::Accueil => t.description_accueil,
        Rubrique::Syntaxe => t.syntaxe_description,
        Rubrique::Installer => t.installer_description,
        Rubrique::Essayer => t.essayer_description[usize::from(statique)],
    }
}

// ---------------------------------------------------------------------------------
// Accueil
// ---------------------------------------------------------------------------------

/// Le modèle de l'aperçu (en français, comme les formats que le moteur lui applique).
const AVANT: &str = "## Dossier {{référence}}\nBonjour **{{prénom}} {{nom | upper}}**,\n\nVotre dossier d'un montant de **{{montant | money}}** sera clôturé le {{échéance | date:full}}, soit {{= montant / 12 | money}} par mois.\n\n-> À bientôt, {{prénom}} ! <-";

fn valeurs_accueil() -> Values {
    let mut v = Values::default();
    v.set("prénom", "Camille")
        .set("nom", "Durand")
        .set("référence", "REF-2026-042")
        .set("montant", "12500")
        .set("échéance", "2026-12-18");
    v
}

/// Les trois applications : (icône, nom). Les noms sont ceux de l'application, en
/// français dans toutes les langues ; leur genre est traduit.
const APPLIS: [(&str, &str); 3] = [
    ("📝", "Documents"),
    ("📊", "Classeur"),
    ("🎞️", "Présentation"),
];

/// Les icônes des cartes de [`Textes::langues`] : typographie, révision, formats,
/// orthographe.
const ICONES_LANGUES: [&str; 4] = ["«»", "✔️", "📅", "📖"];

/// Les icônes des cartes de [`Textes::fonctions`].
const ICONES_FONCTIONS: [&str; 12] = [
    "🔁", "🧮", "🔀", "✉️", "🗂️", "🏷️", "📄", "📤", "💾", "🔍", "∑", "📱",
];

/// Les paquets du workspace, dans l'ordre de [`Textes::capot_roles`].
const PAQUETS: [&str; 4] = ["varium-core", "varium-calc", "varium-slides", "varium"];

const EXEMPLE_RUST: &str = "use varium_core::{Values, Missing, render};\n\nlet mut v = Values::default();\nv.set(\"prénom\", \"Marc\");\nlet doc = render(\"Bonjour {{prénom}} {{nom}}\", &v, Missing::Keep);\nassert_eq!(doc, \"Bonjour Marc {{nom}}\");";

/// `GET /{langue}/`
pub async fn accueil(req: Request, langue: Langue) -> Page {
    let o = Options::de(&req);
    let t = langue.textes();
    let valeurs = valeurs_accueil();

    let hero = section()
        .class("hero")
        .child(p().class("surtitre").text(t.surtitre))
        .child(h1().text(t.accroche))
        .child(p().class("chapo").text(t.chapo))
        .child(
            div()
                .class("row centre")
                .child(
                    a().class("button")
                        .href(o.lien_application(langue))
                        .text(t.installer_varium),
                )
                .child(
                    a().class("button ghost")
                        .href(o.lien(&Rubrique::Essayer.chemin(langue)))
                        .text(t.voir_exemple),
                )
                .child(a().class("button ghost").href(REPO).text(t.code_source)),
        )
        .child(p().class("muted petit").text(t.sous_hero));

    let apercu = section()
        .class("apercu-bloc")
        .child(
            div()
                .class("apercu")
                .attr("lang", "fr")
                .child(
                    div()
                        .class("card")
                        .child(
                            p().class("etiquette")
                                .text(t.apercu[0])
                                .attr("lang", langue.code()),
                        )
                        .child(source_coloree(AVANT)),
                )
                .child(div().class("fleche").attr("aria-hidden", "true").text("→"))
                .child(
                    div()
                        .class("card")
                        .child(
                            p().class("etiquette")
                                .text(t.apercu[1])
                                .attr("lang", langue.code()),
                        )
                        .child(document(AVANT, &valeurs, Langue::Fr.textes())),
                ),
        )
        .child(p().class("center muted petit").text(t.apercu_note));

    // Les noms des applications sont français : les lecteurs d'écran les lisent comme tels.
    let nom_appli = |nom: &str| {
        let titre = h3().text(nom.to_string());
        if langue == Langue::Fr {
            titre
        } else {
            titre.attr("lang", "fr")
        }
    };
    let applis = section()
        .child(h2().class("center").text(t.applis_titre))
        .child(p().class("center muted").text(t.applis_intro))
        .child(
            div()
                .class("grid applis")
                .children(APPLIS.iter().zip(&t.applis).map(|((icone, nom), appli)| {
                    article()
                        .class("card appli")
                        .child(
                            span()
                                .class("icone")
                                .attr("aria-hidden", "true")
                                .text(icone.to_string()),
                        )
                        .child(nom_appli(nom))
                        .child(p().class("genre").text(appli.genre))
                        .child(p().class("muted").text(appli.texte))
                        .child(
                            ul().class("points")
                                .children(appli.points.map(|pt| li().text(pt))),
                        )
                })),
        );

    let ecrire = section()
        .child(h2().class("center").text(t.langues_titre))
        .child(p().class("center muted").text(t.langues_intro))
        .child(
            ul().class("pastilles centre")
                .children(Langue::ALL.map(|l| li().attr("lang", l.code()).text(l.nom()))),
        )
        .child(
            div().class("grid").children(
                ICONES_LANGUES
                    .iter()
                    .zip(&t.langues)
                    .map(|(icone, item)| carte(icone, item.titre, item.texte)),
            ),
        )
        .child(p().class("center muted petit").text(t.langues_note));

    let fonctions = section()
        .child(h2().class("center").text(t.fonctions_titre))
        .child(p().class("center muted").text(t.fonctions_intro))
        .child(
            div().class("grid").children(
                ICONES_FONCTIONS
                    .iter()
                    .zip(&t.fonctions)
                    .map(|(icone, item)| carte(icone, item.titre, item.texte)),
            ),
        );

    let etapes = section()
        .child(h2().class("center").text(t.etapes_titre))
        .child(ol().class("etapes").children(t.etapes.iter().map(|etape| {
            li().child(h3().text(etape.titre))
                .child(p().class("muted").text(etape.texte))
        })));

    let plateformes = section()
        .class("deux")
        .child(
            div()
                .class("card")
                .child(h3().text(format!("🖥️ {}", t.bureau.titre)))
                .child(p().class("muted").text(t.bureau.texte))
                .child(code_bloc("cargo run -p varium --release")),
        )
        .child(
            div()
                .class("card")
                .child(h3().text(format!("🔒 {}", t.prive.titre)))
                .child(p().class("muted").text(t.prive.texte))
                .child(
                    p().child(
                        a().href(o.lien_application(langue))
                            .text(format!("{} →", t.installer_varium)),
                    ),
                ),
        );

    let architecture = section()
        .child(h2().class("center").text(t.capot_titre))
        .child(p().class("center muted").text(t.capot_intro))
        .child(
            table()
                .class("paquets")
                .child(
                    thead().child(
                        tr().child(th().text(t.capot_colonnes[0]))
                            .child(th().text(t.capot_colonnes[1])),
                    ),
                )
                .child(tbody().children(PAQUETS.iter().zip(t.capot_roles).map(
                    |(paquet, role)| {
                        tr().child(td().child(code().text(paquet.to_string())))
                            .child(td().text(role))
                    },
                ))),
        )
        .child(code_bloc(EXEMPLE_RUST));

    let appel = section()
        .class("hero appel")
        .child(h2().text(t.appel_titre))
        .child(p().class("muted").text(t.appel_texte))
        .child(
            div()
                .class("row centre")
                .child(
                    a().class("button")
                        .href(o.lien_application(langue))
                        .text(t.installer_varium),
                )
                .child(
                    a().class("button ghost")
                        .href(o.lien(&Rubrique::Syntaxe.chemin(langue)))
                        .text(t.lire_syntaxe),
                ),
        );

    let contenu = fragment([
        hero.into(),
        apercu.into(),
        applis.into(),
        ecrire.into(),
        fonctions.into(),
        etapes.into(),
        plateformes.into(),
        architecture.into(),
        appel.into(),
    ] as [Node; 9]);

    layout::page(
        &req,
        langue,
        Some(Rubrique::Accueil),
        "Varium",
        t.description_accueil,
        contenu,
    )
}

// ---------------------------------------------------------------------------------
// Syntaxe
// ---------------------------------------------------------------------------------

/// (filtre, argument, valeur d'exemple) ; l'argument de `default` vient de la langue.
fn filtres_exemples(t: &Textes) -> [(&'static str, Option<&'static str>, &'static str); 12] {
    [
        ("upper", None, "Atelier Lumen"),
        ("lower", None, "ATELIER LUMEN"),
        ("capitalize", None, "marc"),
        ("title", None, "jean-pierre dupont"),
        ("trim", None, "  Varium  "),
        ("number", Some("2"), "3.14159"),
        ("money", None, "12500"),
        ("money", Some("usd"), "1999.5"),
        ("date", None, "2026-03-01"),
        ("date", Some("full"), "2026-03-15"),
        ("date", Some("short"), "2026-03-15"),
        ("default", Some(t.filtre_defaut), ""),
    ]
}

/// Les fonctions du Classeur : (français, anglais), dans l'ordre de
/// [`Textes::classeur_fonctions`].
const FONCTIONS_CLASSEUR: [(&str, &str); 3] =
    [("SOMME", "SUM"), ("SI", "IF"), ("RECHERCHEV", "VLOOKUP")];

/// Les formats d'export qui ont un nom propre ; les trois suivants viennent de
/// [`Textes::exports_noms`].
const FORMATS: [&str; 7] = ["PDF", "DOCX", "ODT", "RTF", "EPUB", "HTML", "LaTeX"];

/// Les ancres du sommaire, dans l'ordre de [`Textes::sommaire`].
const ANCRES: [&str; 7] = [
    "ecriture", "filtres", "types", "formules", "classeur", "exports", "api",
];

/// `GET /{langue}/syntaxe/`
pub async fn syntaxe(req: Request, langue: Langue) -> Page {
    let o = Options::de(&req);
    let t = langue.textes();
    let syntaxe = section()
        .child(h2().id(ANCRES[0]).text(t.ecrire_titre))
        .child(p().text(t.ecrire_texte))
        .child(
            table()
                .child(
                    thead().child(
                        tr().child(th().text(t.syntaxe_colonnes[0]))
                            .child(th().text(t.syntaxe_colonnes[1])),
                    ),
                )
                .child(tbody().children(t.syntaxe.iter().map(|ligne| {
                    tr().child(td().child(code().text(ligne.code)))
                        .child(td().text(ligne.effet))
                }))),
        );

    let filtres = section()
        .child(h2().id(ANCRES[1]).text(t.filtres_titre))
        .child(p().text(t.filtres_texte))
        .child(
            table()
                .child(
                    thead().child(
                        tr().child(th().text(t.filtres_colonnes[0]))
                            .child(th().text(t.filtres_colonnes[1]))
                            .child(th().text(t.filtres_colonnes[2])),
                    ),
                )
                .child(tbody().children(filtres_exemples(t).into_iter().map(
                    |(nom, arg, valeur)| {
                        let ecrit = match arg {
                            Some(a)
                                if a.parse::<f64>().is_ok()
                                    || a.chars().all(|c| c.is_ascii_alphabetic()) =>
                            {
                                format!("{nom}:{a}")
                            }
                            Some(a) => format!("{nom}:\"{a}\""),
                            None => nom.to_string(),
                        };
                        let resultat = filters::apply_one(valeur, nom, arg);
                        tr().child(td().child(code().text(ecrit)))
                            .child(td().child(code().text(format!("{valeur:?}"))))
                            .child(td().child(strong().text(resultat)))
                    },
                ))),
        );

    let types = section()
        .child(h2().id(ANCRES[2]).text(t.types_titre))
        .child(p().text(t.types_texte))
        .child(
            ul().class("pastilles").children(
                VarType::ALL
                    .into_iter()
                    .map(|ty| li().text(libelle_type(t, ty))),
            ),
        );

    let formules = section()
        .child(h2().id(ANCRES[3]).text(t.formules_titre))
        .child(p().text(t.formules_texte))
        .child(code_bloc(t.formules_exemple));

    let classeur = section()
        .child(h2().id(ANCRES[4]).text(t.classeur_titre))
        .child(p().text(t.classeur_texte))
        .child(
            table()
                .class("fonctions-classeur")
                .child(
                    thead().child(
                        tr().child(th().text(t.classeur_colonnes[0]))
                            .child(th().text(t.classeur_colonnes[1])),
                    ),
                )
                .child(
                    tbody().children(FONCTIONS_CLASSEUR.iter().zip(t.classeur_fonctions).map(
                        |((fr, en), role)| {
                            tr().child(
                                td().child(code().text(fr.to_string()))
                                    .child(text(" · "))
                                    .child(code().text(en.to_string())),
                            )
                            .child(td().text(role))
                        },
                    )),
                ),
        )
        .child(p().class("muted").text(t.classeur_note));

    let noms_exports = FORMATS.iter().copied().chain(t.exports_noms);
    // Le français met une espace avant les deux-points, pas les autres langues.
    let deux_points = if langue == Langue::Fr {
        "\u{a0}: "
    } else {
        ": "
    };
    let exports = section()
        .child(h2().id(ANCRES[5]).text(t.exports_titre))
        .child(
            ul().children(noms_exports.zip(t.exports).map(|(nom, effet)| {
                li().child(strong().text(nom))
                    .child(text(format!("{deux_points}{effet}")))
            })),
        );

    // Un hôte statique n'a pas d'API : c'est le serveur du site (`cargo run`) qui la sert.
    let (intro_api, adresse_api) = if o.est_statique() {
        (t.api_intro_statique, "http://localhost:3000")
    } else {
        (t.api_intro_serveur, "https://<ce-site>")
    };
    let api = section()
        .child(h2().id(ANCRES[6]).text(t.api_titre))
        .child(p().text(intro_api))
        .child(code_bloc(&format!(
            "curl -X POST {adresse_api}/api/rendu \\\n  -H 'content-type: application/json' \\\n  -d '{{\"source\": \"Bonjour {{{{prénom | upper}}}}\", \"valeurs\": {{\"prénom\": \"Marc\"}}}}'",
        )))
        .child(p().class("muted").text(t.api_reponse));

    let sommaire = nav().class("sommaire").children(
        ANCRES
            .iter()
            .zip(t.sommaire)
            .map(|(ancre, titre)| a().href(format!("#{ancre}")).text(titre)),
    );

    let contenu = article()
        .class("prose")
        .child(h1().text(Rubrique::Syntaxe.nom(langue)))
        .child(p().class("chapo").text(t.syntaxe_chapo))
        .child(sommaire)
        .child(syntaxe)
        .child(filtres)
        .child(types)
        .child(formules)
        .child(classeur)
        .child(exports)
        .child(api)
        .child(
            div()
                .class("row")
                .child(
                    a().class("button")
                        .href(o.lien_application(langue))
                        .text(t.installer_varium),
                )
                .child(
                    a().class("button ghost")
                        .href(o.lien(&Rubrique::Essayer.chemin(langue)))
                        .text(t.voir_exemple),
                ),
        );

    layout::page(
        &req,
        langue,
        Some(Rubrique::Syntaxe),
        Rubrique::Syntaxe.nom(langue),
        t.syntaxe_description,
        contenu.into(),
    )
}

// ---------------------------------------------------------------------------------
// Installer
// ---------------------------------------------------------------------------------

/// Où télécharger les binaires : la dernière Release GitHub (noms de fichiers stables).
pub const TELECHARGEMENTS: &str = "https://github.com/maxlestage/VARIUM/releases/latest/download/";

/// (système, fichier), dans l'ordre de [`Textes::systemes`].
const SYSTEMES: [(&str, &str); 3] = [
    ("Windows", "varium-windows-x86_64.zip"),
    ("macOS", "varium-macos.zip"),
    ("Linux", "varium-linux-x86_64.tar.gz"),
];

/// `GET /{langue}/installer/` : télécharger l'application de bureau.
pub async fn installer(req: Request, langue: Langue) -> Page {
    let o = Options::de(&req);
    let t = langue.textes();
    let cartes =
        div()
            .class("grid telechargements")
            .children(
                SYSTEMES
                    .iter()
                    .zip(&t.systemes)
                    .map(|((nom, fichier), systeme)| {
                        article()
                            .class("card")
                            .id(nom.to_lowercase())
                            .child(h2().text(nom.to_string()))
                            .child(p().class("muted petit").text(systeme.compat))
                            .child(
                                a().class("button")
                                    .href(format!("{TELECHARGEMENTS}{fichier}"))
                                    .text(remplir(t.telecharger_pour, &[nom])),
                            )
                            .child(
                                p().class("muted petit")
                                    .child(code().text(fichier.to_string())),
                            )
                            .child(ol().children(systeme.etapes.map(|e| li().text(e))))
                    }),
            );
    let contenu = article()
        .class("prose large")
        .child(h1().text(t.installer_varium))
        .child(p().class("chapo").text(t.installer_chapo))
        .child(p().class("muted").text(t.langues_note))
        .child(cartes)
        .child(p().class("muted").children(a_trou(
            t.versions,
            a().href("https://github.com/maxlestage/VARIUM/releases")
                .text(t.versions_lien),
        )))
        .child(
            section()
                .child(h2().text(t.source_titre))
                .child(p().text(t.source_texte))
                .child(code_bloc(
                    "cargo install --git https://github.com/maxlestage/VARIUM varium --locked\nvarium",
                ))
                .child(p().class("muted").text(t.source_linux)),
        )
        .child(
            div()
                .class("row")
                .child(a().class("button ghost").href(REPO).text(t.source_github))
                .child(
                    a().class("button ghost")
                        .href(o.lien(&Rubrique::Syntaxe.chemin(langue)))
                        .text(t.lire_syntaxe),
                ),
        );
    layout::page(
        &req,
        langue,
        Some(Rubrique::Installer),
        Rubrique::Installer.nom(langue),
        t.installer_description,
        contenu.into(),
    )
}

// ---------------------------------------------------------------------------------
// Introuvable
// ---------------------------------------------------------------------------------

/// La page introuvable de `langue`.
fn introuvable(req: &Request, langue: Langue) -> Page {
    let o = Options::de(req);
    let t = langue.textes();
    let contenu = section()
        .class("hero")
        .child(h1().text(t.introuvable_h1))
        .child(p().class("muted").text(t.introuvable_texte))
        .child(
            div().class("row centre").child(
                a().class("button")
                    .href(o.lien(&Rubrique::Accueil.chemin(langue)))
                    .text(t.retour_accueil),
            ),
        );
    layout::page(
        req,
        langue,
        None,
        t.introuvable_titre,
        t.introuvable_description,
        contenu.into(),
    )
    .status(404)
}

/// La page introuvable sans langue (une adresse hors de `/{langue}/`, et le `404.html`
/// de l'export, que GitHub Pages sert pour toute adresse inconnue) : le message et le
/// chemin de l'accueil dans chaque langue.
fn introuvable_multilingue(req: &Request) -> Page {
    let o = Options::de(req);
    let fr = Langue::Fr.textes();
    let contenu = section()
        .class("hero aiguillage")
        .child(h1().text(fr.introuvable_h1))
        .child(p().class("muted").text(fr.introuvable_texte))
        .child(ul().class("langues-liste").children(Langue::ALL.map(|l| {
            let t = l.textes();
            li().child(
                a().href(o.lien(&Rubrique::Accueil.chemin(l)))
                    .attr("hreflang", l.code())
                    .attr("lang", l.code())
                    .child(strong().text(t.introuvable_titre))
                    .child(span().class("muted").text(t.retour_accueil)),
            )
        })));
    layout::page_neutre(
        req,
        &format!("{} · Varium", fr.introuvable_titre),
        fr.introuvable_description,
        contenu.into(),
    )
    .seo(Seo::new().noindex(true))
    .status(404)
}

/// Toute route inconnue : l'introuvable de la langue de l'adresse (`/en/…`), sinon
/// celui de toutes les langues.
pub async fn introuvable_partout(req: Request) -> Page {
    match langue_du_chemin(req.path()) {
        Some(langue) => introuvable(&req, langue),
        None => introuvable_multilingue(&req),
    }
}

// ---------------------------------------------------------------------------------
// `/` et anciennes adresses
// ---------------------------------------------------------------------------------

/// Une chaîne JavaScript qui ne peut pas fermer l'élément `<script>`.
fn chaine_js(s: &str) -> String {
    let mut sortie = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => sortie.push_str("\\\""),
            '\\' => sortie.push_str("\\\\"),
            '<' => sortie.push_str("\\u003c"),
            '\n' => sortie.push_str("\\n"),
            c => sortie.push(c),
        }
    }
    sortie.push('"');
    sortie
}

/// Le choix de la langue dans le navigateur, pour la racine de l'export : la première
/// des langues du navigateur (`navigator.languages`) que le site parle, le français
/// sinon. Sans JavaScript, `<meta http-equiv="refresh">` mène au français.
fn redirection_langue(o: &Options) -> String {
    let codes: Vec<String> = Langue::ALL.iter().map(|l| chaine_js(l.code())).collect();
    format!(
        concat!(
            "<script>(function(){{var L=[{codes}],c=null,",
            "n=navigator.languages||[navigator.language||\"\"];",
            "for(var i=0;i<n.length&&!c;i++){{var x=String(n[i]).slice(0,2).toLowerCase();",
            "if(L.indexOf(x)>=0)c=x}}",
            "location.replace({base}+\"/\"+(c||\"fr\")+\"/\"+location.search+location.hash)}})()</script>",
            r#"<noscript><meta http-equiv="refresh" content="0; url={francais}"/></noscript>"#,
        ),
        codes = codes.join(","),
        base = chaine_js(o.chemin_base()),
        francais = o.lien(&Rubrique::Accueil.chemin(Langue::Fr)),
    )
}

/// La page qui mène à `rubrique` dans une langue, avec un lien vers chacune : la
/// racine de l'export (`racine`, qui choisit la langue du navigateur) ou une ancienne
/// adresse (`/syntaxe/`…, qui mène à la version française).
fn aiguillage(req: &Request, rubrique: Rubrique, racine: bool) -> Page {
    let o = Options::de(req);
    let fr = Langue::Fr.textes();
    let choisir = Langue::ALL.iter().enumerate().flat_map(|(i, l)| {
        let separateur = (i > 0).then(|| text(" · "));
        [
            separateur,
            Some(
                span()
                    .attr("lang", l.code())
                    .text(l.textes().langue_choisir)
                    .into(),
            ),
        ]
        .into_iter()
        .flatten()
    });
    let contenu = section()
        .class("hero aiguillage")
        .child(h1().text("Varium"))
        .child(p().class("muted").children(choisir))
        .child(ul().class("langues-liste").children(Langue::ALL.map(|l| {
            let sous_titre = match rubrique {
                Rubrique::Accueil => l.textes().accroche,
                r => r.nom(l),
            };
            li().child(
                a().href(o.lien(&rubrique.chemin(l)))
                    .attr("hreflang", l.code())
                    .attr("lang", l.code())
                    .child(strong().text(l.nom()))
                    .child(span().class("muted").text(sous_titre)),
            )
        })));
    let titre = match rubrique {
        Rubrique::Accueil => format!("Varium · {}", fr.accroche),
        r => format!("{} · Varium", r.nom(Langue::Fr)),
    };
    let page = layout::page_neutre(
        req,
        &titre,
        description(rubrique, Langue::Fr, o.est_statique()),
        contenu.into(),
    );
    if racine {
        let mut seo = Seo::new().canonical("/");
        for l in Langue::ALL {
            seo = seo.alternate(l.code(), Rubrique::Accueil.chemin(l));
        }
        page.seo(seo.alternate("x-default", "/"))
            .head(redirection_langue(o))
    } else {
        let francais = rubrique.chemin(Langue::Fr);
        page.seo(Seo::new().canonical(francais.clone()))
            .head(format!(
                r#"<meta http-equiv="refresh" content="0; url={}"/>"#,
                o.lien(&francais)
            ))
    }
}

/// `GET /` : l'accueil dans la langue du navigateur (`Accept-Language`), le français
/// sinon. Dans l'export statique, une page qui fait ce choix dans le navigateur.
pub async fn racine(req: Request) -> Response {
    let o = Options::de(&req);
    if o.est_statique() {
        return aiguillage(&req, Rubrique::Accueil, true).into_response();
    }
    let langue = langue_preferee(req.header("accept-language").unwrap_or(""));
    Redirect::to(o.lien(&Rubrique::Accueil.chemin(langue)))
        .into_response()
        .header("vary", "Accept-Language")
        .header("cache-control", "no-cache")
}

/// `GET /syntaxe/`, `/essayer/`, `/installer/` : les adresses d'avant les langues, qui
/// étaient en français. Le serveur redirige pour de bon vers `/fr/…` ; l'export écrit
/// une page qui y mène.
pub async fn ancienne(req: Request, rubrique: Rubrique) -> Response {
    let o = Options::de(&req);
    if o.est_statique() {
        return aiguillage(&req, rubrique, false).into_response();
    }
    let requete = req
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    Redirect::permanent(format!("{}{requete}", o.lien(&rubrique.chemin(Langue::Fr))))
        .into_response()
}

// ---------------------------------------------------------------------------------
// robots.txt, sitemap.xml
// ---------------------------------------------------------------------------------

/// `GET /robots.txt`
pub async fn robots(req: Request) -> Response {
    let corps = format!(
        "User-agent: *\nAllow: /\nSitemap: {}/sitemap.xml\n",
        Options::de(&req).origine(&req)
    );
    res::text(corps)
}

/// `GET /sitemap.xml` : chaque page dans chaque langue, avec ses versions (`hreflang`).
pub async fn sitemap(req: Request) -> Response {
    let origine = Options::de(&req).origine(&req);
    let mut urls = String::new();
    for rubrique in Rubrique::ALL {
        for langue in Langue::ALL {
            urls.push_str(&format!(
                "<url><loc>{origine}{}</loc>",
                rubrique.chemin(langue)
            ));
            for l in Langue::ALL {
                urls.push_str(&format!(
                    r#"<xhtml:link rel="alternate" hreflang="{}" href="{origine}{}"/>"#,
                    l.code(),
                    rubrique.chemin(l)
                ));
            }
            urls.push_str(&format!(
                r#"<xhtml:link rel="alternate" hreflang="x-default" href="{origine}{}"/></url>"#,
                rubrique.chemin_par_defaut()
            ));
            urls.push('\n');
        }
    }
    let corps = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\" xmlns:xhtml=\"http://www.w3.org/1999/xhtml\">\n{urls}</urlset>\n"
    );
    res::text(corps).header("content-type", "application/xml; charset=utf-8")
}
