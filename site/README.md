# Le site de Varium

Le site de présentation de Varium, écrit **entièrement avec [Paquets](https://github.com/maxlestage/Paquets)** :
le serveur est une `paquets::App`, les pages sont des `paquets::Page` rendues par le serveur
(balises SEO, Open Graph, URL canonique, thème clair et sombre, Content-Security-Policy), et
la démo fait tourner le vrai moteur `varium-core` côté serveur, sans une ligne de JavaScript.

| Route | Contenu |
|---|---|
| `/` | présentation : aperçu modèle → document, fonctionnalités, plateformes, architecture |
| `/syntaxe` | référence : variables, filtres (résultats calculés par `varium-core`), types, formules, exports, API |
| `/essayer` | démo : on modifie le modèle et les valeurs, le serveur renvoie le document rendu |
| `POST /api/rendu` | le moteur en JSON : `{source, valeurs, types}` → `{texte, html, variables}` |
| `/robots.txt`, `/sitemap.xml` | pour les moteurs de recherche |

| Fichier | Rôle |
|---|---|
| `src/lib.rs` | l'`App` et ses routes |
| `src/layout.rs` | en-tête, navigation, pied de page, SEO |
| `src/pages.rs` | accueil, syntaxe, 404, robots, sitemap |
| `src/demo.rs` | démo et API, rendu du document en vues |
| `public/` | `site.css` et `favicon.svg`, servis sur `/` par Paquets |
| `tests/site.rs` | toutes les routes, testées avec `app().test()` |

## Lancer

```sh
cd site
cargo run --release          # http://localhost:3000 (ou $PORT)
```

À lancer depuis `site/` : Paquets sert le dossier `public/` du répertoire courant.
`SITE_URL` fixe l'adresse publique (URL canoniques, sitemap) ; sinon elle vient de la requête.

## Vérifier

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
