//! Géométrie des dispositions : position des zones de texte et tailles de police,
//! en fractions de la diapositive, pour que l'éditeur et les exports s'accordent.
//!
//! Les positions sont des fractions de la largeur (`x`, `w`) et de la hauteur
//! (`y`, `h`) ; les tailles de police, des fractions de la hauteur. Une même
//! présentation se dessine donc à l'identique quelle que soit la résolution.

use crate::Layout;

/// Rectangle en fractions de la diapositive (0 à 1).
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Frame {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Frame { x, y, w, h }
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    /// Les deux rectangles se chevauchent (des bords communs ne comptent pas).
    pub fn overlaps(&self, o: &Frame) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }

    /// Le rectangle dans une diapositive de `width` × `height` unités :
    /// `(x, y, largeur, hauteur)`.
    /// Le centre du rectangle.
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// Le rectangle rétréci de `dx` et `dy` de chaque côté (jamais négatif).
    pub fn inset(&self, dx: f32, dy: f32) -> Frame {
        let dx = dx.min(self.w / 2.0).max(0.0);
        let dy = dy.min(self.h / 2.0).max(0.0);
        Frame::new(
            self.x + dx,
            self.y + dy,
            self.w - 2.0 * dx,
            self.h - 2.0 * dy,
        )
    }

    pub fn scaled(&self, width: f32, height: f32) -> (f32, f32, f32, f32) {
        (
            self.x * width,
            self.y * height,
            self.w * width,
            self.h * height,
        )
    }
}

/// Ancrage vertical du texte dans sa zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Anchor {
    #[default]
    Top,
    Middle,
    Bottom,
}

/// Zones de texte d'une disposition. Une zone absente n'est pas affichée (son
/// texte est conservé dans la diapositive).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlideLayoutBoxes {
    pub title: Option<Frame>,
    pub subtitle: Option<Frame>,
    /// Contenu ; pour la disposition Citation, la citation elle-même (en italique,
    /// police à empattements, couleur des titres).
    pub body: Option<Frame>,
    /// Seconde colonne.
    pub body2: Option<Frame>,
    /// Filet décoratif plein, à la couleur d'accent.
    pub accent: Option<Frame>,
    pub title_anchor: Anchor,
    pub subtitle_anchor: Anchor,
    pub body_anchor: Anchor,
    /// Titre et sous-titre centrés horizontalement (sinon alignés à gauche).
    pub centered: bool,
    /// Pied de page et numéro affichés sur cette disposition.
    pub footer: bool,
}

/// Interligne des titres et sous-titres (multiple de la taille de police).
pub const TITLE_LINE_HEIGHT: f32 = 1.12;
/// Interligne du contenu.
pub const BODY_LINE_HEIGHT: f32 = 1.3;
/// Taille du pied de page et du numéro, en fraction de la hauteur.
pub const FOOTER_SIZE: f32 = 0.022;

/// Marge latérale commune.
const MX: f32 = 0.07;
/// Zone de titre des dispositions « titre en haut ».
const TITRE_HAUT: Frame = Frame::new(MX, 0.06, 1.0 - 2.0 * MX, 0.16);
/// Filet sous le titre du haut.
const FILET_HAUT: Frame = Frame::new(MX, 0.238, 0.06, 0.007);

/// Pied de page : en bas à gauche.
pub fn footer_frame() -> Frame {
    Frame::new(MX, 0.915, 0.66, 0.05)
}

/// Numéro de diapositive : en bas à droite (texte aligné à droite).
pub fn number_frame() -> Frame {
    Frame::new(0.78, 0.915, 1.0 - MX - 0.78, 0.05)
}

/// Les zones de texte d'une disposition.
pub fn boxes(layout: Layout) -> SlideLayoutBoxes {
    let vide = SlideLayoutBoxes {
        title: None,
        subtitle: None,
        body: None,
        body2: None,
        accent: None,
        title_anchor: Anchor::Bottom,
        subtitle_anchor: Anchor::Top,
        body_anchor: Anchor::Top,
        centered: false,
        footer: true,
    };
    match layout {
        Layout::Title => SlideLayoutBoxes {
            title: Some(Frame::new(0.08, 0.14, 0.84, 0.38)),
            accent: Some(Frame::new(0.46, 0.555, 0.08, 0.008)),
            subtitle: Some(Frame::new(0.1, 0.6, 0.8, 0.22)),
            centered: true,
            footer: false,
            ..vide
        },
        Layout::TitleContent => SlideLayoutBoxes {
            title: Some(TITRE_HAUT),
            accent: Some(FILET_HAUT),
            body: Some(Frame::new(MX, 0.285, 1.0 - 2.0 * MX, 0.6)),
            ..vide
        },
        Layout::TwoColumns => SlideLayoutBoxes {
            title: Some(TITRE_HAUT),
            accent: Some(FILET_HAUT),
            body: Some(Frame::new(MX, 0.285, 0.41, 0.6)),
            body2: Some(Frame::new(0.52, 0.285, 0.41, 0.6)),
            ..vide
        },
        Layout::Section => SlideLayoutBoxes {
            title: Some(Frame::new(0.1, 0.14, 0.8, 0.38)),
            accent: Some(Frame::new(0.1, 0.555, 0.1, 0.01)),
            subtitle: Some(Frame::new(0.1, 0.6, 0.8, 0.2)),
            ..vide
        },
        Layout::Quote => SlideLayoutBoxes {
            body: Some(Frame::new(0.12, 0.1, 0.76, 0.56)),
            accent: Some(Frame::new(0.12, 0.7, 0.06, 0.008)),
            subtitle: Some(Frame::new(0.12, 0.735, 0.76, 0.14)),
            body_anchor: Anchor::Bottom,
            ..vide
        },
        Layout::TitleOnly => SlideLayoutBoxes {
            title: Some(TITRE_HAUT),
            accent: Some(FILET_HAUT),
            ..vide
        },
        Layout::Blank => SlideLayoutBoxes {
            body: Some(Frame::new(MX, 0.07, 1.0 - 2.0 * MX, 0.815)),
            body_anchor: Anchor::Middle,
            ..vide
        },
    }
}

/// Taille des titres, en fraction de la hauteur de la diapositive.
pub fn title_size(layout: Layout) -> f32 {
    match layout {
        Layout::Title => 0.1,
        Layout::Section => 0.085,
        _ => 0.062,
    }
}

/// Taille des sous-titres (et de l'auteur d'une citation), en fraction de la hauteur.
pub fn subtitle_size(layout: Layout) -> f32 {
    match layout {
        Layout::Title => 0.042,
        Layout::Section => 0.04,
        Layout::Quote => 0.034,
        _ => 0.04,
    }
}

/// Taille du contenu, en fraction de la hauteur. Les titres `#` du contenu sont
/// 1,3 / 1,15 / 1,05 fois plus grands, les tableaux 0,85 fois plus petits.
pub fn body_size(layout: Layout) -> f32 {
    match layout {
        Layout::TwoColumns => 0.04,
        Layout::Quote => 0.06,
        // Une diapositive vide sert aux messages courts : texte plus grand.
        Layout::Blank => 0.055,
        _ => 0.045,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn citation_et_vide_sans_titre() {
        assert!(boxes(Layout::Quote).title.is_none());
        assert!(boxes(Layout::Blank).title.is_none());
        assert!(boxes(Layout::TwoColumns).body2.is_some());
        assert!(boxes(Layout::Title).centered);
    }
}
