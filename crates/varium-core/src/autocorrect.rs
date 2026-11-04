//! Correction automatique pendant la frappe, comme celle de LibreOffice Writer,
//! selon la langue du document :
//!
//! - majuscule en début de phrase (« bonjour. merci » → « Bonjour. Merci ») ;
//! - deux majuscules en tête de mot (« BOnjour » → « Bonjour ») ;
//! - fautes de frappe courantes de la langue (« teh » → « the », « parmis » → « parmi ») ;
//! - symboles : (c) → ©, (r) → ®, (tm) → ™, 1/2 → ½, 1/4 → ¼, 3/4 → ¾, -> → →,
//!   <- → ←, ... → … ;
//! - guillemets et apostrophe typographiques de la langue (ceux de [`crate::typo`]).
//!
//! Rien n'est touché dans un champ `{{…}}`, une formule `$…$`, une adresse web, ni
//! dans les marques d'alignement `-> … <-` de Varium. [`apres_frappe`] renvoie un seul
//! remplacement, que l'éditeur applique comme une étape d'annulation à part : un
//! Ctrl+Z juste après rend le texte tapé.

use crate::lang::Lang;

/// Remplacer `debut..fin` (octets) par `texte`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remplacement {
    pub debut: usize,
    pub fin: usize,
    pub texte: String,
}

/// Fautes de frappe courantes, par langue (sans ambiguïté : jamais un vrai mot).
fn fautes(lang: Lang) -> &'static [(&'static str, &'static str)] {
    match lang {
        Lang::Fr => &[
            ("acceuil", "accueil"),
            ("apeller", "appeler"),
            ("apparament", "apparemment"),
            ("aujourdhui", "aujourd’hui"),
            ("biensûr", "bien sûr"),
            ("developpement", "développement"),
            ("dévelopement", "développement"),
            ("malgrés", "malgré"),
            ("néanmois", "néanmoins"),
            ("parmis", "parmi"),
            ("quelquechose", "quelque chose"),
            ("rapeller", "rappeler"),
            ("rapelle", "rappelle"),
            ("vraiement", "vraiment"),
        ],
        Lang::En => &[
            ("accomodate", "accommodate"),
            ("adn", "and"),
            ("alot", "a lot"),
            ("beleive", "believe"),
            ("definately", "definitely"),
            ("i", "I"),
            ("occured", "occurred"),
            ("recieve", "receive"),
            ("seperate", "separate"),
            ("teh", "the"),
            ("thier", "their"),
            ("tommorow", "tomorrow"),
            ("untill", "until"),
            ("wich", "which"),
        ],
        Lang::Es => &[
            ("ademas", "además"),
            ("atraves", "a través"),
            ("aveces", "a veces"),
            ("despues", "después"),
            ("porfavor", "por favor"),
            ("tambien", "también"),
        ],
        Lang::De => &[
            ("addresse", "Adresse"),
            ("ausversehen", "aus Versehen"),
            ("garnicht", "gar nicht"),
            ("nciht", "nicht"),
            ("rythmus", "Rhythmus"),
            ("seperat", "separat"),
            ("standart", "Standard"),
            ("vieleicht", "vielleicht"),
            ("wiederrum", "wiederum"),
        ],
        Lang::It => &[
            ("affinchè", "affinché"),
            ("avvolte", "a volte"),
            ("daccordo", "d’accordo"),
            ("perchè", "perché"),
            ("poichè", "poiché"),
            ("propio", "proprio"),
            ("pò", "po’"),
            ("qual'è", "qual è"),
            ("qual’è", "qual è"),
        ],
        Lang::Pt => &[
            ("apartir", "a partir"),
            ("concerteza", "com certeza"),
            ("derrepente", "de repente"),
            ("excessão", "exceção"),
            ("mendingo", "mendigo"),
            ("menas", "menos"),
            ("porisso", "por isso"),
            ("previlégio", "privilégio"),
        ],
    }
}

/// Abréviations après lesquelles un point ne termine pas la phrase.
const ABREVIATIONS: &[&str] = &[
    "etc", "cf", "ex", "p", "pp", "M", "MM", "Mme", "Mmes", "Mlle", "Dr", "Pr", "Me", "St", "av",
    "bd", "vol", "chap", "fig", "n", "no", "nº", "env", "Mr", "Mrs", "Ms", "vs", "e.g", "i.e",
    "approx", "Sr", "Sra", "Srta", "Hr", "Fr", "Nr", "z.B", "Sig", "Dott", "Sto", "Sta",
];

/// Caractères qui terminent un mot (et déclenchent la correction du mot). Pas « : »,
/// qui suit « https » dans une adresse en cours de frappe.
fn limite(c: char) -> bool {
    c.is_whitespace() || ".,;!?)]»”“\"".contains(c)
}

fn lettre_de_mot(c: char) -> bool {
    c.is_alphabetic() || c == '\'' || c == '’' || c == '-'
}

/// Le curseur est-il dans un champ, une formule ou une adresse de lien ?
fn protege(ligne: &str) -> bool {
    let ouvert = ligne.rfind("{{");
    let ferme = ligne.rfind("}}");
    let dans_champ = match (ouvert, ferme) {
        (Some(o), Some(f)) => o > f,
        (Some(_), None) => true,
        _ => false,
    };
    let dollars = ligne
        .match_indices('$')
        .filter(|(i, _)| *i == 0 || !ligne[..*i].ends_with('\\'))
        .count();
    let dans_lien = ligne.rfind("](").is_some_and(|i| !ligne[i..].contains(')'));
    dans_champ || dollars % 2 == 1 || dans_lien
}

/// Retire les marques Varium de début de ligne (titre, liste, citation, alignement,
/// gras…) ; renvoie ce qui reste avant le mot.
fn sans_marques_de_ligne(avant: &str) -> &str {
    let mut t = avant.trim_start();
    loop {
        let avant_tour = t;
        t = t.trim_start_matches('#').trim_start();
        for p in ["- ", "* ", "+ ", "• ", "> ", "-> ", "->"] {
            if let Some(r) = t.strip_prefix(p) {
                t = r.trim_start();
            }
        }
        let chiffres = t.chars().take_while(char::is_ascii_digit).count();
        if chiffres > 0 && (t[chiffres..].starts_with(". ") || t[chiffres..].starts_with(") ")) {
            t = t[chiffres + 2..].trim_start();
        }
        t = t.trim_start_matches(['*', '_', '~']);
        if t == avant_tour {
            return t;
        }
    }
}

/// Le texte avant le mot termine-t-il une phrase ?
fn debut_de_phrase(avant: &str) -> bool {
    let reste = sans_marques_de_ligne(avant);
    let t = reste.trim_end_matches(|c: char| c.is_whitespace() || "*_~«“„¿¡(\"".contains(c));
    if t.is_empty() {
        // Début de ligne (paragraphe) ; mais pas dans une cellule de tableau.
        return !avant.trim_start().starts_with('|');
    }
    if reste.len() == t.len() {
        return false; // pas d'espace entre la ponctuation et le mot
    }
    let Some(fin) = t.chars().last() else {
        return false;
    };
    match fin {
        '!' | '?' | '…' => true,
        '.' => {
            let sans_point = &t[..t.len() - 1];
            if sans_point.ends_with('.') {
                return true; // « ... » non converti
            }
            let mot: String = sans_point
                .chars()
                .rev()
                .take_while(|c| c.is_alphanumeric() || *c == '.')
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            // Initiale (« J. Martin »), abréviation, nombre (« 2. »).
            !((mot.chars().count() == 1 && mot.chars().all(char::is_alphabetic))
                || ABREVIATIONS.contains(&mot.as_str())
                || mot.chars().all(char::is_numeric))
        }
        _ => false,
    }
}

fn majuscule_initiale(m: &str) -> String {
    let mut c = m.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// Corrige un mot ; `None` s'il n'y a rien à changer.
fn corriger_mot(mot: &str, avant: &str, lang: Lang) -> Option<String> {
    let mut m = mot.to_string();
    // Fautes de frappe (casse de la première lettre conservée).
    let bas = m.to_lowercase();
    if let Some((_, bon)) = fautes(lang).iter().find(|(f, _)| *f == bas) {
        let premiere_maj = m.chars().next().is_some_and(char::is_uppercase);
        m = if premiere_maj {
            majuscule_initiale(bon)
        } else {
            (*bon).to_string()
        };
    }
    // Deux majuscules : « BOnjour » → « Bonjour » (pas les sigles « PDF », « CDs »).
    let cs: Vec<char> = m.chars().collect();
    if cs.len() >= 3
        && cs[0].is_uppercase()
        && cs[1].is_uppercase()
        && cs[2..]
            .iter()
            .filter(|c| c.is_alphabetic())
            .all(|c| c.is_lowercase())
        && cs[2..].iter().filter(|c| c.is_lowercase()).count() >= 2
    {
        m = cs[..1]
            .iter()
            .copied()
            .chain(cs[1].to_lowercase())
            .chain(cs[2..].iter().copied())
            .collect();
    }
    // Majuscule en début de phrase.
    if m.chars().next().is_some_and(char::is_lowercase) && debut_de_phrase(avant) {
        m = majuscule_initiale(&m);
    }
    (m != mot).then_some(m)
}

/// Correction à appliquer juste après la frappe du caractère qui précède `curseur`
/// (position en octets dans `src`).
pub fn apres_frappe(src: &str, curseur: usize, lang: Lang) -> Option<Remplacement> {
    if curseur == 0 || curseur > src.len() || !src.is_char_boundary(curseur) {
        return None;
    }
    let tape = src[..curseur].chars().next_back()?;
    let pos_tape = curseur - tape.len_utf8();
    let debut_ligne = src[..pos_tape].rfind('\n').map_or(0, |i| i + 1);
    let ligne = &src[debut_ligne..pos_tape];
    if protege(ligne) {
        return None;
    }
    let rempl = |debut: usize, fin: usize, texte: &str| {
        Some(Remplacement {
            debut,
            fin,
            texte: texte.to_string(),
        })
    };
    // Symboles déclenchés par le caractère tapé.
    let avec = &src[debut_ligne..curseur];
    let alignement = avec.trim_start().starts_with("->");
    match tape {
        ')' => {
            for (motif, sym) in [("(c)", "©"), ("(r)", "®"), ("(tm)", "™")] {
                let n = motif.len();
                if avec.len() >= n && avec[avec.len() - n..].eq_ignore_ascii_case(motif) {
                    return rempl(curseur - n, curseur, sym);
                }
            }
        }
        '>' if avec.ends_with("->") && !avec.ends_with("-->") && !alignement => {
            return rempl(curseur - 2, curseur, "→");
        }
        '-' if avec.ends_with("<-") && !alignement && !avec.ends_with("<<-") => {
            return rempl(curseur - 2, curseur, "←");
        }
        '.' if avec.ends_with("...") && !avec.ends_with("....") => {
            return rempl(curseur - 3, curseur, "…");
        }
        '"' => {
            let (ouvrant, fermant, interieur) = lang.quotes();
            let ouverts = ligne.matches(ouvrant).count();
            let fermes = if ouvrant == fermant {
                0
            } else {
                ligne.matches(fermant).count()
            };
            let prec = ligne.chars().next_back();
            let ouvre = ouverts <= fermes
                && prec.is_none_or(|p| p.is_whitespace() || "([{—–*_".contains(p));
            let sp = interieur.map(String::from).unwrap_or_default();
            return if ouvre {
                rempl(pos_tape, curseur, &format!("{ouvrant}{sp}"))
            } else {
                let debut = debut_ligne + ligne.trim_end_matches(' ').len();
                rempl(debut, curseur, &format!("{sp}{fermant}"))
            };
        }
        '\'' if ligne.chars().next_back().is_some_and(char::is_alphabetic) => {
            return rempl(pos_tape, curseur, "’");
        }
        _ => {}
    }
    if !limite(tape) {
        return None;
    }
    // Fractions : « 1/2 » suivi d'une limite.
    for (motif, sym) in [("1/2", "½"), ("1/4", "¼"), ("3/4", "¾")] {
        if let Some(avant) = ligne.strip_suffix(motif)
            && !avant
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_digit() || c == '/')
        {
            return rempl(pos_tape - 3, pos_tape, sym);
        }
    }
    // Le mot qui vient d'être terminé (marques de mise en forme à sa droite ignorées).
    let sans_marques = ligne.trim_end_matches(['*', '_', '~']);
    let mot_debut = sans_marques
        .char_indices()
        .rev()
        .take_while(|(_, c)| lettre_de_mot(*c))
        .last()
        .map(|(i, _)| i)?;
    let mot = sans_marques[mot_debut..].trim_start_matches(['\'', '’', '-']);
    let mot_debut = sans_marques.len() - mot.len();
    let mot = mot.trim_end_matches(['\'', '’', '-']);
    if mot.is_empty() {
        return None;
    }
    // Le « mot » fait partie d'une adresse, d'un nombre, d'une marque ? On n'y touche pas.
    let jeton_debut = ligne[..mot_debut]
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(i, c)| i + c.len_utf8());
    let jeton = &ligne[jeton_debut..];
    if jeton
        .chars()
        .any(|c| "/@:\\{}$|=<>#&+".contains(c) || c.is_ascii_digit())
        || jeton[..mot_debut - jeton_debut]
            .chars()
            .any(|c| c.is_alphanumeric() || c == '.')
    {
        return None;
    }
    let nouveau = corriger_mot(mot, &ligne[..mot_debut], lang)?;
    rempl(
        debut_ligne + mot_debut,
        debut_ligne + mot_debut + mot.len(),
        &nouveau,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tape `texte` caractère par caractère en appliquant les corrections.
    fn taper(texte: &str, lang: Lang) -> String {
        let mut s = String::new();
        for c in texte.chars() {
            s.push(c);
            if let Some(r) = apres_frappe(&s, s.len(), lang) {
                assert!(r.debut <= r.fin && r.fin <= s.len());
                s.replace_range(r.debut..r.fin, &r.texte);
            }
        }
        s
    }

    #[test]
    fn majuscules() {
        assert_eq!(
            taper("bonjour. merci ! ça va ? oui", Lang::Fr),
            "Bonjour. Merci ! Ça va ? oui"
        );
        assert_eq!(
            taper("# titre\n- élément un\n> citation ", Lang::Fr),
            "# Titre\n- Élément un\n> Citation "
        );
        assert_eq!(taper("BOnjour CDs PDF ", Lang::Fr), "Bonjour CDs PDF ");
        // Abréviations, initiales, nombres : pas de majuscule.
        assert_eq!(
            taper(
                "Voir p. ex. la page. J. martin etc. fin 2. suite ",
                Lang::Fr
            ),
            "Voir p. ex. la page. J. martin etc. fin 2. suite "
        );
        // Adresses, champs, formules : intacts.
        assert_eq!(
            taper("Voir www.exemple.fr. https://a.fr/b jean@x.fr ", Lang::Fr),
            "Voir www.exemple.fr. https://a.fr/b jean@x.fr "
        );
        assert_eq!(
            taper("{{nom | default:\"teh\"}} $x... y$ ", Lang::En),
            "{{nom | default:\"teh\"}} $x... y$ "
        );
        assert_eq!(taper("**gras** ", Lang::Fr), "**Gras** ");
        assert_eq!(taper("| cellule |", Lang::Fr), "| cellule |");
    }

    #[test]
    fn fautes_par_langue() {
        assert_eq!(taper("teh cat adn i ", Lang::En), "The cat and I ");
        assert_eq!(taper("C'est parmis", Lang::Fr), "C’est parmis");
        assert_eq!(taper("C'est parmis nous ", Lang::Fr), "C’est parmi nous ");
        assert_eq!(
            taper("Das ist vieleicht gut ", Lang::De),
            "Das ist vielleicht gut "
        );
        assert_eq!(taper("Es tambien ", Lang::Es), "Es también ");
        assert_eq!(taper("Ma perchè ", Lang::It), "Ma perché ");
        assert_eq!(taper("Agora porisso ", Lang::Pt), "Agora por isso ");
        // La faute d'une autre langue n'est pas corrigée.
        assert_eq!(taper("Le teh ", Lang::Fr), "Le teh ");
    }

    #[test]
    fn symboles() {
        assert_eq!(
            taper(
                "(c) 2026 (TM) 1/2 litre, 3/4. a -> b <- c... 11/2 ",
                Lang::Fr
            ),
            "© 2026 ™ ½ litre, ¾. a → b ← c… 11/2 "
        );
        // Marques d'alignement de Varium : intactes.
        assert_eq!(taper("-> Centré <-", Lang::Fr), "-> Centré <-");
        assert_eq!(taper("-> À droite ->", Lang::Fr), "-> À droite ->");
        assert_eq!(taper("Un --> b", Lang::Fr), "Un --> b");
    }

    #[test]
    fn guillemets() {
        assert_eq!(
            taper("Il dit \"oui\" puis l'été", Lang::Fr),
            "Il dit «\u{a0}oui\u{a0}» puis l’été"
        );
        assert_eq!(taper("He said \"yes\"", Lang::En), "He said “yes”");
        assert_eq!(taper("Er sagt \"ja\"", Lang::De), "Er sagt „ja“");
        assert_eq!(
            taper("{{t | default:\"x\"}}", Lang::Fr),
            "{{t | default:\"x\"}}"
        );
    }

    #[test]
    fn positions_sures() {
        assert_eq!(apres_frappe("", 0, Lang::Fr), None);
        assert_eq!(apres_frappe("é", 1, Lang::Fr), None);
        assert_eq!(apres_frappe("ab", 9, Lang::Fr), None);
        // Fuzz : aucune panique, plages toujours valides.
        let atomes = [
            "a", "B", " ", ".", "\n", "\"", "'", "(c)", "->", "<-", "1/2", "{{", "}}", "$", "é",
            "😀", "**", "#", "- ", "|", "…", "\u{a0}", "teh", "i",
        ];
        let mut etat = 0x2545_F491_4F6C_DD1D_u64;
        for _ in 0..2000 {
            let mut t = String::new();
            for _ in 0..12 {
                etat ^= etat << 13;
                etat ^= etat >> 7;
                etat ^= etat << 17;
                t.push_str(atomes[(etat % atomes.len() as u64) as usize]);
            }
            for lang in Lang::ALL {
                let _ = taper(&t, lang);
            }
        }
    }
}
