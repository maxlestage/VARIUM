//! Statistiques d'écriture : temps de lecture et de parole, et indice de lisibilité
//! selon la langue du document.
//!
//! | Langue | Formule | Calcul |
//! |---|---|---|
//! | français | Flesch–Kandel (Kandel et Moles, 1958) | 207 − 1,015 × mots/phrase − 73,6 × syllabes/mot |
//! | anglais | Flesch (1948) | 206,835 − 1,015 × mots/phrase − 84,6 × syllabes/mot |
//! | espagnol | Fernández-Huerta (1959) | 206,84 − 0,60 × syllabes pour 100 mots − 1,02 × phrases pour 100 mots |
//! | allemand | Amstad (1978) | 180 − mots/phrase − 58,5 × syllabes/mot |
//! | italien | Gulpease (1988) | 89 + (300 × phrases − 10 × lettres) / mots |
//! | portugais | Flesch adapté (Martins et al., 1996) | 248,835 − 1,015 × mots/phrase − 84,6 × syllabes/mot |
//!
//! Les syllabes sont estimées par les groupes de voyelles, avec quelques règles par
//! langue (« e » muet final en français et en anglais) : une approximation, comme
//! dans tous les traitements de texte.

use crate::lang::Lang;

/// Mesures d'un texte.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Mesures {
    pub mots: usize,
    pub phrases: usize,
    pub syllabes: usize,
    pub lettres: usize,
}

/// Niveau de lecture, du plus simple au plus exigeant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Niveau {
    TresFacile,
    Facile,
    Standard,
    Difficile,
    TresDifficile,
}

/// Indice de lisibilité d'un texte.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lisibilite {
    /// Score de la formule (plus il est haut, plus le texte est facile).
    pub score: f32,
    pub niveau: Niveau,
    /// Nom de la formule employée (nom propre, non traduit).
    pub formule: &'static str,
}

fn voyelle(c: char) -> bool {
    "aeiouyàâäéèêëîïôöùûüœæáíóúãõåìò".contains(c.to_lowercase().next().unwrap_or(c))
}

/// Nombre de syllabes estimé d'un mot.
pub fn syllabes(mot: &str, lang: Lang) -> usize {
    let mot: String = mot
        .chars()
        .filter(|c| c.is_alphabetic())
        .collect::<String>();
    let mot = mot.to_lowercase();
    if mot.is_empty() {
        return 0;
    }
    let cs: Vec<char> = mot.chars().collect();
    let v: Vec<bool> = cs
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            // « qu » et « gu » devant voyelle : le u ne compte pas (sauf en anglais).
            if c == 'u' && i > 0 && matches!(cs[i - 1], 'q' | 'g') && lang != Lang::En {
                return false;
            }
            // « y » en tête de mot ou devant une voyelle est une consonne.
            if c == 'y' && (i == 0 || cs.get(i + 1).is_some_and(|&n| voyelle(n))) {
                return false;
            }
            voyelle(c)
        })
        .collect();
    let mut groupes = 0;
    for i in 0..cs.len() {
        if v[i] && (i == 0 || !v[i - 1]) {
            groupes += 1;
        }
    }
    let n = cs.len();
    match lang {
        Lang::Fr => {
            // « e », « es » muets en fin de mot (table, tables), sauf mot d'une syllabe.
            let muet = mot.ends_with('e') && n > 2 && !v[n - 2]
                || mot.ends_with("es") && n > 3 && !v[n - 3];
            if muet && groupes > 1 {
                groupes -= 1;
            }
        }
        Lang::En => {
            // « e » final muet (make), sauf « -le » (table) ; « -es », « -ed » souvent muets.
            let muet = (mot.ends_with('e') && !mot.ends_with("le") && n > 2)
                || ((mot.ends_with("es") || mot.ends_with("ed"))
                    && n > 3
                    && !matches!(cs[n - 3], 't' | 'd' | 's' | 'x' | 'z' | 'c' | 'g'));
            if muet && groupes > 1 {
                groupes -= 1;
            }
        }
        _ => {}
    }
    groupes.max(1)
}

/// Mesure un texte : mots, phrases, syllabes et lettres.
pub fn mesurer(texte: &str, lang: Lang) -> Mesures {
    let mut m = Mesures::default();
    for mot in texte.split(|c: char| c.is_whitespace() || "—–/".contains(c)) {
        let lettres = mot.chars().filter(|c| c.is_alphabetic()).count();
        if mot.chars().any(char::is_alphanumeric) {
            m.mots += 1;
            m.lettres += lettres;
            m.syllabes += if lettres > 0 { syllabes(mot, lang) } else { 1 };
        }
    }
    // Phrases : terminées par . ! ? … (une suite de ponctuations compte pour une) ou
    // par un saut de ligne après du texte (titres, éléments de liste).
    let mut dans_phrase = false;
    let cs: Vec<char> = texte.chars().collect();
    for (i, &c) in cs.iter().enumerate() {
        if c.is_alphanumeric() {
            dans_phrase = true;
        } else if dans_phrase
            && (matches!(c, '!' | '?' | '…' | '\n')
                || (c == '.' && !cs.get(i + 1).is_some_and(|n| n.is_alphanumeric())))
        {
            m.phrases += 1;
            dans_phrase = false;
        }
    }
    if dans_phrase {
        m.phrases += 1;
    }
    m
}

impl Mesures {
    /// Indice de lisibilité dans la langue ; `None` s'il n'y a pas de mot.
    pub fn lisibilite(&self, lang: Lang) -> Option<Lisibilite> {
        if self.mots == 0 {
            return None;
        }
        let w = self.mots as f32;
        let s = self.phrases.max(1) as f32;
        let mps = w / s;
        let spm = self.syllabes as f32 / w;
        let (score, formule) = match lang {
            Lang::Fr => (207.0 - 1.015 * mps - 73.6 * spm, "Flesch–Kandel"),
            Lang::En => (206.835 - 1.015 * mps - 84.6 * spm, "Flesch"),
            Lang::Es => (
                206.84 - 0.60 * (100.0 * spm) - 1.02 * (100.0 * s / w),
                "Fernández-Huerta",
            ),
            Lang::De => (180.0 - mps - 58.5 * spm, "Amstad"),
            Lang::It => (
                89.0 + (300.0 * s - 10.0 * self.lettres as f32) / w,
                "Gulpease",
            ),
            Lang::Pt => (248.835 - 1.015 * mps - 84.6 * spm, "Flesch (Martins)"),
        };
        let score = score.clamp(0.0, 100.0);
        let niveau = if lang == Lang::It {
            // Gulpease : 80 et plus facile pour l'école primaire, 60 pour le collège,
            // 40 pour le lycée.
            match score {
                s if s >= 80.0 => Niveau::TresFacile,
                s if s >= 60.0 => Niveau::Facile,
                s if s >= 50.0 => Niveau::Standard,
                s if s >= 40.0 => Niveau::Difficile,
                _ => Niveau::TresDifficile,
            }
        } else {
            match score {
                s if s >= 80.0 => Niveau::TresFacile,
                s if s >= 70.0 => Niveau::Facile,
                s if s >= 50.0 => Niveau::Standard,
                s if s >= 30.0 => Niveau::Difficile,
                _ => Niveau::TresDifficile,
            }
        };
        Some(Lisibilite {
            score,
            niveau,
            formule,
        })
    }
}

/// Mots lus par minute, en lecture silencieuse (Trauzettel-Klosinski et al., 2012).
pub fn mots_par_minute_lecture(lang: Lang) -> f32 {
    match lang {
        Lang::Fr => 195.0,
        Lang::En => 228.0,
        Lang::Es => 218.0,
        Lang::De => 179.0,
        Lang::It => 188.0,
        Lang::Pt => 181.0,
    }
}

/// Mots dits par minute à voix haute (discours, présentation).
pub const MOTS_PAR_MINUTE_PAROLE: f32 = 130.0;

/// Temps de lecture en secondes.
pub fn secondes_lecture(mots: usize, lang: Lang) -> u32 {
    (mots as f32 * 60.0 / mots_par_minute_lecture(lang)).round() as u32
}

/// Temps de parole en secondes.
pub fn secondes_parole(mots: usize) -> u32 {
    (mots as f32 * 60.0 / MOTS_PAR_MINUTE_PAROLE).round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syllabes_par_langue() {
        for (mot, lang, n) in [
            ("table", Lang::Fr, 1),
            ("tables", Lang::Fr, 1),
            ("ordinateur", Lang::Fr, 4),
            ("quelque", Lang::Fr, 1),
            ("le", Lang::Fr, 1),
            ("Éléphant", Lang::Fr, 3),
            ("make", Lang::En, 1),
            ("table", Lang::En, 2),
            ("computer", Lang::En, 3),
            ("wanted", Lang::En, 2),
            ("jumped", Lang::En, 1),
            ("casa", Lang::Es, 2),
            ("queso", Lang::Es, 2),
            ("Haus", Lang::De, 1),
            ("Wasserflasche", Lang::De, 4),
            ("ciao", Lang::It, 1),
            ("saudade", Lang::Pt, 3),
        ] {
            assert_eq!(syllabes(mot, lang), n, "{mot}");
        }
        assert_eq!(syllabes("123", Lang::Fr), 0);
    }

    #[test]
    fn mesures() {
        let m = mesurer("Le chat dort. Il fait beau ! Version 2.5 prête…", Lang::Fr);
        assert_eq!(m.mots, 9);
        assert_eq!(m.phrases, 3);
        let m = mesurer("# Titre\n- un\n- deux", Lang::Fr);
        assert_eq!((m.mots, m.phrases), (3, 3));
        assert_eq!(mesurer("", Lang::Fr), Mesures::default());
        assert!(Mesures::default().lisibilite(Lang::Fr).is_none());
    }

    #[test]
    fn formules() {
        let simple = "Le chat dort. Il fait beau. On joue dans le pré.";
        let dur = "L'administration intercommunale, considérant l'imprévisibilité \
                   institutionnelle des réglementations environnementales européennes, \
                   reconsidère méthodiquement l'organisation opérationnelle.";
        for lang in Lang::ALL {
            let f = mesurer(simple, lang).lisibilite(lang).unwrap();
            let d = mesurer(dur, lang).lisibilite(lang).unwrap();
            assert!(f.score > d.score, "{lang:?} : {} ≤ {}", f.score, d.score);
            assert!(f.niveau < d.niveau, "{lang:?}");
            assert!((0.0..=100.0).contains(&f.score));
        }
        // Flesch en anglais, valeur calculée à la main : 1 phrase, 4 mots, 4 syllabes.
        let m = mesurer("The cat sat down.", Lang::En);
        assert_eq!((m.mots, m.phrases, m.syllabes), (4, 1, 4));
        let l = m.lisibilite(Lang::En).unwrap();
        assert!((l.score - 100.0).abs() < 0.01); // 206,835 − 4,06 − 84,6 = 118 → 100
        // Gulpease : 89 + (300 × 1 − 10 × 13) / 4 = 131,5 → 100.
        let m = mesurer("Il gatto dorme bene.", Lang::It);
        assert_eq!(m.lettres, 16);
        assert!((m.lisibilite(Lang::It).unwrap().score - 100.0).abs() < 0.01);
    }

    #[test]
    fn temps() {
        assert_eq!(secondes_lecture(195, Lang::Fr), 60);
        assert_eq!(secondes_parole(130), 60);
        assert_eq!(secondes_lecture(0, Lang::En), 0);
    }
}
