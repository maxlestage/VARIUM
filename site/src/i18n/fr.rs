//! Le français : la langue source du site.

use super::{Appli, Item, Ligne, Systeme, Textes};

pub const TEXTES: Textes = Textes {
    // ---- Commun
    accroche: "Un document. Toutes ses possibilités.",
    nav: ["Présentation", "Syntaxe", "Installer", "Essayer"],
    nav_aria: "Navigation principale",
    accueil_aria: "Varium, accueil",
    langue_choisir: "Choisir la langue",
    installer_varium: "Installer Varium",
    code_source: "Code source",
    pied: "Varium, une suite bureautique à variables écrite en Rust.",
    pied_paquets: "Ce site est écrit avec {}, le framework full-stack Rust.",
    lire_syntaxe: "Lire la syntaxe",
    voir_exemple: "Voir un exemple",

    // ---- Accueil
    description_accueil: "Varium est une suite bureautique écrite en Rust\u{a0}: Documents, Classeur et Présentation. Écrivez {{prénom}} une fois, changez sa valeur, tout suit. En six langues, pour Linux, macOS et Windows.",
    surtitre: "Suite bureautique à variables · Rust",
    chapo: "Documents, Classeur et Présentation\u{a0}: une suite bureautique complète qui tourne sur votre ordinateur. Écrivez {{prénom}}, {{nom}} ou {{montant}} dans un texte ou une diapositive, changez une valeur une seule fois\u{a0}: toutes ses occurrences se mettent à jour.",
    sous_hero: "Application de bureau pour Linux, macOS et Windows · vos documents restent chez vous · 100 % Rust",
    apercu: ["Le modèle", "Le document"],
    apercu_note: "Rendu par le vrai moteur de Varium (varium-core) à la génération de cette page.",
    applis_titre: "Trois applications, un seul Varium",
    applis_intro: "Un traitement de texte, un tableur et un logiciel de présentation, réunis dans la même application de bureau.",
    applis: [
        Appli {
            genre: "Traitement de texte",
            texte: "Des courriers, devis et contrats qui se remplissent tout seuls.",
            points: [
                "Titres, listes, tableaux et vraies pages A4, A5 ou Lettre",
                "Champs {{…}} globaux, calculés et conditionnels, publipostage, détection automatique des champs",
                "Barre d'outils façon Writer, inspecteur façon Pages",
                "Exports PDF, DOCX, ODT, RTF, EPUB, HTML et LaTeX, et un contrôle « Prêt à envoyer » avant le PDF",
                "Orthographe dans six langues, avec de vrais dictionnaires Hunspell",
            ],
        },
        Appli {
            genre: "Tableur",
            texte: "Des feuilles de calcul avec cellules, formules et graphiques.",
            points: [
                "Cellules, formules et plusieurs feuilles par classeur, et des questions en langage simple",
                "Fonctions en français ou en anglais\u{a0}: SOMME ou SUM, SI ou IF, RECHERCHEV ou VLOOKUP",
                "Mise en forme conditionnelle, échelles de couleurs, barres de données et volets figés",
                "Huit graphiques\u{a0}: barres, barres empilées, courbes, aires, secteurs, anneau, nuage de points",
                "Ouvre et enregistre les fichiers Excel (XLSX) et LibreOffice (ODS), et le CSV",
            ],
        },
        Appli {
            genre: "Diaporamas",
            texte: "Des présentations à projeter, avec vos champs dedans.",
            points: [
                "Dispositions, thèmes et transitions",
                "Images et formes à placer à la souris, champs {{…}} dans les diapositives",
                "Mode présentateur sur un deuxième écran\u{a0}: notes, diapositive suivante, chronomètre",
                "Ouvre les présentations PowerPoint (PPTX) et Impress (ODP)",
                "Exports HTML, ODP et PDF",
            ],
        },
    ],
    langues_titre: "Écrire en six langues",
    langues_intro: "Chaque document a sa langue\u{a0}: français, anglais, espagnol, allemand, italien ou portugais. Varium suit ses règles d'écriture.",
    langues: [
        Item {
            titre: "Typographie automatique",
            texte: "Les guillemets «\u{a0}» en français, “\u{a0}” en anglais ou „\u{a0}“ en allemand, l'apostrophe typographique et, en français, l'espace fine avant\u{a0};\u{a0}:\u{a0}!\u{a0}? se posent tout seuls.",
        },
        Item {
            titre: "Révision",
            texte: "Le correcteur signale les doubles espaces, les mots répétés, une majuscule oubliée, une espace avant la ponctuation, un ¿ ou un ¡ manquant en espagnol, «\u{a0}a\u{a0}» ou «\u{a0}an\u{a0}» en anglais…",
        },
        Item {
            titre: "Dates, nombres et devises",
            texte: "Chaque langue a ses formats\u{a0}: 12\u{202f}500,50 en français, 12,500.50 en anglais, 12.500,50 en allemand\u{a0}; les dates, les mois et les montants suivent la langue du document.",
        },
        Item {
            titre: "Orthographe",
            texte: "Les fautes sont soulignées pendant la frappe, avec des suggestions au clic droit. Varium utilise les dictionnaires installés ou les télécharge depuis LibreOffice, et retient vos mots dans un dictionnaire personnel.",
        },
    ],
    langues_note: "L'interface de Varium existe dans ces six langues\u{a0}; la langue du texte se choisit pour chaque document.",
    fonctions_titre: "Tout ce qu'il faut pour vos documents répétitifs",
    fonctions_intro: "Courriers, devis, contrats, attestations\u{a0}: un seul modèle, autant de documents que nécessaire.",
    fonctions: [
        Item {
            titre: "Champs globaux",
            texte: "Une valeur saisie une fois met à jour toutes ses occurrences, dans un document comme dans une présentation. {{prénom}} et {{nom}} restent toujours distincts.",
        },
        Item {
            titre: "Champs calculés",
            texte: "{{= quantité * prix | money}}\u{a0}: totaux, TVA, mensualités et pourcentages se recalculent tout seuls.",
        },
        Item {
            titre: "Texte conditionnel",
            texte: "{{si remise}}…{{sinon}}…{{fin}}\u{a0}: un paragraphe n'apparaît que si le champ est renseigné.",
        },
        Item {
            titre: "Publipostage",
            texte: "Un tableau CSV d'Excel ou de Calc, et Varium produit un document par destinataire, dans un seul fichier.",
        },
        Item {
            titre: "Modèles prêts à l'emploi",
            texte: "Lettre, facture, devis, attestation, contrat, convocation\u{a0}: un clic pour démarrer.",
        },
        Item {
            titre: "Champs typés et filtres",
            texte: "Date avec calendrier, montant, nombre, e-mail, au format de la langue du document\u{a0}; filtres upper, date:full, money:usd…",
        },
        Item {
            titre: "Vraies pages",
            texte: "Aperçu page par page au format A4, A5 ou Lettre, avec marges, en-tête, pied de page, numéros de page et zoom.",
        },
        Item {
            titre: "Exports LibreOffice et Microsoft Office",
            texte: "Documents en PDF natif, DOCX, ODT, RTF et EPUB\u{a0}; classeurs en XLSX, ODS et CSV\u{a0}; présentations en ODP, HTML et PDF.",
        },
        Item {
            titre: "Des fichiers comme dans Writer",
            texte: "Ctrl+S, «\u{a0}Enregistrer sous\u{a0}», documents récents, confirmation avant de fermer et sauvegarde de secours automatique.",
        },
        Item {
            titre: "Rechercher, remplacer, annuler",
            texte: "Recherche avec ou sans casse, remplacement un par un ou en bloc, annuler et rétablir sans limite pratique.",
        },
        Item {
            titre: "Formules LaTeX",
            texte: "$…$ en ligne, $$…$$ centrées. Un $ qui vient d'une valeur (10,00 $US) n'ouvre jamais de formule.",
        },
        Item {
            titre: "Une vraie application Mac",
            texte: "Menus dans la barre de l'écran, raccourcis ⌘, icône, plein écran et mode concentration\u{a0}; aussi pour Windows et Linux.",
        },
        Item {
            titre: "Palette de commandes ⌘K",
            texte: "Ctrl+K ou ⌘K\u{a0}: tapez quelques lettres et lancez n'importe quelle commande des trois modules\u{a0}; F1 affiche l'aide-mémoire des raccourcis.",
        },
        Item {
            titre: "Centre de démarrage",
            texte: "Modèles, fichiers récents et astuces dès l'ouverture\u{a0}; un fichier glissé, ou ouvert par double-clic dans le Finder ou l'Explorateur, part dans le bon module.",
        },
        Item {
            titre: "Remplissage instantané",
            texte: "Ctrl+E dans le Classeur\u{a0}: Varium devine le reste de la colonne à partir d'un exemple, avec saisie semi-automatique, collage intelligent et analyse rapide.",
        },
        Item {
            titre: "Tableau croisé dynamique",
            texte: "Glissez les colonnes en lignes, colonnes et valeurs\u{a0}: totaux, moyennes et comptes par groupe en un instant.",
        },
        Item {
            titre: "Historique des versions",
            texte: "Chaque enregistrement garde une version du document\u{a0}; comparez-les, différences surlignées, et revenez en arrière d'un clic.",
        },
        Item {
            titre: "Codes QR",
            texte: "Un code QR dans un document ou sur une diapositive, et une diapositive «\u{a0}Partager\u{a0}» qui donne l'adresse de la présentation.",
        },
        Item {
            titre: "Embellir et Harmoniser",
            texte: "Des mises en page proposées d'après le contenu de la diapositive, puis toute la présentation rendue cohérente, avec dix thèmes.",
        },
        Item {
            titre: "Mode présentateur",
            texte: "Le public sur le projecteur, et pour vous les notes, la diapositive suivante, le chronomètre, le pointeur laser, le stylo et le zoom.",
        },
        Item {
            titre: "Mode kiosque et album photo",
            texte: "Un diaporama en boucle, sans commandes, qui revient au début après une inactivité\u{a0}; un album tiré d'un dossier d'images, classé par date.",
        },
        Item {
            titre: "Import Word, LibreOffice et PowerPoint",
            texte: "Ouvrez des fichiers .docx et .odt dans Documents, et des présentations .pptx avec leurs couleurs de thème, dégradés, rotations et tableaux.",
        },
        Item {
            titre: "Commentaires et suivi des modifications",
            texte: "Commentez une sélection, proposez des ajouts et des suppressions, puis acceptez ou refusez chaque modification.",
        },
        Item {
            titre: "Notes de bas de page",
            texte: "De vraies notes en bas de la page, numérotées automatiquement, dans l'aperçu comme dans le PDF.",
        },
        Item {
            titre: "Signature manuscrite",
            texte: "Signez à la souris ou au pavé tactile, et la signature se pose dans le document.",
        },
        Item {
            titre: "Lecture à voix haute",
            texte: "Le document lu par la synthèse vocale du système, dans sa langue, pour relire avec les oreilles.",
        },
    ],
    etapes_titre: "Comment ça marche",
    etapes: [
        Item {
            titre: "Écrivez",
            texte: "Rédigez votre texte et placez des {{variables}} là où il change d'un document à l'autre.",
        },
        Item {
            titre: "Remplissez",
            texte: "Varium détecte chaque variable et vous propose un champ, avec le bon type\u{a0}: date, montant, e-mail…",
        },
        Item {
            titre: "Exportez",
            texte: "Le document final, formules comprises, en PDF, DOCX, ODT, HTML, LaTeX ou texte.",
        },
    ],
    bureau: Item {
        titre: "Sur le bureau",
        texte: "Une application native rapide, construite avec egui, pour Linux, macOS et Windows.",
    },
    prive: Item {
        titre: "Vos documents restent chez vous",
        texte: "Pas de compte, pas de serveur, pas de version en ligne\u{a0}: Varium enregistre vos projets sur votre disque, et rien ne quitte votre ordinateur.",
    },
    capot_titre: "Sous le capot",
    capot_intro: "Un workspace Cargo\u{a0}: des moteurs purs, sans interface, et l'application qui les réunit.",
    capot_colonnes: ["Paquet", "Rôle"],
    capot_roles: [
        "Le moteur des documents\u{a0}: analyse, rendu, filtres, formules, langues, exports, format de projet. Testé, sans dépendance graphique.",
        "Le moteur du classeur\u{a0}: cellules, formules, feuilles et exports.",
        "Le moteur des présentations\u{a0}: diapositives, dispositions, thèmes et exports.",
        "L'application de bureau egui/eframe qui réunit les trois modules, pour Linux, macOS et Windows.",
    ],
    appel_titre: "Prêt à essayer\u{a0}?",
    appel_texte: "Installez Varium sur votre ordinateur\u{a0}: écrivez vos modèles, remplissez les valeurs, calculez, présentez, exportez.",

    // ---- Syntaxe
    syntaxe_description: "La syntaxe de Varium\u{a0}: variables {{…}}, filtres, types, formules LaTeX, fonctions du Classeur et exports.",
    syntaxe_chapo: "Tout ce que l'on peut écrire dans un document Varium.",
    sommaire: [
        "Variables",
        "Filtres",
        "Types",
        "Formules",
        "Classeur",
        "Exports",
        "API",
    ],
    ecrire_titre: "Écrire des variables",
    ecrire_texte: "Une variable s'écrit entre doubles accolades. Les noms acceptent les accents, les chiffres, _, . et -\u{a0}: {{prénom}}, {{client.ville}}, {{date-1}}.",
    syntaxe_colonnes: ["Écriture", "Effet"],
    syntaxe: [
        Ligne {
            code: "{{nom}}",
            effet: "un champ\u{a0}: la même valeur partout",
        },
        Ligne {
            code: "{{ nom | upper | trim }}",
            effet: "un champ et des filtres chaînés",
        },
        Ligne {
            code: "{{montant | money}}",
            effet: "un montant en euros (money:usd, money:gbp, money:chf…)",
        },
        Ligne {
            code: "{{date | date}}",
            effet: "une date longue\u{a0}: 15 mars 2026 (date:full, date:short)",
        },
        Ligne {
            code: "{{x | number:2}}",
            effet: "un nombre à deux décimales\u{a0}: 3,14",
        },
        Ligne {
            code: "{{t | default:\"texte\"}}",
            effet: "une valeur par défaut si le champ est vide",
        },
        Ligne {
            code: "{{= quantité * prix | money}}",
            effet: "un champ calculé\u{a0}: + − * / ( ), 20%, arrondi(x; 2), min, max, somme",
        },
        Ligne {
            code: "{{si remise}}…{{sinon}}…{{fin}}",
            effet: "du texte conditionnel",
        },
        Ligne {
            code: "{{aujourdhui}}",
            effet: "la date du jour, remplie automatiquement",
        },
        Ligne {
            code: "# Titre, ## Sous-titre, ### Titre 3",
            effet: "des titres",
        },
        Ligne {
            code: "**gras**, *italique*",
            effet: "gras et italique",
        },
        Ligne {
            code: "__souligné__, ~~barré~~",
            effet: "souligné et barré",
        },
        Ligne {
            code: "- élément, 1. élément",
            effet: "listes à puces et numérotées (2 espaces de retrait par niveau)",
        },
        Ligne {
            code: "> citation",
            effet: "une citation",
        },
        Ligne {
            code: "-> centré <-, -> à droite ->",
            effet: "alignement du paragraphe",
        },
        Ligne {
            code: "| a | b |  puis  |---|---:|",
            effet: "un tableau, son en-tête et l'alignement des colonnes",
        },
        Ligne {
            code: "---, \\newpage",
            effet: "ligne horizontale, saut de page",
        },
        Ligne {
            code: "\\sommaire",
            effet: "une table des matières générée depuis les titres",
        },
        Ligne {
            code: "\\note{texte}",
            effet: "une note numérotée [1], rassemblée en fin de document",
        },
        Ligne {
            code: "$E = mc^2$, $$ \\sum_i x_i $$",
            effet: "des formules LaTeX, en ligne ou centrées",
        },
        Ligne {
            code: "\\{{nom}}",
            effet: "des accolades littérales, sans champ",
        },
        Ligne {
            code: "\\$, \\*, \\|",
            effet: "un caractère littéral",
        },
    ],
    filtres_titre: "Filtres",
    filtres_texte: "Les filtres transforment la valeur d'une occurrence, et seulement elle. Ils s'enchaînent de gauche à droite\u{a0}: {{nom | trim | upper}}. Les résultats ci-dessous sont calculés par varium-core, pour un document en français.",
    filtres_colonnes: ["Filtre", "Valeur", "Résultat"],
    filtre_defaut: "Madame, Monsieur",
    types_titre: "Types de variables",
    types_texte: "Chaque variable a un type, qui choisit son champ de saisie, vérifie la valeur et donne un format par défaut quand l'occurrence n'a pas de filtre\u{a0}: en français, une date s'affiche «\u{a0}1er mars 2026\u{a0}», un montant «\u{a0}12\u{202f}500,00 €\u{a0}».",
    types: ["Texte", "Texte long", "Date", "Nombre", "Montant", "E-mail"],
    formules_titre: "Formules",
    formules_texte: "Entre $…$ (en ligne) ou $$…$$ (centrée), le texte est du LaTeX, et les variables y fonctionnent aussi. Seuls les $ écrits dans la source ouvrent une formule\u{a0}: une valeur comme «\u{a0}10,00 $US\u{a0}» ne casse jamais le document.",
    formules_exemple: "La mensualité vaut $m = \\frac{M}{n}$, avec\n$$ C_n = C_0 \\times (1 + t)^n $$",
    classeur_titre: "Formules du Classeur",
    classeur_texte: "Dans le Classeur, une formule commence par = et les fonctions s'écrivent en français ou en anglais, au choix\u{a0}: =SOMME(A1:A10) et =SUM(A1:A10) donnent le même total.",
    classeur_colonnes: ["Fonction (français · anglais)", "Rôle"],
    classeur_fonctions: [
        "additionne les valeurs d'une plage",
        "renvoie une valeur ou une autre selon une condition",
        "cherche une valeur dans la première colonne d'une plage et renvoie celle d'une autre colonne",
    ],
    classeur_note: "Dans une présentation, les {{champs}} s'écrivent comme dans un document.",
    exports_titre: "Exports",
    exports_noms: ["Texte", "Modèle", "Projet"],
    exports: [
        "natif, sans navigateur, en pages au bon format, avec en-tête et pied de page.",
        "un document Word, lisible aussi par LibreOffice et Google Docs.",
        "un document LibreOffice Writer, avec ses styles, son en-tête et ses numéros de page.",
        "pour Pages et TextEdit.",
        "un livre numérique.",
        "un fichier autonome, formules composées par KaTeX, prêt à imprimer.",
        "un .tex complet, à compiler tel quel.",
        "le document final, sans mise en forme.",
        "la source brute, avec ses {{variables}}.",
        "un fichier .varium.json avec la source, les valeurs et les types.",
    ],
    api_titre: "API de rendu",
    api_intro_statique: "Le serveur de ce site (cd site && cargo run) expose le moteur en JSON, pour l'essayer depuis un terminal\u{a0}:",
    api_intro_serveur: "Ce site expose le moteur en JSON, pour l'essayer depuis un terminal\u{a0}:",
    api_reponse: "La réponse contient le texte rendu (texte), l'export HTML complet (html) et les variables détectées avec leur nombre d'occurrences (variables).",

    // ---- Installer
    installer_description: "Télécharger Varium, la suite bureautique à variables (Documents, Classeur, Présentation), pour Windows, macOS et Linux.",
    installer_chapo: "Varium est une suite bureautique gratuite pour Windows, macOS et Linux\u{a0}: Documents, Classeur et Présentation. Pas de compte, pas de version en ligne\u{a0}: vos fichiers restent sur votre ordinateur.",
    telecharger_pour: "Télécharger pour {}",
    systemes: [
        Systeme {
            compat: "Windows 10 et 11, 64 bits",
            etapes: [
                "Décompressez le fichier (clic droit → Extraire tout).",
                "Lancez varium.exe.",
                "Si Windows affiche «\u{a0}Windows a protégé votre ordinateur\u{a0}»\u{a0}: «\u{a0}Informations complémentaires\u{a0}» → «\u{a0}Exécuter quand même\u{a0}».",
            ],
        },
        Systeme {
            compat: "macOS 11 ou plus récent, Apple Silicon et Intel",
            etapes: [
                "Décompressez le fichier et glissez Varium dans le dossier Applications.",
                "Au premier lancement\u{a0}: clic droit sur Varium → Ouvrir → Ouvrir.",
                "Si macOS refuse encore\u{a0}: Réglages Système → Confidentialité et sécurité → «\u{a0}Ouvrir quand même\u{a0}».",
            ],
        },
        Systeme {
            compat: "x86_64, glibc 2.35 ou plus récente (Ubuntu 22.04, Debian 12, Fedora 36…)",
            etapes: [
                "Décompressez\u{a0}: tar xzf varium-linux-x86_64.tar.gz",
                "Lancez\u{a0}: ./varium/varium",
                "Pour le menu des applications\u{a0}: copiez varium dans ~/.local/bin, varium.desktop dans ~/.local/share/applications et varium.svg dans ~/.local/share/icons.",
            ],
        },
    ],
    versions: "Toutes les versions et leurs notes\u{a0}: {}. Les applications ne sont pas encore signées par Apple ni Microsoft, d'où l'avertissement au premier lancement.",
    versions_lien: "page des versions sur GitHub",
    source_titre: "Depuis le code source",
    source_texte: "Avec Rust (rustup.rs), la commande varium s'installe sur n'importe quel système\u{a0}:",
    source_linux: "Sous Linux, Varium utilise les bibliothèques graphiques habituelles (libxkbcommon, Wayland ou X11, un pilote OpenGL ou Vulkan), déjà présentes sur un bureau.",
    source_github: "Code source sur GitHub",

    // ---- Essayer
    essayer_titre: "Essayer Varium",
    essayer_description: [
        "Écrivez un modèle avec des {{variables}}, donnez-leur une valeur et voyez le document se mettre à jour\u{a0}: la démo en ligne de Varium.",
        "Un modèle avec des {{variables}}, leurs valeurs et le document rendu par Varium. La suite complète s'installe sur votre ordinateur.",
    ],
    essayer_explication: [
        "Modifiez le modèle ou les valeurs puis mettez le document à jour. Le rendu est calculé par le vrai moteur de Varium (varium-core), sur le serveur.",
        "Le courrier d'exemple de Varium\u{a0}: le modèle et ses valeurs à gauche, le document calculé par le vrai moteur (varium-core) à droite.",
    ],
    essayer_installer: Item {
        titre: "Pour écrire vos propres modèles, installez Varium.",
        texte: "La suite complète est une application de bureau pour Linux, macOS et Windows\u{a0}: mise en page, champs calculés, publipostage, classeur, présentations, exports ODT, DOCX et PDF. Vos documents restent sur votre ordinateur.",
    },
    modele: "Modèle",
    modele_aide: "écrivez {{une_variable}} n'importe où",
    lecture_seule: "lecture seule",
    variables: "Variables ({})",
    aucune_variable: "Aucune variable pour l'instant\u{a0}: ajoutez {{prénom}} au modèle puis mettez à jour.",
    mettre_a_jour: "Mettre à jour le document",
    reinitialiser: "Réinitialiser",
    variables_colonnes: ["Variable", "Type", "Valeur"],
    a_remplir: "à remplir",
    document: "Document",
    compteur: [
        "{} variable",
        "{} variables",
        "{} occurrence",
        "{} occurrences",
    ],
    a_remplir_n: "{} à remplir",
    texte_brut: "Texte brut",
    sans_valeur: "«\u{a0}{}\u{a0}» n'a pas encore de valeur",
    saut_de_page: "saut de page",
    type_de: "Type de {}",
    format_date: "AAAA-MM-JJ",
    erreurs: [
        "Date attendue\u{a0}: AAAA-MM-JJ",
        "Nombre attendu",
        "Adresse e-mail invalide",
    ],
    tronque: "Le modèle a été tronqué à {} caractères.",

    // ---- Introuvable
    introuvable_titre: "Page introuvable",
    introuvable_description: "Cette page n'existe pas.",
    introuvable_h1: "{{page}} introuvable",
    introuvable_texte: "Cette variable n'a pas de valeur\u{a0}: la page demandée n'existe pas.",
    retour_accueil: "Retour à l'accueil",
};
