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

## Déployer

Tout est prêt à la racine du dépôt : `Dockerfile`, `heroku.yml`, `app.json` et
`.github/workflows/heroku.yml`. Le serveur écoute sur `$PORT` et ne demande aucune base de
données ni aucun secret.

### Heroku, en un clic

[![Deploy to Heroku](https://www.herokucdn.com/deploy/button.svg)](https://heroku.com/deploy?template=https://github.com/maxlestage/varium)

Heroku lit `app.json`, crée l'app sur la pile `container` et construit l'image du `Dockerfile`.

### Heroku, à chaque push sur `master`

Le workflow `.github/workflows/heroku.yml` déploie le site par l'API Platform de Heroku
(sans la CLI, donc réglable depuis un téléphone). Dans *GitHub → Settings → Secrets and
variables → Actions* :

| Type | Nom | Valeur |
|---|---|---|
| secret | `HEROKU_API_KEY` | *Heroku → Account settings → API Key → Reveal* |
| variable | `HEROKU_APP_NAME` | le nom de l'app (créée si elle n'existe pas) |
| variable (facultative) | `HEROKU_REGION` | `us` (par défaut) ou `eu` |

Sans ces réglages, le workflow s'arrête sans erreur. Le lien du site apparaît dans le
résumé de chaque déploiement.

### N'importe quel hôte Docker

```sh
docker build -t varium-site .            # depuis la racine du dépôt
docker run --rm -p 8080:8080 varium-site # http://localhost:8080
```

Render, Fly.io, Railway, Koyeb, Google Cloud Run… : pointez-les sur le `Dockerfile` de la
racine ; ils fournissent `PORT`. Pour des URL canoniques fixes, définissez
`SITE_URL=https://votre-domaine.fr`.
