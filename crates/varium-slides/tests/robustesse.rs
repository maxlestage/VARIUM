//! Fuzz déterministe : présentations aléatoires (syntaxe, Unicode, indices extrêmes,
//! fichiers abîmés). Aucun export ne doit paniquer, et chaque fichier produit doit
//! rester bien formé.

mod commun;

use commun::{compte, xml_bien_forme, zip_entries};
use varium_slides::{Aspect, Deck, Layout, Slide, Theme, Transition};

/// Générateur pseudo-aléatoire (SplitMix64) : mêmes tirages à chaque exécution.
struct Alea(u64);

impl Alea {
    fn suivant(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn sous(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.suivant() % n as u64) as usize
        }
    }

    fn pile(&mut self) -> bool {
        self.suivant() & 1 == 0
    }

    fn choix<'a, T>(&mut self, t: &'a [T]) -> &'a T {
        &t[self.sous(t.len())]
    }

    /// Un indice quelconque, souvent hors limites.
    fn indice(&mut self, n: usize) -> usize {
        match self.sous(5) {
            0 => usize::MAX,
            1 => n + self.sous(3),
            _ => self.sous(n + 1),
        }
    }
}

const MORCEAUX: &[&str] = &[
    "**",
    "*",
    "__",
    "~~",
    "# ",
    "## ",
    "### ",
    "- ",
    "  - ",
    "      - ",
    "1. ",
    "12) ",
    "> ",
    "-> ",
    " <-",
    " ->",
    "| ",
    " | ",
    " |",
    "\n|---|:---:|---:|\n",
    "\n",
    "\n\n",
    "{{",
    "}}",
    "{{entreprise}}",
    "{{absent}}",
    "{{ville | upper}}",
    "{{= a * b}}",
    "{{= 1 / 0}}",
    "{{si x}}",
    "{{si non x}}",
    "{{sinon}}",
    "{{fin}}",
    "\\{{x}}",
    "\\",
    "$",
    "$$",
    "$x^2$",
    "<script>alert(1)</script>",
    "</section>",
    "]]>",
    "&",
    "&amp;",
    "\"",
    "'",
    "é",
    "€",
    "œ",
    "«\u{202F}",
    "😀",
    "👩‍👩‍👧",
    "漢字",
    "e\u{0301}",
    "\u{200B}",
    "\u{0}",
    "\u{1}",
    "\u{7}",
    "\u{FFFE}",
    "\t",
    "\r\n",
    "عربى",
    " ",
    "   ",
    "mot",
    "Lorem ipsum dolor sit amet",
    "---",
    "\\newpage",
    "%PDF",
    "(parenthèse)",
];

fn chaine(a: &mut Alea, max: usize) -> String {
    let mut s = String::new();
    for _ in 0..a.sous(max + 1) {
        match a.sous(12) {
            0 => {
                // Caractère Unicode quelconque.
                let c = char::from_u32(a.suivant() as u32 % 0x11_0000).unwrap_or('?');
                s.push(c);
            }
            1 => {
                // Mot démesuré.
                let c = *a.choix(&['x', 'é', '漢', '€']);
                s.extend(std::iter::repeat_n(c, 50 + a.sous(400)));
            }
            _ => s.push_str(a.choix(MORCEAUX)),
        }
    }
    s
}

fn diapositive(a: &mut Alea) -> Slide {
    Slide {
        layout: *a.choix(&Layout::ALL),
        title: chaine(a, 6),
        subtitle: chaine(a, 6),
        body: chaine(a, 40),
        body2: chaine(a, 20),
        notes: chaine(a, 10),
        transition: *a.choix(&Transition::ALL),
        background: a
            .pile()
            .then(|| [a.suivant() as u8, a.suivant() as u8, a.suivant() as u8]),
    }
}

fn presentation(a: &mut Alea) -> Deck {
    let mut deck = Deck {
        title: chaine(a, 4),
        theme: *a.choix(&Theme::ALL),
        aspect: *a.choix(&Aspect::ALL),
        footer: chaine(a, 5),
        slide_numbers: a.pile(),
        values: Default::default(),
        slides: (0..a.sous(9)).map(|_| diapositive(a)).collect(),
    };
    for nom in ["entreprise", "ville", "a", "b", "x", "absent"] {
        if a.pile() {
            deck.values.insert(nom.into(), chaine(a, 4));
        }
    }
    deck
}

/// Vérifie les trois exports d'une présentation.
fn verifier(deck: &Deck) {
    let attendu = deck.slides.len().max(1);

    let html = deck.to_html();
    assert_eq!(compte(&html, "<section class=\"slide"), attendu);
    assert_eq!(compte(&html, "<script>"), 1, "script injecté");
    assert_eq!(compte(&html, "</section>"), attendu, "balise injectée");

    let odp = deck.to_odp();
    let fichiers = zip_entries(&odp);
    assert_eq!(fichiers.len(), 5);
    assert_eq!(fichiers[0].0, "mimetype");
    for (nom, contenu) in &fichiers[1..] {
        let xml = std::str::from_utf8(contenu).expect("UTF-8");
        if let Err(e) = xml_bien_forme(xml) {
            panic!("{nom} mal formé : {e}");
        }
    }
    let content = std::str::from_utf8(&fichiers[1].1).unwrap();
    assert_eq!(compte(content, "<draw:page "), attendu);

    let pdf = deck.to_pdf();
    assert!(pdf.starts_with(b"%PDF-1.4\n") && pdf.ends_with(b"%%EOF\n"));
    let texte = String::from_utf8_lossy(&pdf);
    assert_eq!(compte(&texte, "/Type /Page "), attendu);
    assert!(!texte.contains("NaN") && !texte.contains("inf"));

    let _ = deck.variables();
    for i in 0..deck.slides.len() {
        let _ = deck.slides[i].label();
        let _ = deck.slide_palette(i);
    }
}

#[test]
fn fuzz_presentations_aleatoires_sans_panique() {
    let mut a = Alea(0x5EED_D1A9);
    for tour in 0..300 {
        let mut deck = presentation(&mut a);
        // Opérations aux indices quelconques.
        for _ in 0..a.sous(12) {
            let n = deck.slides.len();
            match a.sous(4) {
                0 => {
                    let i = deck.add_slide(a.indice(n), *a.choix(&Layout::ALL));
                    assert!(i < deck.slides.len());
                }
                1 => {
                    let i = deck.duplicate(a.indice(n));
                    assert!(i < deck.slides.len());
                }
                2 => {
                    deck.remove(a.indice(n));
                    assert!(!deck.slides.is_empty());
                }
                _ => {
                    deck.move_slide(a.indice(n), a.indice(n));
                    assert_eq!(deck.slides.len(), n, "tour {tour}");
                }
            }
        }
        verifier(&deck);
        // Aller-retour JSON exact, quel que soit le contenu.
        let relu = Deck::from_json(&deck.to_json()).expect("relecture");
        if deck.slides.is_empty() {
            assert_eq!(relu.slides.len(), 1);
        } else {
            assert_eq!(relu, deck, "tour {tour}");
        }
    }
}

#[test]
fn fuzz_fichiers_abimes_sans_panique() {
    let mut a = Alea(0xF11E_0BAD);
    let modele = Deck::sample().to_json();
    for _ in 0..300 {
        let mut octets = modele.clone().into_bytes();
        for _ in 0..1 + a.sous(6) {
            if octets.is_empty() {
                break;
            }
            let i = a.sous(octets.len());
            match a.sous(3) {
                0 => {
                    octets.remove(i);
                }
                1 => octets.insert(i, *a.choix(b"{}[]\",:0n-\\e")),
                _ => octets.truncate(i),
            }
        }
        let texte = String::from_utf8_lossy(&octets);
        if let Ok(deck) = Deck::from_json(&texte) {
            assert!(!deck.slides.is_empty());
            verifier(&deck);
        }
    }
    // Valeurs de types inattendus à chaque niveau.
    for valeur in ["null", "1", "-1e400", "true", "\"x\"", "[]", "{}", "[[[]]]", "\"\\u0000\""] {
        let json = format!(
            "{{\"title\":{valeur},\"theme\":{valeur},\"aspect\":{valeur},\"footer\":{valeur},\"slide_numbers\":{valeur},\"values\":{valeur},\"slides\":[{{\"layout\":{valeur},\"title\":{valeur},\"body\":{valeur},\"transition\":{valeur},\"background\":{valeur}}}, {valeur}]}}"
        );
        if let Ok(deck) = Deck::from_json(&json) {
            verifier(&deck);
        }
    }
}

#[test]
fn texte_immense_sans_panique() {
    let mut deck = Deck::new();
    let enorme = "**gras** et *italique* | cellule | {{x}}\n- puce très longue ".repeat(300);
    for l in Layout::ALL {
        deck.add_slide(usize::MAX, l);
        let s = deck.slides.last_mut().unwrap();
        s.title = enorme.clone();
        s.subtitle = enorme.clone();
        s.body = enorme.clone();
        s.body2 = format!("| a | b |\n|---|---|\n{}", "| x | y |\n".repeat(500));
        s.notes = enorme.clone();
    }
    deck.values.insert("x".into(), "y".repeat(1_000));
    verifier(&deck);
}
