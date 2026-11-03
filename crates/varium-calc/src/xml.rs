//! Petits outils XML pour les exports.

/// Caractère permis dans un document XML 1.0 ?
fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r')
        || ('\u{20}'..='\u{D7FF}').contains(&c)
        || ('\u{E000}'..='\u{FFFD}').contains(&c)
        || c >= '\u{10000}'
}

/// Échappe un texte pour un contenu ou un attribut XML entre guillemets
/// doubles (les caractères interdits en XML sont retirés).
pub(crate) fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c if xml_char(c) => out.push(c),
            _ => {}
        }
    }
    out
}

/// Échappe un texte de contenu (sauts de ligne gardés tels quels).
pub(crate) fn esc_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c if xml_char(c) => out.push(c),
            _ => {}
        }
    }
    out
}

/// Nombre écrit pour un fichier (point décimal, sans exposant exotique).
pub(crate) fn num(n: f64) -> String {
    if n.is_finite() {
        format!("{n}")
    } else {
        "0".into()
    }
}

/// Couleur `#rrggbb`.
pub(crate) fn hex(c: [u8; 3]) -> String {
    format!("{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echappement() {
        assert_eq!(esc("a<b>&\"'"), "a&lt;b&gt;&amp;&quot;'");
        assert_eq!(esc("x\u{1}y\u{FFFF}z"), "xyz");
        assert_eq!(esc_text("l1\nl2"), "l1\nl2");
        assert_eq!(num(1.5), "1.5");
        assert_eq!(num(f64::NAN), "0");
        assert_eq!(hex([255, 0, 16]), "FF0010");
    }
}
