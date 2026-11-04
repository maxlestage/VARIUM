//! # varium-slides
//!
//! Moteur de présentations de **Varium** : diapositives, dispositions, thèmes,
//! champs `{{…}}` partagés par toute la présentation, et trois exports :
//!
//! - **HTML** : un seul fichier autonome, diaporama plein écran (clavier, clic,
//!   transitions, notes de l'orateur, impression une diapositive par page) ;
//! - **ODP** : OpenDocument Présentation, à ouvrir dans LibreOffice Impress ;
//! - **PDF** : une page par diapositive, écrit à la main avec les polices standard.
//!
//! Le texte des diapositives s'écrit dans la syntaxe des documents Varium
//! (`**gras**`, `- puces`, `1. listes`, `> citation`, tableaux `| a | b |`…),
//! mise en forme par [`varium_core::document`].
//!
//! ```
//! use varium_slides::{Deck, Layout};
//!
//! let mut deck = Deck::new();
//! deck.values.insert("entreprise".into(), "Atelier Varium".into());
//! let i = deck.add_slide(0, Layout::TitleContent);
//! deck.slides[i].title = "Bienvenue chez {{entreprise}}".into();
//! deck.slides[i].body = "- Un **seul** format\n- Trois exports".into();
//! let html = deck.to_html();
//! assert!(html.contains("Atelier Varium") && !html.contains("{{entreprise}}"));
//! assert_eq!(deck.variables(), ["entreprise"]);
//! ```

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Déclare l'identifiant stable (dans le fichier `.json`) de chaque variante d'une
/// énumération, et sa (dé)sérialisation tolérante : une valeur inconnue (fichier
/// d'une version future, faute de frappe) donne la variante par défaut.
macro_rules! identifiants {
    ($t:ident { $($v:ident => $s:literal),* $(,)? }) => {
        impl $t {
            /// Identifiant stable, tel qu'il est écrit dans le fichier.
            pub fn id(self) -> &'static str {
                match self {
                    $($t::$v => $s),*
                }
            }

            /// Variante d'un identifiant (casse, `_`, `-` et espaces ignorés).
            pub fn from_id(s: &str) -> Option<Self> {
                let cle = crate::fichier::cle(s);
                [$($t::$v),*]
                    .into_iter()
                    .find(|v| crate::fichier::cle(v.id()) == cle)
            }
        }

        impl serde::Serialize for $t {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.id())
            }
        }

        impl<'de> serde::Deserialize<'de> for $t {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let v = serde_json::Value::deserialize(d)?;
                Ok(v.as_str().and_then(Self::from_id).unwrap_or_default())
            }
        }
    };
}

mod depuis_document;
mod depuis_texte;
mod embellir;
mod fichier;
mod geometrie;
mod html;
mod import;
mod metriques;
mod minutage;
mod objets;
mod odp;
mod pdf;
mod plan;
mod pptx;
mod texte;
mod theme;
mod xml;

pub use depuis_document::deck_from_document;
pub use depuis_texte::{Closing, deck_from_text};
pub use embellir::{Design, MAX_SUGGESTIONS, Suggestion, design_suggestions, suggestions};
pub use fichier::{FORMAT, VERSION};
pub use geometrie::{
    Anchor, BODY_LINE_HEIGHT, FOOTER_SIZE, Frame, SlideLayoutBoxes, TITLE_LINE_HEIGHT, body_size,
    boxes, footer_frame, number_frame, subtitle_size, title_size,
};
pub use minutage::format_duration;
pub use objets::{
    MAX_IMAGE_BYTES, MAX_IMAGE_SIDE, Media, Object, ObjectKind, arrow_head, base64, image_frame,
    unbase64,
};
pub use pdf::{HandoutNotes, HandoutOptions, HandoutPaper, PNG_HEIGHT, RasterFonts};
pub use plan::{COLUMN_BREAK, OutlineEntry, outline_text, parse_outline};
pub use texte::{MISSING_COLOR, hex_color, parse_color, readable_on, text_lines};
pub use theme::{Palette, Theme};

// ---------------------------------------------------------------------------------
// Dispositions, transitions, format
// ---------------------------------------------------------------------------------

/// Disposition d'une diapositive : quelles zones de texte elle montre, et où.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Layout {
    /// Diapositive de titre : grand titre centré et sous-titre.
    Title,
    /// Titre en haut, contenu (puces, texte, tableau) dessous.
    #[default]
    TitleContent,
    /// Titre et deux colonnes de contenu (`body` et `body2`).
    TwoColumns,
    /// Intercalaire de section : grand titre et sous-titre, alignés à gauche.
    Section,
    /// Citation : le corps est la citation, le sous-titre son auteur.
    Quote,
    /// Titre seul, en haut.
    TitleOnly,
    /// Vide : pas de titre ; le corps, s'il est rempli, occupe toute la diapositive.
    Blank,
}

identifiants!(Layout {
    Title => "title",
    TitleContent => "title_content",
    TwoColumns => "two_columns",
    Section => "section",
    Quote => "quote",
    TitleOnly => "title_only",
    Blank => "blank",
});

impl Layout {
    pub const ALL: [Layout; 7] = [
        Layout::Title,
        Layout::TitleContent,
        Layout::TwoColumns,
        Layout::Section,
        Layout::Quote,
        Layout::TitleOnly,
        Layout::Blank,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Layout::Title => "Titre",
            Layout::TitleContent => "Titre et contenu",
            Layout::TwoColumns => "Deux colonnes",
            Layout::Section => "Section",
            Layout::Quote => "Citation",
            Layout::TitleOnly => "Titre seul",
            Layout::Blank => "Vide",
        }
    }
}

/// Effet d'apparition d'une diapositive pendant le diaporama.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Transition {
    #[default]
    Aucune,
    /// Fondu enchaîné.
    Fondu,
    /// La diapositive pousse la précédente vers la gauche.
    Glissement,
    /// La diapositive grandit en apparaissant.
    Zoom,
}

identifiants!(Transition {
    Aucune => "aucune",
    Fondu => "fondu",
    Glissement => "glissement",
    Zoom => "zoom",
});

impl Transition {
    pub const ALL: [Transition; 4] = [
        Transition::Aucune,
        Transition::Fondu,
        Transition::Glissement,
        Transition::Zoom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Transition::Aucune => "Aucune",
            Transition::Fondu => "Fondu",
            Transition::Glissement => "Glissement",
            Transition::Zoom => "Zoom",
        }
    }
}

/// Format des diapositives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Aspect {
    /// 16:9, le format des écrans actuels.
    #[default]
    Large,
    /// 4:3, le format des anciens vidéoprojecteurs.
    Standard,
}

identifiants!(Aspect {
    Large => "large",
    Standard => "standard",
});

impl Aspect {
    pub const ALL: [Aspect; 2] = [Aspect::Large, Aspect::Standard];

    pub fn label(self) -> &'static str {
        match self {
            Aspect::Large => "Large (16:9)",
            Aspect::Standard => "Standard (4:3)",
        }
    }

    /// Largeur divisée par hauteur.
    pub fn ratio(self) -> f32 {
        match self {
            Aspect::Large => 16.0 / 9.0,
            Aspect::Standard => 4.0 / 3.0,
        }
    }

    /// Dimensions d'une diapositive en millimètres (largeur, hauteur), comme dans
    /// Impress : 254 × 142,875 mm en 16:9, 254 × 190,5 mm en 4:3.
    pub fn size_mm(self) -> (f32, f32) {
        (254.0, 254.0 / self.ratio())
    }
}

// ---------------------------------------------------------------------------------
// Diapositive
// ---------------------------------------------------------------------------------

/// Une diapositive. Tous ses textes acceptent les champs `{{…}}` ; `body`, `body2`
/// et `notes` acceptent en plus toute la mise en forme Varium (titres, puces,
/// tableaux…), `title` et `subtitle` la mise en forme des caractères.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Slide {
    pub layout: Layout,
    #[serde(deserialize_with = "fichier::texte")]
    pub title: String,
    #[serde(deserialize_with = "fichier::texte")]
    pub subtitle: String,
    /// Contenu (syntaxe Varium) ; la citation pour la disposition Citation.
    #[serde(deserialize_with = "fichier::texte")]
    pub body: String,
    /// Seconde colonne de la disposition Deux colonnes.
    #[serde(deserialize_with = "fichier::texte")]
    pub body2: String,
    /// Notes de l'orateur, jamais projetées.
    #[serde(deserialize_with = "fichier::texte")]
    pub notes: String,
    pub transition: Transition,
    /// Couleur de fond propre à la diapositive (sinon celle du thème).
    #[serde(
        serialize_with = "fichier::couleur_ser",
        deserialize_with = "fichier::couleur",
        skip_serializing_if = "Option::is_none"
    )]
    pub background: Option<[u8; 3]>,
    /// Images et formes posées sur la diapositive, de l'arrière-plan au premier plan.
    #[serde(
        deserialize_with = "fichier::objets",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub objects: Vec<Object>,
    /// Minutage répété (« Répéter le minutage »), en secondes : durée d'affichage
    /// de la diapositive, pour l'avance automatique.
    #[serde(
        deserialize_with = "fichier::duree",
        skip_serializing_if = "Option::is_none"
    )]
    pub duration: Option<f32>,
}

impl Slide {
    /// Une diapositive vide de la disposition donnée.
    pub fn new(layout: Layout) -> Self {
        Slide {
            layout,
            ..Slide::default()
        }
    }

    /// Les couleurs de la diapositive dans un thème : celles du thème, ou, avec un
    /// fond propre, des couleurs de texte ajustées pour rester lisibles dessus.
    /// C'est la palette à utiliser pour dessiner la diapositive.
    pub fn palette(&self, theme: Theme) -> Palette {
        let p = theme.palette();
        match self.background {
            Some(bg) => p.on_background(bg),
            None => p,
        }
    }

    /// Libellé court pour la liste des diapositives : le titre, sinon le sous-titre
    /// ou la première ligne du contenu, sans mise en forme.
    pub fn label(&self) -> String {
        let vide = varium_core::Values::default();
        for src in [&self.title, &self.subtitle, &self.body, &self.body2] {
            for ligne in text_lines(src, &vide) {
                let t = varium_core::doc::plain(&ligne);
                let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
                if !t.is_empty() {
                    return texte::tronquer(&t, 60);
                }
            }
        }
        String::new()
    }

    /// Les textes de la diapositive, dans l'ordre de lecture.
    fn texts(&self) -> [&str; 5] {
        [
            &self.title,
            &self.subtitle,
            &self.body,
            &self.body2,
            &self.notes,
        ]
    }
}

// ---------------------------------------------------------------------------------
// Présentation
// ---------------------------------------------------------------------------------

/// Une présentation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Deck {
    #[serde(deserialize_with = "fichier::texte")]
    pub title: String,
    pub theme: Theme,
    pub aspect: Aspect,
    /// Pied de page de chaque diapositive (champs et mise en forme des caractères).
    #[serde(deserialize_with = "fichier::texte")]
    pub footer: String,
    /// Numéro affiché en bas à droite de chaque diapositive.
    #[serde(deserialize_with = "fichier::vrai_par_defaut")]
    pub slide_numbers: bool,
    /// Valeurs des champs `{{…}}`, communes à toutes les diapositives.
    #[serde(deserialize_with = "fichier::valeurs")]
    pub values: BTreeMap<String, String>,
    #[serde(deserialize_with = "fichier::diapositives")]
    pub slides: Vec<Slide>,
    /// Images de la présentation, chacune stockée une seule fois, par identifiant
    /// (voir [`Deck::add_media`]) ; les objets image des diapositives y renvoient.
    #[serde(
        deserialize_with = "fichier::medias",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub media: BTreeMap<String, Media>,
    /// Le diaporama avance seul selon le minutage des diapositives
    /// ([`Slide::duration`]) : en présentation, dans l'export HTML et l'ODP.
    #[serde(
        deserialize_with = "fichier::booleen",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub auto_advance: bool,
}

impl Default for Deck {
    fn default() -> Self {
        Deck::new()
    }
}

impl Deck {
    /// Une présentation neuve : une diapositive de titre, thème Clair, format 16:9.
    pub fn new() -> Self {
        Deck {
            title: String::new(),
            theme: Theme::default(),
            aspect: Aspect::default(),
            footer: String::new(),
            slide_numbers: true,
            values: BTreeMap::new(),
            slides: vec![Slide::new(Layout::Title)],
            media: BTreeMap::new(),
            auto_advance: false,
        }
    }

    /// Une présentation d'exemple, en français, qui présente Varium et emploie
    /// toutes les dispositions, des champs et un tableau.
    pub fn sample() -> Self {
        let valeurs = [
            ("entreprise", "Atelier Varium"),
            ("orateur", "Camille Martin"),
            ("date", "novembre 2026"),
            ("ville", "Lyon"),
            ("site", "varium.app"),
        ];
        let diapo = |layout, title: &str, subtitle: &str, body: &str, notes: &str| Slide {
            layout,
            title: title.into(),
            subtitle: subtitle.into(),
            body: body.into(),
            notes: notes.into(),
            transition: Transition::Fondu,
            ..Slide::default()
        };
        let mut slides = vec![
            diapo(
                Layout::Title,
                "Varium",
                "La suite bureautique qui remplit vos documents pour vous\nPrésentée par {{orateur}} — {{entreprise}}",
                "",
                "Accueillir le public, se présenter, annoncer la durée : **vingt minutes**, questions comprises.",
            ),
            diapo(
                Layout::TitleContent,
                "Pourquoi Varium ?",
                "",
                "- Un document, **toutes ses possibilités** : chaque champ {{…}} se remplit une seule fois\n- Les mises en forme de LibreOffice, sans la complexité\n  - *gras*, *italique*, __souligné__, listes et tableaux\n  - typographie française automatique\n- Des fichiers ouverts : ODT, ODS, ODP, PDF, HTML\n- Libre, rapide et entièrement hors ligne",
                "Insister sur le gain de temps : un contrat type rempli en **trente secondes**.",
            ),
            diapo(
                Layout::Section,
                "La suite",
                "Textes, classeurs et présentations",
                "",
                "Transition vers la démonstration des trois logiciels.",
            ),
            Slide {
                body2: "### Classeur\n- Formules et fonctions françaises\n- Export XLSX, ODS et CSV\n- Les cellules alimentent vos champs".into(),
                ..diapo(
                    Layout::TwoColumns,
                    "Documents et classeurs",
                    "",
                    "### Texte\n- Champs \\{{client}} et \\{{montant | money}}\n- Texte conditionnel\n- Publipostage en un clic",
                    "Montrer un devis rempli depuis un classeur.",
                )
            },
            diapo(
                Layout::TitleContent,
                "Un format, tous les exports",
                "",
                "| Logiciel | Pour | Exports |\n|---|---|---|\n| Texte | courriers, contrats, devis | ODT, DOCX, PDF, EPUB, RTF |\n| Classeur | budgets, listes, calculs | ODS, XLSX, CSV |\n| Présentation | diaporamas, supports | ODP, PDF, HTML |\n\nChaque fichier s’ouvre dans **LibreOffice** comme dans Microsoft Office.",
                "Rappeler que le PDF est produit sans aucune dépendance.",
            ),
            diapo(
                Layout::Quote,
                "",
                "La devise de Varium",
                "Un document. Toutes ses possibilités.",
                "Laisser un temps de silence après la citation.",
            ),
            diapo(
                Layout::TitleOnly,
                "Démonstration en direct",
                "",
                "",
                "Ouvrir l'exemple « Devis » puis exporter cette présentation en HTML.",
            ),
            diapo(
                Layout::Blank,
                "",
                "",
                "-> **Merci !** <-\n\n-> Questions et échanges — {{ville}}, {{date}} <-\n-> *{{site}}* <-",
                "Remercier l'équipe de {{entreprise}}.",
            ),
        ];
        slides[2].transition = Transition::Zoom;
        slides[3].transition = Transition::Glissement;
        slides[4].transition = Transition::Glissement;
        Deck {
            title: "Varium — la suite bureautique".into(),
            theme: Theme::Ocean,
            aspect: Aspect::Large,
            footer: "{{entreprise}} · {{date}}".into(),
            slide_numbers: true,
            values: valeurs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            slides,
            media: BTreeMap::new(),
            auto_advance: false,
        }
    }

    /// Insère une diapositive vide après la diapositive `after` (bornée à la
    /// dernière) et renvoie sa position. Elle reprend la transition de sa voisine.
    pub fn add_slide(&mut self, after: usize, layout: Layout) -> usize {
        let at = match self.slides.len() {
            0 => 0,
            n => after.min(n - 1) + 1,
        };
        let transition = at
            .checked_sub(1)
            .and_then(|i| self.slides.get(i))
            .map(|s| s.transition)
            .unwrap_or_default();
        self.slides.insert(
            at,
            Slide {
                transition,
                ..Slide::new(layout)
            },
        );
        at
    }

    /// Copie la diapositive `i` (bornée à la dernière) juste après elle ; renvoie la
    /// position de la copie.
    pub fn duplicate(&mut self, i: usize) -> usize {
        let Some(last) = self.slides.len().checked_sub(1) else {
            self.slides.push(Slide::default());
            return 0;
        };
        let i = i.min(last);
        let copie = self.slides[i].clone();
        self.slides.insert(i + 1, copie);
        i + 1
    }

    /// Supprime la diapositive `i` (bornée à la dernière). Une présentation garde
    /// toujours au moins une diapositive : supprimer la seule la remplace par une
    /// diapositive vide de même disposition.
    pub fn remove(&mut self, i: usize) {
        match self.slides.len() {
            0 => self.slides.push(Slide::default()),
            1 => {
                let layout = self.slides[0].layout;
                self.slides[0] = Slide::new(layout);
            }
            n => {
                self.slides.remove(i.min(n - 1));
            }
        }
    }

    /// Déplace la diapositive `from` pour qu'elle arrive en position `to` (positions
    /// bornées à la dernière).
    pub fn move_slide(&mut self, from: usize, to: usize) {
        let Some(last) = self.slides.len().checked_sub(1) else {
            return;
        };
        let (from, to) = (from.min(last), to.min(last));
        if from != to {
            let s = self.slides.remove(from);
            self.slides.insert(to, s);
        }
    }

    /// Les valeurs des champs, pour le rendu.
    pub fn values(&self) -> varium_core::Values {
        varium_core::Values {
            values: self.values.clone(),
            types: BTreeMap::new(),
        }
    }

    /// Les champs employés dans les diapositives (titres, sous-titres, contenus,
    /// notes) et le pied de page, sans doublon, dans l'ordre de première apparition.
    pub fn variables(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let textes = self
            .slides
            .iter()
            .flat_map(|s| {
                s.texts()
                    .into_iter()
                    .chain(s.objects.iter().map(|o| o.text.as_str()))
            })
            .chain([self.footer.as_str()]);
        for src in textes {
            for v in varium_core::variables(src) {
                if !out.contains(&v.name) {
                    out.push(v.name);
                }
            }
        }
        out
    }

    /// Couleurs de la diapositive `i` (bornée), voir [`Slide::palette`].
    pub fn slide_palette(&self, i: usize) -> Palette {
        match self.slides.len().checked_sub(1) {
            Some(last) => self.slides[i.min(last)].palette(self.theme),
            None => self.theme.palette(),
        }
    }

    /// Le fichier `.varium-slides.json` (format `varium-slides`, version 1).
    pub fn to_json(&self) -> String {
        fichier::to_json(self)
    }

    /// Lit un fichier de présentation. Les champs absents ou illisibles prennent
    /// leur valeur par défaut ; un fichier sans diapositive en reçoit une vide.
    pub fn from_json(s: &str) -> Result<Self, String> {
        fichier::from_json(s)
    }

    /// Le diaporama en un seul fichier HTML autonome.
    pub fn to_html(&self) -> String {
        html::html(self)
    }

    /// Le fichier OpenDocument Présentation (`.odp`), pour LibreOffice Impress.
    pub fn to_odp(&self) -> Vec<u8> {
        odp::odp(self)
    }

    /// Le PDF : une page par diapositive, au format de la présentation.
    pub fn to_pdf(&self) -> Vec<u8> {
        pdf::pdf(self)
    }

    /// Importe une présentation OpenDocument (`.odp`, LibreOffice Impress).
    /// Erreur en français si le fichier est endommagé ; jamais de panique.
    pub fn from_odp(bytes: &[u8]) -> Result<Self, String> {
        import::odp(bytes)
    }

    /// Importe une présentation PowerPoint (`.pptx`).
    /// Erreur en français si le fichier est endommagé ; jamais de panique.
    pub fn from_pptx(bytes: &[u8]) -> Result<Self, String> {
        pptx::pptx(bytes)
    }

    /// Ouvre un fichier de présentation quel qu'il soit : `.varium-slides.json`,
    /// `.odp` ou `.pptx`, reconnu à son contenu.
    pub fn open_bytes(bytes: &[u8]) -> Result<Self, String> {
        import::ouvrir(bytes)
    }

    /// Les diapositives à exporter : jamais aucune (une diapositive vide sinon).
    fn exported(&self) -> std::borrow::Cow<'_, [Slide]> {
        if self.slides.is_empty() {
            std::borrow::Cow::Owned(vec![Slide::new(Layout::Blank)])
        } else {
            std::borrow::Cow::Borrowed(&self.slides)
        }
    }
}

/// Le texte rendu d'une diapositive, prêt pour un export.
pub(crate) struct Rendu {
    pub palette: Palette,
    pub boxes: SlideLayoutBoxes,
    pub title: Vec<varium_core::Para>,
    pub subtitle: Vec<varium_core::Para>,
    pub body: Vec<varium_core::Para>,
    pub body2: Vec<varium_core::Para>,
    pub notes: Vec<varium_core::Para>,
    /// Pied de page (vide si la disposition n'en montre pas).
    pub footer: Vec<varium_core::Run>,
    /// Numéro affiché (1 pour la première), ou `None`.
    pub number: Option<usize>,
    /// Texte de chaque objet de la diapositive (lignes centrées), dans l'ordre.
    pub objets: Vec<Vec<varium_core::Para>>,
}

impl Deck {
    /// Met en forme chaque diapositive : champs remplis, syntaxe interprétée.
    pub(crate) fn rendus(&self) -> Vec<Rendu> {
        let values = self.values();
        let footer = varium_core::doc::inline(&self.footer, &values);
        self.exported()
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let boxes = boxes(s.layout);
                let lignes = |src: &str| texte::paragraphes_ligne(src, &values);
                Rendu {
                    palette: s.palette(self.theme),
                    boxes,
                    title: if boxes.title.is_some() {
                        lignes(&s.title)
                    } else {
                        Vec::new()
                    },
                    subtitle: if boxes.subtitle.is_some() {
                        lignes(&s.subtitle)
                    } else {
                        Vec::new()
                    },
                    body: if boxes.body.is_some() {
                        varium_core::document(&s.body, &values)
                    } else {
                        Vec::new()
                    },
                    body2: if boxes.body2.is_some() {
                        varium_core::document(&s.body2, &values)
                    } else {
                        Vec::new()
                    },
                    notes: varium_core::document(&s.notes, &values),
                    footer: if boxes.footer {
                        footer.clone()
                    } else {
                        Vec::new()
                    },
                    number: (boxes.footer && self.slide_numbers).then_some(i + 1),
                    objets: s
                        .objects
                        .iter()
                        .map(|o| {
                            if o.kind.is_closed() {
                                texte::paragraphes_centres(&o.text, &values)
                            } else {
                                Vec::new()
                            }
                        })
                        .collect(),
                }
            })
            .collect()
    }
}
