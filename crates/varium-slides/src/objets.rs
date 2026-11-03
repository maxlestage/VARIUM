//! Objets posés sur une diapositive : images et formes (rectangle, rectangle
//! arrondi, ellipse, ligne, flèche), et les médias (images) de la présentation.
//!
//! Une image n'est stockée qu'une fois par présentation, dans [`Deck::media`],
//! sous un identifiant tiré de son contenu ; chaque objet image y renvoie. Les
//! positions sont des fractions de la diapositive, comme les zones de texte.

use std::io::Cursor;
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::texte::contraste;
use crate::{Deck, Frame, Palette, Slide};

/// Taille maximale d'une image insérée.
pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
/// Côté maximal d'une image : au-delà, elle est réduite à l'insertion.
pub const MAX_IMAGE_SIDE: u32 = 2560;
/// Nombre maximal de pixels d'une image à décoder (protection mémoire).
const MAX_PIXELS: u64 = 120_000_000;

// ---------------------------------------------------------------------------------
// Objets
// ---------------------------------------------------------------------------------

/// Nature d'un objet posé sur une diapositive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ObjectKind {
    /// Image (PNG ou JPEG) de la présentation.
    Image,
    #[default]
    Rect,
    RoundRect,
    Ellipse,
    /// Segment entre deux coins opposés du cadre.
    Line,
    /// Segment terminé par une pointe.
    Arrow,
}

identifiants!(ObjectKind {
    Image => "image",
    Rect => "rectangle",
    RoundRect => "rectangle_arrondi",
    Ellipse => "ellipse",
    Line => "ligne",
    Arrow => "fleche",
});

impl ObjectKind {
    /// Les formes que l'on peut insérer (tout sauf l'image).
    pub const SHAPES: [ObjectKind; 5] = [
        ObjectKind::Rect,
        ObjectKind::RoundRect,
        ObjectKind::Ellipse,
        ObjectKind::Line,
        ObjectKind::Arrow,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ObjectKind::Image => "Image",
            ObjectKind::Rect => "Rectangle",
            ObjectKind::RoundRect => "Rectangle arrondi",
            ObjectKind::Ellipse => "Ellipse",
            ObjectKind::Line => "Ligne",
            ObjectKind::Arrow => "Flèche",
        }
    }

    /// Ligne ou flèche : un trait, sans surface ni texte.
    pub fn is_line(self) -> bool {
        matches!(self, ObjectKind::Line | ObjectKind::Arrow)
    }

    /// Forme fermée, qui peut avoir un fond et du texte.
    pub fn is_closed(self) -> bool {
        matches!(
            self,
            ObjectKind::Rect | ObjectKind::RoundRect | ObjectKind::Ellipse
        )
    }
}

/// Un objet d'une diapositive : image ou forme, dessiné au-dessus des zones de
/// texte de la disposition, dans l'ordre de la liste (le dernier au premier plan).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Object {
    pub kind: ObjectKind,
    #[serde(deserialize_with = "crate::fichier::cadre")]
    pub frame: Frame,
    /// Image : identifiant du média dans [`Deck::media`].
    #[serde(
        deserialize_with = "crate::fichier::texte",
        skip_serializing_if = "String::is_empty"
    )]
    pub media: String,
    /// Couleur de fond d'une forme fermée (`None` : transparente).
    #[serde(
        serialize_with = "crate::fichier::couleur_ser",
        deserialize_with = "crate::fichier::couleur"
    )]
    pub fill: Option<[u8; 3]>,
    /// Couleur du trait (`None` : pas de trait).
    #[serde(
        serialize_with = "crate::fichier::couleur_ser",
        deserialize_with = "crate::fichier::couleur"
    )]
    pub stroke: Option<[u8; 3]>,
    /// Épaisseur du trait, en points d'une diapositive de 720 pt de large.
    #[serde(deserialize_with = "crate::fichier::nombre")]
    pub stroke_width: f32,
    /// Ligne et flèche : départ à droite du cadre (sinon à gauche).
    #[serde(deserialize_with = "crate::fichier::booleen")]
    pub flip_h: bool,
    /// Ligne et flèche : départ en bas du cadre (sinon en haut).
    #[serde(deserialize_with = "crate::fichier::booleen")]
    pub flip_v: bool,
    /// Texte centré dans une forme fermée (mise en forme des caractères, champs).
    #[serde(
        deserialize_with = "crate::fichier::texte",
        skip_serializing_if = "String::is_empty"
    )]
    pub text: String,
    /// Couleur du texte (`None` : choisie pour être lisible sur le fond).
    #[serde(
        serialize_with = "crate::fichier::couleur_ser",
        deserialize_with = "crate::fichier::couleur",
        skip_serializing_if = "Option::is_none"
    )]
    pub text_color: Option<[u8; 3]>,
    /// Taille du texte, en fraction de la hauteur de la diapositive.
    #[serde(deserialize_with = "crate::fichier::nombre")]
    pub text_size: f32,
}

impl Default for Object {
    fn default() -> Self {
        Object {
            kind: ObjectKind::Rect,
            frame: Frame::new(0.3, 0.3, 0.4, 0.3),
            media: String::new(),
            fill: None,
            stroke: None,
            stroke_width: 2.0,
            flip_h: false,
            flip_v: false,
            text: String::new(),
            text_color: None,
            text_size: 0.04,
        }
    }
}

/// Une valeur finie bornée, `defaut` sinon.
fn borne(v: f32, min: f32, max: f32, defaut: f32) -> f32 {
    if v.is_finite() {
        v.clamp(min, max)
    } else {
        defaut
    }
}

impl Object {
    /// Une forme du thème : fond d'accent, trait plus foncé ; un trait d'accent
    /// pour une ligne ou une flèche.
    pub fn shape(kind: ObjectKind, palette: &Palette) -> Object {
        let kind = if kind == ObjectKind::Image {
            ObjectKind::Rect
        } else {
            kind
        };
        if kind.is_line() {
            Object {
                kind,
                frame: Frame::new(0.3, 0.5, 0.4, 0.0),
                stroke: Some(palette.accent),
                stroke_width: 3.0,
                ..Object::default()
            }
        } else {
            Object {
                kind,
                frame: Frame::new(0.35, 0.35, 0.3, 0.3),
                fill: Some(palette.accent),
                stroke: None,
                stroke_width: 2.0,
                ..Object::default()
            }
        }
    }

    /// L'objet aux valeurs ramenées dans des bornes raisonnables (fichier abîmé,
    /// saisie extrême) : jamais de NaN ni de cadre démesuré.
    pub fn normalized(mut self) -> Object {
        let f = self.frame;
        self.frame = Frame::new(
            borne(f.x, -1.0, 2.0, 0.3),
            borne(f.y, -1.0, 2.0, 0.3),
            borne(f.w, 0.0, 3.0, 0.3),
            borne(f.h, 0.0, 3.0, 0.3),
        );
        self.stroke_width = borne(self.stroke_width, 0.0, 72.0, 2.0);
        self.text_size = borne(self.text_size, 0.005, 0.4, 0.04);
        if self.kind != ObjectKind::Image {
            self.media.clear();
        }
        self
    }

    /// Extrémités d'une ligne ou d'une flèche, en fractions de la diapositive :
    /// (départ, arrivée). La pointe d'une flèche est à l'arrivée.
    pub fn line_points(&self) -> ((f32, f32), (f32, f32)) {
        let f = self.frame;
        let (x1, x2) = if self.flip_h {
            (f.right(), f.x)
        } else {
            (f.x, f.right())
        };
        let (y1, y2) = if self.flip_v {
            (f.bottom(), f.y)
        } else {
            (f.y, f.bottom())
        };
        ((x1, y1), (x2, y2))
    }

    /// Couleur du texte : la sienne, sinon une couleur lisible sur le fond de la
    /// forme (ou sur celui de la diapositive pour une forme transparente).
    pub fn text_ink(&self, palette: &Palette) -> [u8; 3] {
        if let Some(c) = self.text_color {
            return c;
        }
        match self.fill {
            Some(fond) => {
                let candidats = [palette.title, palette.background, [255, 255, 255], [0, 0, 0]];
                candidats
                    .into_iter()
                    .find(|c| contraste(*c, fond) >= 4.5)
                    .unwrap_or_else(|| crate::texte::readable_on(fond))
            }
            None => palette.text,
        }
    }

    /// Rayon des coins d'un rectangle arrondi, en fraction du plus petit côté.
    pub const CORNER: f32 = 0.18;
}

/// Pointe d'une flèche allant de `a` à `b` (en points), pour un trait d'épaisseur
/// `epaisseur` : le point où s'arrête le trait, puis les trois sommets du
/// triangle (pointe, côté gauche, côté droit).
pub fn arrow_head(
    a: (f32, f32),
    b: (f32, f32),
    epaisseur: f32,
) -> ((f32, f32), [(f32, f32); 3]) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let long = (dx * dx + dy * dy).sqrt();
    let taille = (epaisseur * 3.5).max(8.0).min(long.max(0.001) * 0.6);
    let (ux, uy) = if long > 1e-4 {
        (dx / long, dy / long)
    } else {
        (1.0, 0.0)
    };
    let base = (b.0 - ux * taille, b.1 - uy * taille);
    let demi = taille * 0.5;
    let (nx, ny) = (-uy, ux);
    (
        base,
        [
            b,
            (base.0 + nx * demi, base.1 + ny * demi),
            (base.0 - nx * demi, base.1 - ny * demi),
        ],
    )
}

// ---------------------------------------------------------------------------------
// Médias
// ---------------------------------------------------------------------------------

/// Une image de la présentation. Les octets sont partagés : copier une
/// présentation (annulation) ne copie pas ses images.
#[derive(Debug, Clone, PartialEq)]
pub struct Media {
    /// `image/png` ou `image/jpeg`.
    pub mime: String,
    pub data: Arc<Vec<u8>>,
    /// Dimensions en pixels.
    pub width: u32,
    pub height: u32,
}

impl Media {
    /// Extension de fichier : `png` ou `jpg`.
    pub fn extension(&self) -> &'static str {
        if self.mime == "image/jpeg" {
            "jpg"
        } else {
            "png"
        }
    }

    /// Largeur divisée par hauteur (1 pour une image vide).
    pub fn ratio(&self) -> f32 {
        if self.width == 0 || self.height == 0 {
            1.0
        } else {
            self.width as f32 / self.height as f32
        }
    }

    /// Lit une image PNG ou JPEG : vérifiée, réduite si elle dépasse
    /// [`MAX_IMAGE_SIDE`] pixels de côté, convertie en RVB si c'est un JPEG CMJN.
    pub fn from_bytes(bytes: &[u8]) -> Result<Media, String> {
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(format!(
                "Image trop lourde ({:.1} Mo) : 20 Mo au maximum.",
                bytes.len() as f64 / (1024.0 * 1024.0)
            ));
        }
        let format = format_image(bytes).ok_or_else(|| {
            "Format d'image non pris en charge : seules les images PNG et JPEG sont acceptées."
                .to_string()
        })?;
        let illisible = || "Image illisible ou endommagée.".to_string();
        let mut lecteur = image::ImageReader::with_format(Cursor::new(bytes), format);
        let mut limites = image::Limits::default();
        limites.max_alloc = Some(1024 * 1024 * 1024);
        lecteur.limits(limites);
        let (w, h) = lecteur.into_dimensions().map_err(|_| illisible())?;
        if w == 0 || h == 0 {
            return Err(illisible());
        }
        if w as u64 * h as u64 > MAX_PIXELS {
            return Err(format!(
                "Image trop grande ({w} × {h} pixels) : 120 mégapixels au maximum."
            ));
        }
        let mut lecteur = image::ImageReader::with_format(Cursor::new(bytes), format);
        let mut limites = image::Limits::default();
        limites.max_alloc = Some(1024 * 1024 * 1024);
        lecteur.limits(limites);
        let img = lecteur.decode().map_err(|_| illisible())?;
        let jpeg = format == image::ImageFormat::Jpeg;
        let cmjn = jpeg && !matches!(jpeg_composantes(bytes), Some(1 | 3));
        if w.max(h) <= MAX_IMAGE_SIDE && !cmjn {
            return Ok(Media {
                mime: if jpeg { "image/jpeg" } else { "image/png" }.into(),
                data: Arc::new(bytes.to_vec()),
                width: w,
                height: h,
            });
        }
        let img = if w.max(h) > MAX_IMAGE_SIDE {
            let k = MAX_IMAGE_SIDE as f32 / w.max(h) as f32;
            let (nw, nh) = (
                ((w as f32 * k).round() as u32).max(1),
                ((h as f32 * k).round() as u32).max(1),
            );
            img.resize_exact(nw, nh, image::imageops::FilterType::Triangle)
        } else {
            img
        };
        let mut out = Vec::new();
        if jpeg {
            let rgb = img.to_rgb8();
            let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 88);
            rgb.write_with_encoder(enc).map_err(|_| illisible())?;
        } else {
            img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
                .map_err(|_| illisible())?;
        }
        Ok(Media {
            mime: if jpeg { "image/jpeg" } else { "image/png" }.into(),
            data: Arc::new(out),
            width: img.width(),
            height: img.height(),
        })
    }

    /// Identifiant tiré du contenu (FNV-1a 64 bits, en hexadécimal) : la même
    /// image insérée deux fois n'est stockée qu'une fois.
    pub fn id(&self) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for &b in self.data.iter() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        format!("{h:016x}")
    }
}

/// Format d'une image d'après ses premiers octets (PNG ou JPEG seulement).
pub(crate) fn format_image(bytes: &[u8]) -> Option<image::ImageFormat> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(image::ImageFormat::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(image::ImageFormat::Jpeg)
    } else {
        None
    }
}

/// Nombre de composantes d'un JPEG (1 gris, 3 RVB, 4 CMJN), lu dans son en-tête.
pub(crate) fn jpeg_composantes(b: &[u8]) -> Option<u8> {
    let mut i = 2usize;
    while i + 4 <= b.len() {
        if b[i] != 0xFF {
            return None;
        }
        let marque = b[i + 1];
        if marque == 0xFF {
            i += 1;
            continue;
        }
        let long = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        // Marqueurs SOF0 à SOF15, sauf DHT (C4), JPG (C8) et DAC (CC).
        if (0xC0..=0xCF).contains(&marque) && !matches!(marque, 0xC4 | 0xC8 | 0xCC) {
            return b.get(i + 9).copied();
        }
        if long < 2 {
            return None;
        }
        i += 2 + long;
    }
    None
}

impl Serialize for Media {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Fichier<'a> {
            mime: &'a str,
            width: u32,
            height: u32,
            data: String,
        }
        Fichier {
            mime: &self.mime,
            width: self.width,
            height: self.height,
            data: base64(&self.data),
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for Media {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        media_de_valeur(&v).ok_or_else(|| serde::de::Error::custom("image illisible"))
    }
}

/// Une image du fichier : ses octets doivent être une image PNG ou JPEG ; ses
/// dimensions sont relues dans l'image si elles manquent.
pub(crate) fn media_de_valeur(v: &serde_json::Value) -> Option<Media> {
    let data = unbase64(v.get("data")?.as_str()?)?;
    let format = format_image(&data)?;
    // Les dimensions sont toujours relues dans l'image : celles du fichier
    // pourraient être fausses.
    let (width, height) = image::ImageReader::with_format(Cursor::new(&data), format)
        .into_dimensions()
        .ok()
        .filter(|(w, h)| *w > 0 && *h > 0)?;
    Some(Media {
        mime: if format == image::ImageFormat::Jpeg {
            "image/jpeg"
        } else {
            "image/png"
        }
        .into(),
        data: Arc::new(data),
        width,
        height,
    })
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode en base64 (alphabet standard, avec `=`).
pub fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Décode du base64 (espaces ignorés, `=` final facultatif) ; `None` si invalide.
pub fn unbase64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0u32;
    let mut fin = false;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => {
                fin = true;
                continue;
            }
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        if fin {
            return None;
        }
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------------
// Opérations sur la présentation
// ---------------------------------------------------------------------------------

impl Deck {
    /// Ajoute une image (PNG ou JPEG) aux médias de la présentation et renvoie son
    /// identifiant. Erreur en français si l'image est trop lourde ou illisible.
    pub fn add_media(&mut self, bytes: &[u8]) -> Result<String, String> {
        let m = Media::from_bytes(bytes)?;
        let id = m.id();
        self.media.entry(id.clone()).or_insert(m);
        Ok(id)
    }

    /// Insère une image au centre de la diapositive `slide` (bornée), au plus
    /// 60 % de la diapositive, proportions conservées ; renvoie la position du
    /// nouvel objet dans `slides[slide].objects`.
    pub fn insert_image(&mut self, slide: usize, bytes: &[u8]) -> Result<usize, String> {
        let id = self.add_media(bytes)?;
        let ratio = self.media.get(&id).map(Media::ratio).unwrap_or(1.0);
        let frame = image_frame(ratio, self.aspect.ratio(), 0.6);
        let Some(last) = self.slides.len().checked_sub(1) else {
            return Err("La présentation n'a aucune diapositive.".into());
        };
        let s = &mut self.slides[slide.min(last)];
        s.objects.push(Object {
            kind: ObjectKind::Image,
            frame,
            media: id,
            stroke: None,
            fill: None,
            ..Object::default()
        });
        Ok(s.objects.len() - 1)
    }

    /// Insère une forme du thème sur la diapositive `slide` (bornée) ; renvoie sa
    /// position dans `objects`.
    pub fn insert_shape(&mut self, slide: usize, kind: ObjectKind) -> usize {
        let Some(last) = self.slides.len().checked_sub(1) else {
            return 0;
        };
        let i = slide.min(last);
        let pal = self.slide_palette(i);
        let s = &mut self.slides[i];
        s.objects.push(Object::shape(kind, &pal));
        s.objects.len() - 1
    }

    /// Les médias employés par au moins une diapositive.
    pub fn used_media(&self) -> std::collections::BTreeSet<&str> {
        self.slides
            .iter()
            .flat_map(|s| &s.objects)
            .filter(|o| o.kind == ObjectKind::Image)
            .map(|o| o.media.as_str())
            .filter(|id| self.media.contains_key(*id))
            .collect()
    }

    /// Retire les médias qu'aucune diapositive n'emploie plus.
    pub fn prune_media(&mut self) {
        let garder: std::collections::BTreeSet<String> =
            self.used_media().into_iter().map(String::from).collect();
        self.media.retain(|k, _| garder.contains(k));
    }
}

/// Cadre d'une image de proportions `image` (largeur / hauteur en pixels) centrée
/// sur une diapositive de proportions `slide`, occupant au plus `part` de chaque côté.
pub fn image_frame(image: f32, slide: f32, part: f32) -> Frame {
    let image = if image.is_finite() && image > 0.0 {
        image
    } else {
        1.0
    };
    // En fractions : h = w × slide / image.
    let mut w = part;
    let mut h = w * slide / image;
    if h > part {
        h = part;
        w = h * image / slide;
    }
    Frame::new((1.0 - w) / 2.0, (1.0 - h) / 2.0, w, h)
}

impl Slide {
    /// Copie l'objet `i` (borné), légèrement décalé, au premier plan ; renvoie sa
    /// position, ou `None` s'il n'y a aucun objet.
    pub fn duplicate_object(&mut self, i: usize) -> Option<usize> {
        let last = self.objects.len().checked_sub(1)?;
        let mut o = self.objects[i.min(last)].clone();
        o.frame.x += 0.02;
        o.frame.y += 0.02;
        self.objects.push(o);
        Some(self.objects.len() - 1)
    }

    /// Supprime l'objet `i` s'il existe.
    pub fn remove_object(&mut self, i: usize) {
        if i < self.objects.len() {
            self.objects.remove(i);
        }
    }

    /// Passe l'objet `i` (borné) au premier plan ; renvoie sa nouvelle position.
    pub fn bring_to_front(&mut self, i: usize) -> usize {
        let Some(last) = self.objects.len().checked_sub(1) else {
            return 0;
        };
        let o = self.objects.remove(i.min(last));
        self.objects.push(o);
        last
    }

    /// Passe l'objet `i` (borné) à l'arrière-plan ; renvoie sa nouvelle position.
    pub fn send_to_back(&mut self, i: usize) -> usize {
        let Some(last) = self.objects.len().checked_sub(1) else {
            return 0;
        };
        let o = self.objects.remove(i.min(last));
        self.objects.insert(0, o);
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_aller_retour() {
        for n in 0..20 {
            let d: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(unbase64(&base64(&d)).unwrap(), d);
        }
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(unbase64("TW\nFu").unwrap(), b"Man");
        assert!(unbase64("TW$u").is_none());
    }

    #[test]
    fn composantes_jpeg() {
        let img = image::RgbImage::from_pixel(3, 2, image::Rgb([200, 10, 10]));
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Jpeg)
            .unwrap();
        assert_eq!(jpeg_composantes(&out), Some(3));
        assert_eq!(jpeg_composantes(&[0xFF, 0xD8, 0xFF]), None);
    }

    #[test]
    fn cadre_d_image_proportionne() {
        // Image carrée sur une diapositive 16:9 : plus haute que large en fractions.
        let f = image_frame(1.0, 16.0 / 9.0, 0.6);
        assert!((f.h - 0.6).abs() < 1e-5);
        assert!((f.w * 16.0 / 9.0 - f.h).abs() < 1e-5);
        assert!((f.x + f.w / 2.0 - 0.5).abs() < 1e-5);
    }

    #[test]
    fn pointe_de_fleche() {
        let (base, tri) = arrow_head((0.0, 0.0), (100.0, 0.0), 2.0);
        assert!(base.0 < 100.0 && tri[0] == (100.0, 0.0));
        assert!(tri[1].1 != tri[2].1);
    }
}
