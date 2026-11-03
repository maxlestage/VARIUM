//! Outils de texte communs aux exports : lignes de titre, échappement, couleurs.

use varium_core::{Align, Para, Run, Values};

/// Couleur des champs sans valeur (`{{nom}}` laissé visible), sur tous les thèmes.
pub const MISSING_COLOR: [u8; 3] = [214, 69, 65];

/// Les lignes d'un titre ou d'un sous-titre : champs remplis, mise en forme des
/// caractères (`**gras**`…) interprétée, une entrée par ligne de la source. Les
/// lignes vides du début et de la fin sont retirées.
pub fn text_lines(src: &str, values: &Values) -> Vec<Vec<Run>> {
    let mut lignes: Vec<Vec<Run>> = varium_core::document(src, values)
        .into_iter()
        .filter(|p| !matches!(p, Para::Rule | Para::PageBreak))
        .map(|p| match p {
            Para::Text { runs, .. }
            | Para::Heading { runs, .. }
            | Para::Item { runs, .. }
            | Para::Quote { runs } => runs,
            autre => vec![Run::Text {
                text: autre.plain().replace('\n', " "),
                style: Default::default(),
            }],
        })
        .collect();
    let vide = |l: &Vec<Run>| varium_core::doc::plain(l).trim().is_empty();
    while lignes.last().is_some_and(vide) {
        lignes.pop();
    }
    let debut = lignes.iter().take_while(|l| vide(l)).count();
    lignes.drain(..debut);
    lignes
}

/// Les lignes d'un titre, sous forme de paragraphes (pour les exports).
pub(crate) fn paragraphes_ligne(src: &str, values: &Values) -> Vec<Para> {
    text_lines(src, values)
        .into_iter()
        .map(|runs| Para::Text {
            align: Align::Left,
            runs,
        })
        .collect()
}

/// Échappe un texte pour XML et HTML.
pub(crate) fn echapper(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            // Caractères interdits en XML 1.0 : retirés.
            '\u{0}'..='\u{8}'
            | '\u{B}'
            | '\u{C}'
            | '\u{E}'..='\u{1F}'
            | '\u{FFFE}'
            | '\u{FFFF}' => {}
            _ => out.push(c),
        }
    }
    out
}

/// Couleur au format `#rrggbb`.
pub fn hex_color(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// Lit une couleur `#rrggbb`, `rrggbb` ou `#rgb` ; `None` si elle est invalide.
pub fn parse_color(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if !h.is_ascii() {
        return None;
    }
    let canal = |t: &str| u8::from_str_radix(t, 16).ok();
    match h.len() {
        6 => Some([canal(&h[0..2])?, canal(&h[2..4])?, canal(&h[4..6])?]),
        3 => {
            let c = |i: usize| canal(&h[i..i + 1]).map(|v| v * 17);
            Some([c(0)?, c(1)?, c(2)?])
        }
        _ => None,
    }
}

/// Une couleur de texte lisible sur le fond `bg` : blanc ou presque noir.
pub fn readable_on(bg: [u8; 3]) -> [u8; 3] {
    let (blanc, noir) = ([255, 255, 255], [20, 20, 24]);
    if contraste(blanc, bg) >= contraste(noir, bg) {
        blanc
    } else {
        noir
    }
}

/// Coupe un texte à `max` caractères, avec des points de suspension.
pub(crate) fn tronquer(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
    t = t.trim_end().to_string();
    t.push('…');
    t
}

/// Mélange deux couleurs : `t` = 0 donne `a`, 1 donne `b`.
pub(crate) fn melange(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// Luminance relative (WCAG) d'une couleur, de 0 (noir) à 1 (blanc).
pub(crate) fn luminance(c: [u8; 3]) -> f32 {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// Rapport de contraste (WCAG) entre deux couleurs, de 1 à 21.
pub(crate) fn contraste(a: [u8; 3], b: [u8; 3]) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn couleurs_hexadecimales() {
        assert_eq!(hex_color([255, 0, 16]), "#ff0010");
        assert_eq!(parse_color("#ff0010"), Some([255, 0, 16]));
        assert_eq!(parse_color("abc"), Some([170, 187, 204]));
        assert_eq!(parse_color("#12345"), None);
        assert_eq!(parse_color("#é12345"), None);
        assert_eq!(parse_color("zzzzzz"), None);
    }

    #[test]
    fn lignes_de_titre() {
        let mut v = Values::default();
        v.set("x", "Lyon");
        let l = text_lines("\n**Bonjour** {{x}}\n\nsuite\n\n", &v);
        assert_eq!(l.len(), 3);
        assert_eq!(varium_core::doc::plain(&l[0]), "Bonjour Lyon");
        assert!(l[0][0].style().bold);
        assert_eq!(varium_core::doc::plain(&l[2]), "suite");
        assert!(text_lines("", &v).is_empty());
    }

    #[test]
    fn echappement_xml() {
        assert_eq!(echapper("<a & 'b'>\u{1}"), "&lt;a &amp; &#39;b&#39;&gt;");
    }

    #[test]
    fn contraste_noir_blanc() {
        assert!((contraste([0, 0, 0], [255, 255, 255]) - 21.0).abs() < 0.01);
        assert_eq!(tronquer("abcdef", 4), "abc…");
        assert_eq!(melange([0, 0, 0], [255, 255, 255], 0.5), [128, 128, 128]);
    }
}
