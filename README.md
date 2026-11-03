# Varium

> **Un document. Toutes ses possibilités.**

Varium est un éditeur de documents à variables, écrit entièrement en Rust avec
[egui](https://github.com/emilk/egui). On écrit `{{prénom}}`, `{{nom}}` ou
`{{montant}}` dans un document, on change une valeur une seule fois, et toutes ses
occurrences sont mises à jour sans toucher aux autres champs.

Varium est une **application de bureau** (Linux, macOS, Windows). Il n'y a pas de
version en ligne : vos documents restent sur votre ordinateur.

- **Site de présentation** : <https://maxlestage.github.io/VARIUM/>
- **Télécharger** (dernière version) :
  [Windows](https://github.com/maxlestage/VARIUM/releases/latest/download/varium-windows-x86_64.zip) ·
  [macOS](https://github.com/maxlestage/VARIUM/releases/latest/download/varium-macos.zip) (Apple Silicon et Intel) ·
  [Linux](https://github.com/maxlestage/VARIUM/releases/latest/download/varium-linux-x86_64.tar.gz) —
  mode d'emploi sur la [page Installer](https://maxlestage.github.io/VARIUM/installer/)
- **Depuis les sources** : `cargo install --git https://github.com/maxlestage/VARIUM varium --locked`, puis `varium`

Une nouvelle version se publie en poussant un tag : `git tag v0.3.0 && git push origin v0.3.0`.
Le workflow `.github/workflows/release.yml` compile alors les trois binaires et les joint à
la Release GitHub (il se lance aussi à la main depuis l'onglet Actions).

## Fonctionnalités

**Comme LibreOffice Writer**

- **Barre de menus** Fichier, Édition, Affichage, Insertion, Format, Outils, Aide, et
  **barre d'outils** : styles de paragraphe, gras, italique, souligné, barré,
  alignement, listes, champs, tableau, saut de page.
- **Raccourcis de Writer** : Ctrl+B/I/U, Ctrl+1/2/3 (titres), Ctrl+0 (corps de texte),
  Ctrl+E/R/L (alignement), F12 / Maj+F12 (listes), Ctrl+Entrée (saut de page),
  Ctrl+Z/Y, Ctrl+F/H, Ctrl+S (enregistrer), Ctrl+Maj+S (enregistrer sous), Ctrl+P.
- **Fichiers comme Writer** : Ctrl+S réécrit le fichier ouvert (écriture atomique),
  « Enregistrer sous… », indicateur • de modifications non enregistrées dans le titre
  de la fenêtre, confirmation avant de fermer, de créer ou d'ouvrir un autre document,
  menu « Documents récents », sauvegarde de secours automatique toutes les 30 s
  (restaurée après un plantage ou une coupure).
- **Vue page** : de vraies feuilles A4, A5, Lettre ou Légal, en portrait ou en paysage,
  avec marges, en-tête, pied de page, numéros « Page 1 sur 3 », pagination
  automatique et zoom. Police Liberation Serif ou Sans, celles de LibreOffice.
- **Éditeur mis en forme** : le gras s'affiche en gras, les titres en grand, les marques
  restent discrètes.
- **Rechercher et remplacer**, **annuler et rétablir**, **barre d'état** (pages, mots,
  caractères, champs, zoom).

**Comme Pages, et natif sur Mac**

- **Inspecteur** à droite, comme dans Pages : onglets Format (styles de paragraphe,
  police, alignement, espacement, insertion), Document (mise en page, statistiques),
  Champs et Plan (titres cliquables).
- **Galerie de modèles** au premier lancement, **plein écran** (F11, ⌃⌘F) et **mode
  concentration** (⌘⇧F).
- **Typographie française automatique** : « guillemets », apostrophe ’, …, espaces fines
  avant ; : ! ? (désactivable dans Document).
- **PDF natif** : « Exporter → PDF » écrit directement le fichier (sans navigateur) : polices Times/Helvetica, texte justifié, titres, listes, tableaux coupés entre pages, en-tête, pied de page et « Page X sur N ». Les caractères hors latin-1/WinAnsi (formules exotiques, émojis) sont remplacés par `?` ; l'incorporation des polices est prévue.
- **Exports Pages** : RTF (Pages, TextEdit) et EPUB (livre numérique), en plus de PDF,
  DOCX, ODT, HTML, LaTeX et texte.
- **Sur macOS** : menus dans la barre de l'écran (Varium, Fichier, Édition, Insertion,
  Format, Présentation, Fenêtre, Aide) avec les raccourcis ⌘, À propos, Services,
  Masquer ; `Varium.app` universelle avec son icône. Icône aussi dans `varium.exe` et
  dans le paquet Linux.

**Les champs**

- **Champs globaux** : une valeur saisie met à jour toutes ses occurrences.
  `{{prénom}}` et `{{nom}}` restent toujours distincts, même collés.
- **Champs typés** : texte, texte long, date (avec calendrier), nombre, montant et
  e-mail, avec validation et format français automatique (`12 500,00 €`,
  `1er mars 2026`).
- **Filtres** : `{{nom | upper}}`, `{{montant | money:usd}}`, `{{date | date:full}}`,
  `{{titre | default:"Madame, Monsieur"}}`, etc.
- **Champs calculés** : `{{= quantité * prix | money}}`, `{{= total * 20%}}`,
  `arrondi(x; 2)`, `min`, `max`, `somme`.
- **Texte conditionnel** : `{{si remise}}…{{sinon}}…{{fin}}`, `{{si non champ}}`,
  `{{si statut = payé}}` ; les blocs s'imbriquent, une directive seule sur sa ligne disparaît avec elle.
- **Date du jour** : `{{aujourdhui}}` se remplit tout seul.
- **Renommer partout** (formules comprises) et **figer** une valeur dans le texte.

**Produire des documents**

- **Modèles** : lettre, facture, devis, attestation, contrat, convocation.
- **Publipostage** : un CSV (Excel, Calc, Google Sheets ; `;`, `,` ou tabulation,
  UTF-8 ou Windows-1252) et Varium produit un document par ligne, dans un seul fichier.
- **Exports** : ODT (LibreOffice Writer), DOCX (Word), PDF par l'impression, HTML,
  LaTeX, texte, modèle brut, projet `.varium.json`.
- **Formules LaTeX** : `$…$` en ligne, `$$…$$` centrées.
- **Sauvegarde automatique** (fichier local sur le bureau, stockage de l'application) et **fenêtre étroite** avec des onglets Texte, Champs et Pages.

## Organisation (workspace Cargo)

| Paquet | Rôle |
|---|---|
| [`crates/varium-core`](crates/varium-core) | Moteur pur, sans interface : analyse, champs calculés, mise en forme, mise en page, exports (HTML, LaTeX, texte, ODT, DOCX), publipostage CSV, format de projet. |
| [`crates/varium`](crates/varium) | Application de bureau egui/eframe. |

## Démarrer

```sh
# Application de bureau
cargo run -p varium --release

# Tests et vérifications
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Sous Linux, la version de bureau a besoin des bibliothèques système habituelles
(`libxkbcommon-x11`, `libwayland`, un pilote Vulkan ou OpenGL).

## Fusion et déploiement automatiques

Chaque push sur une branche lance la CI (`.github/workflows/ci.yml`). Quand tout est vert,
la branche est **fusionnée automatiquement dans `master`**, puis `master` est déployé : le
site de présentation sur GitHub Pages, et le serveur du site sur
Heroku quand il est configuré. Les branches nommées `wip/...` ne sont jamais fusionnées :
c'est la place des travaux en cours.

## Site de présentation

Le dossier [`site/`](site) contient le site de présentation de Varium (accueil, syntaxe,
exemple), écrit avec [Paquets](https://github.com/maxlestage/Paquets). Il est publié sur
GitHub Pages : **<https://maxlestage.github.io/VARIUM/>**.

`cd site && cargo run --release`, puis <http://localhost:3000> : le serveur, avec la démo
interactive et l'API de rendu. `cargo run --release -- export dist` écrit le site en
fichiers statiques : voir [`site/README.md`](site/README.md#export-statique-github-pages).
Déploiement du serveur (Heroku, Docker) : voir
[`site/README.md`](site/README.md#déployer-le-serveur).

[![Deploy to Heroku](https://www.herokucdn.com/deploy/button.svg)](https://heroku.com/deploy?template=https://github.com/maxlestage/varium)

## Syntaxe

| Écriture | Effet |
|---|---|
| `{{nom}}` | variable |
| `{{ nom \| upper \| trim }}` | variable + filtres chaînés |
| `{{montant \| money}}` | montant en euros (`money:usd`, `money:gbp`, `money:chf`…) |
| `{{date \| date}}` | `15 mars 2026` (`date:full`, `date:short`) |
| `{{x \| number:2}}` | `3,14` |
| `{{t \| default:"texte"}}` | valeur par défaut si le champ est vide |
| `\{{nom}}` | accolades littérales |
| `$E = mc^2$`, `$$ \sum_i x_i $$` | formules LaTeX |
| `\$` | dollar littéral |
| `{{= quantité * prix \| money}}` | champ calculé (`+ - * / ( )`, `20%`, `arrondi(x; 2)`, `min`, `max`, `somme`) |
| `{{si champ}}…{{sinon}}…{{fin}}` | texte conditionnel (champ renseigné, `si non`, `=`, `!=`) |
| `{{aujourdhui}}` | date du jour |
| `# Titre`, `## Sous-titre`, `### Titre 3` | titres |
| `**gras**`, `*italique*`, `__souligné__`, `~~barré~~` | caractères |
| `- élément`, `1. élément` | listes (2 espaces de retrait par niveau) |
| `> citation` | citation |
| `-> centré <-`, `-> à droite ->` | alignement |
| `\| a \| b \|` puis `\|---\|---:\|` | tableau, en-tête et alignement des colonnes |
| `---` | ligne horizontale |
| `\newpage` | saut de page |
| `\*`, `\_`, `\~`, `\|`, `\#`, `\>` | caractère littéral |
