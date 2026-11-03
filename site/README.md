# Le site de Varium

Le site de présentation de Varium, la suite bureautique à variables (**Documents**,
**Classeur**, **Présentation**), écrit **entièrement avec [Paquets](https://github.com/maxlestage/Paquets)** :
le serveur est une `paquets::App`, les pages sont des `paquets::Page` rendues par le serveur
(balises SEO, Open Graph, URL canonique, `hreflang`, thème clair et sombre,
Content-Security-Policy), et la démo fait tourner le vrai moteur `varium-core` côté
serveur, sans une ligne de JavaScript.

Il est publié sur GitHub Pages : <https://maxlestage.github.io/VARIUM/>. Varium n'a pas
de version en ligne : la page `/{langue}/installer/` explique comment installer
l'application de bureau.

## Six langues

Le site parle **français** (la source), **anglais**, **espagnol**, **allemand**, **italien**
et **portugais**. Chaque page existe dans chaque langue, à la même adresse précédée du code
de la langue : `/fr/syntaxe/`, `/en/syntaxe/`, `/de/syntaxe/`…

| Route | Contenu |
|---|---|
| `/` | redirige vers `/{langue}/` d'après `Accept-Language` (français sinon) |
| `/{langue}/` | présentation : les trois applications, l'écriture en six langues, aperçu modèle → document, fonctionnalités, plateformes, architecture |
| `/{langue}/syntaxe/` | référence : variables, filtres (résultats calculés par `varium-core`), types, formules, fonctions du Classeur, exports, API |
| `/{langue}/installer/` | téléchargements pour Windows, macOS et Linux, et installation depuis les sources |
| `/{langue}/essayer/` | démo : on modifie le modèle et les valeurs, le serveur renvoie le document rendu (en lecture seule dans l'export statique) |
| `/syntaxe/`, `/essayer/`, `/installer/` | les anciennes adresses : redirection permanente vers `/fr/…` |
| `POST /api/rendu` | le moteur en JSON : `{source, valeurs, types}` → `{texte, html, variables}` |
| `/robots.txt`, `/sitemap.xml` | pour les moteurs de recherche (le sitemap liste les 24 pages et leurs versions) |

Chaque page porte `<html lang>`, son URL canonique, `og:locale`, un lien `hreflang` vers
chacune de ses versions et `x-default` (la racine pour l'accueil, la version française
sinon). Le sélecteur de langue de l'en-tête (un `<details>`, sans JavaScript) mène à la
même page dans l'autre langue. Une adresse inconnue sous `/{langue}/` donne la page
introuvable de cette langue ; ailleurs, une page introuvable dans les six langues.

Les textes sont dans `src/i18n/` : un fichier par langue, chacun une constante `Textes`.
Une traduction oubliée ne compile pas (un `struct` exige tous ses champs), et les tests de
`src/i18n/mod.rs` vérifient que :

- aucun texte n'est vide ni entouré d'espaces ;
- aucun texte n'est resté identique au français, hors d'une courte liste de mots
  transparents (`Variables`, `Document`, `Date`, `API`…) ;
- les marques survivent à la traduction : `{}`, `{{…}}` (avec les mêmes filtres et
  mots-clés), `$`, `→`, `…`, et pour le code la syntaxe (`*`, `|`, `#`, `\`…).

Le courrier d'exemple (accueil et démo) reste en français dans toutes les langues, comme
les formats de date et de montant que le moteur lui applique ; le texte autour le dit.

Les pages répondent avec ou sans `/` final ; les liens du site finissent par `/`, comme les
dossiers de l'export.

| Fichier | Rôle |
|---|---|
| `src/lib.rs` | l'`App` et ses routes : `app()` (réglée par l'environnement) et `app_with(Options)` |
| `src/i18n/` | `Langue`, le choix d'après `Accept-Language`, les textes (`fr.rs`, `en.rs`, `es.rs`, `de.rs`, `it.rs`, `pt.rs`) et leurs tests |
| `src/routes.rs` | `Rubrique` : les pages et leurs adresses dans chaque langue |
| `src/options.rs` | `Options` : chemin de base, adresses publiques, mode statique ; `lien()` pour chaque lien interne |
| `src/export.rs` | l'export statique, pour GitHub Pages |
| `src/layout.rs` | en-tête (navigation, langues, « Installer Varium »), pied de page, SEO |
| `src/pages.rs` | accueil, syntaxe, installation, introuvable, racine et anciennes adresses, robots, sitemap |
| `src/demo.rs` | démo et API, rendu du document en vues |
| `public/` | `site.css` et `favicon.svg`, servis sur `/` par Paquets |
| `tests/site.rs` | toutes les routes dans toutes les langues, testées avec `app().test()`, et l'export |

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
| `APP_URL` | l'adresse du bouton « Installer Varium » ; par défaut, la page `{BASE_PATH}/{langue}/installer/` |

## Export statique (GitHub Pages)

```sh
cd site
BASE_PATH=/VARIUM SITE_URL=https://maxlestage.github.io/VARIUM \
  cargo run --release -- export dist
```

Chaque page est rendue par l'application elle-même, par le client en mémoire de Paquets
(`app_with(options).test()`), puis écrite dans `dist/` : `{langue}/index.html`,
`{langue}/syntaxe/index.html`, `{langue}/installer/index.html` et
`{langue}/essayer/index.html` pour les six langues, `index.html` (la racine),
`syntaxe/index.html`, `essayer/index.html`, `installer/index.html` (les anciennes adresses),
`404.html`, `sitemap.xml`, `robots.txt`, le contenu de `public/` et `.nojekyll`. Chaque lien
interne est derrière `BASE_PATH`.

Ce qu'un hôte statique ne sait pas faire est remplacé :

- **La racine** ne peut pas lire `Accept-Language` : `index.html` choisit la langue dans le
  navigateur (la première de `navigator.languages` que le site parle, le français sinon) et
  y va par `location.replace` ; sans JavaScript, un `<meta http-equiv="refresh">` (dans
  `<noscript>`) mène à `/fr/`. La page donne aussi un lien vers chaque langue.
- **Les anciennes adresses** (`/syntaxe/`, `/essayer/`, `/installer/`) sont des pages qui
  mènent à `/fr/…` par `<meta http-equiv="refresh">`, avec l'URL canonique française et un
  lien vers chaque langue.
- **`404.html`**, que GitHub Pages sert pour toute adresse inconnue, dit « introuvable » dans
  les six langues, chacune avec le lien vers son accueil.
- **La démo** : un hôte statique ne reçoit pas de formulaire, donc `/{langue}/essayer/`
  montre le courrier d'exemple en lecture seule (le modèle coloré, ses valeurs et le
  document rendu) et renvoie vers l'installation. `/api/rendu` n'est pas exporté.

Sans `SITE_URL`, les URL absolues sont `https://maxlestage.github.io{BASE_PATH}`.

Pour voir l'export comme sur GitHub Pages :

```sh
mkdir -p /tmp/pages && ln -sfn "$PWD/dist" /tmp/pages/VARIUM
python3 -m http.server -d /tmp/pages 8000   # http://localhost:8000/VARIUM/ (→ /VARIUM/fr/, /VARIUM/en/…)
```

Le workflow `.github/workflows/pages.yml` le fait à chaque push sur `master` : il exporte le
site dans `site/dist` (`BASE_PATH` et `SITE_URL` donnés par `actions/configure-pages`) et
le publie. À activer une seule fois : *Settings → Pages →
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
