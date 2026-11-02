# Varium

> **Un document. Toutes ses possibilités.**

Varium est un éditeur de documents à variables, écrit entièrement en Rust avec
[egui](https://github.com/emilk/egui). On écrit `{{prénom}}`, `{{nom}}` ou
`{{montant}}` dans un document, on change une valeur une seule fois, et toutes ses
occurrences sont mises à jour sans toucher aux autres champs.

La même application tourne **sur le bureau** (Linux, macOS, Windows) et **dans le
navigateur** (WebAssembly).

## Fonctionnalités

- **Variables globales** : une valeur saisie met à jour toutes ses occurrences.
  `{{prénom}}` et `{{nom}}` restent toujours distincts, même collés.
- **Variables typées** : texte, texte long, date (avec calendrier), nombre, montant et
  e-mail, avec validation et format français automatique (`12 500,00 €`,
  `1er mars 2026`).
- **Filtres** : `{{nom | upper}}`, `{{montant | money:usd}}`, `{{date | date:full}}`,
  `{{titre | default:"Madame, Monsieur"}}`, etc.
- **Renommer partout** : ne modifie que la variable visée, en gardant ses filtres.
- **Figer** : remplace une variable par sa valeur dans la source.
- **Formules LaTeX** : `$…$` en ligne, `$$…$$` centrées. Un `$` qui vient d'une valeur
  (par exemple `10,00 $US`) n'ouvre jamais de formule.
- **Exports** : HTML (formules composées par KaTeX, imprimable en PDF), LaTeX (`.tex`
  complet), texte, modèle brut, projet `.varium.json`.
- **Sauvegarde automatique** (fichier local sur le bureau, `localStorage` sur le web).

## Organisation (workspace Cargo)

| Paquet | Rôle |
|---|---|
| [`crates/varium-core`](crates/varium-core) | Moteur pur, sans interface : analyse, rendu, filtres, formules, exports, format de projet. |
| [`crates/varium`](crates/varium) | Application egui/eframe : binaire de bureau et site WebAssembly (Trunk). |

## Démarrer

```sh
# Application de bureau
cargo run -p varium --release

# Site web (http://127.0.0.1:8080)
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
cd crates/varium && trunk serve --release

# Tests et vérifications
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Sous Linux, la version de bureau a besoin des bibliothèques système habituelles
(`libxkbcommon-x11`, `libwayland`, un pilote Vulkan ou OpenGL).

## Site

À chaque push sur `master`, le workflow `.github/workflows/pages.yml` construit le site
avec Trunk et le publie sur GitHub Pages. Il faut l'activer une seule fois : *Settings →
Pages → Source : GitHub Actions*.

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
