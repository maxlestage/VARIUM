//! Export PDF natif, sans passer par l'impression du navigateur.
//!
//! Le PDF est écrit à la main (aucune dépendance hors `miniz_oxide`) à partir de la
//! mise en page commune ([`crate::mise_en_page`], la même que la vue page) :
//!
//! - polices TrueType **incorporées** en sous-ensemble (Liberation Serif ou Sans, plus
//!   des polices de secours pour les symboles et les émojis), codage Identity-H et
//!   table ToUnicode : tout le texte Unicode s'affiche et se copie ; un caractère
//!   qu'aucune police ne connaît s'affiche comme un rectangle (glyphe `.notdef`) ;
//! - sans fichier de police ([`PdfFonts::approximate`]), repli sur les polices
//!   standard Times/Helvetica en WinAnsi (`?` pour les autres caractères) ;
//! - flux compressés (Flate), formules composées (fractions, indices, racines,
//!   matrices…), signatures en traits vectoriels, codes QR ;
//! - signets (un par titre, imbriqués), liens cliquables (adresses, `[texte](url)`,
//!   entrées de la table des matières vers leur titre) ;
//! - métadonnées : titre, auteur, langue (`/Lang`), métadonnées XMP.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::export::Rendered;
use crate::mise_en_page::{self, Cache, Cible, Element, Options};
use crate::page::{DocFont, PageSetup};
use crate::police::Face;

// ---------------------------------------------------------------------------
// Métriques de police (lecture minimale d'un fichier TrueType)
// ---------------------------------------------------------------------------

/// Largeurs des 256 codes WinAnsi, en millièmes d'em.
#[derive(Debug, Clone, PartialEq)]
pub struct Metrics {
    widths: [f32; 256],
}

fn u16_at(d: &[u8], o: usize) -> Option<usize> {
    let b = d.get(o..o.checked_add(2)?)?;
    Some(u16::from_be_bytes([b[0], b[1]]) as usize)
}

fn u32_at(d: &[u8], o: usize) -> Option<usize> {
    let b = d.get(o..o.checked_add(4)?)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
}

fn table(d: &[u8], tag: &[u8; 4]) -> Option<usize> {
    let n = u16_at(d, 4)?;
    (0..n.min(64)).find_map(|i| {
        let rec = 12 + i * 16;
        (d.get(rec..rec + 4)? == tag).then(|| u32_at(d, rec + 8))?
    })
}

/// Glyphe d'un caractère Unicode dans la table `cmap` (format 4).
fn glyph(d: &[u8], cmap: usize, c: u32) -> Option<usize> {
    let n = u16_at(d, cmap + 2)?;
    let mut sub = None;
    for i in 0..n.min(32) {
        let rec = cmap + 4 + i * 8;
        let (pf, en) = (u16_at(d, rec)?, u16_at(d, rec + 2)?);
        if (pf == 3 && en == 1) || pf == 0 {
            sub = Some(cmap + u32_at(d, rec + 4)?);
            if pf == 3 {
                break;
            }
        }
    }
    let s = sub?;
    if u16_at(d, s)? != 4 || c > 0xFFFF {
        return None;
    }
    let segs = u16_at(d, s + 6)? / 2;
    let (ends, starts) = (s + 14, s + 16 + segs * 2);
    let (deltas, ranges) = (starts + segs * 2, starts + segs * 4);
    for i in 0..segs {
        if c as usize <= u16_at(d, ends + i * 2)? {
            let start = u16_at(d, starts + i * 2)?;
            if (c as usize) < start {
                return None;
            }
            let delta = u16_at(d, deltas + i * 2)?;
            let ro = u16_at(d, ranges + i * 2)?;
            let g = if ro == 0 {
                (c as usize + delta) & 0xFFFF
            } else {
                let at = ranges + i * 2 + ro + (c as usize - start) * 2;
                match u16_at(d, at)? {
                    0 => return None,
                    g => (g + delta) & 0xFFFF,
                }
            };
            return (g != 0).then_some(g);
        }
    }
    None
}

impl Metrics {
    /// Largeur approchée (0,5 em) quand aucune police n'est disponible.
    pub fn approximate() -> Self {
        Metrics {
            widths: [500.0; 256],
        }
    }

    /// Lit les largeurs d'une police TrueType ; `None` si le fichier est illisible.
    pub fn from_ttf(d: &[u8]) -> Option<Self> {
        let upem = u16_at(d, table(d, b"head")? + 18)?.max(1) as f32;
        let nh = u16_at(d, table(d, b"hhea")? + 34)?.max(1);
        let hmtx = table(d, b"hmtx")?;
        let cmap = table(d, b"cmap")?;
        let mut widths = [0.0f32; 256];
        for (code, w) in widths.iter_mut().enumerate().skip(32) {
            let adv = glyph(d, cmap, winansi_char(code as u8) as u32)
                .and_then(|g| u16_at(d, hmtx + g.min(nh - 1) * 4))
                .unwrap_or(upem as usize / 2);
            *w = adv as f32 * 1000.0 / upem;
        }
        Some(Metrics { widths })
    }

    /// Largeur d'un caractère (millièmes d'em) ; celle de `?` s'il n'existe pas en WinAnsi.
    pub fn width_of(&self, c: char) -> f32 {
        self.width(winansi(c))
    }

    fn width(&self, byte: u8) -> f32 {
        self.widths[byte as usize]
    }
}

/// Un glyphe choisi : police (voir [`PdfFonts::glyphe`]), numéro et avance en em.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glyphe {
    pub police: u8,
    pub gid: u16,
    pub avance: f32,
}

/// Première police de secours (les numéros 0 à 3 sont les quatre styles).
pub const SECOURS: u8 = 4;
/// Polices standard en WinAnsi (sans fichier de police) : 100 à 103.
pub const WINANSI: u8 = 100;

/// Les polices du document : les quatre variantes d'une famille (largeurs et, si
/// les fichiers sont fournis, polices à incorporer), et des polices de secours.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfFonts {
    pub regular: Metrics,
    pub bold: Metrics,
    pub italic: Metrics,
    pub bold_italic: Metrics,
    /// Polices TrueType des quatre styles (régulier, gras, italique, gras italique).
    pub faces: [Option<Arc<Face>>; 4],
    /// Polices de secours (symboles, émojis), dans l'ordre de recherche.
    pub secours: Vec<Arc<Face>>,
    /// Noms PostScript des quatre styles.
    pub noms: [String; 4],
}

impl PdfFonts {
    /// Polices à partir des fichiers .ttf ; une police illisible est remplacée
    /// par des largeurs approchées (le PDF reste valide).
    pub fn from_ttf(regular: &[u8], bold: &[u8], italic: &[u8], bold_italic: &[u8]) -> Self {
        let m = |d: &[u8]| Metrics::from_ttf(d).unwrap_or_else(Metrics::approximate);
        let f = |d: &[u8]| Face::lire(d).map(Arc::new);
        let faces = [f(regular), f(bold), f(italic), f(bold_italic)];
        let noms = std::array::from_fn(|i| {
            faces[i]
                .as_ref()
                .and_then(|f| f.nom.clone())
                .unwrap_or_else(|| {
                    [
                        "Times-Roman",
                        "Times-Bold",
                        "Times-Italic",
                        "Times-BoldItalic",
                    ][i]
                        .to_string()
                })
        });
        PdfFonts {
            regular: m(regular),
            bold: m(bold),
            italic: m(italic),
            bold_italic: m(bold_italic),
            faces,
            secours: Vec::new(),
            noms,
        }
    }

    /// Les mêmes polices, avec des polices de secours (fichiers TrueType ; les
    /// fichiers illisibles sont ignorés).
    pub fn avec_secours(mut self, polices: &[&[u8]]) -> Self {
        self.secours = polices
            .iter()
            .filter_map(|d| Face::lire(d))
            .map(Arc::new)
            .take(60)
            .collect();
        self
    }

    /// Les mêmes polices, avec des polices de secours déjà lues.
    pub fn avec_secours_lues(mut self, polices: Vec<Arc<Face>>) -> Self {
        self.secours = polices.into_iter().take(60).collect();
        self
    }

    /// Largeurs approchées, sans fichier de police.
    pub fn approximate() -> Self {
        let m = Metrics::approximate();
        PdfFonts {
            regular: m.clone(),
            bold: m.clone(),
            italic: m.clone(),
            bold_italic: m,
            faces: [None, None, None, None],
            secours: Vec::new(),
            noms: std::array::from_fn(|i| {
                [
                    "Times-Roman",
                    "Times-Bold",
                    "Times-Italic",
                    "Times-BoldItalic",
                ][i]
                    .to_string()
            }),
        }
    }

    fn metrics(&self, i: usize) -> &Metrics {
        match i {
            1 => &self.bold,
            2 => &self.italic,
            3 => &self.bold_italic,
            _ => &self.regular,
        }
    }

    /// Une police par son numéro (0–3 : styles, 4 et plus : secours).
    pub fn face(&self, police: u8) -> Option<&Arc<Face>> {
        match police {
            0..=3 => self.faces[police as usize].as_ref(),
            p if (SECOURS..WINANSI).contains(&p) => self.secours.get((p - SECOURS) as usize),
            _ => None,
        }
    }

    /// Le glyphe d'un caractère pour le style `style` (0 régulier, 1 gras, 2 italique,
    /// 3 gras italique) : dans la police du style, sinon la régulière, sinon les
    /// polices de secours, sinon le glyphe `.notdef` (un rectangle).
    pub fn glyphe(&self, c: char, style: usize) -> Glyphe {
        let style = style.min(3);
        let Some(face) = &self.faces[style] else {
            // Polices standard : WinAnsi.
            let b = winansi(c);
            return Glyphe {
                police: WINANSI + style as u8,
                gid: b as u16,
                avance: self.metrics(style).width(b) / 1000.0,
            };
        };
        if let Some(g) = face.glyphe(c) {
            return Glyphe {
                police: style as u8,
                gid: g,
                avance: face.avance(g),
            };
        }
        if let Some(r) = &self.faces[0]
            && let Some(g) = r.glyphe(c)
        {
            return Glyphe {
                police: 0,
                gid: g,
                avance: r.avance(g),
            };
        }
        for (i, s) in self.secours.iter().enumerate() {
            if let Some(g) = s.glyphe(c) {
                return Glyphe {
                    police: SECOURS + i as u8,
                    gid: g,
                    avance: s.avance(g),
                };
            }
        }
        // Caractère inconnu de toutes les polices : un signe de remplacement visible.
        if c != '\u{FFFD}' && c != '\u{25A1}' {
            for r in ['\u{FFFD}', '\u{25A1}'] {
                let g = self.glyphe(r, style);
                if g.gid != 0 {
                    return g;
                }
            }
        }
        Glyphe {
            police: style as u8,
            gid: 0,
            avance: face.avance(0).max(0.3),
        }
    }

    /// Avance d'un glyphe déjà choisi, en em.
    pub fn avance_glyphe(&self, police: u8, gid: u16, style: usize) -> Option<f32> {
        if police >= WINANSI {
            return Some(self.metrics(style).width(gid.min(255) as u8) / 1000.0);
        }
        self.face(police).map(|f| f.avance(gid))
    }

    /// La police a-t-elle une vraie italique (sinon l'italique est simulé) ?
    pub fn vraie_italique(&self, police: u8) -> bool {
        match police {
            2 | 3 => self.faces[police as usize]
                .as_ref()
                .is_some_and(|f| f.italic_angle.abs() > 0.1),
            p if p >= WINANSI => true,
            _ => false,
        }
    }

    /// Hauteur, profondeur et interligne supplémentaire de la police régulière (em).
    pub fn metriques(&self) -> (f32, f32, f32) {
        match &self.faces[0] {
            Some(f) if f.ascent > 0.0 && f.ascent + f.descent > 0.5 => {
                (f.ascent, f.descent.max(0.0), f.line_gap)
            }
            _ => (0.891, 0.216, 0.042),
        }
    }

    /// Identifiant des polices (pour le cache de mise en page).
    pub fn id(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for f in self.faces.iter().chain(self.secours.iter().map(|_| &None)) {
            match f {
                Some(f) => (Arc::as_ptr(f) as usize).hash(&mut h),
                None => 0usize.hash(&mut h),
            }
        }
        for s in &self.secours {
            (Arc::as_ptr(s) as usize).hash(&mut h);
        }
        self.regular
            .widths
            .iter()
            .take(128)
            .map(|w| w.to_bits())
            .collect::<Vec<_>>()
            .hash(&mut h);
        h.finish()
    }
}

// ---------------------------------------------------------------------------
// Codage WinAnsi
// ---------------------------------------------------------------------------

const WIN_80_9F: [char; 32] = [
    '€', '?', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '?', 'Ž', '?', '?', '‘', '’',
    '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '?', 'ž', 'Ÿ',
];

fn winansi_char(b: u8) -> char {
    match b {
        0x80..=0x9F => WIN_80_9F[(b - 0x80) as usize],
        _ => b as char,
    }
}

/// Code WinAnsi d'un caractère ; `?` s'il n'existe pas.
fn winansi(c: char) -> u8 {
    match c {
        '\u{20}'..='\u{7E}' | '\u{A0}'..='\u{FF}' => c as u32 as u8,
        '\u{202F}' | '\u{2009}' | '\u{2002}' | '\u{2003}' | '\u{200A}' | '\u{2007}' => 0x20,
        '\u{2212}' | '\u{2010}' | '\u{2011}' => b'-',
        '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' => 0,
        _ => WIN_80_9F
            .iter()
            .position(|&w| w == c && c != '?')
            .map(|i| 0x80 + i as u8)
            .unwrap_or(b'?'),
    }
}

// ---------------------------------------------------------------------------
// Écriture du fichier
// ---------------------------------------------------------------------------

fn n(v: f32) -> String {
    let s = format!("{:.2}", if v.is_finite() { v } else { 0.0 });
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

fn utf16_hex(s: &str) -> String {
    let mut out = String::from("FEFF");
    for u in s.encode_utf16() {
        out.push_str(&format!("{u:04X}"));
    }
    out
}

fn rvb(c: crate::doc::Couleur) -> String {
    let [r, g, b] = c.unites();
    format!("{} {} {}", n(r), n(g), n(b))
}

/// Si le mot est une adresse (`http://`, `https://`, `mailto:`), renvoie la cible du lien,
/// sans la ponctuation finale (`.,;:!?)`).
fn adresse_de(t: &str) -> Option<String> {
    let bas = t.to_ascii_lowercase();
    let reste = ["http://", "https://", "mailto:"]
        .iter()
        .find_map(|p| bas.strip_prefix(p).map(|r| (p.len(), r.len())))?;
    if reste.1 == 0 {
        return None;
    }
    let t = t.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '»', '"']);
    if t.len() <= reste.0 || !t.is_ascii() {
        return None;
    }
    Some(t.to_string())
}

/// Date PDF (`D:AAAAMMJJHHmmSSZ`) et ISO 8601 de l'instant présent (UTC).
fn dates() -> (String, String) {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (jours, sec) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    // Date civile depuis le nombre de jours (algorithme de H. Hinnant).
    let z = jours + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    let (h, mi, se) = (sec / 3600, sec % 3600 / 60, sec % 60);
    (
        format!("D:{y:04}{m:02}{d:02}{h:02}{mi:02}{se:02}Z"),
        format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{se:02}Z"),
    )
}

/// Flux compressé.
fn flux(dico: &str, data: &[u8]) -> Vec<u8> {
    let c = miniz_oxide::deflate::compress_to_vec_zlib(data, 6);
    let mut o = format!(
        "<< {dico} /Filter /FlateDecode /Length {} >>\nstream\n",
        c.len()
    )
    .into_bytes();
    o.extend_from_slice(&c);
    o.extend_from_slice(b"\nendstream");
    o
}

/// Table ToUnicode d'une police (glyphe → texte).
fn to_unicode(glyphes: &BTreeMap<u16, String>) -> String {
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let v: Vec<(&u16, &String)> = glyphes.iter().collect();
    for bloc in v.chunks(100) {
        s.push_str(&format!("{} beginbfchar\n", bloc.len()));
        for (g, t) in bloc {
            let mut u = String::new();
            for x in t.encode_utf16() {
                u.push_str(&format!("{x:04X}"));
            }
            if u.is_empty() {
                u.push_str("FFFD");
            }
            s.push_str(&format!("<{g:04X}> <{u}>\n"));
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    s
}

/// Zone cliquable : x, y, largeur, hauteur, cible.
type Zone = (f32, f32, f32, f32, Cible);

struct Objets {
    o: Vec<Vec<u8>>,
}

impl Objets {
    fn reserver(&mut self) -> usize {
        self.o.push(Vec::new());
        self.o.len()
    }
    fn mettre(&mut self, id: usize, v: Vec<u8>) {
        if let Some(x) = self.o.get_mut(id - 1) {
            *x = v;
        }
    }
    fn ajouter(&mut self, v: Vec<u8>) -> usize {
        self.o.push(v);
        self.o.len()
    }
}

/// Le document PDF.
pub fn pdf(title: &str, docs: &[Rendered], page: &PageSetup, fonts: &PdfFonts) -> Vec<u8> {
    let mise = mise_en_page::mettre_en_page(docs, page, &Options::new(fonts), &mut Cache::new());
    ecrire(title, &mise, page, fonts)
}

/// Écrit le PDF d'un document déjà mis en page.
pub fn ecrire(
    title: &str,
    mise: &mise_en_page::Mise,
    page: &PageSetup,
    fonts: &PdfFonts,
) -> Vec<u8> {
    let h = mise.hauteur;
    let np = mise.pages.len().max(1);
    // Glyphes employés par police : numéro → texte (pour ToUnicode).
    let mut usage: BTreeMap<u8, BTreeMap<u16, String>> = BTreeMap::new();
    let mut contenus: Vec<String> = Vec::with_capacity(np);
    let mut liens: Vec<Vec<Zone>> = Vec::with_capacity(np);
    for p in &mise.pages {
        let mut ops = String::new();
        let mut zones = Vec::new();
        p.pour_chaque(|e| match e {
            Element::Texte(t) => {
                if t.glyphes.is_empty() {
                    return;
                }
                let u = usage.entry(t.police).or_default();
                let chars: Vec<char> = t.texte.chars().collect();
                let mut code = String::new();
                for (i, g) in t.glyphes.iter().enumerate() {
                    if t.police >= WINANSI {
                        code.push_str(&format!("{:02X}", (*g).min(255)));
                    } else {
                        code.push_str(&format!("{g:04X}"));
                        let c = if chars.len() == t.glyphes.len() {
                            chars[i].to_string()
                        } else if i == 0 {
                            t.texte.clone()
                        } else {
                            String::new()
                        };
                        u.entry(*g)
                            .or_insert(if *g == 0 { "\u{FFFD}".into() } else { c });
                    }
                }
                let skew = if t.oblique { "0.21" } else { "0" };
                ops.push_str(&format!(
                    "{} rg BT /F{} {} Tf 1 0 {skew} 1 {} {} Tm <{code}> Tj ET\n",
                    rvb(t.couleur),
                    t.police,
                    n(t.taille),
                    n(t.x),
                    n(h - t.y)
                ));
                if t.genre == mise_en_page::Genre::Normal
                    && let Some(url) = adresse_de(&t.texte)
                {
                    zones.push((
                        t.x,
                        t.y - t.taille * 0.9,
                        t.largeur,
                        t.taille * 1.15,
                        Cible::Url(url),
                    ));
                }
            }
            Element::Rect {
                x,
                y,
                l,
                h: rh,
                couleur,
            } => ops.push_str(&format!(
                "{} rg {} {} {} {} re f\n",
                rvb(*couleur),
                n(*x),
                n(h - y - rh),
                n(*l),
                n(*rh)
            )),
            Element::Ligne {
                x1,
                y1,
                x2,
                y2,
                ep,
                couleur,
            } => ops.push_str(&format!(
                "{} w {} RG {} {} m {} {} l S\n",
                n(*ep),
                rvb(*couleur),
                n(*x1),
                n(h - y1),
                n(*x2),
                n(h - y2)
            )),
            Element::Trait {
                points,
                ep,
                couleur,
            } => {
                if let Some((x0, y0)) = points.first() {
                    ops.push_str(&format!(
                        "1 J 1 j {} w {} RG {} {} m",
                        n(*ep),
                        rvb(*couleur),
                        n(*x0),
                        n(h - y0)
                    ));
                    for (x, y) in &points[1..] {
                        ops.push_str(&format!(" {} {} l", n(*x), n(h - y)));
                    }
                    ops.push_str(" S 0 J 0 j\n");
                }
            }
            Element::SansSignature { x, y, l, h: sh } => ops.push_str(&format!(
                "0.6 w 0.55 0.55 0.55 RG {} {} m {} {} l S\n",
                n(*x),
                n(h - y - sh),
                n(x + l),
                n(h - y - sh)
            )),
            Element::Lien {
                x,
                y,
                l,
                h: lh,
                cible,
            } => {
                zones.push((*x, *y, *l, *lh, cible.clone()));
            }
        });
        contenus.push(ops);
        liens.push(zones);
    }
    if contenus.is_empty() {
        contenus.push(String::new());
        liens.push(Vec::new());
    }

    let mut ob = Objets { o: Vec::new() };
    let catalogue = ob.reserver();
    let pages_id = ob.reserver();
    let info = ob.reserver();
    // Polices.
    let mut ressources = String::new();
    for (&police, glyphes) in &usage {
        let id = if police >= WINANSI {
            let base = match page.font {
                DocFont::Serif => [
                    "Times-Roman",
                    "Times-Bold",
                    "Times-Italic",
                    "Times-BoldItalic",
                ],
                DocFont::Sans => [
                    "Helvetica",
                    "Helvetica-Bold",
                    "Helvetica-Oblique",
                    "Helvetica-BoldOblique",
                ],
            }[((police - WINANSI) as usize).min(3)];
            ob.ajouter(
                format!(
                    "<< /Type /Font /Subtype /Type1 /BaseFont /{base} /Encoding /WinAnsiEncoding >>"
                )
                .into_bytes(),
            )
        } else {
            let Some(face) = fonts.face(police) else {
                continue;
            };
            let gids: BTreeSet<u16> = glyphes.keys().copied().collect();
            let sous = face.sous_ensemble(&gids);
            let nom_base = if police < SECOURS {
                fonts.noms[police as usize].clone()
            } else {
                face.nom
                    .clone()
                    .unwrap_or_else(|| format!("Secours{police}"))
            };
            // Préfixe de sous-ensemble : six capitales dérivées des glyphes.
            let mut hh: u64 = 0xcbf2_9ce4_8422_2325 ^ police as u64;
            for g in &gids {
                hh = (hh ^ *g as u64).wrapping_mul(0x100_0000_01b3);
            }
            let prefixe: String = (0..6)
                .map(|i| (b'A' + ((hh >> (i * 5)) % 26) as u8) as char)
                .collect();
            let nom = format!("{prefixe}+{nom_base}");
            let fichier = {
                let c = miniz_oxide::deflate::compress_to_vec_zlib(&sous, 6);
                let mut o = format!(
                    "<< /Length1 {} /Filter /FlateDecode /Length {} >>\nstream\n",
                    sous.len(),
                    c.len()
                )
                .into_bytes();
                o.extend_from_slice(&c);
                o.extend_from_slice(b"\nendstream");
                ob.ajouter(o)
            };
            let flags = if police < SECOURS { 32 } else { 4 }
                | if face.italic_angle.abs() > 0.1 { 64 } else { 0 };
            let desc = ob.ajouter(
                format!(
                    "<< /Type /FontDescriptor /FontName /{nom} /Flags {flags} /FontBBox [{} {} {} {}] /ItalicAngle {} /Ascent {} /Descent {} /CapHeight {} /StemV 80 /FontFile2 {fichier} 0 R >>",
                    n(face.bbox[0]),
                    n(face.bbox[1]),
                    n(face.bbox[2]),
                    n(face.bbox[3]),
                    n(face.italic_angle),
                    n(face.ascent * 1000.0),
                    n(-face.descent * 1000.0),
                    n(face.cap_height),
                )
                .into_bytes(),
            );
            let w: String = gids
                .iter()
                .map(|g| format!("{g} [{}] ", n(face.avance_mille(*g))))
                .collect();
            let cid = ob.ajouter(
                format!(
                    "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{nom} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {desc} 0 R /DW 1000 /W [{w}] /CIDToGIDMap /Identity >>"
                )
                .into_bytes(),
            );
            let tu = ob.ajouter(flux("", to_unicode(glyphes).as_bytes()));
            ob.ajouter(
                format!(
                    "<< /Type /Font /Subtype /Type0 /BaseFont /{nom} /Encoding /Identity-H /DescendantFonts [{cid} 0 R] /ToUnicode {tu} 0 R >>"
                )
                .into_bytes(),
            )
        };
        ressources.push_str(&format!("/F{police} {id} 0 R "));
    }
    // Pages et contenus.
    let ids_pages: Vec<usize> = (0..contenus.len()).map(|_| ob.reserver()).collect();
    let mut annots_pages = Vec::new();
    for (i, zones) in liens.iter().enumerate() {
        let mut ids = String::new();
        for (x, y, l, lh, cible) in zones {
            let rect = format!("[{} {} {} {}]", n(*x), n(h - y - lh), n(x + l), n(h - y));
            let action = match cible {
                Cible::Url(u) => format!("/A << /S /URI /URI <{}> >>", hex(u.as_bytes())),
                Cible::Titre(k) => match mise.titres.get(*k) {
                    Some(t) => format!(
                        "/Dest [{} 0 R /XYZ {} {} null]",
                        ids_pages.get(t.page).copied().unwrap_or(ids_pages[0]),
                        n(mise.zone.0),
                        n((h - t.y + 4.0).min(h))
                    ),
                    None => continue,
                },
            };
            let id = ob.ajouter(
                format!("<< /Type /Annot /Subtype /Link /Rect {rect} /Border [0 0 0] {action} >>")
                    .into_bytes(),
            );
            ids.push_str(&format!("{id} 0 R "));
        }
        let _ = i;
        annots_pages.push(ids);
    }
    for (i, c) in contenus.iter().enumerate() {
        let contenu = ob.ajouter(flux("", c.as_bytes()));
        let annots = if annots_pages[i].is_empty() {
            String::new()
        } else {
            format!(" /Annots [{}]", annots_pages[i].trim_end())
        };
        ob.mettre(
            ids_pages[i],
            format!(
                "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 {} {}] /Resources << /Font << {ressources}>> >> /Contents {contenu} 0 R{annots} >>",
                n(mise.largeur),
                n(h)
            )
            .into_bytes(),
        );
    }
    let kids: String = ids_pages.iter().map(|i| format!("{i} 0 R ")).collect();
    ob.mettre(
        pages_id,
        format!(
            "<< /Type /Pages /Kids [{kids}] /Count {} >>",
            ids_pages.len()
        )
        .into_bytes(),
    );
    // Signets : un titre est rattaché au titre de niveau inférieur qui le précède.
    let marks: Vec<&mise_en_page::TitrePlace> = mise
        .titres
        .iter()
        .filter(|t| t.page < ids_pages.len() && !t.texte.is_empty())
        .collect();
    let nm = marks.len();
    let mut racine = None;
    if nm > 0 {
        let mut parent: Vec<Option<usize>> = vec![None; nm];
        let mut pile: Vec<usize> = Vec::new();
        for i in 0..nm {
            while pile
                .last()
                .is_some_and(|&j| marks[j].niveau >= marks[i].niveau)
            {
                pile.pop();
            }
            parent[i] = pile.last().copied();
            pile.push(i);
        }
        let enfants =
            |p: Option<usize>| -> Vec<usize> { (0..nm).filter(|&i| parent[i] == p).collect() };
        let r = ob.reserver();
        let ids: Vec<usize> = (0..nm).map(|_| ob.reserver()).collect();
        let top = enfants(None);
        ob.mettre(
            r,
            format!(
                "<< /Type /Outlines /First {} 0 R /Last {} 0 R /Count {} >>",
                ids[top[0]],
                ids[top[top.len() - 1]],
                nm
            )
            .into_bytes(),
        );
        for (i, m) in marks.iter().enumerate() {
            let freres = enfants(parent[i]);
            let pos = freres.iter().position(|&x| x == i).unwrap_or(0);
            let fils = enfants(Some(i));
            let mut d = format!(
                "<< /Title <{}> /Parent {} 0 R /Dest [{} 0 R /XYZ {} {} null]",
                utf16_hex(&m.texte),
                parent[i].map_or(r, |p| ids[p]),
                ids_pages[m.page],
                n(mise.zone.0),
                n((h - m.y + 4.0).min(h))
            );
            if pos > 0 {
                d.push_str(&format!(" /Prev {} 0 R", ids[freres[pos - 1]]));
            }
            if pos + 1 < freres.len() {
                d.push_str(&format!(" /Next {} 0 R", ids[freres[pos + 1]]));
            }
            if let (Some(a), Some(b)) = (fils.first(), fils.last()) {
                d.push_str(&format!(
                    " /First {} 0 R /Last {} 0 R /Count {}",
                    ids[*a],
                    ids[*b],
                    fils.len()
                ));
            }
            d.push_str(" >>");
            ob.mettre(ids[i], d.into_bytes());
        }
        racine = Some(r);
    }
    // Métadonnées : dictionnaire Info et XMP.
    let (date_pdf, date_iso) = dates();
    let auteur = page.auteur.trim();
    let lang = page.langue.tag();
    let x = crate::export::xml_escape;
    let xmp = format!(
        r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:pdf="http://ns.adobe.com/pdf/1.3/" xmlns:xmp="http://ns.adobe.com/xap/1.0/"><dc:format>application/pdf</dc:format><dc:title><rdf:Alt><rdf:li xml:lang="x-default">{}</rdf:li></rdf:Alt></dc:title><dc:creator><rdf:Seq><rdf:li>{}</rdf:li></rdf:Seq></dc:creator><dc:language><rdf:Bag><rdf:li>{lang}</rdf:li></rdf:Bag></dc:language><pdf:Producer>Varium</pdf:Producer><xmp:CreatorTool>Varium</xmp:CreatorTool><xmp:CreateDate>{date_iso}</xmp:CreateDate><xmp:ModifyDate>{date_iso}</xmp:ModifyDate></rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#,
        x(title),
        x(auteur)
    );
    let meta = {
        let mut o = format!(
            "<< /Type /Metadata /Subtype /XML /Length {} >>\nstream\n",
            xmp.len()
        )
        .into_bytes();
        o.extend_from_slice(xmp.as_bytes());
        o.extend_from_slice(b"\nendstream");
        ob.ajouter(o)
    };
    let auteur_info = if auteur.is_empty() {
        String::new()
    } else {
        format!(" /Author <{}>", utf16_hex(auteur))
    };
    ob.mettre(
        info,
        format!(
            "<< /Title <{}>{auteur_info} /Producer (Varium) /Creator (Varium) /CreationDate ({date_pdf}) /ModDate ({date_pdf}) >>",
            utf16_hex(title)
        )
        .into_bytes(),
    );
    let signets = racine.map_or(String::new(), |r| {
        format!(" /Outlines {r} 0 R /PageMode /UseOutlines")
    });
    ob.mettre(
        catalogue,
        format!(
            "<< /Type /Catalog /Pages {pages_id} 0 R{signets} /Lang ({lang}) /Metadata {meta} 0 R /ViewerPreferences << /DisplayDocTitle true >> >>"
        )
        .into_bytes(),
    );

    let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(ob.o.len());
    for (i, o) in ob.o.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", ob.o.len() + 1).as_bytes());
    for o in &offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root {catalogue} 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            ob.o.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// Exporte directement une source avec ses valeurs (une page par document).
pub fn to_pdf(title: &str, src: &str, values: &crate::render::Values, fonts: &PdfFonts) -> Vec<u8> {
    let page = PageSetup::default();
    let r = crate::export::prepare(src, values, &page);
    pdf(title, &[r], &page, fonts)
}
