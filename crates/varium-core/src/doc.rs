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
//! | `==surligné==`, `{couleur:rouge}texte{/}`, `{surlignage:vert}texte{/}` | couleur et surlignage |
//! | `- élément`, `1. élément` (2 espaces de retrait par niveau) | listes |
//! | `> citation` | citation |
//! | `-> centré <-`, `-> à droite ->` | alignement |
//! | `{: retrait=1cm alinéa=5mm interligne=1,5 avant=6pt après=6pt} texte` | mise en forme du paragraphe |
//! | `{: interligne=double}` seul sur sa ligne (puis `{:}`) | mise en forme des paragraphes suivants |
//! | `\| a \| b \|` (+ `\|---\|:---:\|` sous l'en-tête) | tableau |
//! | `---` | ligne horizontale |
//! | `\newpage` | saut de page |
//! | `\sommaire` | table des matières (titres et numéros de page) |
//! | `texte[^1]` puis `[^1]: la note`, `^[note en ligne]`, `\note{texte}` | notes de bas de page |
//! | `[texte](https://…)` | lien (aussi `mailto:`, `www.`) |
//! | `{{qr:texte}}`, `{{champ \| qr}}` | code QR, dans son propre paragraphe |
//! | `{{signature}}`, `{{signature:nom}}` | signature manuscrite |
//! | `\*`, `\_`, `\~`, `\|`, `\#`, `\>`, `\=`, `\{` | caractère littéral |

use crate::render::{Block, Piece, Values, blocks, render_pieces};
use crate::tex;

/// Une couleur RVB (texte ou surlignage).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Couleur {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Couleurs nommées du texte : (noms acceptés, couleur du texte, couleur de surlignage).
const COULEURS: &[(&[&str], u32, u32)] = &[
    (&["noir", "black"], 0x000000, 0xBFBFBF),
    (&["blanc", "white"], 0xFFFFFF, 0xFFFFFF),
    (&["gris", "grey", "gray"], 0x707070, 0xDDDDDD),
    (&["rouge", "red"], 0xC9211E, 0xFFB3B3),
    (&["orange"], 0xE8710A, 0xFFD8A8),
    (&["jaune", "yellow"], 0xB8860B, 0xFFF176),
    (&["vert", "green"], 0x2E8B3A, 0xB6F5A9),
    (&["turquoise", "teal"], 0x1AA39A, 0xB0EFE5),
    (&["cyan"], 0x0092B0, 0xB8F4FF),
    (&["bleu", "blue"], 0x1F5FBF, 0xBFE0FF),
    (&["violet", "purple"], 0x7B3FA0, 0xE0C8FF),
    (&["magenta"], 0xC2187E, 0xFFC0E8),
    (&["rose", "pink"], 0xE0529C, 0xFFC6E5),
    (&["marron", "brun", "brown"], 0x8B5A2B, 0xE6CDB0),
];

impl Couleur {
    pub const fn rvb(v: u32) -> Couleur {
        Couleur {
            r: (v >> 16) as u8,
            g: (v >> 8) as u8,
            b: v as u8,
        }
    }

    /// Le jaune du surlignage `==…==`.
    pub const SURLIGNAGE: Couleur = Couleur::rvb(0xFFF176);

    /// Les noms français des couleurs proposées dans l'interface.
    pub fn noms() -> impl Iterator<Item = &'static str> {
        COULEURS.iter().map(|(n, _, _)| n[0])
    }

    fn lire_avec(s: &str, surlignage: bool) -> Option<Couleur> {
        let s = s.trim();
        if let Some(h) = s.strip_prefix('#') {
            if !h.chars().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            return match h.len() {
                6 => u32::from_str_radix(h, 16).ok().map(Couleur::rvb),
                3 => {
                    let v = u32::from_str_radix(h, 16).ok()?;
                    let (r, g, b) = ((v >> 8) & 15, (v >> 4) & 15, v & 15);
                    Some(Couleur::rvb((r * 17) << 16 | (g * 17) << 8 | b * 17))
                }
                _ => None,
            };
        }
        let bas = s.to_lowercase();
        COULEURS
            .iter()
            .find(|(noms, _, _)| noms.contains(&bas.as_str()))
            .map(|(_, t, f)| Couleur::rvb(if surlignage { *f } else { *t }))
    }

    /// Couleur du texte : un nom (`rouge`, `bleu`…, aussi en anglais) ou `#rrggbb` / `#rgb`.
    pub fn lire(s: &str) -> Option<Couleur> {
        Couleur::lire_avec(s, false)
    }

    /// Couleur de surlignage : les mêmes noms donnent des teintes claires.
    pub fn lire_surlignage(s: &str) -> Option<Couleur> {
        Couleur::lire_avec(s, true)
    }

    /// `rrggbb`, en minuscules et sans `#`.
    pub fn hex(self) -> String {
        format!("{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Composantes entre 0 et 1.
    pub fn unites(self) -> [f32; 3] {
        [self.r, self.g, self.b].map(|c| c as f32 / 255.0)
    }
}

/// Style de caractères.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    /// Couleur du texte (`{couleur:rouge}…{/}`) ; `None` : noir.
    pub couleur: Option<Couleur>,
    /// Surlignage (`==…==`, `{surlignage:vert}…{/}`).
    pub surlignage: Option<Couleur>,
    /// Exposant (appel de note).
    pub exposant: bool,
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
#[derive(Debug, Clone, PartialEq, Hash)]
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

/// Préfixe du nom d'un lien `[texte](adresse)` : `lien:adresse`.
pub const LIEN: &str = "lien:";
/// Préfixe du nom d'un appel de note de bas de page : `note:3`.
pub const NOTE: &str = "note:";
/// Préfixe du nom d'une entrée de table des matières : `sommaire:2` renvoie au
/// troisième titre du document.
pub const SOMMAIRE: &str = "sommaire:";

impl Run {
    /// Le contenu d'un code QR (`{{qr:…}}` ou `{{champ | qr}}`).
    pub fn qr(&self) -> Option<&str> {
        match self {
            Run::Value { name, text, .. } if name.starts_with(crate::parse::QR) => Some(text),
            _ => None,
        }
    }

    /// Un lien : (adresse, texte affiché).
    pub fn lien(&self) -> Option<(&str, &str)> {
        match self {
            Run::Value { name, text, .. } => name.strip_prefix(LIEN).map(|u| (u, text.as_str())),
            _ => None,
        }
    }

    /// Le numéro (à partir de 1) d'un appel de note de bas de page.
    pub fn note(&self) -> Option<usize> {
        match self {
            Run::Value { name, .. } => name.strip_prefix(NOTE)?.parse().ok(),
            _ => None,
        }
    }

    /// Une entrée de table des matières : le rang du titre visé (à partir de 0).
    pub fn sommaire(&self) -> Option<usize> {
        match self {
            Run::Value { name, .. } => name.strip_prefix(SOMMAIRE)?.parse().ok(),
            _ => None,
        }
    }

    /// Une signature manuscrite : son nom (vide pour la signature par défaut).
    pub fn signature(&self) -> Option<&str> {
        match self {
            Run::Value { name, .. } => name.strip_prefix(crate::parse::SIGNATURE),
            _ => None,
        }
    }
}

/// Interligne d'un paragraphe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Interligne {
    /// Multiple de l'interligne simple (1, 1,15, 1,5, 2…).
    Multiple(f32),
    /// Hauteur de ligne fixe, en points.
    Exact(f32),
}

/// Mise en forme d'un paragraphe (`{: retrait=1cm interligne=1,5 …}`). Les valeurs
/// absentes suivent la mise en page du document.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ParaFormat {
    /// Retrait à gauche, en millimètres.
    pub gauche: Option<f32>,
    /// Retrait à droite, en millimètres.
    pub droite: Option<f32>,
    /// Retrait de première ligne (alinéa), en millimètres ; négatif : retrait suspendu.
    pub alinea: Option<f32>,
    pub interligne: Option<Interligne>,
    /// Espace avant le paragraphe, en points.
    pub avant: Option<f32>,
    /// Espace après le paragraphe, en points.
    pub apres: Option<f32>,
}

impl std::hash::Hash for ParaFormat {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        for v in [
            self.gauche,
            self.droite,
            self.alinea,
            self.avant,
            self.apres,
        ] {
            v.map(f32::to_bits).hash(h);
        }
        match self.interligne {
            None => 0u32.hash(h),
            Some(Interligne::Multiple(m)) => (1u32, m.to_bits()).hash(h),
            Some(Interligne::Exact(m)) => (2u32, m.to_bits()).hash(h),
        }
    }
}

impl ParaFormat {
    /// Aucune mise en forme particulière.
    pub fn est_vide(&self) -> bool {
        *self == ParaFormat::default()
    }

    /// Les valeurs de `self`, complétées par celles de `base`.
    pub fn sur(self, base: ParaFormat) -> ParaFormat {
        ParaFormat {
            gauche: self.gauche.or(base.gauche),
            droite: self.droite.or(base.droite),
            alinea: self.alinea.or(base.alinea),
            interligne: self.interligne.or(base.interligne),
            avant: self.avant.or(base.avant),
            apres: self.apres.or(base.apres),
        }
    }

    /// Multiple de l'interligne simple, si l'interligne est proportionnel.
    pub fn multiple(&self) -> Option<f32> {
        match self.interligne {
            Some(Interligne::Multiple(m)) => Some(m),
            _ => None,
        }
    }
}

/// L'adresse complète d'un lien (`www.x.fr` → `https://www.x.fr`).
pub fn adresse_lien(url: &str) -> String {
    if url.starts_with("www.") {
        format!("https://{url}")
    } else {
        url.to_string()
    }
}

/// Alignement d'un paragraphe.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Un tableau.
#[derive(Debug, Clone, PartialEq, Default, Hash)]
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
#[derive(Debug, Clone, PartialEq, Hash)]
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

    /// Applique `f` à chaque suite de morceaux du paragraphe (cellules comprises).
    pub fn pour_chaque_runs(&mut self, f: &mut dyn FnMut(&mut Vec<Run>)) {
        match self {
            Para::Text { runs, .. }
            | Para::Heading { runs, .. }
            | Para::Item { runs, .. }
            | Para::Quote { runs } => f(runs),
            Para::Table(t) => t.rows.iter_mut().flatten().for_each(|c| f(c)),
            Para::Math { .. } | Para::Rule | Para::PageBreak => {}
        }
    }

    /// Le contenu du code QR si le paragraphe n'est qu'un code QR.
    pub fn qr(&self) -> Option<&str> {
        match self {
            Para::Text { runs, .. } if runs.len() == 1 => runs[0].qr(),
            _ => None,
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
    /// Lien `[texte](adresse)`.
    Lien {
        url: String,
        label: String,
    },
    Value {
        name: String,
        text: String,
    },
    Missing {
        name: String,
        raw: String,
    },
    Math(String),
    /// Appel de la note de bas de page n° k.
    Note(usize),
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

/// Le document complet : paragraphes, mise en forme de chacun et notes de bas de page.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Document {
    pub paras: Vec<Para>,
    /// Mise en forme de chaque paragraphe (même longueur que `paras`).
    pub formats: Vec<ParaFormat>,
    /// Contenu des notes de bas de page : la note n° k est à l'indice k − 1.
    pub notes: Vec<Vec<Run>>,
}

/// Attributs d'un paragraphe : `{: retrait=1cm interligne=1,5 couleur=bleu}`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Attributs {
    pub format: ParaFormat,
    /// Couleur du texte de tout le paragraphe.
    pub couleur: Option<Couleur>,
    /// Surlignage de tout le paragraphe.
    pub surlignage: Option<Couleur>,
}

impl Attributs {
    /// Les valeurs de `self`, complétées par celles de `base`.
    pub fn sur(self, base: Attributs) -> Attributs {
        Attributs {
            format: self.format.sur(base.format),
            couleur: self.couleur.or(base.couleur),
            surlignage: self.surlignage.or(base.surlignage),
        }
    }

    pub fn est_vide(&self) -> bool {
        *self == Attributs::default()
    }

    fn style(&self) -> Style {
        Style {
            couleur: self.couleur,
            surlignage: self.surlignage,
            ..Style::default()
        }
    }
}

/// Nombre à la française : `1,5`, `12`.
fn nombre_fr(v: f32) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.replace('.', ",")
}

/// Longueur en millimètres écrite en cm quand c'est un compte rond de millimètres.
fn longueur_ecrite(mm: f32) -> String {
    if (mm * 10.0).round() % 10.0 == 0.0 && mm.abs() >= 10.0 {
        format!("{}cm", nombre_fr(mm / 10.0))
    } else {
        format!("{}mm", nombre_fr(mm))
    }
}

/// L'écriture `{: …}` d'attributs ; `{:}` s'ils sont vides.
pub fn ecrire_attributs(a: &Attributs) -> String {
    let f = &a.format;
    let mut parts: Vec<String> = Vec::new();
    if let Some(v) = f.gauche {
        parts.push(format!("retrait={}", longueur_ecrite(v)));
    }
    if let Some(v) = f.droite {
        parts.push(format!("droite={}", longueur_ecrite(v)));
    }
    if let Some(v) = f.alinea {
        parts.push(format!("alinéa={}", longueur_ecrite(v)));
    }
    match f.interligne {
        Some(Interligne::Multiple(m)) if (m - 1.0).abs() < 0.001 => {
            parts.push("interligne=simple".into())
        }
        Some(Interligne::Multiple(m)) if (m - 2.0).abs() < 0.001 => {
            parts.push("interligne=double".into())
        }
        Some(Interligne::Multiple(m)) => parts.push(format!("interligne={}", nombre_fr(m))),
        Some(Interligne::Exact(p)) => parts.push(format!("interligne=exact:{}pt", nombre_fr(p))),
        None => {}
    }
    if let Some(v) = f.avant {
        parts.push(format!("avant={}pt", nombre_fr(v)));
    }
    if let Some(v) = f.apres {
        parts.push(format!("après={}pt", nombre_fr(v)));
    }
    let nom = |c: Couleur, surl: bool| {
        COULEURS
            .iter()
            .find(|(_, t, s)| Couleur::rvb(if surl { *s } else { *t }) == c)
            .map(|(n, _, _)| n[0].to_string())
            .unwrap_or_else(|| format!("#{}", c.hex()))
    };
    if let Some(c) = a.couleur {
        parts.push(format!("couleur={}", nom(c, false)));
    }
    if let Some(c) = a.surlignage {
        parts.push(format!("surlignage={}", nom(c, true)));
    }
    if parts.is_empty() {
        "{:}".into()
    } else {
        format!("{{: {}}}", parts.join(" "))
    }
}

/// Un nombre (virgule ou point décimal) suivi d'une unité facultative.
fn nombre_unite(v: &str) -> Option<(f32, &str)> {
    let v = v.trim();
    let fin = v
        .char_indices()
        .find(|(i, c)| {
            !(c.is_ascii_digit() || *c == ',' || *c == '.' || (*i == 0 && (*c == '-' || *c == '+')))
        })
        .map_or(v.len(), |(i, _)| i);
    let n: f32 = v[..fin].replace(',', ".").parse().ok()?;
    n.is_finite().then_some((n, v[fin..].trim()))
}

/// Longueur en millimètres ; `defaut` est l'unité d'un nombre seul.
fn longueur_mm(v: &str, defaut: &str) -> Option<f32> {
    let (n, u) = nombre_unite(v)?;
    let u = if u.is_empty() { defaut } else { u };
    let mm = match u.to_lowercase().as_str() {
        "mm" => n,
        "cm" => n * 10.0,
        "pt" => n * 25.4 / 72.0,
        "in" | "po" | "pouce" | "pouces" => n * 25.4,
        _ => return None,
    };
    Some(mm.clamp(-500.0, 500.0))
}

/// Longueur en points (espaces avant et après).
fn longueur_pt(v: &str) -> Option<f32> {
    longueur_mm(v, "pt").map(|mm| (mm * 72.0 / 25.4).clamp(0.0, 500.0))
}

fn lire_interligne(v: &str) -> Option<Interligne> {
    let b = v.trim().to_lowercase();
    match b.as_str() {
        "simple" | "single" => return Some(Interligne::Multiple(1.0)),
        "double" => return Some(Interligne::Multiple(2.0)),
        _ => {}
    }
    if let Some(e) = b
        .strip_prefix("exact")
        .map(|r| r.trim_start_matches([':', '=']))
    {
        return longueur_pt(e).map(|p| Interligne::Exact(p.clamp(2.0, 200.0)));
    }
    let (n, u) = nombre_unite(&b)?;
    if u.is_empty() || u == "x" || u == "×" {
        Some(Interligne::Multiple(n.clamp(0.5, 5.0)))
    } else {
        longueur_pt(&b).map(|p| Interligne::Exact(p.clamp(2.0, 200.0)))
    }
}

/// Lit le contenu d'un bloc `{: …}` (sans les accolades) ; `None` si une clé est
/// inconnue ou une valeur illisible (le bloc reste alors du texte).
pub fn lire_attributs(contenu: &str) -> Option<Attributs> {
    let mut a = Attributs::default();
    for tok in contenu.split(|c: char| c.is_whitespace() || c == ';') {
        if tok.is_empty() {
            continue;
        }
        let (k, v) = tok.split_once(['=', ':'])?;
        let k = k.trim().to_lowercase();
        let f = &mut a.format;
        match k.as_str() {
            "retrait" | "gauche" | "retrait-gauche" => f.gauche = Some(longueur_mm(v, "cm")?),
            "droite" | "retrait-droit" | "retrait-droite" => f.droite = Some(longueur_mm(v, "cm")?),
            "alinéa" | "alinea" | "première-ligne" | "premiere-ligne" => {
                f.alinea = Some(longueur_mm(v, "cm")?)
            }
            "interligne" => f.interligne = Some(lire_interligne(v)?),
            "avant" | "espace-avant" => f.avant = Some(longueur_pt(v)?),
            "après" | "apres" | "espace-après" | "espace-apres" => {
                f.apres = Some(longueur_pt(v)?)
            }
            "couleur" | "color" => a.couleur = Some(Couleur::lire(v)?),
            "surlignage" | "fond" | "highlight" => {
                a.surlignage = Some(Couleur::lire_surlignage(v)?)
            }
            _ => return None,
        }
    }
    Some(a)
}

/// Le bloc `{: …}` au début d'une ligne de la source : ses attributs et sa longueur en
/// octets (espace qui le suit comprise).
pub fn attributs_de_ligne(ligne: &str) -> Option<(Attributs, usize)> {
    let t = ligne.trim_start();
    let decalage = ligne.len() - t.len();
    let reste = t.strip_prefix("{:")?;
    let fin = reste.find(['}', '{', '\n'])?;
    if !reste[fin..].starts_with('}') {
        return None;
    }
    let a = lire_attributs(&reste[..fin])?;
    let mut n = decalage + 2 + fin + 1;
    if ligne[n..].starts_with(' ') {
        n += 1;
    }
    Some((a, n))
}

/// Applique `f` aux attributs `{: …}` des lignes de la source qui touchent les octets
/// `debut..=fin` : le bloc est réécrit, ajouté ou retiré (attributs vides).
pub fn modifier_attributs(
    src: &str,
    debut: usize,
    fin: usize,
    f: &dyn Fn(Attributs) -> Attributs,
) -> String {
    let mut out = String::with_capacity(src.len() + 32);
    let mut pos = 0;
    let mut lignes: Vec<&str> = src.split_inclusive('\n').collect();
    if src.is_empty() || src.ends_with('\n') {
        lignes.push("");
    }
    for ligne in lignes {
        let contenu = ligne.strip_suffix('\n').unwrap_or(ligne);
        let fin_ligne = pos + contenu.len();
        if pos <= fin && debut <= fin_ligne {
            let (a, n) = attributs_de_ligne(contenu).unwrap_or_default();
            let nouveaux = f(a);
            if !nouveaux.est_vide() {
                out.push_str(&ecrire_attributs(&nouveaux));
                out.push(' ');
            }
            out.push_str(&contenu[n..]);
            out.push_str(&ligne[contenu.len()..]);
        } else {
            out.push_str(ligne);
        }
        pos += ligne.len();
    }
    out
}

/// Retire le bloc `{: …}` du début de la ligne.
fn extraire_attributs(items: &mut Vec<Item>) -> Option<Attributs> {
    let (a, n) = attributs_de_ligne(leading(items))?;
    strip_start(items, n);
    Some(a)
}

fn seulement_des_espaces(items: &[Item]) -> bool {
    text_only(items).is_some_and(|t| t.trim().is_empty())
}

/// Les notes du document : définitions `[^id]: …`, numéros attribués et contenus.
#[derive(Default)]
struct Notes {
    definitions: std::collections::HashMap<String, Vec<Item>>,
    numeros: std::collections::HashMap<String, usize>,
    contenus: Vec<Vec<Item>>,
}

impl Notes {
    fn ajouter(&mut self, contenu: Vec<Item>) -> usize {
        self.contenus.push(contenu);
        self.contenus.len()
    }

    /// Numéro de la note `[^id]`, attribué au premier appel ; `None` sans définition.
    fn appel(&mut self, id: &str) -> Option<usize> {
        if let Some(n) = self.numeros.get(id) {
            return Some(*n);
        }
        let contenu = self.definitions.get(id)?.clone();
        let n = self.ajouter(contenu);
        self.numeros.insert(id.to_string(), n);
        Some(n)
    }
}

/// Identifiant valide d'une note `[^id]`.
fn id_note(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && !s.chars().any(|c| c.is_whitespace() || "[]^".contains(c))
}

/// Une ligne `[^id]: texte` : l'identifiant et le contenu de la note.
fn definition_note(items: &mut Vec<Item>) -> Option<(String, Vec<Item>)> {
    let lead = leading(items);
    let t = lead.trim_start_matches(' ');
    if lead.len() - t.len() > 3 {
        return None;
    }
    let r = t.strip_prefix("[^")?;
    let fin = r.find(']')?;
    let id = &r[..fin];
    if !id_note(id) || !r[fin + 1..].starts_with(':') {
        return None;
    }
    let id = id.to_string();
    let apres = &r[fin + 2..];
    let n = lead.len() - apres.len() + (apres.len() - apres.trim_start().len());
    let mut contenu = std::mem::take(items);
    strip_start(&mut contenu, n);
    Some((id, contenu))
}

/// Sortie en construction : paragraphes et mise en forme de chacun.
#[derive(Default)]
struct Sortie {
    paras: Vec<Para>,
    formats: Vec<ParaFormat>,
}

impl Sortie {
    fn push(&mut self, p: Para, f: ParaFormat) {
        self.paras.push(p);
        self.formats.push(f);
    }
}

/// Le document mis en forme, avec la mise en forme des paragraphes et les notes de
/// bas de page à part (appels `note:n` dans le texte).
pub fn document_complet(src: &str, values: &Values) -> Document {
    let mut toutes = lines(src, values);
    // Définitions de notes `[^id]: texte` : retirées du fil du texte (la première
    // définition d'un identifiant l'emporte).
    let mut notes = Notes::default();
    toutes.retain_mut(|l| match l {
        Line::Items(items) => match definition_note(items) {
            Some((id, contenu)) => {
                notes.definitions.entry(id).or_insert(contenu);
                false
            }
            None => true,
        },
        Line::Display(_) => true,
    });

    let mut out = Sortie::default();
    // Positions des `\sommaire`, remplies une fois tous les titres connus.
    let mut sommaires: Vec<usize> = Vec::new();
    // Attributs d'un `{: …}` seul sur sa ligne : valent pour les paragraphes suivants.
    let mut courants = Attributs::default();
    let mut lignes = toutes.into_iter().peekable();
    while let Some(line) = lignes.next() {
        let mut items = match line {
            Line::Display(t) => {
                out.push(Para::Math { tex: t }, courants.format);
                continue;
            }
            Line::Items(items) => items,
        };
        let attrs = match extraire_attributs(&mut items) {
            Some(a) if seulement_des_espaces(&items) => {
                courants = a;
                continue;
            }
            Some(a) => a.sur(courants),
            None => courants,
        };
        let items = extraire_notes(items, &mut notes);
        let base = attrs.style();

        // Tableau : lignes consécutives qui commencent et finissent par `|`.
        if is_table_line(&items) {
            let mut table = Table::default();
            let mut rows = vec![items];
            while let Some(Line::Items(next)) = lignes.peek() {
                if !is_table_line(next) {
                    break;
                }
                let Some(Line::Items(next)) = lignes.next() else {
                    break;
                };
                rows.push(extraire_notes(next, &mut notes));
            }
            for (i, row) in rows.into_iter().enumerate() {
                if let Some(aligns) = text_only(&row).as_deref().and_then(separator_aligns) {
                    if i == 1 && !table.header {
                        table.header = true;
                        table.aligns = aligns;
                    }
                    continue;
                }
                table.rows.push(cells(row, base));
            }
            if !table.rows.is_empty() {
                out.push(Para::Table(table), attrs.format);
            }
            continue;
        }

        if text_only(&items).is_some_and(|t| t.trim() == "\\sommaire") {
            sommaires.push(out.paras.len());
            out.push(Para::Rule, ParaFormat::default());
            continue;
        }

        let mut morceaux = Vec::new();
        isoler_qr(paragraph(items, base), &mut morceaux);
        for p in morceaux {
            out.push(p, attrs.format);
        }
    }
    // Du dernier au premier, pour que les positions restent valables.
    if !sommaires.is_empty() {
        let toc = table_des_matieres(&out.paras);
        for pos in sommaires.into_iter().rev() {
            out.paras.splice(pos..=pos, toc.iter().cloned());
            out.formats
                .splice(pos..=pos, toc.iter().map(|_| ParaFormat::default()));
        }
    }
    let notes = notes
        .contenus
        .into_iter()
        .map(|c| {
            let mut r = runs(c, Style::default());
            trim_runs(&mut r);
            r
        })
        .collect();
    Document {
        paras: out.paras,
        formats: out.formats,
        notes,
    }
}

/// Le document mis en forme. Les notes de bas de page y deviennent des appels `[n]`,
/// rassemblées en fin de document (aperçus, diapositives, statistiques) ; pour de
/// vraies notes de bas de page, voir [`document_complet`].
pub fn document(src: &str, values: &Values) -> Vec<Para> {
    notes_en_fin(document_complet(src, values))
}

/// Les paragraphes d'un document complet, notes rassemblées à la fin et table des
/// matières en texte simple.
pub fn notes_en_fin(d: Document) -> Vec<Para> {
    let mut paras = d.paras;
    for p in &mut paras {
        p.pour_chaque_runs(&mut |runs: &mut Vec<Run>| {
            for r in runs.iter_mut() {
                if let Some(n) = r.note() {
                    *r = Run::Text {
                        text: format!("[{n}]"),
                        style: Style {
                            exposant: false,
                            ..r.style()
                        },
                    };
                } else if r.sommaire().is_some() {
                    *r = Run::Text {
                        text: r.plain(),
                        style: r.style(),
                    };
                }
            }
        });
    }
    ajouter_notes(&mut paras, d.notes);
    paras
}

/// Un code QR occupe son propre paragraphe : le texte autour forme des paragraphes
/// à part (même alignement).
fn isoler_qr(p: Para, out: &mut Vec<Para>) {
    let Para::Text { align, runs } = p else {
        out.push(p);
        return;
    };
    if runs.len() <= 1 || !runs.iter().any(|r| r.qr().is_some()) {
        out.push(Para::Text { align, runs });
        return;
    }
    let mut cur: Vec<Run> = Vec::new();
    let vider = |cur: &mut Vec<Run>, out: &mut Vec<Para>| {
        trim_runs(cur);
        if !cur.is_empty() {
            out.push(Para::Text {
                align,
                runs: std::mem::take(cur),
            });
        }
    };
    for r in runs {
        if r.qr().is_some() {
            vider(&mut cur, out);
            out.push(Para::Text {
                align,
                runs: vec![r],
            });
        } else {
            cur.push(r);
        }
    }
    vider(&mut cur, out);
}

/// Marques de notes : `\note{…}` et `^[…]` (contenu en ligne), `[^id]` (appel d'une
/// note définie ailleurs).
const NOTE_TEX: &str = "\\note{";
const NOTE_EN_LIGNE: &str = "^[";
const APPEL: &str = "[^";

/// Prochaine marque de note dans `t` : (position, marque).
fn prochaine_marque(t: &str) -> Option<(usize, &'static str)> {
    [NOTE_TEX, NOTE_EN_LIGNE, APPEL]
        .iter()
        .filter_map(|m| t.find(m).map(|i| (i, *m)))
        .min_by_key(|(i, _)| *i)
}

/// Remplace chaque note de la ligne par son appel numéroté et range son contenu
/// (champs compris) dans `notes`. Une note jamais fermée reste du texte ordinaire ;
/// un appel `[^id]` sans définition aussi.
fn extraire_notes(items: Vec<Item>, notes: &mut Notes) -> Vec<Item> {
    if !items.iter().any(|i| {
        matches!(i, Item::Text(t) if t.contains(NOTE_TEX) || t.contains(NOTE_EN_LIGNE) || t.contains(APPEL))
    }) {
        return items;
    }
    let mut out: Vec<Item> = Vec::new();
    // Note en cours de lecture : sa marque d'ouverture, son caractère de fin et son contenu.
    let mut cours: Option<(&'static str, char, Vec<Item>)> = None;
    for item in items {
        let Item::Text(mut t) = item else {
            match cours.as_mut() {
                Some((_, _, c)) => c.push(item),
                None => out.push(item),
            }
            continue;
        };
        loop {
            match cours.as_mut() {
                None => match prochaine_marque(&t) {
                    Some((i, APPEL)) => {
                        let r = &t[i + APPEL.len()..];
                        let appel = r
                            .find(']')
                            .filter(|&f| id_note(&r[..f]))
                            .and_then(|f| notes.appel(&r[..f]).map(|n| (n, f)));
                        match appel {
                            Some((n, f)) => {
                                if i > 0 {
                                    out.push(Item::Text(t[..i].to_string()));
                                }
                                out.push(Item::Note(n));
                                t = t[i + APPEL.len() + f + 1..].to_string();
                            }
                            None => {
                                out.push(Item::Text(t[..i + APPEL.len()].to_string()));
                                t = t[i + APPEL.len()..].to_string();
                            }
                        }
                    }
                    Some((i, marque)) => {
                        if i > 0 {
                            out.push(Item::Text(t[..i].to_string()));
                        }
                        t = t[i + marque.len()..].to_string();
                        let fin = if marque == NOTE_TEX { '}' } else { ']' };
                        cours = Some((marque, fin, Vec::new()));
                    }
                    None => {
                        if !t.is_empty() {
                            out.push(Item::Text(t));
                        }
                        break;
                    }
                },
                Some((_, fin, c)) => match t.find(*fin) {
                    Some(i) => {
                        if i > 0 {
                            c.push(Item::Text(t[..i].to_string()));
                        }
                        let contenu = cours.take().map(|(_, _, c)| c).unwrap_or_default();
                        out.push(Item::Note(notes.ajouter(contenu)));
                        t = t[i + 1..].to_string();
                    }
                    None => {
                        if !t.is_empty() {
                            c.push(Item::Text(t));
                        }
                        break;
                    }
                },
            }
        }
    }
    if let Some((marque, _, c)) = cours.take() {
        out.push(Item::Text(marque.to_string()));
        out.extend(c);
    }
    // Textes voisins fusionnés : le début de ligne (titres, listes…) reste lisible.
    let mut fusion: Vec<Item> = Vec::with_capacity(out.len());
    for i in out {
        match (fusion.last_mut(), i) {
            (Some(Item::Text(a)), Item::Text(b)) => a.push_str(&b),
            (_, i) => fusion.push(i),
        }
    }
    fusion
}

/// Ajoute en fin de document la liste des notes : un filet, « Notes » en gras
/// puis un paragraphe `[n] texte` par note.
fn ajouter_notes(out: &mut Vec<Para>, notes: Vec<Vec<Run>>) {
    if notes.is_empty() {
        return;
    }
    out.push(Para::Rule);
    out.push(Para::Text {
        align: Align::Left,
        runs: vec![Run::Text {
            text: "Notes".to_string(),
            style: Style {
                bold: true,
                ..Style::default()
            },
        }],
    });
    for (i, contenu) in notes.into_iter().enumerate() {
        let mut runs = vec![Run::Text {
            text: format!("[{}] ", i + 1),
            style: Style::default(),
        }];
        runs.extend(contenu);
        out.push(Para::Text {
            align: Align::Left,
            runs,
        });
    }
}

/// Table des matières : un titre en gras puis un élément de liste par titre du
/// document (niveau 1 → retrait 0, niveau 3 → retrait 2). Chaque entrée renvoie à
/// son titre (`sommaire:rang`), pour les numéros de page et les liens.
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
    let titres = doc.iter().filter_map(|p| match p {
        Para::Heading { level, runs } => Some((*level, runs)),
        _ => None,
    });
    for (rang, (level, runs)) in titres.enumerate() {
        let text = plain(runs);
        if text.trim().is_empty() {
            continue;
        }
        any = true;
        toc.push(Para::Item {
            number: None,
            depth: level.saturating_sub(1).min(5),
            runs: vec![Run::Value {
                name: format!("{SOMMAIRE}{rang}"),
                text: text.split_whitespace().collect::<Vec<_>>().join(" "),
                style: Style::default(),
            }],
        });
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

fn paragraph(mut items: Vec<Item>, base: Style) -> Para {
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
            runs: runs(items, base),
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
                runs: runs(items, base),
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
            runs: runs(items, base),
        };
    }

    // Citation
    if let Some(r) = lead.strip_prefix('>') {
        strip_start(&mut items, if r.starts_with(' ') { 2 } else { 1 });
        return Para::Quote {
            runs: runs(items, base),
        };
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
            let mut runs = runs(items, base);
            trim_runs(&mut runs);
            return Para::Text { align, runs };
        }
    }

    Para::Text {
        align: Align::Left,
        runs: runs(items, base),
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
fn cells(items: Vec<Item>, base: Style) -> Vec<Vec<Run>> {
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
            let mut r = runs(c, base);
            trim_runs(&mut r);
            r
        })
        .collect()
}

// ---------------------------------------------------------------------------------
// Caractères : **gras**, *italique*, __souligné__, ~~barré~~, ==surligné==,
// {couleur:rouge}…{/}, {surlignage:vert}…{/}
// ---------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    Bold,
    Italic,
    Underline,
    Strike,
    Highlight,
}

impl Mark {
    fn text(self) -> &'static str {
        match self {
            Mark::Bold => "**",
            Mark::Italic => "*",
            Mark::Underline => "__",
            Mark::Strike => "~~",
            Mark::Highlight => "==",
        }
    }
}

enum Tok {
    Char(char),
    Mark(Mark),
    /// `{couleur:…}` ou `{surlignage:…}` : (surlignage ?, couleur, écriture).
    Ouvre(bool, Couleur, String),
    /// `{/}`
    Ferme,
    Atom(Item),
}

/// Une ouverture `{couleur:x}`, `{surlignage:x}` ou `{fond:x}` au début de `t` :
/// (surlignage ?, couleur, longueur en caractères).
fn ouverture(t: &[char]) -> Option<(bool, Couleur, usize)> {
    let fin = t.iter().take(40).position(|c| *c == '}')?;
    let inner: String = t[1..fin].iter().collect();
    let (k, v) = inner.split_once(':')?;
    let (surl, c) = match k.trim().to_lowercase().as_str() {
        "couleur" | "color" => (false, Couleur::lire(v)?),
        "surlignage" | "fond" | "highlight" => (true, Couleur::lire_surlignage(v)?),
        _ => return None,
    };
    Some((surl, c, fin + 1))
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
            if let ('\\', Some(e @ ('*' | '_' | '~' | '|' | '#' | '>' | '=' | '{'))) = (c, next) {
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
            } else if c == '=' && next == Some('=') && chars.get(i + 2) != Some(&'=') {
                out.push(Tok::Mark(Mark::Highlight));
                i += 2;
            } else if c == '=' && next == Some('=') {
                // `===` ou plus : du texte.
                while chars.get(i) == Some(&'=') {
                    out.push(Tok::Char('='));
                    i += 1;
                }
            } else if c == '*' {
                out.push(Tok::Mark(Mark::Italic));
                i += 1;
            } else if c == '{' && chars[i..].starts_with(&['{', '/', '}']) {
                out.push(Tok::Ferme);
                i += 3;
            } else if let Some((surl, coul, n)) =
                (c == '{').then(|| ouverture(&chars[i..])).flatten()
            {
                out.push(Tok::Ouvre(surl, coul, chars[i..i + n].iter().collect()));
                i += n;
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

/// Fin d'une adresse de lien commençant à `debut` dans `t` (parenthèses équilibrées).
fn fin_adresse(t: &str, debut: usize) -> Option<usize> {
    let url = &t[debut..];
    if !["https://", "http://", "mailto:", "www.", "ftp://"]
        .iter()
        .any(|p| url.starts_with(p))
    {
        return None;
    }
    let mut prof = 0usize;
    for (i, c) in url.char_indices() {
        match c {
            '(' => prof += 1,
            ')' if prof == 0 => return (i > 0).then_some(debut + i),
            ')' => prof -= 1,
            c if c.is_whitespace() => return None,
            _ => {}
        }
    }
    None
}

/// Repère les liens `[texte](adresse)` dans le texte de la source.
fn liens(items: Vec<Item>) -> Vec<Item> {
    let mut out = Vec::new();
    for item in items {
        let Item::Text(t) = item else {
            out.push(item);
            continue;
        };
        let mut reste = 0;
        let mut i = 0;
        while let Some(off) = t[i..].find('[') {
            let ouv = i + off;
            i = ouv + 1;
            if ouv > 0 && t.as_bytes()[ouv - 1] == b'\\' {
                continue;
            }
            let Some(ferm) = t[ouv + 1..].find(']').map(|f| ouv + 1 + f) else {
                break;
            };
            let label = &t[ouv + 1..ferm];
            if label.trim().is_empty() || label.contains('[') || !t[ferm + 1..].starts_with('(') {
                continue;
            }
            let Some(fin) = fin_adresse(&t, ferm + 2) else {
                continue;
            };
            if ouv > reste {
                out.push(Item::Text(t[reste..ouv].to_string()));
            }
            out.push(Item::Lien {
                url: t[ferm + 2..fin].to_string(),
                label: label.to_string(),
            });
            reste = fin + 1;
            i = reste;
        }
        if reste < t.len() {
            out.push(Item::Text(t[reste..].to_string()));
        }
    }
    out
}

/// Les morceaux mis en forme d'une suite d'éléments ; `base` porte la couleur et le
/// surlignage du paragraphe.
fn runs(items: Vec<Item>, base: Style) -> Vec<Run> {
    let toks = tokens(liens(items));
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
    // Nombre de `{/}` restants après chaque position : une ouverture n'est retenue que
    // si une fermeture l'attend.
    let mut fermetures_apres = vec![0usize; toks.len() + 1];
    for i in (0..toks.len()).rev() {
        fermetures_apres[i] = fermetures_apres[i + 1] + matches!(toks[i], Tok::Ferme) as usize;
    }

    let mut out: Vec<Run> = Vec::new();
    let mut style = base;
    let mut surligne = false;
    // Couleurs ouvertes : (surlignage ?, couleur).
    let mut pile: Vec<(bool, Couleur)> = Vec::new();
    let effectif = |style: Style, surligne: bool, pile: &[(bool, Couleur)]| {
        let mut s = style;
        if let Some((_, c)) = pile.iter().rev().find(|(s, _)| !s) {
            s.couleur = Some(*c);
        }
        if let Some((_, c)) = pile.iter().rev().find(|(s, _)| *s) {
            s.surlignage = Some(*c);
        }
        if surligne {
            s.surlignage = Some(Couleur::SURLIGNAGE);
        }
        s
    };
    let push_text = |out: &mut Vec<Run>, s: &str, style: Style| match out.last_mut() {
        Some(Run::Text { text, style: st }) if *st == style => text.push_str(s),
        _ => out.push(Run::Text {
            text: s.to_string(),
            style,
        }),
    };

    for (i, tok) in toks.iter().enumerate() {
        let eff = effectif(style, surligne, &pile);
        match tok {
            Tok::Char(c) => push_text(&mut out, c.encode_utf8(&mut [0; 4]), eff),
            Tok::Mark(m) => {
                let on = match m {
                    Mark::Bold => &mut style.bold,
                    Mark::Italic => &mut style.italic,
                    Mark::Underline => &mut style.underline,
                    Mark::Strike => &mut style.strike,
                    Mark::Highlight => &mut surligne,
                };
                if *on && can_close(i) {
                    *on = false;
                } else if !*on && can_open(i) && closes_later(i, *m) {
                    *on = true;
                } else {
                    push_text(&mut out, m.text(), eff);
                }
            }
            Tok::Ouvre(surl, c, brut) => {
                if fermetures_apres[i + 1] > pile.len() {
                    pile.push((*surl, *c));
                } else {
                    push_text(&mut out, brut, eff);
                }
            }
            Tok::Ferme => {
                if pile.pop().is_none() {
                    push_text(&mut out, "{/}", eff);
                }
            }
            Tok::Atom(item) => out.push(match item.clone() {
                Item::Value { name, text } => Run::Value {
                    name,
                    text,
                    style: eff,
                },
                Item::Lien { url, label } => Run::Value {
                    name: format!("{LIEN}{url}"),
                    text: label,
                    style: eff,
                },
                Item::Note(n) => Run::Value {
                    name: format!("{NOTE}{n}"),
                    text: n.to_string(),
                    style: Style {
                        exposant: true,
                        ..eff
                    },
                },
                Item::Missing { name, raw } => Run::Missing {
                    name,
                    raw,
                    style: eff,
                },
                Item::Math(t) => Run::Math { tex: t, style: eff },
                Item::Text(t) => Run::Text {
                    text: t,
                    style: eff,
                },
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
        couleur: None,
        surlignage: None,
        exposant: false,
    };
    const I: Style = Style {
        bold: false,
        italic: true,
        underline: false,
        strike: false,
        couleur: None,
        surlignage: None,
        exposant: false,
    };
    const N: Style = Style {
        bold: false,
        italic: false,
        underline: false,
        strike: false,
        couleur: None,
        surlignage: None,
        exposant: false,
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

#[cfg(test)]
mod tests_notes {
    use super::*;

    fn textes(doc: &[Para]) -> Vec<String> {
        doc.iter().map(Para::plain).collect()
    }

    #[test]
    fn note_numerotee_et_rassemblee_en_fin() {
        let doc = document(
            "Un mot\\note{première} puis\\note{seconde}.\nFin",
            &Values::default(),
        );
        let t = textes(&doc);
        assert_eq!(t[0], "Un mot[1] puis[2].");
        assert_eq!(t[1], "Fin");
        assert!(t.contains(&"Notes".to_string()));
        assert_eq!(t[t.len() - 2], "[1] première");
        assert_eq!(t[t.len() - 1], "[2] seconde");
    }

    #[test]
    fn note_avec_champ_et_mise_en_forme() {
        let mut v = Values::default();
        v.set("nom", "Zoé");
        let doc = document("Texte\\note{vu par **{{nom}}**}", &v);
        let t = textes(&doc);
        assert_eq!(t[t.len() - 1], "[1] vu par Zoé");
        let runs = doc[doc.len() - 1].runs();
        assert!(runs.iter().any(|r| r.style().bold && r.plain() == "Zoé"));
    }

    #[test]
    fn note_non_fermee_reste_du_texte() {
        let doc = document("a \\note{jamais fermée {{x}}", &Values::default());
        assert_eq!(doc.len(), 1);
        assert!(doc[0].plain().starts_with("a \\note{jamais fermée"));
    }

    #[test]
    fn sans_note_pas_de_section() {
        let doc = document("rien", &Values::default());
        assert_eq!(doc.len(), 1);
    }

    #[test]
    fn notes_unicode_et_vides() {
        let doc = document("é\\note{}ü\\note{日本}\\note{", &Values::default());
        let t = textes(&doc);
        assert!(t.iter().any(|l| l == "[1] "));
        assert!(t.iter().any(|l| l == "[2] 日本"));
    }
}

#[cfg(test)]
mod tests_liens_qr {
    use super::*;

    #[test]
    fn liens() {
        let d = document(
            "Voir [le site](https://ex.fr/a_(b)?x=1__2) et **[gras](www.ex.fr)** ou [pas un lien](ici) \\[non](https://x.fr).",
            &Values::default(),
        );
        let runs = d[0].runs();
        let liens: Vec<(&str, &str)> = runs.iter().filter_map(Run::lien).collect();
        assert_eq!(
            liens,
            [
                ("https://ex.fr/a_(b)?x=1__2", "le site"),
                ("www.ex.fr", "gras")
            ]
        );
        assert!(runs.iter().any(|r| r.lien().is_some() && r.style().bold));
        assert!(d[0].plain().contains("[pas un lien](ici)"));
        assert_eq!(adresse_lien("www.ex.fr"), "https://www.ex.fr");
        // Un lien n'est pas un champ à remplir.
        assert!(crate::variables("[a](https://b.fr)").is_empty());
    }

    #[test]
    fn codes_qr() {
        let mut v = Values::default();
        v.set("site", "https://varium.fr");
        let d = document(
            "Avant {{qr:Bonjour | le monde}} après\n{{site | qr}}\n{{vide | qr}}",
            &v,
        );
        let qr: Vec<Option<&str>> = d.iter().map(Para::qr).collect();
        assert_eq!(
            qr,
            [
                None,
                Some("Bonjour | le monde"),
                None,
                Some("https://varium.fr"),
                None
            ]
        );
        assert_eq!(d[0].plain(), "Avant");
        assert_eq!(d[2].plain(), "après");
        assert!(matches!(&d[4].runs()[0], Run::Missing { .. }));
        // `{{qr:…}}` n'est pas un champ ; `{{site | qr}}` en est un.
        let noms: Vec<String> = crate::variables("{{qr:x}} {{site | qr}}")
            .into_iter()
            .map(|i| i.name)
            .collect();
        assert_eq!(noms, ["site"]);
        assert!(
            crate::parse::parse("{{qr:}}")
                .iter()
                .all(|s| matches!(s, crate::Segment::Text(_)))
        );
    }
}
