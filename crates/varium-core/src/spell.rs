//! Orthographe : découpage du texte en mots selon la langue et vérification par
//! un dictionnaire (Hunspell côté application, n'importe quel [`Speller`] ici).
//!
//! Ne sont pas examinés : les champs `{{…}}` et les formules `$…$`, les adresses
//! web et électroniques, les noms de fichiers, les nombres et les mots qui
//! contiennent des chiffres, les sigles en capitales, les commandes
//! (`\newpage`, `\sommaire`), le code entre accents graves et la syntaxe de
//! mise en forme (`#`, listes, tableaux, liens).
//!
//! Les mots composés avec un trait d'union sont d'abord vérifiés en entier
//! (« peut-être »), puis partie par partie ; en français et en italien, les
//! élisions (« l'homme », « qu'il », « dell'anno ») sont séparées, sauf quand le
//! dictionnaire connaît le mot entier (« aujourd'hui »). Les mots composés
//! allemands restent d'un seul tenant : le dictionnaire sait les décomposer.

use std::ops::Range;

use crate::check::{Issue, Kind};
use crate::lang::Lang;

/// Un correcteur orthographique (un dictionnaire Hunspell, par exemple).
pub trait Speller {
    /// Le mot est-il correct ?
    fn check(&self, word: &str) -> bool;
    /// Les corrections proposées, de la plus probable à la moins probable.
    fn suggest(&self, word: &str) -> Vec<String>;
}

/// Les paragraphes de la source (séparés par une ligne vide), en octets. Chaque
/// paragraphe peut être vérifié seul : l'application garde le résultat en cache
/// tant que son texte ne change pas.
pub fn paragraphs(src: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut pos = 0;
    for line in src.split('\n') {
        let end = pos + line.len();
        if line.trim().is_empty() {
            if let Some(s) = start.take() {
                out.push(s..pos.saturating_sub(1).max(s));
            }
        } else if start.is_none() {
            start = Some(pos);
        }
        pos = end + 1;
    }
    if let Some(s) = start {
        out.push(s..src.len());
    }
    out
}

/// Les mots mal orthographiés de `text`, en octets, triés.
///
/// `ignored` reçoit chaque mot avant le dictionnaire : `true` l'accepte
/// (dictionnaire personnel, mots ignorés pour ce document).
pub fn misspelled(
    text: &str,
    lang: Lang,
    speller: &dyn Speller,
    ignored: &dyn Fn(&str) -> bool,
) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    for p in paragraphs(text) {
        let base = p.start;
        for r in misspelled_paragraph(&text[p], lang, speller, ignored) {
            out.push(base + r.start..base + r.end);
        }
    }
    out
}

/// Vérifie l'orthographe de la source. Chaque remarque propose la première
/// suggestion du dictionnaire comme correction.
pub fn check_spelling(
    src: &str,
    lang: Lang,
    speller: &dyn Speller,
    ignored: &dyn Fn(&str) -> bool,
) -> Vec<Issue> {
    misspelled(src, lang, speller, ignored)
        .into_iter()
        .map(|r| {
            let sugg = speller.suggest(&src[r.clone()]);
            spelling_issue(src, r, &sugg)
        })
        .collect()
}

/// La remarque pour un mot inconnu, avec ses suggestions.
pub fn spelling_issue(src: &str, r: Range<usize>, suggestions: &[String]) -> Issue {
    let word = &src[r.clone()];
    Issue {
        start: r.start,
        end: r.end,
        kind: Kind::Orthographe,
        message: match suggestions.first() {
            Some(s) => format!("« {word} » est inconnu du dictionnaire : « {s} » ?"),
            None => format!("« {word} » est inconnu du dictionnaire"),
        },
        replacement: suggestions.first().cloned(),
    }
}

fn is_apostrophe(c: char) -> bool {
    matches!(c, '\'' | '’' | 'ʼ')
}

fn is_hyphen(c: char) -> bool {
    matches!(c, '-' | '\u{2011}')
}

fn is_letter(c: char) -> bool {
    c.is_alphanumeric() || ('\u{0300}'..='\u{036F}').contains(&c)
}

/// Les octets à ne pas examiner dans un paragraphe.
fn mask(text: &str) -> Vec<bool> {
    let n = text.len();
    let mut masked = vec![true; n];
    for (a, b) in crate::check::zones(text) {
        masked[a..b].fill(false);
    }
    for r in crate::conditions::directive_ranges(text) {
        masked[r.start.min(n)..r.end.min(n)].fill(true);
    }
    let bytes = text.as_bytes();
    let mut line_start = 0;
    for line in text.split('\n') {
        let lb = line.as_bytes();
        // Code `…`
        let mut open: Option<usize> = None;
        for (i, &c) in lb.iter().enumerate() {
            if c == b'`' {
                match open.take() {
                    Some(o) => masked[line_start + o..=line_start + i].fill(true),
                    None => open = Some(i),
                }
            }
        }
        // Commandes \newpage, \sommaire… (le caractère échappé \* reste)
        let mut i = 0;
        while i < lb.len() {
            if lb[i] == b'\\' && lb.get(i + 1).is_some_and(u8::is_ascii_alphabetic) {
                let mut j = i + 1;
                while j < lb.len() && lb[j].is_ascii_alphabetic() {
                    j += 1;
                }
                masked[line_start + i..line_start + j].fill(true);
                i = j;
            } else {
                i += 1;
            }
        }
        // Cible d'un lien [texte](adresse)
        let mut from = 0;
        while let Some(k) = line[from..].find("](") {
            let s = from + k + 1;
            let e = line[s..].find(')').map_or(line.len(), |x| s + x + 1);
            masked[line_start + s..line_start + e].fill(true);
            from = e.max(s + 1);
            if from >= line.len() {
                break;
            }
        }
        line_start += line.len() + 1;
    }
    // Adresses web et électroniques, noms de fichiers et de domaines : tout le bloc
    // sans espace qui les contient.
    let mut s = 0;
    while s < n {
        if bytes[s].is_ascii_whitespace() {
            s += 1;
            continue;
        }
        let e = text[s..]
            .find(char::is_whitespace)
            .map_or(n, |k| s + k);
        let chunk = &text[s..e];
        let trimmed = chunk.trim_start_matches(|c: char| !c.is_alphanumeric());
        let dotted = chunk
            .char_indices()
            .any(|(k, c)| {
                c == '.'
                    && chunk[..k].chars().next_back().is_some_and(char::is_alphanumeric)
                    && chunk[k + 1..].chars().next().is_some_and(char::is_alphanumeric)
            });
        let email = chunk.char_indices().any(|(k, c)| {
            c == '@'
                && chunk[..k].chars().next_back().is_some_and(char::is_alphanumeric)
                && chunk[k + 1..].contains('.')
        });
        if chunk.contains("://") || trimmed.starts_with("www.") || email || dotted {
            masked[s..e].fill(true);
        }
        s = e;
    }
    masked
}

fn elisions(lang: Lang) -> &'static [&'static str] {
    match lang {
        Lang::Fr => &[
            "l", "d", "j", "m", "n", "s", "t", "c", "ç", "qu", "lorsqu", "puisqu", "jusqu",
            "quoiqu", "presqu", "quelqu",
        ],
        Lang::It => &[
            "l", "un", "dell", "all", "dall", "nell", "sull", "coll", "pell", "d", "c", "quest",
            "quell", "bell", "sant", "tutt", "nessun", "ciascun", "buon", "m", "t", "s", "v",
            "gl", "senz", "com", "dov", "anch",
        ],
        Lang::Pt => &["d", "n"],
        Lang::En | Lang::Es | Lang::De => &[],
    }
}

struct Ctx<'a> {
    text: &'a str,
    lang: Lang,
    speller: &'a dyn Speller,
    ignored: &'a dyn Fn(&str) -> bool,
}

impl Ctx<'_> {
    /// Le dictionnaire accepte le mot (avec l'une ou l'autre apostrophe).
    fn accepts(&self, w: &str) -> bool {
        if (self.ignored)(w) || self.speller.check(w) {
            return true;
        }
        if w.contains(['’', 'ʼ']) {
            let straight = w.replace(['’', 'ʼ'], "'");
            if (self.ignored)(&straight) || self.speller.check(&straight) {
                return true;
            }
        }
        if w.contains('\'') {
            let curly = w.replace('\'', "’");
            if (self.ignored)(&curly) || self.speller.check(&curly) {
                return true;
            }
        }
        false
    }

    /// Vérifie le mot `s..e` ; ajoute à `out` les parties fautives.
    fn word(&self, s: usize, e: usize, out: &mut Vec<Range<usize>>) {
        let w = &self.text[s..e];
        let letters: Vec<char> = w.chars().filter(|c| c.is_alphabetic()).collect();
        if letters.len() < 2 || w.chars().any(|c| c.is_numeric()) {
            return;
        }
        // Sigles (ONU, PDF, ÉTÉ) et casse mixte (iPhone, McDonald, LaTeX)
        if letters.iter().all(|c| c.is_uppercase()) {
            return;
        }
        if letters
            .windows(2)
            .any(|p| p[0].is_lowercase() && p[1].is_uppercase())
        {
            return;
        }
        if self.accepts(w) {
            return;
        }
        // « po' », « mo' » : l'apostrophe finale fait partie du mot.
        if let Some(next) = self.text[e..].chars().next()
            && is_apostrophe(next)
            && self.accepts(&self.text[s..e + next.len_utf8()])
        {
            return;
        }
        // Mot composé : chaque partie.
        if w.contains(is_hyphen) {
            let mut a = s;
            for (k, c) in w.char_indices() {
                if is_hyphen(c) {
                    self.word(a, s + k, out);
                    a = s + k + c.len_utf8();
                }
            }
            self.word(a, e, out);
            return;
        }
        // Élision : l'homme, qu'il, dell'anno.
        if let Some((k, c)) = w.char_indices().find(|&(_, c)| is_apostrophe(c)) {
            let prefix = w[..k].to_lowercase();
            if elisions(self.lang).contains(&prefix.as_str()) {
                self.word(s + k + c.len_utf8(), e, out);
                return;
            }
            // Anglais : possessif « John's ».
            if self.lang == Lang::En && w[k + c.len_utf8()..].eq_ignore_ascii_case("s") {
                self.word(s, s + k, out);
                return;
            }
        }
        out.push(s..e);
    }
}

fn misspelled_paragraph(
    text: &str,
    lang: Lang,
    speller: &dyn Speller,
    ignored: &dyn Fn(&str) -> bool,
) -> Vec<Range<usize>> {
    let masked = mask(text);
    let ctx = Ctx {
        text,
        lang,
        speller,
        ignored,
    };
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut k = 0;
    while k < chars.len() {
        let (p, c) = chars[k];
        if masked[p] || !is_letter(c) {
            k += 1;
            continue;
        }
        // Un mot : lettres, avec apostrophes et traits d'union intérieurs.
        let mut j = k + 1;
        while j < chars.len() {
            let (q, d) = chars[j];
            if masked[q] {
                break;
            }
            if is_letter(d) {
                j += 1;
            } else if (is_apostrophe(d) || is_hyphen(d))
                && chars
                    .get(j + 1)
                    .is_some_and(|&(r, x)| is_letter(x) && !masked[r])
            {
                j += 2;
            } else {
                break;
            }
        }
        let end = chars.get(j).map_or(text.len(), |x| x.0);
        // Un mot collé à un caractère masqué (adresse, champ) n'est pas examiné.
        let glued = p > 0 && masked[p - 1] && !text[..p].ends_with(char::is_whitespace);
        if !glued {
            ctx.word(p, end, &mut out);
        }
        k = j;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un petit dictionnaire : quelques mots connus, et « monde » proposé pour
    /// tout mot inconnu qui commence par « mon ».
    struct Fake(&'static [&'static str]);

    impl Speller for Fake {
        fn check(&self, word: &str) -> bool {
            let lower = word.to_lowercase();
            self.0.contains(&word) || self.0.contains(&lower.as_str())
        }
        fn suggest(&self, word: &str) -> Vec<String> {
            if word.starts_with("mon") {
                vec!["monde".into(), "mondes".into()]
            } else {
                Vec::new()
            }
        }
    }

    const FR: Fake = Fake(&[
        "bonjour",
        "le",
        "monde",
        "homme",
        "il",
        "est",
        "aujourd’hui",
        "peut-être",
        "arc",
        "en",
        "ciel",
        "va",
        "voir",
        "et",
        "on",
        "titre",
        "liste",
        "texte",
        "a",
    ]);

    fn bad(src: &str, lang: Lang, sp: &dyn Speller) -> Vec<String> {
        misspelled(src, lang, sp, &|_| false)
            .into_iter()
            .map(|r| src[r].to_string())
            .collect()
    }

    #[test]
    fn mot_inconnu_et_suggestion() {
        let src = "Bonjour le monnde";
        let issues = check_spelling(src, Lang::Fr, &FR, &|_| false);
        assert_eq!(issues.len(), 1);
        assert_eq!(&src[issues[0].start..issues[0].end], "monnde");
        assert_eq!(issues[0].kind, Kind::Orthographe);
        assert_eq!(issues[0].replacement.as_deref(), Some("monde"));
        // ignoré (dictionnaire personnel)
        assert!(check_spelling(src, Lang::Fr, &FR, &|w| w == "monnde").is_empty());
    }

    #[test]
    fn elisions_et_traits_d_union() {
        assert!(bad("L'homme qu'il voit est aujourd'hui là", Lang::Fr, &FR) == ["voit", "là"]);
        assert!(bad("l’homme, aujourd’hui, peut-être", Lang::Fr, &FR).is_empty());
        assert!(bad("arc-en-ciel et va-t-on", Lang::Fr, &FR).is_empty());
        assert_eq!(bad("arc-en-cial", Lang::Fr, &FR), ["cial"]);
        // En anglais, pas d'élision : le mot entier est inconnu.
        assert_eq!(bad("l'homme", Lang::En, &FR), ["l'homme"]);
        let it = Fake(&["anno", "dell", "po'", "perché"]);
        assert!(bad("dell'anno, un po' perché", Lang::It, &it) == ["un"]);
    }

    #[test]
    fn allemand_et_espagnol() {
        let de = Fake(&["donaudampfschifffahrt", "und"]);
        assert!(bad("Donaudampfschifffahrt und", Lang::De, &de).is_empty());
        // un composé inconnu est signalé d'un seul tenant
        assert_eq!(bad("Donaudampfschiffahrt", Lang::De, &de), ["Donaudampfschiffahrt"]);
        let es = Fake(&["cómo", "estás", "qué", "bien"]);
        assert!(bad("¿Cómo estás? ¡Qué bien!", Lang::Es, &es).is_empty());
        assert_eq!(bad("¿Cómo etsás?", Lang::Es, &es), ["etsás"]);
    }

    #[test]
    fn ce_qui_n_est_pas_examine() {
        let src = "# Titre\n\n- liste {{ nomzz | upper }} $xyzt + qqq$ https://exemple.fr/abcd \
                   www.qwerty.org jean.dupont@exemple.fr rapport.pdf 12h30 3e B2B ONU \
                   iPhone \\newpage \\sommaire `codezz` [texte](lien/qqq) | texte |\n|---|---|";
        assert!(bad(src, Lang::Fr, &FR).is_empty(), "{:?}", bad(src, Lang::Fr, &FR));
        assert!(bad("{{si x}}texte{{sinon}}texte{{fin}}", Lang::Fr, &FR).is_empty());
    }

    #[test]
    fn positions_et_paragraphes() {
        let src = "Bonjour\n\nle monnde\n\n\nxyzzy";
        assert_eq!(paragraphs(src), [0..7, 9..18, 21..26]);
        let r = misspelled(src, Lang::Fr, &FR, &|_| false);
        assert_eq!(r, [12..18, 21..26]);
        assert_eq!(paragraphs(""), Vec::<Range<usize>>::new());
        assert_eq!(paragraphs("a\n"), [0..1]);
    }

    #[test]
    fn aucune_panique() {
        let mut seed = 11u64;
        let alphabet: Vec<char> = "aé ’'-.@:/\\`[]()$ {}\n#|ZzÉ1".chars().collect();
        for _ in 0..3000 {
            let len = (seed % 30) as usize;
            let s: String = (0..len)
                .map(|_| {
                    seed = seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    alphabet[(seed >> 33) as usize % alphabet.len()]
                })
                .collect();
            for lang in Lang::ALL {
                for r in misspelled(&s, lang, &FR, &|_| false) {
                    assert!(s.is_char_boundary(r.start) && s.is_char_boundary(r.end));
                }
            }
        }
    }
}
