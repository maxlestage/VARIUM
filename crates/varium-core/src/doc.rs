//! Mise en forme du document, façon traitement de texte.
//!
//! Chaque ligne de la source est un paragraphe (comme la touche Entrée dans
//! LibreOffice Writer). Quelques marques simples, lues **uniquement dans la
//! source** (jamais dans les valeurs saisies), donnent la mise en forme :
//!
//! | Écriture | Effet |
//! |---|---|
//! | `# Titre`, `## Sous-titre`, `### Titre 3` | titres |
//! | `**gras**`, `*italique*`, `__souligné__`, `~~barré~~` | caractères |
//! | `- élément`, `1. élément` (2 espaces de retrait par niveau) | listes |
//! | `> citation` | citation |
//! | `-> centré <-`, `-> à droite ->` | alignement |
//! | `\| a \| b \|` (+ `\|---\|:---:\|` sous l'en-tête) | tableau |
//! | `---` | ligne horizontale |
//! | `\newpage` | saut de page |
//! | `\sommaire` | table des matières (liste des titres, générée) |
//! | `\*`, `\_`, `\~`, `\|`, `\#`, `\>` | caractère littéral |

use crate::render::{Block, Piece, Values, blocks, render_pieces};
use crate::tex;

/// Style de caractères.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
}

impl Style {
    /// Identifiant compact (0 à 15), pratique pour nommer des styles d'export.
    pub fn mask(self) -> u8 {
        self.bold as u8
            | (self.italic as u8) << 1
            | (self.underline as u8) << 2
            | (self.strike as u8) << 3
    }
}

/// Un morceau de texte mis en forme.
#[derive(Debug, Clone, PartialEq)]
pub enum Run {
    /// Texte de la source.
    Text { text: String, style: Style },
    /// Valeur d'une variable.
    Value {
        name: String,
        text: String,
        style: Style,
    },
    /// Variable sans valeur (`raw` est son écriture, ex. `{{nom}}`).
    Missing {
        name: String,
        raw: String,
        style: Style,
    },
    /// Formule en ligne (`$…$`) : le LaTeX source.
    Math { tex: String, style: Style },
}

impl Run {
    pub fn style(&self) -> Style {
        match self {
            Run::Text { style, .. }
            | Run::Value { style, .. }
            | Run::Missing { style, .. }
            | Run::Math { style, .. } => *style,
        }
    }

    pub fn style_mut(&mut self) -> &mut Style {
        match self {
            Run::Text { style, .. }
            | Run::Value { style, .. }
            | Run::Missing { style, .. }
            | Run::Math { style, .. } => style,
        }
    }

    /// Le texte affiché (formules converties en Unicode).
    pub fn plain(&self) -> String {
        match self {
            Run::Text { text, .. } | Run::Value { text, .. } => text.clone(),
            Run::Missing { raw, .. } => raw.clone(),
            Run::Math { tex: t, .. } => tex::to_unicode(t),
        }
    }
}

/// Le texte de plusieurs morceaux.
pub fn plain(runs: &[Run]) -> String {
    runs.iter().map(Run::plain).collect()
}

/// Alignement d'un paragraphe.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Un tableau.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Table {
    /// Alignement de chaque colonne.
    pub aligns: Vec<Align>,
    /// La première ligne est un en-tête.
    pub header: bool,
    pub rows: Vec<Vec<Vec<Run>>>,
}

impl Table {
    pub fn columns(&self) -> usize {
        self.rows.iter().map(Vec::len).max().unwrap_or(0)
    }

    pub fn align(&self, col: usize) -> Align {
        self.aligns.get(col).copied().unwrap_or_default()
    }
}

/// Un paragraphe du document.
#[derive(Debug, Clone, PartialEq)]
pub enum Para {
    /// Paragraphe courant ; sans morceau, c'est une ligne vide.
    Text {
        align: Align,
        runs: Vec<Run>,
    },
    /// Titre de niveau 1 à 3.
    Heading {
        level: u8,
        runs: Vec<Run>,
    },
    /// Élément de liste : `number` pour une liste numérotée, `depth` à partir de 0.
    Item {
        number: Option<u32>,
        depth: u8,
        runs: Vec<Run>,
    },
    Quote {
        runs: Vec<Run>,
    },
    /// Formule centrée (`$$…$$`).
    Math {
        tex: String,
    },
    Table(Table),
    Rule,
    PageBreak,
}

impl Para {
    /// Les morceaux de texte du paragraphe (vide pour un tableau, un filet…).
    pub fn runs(&self) -> &[Run] {
        match self {
            Para::Text { runs, .. }
            | Para::Heading { runs, .. }
            | Para::Item { runs, .. }
            | Para::Quote { runs } => runs,
            _ => &[],
        }
    }

    /// Texte brut du paragraphe, sans mise en forme.
    pub fn plain(&self) -> String {
        match self {
            Para::Math { tex: t } => tex::to_unicode(t.trim()),
            Para::Table(t) => t
                .rows
                .iter()
                .map(|r| r.iter().map(|c| plain(c)).collect::<Vec<_>>().join(" | "))
                .collect::<Vec<_>>()
                .join("\n"),
            other => plain(other.runs()),
        }
    }
}

// ---------------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Item {
    Text(String),
    Value { name: String, text: String },
    Missing { name: String, raw: String },
    Math(String),
}

enum Line {
    Items(Vec<Item>),
    Display(String),
}

fn math_text(pieces: &[Piece]) -> String {
    pieces.iter().map(Piece::text).collect()
}

fn lines(src: &str, values: &Values) -> Vec<Line> {
    let mut out = Vec::new();
    let mut cur: Vec<Item> = Vec::new();
    let mut skip_newline = false;

    for block in blocks(render_pieces(src, values)) {
        match block {
            Block::Prose(pieces) => {
                for piece in pieces {
                    match piece {
                        Piece::Text(t) => {
                            for (i, part) in t.split('\n').enumerate() {
                                if i > 0 {
                                    if skip_newline && cur.is_empty() {
                                        skip_newline = false;
                                    } else {
                                        out.push(Line::Items(std::mem::take(&mut cur)));
                                    }
                                }
                                if !part.is_empty() {
                                    skip_newline = false;
                                    match cur.last_mut() {
                                        Some(Item::Text(prev)) => prev.push_str(part),
                                        _ => cur.push(Item::Text(part.to_string())),
                                    }
                                }
                            }
                        }
                        Piece::Value { name, text } => {
                            skip_newline = false;
                            cur.push(Item::Value { name, text });
                        }
                        Piece::Missing { name, raw } => {
                            skip_newline = false;
                            cur.push(Item::Missing { name, raw });
                        }
                    }
                }
            }
            Block::Math {
                display: false,
                pieces,
            } => {
                skip_newline = false;
                cur.push(Item::Math(math_text(&pieces)));
            }
            Block::Math {
                display: true,
                pieces,
            } => {
                if !cur.is_empty() {
                    out.push(Line::Items(std::mem::take(&mut cur)));
                }
                out.push(Line::Display(math_text(&pieces)));
                skip_newline = true;
            }
        }
    }
    if !cur.is_empty() {
        out.push(Line::Items(cur));
    }
    out
}

/// Le texte de la ligne si elle ne contient que du texte de la source.
fn text_only(items: &[Item]) -> Option<String> {
    let mut s = String::new();
    for i in items {
        match i {
            Item::Text(t) => s.push_str(t),
            _ => return None,
        }
    }
    Some(s)
}

fn leading(items: &[Item]) -> &str {
    match items.first() {
        Some(Item::Text(t)) => t,
        _ => "",
    }
}

fn trailing(items: &[Item]) -> &str {
    match items.last() {
        Some(Item::Text(t)) => t,
        _ => "",
    }
}

/// Retire `n` octets au début du premier texte de la ligne.
fn strip_start(items: &mut Vec<Item>, n: usize) {
    if let Some(Item::Text(t)) = items.first_mut() {
        t.drain(..n);
        if t.is_empty() {
            items.remove(0);
        }
    }
}

/// Retire `n` octets à la fin du dernier texte de la ligne.
fn strip_end(items: &mut Vec<Item>, n: usize) {
    if let Some(Item::Text(t)) = items.last_mut() {
        t.truncate(t.len() - n);
        if t.is_empty() {
            items.pop();
        }
    }
}

fn is_rule(s: &str) -> bool {
    let s = s.trim();
    s.len() >= 3 && ['-', '*', '_'].iter().any(|c| s.chars().all(|x| x == *c))
}

fn is_table_line(items: &[Item]) -> bool {
    leading(items).trim_start().starts_with('|')
        && trailing(items).trim_end().ends_with('|')
        && !(items.len() == 1 && leading(items).trim() == "|")
}

fn separator_aligns(s: &str) -> Option<Vec<Align>> {
    let s = s.trim();
    if !s.contains('-') || !s.chars().all(|c| matches!(c, '|' | ':' | '-' | ' ')) {
        return None;
    }
    let inner = s.trim_start_matches('|').trim_end_matches('|');
    Some(
        inner
            .split('|')
            .map(|c| {
                let c = c.trim();
                match (c.starts_with(':'), c.ends_with(':')) {
                    (true, true) => Align::Center,
                    (false, true) => Align::Right,
                    _ => Align::Left,
                }
            })
            .collect(),
    )
}

/// Le document mis en forme.
pub fn document(src: &str, values: &Values) -> Vec<Para> {
    let mut out = Vec::new();
    // Positions des `\sommaire`, remplies une fois tous les titres connus.
    let mut sommaires: Vec<usize> = Vec::new();
    let mut lines = lines(src, values).into_iter().peekable();
    while let Some(line) = lines.next() {
        let items = match line {
            Line::Display(t) => {
                out.push(Para::Math { tex: t });
                continue;
            }
            Line::Items(items) => items,
        };

        // Tableau : lignes consécutives qui commencent et finissent par `|`.
        if is_table_line(&items) {
            let mut table = Table::default();
            let mut rows = vec![items];
            while let Some(Line::Items(next)) = lines.peek() {
                if !is_table_line(next) {
                    break;
                }
                let Some(Line::Items(next)) = lines.next() else {
                    unreachable!()
                };
                rows.push(next);
            }
            for (i, row) in rows.into_iter().enumerate() {
                if let Some(aligns) = text_only(&row).as_deref().and_then(separator_aligns) {
                    if i == 1 && !table.header {
                        table.header = true;
                        table.aligns = aligns;
                    }
                    continue;
                }
                table.rows.push(cells(row));
            }
            if !table.rows.is_empty() {
                out.push(Para::Table(table));
            }
            continue;
        }

        if text_only(&items).is_some_and(|t| t.trim() == "\\sommaire") {
            sommaires.push(out.len());
            out.push(Para::Rule);
            continue;
        }

        out.push(paragraph(items));
    }
    // Du dernier au premier, pour que les positions restent valables.
    if !sommaires.is_empty() {
        let toc = table_des_matieres(&out);
        for pos in sommaires.into_iter().rev() {
            out.splice(pos..=pos, toc.iter().cloned());
        }
    }
    out
}

/// Table des matières : un titre en gras puis un élément de liste par titre du
/// document (niveau 1 → retrait 0, niveau 3 → retrait 2).
fn table_des_matieres(doc: &[Para]) -> Vec<Para> {
    let bold = Style {
        bold: true,
        ..Style::default()
    };
    let mut toc = vec![Para::Text {
        align: Align::Left,
        runs: vec![Run::Text {
            text: "Table des matières".to_string(),
            style: bold,
        }],
    }];
    let mut any = false;
    for para in doc {
        if let Para::Heading { level, runs } = para {
            let text = plain(runs);
            if text.trim().is_empty() {
                continue;
            }
            any = true;
            toc.push(Para::Item {
                number: None,
                depth: level.saturating_sub(1).min(5),
                runs: vec![Run::Text {
                    text: text.trim().to_string(),
                    style: Style::default(),
                }],
            });
        }
    }
    if !any {
        toc.push(Para::Text {
            align: Align::Left,
            runs: vec![Run::Text {
                text: "(aucun titre dans le document)".to_string(),
                style: Style {
                    italic: true,
                    ..Style::default()
                },
            }],
        });
    }
    toc
}

fn paragraph(mut items: Vec<Item>) -> Para {
    if let Some(t) = text_only(&items) {
        if t.trim() == "\\newpage" {
            return Para::PageBreak;
        }
        if is_rule(&t) {
            return Para::Rule;
        }
    }

    let lead = leading(&items).to_string();

    // Titres
    let hashes = lead.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && lead[hashes..].starts_with(' ') {
        strip_start(&mut items, hashes + 1);
        return Para::Heading {
            level: hashes.min(3) as u8,
            runs: runs(items),
        };
    }

    // Listes
    let indent = lead.chars().take_while(|c| *c == ' ').count();
    let rest = &lead[indent..];
    let depth = (indent / 2).min(5) as u8;
    for bullet in ["- ", "* ", "• ", "+ "] {
        if rest.starts_with(bullet) {
            strip_start(&mut items, indent + bullet.len());
            return Para::Item {
                number: None,
                depth,
                runs: runs(items),
            };
        }
    }
    let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if (1..=4).contains(&digits)
        && (rest[digits..].starts_with(". ") || rest[digits..].starts_with(") "))
    {
        let number = rest[..digits].parse().unwrap_or(1);
        strip_start(&mut items, indent + digits + 2);
        return Para::Item {
            number: Some(number),
            depth,
            runs: runs(items),
        };
    }

    // Citation
    if let Some(r) = lead.strip_prefix('>') {
        strip_start(&mut items, if r.starts_with(' ') { 2 } else { 1 });
        return Para::Quote { runs: runs(items) };
    }

    // Alignement : `-> centré <-`, `-> à droite ->`
    if lead.starts_with("->") {
        let tail = trailing(&items).trim_end().to_string();
        let align = if tail.ends_with("<-") {
            Some(Align::Center)
        } else if tail.ends_with("->") && !(items.len() == 1 && tail.trim() == "->") {
            Some(Align::Right)
        } else {
            None
        };
        if let Some(align) = align {
            let trailing_spaces = trailing(&items).len() - tail.len();
            strip_end(&mut items, trailing_spaces + 2);
            strip_start(&mut items, 2);
            let mut runs = runs(items);
            trim_runs(&mut runs);
            return Para::Text { align, runs };
        }
    }

    Para::Text {
        align: Align::Left,
        runs: runs(items),
    }
}

fn trim_runs(runs: &mut Vec<Run>) {
    if let Some(Run::Text { text, .. }) = runs.first_mut() {
        *text = text.trim_start().to_string();
    }
    if let Some(Run::Text { text, .. }) = runs.last_mut() {
        *text = text.trim_end().to_string();
    }
    runs.retain(|r| !matches!(r, Run::Text { text, .. } if text.is_empty()));
}

/// Dernière cellule, créée au besoin : ne panique jamais.
fn derniere_cellule(cells: &mut Vec<Vec<Item>>) -> &mut Vec<Item> {
    if cells.is_empty() {
        cells.push(Vec::new());
    }
    let i = cells.len() - 1;
    &mut cells[i]
}

/// Découpe une ligne de tableau en cellules, sur les `|` de la source.
fn cells(items: Vec<Item>) -> Vec<Vec<Run>> {
    let mut cells: Vec<Vec<Item>> = vec![Vec::new()];
    for item in items {
        match item {
            Item::Text(t) => {
                let mut buf = String::new();
                let mut chars = t.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '\\' && chars.peek() == Some(&'|') {
                        buf.push_str("\\|");
                        chars.next();
                    } else if c == '|' {
                        if !buf.is_empty() {
                            derniere_cellule(&mut cells).push(Item::Text(std::mem::take(&mut buf)));
                        }
                        cells.push(Vec::new());
                    } else {
                        buf.push(c);
                    }
                }
                if !buf.is_empty() {
                    derniere_cellule(&mut cells).push(Item::Text(buf));
                }
            }
            other => derniere_cellule(&mut cells).push(other),
        }
    }
    // Le texte avant le premier `|` et après le dernier n'est pas une cellule.
    cells.remove(0);
    cells.pop();
    cells
        .into_iter()
        .map(|c| {
            let mut r = runs(c);
            trim_runs(&mut r);
            r
        })
        .collect()
}

// ---------------------------------------------------------------------------------
// Caractères : **gras**, *italique*, __souligné__, ~~barré~~
// ---------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    Bold,
    Italic,
    Underline,
    Strike,
}

impl Mark {
    fn text(self) -> &'static str {
        match self {
            Mark::Bold => "**",
            Mark::Italic => "*",
            Mark::Underline => "__",
            Mark::Strike => "~~",
        }
    }
}

enum Tok {
    Char(char),
    Mark(Mark),
    Atom(Item),
}

fn tokens(items: Vec<Item>) -> Vec<Tok> {
    let mut out = Vec::new();
    for item in items {
        let Item::Text(t) = item else {
            out.push(Tok::Atom(item));
            continue;
        };
        let chars: Vec<char> = t.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            let next = chars.get(i + 1).copied();
            if let ('\\', Some(e @ ('*' | '_' | '~' | '|' | '#' | '>'))) = (c, next) {
                out.push(Tok::Char(e));
                i += 2;
            } else if c == '*' && next == Some('*') {
                out.push(Tok::Mark(Mark::Bold));
                i += 2;
            } else if c == '_' && next == Some('_') {
                out.push(Tok::Mark(Mark::Underline));
                i += 2;
            } else if c == '~' && next == Some('~') {
                out.push(Tok::Mark(Mark::Strike));
                i += 2;
            } else if c == '*' {
                out.push(Tok::Mark(Mark::Italic));
                i += 1;
            } else {
                out.push(Tok::Char(c));
                i += 1;
            }
        }
    }
    out
}

fn is_space(t: Option<&Tok>) -> bool {
    match t {
        None => true,
        Some(Tok::Char(c)) => c.is_whitespace(),
        _ => false,
    }
}

/// Les morceaux mis en forme d'une suite d'éléments.
fn runs(items: Vec<Item>) -> Vec<Run> {
    let toks = tokens(items);
    // Une marque ouvre si elle est suivie d'autre chose qu'une espace et qu'une marque
    // de même nature, précédée d'autre chose qu'une espace, la ferme plus loin.
    let can_open = |i: usize| !is_space(toks.get(i + 1));
    let can_close = |i: usize| i > 0 && !is_space(toks.get(i - 1));
    let closes_later = |i: usize, m: Mark| {
        toks.iter()
            .enumerate()
            .skip(i + 2)
            .any(|(j, t)| matches!(t, Tok::Mark(k) if *k == m) && can_close(j))
    };

    let mut out: Vec<Run> = Vec::new();
    let mut style = Style::default();
    let push_text = |out: &mut Vec<Run>, s: &str, style: Style| match out.last_mut() {
        Some(Run::Text { text, style: st }) if *st == style => text.push_str(s),
        _ => out.push(Run::Text {
            text: s.to_string(),
            style,
        }),
    };

    for (i, tok) in toks.iter().enumerate() {
        match tok {
            Tok::Char(c) => push_text(&mut out, c.encode_utf8(&mut [0; 4]), style),
            Tok::Mark(m) => {
                let on = match m {
                    Mark::Bold => &mut style.bold,
                    Mark::Italic => &mut style.italic,
                    Mark::Underline => &mut style.underline,
                    Mark::Strike => &mut style.strike,
                };
                if *on && can_close(i) {
                    *on = false;
                } else if !*on && can_open(i) && closes_later(i, *m) {
                    *on = true;
                } else {
                    push_text(&mut out, m.text(), style);
                }
            }
            Tok::Atom(item) => out.push(match item.clone() {
                Item::Value { name, text } => Run::Value { name, text, style },
                Item::Missing { name, raw } => Run::Missing { name, raw, style },
                Item::Math(t) => Run::Math { tex: t, style },
                Item::Text(t) => Run::Text { text: t, style },
            }),
        }
    }
    out
}

/// Une ligne de texte mise en forme (en-tête, pied de page, cellule…).
pub fn inline(src: &str, values: &Values) -> Vec<Run> {
    let mut all = Vec::new();
    for para in document(src, values) {
        if !all.is_empty() {
            all.push(Run::Text {
                text: " ".into(),
                style: Style::default(),
            });
        }
        all.extend(para.runs().iter().cloned());
    }
    all
}

// ---------------------------------------------------------------------------------
// Statistiques
// ---------------------------------------------------------------------------------

/// Nombre de mots et de caractères (espaces comprises), comme la barre d'état de Writer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    pub words: usize,
    pub chars: usize,
    pub paragraphs: usize,
}

pub fn stats(doc: &[Para]) -> Stats {
    let mut s = Stats::default();
    for p in doc {
        let text = p.plain();
        if text.trim().is_empty() {
            continue;
        }
        s.paragraphs += 1;
        s.words += text.split_whitespace().count();
        s.chars += text.chars().filter(|c| *c != '\n').count();
    }
    s
}

#[cfg(test)]
mod tests_sommaire {
    use super::*;

    fn textes(doc: &[Para]) -> Vec<String> {
        doc.iter().map(Para::plain).collect()
    }

    #[test]
    fn sommaire_liste_les_titres() {
        let doc = document("\\sommaire\n# Un\n## Deux {{x}}\n### Trois\ntexte", &{
            let mut v = Values::default();
            v.set("x", "bis");
            v
        });
        let t = textes(&doc);
        assert_eq!(t[0], "Table des matières");
        assert_eq!(&t[1..4], ["Un", "Deux bis", "Trois"]);
        assert!(matches!(doc[1], Para::Item { depth: 0, .. }));
        assert!(matches!(doc[3], Para::Item { depth: 2, .. }));
        // Les titres eux-mêmes sont conservés après le sommaire.
        assert!(matches!(doc[4], Para::Heading { level: 1, .. }));
    }

    #[test]
    fn sommaire_sans_titre_et_multiple() {
        let doc = document("\\sommaire\nrien", &Values::default());
        assert_eq!(doc.len(), 3);
        assert!(textes(&doc)[1].contains("aucun titre"));
        let doc = document("\\sommaire\n# A\n\\sommaire", &Values::default());
        assert_eq!(
            doc.iter()
                .filter(|p| p.plain() == "Table des matières")
                .count(),
            2
        );
    }

    #[test]
    fn sommaire_dans_un_paragraphe_reste_du_texte() {
        let doc = document("voir \\sommaire ici", &Values::default());
        assert_eq!(doc.len(), 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(src: &str) -> Vec<Para> {
        let mut v = Values::default();
        v.set("nom", "**Dupont**").set("x", "5");
        document(src, &v)
    }

    fn texts(runs: &[Run]) -> Vec<(String, Style)> {
        runs.iter().map(|r| (r.plain(), r.style())).collect()
    }

    const B: Style = Style {
        bold: true,
        italic: false,
        underline: false,
        strike: false,
    };
    const I: Style = Style {
        bold: false,
        italic: true,
        underline: false,
        strike: false,
    };
    const N: Style = Style {
        bold: false,
        italic: false,
        underline: false,
        strike: false,
    };

    #[test]
    fn caracteres() {
        let d = doc("Un **mot** et *un autre* et __s__ et ~~b~~");
        let Para::Text { runs, .. } = &d[0] else {
            panic!()
        };
        let t = texts(runs);
        assert_eq!(t[0], ("Un ".into(), N));
        assert_eq!(t[1], ("mot".into(), B));
        assert_eq!(t[3], ("un autre".into(), I));
        assert!(t[5].1.underline && t[7].1.strike);
    }

    #[test]
    fn les_valeurs_ne_sont_jamais_interpretees() {
        let d = doc("Bonjour {{nom}} # pas un titre");
        let Para::Text { runs, .. } = &d[0] else {
            panic!()
        };
        assert_eq!(runs[1].plain(), "**Dupont**");
        assert_eq!(runs[1].style(), N);
        // une valeur en gras reste une valeur
        let d = doc("**{{nom}}**");
        assert!(matches!(&d[0].runs()[0], Run::Value { style, .. } if style.bold));
    }

    #[test]
    fn marques_seules_restent_du_texte() {
        let d = doc("5 * 3 = 15 et un ** seul");
        assert_eq!(d[0].plain(), "5 * 3 = 15 et un ** seul");
        assert!(d[0].runs().iter().all(|r| r.style() == N));
        assert_eq!(doc("\\*pas italique\\*")[0].plain(), "*pas italique*");
    }

    #[test]
    fn structure() {
        let d = doc(
            "# Titre\n## Sous {{x}}\n- a\n  - b\n3. c\n> cite\n-> centre <-\n-> droite ->\n---\n\\newpage\n\nfin",
        );
        assert!(matches!(&d[0], Para::Heading { level: 1, .. }));
        assert_eq!(d[1].plain(), "Sous 5");
        assert!(matches!(
            &d[2],
            Para::Item {
                number: None,
                depth: 0,
                ..
            }
        ));
        assert!(matches!(
            &d[3],
            Para::Item {
                number: None,
                depth: 1,
                ..
            }
        ));
        assert!(matches!(
            &d[4],
            Para::Item {
                number: Some(3),
                ..
            }
        ));
        assert!(matches!(&d[5], Para::Quote { .. }));
        assert!(matches!(
            &d[6],
            Para::Text {
                align: Align::Center,
                ..
            }
        ));
        assert_eq!(d[6].plain(), "centre");
        assert!(matches!(
            &d[7],
            Para::Text {
                align: Align::Right,
                ..
            }
        ));
        assert_eq!(d[8], Para::Rule);
        assert_eq!(d[9], Para::PageBreak);
        assert_eq!(
            d[10],
            Para::Text {
                align: Align::Left,
                runs: vec![]
            }
        );
        assert_eq!(d[11].plain(), "fin");
        // une flèche dans le texte n'aligne rien
        assert_eq!(doc("a -> b")[0].plain(), "a -> b");
    }

    #[test]
    fn tableaux() {
        let d =
            doc("| Article | Prix |\n|---|---:|\n| Stylo **bleu** | {{x}} € |\n| a \\| b | 2 |");
        let Para::Table(t) = &d[0] else { panic!() };
        assert!(t.header);
        assert_eq!(t.aligns, [Align::Left, Align::Right]);
        assert_eq!(t.rows.len(), 3);
        assert_eq!(plain(&t.rows[1][0]), "Stylo bleu");
        assert_eq!(plain(&t.rows[1][1]), "5 €");
        assert_eq!(plain(&t.rows[2][0]), "a | b");
    }

    #[test]
    fn formules_centrees() {
        let d = doc("Avant :\n$$ x^2 $$\nAprès");
        assert_eq!(d.len(), 3);
        assert!(matches!(&d[1], Para::Math { .. }));
        assert_eq!(d[2].plain(), "Après");
    }

    #[test]
    fn statistiques() {
        let s = stats(&doc("Deux mots\n\n- trois mots ici"));
        assert_eq!(s.words, 5);
        assert_eq!(s.paragraphs, 2);
    }
}
