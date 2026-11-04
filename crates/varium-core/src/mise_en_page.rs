//! Mise en page : découpe des paragraphes en lignes et des lignes en pages, commune
//! à la vue page de l'application et à l'export PDF (ce que l'on voit est ce que l'on
//! imprime).
//!
//! Gère les retraits, alinéas, interlignes et espacements de chaque paragraphe, la
//! justification, les couleurs et surlignages, les formules composées, les codes QR,
//! les signatures, les tableaux (lignes coupées d'une page à l'autre), les veuves et
//! orphelines, les titres gardés avec la suite, les notes de bas de page (placées en
//! bas de la page de leur appel, la place est réservée), la table des matières avec
//! ses points de conduite et numéros de page, l'en-tête, le pied de page et la
//! numérotation (document par document en publipostage).
//!
//! Toutes les mesures sont en points, l'origine en haut à gauche de la page. Les
//! lignes déjà calculées sont gardées dans un [`Cache`] (clé : contenu et mise en
//! forme du paragraphe) : un long document ne recalcule que ce qui a changé.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::doc::{Align, Couleur, Interligne, Para, ParaFormat, Run, Style, Table};
use crate::export::Rendered;
use crate::formule::{self, Trace};
use crate::page::PageSetup;
use crate::pdf::PdfFonts;

/// Points par millimètre.
pub const PT_MM: f32 = 72.0 / 25.4;

/// Nature d'un texte, pour ses couleurs à l'écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Genre {
    Normal,
    /// Valeur d'un champ.
    Valeur,
    /// Champ sans valeur (`{{nom}}` affiché tel quel).
    Manquant,
    Lien,
    Formule,
    /// En-tête, pied de page, numéro de page.
    Marge,
}

/// Un texte posé : glyphes d'une même police, sur une même ligne de base.
#[derive(Debug, Clone, PartialEq)]
pub struct Texte {
    pub x: f32,
    /// Ligne de base.
    pub y: f32,
    pub taille: f32,
    /// Police dans [`PdfFonts`] (voir [`PdfFonts::glyphe`]).
    pub police: u8,
    pub glyphes: Vec<u16>,
    pub texte: String,
    pub gras: bool,
    pub italique: bool,
    /// Italique simulé par inclinaison (la police n'a pas de vraie italique).
    pub oblique: bool,
    pub couleur: Couleur,
    pub largeur: f32,
    pub genre: Genre,
}

/// Cible d'un lien.
#[derive(Debug, Clone, PartialEq)]
pub enum Cible {
    Url(String),
    /// Le titre de rang k (table des matières).
    Titre(usize),
}

/// Un élément dessiné de la page.
#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    Texte(Texte),
    /// Rectangle plein (surlignage, soulignement, barre de fraction, module de QR…).
    Rect {
        x: f32,
        y: f32,
        l: f32,
        h: f32,
        couleur: Couleur,
    },
    /// Segment (filet, bordure de tableau).
    Ligne {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        ep: f32,
        couleur: Couleur,
    },
    /// Ligne brisée aux bouts arrondis (signature, signe radical).
    Trait {
        points: Vec<(f32, f32)>,
        ep: f32,
        couleur: Couleur,
    },
    /// Zone cliquable.
    Lien {
        x: f32,
        y: f32,
        l: f32,
        h: f32,
        cible: Cible,
    },
    /// Emplacement d'une signature introuvable (cadre et nom).
    SansSignature {
        x: f32,
        y: f32,
        l: f32,
        h: f32,
    },
}

impl Element {
    fn decaler(&self, dx: f32, dy: f32) -> Element {
        let mut e = self.clone();
        match &mut e {
            Element::Texte(t) => {
                t.x += dx;
                t.y += dy;
            }
            Element::Rect { x, y, .. }
            | Element::Lien { x, y, .. }
            | Element::SansSignature { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Element::Ligne { x1, y1, x2, y2, .. } => {
                *x1 += dx;
                *x2 += dx;
                *y1 += dy;
                *y2 += dy;
            }
            Element::Trait { points, .. } => {
                for p in points {
                    p.0 += dx;
                    p.1 += dy;
                }
            }
        }
        e
    }
}

/// Une ligne calculée : ses éléments relatifs à son coin haut-gauche.
#[derive(Debug, Clone, Default)]
pub struct Ligne {
    pub hauteur: f32,
    pub elements: Vec<Element>,
    /// Notes de bas de page appelées sur la ligne.
    notes: Vec<usize>,
    /// Entrée de table des matières : (rang du titre, fin du texte, ligne de base, taille, police).
    sommaire: Option<(usize, f32, f32, f32, bool)>,
}

/// Un bloc : les lignes d'un paragraphe (ou d'une ligne de tableau).
#[derive(Debug, Clone, Default)]
struct Bloc {
    avant: f32,
    apres: f32,
    lignes: Vec<Ligne>,
    /// Décalage horizontal du bloc dans la zone de texte.
    x: f32,
    /// Garder avec le bloc suivant (titres).
    garder: bool,
    /// Ne pas couper si possible (lignes de tableau courtes, formule, code QR).
    insecable: bool,
    /// Titre : (niveau, texte).
    titre: Option<(u8, String)>,
    /// Barre verticale à gauche (citation), à cette distance du bloc.
    barre: Option<f32>,
    saut: bool,
}

type Blocs = Arc<Vec<Bloc>>;

/// Une ligne placée sur une page.
#[derive(Debug, Clone)]
pub struct Placee {
    blocs: Blocs,
    bloc: u32,
    ligne: u32,
    pub x: f32,
    pub y: f32,
}

impl Placee {
    pub fn ligne(&self) -> &Ligne {
        &self.blocs[self.bloc as usize].lignes[self.ligne as usize]
    }
}

/// Une page mise en page.
#[derive(Debug, Clone, Default)]
pub struct PageMise {
    pub lignes: Vec<Placee>,
    /// Éléments à leur position absolue (en-tête, pied, filets, numéros du sommaire…).
    pub extra: Vec<Element>,
    /// Document (publipostage) auquel appartient la page.
    pub doc: usize,
}

impl PageMise {
    /// Parcourt les éléments de la page, à leur position absolue.
    pub fn pour_chaque(&self, mut f: impl FnMut(&Element)) {
        for e in &self.extra {
            f(e);
        }
        for p in &self.lignes {
            for e in &p.ligne().elements {
                f(&e.decaler(p.x, p.y));
            }
        }
    }

    /// Les éléments à leur position absolue.
    pub fn elements(&self) -> Vec<Element> {
        let mut v = Vec::new();
        self.pour_chaque(|e| v.push(e.clone()));
        v
    }
}

/// Un titre placé : niveau, texte, page et position verticale.
#[derive(Debug, Clone, PartialEq)]
pub struct TitrePlace {
    pub niveau: u8,
    pub texte: String,
    pub page: usize,
    pub y: f32,
}

/// Le document mis en page.
#[derive(Debug, Clone, Default)]
pub struct Mise {
    pub pages: Vec<PageMise>,
    pub largeur: f32,
    pub hauteur: f32,
    /// Marges gauche et haute, largeur et hauteur de la zone de texte.
    pub zone: (f32, f32, f32, f32),
    /// Tous les titres, dans l'ordre (le rang d'un titre est son indice).
    pub titres: Vec<TitrePlace>,
}

/// Lignes déjà calculées, réutilisées d'une mise en page à l'autre.
#[derive(Default)]
pub struct Cache {
    blocs: HashMap<u64, (u64, Blocs)>,
    generation: u64,
}

impl Cache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Nombre de paragraphes gardés.
    pub fn len(&self) -> usize {
        self.blocs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocs.is_empty()
    }

    fn obtenir(&mut self, cle: u64, calcul: impl FnOnce() -> Vec<Bloc>) -> Blocs {
        let g = self.generation;
        if let Some((gen_, b)) = self.blocs.get_mut(&cle) {
            *gen_ = g;
            return b.clone();
        }
        let b = Arc::new(calcul());
        self.blocs.insert(cle, (g, b.clone()));
        b
    }

    /// Oublie ce qui n'a pas servi depuis deux mises en page.
    fn nettoyer(&mut self) {
        let g = self.generation;
        if self.blocs.len() > 2000 {
            self.blocs.retain(|_, (u, _)| *u + 1 >= g);
        }
        self.generation += 1;
    }
}

/// Réglages de la mise en page.
pub struct Options<'a> {
    pub polices: &'a PdfFonts,
    /// Modèle du numéro de page, `{n}` et `{total}` remplacés.
    pub numero: &'a str,
}

impl<'a> Options<'a> {
    pub fn new(polices: &'a PdfFonts) -> Self {
        Options {
            polices,
            numero: "Page {n} sur {total}",
        }
    }
}

const NOIR: Couleur = Couleur::rvb(0x000000);
const ENCRE: Couleur = Couleur::rvb(0x151412);
const GRIS_MARGE: Couleur = Couleur::rvb(0x444444);
const ROUGE_MANQUANT: Couleur = Couleur::rvb(0xA1201B);
const FOND_MANQUANT: Couleur = Couleur::rvb(0xFDE2E1);
const BLEU_LIEN: Couleur = Couleur::rvb(0x1F5FBF);
const ENCRE_SIGNATURE: Couleur = Couleur::rvb(0x1A2A5A);

fn fini(v: f32, defaut: f32) -> f32 {
    if v.is_finite() { v } else { defaut }
}

/// Mesures communes d'une mise en page.
#[derive(Clone, Copy)]
struct Gabarit {
    taille: f32,
    interligne: f32,
    justifier: bool,
    largeur: f32,
    hauteur: f32,
    ml: f32,
    mt: f32,
    mb: f32,
    tw: f32,
    th: f32,
    /// Interligne simple de la police (en em).
    simple: f32,
    ascent: f32,
    descent: f32,
    id_polices: u64,
}

impl Gabarit {
    fn new(page: &PageSetup, o: &Options) -> Self {
        let taille = fini(page.font_size, 12.0).clamp(1.0, 400.0);
        let interligne = fini(page.line_spacing, 1.15).clamp(0.1, 6.0);
        let (w, h) = page.size_mm();
        let m = |v: f32| fini(v, 20.0).max(0.0);
        let (ml, mt, mb) = (
            m(page.margin_left).min(w - 20.0).max(0.0),
            m(page.margin_top).min(h - 20.0).max(0.0),
            m(page.margin_bottom).min(h - 20.0).max(0.0),
        );
        let (ascent, descent, gap) = o.polices.metriques();
        Gabarit {
            taille,
            interligne,
            justifier: page.justify,
            largeur: w * PT_MM,
            hauteur: h * PT_MM,
            ml: ml * PT_MM,
            mt: mt * PT_MM,
            mb: mb * PT_MM,
            tw: page.text_width_mm() * PT_MM,
            th: page.text_height_mm() * PT_MM,
            simple: ascent + descent + gap,
            ascent,
            descent,
            id_polices: o.polices.id(),
        }
    }

    /// Hauteur de ligne pour la taille `t` et la mise en forme `f`.
    fn hauteur_ligne(&self, t: f32, f: &ParaFormat) -> f32 {
        match f.interligne {
            Some(Interligne::Exact(p)) => p.clamp(1.0, 400.0) * t / self.taille,
            Some(Interligne::Multiple(m)) => t * self.simple * m.clamp(0.1, 6.0),
            None => t * self.simple * self.interligne,
        }
    }
}

// ---------------------------------------------------------------------------
// Atomes et lignes
// ---------------------------------------------------------------------------

/// Un morceau de mot dans une seule police et un seul style.
#[derive(Clone)]
struct Morceau {
    police: u8,
    glyphes: Vec<u16>,
    texte: String,
    largeur: f32,
    taille: f32,
    /// Décalage de la ligne de base (exposant), vers le haut.
    monte: f32,
    style: Style,
    genre: Genre,
    oblique: bool,
    lien: Option<Cible>,
    /// Blanc insécable : avance sans glyphe.
    blanc: bool,
    /// Appel de la note n° k.
    note: Option<usize>,
}

/// Une boîte en ligne (formule, signature, code QR) : éléments relatifs à sa ligne de
/// base, à son bord gauche.
#[derive(Clone)]
struct Boite {
    largeur: f32,
    hauteur: f32,
    profondeur: f32,
    elements: Vec<Element>,
}

#[derive(Clone)]
enum Atome {
    Mot(Vec<Morceau>, f32),
    Espace(f32, Style),
    Boite(Boite),
    Saut,
}

impl Atome {
    fn largeur(&self) -> f32 {
        match self {
            Atome::Mot(_, l) => *l,
            Atome::Espace(l, _) => *l,
            Atome::Boite(b) => b.largeur,
            Atome::Saut => 0.0,
        }
    }
}

struct Composeur<'a> {
    f: &'a PdfFonts,
    signatures: &'a std::collections::BTreeMap<String, crate::signature::Signature>,
}

fn indice_style(gras: bool, italique: bool) -> usize {
    gras as usize | (italique as usize) << 1
}

/// Blancs typographiques : insécables et largeur propre (en em).
fn blanc(c: char) -> Option<f32> {
    match c {
        '\u{00A0}' => Some(-1.0), // largeur de l'espace
        '\u{202F}' | '\u{2009}' => Some(0.17),
        '\u{2007}' => Some(0.5),
        '\u{2002}' => Some(0.5),
        '\u{2003}' => Some(1.0),
        '\u{200A}' => Some(0.08),
        _ => None,
    }
}

impl Composeur<'_> {
    /// Les morceaux d'un texte (sans espace sécable), dans le style donné.
    fn morceaux(
        &self,
        texte: &str,
        st: Style,
        genre: Genre,
        taille: f32,
        lien: &Option<Cible>,
        out: &mut Vec<Morceau>,
    ) {
        let si = indice_style(st.bold, st.italic);
        let monte = if st.exposant { taille * 0.36 } else { 0.0 };
        let t = if st.exposant { taille * 0.66 } else { taille };
        for c in texte.chars() {
            if matches!(
                c,
                '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' | '\u{AD}'
            ) {
                continue;
            }
            if let Some(em) = blanc(c) {
                let l = if em < 0.0 {
                    self.f.glyphe(' ', si).avance * t
                } else {
                    em * t
                };
                out.push(Morceau {
                    police: 0,
                    glyphes: Vec::new(),
                    texte: " ".into(),
                    largeur: l,
                    taille: t,
                    monte,
                    style: st,
                    genre,
                    oblique: false,
                    lien: lien.clone(),
                    blanc: true,
                    note: None,
                });
                continue;
            }
            let g = self.f.glyphe(c, si);
            let oblique = st.italic && !self.f.vraie_italique(g.police);
            match out.last_mut() {
                Some(m)
                    if !m.blanc
                        && m.note.is_none()
                        && m.police == g.police
                        && m.style == st
                        && m.genre == genre
                        && m.lien == *lien
                        && m.oblique == oblique =>
                {
                    m.glyphes.push(g.gid);
                    m.texte.push(c);
                    m.largeur += g.avance * t;
                }
                _ => out.push(Morceau {
                    police: g.police,
                    glyphes: vec![g.gid],
                    texte: c.to_string(),
                    largeur: g.avance * t,
                    taille: t,
                    monte,
                    style: st,
                    genre,
                    oblique,
                    lien: lien.clone(),
                    blanc: false,
                    note: None,
                }),
            }
        }
    }

    fn formule(&self, tex: &str, taille: f32, centree: bool, st: Style) -> Boite {
        let mesure = |c: char, it: bool| self.f.glyphe(c, indice_style(false, it)).avance;
        let b = formule::composer(&formule::lire(tex), taille, centree, &mesure);
        let couleur = st.couleur.unwrap_or(ENCRE);
        let mut elements = Vec::new();
        for t in b.traces {
            match t {
                Trace::Glyphe {
                    x,
                    y,
                    c,
                    taille,
                    italique,
                } => {
                    let g = self.f.glyphe(c, indice_style(false, italique));
                    elements.push(Element::Texte(Texte {
                        x,
                        y,
                        taille,
                        police: g.police,
                        glyphes: vec![g.gid],
                        texte: c.to_string(),
                        gras: false,
                        italique,
                        oblique: italique && !self.f.vraie_italique(g.police),
                        couleur,
                        largeur: g.avance * taille,
                        genre: Genre::Formule,
                    }));
                }
                Trace::Barre { x, y, l, h } => elements.push(Element::Rect {
                    x,
                    y,
                    l,
                    h,
                    couleur,
                }),
                Trace::Trait { points, ep } => elements.push(Element::Trait {
                    points,
                    ep,
                    couleur,
                }),
            }
        }
        Boite {
            largeur: b.largeur,
            hauteur: b.hauteur,
            profondeur: b.profondeur,
            elements,
        }
    }

    fn qr(&self, contenu: &str, cote: f32) -> Option<Boite> {
        let q = crate::qr::QrCode::encode(contenu).ok()?;
        let module = cote / (q.size() as f32 + 4.0);
        let elements = q
            .segments()
            .into_iter()
            .map(|(x, y, l)| Element::Rect {
                x: (x as f32 + 2.0) * module,
                y: (y as f32 + 2.0) * module - cote,
                l: l as f32 * module,
                h: module,
                couleur: NOIR,
            })
            .collect();
        Some(Boite {
            largeur: cote,
            hauteur: cote,
            profondeur: 0.0,
            elements,
        })
    }

    fn signature(&self, nom: &str, max: f32) -> Boite {
        match self.signatures.get(nom) {
            Some(s) => {
                let (l, h) = s.taille_mm();
                let k = (max / (l * PT_MM)).min(1.0);
                let (l, h) = (l * PT_MM * k, h * PT_MM * k);
                let ep = s.epaisseur() * h / s.hauteur.max(1.0);
                let elements = s
                    .a_l_echelle(l, h)
                    .into_iter()
                    .map(|mut points| {
                        if points.len() == 1 {
                            points.push((points[0].0 + 0.01, points[0].1));
                        }
                        Element::Trait {
                            points: points.into_iter().map(|(x, y)| (x, y - h)).collect(),
                            ep: ep.max(0.4),
                            couleur: ENCRE_SIGNATURE,
                        }
                    })
                    .collect();
                Boite {
                    largeur: l,
                    hauteur: h,
                    profondeur: 0.0,
                    elements,
                }
            }
            None => {
                let (l, h) = ((45.0 * PT_MM).min(max), 12.0 * PT_MM);
                Boite {
                    largeur: l,
                    hauteur: h,
                    profondeur: 0.0,
                    elements: vec![Element::SansSignature {
                        x: 0.0,
                        y: -h,
                        l,
                        h,
                    }],
                }
            }
        }
    }

    /// Les atomes d'une suite de morceaux ; `force` ajoute gras ou italique.
    fn atomes(&self, runs: &[Run], taille: f32, force: Style, largeur: f32) -> Vec<Atome> {
        let mut out: Vec<Atome> = Vec::new();
        let mut mot: Vec<Morceau> = Vec::new();
        let vider = |mot: &mut Vec<Morceau>, out: &mut Vec<Atome>| {
            if !mot.is_empty() {
                let l = mot.iter().map(|m| m.largeur).sum();
                out.push(Atome::Mot(std::mem::take(mot), l));
            }
        };
        for r in runs {
            let mut st = r.style();
            st.bold |= force.bold;
            st.italic |= force.italic;
            let lien = r
                .lien()
                .map(|(u, _)| Cible::Url(crate::doc::adresse_lien(u)));
            let genre = match r {
                _ if lien.is_some() => Genre::Lien,
                Run::Missing { .. } => Genre::Manquant,
                Run::Value { .. } if r.sommaire().is_some() => Genre::Normal,
                Run::Value { .. } => Genre::Valeur,
                _ => Genre::Normal,
            };
            if let Some(n) = r.note() {
                // L'appel fait partie du mot qui le précède.
                let mut m = Vec::new();
                self.morceaux(&n.to_string(), st, Genre::Normal, taille, &None, &mut m);
                if let Some(premier) = m.first_mut() {
                    premier.note = Some(n);
                }
                mot.extend(m);
                continue;
            }
            if let Some(q) = r.qr() {
                vider(&mut mot, &mut out);
                if let Some(b) = self.qr(q, (20.0 * PT_MM).min(largeur)) {
                    out.push(Atome::Boite(b));
                }
                continue;
            }
            if let Some(nom) = r.signature() {
                vider(&mut mot, &mut out);
                out.push(Atome::Boite(self.signature(nom, largeur)));
                continue;
            }
            if let Run::Math { tex, .. } = r {
                vider(&mut mot, &mut out);
                out.push(Atome::Boite(self.formule(tex, taille, false, st)));
                continue;
            }
            let texte = match r {
                Run::Missing { raw, .. } => raw.clone(),
                _ if r.lien().is_some() => r.lien().map(|(_, t)| t.to_string()).unwrap_or_default(),
                _ => r.plain(),
            };
            let si = indice_style(st.bold, st.italic);
            let mut debut = 0;
            for (i, c) in texte.char_indices() {
                if c == ' ' || c == '\t' || c == '\n' || c == '\r' {
                    let avant = &texte[debut..i];
                    if !avant.is_empty() {
                        self.morceaux(avant, st, genre, taille, &lien, &mut mot);
                    }
                    debut = i + c.len_utf8();
                    vider(&mut mot, &mut out);
                    match c {
                        '\n' => out.push(Atome::Saut),
                        '\r' => {}
                        _ => {
                            let e = self.f.glyphe(' ', si).avance * taille;
                            for _ in 0..if c == '\t' { 4 } else { 1 } {
                                out.push(Atome::Espace(e, st));
                            }
                        }
                    }
                }
            }
            if debut < texte.len() {
                self.morceaux(&texte[debut..], st, genre, taille, &lien, &mut mot);
            }
            // Pas de coupure entre deux morceaux qui se touchent (« **gras**, »).
        }
        vider(&mut mot, &mut out);
        // Fusion des mots voisins sans espace (styles différents dans un même mot).
        let mut fusion: Vec<Atome> = Vec::with_capacity(out.len());
        for a in out {
            match (fusion.last_mut(), a) {
                (Some(Atome::Mot(m1, l1)), Atome::Mot(m2, l2)) => {
                    m1.extend(m2);
                    *l1 += l2;
                }
                (_, a) => fusion.push(a),
            }
        }
        fusion
    }

    /// Coupe un mot trop long en morceaux d'au plus `max` points.
    fn couper(&self, morceaux: Vec<Morceau>, max: f32) -> Vec<Atome> {
        let mut out = Vec::new();
        let mut cur: Vec<Morceau> = Vec::new();
        let mut l = 0.0f32;
        for m in morceaux {
            let chars: Vec<char> = m.texte.chars().collect();
            if m.blanc || chars.len() != m.glyphes.len() {
                if l + m.largeur > max && !cur.is_empty() {
                    out.push(Atome::Mot(std::mem::take(&mut cur), l));
                    l = 0.0;
                }
                l += m.largeur;
                cur.push(m);
                continue;
            }
            let par = if chars.is_empty() {
                0.0
            } else {
                m.largeur / chars.len() as f32
            };
            let mut i = 0;
            while i < chars.len() {
                let si = indice_style(m.style.bold, m.style.italic);
                let w = self
                    .f
                    .avance_glyphe(m.police, m.glyphes[i], si)
                    .unwrap_or(par)
                    * m.taille;
                if l + w > max && l > 0.0 {
                    out.push(Atome::Mot(std::mem::take(&mut cur), l));
                    l = 0.0;
                }
                match cur.last_mut() {
                    Some(c)
                        if c.police == m.police
                            && c.style == m.style
                            && !c.blanc
                            && c.genre == m.genre =>
                    {
                        c.glyphes.push(m.glyphes[i]);
                        c.texte.push(chars[i]);
                        c.largeur += w;
                    }
                    _ => cur.push(Morceau {
                        glyphes: vec![m.glyphes[i]],
                        texte: chars[i].to_string(),
                        largeur: w,
                        ..m.clone()
                    }),
                }
                l += w;
                i += 1;
            }
        }
        if !cur.is_empty() {
            out.push(Atome::Mot(cur, l));
        }
        out
    }
}

/// Une ligne en construction.
struct LigneA {
    atomes: Vec<Atome>,
    largeur: f32,
    derniere: bool,
}

fn decouper(c: &Composeur, atomes: Vec<Atome>, premiere: f32, suite: f32) -> Vec<LigneA> {
    let mut lignes: Vec<LigneA> = Vec::new();
    let mut cur: Vec<Atome> = Vec::new();
    let mut l = 0.0f32;
    let max = |n: usize| if n == 0 { premiere } else { suite }.max(10.0);
    let finir = |cur: &mut Vec<Atome>, l: &mut f32, lignes: &mut Vec<LigneA>, derniere: bool| {
        while matches!(cur.last(), Some(Atome::Espace(..))) {
            if let Some(a) = cur.pop() {
                *l -= a.largeur();
            }
        }
        lignes.push(LigneA {
            atomes: std::mem::take(cur),
            largeur: l.max(0.0),
            derniere,
        });
        *l = 0.0;
    };
    for a in atomes {
        match a {
            Atome::Saut => finir(&mut cur, &mut l, &mut lignes, true),
            Atome::Espace(..) => {
                if cur.is_empty() && lignes.last().is_some_and(|x| !x.derniere) {
                    continue;
                }
                l += a.largeur();
                cur.push(a);
            }
            Atome::Mot(m, w) => {
                let m_max = max(lignes.len());
                let pieces = if w > m_max {
                    c.couper(m, m_max)
                } else {
                    vec![Atome::Mot(m, w)]
                };
                for p in pieces {
                    let w = p.largeur();
                    if l + w > max(lignes.len()) + 0.01
                        && cur.iter().any(|x| !matches!(x, Atome::Espace(..)))
                    {
                        finir(&mut cur, &mut l, &mut lignes, false);
                    }
                    l += w;
                    cur.push(p);
                }
            }
            Atome::Boite(b) => {
                let w = b.largeur;
                if l + w > max(lignes.len()) + 0.01
                    && cur.iter().any(|x| !matches!(x, Atome::Espace(..)))
                {
                    finir(&mut cur, &mut l, &mut lignes, false);
                }
                l += w;
                cur.push(Atome::Boite(b));
            }
        }
    }
    finir(&mut cur, &mut l, &mut lignes, true);
    lignes
}

/// Réglages d'un paragraphe à poser.
struct Pose {
    taille: f32,
    hl: f32,
    align: Align,
    gauche: f32,
    largeur: f32,
    alinea: f32,
}

fn poser_lignes(g: &Gabarit, lignes: Vec<LigneA>, p: &Pose) -> Vec<Ligne> {
    let mut out = Vec::with_capacity(lignes.len());
    for (i, la) in lignes.into_iter().enumerate() {
        let retrait = if i == 0 { p.alinea } else { 0.0 };
        let dispo = (p.largeur - retrait).max(10.0);
        // Hauteur : celle du texte, ou plus si une boîte dépasse.
        let asc_t = g.ascent * p.taille;
        let desc_t = g.descent * p.taille;
        let (mut asc, mut desc) = (asc_t, desc_t);
        for a in &la.atomes {
            if let Atome::Boite(b) = a {
                asc = asc.max(b.hauteur);
                desc = desc.max(b.profondeur);
            }
        }
        let extra = (p.hl - (asc_t + desc_t)).max(0.0);
        let hauteur = if asc > asc_t || desc > desc_t {
            (asc + desc + extra).max(p.hl)
        } else {
            p.hl
        };
        let base = (hauteur - (asc + desc)) / 2.0 + asc;
        let espaces = la
            .atomes
            .iter()
            .filter(|a| matches!(a, Atome::Espace(..)))
            .count();
        let justifier =
            p.align == Align::Justify && !la.derniere && espaces > 0 && la.largeur < dispo;
        let en_plus = if justifier {
            (dispo - la.largeur) / espaces as f32
        } else {
            0.0
        };
        let total = la.largeur + en_plus * espaces as f32;
        let mut x = p.gauche
            + retrait
            + match p.align {
                Align::Center => ((dispo - total) / 2.0).max(0.0),
                Align::Right => (dispo - total).max(0.0),
                _ => 0.0,
            };
        let mut l = Ligne {
            hauteur,
            ..Ligne::default()
        };
        let mut fonds: Vec<Element> = Vec::new();
        let mut textes: Vec<Element> = Vec::new();
        let mut prec_style: Option<(Style, f32)> = None;
        for a in la.atomes {
            match a {
                Atome::Espace(w, st) => {
                    let w = w + en_plus;
                    // Le surlignage et le soulignement continuent sur l'espace.
                    if let Some((ps, _)) = prec_style
                        && ps == st
                    {
                        decor(
                            &mut fonds,
                            &mut textes,
                            st,
                            x,
                            base,
                            w,
                            p.taille,
                            Genre::Normal,
                        );
                    }
                    x += w;
                }
                Atome::Saut => {}
                Atome::Boite(b) => {
                    for e in &b.elements {
                        textes.push(e.decaler(x, base));
                    }
                    x += b.largeur;
                    prec_style = None;
                }
                Atome::Mot(morceaux, _) => {
                    for m in morceaux {
                        if let Some(n) = m.note
                            && !l.notes.contains(&n)
                        {
                            l.notes.push(n);
                        }
                        let y = base - m.monte;
                        decor(
                            &mut fonds,
                            &mut textes,
                            m.style,
                            x,
                            y,
                            m.largeur,
                            m.taille,
                            m.genre,
                        );
                        if let Some(cible) = &m.lien {
                            textes.push(Element::Lien {
                                x,
                                y: y - m.taille * 0.9,
                                l: m.largeur,
                                h: m.taille * 1.15,
                                cible: cible.clone(),
                            });
                        }
                        if !m.blanc {
                            let couleur = match m.genre {
                                Genre::Manquant => ROUGE_MANQUANT,
                                Genre::Lien => m.style.couleur.unwrap_or(BLEU_LIEN),
                                _ => m.style.couleur.unwrap_or(ENCRE),
                            };
                            textes.push(Element::Texte(Texte {
                                x,
                                y,
                                taille: m.taille,
                                police: m.police,
                                glyphes: m.glyphes,
                                texte: m.texte,
                                gras: m.style.bold,
                                italique: m.style.italic,
                                oblique: m.oblique,
                                couleur,
                                largeur: m.largeur,
                                genre: m.genre,
                            }));
                        }
                        x += m.largeur;
                        prec_style = Some((m.style, m.taille));
                    }
                }
            }
        }
        fonds.append(&mut textes);
        l.elements = fonds;
        out.push(l);
    }
    out
}

/// Fond (surlignage, champ manquant), soulignement et barré d'un morceau.
#[allow(clippy::too_many_arguments)]
fn decor(
    fonds: &mut Vec<Element>,
    traits: &mut Vec<Element>,
    st: Style,
    x: f32,
    base: f32,
    l: f32,
    t: f32,
    genre: Genre,
) {
    if l <= 0.0 {
        return;
    }
    let fond = match genre {
        Genre::Manquant => Some(FOND_MANQUANT),
        _ => st.surlignage,
    };
    if let Some(c) = fond {
        fonds.push(Element::Rect {
            x,
            y: base - t * 0.88,
            l,
            h: t * 1.12,
            couleur: c,
        });
    }
    let couleur = match genre {
        Genre::Lien => st.couleur.unwrap_or(BLEU_LIEN),
        Genre::Manquant => ROUGE_MANQUANT,
        _ => st.couleur.unwrap_or(ENCRE),
    };
    let ep = (t * 0.05).max(0.3);
    if st.underline || genre == Genre::Lien {
        traits.push(Element::Rect {
            x,
            y: base + t * 0.1,
            l,
            h: ep,
            couleur,
        });
    }
    if st.strike {
        traits.push(Element::Rect {
            x,
            y: base - t * 0.3,
            l,
            h: ep,
            couleur,
        });
    }
}

// ---------------------------------------------------------------------------
// Paragraphes en blocs
// ---------------------------------------------------------------------------

fn texte_simple(s: &str) -> Run {
    Run::Text {
        text: s.to_string(),
        style: Style::default(),
    }
}

struct Contexte<'a> {
    g: Gabarit,
    c: Composeur<'a>,
}

impl Contexte<'_> {
    #[allow(clippy::too_many_arguments)]
    fn bloc_texte(
        &self,
        runs: &[Run],
        f: &ParaFormat,
        taille: f32,
        force: Style,
        align: Align,
        gauche: f32,
        droite: f32,
    ) -> Bloc {
        let g = &self.g;
        let largeur = (g.tw - gauche - droite).max(20.0);
        let alinea = f.alinea.unwrap_or(0.0) * PT_MM;
        let atomes = self.c.atomes(runs, taille, force, largeur);
        let lignes = decouper(&self.c, atomes, (largeur - alinea).max(20.0), largeur);
        let hl = g.hauteur_ligne(taille, f);
        let mut l = poser_lignes(
            g,
            lignes,
            &Pose {
                taille,
                hl,
                align,
                gauche: 0.0,
                largeur,
                alinea,
            },
        );
        // Une entrée de table des matières : place réservée au numéro de page.
        if let Some(k) = runs.iter().find_map(Run::sommaire)
            && let Some(der) = l.last_mut()
        {
            let fin = der
                .elements
                .iter()
                .filter_map(|e| match e {
                    Element::Texte(t) => Some(t.x + t.largeur),
                    _ => None,
                })
                .fold(0.0f32, f32::max);
            let base = (der.hauteur - (g.ascent + g.descent) * taille) / 2.0 + g.ascent * taille;
            der.sommaire = Some((k, fin, base, taille, force.bold));
        }
        Bloc {
            lignes: l,
            x: gauche,
            avant: f.avant.unwrap_or(0.0),
            apres: f.apres.unwrap_or(0.0),
            ..Bloc::default()
        }
    }

    /// Les blocs d'un paragraphe.
    fn blocs(&self, p: &Para, f: &ParaFormat) -> Vec<Bloc> {
        let g = &self.g;
        let t = g.taille;
        let none = Style::default();
        let (fg, fd) = (
            f.gauche.unwrap_or(0.0) * PT_MM,
            f.droite.unwrap_or(0.0) * PT_MM,
        );
        if let (Some(q), Para::Text { align, .. }) = (p.qr(), p) {
            let cote = (crate::qr::TAILLE_MM * PT_MM).min(g.tw).min(g.th);
            if let Some(b) = self.c.qr(q, cote) {
                let x = match align {
                    Align::Center => (g.tw - cote) / 2.0,
                    Align::Right => g.tw - cote,
                    _ => fg,
                };
                return vec![Bloc {
                    lignes: vec![Ligne {
                        hauteur: cote,
                        elements: b.elements.iter().map(|e| e.decaler(x, cote)).collect(),
                        ..Ligne::default()
                    }],
                    insecable: true,
                    avant: f.avant.unwrap_or(0.0),
                    apres: f.apres.unwrap_or(0.0),
                    ..Bloc::default()
                }];
            }
        }
        match p {
            Para::Text { align, runs } => {
                let a = match (*align, g.justifier) {
                    (Align::Left, true) => Align::Justify,
                    (a, _) => a,
                };
                vec![self.bloc_texte(runs, f, t, none, a, fg, fd)]
            }
            Para::Heading { level, runs } => {
                let k = [1.6, 1.35, 1.15][(*level as usize).clamp(1, 3) - 1];
                let hs = t * k;
                let fh = ParaFormat {
                    interligne: f.interligne.or(Some(Interligne::Multiple(1.1))),
                    ..*f
                };
                let mut b = self.bloc_texte(
                    runs,
                    &fh,
                    hs,
                    Style { bold: true, ..none },
                    Align::Left,
                    fg,
                    fd,
                );
                b.avant = f.avant.unwrap_or(t * 0.6);
                b.apres = f.apres.unwrap_or(t * 0.3);
                b.garder = true;
                let titre = crate::doc::plain(runs);
                b.titre = Some((
                    (*level).clamp(1, 3),
                    titre.split_whitespace().collect::<Vec<_>>().join(" "),
                ));
                vec![b]
            }
            Para::Item {
                number,
                depth,
                runs,
            } => {
                let retrait = 18.0 * (*depth as f32 + 1.0) + fg;
                let label = match number {
                    Some(k) => format!("{k}."),
                    None => ["\u{2022}", "\u{25E6}", "\u{25AA}"][*depth as usize % 3].to_string(),
                };
                let sommaire = runs.iter().any(|r| r.sommaire().is_some());
                let retrait = if sommaire {
                    14.0 * *depth as f32 + fg
                } else {
                    retrait
                };
                let droite = fd + if sommaire { t * 2.5 } else { 0.0 };
                let mut b = self.bloc_texte(runs, f, t, none, Align::Left, retrait, droite);
                if !sommaire && let Some(prem) = b.lignes.first_mut() {
                    let lab = self.c.atomes(&[texte_simple(&label)], t, none, 100.0);
                    let lw: f32 = lab.iter().map(Atome::largeur).sum();
                    let base = (prem.hauteur - (g.ascent + g.descent) * t) / 2.0 + g.ascent * t;
                    let mut x = -6.0 - lw;
                    for a in lab {
                        if let Atome::Mot(ms, _) = a {
                            for m in ms {
                                prem.elements.push(Element::Texte(Texte {
                                    x,
                                    y: base,
                                    taille: m.taille,
                                    police: m.police,
                                    glyphes: m.glyphes,
                                    texte: m.texte,
                                    gras: false,
                                    italique: false,
                                    oblique: false,
                                    couleur: ENCRE,
                                    largeur: m.largeur,
                                    genre: Genre::Normal,
                                }));
                                x += m.largeur;
                            }
                        }
                    }
                }
                vec![b]
            }
            Para::Quote { runs } => {
                let ind = 28.0;
                let mut b = self.bloc_texte(
                    runs,
                    f,
                    t,
                    Style {
                        italic: true,
                        ..none
                    },
                    Align::Left,
                    ind + fg,
                    ind + fd,
                );
                b.barre = Some(-9.0);
                b.avant = f.avant.unwrap_or(t * 0.15);
                b.apres = f.apres.unwrap_or(t * 0.35);
                vec![b]
            }
            Para::Math { tex } => {
                let boite = self.c.formule(tex, t * 1.1, true, none);
                let h = boite.hauteur + boite.profondeur + t * 0.3;
                let x = fg + ((g.tw - fg - fd - boite.largeur) / 2.0).max(0.0);
                let base = t * 0.15 + boite.hauteur;
                vec![Bloc {
                    lignes: vec![Ligne {
                        hauteur: h.max(g.hauteur_ligne(t, f)),
                        elements: boite.elements.iter().map(|e| e.decaler(x, base)).collect(),
                        ..Ligne::default()
                    }],
                    avant: f.avant.unwrap_or(t * 0.3),
                    apres: f.apres.unwrap_or(t * 0.3),
                    insecable: true,
                    ..Bloc::default()
                }]
            }
            Para::Table(tab) => self.tableau(tab, f),
            Para::Rule => {
                let h = t * 1.0;
                vec![Bloc {
                    lignes: vec![Ligne {
                        hauteur: h,
                        elements: vec![Element::Ligne {
                            x1: fg,
                            y1: h / 2.0,
                            x2: g.tw - fd,
                            y2: h / 2.0,
                            ep: 0.5,
                            couleur: Couleur::rvb(0x444444),
                        }],
                        ..Ligne::default()
                    }],
                    ..Bloc::default()
                }]
            }
            Para::PageBreak => vec![Bloc {
                saut: true,
                ..Bloc::default()
            }],
        }
    }

    fn tableau(&self, tab: &Table, f: &ParaFormat) -> Vec<Bloc> {
        let g = &self.g;
        let t = g.taille;
        let cols = tab.columns().max(1);
        let (fg, fd) = (
            f.gauche.unwrap_or(0.0) * PT_MM,
            f.droite.unwrap_or(0.0) * PT_MM,
        );
        let tw = (g.tw - fg - fd).max(20.0);
        let cw = tw / cols as f32;
        let pad = 3.0;
        let bord = Couleur::rvb(0x5A5A5A);
        let mut blocs = Vec::new();
        for (ri, row) in tab.rows.iter().enumerate() {
            let tete = ri == 0 && tab.header;
            let force = Style {
                bold: tete,
                ..Style::default()
            };
            let cellules: Vec<Vec<Ligne>> = (0..cols)
                .map(|c| {
                    let runs = row.get(c).map(Vec::as_slice).unwrap_or(&[]);
                    let atomes = self.c.atomes(runs, t, force, cw - 2.0 * pad);
                    let lignes = decouper(&self.c, atomes, cw - 2.0 * pad, cw - 2.0 * pad);
                    poser_lignes(
                        g,
                        lignes,
                        &Pose {
                            taille: t,
                            hl: g.hauteur_ligne(t, f),
                            align: tab.align(c),
                            gauche: 0.0,
                            largeur: cw - 2.0 * pad,
                            alinea: 0.0,
                        },
                    )
                })
                .collect();
            let n = cellules.iter().map(Vec::len).max().unwrap_or(1).max(1);
            let mut bandes = Vec::with_capacity(n);
            for i in 0..n {
                let h = cellules
                    .iter()
                    .filter_map(|c| c.get(i).map(|l| l.hauteur))
                    .fold(0.0f32, f32::max)
                    .max(1.0);
                let haut = if i == 0 { pad } else { 0.0 };
                let bas = if i + 1 == n { pad } else { 0.0 };
                let hb = h + haut + bas;
                let mut el = Vec::new();
                if tete {
                    el.push(Element::Rect {
                        x: fg,
                        y: 0.0,
                        l: tw,
                        h: hb,
                        couleur: Couleur::rvb(0xF0EEE9),
                    });
                }
                for (c, cell) in cellules.iter().enumerate() {
                    if let Some(l) = cell.get(i) {
                        let x = fg + c as f32 * cw + pad;
                        el.extend(l.elements.iter().map(|e| e.decaler(x, haut)));
                    }
                }
                let ligne = |x1: f32, y1: f32, x2: f32, y2: f32| Element::Ligne {
                    x1,
                    y1,
                    x2,
                    y2,
                    ep: 0.4,
                    couleur: bord,
                };
                for c in 0..=cols {
                    let x = fg + c as f32 * cw;
                    el.push(ligne(x, 0.0, x, hb));
                }
                if i == 0 {
                    el.push(ligne(fg, 0.0, fg + tw, 0.0));
                }
                if i + 1 == n {
                    el.push(ligne(fg, hb, fg + tw, hb));
                }
                let notes = cellules
                    .iter()
                    .filter_map(|c| c.get(i))
                    .flat_map(|l| l.notes.iter().copied())
                    .collect();
                bandes.push(Ligne {
                    hauteur: hb,
                    elements: el,
                    notes,
                    sommaire: None,
                });
            }
            blocs.push(Bloc {
                insecable: n <= 6,
                avant: if ri == 0 {
                    f.avant.unwrap_or(t * 0.4)
                } else {
                    0.0
                },
                apres: if ri + 1 == tab.rows.len() {
                    f.apres.unwrap_or(t * 0.4)
                } else {
                    0.0
                },
                lignes: bandes,
                ..Bloc::default()
            });
        }
        blocs
    }
}

fn cle_para(g: &Gabarit, p: &Para, f: &ParaFormat, sig: u64, sel: u8) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    p.hash(&mut h);
    f.hash(&mut h);
    sig.hash(&mut h);
    sel.hash(&mut h);
    (
        g.taille.to_bits(),
        g.interligne.to_bits(),
        g.justifier,
        g.tw.to_bits(),
        g.th.to_bits(),
        g.id_polices,
    )
        .hash(&mut h);
    h.finish()
}

fn cle_signatures(d: &Rendered) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (n, s) in &d.signatures {
        n.hash(&mut h);
        s.ecrire().hash(&mut h);
    }
    h.finish()
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

struct Paginateur<'a> {
    g: Gabarit,
    pages: Vec<PageMise>,
    y: f32,
    vide: bool,
    /// Notes de la page courante : (numéro, blocs) et leur hauteur totale.
    notes_page: Vec<(usize, Blocs)>,
    h_notes: f32,
    titres: Vec<TitrePlace>,
    doc: usize,
    notes_doc: &'a dyn Fn(usize) -> Option<Blocs>,
}

impl Paginateur<'_> {
    fn nouvelle_page(&mut self) {
        self.finir_notes();
        self.pages.push(PageMise {
            doc: self.doc,
            ..PageMise::default()
        });
        self.y = 0.0;
        self.vide = true;
    }

    fn page(&mut self) -> &mut PageMise {
        if self.pages.is_empty() {
            self.pages.push(PageMise {
                doc: self.doc,
                ..PageMise::default()
            });
        }
        let i = self.pages.len() - 1;
        &mut self.pages[i]
    }

    /// Place les notes accumulées en bas de la page courante.
    fn finir_notes(&mut self) {
        if self.notes_page.is_empty() {
            return;
        }
        let g = self.g;
        let notes = std::mem::take(&mut self.notes_page);
        let mut y = g.th - self.h_notes;
        self.h_notes = 0.0;
        let p = self.page();
        p.extra.push(Element::Ligne {
            x1: g.ml,
            y1: g.mt + y + g.taille * 0.35,
            x2: g.ml + g.tw * 0.3,
            y2: g.mt + y + g.taille * 0.35,
            ep: 0.5,
            couleur: Couleur::rvb(0x444444),
        });
        y += g.taille * 0.7;
        for (_, blocs) in notes {
            for (bi, b) in blocs.iter().enumerate() {
                for (li, l) in b.lignes.iter().enumerate() {
                    p.lignes.push(Placee {
                        blocs: blocs.clone(),
                        bloc: bi as u32,
                        ligne: li as u32,
                        x: g.ml + b.x,
                        y: g.mt + y,
                    });
                    y += l.hauteur;
                }
            }
        }
    }

    /// Hauteur ajoutée par les notes `ns` qui ne sont pas encore sur la page.
    fn hauteur_notes(&self, ns: &[usize]) -> (f32, Vec<(usize, Blocs)>) {
        let mut h = 0.0;
        let mut nouvelles = Vec::new();
        for &n in ns {
            if self.notes_page.iter().any(|(k, _)| *k == n)
                || nouvelles.iter().any(|(k, _): &(usize, Blocs)| *k == n)
            {
                continue;
            }
            if let Some(b) = (self.notes_doc)(n) {
                if self.notes_page.is_empty() && nouvelles.is_empty() {
                    h += self.g.taille * 0.7;
                }
                h += b
                    .iter()
                    .flat_map(|b| b.lignes.iter())
                    .map(|l| l.hauteur)
                    .sum::<f32>();
                nouvelles.push((n, b));
            }
        }
        (h, nouvelles)
    }

    fn place(&self) -> f32 {
        self.g.th - self.h_notes - self.y
    }

    /// Place une ligne ; `false` si elle ne tient pas (la page n'est pas vide).
    fn poser(&mut self, blocs: &Blocs, bi: usize, li: usize, forcer: bool) -> bool {
        let b = &blocs[bi];
        let l = &b.lignes[li];
        let (hn, nouvelles) = self.hauteur_notes(&l.notes);
        if !self.vide && !forcer && l.hauteur + hn > self.place() + 0.01 {
            return false;
        }
        let g = self.g;
        let y = self.y;
        let page = self.pages.len().saturating_sub(1);
        if li == 0
            && let Some((niveau, texte)) = &b.titre
        {
            self.titres.push(TitrePlace {
                niveau: *niveau,
                texte: texte.clone(),
                page,
                y: g.mt + y,
            });
        }
        if let Some(bx) = b.barre {
            let x = g.ml + b.x + bx;
            self.page().extra.push(Element::Ligne {
                x1: x,
                y1: g.mt + y,
                x2: x,
                y2: g.mt + y + l.hauteur,
                ep: 1.2,
                couleur: Couleur::rvb(0xAAAAAA),
            });
        }
        self.page().lignes.push(Placee {
            blocs: blocs.clone(),
            bloc: bi as u32,
            ligne: li as u32,
            x: g.ml + b.x,
            y: g.mt + y,
        });
        self.y += l.hauteur;
        self.h_notes += hn;
        self.notes_page.extend(nouvelles);
        self.vide = false;
        true
    }

    /// Hauteur des `n` premières lignes d'un bloc.
    fn h_debut(b: &Bloc, n: usize) -> f32 {
        b.lignes.iter().take(n).map(|l| l.hauteur).sum()
    }

    fn bloc(&mut self, blocs: &Blocs, bi: usize, suivant: Option<&Bloc>) {
        let b = &blocs[bi];
        if b.saut {
            // Un saut en tête du document n'ajoute pas de page blanche.
            if !(self.vide && self.pages.len() <= 1) {
                self.nouvelle_page();
            }
            return;
        }
        if !self.vide {
            self.y += b.avant.min(self.place().max(0.0));
        }
        let total = b.lignes.len();
        let h_tout = Bloc::lignes_hauteur(b);
        // Un titre garde avec lui le début du bloc suivant ; un bloc insécable ou court
        // ne se coupe pas s'il tient sur une page.
        let mut besoin = if b.insecable || total <= 3 {
            h_tout
        } else {
            Self::h_debut(b, 2)
        };
        if b.garder
            && let Some(s) = suivant
        {
            besoin = h_tout + s.avant + Self::h_debut(s, 2.min(s.lignes.len()));
        }
        if !self.vide && besoin > self.place() + 0.01 && besoin <= self.g.th {
            self.nouvelle_page();
        }
        let mut li = 0;
        let mut garde = 0;
        while li < total {
            garde += 1;
            if garde > 4 * total + 8 {
                break;
            }
            if !self.vide {
                // Lignes qui tiennent encore (sans compter les notes).
                let place = self.place();
                let mut k = 0;
                let mut h = 0.0;
                while li + k < total && h + b.lignes[li + k].hauteur <= place + 0.01 {
                    h += b.lignes[li + k].hauteur;
                    k += 1;
                }
                let reste = total - li;
                if k < reste {
                    // Orphelines : au moins deux lignes en bas de page ; veuves : au
                    // moins deux lignes en haut de la suivante.
                    if li == 0 && k < 2 && total >= 2 {
                        k = 0;
                    } else if reste - k == 1 && k >= 3 && total >= 4 {
                        k -= 1;
                    }
                    for _ in 0..k {
                        if !self.poser(blocs, bi, li, false) {
                            break;
                        }
                        li += 1;
                    }
                    self.nouvelle_page();
                    continue;
                }
            }
            if !self.poser(blocs, bi, li, false) {
                self.nouvelle_page();
                if !self.poser(blocs, bi, li, false) {
                    self.poser(blocs, bi, li, true);
                }
            }
            li += 1;
        }
        self.y += b.apres.min(self.place().max(0.0));
    }
}

impl Bloc {
    fn lignes_hauteur(b: &Bloc) -> f32 {
        b.lignes.iter().map(|l| l.hauteur).sum()
    }
}

/// Met en page les documents (un par destinataire en publipostage, chacun sur ses
/// propres pages).
pub fn mettre_en_page(docs: &[Rendered], page: &PageSetup, o: &Options, cache: &mut Cache) -> Mise {
    let g = Gabarit::new(page, o);
    let mut mise = Mise {
        largeur: g.largeur,
        hauteur: g.hauteur,
        zone: (g.ml, g.mt, g.tw, g.th),
        ..Mise::default()
    };
    let mut toutes: Vec<PageMise> = Vec::new();
    let mut titres_tous: Vec<TitrePlace> = Vec::new();
    let mut debuts = Vec::new();
    // Entrées de sommaire à compléter : (page, ligne placée, rang du titre, doc, décalage des titres).
    for (di, d) in docs.iter().enumerate() {
        let sig = cle_signatures(d);
        let ctx = Contexte {
            g,
            c: Composeur {
                f: o.polices,
                signatures: &d.signatures,
            },
        };
        let mut liste: Vec<Blocs> = Vec::with_capacity(d.paras.len());
        for (i, p) in d.paras.iter().enumerate() {
            let f = d.format(i);
            let cle = cle_para(&g, p, &f, sig, 0);
            liste.push(cache.obtenir(cle, || ctx.blocs(p, &f)));
        }
        // Notes : petit corps, pleine largeur.
        let taille_note = g.taille * 0.83;
        let mut notes: Vec<Blocs> = Vec::new();
        for (k, contenu) in d.notes.iter().enumerate() {
            let mut runs = vec![Run::Text {
                text: format!("{} ", k + 1),
                style: Style {
                    exposant: true,
                    ..Style::default()
                },
            }];
            runs.extend(contenu.iter().cloned());
            let para = Para::Text {
                align: Align::Left,
                runs,
            };
            let mut gn = g;
            gn.taille = taille_note;
            let cle = cle_para(&gn, &para, &ParaFormat::default(), sig, 1);
            let ctxn = Contexte {
                g: Gabarit {
                    interligne: 1.0,
                    justifier: false,
                    ..gn
                },
                c: Composeur {
                    f: o.polices,
                    signatures: &d.signatures,
                },
            };
            notes.push(cache.obtenir(cle, || {
                let Para::Text { runs, .. } = &para else {
                    return Vec::new();
                };
                vec![ctxn.bloc_texte(
                    runs,
                    &ParaFormat::default(),
                    taille_note,
                    Style::default(),
                    Align::Left,
                    0.0,
                    0.0,
                )]
            }));
        }
        let notes_doc = |n: usize| n.checked_sub(1).and_then(|i| notes.get(i)).cloned();
        let mut p = Paginateur {
            g,
            pages: Vec::new(),
            y: 0.0,
            vide: true,
            notes_page: Vec::new(),
            h_notes: 0.0,
            titres: Vec::new(),
            doc: di,
            notes_doc: &notes_doc,
        };
        p.nouvelle_page();
        let plat: Vec<(usize, usize)> = liste
            .iter()
            .enumerate()
            .flat_map(|(i, b)| (0..b.len()).map(move |j| (i, j)))
            .collect();
        for (k, &(i, j)) in plat.iter().enumerate() {
            let suivant = plat.get(k + 1).map(|&(a, b)| &liste[a][b]);
            p.bloc(&liste[i], j, suivant);
        }
        p.finir_notes();
        let decal = toutes.len();
        let base_titres = titres_tous.len();
        debuts.push((decal, base_titres));
        for mut t in p.titres {
            t.page += decal;
            titres_tous.push(t);
        }
        toutes.extend(p.pages);
    }
    if toutes.is_empty() {
        toutes.push(PageMise::default());
        debuts.push((0, 0));
    }
    // Table des matières : points de conduite, numéros de page et liens.
    let premiere_page = |doc: usize| debuts.get(doc).map_or(0, |d| d.0);
    let base_titre = |doc: usize| debuts.get(doc).map_or(0, |d| d.1);
    let ctx0 = Composeur {
        f: o.polices,
        signatures: &std::collections::BTreeMap::new(),
    };
    for pi in 0..toutes.len() {
        let doc = toutes[pi].doc;
        let mut ajouts = Vec::new();
        for pl in &toutes[pi].lignes {
            let l = pl.ligne();
            let Some((k, fin, base, taille, gras)) = l.sommaire else {
                continue;
            };
            let Some(t) = titres_tous.get(base_titre(doc) + k) else {
                continue;
            };
            let num = (t.page - premiere_page(doc) + 1).to_string();
            let st = Style {
                bold: gras,
                ..Style::default()
            };
            let mut m = Vec::new();
            ctx0.morceaux(&num, st, Genre::Normal, taille, &None, &mut m);
            let lw: f32 = m.iter().map(|m| m.largeur).sum();
            let droite = g.ml + g.tw;
            let mut x = droite - lw;
            let y = pl.y + base;
            let mut points = Vec::new();
            ctx0.morceaux(".", st, Genre::Normal, taille, &None, &mut points);
            if let Some(pt) = points.first() {
                let pas = taille * 0.45;
                let debut = pl.x + fin + taille * 0.4;
                let fin_points = x - taille * 0.4;
                let mut px = (debut / pas).ceil() * pas;
                let mut gl = Vec::new();
                let mut tx = String::new();
                while px + pt.largeur <= fin_points {
                    gl.push(px);
                    tx.push('.');
                    px += pas;
                }
                for xp in gl {
                    ajouts.push(Element::Texte(Texte {
                        x: xp,
                        y,
                        taille,
                        police: pt.police,
                        glyphes: pt.glyphes.clone(),
                        texte: ".".into(),
                        gras,
                        italique: false,
                        oblique: false,
                        couleur: Couleur::rvb(0x555555),
                        largeur: pt.largeur,
                        genre: Genre::Normal,
                    }));
                }
            }
            for mm in m {
                ajouts.push(Element::Texte(Texte {
                    x,
                    y,
                    taille: mm.taille,
                    police: mm.police,
                    glyphes: mm.glyphes,
                    texte: mm.texte,
                    gras,
                    italique: false,
                    oblique: mm.oblique,
                    couleur: ENCRE,
                    largeur: mm.largeur,
                    genre: Genre::Normal,
                }));
                x += mm.largeur;
            }
            ajouts.push(Element::Lien {
                x: pl.x,
                y: pl.y,
                l: droite - pl.x,
                h: l.hauteur,
                cible: Cible::Titre(base_titre(doc) + k),
            });
        }
        toutes[pi].extra.extend(ajouts);
    }
    // En-têtes, pieds de page et numéros.
    let small = g.taille * 0.75;
    for di in 0..docs.len().max(1) {
        let (a, _) = debuts.get(di).copied().unwrap_or((0, 0));
        let b = debuts.get(di + 1).map_or(toutes.len(), |d| d.0);
        let total = b.saturating_sub(a);
        let d = docs.get(di);
        let ctx = Contexte {
            g,
            c: Composeur {
                f: o.polices,
                signatures: d.map_or(&EMPTY_SIG, |d| &d.signatures),
            },
        };
        let ligne_marge = |runs: &[Run], largeur: f32| -> Option<Ligne> {
            if runs.is_empty() {
                return None;
            }
            let at = ctx.c.atomes(runs, small, Style::default(), largeur);
            let lignes = decouper(&ctx.c, at, largeur, largeur);
            poser_lignes(
                &g,
                lignes.into_iter().take(1).collect(),
                &Pose {
                    taille: small,
                    hl: small * g.simple,
                    align: Align::Left,
                    gauche: 0.0,
                    largeur,
                    alinea: 0.0,
                },
            )
            .into_iter()
            .next()
        };
        let largeur_ligne = |l: &Ligne| {
            l.elements
                .iter()
                .filter_map(|e| match e {
                    Element::Texte(t) => Some(t.x + t.largeur),
                    _ => None,
                })
                .fold(0.0f32, f32::max)
        };
        let entete = d.and_then(|d| ligne_marge(&d.header, g.tw));
        let pied = d.and_then(|d| ligne_marge(&d.footer, g.tw * 0.7));
        for (k, pg) in toutes.iter_mut().enumerate().take(b).skip(a) {
            let mut extra = Vec::new();
            let y_pied = g.hauteur - (g.mb / 2.0).max(small) - small * 0.6;
            if let Some(l) = &entete {
                let x = g.ml + (g.tw - largeur_ligne(l)) / 2.0;
                let y = (g.mt / 2.0 - l.hauteur / 2.0).max(0.0);
                extra.extend(l.elements.iter().map(|e| marge(e.decaler(x, y))));
            }
            if let Some(l) = &pied {
                extra.extend(l.elements.iter().map(|e| marge(e.decaler(g.ml, y_pied))));
            }
            if page.page_numbers {
                let txt = o
                    .numero
                    .replace("{n}", &(k - a + 1).to_string())
                    .replace("{total}", &total.to_string());
                if let Some(l) = ligne_marge(&[texte_simple(&txt)], g.tw) {
                    let x = g.ml + g.tw - largeur_ligne(&l);
                    extra.extend(l.elements.iter().map(|e| marge(e.decaler(x, y_pied))));
                }
            }
            pg.extra.extend(extra);
        }
    }
    cache.nettoyer();
    mise.pages = toutes;
    mise.titres = titres_tous;
    mise
}

static EMPTY_SIG: std::collections::BTreeMap<String, crate::signature::Signature> =
    std::collections::BTreeMap::new();

/// Texte d'en-tête ou de pied de page : gris.
fn marge(mut e: Element) -> Element {
    if let Element::Texte(t) = &mut e {
        if t.genre == Genre::Normal {
            t.couleur = t.couleur.max_gris(GRIS_MARGE);
        }
        t.genre = match t.genre {
            Genre::Normal => Genre::Marge,
            g => g,
        };
    }
    e
}

impl Couleur {
    /// L'encre par défaut devient `gris` ; une couleur choisie reste.
    fn max_gris(self, gris: Couleur) -> Couleur {
        if self == ENCRE { gris } else { self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Values;

    fn polices() -> PdfFonts {
        let d = |f: &str| {
            std::fs::read(format!(
                "{}/../varium/assets/fonts/{f}",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap()
        };
        PdfFonts::from_ttf(
            &d("LiberationSerif-Regular.ttf"),
            &d("LiberationSerif-Bold.ttf"),
            &d("LiberationSerif-Italic.ttf"),
            &d("LiberationSerif-BoldItalic.ttf"),
        )
    }

    fn mise(src: &str, page: &PageSetup, f: &PdfFonts) -> Mise {
        let d = crate::export::prepare(src, &Values::default(), page);
        mettre_en_page(&[d], page, &Options::new(f), &mut Cache::new())
    }

    fn textes(p: &PageMise) -> String {
        let mut s = String::new();
        p.pour_chaque(|e| {
            if let Element::Texte(t) = e {
                s.push_str(&t.texte);
                s.push(' ');
            }
        });
        s
    }

    #[test]
    fn notes_en_bas_de_leur_page() {
        let f = polices();
        let page = PageSetup::default();
        let mut src = String::new();
        for i in 0..120 {
            src.push_str(&format!(
                "Paragraphe {i} assez long pour remplir des lignes de texte.\n"
            ));
            if i == 70 {
                src.push_str("Ici une note[^a].\n");
            }
        }
        src.push_str("[^a]: Le contenu de la note.\n");
        let m = mise(&src, &page, &f);
        assert!(m.pages.len() >= 2);
        let ou: Vec<usize> = m
            .pages
            .iter()
            .enumerate()
            .filter(|(_, p)| textes(p).contains("contenu"))
            .map(|(i, _)| i)
            .collect();
        let appel: Vec<usize> = m
            .pages
            .iter()
            .enumerate()
            .filter(|(_, p)| textes(p).contains("Ici"))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(ou, appel, "la note est sur la page de son appel");
        // La note est sous le texte de la page.
        let p = &m.pages[ou[0]];
        let (mut y_note, mut y_max) = (0.0f32, 0.0f32);
        p.pour_chaque(|e| {
            if let Element::Texte(t) = e {
                if t.texte.contains("contenu") {
                    y_note = t.y;
                } else if t.genre == Genre::Normal && t.texte.starts_with("Paragraphe") {
                    y_max = y_max.max(t.y);
                }
            }
        });
        assert!(y_note > y_max);
        assert!(y_note < m.hauteur - m.zone.1 / 2.0);
    }

    #[test]
    fn sommaire_avec_numeros() {
        let f = polices();
        let page = PageSetup::default();
        let src = "\\sommaire\n# Un\ntexte\n\\newpage\n# Deux\n\\newpage\n\\newpage\n## Trois\n";
        let m = mise(src, &page, &f);
        assert_eq!(m.titres.len(), 3);
        assert_eq!(m.titres[1].page, 1);
        let t = textes(&m.pages[0]);
        assert!(
            t.contains("Un") && t.contains(" 1 ") && t.contains(" 2 ") && t.contains(" 4 "),
            "{t}"
        );
        assert!(t.matches(". ").count() > 20, "points de conduite");
        let liens = m.pages[0]
            .elements()
            .into_iter()
            .filter(|e| {
                matches!(
                    e,
                    Element::Lien {
                        cible: Cible::Titre(_),
                        ..
                    }
                )
            })
            .count();
        assert_eq!(liens, 3);
    }

    #[test]
    fn formats_de_paragraphe() {
        let f = polices();
        let page = PageSetup::default();
        let m = mise("{: retrait=2cm alinéa=1cm} Texte\nTexte", &page, &f);
        let mut xs = Vec::new();
        m.pages[0].pour_chaque(|e| {
            if let Element::Texte(t) = e {
                xs.push(t.x);
            }
        });
        let ml = m.zone.0;
        assert!((xs[0] - ml - 30.0 * PT_MM).abs() < 0.5, "{xs:?}");
        assert!((xs[1] - ml).abs() < 0.5);
        // Interligne double : lignes deux fois plus espacées.
        let src = "{: interligne=double}\n".to_string() + &"mot ".repeat(200);
        let m2 = mise(&src, &page, &f);
        let m1 = mise(&"mot ".repeat(200), &page, &f);
        let ecart = |m: &Mise| {
            let mut ys: Vec<f32> = Vec::new();
            m.pages[0].pour_chaque(|e| {
                if let Element::Texte(t) = e
                    && !ys.contains(&t.y)
                {
                    ys.push(t.y);
                }
            });
            ys[1] - ys[0]
        };
        let (e1, e2) = (ecart(&m1), ecart(&m2));
        assert!((e2 / e1 - 2.0 / 1.15).abs() < 0.05, "{e1} {e2}");
    }

    #[test]
    fn couleurs_et_surlignage() {
        let f = polices();
        let page = PageSetup::default();
        let m = mise("{couleur:rouge}rouge{/} et ==jaune==", &page, &f);
        let el = m.pages[0].elements();
        assert!(el.iter().any(|e| matches!(e, Element::Texte(t) if t.texte == "rouge" && t.couleur == Couleur::rvb(0xC9211E))));
        assert!(el.iter().any(
            |e| matches!(e, Element::Rect { couleur, .. } if *couleur == Couleur::SURLIGNAGE)
        ));
    }

    #[test]
    fn cache_reutilise() {
        let f = polices();
        let page = PageSetup::default();
        let d = crate::export::prepare(&"Un paragraphe.\n".repeat(50), &Values::default(), &page);
        let mut c = Cache::new();
        let _ = mettre_en_page(std::slice::from_ref(&d), &page, &Options::new(&f), &mut c);
        let n = c.len();
        let _ = mettre_en_page(&[d], &page, &Options::new(&f), &mut c);
        assert_eq!(c.len(), n);
        assert!(n >= 1);
    }

    #[test]
    fn longs_documents_rapides() {
        // Seulement en mode optimisé : le budget n'a pas de sens en débogage.
        if cfg!(debug_assertions) {
            return;
        }
        let f = polices();
        let page = PageSetup::default();
        let mut src = String::from("\\sommaire\n");
        for i in 0..1500 {
            if i % 50 == 0 {
                src.push_str(&format!("# Chapitre {i}\n"));
            }
            src.push_str(&format!("Paragraphe {i} : un texte **assez** long, avec une note[^n{i}] et des mots, pour remplir trois ou quatre lignes de la page. Encore quelques mots pour que la ligne se coupe ici et là, de manière réaliste.\n"));
            src.push_str(&format!("[^n{i}]: Note {i}.\n"));
        }
        let d = crate::export::prepare(&src, &Values::default(), &page);
        let mut c = Cache::new();
        let t = std::time::Instant::now();
        let m = mettre_en_page(std::slice::from_ref(&d), &page, &Options::new(&f), &mut c);
        let premiere = t.elapsed();
        assert!(m.pages.len() >= 300, "{} pages", m.pages.len());
        let t = std::time::Instant::now();
        let _ = mettre_en_page(&[d], &page, &Options::new(&f), &mut c);
        let seconde = t.elapsed();
        assert!(
            premiere.as_millis() < 3000,
            "première mise en page : {premiere:?}"
        );
        assert!(seconde.as_millis() < 150, "avec le cache : {seconde:?}");
    }
}

// ---------------------------------------------------------------------------
// Polices par défaut (pour les exports qui ont besoin de mesurer : numéros de page
// de la table des matières en ODT et DOCX, taille des formules)
// ---------------------------------------------------------------------------

static POLICES: std::sync::RwLock<Vec<(crate::page::DocFont, Arc<PdfFonts>)>> =
    std::sync::RwLock::new(Vec::new());

/// Enregistre les polices d'une famille (l'application le fait au démarrage).
pub fn enregistrer_polices(famille: crate::page::DocFont, p: PdfFonts) {
    if let Ok(mut v) = POLICES.write() {
        v.retain(|(f, _)| *f != famille);
        v.push((famille, Arc::new(p)));
    }
}

/// Les polices enregistrées pour une famille, sinon des largeurs approchées.
pub fn polices_pour(famille: crate::page::DocFont) -> Arc<PdfFonts> {
    POLICES
        .read()
        .ok()
        .and_then(|v| {
            v.iter()
                .find(|(f, _)| *f == famille)
                .map(|(_, p)| p.clone())
        })
        .unwrap_or_else(|| Arc::new(PdfFonts::approximate()))
}

/// Pour chaque document, le numéro de page (à partir de 1) de chacun de ses titres.
pub fn pages_des_titres(docs: &[Rendered], page: &PageSetup) -> Vec<Vec<usize>> {
    let f = polices_pour(page.font);
    let m = mettre_en_page(docs, page, &Options::new(&f), &mut Cache::new());
    let mut premiere = vec![usize::MAX; docs.len()];
    for (i, p) in m.pages.iter().enumerate() {
        if let Some(x) = premiere.get_mut(p.doc) {
            *x = (*x).min(i);
        }
    }
    let mut out = vec![Vec::new(); docs.len()];
    for t in &m.titres {
        let doc = m.pages.get(t.page).map_or(0, |p| p.doc);
        if let (Some(v), Some(&p0)) = (out.get_mut(doc), premiere.get(doc)) {
            v.push(t.page.saturating_sub(p0) + 1);
        }
    }
    out
}

/// Largeur, hauteur et profondeur (points) d'une formule composée.
pub fn mesure_formule(
    tex: &str,
    centree: bool,
    taille: f32,
    famille: crate::page::DocFont,
) -> (f32, f32, f32) {
    let f = polices_pour(famille);
    let mesure = |c: char, it: bool| f.glyphe(c, indice_style(false, it)).avance;
    let b = formule::composer(&formule::lire(tex), taille, centree, &mesure);
    (b.largeur, b.hauteur, b.profondeur)
}
