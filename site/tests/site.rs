//! Le site entier, testé avec le client en mémoire de Paquets (`app().test()`).

use std::path::{Path, PathBuf};

use paquets::prelude::*;
use varium_site::{Langue, Options, Rubrique, app, app_with};

/// `texte` tel que les pages l'écrivent dans le HTML.
fn html(texte: &str) -> String {
    texte
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Le serveur sans variables d'environnement (pas de base, pas d'`APP_URL`).
async fn serveur() -> paquets::test::TestClient {
    app_with(Options::new()).test().await
}

#[tokio::test]
async fn chaque_page_repond_dans_chaque_langue() {
    let client = serveur().await;
    for langue in Langue::ALL {
        let t = langue.textes();
        let code = langue.code();
        for rubrique in Rubrique::ALL {
            let chemin = rubrique.chemin(langue);
            let res = client.get(&chemin).await;
            assert_eq!(res.status(), 200, "{chemin}");
            assert!(res.header("content-security-policy").is_some(), "{chemin}");
            let html = res.text();
            let titre = match rubrique {
                Rubrique::Accueil => format!("<title>Varium · {}</title>", t.accroche),
                r => format!("<title>{} · Varium</title>", r.nom(langue)),
            };
            assert!(html.contains(&titre), "{chemin} : {titre}");
            assert!(
                html.contains(&format!(r#"<html lang="{code}""#)),
                "{chemin}"
            );
            assert!(
                html.contains(&format!(
                    r#"<link rel="canonical" href="http://localhost:3000{chemin}"/>"#
                )),
                "{chemin}"
            );
            assert!(
                html.contains(&format!(
                    r#"<meta property="og:locale" content="{}"/>"#,
                    langue.locale()
                )),
                "{chemin}"
            );
            // Une version par langue, et x-default.
            for l in Langue::ALL {
                let alternative = format!(
                    r#"<link rel="alternate" hreflang="{}" href="http://localhost:3000{}"/>"#,
                    l.code(),
                    rubrique.chemin(l)
                );
                assert!(html.contains(&alternative), "{chemin} : {alternative}");
            }
            assert!(
                html.contains(&format!(
                    r#"<link rel="alternate" hreflang="x-default" href="http://localhost:3000{}"/>"#,
                    rubrique.chemin_par_defaut()
                )),
                "{chemin}"
            );
            // Le sélecteur garde la page et nomme chaque langue dans sa langue.
            for l in Langue::ALL {
                let lien = format!(
                    r#"href="{}" hreflang="{}" lang="{}""#,
                    rubrique.chemin(l),
                    l.code(),
                    l.code()
                );
                assert!(html.contains(&lien), "{chemin} : {lien}");
                assert!(html.contains(&format!(">{}</a>", l.nom())), "{chemin}");
            }
            // La navigation, dans la langue de la page.
            for r in Rubrique::ALL {
                assert!(
                    html.contains(&format!(r#"href="{}""#, r.chemin(langue))),
                    "{chemin}"
                );
                assert!(html.contains(&self::html(r.nom(langue))), "{chemin}");
            }
            assert!(html.contains(&self::html(t.pied)), "{chemin}");
        }
    }
}

#[tokio::test]
async fn les_textes_sont_traduits() {
    let client = serveur().await;
    for (chemin, attendus) in [
        (
            "/en/",
            &[
                "Three applications, one Varium",
                "Write in six languages",
                "Proofreading",
            ][..],
        ),
        (
            "/de/",
            &[
                "Drei Anwendungen, ein Varium",
                "Tabellenkalkulation",
                "Präsentationen",
            ][..],
        ),
        (
            "/es/syntaxe/",
            &[
                "Escribir variables",
                "Fórmulas de la hoja de cálculo (Classeur)",
            ][..],
        ),
        (
            "/it/installer/",
            &["Scarica per Windows", "Esegui comunque"][..],
        ),
        (
            "/pt/essayer/",
            &["Experimentar o Varium", "Atualizar o documento"][..],
        ),
        (
            "/fr/",
            &[
                "Trois applications, un seul Varium",
                "Écrire en six langues",
            ][..],
        ),
    ] {
        let html = client.get(chemin).await.text();
        for attendu in attendus {
            assert!(html.contains(attendu), "{chemin} : {attendu}");
        }
    }
}

#[tokio::test]
async fn l_accueil_presente_les_trois_applications() {
    let client = serveur().await;
    for langue in Langue::ALL {
        let html = client.get(&Rubrique::Accueil.chemin(langue)).await.text();
        for nom in ["Documents", "Classeur", "Présentation"] {
            assert!(
                html.contains(&format!(">{nom}</h3>")),
                "{} : {nom}",
                langue.code()
            );
        }
        for format in ["XLSX", "ODS", "CSV", "ODP", "VLOOKUP", "RECHERCHEV"] {
            assert!(html.contains(format), "{} : {format}", langue.code());
        }
        // Les six langues d'écriture, chacune dans sa langue.
        for l in Langue::ALL {
            assert!(
                html.contains(&format!(r#"<li lang="{}">{}</li>"#, l.code(), l.nom())),
                "{}",
                langue.code()
            );
        }
    }
}

#[tokio::test]
async fn l_accueil_montre_un_vrai_rendu() {
    let client = serveur().await;
    for chemin in ["/fr/", "/en/", "/de/"] {
        let html = client.get(chemin).await.text();
        // Calculé par varium-core : filtre upper, montant et date au format français.
        assert!(html.contains("DURAND"), "{chemin}");
        assert!(html.contains("12\u{202f}500,00\u{a0}€"), "{chemin}");
        assert!(html.contains("vendredi 18 décembre 2026"), "{chemin}");
    }
    let html = client.get("/en/").await.text();
    assert!(html.contains("The template") && html.contains("The document"));
}

#[tokio::test]
async fn la_syntaxe_calcule_les_filtres() {
    let client = serveur().await;
    let html = client.get("/fr/syntaxe/").await.text();
    assert!(html.contains("<strong>1er mars 2026</strong>"));
    assert!(html.contains("<strong>Madame, Monsieur</strong>"));
    assert!(html.contains(r##"<a href="#classeur">Classeur</a>"##));
    let html = client.get("/de/syntaxe/").await.text();
    assert!(html.contains("<strong>Sehr geehrte Damen und Herren</strong>"));
    assert!(html.contains("<strong>1er mars 2026</strong>"));
    assert!(html.contains("<code>VLOOKUP</code>"));
}

#[tokio::test]
async fn la_racine_suit_la_langue_du_navigateur() {
    let client = serveur().await;
    for (entete, cible) in [
        (Some("de-DE,de;q=0.9,en;q=0.8"), "/de/"),
        (Some("pt-BR, en;q=0.5"), "/pt/"),
        (Some("ja, it;q=0.4"), "/it/"),
        (Some("ja, zh"), "/fr/"),
        (None, "/fr/"),
    ] {
        let mut requete = client.get("/");
        if let Some(entete) = entete {
            requete = requete.header("accept-language", entete);
        }
        let res = requete.await;
        assert_eq!(res.status(), 302, "{entete:?}");
        assert_eq!(res.header("location"), Some(cible), "{entete:?}");
        assert_eq!(res.header("vary"), Some("Accept-Language"));
    }
}

#[tokio::test]
async fn les_anciennes_adresses_menent_au_francais() {
    let client = serveur().await;
    for (ancienne, nouvelle) in [
        ("/syntaxe/", "/fr/syntaxe/"),
        ("/syntaxe", "/fr/syntaxe/"),
        ("/essayer/", "/fr/essayer/"),
        ("/installer/", "/fr/installer/"),
        ("/syntaxe/?x=1", "/fr/syntaxe/?x=1"),
    ] {
        let res = client.get(ancienne).await;
        assert_eq!(res.status(), 301, "{ancienne}");
        assert_eq!(res.header("location"), Some(nouvelle), "{ancienne}");
    }
    // Le formulaire d'avant les langues fonctionne toujours, en français.
    let res = client
        .post("/essayer/")
        .form(&[("source", "{{x}}"), ("val.x", "ok")])
        .await;
    assert_eq!(res.status(), 200);
    let html = res.text();
    assert!(html.contains(">ok</mark>"));
    assert!(html.contains(r#"<html lang="fr""#));
}

#[tokio::test]
async fn la_demo_rend_le_formulaire_envoye_dans_chaque_langue() {
    let client = serveur().await;
    for langue in Langue::ALL {
        let t = langue.textes();
        let chemin = Rubrique::Essayer.chemin(langue);
        let res = client
            .post(&chemin)
            .form(&[
                ("source", "Bonjour {{prénom | upper}} {{nom}} <b> {{date}}"),
                ("val.prénom", "Léa"),
                ("type.prénom", "text"),
                ("val.date", "demain"),
                ("type.date", "date"),
            ])
            .await;
        assert_eq!(res.status(), 200, "{chemin}");
        let html = res.text();
        assert!(html.contains(">LÉA</mark>"), "{chemin}");
        assert!(html.contains(r#"class="manquante""#), "{chemin}"); // {{nom}} n'a pas de valeur
        assert!(html.contains(r#"name="val.nom""#), "{chemin}"); // un champ par variable détectée
        assert!(!html.contains("<b>"), "{chemin}"); // le texte de l'utilisateur est échappé
        // Le formulaire revient à la page de la même langue, dans cette langue.
        assert!(
            html.contains(&format!(
                r#"<form class="stack" method="post" action="{chemin}">"#
            )),
            "{chemin}"
        );
        assert!(html.contains(&self::html(t.mettre_a_jour)), "{chemin}");
        // « demain » n'est pas une date.
        assert!(html.contains(&self::html(t.erreurs[0])), "{chemin}");
        assert!(
            html.contains(&format!(r#"<html lang="{}""#, langue.code())),
            "{chemin}"
        );
    }
}

#[tokio::test]
async fn les_compteurs_s_accordent() {
    let client = serveur().await;
    let poster = |chemin: &'static str| {
        let client = &client;
        async move {
            client
                .post(chemin)
                .form(&[("source", "{{a}} {{a}} {{b}}"), ("val.a", "1")])
                .await
                .text()
        }
    };
    assert!(
        poster("/fr/essayer/")
            .await
            .contains("2 variables · 3 occurrences · 1 à remplir")
    );
    assert!(
        poster("/en/essayer/")
            .await
            .contains("2 variables · 3 occurrences · 1 to fill in")
    );
    assert!(
        poster("/de/essayer/")
            .await
            .contains("2 Variablen · 3 Vorkommen · 1 ohne Wert")
    );
    let html = client
        .post("/en/essayer/")
        .form(&[("source", "{{a}}")])
        .await
        .text();
    assert!(html.contains("1 variable · 1 occurrence · 1 to fill in"));
}

#[tokio::test]
async fn la_demo_refuse_les_autres_sites() {
    let client = serveur().await;
    for chemin in ["/essayer", "/en/essayer/"] {
        let res = client
            .post(chemin)
            .header("sec-fetch-site", "cross-site")
            .form(&[("source", "x")])
            .await;
        assert_eq!(res.status(), 403, "{chemin}");
    }
}

#[tokio::test]
async fn l_api_rend_du_json() {
    let res = app()
        .test()
        .await
        .post("/api/rendu")
        .json(&json!({
            "source": "{{montant}} pour {{nom}}, {{nom}}",
            "valeurs": { "montant": "1999.5" },
            "types": { "montant": "money" }
        }))
        .await;
    assert_eq!(res.status(), 200);
    let v: paquets::serde_json::Value = res.json();
    assert_eq!(v["texte"], "1\u{202f}999,50\u{a0}€ pour {{nom}}, {{nom}}");
    assert_eq!(v["variables"][1]["nom"], "nom");
    assert_eq!(v["variables"][1]["occurrences"], 2);
    assert_eq!(v["variables"][1]["renseignee"], false);
    assert!(v["html"].as_str().unwrap().starts_with("<!doctype html>"));
}

#[tokio::test]
async fn l_api_refuse_les_sources_trop_longues() {
    let res = app()
        .test()
        .await
        .post("/api/rendu")
        .json(&json!({ "source": "a".repeat(varium_site::LIMITE + 1) }))
        .await;
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn introuvable_dans_chaque_langue() {
    let client = serveur().await;
    for langue in Langue::ALL {
        let t = langue.textes();
        let res = client.get(&format!("/{}/existe-pas/", langue.code())).await;
        assert_eq!(res.status(), 404);
        let html = res.text();
        assert!(html.contains("noindex"));
        assert!(html.contains(&format!(r#"<html lang="{}""#, langue.code())));
        assert!(html.contains(&self::html(t.introuvable_h1)));
        assert!(!html.contains(r#"rel="canonical""#));
        // Le sélecteur mène à l'accueil de chaque langue.
        assert!(html.contains(r#"href="/en/" hreflang="en""#));
    }
    // Hors de toute langue : le message dans les six.
    let res = client.get("/existe-pas").await;
    assert_eq!(res.status(), 404);
    let html = res.text();
    assert!(html.contains("noindex"));
    for langue in Langue::ALL {
        assert!(html.contains(&self::html(langue.textes().introuvable_titre)));
        assert!(html.contains(&format!(
            r#"href="/{0}/" hreflang="{0}" lang="{0}""#,
            langue.code()
        )));
    }
    // Une langue que le site ne parle pas.
    assert_eq!(client.get("/ja/").await.status(), 404);
}

#[tokio::test]
async fn robots_et_sitemap() {
    let client = serveur().await;
    assert!(
        client
            .get("/robots.txt")
            .await
            .text()
            .contains("/sitemap.xml")
    );
    let sitemap = client.get("/sitemap.xml").await.text();
    for langue in Langue::ALL {
        for rubrique in Rubrique::ALL {
            let chemin = rubrique.chemin(langue);
            assert!(
                sitemap.contains(&format!("<loc>http://localhost:3000{chemin}</loc>")),
                "{chemin}"
            );
            assert!(sitemap.contains(&format!(
                r#"<xhtml:link rel="alternate" hreflang="{}" href="http://localhost:3000{chemin}"/>"#,
                langue.code()
            )));
        }
    }
    assert_eq!(sitemap.matches("<url>").count(), 6 * 4);
    assert_eq!(sitemap.matches(r#"hreflang="x-default""#).count(), 6 * 4);
}

#[tokio::test]
async fn les_routes_repondent_avec_et_sans_barre_finale() {
    let client = serveur().await;
    for chemin in [
        "/fr",
        "/fr/",
        "/en/syntaxe",
        "/en/syntaxe/",
        "/de/essayer",
        "/de/essayer/",
    ] {
        assert_eq!(client.get(chemin).await.status(), 200, "{chemin}");
    }
}

#[tokio::test]
async fn le_serveur_garde_le_formulaire_et_mene_a_l_installation() {
    let client = serveur().await;
    let html = client.get("/fr/essayer/").await.text();
    assert!(html.contains(r#"<form class="stack" method="post" action="/fr/essayer/">"#));
    assert!(
        html.contains(
            r#"<a class="button petit installer" href="/fr/installer/">Installer Varium"#
        )
    );
    let html = client.get("/de/").await.text();
    assert!(html.contains(r#"href="/de/installer/">Varium installieren"#));
    // Pas de version en ligne : le serveur mène à la page d'installation.
    let html = app().test().await.get("/fr/").await.text();
    if std::env::var_os("APP_URL").is_none() {
        assert!(html.contains(r#"href="/fr/installer/""#));
    }
}

#[tokio::test]
async fn le_serveur_repond_derriere_la_base() {
    let options = Options::new()
        .base("/VARIUM")
        .site("https://maxlestage.github.io/VARIUM");
    let client = app_with(options).test().await;
    let res = client.get("/VARIUM/en/syntaxe/").await;
    assert_eq!(res.status(), 200);
    let html = res.text();
    assert!(html.contains(r#"href="/VARIUM/site.css""#));
    assert!(html.contains(r#"href="https://maxlestage.github.io/VARIUM/en/syntaxe/""#));
    assert!(html.contains(r#"href="/VARIUM/de/syntaxe/" hreflang="de""#));
    assert_eq!(liens_sans_base(&html), Vec::<String>::new());
    let res = client.get("/VARIUM").header("accept-language", "es").await;
    assert_eq!(res.status(), 302);
    assert_eq!(res.header("location"), Some("/VARIUM/es/"));
    let res = client.get("/VARIUM/syntaxe/").await;
    assert_eq!(res.header("location"), Some("/VARIUM/fr/syntaxe/"));
    assert_eq!(client.get("/VARIUM/site.css").await.status(), 200);
    let res = client
        .post("/VARIUM/it/essayer/")
        .form(&[("source", "{{x}}"), ("val.x", "ok")])
        .await;
    assert_eq!(res.status(), 200);
    let html = res.text();
    assert!(html.contains(">ok</mark>"));
    assert!(html.contains(r#"action="/VARIUM/it/essayer/""#));
    assert_eq!(client.get("/VARIUMX/").await.status(), 404);
}

/// Les liens internes (`href="/…"`, `src="/…"`, `action="/…"`, `url=/…`) de `html`
/// qui ne sont pas derrière `/VARIUM/`.
fn liens_sans_base(html: &str) -> Vec<String> {
    let mut fautifs = Vec::new();
    for attribut in ["href=\"/", "src=\"/", "action=\"/", "url=/"] {
        for (i, _) in html.match_indices(attribut) {
            let suite = &html[i + attribut.len()..];
            if !suite.starts_with("VARIUM/") {
                fautifs.push(suite.chars().take(40).collect());
            }
        }
    }
    fautifs
}

/// Tous les fichiers `.html` sous `dossier`.
fn pages_html(dossier: &Path) -> Vec<PathBuf> {
    let mut trouves = Vec::new();
    for entree in std::fs::read_dir(dossier).unwrap().flatten() {
        let chemin = entree.path();
        if chemin.is_dir() {
            trouves.extend(pages_html(&chemin));
        } else if chemin.extension().is_some_and(|e| e == "html") {
            trouves.push(chemin);
        }
    }
    trouves
}

#[tokio::test]
async fn l_export_statique() {
    let dossier = Path::new(env!("CARGO_TARGET_TMPDIR")).join("export-varium");
    let _ = std::fs::remove_dir_all(&dossier);
    let options = Options::new()
        .base("/VARIUM")
        .site("https://maxlestage.github.io/VARIUM");
    let fichiers = varium_site::export::exporter(&dossier, options)
        .await
        .expect("export");
    let mut attendus: Vec<String> = [
        "index.html",
        "syntaxe/index.html",
        "essayer/index.html",
        "installer/index.html",
        "404.html",
        "sitemap.xml",
        "robots.txt",
        "site.css",
        "favicon.svg",
        ".nojekyll",
    ]
    .map(String::from)
    .to_vec();
    for langue in Langue::ALL {
        for rubrique in Rubrique::ALL {
            attendus.push(format!(
                "{}index.html",
                rubrique.chemin(langue).trim_start_matches('/')
            ));
        }
    }
    for fichier in &attendus {
        assert!(dossier.join(fichier).is_file(), "{fichier}");
        assert!(fichiers.iter().any(|f| f == fichier), "{fichier}");
    }
    let lire = |f: &str| std::fs::read_to_string(dossier.join(f)).unwrap();

    // Aucun lien interne sans la base, dans aucune page.
    let pages = pages_html(&dossier);
    assert_eq!(pages.len(), 6 * 4 + 1 + 3 + 1);
    for page in &pages {
        let html = std::fs::read_to_string(page).unwrap();
        assert_eq!(liens_sans_base(&html), Vec::<String>::new(), "{page:?}");
        assert!(html.contains(r#"href="/VARIUM/site.css""#), "{page:?}");
    }

    for langue in Langue::ALL {
        let code = langue.code();
        let accueil = lire(&format!("{code}/index.html"));
        assert!(accueil.contains(&format!(r#"<html lang="{code}""#)));
        assert!(accueil.contains(&format!(
            r#"<a class="button" href="/VARIUM/{code}/installer/">"#
        )));
        assert!(accueil.contains(&format!(
            r#"<link rel="canonical" href="https://maxlestage.github.io/VARIUM/{code}/"/>"#
        )));
        assert!(accueil.contains(
            r#"<link rel="alternate" hreflang="x-default" href="https://maxlestage.github.io/VARIUM/"/>"#
        ));

        let essayer = lire(&format!("{code}/essayer/index.html"));
        assert!(!essayer.contains("<form"), "{code}");
        assert!(!essayer.contains(r#"method="post""#), "{code}");
        assert!(essayer.contains(r#"class="source""#)); // le modèle, coloré
        assert!(essayer.contains(r#"class="document""#)); // le document rendu
        assert!(essayer.contains("ATELIER LUMEN"));
        assert!(essayer.contains(&html(langue.textes().essayer_installer.titre)));
        assert!(essayer.contains(&format!(
            r#"<a class="button" href="/VARIUM/{code}/installer/">"#
        )));
        assert!(essayer.contains(&format!(
            r#"href="https://maxlestage.github.io/VARIUM/{code}/essayer/""#
        )));
        assert!(
            lire(&format!("{code}/syntaxe/index.html")).contains("http://localhost:3000/api/rendu")
        );
    }

    // La racine choisit la langue dans le navigateur, le français sans JavaScript, et
    // donne un lien vers chaque langue.
    let racine = lire("index.html");
    assert!(racine.contains("navigator.languages"));
    assert!(racine.contains(
        r#"<noscript><meta http-equiv="refresh" content="0; url=/VARIUM/fr/"/></noscript>"#
    ));
    assert!(racine.contains(r#"location.replace("/VARIUM"+"/"+(c||"fr")+"/""#));
    for langue in Langue::ALL {
        assert!(racine.contains(&format!(
            r#"<a href="/VARIUM/{0}/" hreflang="{0}" lang="{0}"><strong>{1}</strong>"#,
            langue.code(),
            langue.nom()
        )));
    }
    assert!(
        racine.contains(r#"<link rel="canonical" href="https://maxlestage.github.io/VARIUM/"/>"#)
    );

    // Les anciennes adresses mènent à la version française.
    for rubrique in ["syntaxe", "essayer", "installer"] {
        let html = lire(&format!("{rubrique}/index.html"));
        assert!(html.contains(&format!(
            r#"<meta http-equiv="refresh" content="0; url=/VARIUM/fr/{rubrique}/"/>"#
        )));
        assert!(html.contains(&format!(
            r#"<link rel="canonical" href="https://maxlestage.github.io/VARIUM/fr/{rubrique}/"/>"#
        )));
        assert!(html.contains(&format!(r#"href="/VARIUM/de/{rubrique}/""#)));
        assert!(!html.contains("<script"));
    }

    let introuvable = lire("404.html");
    assert!(introuvable.contains("noindex"));
    for langue in Langue::ALL {
        assert!(introuvable.contains(&format!(r#"href="/VARIUM/{}/""#, langue.code())));
        assert!(introuvable.contains(&html(langue.textes().retour_accueil)));
    }

    let sitemap = lire("sitemap.xml");
    for langue in Langue::ALL {
        for rubrique in Rubrique::ALL {
            let url = format!(
                "<loc>https://maxlestage.github.io/VARIUM{}</loc>",
                rubrique.chemin(langue)
            );
            assert!(sitemap.contains(&url), "{url}");
        }
    }
    assert!(lire("robots.txt").contains("https://maxlestage.github.io/VARIUM/sitemap.xml"));
}

#[tokio::test]
async fn la_page_installer_propose_les_trois_systemes() {
    let client = serveur().await;
    for langue in Langue::ALL {
        let html = client.get(&Rubrique::Installer.chemin(langue)).await.text();
        for fichier in [
            "varium-windows-x86_64.zip",
            "varium-macos.zip",
            "varium-linux-x86_64.tar.gz",
        ] {
            let lien =
                format!("https://github.com/maxlestage/VARIUM/releases/latest/download/{fichier}");
            assert!(html.contains(&lien), "{fichier}");
            // Le workflow de publication produit bien ce fichier.
            assert!(
                include_str!("../../.github/workflows/release.yml").contains(fichier),
                "{fichier}"
            );
        }
        for systeme in ["Windows", "macOS", "Linux"] {
            let bouton = varium_site::i18n::remplir(langue.textes().telecharger_pour, &[systeme]);
            assert!(
                html.contains(&self::html(&bouton)),
                "{} : {bouton}",
                langue.code()
            );
        }
    }
}
