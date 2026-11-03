//! Le site entier, testé avec le client en mémoire de Paquets (`app().test()`).

use std::path::Path;

use paquets::prelude::*;
use varium_site::{Options, app, app_with};

#[tokio::test]
async fn les_pages_repondent() {
    let client = app().test().await;
    for (chemin, titre) in [
        (
            "/",
            "<title>Varium · Un document. Toutes ses possibilités.</title>",
        ),
        ("/syntaxe", "<title>Syntaxe · Varium</title>"),
        ("/essayer", "<title>Essayer · Varium</title>"),
    ] {
        let res = client.get(chemin).await;
        assert_eq!(res.status(), 200, "{chemin}");
        assert!(res.header("content-security-policy").is_some(), "{chemin}");
        let html = res.text();
        assert!(html.contains(titre), "{chemin}");
        assert!(html.contains(r#"<html lang="fr""#), "{chemin}");
        assert!(html.contains("rel=\"canonical\""), "{chemin}");
    }
}

#[tokio::test]
async fn l_accueil_montre_un_vrai_rendu() {
    let html = app().test().await.get("/").await.text();
    // Calculé par varium-core : filtre upper, montant et date au format français.
    assert!(html.contains("DURAND"));
    assert!(html.contains("12\u{202f}500,00\u{a0}€"));
    assert!(html.contains("vendredi 18 décembre 2026"));
}

#[tokio::test]
async fn la_syntaxe_calcule_les_filtres() {
    let html = app().test().await.get("/syntaxe").await.text();
    assert!(html.contains("<strong>1er mars 2026</strong>"));
    assert!(html.contains("<strong>Madame, Monsieur</strong>"));
}

#[tokio::test]
async fn la_demo_rend_le_formulaire_envoye() {
    let client = app().test().await;
    let res = client
        .post("/essayer")
        .form(&[
            ("source", "Bonjour {{prénom | upper}} {{nom}} <b>"),
            ("val.prénom", "Léa"),
            ("type.prénom", "text"),
        ])
        .await;
    assert_eq!(res.status(), 200);
    let html = res.text();
    assert!(html.contains(">LÉA</mark>"));
    assert!(html.contains(r#"class="manquante""#)); // {{nom}} n'a pas de valeur
    assert!(html.contains(r#"name="val.nom""#)); // un champ par variable détectée
    assert!(!html.contains("<b>")); // le texte de l'utilisateur est échappé
}

#[tokio::test]
async fn la_demo_refuse_les_autres_sites() {
    let res = app()
        .test()
        .await
        .post("/essayer")
        .header("sec-fetch-site", "cross-site")
        .form(&[("source", "x")])
        .await;
    assert_eq!(res.status(), 403);
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
async fn introuvable_robots_sitemap() {
    let client = app().test().await;
    let res = client.get("/existe-pas").await;
    assert_eq!(res.status(), 404);
    assert!(res.text().contains("noindex"));
    assert!(
        client
            .get("/robots.txt")
            .await
            .text()
            .contains("/sitemap.xml")
    );
    let sitemap = client.get("/sitemap.xml").await.text();
    assert!(sitemap.contains("/essayer/</loc>"));
}

#[tokio::test]
async fn les_routes_repondent_avec_et_sans_barre_finale() {
    let client = app().test().await;
    for chemin in ["/syntaxe", "/syntaxe/", "/essayer", "/essayer/"] {
        assert_eq!(client.get(chemin).await.status(), 200, "{chemin}");
    }
}

#[tokio::test]
async fn le_serveur_garde_le_formulaire_et_ouvre_l_application() {
    let client = app_with(Options::new()).test().await;
    let html = client.get("/essayer/").await.text();
    assert!(html.contains(r#"<form class="stack" method="post" action="/essayer/">"#));
    assert!(html.contains(r#"<a class="button petit" href="/app/">Ouvrir l&#x27;application"#));
    // Le serveur, sans APP_URL, mène à l'application publiée sur GitHub Pages.
    let html = app().test().await.get("/").await.text();
    if std::env::var_os("APP_URL").is_none() {
        assert!(html.contains(&format!(r#"href="{}""#, varium_site::APP_PUBLIQUE)));
    }
}

#[tokio::test]
async fn le_serveur_repond_derriere_la_base() {
    let options = Options::new()
        .base("/VARIUM")
        .site("https://maxlestage.github.io/VARIUM");
    let client = app_with(options).test().await;
    let res = client.get("/VARIUM/syntaxe/").await;
    assert_eq!(res.status(), 200);
    let html = res.text();
    assert!(html.contains(r#"href="/VARIUM/site.css""#));
    assert!(html.contains(r#"href="https://maxlestage.github.io/VARIUM/syntaxe/""#));
    assert_eq!(client.get("/VARIUM").await.status(), 200);
    assert_eq!(client.get("/VARIUM/site.css").await.status(), 200);
    let res = client
        .post("/VARIUM/essayer/")
        .form(&[("source", "{{x}}"), ("val.x", "ok")])
        .await;
    assert_eq!(res.status(), 200);
    assert!(res.text().contains(">ok</mark>"));
    assert_eq!(client.get("/VARIUMX/").await.status(), 404);
}

/// Les liens internes (`href="/…"`, `src="/…"`, `action="/…"`) de `html` qui ne sont
/// pas derrière `/VARIUM/`.
fn liens_sans_base(html: &str) -> Vec<String> {
    let mut fautifs = Vec::new();
    for attribut in ["href=\"/", "src=\"/", "action=\"/"] {
        for (i, _) in html.match_indices(attribut) {
            let suite = &html[i + attribut.len()..];
            if !suite.starts_with("VARIUM/") {
                fautifs.push(suite.chars().take(40).collect());
            }
        }
    }
    fautifs
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
    for fichier in [
        "index.html",
        "syntaxe/index.html",
        "essayer/index.html",
        "404.html",
        "sitemap.xml",
        "robots.txt",
        "site.css",
        "favicon.svg",
        ".nojekyll",
    ] {
        assert!(dossier.join(fichier).is_file(), "{fichier}");
        assert!(fichiers.iter().any(|f| f == fichier), "{fichier}");
    }
    let lire = |f: &str| std::fs::read_to_string(dossier.join(f)).unwrap();

    for page in [
        "index.html",
        "syntaxe/index.html",
        "essayer/index.html",
        "404.html",
    ] {
        let html = lire(page);
        assert_eq!(liens_sans_base(&html), Vec::<String>::new(), "{page}");
        assert!(html.contains(r#"href="/VARIUM/site.css""#), "{page}");
        assert!(html.contains(r#"href="/VARIUM/syntaxe/""#), "{page}");
        // Le bouton de l'en-tête : l'application, publiée à côté du site.
        assert!(html.contains(r#"href="/VARIUM/app/""#), "{page}");
    }

    let accueil = lire("index.html");
    assert!(accueil.contains(r#"<a class="button" href="/VARIUM/app/">"#));
    assert!(accueil.contains(r#"href="https://maxlestage.github.io/VARIUM/""#));

    let essayer = lire("essayer/index.html");
    assert!(!essayer.contains(r#"<form method="post""#));
    assert!(!essayer.contains(r#"method="post""#));
    assert!(!essayer.contains("<form"));
    assert!(essayer.contains(r#"class="source""#)); // le modèle, coloré
    assert!(essayer.contains(r#"class="document""#)); // le document rendu
    assert!(essayer.contains("ATELIER LUMEN"));
    assert!(essayer.contains(r#"<a class="button" href="/VARIUM/app/">"#));
    assert!(essayer.contains(r#"href="https://maxlestage.github.io/VARIUM/essayer/""#));

    let introuvable = lire("404.html");
    assert!(introuvable.contains("noindex"));
    assert!(introuvable.contains(r#"href="/VARIUM/""#));

    let sitemap = lire("sitemap.xml");
    for url in [
        "/VARIUM/</loc>",
        "/VARIUM/syntaxe/</loc>",
        "/VARIUM/essayer/</loc>",
    ] {
        assert!(
            sitemap.contains(&format!("https://maxlestage.github.io{url}")),
            "{url}"
        );
    }
    assert!(lire("robots.txt").contains("https://maxlestage.github.io/VARIUM/sitemap.xml"));
    assert!(lire("syntaxe/index.html").contains("http://localhost:3000/api/rendu"));
}
