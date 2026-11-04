//! « Nouvelle présentation depuis un texte » : des notes de réunion, un courriel
//! ou n'importe quel texte libre deviennent une présentation.
//!
//! - le titre vient de l'objet du courriel, sinon d'une première ligne courte ;
//! - les formules d'appel (« Bonjour, ») et la signature (« Cordialement… ») sont
//!   écartées ;
//! - une ligne courte isolée (ou terminée par « : », ou `# Titre`) ouvre une
//!   diapositive ; le texte qui suit en est le contenu ;
//! - les listes restent des listes ; un paragraphe devient une puce par phrase ;
//! - une diapositive trop pleine continue sur la suivante ;
//! - une diapositive de clôture (« Merci ! », « Questions ? », dans la langue de
//!   l'interface) termine la présentation ;
//! - le thème est choisi d'après le vocabulaire du texte.
//!
//! Jamais de panique, quel que soit le texte.

use crate::{Deck, Layout, Slide, Theme, Transition};

/// Taille maximale du texte lu (en caractères) ; le reste est ignoré.
const MAX_CARACTERES: usize = 100_000;
/// Nombre maximal de diapositives produites.
const MAX_DIAPOSITIVES: usize = 80;
/// Puces au plus par diapositive.
const MAX_PUCES: usize = 6;
/// Longueur maximale d'une ligne qui sert de titre.
const TITRE_MAX: usize = 70;

/// Les textes de la diapositive de clôture (dans la langue de l'interface).
#[derive(Debug, Clone, Copy)]
pub struct Closing<'a> {
    pub thanks: &'a str,
    pub questions: &'a str,
}

impl Default for Closing<'_> {
    fn default() -> Self {
        Closing {
            thanks: "Merci !",
            questions: "Questions ?",
        }
    }
}

fn minuscules(s: &str) -> String {
    s.to_lowercase()
}

/// Un en-tête de courriel : (nom, valeur).
fn entete(l: &str) -> Option<(String, String)> {
    let (nom, valeur) = l.split_once(':')?;
    let nom = minuscules(nom.trim());
    let connus = [
        "objet",
        "sujet",
        "subject",
        "asunto",
        "betreff",
        "oggetto",
        "assunto",
        "de",
        "from",
        "von",
        "da",
        "à",
        "a",
        "to",
        "an",
        "para",
        "cc",
        "cci",
        "bcc",
        "date",
        "fecha",
        "datum",
        "data",
        "sent",
        "envoyé",
        "envoyé le",
        "gesendet",
        "inviato",
        "enviado",
    ];
    connus
        .contains(&nom.as_str())
        .then(|| (nom, valeur.trim().to_string()))
}

fn est_objet(nom: &str) -> bool {
    matches!(
        nom,
        "objet" | "sujet" | "subject" | "asunto" | "betreff" | "oggetto" | "assunto"
    )
}

fn est_expediteur(nom: &str) -> bool {
    matches!(nom, "de" | "from" | "von" | "da")
}

/// Une formule d'appel (« Bonjour à tous, »).
fn salutation(l: &str) -> bool {
    let m = minuscules(l.trim());
    let debuts = [
        "bonjour",
        "bonsoir",
        "salut",
        "coucou",
        "cher ",
        "chère ",
        "chers ",
        "madame",
        "monsieur",
        "hello",
        "hi ",
        "hi,",
        "hey",
        "dear",
        "hola",
        "estimad",
        "hallo",
        "liebe",
        "lieber",
        "sehr geehrte",
        "guten tag",
        "ciao",
        "buongiorno",
        "gentil",
        "olá",
        "ola ",
        "bom dia",
        "caro",
        "cara ",
    ];
    m.chars().count() <= 50 && (debuts.iter().any(|d| m.starts_with(d)) || m == "hi")
}

/// Une formule de politesse finale : la signature suit.
fn signature(l: &str) -> bool {
    let m = minuscules(l.trim());
    let debuts = [
        "cordialement",
        "bien cordialement",
        "bien à vous",
        "bien à toi",
        "amicalement",
        "bonne journée",
        "belle journée",
        "à bientôt",
        "salutations",
        "best regards",
        "kind regards",
        "regards",
        "best,",
        "cheers",
        "sincerely",
        "thanks,",
        "thank you,",
        "saludos",
        "un saludo",
        "atentamente",
        "mit freundlichen",
        "viele grüße",
        "beste grüße",
        "liebe grüße",
        "cordiali saluti",
        "distinti saluti",
        "saluti",
        "atenciosamente",
        "cumprimentos",
        "abraços",
        "--",
        "__",
        "envoyé de mon",
        "sent from my",
    ];
    m == "--" || (m.chars().count() <= 40 && debuts.iter().any(|d| m.starts_with(d)))
}

/// Une marque de liste en tête de ligne : renvoie le texte de l'élément.
fn puce(l: &str) -> Option<&str> {
    let t = l.trim_start();
    for m in [
        "- ", "* ", "+ ", "• ", "· ", "– ", "— ", "▪ ", "► ", "→ ", "✓ ", "✔ ", "☐ ",
    ] {
        if let Some(r) = t.strip_prefix(m) {
            return Some(r.trim());
        }
    }
    let chiffres = t.chars().take_while(char::is_ascii_digit).count();
    if (1..=3).contains(&chiffres) {
        let r = &t[chiffres..];
        for m in [". ", ") ", "- "] {
            if let Some(r) = r.strip_prefix(m) {
                return Some(r.trim());
            }
        }
    }
    let lettre = t.chars().next().filter(char::is_ascii_lowercase);
    if let Some(c) = lettre
        && let Some(r) = t[c.len_utf8()..].strip_prefix(") ")
    {
        return Some(r.trim());
    }
    None
}

/// Retrait d'une ligne (en espaces, une tabulation compte pour quatre).
fn retrait(l: &str) -> usize {
    let mut n = 0;
    for c in l.chars() {
        match c {
            ' ' => n += 1,
            '\t' => n += 4,
            _ => break,
        }
    }
    n
}

/// La ligne ressemble-t-elle à un titre ?
fn titre_possible(l: &str) -> bool {
    let t = l.trim();
    if t.is_empty() || t.chars().count() > TITRE_MAX || puce(l).is_some() {
        return false;
    }
    if t.starts_with('#') || t.ends_with(':') {
        return true;
    }
    let fin = t.chars().last().unwrap_or(' ');
    !matches!(fin, '.' | ',' | ';' | '…') && t.split_whitespace().count() <= 10
}

fn sans_marque_de_titre(l: &str) -> String {
    let t = l.trim().trim_start_matches('#').trim();
    let t = t.strip_suffix(':').unwrap_or(t).trim();
    t.trim_matches(|c| c == '*' || c == '_').trim().to_string()
}

/// Abréviations suivies d'un point qui ne terminent pas une phrase.
const ABREVIATIONS: [&str; 26] = [
    "m", "mme", "mlle", "mm", "dr", "pr", "me", "st", "ste", "ex", "cf", "etc", "env", "p", "vol",
    "no", "n°", "mr", "mrs", "ms", "vs", "e.g", "i.e", "sr", "sra", "hr",
];

/// Coupe un paragraphe en phrases.
pub(crate) fn phrases(p: &str) -> Vec<String> {
    let cs: Vec<char> = p.chars().collect();
    let mut out = Vec::new();
    let mut debut = 0;
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if matches!(c, '.' | '!' | '?' | '…' | '。') {
            // Ponctuation répétée ou fermante (« ?! », « . » », « .) »).
            let mut j = i + 1;
            while j < cs.len() && matches!(cs[j], '.' | '!' | '?' | '»' | '"' | '”' | ')' | '’')
            {
                j += 1;
            }
            let espace = j < cs.len() && cs[j].is_whitespace();
            let fin = j >= cs.len();
            // Le mot avant le point : abréviation, initiale ou nombre décimal ?
            let mot: String = cs[debut..i]
                .iter()
                .rev()
                .take_while(|c| !c.is_whitespace())
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<String>()
                .to_lowercase();
            let mot = mot.trim_start_matches(['(', '«', '"', '“']);
            let abreviation = c == '.'
                && (ABREVIATIONS.contains(&mot)
                    || (mot.chars().count() == 1 && mot.chars().all(char::is_alphabetic)));
            let suivant = cs[j..].iter().find(|c| !c.is_whitespace()).copied();
            let debut_de_phrase = suivant.is_none_or(|s| {
                s.is_uppercase() || s.is_ascii_digit() || matches!(s, '«' | '"' | '“' | '(' | '-')
            });
            if fin || (espace && !abreviation && debut_de_phrase) {
                let ph: String = cs[debut..j].iter().collect::<String>().trim().to_string();
                if !ph.is_empty() {
                    out.push(ph);
                }
                debut = j;
                i = j;
                continue;
            }
        }
        i += 1;
    }
    let reste: String = cs[debut.min(cs.len())..]
        .iter()
        .collect::<String>()
        .trim()
        .to_string();
    if !reste.is_empty() {
        out.push(reste);
    }
    out
}

/// Un morceau de contenu : (niveau, texte).
type Element = (usize, String);

/// Une diapositive en préparation.
#[derive(Default)]
struct Brouillon {
    titre: String,
    elements: Vec<Element>,
    /// Le titre est tiré du texte (aucun titre écrit).
    deduit: bool,
}

/// Les premiers mots d'une phrase, pour un titre.
fn titre_court(s: &str) -> String {
    let s = s.trim().trim_end_matches(['.', ':', ';', ',']);
    let mots: Vec<&str> = s.split_whitespace().collect();
    if s.chars().count() <= 50 {
        return s.to_string();
    }
    let mut t = String::new();
    for m in mots {
        if t.chars().count() + m.chars().count() > 42 {
            break;
        }
        if !t.is_empty() {
            t.push(' ');
        }
        t.push_str(m);
    }
    let t = t.trim_end_matches([',', ';', ':']).to_string();
    format!("{t}…")
}

/// Le thème qui convient au vocabulaire du texte.
fn theme_pour(texte: &str) -> Theme {
    let m = minuscules(texte);
    let compte = |mots: &[&str]| mots.iter().map(|w| m.matches(w).count()).sum::<usize>();
    let scores = [
        (
            Theme::Clair,
            compte(&[
                "budget",
                "client",
                "vente",
                "chiffre",
                "trimestre",
                "objectif",
                "revenue",
                "sales",
                "quarter",
                "kpi",
                "marge",
                "€",
                "$",
                "ventas",
                "umsatz",
                "fatturato",
            ]),
        ),
        (
            Theme::Sombre,
            compte(&[
                "logiciel",
                "code",
                "serveur",
                "api",
                "données",
                "software",
                "server",
                "data",
                "cloud",
                "sécurité",
                "security",
                "release",
                "version",
                "bug",
                "déploiement",
            ]),
        ),
        (
            Theme::Craie,
            compte(&[
                "cours",
                "élève",
                "école",
                "leçon",
                "exercice",
                "formation",
                "étudiant",
                "lesson",
                "student",
                "school",
                "class",
                "escuela",
                "schule",
                "scuola",
                "aula",
            ]),
        ),
        (
            Theme::Corail,
            compte(&[
                "fête",
                "anniversaire",
                "soirée",
                "événement",
                "équipe",
                "bienvenue",
                "party",
                "event",
                "team",
                "welcome",
                "fiesta",
                "festa",
                "feier",
                "célébr",
            ]),
        ),
        (
            Theme::Elegant,
            compte(&[
                "rapport",
                "assemblée",
                "conseil",
                "comité",
                "stratégie",
                "report",
                "board",
                "committee",
                "strategy",
                "annual",
                "annuel",
                "informe",
                "bericht",
                "relazione",
            ]),
        ),
    ];
    scores
        .iter()
        .filter(|(_, n)| *n > 0)
        .max_by_key(|(_, n)| *n)
        .map_or(Theme::Ocean, |(t, _)| *t)
}

/// La présentation tirée d'un texte libre (voir le module).
pub fn deck_from_text(text: &str, closing: Closing<'_>) -> Deck {
    // Texte borné, fins de ligne unifiées, caractères de contrôle retirés.
    let texte: String = text
        .chars()
        .take(MAX_CARACTERES)
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t' | '\r'))
        .collect::<String>()
        .replace("\r\n", "\n")
        .replace('\r', "\n");

    let mut objet: Option<String> = None;
    let mut auteur: Option<String> = None;
    let mut lignes: Vec<&str> = Vec::new();
    let mut en_tetes = true;
    for (k, l) in texte.split('\n').enumerate() {
        if en_tetes && k < 20 {
            if let Some((nom, valeur)) = entete(l) {
                if est_objet(&nom) && !valeur.is_empty() {
                    let mut v = valeur.as_str();
                    'prefixes: loop {
                        for p in ["re:", "tr:", "fw:", "fwd:", "aw:", "wg:", "rv:", "r:"] {
                            if v.len() >= p.len()
                                && v.is_char_boundary(p.len())
                                && v[..p.len()].eq_ignore_ascii_case(p)
                            {
                                v = v[p.len()..].trim_start();
                                continue 'prefixes;
                            }
                        }
                        break;
                    }
                    let v = v.trim().to_string();
                    objet = Some(v);
                } else if est_expediteur(&nom) && !valeur.is_empty() {
                    let nom = valeur
                        .split('<')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_matches('"')
                        .to_string();
                    if !nom.is_empty() {
                        auteur = Some(nom);
                    }
                }
                continue;
            }
            if l.trim().is_empty() && lignes.is_empty() {
                continue;
            }
            en_tetes = false;
        }
        if signature(l) && !lignes.is_empty() {
            break;
        }
        if salutation(l) && lignes.iter().all(|l| l.trim().is_empty()) {
            continue;
        }
        lignes.push(l);
    }

    // Blocs séparés par des lignes vides.
    let mut blocs: Vec<Vec<&str>> = Vec::new();
    let mut courant: Vec<&str> = Vec::new();
    for l in lignes {
        if l.trim().is_empty() {
            if !courant.is_empty() {
                blocs.push(std::mem::take(&mut courant));
            }
        } else {
            courant.push(l);
        }
    }
    if !courant.is_empty() {
        blocs.push(courant);
    }

    // Le titre de la présentation.
    let mut titre = objet.clone();
    if titre.is_none()
        && let Some(premier) = blocs.first()
        && (premier.len() == 1 || premier[0].trim_start().starts_with('#'))
        && titre_possible(premier[0])
        && (blocs.len() > 1 || premier.len() > 1)
    {
        titre = Some(sans_marque_de_titre(premier[0]));
        if premier.len() == 1 {
            blocs.remove(0);
        } else if let Some(b) = blocs.first_mut() {
            b.remove(0);
        }
    }

    // Les diapositives de contenu.
    let mut brouillons: Vec<Brouillon> = Vec::new();
    let mut ouvert = false;
    for bloc in &blocs {
        let mut lignes: &[&str] = bloc;
        // Une ligne de titre : un bloc d'une seule ligne courte, `# Titre`, ou
        // une ligne terminée par « : » suivie d'autres lignes.
        let premiere = lignes[0];
        let seul = lignes.len() == 1 && titre_possible(premiere);
        // Une ligne courte (six mots au plus) au-dessus d'un paragraphe : un titre.
        let courte = lignes.len() > 1
            && titre_possible(premiere)
            && premiere.split_whitespace().count() <= 6
            && puce(lignes[1]).is_none()
            && lignes[1].trim().chars().count() > premiere.trim().chars().count();
        let entete_explicite = premiere.trim_start().starts_with('#')
            || courte
            || (premiere.trim_end().ends_with(':') && titre_possible(premiere) && lignes.len() > 1);
        // « Présents : Alice, Bob » : un libellé court et sa valeur.
        let libelle = (lignes.len() == 1 && puce(premiere).is_none())
            .then(|| premiere.split_once(':'))
            .flatten()
            .filter(|(a, b)| {
                let a = a.trim();
                !a.is_empty()
                    && a.chars().count() <= 30
                    && a.split_whitespace().count() <= 4
                    && !b.trim().is_empty()
                    && !a.contains("http")
            });
        if libelle.is_none()
            && seul
            && !premiere.trim().ends_with(':')
            && !premiere.trim_start().starts_with('#')
        {
            // Bloc d'une ligne courte : un titre de diapositive, sauf s'il clôt un
            // contenu (une phrase seule à la fin, sans rien après).
            brouillons.push(Brouillon {
                titre: sans_marque_de_titre(premiere),
                ..Brouillon::default()
            });
            ouvert = true;
            continue;
        }
        if entete_explicite || (seul && premiere.trim().ends_with(':')) {
            brouillons.push(Brouillon {
                titre: sans_marque_de_titre(premiere),
                ..Brouillon::default()
            });
            ouvert = true;
            lignes = &lignes[1..];
        } else if let Some((a, b)) = libelle {
            brouillons.push(Brouillon {
                titre: sans_marque_de_titre(a),
                elements: vec![(1, b.trim().to_string())],
                deduit: false,
            });
            ouvert = false;
            continue;
        }
        // Le contenu du bloc.
        let mut elements: Vec<Element> = Vec::new();
        let mut paragraphe = String::new();
        let base = lignes.iter().map(|l| retrait(l)).min().unwrap_or(0);
        let vider = |p: &mut String, el: &mut Vec<Element>| {
            if p.trim().is_empty() {
                p.clear();
                return;
            }
            for ph in phrases(p) {
                el.push((1, ph));
            }
            p.clear();
        };
        for l in lignes {
            if let Some(t) = puce(l) {
                vider(&mut paragraphe, &mut elements);
                let niveau = 1 + (retrait(l).saturating_sub(base) / 2).min(3);
                if !t.is_empty() {
                    elements.push((niveau, t.to_string()));
                }
            } else {
                if !paragraphe.is_empty() {
                    paragraphe.push(' ');
                }
                paragraphe.push_str(l.trim());
            }
        }
        vider(&mut paragraphe, &mut elements);
        if elements.is_empty() {
            continue;
        }
        let ajouter_a_l_ouvert = ouvert
            && brouillons
                .last()
                .is_some_and(|b| b.elements.len() + elements.len() <= MAX_PUCES * 2);
        if ajouter_a_l_ouvert {
            if let Some(b) = brouillons.last_mut() {
                b.elements.extend(elements);
            }
        } else {
            // Contenu sans titre : celui de la présentation pour le premier,
            // sinon la première phrase.
            let premier = elements[0].1.clone();
            let court = premier.chars().count() <= 60;
            let (t, reste) = if brouillons.is_empty()
                && objet.is_none()
                && let Some(t) = &titre
            {
                (t.clone(), elements)
            } else if court && elements.len() > 1 {
                (titre_court(&premier), elements[1..].to_vec())
            } else {
                (titre_court(&premier), elements)
            };
            brouillons.push(Brouillon {
                titre: t,
                elements: reste,
                deduit: true,
            });
            ouvert = true;
        }
        if brouillons.len() > MAX_DIAPOSITIVES {
            break;
        }
    }

    // Un titre de diapositive sans contenu, suivi d'un autre : il devient puce
    // de la diapositive précédente… sauf s'il est le seul de son espèce.
    let mut slides: Vec<Slide> = Vec::new();
    let titre_general = titre.clone().unwrap_or_else(|| {
        brouillons
            .first()
            .map(|b| b.titre.clone())
            .unwrap_or_default()
    });
    let mut titre_slide = Slide {
        layout: Layout::Title,
        title: titre_general.clone(),
        subtitle: auteur.unwrap_or_default(),
        transition: Transition::Fondu,
        ..Slide::default()
    };
    if titre.is_none() && brouillons.len() > 1 && brouillons[0].elements.is_empty() {
        // La première diapositive n'était qu'un titre : c'est celui de la présentation.
        brouillons.remove(0);
    } else if titre.is_none() && brouillons.first().is_some_and(|b| b.deduit) {
        titre_slide.title = brouillons[0].titre.clone();
    }
    slides.push(titre_slide);
    for b in brouillons {
        let morceaux: Vec<&[Element]> = if b.elements.is_empty() {
            vec![&[][..]]
        } else {
            b.elements.chunks(MAX_PUCES).collect()
        };
        let n = morceaux.len();
        for (k, m) in morceaux.into_iter().enumerate() {
            let body = m
                .iter()
                .map(|(niveau, t)| format!("{}- {}", "  ".repeat(niveau - 1), t))
                .collect::<Vec<_>>()
                .join("\n");
            let title = if n > 1 && k > 0 {
                format!("{} ({}/{n})", b.titre, k + 1)
            } else {
                b.titre.clone()
            };
            slides.push(Slide {
                layout: if body.is_empty() {
                    Layout::Section
                } else {
                    Layout::TitleContent
                },
                title,
                body,
                transition: Transition::Fondu,
                ..Slide::default()
            });
            if slides.len() >= MAX_DIAPOSITIVES {
                break;
            }
        }
        if slides.len() >= MAX_DIAPOSITIVES {
            break;
        }
    }
    slides.push(Slide {
        layout: Layout::Title,
        title: closing.thanks.to_string(),
        subtitle: closing.questions.to_string(),
        transition: Transition::Fondu,
        ..Slide::default()
    });

    let mut deck = Deck::new();
    deck.title = titre_general;
    deck.theme = theme_pour(&texte);
    deck.slides = slides;
    deck
}

impl Deck {
    /// La présentation tirée d'un texte libre : voir [`deck_from_text`].
    pub fn from_text(text: &str, closing: Closing<'_>) -> Deck {
        deck_from_text(text, closing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoupage_en_phrases() {
        assert_eq!(
            phrases("Le budget est validé. Il augmente de 12,5 %. M. Durand signe."),
            [
                "Le budget est validé.",
                "Il augmente de 12,5 %.",
                "M. Durand signe."
            ]
        );
        assert_eq!(
            phrases("Vraiment ?! Oui. 3.5 points"),
            ["Vraiment ?!", "Oui.", "3.5 points"]
        );
        assert_eq!(phrases("Pas de point"), ["Pas de point"]);
        assert!(phrases("").is_empty());
        assert_eq!(phrases("Voir p. 12. Fin."), ["Voir p. 12.", "Fin."]);
    }

    #[test]
    fn notes_de_reunion() {
        let notes = "Réunion d'équipe du 3 novembre\n\nPrésents : Alice, Bob, Chloé\n\nBudget\nLe budget 2027 est validé. Il augmente de 12 %. La part marketing reste stable.\n\nRecrutement :\n- deux développeurs\n- un designer\n  - à temps partiel\n\nProchaine réunion le 10 novembre.";
        let d = deck_from_text(notes, Closing::default());
        let titres: Vec<&str> = d.slides.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(
            titres,
            [
                "Réunion d'équipe du 3 novembre",
                "Présents",
                "Budget",
                "Recrutement",
                "Merci !"
            ]
        );
        assert_eq!(d.title, "Réunion d'équipe du 3 novembre");
        assert_eq!(d.slides[0].layout, Layout::Title);
        assert_eq!(d.slides[1].body, "- Alice, Bob, Chloé");
        assert_eq!(
            d.slides[2].body,
            "- Le budget 2027 est validé.\n- Il augmente de 12 %.\n- La part marketing reste stable."
        );
        // La phrase isolée de la fin rejoint la diapositive ouverte.
        assert_eq!(
            d.slides[3].body,
            "- deux développeurs\n- un designer\n  - à temps partiel\n- Prochaine réunion le 10 novembre."
        );
        assert_eq!(d.slides[4].subtitle, "Questions ?");
        assert_eq!(d.theme, Theme::Clair);
    }

    #[test]
    fn courriel() {
        let mail = "De : Camille Martin <camille@example.com>\nObjet : Lancement du produit\nDate : 3 novembre 2026\n\nBonjour à tous,\n\nLe lancement est prévu le 15 janvier. Nous visons 500 clients la première année. La campagne commencera en décembre.\n\nPoints à valider :\n- le prix\n- la date\n\nCordialement,\nCamille";
        let c = Closing {
            thanks: "Thank you!",
            questions: "Questions?",
        };
        let d = deck_from_text(mail, c);
        assert_eq!(d.title, "Lancement du produit");
        assert_eq!(d.slides[0].subtitle, "Camille Martin");
        assert_eq!(d.slides.last().unwrap().title, "Thank you!");
        let corps: String = d.slides.iter().map(|s| s.body.clone()).collect();
        assert!(!corps.contains("Bonjour"));
        assert!(!corps.contains("Cordialement"));
        assert!(!corps.contains("Camille\n"));
        assert_eq!(d.slides[2].title, "Points à valider");
        assert_eq!(d.slides[2].body, "- le prix\n- la date");
        // Paragraphe sans titre : titre tiré de la première phrase.
        assert_eq!(d.slides[1].title, "Le lancement est prévu le 15 janvier");
        assert_eq!(d.slides[1].body.lines().count(), 2);
    }

    #[test]
    fn longue_liste_continuee() {
        let texte = format!(
            "Courses\n\n{}",
            (1..=14)
                .map(|i| format!("- article {i}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        let d = deck_from_text(&texte, Closing::default());
        // Titre, 3 diapositives de 6 / 6 / 2 puces, clôture.
        assert_eq!(d.slides.len(), 5, "{:#?}", d.slides);
        assert_eq!(d.slides[1].title, "Courses");
        assert_eq!(d.slides[2].title, "Courses (2/3)");
        assert_eq!(d.slides[3].body.lines().count(), 2);
    }

    #[test]
    fn theme_selon_le_vocabulaire() {
        assert_eq!(
            theme_pour("Le cours de demain : exercice et leçon"),
            Theme::Craie
        );
        assert_eq!(theme_pour("rien de spécial"), Theme::Ocean);
        assert_eq!(
            theme_pour("Release notes: new API version, server fixes"),
            Theme::Sombre
        );
    }

    #[test]
    fn textes_vides_ou_etranges() {
        for t in [
            "",
            "\n\n\n",
            "   ",
            "Bonjour,",
            "--",
            "a",
            "#",
            ":",
            "- ",
            "1)",
            "\u{0}\u{7}",
        ] {
            let d = deck_from_text(t, Closing::default());
            assert!(d.slides.len() >= 2, "{t:?}");
            assert_eq!(d.slides.last().unwrap().title, "Merci !");
        }
    }

    #[test]
    fn textes_aleatoires_sans_panique() {
        let mut graine: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut alea = || {
            graine ^= graine << 13;
            graine ^= graine >> 7;
            graine ^= graine << 17;
            graine
        };
        let morceaux = [
            "\n",
            "\n\n",
            " ",
            "\t",
            ".",
            ". ",
            "?",
            "!",
            "…",
            "- ",
            "* ",
            "1. ",
            "a) ",
            ":",
            "#",
            "Objet : ",
            "De : ",
            "Bonjour",
            "Cordialement",
            "Le",
            "Très",
            "é",
            "🎉",
            "«",
            "»",
            "\r",
            "\u{0}",
            "M.",
            "3.5",
            "x",
            "—",
            "|",
            "{{",
            "}}",
            "**",
        ];
        for _ in 0..600 {
            let n = (alea() % 120) as usize;
            let t: String = (0..n)
                .map(|_| morceaux[(alea() % morceaux.len() as u64) as usize])
                .collect();
            let d = deck_from_text(&t, Closing::default());
            assert!(d.slides.len() >= 2 && d.slides.len() <= MAX_DIAPOSITIVES + 2);
            // La présentation produite s'exporte sans encombre.
            let _ = d.to_json();
        }
        // Très long texte : borné.
        let long = "Une phrase assez longue pour remplir. ".repeat(20_000);
        let d = deck_from_text(&long, Closing::default());
        assert!(d.slides.len() <= MAX_DIAPOSITIVES + 2);
    }
}
