//! Les pages : accueil, syntaxe, 404, et les fichiers pour les moteurs de recherche.

use paquets::prelude::*;
use varium_core::{Values, VarType, filters};

use crate::demo::{document, source_coloree};
use crate::{REPO, layout};

/// Les pages indexées (pour `sitemap.xml`).
const PAGES: [&str; 3] = ["/", "/syntaxe", "/essayer"];

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

// ---------------------------------------------------------------------------------
// Accueil
// ---------------------------------------------------------------------------------

const AVANT: &str = "Bonjour {{prénom}} {{nom | upper}},\n\nVotre dossier {{référence}} d'un montant de {{montant | money}}\nsera clôturé le {{échéance | date:full}}.\n\nÀ bientôt {{prénom}} !";

fn valeurs_accueil() -> Values {
    let mut v = Values::default();
    v.set("prénom", "Camille")
        .set("nom", "Durand")
        .set("référence", "REF-2026-042")
        .set("montant", "12500")
        .set("échéance", "2026-12-18");
    v
}

const FONCTIONS: [(&str, &str, &str); 8] = [
    (
        "🔁",
        "Variables globales",
        "Une valeur saisie une fois met à jour toutes ses occurrences. {{prénom}} et {{nom}} restent toujours distincts, même collés.",
    ),
    (
        "🏷️",
        "Variables typées",
        "Texte, texte long, date avec calendrier, nombre, montant, e-mail : chaque saisie est validée et mise au format français.",
    ),
    (
        "🧪",
        "Filtres",
        "upper, capitalize, money:usd, date:full, number:2, default:\"…\" : des filtres chaînables pour chaque occurrence.",
    ),
    (
        "✏️",
        "Renommer partout",
        "Renommez une variable : seules ses occurrences changent, et elles gardent leurs filtres.",
    ),
    (
        "🧊",
        "Figer",
        "Remplacez une variable par sa valeur directement dans la source, quand le document est définitif.",
    ),
    (
        "∑",
        "Formules LaTeX",
        "$…$ en ligne, $$…$$ centrées. Un $ qui vient d'une valeur (10,00 $US) n'ouvre jamais de formule.",
    ),
    (
        "📤",
        "Exports",
        "HTML avec KaTeX (imprimable en PDF), LaTeX complet, texte, modèle brut, projet .varium.json.",
    ),
    (
        "💾",
        "Sauvegarde automatique",
        "Fichier local sur le bureau, stockage du navigateur sur le web : rien ne se perd.",
    ),
];

/// `GET /`
pub async fn accueil(req: Request) -> Page {
    let valeurs = valeurs_accueil();

    let hero = section()
        .class("hero")
        .child(p().class("surtitre").text("Éditeur de documents à variables · Rust"))
        .child(h1().text("Un document. Toutes ses possibilités."))
        .child(p().class("chapo").text(
            "Écrivez {{prénom}}, {{nom}} ou {{montant}} dans votre document, changez une valeur une seule fois : toutes ses occurrences se mettent à jour, sans toucher aux autres champs.",
        ))
        .child(
            div()
                .class("row centre")
                .child(a().class("button").href("/essayer").text("Essayer en ligne"))
                .child(a().class("button ghost").href(REPO).text("Voir le code source")),
        )
        .child(p().class("muted petit").text("Bureau (Linux, macOS, Windows) et navigateur (WebAssembly) · 100 % Rust"));

    let apercu = section()
        .class("apercu")
        .child(
            div()
                .class("card")
                .child(p().class("etiquette").text("Le modèle"))
                .child(source_coloree(AVANT)),
        )
        .child(div().class("fleche").attr("aria-hidden", "true").text("→"))
        .child(
            div()
                .class("card")
                .child(p().class("etiquette").text("Le document"))
                .child(document(AVANT, &valeurs)),
        );

    let fonctions = section()
        .child(h2().class("center").text("Tout ce qu'il faut pour vos documents répétitifs"))
        .child(p().class("center muted").text("Courriers, devis, contrats, attestations : un seul modèle, autant de documents que nécessaire."))
        .child(
            div()
                .class("grid")
                .children(FONCTIONS.iter().map(|(i, t, d)| carte(i, t, d))),
        );

    let etapes = section()
        .child(h2().class("center").text("Comment ça marche"))
        .child(
            ol().class("etapes")
                .child(li().child(h3().text("Écrivez")).child(p().class("muted").text(
                    "Rédigez votre texte et placez des {{variables}} là où il change d'un document à l'autre.",
                )))
                .child(li().child(h3().text("Remplissez")).child(p().class("muted").text(
                    "Varium détecte chaque variable et vous propose un champ, avec le bon type : date, montant, e-mail…",
                )))
                .child(li().child(h3().text("Exportez")).child(p().class("muted").text(
                    "Le document final, formules comprises, en HTML, en PDF, en LaTeX ou en texte.",
                ))),
        );

    let plateformes = section()
        .class("deux")
        .child(
            div()
                .class("card")
                .child(h3().text("🖥️ Sur le bureau"))
                .child(p().class("muted").text("Une application native rapide, construite avec egui, pour Linux, macOS et Windows."))
                .child(code_bloc("cargo run -p varium --release")),
        )
        .child(
            div()
                .class("card")
                .child(h3().text("🌐 Dans le navigateur"))
                .child(p().class("muted").text("La même application, compilée en WebAssembly, sans rien installer."))
                .child(code_bloc(
                    "rustup target add wasm32-unknown-unknown\ncargo install trunk --locked\ncd crates/varium && trunk serve --release",
                )),
        );

    let architecture = section()
        .child(h2().class("center").text("Sous le capot"))
        .child(p().class("center muted").text(
            "Un workspace Cargo en deux paquets : un moteur pur, sans interface, et l'application qui l'utilise.",
        ))
        .child(
            table()
                .child(thead().child(tr().child(th().text("Paquet")).child(th().text("Rôle"))))
                .child(
                    tbody()
                        .child(tr().child(td().child(code().text("varium-core"))).child(td().text(
                            "Le moteur : analyse, rendu, filtres, formules, exports, format de projet. Testé, sans dépendance graphique.",
                        )))
                        .child(tr().child(td().child(code().text("varium"))).child(td().text(
                            "L'application egui/eframe : binaire de bureau et site WebAssembly (Trunk).",
                        ))),
                ),
        )
        .child(code_bloc(
            "use varium_core::{Values, Missing, render};\n\nlet mut v = Values::default();\nv.set(\"prénom\", \"Marc\");\nlet doc = render(\"Bonjour {{prénom}} {{nom}}\", &v, Missing::Keep);\nassert_eq!(doc, \"Bonjour Marc {{nom}}\");",
        ));

    let appel = section()
        .class("hero appel")
        .child(h2().text("Prêt à essayer ?"))
        .child(p().class("muted").text("La démo fait tourner le vrai moteur de Varium. Modifiez le courrier d'exemple, ajoutez vos propres variables."))
        .child(
            div()
                .class("row centre")
                .child(a().class("button").href("/essayer").text("Ouvrir la démo"))
                .child(a().class("button ghost").href("/syntaxe").text("Lire la syntaxe")),
        );

    let contenu = fragment([
        hero.into(),
        apercu.into(),
        fonctions.into(),
        etapes.into(),
        plateformes.into(),
        architecture.into(),
        appel.into(),
    ] as [Node; 7]);

    layout::page(
        &req,
        "Varium",
        "Varium est un éditeur de documents à variables écrit en Rust : écrivez {{prénom}} une fois, changez sa valeur, tout le document suit. Bureau et navigateur.",
        "/",
        contenu,
    )
}

// ---------------------------------------------------------------------------------
// Syntaxe
// ---------------------------------------------------------------------------------

const SYNTAXE: [(&str, &str); 9] = [
    ("{{nom}}", "une variable"),
    (
        "{{ nom | upper | trim }}",
        "une variable et des filtres chaînés",
    ),
    (
        "{{montant | money}}",
        "un montant en euros (money:usd, money:gbp, money:chf…)",
    ),
    (
        "{{date | date}}",
        "une date longue : 15 mars 2026 (date:full, date:short)",
    ),
    ("{{x | number:2}}", "un nombre à deux décimales : 3,14"),
    (
        "{{t | default:\"texte\"}}",
        "une valeur par défaut si le champ est vide",
    ),
    ("\\{{nom}}", "des accolades littérales, sans variable"),
    (
        "$E = mc^2$, $$ \\sum_i x_i $$",
        "des formules LaTeX, en ligne ou centrées",
    ),
    ("\\$", "un dollar littéral"),
];

/// (filtre, argument, valeur d'exemple)
const FILTRES: [(&str, Option<&str>, &str); 12] = [
    ("upper", None, "Atelier Lumen"),
    ("lower", None, "ATELIER LUMEN"),
    ("capitalize", None, "marc"),
    ("title", None, "jean-pierre dupont"),
    ("trim", None, "  espaces  "),
    ("number", Some("2"), "3.14159"),
    ("money", None, "12500"),
    ("money", Some("usd"), "1999.5"),
    ("date", None, "2026-03-01"),
    ("date", Some("full"), "2026-03-15"),
    ("date", Some("short"), "2026-03-15"),
    ("default", Some("Madame, Monsieur"), ""),
];

/// `GET /syntaxe`
pub async fn syntaxe(req: Request) -> Page {
    let syntaxe = section()
        .child(h2().id("ecriture").text("Écrire des variables"))
        .child(p().text(
            "Une variable s'écrit entre doubles accolades. Les noms acceptent les accents, les chiffres, _, . et - : {{prénom}}, {{client.ville}}, {{date-1}}.",
        ))
        .child(
            table()
                .child(thead().child(tr().child(th().text("Écriture")).child(th().text("Effet"))))
                .child(tbody().children(SYNTAXE.iter().map(|(e, d)| {
                    tr().child(td().child(code().text(e.to_string()))).child(td().text(d.to_string()))
                }))),
        );

    let filtres = section()
        .child(h2().id("filtres").text("Filtres"))
        .child(p().text(
            "Les filtres transforment la valeur d'une occurrence, et seulement elle. Ils s'enchaînent de gauche à droite : {{nom | trim | upper}}. Les résultats ci-dessous sont calculés par varium-core à l'affichage de la page.",
        ))
        .child(
            table()
                .child(thead().child(
                    tr().child(th().text("Filtre"))
                        .child(th().text("Valeur"))
                        .child(th().text("Résultat")),
                ))
                .child(tbody().children(FILTRES.iter().map(|(nom, arg, valeur)| {
                    let ecrit = match arg {
                        Some(a) if a.parse::<f64>().is_ok() || a.chars().all(|c| c.is_ascii_alphabetic()) => format!("{nom}:{a}"),
                        Some(a) => format!("{nom}:\"{a}\""),
                        None => nom.to_string(),
                    };
                    let resultat = filters::apply_one(valeur, nom, *arg);
                    tr().child(td().child(code().text(ecrit)))
                        .child(td().child(code().text(format!("{valeur:?}"))))
                        .child(td().child(strong().text(resultat)))
                }))),
        );

    let types = section()
        .child(h2().id("types").text("Types de variables"))
        .child(p().text(
            "Chaque variable a un type, qui choisit son champ de saisie, vérifie la valeur et donne un format par défaut quand l'occurrence n'a pas de filtre : une date s'affiche « 1er mars 2026 », un montant « 12 500,00 € ».",
        ))
        .child(
            ul().class("pastilles")
                .children(VarType::ALL.into_iter().map(|t| li().text(t.label()))),
        );

    let formules = section()
        .child(h2().id("formules").text("Formules"))
        .child(p().text(
            "Entre $…$ (en ligne) ou $$…$$ (centrée), le texte est du LaTeX, et les variables y fonctionnent aussi. Seuls les $ écrits dans la source ouvrent une formule : une valeur comme « 10,00 $US » ne casse jamais le document.",
        ))
        .child(code_bloc("La mensualité vaut $m = \\frac{M}{n}$, avec\n$$ C_n = C_0 \\times (1 + t)^n $$"));

    let exports = section().child(h2().id("exports").text("Exports")).child(
        ul().child(li().child(strong().text("HTML")).child(text(
            " : un fichier autonome, formules composées par KaTeX, prêt à imprimer en PDF.",
        )))
        .child(
            li().child(strong().text("LaTeX"))
                .child(text(" : un .tex complet, à compiler tel quel.")),
        )
        .child(
            li().child(strong().text("Texte"))
                .child(text(" : le document final, sans mise en forme.")),
        )
        .child(
            li().child(strong().text("Modèle"))
                .child(text(" : la source brute, avec ses {{variables}}.")),
        )
        .child(li().child(strong().text("Projet")).child(text(
            " : un fichier .varium.json avec la source, les valeurs et les types.",
        ))),
    );

    let api = section()
        .child(h2().id("api").text("API de rendu"))
        .child(p().text("Ce site expose le moteur en JSON, pour l'essayer depuis un terminal :"))
        .child(code_bloc(
            "curl -X POST https://<ce-site>/api/rendu \\\n  -H 'content-type: application/json' \\\n  -d '{\"source\": \"Bonjour {{prénom | upper}}\", \"valeurs\": {\"prénom\": \"Marc\"}}'",
        ))
        .child(p().class("muted").text(
            "La réponse contient le texte rendu (texte), l'export HTML complet (html) et les variables détectées avec leur nombre d'occurrences (variables).",
        ));

    let sommaire = nav().class("sommaire").children(
        [
            ("#ecriture", "Variables"),
            ("#filtres", "Filtres"),
            ("#types", "Types"),
            ("#formules", "Formules"),
            ("#exports", "Exports"),
            ("#api", "API"),
        ]
        .into_iter()
        .map(|(h, t)| a().href(h).text(t)),
    );

    let contenu = article()
        .class("prose")
        .child(h1().text("Syntaxe"))
        .child(
            p().class("chapo")
                .text("Tout ce que l'on peut écrire dans un document Varium."),
        )
        .child(sommaire)
        .child(syntaxe)
        .child(filtres)
        .child(types)
        .child(formules)
        .child(exports)
        .child(api)
        .child(
            div().class("row").child(
                a().class("button")
                    .href("/essayer")
                    .text("Essayer dans la démo"),
            ),
        );

    layout::page(
        &req,
        "Syntaxe",
        "La syntaxe de Varium : variables {{…}}, filtres, types, formules LaTeX et exports.",
        "/syntaxe",
        contenu.into(),
    )
}

// ---------------------------------------------------------------------------------
// 404, robots.txt, sitemap.xml
// ---------------------------------------------------------------------------------

/// Toute route inconnue.
pub async fn introuvable(req: Request) -> Page {
    let contenu = section()
        .class("hero")
        .child(h1().text("{{page}} introuvable"))
        .child(
            p().class("muted")
                .text("Cette variable n'a pas de valeur : la page demandée n'existe pas."),
        )
        .child(
            div()
                .class("row centre")
                .child(a().class("button").href("/").text("Retour à l'accueil")),
        );
    layout::page(
        &req,
        "Page introuvable",
        "Cette page n'existe pas.",
        "/404",
        contenu.into(),
    )
    .seo(Seo::new().noindex(true))
    .status(404)
}

/// `GET /robots.txt`
pub async fn robots(req: Request) -> Response {
    let corps = format!(
        "User-agent: *\nAllow: /\nSitemap: {}/sitemap.xml\n",
        layout::origine(&req)
    );
    res::text(corps)
}

/// `GET /sitemap.xml`
pub async fn sitemap(req: Request) -> Response {
    let origine = layout::origine(&req);
    let urls: String = PAGES
        .iter()
        .map(|p| format!("<url><loc>{origine}{p}</loc></url>"))
        .collect();
    let corps = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{urls}</urlset>"#
    );
    res::text(corps).header("content-type", "application/xml; charset=utf-8")
}
