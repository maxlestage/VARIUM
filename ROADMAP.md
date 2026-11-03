# Feuille de route de Varium

Objectif : un logiciel très robuste, le plus proche possible de LibreOffice Writer.
Règles : pas de version web, jamais de push sur master, `.varium.json` toujours rétro-compatible.

## 1. Robustesse
- [x] Tests de fuzz déterministes du moteur (`crates/varium-core/tests/robustesse.rs`) : analyse, rendu, formules, CSV, exports, projets corrompus, entrées très longues — « Robustesse : fuzz, profondeur des formules, parseur linéaire »
- [x] Formules très imbriquées refusées proprement (plus de dépassement de pile) — idem
- [x] Parseur linéaire sur les `{{` non fermés (était quadratique) — idem
- [ ] Fuzz côté application (editor.rs, pageview.rs) : indexation sur positions de curseur/Unicode
- [ ] Revue des `unwrap`/`expect` restants dans `crates/varium` (app.rs, io.rs)
- [ ] Récupération d'un projet corrompu dans l'application (message clair, ouverture de la source brute)
- [x] Test de rétro-compatibilité des projets v1 : existe déjà (`projets_v1_toujours_lisibles`, tests/engine.rs)

## 2. Fichiers comme Writer
- [x] Ctrl+S réécrit le fichier ouvert, Ctrl+Maj+S « Enregistrer sous » — « Fichiers comme Writer : Ctrl+S, Enregistrer sous, récents, confirmation »
- [x] Indicateur de modifications non enregistrées dans le titre — « Fichiers comme Writer : Ctrl+S, Enregistrer sous, récents, confirmation »
- [x] Confirmation avant fermeture / ouverture d'un autre document — « Fichiers comme Writer : Ctrl+S, Enregistrer sous, récents, confirmation »
- [x] Documents récents — « Fichiers comme Writer : Ctrl+S, Enregistrer sous, récents, confirmation »
- [x] Sauvegarde de secours automatique sur disque (`.nom.varium-secours` à côté du document, ou dossier de données ; restauration proposée à l'ouverture/au démarrage ; secours.rs) — « Sauvegarde de secours automatique »
- [ ] Test d'intégration de l'état « modifié » de l'application (logique is_dirty/guard hors egui)
- [ ] Restaurer le fichier courant au redémarrage (le chemin n'est pas conservé)

## 3. Distribution
- [x] Workflow de release (binaires Linux, macOS, Windows à chaque tag) — « Binaires à télécharger pour Windows, macOS et Linux (version 0.2.0) »
- [x] Page /installer/ du site avec les liens de téléchargement — idem

## 4. Export PDF natif (en-tête, pied de page, numéros de page)
- [x] PDF natif sans dépendance (`pdf.rs`) : césure, justification, titres, listes, citations, tableaux, filets, sauts de page, en-tête/pied/« Page X sur N » par document, métriques Liberation — « Export PDF natif »
- [ ] Incorporer les polices Liberation (sous-ensemble TrueType) pour Unicode complet (grec des formules, émojis) au lieu de `?`
- [ ] Compression Flate des flux (fichiers plus petits) ; formules `$$…$$` en vrai rendu plutôt qu'en texte Unicode
- [ ] Liens et signets (plan des titres) dans le PDF

## 5. Proche de Writer
- [x] Texte conditionnel `{{si champ}}…{{sinon}}…{{fin}}` (conditions.rs) — « Texte conditionnel »
- [x] Table des matières `\sommaire` (liste des titres, sans numéros de page) — « Table des matières »
- [ ] Numéros de page dans la table des matières (nécessite la pagination)
- [ ] Notes de bas de page
- [ ] Retraits, couleurs/surlignage, interligne par paragraphe
- [ ] Import .odt / .docx simples
- [x] Détection des doubles espaces / correcteur orthographique

## 6. Performance : cache de rendu et de pagination pour les longs documents

## 7. Accessibilité et ergonomie : clavier, info-bulles, messages d'erreur en français

## 8. Site et README à jour avec chaque fonction livrée

## 9. Comme Pages et natif sur Mac
- [x] Inspecteur Format / Document / Champs / Plan, galerie de modèles au premier lancement — « Varium façon Pages, et natif sur Mac »
- [x] Menus natifs macOS (muda), raccourcis ⌘, plein écran, mode concentration — idem
- [x] Typographie française automatique, exports RTF et EPUB — idem
- [x] Icône de l'application (Dock, .icns, varium.exe, Linux) et job CI macOS — idem
- [ ] Ouvrir un document par double-clic dans le Finder (événements Apple « open documents » non transmis par winit)
- [ ] Barre de titre unifiée façon macOS, à valider sur un vrai Mac
- [ ] Vignettes des pages dans la barre latérale (comme Pages)
- [ ] Commentaires et suivi des modifications

## 10. Suite bureautique et langues
- [x] Module Classeur (varium-calc + calc_ui) : formules FR/EN, feuilles, formats, tri, recopie, graphiques, XLSX/ODS/CSV
- [x] Module Présentation (varium-slides + slides_ui) : dispositions, thèmes, transitions, notes, diaporama plein écran, PDF/ODP/HTML
- [x] Langue du document : formats, typographie, révision, aide par langue
- [x] Site traduit (fr, en, es, de, it, pt)
- [x] Révision de l'écriture par langue (espaces, ponctuation, majuscules, accords a/an, ¿ ¡) — version 0.4.0
- [x] Classeur : mise en forme conditionnelle, figer les volets, import XLSX/ODS, plus de types de graphiques — version 0.5.0
- [x] Présentation : images, formes, mode présentateur sur deuxième écran, import ODP/PPTX — version 0.5.0
- [x] Dictionnaires orthographiques (Hunspell) par langue : installés, importés ou téléchargés — version 0.5.0
- [x] Interface traduite en six langues (choix automatique ou manuel, sans redémarrer) — version 0.5.0
- [x] Menus natifs macOS pour le Classeur et la Présentation (à valider sur un vrai Mac) — version 0.5.0
- [ ] Classeur : couleurs de thème à l'import XLSX, hauteur de ligne variable
- [ ] Présentation : rotations, dégradés et tableaux à l'import PPTX
- [ ] Messages du moteur (remarques de révision, erreurs de formule) traduits
