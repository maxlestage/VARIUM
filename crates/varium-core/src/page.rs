//! Mise en page : format du papier, marges, police, interligne, en-tête et pied de page.

/// Format du papier.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Paper {
    #[default]
    A4,
    A5,
    Letter,
    Legal,
}

impl Paper {
    pub const ALL: [Paper; 4] = [Paper::A4, Paper::A5, Paper::Letter, Paper::Legal];

    pub fn label(self) -> &'static str {
        match self {
            Paper::A4 => "A4 (21 × 29,7 cm)",
            Paper::A5 => "A5 (14,8 × 21 cm)",
            Paper::Letter => "Lettre US",
            Paper::Legal => "Légal US",
        }
    }

    /// Largeur et hauteur en millimètres, en portrait.
    pub fn size_mm(self) -> (f32, f32) {
        match self {
            Paper::A4 => (210.0, 297.0),
            Paper::A5 => (148.0, 210.0),
            Paper::Letter => (215.9, 279.4),
            Paper::Legal => (215.9, 355.6),
        }
    }

    /// Nom CSS (`@page { size: … }`).
    pub fn css(self) -> &'static str {
        match self {
            Paper::A4 => "A4",
            Paper::A5 => "A5",
            Paper::Letter => "letter",
            Paper::Legal => "legal",
        }
    }
}

/// Police du document.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum DocFont {
    /// Liberation Serif (métrique de Times New Roman), la police de LibreOffice.
    #[default]
    Serif,
    /// Liberation Sans (métrique d'Arial).
    Sans,
}

impl DocFont {
    pub fn label(self) -> &'static str {
        match self {
            DocFont::Serif => "Liberation Serif (Times)",
            DocFont::Sans => "Liberation Sans (Arial)",
        }
    }

    /// Nom de la police dans les exports.
    pub fn family(self) -> &'static str {
        match self {
            DocFont::Serif => "Liberation Serif",
            DocFont::Sans => "Liberation Sans",
        }
    }

    /// Pile CSS avec des polices de repli équivalentes.
    pub fn css(self) -> &'static str {
        match self {
            DocFont::Serif => "'Liberation Serif', 'Times New Roman', Tinos, Times, Georgia, serif",
            DocFont::Sans => "'Liberation Sans', Arial, Arimo, Helvetica, sans-serif",
        }
    }
}

/// La mise en page d'un document.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct PageSetup {
    pub paper: Paper,
    pub landscape: bool,
    /// Marges en millimètres.
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub margin_right: f32,
    pub font: DocFont,
    /// Taille du texte en points.
    pub font_size: f32,
    /// Interligne (1 = simple, 1,15 = LibreOffice, 1,5, 2 = double).
    pub line_spacing: f32,
    /// Texte justifié par défaut.
    pub justify: bool,
    /// En-tête (peut contenir des {{variables}} et de la mise en forme).
    pub header: String,
    /// Pied de page.
    pub footer: String,
    /// « Page 1 sur 3 » en bas de chaque page.
    pub page_numbers: bool,
    /// Typographie française automatique (« », ’, …, espaces fines).
    pub typographie: bool,
}

impl Default for PageSetup {
    fn default() -> Self {
        Self {
            paper: Paper::A4,
            landscape: false,
            margin_top: 20.0,
            margin_bottom: 20.0,
            margin_left: 20.0,
            margin_right: 20.0,
            font: DocFont::Serif,
            font_size: 12.0,
            line_spacing: 1.15,
            justify: false,
            header: String::new(),
            footer: String::new(),
            page_numbers: false,
            typographie: true,
        }
    }
}

impl PageSetup {
    /// Largeur et hauteur de la page en millimètres, orientation comprise.
    pub fn size_mm(&self) -> (f32, f32) {
        let (w, h) = self.paper.size_mm();
        if self.landscape { (h, w) } else { (w, h) }
    }

    /// Largeur utile (entre les marges) en millimètres.
    pub fn text_width_mm(&self) -> f32 {
        (self.size_mm().0 - self.margin_left - self.margin_right).max(20.0)
    }

    /// Hauteur utile en millimètres.
    pub fn text_height_mm(&self) -> f32 {
        (self.size_mm().1 - self.margin_top - self.margin_bottom).max(20.0)
    }
}

/// Points typographiques → millimètres.
pub fn pt_to_mm(pt: f32) -> f32 {
    pt * 25.4 / 72.0
}
