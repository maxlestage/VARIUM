//! Export PDF natif, sans passer par l'impression du navigateur.
//!
//! Le PDF est écrit à la main (aucune dépendance) : texte en police Times ou
//! Helvetica (polices standard de tout lecteur PDF, donc non incorporées), codage
//! WinAnsi (français, latin-1, « » ’ … € œ). Les largeurs de caractères viennent
//! des polices Liberation, dont les métriques sont celles de Times et d'Arial :
//! la mise en page est donc exacte. Un caractère hors WinAnsi (formule exotique,
//! émoji) est remplacé par `?`.
//!
//! Gère : césure des lignes, alignements (justifié compris), titres, listes,
//! citations, tableaux (lignes coupées d'une page à l'autre), filets, sauts de
//! page, gras/italique/souligné/barré, en-tête, pied de page et « Page X sur N »
//! (numérotés document par document en publipostage), plus un plan de signets
//! (un signet par titre, imbriqués selon leur niveau).

use crate::doc::{Align, Para, Run, Style, Table};
use crate::export::Rendered;
use crate::page::{DocFont, PageSetup};

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

/// Les quatre variantes d'une famille.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfFonts {
    pub regular: Metrics,
    pub bold: Metrics,
    pub italic: Metrics,
    pub bold_italic: Metrics,
}

impl PdfFonts {
    /// Métriques à partir des fichiers .ttf ; une police illisible est remplacée
    /// par des largeurs approchées (le PDF reste valide).
    pub fn from_ttf(regular: &[u8], bold: &[u8], italic: &[u8], bold_italic: &[u8]) -> Self {
        let m = |d: &[u8]| Metrics::from_ttf(d).unwrap_or_else(Metrics::approximate);
        PdfFonts {
            regular: m(regular),
            bold: m(bold),
            italic: m(italic),
            bold_italic: m(bold_italic),
        }
    }

    /// Largeurs approchées, sans fichier de police.
    pub fn approximate() -> Self {
        let m = Metrics::approximate();
        PdfFonts {
            regular: m.clone(),
            bold: m.clone(),
            italic: m.clone(),
            bold_italic: m,
        }
    }

    fn get(&self, st: Style) -> &Metrics {
        match (st.bold, st.italic) {
            (false, false) => &self.regular,
            (true, false) => &self.bold,
            (false, true) => &self.italic,
            (true, true) => &self.bold_italic,
        }
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
// Découpage en lignes
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Atom {
    bytes: Vec<u8>,
    style: Style,
    missing: bool,
    /// Adresse du lien `[texte](adresse)` dont l'atome fait partie.
    lien: Option<std::sync::Arc<str>>,
    width: f32,
    space: bool,
    /// Saut de ligne forcé (`\n`).
    brk: bool,
}

struct Line {
    atoms: Vec<Atom>,
    width: f32,
    /// Dernière ligne du paragraphe ou avant un saut forcé : jamais justifiée.
    last: bool,
}

struct Ctx<'a> {
    fonts: &'a PdfFonts,
    size: f32,
}

impl Ctx<'_> {
    fn bw(&self, st: Style, size: f32, b: u8) -> f32 {
        self.fonts.get(st).width(b) * size / 1000.0
    }

    /// Les atomes (mots, espaces, sauts) d'une suite de morceaux.
    fn atoms(&self, runs: &[Run], size: f32, force: Style) -> Vec<Atom> {
        let mut out: Vec<Atom> = Vec::new();
        for r in runs {
            let mut st = r.style();
            st.bold |= force.bold;
            st.italic |= force.italic || matches!(r, Run::Math { .. });
            let missing = matches!(r, Run::Missing { .. });
            let lien: Option<std::sync::Arc<str>> =
                r.lien().map(|(u, _)| crate::doc::adresse_lien(u).into());
            let mut word: Vec<u8> = Vec::new();
            let flush = |word: &mut Vec<u8>, out: &mut Vec<Atom>| {
                if word.is_empty() {
                    return;
                }
                let bytes = std::mem::take(word);
                let width = bytes.iter().map(|&b| self.bw(st, size, b)).sum();
                out.push(Atom {
                    bytes,
                    style: st,
                    missing,
                    lien: lien.clone(),
                    width,
                    space: false,
                    brk: false,
                });
            };
            for c in r.plain().chars() {
                match c {
                    '\n' | '\r' => {
                        flush(&mut word, &mut out);
                        if c == '\n' {
                            out.push(Atom {
                                bytes: vec![],
                                style: st,
                                missing,
                                lien: lien.clone(),
                                width: 0.0,
                                space: false,
                                brk: true,
                            });
                        }
                    }
                    ' ' | '\t' => {
                        flush(&mut word, &mut out);
                        let n = if c == '\t' { 4 } else { 1 };
                        for _ in 0..n {
                            out.push(Atom {
                                bytes: vec![0x20],
                                style: st,
                                missing,
                                lien: lien.clone(),
                                width: self.bw(st, size, 0x20),
                                space: true,
                                brk: false,
                            });
                        }
                    }
                    c => match winansi(c) {
                        0 => {}
                        b => word.push(b),
                    },
                }
            }
            flush(&mut word, &mut out);
        }
        out
    }

    /// Répartit les atomes en lignes d'au plus `max` points.
    fn wrap(&self, atoms: Vec<Atom>, max: f32) -> Vec<Line> {
        let max = max.max(10.0);
        let mut lines: Vec<Line> = Vec::new();
        let mut cur: Vec<Atom> = Vec::new();
        let mut w = 0.0f32;
        let finish = |cur: &mut Vec<Atom>, w: &mut f32, lines: &mut Vec<Line>, last: bool| {
            while cur.last().is_some_and(|a| a.space) {
                if let Some(a) = cur.pop() {
                    *w -= a.width;
                }
            }
            lines.push(Line {
                atoms: std::mem::take(cur),
                width: w.max(0.0),
                last,
            });
            *w = 0.0;
        };
        for a in atoms {
            if a.brk {
                finish(&mut cur, &mut w, &mut lines, true);
                continue;
            }
            if a.space {
                // Pas d'espace en tête d'une ligne issue d'un retour automatique.
                if cur.is_empty() && lines.last().is_some_and(|l| !l.last) {
                    continue;
                }
                w += a.width;
                cur.push(a);
                continue;
            }
            // Un mot plus large que la ligne est coupé caractère par caractère.
            let pieces = if a.width > max {
                self.split_word(&a, max)
            } else {
                vec![a]
            };
            for p in pieces {
                if w + p.width > max + 0.01 && cur.iter().any(|x| !x.space) {
                    finish(&mut cur, &mut w, &mut lines, false);
                }
                w += p.width;
                cur.push(p);
            }
        }
        finish(&mut cur, &mut w, &mut lines, true);
        lines
    }

    fn split_word(&self, a: &Atom, max: f32) -> Vec<Atom> {
        let mut out = Vec::new();
        let mut bytes = Vec::new();
        let mut width = 0.0;
        let size = self.size;
        for &b in &a.bytes {
            let bw = self.bw(a.style, size, b);
            if width + bw > max && !bytes.is_empty() {
                out.push(Atom {
                    bytes: std::mem::take(&mut bytes),
                    width,
                    ..a.clone()
                });
                width = 0.0;
            }
            bytes.push(b);
            width += bw;
        }
        out.push(Atom {
            bytes,
            width,
            ..a.clone()
        });
        out
    }
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

/// Taille de police par atome : on la mémorise dans la ligne à l'écriture.
struct PageBuf {
    ops: String,
    /// Liens cliquables : (x, y du bas, largeur, hauteur, adresse).
    links: Vec<(f32, f32, f32, f32, String)>,
    used: f32,
    empty: bool,
    /// Zones cliquables des liens : (x0, y0, x1, y1, adresse).
    liens: Vec<(f32, f32, f32, f32, std::sync::Arc<str>)>,
}

struct Layout<'a> {
    ctx: Ctx<'a>,
    page: &'a PageSetup,
    pages: Vec<PageBuf>,
    /// Pages de départ de chaque document.
    starts: Vec<usize>,
    /// Titres rencontrés : (niveau, texte, page, distance depuis le haut de la zone de texte).
    marks: Vec<(u8, String, usize, f32)>,
    pw: f32,
    ph: f32,
    ml: f32,
    mt: f32,
    tw: f32,
    th: f32,
    lh: f32,
}

fn mm(v: f32) -> f32 {
    v * 72.0 / 25.4
}

fn n(v: f32) -> String {
    let s = format!("{:.2}", if v.is_finite() { v } else { 0.0 });
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

fn font_name(st: Style) -> &'static str {
    match (st.bold, st.italic) {
        (false, false) => "F1",
        (true, false) => "F2",
        (false, true) => "F3",
        (true, true) => "F4",
    }
}

impl Layout<'_> {
    fn new_page(&mut self) {
        self.pages.push(PageBuf {
            ops: String::new(),
            links: Vec::new(),
            used: 0.0,
            empty: true,
            liens: Vec::new(),
        });
    }

    fn cur(&mut self) -> &mut PageBuf {
        if self.pages.is_empty() {
            self.new_page();
        }
        let i = self.pages.len() - 1;
        &mut self.pages[i]
    }

    fn room(&mut self) -> f32 {
        let th = self.th;
        th - self.cur().used
    }

    /// Passe à la page suivante si la page courante contient quelque chose.
    fn break_page(&mut self) {
        if !self.cur().empty {
            self.new_page();
        }
    }

    /// Écrit une ligne dont le haut est à `top` (depuis le haut de la zone de texte).
    #[allow(clippy::too_many_arguments)]
    fn draw_line(&mut self, l: &Line, x0: f32, width: f32, align: Align, size: f32, lh: f32) {
        let top = self.cur().used;
        let pi = self.pages.len() - 1;
        self.draw_line_at(pi, top, l, x0, width, align, size);
        let p = self.cur();
        p.used += lh;
        p.empty = false;
    }

    /// Écrit une ligne sur la page `pi`, son haut à `top` points sous la marge haute.
    #[allow(clippy::too_many_arguments)]
    fn draw_line_at(
        &mut self,
        pi: usize,
        top: f32,
        l: &Line,
        x0: f32,
        width: f32,
        align: Align,
        size: f32,
    ) {
        let lh = self.lh;
        let baseline = self.ph - self.mt - top - (lh - 1.107 * size).max(0.0) / 2.0 - 0.891 * size;
        let spaces = l
            .atoms
            .iter()
            .filter(|a| a.space && a.bytes == [0x20])
            .count();
        let justify = align == Align::Justify && !l.last && spaces > 0 && l.width < width;
        let extra = if justify {
            (width - l.width) / spaces as f32
        } else {
            0.0
        };
        let total = l.width + extra * spaces as f32;
        let mut x = x0
            + match align {
                Align::Center => ((width - total) / 2.0).max(0.0),
                Align::Right => (width - total).max(0.0),
                _ => 0.0,
            };
        let mut ops = String::new();
        let mut i = 0;
        while i < l.atoms.len() {
            let a = &l.atoms[i];
            let mut j = i;
            let mut bytes = Vec::new();
            let mut w = 0.0;
            let mut sp = 0usize;
            while j < l.atoms.len()
                && l.atoms[j].style == a.style
                && l.atoms[j].missing == a.missing
                && l.atoms[j].lien == a.lien
            {
                bytes.extend_from_slice(&l.atoms[j].bytes);
                w += l.atoms[j].width;
                if l.atoms[j].space && l.atoms[j].bytes == [0x20] {
                    sp += 1;
                }
                j += 1;
            }
            let pw = w + extra * sp as f32;
            let color = if a.missing {
                "0.63 0.13 0.11"
            } else if a.lien.is_some() {
                "0.12 0.37 0.75"
            } else {
                "0 0 0"
            };
            ops.push_str(&format!(
                "{color} rg BT /{} {} Tf {} Tw 1 0 0 1 {} {} Tm <{}> Tj ET\n",
                font_name(a.style),
                n(size),
                n(extra),
                n(x),
                n(baseline),
                hex(&bytes)
            ));
            let thick = (size * 0.05).max(0.3);
            if let Some(url) = &a.lien {
                self.pages[pi].liens.push((
                    x,
                    baseline - size * 0.25,
                    x + pw,
                    baseline + size * 0.85,
                    url.clone(),
                ));
            }
            if a.style.underline || a.lien.is_some() {
                ops.push_str(&format!(
                    "{} {} {} {} re f\n",
                    n(x),
                    n(baseline - size * 0.12),
                    n(pw),
                    n(thick)
                ));
            }
            if a.style.strike {
                ops.push_str(&format!(
                    "{} {} {} {} re f\n",
                    n(x),
                    n(baseline + size * 0.3),
                    n(pw),
                    n(thick)
                ));
            }
            x += pw;
            i = j;
        }
        // Liens cliquables : les mots qui sont des adresses (http, https, mailto).
        let mut lx = x0
            + match align {
                Align::Center => ((width - total) / 2.0).max(0.0),
                Align::Right => (width - total).max(0.0),
                _ => 0.0,
            };
        for a in &l.atoms {
            let w = a.width
                + if justify && a.space && a.bytes == [0x20] {
                    extra
                } else {
                    0.0
                };
            // Un lien [texte](url) a déjà sa zone cliquable.
            if a.lien.is_none()
                && let Some(url) = adresse_de(&a.bytes)
            {
                self.pages[pi]
                    .links
                    .push((lx, baseline - size * 0.2, w, size * 1.1, url));
            }
            lx += w;
        }
        self.pages[pi].ops.push_str(&ops);
        self.pages[pi].empty = false;
    }

    /// Place un paragraphe déjà découpé en lignes, avec gestion des veuves et orphelines.
    #[allow(clippy::too_many_arguments)]
    fn place(
        &mut self,
        lines: &[Line],
        x0: f32,
        width: f32,
        align: Align,
        size: f32,
        lh: f32,
        keep_next: bool,
    ) -> (usize, f32) {
        let total = lines.len();
        let mut done = 0;
        let mut first = None;
        while done < total {
            let left = total - done;
            let fit = ((self.room() + 0.01) / lh).floor().max(0.0) as usize;
            let mut take = fit.min(left);
            if take < left {
                // Veuves et orphelines : au moins deux lignes de chaque côté.
                if done == 0 && left <= 3 {
                    take = 0;
                } else if left - take < 2 {
                    take = left.saturating_sub(2);
                }
                if done == 0 && take < 2 && left > 3 {
                    take = 0;
                }
            } else if keep_next && done + take == total {
                // Un titre doit avoir de la place pour deux lignes à sa suite.
                let after = self.room() - take as f32 * lh;
                if after < 2.0 * self.lh && !self.cur().empty && take == left && done == 0 {
                    take = 0;
                }
            }
            if take == 0 {
                if self.cur().empty {
                    take = 1; // au moins une ligne par page : on avance toujours
                } else {
                    self.new_page();
                    continue;
                }
            }
            if first.is_none() {
                first = Some((self.pages.len() - 1, self.cur().used));
            }
            for l in &lines[done..done + take] {
                self.draw_line(l, x0, width, align, size, lh);
            }
            done += take;
            if done < total {
                self.new_page();
            }
        }
        first.unwrap_or((self.pages.len().saturating_sub(1), 0.0))
    }

    fn space(&mut self, h: f32) {
        // L'espace de début de page est ignoré, comme dans un traitement de texte.
        let room = self.room();
        let p = self.cur();
        if !p.empty {
            p.used += h.min(room.max(0.0));
        }
    }

    fn text_para(&mut self, runs: &[Run], align: Align, x0: f32, width: f32, force: Style) {
        let size = self.ctx.size;
        let lines = self.ctx.wrap(self.ctx.atoms(runs, size, force), width);
        let lh = self.lh;
        self.place(&lines, x0, width, align, size, lh, false);
    }

    fn table(&mut self, t: &Table) {
        let cols = t.columns().max(1);
        let cw = self.tw / cols as f32;
        let pad = 3.0;
        let size = self.ctx.size;
        let lh = self.lh;
        for (ri, row) in t.rows.iter().enumerate() {
            let head = ri == 0 && t.header;
            let force = Style {
                bold: head,
                ..Style::default()
            };
            let cells: Vec<Vec<Line>> = (0..cols)
                .map(|c| {
                    let runs = row.get(c).map(Vec::as_slice).unwrap_or(&[]);
                    self.ctx
                        .wrap(self.ctx.atoms(runs, size, force), cw - 2.0 * pad)
                })
                .collect();
            let nlines = cells.iter().map(Vec::len).max().unwrap_or(1).max(1);
            let mut done = 0;
            while done < nlines {
                let fit = ((self.room() - 2.0 * pad + 0.01) / lh).floor().max(0.0) as usize;
                let mut take = fit.min(nlines - done);
                if take == 0 {
                    if self.cur().empty {
                        take = 1;
                    } else {
                        self.new_page();
                        continue;
                    }
                }
                // Une ligne de tableau courte n'est jamais coupée si elle tient ailleurs.
                if done == 0 && take < nlines && nlines <= 6 && !self.cur().empty {
                    self.new_page();
                    continue;
                }
                let top = self.cur().used;
                let height = take as f32 * lh + 2.0 * pad;
                let ytop = self.ph - self.mt - top;
                let mut rect = String::new();
                for c in 0..cols {
                    rect.push_str(&format!(
                        "{} {} {} {} re S\n",
                        n(self.ml + c as f32 * cw),
                        n(ytop - height),
                        n(cw),
                        n(height)
                    ));
                }
                self.cur().ops.push_str("0.3 w 0 0 0 RG\n");
                self.cur().ops.push_str(&rect);
                self.cur().used += pad;
                let inner = self.cur().used;
                for (c, cell) in cells.iter().enumerate() {
                    self.cur().used = inner;
                    let end = (done + take).min(cell.len());
                    if done < end {
                        for l in &cell[done..end] {
                            let a = t.align(c);
                            self.draw_line(
                                l,
                                self.ml + c as f32 * cw + pad,
                                cw - 2.0 * pad,
                                a,
                                size,
                                lh,
                            );
                        }
                    }
                }
                self.cur().used = inner + take as f32 * lh + pad;
                done += take;
                if done < nlines {
                    self.new_page();
                }
            }
        }
        self.space(size * 0.4);
    }

    /// Un code QR en carrés vectoriels, de `TAILLE_MM` de côté (marge blanche comprise).
    fn qr(&mut self, contenu: &str, align: Align) -> bool {
        let Ok(q) = crate::qr::QrCode::encode(contenu) else {
            return false;
        };
        let cote = mm(crate::qr::TAILLE_MM).min(self.tw).min(self.th);
        if self.room() < cote && !self.cur().empty {
            self.new_page();
        }
        let x0 = match align {
            Align::Center => self.ml + (self.tw - cote) / 2.0,
            Align::Right => self.ml + self.tw - cote,
            _ => self.ml,
        };
        let top = self.ph - self.mt - self.cur().used;
        let marge = 2.0;
        let module = cote / (q.size() as f32 + 2.0 * marge);
        let mut ops = String::from("q 0 0 0 rg\n");
        for (x, y, l) in q.segments() {
            ops.push_str(&format!(
                "{} {} {} {} re\n",
                n(x0 + (x as f32 + marge) * module),
                n(top - (y as f32 + marge + 1.0) * module),
                n(l as f32 * module),
                n(module)
            ));
        }
        ops.push_str("f Q\n");
        let p = self.cur();
        p.ops.push_str(&ops);
        p.used += cote;
        p.empty = false;
        true
    }

    fn para(&mut self, p: &Para) {
        let size = self.ctx.size;
        let (ml, tw) = (self.ml, self.tw);
        let none = Style::default();
        if let (Some(q), Para::Text { align, .. }) = (p.qr(), p)
            && self.qr(q, *align)
        {
            return;
        }
        match p {
            Para::Text { align, runs } => {
                if runs.is_empty() {
                    let (lh, room) = (self.lh, self.room());
                    let pg = self.cur();
                    pg.used += lh.min(room.max(0.0));
                    return;
                }
                let a = match (*align, self.page.justify) {
                    (Align::Left, true) => Align::Justify,
                    (a, _) => a,
                };
                self.text_para(runs, a, ml, tw, none);
            }
            Para::Heading { level, runs } => {
                let f = [1.6, 1.35, 1.15][(*level as usize).clamp(1, 3) - 1];
                let hs = size * f;
                let lh = hs * 1.25;
                self.space(size * 0.5);
                let bold = Style { bold: true, ..none };
                let lines = self.ctx.wrap(self.ctx.atoms(runs, hs, bold), tw);
                let (pg, y) = self.place(&lines, ml, tw, Align::Left, hs, lh, true);
                let titre = crate::doc::plain(runs);
                let titre = titre.split_whitespace().collect::<Vec<_>>().join(" ");
                if !titre.is_empty() {
                    self.marks.push(((*level).clamp(1, 3), titre, pg, y));
                }
                self.space(size * 0.25);
            }
            Para::Item {
                number,
                depth,
                runs,
            } => {
                let indent = 18.0 * (*depth as f32 + 1.0);
                let label = match number {
                    Some(k) => format!("{k}."),
                    None => "\u{2022}".into(),
                };
                let label_atoms = self.ctx.atoms(&[plain_run(&label)], size, none);
                let lw: f32 = label_atoms.iter().map(|a| a.width).sum();
                let body = self
                    .ctx
                    .wrap(self.ctx.atoms(runs, size, none), (tw - indent).max(20.0));
                let lh = self.lh;
                let (pi, top) = self.place(
                    &body,
                    ml + indent,
                    tw - indent,
                    Align::Left,
                    size,
                    lh,
                    false,
                );
                let line = Line {
                    width: lw,
                    atoms: label_atoms,
                    last: true,
                };
                self.draw_line_at(
                    pi,
                    top,
                    &line,
                    ml + indent - 4.0 - lw,
                    lw + 1.0,
                    Align::Left,
                    size,
                );
            }
            Para::Quote { runs } => {
                let ital = Style {
                    italic: true,
                    ..none
                };
                self.text_para(runs, Align::Left, ml + 28.0, tw - 56.0, ital);
                self.space(size * 0.3);
            }
            Para::Math { .. } => {
                let ital = Style {
                    italic: true,
                    ..none
                };
                let run = plain_run(&p.plain());
                self.text_para(&[run], Align::Center, ml, tw, ital);
            }
            Para::Table(t) => {
                self.space(size * 0.4);
                self.table(t);
            }
            Para::Rule => {
                self.space(size * 0.5);
                let y = self.ph - self.mt - self.cur().used;
                let room = self.room();
                if room < 1.0 && !self.cur().empty {
                    self.new_page();
                }
                let y = if room < 1.0 { self.ph - self.mt } else { y };
                self.cur().ops.push_str(&format!(
                    "0.5 w 0.27 0.27 0.27 RG {} {} m {} {} l S\n",
                    n(ml),
                    n(y),
                    n(ml + tw),
                    n(y)
                ));
                self.cur().empty = false;
                self.space(size * 0.5);
            }
            Para::PageBreak => self.break_page(),
        }
    }
}

/// Si le mot est une adresse (`http://`, `https://`, `mailto:`), renvoie la cible du lien,
/// sans la ponctuation finale (`.,;:!?)`).
fn adresse_de(bytes: &[u8]) -> Option<String> {
    let t = String::from_utf8_lossy(bytes);
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

/// Échappe une chaîne ASCII pour une chaîne littérale PDF.
fn pdf_texte(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '(' | ')' | '\\' => {
                o.push('\\');
                o.push(c);
            }
            c if c.is_ascii_graphic() => o.push(c),
            _ => {}
        }
    }
    o
}

fn plain_run(s: &str) -> Run {
    Run::Text {
        text: s.to_string(),
        style: Style::default(),
    }
}

// ---------------------------------------------------------------------------
// Fichier PDF
// ---------------------------------------------------------------------------

fn utf16_hex(s: &str) -> String {
    let mut out = String::from("FEFF");
    for u in s.encode_utf16() {
        out.push_str(&format!("{u:04X}"));
    }
    out
}

/// Le document PDF.
pub fn pdf(title: &str, docs: &[Rendered], page: &PageSetup, fonts: &PdfFonts) -> Vec<u8> {
    let size = if page.font_size.is_finite() {
        page.font_size.clamp(4.0, 72.0)
    } else {
        12.0
    };
    let spacing = if page.line_spacing.is_finite() {
        page.line_spacing.clamp(0.8, 4.0)
    } else {
        1.15
    };
    let (w, h) = page.size_mm();
    let m = |v: f32| if v.is_finite() { v.max(0.0) } else { 20.0 };
    let (ml, mt) = (m(page.margin_left), m(page.margin_top));
    let mut lay = Layout {
        ctx: Ctx { fonts, size },
        page,
        pages: Vec::new(),
        starts: Vec::new(),
        marks: Vec::new(),
        pw: mm(w),
        ph: mm(h),
        ml: mm(ml.min(w - 20.0)),
        mt: mm(mt.min(h - 20.0)),
        tw: mm(page.text_width_mm()),
        th: mm(page.text_height_mm()),
        lh: size * spacing,
    };
    for d in docs {
        lay.starts.push(lay.pages.len());
        lay.new_page();
        for p in &d.paras {
            lay.para(p);
        }
    }
    if lay.pages.is_empty() {
        lay.new_page();
    }
    lay.starts.push(lay.pages.len());

    // En-têtes, pieds de page et numéros.
    let small = size * 0.75;
    let none = Style::default();
    let mut ops_extra: Vec<String> = vec![String::new(); lay.pages.len()];
    for (di, d) in docs.iter().enumerate() {
        let (a, b) = (lay.starts[di], lay.starts[di + 1]);
        let count = b - a;
        for (k, extra) in ops_extra.iter_mut().enumerate().take(b).skip(a) {
            let mut s = String::new();
            if !d.header.is_empty() {
                let at = lay.ctx.atoms(&d.header, small, none);
                let line = lay.ctx.wrap(at, lay.tw).into_iter().next();
                if let Some(line) = line {
                    let y = lay.ph - lay.mt / 2.0 - small * 0.3;
                    s.push_str(&line_ops(&line, lay.ml, lay.tw, Align::Center, small, y));
                }
            }
            let y = lay.mt.min(lay.ph) * 0.0 + mm(page.margin_bottom.max(0.0)).min(lay.ph) / 2.0;
            let y = y.max(small * 0.5);
            if !d.footer.is_empty() {
                let at = lay.ctx.atoms(&d.footer, small, none);
                if let Some(line) = lay.ctx.wrap(at, lay.tw * 0.7).into_iter().next() {
                    s.push_str(&line_ops(&line, lay.ml, lay.tw, Align::Left, small, y));
                }
            }
            if page.page_numbers {
                let txt = format!("Page {} sur {}", k - a + 1, count);
                let at = lay.ctx.atoms(&[plain_run(&txt)], small, none);
                if let Some(line) = lay.ctx.wrap(at, lay.tw).into_iter().next() {
                    s.push_str(&line_ops(&line, lay.ml, lay.tw, Align::Right, small, y));
                }
            }
            *extra = s;
        }
    }

    // Assemblage des objets.
    let np = lay.pages.len();
    let (base, sans) = match page.font {
        DocFont::Serif => (
            [
                "Times-Roman",
                "Times-Bold",
                "Times-Italic",
                "Times-BoldItalic",
            ],
            false,
        ),
        DocFont::Sans => (
            [
                "Helvetica",
                "Helvetica-Bold",
                "Helvetica-Oblique",
                "Helvetica-BoldOblique",
            ],
            true,
        ),
    };
    let _ = sans;
    let mut objs: Vec<Vec<u8>> = Vec::new();
    // 1 catalogue, 2 pages, 3 info, 4-7 polices, puis (page, contenu) par page.
    // Signets : un titre est rattaché au titre de niveau inférieur qui le précède.
    let marks: Vec<&(u8, String, usize, f32)> = lay.marks.iter().filter(|m| m.2 < np).collect();
    let nm = marks.len();
    let mut parent: Vec<Option<usize>> = vec![None; nm];
    let mut pile: Vec<usize> = Vec::new();
    for i in 0..nm {
        while pile.last().is_some_and(|&j| marks[j].0 >= marks[i].0) {
            pile.pop();
        }
        parent[i] = pile.last().copied();
        pile.push(i);
    }
    let enfants =
        |p: Option<usize>| -> Vec<usize> { (0..nm).filter(|&i| parent[i] == p).collect() };
    let racine = 8 + 2 * np; // numéro d'objet de la racine des signets
    if nm == 0 {
        objs.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    } else {
        objs.push(
            format!(
                "<< /Type /Catalog /Pages 2 0 R /Outlines {racine} 0 R /PageMode /UseOutlines >>"
            )
            .into_bytes(),
        );
    }
    let kids: String = (0..np).map(|i| format!("{} 0 R ", 8 + i * 2)).collect();
    objs.push(format!("<< /Type /Pages /Kids [{kids}] /Count {np} >>").into_bytes());
    objs.push(
        format!(
            "<< /Title <{}> /Producer (Varium) /Creator (Varium) >>",
            utf16_hex(title)
        )
        .into_bytes(),
    );
    for b in base {
        objs.push(
            format!("<< /Type /Font /Subtype /Type1 /BaseFont /{b} /Encoding /WinAnsiEncoding >>")
                .into_bytes(),
        );
    }
    // Les annotations de lien viennent après les signets.
    let mut prochain = if nm > 0 { racine + nm + 1 } else { racine };
    let mut annots: Vec<String> = Vec::new();
    let mut objets_liens: Vec<Vec<u8>> = Vec::new();
    for page_buf in &lay.pages {
        let mut ids = String::new();
        for (x, y, w, h, url) in &page_buf.links {
            ids.push_str(&format!("{prochain} 0 R "));
            prochain += 1;
            objets_liens.push(
                format!(
                    "<< /Type /Annot /Subtype /Link /Rect [{} {} {} {}] /Border [0 0 0] /A << /S /URI /URI ({}) >> >>",
                    n(*x),
                    n(*y),
                    n(x + w),
                    n(y + h),
                    pdf_texte(url)
                )
                .into_bytes(),
            );
        }
        annots.push(ids);
    }
    for (i, (page_buf, extra)) in lay.pages.iter().zip(&ops_extra).enumerate() {
        let content = format!("{}{}", page_buf.ops, extra);
        // Liens : annotations cliquables vers leur adresse ([texte](url) en ligne, et
        // adresses nues en objets séparés).
        let mut a: String = page_buf
            .liens
            .iter()
            .map(|(x0, y0, x1, y1, url)| {
                format!(
                    "<< /Type /Annot /Subtype /Link /Rect [{} {} {} {}] /Border [0 0 0] /A << /S /URI /URI <{}> >> >> ",
                    n(*x0),
                    n(*y0),
                    n(*x1),
                    n(*y1),
                    hex(url.as_bytes())
                )
            })
            .collect();
        a.push_str(&annots[i]);
        let annots_page = if a.is_empty() {
            String::new()
        } else {
            format!(" /Annots [{}]", a.trim_end())
        };
        objs.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 4 0 R /F2 5 0 R /F3 6 0 R /F4 7 0 R >> >> /Contents {} 0 R{annots_page} >>",
                n(lay.pw),
                n(lay.ph),
                9 + i * 2
            )
            .into_bytes(),
        );
        let mut c = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        c.extend_from_slice(content.as_bytes());
        c.extend_from_slice(b"endstream");
        objs.push(c);
    }
    if nm > 0 {
        let id = |i: usize| racine + 1 + i;
        let top = enfants(None);
        objs.push(
            format!(
                "<< /Type /Outlines /First {} 0 R /Last {} 0 R /Count {} >>",
                id(top[0]),
                id(top[top.len() - 1]),
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
                utf16_hex(&m.1),
                parent[i].map_or(racine, id),
                8 + 2 * m.2,
                n(lay.ml),
                n((lay.ph - lay.mt - m.3 + 4.0).min(lay.ph))
            );
            if pos > 0 {
                d.push_str(&format!(" /Prev {} 0 R", id(freres[pos - 1])));
            }
            if pos + 1 < freres.len() {
                d.push_str(&format!(" /Next {} 0 R", id(freres[pos + 1])));
            }
            if let (Some(a), Some(b)) = (fils.first(), fils.last()) {
                d.push_str(&format!(
                    " /First {} 0 R /Last {} 0 R /Count {}",
                    id(*a),
                    id(*b),
                    fils.len()
                ));
            }
            d.push_str(" >>");
            objs.push(d.into_bytes());
        }
    }
    objs.extend(objets_liens);
    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objs.len());
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in &offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 3 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// Écrit une ligne à une hauteur absolue (en-tête, pied de page).
fn line_ops(l: &Line, x0: f32, width: f32, align: Align, size: f32, baseline: f32) -> String {
    let total = l.width;
    let mut x = x0
        + match align {
            Align::Center => ((width - total) / 2.0).max(0.0),
            Align::Right => (width - total).max(0.0),
            _ => 0.0,
        };
    let mut s = String::new();
    let mut i = 0;
    while i < l.atoms.len() {
        let a = &l.atoms[i];
        let mut bytes = Vec::new();
        let mut w = 0.0;
        let mut j = i;
        while j < l.atoms.len() && l.atoms[j].style == a.style && l.atoms[j].missing == a.missing {
            bytes.extend_from_slice(&l.atoms[j].bytes);
            w += l.atoms[j].width;
            j += 1;
        }
        let color = if a.missing {
            "0.63 0.13 0.11"
        } else {
            "0.25 0.25 0.25"
        };
        s.push_str(&format!(
            "{color} rg BT /{} {} Tf 0 Tw 1 0 0 1 {} {} Tm <{}> Tj ET\n",
            font_name(a.style),
            n(size),
            n(x),
            n(baseline),
            hex(&bytes)
        ));
        x += w;
        i = j;
    }
    s
}

/// Exporte directement une source avec ses valeurs (une page par document).
pub fn to_pdf(title: &str, src: &str, values: &crate::render::Values, fonts: &PdfFonts) -> Vec<u8> {
    let page = PageSetup::default();
    let r = crate::export::prepare(src, values, &page);
    pdf(title, &[r], &page, fonts)
}
