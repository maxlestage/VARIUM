//! Polices TrueType : lecture minimale (correspondance caractère → glyphe, largeurs,
//! métriques verticales) et sous-ensemble pour l'incorporation dans le PDF.
//!
//! Aucune dépendance : seules les tables utiles sont lues (`head`, `hhea`, `hmtx`,
//! `maxp`, `cmap` formats 4 et 12, `loca`, `glyf`, `post`). Une police sans contours
//! TrueType (`glyf`), par exemple une police CFF, est refusée : on ne saurait pas
//! l'incorporer.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

fn u16_at(d: &[u8], o: usize) -> Option<usize> {
    let b = d.get(o..o.checked_add(2)?)?;
    Some(u16::from_be_bytes([b[0], b[1]]) as usize)
}

fn i16_at(d: &[u8], o: usize) -> Option<i16> {
    u16_at(d, o).map(|v| v as u16 as i16)
}

fn u32_at(d: &[u8], o: usize) -> Option<usize> {
    let b = d.get(o..o.checked_add(4)?)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
}

/// Table `tag` : (position, longueur).
fn table(d: &[u8], tag: &[u8; 4]) -> Option<(usize, usize)> {
    let n = u16_at(d, 4)?;
    (0..n.min(64)).find_map(|i| {
        let rec = 12 + i * 16;
        if d.get(rec..rec + 4)? != tag {
            return None;
        }
        let (o, l) = (u32_at(d, rec + 8)?, u32_at(d, rec + 12)?);
        (o.checked_add(l)? <= d.len()).then_some((o, l))
    })
}

/// Une police TrueType lue.
pub struct Face {
    data: Arc<[u8]>,
    upem: f32,
    /// Hauteur au-dessus de la ligne de base, profondeur dessous et interligne
    /// supplémentaire, en em.
    pub ascent: f32,
    pub descent: f32,
    pub line_gap: f32,
    /// Boîte englobante des glyphes, en millièmes d'em.
    pub bbox: [f32; 4],
    /// Angle de l'italique, en degrés.
    pub italic_angle: f32,
    /// Hauteur des capitales en millièmes d'em (approchée).
    pub cap_height: f32,
    /// Nom PostScript de la police (table `name`), s'il est lisible.
    pub nom: Option<String>,
    avances: Vec<u16>,
    /// Caractères usuels (latin, grec, cyrillique) : accès direct.
    rapide: Vec<u16>,
    autres: HashMap<u32, u16>,
    loca: usize,
    loca_long: bool,
    glyf: (usize, usize),
    nb_glyphes: usize,
}

impl std::fmt::Debug for Face {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Face({} glyphes)", self.nb_glyphes)
    }
}

impl PartialEq for Face {
    fn eq(&self, o: &Self) -> bool {
        self.data == o.data
    }
}

const RAPIDE: usize = 0x0530;

impl Face {
    /// Lit une police TrueType ; `None` si le fichier est illisible ou sans `glyf`.
    pub fn lire(d: &[u8]) -> Option<Face> {
        let (head, _) = table(d, b"head")?;
        let upem = u16_at(d, head + 18)?.clamp(16, 16384) as f32;
        let loca_long = i16_at(d, head + 50)? != 0;
        let bbox = [
            i16_at(d, head + 36)?,
            i16_at(d, head + 38)?,
            i16_at(d, head + 40)?,
            i16_at(d, head + 42)?,
        ]
        .map(|v| v as f32 * 1000.0 / upem);
        let (hhea, _) = table(d, b"hhea")?;
        let ascent = i16_at(d, hhea + 4)? as f32 / upem;
        let descent = -(i16_at(d, hhea + 6)? as f32) / upem;
        let line_gap = (i16_at(d, hhea + 8)? as f32 / upem).max(0.0);
        let nh = u16_at(d, hhea + 34)?.max(1);
        let (maxp, _) = table(d, b"maxp")?;
        let nb_glyphes = u16_at(d, maxp + 4)?.max(1);
        let (hmtx, hmtx_len) = table(d, b"hmtx")?;
        let nh = nh.min(hmtx_len / 4).max(1);
        let mut avances = Vec::with_capacity(nb_glyphes);
        for g in 0..nb_glyphes {
            avances.push(u16_at(d, hmtx + g.min(nh - 1) * 4).unwrap_or(0) as u16);
        }
        let (loca, _) = table(d, b"loca")?;
        let glyf = table(d, b"glyf")?;
        let italic_angle = table(d, b"post")
            .and_then(|(p, _)| Some(i16_at(d, p + 4)? as f32 + u16_at(d, p + 6)? as f32 / 65536.0))
            .unwrap_or(0.0);
        let cap_height = table(d, b"OS/2")
            .and_then(|(o, l)| {
                (l >= 90)
                    .then(|| i16_at(d, o + 88))
                    .flatten()
                    .map(|v| v as f32 * 1000.0 / upem)
            })
            .filter(|v| *v > 0.0)
            .unwrap_or(ascent * 700.0);
        let mut f = Face {
            data: Arc::from(d),
            upem,
            ascent,
            descent,
            line_gap,
            bbox,
            italic_angle,
            cap_height,
            nom: nom_postscript(d),
            avances,
            rapide: vec![0; RAPIDE],
            autres: HashMap::new(),
            loca,
            loca_long,
            glyf,
            nb_glyphes,
        };
        f.lire_cmap()?;
        Some(f)
    }

    fn noter(&mut self, c: u32, g: usize) {
        if g == 0 || g >= self.nb_glyphes {
            return;
        }
        if (c as usize) < RAPIDE {
            if self.rapide[c as usize] == 0 {
                self.rapide[c as usize] = g as u16;
            }
        } else {
            self.autres.entry(c).or_insert(g as u16);
        }
    }

    /// Remplit la correspondance caractère → glyphe (formats 4 et 12).
    fn lire_cmap(&mut self) -> Option<()> {
        let data = self.data.clone();
        let d: &[u8] = &data;
        let (cmap, _) = table(d, b"cmap")?;
        let n = u16_at(d, cmap + 2)?;
        let mut f4 = None;
        let mut f12 = None;
        for i in 0..n.min(32) {
            let rec = cmap + 4 + i * 8;
            let (pf, en) = (u16_at(d, rec)?, u16_at(d, rec + 2)?);
            let s = cmap + u32_at(d, rec + 4)?;
            let unicode = pf == 0 || (pf == 3 && (en == 1 || en == 10)) || (pf == 3 && en == 0);
            if !unicode {
                continue;
            }
            match u16_at(d, s) {
                Some(4) if f4.is_none() || pf == 3 => f4 = Some(s),
                Some(12) => f12 = Some(s),
                _ => {}
            }
        }
        if let Some(s) = f12 {
            let ng = u32_at(d, s + 12)?.min(100_000);
            for i in 0..ng {
                let g = s + 16 + i * 12;
                let (a, b, start) = (u32_at(d, g)?, u32_at(d, g + 4)?, u32_at(d, g + 8)?);
                if b < a || b - a > 0x20000 || b > 0x10FFFF {
                    continue;
                }
                for c in a..=b {
                    self.noter(c as u32, start + (c - a));
                }
            }
        }
        if let Some(s) = f4 {
            let segs = u16_at(d, s + 6)? / 2;
            let (ends, starts) = (s + 14, s + 16 + segs * 2);
            let (deltas, ranges) = (starts + segs * 2, starts + segs * 4);
            for i in 0..segs {
                let (end, start) = (u16_at(d, ends + i * 2)?, u16_at(d, starts + i * 2)?);
                let delta = u16_at(d, deltas + i * 2)?;
                let ro = u16_at(d, ranges + i * 2)?;
                if end < start || end == 0xFFFF && start == 0xFFFF {
                    continue;
                }
                for c in start..=end.min(0xFFFE) {
                    let g = if ro == 0 {
                        (c + delta) & 0xFFFF
                    } else {
                        let at = ranges + i * 2 + ro + (c - start) * 2;
                        match u16_at(d, at) {
                            Some(0) | None => continue,
                            Some(g) => (g + delta) & 0xFFFF,
                        }
                    };
                    self.noter(c as u32, g);
                }
            }
        }
        (f4.is_some() || f12.is_some()).then_some(())
    }

    /// Le glyphe d'un caractère, s'il existe.
    pub fn glyphe(&self, c: char) -> Option<u16> {
        let u = c as u32;
        let g = if (u as usize) < RAPIDE {
            self.rapide[u as usize]
        } else {
            self.autres.get(&u).copied().unwrap_or(0)
        };
        (g != 0).then_some(g)
    }

    /// Avance d'un glyphe, en em.
    pub fn avance(&self, g: u16) -> f32 {
        self.avances.get(g as usize).copied().unwrap_or(0) as f32 / self.upem
    }

    /// Avance d'un glyphe en millièmes d'em (largeurs du PDF).
    pub fn avance_mille(&self, g: u16) -> f32 {
        (self.avance(g) * 1000.0).round()
    }

    fn glyphe_data(&self, g: usize) -> Option<(usize, usize)> {
        let d: &[u8] = &self.data;
        let (a, b) = if self.loca_long {
            (
                u32_at(d, self.loca + g * 4)?,
                u32_at(d, self.loca + g * 4 + 4)?,
            )
        } else {
            (
                u16_at(d, self.loca + g * 2)? * 2,
                u16_at(d, self.loca + g * 2 + 2)? * 2,
            )
        };
        if b < a || b > self.glyf.1 {
            return None;
        }
        Some((self.glyf.0 + a, b - a))
    }

    /// Glyphes nécessaires à `gids`, composants des glyphes composés compris.
    fn fermeture(&self, gids: &BTreeSet<u16>) -> BTreeSet<u16> {
        let d: &[u8] = &self.data;
        let mut tous: BTreeSet<u16> = gids.clone();
        tous.insert(0);
        let mut a_voir: Vec<u16> = tous.iter().copied().collect();
        let mut tours = 0;
        while let Some(g) = a_voir.pop() {
            tours += 1;
            if tours > 100_000 {
                break;
            }
            let Some((o, l)) = self.glyphe_data(g as usize) else {
                continue;
            };
            if l < 10 || i16_at(d, o).is_none_or(|n| n >= 0) {
                continue;
            }
            let mut p = o + 10;
            for _ in 0..64 {
                let (Some(flags), Some(gi)) = (u16_at(d, p), u16_at(d, p + 2)) else {
                    break;
                };
                if gi < self.nb_glyphes && tous.insert(gi as u16) {
                    a_voir.push(gi as u16);
                }
                p += 4 + if flags & 1 != 0 { 4 } else { 2 };
                p += if flags & 0x08 != 0 {
                    2
                } else if flags & 0x40 != 0 {
                    4
                } else if flags & 0x80 != 0 {
                    8
                } else {
                    0
                };
                if flags & 0x20 == 0 {
                    break;
                }
            }
        }
        tous
    }

    /// Une police TrueType réduite aux glyphes `gids` (mêmes numéros de glyphes : les
    /// autres sont vides). Tables gardées : celles qu'exige l'incorporation dans un PDF.
    pub fn sous_ensemble(&self, gids: &BTreeSet<u16>) -> Vec<u8> {
        let d: &[u8] = &self.data;
        let garder = self.fermeture(gids);
        let mut glyf = Vec::new();
        let mut loca = Vec::with_capacity((self.nb_glyphes + 1) * 4);
        for g in 0..self.nb_glyphes {
            loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
            if garder.contains(&(g as u16))
                && let Some((o, l)) = self.glyphe_data(g)
                && let Some(b) = d.get(o..o + l)
            {
                glyf.extend_from_slice(b);
                while glyf.len() % 4 != 0 {
                    glyf.push(0);
                }
            }
        }
        loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
        let mut tables: Vec<([u8; 4], Vec<u8>)> = Vec::new();
        for tag in [
            b"OS/2", b"cvt ", b"fpgm", b"head", b"hhea", b"hmtx", b"maxp", b"prep",
        ] {
            if let Some((o, l)) = table(d, tag) {
                let mut t = d[o..o + l].to_vec();
                if tag == b"head" && t.len() >= 54 {
                    t[8..12].copy_from_slice(&[0; 4]);
                    t[50..52].copy_from_slice(&1u16.to_be_bytes());
                }
                tables.push((*tag, t));
            }
        }
        tables.push((*b"glyf", glyf));
        tables.push((*b"loca", loca));
        tables.sort_by(|a, b| a.0.cmp(&b.0));
        assembler(&mut tables)
    }
}

/// Nom PostScript (identifiant 6 de la table `name`), en ASCII sans espace.
fn nom_postscript(d: &[u8]) -> Option<String> {
    let (o, _) = table(d, b"name")?;
    let n = u16_at(d, o + 2)?;
    let chaines = o + u16_at(d, o + 4)?;
    for i in 0..n.min(512) {
        let r = o + 6 + i * 12;
        let (pf, id) = (u16_at(d, r)?, u16_at(d, r + 6)?);
        if id != 6 {
            continue;
        }
        let (l, off) = (u16_at(d, r + 8)?, u16_at(d, r + 10)?);
        let b = d.get(chaines + off..chaines + off + l)?;
        let s: String = if pf == 3 || pf == 0 {
            b.chunks(2)
                .filter_map(|c| (c.len() == 2).then(|| u16::from_be_bytes([c[0], c[1]])))
                .filter_map(|u| char::from_u32(u as u32))
                .collect()
        } else {
            b.iter().map(|&c| c as char).collect()
        };
        let s: String = s
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
            .take(60)
            .collect();
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

fn somme(t: &[u8]) -> u32 {
    t.chunks(4).fold(0u32, |s, c| {
        let mut b = [0u8; 4];
        b[..c.len()].copy_from_slice(c);
        s.wrapping_add(u32::from_be_bytes(b))
    })
}

/// Écrit le fichier TrueType : répertoire des tables, tables alignées, sommes de contrôle.
fn assembler(tables: &mut [([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let n = tables.len() as u16;
    let mut puiss = 1u16;
    let mut log = 0u16;
    while puiss * 2 <= n {
        puiss *= 2;
        log += 1;
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&n.to_be_bytes());
    out.extend_from_slice(&(puiss * 16).to_be_bytes());
    out.extend_from_slice(&log.to_be_bytes());
    out.extend_from_slice(&(n * 16 - puiss * 16).to_be_bytes());
    let mut offset = 12 + 16 * tables.len();
    let mut corps = Vec::new();
    let mut head_pos = None;
    for (tag, t) in tables.iter() {
        out.extend_from_slice(tag);
        out.extend_from_slice(&somme(t).to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(t.len() as u32).to_be_bytes());
        if tag == b"head" {
            head_pos = Some(offset);
        }
        corps.extend_from_slice(t);
        while corps.len() % 4 != 0 {
            corps.push(0);
        }
        offset = 12 + 16 * tables.len() + corps.len();
    }
    out.extend_from_slice(&corps);
    if let Some(h) = head_pos
        && h + 12 <= out.len()
    {
        let ajust = 0xB1B0_AFBAu32.wrapping_sub(somme(&out));
        out[h + 8..h + 12].copy_from_slice(&ajust.to_be_bytes());
    }
    out
}

/// Une police partagée.
pub type FaceArc = Arc<Face>;

#[cfg(test)]
mod tests {
    use super::*;

    fn liberation() -> Vec<u8> {
        std::fs::read(format!(
            "{}/../varium/assets/fonts/LiberationSerif-Regular.ttf",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn lecture_et_sous_ensemble() {
        let d = liberation();
        let f = Face::lire(&d).unwrap();
        let a = f.glyphe('a').unwrap();
        assert!((f.avance(a) - 0.444).abs() < 0.01);
        assert!(f.glyphe('α').is_some() && f.glyphe('€').is_some());
        assert!(f.glyphe('😀').is_none());
        assert!((f.ascent + f.descent + f.line_gap - 1.149).abs() < 0.01);
        let mut g = BTreeSet::new();
        g.insert(a);
        g.insert(f.glyphe('É').unwrap());
        let s = f.sous_ensemble(&g);
        assert!(s.len() < d.len() / 3, "{} / {}", s.len(), d.len());
        // Le sous-ensemble se relit et garde les mêmes numéros de glyphes.
        let f2 = Face::lire_sans_cmap(&s).unwrap();
        assert!(f2.glyphe_data(a as usize).is_some_and(|(_, l)| l > 0));
        assert!(
            f2.glyphe_data(f.glyphe('z').unwrap() as usize)
                .is_some_and(|(_, l)| l == 0)
        );
    }

    #[test]
    fn polices_abimees_refusees() {
        let d = liberation();
        assert!(Face::lire(b"rien").is_none());
        for n in [0, 12, 40, 200, 1000, d.len() / 2] {
            let _ = Face::lire(&d[..n]);
        }
        let mut abime = d.clone();
        for i in (0..abime.len()).step_by(97) {
            abime[i] = abime[i].wrapping_mul(31).wrapping_add(7);
        }
        if let Some(f) = Face::lire(&abime) {
            let _ = f.sous_ensemble(&(0..300).collect());
        }
    }

    impl Face {
        /// Relit un sous-ensemble (sans table `cmap`), pour les tests.
        fn lire_sans_cmap(d: &[u8]) -> Option<Face> {
            let (head, _) = table(d, b"head")?;
            let (maxp, _) = table(d, b"maxp")?;
            Some(Face {
                data: Arc::from(d),
                upem: u16_at(d, head + 18)? as f32,
                ascent: 0.0,
                descent: 0.0,
                line_gap: 0.0,
                bbox: [0.0; 4],
                italic_angle: 0.0,
                cap_height: 0.0,
                nom: None,
                avances: Vec::new(),
                rapide: Vec::new(),
                autres: HashMap::new(),
                loca: table(d, b"loca")?.0,
                loca_long: i16_at(d, head + 50)? != 0,
                glyf: table(d, b"glyf")?,
                nb_glyphes: u16_at(d, maxp + 4)?,
            })
        }
    }
}
