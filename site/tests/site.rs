//! Le site entier, testé avec le client en mémoire de Paquets (`app().test()`).

use paquets::prelude::*;
use varium_site::app;

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
    assert!(sitemap.contains("/essayer</loc>"));
}
