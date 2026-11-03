//! Typographie française automatique, comme la correction automatique de Pages et
//! de Writer : guillemets « », apostrophe ’, points de suspension …, tirets – —,
//! espaces fines insécables avant ; : ! ? et à l'intérieur des guillemets.
//!
//! Seul le texte écrit dans la source est corrigé : les valeurs des champs et les
//! formules restent telles quelles.

use crate::doc::{Para, Run};
use crate::filters::{NBSP, THIN_NBSP};

/// État conservé d'un morceau de texte à l'autre dans un paragraphe.
#[derive(Default)]
struct State {
    /// Un guillemet « est ouvert.
    open_quote: bool,
    /// Dernier caractère écrit (pour savoir si un guillemet ouvre ou ferme).
    prev: Option<char>,
}

fn fix(text: &str, st: &mut State) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 8);
    let mut i = 0;
    let push = |out: &mut String, st: &mut State, c: char| {
        out.push(c);
        st.prev = Some(c);
    };
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            // Points de suspension
            '.' if next == Some('.') && chars.get(i + 2) == Some(&'.') => {
                push(&mut out, st, '…');
                i += 3;
                continue;
            }
            // Tirets : `--` → tiret demi-cadratin, `---` est un filet (déjà traité ailleurs)
            // Trois tirets ou plus (filet, tableau) : inchangés.
            '-' if chars[i..].iter().take_while(|c| **c == '-').count() >= 3 => {
                while chars.get(i) == Some(&'-') {
                    push(&mut out, st, '-');
                    i += 1;
                }
                continue;
            }
            '-' if next == Some('-') => {
                let spaced = st.prev.is_some_and(char::is_whitespace);
                push(&mut out, st, if spaced { '—' } else { '–' });
                i += 2;
                continue;
            }
            // Apostrophe
            '\'' => push(&mut out, st, '’'),
            // Guillemets droits → « »
            '"' => {
                let opening = !st.open_quote
                    && st
                        .prev
                        .is_none_or(|p| p.is_whitespace() || "([{—–".contains(p));
                if opening {
                    push(&mut out, st, '«');
                    out.push(NBSP);
                    st.open_quote = true;
                    // les espaces déjà tapées après le guillemet sont remplacées
                    while chars.get(i + 1).is_some_and(|c| *c == ' ') {
                        i += 1;
                    }
                } else {
                    while out.ends_with(' ') {
                        out.pop();
                    }
                    out.push(NBSP);
                    push(&mut out, st, '»');
                    st.open_quote = false;
                }
            }
            // Ponctuation haute : espace fine insécable avant
            ';' | '!' | '?' | ':' => {
                if out.ends_with(' ') {
                    out.pop();
                    out.push(if c == ':' { NBSP } else { THIN_NBSP });
                }
                push(&mut out, st, c);
            }
            _ => push(&mut out, st, c),
        }
        i += 1;
    }
    out
}

fn fix_runs(runs: &mut [Run]) {
    let mut st = State::default();
    for r in runs {
        match r {
            Run::Text { text, .. } => *text = fix(text, &mut st),
            other => st.prev = other.plain().chars().last(),
        }
    }
}

/// Applique la typographie française au document mis en forme.
pub fn apply(paras: &mut [Para]) {
    for p in paras {
        match p {
            Para::Text { runs, .. }
            | Para::Heading { runs, .. }
            | Para::Item { runs, .. }
            | Para::Quote { runs } => fix_runs(runs),
            Para::Table(t) => t.rows.iter_mut().flatten().for_each(|c| fix_runs(c)),
            _ => {}
        }
    }
}

/// La correction d'un texte seul (pour les tests et l'aperçu).
pub fn correct(text: &str) -> String {
    fix(text, &mut State::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrections() {
        assert_eq!(correct("l'été"), "l’été");
        assert_eq!(
            correct("Il dit \"oui\" !"),
            "Il dit «\u{a0}oui\u{a0}»\u{202f}!"
        );
        assert_eq!(
            correct("Pourquoi ? Voici : ceci ; cela"),
            "Pourquoi\u{202f}? Voici\u{a0}: ceci\u{202f}; cela"
        );
        assert_eq!(correct("Attendez..."), "Attendez…");
        assert_eq!(correct("pages 3--5"), "pages 3–5");
        assert_eq!(correct("oui -- non"), "oui — non");
        // pas d'espace avant : rien ne change (heures, adresses web)
        assert_eq!(correct("10:30 https://x.fr"), "10:30 https://x.fr");
        assert_eq!(correct("---"), "---");
    }

    #[test]
    fn les_valeurs_ne_sont_pas_touchees() {
        let mut v = crate::Values::default();
        v.set("nom", "l'ami \"Jo\" ?");
        let mut d = crate::document("Bonjour {{nom}} ?", &v);
        apply(&mut d);
        assert_eq!(d[0].plain(), "Bonjour l'ami \"Jo\" ?\u{202f}?");
    }
}
