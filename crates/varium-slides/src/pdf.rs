//! Export PDF : une page par diapositive, au format de la présentation (720 pt de
//! large, soit 254 mm, comme Impress).
//!
//! Le PDF est écrit à la main, sans dépendance : polices standard Helvetica et
//! Times (non incorporées, présentes dans tout lecteur), codage WinAnsi (français,
//! latin-1, « » ’ … € œ) ; un caractère hors WinAnsi (émoji, idéogramme) devient
//! `?`. Les largeurs viennent des polices Liberation (mêmes métriques), si bien que
//! la césure et le centrage sont exacts. Le texte trop long pour sa zone est
//! réduit, comme l'ajustement automatique d'Impress, puis coupé s'il le faut.

use std::collections::BTreeMap;
use std::fmt::Write;

use varium_core::{Align, Para, Run, Table};

use crate::geometrie::{
    Anchor, BODY_LINE_HEIGHT, FOOTER_SIZE, Frame, TITLE_LINE_HEIGHT, footer_frame, number_frame,
};
use crate::metriques::LARGEURS;
use crate::objets::{Object, ObjectKind, arrow_head, format_image, jpeg_composantes};
use crate::texte::MISSING_COLOR;
use crate::{Deck, Layout, Palette, Rendu, Slide, body_size, subtitle_size, title_size};

mod polycopie;
mod raster;

pub use polycopie::{HandoutNotes, HandoutOptions, HandoutPaper};
pub use raster::{PNG_HEIGHT, RasterFonts};

/// Largeur d'une page, en points.
const LARGEUR_PAGE: f32 = 720.0;
/// Hauteur de la partie visible d'une police (ascendante + descendante), en em.
const HAUTEUR_TEXTE: f32 = 1.117;
/// Hauteur de l'ascendante, en em.
const ASCENDANTE: f32 = 0.905;

// ---------------------------------------------------------------------------------
// Polices et codage
// ---------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Police {
    pub serif: bool,
    pub gras: bool,
    pub italique: bool,
}

impl Police {
    /// Position dans `LARGEURS` et numéro de la ressource (`/F1` à `/F8`).
    fn index(self) -> usize {
        self.serif as usize * 4 + self.gras as usize + self.italique as usize * 2
    }

    /// Largeur d'un octet WinAnsi, en em.
    fn largeur(self, b: u8) -> f32 {
        LARGEURS[self.index()][b as usize] as f32 / 1000.0
    }
}

const POLICES: [&str; 8] = [
    "Helvetica",
    "Helvetica-Bold",
    "Helvetica-Oblique",
    "Helvetica-BoldOblique",
    "Times-Roman",
    "Times-Bold",
    "Times-Italic",
    "Times-BoldItalic",
];

const WIN_80_9F: [char; 32] = [
    '€', '\u{0}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{0}', 'Ž', '\u{0}',
    '\u{0}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{0}', 'ž', 'Ÿ',
];

/// Code WinAnsi d'un caractère : `None` pour un caractère invisible à omettre,
/// `?` pour un caractère qui n'existe pas dans ce codage.
fn winansi(c: char) -> Option<u8> {
    match c {
        // Invisibles : césure conditionnelle, espaces sans chasse, marques de sens.
        '\u{AD}' | '\u{200B}'..='\u{200F}' | '\u{FEFF}' => None,
        c if c.is_control() => None,
        '\u{20}'..='\u{7E}' | '\u{A0}'..='\u{FF}' => Some(c as u32 as u8),
        // Espaces fines et insécables de la typographie française : insécables.
        '\u{202F}' | '\u{2007}' | '\u{2060}' => Some(0xA0),
        '\u{2002}'..='\u{2006}' | '\u{2008}'..='\u{200A}' | '\u{3000}' => Some(b' '),
        '\u{2212}' | '\u{2010}' | '\u{2011}' => Some(b'-'),
        c => Some(
            WIN_80_9F
                .iter()
                .position(|&w| w == c)
                .map(|i| 0x80 + i as u8)
                .unwrap_or(b'?'),
        ),
    }
}

// ---------------------------------------------------------------------------------
// Mots et lignes
// ---------------------------------------------------------------------------------

/// Apparence d'un mot.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Encre {
    police: Police,
    couleur: [u8; 3],
    souligne: bool,
    barre: bool,
}

#[derive(Debug, Clone)]
struct Mot {
    octets: Vec<u8>,
    encre: Encre,
    largeur: f32,
    espace: bool,
    /// Retour à la ligne forcé (valeur de champ sur plusieurs lignes).
    saut: bool,
}

#[derive(Debug, Clone, Default)]
struct Ligne {
    mots: Vec<Mot>,
    largeur: f32,
}

/// Taille et approche (espacement des lettres, en points) d'un texte.
#[derive(Debug, Clone, Copy)]
struct Corps {
    taille: f32,
    approche: f32,
}

impl Corps {
    fn largeur(&self, police: Police, octets: &[u8]) -> f32 {
        octets
            .iter()
            .map(|&b| police.largeur(b) * self.taille + self.approche)
            .sum()
    }
}

/// Style de base d'un paragraphe.
#[derive(Debug, Clone, Copy)]
struct Base {
    police: Police,
    couleur: [u8; 3],
    majuscules: bool,
}

/// Les mots d'un texte. `budget` borne la largeur cumulée produite (le texte au-delà
/// ne tiendrait pas dans la zone) : un paragraphe immense coûte ce que montre la zone.
fn mots(runs: &[Run], base: Base, corps: Corps, budget: f32) -> Vec<Mot> {
    let mut out: Vec<Mot> = Vec::new();
    let mut cumul = 0.0f32;
    for r in runs {
        let st = r.style();
        let encre = Encre {
            police: Police {
                serif: base.police.serif || matches!(r, Run::Math { .. }),
                gras: base.police.gras || st.bold,
                italique: base.police.italique || st.italic || matches!(r, Run::Math { .. }),
            },
            couleur: if matches!(r, Run::Missing { .. }) {
                MISSING_COLOR
            } else {
                base.couleur
            },
            souligne: st.underline,
            barre: st.strike,
        };
        let mut texte = r.plain();
        if base.majuscules {
            texte = texte.to_uppercase();
        }
        // Ajoute un mot et renvoie sa largeur.
        let pousser = |out: &mut Vec<Mot>, octets: Vec<u8>, espace: bool, saut: bool| {
            let largeur = corps.largeur(encre.police, &octets);
            out.push(Mot {
                octets,
                encre,
                largeur,
                espace,
                saut,
            });
            largeur
        };
        let mut mot: Vec<u8> = Vec::new();
        for c in texte.chars() {
            match c {
                '\n' | ' ' | '\t' => {
                    if !mot.is_empty() {
                        cumul += pousser(&mut out, std::mem::take(&mut mot), false, false);
                    }
                    cumul += if c == '\n' {
                        pousser(&mut out, Vec::new(), false, true)
                    } else {
                        pousser(&mut out, vec![b' '], true, false)
                    };
                }
                c => {
                    if let Some(b) = winansi(c) {
                        mot.push(b);
                        // Mot démesuré : coupé en morceaux pour respecter le budget.
                        if mot.len() >= 256 {
                            cumul += pousser(&mut out, std::mem::take(&mut mot), false, false);
                        }
                    }
                }
            }
            if cumul > budget {
                return out;
            }
        }
        if !mot.is_empty() {
            cumul += pousser(&mut out, mot, false, false);
        }
        if cumul > budget {
            break;
        }
    }
    out
}

/// Répartit les mots en lignes d'au plus `max` points. Les morceaux collés (un
/// mot dont une partie est en gras, par exemple) ne sont jamais séparés, sauf
/// s'ils sont plus larges que la ligne : ils sont alors coupés caractère par caractère.
fn couper(mots: Vec<Mot>, max: f32, corps: Corps) -> Vec<Ligne> {
    let max = max.max(1.0);
    let mut lignes: Vec<Ligne> = Vec::new();
    let mut cur = Ligne::default();
    let fin = |cur: &mut Ligne, lignes: &mut Vec<Ligne>| {
        while cur.mots.last().is_some_and(|m| m.espace) {
            if let Some(m) = cur.mots.pop() {
                cur.largeur -= m.largeur;
            }
        }
        cur.largeur = cur.largeur.max(0.0);
        lignes.push(std::mem::take(cur));
    };
    let mut mots = mots.into_iter().peekable();
    while let Some(m) = mots.next() {
        if m.saut {
            fin(&mut cur, &mut lignes);
            continue;
        }
        if m.espace {
            if !cur.mots.is_empty() {
                cur.largeur += m.largeur;
                cur.mots.push(m);
            }
            continue;
        }
        let mut groupe = vec![m];
        while let Some(suite) = mots.next_if(|x| !x.espace && !x.saut) {
            groupe.push(suite);
        }
        let largeur: f32 = groupe.iter().map(|m| m.largeur).sum();
        let plein = |cur: &Ligne, w: f32| {
            cur.largeur + w > max + 0.01 && cur.mots.iter().any(|x| !x.espace)
        };
        if largeur <= max {
            if plein(&cur, largeur) {
                fin(&mut cur, &mut lignes);
            }
            cur.largeur += largeur;
            cur.mots.extend(groupe);
            continue;
        }
        for m in groupe {
            let morceaux = if m.largeur > max {
                scinder(&m, max, corps)
            } else {
                vec![m]
            };
            for p in morceaux {
                if plein(&cur, p.largeur) {
                    fin(&mut cur, &mut lignes);
                }
                cur.largeur += p.largeur;
                cur.mots.push(p);
            }
        }
    }
    if !cur.mots.is_empty() || lignes.is_empty() {
        fin(&mut cur, &mut lignes);
    }
    lignes
}

/// Coupe un mot plus large que la ligne, caractère par caractère.
fn scinder(m: &Mot, max: f32, corps: Corps) -> Vec<Mot> {
    let mut out = Vec::new();
    let mut octets = Vec::new();
    let mut largeur = 0.0;
    for &b in &m.octets {
        let w = corps.largeur(m.encre.police, &[b]);
        if largeur + w > max && !octets.is_empty() {
            out.push(Mot {
                octets: std::mem::take(&mut octets),
                largeur,
                ..m.clone()
            });
            largeur = 0.0;
        }
        octets.push(b);
        largeur += w;
    }
    out.push(Mot {
        octets,
        largeur,
        ..m.clone()
    });
    out
}

/// Largeurs des colonnes d'un tableau, en points, pour une largeur totale donnée :
/// chaque colonne reçoit si possible la largeur de son mot le plus long, puis le
/// reste se partage au prorata du texte. Renvoie aussi la largeur naturelle totale
/// (texte de chaque colonne sur une seule ligne), marges `px` comprises.
pub(crate) fn colonnes(
    t: &Table,
    police: Police,
    taille: f32,
    px: f32,
    largeur: f32,
) -> (Vec<f32>, f32) {
    let cols = t.columns().max(1);
    let corps = Corps {
        taille,
        approche: 0.0,
    };
    let mut naturelles = vec![2.0 * px; cols];
    let mut minimales = vec![2.0 * px; cols];
    // Les premiers rangs suffisent à estimer les proportions.
    for (ri, row) in t.rows.iter().enumerate().take(200) {
        let base = Base {
            police: Police {
                gras: police.gras || (ri == 0 && t.header),
                ..police
            },
            couleur: [0, 0, 0],
            majuscules: false,
        };
        for (c, cell) in row.iter().enumerate().take(cols) {
            let m = mots(cell, base, corps, largeur * 2.0);
            let total: f32 = m.iter().map(|m| m.largeur).sum();
            let long = m
                .iter()
                .filter(|m| !m.espace)
                .map(|m| m.largeur)
                .fold(0.0f32, f32::max);
            naturelles[c] = naturelles[c].max(total + 2.0 * px);
            minimales[c] = minimales[c].max(long.min(largeur) + 2.0 * px);
        }
    }
    let (somme_nat, somme_min): (f32, f32) = (naturelles.iter().sum(), minimales.iter().sum());
    let largeurs = if somme_nat <= 0.0 {
        vec![largeur / cols as f32; cols]
    } else if somme_nat <= largeur {
        naturelles.iter().map(|w| w * largeur / somme_nat).collect()
    } else if somme_min < largeur && somme_nat > somme_min {
        let k = (largeur - somme_min) / (somme_nat - somme_min);
        minimales
            .iter()
            .zip(&naturelles)
            .map(|(mi, na)| mi + (na - mi) * k)
            .collect()
    } else {
        minimales.iter().map(|w| w * largeur / somme_min).collect()
    };
    (largeurs, somme_nat)
}

// ---------------------------------------------------------------------------------
// Composition d'une zone de texte
// ---------------------------------------------------------------------------------

/// Élément dessiné, positionné par rapport au haut de sa zone.
enum Prim {
    Ligne {
        ligne: Ligne,
        x: f32,
        haut: f32,
        hauteur: f32,
        corps: Corps,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        couleur: [u8; 3],
    },
    /// Puce dessinée (pas de glyphe : « ◦ » n'existe pas en WinAnsi), alignée
    /// sur la première ligne de son élément.
    Puce {
        x: f32,
        haut: f32,
        hauteur: f32,
        taille: f32,
        profondeur: u8,
        couleur: [u8; 3],
    },
}

impl Prim {
    fn bas(&self) -> f32 {
        match self {
            Prim::Ligne { haut, hauteur, .. } | Prim::Puce { haut, hauteur, .. } => haut + hauteur,
            Prim::Rect { y, h, .. } => y + h,
        }
    }
}

/// Habillage d'une zone : police, couleurs et rythme de son texte.
#[derive(Debug, Clone, Copy)]
struct Habillage {
    base: Base,
    pal: Palette,
    interligne: f32,
    /// Approche, en em (négative pour resserrer les titres).
    approche: f32,
    /// Alignement des paragraphes sans alignement propre.
    align: Align,
    /// Zone de titre : pas d'espace entre les lignes, une ligne vide compte.
    titre: bool,
    /// Contenu de la disposition Citation : les citations `>` n'ont pas de filet.
    citation: bool,
}

struct Compo<'a> {
    hab: &'a Habillage,
    largeur: f32,
    taille: f32,
    prims: Vec<Prim>,
    /// Hauteur au-delà de laquelle la composition s'arrête : le reste ne serait
    /// pas dessiné, et le coût reste borné par la taille de la zone.
    limite: f32,
}

impl Compo<'_> {
    fn corps(&self, taille: f32, approche: f32) -> Corps {
        Corps {
            taille,
            approche: approche * taille,
        }
    }

    /// Place un paragraphe à partir de `y` ; renvoie sa hauteur.
    #[allow(clippy::too_many_arguments)]
    fn paragraphe(
        &mut self,
        runs: &[Run],
        base: Base,
        y: f32,
        x: f32,
        largeur: f32,
        corps: Corps,
        interligne: f32,
        align: Align,
    ) -> f32 {
        let lh = corps.taille * interligne;
        let possibles = ((self.limite - y).max(0.0) / lh.max(0.1)).min(1e6) + 2.0;
        let budget = largeur * 2.0 * possibles;
        let lignes = couper(mots(runs, base, corps, budget), largeur, corps);
        let n = lignes.len();
        for (k, l) in lignes.into_iter().enumerate() {
            let dx = match align {
                Align::Center => (largeur - l.largeur) / 2.0,
                Align::Right => largeur - l.largeur,
                _ => 0.0,
            }
            .max(0.0);
            self.prims.push(Prim::Ligne {
                ligne: l,
                x: x + dx,
                haut: y + k as f32 * lh,
                hauteur: lh,
                corps,
            });
        }
        n as f32 * lh
    }

    fn tableau(&mut self, t: &Table, y: f32) -> f32 {
        let hab = self.hab;
        let pal = hab.pal;
        let taille = self.taille * 0.85;
        let corps = self.corps(taille, 0.0);
        let (px, py) = (0.75 * taille, 0.45 * taille);
        let lh = taille * 1.25;
        let base = |entete: bool| Base {
            police: Police {
                gras: entete,
                ..hab.base.police
            },
            couleur: if entete { pal.title } else { pal.text },
            majuscules: false,
        };
        let largeurs = colonnes(t, hab.base.police, taille, px, self.largeur).0;
        let mut h = 0.0;
        for (ri, row) in t.rows.iter().enumerate() {
            if y + h > self.limite {
                break;
            }
            let entete = ri == 0 && t.header;
            let mut x = 0.0;
            let mut cellules = Vec::new();
            let mut lignes_max = 1usize;
            for (c, &w) in largeurs.iter().enumerate() {
                let runs = row.get(c).map(Vec::as_slice).unwrap_or(&[]);
                let possibles = ((self.limite - y - h).max(0.0) / lh).min(1e6) + 2.0;
                let budget = w * 2.0 * possibles;
                let lignes = couper(mots(runs, base(entete), corps, budget), w - 2.0 * px, corps);
                lignes_max = lignes_max.max(lignes.len());
                cellules.push((x, w, t.align(c), lignes));
                x += w;
            }
            let hr = lignes_max as f32 * lh + 2.0 * py;
            let top = y + h;
            if entete {
                self.prims.push(Prim::Rect {
                    x: 0.0,
                    y: top,
                    w: self.largeur,
                    h: hr,
                    couleur: pal.tint(),
                });
            }
            for (x, w, align, lignes) in cellules {
                for (k, l) in lignes.into_iter().enumerate() {
                    let dx = match align {
                        Align::Center => (w - 2.0 * px - l.largeur) / 2.0,
                        Align::Right => w - 2.0 * px - l.largeur,
                        _ => 0.0,
                    }
                    .max(0.0);
                    self.prims.push(Prim::Ligne {
                        ligne: l,
                        x: x + px + dx,
                        haut: top + py + k as f32 * lh,
                        hauteur: lh,
                        corps,
                    });
                }
            }
            let (epais, couleur) = if entete {
                (0.12 * taille, pal.accent)
            } else {
                (0.06 * taille, pal.rule())
            };
            self.prims.push(Prim::Rect {
                x: 0.0,
                y: top + hr,
                w: self.largeur,
                h: epais,
                couleur,
            });
            h += hr + epais;
        }
        h
    }

    fn composer(&mut self, paras: &[Para]) -> f32 {
        let hab = self.hab;
        let pal = hab.pal;
        let taille = self.taille;
        let largeur = self.largeur;
        let marge = if hab.titre { 0.0 } else { 0.4 * taille };
        let corps = self.corps(taille, hab.approche);
        let mut y = 0.0f32;
        let mut attente = 0.0f32;
        for p in paras {
            if y > self.limite {
                break;
            }
            match p {
                Para::Text { runs, .. } if runs.iter().all(|r| r.plain().is_empty()) => {
                    y += attente;
                    attente = 0.0;
                    y += if hab.titre {
                        taille * hab.interligne
                    } else {
                        0.45 * taille
                    };
                }
                Para::Text { align, runs } => {
                    y += attente;
                    let a = if *align == Align::Left {
                        hab.align
                    } else {
                        *align
                    };
                    y += self.paragraphe(runs, hab.base, y, 0.0, largeur, corps, hab.interligne, a);
                    attente = marge;
                }
                Para::Heading { level, runs } => {
                    y += attente;
                    let f = [1.3, 1.15, 1.05][((*level).clamp(1, 3) - 1) as usize];
                    let base = Base {
                        police: Police {
                            serif: pal.serif_titles,
                            gras: true,
                            italique: hab.base.police.italique,
                        },
                        couleur: pal.title,
                        majuscules: false,
                    };
                    let c = self.corps(taille * f, -0.01);
                    y += self.paragraphe(runs, base, y, 0.0, largeur, c, 1.15, Align::Left);
                    attente = marge;
                }
                Para::Item {
                    number,
                    depth,
                    runs,
                } => {
                    y += attente;
                    let pas = 1.25 * taille;
                    let d = (*depth).min(5) as f32;
                    let xt = (pas * (d + 1.0)).min(largeur * 0.5);
                    let h = self.paragraphe(
                        runs,
                        hab.base,
                        y,
                        xt,
                        largeur - xt,
                        corps,
                        hab.interligne,
                        Align::Left,
                    );
                    let lh = taille * hab.interligne;
                    let x = (pas * d).min(largeur * 0.5);
                    match number {
                        Some(_) => {
                            let base = Base {
                                police: Police {
                                    gras: true,
                                    ..hab.base.police
                                },
                                couleur: pal.accent,
                                majuscules: false,
                            };
                            let run = Run::Text {
                                text: crate::html::puce(*number, *depth),
                                style: Default::default(),
                            };
                            let c = self.corps(taille, 0.0);
                            let label = couper(mots(&[run], base, c, largeur), largeur, c);
                            if let Some(l) = label.into_iter().next() {
                                self.prims.push(Prim::Ligne {
                                    ligne: l,
                                    x,
                                    haut: y,
                                    hauteur: lh,
                                    corps: c,
                                });
                            }
                        }
                        None => self.prims.push(Prim::Puce {
                            x,
                            haut: y,
                            hauteur: lh,
                            taille,
                            profondeur: *depth,
                            couleur: pal.accent,
                        }),
                    }
                    y += h.max(lh);
                    attente = marge;
                }
                Para::Quote { runs } => {
                    y += attente;
                    let base = Base {
                        police: Police {
                            italique: true,
                            ..hab.base.police
                        },
                        couleur: pal.title,
                        majuscules: false,
                    };
                    let (trait_, retrait) = if hab.citation {
                        (0.0, 0.0)
                    } else {
                        (0.14 * taille, 0.94 * taille)
                    };
                    let h = self.paragraphe(
                        runs,
                        base,
                        y,
                        retrait,
                        largeur - retrait,
                        corps,
                        hab.interligne,
                        Align::Left,
                    );
                    if trait_ > 0.0 {
                        self.prims.push(Prim::Rect {
                            x: 0.0,
                            y,
                            w: trait_,
                            h,
                            couleur: pal.accent,
                        });
                    }
                    y += h;
                    attente = marge;
                }
                Para::Math { .. } => {
                    y += attente;
                    let base = Base {
                        police: Police {
                            serif: true,
                            italique: true,
                            gras: false,
                        },
                        ..hab.base
                    };
                    let run = Run::Text {
                        text: p.plain(),
                        style: Default::default(),
                    };
                    y += self.paragraphe(
                        &[run],
                        base,
                        y,
                        0.0,
                        largeur,
                        corps,
                        hab.interligne,
                        Align::Center,
                    );
                    attente = marge;
                }
                Para::Table(t) => {
                    y += attente;
                    y += self.tableau(t, y);
                    attente = marge;
                }
                Para::Rule => {
                    y += attente;
                    let h = 0.06 * taille;
                    self.prims.push(Prim::Rect {
                        x: 0.0,
                        y,
                        w: largeur,
                        h,
                        couleur: pal.rule(),
                    });
                    y += h;
                    attente = marge;
                }
                Para::PageBreak => {}
            }
        }
        y
    }
}

// ---------------------------------------------------------------------------------
// Dessin
// ---------------------------------------------------------------------------------

fn n(v: f32) -> String {
    let s = format!("{:.2}", if v.is_finite() { v } else { 0.0 });
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

fn rgb(c: [u8; 3]) -> String {
    let f = |v: u8| {
        let s = format!("{:.3}", v as f32 / 255.0);
        let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        if s.is_empty() { "0".into() } else { s }
    };
    format!("{} {} {}", f(c[0]), f(c[1]), f(c[2]))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

/// Écrit une ligne dont le haut est à `haut` points sous le haut de la page.
fn dessiner_ligne(
    out: &mut String,
    l: &Ligne,
    x: f32,
    haut: f32,
    hauteur: f32,
    corps: Corps,
    ph: f32,
) {
    let t = corps.taille;
    let base = haut + (hauteur - HAUTEUR_TEXTE * t) / 2.0 + ASCENDANTE * t;
    let y = ph - base;
    let mut cx = x;
    let mut i = 0;
    while i < l.mots.len() {
        let encre = l.mots[i].encre;
        let mut octets = Vec::new();
        let mut w = 0.0;
        while i < l.mots.len() && l.mots[i].encre == encre {
            octets.extend_from_slice(&l.mots[i].octets);
            w += l.mots[i].largeur;
            i += 1;
        }
        if octets.is_empty() {
            continue;
        }
        let _ = writeln!(
            out,
            "BT /F{} {} Tf {} Tc {} rg 1 0 0 1 {} {} Tm <{}> Tj ET",
            encre.police.index() + 1,
            n(t),
            n(corps.approche),
            rgb(encre.couleur),
            n(cx),
            n(y),
            hex(&octets)
        );
        let epais = (t * 0.06).max(0.4);
        for (actif, dy) in [(encre.souligne, -0.12 * t), (encre.barre, 0.3 * t)] {
            if actif {
                let _ = writeln!(
                    out,
                    "{} rg {} {} {} {} re f",
                    rgb(encre.couleur),
                    n(cx),
                    n(y + dy - epais / 2.0),
                    n(w),
                    n(epais)
                );
            }
        }
        cx += w;
    }
}

fn dessiner(out: &mut String, p: &Prim, x0: f32, y0: f32, ph: f32) {
    match p {
        Prim::Ligne {
            ligne,
            x,
            haut,
            hauteur,
            corps,
        } => dessiner_ligne(out, ligne, x0 + x, y0 + haut, *hauteur, *corps, ph),
        Prim::Rect {
            x,
            y,
            w,
            h,
            couleur,
        } => {
            let _ = writeln!(
                out,
                "{} rg {} {} {} {} re f",
                rgb(*couleur),
                n(x0 + x),
                n(ph - (y0 + y + h)),
                n(*w),
                n(*h)
            );
        }
        Prim::Puce {
            x,
            haut,
            hauteur,
            taille: t,
            profondeur,
            couleur,
        } => {
            // Centre à mi-hauteur des minuscules de la première ligne.
            let base = y0 + haut + (hauteur - HAUTEUR_TEXTE * t) / 2.0 + ASCENDANTE * t;
            let cy = ph - (base - 0.3 * t);
            let x = x0 + x;
            match profondeur % 3 {
                0 => {
                    let _ = writeln!(
                        out,
                        "{} rg {} f",
                        rgb(*couleur),
                        cercle(x + 0.2 * t, cy, 0.135 * t)
                    );
                }
                1 => {
                    let e = 0.075 * t;
                    let _ = writeln!(
                        out,
                        "{} rg {} {} {} {} re f",
                        rgb(*couleur),
                        n(x + 0.04 * t),
                        n(cy - e / 2.0),
                        n(0.42 * t),
                        n(e)
                    );
                }
                _ => {
                    let _ = writeln!(
                        out,
                        "{} RG {} w {} S",
                        rgb(*couleur),
                        n(0.055 * t),
                        cercle(x + 0.2 * t, cy, 0.1 * t)
                    );
                }
            }
        }
    }
}

/// Chemin d'un cercle (quatre arcs de Bézier).
fn cercle(cx: f32, cy: f32, r: f32) -> String {
    ellipse(cx, cy, r, r)
}

/// Chemin d'une ellipse (quatre arcs de Bézier).
fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> String {
    let (kx, ky) = (0.552_284_8 * rx, 0.552_284_8 * ry);
    let (r, k) = ((rx, ry), (kx, ky));
    ellipse_xy(cx, cy, r, k)
}

fn ellipse_xy(cx: f32, cy: f32, r: (f32, f32), k: (f32, f32)) -> String {
    let (rx, ry) = r;
    let (kx, ky) = k;
    format!(
        "{} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c h",
        n(cx + rx),
        n(cy),
        n(cx + rx),
        n(cy + ky),
        n(cx + kx),
        n(cy + ry),
        n(cx),
        n(cy + ry),
        n(cx - kx),
        n(cy + ry),
        n(cx - rx),
        n(cy + ky),
        n(cx - rx),
        n(cy),
        n(cx - rx),
        n(cy - ky),
        n(cx - kx),
        n(cy - ry),
        n(cx),
        n(cy - ry),
        n(cx + kx),
        n(cy - ry),
        n(cx + rx),
        n(cy - ky),
        n(cx + rx),
        n(cy)
    )
}

/// Compose une zone, réduit le texte jusqu'à ce qu'il tienne, puis le dessine.
#[allow(clippy::too_many_arguments)]
fn zone(
    out: &mut String,
    paras: &[Para],
    f: Frame,
    taille: f32,
    hab: &Habillage,
    ancre: Anchor,
    pw: f32,
    ph: f32,
) {
    if paras.is_empty() {
        return;
    }
    let (x, y, w, h) = f.scaled(pw, ph);
    let mut taille = if taille.is_finite() {
        taille.max(1.0)
    } else {
        12.0
    };
    let composer = |taille: f32| {
        let mut c = Compo {
            hab,
            largeur: w.max(1.0),
            taille,
            prims: Vec::new(),
            limite: h + 1.0,
        };
        let haut = c.composer(paras);
        (c.prims, haut)
    };
    let (mut prims, mut haut) = composer(taille);
    for _ in 0..14 {
        if haut <= h + 0.01 {
            break;
        }
        taille *= 0.92;
        (prims, haut) = composer(taille);
    }
    let dy = match ancre {
        Anchor::Top => 0.0,
        Anchor::Middle => ((h - haut) / 2.0).max(0.0),
        Anchor::Bottom => (h - haut).max(0.0),
    };
    for p in &prims {
        if p.bas() + dy <= h + 0.5 {
            dessiner(out, p, x, y + dy, ph);
        }
    }
}

/// Une ligne isolée (pied de page, numéro), centrée verticalement dans `f`.
#[allow(clippy::too_many_arguments)]
fn ligne_seule(
    out: &mut String,
    runs: &[Run],
    f: Frame,
    base: Base,
    corps: Corps,
    droite: bool,
    pw: f32,
    ph: f32,
) {
    let (x, y, w, h) = f.scaled(pw, ph);
    if let Some(l) = couper(mots(runs, base, corps, w * 2.0), w, corps)
        .into_iter()
        .next()
    {
        let x = if droite { x + w - l.largeur } else { x };
        dessiner_ligne(out, &l, x, y, h, corps, ph);
    }
}

/// Filet décoratif aux bouts arrondis.
fn filet(out: &mut String, f: Frame, couleur: [u8; 3], pw: f32, ph: f32) {
    let (x, y, w, h) = f.scaled(pw, ph);
    let r = h.min(w) / 2.0;
    let ym = ph - (y + h / 2.0);
    let _ = writeln!(
        out,
        "{} RG {} w 1 J {} {} m {} {} l S",
        rgb(couleur),
        n(h.min(w)),
        n(x + r),
        n(ym),
        n(x + w - r),
        n(ym)
    );
}

/// Rectangle aux coins arrondis de rayon `r` (`y` : bas du rectangle).
fn rect_arrondi(x: f32, y: f32, w: f32, h: f32, r: f32) -> String {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let k = r * (1.0 - 0.552_284_8);
    let (x2, y2) = (x + w, y + h);
    format!(
        "{} {} m {} {} l {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c h",
        n(x + r),
        n(y),
        n(x2 - r),
        n(y),
        n(x2 - k),
        n(y),
        n(x2),
        n(y + k),
        n(x2),
        n(y + r),
        n(x2),
        n(y2 - r),
        n(x2),
        n(y2 - k),
        n(x2 - k),
        n(y2),
        n(x2 - r),
        n(y2),
        n(x + r),
        n(y2),
        n(x + k),
        n(y2),
        n(x),
        n(y2 - k),
        n(x),
        n(y2 - r),
        n(x),
        n(y + r),
        n(x),
        n(y + k),
        n(x + k),
        n(y),
        n(x + r),
        n(y)
    )
}

/// Un objet de la diapositive (image ou forme) en tracés PDF.
fn objet_pdf(
    out: &mut String,
    o: &Object,
    texte: &[Para],
    pal: Palette,
    images: &BTreeMap<String, usize>,
    pw: f32,
    ph: f32,
) {
    let (x, y, w, h) = o.frame.scaled(pw, ph);
    // Coordonnées PDF : origine en bas à gauche.
    let bas = ph - (y + h);
    match o.kind {
        ObjectKind::Image => {
            if let Some(num) = images.get(&o.media) {
                let _ = writeln!(
                    out,
                    "q {} 0 0 {} {} {} cm /Im{num} Do Q",
                    n(w),
                    n(h),
                    n(x),
                    n(bas)
                );
            }
        }
        ObjectKind::Line | ObjectKind::Arrow => {
            let Some(c) = o.stroke else {
                return;
            };
            let ((x1, y1), (x2, y2)) = o.line_points();
            let a = (x1 * pw, ph - y1 * ph);
            let b = (x2 * pw, ph - y2 * ph);
            let sw = o.stroke_width.max(0.5);
            let fin = if o.kind == ObjectKind::Arrow {
                let (fin, tri) = arrow_head(a, b, sw);
                let _ = writeln!(
                    out,
                    "q {} rg {} {} m {} {} l {} {} l h f Q",
                    rgb(c),
                    n(tri[0].0),
                    n(tri[0].1),
                    n(tri[1].0),
                    n(tri[1].1),
                    n(tri[2].0),
                    n(tri[2].1)
                );
                fin
            } else {
                b
            };
            let _ = writeln!(
                out,
                "q {} RG {} w 1 J {} {} m {} {} l S Q",
                rgb(c),
                n(sw),
                n(a.0),
                n(a.1),
                n(fin.0),
                n(fin.1)
            );
        }
        ObjectKind::Rect | ObjectKind::RoundRect | ObjectKind::Ellipse => {
            let trait_ = o.stroke.filter(|_| o.stroke_width > 0.0);
            let op = match (o.fill.is_some(), trait_.is_some()) {
                (true, true) => "B",
                (true, false) => "f",
                (false, true) => "S",
                (false, false) => "n",
            };
            let mut c = String::from("q ");
            if let Some(f) = o.fill {
                let _ = write!(c, "{} rg ", rgb(f));
            }
            if let Some(t) = trait_ {
                let _ = write!(c, "{} RG {} w 1 j ", rgb(t), n(o.stroke_width));
            }
            let chemin = match o.kind {
                ObjectKind::Ellipse => ellipse(x + w / 2.0, bas + h / 2.0, w / 2.0, h / 2.0),
                ObjectKind::RoundRect => rect_arrondi(x, bas, w, h, w.min(h) * Object::CORNER),
                _ => format!("{} {} {} {} re", n(x), n(bas), n(w), n(h)),
            };
            let _ = writeln!(out, "{c}{chemin} {op} Q");
            if !texte.is_empty() {
                let hab = Habillage {
                    base: Base {
                        police: Police::default(),
                        couleur: o.text_ink(&pal),
                        majuscules: false,
                    },
                    pal,
                    interligne: 1.2,
                    approche: 0.0,
                    align: Align::Center,
                    titre: true,
                    citation: false,
                };
                let zone_texte = o.frame.inset(0.012, 0.012 * pw / ph);
                zone(
                    out,
                    texte,
                    zone_texte,
                    o.text_size * ph,
                    &hab,
                    Anchor::Middle,
                    pw,
                    ph,
                );
            }
        }
    }
}

/// Les images de la présentation en objets PDF, numérotés à partir de
/// `premier` : JPEG tels quels (DCTDecode), PNG décodés en RVB compressé
/// (FlateDecode) avec leur transparence en masque doux (SMask). Renvoie les
/// objets et le numéro d'objet de chaque image.
fn images_pdf(deck: &Deck, premier: usize) -> (Vec<Vec<u8>>, BTreeMap<String, usize>) {
    let mut objs: Vec<Vec<u8>> = Vec::new();
    let mut nums = BTreeMap::new();
    let flux = |dict: String, data: &[u8]| {
        let mut o = format!("{dict} /Length {} >>\nstream\n", data.len()).into_bytes();
        o.extend_from_slice(data);
        o.extend_from_slice(b"\nendstream");
        o
    };
    for id in deck.used_media() {
        let Some(m) = deck.media.get(id) else {
            continue;
        };
        let num = premier + objs.len();
        let format = format_image(&m.data);
        let composantes = if format == Some(image::ImageFormat::Jpeg) {
            jpeg_composantes(&m.data)
        } else {
            None
        };
        if let (Some(c @ (1 | 3)), Ok((w, h))) = (
            composantes,
            image::ImageReader::with_format(
                std::io::Cursor::new(&m.data[..]),
                image::ImageFormat::Jpeg,
            )
            .into_dimensions(),
        ) {
            let espace = if c == 1 { "/DeviceGray" } else { "/DeviceRGB" };
            objs.push(flux(
                format!(
                    "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace {espace} /BitsPerComponent 8 /Filter /DCTDecode"
                ),
                &m.data,
            ));
            nums.insert(id.to_string(), num);
            continue;
        }
        let Some(img) = format.and_then(|f| image::load_from_memory_with_format(&m.data, f).ok())
        else {
            continue;
        };
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let mut rvb = Vec::with_capacity((w * h * 3) as usize);
        let mut alpha = Vec::with_capacity((w * h) as usize);
        for p in rgba.pixels() {
            rvb.extend_from_slice(&p.0[..3]);
            alpha.push(p.0[3]);
        }
        let transparente = alpha.iter().any(|&a| a != 255);
        let masque = if transparente {
            format!(" /SMask {} 0 R", num + 1)
        } else {
            String::new()
        };
        objs.push(flux(
            format!(
                "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode{masque}"
            ),
            &miniz_oxide::deflate::compress_to_vec_zlib(&rvb, 6),
        ));
        if transparente {
            objs.push(flux(
                format!(
                    "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode"
                ),
                &miniz_oxide::deflate::compress_to_vec_zlib(&alpha, 6),
            ));
        }
        nums.insert(id.to_string(), num);
    }
    (objs, nums)
}

fn page(r: &Rendu, s: &Slide, images: &BTreeMap<String, usize>, pw: f32, ph: f32) -> String {
    let pal = r.palette;
    let b = &r.boxes;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} rg 0 0 {} {} re f",
        rgb(pal.background),
        n(pw),
        n(ph)
    );
    if let Some(f) = b.accent {
        filet(&mut out, f, pal.accent, pw, ph);
    }
    let titres = |couleur, gras| Base {
        police: Police {
            serif: pal.serif_titles,
            gras,
            italique: false,
        },
        couleur,
        majuscules: false,
    };
    let align = if b.centered {
        Align::Center
    } else {
        Align::Left
    };
    let grand = matches!(s.layout, Layout::Title | Layout::Section);
    if let Some(f) = b.title {
        let hab = Habillage {
            base: titres(pal.title, true),
            pal,
            interligne: if grand { 1.05 } else { TITLE_LINE_HEIGHT },
            approche: if grand { -0.03 } else { -0.02 },
            align,
            titre: true,
            citation: false,
        };
        let t = title_size(s.layout) * ph;
        zone(&mut out, &r.title, f, t, &hab, b.title_anchor, pw, ph);
    }
    if let Some(f) = b.subtitle {
        let citation = s.layout == Layout::Quote;
        let mut base = titres(pal.muted, false);
        let mut t = subtitle_size(s.layout) * ph;
        if citation {
            base.police.serif = false;
            base.majuscules = true;
            t *= 0.8;
        }
        let hab = Habillage {
            base,
            pal,
            interligne: 1.3,
            approche: if citation { 0.12 } else { 0.0 },
            align,
            titre: true,
            citation: false,
        };
        zone(&mut out, &r.subtitle, f, t, &hab, b.subtitle_anchor, pw, ph);
    }
    let citation = s.layout == Layout::Quote;
    let hab = Habillage {
        base: Base {
            police: Police {
                serif: citation,
                gras: false,
                italique: citation,
            },
            couleur: if citation { pal.title } else { pal.text },
            majuscules: false,
        },
        pal,
        interligne: if citation { 1.22 } else { BODY_LINE_HEIGHT },
        approche: 0.0,
        align: Align::Left,
        titre: false,
        citation,
    };
    let t = body_size(s.layout) * ph;
    for (f, paras) in [(b.body, &r.body), (b.body2, &r.body2)] {
        if let Some(f) = f {
            zone(&mut out, paras, f, t, &hab, b.body_anchor, pw, ph);
        }
    }
    for (o, texte) in s.objects.iter().zip(&r.objets) {
        objet_pdf(&mut out, o, texte, pal, images, pw, ph);
    }
    let pied = Base {
        police: Police::default(),
        couleur: pal.muted,
        majuscules: false,
    };
    let corps = Corps {
        taille: FOOTER_SIZE * ph,
        approche: 0.03 * FOOTER_SIZE * ph,
    };
    if !r.footer.is_empty() {
        ligne_seule(
            &mut out,
            &r.footer,
            footer_frame(),
            pied,
            corps,
            false,
            pw,
            ph,
        );
    }
    if let Some(num) = r.number {
        let run = Run::Text {
            text: num.to_string(),
            style: Default::default(),
        };
        let gras = Base {
            police: Police {
                gras: true,
                ..Police::default()
            },
            ..pied
        };
        ligne_seule(&mut out, &[run], number_frame(), gras, corps, true, pw, ph);
    }
    out
}

// ---------------------------------------------------------------------------------
// Fichier
// ---------------------------------------------------------------------------------

/// Chaîne PDF en UTF-16 (métadonnées).
fn utf16(s: &str) -> String {
    let mut out = String::from("<FEFF");
    for u in s.encode_utf16() {
        let _ = write!(out, "{u:04X}");
    }
    out.push('>');
    out
}

pub(crate) fn pdf(deck: &Deck) -> Vec<u8> {
    let pw = LARGEUR_PAGE;
    let ph = hauteur_diapo(deck);
    let rendus = deck.rendus();
    let slides = deck.exported();
    let np = rendus.len();
    // Les images suivent les pages : objets 12 + 2 × pages et au-delà.
    let (images, nums) = images_pdf(deck, 12 + 2 * np);
    let pages: Vec<(f32, f32, String)> = rendus
        .iter()
        .zip(slides.iter())
        .map(|(r, s)| (pw, ph, page(r, s, &nums, pw, ph)))
        .collect();
    assembler(deck, &pages, images, &nums)
}

/// Hauteur d'une diapositive en points (largeur 720 pt).
fn hauteur_diapo(deck: &Deck) -> f32 {
    (LARGEUR_PAGE / deck.aspect.ratio() * 1000.0).round() / 1000.0
}

/// Écrit le fichier PDF : pages `(largeur, hauteur, contenu)`, images déjà
/// numérotées à partir de `12 + 2 × pages`.
fn assembler(
    deck: &Deck,
    pages: &[(f32, f32, String)],
    images: Vec<Vec<u8>>,
    nums: &BTreeMap<String, usize>,
) -> Vec<u8> {
    let np = pages.len();
    let xobjets: String = if nums.is_empty() {
        String::new()
    } else {
        let refs: String = nums
            .values()
            .map(|num| format!("/Im{num} {num} 0 R "))
            .collect();
        format!(" /XObject << {refs}>>")
    };

    // 1 catalogue, 2 arbre des pages, 3 infos, 4 à 11 polices, puis (page, contenu).
    let mut objs: Vec<Vec<u8>> = Vec::new();
    objs.push(
        b"<< /Type /Catalog /Pages 2 0 R /Lang (fr-FR) /ViewerPreferences << /DisplayDocTitle true >> >>"
            .to_vec(),
    );
    let kids: String = (0..np).map(|i| format!("{} 0 R ", 12 + i * 2)).collect();
    objs.push(format!("<< /Type /Pages /Kids [{kids}] /Count {np} >>").into_bytes());
    let titre = match deck.title.trim() {
        "" => "Présentation",
        t => t,
    };
    objs.push(
        format!(
            "<< /Title {} /Producer (Varium) /Creator (Varium) >>",
            utf16(titre)
        )
        .into_bytes(),
    );
    for p in POLICES {
        objs.push(
            format!("<< /Type /Font /Subtype /Type1 /BaseFont /{p} /Encoding /WinAnsiEncoding >>")
                .into_bytes(),
        );
    }
    let polices: String = (0..POLICES.len())
        .map(|i| format!("/F{} {} 0 R ", i + 1, 4 + i))
        .collect();
    for (i, (pw, ph, contenu)) in pages.iter().enumerate() {
        objs.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << {polices}>>{xobjets} >> /Contents {} 0 R >>",
                n(*pw),
                n(*ph),
                13 + i * 2
            )
            .into_bytes(),
        );
        let mut c = format!("<< /Length {} >>\nstream\n", contenu.len()).into_bytes();
        c.extend_from_slice(contenu.as_bytes());
        c.extend_from_slice(b"endstream");
        objs.push(c);
    }
    objs.extend(images);

    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut positions = Vec::with_capacity(objs.len());
    for (i, o) in objs.iter().enumerate() {
        positions.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for p in &positions {
        out.extend_from_slice(format!("{p:010} 00000 n \n").as_bytes());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codage_winansi() {
        assert_eq!(winansi('é'), Some(0xE9));
        assert_eq!(winansi('€'), Some(0x80));
        assert_eq!(winansi('œ'), Some(0x9C));
        assert_eq!(winansi('’'), Some(0x92));
        assert_eq!(winansi('\u{202F}'), Some(0xA0));
        assert_eq!(winansi('😀'), Some(b'?'));
        assert_eq!(winansi('\u{200B}'), None);
        assert_eq!(winansi('\u{1}'), None);
    }

    #[test]
    fn largeurs_helvetica() {
        let p = Police::default();
        assert_eq!(p.largeur(b'a'), 0.556);
        assert_eq!(p.largeur(b' '), 0.278);
    }

    #[test]
    fn coupure_des_lignes() {
        let corps = Corps {
            taille: 10.0,
            approche: 0.0,
        };
        let base = Base {
            police: Police::default(),
            couleur: [0, 0, 0],
            majuscules: false,
        };
        let runs = [Run::Text {
            text: "un deux trois quatre cinq six sept huit".into(),
            style: Default::default(),
        }];
        let lignes = couper(mots(&runs, base, corps, f32::INFINITY), 60.0, corps);
        assert!(lignes.len() > 2);
        assert!(lignes.iter().all(|l| l.largeur <= 60.01));
        // Un mot immense est coupé, jamais perdu.
        let long = [Run::Text {
            text: "x".repeat(500),
            style: Default::default(),
        }];
        let lignes = couper(mots(&long, base, corps, f32::INFINITY), 50.0, corps);
        let total: usize = lignes
            .iter()
            .flat_map(|l| &l.mots)
            .map(|m| m.octets.len())
            .sum();
        assert_eq!(total, 500);
    }
}
