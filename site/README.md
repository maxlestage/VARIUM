# Le site de Varium

Le site de présentation de Varium, écrit **entièrement avec [Paquets](https://github.com/maxlestage/Paquets)** :
le serveur est une `paquets::App`, les pages sont des `paquets::Page` rendues par le serveur
(balises SEO, Open Graph, URL canonique, thème clair et sombre, Content-Security-Policy), et
la démo fait tourner le vrai moteur `varium-core` côté serveur, sans une ligne de JavaScript.

Il est publié sur GitHub Pages, avec l'application web à côté de lui :

| Adresse | Contenu |
|---|---|
| <https://maxlestage.github.io/VARIUM/> | ce site, exporté en fichiers statiques |
| <https://maxlestage.github.io/VARIUM/app/> | l'application web (egui, WebAssembly), le vrai produit |

| Route | Contenu |
|---|---|
| `/` | présentation : aperçu modèle → document, fonctionnalités, plateformes, architecture |
| `/syntaxe/` | référence : variables, filtres (résultats calculés par `varium-core`), types, formules, exports, API |
| `/essayer/` | démo : on modifie le modèle et les valeurs, le serveur renvoie le document rendu (en lecture seule dans l'export statique) |
| `POST /api/rendu` | le moteur en JSON : `{source, valeurs, types}` → `{texte, html, variables}` |
| `/robots.txt`, `/sitemap.xml` | pour les moteurs de recherche |

Les pages répondent avec ou sans `/` final ; les liens du site finissent par `/`, comme les
dossiers de l'export.

| Fichier | Rôle |
|---|---|
| `src/lib.rs` | l'`App` et ses routes : `app()` (réglée par l'environnement) et `app_with(Options)` |
| `src/options.rs` | `Options` : chemin de base, adresses publiques, mode statique ; `lien()` pour chaque lien interne |
| `src/export.rs` | l'export statique, pour GitHub Pages |
| `src/layout.rs` | en-tête (avec « Ouvrir l'application »), navigation, pied de page, SEO |
| `src/pages.rs` | accueil, syntaxe, 404, robots, sitemap |
| `src/demo.rs` | démo et API, rendu du document en vues |
| `public/` | `site.css` et `favicon.svg`, servis sur `/` par Paquets |
| `tests/site.rs` | toutes les routes, testées avec `app().test()`, et l'export |

## Lancer

```sh
cd site
cargo run --release          # http://localhost:3000 (ou $PORT)
```

À lancer depuis `site/` : Paquets sert le dossier `public/` du répertoire courant.

| Variable | Effet |
|---|---|
| `SITE_URL` | l'adresse publique, base comprise (URL canoniques, sitemap) ; sinon elle vient de la requête |
| `BASE_PATH` | un chemin de base (`/VARIUM`) devant chaque lien interne ; le serveur répond aussi derrière lui |
| `APP_URL` | l'adresse du bouton « Ouvrir l'application » ; par défaut, l'application publiée sur GitHub Pages pour le serveur, `{BASE_PATH}/app/` pour l'export |

## Export statique (GitHub Pages)

```sh
cd site
BASE_PATH=/VARIUM SITE_URL=https://maxlestage.github.io/VARIUM \
  cargo run --release -- export dist
```

Chaque page est rendue par l'application elle-même, par le client en mémoire de Paquets
(`app_with(options).test()`), puis écrite dans `dist/` : `index.html`, `syntaxe/index.html`,
`essayer/index.html`, `404.html` (une adresse inconnue), `sitemap.xml`, `robots.txt`, le
contenu de `public/` et `.nojekyll`. Chaque lien interne est derrière `BASE_PATH`.

Un hôte statique ne reçoit pas de formulaire : dans l'export, `/essayer/` montre le courrier
d'exemple en lecture seule (le modèle coloré, ses valeurs et le document rendu) et renvoie
vers l'application web, où tourne l'éditeur complet. `/api/rendu` n'est pas exporté. Sans
`SITE_URL`, les URL absolues sont `https://maxlestage.github.io{BASE_PATH}`.

Pour voir l'export comme sur GitHub Pages :

```sh
mkdir -p /tmp/pages && ln -sfn "$PWD/dist" /tmp/pages/VARIUM
python3 -m http.server -d /tmp/pages 8000   # http://localhost:8000/VARIUM/
```

Le workflow `.github/workflows/pages.yml` le fait à chaque push sur `master` : il exporte le
site dans `site/dist` (`BASE_PATH` et `SITE_URL` donnés par `actions/configure-pages`),
construit l'application avec `trunk build --release --public-url /VARIUM/app/`, la copie
dans `site/dist/app` et publie le tout. À activer une seule fois : *Settings → Pages →
Source : GitHub Actions*.

## Vérifier

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Déployer le serveur

GitHub Pages publie l'export statique (voir plus haut). Pour la démo interactive et
`POST /api/rendu`, il faut le serveur : tout est prêt à la racine du dépôt : `Dockerfile`, `heroku.yml`, `app.json` et
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
