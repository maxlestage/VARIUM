//! Document de présentation (polycopié) : 1, 2, 3 ou 6 diapositives par page A4
//! ou Lettre US, avec des lignes pour les notes ou les notes de l'orateur à côté
//! de chaque diapositive.

use std::fmt::Write;

use varium_core::{Align, Run};

use super::{
    Base, Corps, Habillage, LARGEUR_PAGE, Police, assembler, hauteur_diapo, images_pdf,
    ligne_seule, n, page, rgb, zone,
};
use crate::geometrie::{Anchor, Frame};
use crate::{Deck, Palette};

/// Format du papier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HandoutPaper {
    #[default]
    A4,
    Letter,
}

impl HandoutPaper {
    /// Dimensions en points (portrait).
    pub fn size_pt(self) -> (f32, f32) {
        match self {
            HandoutPaper::A4 => (595.276, 841.89),
            HandoutPaper::Letter => (612.0, 792.0),
        }
    }
}

/// Ce qui accompagne chaque diapositive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HandoutNotes {
    /// Des lignes pour prendre des notes.
    #[default]
    Lines,
    /// Les notes de l'orateur (des lignes si la diapositive n'en a pas).
    Speaker,
    /// Rien : les diapositives seules.
    None,
}

/// Réglages du document de présentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandoutOptions {
    /// Diapositives par page : 1, 2, 3 ou 6 (une autre valeur est ramenée à la
    /// plus proche).
    pub per_page: usize,
    pub notes: HandoutNotes,
    pub paper: HandoutPaper,
}

impl Default for HandoutOptions {
    fn default() -> Self {
        HandoutOptions {
            per_page: 3,
            notes: HandoutNotes::Lines,
            paper: HandoutPaper::A4,
        }
    }
}

impl HandoutOptions {
    /// Valeurs proposées pour [`HandoutOptions::per_page`].
    pub const PER_PAGE: [usize; 4] = [1, 2, 3, 6];

    fn par_page(&self) -> usize {
        match self.per_page {
            0 | 1 => 1,
            2 => 2,
            3 | 4 => 3,
            _ => 6,
        }
    }
}

/// Couleurs du papier.
const PAPIER: Palette = Palette {
    background: [255, 255, 255],
    text: [40, 40, 44],
    title: [20, 20, 24],
    accent: [90, 90, 96],
    muted: [120, 120, 126],
    serif_titles: false,
};

const MARGE: f32 = 42.0;
const HAUT: f32 = 58.0;
const BAS: f32 = 48.0;

/// Des lignes horizontales pour écrire, dans le rectangle (haut à `y`).
fn lignes(out: &mut String, x: f32, y: f32, w: f32, h: f32, ph: f32, pas: f32) {
    let _ = writeln!(out, "0.78 0.78 0.8 RG 0.5 w");
    let mut yy = y + pas;
    while yy <= y + h + 0.01 {
        let _ = writeln!(
            out,
            "{} {} m {} {} l S",
            n(x),
            n(ph - yy),
            n(x + w),
            n(ph - yy)
        );
        yy += pas;
    }
}

fn texte(s: &str) -> Vec<Run> {
    vec![Run::Text {
        text: s.to_string(),
        style: Default::default(),
    }]
}

pub(crate) fn polycopie(deck: &Deck, o: HandoutOptions) -> Vec<u8> {
    let (pw, ph) = o.paper.size_pt();
    let par_page = o.par_page();
    let rendus = deck.rendus();
    let slides = deck.exported();
    let nd = rendus.len();
    let np = nd.div_ceil(par_page).max(1);
    let (images, nums) = images_pdf(deck, 12 + 2 * np);
    let dw = LARGEUR_PAGE;
    let dh = hauteur_diapo(deck);
    let ratio = dw / dh;
    let (cx, cy) = (MARGE, HAUT);
    let (cw, ch) = (pw - 2.0 * MARGE, ph - HAUT - BAS);
    let pied = Base {
        police: Police::default(),
        couleur: PAPIER.muted,
        majuscules: false,
    };
    let petit = Corps {
        taille: 9.0,
        approche: 0.0,
    };
    let titre = deck.title.trim();
    let mut pages = Vec::with_capacity(np);
    for p in 0..np {
        let mut out = String::new();
        let _ = writeln!(out, "1 g 0 0 {} {} re f", n(pw), n(ph));
        if !titre.is_empty() {
            ligne_seule(
                &mut out,
                &texte(titre),
                Frame::new(MARGE / pw, 24.0 / ph, cw / pw, 16.0 / ph),
                pied,
                petit,
                false,
                pw,
                ph,
            );
        }
        ligne_seule(
            &mut out,
            &texte(&format!("{} / {np}", p + 1)),
            Frame::new(MARGE / pw, (ph - 34.0) / ph, cw / pw, 16.0 / ph),
            pied,
            petit,
            true,
            pw,
            ph,
        );
        for k in 0..par_page {
            let i = p * par_page + k;
            let (Some(r), Some(s)) = (rendus.get(i), slides.get(i)) else {
                break;
            };
            // Place de la diapositive et de ses notes.
            let (sx, sy, sw, sh, notes) = if par_page == 1 {
                let sw = cw;
                let sh = sw / ratio;
                let sh = sh.min(ch * 0.55);
                let sw = sh * ratio;
                let ny = cy + sh + 24.0;
                (
                    cx + (cw - sw) / 2.0,
                    cy,
                    sw,
                    sh,
                    (cx, ny, cw, (cy + ch - ny).max(0.0)),
                )
            } else {
                let rh = ch / par_page as f32;
                let fraction = match par_page {
                    2 => 0.56,
                    3 => 0.48,
                    _ => 0.4,
                };
                let sw = (cw * fraction).min((rh - 14.0).max(10.0) * ratio);
                let sh = sw / ratio;
                let sy = cy + rh * k as f32 + (rh - sh) / 2.0;
                let nx = cx + sw + 20.0;
                (cx, sy, sw, sh, (nx, sy, (cx + cw - nx).max(0.0), sh))
            };
            let echelle = sw / dw;
            let contenu = page(r, s, &nums, dw, dh);
            let _ = writeln!(
                out,
                "q {} 0 0 {} {} {} cm 0 0 {} {} re W n",
                n(echelle),
                n(echelle),
                n(sx),
                n(ph - (sy + sh)),
                n(dw),
                n(dh)
            );
            out.push_str(&contenu);
            let _ = writeln!(out, "Q");
            let _ = writeln!(
                out,
                "{} RG 0.6 w {} {} {} {} re S",
                rgb([170, 170, 176]),
                n(sx),
                n(ph - (sy + sh)),
                n(sw),
                n(sh)
            );
            // Numéro de la diapositive, dans la marge.
            ligne_seule(
                &mut out,
                &texte(&(i + 1).to_string()),
                Frame::new((sx - 30.0) / pw, sy / ph, 24.0 / pw, 12.0 / ph),
                pied,
                Corps {
                    taille: 8.0,
                    approche: 0.0,
                },
                true,
                pw,
                ph,
            );
            let (nx, ny, nw, nh) = notes;
            if nw < 20.0 || nh < 20.0 {
                continue;
            }
            let orateur = o.notes == HandoutNotes::Speaker && !r.notes.is_empty();
            if orateur {
                let hab = Habillage {
                    base: Base {
                        police: Police::default(),
                        couleur: PAPIER.text,
                        majuscules: false,
                    },
                    pal: PAPIER,
                    interligne: 1.3,
                    approche: 0.0,
                    align: Align::Left,
                    titre: false,
                    citation: false,
                };
                zone(
                    &mut out,
                    &r.notes,
                    Frame::new(nx / pw, ny / ph, nw / pw, nh / ph),
                    if par_page == 1 { 12.0 } else { 10.0 },
                    &hab,
                    Anchor::Top,
                    pw,
                    ph,
                );
            } else if o.notes != HandoutNotes::None {
                let pas = if par_page >= 6 { 16.0 } else { 22.0 };
                lignes(&mut out, nx, ny, nw, nh, ph, pas);
            }
        }
        pages.push((pw, ph, out));
    }
    assembler(deck, &pages, images, &nums)
}

impl Deck {
    /// Le document de présentation (polycopié) en PDF.
    pub fn to_handout_pdf(&self, options: HandoutOptions) -> Vec<u8> {
        polycopie(self, options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_et_formats() {
        let deck = Deck::sample();
        let compte = |pdf: &[u8]| String::from_utf8_lossy(pdf).matches("/Type /Page ").count();
        for (par, attendu) in [(1, 8), (2, 4), (3, 3), (6, 2), (0, 8), (4, 3), (99, 2)] {
            let pdf = deck.to_handout_pdf(HandoutOptions {
                per_page: par,
                ..HandoutOptions::default()
            });
            assert!(pdf.starts_with(b"%PDF-1.4"));
            assert_eq!(compte(&pdf), attendu, "{par} par page");
        }
        let lettre = deck.to_handout_pdf(HandoutOptions {
            paper: HandoutPaper::Letter,
            notes: HandoutNotes::Speaker,
            per_page: 2,
        });
        let t = String::from_utf8_lossy(&lettre);
        assert!(t.contains("/MediaBox [0 0 612 792]"));
        // Les notes de l'orateur sont écrites (codage WinAnsi, en hexadécimal).
        let mot: String = "Accueillir".bytes().map(|b| format!("{b:02X}")).collect();
        assert!(t.contains(&mot));
        let vide = Deck::new().to_handout_pdf(HandoutOptions::default());
        assert_eq!(compte(&vide), 1);
    }
}
