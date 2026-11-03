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

## 5. Proche de Writer
- [x] Texte conditionnel `{{si champ}}…{{sinon}}…{{fin}}` (conditions.rs) — « Texte conditionnel »
- [ ] Table des matières, notes de bas de page
- [ ] Retraits, couleurs/surlignage, interligne par paragraphe
- [ ] Import .odt / .docx simples
- [ ] Détection des doubles espaces / correcteur orthographique

## 6. Performance : cache de rendu et de pagination pour les longs documents

## 7. Accessibilité et ergonomie : clavier, info-bulles, messages d'erreur en français

## 8. Site et README à jour avec chaque fonction livrée
