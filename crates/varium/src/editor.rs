//! L'éditeur de texte : rendu « tel quel » de la mise en forme (le gras en gras,
//! les titres plus grands, les marques estompées) et commandes de mise en forme
//! sur la sélection, comme la barre de formatage de Writer.

use egui::text::LayoutJob;
use egui::{Color32, FontId, Stroke, TextFormat};
use varium_core::DocFont;

use crate::fonts;

// ---------------------------------------------------------------------------------
// Index caractères ↔ octets
// ---------------------------------------------------------------------------------

pub fn byte_of(s: &str, ch: usize) -> usize {
    s.char_indices().nth(ch).map_or(s.len(), |(b, _)| b)
}

pub fn char_of(s: &str, byte: usize) -> usize {
    s[..byte.min(s.len())].chars().count()
}

/// Une sélection en caractères : (début, fin), début ≤ fin.
pub type Sel = (usize, usize);

fn sorted(sel: Sel) -> Sel {
    (sel.0.min(sel.1), sel.0.max(sel.1))
}

// ---------------------------------------------------------------------------------
// Commandes
// ---------------------------------------------------------------------------------

/// Insère `text` à la place de la sélection. `select` choisit ce qui est
/// sélectionné ensuite, relativement au texte inséré (sinon : le curseur après).
pub fn insert(src: &mut String, sel: Sel, text: &str, select: Option<Sel>) -> Sel {
    let (a, b) = sorted(sel);
    let (ba, bb) = (byte_of(src, a), byte_of(src, b));
    src.replace_range(ba..bb, text);
    match select {
        Some((x, y)) => (a + x, a + y),
        None => {
            let end = a + text.chars().count();
            (end, end)
        }
    }
}

/// Met la sélection en gras / italique / souligné / barré, ou retire la marque.
pub fn toggle_wrap(src: &mut String, sel: Sel, marker: &str) -> Sel {
    let (a, b) = sorted(sel);
    let m = marker.chars().count();
    let (ba, bb) = (byte_of(src, a), byte_of(src, b));
    // Déjà entourée ? on retire les marques.
    if a >= m && src[..ba].ends_with(marker) && src[bb..].starts_with(marker) {
        src.replace_range(bb..bb + marker.len(), "");
        src.replace_range(ba - marker.len()..ba, "");
        return (a - m, b - m);
    }
    let inner = &src[ba..bb];
    if inner.len() >= 2 * marker.len() && inner.starts_with(marker) && inner.ends_with(marker) {
        let stripped = inner[marker.len()..inner.len() - marker.len()].to_string();
        src.replace_range(ba..bb, &stripped);
        return (a, b - 2 * m);
    }
    // Les espaces autour de la sélection restent en dehors des marques.
    let lead = inner.chars().take_while(|c| c.is_whitespace()).count();
    let trail = inner
        .chars()
        .rev()
        .take_while(|c| c.is_whitespace())
        .count();
    if a + lead >= b.saturating_sub(trail) {
        // Rien de sélectionné : on insère les marques et on place le curseur entre elles.
        let pair = format!("{marker}{marker}");
        return insert(src, (a, b), &pair, Some((m, m)));
    }
    let (a2, b2) = (a + lead, b - trail);
    let (ba2, bb2) = (byte_of(src, a2), byte_of(src, b2));
    src.insert_str(bb2, marker);
    src.insert_str(ba2, marker);
    (a2 + m, b2 + m)
}

/// Style de paragraphe, comme la liste « Styles » de Writer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineStyle {
    Body,
    Heading(u8),
    Bullet,
    Numbered,
    Quote,
    Center,
    Right,
}

impl LineStyle {
    pub const MENU: [LineStyle; 5] = [
        LineStyle::Body,
        LineStyle::Heading(1),
        LineStyle::Heading(2),
        LineStyle::Heading(3),
        LineStyle::Quote,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LineStyle::Body => "Corps de texte",
            LineStyle::Heading(1) => "Titre 1",
            LineStyle::Heading(2) => "Titre 2",
            LineStyle::Heading(_) => "Titre 3",
            LineStyle::Bullet => "Liste à puces",
            LineStyle::Numbered => "Liste numérotée",
            LineStyle::Quote => "Citation",
            LineStyle::Center => "Centré",
            LineStyle::Right => "Aligné à droite",
        }
    }
}

/// (style, longueur du préfixe, longueur du suffixe) d'une ligne, en octets.
fn line_markup(line: &str) -> (LineStyle, usize, usize) {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        return (LineStyle::Heading(hashes.min(3) as u8), hashes + 1, 0);
    }
    let indent = line.len() - line.trim_start_matches(' ').len();
    let rest = &line[indent..];
    for b in ["- ", "* ", "• ", "+ "] {
        if rest.starts_with(b) && !rest.trim().chars().all(|c| c == '-' || c == '*') {
            return (LineStyle::Bullet, indent + b.len(), 0);
        }
    }
    let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if (1..=4).contains(&digits)
        && (rest[digits..].starts_with(". ") || rest[digits..].starts_with(") "))
    {
        return (LineStyle::Numbered, indent + digits + 2, 0);
    }
    if let Some(r) = line.strip_prefix('>') {
        return (LineStyle::Quote, if r.starts_with(' ') { 2 } else { 1 }, 0);
    }
    if line.starts_with("->") {
        let t = line.trim_end();
        let pre = if line.starts_with("-> ") { 3 } else { 2 };
        let suffix_ws = line.len() - t.len();
        if t.ends_with("<-") && t.len() >= pre + 2 {
            let sp = usize::from(t[..t.len() - 2].ends_with(' '));
            return (LineStyle::Center, pre, 2 + sp + suffix_ws);
        }
        if t.ends_with("->") && t.len() > pre + 1 {
            let sp = usize::from(t[..t.len() - 2].ends_with(' '));
            return (LineStyle::Right, pre, 2 + sp + suffix_ws);
        }
    }
    (LineStyle::Body, 0, 0)
}

/// Style du paragraphe où se trouve le curseur.
pub fn line_style_at(src: &str, ch: usize) -> LineStyle {
    let b = byte_of(src, ch);
    let start = src[..b].rfind('\n').map_or(0, |i| i + 1);
    let end = src[b..].find('\n').map_or(src.len(), |i| b + i);
    line_markup(&src[start..end]).0
}

/// Applique un style de paragraphe aux lignes de la sélection. Si toutes les
/// lignes l'ont déjà, il est retiré (comme les boutons de liste de Writer).
pub fn set_line_style(src: &mut String, sel: Sel, style: LineStyle) -> Sel {
    let (a, b) = sorted(sel);
    let (ba, bb) = (byte_of(src, a), byte_of(src, b));
    let start = src[..ba].rfind('\n').map_or(0, |i| i + 1);
    let end = src[bb..].find('\n').map_or(src.len(), |i| bb + i);
    let lines: Vec<&str> = src[start..end].split('\n').collect();
    let all_same = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .all(|l| line_markup(l).0 == style);
    let target = if all_same && style != LineStyle::Body {
        LineStyle::Body
    } else {
        style
    };
    let mut n = 0;
    let out: Vec<String> = lines
        .iter()
        .map(|line| {
            let (_, pre, suf) = line_markup(line);
            let body = &line[pre..line.len() - suf];
            if line.trim().is_empty() && lines.len() > 1 {
                return line.to_string();
            }
            n += 1;
            match target {
                LineStyle::Body => body.to_string(),
                LineStyle::Heading(l) => format!("{} {body}", "#".repeat(l as usize)),
                LineStyle::Bullet => format!("- {body}"),
                LineStyle::Numbered => format!("{n}. {body}"),
                LineStyle::Quote => format!("> {body}"),
                LineStyle::Center => format!("-> {body} <-"),
                LineStyle::Right => format!("-> {body} ->"),
            }
        })
        .collect();
    let joined = out.join("\n");
    let new_len = joined.chars().count();
    src.replace_range(start..end, &joined);
    let cs = char_of(src, start);
    if a == b {
        // Curseur seul : il reste en fin de ligne.
        let first_line = out.first().map_or(0, |l| l.chars().count());
        (cs + first_line, cs + first_line)
    } else {
        (cs, cs + new_len)
    }
}

/// Un tableau vide de `rows` lignes (en-tête compris) et `cols` colonnes.
pub fn table_markup(rows: usize, cols: usize) -> String {
    let cols = cols.clamp(1, 12);
    let mut s = String::new();
    let header: Vec<String> = (1..=cols).map(|c| format!("Colonne {c}")).collect();
    s.push_str(&format!("| {} |\n", header.join(" | ")));
    s.push_str(&format!("|{}\n", "---|".repeat(cols)));
    for _ in 1..rows.max(2) {
        s.push_str(&format!("|{}\n", "   |".repeat(cols)));
    }
    s
}

// ---------------------------------------------------------------------------------
// Coloration
// ---------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
struct Attr {
    color: Color32,
    bg: Color32,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    scale: f32,
}

pub struct Palette {
    pub text: Color32,
    pub dim: Color32,
    pub var: Color32,
    pub var_bg: Color32,
    pub formula: Color32,
    pub math: Color32,
    pub heading: Color32,
    pub found: Color32,
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                text: Color32::from_rgb(0xE4, 0xE1, 0xDB),
                dim: Color32::from_gray(120),
                var: Color32::from_rgb(0x9C, 0xC2, 0xFA),
                var_bg: Color32::from_rgba_unmultiplied(0x8A, 0xB4, 0xF8, 34),
                formula: Color32::from_rgb(0xD0, 0xA2, 0xFA),
                math: Color32::from_rgb(0x81, 0xC9, 0x95),
                heading: Color32::from_rgb(0xF2, 0xEE, 0xE8),
                found: Color32::from_rgba_unmultiplied(0xF2, 0xC0, 0x4B, 90),
            }
        } else {
            Self {
                text: Color32::from_rgb(0x1F, 0x1D, 0x1A),
                dim: Color32::from_gray(150),
                var: Color32::from_rgb(0x17, 0x52, 0xB0),
                var_bg: Color32::from_rgb(0xE4, 0xEE, 0xFC),
                formula: Color32::from_rgb(0x7B, 0x3F, 0xB5),
                math: Color32::from_rgb(0x1E, 0x7A, 0x3C),
                heading: Color32::from_rgb(0x10, 0x10, 0x10),
                found: Color32::from_rgb(0xFF, 0xE0, 0x82),
            }
        }
    }
}

/// Marques de caractères d'une ligne : (position du premier octet, longueur, genre).
fn inline_marks(line: &str, protected: &dyn Fn(usize) -> bool) -> Vec<(usize, usize, u8)> {
    let bytes = line.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if protected(i) {
            i += 1;
            continue;
        }
        let c = bytes[i];
        if c == b'\\' && i + 1 < bytes.len() && b"*_~|#>".contains(&bytes[i + 1]) {
            i += 2;
            continue;
        }
        let two = |x: u8| c == x && bytes.get(i + 1) == Some(&x);
        if two(b'*') {
            toks.push((i, 2, 0u8));
            i += 2;
        } else if two(b'_') {
            toks.push((i, 2, 2));
            i += 2;
        } else if two(b'~') {
            toks.push((i, 2, 3));
            i += 2;
        } else if c == b'*' {
            toks.push((i, 1, 1));
            i += 1;
        } else {
            i += 1;
        }
    }
    let space_at = |b: usize| line[b..].chars().next().is_none_or(char::is_whitespace);
    let space_before = |b: usize| {
        line[..b]
            .chars()
            .next_back()
            .is_none_or(char::is_whitespace)
    };
    let can_open = |(p, l, _): (usize, usize, u8)| !space_at(p + l);
    let can_close = |(p, _, _): (usize, usize, u8)| !space_before(p);
    let mut open = [false; 4];
    let mut active = Vec::new();
    for (k, t) in toks.iter().enumerate() {
        let kind = t.2 as usize;
        if open[kind] && can_close(*t) {
            open[kind] = false;
            active.push(*t);
        } else if !open[kind]
            && can_open(*t)
            && toks[k + 1..].iter().any(|u| u.2 == t.2 && can_close(*u))
        {
            open[kind] = true;
            active.push(*t);
        }
    }
    active
}

/// La mise en page de la source pour l'éditeur.
pub fn highlight(
    src: &str,
    pal: &Palette,
    font: DocFont,
    size: f32,
    found: &[(usize, usize)],
) -> LayoutJob {
    let base = Attr {
        color: pal.text,
        bg: Color32::TRANSPARENT,
        bold: false,
        italic: false,
        underline: false,
        strike: false,
        scale: 1.0,
    };
    let mut attrs = vec![base; src.len()];
    let mut protected = vec![false; src.len()];

    // Variables et champs calculés
    for seg in varium_core::parse(src) {
        if let varium_core::Segment::Var(v) = seg {
            let color = if v.formula.is_some() {
                pal.formula
            } else {
                pal.var
            };
            for i in v.range.clone() {
                attrs[i].color = color;
                attrs[i].bg = pal.var_bg;
                protected[i] = true;
            }
            if v.range.start > 0 && src.as_bytes()[v.range.start - 1] == b'\\' {
                attrs[v.range.start - 1].color = pal.dim;
            }
        }
    }
    // Directives {{si}} / {{sinon}} / {{fin}}
    for r in varium_core::conditions::directive_ranges(src) {
        for i in r {
            attrs[i].color = pal.formula;
            attrs[i].bg = pal.var_bg;
            protected[i] = true;
        }
    }
    // Formules $…$
    let bytes = src.as_bytes();
    let mut in_math = false;
    for i in 0..bytes.len() {
        if bytes[i] == b'$' && !protected[i] {
            let escaped = i > 0 && bytes[i - 1] == b'\\';
            attrs[i].color = pal.math;
            if !escaped {
                in_math = !in_math;
            }
            protected[i] = true;
        } else if in_math && !protected[i] {
            attrs[i].color = pal.math;
            protected[i] = true;
        }
    }

    // Lignes : préfixes de paragraphe, titres, marques de caractères
    let mut start = 0;
    for line in src.split('\n') {
        let end = start + line.len();
        let (style, pre, suf) = line_markup(line);
        for a in &mut attrs[start..start + pre] {
            a.color = pal.dim;
        }
        for a in &mut attrs[end - suf..end] {
            a.color = pal.dim;
        }
        if let LineStyle::Heading(l) = style {
            let scale = [1.45, 1.25, 1.1][(l as usize).clamp(1, 3) - 1];
            for a in &mut attrs[start..end] {
                a.scale = scale;
                a.bold = true;
                if a.color == pal.text {
                    a.color = pal.heading;
                }
            }
        }
        let t = line.trim();
        if t == "\\newpage" || (t.len() >= 3 && t.chars().all(|c| c == '-')) {
            for a in &mut attrs[start..end] {
                a.color = pal.dim;
            }
        }
        if t.starts_with('|') && t.ends_with('|') {
            for (k, c) in line.char_indices() {
                if c == '|'
                    || (c == '-' && t.chars().all(|x| "|-: ".contains(x)))
                    || c == ':' && t.chars().all(|x| "|-: ".contains(x))
                {
                    attrs[start + k].color = pal.dim;
                }
            }
        }
        let prot = |b: usize| protected[start + b];
        let marks = inline_marks(line, &prot);
        let mut state = [false; 4];
        let mut m = marks.iter().peekable();
        let mut k = 0;
        while k < line.len() {
            if let Some(&&(p, l, kind)) = m.peek()
                && p == k
            {
                state[kind as usize] = !state[kind as usize];
                for a in &mut attrs[start + p..start + p + l] {
                    a.color = pal.dim;
                }
                m.next();
                k += l;
                continue;
            }
            let a = &mut attrs[start + k];
            a.bold |= state[0];
            a.italic |= state[1];
            a.underline |= state[2];
            a.strike |= state[3];
            k += 1;
        }
        start = end + 1;
    }

    // Résultats de recherche
    for &(a, b) in found {
        for at in attrs.iter_mut().take(b.min(src.len())).skip(a) {
            at.bg = pal.found;
        }
    }

    let mut job = LayoutJob::default();
    let mut run_start = 0;
    let fmt = |a: &Attr| {
        let s = size * a.scale;
        TextFormat {
            font_id: FontId::new(s, fonts::family(font, a.bold, a.italic)),
            color: a.color,
            background: a.bg,
            italics: a.italic && !fonts::has_italic(font),
            underline: if a.underline {
                Stroke::new(1.0, a.color)
            } else {
                Stroke::NONE
            },
            strikethrough: if a.strike {
                Stroke::new(1.0, a.color)
            } else {
                Stroke::NONE
            },
            line_height: Some(s * 1.35),
            ..Default::default()
        }
    };
    for (i, _) in src.char_indices() {
        if attrs[i] != attrs[run_start] {
            job.append(&src[run_start..i], 0.0, fmt(&attrs[run_start]));
            run_start = i;
        }
    }
    if run_start < src.len() {
        job.append(&src[run_start..], 0.0, fmt(&attrs[run_start]));
    }
    if src.is_empty() {
        job.append("", 0.0, fmt(&base));
    }
    job
}

/// Positions (en octets) des occurrences de `query`.
pub fn find_all(src: &str, query: &str, case: bool) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return Vec::new();
    }
    if case {
        return src
            .match_indices(query)
            .map(|(i, m)| (i, i + m.len()))
            .collect();
    }
    // Insensible à la casse : on compare caractère par caractère.
    let q: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    let chars: Vec<(usize, char)> = src.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let mut j = 0;
        let mut k = i;
        while j < q.len() && k < chars.len() {
            let lower: Vec<char> = chars[k].1.to_lowercase().collect();
            if q[j..].starts_with(&lower) {
                j += lower.len();
                k += 1;
            } else {
                break;
            }
        }
        if j == q.len() {
            let end = chars.get(k).map_or(src.len(), |c| c.0);
            out.push((chars[i].0, end));
            i = k.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gras_et_retrait() {
        let mut s = String::from("Un mot ici");
        let sel = toggle_wrap(&mut s, (3, 6), "**");
        assert_eq!(s, "Un **mot** ici");
        assert_eq!(sel, (5, 8));
        let sel = toggle_wrap(&mut s, sel, "**");
        assert_eq!(s, "Un mot ici");
        assert_eq!(sel, (3, 6));
        // sans sélection : les marques et le curseur entre elles
        let mut s = String::from("ab");
        assert_eq!(toggle_wrap(&mut s, (1, 1), "*"), (2, 2));
        assert_eq!(s, "a**b");
        // les espaces sélectionnés restent dehors
        let mut s = String::from("a mot b");
        toggle_wrap(&mut s, (1, 6), "__");
        assert_eq!(s, "a __mot__ b");
    }

    #[test]
    fn styles_de_paragraphe() {
        let mut s = String::from("intro\nun\ndeux\nfin");
        let sel = set_line_style(&mut s, (7, 10), LineStyle::Numbered);
        assert_eq!(s, "intro\n1. un\n2. deux\nfin");
        let sel = set_line_style(&mut s, sel, LineStyle::Bullet);
        assert_eq!(s, "intro\n- un\n- deux\nfin");
        // deuxième clic : retire la liste
        set_line_style(&mut s, sel, LineStyle::Bullet);
        assert_eq!(s, "intro\nun\ndeux\nfin");
        set_line_style(&mut s, (0, 0), LineStyle::Heading(2));
        assert_eq!(s, "## intro\nun\ndeux\nfin");
        assert_eq!(line_style_at(&s, 3), LineStyle::Heading(2));
        set_line_style(&mut s, (0, 0), LineStyle::Center);
        assert_eq!(s, "-> intro <-\nun\ndeux\nfin");
        assert_eq!(line_style_at(&s, 0), LineStyle::Center);
        set_line_style(&mut s, (0, 0), LineStyle::Right);
        assert_eq!(s, "-> intro ->\nun\ndeux\nfin");
        set_line_style(&mut s, (0, 0), LineStyle::Body);
        assert_eq!(s, "intro\nun\ndeux\nfin");
    }

    #[test]
    fn recherche() {
        assert_eq!(find_all("Été, été, ÉTÉ", "été", false).len(), 3);
        assert_eq!(find_all("Été, été", "été", true), [(7, 12)]);
        assert!(find_all("abc", "", false).is_empty());
    }

    #[test]
    fn insertion_et_tableau() {
        let mut s = String::from("aé b");
        let sel = insert(&mut s, (2, 2), "{{x}}", Some((2, 3)));
        assert_eq!(s, "aé{{x}} b");
        assert_eq!(sel, (4, 5));
        assert!(table_markup(3, 2).starts_with("| Colonne 1 | Colonne 2 |\n|---|---|\n"));
    }

    #[test]
    fn coloration_sans_panique() {
        let pal = Palette::new(false);
        for s in [
            "",
            "# T",
            "**a",
            "| a |\n|---|",
            "$x",
            "é **gras** {{v}} $y$ \\newpage",
            "-> c <-",
        ] {
            let job = highlight(s, &pal, DocFont::Serif, 15.0, &[(0, s.len())]);
            assert_eq!(job.text, s);
        }
    }
}
