//! Thèmes : palette de couleurs et choix des polices de titre.

use serde::Serialize;

use crate::texte::{contraste, melange};

/// Thème d'une présentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Theme {
    /// Fond blanc, titres presque noirs, accent bleu.
    #[default]
    Clair,
    /// Fond anthracite, texte clair, accent indigo.
    Sombre,
    /// Bleu nuit, accent turquoise.
    Ocean,
    /// Ivoire, accent bronze, titres à empattements.
    Elegant,
    /// Tableau noir vert, accent jaune craie, titres à empattements.
    Craie,
    /// Crème rosée, accent corail.
    Corail,
}

identifiants!(Theme {
    Clair => "clair",
    Sombre => "sombre",
    Ocean => "ocean",
    Elegant => "elegant",
    Craie => "craie",
    Corail => "corail",
});

/// Couleurs d'une diapositive (RVB).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Palette {
    /// Fond de la diapositive.
    pub background: [u8; 3],
    /// Texte courant (contenu, colonnes).
    pub text: [u8; 3],
    /// Titres.
    pub title: [u8; 3],
    /// Puces, numéros, filets décoratifs, en-têtes de tableau.
    pub accent: [u8; 3],
    /// Sous-titres, pied de page, numéros de diapositive, traits de tableau.
    pub muted: [u8; 3],
    /// Titres et sous-titres en police à empattements (sinon sans empattements).
    pub serif_titles: bool,
}

impl Theme {
    pub const ALL: [Theme; 6] = [
        Theme::Clair,
        Theme::Sombre,
        Theme::Ocean,
        Theme::Elegant,
        Theme::Craie,
        Theme::Corail,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Theme::Clair => "Clair",
            Theme::Sombre => "Sombre",
            Theme::Ocean => "Océan",
            Theme::Elegant => "Élégant",
            Theme::Craie => "Craie",
            Theme::Corail => "Corail",
        }
    }

    pub fn palette(self) -> Palette {
        let p = |background, text, title, accent, muted, serif_titles| Palette {
            background,
            text,
            title,
            accent,
            muted,
            serif_titles,
        };
        match self {
            Theme::Clair => p(
                [255, 255, 255],
                [55, 60, 70],
                [17, 24, 39],
                [37, 99, 235],
                [107, 114, 128],
                false,
            ),
            Theme::Sombre => p(
                [22, 24, 29],
                [214, 219, 228],
                [255, 255, 255],
                [129, 140, 248],
                [139, 146, 160],
                false,
            ),
            Theme::Ocean => p(
                [11, 37, 64],
                [205, 226, 240],
                [255, 255, 255],
                [45, 212, 191],
                [125, 160, 190],
                false,
            ),
            Theme::Elegant => p(
                [250, 246, 240],
                [68, 60, 54],
                [38, 32, 28],
                [166, 124, 62],
                [140, 128, 116],
                true,
            ),
            Theme::Craie => p(
                [37, 56, 48],
                [232, 236, 226],
                [250, 250, 244],
                [242, 201, 107],
                [160, 180, 168],
                true,
            ),
            Theme::Corail => p(
                [255, 247, 243],
                [74, 52, 48],
                [120, 42, 30],
                [234, 98, 76],
                [158, 124, 116],
                false,
            ),
        }
    }
}

impl Palette {
    /// La palette posée sur un autre fond : les couleurs trop peu contrastées
    /// sont remplacées par du clair (fond sombre) ou du foncé (fond clair).
    pub fn on_background(self, bg: [u8; 3]) -> Palette {
        // Couleurs douces d'abord ; noir ou blanc purs pour les fonds intermédiaires.
        let (blanc, noir) = ([255, 255, 255], [0, 0, 0]);
        let lisible = [[244, 244, 240], [28, 28, 32], blanc, noir]
            .into_iter()
            .find(|c| contraste(*c, bg) >= 4.5)
            .unwrap_or(if contraste(blanc, bg) >= contraste(noir, bg) {
                blanc
            } else {
                noir
            });
        let garde = |c: [u8; 3], seuil: f32, repli: [u8; 3]| {
            if contraste(c, bg) >= seuil { c } else { repli }
        };
        let muted_repli = melange(lisible, bg, 0.35);
        let title = garde(self.title, 3.0, lisible);
        Palette {
            background: bg,
            text: garde(self.text, 4.5, lisible),
            title,
            accent: garde(self.accent, 2.2, title),
            muted: garde(self.muted, 2.5, muted_repli),
            serif_titles: self.serif_titles,
        }
    }

    /// Couleur des traits fins (tableaux, filets du contenu).
    pub fn rule(&self) -> [u8; 3] {
        melange(self.muted, self.background, 0.55)
    }

    /// Fond léger des en-têtes de tableau.
    pub fn tint(&self) -> [u8; 3] {
        melange(self.accent, self.background, 0.86)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palettes_lisibles() {
        for t in Theme::ALL {
            let p = t.palette();
            assert!(
                contraste(p.text, p.background) >= 4.5,
                "texte du thème {}",
                t.label()
            );
            assert!(contraste(p.title, p.background) >= 4.5);
            assert!(contraste(p.accent, p.background) >= 2.2);
            assert!(contraste(p.muted, p.background) >= 2.5);
        }
    }

    #[test]
    fn fond_propre_toujours_lisible() {
        for t in Theme::ALL {
            for bg in [[0, 0, 0], [255, 255, 255], [128, 128, 128], [200, 30, 30]] {
                let p = t.palette().on_background(bg);
                assert_eq!(p.background, bg);
                assert!(contraste(p.text, bg) >= 4.5, "{} sur {bg:?}", t.label());
            }
        }
    }
}
