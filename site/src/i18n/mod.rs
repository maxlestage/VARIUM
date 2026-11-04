//! Les langues du site : français (la source), anglais, espagnol, allemand, italien et
//! portugais. La langue d'une page vient de son adresse (`/fr/`, `/en/`, `/es/`…).
//!
//! Chaque langue est une constante [`Textes`] (un fichier par langue) : une traduction
//! oubliée est une erreur de compilation, puisqu'un `struct` doit recevoir tous ses
//! champs. Les tests de ce module vérifient en plus que rien n'est vide, que rien n'est
//! resté en français (sauf les noms propres et le code, listés), et que les marques
//! (`{}`, `{{…}}`, `$`, `→`, filtres) sont les mêmes dans chaque langue.

mod de;
mod en;
mod es;
mod fr;
mod it;
mod pt;

/// Une langue du site.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Langue {
    Fr,
    En,
    Es,
    De,
    It,
    Pt,
}

impl Langue {
    /// Toutes les langues, dans l'ordre du sélecteur. Le français, la source, d'abord.
    pub const ALL: [Langue; 6] = [
        Langue::Fr,
        Langue::En,
        Langue::Es,
        Langue::De,
        Langue::It,
        Langue::Pt,
    ];

    /// La valeur de cette langue dans `[fr, en, es, de, it, pt]`.
    pub fn choix<T: Copy>(self, valeurs: [T; 6]) -> T {
        valeurs[match self {
            Langue::Fr => 0,
            Langue::En => 1,
            Langue::Es => 2,
            Langue::De => 3,
            Langue::It => 4,
            Langue::Pt => 5,
        }]
    }

    /// Le code ISO 639-1, qui est aussi le premier segment des adresses : `fr`.
    pub fn code(self) -> &'static str {
        self.choix(["fr", "en", "es", "de", "it", "pt"])
    }

    /// Le nom de la langue dans la langue elle-même, pour le sélecteur.
    pub fn nom(self) -> &'static str {
        self.choix([
            "Français",
            "English",
            "Español",
            "Deutsch",
            "Italiano",
            "Português",
        ])
    }

    /// La locale Open Graph (`og:locale`).
    pub fn locale(self) -> &'static str {
        self.choix(["fr_FR", "en_US", "es_ES", "de_DE", "it_IT", "pt_PT"])
    }

    /// Les textes du site dans cette langue.
    pub fn textes(self) -> &'static Textes {
        self.choix([
            &fr::TEXTES,
            &en::TEXTES,
            &es::TEXTES,
            &de::TEXTES,
            &it::TEXTES,
            &pt::TEXTES,
        ])
    }

    /// `fr`, `en`… exactement (les adresses sont en minuscules).
    pub fn depuis_code(code: &str) -> Option<Langue> {
        Langue::ALL.into_iter().find(|l| l.code() == code)
    }

    /// La langue d'une étiquette BCP 47 : `fr-CA`, `pt-BR`, `DE`.
    pub fn depuis_etiquette(etiquette: &str) -> Option<Langue> {
        let principale = etiquette.trim().split(['-', '_']).next()?;
        Langue::depuis_code(&principale.to_ascii_lowercase())
    }

    /// `true` quand `n` demande le pluriel : à partir de 2 en français, pour tout
    /// nombre différent de 1 ailleurs (« 0 variables » en anglais).
    pub fn pluriel(self, n: usize) -> bool {
        match self {
            Langue::Fr => n > 1,
            _ => n != 1,
        }
    }
}

/// La langue préférée d'un en-tête `Accept-Language` parmi celles du site, le français
/// sinon : `de-CH, de;q=0.9, en;q=0.8` donne l'allemand, `ja, zh` le français.
pub fn langue_preferee(accept_language: &str) -> Langue {
    let mut classees: Vec<(Langue, f32)> = accept_language
        .split(',')
        .filter_map(|morceau| {
            let mut parties = morceau.split(';');
            let langue = Langue::depuis_etiquette(parties.next()?)?;
            let q = parties
                .filter_map(|p| p.trim().strip_prefix("q="))
                .find_map(|q| q.trim().parse::<f32>().ok())
                .unwrap_or(1.0);
            Some((langue, q))
        })
        .filter(|(_, q)| *q > 0.0)
        .collect();
    // Tri stable : à qualité égale, l'ordre du navigateur.
    classees.sort_by(|a, b| b.1.total_cmp(&a.1));
    classees.first().map_or(Langue::Fr, |(l, _)| *l)
}

/// Remplace les `{}` de `modele`, dans l'ordre, par `valeurs`.
pub fn remplir(modele: &str, valeurs: &[&str]) -> String {
    let mut sortie = String::with_capacity(modele.len() + 16);
    let mut reste = modele;
    let mut valeurs = valeurs.iter();
    while let Some(i) = reste.find("{}") {
        sortie.push_str(&reste[..i]);
        sortie.push_str(valeurs.next().copied().unwrap_or("{}"));
        reste = &reste[i + 2..];
    }
    sortie.push_str(reste);
    sortie
}

/// Un titre et son texte.
pub struct Item {
    pub titre: &'static str,
    pub texte: &'static str,
}

/// Une des trois applications de Varium (son nom, propre, est dans `pages.rs`).
pub struct Appli {
    /// Le genre de logiciel : « Traitement de texte ».
    pub genre: &'static str,
    pub texte: &'static str,
    pub points: [&'static str; 5],
}

/// Un système de la page d'installation (son nom et son fichier sont dans `pages.rs`).
pub struct Systeme {
    /// Les versions prises en charge.
    pub compat: &'static str,
    pub etapes: [&'static str; 3],
}

/// Une ligne de la table de syntaxe : ce qu'on écrit (du code, dont seuls les mots
/// d'exemple sont traduits) et son effet.
pub struct Ligne {
    pub code: &'static str,
    pub effet: &'static str,
}

/// Tous les textes du site dans une langue. Les noms propres (Varium, Documents,
/// Classeur, Présentation, les formats, les systèmes) et le code non traduisible restent
/// dans les pages.
pub struct Textes {
    // ---- Commun : en-tête, pied de page --------------------------------------------
    /// La devise de Varium : titre de l'accueil et grand titre.
    pub accroche: &'static str,
    /// Les entrées de la navigation, dans l'ordre de [`Rubrique::ALL`](crate::routes::Rubrique::ALL).
    pub nav: [&'static str; 4],
    pub nav_aria: &'static str,
    /// Nom accessible du logo, qui mène à l'accueil.
    pub accueil_aria: &'static str,
    /// « Choisir la langue » : le sélecteur, et la page qui aiguille vers une langue.
    pub langue_choisir: &'static str,
    pub installer_varium: &'static str,
    pub code_source: &'static str,
    pub pied: &'static str,
    /// `{}` : le lien vers Paquets.
    pub pied_paquets: &'static str,
    pub lire_syntaxe: &'static str,
    pub voir_exemple: &'static str,

    // ---- Accueil -------------------------------------------------------------------
    pub description_accueil: &'static str,
    pub surtitre: &'static str,
    pub chapo: &'static str,
    pub sous_hero: &'static str,
    /// « Le modèle », « Le document ».
    pub apercu: [&'static str; 2],
    pub apercu_note: &'static str,
    pub applis_titre: &'static str,
    pub applis_intro: &'static str,
    /// Documents, Classeur, Présentation.
    pub applis: [Appli; 3],
    pub langues_titre: &'static str,
    pub langues_intro: &'static str,
    /// Typographie, révision, formats.
    pub langues: [Item; 4],
    pub langues_note: &'static str,
    pub fonctions_titre: &'static str,
    pub fonctions_intro: &'static str,
    pub fonctions: [Item; 27],
    pub etapes_titre: &'static str,
    pub etapes: [Item; 3],
    pub bureau: Item,
    pub prive: Item,
    pub capot_titre: &'static str,
    pub capot_intro: &'static str,
    /// « Paquet », « Rôle ».
    pub capot_colonnes: [&'static str; 2],
    /// varium-core, varium-calc, varium-slides, varium.
    pub capot_roles: [&'static str; 4],
    pub appel_titre: &'static str,
    pub appel_texte: &'static str,

    // ---- Syntaxe -------------------------------------------------------------------
    pub syntaxe_description: &'static str,
    pub syntaxe_chapo: &'static str,
    /// Variables, Filtres, Types, Formules, Classeur, Exports, API.
    pub sommaire: [&'static str; 7],
    pub ecrire_titre: &'static str,
    pub ecrire_texte: &'static str,
    /// « Écriture », « Effet ».
    pub syntaxe_colonnes: [&'static str; 2],
    pub syntaxe: [Ligne; 22],
    pub filtres_titre: &'static str,
    pub filtres_texte: &'static str,
    /// « Filtre », « Valeur », « Résultat ».
    pub filtres_colonnes: [&'static str; 3],
    /// La valeur d'exemple du filtre `default` : une formule de politesse.
    pub filtre_defaut: &'static str,
    pub types_titre: &'static str,
    pub types_texte: &'static str,
    /// Dans l'ordre de `VarType::ALL` : texte, texte long, date, nombre, montant, e-mail.
    pub types: [&'static str; 6],
    pub formules_titre: &'static str,
    pub formules_texte: &'static str,
    /// Un exemple de texte avec des formules (du code, mots traduits).
    pub formules_exemple: &'static str,
    pub classeur_titre: &'static str,
    pub classeur_texte: &'static str,
    /// « Fonction (français · anglais) », « Rôle ».
    pub classeur_colonnes: [&'static str; 2],
    /// SOMME/SUM, SI/IF, RECHERCHEV/VLOOKUP.
    pub classeur_fonctions: [&'static str; 3],
    pub classeur_note: &'static str,
    pub exports_titre: &'static str,
    /// Les formats qui ont un nom commun : texte, modèle, projet.
    pub exports_noms: [&'static str; 3],
    /// PDF, DOCX, ODT, RTF, EPUB, HTML, LaTeX, puis texte, modèle, projet.
    pub exports: [&'static str; 10],
    pub api_titre: &'static str,
    pub api_intro_statique: &'static str,
    pub api_intro_serveur: &'static str,
    pub api_reponse: &'static str,

    // ---- Installer -----------------------------------------------------------------
    pub installer_description: &'static str,
    pub installer_chapo: &'static str,
    /// `{}` : le système.
    pub telecharger_pour: &'static str,
    /// Windows, macOS, Linux.
    pub systemes: [Systeme; 3],
    /// `{}` : le lien vers la page des versions.
    pub versions: &'static str,
    pub versions_lien: &'static str,
    pub source_titre: &'static str,
    pub source_texte: &'static str,
    pub source_linux: &'static str,
    pub source_github: &'static str,

    // ---- Essayer -------------------------------------------------------------------
    pub essayer_titre: &'static str,
    /// La description de la page : [serveur, export statique].
    pub essayer_description: [&'static str; 2],
    /// Le texte d'introduction : [serveur, export statique].
    pub essayer_explication: [&'static str; 2],
    /// L'encart de l'export statique : la phrase en gras, puis la suite.
    pub essayer_installer: Item,
    pub modele: &'static str,
    pub modele_aide: &'static str,
    pub lecture_seule: &'static str,
    /// `{}` : le nombre de variables.
    pub variables: &'static str,
    pub aucune_variable: &'static str,
    pub mettre_a_jour: &'static str,
    pub reinitialiser: &'static str,
    /// « Variable », « Type », « Valeur ».
    pub variables_colonnes: [&'static str; 3],
    pub a_remplir: &'static str,
    pub document: &'static str,
    /// `{}` : un nombre. Variable au singulier et au pluriel, occurrence de même.
    pub compteur: [&'static str; 4],
    /// `{}` : le nombre de variables sans valeur.
    pub a_remplir_n: &'static str,
    pub texte_brut: &'static str,
    /// `{}` : le nom de la variable.
    pub sans_valeur: &'static str,
    pub saut_de_page: &'static str,
    /// `{}` : le nom de la variable.
    pub type_de: &'static str,
    /// Le format attendu d'une date : AAAA-MM-JJ.
    pub format_date: &'static str,
    /// Les erreurs de saisie : date, nombre, e-mail.
    pub erreurs: [&'static str; 3],
    /// `{}` : la limite, en caractères.
    pub tronque: &'static str,

    // ---- Introuvable ---------------------------------------------------------------
    pub introuvable_titre: &'static str,
    pub introuvable_description: &'static str,
    pub introuvable_h1: &'static str,
    pub introuvable_texte: &'static str,
    pub retour_accueil: &'static str,
}

/// La nature d'un texte, pour les tests : la prose est traduite, le code ne l'est
/// qu'en partie (les mots d'exemple) et garde sa syntaxe.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sorte {
    Texte,
    Code,
}

impl Textes {
    /// Tous les textes de la langue, dans un ordre fixe. La déstructuration n'a pas de
    /// `..` : un champ ajouté sans être listé ici ne compile pas.
    pub fn chaines(&self) -> Vec<(Sorte, &'static str)> {
        let Textes {
            accroche,
            nav,
            nav_aria,
            accueil_aria,
            langue_choisir,
            installer_varium,
            code_source,
            pied,
            pied_paquets,
            lire_syntaxe,
            voir_exemple,
            description_accueil,
            surtitre,
            chapo,
            sous_hero,
            apercu,
            apercu_note,
            applis_titre,
            applis_intro,
            applis,
            langues_titre,
            langues_intro,
            langues,
            langues_note,
            fonctions_titre,
            fonctions_intro,
            fonctions,
            etapes_titre,
            etapes,
            bureau,
            prive,
            capot_titre,
            capot_intro,
            capot_colonnes,
            capot_roles,
            appel_titre,
            appel_texte,
            syntaxe_description,
            syntaxe_chapo,
            sommaire,
            ecrire_titre,
            ecrire_texte,
            syntaxe_colonnes,
            syntaxe,
            filtres_titre,
            filtres_texte,
            filtres_colonnes,
            filtre_defaut,
            types_titre,
            types_texte,
            types,
            formules_titre,
            formules_texte,
            formules_exemple,
            classeur_titre,
            classeur_texte,
            classeur_colonnes,
            classeur_fonctions,
            classeur_note,
            exports_titre,
            exports_noms,
            exports,
            api_titre,
            api_intro_statique,
            api_intro_serveur,
            api_reponse,
            installer_description,
            installer_chapo,
            telecharger_pour,
            systemes,
            versions,
            versions_lien,
            source_titre,
            source_texte,
            source_linux,
            source_github,
            essayer_titre,
            essayer_description,
            essayer_explication,
            essayer_installer,
            modele,
            modele_aide,
            lecture_seule,
            variables,
            aucune_variable,
            mettre_a_jour,
            reinitialiser,
            variables_colonnes,
            a_remplir,
            document,
            compteur,
            a_remplir_n,
            texte_brut,
            sans_valeur,
            saut_de_page,
            type_de,
            format_date,
            erreurs,
            tronque,
            introuvable_titre,
            introuvable_description,
            introuvable_h1,
            introuvable_texte,
            retour_accueil,
        } = self;

        let mut v: Vec<&'static str> = vec![
            accroche,
            nav_aria,
            accueil_aria,
            langue_choisir,
            installer_varium,
            code_source,
            pied,
            pied_paquets,
            lire_syntaxe,
            voir_exemple,
            description_accueil,
            surtitre,
            chapo,
            sous_hero,
            apercu_note,
            applis_titre,
            applis_intro,
            langues_titre,
            langues_intro,
            langues_note,
            fonctions_titre,
            fonctions_intro,
            etapes_titre,
            capot_titre,
            capot_intro,
            appel_titre,
            appel_texte,
            syntaxe_description,
            syntaxe_chapo,
            ecrire_titre,
            ecrire_texte,
            filtres_titre,
            filtres_texte,
            filtre_defaut,
            types_titre,
            types_texte,
            formules_titre,
            formules_texte,
            classeur_titre,
            classeur_texte,
            classeur_note,
            exports_titre,
            api_titre,
            api_intro_statique,
            api_intro_serveur,
            api_reponse,
            installer_description,
            installer_chapo,
            telecharger_pour,
            versions,
            versions_lien,
            source_titre,
            source_texte,
            source_linux,
            source_github,
            essayer_titre,
            modele,
            modele_aide,
            lecture_seule,
            variables,
            aucune_variable,
            mettre_a_jour,
            reinitialiser,
            a_remplir,
            document,
            a_remplir_n,
            texte_brut,
            sans_valeur,
            saut_de_page,
            type_de,
            format_date,
            tronque,
            introuvable_titre,
            introuvable_description,
            introuvable_h1,
            introuvable_texte,
            retour_accueil,
        ];
        v.extend(nav);
        v.extend(apercu);
        for a in applis {
            v.extend([a.genre, a.texte]);
            v.extend(a.points);
        }
        for item in langues.iter().chain(fonctions).chain(etapes) {
            v.extend([item.titre, item.texte]);
        }
        for item in [bureau, prive, essayer_installer] {
            v.extend([item.titre, item.texte]);
        }
        v.extend(capot_colonnes);
        v.extend(capot_roles);
        v.extend(sommaire);
        v.extend(syntaxe_colonnes);
        v.extend(syntaxe.iter().map(|l| l.effet));
        v.extend(filtres_colonnes);
        v.extend(types);
        v.extend(classeur_colonnes);
        v.extend(classeur_fonctions);
        v.extend(exports_noms);
        v.extend(exports);
        for s in systemes {
            v.push(s.compat);
            v.extend(s.etapes);
        }
        v.extend(essayer_description);
        v.extend(essayer_explication);
        v.extend(variables_colonnes);
        v.extend(compteur);
        v.extend(erreurs);

        let mut chaines: Vec<(Sorte, &'static str)> =
            v.into_iter().map(|s| (Sorte::Texte, s)).collect();
        chaines.extend(syntaxe.iter().map(|l| (Sorte::Code, l.code)));
        chaines.push((Sorte::Code, formules_exemple));
        chaines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les textes qui peuvent rester identiques au français dans une autre langue : des
    /// mots transparents (Variables, Document, Date…), des sigles et des noms propres.
    /// Tout autre texte identique au français est une traduction oubliée.
    const IDENTIQUES: &[&str] = &[
        "API",
        "Date",
        "Document",
        "E-mail",
        "Exports",
        "Type",
        "Types",
        "Variable",
        "Variables",
        "Variables ({})",
        "{} occurrence",
        "{} occurrences",
        "{} variable",
        "{} variables",
    ];

    #[test]
    fn chaque_langue_a_tous_ses_textes() {
        let source = Langue::Fr.textes().chaines();
        for langue in Langue::ALL {
            let chaines = langue.textes().chaines();
            assert_eq!(chaines.len(), source.len(), "{}", langue.code());
            for (i, ((sorte, texte), (sorte_fr, _))) in chaines.iter().zip(&source).enumerate() {
                assert_eq!(sorte, sorte_fr, "{} #{i}", langue.code());
                assert!(
                    !texte.trim().is_empty(),
                    "{} : texte #{i} vide",
                    langue.code()
                );
                assert_eq!(
                    texte.trim(),
                    *texte,
                    "{} : espaces autour de « {texte} »",
                    langue.code()
                );
            }
        }
    }

    #[test]
    fn les_langues_sont_vraiment_traduites() {
        let source = Langue::Fr.textes().chaines();
        for langue in Langue::ALL.into_iter().skip(1) {
            let oublis: Vec<&str> = langue
                .textes()
                .chaines()
                .iter()
                .zip(&source)
                .filter(|((sorte, texte), (_, fr))| {
                    *sorte == Sorte::Texte && texte == fr && !IDENTIQUES.contains(texte)
                })
                .map(|((_, texte), _)| *texte)
                .collect();
            assert!(
                oublis.is_empty(),
                "{} : textes restés en français : {oublis:?}",
                langue.code()
            );
        }
        // La liste ne garde que des textes qui existent bien en français.
        for permis in IDENTIQUES {
            assert!(
                source.iter().any(|(_, fr)| fr == permis),
                "« {permis} » n'est plus un texte français : retirez-le de IDENTIQUES"
            );
        }
    }

    /// Les filtres et mots-clés de chaque `{{…}}` : `{{ nom | upper }}` donne `["_|upper"]`,
    /// `{{si remise}}` donne `["si"]`, `{{= a * b | money}}` donne `["=|money"]`.
    fn champs(texte: &str) -> Vec<String> {
        let mut trouves = Vec::new();
        let mut reste = texte;
        while let Some(debut) = reste.find("{{") {
            let apres = &reste[debut + 2..];
            let Some(fin) = apres.find("}}") else { break };
            let interieur = apres[..fin].trim();
            let mut morceaux = interieur.split('|');
            let tete = morceaux.next().unwrap_or("").trim();
            let premier = tete.split_whitespace().next().unwrap_or("");
            let mut signature = match premier {
                "si" | "sinon" | "fin" | "aujourdhui" => premier.to_string(),
                _ if tete.starts_with('=') => "=".to_string(),
                _ => "_".to_string(),
            };
            for filtre in morceaux {
                let nom = filtre.trim().split(':').next().unwrap_or("").trim();
                signature.push('|');
                signature.push_str(nom);
            }
            trouves.push(signature);
            reste = &apres[fin + 2..];
        }
        trouves
    }

    /// Le nombre de chaque marque qui doit survivre à la traduction.
    fn marques(texte: &str, sorte: Sorte) -> Vec<(&'static str, usize)> {
        let mut liste = vec!["{}", "{{", "}}", "$", "→", "…"];
        if sorte == Sorte::Code {
            liste.extend(["|", "*", "_", "~", "#", "<", ">", "\\", "=", "\n"]);
        }
        liste
            .into_iter()
            .map(|m| (m, texte.matches(m).count()))
            .collect()
    }

    #[test]
    fn les_marques_sont_les_memes_partout() {
        let source = Langue::Fr.textes().chaines();
        for langue in Langue::ALL.into_iter().skip(1) {
            for ((sorte, texte), (_, fr)) in langue.textes().chaines().iter().zip(&source) {
                assert_eq!(
                    marques(texte, *sorte),
                    marques(fr, *sorte),
                    "{} : « {texte} » ne garde pas les marques de « {fr} »",
                    langue.code()
                );
                assert_eq!(
                    champs(texte),
                    champs(fr),
                    "{} : « {texte} » change les champs de « {fr} »",
                    langue.code()
                );
            }
        }
    }

    #[test]
    fn la_revision_cite_les_memes_exemples() {
        for langue in Langue::ALL {
            let t = langue.textes();
            let revision = t.langues[1].texte;
            for exemple in ["¿", "¡"] {
                assert!(
                    revision.contains(exemple),
                    "{} : {exemple:?}",
                    langue.code()
                );
            }
            // « a » et « an », entre les guillemets de la langue.
            assert!(
                ["an\u{a0}»", "an»", "an”", "an“"]
                    .iter()
                    .any(|an| revision.contains(an)),
                "{}",
                langue.code()
            );
            // Les exemples de typographie ne se coupent pas en fin de ligne.
            let typographie = t.langues[0].texte;
            for guillemets in ["«\u{a0}»", "“\u{a0}”", "„\u{a0}“", ";\u{a0}:\u{a0}!\u{a0}?"]
            {
                assert!(
                    typographie.contains(guillemets),
                    "{} : {guillemets}",
                    langue.code()
                );
            }
        }
    }

    #[test]
    fn analyse_des_champs() {
        assert_eq!(champs("{{ nom | upper | trim }}"), ["_|upper|trim"]);
        assert_eq!(
            champs("{{si remise}}…{{sinon}}…{{fin}}"),
            ["si", "sinon", "fin"]
        );
        assert_eq!(champs("{{= quantité * prix | money}}"), ["=|money"]);
        assert_eq!(champs("{{t | default:\"texte\"}}"), ["_|default"]);
        assert_eq!(champs("rien"), Vec::<String>::new());
    }

    #[test]
    fn codes_noms_et_locales() {
        assert_eq!(
            Langue::ALL.map(Langue::code),
            ["fr", "en", "es", "de", "it", "pt"]
        );
        for langue in Langue::ALL {
            assert_eq!(Langue::depuis_code(langue.code()), Some(langue));
            assert!(langue.locale().starts_with(langue.code()));
        }
        assert_eq!(Langue::De.nom(), "Deutsch");
        assert_eq!(Langue::depuis_code("EN"), None);
        assert_eq!(Langue::depuis_code("ja"), None);
        assert_eq!(Langue::depuis_etiquette("pt-BR"), Some(Langue::Pt));
        assert_eq!(Langue::depuis_etiquette("ES"), Some(Langue::Es));
        assert_eq!(Langue::depuis_etiquette("en_GB"), Some(Langue::En));
        assert_eq!(Langue::depuis_etiquette("*"), None);
        assert_eq!(Langue::depuis_etiquette(""), None);
    }

    #[test]
    fn choisit_la_langue_du_navigateur() {
        assert_eq!(langue_preferee("de-DE,de;q=0.9,en;q=0.8"), Langue::De);
        assert_eq!(langue_preferee("ja, it;q=0.5, en;q=0.7"), Langue::En);
        assert_eq!(langue_preferee("pt-BR"), Langue::Pt);
        assert_eq!(langue_preferee("es-419, es; q=0.9"), Langue::Es);
        // À qualité égale, l'ordre du navigateur.
        assert_eq!(langue_preferee("it, en"), Langue::It);
        assert_eq!(langue_preferee("en;q=0, de"), Langue::De);
        // Aucune langue du site : le français.
        assert_eq!(langue_preferee("ja, zh-CN"), Langue::Fr);
        assert_eq!(langue_preferee("*"), Langue::Fr);
        assert_eq!(langue_preferee(""), Langue::Fr);
    }

    #[test]
    fn pluriels_et_modeles() {
        assert!(!Langue::Fr.pluriel(0) && !Langue::Fr.pluriel(1) && Langue::Fr.pluriel(2));
        assert!(Langue::En.pluriel(0) && !Langue::En.pluriel(1) && Langue::De.pluriel(3));
        assert_eq!(remplir("{} sur {}", &["1", "2"]), "1 sur 2");
        assert_eq!(remplir("Type de {}", &["nom"]), "Type de nom");
        assert_eq!(remplir("sans", &["x"]), "sans");
    }
}
