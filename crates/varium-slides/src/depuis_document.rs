//! Conversion d'un document Varium en présentation : les titres `#` et `##`
//! deviennent des diapositives, le reste (paragraphes, listes, tableaux, champs)
//! leur contenu, et les valeurs des champs sont reprises par la présentation.

use std::collections::BTreeMap;

use crate::{Deck, Layout, Slide, Transition};

/// Au-delà de ce nombre de lignes, le contenu d'une diapositive continue sur la
/// suivante (coupé entre deux blocs, jamais au milieu d'un tableau ou d'une liste).
const MAX_LIGNES: usize = 10;

/// Le niveau d'un titre de document (`# …` → 1), s'il en est un.
fn titre(ligne: &str) -> Option<(usize, &str)> {
    let n = ligne.chars().take_while(|c| *c == '#').count();
    let reste = &ligne[n..];
    ((1..=6).contains(&n) && reste.starts_with(' ')).then(|| (n, reste.trim()))
}

/// Directives de mise en page qui n'ont pas de sens sur une diapositive.
fn ignoree(ligne: &str) -> bool {
    let t = ligne.trim();
    t == "\\newpage" || t == "\\sommaire" || (t.len() >= 3 && t.chars().all(|c| c == '-'))
}

/// Une section du document : son titre (s'il y en a un) et ses lignes.
struct Section {
    niveau: usize,
    titre: String,
    lignes: Vec<String>,
}

/// Les blocs d'un contenu (séparés par des lignes vides), sans lignes vides en trop.
fn blocs(lignes: &[String]) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    for l in lignes {
        if l.trim().is_empty() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(l.clone());
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Le contenu en morceaux d'au plus `MAX_LIGNES` lignes (un bloc plus long reste
/// entier).
fn morceaux(lignes: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    let mut n = 0;
    for b in blocs(lignes) {
        if n > 0 && n + b.len() > MAX_LIGNES {
            out.push(cur.join("\n\n"));
            cur.clear();
            n = 0;
        }
        n += b.len();
        cur.push(b.join("\n"));
    }
    if !cur.is_empty() {
        out.push(cur.join("\n\n"));
    }
    out
}

/// Un bloc de texte simple (ni liste, ni tableau, ni titre, ni citation) : il peut
/// servir de sous-titre à la diapositive de titre.
fn texte_simple(b: &[String]) -> bool {
    b.iter().all(|l| {
        let t = l.trim_start();
        !(t.starts_with('|')
            || t.starts_with('>')
            || t.starts_with('#')
            || ["- ", "* ", "• ", "+ "].iter().any(|p| t.starts_with(p))
            || t.split_once(". ").is_some_and(|(n, _)| {
                !n.is_empty() && n.len() <= 4 && n.chars().all(|c| c.is_ascii_digit())
            }))
    })
}

/// Construit une présentation à partir de la source d'un document Varium.
///
/// - le premier titre `#` donne la diapositive de titre (et le titre de la
///   présentation) ; un court paragraphe qui le suit devient son sous-titre ;
/// - chaque autre titre `#` ou `##` donne une diapositive Titre et contenu ;
/// - les titres `###` et plus, les paragraphes, listes, citations et tableaux
///   forment le contenu, coupé sur plusieurs diapositives s'il est long ;
/// - le texte placé avant le premier titre ouvre la présentation ;
/// - les champs `{{…}}` sont conservés tels quels et `values` devient les valeurs
///   de la présentation.
pub fn deck_from_document(src: &str, values: &BTreeMap<String, String>) -> Deck {
    let mut sections: Vec<Section> = vec![Section {
        niveau: 0,
        titre: String::new(),
        lignes: Vec::new(),
    }];
    for ligne in src.lines() {
        if ignoree(ligne) {
            continue;
        }
        match titre(ligne) {
            Some((n, t)) if n <= 2 => sections.push(Section {
                niveau: n,
                titre: t.to_string(),
                lignes: Vec::new(),
            }),
            _ => {
                if let Some(s) = sections.last_mut() {
                    s.lignes.push(ligne.trim_end().to_string());
                }
            }
        }
    }

    let mut deck = Deck::new();
    deck.slides.clear();
    deck.values = values.clone();
    let mut titre_fait = false;
    for (i, s) in sections.into_iter().enumerate() {
        let mut lignes = s.lignes;
        if i == 0 && blocs(&lignes).is_empty() {
            continue;
        }
        if s.niveau == 1 && !titre_fait {
            titre_fait = true;
            deck.title = s.titre.clone();
            let mut b = blocs(&lignes);
            let mut subtitle = String::new();
            if let Some(first) = b.first()
                && first.len() <= 3
                && texte_simple(first)
            {
                subtitle = b.remove(0).join("\n");
            }
            deck.slides.push(Slide {
                layout: Layout::Title,
                title: s.titre.clone(),
                subtitle,
                transition: Transition::Fondu,
                ..Slide::default()
            });
            lignes = b
                .into_iter()
                .flat_map(|b| b.into_iter().chain([String::new()]))
                .collect();
            if blocs(&lignes).is_empty() {
                continue;
            }
        }
        let parts = morceaux(&lignes);
        if parts.is_empty() {
            deck.slides.push(Slide {
                layout: Layout::TitleOnly,
                title: s.titre,
                transition: Transition::Fondu,
                ..Slide::default()
            });
            continue;
        }
        for body in parts {
            deck.slides.push(Slide {
                layout: Layout::TitleContent,
                title: s.titre.clone(),
                body,
                transition: Transition::Fondu,
                ..Slide::default()
            });
        }
    }
    if deck.slides.is_empty() {
        deck.slides.push(Slide::new(Layout::Title));
    }
    deck
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vals() -> BTreeMap<String, String> {
        [("client".to_string(), "Dupont".to_string())].into()
    }

    #[test]
    fn titres_en_diapositives() {
        let src = "# Rapport annuel\nPréparé pour {{client}}\n\n## Chiffres\n- Ventes : +12 %\n- Clients : 340\n\n## Tableau\n| a | b |\n|---|---|\n| 1 | 2 |\n\n# Conclusion\nMerci.";
        let d = deck_from_document(src, &vals());
        assert_eq!(d.title, "Rapport annuel");
        assert_eq!(d.slides.len(), 4);
        assert_eq!(d.slides[0].layout, Layout::Title);
        assert_eq!(d.slides[0].subtitle, "Préparé pour {{client}}");
        assert_eq!(d.slides[1].layout, Layout::TitleContent);
        assert_eq!(d.slides[1].title, "Chiffres");
        assert_eq!(d.slides[1].body, "- Ventes : +12 %\n- Clients : 340");
        assert!(d.slides[2].body.contains("| 1 | 2 |"));
        assert_eq!(d.slides[3].title, "Conclusion");
        assert_eq!(d.slides[3].layout, Layout::TitleContent);
        assert_eq!(d.values.get("client").map(String::as_str), Some("Dupont"));
        assert_eq!(d.variables(), ["client"]);
    }

    #[test]
    fn titre_avec_liste_garde_son_contenu() {
        let d = deck_from_document("# Projet\n- un\n- deux", &BTreeMap::new());
        assert_eq!(d.slides.len(), 2);
        assert_eq!(d.slides[0].subtitle, "");
        assert_eq!(d.slides[1].title, "Projet");
        assert_eq!(d.slides[1].body, "- un\n- deux");
    }

    #[test]
    fn texte_avant_le_premier_titre_et_sans_titre() {
        let d = deck_from_document("Bonjour {{client}},\n\nVoici le devis.", &vals());
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.slides[0].layout, Layout::TitleContent);
        assert!(d.slides[0].body.starts_with("Bonjour {{client}},"));
        let d = deck_from_document("", &BTreeMap::new());
        assert_eq!(d.slides.len(), 1);
    }

    #[test]
    fn long_contenu_coupe_et_directives_ignorees() {
        let mut src = String::from("## Liste\n");
        for i in 0..8 {
            src.push_str(&format!("Paragraphe {i}\nsuite {i}\n\n"));
        }
        src.push_str("\\newpage\n---\n### Détail\ntexte");
        let d = deck_from_document(&src, &BTreeMap::new());
        assert!(d.slides.len() >= 2);
        assert!(d.slides.iter().all(|s| s.title == "Liste"));
        let tout: String = d.slides.iter().map(|s| s.body.clone()).collect();
        assert!(!tout.contains("\\newpage") && !tout.contains("---"));
        assert!(tout.contains("### Détail"));
        assert!(
            d.slides
                .iter()
                .all(|s| s.body.lines().filter(|l| !l.is_empty()).count() <= MAX_LIGNES)
        );
    }

    #[test]
    fn titre_seul() {
        let d = deck_from_document("# A\n## B\n## C\ntexte", &BTreeMap::new());
        assert_eq!(d.slides.len(), 3);
        assert_eq!(d.slides[1].layout, Layout::TitleOnly);
        assert_eq!(d.slides[2].body, "texte");
    }
}
