//! « Embellir » : des mises en page de rechange pour une diapositive, choisies
//! d'après son contenu (comme le Concepteur de PowerPoint), et « Harmoniser » :
//! une présentation rendue cohérente d'une diapositive à l'autre.
//!
//! Tout est déterministe : la même diapositive donne toujours les mêmes
//! propositions, dans le même ordre.

use crate::plan::{ligne_de_corps, marque_de_liste};
use crate::texte::{luminance, melange};
use crate::{Deck, Frame, Layout, Object, ObjectKind, Slide, Transition, boxes};

/// La nature d'une proposition d'« Embellir » (pour son libellé).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Design {
    /// Une longue liste répartie sur deux colonnes.
    DeuxColonnes,
    /// Un chiffre clé en très grand, sa légende dessous.
    GrandChiffre,
    /// Le chiffre clé dans une pastille de couleur.
    ChiffrePastille,
    /// Une citation mise en valeur, son auteur dessous.
    Citation,
    /// La citation sur un fond de couleur.
    CitationFond,
    /// L'image à gauche, le texte à droite.
    ImageGauche,
    /// L'image à droite, le texte à gauche.
    ImageDroite,
    /// L'image sur toute la diapositive, le titre dans un bandeau.
    ImagePleinCadre,
    /// Deux listes côte à côte, séparées d'un trait.
    Comparaison,
    /// Deux listes dans deux cadres.
    ComparaisonCadres,
    /// Une phrase courte en grand, comme un intercalaire.
    Message,
    /// Un grand titre aligné à gauche (intercalaire).
    Section,
    /// Le titre centré, en grand.
    TitreCentre,
    /// Une barre de couleur le long du bord gauche.
    BarreAccent,
    /// Un fond de la couleur d'accent : le titre ressort.
    FondContraste,
    /// Un disque de couleur dans le coin.
    CoinDecoratif,
    /// Un bandeau de couleur en haut de la diapositive.
    Bandeau,
}

impl Design {
    /// Libellé français (l'interface le traduit).
    pub fn label(self) -> &'static str {
        match self {
            Design::DeuxColonnes => "Deux colonnes",
            Design::GrandChiffre => "Grand chiffre",
            Design::ChiffrePastille => "Chiffre en pastille",
            Design::Citation => "Citation",
            Design::CitationFond => "Citation sur fond",
            Design::ImageGauche => "Image à gauche",
            Design::ImageDroite => "Image à droite",
            Design::ImagePleinCadre => "Image plein cadre",
            Design::Comparaison => "Comparaison",
            Design::ComparaisonCadres => "Comparaison en cadres",
            Design::Message => "Message clé",
            Design::Section => "Intercalaire",
            Design::TitreCentre => "Titre centré",
            Design::BarreAccent => "Barre d'accent",
            Design::FondContraste => "Fond contrasté",
            Design::CoinDecoratif => "Coin décoratif",
            Design::Bandeau => "Bandeau",
        }
    }
}

/// Une proposition d'« Embellir ».
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub design: Design,
    pub slide: Slide,
}

/// Nombre maximal de propositions.
pub const MAX_SUGGESTIONS: usize = 6;

/// Ce que contient une diapositive, du point de vue de la mise en page.
struct Analyse {
    /// Éléments de premier niveau du contenu, chacun avec ses sous-lignes (source).
    elements: Vec<Vec<String>>,
    /// Lignes non vides du contenu (texte brut, sans marque de liste).
    lignes: Vec<String>,
    /// Un chiffre clé : (le nombre, sa légende).
    chiffre: Option<(String, String)>,
    /// Une citation : (texte, auteur).
    citation: Option<(String, String)>,
    /// Position de la première image et rapport largeur / hauteur de l'image.
    image: Option<(usize, f32)>,
    /// Deux listes à comparer (source Varium de chaque colonne).
    listes: Option<(String, String)>,
}

/// Texte sans les marques de caractères les plus courantes.
fn brut(s: &str) -> String {
    let t: String = s
        .chars()
        .filter(|c| !matches!(c, '*' | '_' | '~'))
        .collect();
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Le début de `s` est-il un chiffre clé (« 87 % », « 1,2 M€ », « +35 % », « x3 ») ?
/// Renvoie le nombre et la suite du texte.
fn chiffre_en_tete(s: &str) -> Option<(String, String)> {
    let s = s.trim();
    let mut fin = 0;
    let mut chiffres = 0;
    let mut vu_espace_apres = false;
    for (i, c) in s.char_indices() {
        let ok = c.is_ascii_digit()
            || (i == 0 && matches!(c, '+' | '-' | '−' | '×' | 'x' | '~' | '≈'))
            || (chiffres > 0 && matches!(c, ',' | '.' | '\u{a0}' | '\u{202f}' | '\''))
            || (chiffres > 0 && c == ' ' && !vu_espace_apres);
        if !ok {
            break;
        }
        if c.is_ascii_digit() {
            chiffres += 1;
            vu_espace_apres = false;
        } else if c == ' ' {
            vu_espace_apres = true;
        }
        fin = i + c.len_utf8();
    }
    if chiffres == 0 || chiffres > 12 {
        return None;
    }
    let nombre = &s[..fin];
    let mut nombre = nombre.trim_end_matches([' ', ',', '.', '\'']).to_string();
    let mut reste = s[nombre.len()..].trim_start().to_string();
    // Unité collée au nombre.
    let unites = [
        "%", "‰", "€", "$", "£", "¥", "k€", "M€", "Md€", "k$", "M$", "k", "K", "M", "Md", "Mds",
        "G", "x", "×", "pts", "pt", "ans", "h", "min", "s", "km", "kg", "t",
    ];
    let mut meilleure: Option<&str> = None;
    for u in unites {
        if let Some(apres) = reste.strip_prefix(u) {
            let suivant = apres.chars().next();
            if suivant.is_none_or(|c| !c.is_alphanumeric())
                && meilleure.is_none_or(|m| u.len() > m.len())
            {
                meilleure = Some(u);
            }
        }
    }
    if let Some(u) = meilleure {
        let sep = if s[nombre.len()..].starts_with(' ') || s[nombre.len()..].starts_with('\u{a0}') {
            "\u{a0}"
        } else {
            ""
        };
        nombre = format!("{nombre}{sep}{u}");
        reste = reste[u.len()..].trim_start().to_string();
    }
    // Un nombre seul au milieu d'une phrase longue n'est pas un chiffre clé.
    if reste.chars().count() > 90 {
        return None;
    }
    let reste = reste
        .trim_start_matches([':', '—', '–', '-', ','])
        .trim()
        .to_string();
    Some((nombre, reste))
}

const GUILLEMETS: [char; 8] = ['«', '»', '“', '”', '"', '„', '‘', '’'];

fn analyser(slide: &Slide, deck: &Deck) -> Analyse {
    let mut elements: Vec<Vec<String>> = Vec::new();
    let mut lignes = Vec::new();
    for l in slide.body.lines() {
        if l.trim().is_empty() {
            continue;
        }
        let (niveau, texte) = ligne_de_corps(l);
        lignes.push(brut(&texte));
        if niveau <= 1 || elements.is_empty() {
            elements.push(vec![l.to_string()]);
        } else if let Some(e) = elements.last_mut() {
            e.push(l.to_string());
        }
    }

    // Chiffre clé : un contenu très court qui commence par un nombre.
    let chiffre = if lignes.len() <= 2 && slide.body2.trim().is_empty() {
        lignes.first().and_then(|premiere| {
            let (n, legende) = chiffre_en_tete(premiere)?;
            let mut legende = legende;
            if let Some(seconde) = lignes.get(1) {
                if !legende.is_empty() {
                    legende.push(' ');
                }
                legende.push_str(seconde);
            }
            Some((n, legende))
        })
    } else {
        None
    };

    // Citation : un seul paragraphe entre guillemets ou en « > », peut-être signé.
    let citation = {
        let sources: Vec<&str> = slide
            .body
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let mut texte = String::new();
        let mut auteur = String::new();
        let mut entre = false;
        for (k, l) in sources.iter().enumerate() {
            let sans = l.trim_start_matches('>').trim();
            let signature = sans
                .strip_prefix('—')
                .or_else(|| sans.strip_prefix('–'))
                .or_else(|| sans.strip_prefix("- ").filter(|_| k > 0 && entre));
            if let Some(a) = signature.filter(|_| k > 0) {
                auteur = brut(a);
                continue;
            }
            if k == 0 {
                entre = l.starts_with('>') || sans.starts_with(GUILLEMETS);
            }
            if !texte.is_empty() {
                texte.push(' ');
            }
            texte.push_str(&brut(sans));
        }
        let texte = texte.trim_matches(|c: char| GUILLEMETS.contains(&c) || c.is_whitespace());
        let assez = (8..=320).contains(&texte.chars().count());
        if (entre || slide.layout == Layout::Quote) && assez && sources.len() <= 4 {
            if auteur.is_empty() {
                auteur = brut(if slide.layout == Layout::Quote {
                    &slide.subtitle
                } else {
                    &slide.title
                });
            }
            Some((texte.to_string(), auteur))
        } else {
            None
        }
    };

    let image = slide.objects.iter().enumerate().find_map(|(k, o)| {
        (o.kind == ObjectKind::Image)
            .then(|| (k, deck.media.get(&o.media).map_or(1.5, |m| m.ratio())))
    });

    // Deux listes : une seconde colonne remplie, ou deux groupes (titres `#`,
    // lignes terminées par « : », ou séparés d'une ligne vide).
    let listes = if !slide.body2.trim().is_empty() && !slide.body.trim().is_empty() {
        Some((
            slide.body.trim().to_string(),
            slide.body2.trim().to_string(),
        ))
    } else {
        deux_groupes(&slide.body)
    };

    Analyse {
        elements,
        lignes,
        chiffre,
        citation,
        image,
        listes,
    }
}

/// Le contenu coupé en deux groupes comparables, s'il en a exactement deux.
fn deux_groupes(body: &str) -> Option<(String, String)> {
    let mut groupes: Vec<Vec<&str>> = Vec::new();
    let mut nouveau = true;
    for l in body.lines() {
        let t = l.trim();
        if t.is_empty() {
            nouveau = true;
            continue;
        }
        let entete = t.starts_with('#') || (t.ends_with(':') && marque_de_liste(t).is_none());
        if entete || nouveau || groupes.is_empty() {
            // Une ligne vide ne coupe pas une liste qui vient d'un en-tête.
            let suite_d_entete = !entete
                && groupes
                    .last()
                    .is_some_and(|g| g.len() == 1 && entete_de(g[0]));
            if suite_d_entete {
                if let Some(g) = groupes.last_mut() {
                    g.push(l);
                }
            } else {
                groupes.push(vec![l]);
            }
        } else if let Some(g) = groupes.last_mut() {
            g.push(l);
        }
        nouveau = false;
    }
    if groupes.len() != 2 || groupes.iter().any(|g| g.len() < 2) {
        return None;
    }
    let colonne = |g: &Vec<&str>| {
        g.iter()
            .map(|l| {
                let t = l.trim();
                if entete_de(t) && !t.starts_with('#') {
                    format!("### {}", t.trim_end_matches(':').trim())
                } else if let Some(reste) = t.strip_prefix('#') {
                    format!("### {}", reste.trim_start_matches('#').trim())
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    Some((colonne(&groupes[0]), colonne(&groupes[1])))
}

fn entete_de(t: &str) -> bool {
    let t = t.trim();
    t.starts_with('#') || (t.ends_with(':') && marque_de_liste(t).is_none())
}

/// Hauteur d'une fraction de largeur, en fraction de hauteur (pour un carré).
fn carre(deck: &Deck, w: f32) -> f32 {
    w * deck.aspect.ratio()
}

/// Un cadre de texte transparent (forme sans fond ni trait).
fn texte_libre(frame: Frame, text: String, size: f32, color: Option<[u8; 3]>) -> Object {
    Object {
        kind: ObjectKind::Rect,
        frame,
        fill: None,
        stroke: None,
        stroke_width: 0.0,
        text,
        text_color: color,
        text_size: size,
        ..Object::default()
    }
}

fn rect(frame: Frame, fill: [u8; 3]) -> Object {
    Object {
        kind: ObjectKind::Rect,
        frame,
        fill: Some(fill),
        stroke: None,
        stroke_width: 0.0,
        ..Object::default()
    }
}

/// L'image dans `zone` en gardant ses proportions (centrée) ; `couvrir` : la zone
/// est entièrement couverte (le surplus dépasse), sinon l'image y tient entière.
fn cadre_image(zone: Frame, ratio_image: f32, ratio_diapo: f32, couvrir: bool) -> Frame {
    let ratio_image = if ratio_image.is_finite() && ratio_image > 0.01 {
        ratio_image
    } else {
        1.5
    };
    // Rapport de la zone en vraies proportions.
    let ratio_zone = zone.w * ratio_diapo / zone.h.max(1e-3);
    let plus_large = ratio_image > ratio_zone;
    let (w, h) = if plus_large == couvrir {
        // Hauteur de la zone, largeur déduite.
        (zone.h * ratio_image / ratio_diapo, zone.h)
    } else {
        (zone.w, zone.w * ratio_diapo / ratio_image)
    };
    Frame::new(
        zone.x + (zone.w - w) / 2.0,
        zone.y + (zone.h - h) / 2.0,
        w,
        h,
    )
}

/// Couleur de fond contrastée tirée du thème.
fn fond_fort(deck: &Deck) -> [u8; 3] {
    let p = deck.theme.palette();
    if luminance(p.background) > 0.4 {
        melange(p.accent, [0, 0, 0], 0.3)
    } else {
        melange(p.accent, p.background, 0.35)
    }
}

/// Le contenu en colonnes : deux moitiés, coupées entre deux éléments.
fn couper_en_deux(elements: &[Vec<String>]) -> (String, String) {
    let total: usize = elements.iter().map(Vec::len).sum();
    let mut cumul = 0;
    let mut coupe = elements.len();
    for (k, e) in elements.iter().enumerate() {
        if cumul * 2 >= total {
            coupe = k;
            break;
        }
        cumul += e.len();
    }
    let coupe = coupe.clamp(1, elements.len().saturating_sub(1).max(1));
    let joindre = |es: &[Vec<String>]| {
        es.iter()
            .flat_map(|e| e.iter().cloned())
            .collect::<Vec<_>>()
            .join("\n")
    };
    (
        joindre(&elements[..coupe.min(elements.len())]),
        joindre(&elements[coupe.min(elements.len())..]),
    )
}

/// Les objets autres que les images (gardés tels quels par la plupart des
/// propositions).
fn formes(slide: &Slide) -> Vec<Object> {
    slide
        .objects
        .iter()
        .filter(|o| o.kind != ObjectKind::Image)
        .cloned()
        .collect()
}

/// Le titre en clair (une ligne).
fn titre_ligne(slide: &Slide) -> String {
    slide
        .title
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Le texte complet du contenu (deux colonnes réunies).
fn tout_le_corps(slide: &Slide) -> String {
    match (slide.body.trim(), slide.body2.trim()) {
        (a, "") => a.to_string(),
        ("", b) => b.to_string(),
        (a, b) if slide.layout == Layout::TwoColumns => format!("{a}\n{b}"),
        (a, _) => a.to_string(),
    }
}

/// Les propositions d'« Embellir » pour une diapositive, de la plus adaptée à
/// son contenu à la plus générale : de 3 à 6, toutes différentes de la
/// diapositive et entre elles. Le texte n'est jamais perdu : ce qui ne s'affiche
/// plus reste dans la diapositive.
pub fn design_suggestions(slide: &Slide, deck: &Deck) -> Vec<Suggestion> {
    let a = analyser(slide, deck);
    let pal = slide.palette(deck.theme);
    let ratio = deck.aspect.ratio();
    let titre = titre_ligne(slide);
    let mut out: Vec<Suggestion> = Vec::new();
    let proposer = |design: Design, s: Slide, out: &mut Vec<Suggestion>| {
        if s != *slide && !out.iter().any(|p| p.slide == s) {
            out.push(Suggestion { design, slide: s });
        }
    };
    let base = || Slide {
        objects: formes(slide),
        ..slide.clone()
    };
    let contenu_haut = !matches!(
        slide.layout,
        Layout::Title | Layout::Section | Layout::Quote | Layout::Blank
    );

    // Chiffre clé.
    if let Some((nombre, legende)) = &a.chiffre {
        let couleur = Some(pal.accent);
        let mut s = base();
        s.layout = if titre.is_empty() {
            Layout::Blank
        } else {
            Layout::TitleOnly
        };
        s.body.clear();
        s.body2.clear();
        let mut objets = vec![texte_libre(
            Frame::new(0.07, 0.27, 0.86, 0.4),
            format!("**{nombre}**"),
            0.24,
            couleur,
        )];
        if !legende.is_empty() {
            objets.push(texte_libre(
                Frame::new(0.1, 0.68, 0.8, 0.16),
                legende.clone(),
                0.05,
                None,
            ));
        }
        objets.extend(formes(slide));
        s.objects = objets;
        proposer(Design::GrandChiffre, s.clone(), &mut out);

        let d = 0.3;
        let mut p = s;
        let h = carre(deck, d);
        let y = 0.3 + (0.6 - h).max(0.0) / 2.0;
        let mut pastille = Object {
            kind: ObjectKind::Ellipse,
            frame: Frame::new(0.08, y, d, h),
            fill: Some(pal.accent),
            stroke: None,
            stroke_width: 0.0,
            text: format!("**{nombre}**"),
            text_size: (0.13 / (nombre.chars().count().max(3) as f32 / 3.0)).max(0.05),
            ..Object::default()
        };
        pastille.text_color = Some(pastille.text_ink(&pal));
        let mut objets = vec![pastille];
        if !legende.is_empty() {
            objets.push(texte_libre(
                Frame::new(0.44, y, 0.49, h),
                legende.clone(),
                0.055,
                None,
            ));
        }
        objets.extend(formes(slide));
        p.objects = objets;
        proposer(Design::ChiffrePastille, p, &mut out);
    }

    // Citation.
    if let Some((texte, auteur)) = &a.citation {
        let mut s = base();
        s.layout = Layout::Quote;
        s.body = format!("« {texte} »");
        s.subtitle = auteur.clone();
        s.body2.clear();
        s.background = None;
        proposer(Design::Citation, s.clone(), &mut out);
        s.background = Some(fond_fort(deck));
        s.objects.insert(
            0,
            texte_libre(
                Frame::new(0.06, 0.02, 0.2, 0.3),
                "“".into(),
                0.3,
                Some(
                    slide
                        .palette(deck.theme)
                        .on_background(fond_fort(deck))
                        .accent,
                ),
            ),
        );
        proposer(Design::CitationFond, s, &mut out);
    }

    // Image.
    if let Some((k, r)) = a.image {
        let img = slide.objects[k].clone();
        let autres: Vec<Object> = slide
            .objects
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != k)
            .map(|(_, o)| o.clone())
            .collect();
        let texte = tout_le_corps(slide);
        let bx = boxes(Layout::TwoColumns);
        if let (Some(gauche), Some(droite)) = (bx.body, bx.body2) {
            let mut g = slide.clone();
            g.layout = Layout::TwoColumns;
            g.body = String::new();
            g.body2 = texte.clone();
            let mut im = img.clone();
            im.frame = cadre_image(gauche, r, ratio, false);
            g.objects = std::iter::once(im).chain(autres.clone()).collect();
            proposer(Design::ImageGauche, g, &mut out);

            let mut d = slide.clone();
            d.layout = Layout::TwoColumns;
            d.body = texte.clone();
            d.body2 = String::new();
            let mut im = img.clone();
            im.frame = cadre_image(droite, r, ratio, false);
            d.objects = std::iter::once(im).chain(autres.clone()).collect();
            proposer(Design::ImageDroite, d, &mut out);
        }
        // Plein cadre : seulement si le texte tient dans un bandeau.
        if a.lignes.len() <= 2 {
            let mut p = slide.clone();
            p.layout = Layout::Blank;
            let mut im = img.clone();
            im.frame = cadre_image(Frame::new(0.0, 0.0, 1.0, 1.0), r, ratio, true);
            let mut legende = if titre.is_empty() {
                String::new()
            } else {
                format!("**{titre}**")
            };
            for l in &a.lignes {
                if !legende.is_empty() {
                    legende.push('\n');
                }
                legende.push_str(l);
            }
            let mut objets = vec![im];
            if !legende.is_empty() {
                let fond = [18, 22, 30];
                let mut bandeau = rect(Frame::new(0.0, 0.72, 1.0, 0.28), fond);
                bandeau.text = legende;
                bandeau.text_size = 0.05;
                bandeau.text_color = Some([255, 255, 255]);
                objets.push(bandeau);
            }
            p.objects = objets;
            proposer(Design::ImagePleinCadre, p, &mut out);
        }
    }

    // Deux listes à comparer.
    if let Some((un, deux)) = &a.listes {
        let bx = boxes(Layout::TwoColumns);
        let mut s = base();
        s.layout = Layout::TwoColumns;
        s.body = un.clone();
        s.body2 = deux.clone();
        let mut c = s.clone();
        s.objects.insert(
            0,
            Object {
                kind: ObjectKind::Line,
                frame: Frame::new(0.495, 0.3, 0.0, 0.56),
                stroke: Some(pal.accent),
                stroke_width: 2.0,
                ..Object::default()
            },
        );
        proposer(Design::Comparaison, s, &mut out);
        if let (Some(g), Some(d)) = (bx.body, bx.body2) {
            for (k, f) in [g, d].into_iter().enumerate() {
                c.objects.insert(
                    k,
                    Object {
                        kind: ObjectKind::RoundRect,
                        frame: Frame::new(f.x - 0.02, f.y - 0.03, f.w + 0.04, f.h + 0.04),
                        fill: None,
                        stroke: Some(pal.accent),
                        stroke_width: 2.0,
                        ..Object::default()
                    },
                );
            }
            proposer(Design::ComparaisonCadres, c, &mut out);
        }
    }

    // Longue liste : deux colonnes.
    let longue = a.elements.len() >= 2 && (a.elements.len() >= 6 || a.lignes.len() >= 6);
    if longue && a.listes.is_none() && slide.body2.trim().is_empty() && a.image.is_none() {
        let (un, deux) = couper_en_deux(&a.elements);
        if !un.trim().is_empty() && !deux.trim().is_empty() {
            let mut s = base();
            s.layout = Layout::TwoColumns;
            s.body = un;
            s.body2 = deux;
            proposer(Design::DeuxColonnes, s, &mut out);
        }
    }

    // Une phrase courte seule : message clé.
    let phrase = a.lignes.len() == 1
        && a.chiffre.is_none()
        && a.citation.is_none()
        && a.lignes[0].chars().count() <= 110;
    if phrase && a.image.is_none() {
        let mut s = base();
        s.layout = Layout::Section;
        s.title = a.lignes[0].clone();
        s.subtitle = titre.clone();
        proposer(Design::Message, s, &mut out);
    }

    // Peu de contenu : intercalaire ou titre centré.
    let vide = a.lignes.is_empty() && a.image.is_none();
    if vide && !titre.is_empty() {
        for (design, layout) in [
            (Design::Section, Layout::Section),
            (Design::TitreCentre, Layout::Title),
        ] {
            let mut s = base();
            s.layout = layout;
            proposer(design, s, &mut out);
        }
    }

    // Propositions générales : un accent de couleur, pour toute diapositive.
    let generales = [
        Design::BarreAccent,
        Design::FondContraste,
        Design::CoinDecoratif,
        Design::Bandeau,
    ];
    for design in generales {
        if out.len() >= MAX_SUGGESTIONS {
            break;
        }
        // Les propositions générales complètent les autres, sans les noyer.
        if out.len() >= 4 && design != Design::BarreAccent {
            break;
        }
        let mut s = slide.clone();
        match design {
            Design::BarreAccent => {
                s.objects
                    .insert(0, rect(Frame::new(0.0, 0.0, 0.022, 1.0), pal.accent));
            }
            Design::FondContraste => {
                s.background = Some(fond_fort(deck));
            }
            Design::CoinDecoratif => {
                let d = 0.26;
                let mut disque = Object {
                    kind: ObjectKind::Ellipse,
                    frame: Frame::new(1.0 - d * 0.6, -carre(deck, d) * 0.4, d, carre(deck, d)),
                    fill: Some(pal.accent),
                    stroke: None,
                    stroke_width: 0.0,
                    ..Object::default()
                };
                if contenu_haut {
                    // Le titre s'arrête avant le disque.
                    disque.frame.x = 0.93 - d * 0.25;
                    disque.frame.w = d * 0.5;
                    disque.frame.h = carre(deck, d * 0.5);
                    disque.frame.y = -disque.frame.h * 0.35;
                }
                s.objects.insert(0, disque);
            }
            _ => {
                s.objects
                    .insert(0, rect(Frame::new(0.0, 0.0, 1.0, 0.03), pal.accent));
                s.objects.insert(
                    1,
                    rect(
                        Frame::new(0.0, 0.03, 1.0, 0.006),
                        melange(pal.accent, pal.background, 0.5),
                    ),
                );
            }
        }
        proposer(design, s, &mut out);
    }
    out.truncate(MAX_SUGGESTIONS);
    out
}

/// Les diapositives proposées par « Embellir » (voir [`design_suggestions`]).
pub fn suggestions(slide: &Slide, deck: &Deck) -> Vec<Slide> {
    design_suggestions(slide, deck)
        .into_iter()
        .map(|s| s.slide)
        .collect()
}

// ---------------------------------------------------------------------------------
// Harmoniser
// ---------------------------------------------------------------------------------

/// Tolérance pour reconnaître deux objets « à la même place ».
const PROCHE: f32 = 0.04;

fn proches(a: &Frame, b: &Frame) -> bool {
    (a.x - b.x).abs() <= PROCHE
        && (a.y - b.y).abs() <= PROCHE
        && (a.w - b.w).abs() <= PROCHE
        && (a.h - b.h).abs() <= PROCHE
}

/// La valeur la plus fréquente (la première rencontrée en cas d'égalité).
fn plus_frequent<T: PartialEq + Clone>(v: &[T]) -> Option<T> {
    let mut meilleur: Option<(&T, usize)> = None;
    for x in v {
        let n = v.iter().filter(|y| *y == x).count();
        if meilleur.is_none_or(|(_, m)| n > m) {
            meilleur = Some((x, n));
        }
    }
    meilleur.map(|(x, _)| x.clone())
}

/// Le titre nettoyé : espaces superflus, point final retiré, majuscule initiale.
fn titre_harmonise(t: &str) -> String {
    let lignes: Vec<String> = t
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    let mut s = lignes.join("\n").trim().to_string();
    if s.ends_with('.') && !s.ends_with("..") {
        s.pop();
    }
    let mut cs = s.chars();
    match cs.next() {
        Some(c) if c.is_lowercase() => c.to_uppercase().chain(cs).collect(),
        _ => s,
    }
}

/// Le contenu nettoyé : puces « * », « • », « – » écrites « - », espaces de fin
/// retirés, pas plus d'une ligne vide de suite.
fn corps_harmonise(src: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for l in src.lines() {
        let l = l.trim_end();
        let retrait = l.len() - l.trim_start().len();
        let t = l.trim_start();
        let t = ["* ", "• ", "– ", "· ", "▪ ", "► "]
            .iter()
            .find_map(|m| t.strip_prefix(m))
            .map_or_else(
                || t.to_string(),
                |reste| format!("- {}", reste.trim_start()),
            );
        let ligne = format!("{}{t}", &l[..retrait]);
        if ligne.is_empty() && out.last().is_some_and(String::is_empty) {
            continue;
        }
        out.push(ligne);
    }
    while out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    while out.first().is_some_and(String::is_empty) {
        out.remove(0);
    }
    out.join("\n")
}

impl Deck {
    /// « Embellir » : les propositions de mise en page pour la diapositive `i`
    /// (vide si elle n'existe pas).
    pub fn design_suggestions(&self, i: usize) -> Vec<Suggestion> {
        self.slides
            .get(i)
            .map(|s| design_suggestions(s, self))
            .unwrap_or_default()
    }

    /// « Harmoniser » : rend la présentation cohérente.
    ///
    /// - titres nettoyés (espaces, point final, majuscule initiale) ;
    /// - puces du contenu écrites de la même façon ;
    /// - une diapositive « Titre seul » qui a du contenu passe en « Titre et contenu » ;
    /// - une même transition pour toutes les diapositives (la plus employée) ;
    /// - les objets semblables d'une diapositive à l'autre (même nature, presque
    ///   la même place) alignés sur la même position, avec la même taille de texte
    ///   et la même épaisseur de trait ;
    /// - les tailles de texte proches des formes ramenées à la plus employée.
    ///
    /// Renvoie le nombre de diapositives modifiées.
    pub fn harmonize(&mut self) -> usize {
        let avant = self.slides.clone();
        for s in &mut self.slides {
            s.title = titre_harmonise(&s.title);
            s.body = corps_harmonise(&s.body);
            s.body2 = corps_harmonise(&s.body2);
            if s.layout == Layout::TitleOnly && !s.body.trim().is_empty() {
                s.layout = Layout::TitleContent;
            }
        }
        // Transitions.
        let transitions: Vec<Transition> = self.slides.iter().map(|s| s.transition).collect();
        if let Some(t) = plus_frequent(&transitions) {
            for s in &mut self.slides {
                s.transition = t;
            }
        }
        // Objets semblables : la place, le texte et le trait du premier.
        let mut faits: Vec<(usize, usize)> = Vec::new();
        for i in 0..self.slides.len() {
            for k in 0..self.slides[i].objects.len() {
                if faits.contains(&(i, k)) {
                    continue;
                }
                let modele = self.slides[i].objects[k].clone();
                let mut groupe = vec![(i, k)];
                for j in i + 1..self.slides.len() {
                    if let Some(m) = self.slides[j]
                        .objects
                        .iter()
                        .enumerate()
                        .position(|(m, o)| {
                            o.kind == modele.kind
                                && proches(&o.frame, &modele.frame)
                                && !faits.contains(&(j, m))
                        })
                    {
                        groupe.push((j, m));
                    }
                }
                if groupe.len() < 2 {
                    continue;
                }
                let tailles: Vec<u32> = groupe
                    .iter()
                    .map(|&(a, b)| (self.slides[a].objects[b].text_size * 1000.0).round() as u32)
                    .collect();
                let traits: Vec<u32> = groupe
                    .iter()
                    .map(|&(a, b)| (self.slides[a].objects[b].stroke_width * 10.0).round() as u32)
                    .collect();
                let taille =
                    plus_frequent(&tailles).map_or(modele.text_size, |t| t as f32 / 1000.0);
                let epaisseur =
                    plus_frequent(&traits).map_or(modele.stroke_width, |t| t as f32 / 10.0);
                for &(a, b) in &groupe {
                    let o = &mut self.slides[a].objects[b];
                    if o.kind == ObjectKind::Image {
                        // Une image garde ses proportions : seule sa place change.
                        o.frame.x = modele.frame.x;
                        o.frame.y = modele.frame.y;
                    } else {
                        o.frame = modele.frame;
                    }
                    o.text_size = taille;
                    o.stroke_width = epaisseur;
                    faits.push((a, b));
                }
            }
        }
        // Tailles de texte voisines (à 15 % près) : la plus employée.
        let mut tailles: Vec<u32> = self
            .slides
            .iter()
            .flat_map(|s| &s.objects)
            .filter(|o| o.kind.is_closed() && !o.text.trim().is_empty())
            .map(|o| (o.text_size * 1000.0).round() as u32)
            .collect();
        tailles.sort_unstable();
        for s in &mut self.slides {
            for o in &mut s.objects {
                if !o.kind.is_closed() || o.text.trim().is_empty() {
                    continue;
                }
                let t = (o.text_size * 1000.0).round() as u32;
                let voisines: Vec<u32> = tailles
                    .iter()
                    .copied()
                    .filter(|&u| (u as f32 - t as f32).abs() <= t as f32 * 0.15)
                    .collect();
                if let Some(m) = plus_frequent(&voisines)
                    && voisines.iter().filter(|&&u| u == m).count() > 1
                {
                    o.text_size = m as f32 / 1000.0;
                }
            }
        }
        self.slides
            .iter()
            .zip(&avant)
            .filter(|(a, b)| a != b)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck_avec(slide: Slide) -> Deck {
        let mut d = Deck::new();
        d.slides = vec![slide];
        d
    }

    fn diapo(layout: Layout, title: &str, body: &str) -> Slide {
        Slide {
            layout,
            title: title.into(),
            body: body.into(),
            ..Slide::default()
        }
    }

    fn designs(s: &Slide) -> Vec<Design> {
        let d = deck_avec(s.clone());
        design_suggestions(s, &d)
            .into_iter()
            .map(|p| p.design)
            .collect()
    }

    #[test]
    fn chiffres_cles() {
        assert_eq!(
            chiffre_en_tete("87 % de clients satisfaits"),
            Some(("87\u{a0}%".into(), "de clients satisfaits".into()))
        );
        assert_eq!(
            chiffre_en_tete("+35%"),
            Some(("+35%".into(), String::new()))
        );
        assert_eq!(
            chiffre_en_tete("1 200 000 visiteurs"),
            Some(("1 200 000".into(), "visiteurs".into()))
        );
        assert_eq!(chiffre_en_tete("Bonjour"), None);
        assert_eq!(chiffre_en_tete(""), None);
        assert_eq!(chiffre_en_tete("x"), None);
        assert_eq!(
            chiffre_en_tete("3,5 M€ de chiffre d'affaires").unwrap().0,
            "3,5\u{a0}M€"
        );
    }

    #[test]
    fn grand_chiffre() {
        let s = diapo(
            Layout::TitleContent,
            "Satisfaction",
            "87 %\nde clients satisfaits",
        );
        let d = designs(&s);
        assert_eq!(d[0], Design::GrandChiffre);
        assert!(d.contains(&Design::ChiffrePastille));
        let deck = deck_avec(s.clone());
        let p = &design_suggestions(&s, &deck)[0].slide;
        assert_eq!(p.layout, Layout::TitleOnly);
        assert!(p.objects[0].text.contains("87"));
        assert!(p.objects[1].text.contains("clients"));
        assert_eq!(p.title, "Satisfaction");
    }

    #[test]
    fn longue_liste_en_colonnes() {
        let body = (1..=8)
            .map(|i| format!("- Point {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let s = diapo(Layout::TitleContent, "Programme", &body);
        let deck = deck_avec(s.clone());
        let p = design_suggestions(&s, &deck);
        assert_eq!(p[0].design, Design::DeuxColonnes);
        assert_eq!(p[0].slide.layout, Layout::TwoColumns);
        assert_eq!(p[0].slide.body.lines().count(), 4);
        assert_eq!(p[0].slide.body2.lines().count(), 4);
        assert!(p.len() >= 3 && p.len() <= MAX_SUGGESTIONS);
    }

    #[test]
    fn citation() {
        let s = diapo(
            Layout::TitleContent,
            "Notre devise",
            "« Un document. Toutes ses possibilités. »\n— Camille Martin",
        );
        let deck = deck_avec(s.clone());
        let p = design_suggestions(&s, &deck);
        assert_eq!(p[0].design, Design::Citation);
        assert_eq!(p[0].slide.layout, Layout::Quote);
        assert_eq!(p[0].slide.subtitle, "Camille Martin");
        assert!(
            p[0].slide
                .body
                .contains("Un document. Toutes ses possibilités.")
        );
        assert_eq!(p[1].design, Design::CitationFond);
    }

    #[test]
    fn comparaison() {
        let s = diapo(
            Layout::TitleContent,
            "Avant / après",
            "Avant :\n- lent\n- cher\n\nAprès :\n- rapide\n- gratuit",
        );
        let deck = deck_avec(s.clone());
        let p = design_suggestions(&s, &deck);
        assert_eq!(p[0].design, Design::Comparaison);
        assert_eq!(p[0].slide.body, "### Avant\n- lent\n- cher");
        assert_eq!(p[0].slide.body2, "### Après\n- rapide\n- gratuit");
        assert_eq!(p[1].design, Design::ComparaisonCadres);
    }

    #[test]
    fn image() {
        let mut deck = Deck::new();
        let png = {
            let img = image::RgbImage::from_pixel(40, 20, image::Rgb([200, 30, 30]));
            let mut v = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut v), image::ImageFormat::Png)
                .unwrap();
            v
        };
        deck.slides = vec![diapo(Layout::TitleContent, "Notre atelier", "- À Lyon")];
        deck.insert_image(0, &png).unwrap();
        let p = deck.design_suggestions(0);
        let d: Vec<Design> = p.iter().map(|s| s.design).collect();
        assert_eq!(
            d[..3],
            [
                Design::ImageGauche,
                Design::ImageDroite,
                Design::ImagePleinCadre
            ]
        );
        // Image à gauche : texte à droite, image dans la colonne de gauche,
        // proportions gardées (2:1).
        let g = &p[0].slide;
        assert_eq!(g.body2, "- À Lyon");
        let f = g.objects[0].frame;
        assert!(f.x >= 0.069 && f.right() <= 0.481);
        let vrai = f.w * deck.aspect.ratio() / f.h;
        assert!((vrai - 2.0).abs() < 0.01, "{vrai}");
        // Plein cadre : l'image couvre tout, le bandeau porte le titre.
        let pc = &p[2].slide;
        let f = pc.objects[0].frame;
        assert!(f.x <= 0.0 && f.y <= 0.0 && f.right() >= 1.0 && f.bottom() >= 1.0);
        assert!(pc.objects[1].text.contains("Notre atelier"));
    }

    #[test]
    fn toujours_trois_a_six_propositions_distinctes() {
        let cas = [
            Slide::default(),
            Slide::new(Layout::Title),
            diapo(Layout::TitleContent, "Titre", ""),
            diapo(Layout::TitleContent, "Titre", "Une seule phrase."),
            diapo(Layout::Blank, "", "-> **Merci !** <-"),
            diapo(Layout::TwoColumns, "A", "- a\n- b"),
        ];
        for s in cas {
            let deck = deck_avec(s.clone());
            let p = design_suggestions(&s, &deck);
            assert!(p.len() >= 3 && p.len() <= 6, "{s:?} : {}", p.len());
            for (k, a) in p.iter().enumerate() {
                assert_ne!(a.slide, s);
                for b in &p[k + 1..] {
                    assert_ne!(a.slide, b.slide);
                }
            }
            // Déterministe.
            assert_eq!(p, design_suggestions(&s, &deck));
        }
        for s in Deck::sample().slides {
            let deck = Deck::sample();
            let p = design_suggestions(&s, &deck);
            assert!((3..=6).contains(&p.len()));
        }
    }

    #[test]
    fn harmoniser() {
        let mut deck = Deck::new();
        let forme = |x: f32, t: f32| Object {
            frame: Frame::new(x, 0.8, 0.2, 0.1),
            text: "x".into(),
            text_size: t,
            ..Object::default()
        };
        deck.slides = vec![
            Slide {
                title: "bonjour.".into(),
                body: "* un\n• deux  \n\n\n\ntrois".into(),
                objects: vec![forme(0.1, 0.04)],
                ..Slide::new(Layout::TitleContent)
            },
            Slide {
                title: "Suite".into(),
                transition: Transition::Zoom,
                objects: vec![forme(0.12, 0.042)],
                ..Slide::new(Layout::TitleContent)
            },
            Slide {
                title: "Fin".into(),
                body: "- a".into(),
                objects: vec![forme(0.6, 0.04)],
                ..Slide::new(Layout::TitleOnly)
            },
        ];
        assert_eq!(deck.harmonize(), 3);
        assert_eq!(deck.slides[0].title, "Bonjour");
        assert_eq!(deck.slides[0].body, "- un\n- deux\n\ntrois");
        assert_eq!(deck.slides[1].transition, Transition::Aucune);
        assert_eq!(
            deck.slides[1].objects[0].frame,
            deck.slides[0].objects[0].frame
        );
        assert_eq!(deck.slides[1].objects[0].text_size, 0.04);
        // L'objet éloigné ne bouge pas.
        assert_eq!(deck.slides[2].objects[0].frame.x, 0.6);
        assert_eq!(deck.slides[2].layout, Layout::TitleContent);
        // Une seconde passe ne change plus rien.
        assert_eq!(deck.harmonize(), 0);
    }
}
