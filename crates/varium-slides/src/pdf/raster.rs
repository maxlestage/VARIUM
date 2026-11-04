//! Export en images : chaque diapositive en PNG (1920 × 1080 en 16:9).
//!
//! L'image est tirée du dessin PDF de la diapositive : le flux de contenu de la
//! page est relu et peint avec `tiny-skia` (tracés, traits, images), le texte
//! avec les polices Liberation (mêmes métriques qu'Helvetica et Times : la mise
//! en page est celle du PDF, au point près).

use std::collections::BTreeMap;
use std::rc::Rc;

use ab_glyph::{Font, FontRef, OutlineCurve};
use tiny_skia::{
    FillRule, FilterQuality, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform,
};

use super::{LARGEUR_PAGE, WIN_80_9F, hauteur_diapo, page};
use crate::Deck;

/// Les polices du rendu en images (fichiers TrueType). Une police illisible
/// n'empêche pas le rendu : son texte est omis.
#[derive(Debug, Clone, Copy)]
pub struct RasterFonts<'a> {
    pub sans: &'a [u8],
    pub sans_bold: &'a [u8],
    pub serif: &'a [u8],
    pub serif_bold: &'a [u8],
    pub serif_italic: &'a [u8],
    pub serif_bold_italic: &'a [u8],
}

/// Hauteur des images exportées, en pixels.
pub const PNG_HEIGHT: u32 = 1080;

/// Lexème d'un flux de contenu PDF.
#[derive(Debug, Clone, PartialEq)]
enum Lex {
    Nombre(f32),
    Nom(String),
    Octets(Vec<u8>),
    Op(String),
}

fn lexemes(src: &str) -> Vec<Lex> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'<' {
            let fin = b[i + 1..]
                .iter()
                .position(|&x| x == b'>')
                .map_or(b.len(), |p| i + 1 + p);
            let hex: Vec<u8> = b[i + 1..fin]
                .iter()
                .copied()
                .filter(u8::is_ascii_hexdigit)
                .collect();
            let octets = hex
                .chunks(2)
                .filter_map(|p| {
                    let s = std::str::from_utf8(p).ok()?;
                    u8::from_str_radix(&format!("{s:0<2}"), 16).ok()
                })
                .collect();
            out.push(Lex::Octets(octets));
            i = fin + 1;
        } else if c == b'/' {
            let debut = i + 1;
            i += 1;
            while i < b.len() && !b[i].is_ascii_whitespace() && !b"/<[(".contains(&b[i]) {
                i += 1;
            }
            out.push(Lex::Nom(src[debut..i].to_string()));
        } else if c.is_ascii_digit() || c == b'-' || c == b'+' || c == b'.' {
            let debut = i;
            i += 1;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            out.push(Lex::Nombre(src[debut..i].parse().unwrap_or(0.0)));
        } else if c.is_ascii_alphabetic() || c == b'*' || c == b'\'' || c == b'"' {
            let debut = i;
            i += 1;
            while i < b.len() && (b[i].is_ascii_alphabetic() || b[i] == b'*') {
                i += 1;
            }
            out.push(Lex::Op(src[debut..i].to_string()));
        } else {
            i += 1;
        }
    }
    out
}

/// Un caractère WinAnsi.
fn depuis_winansi(b: u8) -> char {
    match b {
        0x80..=0x9F => WIN_80_9F[(b - 0x80) as usize],
        _ => b as char,
    }
}

#[derive(Clone)]
struct Etat {
    ctm: Transform,
    remplissage: [u8; 3],
    trait_: [u8; 3],
    epaisseur: f32,
    police: usize,
    taille: f32,
    approche: f32,
    clip: Option<Rc<Mask>>,
}

fn peinture(c: [u8; 3]) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c[0], c[1], c[2], 255);
    p.anti_alias = true;
    p
}

fn couleur(ops: &[f32]) -> Option<[u8; 3]> {
    let v = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    match ops {
        [.., r, g, b] => Some([v(*r), v(*g), v(*b)]),
        _ => None,
    }
}

/// Peint un flux de contenu PDF (celui d'une page `pw` × `ph` points) dans
/// `pixmap`, à l'échelle `s` pixels par point.
#[allow(clippy::too_many_arguments)]
fn peindre(
    pixmap: &mut Pixmap,
    flux: &str,
    s: f32,
    ph: f32,
    polices: &[Option<FontRef<'_>>; 8],
    images: &BTreeMap<String, Pixmap>,
) {
    let mut etat = Etat {
        ctm: Transform::from_row(s, 0.0, 0.0, -s, 0.0, ph * s),
        remplissage: [0, 0, 0],
        trait_: [0, 0, 0],
        epaisseur: 1.0,
        police: 0,
        taille: 12.0,
        approche: 0.0,
        clip: None,
    };
    let mut pile: Vec<Etat> = Vec::new();
    let mut chemin = PathBuilder::new();
    let mut vide = true;
    let mut couper = false;
    let mut operandes: Vec<f32> = Vec::new();
    let mut nom = String::new();
    let mut octets: Vec<u8> = Vec::new();
    let mut position = (0.0f32, 0.0f32);
    let (w, h) = (pixmap.width(), pixmap.height());
    for lex in lexemes(flux) {
        let op = match lex {
            Lex::Nombre(v) => {
                operandes.push(if v.is_finite() { v } else { 0.0 });
                continue;
            }
            Lex::Nom(n) => {
                nom = n;
                continue;
            }
            Lex::Octets(o) => {
                octets = o;
                continue;
            }
            Lex::Op(op) => op,
        };
        let o = &operandes;
        let a = |k: usize| o.get(k).copied().unwrap_or(0.0);
        match op.as_str() {
            "q" => pile.push(etat.clone()),
            "Q" => {
                if let Some(e) = pile.pop() {
                    etat = e;
                }
            }
            "cm" if o.len() >= 6 => {
                let m = Transform::from_row(a(0), a(1), a(2), a(3), a(4), a(5));
                etat.ctm = etat.ctm.pre_concat(m);
            }
            "rg" => etat.remplissage = couleur(o).unwrap_or(etat.remplissage),
            "RG" => etat.trait_ = couleur(o).unwrap_or(etat.trait_),
            "g" => {
                let g = a(0);
                etat.remplissage = couleur(&[g, g, g]).unwrap_or(etat.remplissage);
            }
            "G" => {
                let g = a(0);
                etat.trait_ = couleur(&[g, g, g]).unwrap_or(etat.trait_);
            }
            "w" => etat.epaisseur = a(0).max(0.0),
            "m" => {
                chemin.move_to(a(0), a(1));
                vide = false;
            }
            "l" => {
                chemin.line_to(a(0), a(1));
                vide = false;
            }
            "c" if o.len() >= 6 => {
                chemin.cubic_to(a(0), a(1), a(2), a(3), a(4), a(5));
                vide = false;
            }
            "h" => chemin.close(),
            "re" => {
                let (x, y, rw, rh) = (a(0), a(1), a(2), a(3));
                chemin.move_to(x, y);
                chemin.line_to(x + rw, y);
                chemin.line_to(x + rw, y + rh);
                chemin.line_to(x, y + rh);
                chemin.close();
                vide = false;
            }
            "W" | "W*" => couper = true,
            "f" | "f*" | "F" | "S" | "B" | "B*" | "b" | "n" => {
                let p = std::mem::replace(&mut chemin, PathBuilder::new());
                let fini = if vide { None } else { p.finish() };
                vide = true;
                if let Some(p) = fini {
                    let masque = etat.clip.as_deref();
                    if matches!(op.as_str(), "f" | "f*" | "F" | "B" | "B*" | "b") {
                        let regle = if op.ends_with('*') {
                            FillRule::EvenOdd
                        } else {
                            FillRule::Winding
                        };
                        pixmap.fill_path(&p, &peinture(etat.remplissage), regle, etat.ctm, masque);
                    }
                    if matches!(op.as_str(), "S" | "B" | "B*" | "b") {
                        let st = Stroke {
                            width: etat.epaisseur.max(0.01),
                            line_cap: tiny_skia::LineCap::Round,
                            line_join: tiny_skia::LineJoin::Round,
                            ..Stroke::default()
                        };
                        pixmap.stroke_path(&p, &peinture(etat.trait_), &st, etat.ctm, masque);
                    }
                    if couper && let Some(mut m) = Mask::new(w, h) {
                        m.fill_path(&p, FillRule::Winding, true, etat.ctm);
                        etat.clip = Some(Rc::new(m));
                    }
                }
                couper = false;
            }
            "BT" => position = (0.0, 0.0),
            "Tf" => {
                etat.police = nom
                    .trim_start_matches('F')
                    .parse::<usize>()
                    .unwrap_or(1)
                    .clamp(1, 8)
                    - 1;
                etat.taille = a(0).max(0.0);
            }
            "Tc" => etat.approche = a(0),
            "Tm" if o.len() >= 6 => position = (a(4), a(5)),
            "Td" if o.len() >= 2 => position = (position.0 + a(0), position.1 + a(1)),
            "Tj" => {
                if let Some(f) = &polices[etat.police] {
                    let texte = std::mem::take(&mut octets);
                    position.0 = glyphes(pixmap, f, &texte, position, &etat);
                }
            }
            "Do" => {
                if let Some(img) = images.get(nom.as_str()) {
                    let (iw, ih) = (img.width() as f32, img.height() as f32);
                    let t = etat.ctm.pre_concat(Transform::from_row(
                        1.0 / iw,
                        0.0,
                        0.0,
                        -1.0 / ih,
                        0.0,
                        1.0,
                    ));
                    let p = PixmapPaint {
                        quality: FilterQuality::Bilinear,
                        ..PixmapPaint::default()
                    };
                    pixmap.draw_pixmap(0, 0, img.as_ref(), &p, t, etat.clip.as_deref());
                }
            }
            _ => {}
        }
        operandes.clear();
    }
}

/// Dessine un texte à partir de `(x, y)` (espace utilisateur) ; renvoie la
/// position après le texte.
fn glyphes(
    pixmap: &mut Pixmap,
    f: &FontRef<'_>,
    texte: &[u8],
    (mut x, y): (f32, f32),
    etat: &Etat,
) -> f32 {
    let upm = f.units_per_em().unwrap_or(1000.0).max(1.0);
    let em = etat.taille / upm;
    let couleur = peinture(etat.remplissage);
    for &b in texte {
        let c = depuis_winansi(b);
        let id = f.glyph_id(c);
        if let Some(contour) = f.outline(id) {
            let mut pb = PathBuilder::new();
            let pt = |p: ab_glyph::Point| (x + p.x * em, y + p.y * em);
            let mut dernier: Option<(f32, f32)> = None;
            for courbe in &contour.curves {
                let (debut, fin) = match courbe {
                    OutlineCurve::Line(a, b) => (pt(*a), pt(*b)),
                    OutlineCurve::Quad(a, _, c) => (pt(*a), pt(*c)),
                    OutlineCurve::Cubic(a, _, _, d) => (pt(*a), pt(*d)),
                };
                if dernier != Some(debut) {
                    if dernier.is_some() {
                        pb.close();
                    }
                    pb.move_to(debut.0, debut.1);
                }
                match courbe {
                    OutlineCurve::Line(_, _) => pb.line_to(fin.0, fin.1),
                    OutlineCurve::Quad(_, b, _) => {
                        let b = pt(*b);
                        pb.quad_to(b.0, b.1, fin.0, fin.1);
                    }
                    OutlineCurve::Cubic(_, b, c, _) => {
                        let (b, c) = (pt(*b), pt(*c));
                        pb.cubic_to(b.0, b.1, c.0, c.1, fin.0, fin.1);
                    }
                }
                dernier = Some(fin);
            }
            pb.close();
            if let Some(p) = pb.finish() {
                pixmap.fill_path(
                    &p,
                    &couleur,
                    FillRule::Winding,
                    etat.ctm,
                    etat.clip.as_deref(),
                );
            }
        }
        x += f.h_advance_unscaled(id) * em + etat.approche;
    }
    x
}

/// Une image de la présentation, décodée en pixels prémultipliés.
fn pixmap_de(data: &[u8]) -> Option<Pixmap> {
    let img = image::load_from_memory(data).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let mut px = img.into_raw();
    for p in px.chunks_exact_mut(4) {
        let a = p[3] as u16;
        for c in &mut p[..3] {
            *c = ((*c as u16 * a + 127) / 255) as u8;
        }
    }
    Pixmap::from_vec(px, tiny_skia::IntSize::from_wh(w, h)?)
}

impl Deck {
    /// Les diapositives en images PNG de `height` pixels de haut (1080 donne
    /// 1920 × 1080 en 16:9, 1440 × 1080 en 4:3).
    pub fn to_png_images(&self, fonts: &RasterFonts<'_>, height: u32) -> Vec<Vec<u8>> {
        let height = height.clamp(16, 4320);
        let pw = LARGEUR_PAGE;
        let ph = hauteur_diapo(self);
        let s = height as f32 / ph;
        let width = (pw * s).round().max(1.0) as u32;
        let polices: [Option<FontRef<'_>>; 8] = [
            fonts.sans,
            fonts.sans_bold,
            fonts.sans,
            fonts.sans_bold,
            fonts.serif,
            fonts.serif_bold,
            fonts.serif_italic,
            fonts.serif_bold_italic,
        ]
        .map(|b| FontRef::try_from_slice(b).ok());
        // Les images, numérotées comme dans le PDF (`/Im1`, `/Im2`…).
        let mut nums = BTreeMap::new();
        let mut images = BTreeMap::new();
        for (k, id) in self.used_media().into_iter().enumerate() {
            nums.insert(id.to_string(), k + 1);
            if let Some(p) = self.media.get(id).and_then(|m| pixmap_de(&m.data)) {
                images.insert(format!("Im{}", k + 1), p);
            }
        }
        let rendus = self.rendus();
        let slides = self.exported();
        rendus
            .iter()
            .zip(slides.iter())
            .map(|(r, sl)| {
                let flux = page(r, sl, &nums, pw, ph);
                let Some(mut pixmap) = Pixmap::new(width, height) else {
                    return Vec::new();
                };
                pixmap.fill(tiny_skia::Color::WHITE);
                peindre(&mut pixmap, &flux, s, ph, &polices, &images);
                let rgb: Vec<u8> = pixmap
                    .data()
                    .chunks_exact(4)
                    .flat_map(|p| [p[0], p[1], p[2]])
                    .collect();
                let mut png = Vec::new();
                if let Some(img) = image::RgbImage::from_raw(width, height, rgb) {
                    let _ =
                        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png);
                }
                png
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn polices() -> RasterFonts<'static> {
        RasterFonts {
            sans: include_bytes!("../../../varium/assets/fonts/LiberationSans-Regular.ttf"),
            sans_bold: include_bytes!("../../../varium/assets/fonts/LiberationSans-Bold.ttf"),
            serif: include_bytes!("../../../varium/assets/fonts/LiberationSerif-Regular.ttf"),
            serif_bold: include_bytes!("../../../varium/assets/fonts/LiberationSerif-Bold.ttf"),
            serif_italic: include_bytes!("../../../varium/assets/fonts/LiberationSerif-Italic.ttf"),
            serif_bold_italic: include_bytes!(
                "../../../varium/assets/fonts/LiberationSerif-BoldItalic.ttf"
            ),
        }
    }

    #[test]
    fn lexemes_du_flux() {
        let l = lexemes("q 1 0 0 1 -2.5 3 cm /Im4 Do Q BT /F2 12 Tf <48C9> Tj ET");
        assert_eq!(l[0], Lex::Op("q".into()));
        assert_eq!(l[5], Lex::Nombre(-2.5));
        assert_eq!(l[8], Lex::Nom("Im4".into()));
        assert!(l.contains(&Lex::Octets(vec![0x48, 0xC9])));
        assert_eq!(depuis_winansi(0x80), '€');
        assert_eq!(depuis_winansi(0xE9), 'é');
        // Flux abîmé : pas de panique.
        let _ = lexemes("<4 <<< /// -- . ..");
    }

    #[test]
    fn images_de_l_exemple() {
        let mut deck = Deck::sample();
        let png = {
            let img = image::RgbaImage::from_pixel(8, 4, image::Rgba([250, 10, 10, 255]));
            let mut v = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut v), image::ImageFormat::Png)
                .unwrap();
            v
        };
        let k = deck.insert_image(1, &png).unwrap();
        deck.slides[1].objects[k].frame = crate::Frame::new(0.0, 0.0, 0.25, 0.25);
        let images = deck.to_png_images(&polices(), PNG_HEIGHT);
        assert_eq!(images.len(), deck.slides.len());
        let img = image::load_from_memory(&images[0]).unwrap().to_rgb8();
        assert_eq!(img.dimensions(), (1920, 1080));
        // Le fond du thème Océan.
        assert_eq!(img.get_pixel(5, 5).0, [11, 37, 64]);
        // Le titre blanc est dessiné : des pixels clairs au milieu de la diapositive.
        let clairs = (0..1920)
            .flat_map(|x| (300..560).map(move |y| (x, y)))
            .filter(|&(x, y)| img.get_pixel(x, y).0[0] > 200)
            .count();
        assert!(clairs > 2000, "{clairs}");
        // L'image rouge en haut à gauche de la deuxième diapositive.
        let img = image::load_from_memory(&images[1]).unwrap().to_rgb8();
        let p = img.get_pixel(100, 100).0;
        assert!(p[0] > 230 && p[1] < 40, "{p:?}");
        // 4:3 : 1440 × 1080.
        deck.aspect = crate::Aspect::Standard;
        let images = deck.to_png_images(&polices(), PNG_HEIGHT);
        let img = image::load_from_memory(&images[0]).unwrap();
        assert_eq!((img.width(), img.height()), (1440, 1080));
        // Polices illisibles : le rendu se fait sans texte.
        let vides = RasterFonts {
            sans: b"",
            sans_bold: b"",
            serif: b"",
            serif_bold: b"",
            serif_italic: b"",
            serif_bold_italic: b"",
        };
        assert_eq!(deck.to_png_images(&vides, 64).len(), deck.slides.len());
    }
}
