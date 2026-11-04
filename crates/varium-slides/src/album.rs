//! Album photo : une présentation tirée d'un dossier d'images, une diapositive par
//! image, et la diapositive « Partager » (code QR de l'adresse de la présentation).
//!
//! Les images sont classées par date de prise de vue (EXIF `DateTimeOriginal`)
//! quand elle se lit, sinon par nom (les nombres dans l'ordre naturel :
//! `photo2` avant `photo10`). Les dispositions alternent entre plein cadre
//! (l'image couvre toute la diapositive) et encadrée (passe-partout blanc sur le
//! fond du thème) ; la légende, facultative, vient du nom du fichier. L'orientation
//! EXIF (photo prise de côté) devient une rotation de l'objet.

use crate::objets::{Media, Object, ObjectKind};
use crate::{Deck, Frame, Layout, Slide, Theme, Transition};

/// Nombre maximal d'images d'un album.
pub const MAX_ALBUM_IMAGES: usize = 500;

/// Une image du dossier : nom du fichier et octets.
#[derive(Debug, Clone)]
pub struct AlbumImage {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Réglages de l'album.
#[derive(Debug, Clone, PartialEq)]
pub struct AlbumOptions {
    /// Titre de la première diapositive (aucune diapositive de titre s'il est vide).
    pub title: String,
    /// Sous-titre de la diapositive de titre.
    pub subtitle: String,
    /// Légende tirée du nom de chaque fichier.
    pub captions: bool,
    /// Alterner plein cadre et image encadrée (sinon : toujours plein cadre).
    pub alternate: bool,
    pub transition: Transition,
    /// Avance automatique après ce nombre de secondes par image.
    pub seconds: Option<f32>,
    pub theme: Theme,
}

impl Default for AlbumOptions {
    fn default() -> Self {
        AlbumOptions {
            title: String::new(),
            subtitle: String::new(),
            captions: true,
            alternate: true,
            transition: Transition::Fondu,
            seconds: Some(5.0),
            theme: Theme::Sombre,
        }
    }
}

// ---------------------------------------------------------------------------------
// EXIF
// ---------------------------------------------------------------------------------

/// Le bloc TIFF de l'EXIF d'un JPEG (segment APP1 `Exif\0\0`).
fn tiff_exif(b: &[u8]) -> Option<&[u8]> {
    if !b.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut i = 2usize;
    for _ in 0..64 {
        if b.get(i)? != &0xFF {
            return None;
        }
        let marque = *b.get(i + 1)?;
        if marque == 0xD8 || (0xD0..=0xD7).contains(&marque) || marque == 0x01 {
            i += 2;
            continue;
        }
        // Début des données de l'image : plus d'en-tête après.
        if marque == 0xDA || marque == 0xD9 {
            return None;
        }
        let long = u16::from_be_bytes([*b.get(i + 2)?, *b.get(i + 3)?]) as usize;
        if long < 2 {
            return None;
        }
        let donnees = b.get(i + 4..i + 2 + long)?;
        if marque == 0xE1 && donnees.starts_with(b"Exif\0\0") {
            return donnees.get(6..);
        }
        i += 2 + long;
    }
    None
}

/// Lecture d'un bloc TIFF (ordre des octets du fichier).
struct Tiff<'a> {
    b: &'a [u8],
    le: bool,
}

impl<'a> Tiff<'a> {
    fn new(b: &'a [u8]) -> Option<Self> {
        let le = match b.get(..4)? {
            [b'I', b'I', 42, 0] => true,
            [b'M', b'M', 0, 42] => false,
            _ => return None,
        };
        Some(Tiff { b, le })
    }

    fn u16(&self, at: usize) -> Option<u16> {
        let o: [u8; 2] = self.b.get(at..at.checked_add(2)?)?.try_into().ok()?;
        Some(if self.le {
            u16::from_le_bytes(o)
        } else {
            u16::from_be_bytes(o)
        })
    }

    fn u32(&self, at: usize) -> Option<u32> {
        let o: [u8; 4] = self.b.get(at..at.checked_add(4)?)?.try_into().ok()?;
        Some(if self.le {
            u32::from_le_bytes(o)
        } else {
            u32::from_be_bytes(o)
        })
    }

    /// Les entrées d'un répertoire (IFD) : (étiquette, type, nombre, position de la
    /// valeur ou de son pointeur).
    fn entrees(&self, ifd: usize) -> Vec<(u16, u16, u32, usize)> {
        let Some(n) = self.u16(ifd) else {
            return Vec::new();
        };
        (0..n.min(512) as usize)
            .filter_map(|k| {
                let e = ifd.checked_add(2 + 12 * k)?;
                Some((self.u16(e)?, self.u16(e + 2)?, self.u32(e + 4)?, e + 8))
            })
            .collect()
    }

    /// Texte ASCII d'une entrée.
    fn texte(&self, nombre: u32, valeur: usize) -> Option<String> {
        let n = (nombre as usize).min(64);
        let debut = if n <= 4 {
            valeur
        } else {
            self.u32(valeur)? as usize
        };
        let octets = self.b.get(debut..debut.checked_add(n)?)?;
        let t: String = octets
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as char)
            .collect();
        Some(t.trim().to_string())
    }
}

/// Une date EXIF « AAAA:MM:JJ HH:MM:SS » valable (pas « 0000:00:00 »).
fn date_valable(t: &str) -> bool {
    let b = t.as_bytes();
    b.len() >= 10
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..10].iter().all(u8::is_ascii_digit)
        && &b[..4] != b"0000"
}

/// Date de prise de vue d'un JPEG (`DateTimeOriginal`, sinon `DateTime`), au
/// format EXIF « AAAA:MM:JJ HH:MM:SS » (qui se trie comme du texte).
pub fn exif_date(bytes: &[u8]) -> Option<String> {
    let t = Tiff::new(tiff_exif(bytes)?)?;
    let ifd0 = t.u32(4)? as usize;
    let mut date = None;
    let mut exif = None;
    for (tag, _, n, v) in t.entrees(ifd0) {
        match tag {
            0x0132 => date = t.texte(n, v),
            0x8769 => exif = t.u32(v).map(|p| p as usize),
            _ => {}
        }
    }
    if let Some(p) = exif {
        for (tag, _, n, v) in t.entrees(p) {
            if matches!(tag, 0x9003 | 0x9004)
                && let Some(d) = t.texte(n, v).filter(|d| date_valable(d))
            {
                return Some(d);
            }
        }
    }
    date.filter(|d| date_valable(d))
}

/// Orientation EXIF d'un JPEG (1 à 8 ; 6 : à tourner de 90° dans le sens des
/// aiguilles d'une montre, 3 : 180°, 8 : 270°).
pub fn exif_orientation(bytes: &[u8]) -> Option<u16> {
    let t = Tiff::new(tiff_exif(bytes)?)?;
    let ifd0 = t.u32(4)? as usize;
    t.entrees(ifd0)
        .into_iter()
        .find(|e| e.0 == 0x0112)
        .and_then(|(_, _, _, v)| t.u16(v))
        .filter(|o| (1..=8).contains(o))
}

/// Rotation (degrés, sens des aiguilles d'une montre) d'une orientation EXIF.
fn rotation_exif(o: Option<u16>) -> f32 {
    match o {
        Some(3 | 4) => 180.0,
        Some(5 | 6) => 90.0,
        Some(7 | 8) => 270.0,
        _ => 0.0,
    }
}

// ---------------------------------------------------------------------------------
// Noms
// ---------------------------------------------------------------------------------

/// Clé de tri naturel : les suites de chiffres comparées comme des nombres.
pub fn natural_key(s: &str) -> Vec<(String, u128)> {
    let mut out = Vec::new();
    let mut texte = String::new();
    let mut chiffres = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_digit() {
            chiffres.push(c);
        } else {
            if !chiffres.is_empty() {
                let n = chiffres.parse::<u128>().unwrap_or(u128::MAX);
                out.push((std::mem::take(&mut texte), n));
                chiffres.clear();
            }
            texte.push(c);
        }
    }
    let n = if chiffres.is_empty() {
        0
    } else {
        chiffres.parse::<u128>().unwrap_or(u128::MAX)
    };
    out.push((texte, n));
    out
}

/// La légende tirée d'un nom de fichier : sans extension, `_` et `-` en espaces,
/// majuscule initiale ; rien pour un nom d'appareil photo (`IMG_1234`,
/// `DSC01234`, `PXL_20240101_…`) ou fait surtout de chiffres.
pub fn caption_from_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let base = match base.rsplit_once('.') {
        Some((b, ext)) if !b.is_empty() && ext.len() <= 5 => b,
        _ => base,
    };
    let t: String = base
        .chars()
        .map(|c| if c == '_' || c == '-' { ' ' } else { c })
        .collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let lettres = t.chars().filter(|c| c.is_alphabetic()).count();
    let chiffres = t.chars().filter(|c| c.is_ascii_digit()).count();
    let haut = t.to_uppercase();
    let appareil = [
        "IMG", "DSC", "DSCN", "DSCF", "PXL", "DCIM", "P", "GOPR", "MVIMG",
    ]
    .iter()
    .any(|p| {
        haut.strip_prefix(p).is_some_and(|reste| {
            reste
                .trim_start()
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
        })
    });
    if t.is_empty() || appareil || lettres < 2 || chiffres > lettres * 2 {
        return String::new();
    }
    let mut c = t.chars();
    let premiere: String = c
        .next()
        .map(|f| f.to_uppercase().collect())
        .unwrap_or_default();
    crate::import::echapper_markup(&format!("{premiere}{}", c.as_str()))
}

// ---------------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------------

/// Cadre d'une image affichée dans les proportions `ratio` (largeur / hauteur),
/// tournée de `rotation` degrés, sur une diapositive de proportions `slide` : le
/// cadre de l'objet (avant rotation) pour qu'une fois tournée elle occupe `vu`.
fn cadre_tourne(vu: Frame, rotation: f32, slide: f32) -> Frame {
    if rotation != 90.0 && rotation != 270.0 {
        return vu;
    }
    // Largeur et hauteur échangées, en vraies proportions.
    let (cx, cy) = vu.center();
    let w = vu.h / slide;
    let h = vu.w * slide;
    Frame::new(cx - w / 2.0, cy - h / 2.0, w, h)
}

/// Cadre qui couvre toute la diapositive (l'image déborde, proportions gardées).
fn cadre_couvrant(image: f32, slide: f32) -> Frame {
    let image = if image.is_finite() && image > 0.0 {
        image
    } else {
        1.0
    };
    if image > slide {
        // Plus large que la diapositive : toute la hauteur, la largeur déborde.
        let w = image / slide;
        Frame::new((1.0 - w) / 2.0, 0.0, w, 1.0)
    } else {
        let h = slide / image;
        Frame::new(0.0, (1.0 - h) / 2.0, 1.0, h)
    }
}

impl Deck {
    /// Un album photo : une diapositive par image lisible (PNG ou JPEG), classées
    /// par date de prise de vue puis par nom. Renvoie la présentation et les noms
    /// des fichiers ignorés (illisibles ou trop lourds). Jamais de panique.
    pub fn photo_album(images: &[AlbumImage], opts: &AlbumOptions) -> (Deck, Vec<String>) {
        let mut deck = Deck::new();
        deck.slides.clear();
        deck.theme = opts.theme;
        deck.title = opts.title.trim().to_string();
        deck.slide_numbers = false;
        let seconds = opts
            .seconds
            .filter(|s| s.is_finite() && *s > 0.0)
            .map(|s| s.clamp(0.5, 3600.0));
        deck.auto_advance = seconds.is_some();
        let ratio = deck.aspect.ratio();
        let palette = opts.theme.palette();

        let mut classees: Vec<(Option<String>, &AlbumImage)> = images
            .iter()
            .take(MAX_ALBUM_IMAGES)
            .map(|im| (exif_date(&im.bytes), im))
            .collect();
        classees.sort_by(|a, b| {
            let cle = |x: &(Option<String>, &AlbumImage)| {
                (x.0.is_none(), x.0.clone(), natural_key(&x.1.name))
            };
            cle(a).cmp(&cle(b))
        });

        if !deck.title.is_empty() {
            deck.slides.push(Slide {
                layout: Layout::Title,
                title: crate::import::echapper_markup(&deck.title),
                subtitle: crate::import::echapper_markup(opts.subtitle.trim()),
                transition: opts.transition,
                duration: seconds,
                ..Slide::default()
            });
        }
        let mut ignorees = Vec::new();
        for (k, (_, im)) in classees.iter().enumerate() {
            let Ok(media) = Media::from_bytes(&im.bytes) else {
                ignorees.push(im.name.clone());
                continue;
            };
            let id = media.id();
            let rotation = rotation_exif(exif_orientation(&im.bytes));
            // Proportions affichées (une photo tournée échange largeur et hauteur).
            let vue = if rotation == 90.0 || rotation == 270.0 {
                1.0 / media.ratio()
            } else {
                media.ratio()
            };
            deck.media.entry(id.clone()).or_insert(media);
            let legende = if opts.captions {
                caption_from_name(&im.name)
            } else {
                String::new()
            };
            let plein = !opts.alternate || k % 2 == 0;
            let mut s = Slide {
                layout: Layout::Blank,
                transition: opts.transition,
                duration: seconds,
                ..Slide::default()
            };
            if plein {
                s.background = Some([0, 0, 0]);
                let vu = cadre_couvrant(vue, ratio);
                s.objects.push(Object {
                    kind: ObjectKind::Image,
                    frame: cadre_tourne(vu, rotation, ratio),
                    media: id,
                    rotation,
                    ..Object::default()
                });
                if !legende.is_empty() {
                    s.objects.push(Object {
                        kind: ObjectKind::Rect,
                        frame: Frame::new(0.0, 0.86, 1.0, 0.14),
                        fill: Some([12, 12, 14]),
                        stroke: None,
                        text: legende,
                        text_color: Some([245, 245, 240]),
                        text_size: 0.045,
                        ..Object::default()
                    });
                }
            } else {
                // Encadrée : l'image dans un passe-partout blanc, légende dessous.
                let haut = if legende.is_empty() { 0.8 } else { 0.7 };
                let mut vu = crate::objets::image_frame(vue, ratio, haut);
                vu.y = if legende.is_empty() {
                    (1.0 - vu.h) / 2.0
                } else {
                    0.07
                };
                let bord = 0.012;
                s.objects.push(Object {
                    kind: ObjectKind::Rect,
                    frame: Frame::new(
                        vu.x - bord,
                        vu.y - bord * ratio,
                        vu.w + 2.0 * bord,
                        vu.h + 2.0 * bord * ratio,
                    ),
                    fill: Some([255, 255, 255]),
                    stroke: None,
                    ..Object::default()
                });
                s.objects.push(Object {
                    kind: ObjectKind::Image,
                    frame: cadre_tourne(vu, rotation, ratio),
                    media: id,
                    rotation,
                    ..Object::default()
                });
                if !legende.is_empty() {
                    s.objects.push(Object {
                        kind: ObjectKind::Rect,
                        frame: Frame::new(0.1, vu.bottom() + 0.04, 0.8, 0.1),
                        fill: None,
                        stroke: None,
                        text: legende,
                        text_color: Some(palette.text),
                        text_size: 0.045,
                        ..Object::default()
                    });
                }
            }
            deck.slides.push(s);
        }
        if deck.slides.is_empty() {
            deck.slides.push(Slide::new(Layout::Title));
        }
        (deck, ignorees)
    }

    /// Ajoute après la diapositive `after` (bornée) une diapositive « Partager » :
    /// `title`, l'adresse en sous-titre et son code QR en grand. Renvoie sa
    /// position ; erreur en français si l'adresse ne tient pas dans un code QR.
    pub fn add_share_slide(
        &mut self,
        after: usize,
        url: &str,
        title: &str,
    ) -> Result<usize, String> {
        let url = url.trim();
        if url.is_empty() {
            return Err("Adresse vide : écrivez l'adresse à partager.".into());
        }
        varium_core::qr::QrCode::encode(url).map_err(|e| e.to_string())?;
        let i = self.add_slide(after, Layout::TitleOnly);
        let ratio = self.aspect.ratio();
        let s = &mut self.slides[i];
        s.title = crate::import::echapper_markup(title.trim());
        let mut qr = Object::qr(url, ratio, 0.5);
        qr.frame.y = 0.3;
        s.objects.push(qr);
        s.objects.push(Object {
            kind: ObjectKind::Rect,
            frame: Frame::new(0.1, 0.83, 0.8, 0.08),
            fill: None,
            stroke: None,
            text: crate::import::echapper_markup(url),
            text_size: 0.035,
            ..Object::default()
        });
        Ok(i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un JPEG minimal avec un segment EXIF (petit-boutiste ou gros-boutiste).
    pub(crate) fn jpeg_exif(date: &str, orientation: u16, le: bool) -> Vec<u8> {
        let mut t: Vec<u8> = Vec::new();
        let u16b = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let u32b = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        t.extend(if le { b"II" } else { b"MM" });
        t.extend(u16b(42));
        t.extend(u32b(8));
        // IFD0 : orientation, pointeur EXIF.
        t.extend(u16b(2));
        t.extend(u16b(0x0112));
        t.extend(u16b(3));
        t.extend(u32b(1));
        t.extend(u16b(orientation));
        t.extend([0, 0]);
        t.extend(u16b(0x8769));
        t.extend(u16b(4));
        t.extend(u32b(1));
        let exif_ifd = 8 + 2 + 24 + 4;
        t.extend(u32b(exif_ifd));
        t.extend(u32b(0));
        // IFD EXIF : DateTimeOriginal.
        t.extend(u16b(1));
        t.extend(u16b(0x9003));
        t.extend(u16b(2));
        t.extend(u32b(20));
        t.extend(u32b(exif_ifd + 2 + 12 + 4));
        t.extend(u32b(0));
        let mut d = date.as_bytes().to_vec();
        d.resize(20, 0);
        t.extend(d);
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend(t);
        let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
        out.extend(((app1.len() + 2) as u16).to_be_bytes());
        out.extend(app1);
        out.extend([0xFF, 0xD9]);
        out
    }

    #[test]
    fn date_et_orientation_exif() {
        for le in [true, false] {
            let j = jpeg_exif("2024:07:14 10:30:00", 6, le);
            assert_eq!(exif_date(&j).as_deref(), Some("2024:07:14 10:30:00"));
            assert_eq!(exif_orientation(&j), Some(6));
        }
        assert_eq!(exif_date(&jpeg_exif("0000:00:00 00:00:00", 1, true)), None);
        assert_eq!(exif_date(b"\xFF\xD8"), None);
        assert_eq!(exif_date(b""), None);
    }

    #[test]
    fn exif_abime_sans_panique() {
        let modele = jpeg_exif("2024:07:14 10:30:00", 8, true);
        let mut graine = 0x1234_5678_u64;
        for _ in 0..3000 {
            let mut o = modele.clone();
            for _ in 0..3 {
                graine ^= graine << 13;
                graine ^= graine >> 7;
                graine ^= graine << 17;
                let i = (graine as usize) % o.len().max(1);
                match graine % 3 {
                    0 => o.truncate(i),
                    1 => {
                        if let Some(b) = o.get_mut(i) {
                            *b = (graine >> 20) as u8;
                        }
                    }
                    _ => o.insert(i, 0xFF),
                }
            }
            let _ = (exif_date(&o), exif_orientation(&o));
        }
    }

    #[test]
    fn tri_naturel_et_legendes() {
        let mut v = ["photo10.jpg", "photo2.jpg", "Photo1.jpg"];
        v.sort_by_key(|n| natural_key(n));
        assert_eq!(v, ["Photo1.jpg", "photo2.jpg", "photo10.jpg"]);
        assert_eq!(
            caption_from_name("plage_de_nice-2024.jpg"),
            "Plage de nice 2024"
        );
        assert_eq!(caption_from_name("IMG_1234.JPG"), "");
        assert_eq!(caption_from_name("DSC01234.jpg"), "");
        assert_eq!(caption_from_name("20240714_103000.jpg"), "");
        assert_eq!(caption_from_name("a*b.png"), "A\\*b");
    }

    #[test]
    fn cadres_couvrants() {
        let f = cadre_couvrant(4.0 / 3.0, 16.0 / 9.0);
        assert!((f.w - 1.0).abs() < 1e-5 && f.h > 1.0 && f.y < 0.0);
        let f = cadre_couvrant(3.0, 16.0 / 9.0);
        assert!((f.h - 1.0).abs() < 1e-5 && f.w > 1.0);
        let t = cadre_tourne(Frame::new(0.2, 0.1, 0.3, 0.8), 90.0, 16.0 / 9.0);
        assert!((t.center().0 - 0.35).abs() < 1e-5);
        assert!((t.w * 16.0 / 9.0 - 0.8).abs() < 1e-4);
    }
}
