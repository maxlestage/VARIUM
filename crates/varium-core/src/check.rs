//! Révision : vérification de l'écriture selon la langue du document, comme la
//! correction grammaticale de Writer ou de Pages. Seul le texte écrit dans la
//! source est examiné : les champs `{{…}}` et les formules `$…$` sont ignorés.
//!
//! Chaque remarque donne sa position dans la source (octets) et, quand elle est
//! sûre, la correction à appliquer.

use crate::lang::Lang;
use crate::parse::{Segment, parse};

/// Orthographe (dictionnaire) : voir [`crate::spell`].
pub use crate::spell::{Speller, check_spelling};

/// Le genre d'une remarque, pour l'affichage et le filtrage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Espaces (doubles, avant ou après la ponctuation).
    Espace,
    /// Ponctuation propre à la langue (¿ ¡, guillemets, insécables).
    Ponctuation,
    /// Majuscule en début de phrase ou pronom « I ».
    Majuscule,
    /// Mot répété.
    Repetition,
    /// Accord ou mot fréquemment mal écrit.
    Orthographe,
    /// Parenthèse ou guillemet non fermé.
    Equilibre,
}

impl Kind {
    /// Libellé dans une langue de l'interface.
    pub fn label_in(self, lang: Lang) -> &'static str {
        let t: [&'static str; 6] = match self {
            Kind::Espace => [
                "Espace",
                "Spacing",
                "Espacio",
                "Leerzeichen",
                "Spazio",
                "Espaço",
            ],
            Kind::Ponctuation => [
                "Ponctuation",
                "Punctuation",
                "Puntuación",
                "Zeichensetzung",
                "Punteggiatura",
                "Pontuação",
            ],
            Kind::Majuscule => [
                "Majuscule",
                "Capitalisation",
                "Mayúscula",
                "Großschreibung",
                "Maiuscola",
                "Maiúscula",
            ],
            Kind::Repetition => [
                "Répétition",
                "Repetition",
                "Repetición",
                "Wiederholung",
                "Ripetizione",
                "Repetição",
            ],
            Kind::Orthographe => [
                "Orthographe",
                "Spelling",
                "Ortografía",
                "Rechtschreibung",
                "Ortografia",
                "Ortografia",
            ],
            Kind::Equilibre => [
                "Parenthèses",
                "Brackets",
                "Paréntesis",
                "Klammern",
                "Parentesi",
                "Parênteses",
            ],
        };
        t[Lang::ALL.iter().position(|&l| l == lang).unwrap_or(0)]
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Espace => "Espace",
            Kind::Ponctuation => "Ponctuation",
            Kind::Majuscule => "Majuscule",
            Kind::Repetition => "Répétition",
            Kind::Orthographe => "Orthographe",
            Kind::Equilibre => "Parenthèses",
        }
    }
}

/// Le code d'une remarque : il donne son message dans chaque langue de
/// l'interface ([`Issue::message_in`]), les `{0}`, `{1}` étant remplacés par
/// [`Issue::args`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Code {
    /// Plusieurs espaces de suite.
    EspacesEnTrop,
    /// Espace avant une virgule ou un point.
    PasEspaceAvant,
    /// Espace manquante après une virgule ou un point-virgule.
    EspaceManquante,
    /// Français : espace avant `; : ! ?`.
    EspaceAvantFr,
    /// Autres langues : pas d'espace avant `; : ! ?`.
    PasEspaceAvantLangue,
    /// Mot répété.
    MotRepete,
    /// Mot fréquemment mal écrit (argument : la bonne orthographe).
    OnEcrit,
    /// Anglais : « an » devant un son de voyelle.
    AnVoyelle,
    /// Anglais : « a » devant un son de consonne.
    AConsonne,
    /// Anglais : « I » en majuscule.
    PronomI,
    /// Espagnol : ¿ ou ¡ en tête de phrase.
    OuvertureEs,
    /// Majuscule en début de phrase.
    MajusculePhrase,
    /// Parenthèse ou crochet fermant sans ouvrant.
    SansOuverture,
    /// Parenthèse ou guillemet ouvrant jamais fermé.
    JamaisFerme,
    /// Mot absent du dictionnaire.
    Inconnu,
    /// Mot absent du dictionnaire, avec une suggestion.
    InconnuSuggestion,
}

impl Code {
    pub const ALL: [Code; 16] = [
        Code::EspacesEnTrop,
        Code::PasEspaceAvant,
        Code::EspaceManquante,
        Code::EspaceAvantFr,
        Code::PasEspaceAvantLangue,
        Code::MotRepete,
        Code::OnEcrit,
        Code::AnVoyelle,
        Code::AConsonne,
        Code::PronomI,
        Code::OuvertureEs,
        Code::MajusculePhrase,
        Code::SansOuverture,
        Code::JamaisFerme,
        Code::Inconnu,
        Code::InconnuSuggestion,
    ];

    /// Les modèles du message : français, anglais, espagnol, allemand, italien,
    /// portugais (ordre de [`Lang::ALL`]).
    fn modeles(self) -> [&'static str; 6] {
        match self {
            Code::EspacesEnTrop => [
                "Espaces en trop",
                "Extra spaces",
                "Espacios de más",
                "Überflüssige Leerzeichen",
                "Spazi in eccesso",
                "Espaços a mais",
            ],
            Code::PasEspaceAvant => [
                "Pas d'espace avant « {0} »",
                "No space before “{0}”",
                "Sin espacio antes de «{0}»",
                "Kein Leerzeichen vor „{0}“",
                "Nessuno spazio prima di «{0}»",
                "Sem espaço antes de «{0}»",
            ],
            Code::EspaceManquante => [
                "Espace manquante après « {0} »",
                "Missing space after “{0}”",
                "Falta un espacio después de «{0}»",
                "Fehlendes Leerzeichen nach „{0}“",
                "Manca uno spazio dopo «{0}»",
                "Falta um espaço depois de «{0}»",
            ],
            Code::EspaceAvantFr => [
                "En français, une espace précède « {0} »",
                "In French, a space comes before “{0}”",
                "En francés, se pone un espacio antes de «{0}»",
                "Im Französischen steht vor „{0}“ ein Leerzeichen",
                "In francese, «{0}» è preceduto da uno spazio",
                "Em francês, «{0}» é precedido de um espaço",
            ],
            Code::PasEspaceAvantLangue => [
                "Pas d'espace avant « {0} » dans cette langue",
                "No space before “{0}” in this language",
                "En este idioma no se pone espacio antes de «{0}»",
                "In dieser Sprache kein Leerzeichen vor „{0}“",
                "In questa lingua niente spazio prima di «{0}»",
                "Nesta língua não se põe espaço antes de «{0}»",
            ],
            Code::MotRepete => [
                "Mot répété : « {0} »",
                "Repeated word: “{0}”",
                "Palabra repetida: «{0}»",
                "Wiederholtes Wort: „{0}“",
                "Parola ripetuta: «{0}»",
                "Palavra repetida: «{0}»",
            ],
            Code::OnEcrit => [
                "On écrit « {0} »",
                "The correct spelling is “{0}”",
                "Se escribe «{0}»",
                "Richtig ist „{0}“",
                "Si scrive «{0}»",
                "Escreve-se «{0}»",
            ],
            Code::AnVoyelle => [
                "« {0} » devant un son de voyelle",
                "“{0}” before a vowel sound",
                "«{0}» delante de un sonido vocálico",
                "„{0}“ vor einem Vokallaut",
                "«{0}» davanti a un suono vocalico",
                "«{0}» antes de um som de vogal",
            ],
            Code::AConsonne => [
                "« {0} » devant un son de consonne",
                "“{0}” before a consonant sound",
                "«{0}» delante de un sonido consonántico",
                "„{0}“ vor einem Konsonantenlaut",
                "«{0}» davanti a un suono consonantico",
                "«{0}» antes de um som de consoante",
            ],
            Code::PronomI => [
                "Le pronom anglais « I » prend toujours une majuscule",
                "The pronoun “I” is always capitalised",
                "El pronombre inglés «I» siempre va en mayúscula",
                "Das englische Pronomen „I“ wird immer großgeschrieben",
                "Il pronome inglese «I» vuole sempre la maiuscola",
                "O pronome inglês «I» leva sempre maiúscula",
            ],
            Code::OuvertureEs => [
                "En espagnol, la phrase commence par « {0} »",
                "In Spanish, the sentence starts with “{0}”",
                "En español, la oración empieza por «{0}»",
                "Im Spanischen beginnt der Satz mit „{0}“",
                "In spagnolo, la frase inizia con «{0}»",
                "Em espanhol, a frase começa por «{0}»",
            ],
            Code::MajusculePhrase => [
                "Majuscule en début de phrase",
                "Capital letter at the start of a sentence",
                "Mayúscula al principio de la oración",
                "Großschreibung am Satzanfang",
                "Maiuscola a inizio frase",
                "Maiúscula no início da frase",
            ],
            Code::SansOuverture => [
                "« {0} » sans ouverture",
                "“{0}” without an opening match",
                "«{0}» sin apertura",
                "„{0}“ ohne Öffnung",
                "«{0}» senza apertura",
                "«{0}» sem abertura",
            ],
            Code::JamaisFerme => [
                "« {0} » jamais fermé",
                "“{0}” never closed",
                "«{0}» sin cerrar",
                "„{0}“ nie geschlossen",
                "«{0}» mai chiuso",
                "«{0}» nunca fechado",
            ],
            Code::Inconnu => [
                "« {0} » est inconnu du dictionnaire",
                "“{0}” is not in the dictionary",
                "«{0}» no está en el diccionario",
                "„{0}“ steht nicht im Wörterbuch",
                "«{0}» non è nel dizionario",
                "«{0}» não está no dicionário",
            ],
            Code::InconnuSuggestion => [
                "« {0} » est inconnu du dictionnaire : « {1} » ?",
                "“{0}” is not in the dictionary: “{1}”?",
                "«{0}» no está en el diccionario: ¿«{1}»?",
                "„{0}“ steht nicht im Wörterbuch: „{1}“?",
                "«{0}» non è nel dizionario: «{1}»?",
                "«{0}» não está no dicionário: «{1}»?",
            ],
        }
    }

    /// Le modèle du message dans une langue (`{0}`, `{1}` : les arguments).
    pub fn template(self, lang: Lang) -> &'static str {
        let i = Lang::ALL.iter().position(|&l| l == lang).unwrap_or(0);
        self.modeles()[i]
    }

    /// Le message dans une langue, arguments remplis (un argument manquant
    /// devient vide).
    pub fn message(self, lang: Lang, args: &[String]) -> String {
        let mut out = self.template(lang).to_string();
        for k in 0..9 {
            let marque = format!("{{{k}}}");
            if out.contains(&marque) {
                out = out.replace(&marque, args.get(k).map_or("", String::as_str));
            }
        }
        out
    }
}

/// Une remarque de la révision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Position dans la source, en octets.
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
    /// Explication, en français (voir [`Issue::message_in`] pour les autres langues).
    pub message: String,
    /// Le texte qui remplace `start..end`, quand la correction est sûre.
    pub replacement: Option<String>,
    /// Le genre précis de la remarque, qui donne son message dans chaque langue.
    pub code: Code,
    /// Les éléments variables du message (mot, signe, correction).
    pub args: Vec<String>,
}

impl Issue {
    /// Une remarque, son message français tiré de son code.
    pub fn new(
        start: usize,
        end: usize,
        kind: Kind,
        code: Code,
        args: Vec<String>,
        replacement: Option<String>,
    ) -> Issue {
        Issue {
            start,
            end,
            kind,
            message: code.message(Lang::Fr, &args),
            replacement,
            code,
            args,
        }
    }

    /// L'explication dans la langue `lang` (celle de l'interface).
    pub fn message_in(&self, lang: Lang) -> String {
        self.code.message(lang, &self.args)
    }
}

/// Les zones de texte vérifiables : hors champs et hors formules.
pub(crate) fn zones(src: &str) -> Vec<(usize, usize)> {
    // 1. hors champs {{…}}
    let mut text_zones = Vec::new();
    let mut last = 0;
    for seg in parse(src) {
        if let Segment::Var(v) = seg {
            if v.range.start > last {
                text_zones.push((last, v.range.start));
            }
            last = v.range.end;
        }
    }
    if last < src.len() {
        text_zones.push((last, src.len()));
    }
    // 2. hors formules $…$ (les `\$` ne comptent pas)
    let bytes = src.as_bytes();
    let mut in_math = false;
    let mut out = Vec::new();
    for (a, b) in text_zones {
        let mut start = a;
        let mut i = a;
        while i < b {
            if bytes[i] == b'$' && (i == 0 || bytes[i - 1] != b'\\') {
                if !in_math && i > start {
                    out.push((start, i));
                }
                in_math = !in_math;
                start = i + 1;
            }
            i += 1;
        }
        if !in_math && start < b {
            out.push((start, b));
        }
    }
    out
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '\'' || c == '’' || c == '-'
}

/// Les mots d'une zone : (début, fin) en octets.
fn words(src: &str, (a, b): (usize, usize)) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in src[a..b].char_indices() {
        let i = a + i;
        if is_word(c) && !(c == '-' && start.is_none()) {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            out.push((s, i));
        }
    }
    if let Some(s) = start {
        out.push((s, b));
    }
    out
}

fn issue(
    start: usize,
    end: usize,
    kind: Kind,
    code: Code,
    args: &[&dyn std::fmt::Display],
    rep: Option<String>,
) -> Issue {
    Issue::new(
        start,
        end,
        kind,
        code,
        args.iter().map(|a| a.to_string()).collect(),
        rep,
    )
}

/// Mots qui peuvent légitimement se répéter.
fn repetition_allowed(lang: Lang, w: &str) -> bool {
    let w = w.to_lowercase();
    if w.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    match lang {
        Lang::Fr => matches!(
            w.as_str(),
            "nous" | "vous" | "si" | "très" | "bien" | "tout"
        ),
        Lang::En => matches!(w.as_str(), "that" | "had" | "is" | "very" | "so"),
        Lang::De => matches!(w.as_str(), "die" | "der" | "das" | "sehr"),
        Lang::Es | Lang::Pt | Lang::It => matches!(w.as_str(), "muy" | "molto" | "muito"),
    }
}

/// Abréviations après lesquelles un point n'achève pas la phrase.
fn is_abbreviation(word: &str) -> bool {
    let w = word.trim_end_matches('.').to_lowercase();
    w.chars().count() == 1
        || matches!(
            w.as_str(),
            "etc"
                | "ex"
                | "cf"
                | "p"
                | "pp"
                | "env"
                | "mr"
                | "mrs"
                | "ms"
                | "dr"
                | "st"
                | "e.g"
                | "i.e"
                | "vs"
                | "approx"
                | "no"
                | "nr"
                | "bzw"
                | "usw"
                | "sr"
                | "sra"
                | "dra"
                | "ca"
                | "art"
                | "mme"
                | "mlle"
                | "me"
                | "av"
                | "tel"
                | "tél"
                | "chap"
                | "vol"
                | "fig"
                | "z.b"
                | "d.h"
                | "u.a"
                | "pág"
                | "pag"
                | "sig"
                | "ecc"
        )
}

/// Vérifie la source dans la langue donnée. Les remarques sont triées par position.
pub fn check(src: &str, lang: Lang) -> Vec<Issue> {
    let mut out = Vec::new();
    for z in zones(src) {
        check_zone(src, z, lang, &mut out);
    }
    check_lines(src, lang, &mut out);
    out.sort_by_key(|i| (i.start, i.end));
    out.dedup_by(|a, b| a.start == b.start && a.kind == b.kind);
    out
}

fn check_zone(src: &str, (a, b): (usize, usize), lang: Lang, out: &mut Vec<Issue>) {
    let text = &src[a..b];
    let chars: Vec<(usize, char)> = text.char_indices().map(|(i, c)| (a + i, c)).collect();
    let at = |k: usize| chars.get(k).map(|x| x.1);
    let pos = |k: usize| chars.get(k).map_or(b, |x| x.0);

    for k in 0..chars.len() {
        let (p, c) = chars[k];
        let prev = if k > 0 { at(k - 1) } else { None };
        let next = at(k + 1);

        // Doubles espaces (hors début de ligne : retraits des listes)
        // (le caractère d'avant est lu dans la source : la zone peut suivre un champ)
        let before = src[..p].chars().next_back();
        if c == ' ' && next == Some(' ') && before.is_some_and(|x| x != ' ' && x != '\n') {
            let mut e = k + 1;
            while at(e) == Some(' ') {
                e += 1;
            }
            if src[pos(e)..].chars().next().is_some_and(|x| x != '\n') {
                out.push(issue(
                    p,
                    pos(e),
                    Kind::Espace,
                    Code::EspacesEnTrop,
                    &[],
                    Some(" ".into()),
                ));
            }
        }

        // Espace avant la virgule ou le point
        if (c == ',' || c == '.')
            && prev == Some(' ')
            && k >= 2
            && at(k - 2).is_some_and(char::is_alphanumeric)
            && !(c == '.' && next == Some('.'))
        {
            let start = chars[k - 1].0;
            out.push(issue(
                start,
                p + c.len_utf8(),
                Kind::Espace,
                Code::PasEspaceAvant,
                &[&c],
                Some(c.to_string()),
            ));
        }

        // Espace manquante après , ; entre deux lettres
        if (c == ',' || c == ';')
            && prev.is_some_and(char::is_alphabetic)
            && next.is_some_and(char::is_alphabetic)
        {
            out.push(issue(
                p,
                p + 1,
                Kind::Espace,
                Code::EspaceManquante,
                &[&c],
                Some(format!("{c} ")),
            ));
        }

        // Ponctuation haute selon la langue
        if matches!(c, ';' | ':' | '!' | '?') {
            match lang {
                Lang::Fr => {
                    // « Bonjour! » → espace avant (sauf heures, URL, :: ou ?!)
                    let glued = prev.is_some_and(|x| x.is_alphabetic() || x == '»' || x == ')');
                    let url = c == ':' && next == Some('/');
                    if glued && !url {
                        out.push(issue(
                            p,
                            p + 1,
                            Kind::Ponctuation,
                            Code::EspaceAvantFr,
                            &[&c],
                            Some(format!(" {c}")),
                        ));
                    }
                }
                _ => {
                    if prev == Some(' ')
                        && k >= 2
                        && at(k - 2).is_some_and(|x| {
                            x.is_alphanumeric() || x == '”' || x == '“' || x == '»'
                        })
                    {
                        out.push(issue(
                            chars[k - 1].0,
                            p + 1,
                            Kind::Ponctuation,
                            Code::PasEspaceAvantLangue,
                            &[&c],
                            Some(c.to_string()),
                        ));
                    }
                }
            }
        }
    }

    // Phrases : majuscule initiale, ¿ ¡ en espagnol
    sentences(src, (a, b), lang, out);

    // Mots
    let ws = words(src, (a, b));
    for (i, &(s, e)) in ws.iter().enumerate() {
        let w = &src[s..e];
        // Mot répété : « le le »
        if let Some(&(s2, e2)) = ws.get(i + 1) {
            let w2 = &src[s2..e2];
            let between = &src[e..s2];
            if w.to_lowercase() == w2.to_lowercase()
                && between.chars().all(|c| c == ' ')
                && !between.is_empty()
                && !repetition_allowed(lang, w)
            {
                out.push(issue(
                    s,
                    e2,
                    Kind::Repetition,
                    Code::MotRepete,
                    &[&w],
                    Some(w.to_string()),
                ));
            }
        }
        match lang {
            Lang::En => english_word(src, &ws, i, out),
            Lang::It => {
                let fix = match w {
                    "perchè" => Some("perché"),
                    "poichè" => Some("poiché"),
                    "affinchè" => Some("affinché"),
                    "benchè" => Some("benché"),
                    "nè" => Some("né"),
                    "pò" => Some("po’"),
                    "qual'è" | "qual’è" => Some("qual è"),
                    "Perchè" => Some("Perché"),
                    _ => None,
                };
                if let Some(f) = fix {
                    let f = keep_case(w, f);
                    out.push(issue(
                        s,
                        e,
                        Kind::Orthographe,
                        Code::OnEcrit,
                        &[&f],
                        Some(f.clone()),
                    ));
                }
            }
            Lang::Fr => {
                let fix = match w.to_lowercase().as_str() {
                    "quelque-soit" => Some("quel que soit"),
                    "parmis" => Some("parmi"),
                    "malgrés" => Some("malgré"),
                    "language" => Some("langage"),
                    "dilemne" => Some("dilemme"),
                    "connection" => Some("connexion"),
                    "addresse" => Some("adresse"),
                    "apparament" => Some("apparemment"),
                    _ => None,
                };
                if let Some(f) = fix {
                    let f = keep_case(w, f);
                    out.push(issue(
                        s,
                        e,
                        Kind::Orthographe,
                        Code::OnEcrit,
                        &[&f],
                        Some(f.clone()),
                    ));
                }
            }
            // En allemand, « das »/« dass » est trop ambigu pour une règle sûre.
            Lang::De => {}
            Lang::Es => {
                let fix = match w {
                    "aveces" => Some("a veces"),
                    "osea" => Some("o sea"),
                    "porfavor" => Some("por favor"),
                    _ => None,
                };
                if let Some(f) = fix {
                    let f = keep_case(w, f);
                    out.push(issue(
                        s,
                        e,
                        Kind::Orthographe,
                        Code::OnEcrit,
                        &[&f],
                        Some(f.clone()),
                    ));
                }
            }
            Lang::Pt => {
                let fix = match w {
                    "concerteza" => Some("com certeza"),
                    "derrepente" => Some("de repente"),
                    _ => None,
                };
                if let Some(f) = fix {
                    let f = keep_case(w, f);
                    out.push(issue(
                        s,
                        e,
                        Kind::Orthographe,
                        Code::OnEcrit,
                        &[&f],
                        Some(f.clone()),
                    ));
                }
            }
        }
    }
}

/// La correction avec la majuscule initiale du mot d'origine.
fn keep_case(original: &str, fix: &str) -> String {
    if original.chars().next().is_some_and(char::is_uppercase) {
        let mut c = fix.chars();
        c.next()
            .map(|f| f.to_uppercase().chain(c).collect())
            .unwrap_or_default()
    } else {
        fix.to_string()
    }
}

/// a/an et « I » en anglais.
fn english_word(src: &str, ws: &[(usize, usize)], i: usize, out: &mut Vec<Issue>) {
    let (s, e) = ws[i];
    let w = &src[s..e];
    if w == "i" {
        out.push(issue(
            s,
            e,
            Kind::Majuscule,
            Code::PronomI,
            &[],
            Some("I".into()),
        ));
    }
    let Some(&(s2, e2)) = ws.get(i + 1) else {
        return;
    };
    if src[e..s2] != *" " {
        return;
    }
    let next = src[s2..e2].to_lowercase();
    let first = next.chars().next().unwrap_or('x');
    // Exceptions courantes : le son compte, pas la lettre.
    let vowel_sound = (matches!(first, 'a' | 'e' | 'i' | 'o' | 'u')
        && !["uni", "use", "usu", "uti", "eu", "one", "once", "ur"]
            .iter()
            .any(|p| next.starts_with(p)))
        || ["hour", "honest", "honou", "heir"]
            .iter()
            .any(|p| next.starts_with(p));
    let is_acronym = src[s2..e2].chars().all(|c| c.is_ascii_uppercase()) && e2 - s2 > 1;
    if is_acronym {
        return;
    }
    match w {
        "a" | "A" if vowel_sound => {
            let rep = if w == "A" { "An" } else { "an" };
            out.push(issue(
                s,
                e,
                Kind::Orthographe,
                Code::AnVoyelle,
                &[&rep],
                Some(rep.into()),
            ));
        }
        "an" | "An" if !vowel_sound => {
            let rep = if w == "An" { "A" } else { "a" };
            out.push(issue(
                s,
                e,
                Kind::Orthographe,
                Code::AConsonne,
                &[&rep],
                Some(rep.into()),
            ));
        }
        _ => {}
    }
}

/// Majuscule au début des phrases, et ¿ ¡ en espagnol.
fn sentences(src: &str, (a, b): (usize, usize), lang: Lang, out: &mut Vec<Issue>) {
    let text = &src[a..b];
    let chars: Vec<(usize, char)> = text.char_indices().map(|(i, c)| (a + i, c)).collect();
    let mut sentence_start: Option<usize> = None; // index dans chars
    for k in 0..chars.len() {
        let c = chars[k].1;
        if sentence_start.is_none() && c.is_alphabetic() {
            sentence_start = Some(k);
        }
        let ends = matches!(c, '.' | '!' | '?') || c == '\n';
        if !ends {
            continue;
        }
        if c == '.' {
            // Abréviation, nombre décimal, points de suspension : pas une fin de phrase.
            let word_start = chars[..k]
                .iter()
                .rposition(|(_, x)| !(x.is_alphanumeric() || *x == '.'))
                .map_or(0, |p| p + 1);
            let word: String = chars[word_start..k].iter().map(|x| x.1).collect();
            let next = chars.get(k + 1).map(|x| x.1);
            if is_abbreviation(&word) || next.is_some_and(|x| x.is_alphanumeric() || x == '.') {
                continue;
            }
        }
        // ¿ ¡ en espagnol
        if lang == Lang::Es
            && (c == '?' || c == '!')
            && let Some(st) = sentence_start
        {
            let opening = if c == '?' { '¿' } else { '¡' };
            let has_opening = chars[..k]
                .iter()
                .rev()
                .take(k - st + 2)
                .any(|x| x.1 == opening);
            if !has_opening {
                let (p, first) = chars[st];
                out.push(issue(
                    p,
                    p + first.len_utf8(),
                    Kind::Ponctuation,
                    Code::OuvertureEs,
                    &[&opening],
                    Some(format!("{opening}{first}")),
                ));
            }
        }
        // Majuscule après . ! ? (pas après un retour à la ligne : listes, adresses)
        if c != '\n'
            && let Some((p, first)) = chars[k + 1..]
                .iter()
                .find(|x| !x.1.is_whitespace())
                .copied()
            && chars.get(k + 1).is_some_and(|x| x.1 == ' ')
            && first.is_lowercase()
            && !chars[k + 1..]
                .iter()
                .take_while(|x| x.1 != first)
                .any(|x| x.1 == '\n')
        {
            let up: String = first.to_uppercase().collect();
            out.push(issue(
                p,
                p + first.len_utf8(),
                Kind::Majuscule,
                Code::MajusculePhrase,
                &[],
                Some(up),
            ));
        }
        sentence_start = None;
    }
}

/// Parenthèses et guillemets non fermés, ligne par ligne.
fn check_lines(src: &str, lang: Lang, out: &mut Vec<Issue>) {
    let (open_q, close_q, _) = lang.quotes();
    let mut offset = 0;
    for line in src.split('\n') {
        let mut stack: Vec<(usize, char)> = Vec::new();
        for (i, c) in line.char_indices() {
            match c {
                '(' | '[' => stack.push((offset + i, c)),
                ')' | ']' => {
                    let want = if c == ')' { '(' } else { '[' };
                    if stack.last().is_some_and(|x| x.1 == want) {
                        stack.pop();
                    } else {
                        // « a) b) c) » : une énumération, pas une parenthèse oubliée.
                        let before = line[..i].rsplit(' ').next().unwrap_or("");
                        let enumeration = !before.is_empty()
                            && before.chars().count() <= 2
                            && before.chars().all(char::is_alphanumeric);
                        if !enumeration {
                            out.push(issue(
                                offset + i,
                                offset + i + 1,
                                Kind::Equilibre,
                                Code::SansOuverture,
                                &[&c],
                                None,
                            ));
                        }
                    }
                }
                c if c == open_q && open_q != close_q => stack.push((offset + i, c)),
                c if c == close_q
                    && open_q != close_q
                    && stack.last().is_some_and(|x| x.1 == open_q) =>
                {
                    stack.pop();
                }
                _ => {}
            }
        }
        for (p, c) in stack {
            out.push(issue(
                p,
                p + c.len_utf8(),
                Kind::Equilibre,
                Code::JamaisFerme,
                &[&c],
                None,
            ));
        }
        offset += line.len() + 1;
    }
}

/// Applique une correction à la source (si elle est encore valide).
pub fn apply_fix(src: &mut String, issue: &Issue) -> bool {
    let Some(rep) = &issue.replacement else {
        return false;
    };
    if issue.end > src.len()
        || !src.is_char_boundary(issue.start)
        || !src.is_char_boundary(issue.end)
    {
        return false;
    }
    src.replace_range(issue.start..issue.end, rep);
    true
}

/// Applique toutes les corrections sûres, de la fin vers le début.
pub fn apply_all(src: &mut String, lang: Lang) -> usize {
    let issues = check(src, lang);
    let mut n = 0;
    let mut last_start = usize::MAX;
    for i in issues.iter().rev() {
        if i.end <= last_start && apply_fix(src, i) {
            n += 1;
            last_start = i.start;
        }
    }
    n
}

#[cfg(test)]
mod tests_messages {
    use super::*;

    fn accolades(s: &str) -> Vec<String> {
        let mut v: Vec<String> = s
            .match_indices('{')
            .filter_map(|(i, _)| s[i..].find('}').map(|j| s[i..i + j + 1].to_string()))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn chaque_code_a_un_message_dans_les_six_langues() {
        for c in Code::ALL {
            let fr = c.template(Lang::Fr);
            for l in Lang::ALL {
                let t = c.template(l);
                assert!(!t.trim().is_empty(), "{c:?} en {}", l.code());
                assert_eq!(accolades(t), accolades(fr), "{c:?} en {}", l.code());
            }
        }
        for k in [
            Kind::Espace,
            Kind::Ponctuation,
            Kind::Majuscule,
            Kind::Repetition,
            Kind::Orthographe,
            Kind::Equilibre,
        ] {
            assert_eq!(k.label_in(Lang::Fr), k.label());
            for l in Lang::ALL {
                assert!(!k.label_in(l).is_empty());
            }
        }
        for l in Lang::ALL {
            assert_eq!(l.label_in(Lang::Fr), l.label());
            for ui in Lang::ALL {
                assert!(!l.label_in(ui).is_empty());
            }
        }
    }

    #[test]
    fn remarques_traduites() {
        for (lang, src) in [
            (Lang::Fr, "Bonjour! le le chat (oups"),
            (Lang::En, "i saw a apple ,ok. then"),
            (Lang::Es, "Hola? porfavor"),
            (Lang::Pt, "derrepente  sim"),
            (Lang::It, "perchè no"),
        ] {
            let issues = check(src, lang);
            assert!(!issues.is_empty(), "{src}");
            for i in &issues {
                assert_eq!(i.message, i.message_in(Lang::Fr));
                for l in Lang::ALL {
                    let m = i.message_in(l);
                    assert!(!m.contains('{') && !m.is_empty(), "{m}");
                }
            }
        }
        let i = &check("le le chat", Lang::Fr)[0];
        assert_eq!(i.code, Code::MotRepete);
        assert_eq!(i.message_in(Lang::En), "Repeated word: “le”");
        assert_eq!(i.message_in(Lang::De), "Wiederholtes Wort: „le“");
        // Argument manquant : jamais d'accolade affichée.
        assert_eq!(Code::MotRepete.message(Lang::Fr, &[]), "Mot répété : «  »");
        let sp = crate::spell::spelling_issue("chatt", 0..5, &["chat".to_string()]);
        assert_eq!(
            sp.message_in(Lang::It),
            "«chatt» non è nel dizionario: «chat»?"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str, lang: Lang) -> Vec<(Kind, String)> {
        check(src, lang)
            .into_iter()
            .map(|i| (i.kind, src[i.start..i.end].to_string()))
            .collect()
    }

    fn fixed(src: &str, lang: Lang) -> String {
        let mut s = src.to_string();
        apply_all(&mut s, lang);
        s
    }

    #[test]
    fn regles_communes() {
        assert_eq!(fixed("Bonjour  le monde", Lang::Fr), "Bonjour le monde");
        assert_eq!(
            fixed("Il est venu , puis parti", Lang::Fr),
            "Il est venu, puis parti"
        );
        assert_eq!(fixed("rouge,vert", Lang::Fr), "rouge, vert");
        assert_eq!(fixed("Le le chat", Lang::Fr), "Le chat");
        assert_eq!(fixed("Fin. début", Lang::Fr), "Fin. Début");
        // abréviations, nombres et retraits ne sont pas signalés
        assert!(check("p. ex. voir 3.5 et 3,5\n  - liste", Lang::Fr).is_empty());
        assert!(check("Nous nous voyons.", Lang::Fr).is_empty());
    }

    #[test]
    fn commandes_ignorees() {
        let src = "# Rapport\n\n\\sommaire\n\n## Contexte\n\nLe texte.\n\n\\newpage\n";
        for lang in Lang::ALL {
            assert!(check(src, lang).is_empty(), "{lang:?}");
        }
    }

    #[test]
    fn francais() {
        assert_eq!(fixed("Bonjour! Ça va?", Lang::Fr), "Bonjour ! Ça va ?");
        assert!(check("https://exemple.fr et 10:30", Lang::Fr).is_empty());
        assert_eq!(fixed("Parmis eux", Lang::Fr), "Parmi eux");
    }

    #[test]
    fn anglais() {
        assert_eq!(
            fixed("Hello ! I saw a apple and an car.", Lang::En),
            "Hello! I saw an apple and a car."
        );
        assert_eq!(fixed("Then i left.", Lang::En), "Then I left.");
        assert!(check("an hour, a user, a university, an FAQ", Lang::En).is_empty());
    }

    #[test]
    fn espagnol() {
        assert_eq!(fixed("Cómo estás?", Lang::Es), "¿Cómo estás?");
        assert_eq!(fixed("Qué bien!", Lang::Es), "¡Qué bien!");
        assert!(check("¿Cómo estás? ¡Bien!", Lang::Es).is_empty());
    }

    #[test]
    fn italien_et_equilibre() {
        assert_eq!(fixed("Non so perchè.", Lang::It), "Non so perché.");
        let k = kinds("Une (parenthèse ouverte", Lang::Fr);
        assert_eq!(k, [(Kind::Equilibre, "(".to_string())]);
        assert!(
            check("a) premier b) second", Lang::Fr)
                .iter()
                .all(|i| i.kind != Kind::Equilibre)
        );
    }

    #[test]
    fn champs_et_formules_ignores() {
        assert!(check("{{ nom  }} et $a  ,b$", Lang::Fr).is_empty());
        assert_eq!(fixed("{{x}}  {{y}}", Lang::Fr), "{{x}} {{y}}");
    }

    #[test]
    fn aucune_panique() {
        let mut seed = 7u64;
        let alphabet: Vec<char> = "aé .,;:!?¿¡()[]«»“”\"'{}$\n\\ZZi an a".chars().collect();
        for _ in 0..2000 {
            let len = (seed % 40) as usize;
            let s: String = (0..len)
                .map(|_| {
                    seed = seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    alphabet[(seed >> 33) as usize % alphabet.len()]
                })
                .collect();
            for lang in Lang::ALL {
                let mut t = s.clone();
                apply_all(&mut t, lang);
            }
        }
    }
}
