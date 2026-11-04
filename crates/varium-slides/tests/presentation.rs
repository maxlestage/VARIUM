//! Modèle de présentation : fichier JSON, opérations sur les diapositives, champs,
//! géométrie des dispositions, thèmes.

use varium_slides::{
    Aspect, Deck, FORMAT, Frame, Layout, Slide, Theme, Transition, VERSION, body_size, boxes,
    footer_frame, number_frame, subtitle_size, title_size,
};

fn titres(deck: &Deck) -> Vec<&str> {
    deck.slides.iter().map(|s| s.title.as_str()).collect()
}

fn numerotee(n: usize) -> Deck {
    let mut deck = Deck::new();
    deck.slides = (0..n)
        .map(|i| Slide {
            title: format!("D{i}"),
            ..Slide::default()
        })
        .collect();
    deck
}

// ---------------------------------------------------------------------------------
// Fichier JSON
// ---------------------------------------------------------------------------------

#[test]
fn aller_retour_json_de_l_exemple() {
    let mut deck = Deck::sample();
    deck.slides[1].background = Some([12, 200, 7]);
    deck.aspect = Aspect::Standard;
    deck.slide_numbers = false;
    let json = deck.to_json();
    assert!(json.contains("\"format\": \"varium-slides\""));
    assert!(json.contains("\"version\": 1"));
    assert!(
        json.contains("\"#0cc807\""),
        "couleur écrite en hexadécimal"
    );
    assert!(json.contains("\"title_content\""));
    let relu = Deck::from_json(&json).expect("relecture");
    assert_eq!(relu, deck);
    // Écriture stable : relire puis réécrire donne le même fichier.
    assert_eq!(relu.to_json(), json);
}

#[test]
fn format_et_version_du_fichier() {
    assert_eq!(FORMAT, "varium-slides");
    assert_eq!(VERSION, 1);
    let json = Deck::new().to_json();
    let debut = json.find("\"format\"").unwrap();
    assert!(debut < json.find("\"slides\"").unwrap(), "format en tête");
}

#[test]
fn champs_absents_ou_anciens_prennent_leur_valeur_par_defaut() {
    // Fichier minimal : aucune diapositive, aucun réglage.
    let deck = Deck::from_json(r#"{"format":"varium-slides","version":1}"#).unwrap();
    assert_eq!(deck.slides.len(), 1, "une diapositive au moins");
    assert_eq!(deck.theme, Theme::Clair);
    assert_eq!(deck.aspect, Aspect::Large);
    assert!(deck.slide_numbers);
    assert!(deck.values.is_empty());

    // Diapositive partielle, valeurs de types variés, champs inconnus ignorés.
    let json = r##"{
        "title": "Ancien",
        "theme": "sombre",
        "futur": {"x": 1},
        "values": {"n": 42, "ok": true, "liste": [1, 2], "nom": "Zoé", "rien": null},
        "slides": [
            {"title": "Seul titre"},
            {"layout": "two_columns", "body2": "B", "transition": "glissement", "background": "#abc"},
            "pas une diapositive",
            {"layout": "inconnue", "transition": 3, "background": "rouge", "notes": null},
            {"background": [255, 0, 300]}
        ]
    }"##;
    let deck = Deck::from_json(json).unwrap();
    assert_eq!(deck.title, "Ancien");
    assert_eq!(deck.theme, Theme::Sombre);
    assert_eq!(deck.values.get("n").map(String::as_str), Some("42"));
    assert_eq!(deck.values.get("ok").map(String::as_str), Some("true"));
    assert_eq!(deck.values.get("nom").map(String::as_str), Some("Zoé"));
    assert!(!deck.values.contains_key("liste") && !deck.values.contains_key("rien"));
    assert_eq!(
        deck.slides.len(),
        4,
        "l'élément qui n'est pas un objet est ignoré"
    );
    assert_eq!(deck.slides[0].layout, Layout::TitleContent);
    assert_eq!(deck.slides[0].transition, Transition::Aucune);
    assert_eq!(deck.slides[0].background, None);
    assert_eq!(deck.slides[1].layout, Layout::TwoColumns);
    assert_eq!(deck.slides[1].transition, Transition::Glissement);
    assert_eq!(deck.slides[1].background, Some([170, 187, 204]));
    assert_eq!(
        deck.slides[2].layout,
        Layout::TitleContent,
        "valeur inconnue"
    );
    assert_eq!(deck.slides[2].notes, "");
    assert_eq!(deck.slides[2].background, None);
    assert_eq!(deck.slides[3].background, Some([255, 0, 255]));
}

#[test]
fn identifiants_tolerants() {
    assert_eq!(Layout::from_id("TitleContent"), Some(Layout::TitleContent));
    assert_eq!(Layout::from_id("title-only"), Some(Layout::TitleOnly));
    assert_eq!(Theme::from_id("OCEAN"), Some(Theme::Ocean));
    assert_eq!(Theme::from_id("Océan"), None);
    for l in Layout::ALL {
        assert_eq!(Layout::from_id(l.id()), Some(l));
    }
    for t in Theme::ALL {
        assert_eq!(Theme::from_id(t.id()), Some(t));
    }
    for t in Transition::ALL {
        assert_eq!(Transition::from_id(t.id()), Some(t));
    }
    for a in Aspect::ALL {
        assert_eq!(Aspect::from_id(a.id()), Some(a));
    }
}

#[test]
fn fichiers_refuses_avec_un_message_clair() {
    let erreur = |s: &str| Deck::from_json(s).unwrap_err();
    assert!(erreur("").starts_with("Présentation illisible"));
    assert!(erreur("[1, 2]").contains("objet JSON"));
    assert!(erreur("{\"format\": \"autre\"}").contains("Format inconnu"));
    assert!(erreur("{\"format\": 7}").contains("Format inconnu"));
    assert!(erreur("{\"format\": \"varium\", \"source\": \"x\"}").contains("document Varium"));
    assert!(erreur("{\"format\": \"varium-slides\", \"version\": 2}").contains("plus récente"));
    // Imbrication très profonde : erreur, pas de dépassement de pile.
    let profond = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
    assert!(Deck::from_json(&profond).is_err());
}

// ---------------------------------------------------------------------------------
// Opérations
// ---------------------------------------------------------------------------------

#[test]
fn ajouter_une_diapositive() {
    let mut deck = numerotee(3);
    deck.slides[0].transition = Transition::Zoom;
    let i = deck.add_slide(0, Layout::Quote);
    assert_eq!(i, 1);
    assert_eq!(deck.slides[1].layout, Layout::Quote);
    assert_eq!(
        deck.slides[1].transition,
        Transition::Zoom,
        "reprend la voisine"
    );
    assert_eq!(deck.slides[1].title, "");
    // Position au-delà de la fin : ajout à la fin.
    let j = deck.add_slide(usize::MAX, Layout::Blank);
    assert_eq!(j, deck.slides.len() - 1);
    assert_eq!(deck.slides[j].layout, Layout::Blank);
    // Présentation vide (construite à la main) : première diapositive.
    let mut vide = Deck {
        slides: Vec::new(),
        ..Deck::new()
    };
    assert_eq!(vide.add_slide(5, Layout::Title), 0);
    assert_eq!(vide.slides.len(), 1);
}

#[test]
fn dupliquer_une_diapositive() {
    let mut deck = numerotee(3);
    assert_eq!(deck.duplicate(1), 2);
    assert_eq!(titres(&deck), ["D0", "D1", "D1", "D2"]);
    assert_eq!(deck.duplicate(99), 4, "bornée à la dernière");
    assert_eq!(titres(&deck), ["D0", "D1", "D1", "D2", "D2"]);
    let mut vide = Deck {
        slides: Vec::new(),
        ..Deck::new()
    };
    assert_eq!(vide.duplicate(0), 0);
    assert_eq!(vide.slides.len(), 1);
}

#[test]
fn supprimer_garde_toujours_une_diapositive() {
    let mut deck = numerotee(3);
    deck.remove(1);
    assert_eq!(titres(&deck), ["D0", "D2"]);
    deck.remove(42);
    assert_eq!(titres(&deck), ["D0"], "indice borné à la dernière");
    deck.slides[0].layout = Layout::Section;
    deck.remove(0);
    assert_eq!(deck.slides.len(), 1);
    assert_eq!(
        deck.slides[0],
        Slide::new(Layout::Section),
        "vidée, pas supprimée"
    );
    let mut vide = Deck {
        slides: Vec::new(),
        ..Deck::new()
    };
    vide.remove(0);
    assert_eq!(vide.slides.len(), 1);
}

#[test]
fn deplacer_une_diapositive() {
    let mut deck = numerotee(4);
    deck.move_slide(0, 2);
    assert_eq!(titres(&deck), ["D1", "D2", "D0", "D3"]);
    deck.move_slide(3, 0);
    assert_eq!(titres(&deck), ["D3", "D1", "D2", "D0"]);
    deck.move_slide(1, 100);
    assert_eq!(
        titres(&deck),
        ["D3", "D2", "D0", "D1"],
        "destination bornée"
    );
    deck.move_slide(100, 0);
    assert_eq!(titres(&deck), ["D1", "D3", "D2", "D0"], "origine bornée");
    deck.move_slide(2, 2);
    assert_eq!(titres(&deck), ["D1", "D3", "D2", "D0"]);
    let mut vide = Deck {
        slides: Vec::new(),
        ..Deck::new()
    };
    vide.move_slide(0, 1);
    assert!(vide.slides.is_empty());
}

#[test]
fn nouvelle_presentation() {
    let deck = Deck::new();
    assert_eq!(deck, Deck::default());
    assert_eq!(deck.slides.len(), 1);
    assert_eq!(deck.slides[0].layout, Layout::Title);
    assert!(deck.variables().is_empty());
}

// ---------------------------------------------------------------------------------
// Champs
// ---------------------------------------------------------------------------------

#[test]
fn champs_de_toute_la_presentation() {
    let mut deck = numerotee(2);
    deck.slides[0].title = "Bonjour {{client}}".into();
    deck.slides[0].subtitle = "{{date | date:full}}".into();
    deck.slides[0].body = "- {{client}} et {{= prix * quantité | money}}\n\\{{littéral}}".into();
    deck.slides[1].body2 = "{{si remise}}Remise {{remise}}{{fin}}".into();
    deck.slides[1].notes = "Penser à {{orateur}}".into();
    deck.footer = "{{entreprise}} — {{client}}".into();
    assert_eq!(
        deck.variables(),
        [
            "client",
            "date",
            "prix",
            "quantité",
            "remise",
            "orateur",
            "entreprise"
        ]
    );
    deck.values.insert("client".into(), "Dupont".into());
    assert_eq!(deck.values().get("client"), "Dupont");
}

#[test]
fn l_exemple_emploie_toutes_les_dispositions() {
    let deck = Deck::sample();
    assert!((6..=8).contains(&deck.slides.len()));
    for l in Layout::ALL {
        assert!(
            deck.slides.iter().any(|s| s.layout == l),
            "disposition {} absente",
            l.label()
        );
    }
    assert!(deck.slides.iter().any(|s| s.body.contains("|---|")));
    let vars = deck.variables();
    assert!(vars.contains(&"entreprise".to_string()));
    for v in &vars {
        assert!(deck.values.contains_key(v), "champ {v} sans valeur");
    }
    assert!(deck.slides.iter().all(|s| !s.notes.is_empty()));
}

#[test]
fn libelle_des_diapositives() {
    let mut s = Slide::new(Layout::Quote);
    assert_eq!(s.label(), "");
    s.body = "\n**Être** ou ne pas être\nsuite".into();
    assert_eq!(s.label(), "Être ou ne pas être");
    s.title = "  Titre   {{x}}  ".into();
    assert_eq!(s.label(), "Titre {{x}}");
    s.title = "a".repeat(200);
    assert_eq!(s.label().chars().count(), 60);
}

// ---------------------------------------------------------------------------------
// Géométrie
// ---------------------------------------------------------------------------------

fn dans_la_diapositive(f: &Frame) -> bool {
    f.x >= 0.0 && f.y >= 0.0 && f.w > 0.0 && f.h > 0.0 && f.right() <= 1.0 && f.bottom() <= 1.0
}

#[test]
fn zones_dans_la_diapositive_et_sans_chevauchement() {
    for l in Layout::ALL {
        let b = boxes(l);
        let zones: Vec<(&str, Frame)> = [
            ("titre", b.title),
            ("sous-titre", b.subtitle),
            ("contenu", b.body),
            ("colonne 2", b.body2),
            ("filet", b.accent),
        ]
        .into_iter()
        .filter_map(|(n, f)| f.map(|f| (n, f)))
        .collect();
        for (n, f) in &zones {
            assert!(dans_la_diapositive(f), "{n} de {} hors cadre", l.label());
        }
        for (i, (na, a)) in zones.iter().enumerate() {
            for (nb, z) in &zones[i + 1..] {
                assert!(
                    !a.overlaps(z),
                    "{na} et {nb} se chevauchent ({})",
                    l.label()
                );
            }
            if b.footer {
                assert!(
                    !a.overlaps(&footer_frame()) && !a.overlaps(&number_frame()),
                    "{na} chevauche le pied de page ({})",
                    l.label()
                );
            }
        }
        assert!(
            title_size(l) > body_size(l),
            "titre plus grand ({})",
            l.label()
        );
        for t in [title_size(l), body_size(l), subtitle_size(l)] {
            assert!(t > 0.01 && t < 0.2);
        }
    }
    assert!(dans_la_diapositive(&footer_frame()) && dans_la_diapositive(&number_frame()));
    assert!(!footer_frame().overlaps(&number_frame()));
    // Ce que chaque disposition montre.
    assert!(boxes(Layout::Title).subtitle.is_some() && boxes(Layout::Title).body.is_none());
    assert!(boxes(Layout::TitleContent).body.is_some());
    assert!(boxes(Layout::TwoColumns).body2.is_some());
    assert!(boxes(Layout::Quote).title.is_none() && boxes(Layout::Quote).body.is_some());
    assert!(boxes(Layout::TitleOnly).body.is_none());
    assert!(boxes(Layout::Blank).title.is_none());
}

#[test]
fn libelles_en_francais() {
    let dispositions: Vec<&str> = Layout::ALL.iter().map(|l| l.label()).collect();
    assert_eq!(
        dispositions,
        [
            "Titre",
            "Titre et contenu",
            "Deux colonnes",
            "Section",
            "Citation",
            "Titre seul",
            "Vide"
        ]
    );
    let themes: Vec<&str> = Theme::ALL.iter().map(|t| t.label()).collect();
    assert_eq!(
        themes,
        [
            "Clair",
            "Sombre",
            "Océan",
            "Élégant",
            "Craie",
            "Corail",
            "Papier",
            "Minuit",
            "Forêt",
            "Agrume"
        ]
    );
    let transitions: Vec<&str> = Transition::ALL.iter().map(|t| t.label()).collect();
    assert_eq!(transitions, ["Aucune", "Fondu", "Glissement", "Zoom"]);
}

#[test]
fn formats_des_diapositives() {
    assert!((Aspect::Large.ratio() - 16.0 / 9.0).abs() < 1e-6);
    assert!((Aspect::Standard.ratio() - 4.0 / 3.0).abs() < 1e-6);
    let (w, h) = Aspect::Large.size_mm();
    assert!((w - 254.0).abs() < 1e-3 && (h - 142.875).abs() < 1e-3);
    assert!((Aspect::Standard.size_mm().1 - 190.5).abs() < 1e-3);
}

#[test]
fn fond_propre_et_palette() {
    let mut deck = Deck::sample();
    deck.theme = Theme::Clair;
    let clair = deck.slide_palette(1);
    assert_eq!(clair, Theme::Clair.palette());
    deck.slides[1].background = Some([10, 10, 10]);
    let p = deck.slide_palette(1);
    assert_eq!(p.background, [10, 10, 10]);
    assert_ne!(p.text, clair.text, "texte éclairci sur fond noir");
    assert_eq!(
        deck.slide_palette(999),
        deck.slides.last().unwrap().palette(Theme::Clair)
    );
    assert!(Theme::Elegant.palette().serif_titles && !Theme::Clair.palette().serif_titles);
}
