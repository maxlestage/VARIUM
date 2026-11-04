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

Une nouvelle version se publie en poussant un tag : `git tag v0.6.0 && git push origin v0.6.0`.
Le workflow `.github/workflows/release.yml` compile alors les trois binaires et les joint à
la Release GitHub (il se lance aussi à la main depuis l'onglet Actions).

## Trois applications, une suite

Comme LibreOffice, Varium réunit trois modules dans la même fenêtre (sélecteur en haut) :

| Module | Pour | Points forts |
|---|---|---|
| 📝 **Documents** | courriers, factures, contrats | champs `{{…}}`, mise en page, publipostage, exports PDF/ODT/DOCX/RTF/EPUB |
| 📊 **Classeur** | tableaux, budgets, calculs | formules en français et en anglais (`=SOMME(A1:A9)` / `=SUM(…)`), plusieurs feuilles, formats (€, %, dates), tri, recopie, mise en forme conditionnelle, volets figés, 8 graphiques, ouvre et enregistre XLSX, ODS, CSV |
| 🎞 **Présentation** | diaporamas | dispositions, thèmes, transitions, notes de l'orateur, `{{champs}}` partagés, images et formes, diaporama plein écran (F5), mode présentateur sur deuxième écran, ouvre PPTX et ODP, exports PDF, ODP (Impress), HTML |

L'interface est disponible en français, anglais, espagnol, allemand, italien et portugais (bouton 🌐 en haut à droite, ou automatique selon le système). Sur macOS, la barre de menus native suit le module actif.

## Écrire dans plusieurs langues

Document → **Langue** : français, anglais, espagnol, allemand, italien ou portugais.

- **Formats** : dates (« 1er mars 2026 », « 1 March 2026 », « 1. März 2026 »…), nombres et
  montants (« 12 500,00 € », « €12,500.00 », « 12.500,00 € ») selon la langue.
- **Typographie automatique** : « » avec espaces insécables en français, “ ” en anglais et
  en portugais, „ “ en allemand, «» en espagnol et en italien ; apostrophe ’, points de
  suspension, tirets.
- **Révision** (onglet de l'inspecteur) : doubles espaces, mots répétés, majuscule en début
  de phrase, espaces autour de la ponctuation (insécables en français, aucune ailleurs),
  ¿ ¡ en espagnol, a/an et « I » en anglais, accents fréquents en italien, parenthèses non
  fermées ; correction en un clic, remarques soulignées dans le texte.
- **Aide → Écrire en plusieurs langues** : les règles de chaque langue.
- Les exports portent la langue (HTML `lang`, ODT, DOCX, EPUB).

- **Orthographe** : vrais dictionnaires Hunspell (ceux de l'ordinateur, importés ou téléchargés depuis LibreOffice), fautes soulignées pendant la frappe, suggestions au clic droit, dictionnaire personnel.

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
  automatique (veuves et orphelines, notes de bas de page en bas de leur page) et zoom ;
  la même mise en page que le PDF. Police Liberation Serif ou Sans, celles de LibreOffice.
  **Vignettes des pages** dans une barre latérale (Affichage → Vignettes des pages),
  comme Pages : un clic mène à la page.
- **Mise en forme du paragraphe** (inspecteur Format → Paragraphe) : retraits, alinéa,
  interligne (simple, 1,15, 1,5, double, exact), espaces avant et après, couleur du
  texte et surlignage ; dans tous les exports.
- **Signature manuscrite** (Insertion → Signature…) : dessinée à la souris ou au pavé
  tactile (trait lissé), gardée sur l'ordinateur pour la réutiliser ou intégrée au
  document ; en traits vectoriels dans la page et le PDF, en SVG en HTML, en image en
  ODT, DOCX et RTF.
- **Éditeur mis en forme** : le gras s'affiche en gras, les titres en grand, les marques
  restent discrètes.
- **Rechercher et remplacer**, **annuler et rétablir**, **barre d'état** (pages, mots,
  caractères, champs, zoom).

**Ouvrir Word et LibreOffice, réviser à plusieurs**

- **Ouvrir .docx et .odt** (Fichier → Ouvrir, glisser-déposer, palette, documents récents) :
  titres, paragraphes, gras, italique, souligné, barré, listes à plusieurs niveaux
  (puces et numéros), tableaux (en-tête, alignement des colonnes), liens, alignement,
  sauts de page, notes de bas de page (`[^1]`), table des matières (`\sommaire`), format
  du papier, orientation, marges, police, langue, en-tête et pied de page (numéros de
  page). Les images ne sont pas importées : un commentaire marque leur place. Les
  commentaires et modifications suivies de Word ou de LibreOffice deviennent des marques
  de révision. Enregistrer crée un document Varium, l'original n'est jamais réécrit ;
  un fichier abîmé donne un message clair, jamais un plantage.
- **Commentaires et suivi des modifications** (Outils → Révision, onglet « Modifications »
  de l'inspecteur) : « Suivre les modifications » marque ce que vous tapez (`{++ajout++}`,
  `{--suppression--}`, `{~~ancien~>nouveau~~}`), signé de votre nom et de la date ;
  « Commenter la sélection » (`{==passage==}{>>remarque<<}`) ; Accepter / Refuser chaque
  marque, Tout accepter / Tout refuser, aller au texte. Les marques sont colorées dans
  l'éditeur. Les exports montrent la version finale (ou les marques, en option : ajouts
  soulignés, suppressions barrées, commentaires en notes).
- **Modèles intelligents** : Fichier → « Créer un modèle à partir de ce document » repère
  les dates, montants, e-mails et noms (Détecter les champs), vide les champs et prépare
  une question par champ (« Quel est le nom du client ? »), dans la galerie de vos
  modèles. « Nouveau à partir d'un modèle » (et l'accueil) pose ensuite les questions une
  à une — progression, Précédent / Suivant, saisie adaptée (calendrier, montant, liste de
  choix, oui/non) — puis écrit le document. Six **lettres types** fournies en six
  langues : résiliation, réclamation, relance de facture, attestation d'emploi, demande
  de congés, lettre de motivation.
- **Badge « 3 champs possibles »** dans la barre d'état quand on colle ou ouvre un texte qui
  contient des dates, montants, e-mails ou noms répétés : un clic ouvre la détection.
- **Lire à voix haute** (Outils) : la sélection, ou le document depuis le curseur, lue par
  la synthèse vocale du système (`say` sur macOS avec une voix de la langue du document,
  `espeak-ng`, `espeak` ou `spd-say` sous Linux, PowerShell sous Windows), phrase par
  phrase, la phrase lue surlignée ; Pause / Reprendre / Arrêter dans la barre d'état.

**Comme Pages, et natif sur Mac**

- **Inspecteur** à droite, comme dans Pages : onglets Format (styles de paragraphe,
  police, alignement, espacement, insertion), Document (mise en page, statistiques),
  Champs et Plan (titres cliquables).
- **Galerie de modèles** au premier lancement, **plein écran** (⌃⌘F) et **mode
  concentration** (F11 ou Ctrl+Maj+F, ⌘⇧F) : menus et panneaux masqués, texte au centre,
  paragraphe en cours mis en avant, défilement « machine à écrire » ; Échap pour sortir.
- **Historique des versions**, comme Pages ou Time Machine (Fichier → Historique des
  versions…, Ctrl+Maj+H) : une version à chaque enregistrement, toutes les cinq minutes
  d'écriture et avant les grosses opérations (Tout corriger, Tout remplacer, renommage…),
  sans doublon et dans une taille limitée ; heure, nombre de mots et résumé de ce qui a
  changé ; comparaison mot à mot (dans le texte ou côte à côte, ajouts en vert, retraits
  en rouge), « Restaurer cette version », « Copier le texte », versions nommées
  (« Marquer cette version… », jamais effacées).
- **Objectif de mots** avec un anneau de progression dans la barre d'état, mots de la
  session, temps de lecture et de parole, et **lisibilité** selon la langue (Flesch–Kandel
  en français, Flesch en anglais, Fernández-Huerta en espagnol, Amstad en allemand,
  Gulpease en italien, Flesch adapté en portugais) expliquée en mots simples
  (inspecteur → Document).
- **Collage intelligent** : un texte copié depuis un navigateur, Word, LibreOffice ou
  Google Docs garde ses titres, gras, italique, listes, liens et tableaux ; des cellules
  de tableur ou un CSV deviennent un tableau ; une adresse collée sur une sélection en
  fait un lien ; le Markdown est converti. Ctrl+Maj+V colle du texte brut.
- **Correction automatique pendant la frappe** (Outils, selon la langue) : majuscule en
  début de phrase, « BOnjour » → « Bonjour », fautes de frappe courantes, (c) → ©,
  1/2 → ½, -> → →, ... → …, guillemets et apostrophe typographiques ; un Ctrl+Z juste
  après rend le texte tapé.
- **Remplir comme un formulaire** (Outils) : les champs du document deviennent des
  questions (libellés lisibles, calendrier pour les dates, listes de choix tirées des
  `{{si champ = valeur}}`, cases à cocher), avec validation et l'aperçu de la page à côté.
- **Détecter les champs** (Outils, Ctrl+K) : dans un document déjà écrit, Varium repère
  les dates, montants (`1 250,50 €`), adresses e-mail, numéros de téléphone et noms qui
  reviennent, et propose d'en faire des `{{champs}}` en un clic (noms modifiables, cases à
  cocher). Chaque champ garde la valeur d'origine : le document ne change pas, mais il
  devient un modèle réutilisable (publipostage compris). Annulable avec Ctrl+Z.
- **Poser une question** (Classeur, menu Données, Ctrl+Maj+Q) : écrivez « total des ventes par région », « moyenne de B », « combien de clients » ou « maximum des ventes » ; Varium la traduit en formules (`SOMME`, `MOYENNE`, `NBVAL`, `MAX`, `MIN`, `MEDIANE`, `SOMME.SI`, `MOYENNE.SI`, `NB.SI`), montre le résultat et l'écrit dans un espace libre, sans toucher à vos données. Les colonnes se désignent par leur titre ou leur lettre ; les mots-clés sont compris en français, anglais, espagnol, allemand, italien et portugais.
- **Apparence et taille de l'interface** (bouton 🎨 en haut à droite, sur les trois modules) :
  thème automatique (suit le système), clair, sombre, **sépia** (papier chaud, reposant) ou
  **contraste élevé** (noir et blanc, rapport ≥ 7:1, contours épais), et zoom de 80 à 200 %
  pour les gros caractères. Le choix est mémorisé.
- **Typographie française automatique** : « guillemets », apostrophe ’, …, espaces fines
  avant ; : ! ? (désactivable dans Document).
- **PDF natif** : « Exporter → PDF » écrit directement le fichier (sans navigateur), avec la même mise en page que la vue page : polices Liberation **incorporées** (sous-ensemble TrueType, plus une police de secours pour les symboles et les émojis : tout le texte Unicode s'affiche et se copie), flux compressés, texte justifié, titres, listes, tableaux coupés entre pages, **notes de bas de page** en bas de leur page, **formules composées** (fractions, indices, racines, sommes et intégrales avec bornes, matrices), signatures en traits vectoriels, en-tête, pied de page et « Page X sur N », table des matières avec numéros de page et liens vers les titres, plan de signets, liens cliquables (adresses `http`, `https`, `mailto`), métadonnées (titre, auteur, langue, XMP).
- **Exports Pages** : RTF (Pages, TextEdit) et EPUB (livre numérique), en plus de PDF,
  DOCX, ODT, HTML, LaTeX et texte.
- **Sur macOS** : menus dans la barre de l'écran (Varium, Fichier, Édition, Insertion,
  Format, Présentation, Fenêtre, Aide) avec les raccourcis ⌘, À propos, Services,
  Masquer ; `Varium.app` universelle avec son icône. Icône aussi dans `varium.exe` et
  dans le paquet Linux.

**Les champs**

- **Champs globaux** : une valeur saisie met à jour toutes ses occurrences.
  `{{prénom}}` et `{{nom}}` restent toujours distincts, même collés.
- **Codes QR** : `{{qr:https://exemple.fr}}` ou `{{site | qr}}` (Insertion → Code QR…),
  dessinés dans la page et dans tous les exports (PDF vectoriel, HTML en SVG, ODT et DOCX
  en image) ; générés par Varium lui-même (niveau de correction M, versions 1 à 40).
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


### Site installable (PWA)

Le site de présentation s'installe comme une application (Chrome, Edge, Safari « Sur l'écran d'accueil », Android) et reste lisible hors ligne :

- un manifeste par langue (`/{langue}/manifest.webmanifest`) : nom, description, raccourcis (Télécharger, Syntaxe, Essayer) et captures dans la langue ;
- un service worker (`/sw.js`) qui garde les 24 pages, la feuille de style et les icônes ;
- les icônes : favicon SVG et ICO, PNG de 16 à 512 px, icônes adaptatives Android, icône Apple, masque Safari ;
- le partage : image 1200 × 630 par langue (Open Graph, Twitter), bouton de partage natif, « Copier le lien », liens e-mail, WhatsApp, Telegram, X, Bluesky, LinkedIn, Facebook et Reddit, et « Installer le site » quand le navigateur le propose.

Le tout suit le chemin de base (`/VARIUM` sur GitHub Pages) et n'utilise aucun script en ligne (`public/pwa.js`). Ce n'est pas une version web de l'application : Varium reste une application de bureau.

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
| `$E = mc^2$`, `$$ \sum_i x_i $$` | formules LaTeX, composées dans la page et le PDF (MathML en HTML/EPUB/ODT, formules Word en DOCX) |
| `\$` | dollar littéral |
| `{{= quantité * prix \| money}}` | champ calculé (`+ - * / ( )`, `20%`, `arrondi(x; 2)`, `min`, `max`, `somme`) |
| `{{si champ}}…{{sinon}}…{{fin}}` | texte conditionnel (champ renseigné, `si non`, `=`, `!=`) |
| `{{aujourdhui}}` | date du jour |
| `{{qr:https://exemple.fr}}` | code QR du texte (sur sa propre ligne) |
| `{{site \| qr}}` | code QR de la valeur d'un champ |
| `[texte](https://exemple.fr)` | lien (aussi `mailto:`, `www.`) |
| `# Titre`, `## Sous-titre`, `### Titre 3` | titres |
| `**gras**`, `*italique*`, `__souligné__`, `~~barré~~` | caractères |
| `- élément`, `1. élément` | listes (2 espaces de retrait par niveau) |
| `> citation` | citation |
| `-> centré <-`, `-> à droite ->` | alignement |
| `\| a \| b \|` puis `\|---\|---:\|` | tableau, en-tête et alignement des colonnes |
| `---` | ligne horizontale |
| `\newpage` | saut de page |
| `{: retrait=1cm alinéa=5mm interligne=1,5 avant=6pt après=6pt} texte` | mise en forme du paragraphe : retraits (`retrait=`, `droite=`, `alinéa=` négatif pour un retrait suspendu ; cm par défaut, aussi `mm`, `pt`, `in`), interligne (`simple`, `1,15`, `1,5`, `double`, `exact:14pt`), espaces avant et après (pt), `couleur=`, `surlignage=` ; se combine avec `#`, `-`, `->` |
| `{: interligne=double}` seul sur sa ligne, puis `{:}` | la même mise en forme pour tous les paragraphes suivants, puis retour à la normale |
| `{couleur:rouge}texte{/}`, `{surlignage:vert}texte{/}`, `==surligné==` | couleur du texte et surlignage (noir, gris, rouge, orange, jaune, vert, turquoise, cyan, bleu, violet, magenta, rose, marron ou `#rrggbb`) |
| `\sommaire` | table des matières : titres, points de conduite, numéros de page, liens |
| `texte[^1]` puis `[^1]: la note`, `^[note en ligne]`, `\note{texte}` | notes de bas de page numérotées, en bas de la page de leur appel (vraies notes en ODT et DOCX, notes de fin en HTML et EPUB) |
| `{{signature}}`, `{{signature:nom}}` | signature manuscrite (Insertion → Signature…) |
| `\*`, `\_`, `\~`, `\|`, `\#`, `\>`, `\=`, `\{` | caractère littéral |
| `{++ajout++}`, `{--retrait--}` | modification suivie (absente des exports : version finale) |
| `{~~ancien~>nouveau~~}` | remplacement proposé |
| `{==passage==}{>>remarque<<}`, `{>>remarque<<}` | commentaire (« auteur · date : remarque ») |
