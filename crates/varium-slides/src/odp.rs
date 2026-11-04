//! Export OpenDocument Présentation (`.odp`), le format de LibreOffice Impress.
//!
//! Une page `draw:page` par diapositive : fond de la couleur du thème, transition,
//! titre (`presentation:class="title"`), sous-titre, zones de contenu (listes à
//! puces en `text:list`, tableaux en colonnes à tabulations), filet décoratif,
//! pied de page, numéro (champ `text:page-number`) et notes de l'orateur dans
//! `presentation:notes`. Les zones de texte réduisent leur contenu s'il déborde.

use std::collections::HashMap;
use std::fmt::Write;

use varium_core::zip::Zip;
use varium_core::{Align, Para, Run, Style, Table};

use crate::geometrie::{
    Anchor, BODY_LINE_HEIGHT, FOOTER_SIZE, Frame, TITLE_LINE_HEIGHT, footer_frame, number_frame,
};
use crate::objets::{Object, ObjectKind};
use crate::pdf::{Police, colonnes};
use crate::texte::{MISSING_COLOR, echapper, hex_color};
use crate::{
    Deck, Layout, Palette, Rendu, Slide, Transition, body_size, subtitle_size, title_size,
};

const NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" xmlns:number="urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0" xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" xmlns:smil="urn:oasis:names:tc:opendocument:xmlns:smil-compatible:1.0" xmlns:anim="urn:oasis:names:tc:opendocument:xmlns:animation:1.0" office:version="1.3""#;

const POLICES: &str = r#"<office:font-face-decls><style:font-face style:name="Liberation Sans" svg:font-family="'Liberation Sans'" style:font-family-generic="swiss" style:font-pitch="variable"/><style:font-face style:name="Liberation Serif" svg:font-family="'Liberation Serif'" style:font-family-generic="roman" style:font-pitch="variable"/></office:font-face-decls>"#;

/// Largeur d'une diapositive, en points (25,4 cm).
const LARGEUR: f32 = 720.0;

/// Points vers centimètres ODF.
fn cm(pt: f32) -> String {
    let v = if pt.is_finite() {
        pt * 2.54 / 72.0
    } else {
        0.0
    };
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0cm".into()
    } else {
        format!("{s}cm")
    }
}

fn pt(v: f32) -> String {
    let s = format!("{:.1}", if v.is_finite() { v } else { 12.0 });
    format!("{}pt", s.trim_end_matches('0').trim_end_matches('.'))
}

/// Texte ODF : les espaces multiples, tabulations et retours doivent être explicites.
fn odf_text(s: &str) -> String {
    let mut out = String::new();
    let mut espaces = 0usize;
    let mut debut = true;
    let vider = |out: &mut String, espaces: &mut usize, debut: bool| {
        if *espaces == 0 {
            return;
        }
        if debut {
            let _ = write!(out, "<text:s text:c=\"{espaces}\"/>");
        } else {
            out.push(' ');
            if *espaces > 1 {
                let _ = write!(out, "<text:s text:c=\"{}\"/>", *espaces - 1);
            }
        }
        *espaces = 0;
    };
    for c in s.chars() {
        match c {
            ' ' => espaces += 1,
            '\t' => {
                vider(&mut out, &mut espaces, debut);
                out.push_str("<text:tab/>");
                debut = false;
            }
            '\n' => {
                vider(&mut out, &mut espaces, debut);
                out.push_str("<text:line-break/>");
                debut = false;
            }
            '\r' => {}
            c => {
                vider(&mut out, &mut espaces, debut);
                debut = false;
                out.push_str(&echapper(c.encode_utf8(&mut [0; 4])));
            }
        }
    }
    vider(&mut out, &mut espaces, debut);
    out
}

// ---------------------------------------------------------------------------------
// Styles automatiques
// ---------------------------------------------------------------------------------

/// Style de paragraphe (et de ses caractères).
#[derive(Debug, Clone, PartialEq)]
struct PStyle {
    align: Align,
    taille: f32,
    couleur: [u8; 3],
    serif: bool,
    gras: bool,
    italique: bool,
    /// Interligne, en pourcentage de la hauteur de ligne de la police.
    interligne: u32,
    marge_bas: f32,
    marge_gauche: f32,
    majuscules: bool,
    /// Approche, en points.
    approche: f32,
    /// Taquets de tabulation : (position en points, type ODF).
    taquets: Vec<(f32, &'static str)>,
}

/// Les styles automatiques, sans doublon.
#[derive(Default)]
struct Styles {
    xml: String,
    noms: HashMap<String, String>,
    compte: HashMap<&'static str, usize>,
    /// Dégradés nommés (`draw:gradient`), écrits dans `styles.xml`.
    degrades: String,
    noms_degrades: HashMap<String, String>,
}

impl Styles {
    /// Nom du dégradé `g` (déclaré une seule fois dans `office:styles`).
    fn degrade(&mut self, g: &crate::Gradient) -> String {
        let (debut, fin) = match (g.stops.first(), g.stops.last()) {
            (Some(a), Some(b)) => (a.color, b.color),
            _ => ([0, 0, 0], [0, 0, 0]),
        };
        let xml_style = if g.radial {
            // ODF : la couleur de départ est au bord, celle d'arrivée au centre.
            format!(
                "draw:style=\"ellipsoid\" draw:cx=\"50%\" draw:cy=\"50%\" draw:start-color=\"{}\" draw:end-color=\"{}\"",
                hex_color(fin),
                hex_color(debut)
            )
        } else {
            // ODF : angle 0 de haut en bas, compté dans le sens inverse des aiguilles.
            let a = ((90.0 - g.angle).rem_euclid(360.0) * 10.0).round() as i32 % 3600;
            format!(
                "draw:style=\"linear\" draw:angle=\"{a}\" draw:start-color=\"{}\" draw:end-color=\"{}\"",
                hex_color(debut),
                hex_color(fin)
            )
        };
        if let Some(n) = self.noms_degrades.get(&xml_style) {
            return n.clone();
        }
        let k = self.noms_degrades.len() + 1;
        let nom = format!("Varium_20_degrade_20_{k}");
        let _ = write!(
            self.degrades,
            "<draw:gradient draw:name=\"{nom}\" draw:display-name=\"Varium degrade {k}\" {xml_style} draw:start-intensity=\"100%\" draw:end-intensity=\"100%\" draw:border=\"0%\"/>"
        );
        self.noms_degrades.insert(xml_style, nom.clone());
        nom
    }

    fn style(
        &mut self,
        prefixe: &'static str,
        cle: String,
        xml: impl FnOnce(&str) -> String,
    ) -> String {
        let cle = format!("{prefixe}|{cle}");
        if let Some(n) = self.noms.get(&cle) {
            return n.clone();
        }
        let k = self.compte.entry(prefixe).or_insert(0);
        *k += 1;
        let nom = format!("{prefixe}{k}");
        self.xml.push_str(&xml(&nom));
        self.noms.insert(cle, nom.clone());
        nom
    }

    fn paragraphe(&mut self, s: &PStyle) -> String {
        let s2 = s.clone();
        self.style("P", format!("{s:?}"), move |nom| {
            let s = s2;
            let align = match s.align {
                Align::Left => "start",
                Align::Center => "center",
                Align::Right => "end",
                Align::Justify => "justify",
            };
            let mut taquets = String::new();
            if !s.taquets.is_empty() {
                taquets.push_str("<style:tab-stops>");
                for (p, t) in &s.taquets {
                    let _ = write!(
                        taquets,
                        "<style:tab-stop style:position=\"{}\" style:type=\"{t}\"/>",
                        cm(*p)
                    );
                }
                taquets.push_str("</style:tab-stops>");
            }
            let police = if s.serif {
                "Liberation Serif"
            } else {
                "Liberation Sans"
            };
            let mut texte = format!(
                "style:font-name=\"{police}\" fo:font-size=\"{}\" fo:color=\"{}\"",
                pt(s.taille),
                hex_color(s.couleur)
            );
            texte.push_str(if s.gras {
                " fo:font-weight=\"bold\""
            } else {
                " fo:font-weight=\"normal\""
            });
            texte.push_str(if s.italique {
                " fo:font-style=\"italic\""
            } else {
                " fo:font-style=\"normal\""
            });
            if s.majuscules {
                texte.push_str(" fo:text-transform=\"uppercase\"");
            }
            if s.approche.abs() > 0.01 {
                let _ = write!(texte, " fo:letter-spacing=\"{}\"", cm(s.approche));
            }
            format!(
                "<style:style style:name=\"{nom}\" style:family=\"paragraph\"><style:paragraph-properties fo:text-align=\"{align}\" fo:margin-top=\"0cm\" fo:margin-bottom=\"{}\" fo:margin-left=\"{}\" fo:margin-right=\"0cm\" fo:text-indent=\"0cm\" fo:line-height=\"{}%\">{taquets}</style:paragraph-properties><style:text-properties {texte}/></style:style>",
                cm(s.marge_bas),
                cm(s.marge_gauche),
                s.interligne
            )
        })
    }

    /// Style de caractères d'un morceau ; `None` s'il n'a rien de particulier.
    fn caracteres(&mut self, st: Style, manque: bool) -> Option<String> {
        if st.mask() == 0 && !manque {
            return None;
        }
        Some(self.style("T", format!("{}{manque}", st.mask()), |nom| {
            let mut p = String::new();
            if st.bold {
                p.push_str(" fo:font-weight=\"bold\"");
            }
            if st.italic {
                p.push_str(" fo:font-style=\"italic\"");
            }
            if st.underline {
                p.push_str(
                    " style:text-underline-style=\"solid\" style:text-underline-width=\"auto\" style:text-underline-color=\"font-color\"",
                );
            }
            if st.strike {
                p.push_str(
                    " style:text-line-through-style=\"solid\" style:text-line-through-type=\"single\"",
                );
            }
            if manque {
                let _ = write!(p, " fo:color=\"{}\"", hex_color(MISSING_COLOR));
            }
            format!("<style:style style:name=\"{nom}\" style:family=\"text\"><style:text-properties{p}/></style:style>")
        }))
    }

    /// Style de liste (puces ou numéros) à la couleur d'accent.
    fn liste(&mut self, numerote: bool, accent: [u8; 3], taille: f32, serif: bool) -> String {
        let cle = format!("{numerote}{accent:?}{taille}{serif}");
        self.style("L", cle, |nom| {
            let pas = 1.25 * taille;
            let police = if serif {
                "Liberation Serif"
            } else {
                "Liberation Sans"
            };
            let mut s = format!("<text:list-style style:name=\"{nom}\">");
            for niveau in 1..=10u32 {
                let props = format!(
                    "<style:list-level-properties text:space-before=\"{}\" text:min-label-width=\"{}\"/><style:text-properties fo:font-family=\"{police}\" style:font-name=\"{police}\" fo:color=\"{}\" style:use-window-font-color=\"false\" fo:font-size=\"100%\" fo:font-weight=\"bold\"/>",
                    cm(pas * (niveau - 1) as f32),
                    cm(pas),
                    hex_color(accent)
                );
                if numerote {
                    let _ = write!(
                        s,
                        "<text:list-level-style-number text:level=\"{niveau}\" style:num-format=\"1\" style:num-suffix=\".\">{props}</text:list-level-style-number>"
                    );
                } else {
                    let puce = ["•", "–", "◦"][(niveau as usize - 1) % 3];
                    let _ = write!(
                        s,
                        "<text:list-level-style-bullet text:level=\"{niveau}\" text:bullet-char=\"{puce}\" text:bullet-relative-size=\"100%\">{props}</text:list-level-style-bullet>"
                    );
                }
            }
            s.push_str("</text:list-style>");
            s
        })
    }

    /// Style d'un cadre de texte : sans fond ni bordure, ancré, ajusté au contenu.
    fn cadre(&mut self, famille: &'static str, ancre: Anchor) -> String {
        let prefixe = if famille == "presentation" {
            "pr"
        } else {
            "gr"
        };
        self.style(prefixe, format!("{ancre:?}"), |nom| {
            let v = match ancre {
                Anchor::Top => "top",
                Anchor::Middle => "middle",
                Anchor::Bottom => "bottom",
            };
            format!(
                "<style:style style:name=\"{nom}\" style:family=\"{famille}\"><style:graphic-properties draw:stroke=\"none\" draw:fill=\"none\" draw:textarea-horizontal-align=\"justify\" draw:textarea-vertical-align=\"{v}\" draw:auto-grow-height=\"false\" draw:auto-grow-width=\"false\" fo:min-height=\"0cm\" fo:padding-top=\"0cm\" fo:padding-bottom=\"0cm\" fo:padding-left=\"0cm\" fo:padding-right=\"0cm\" draw:fit-to-size=\"false\" style:shrink-to-fit=\"true\" fo:wrap-option=\"wrap\"/></style:style>"
            )
        })
    }

    /// Style du filet décoratif.
    fn filet(&mut self, couleur: [u8; 3]) -> String {
        self.style("ga", hex_color(couleur), |nom| {
            format!(
                "<style:style style:name=\"{nom}\" style:family=\"graphic\"><style:graphic-properties draw:stroke=\"none\" draw:fill=\"solid\" draw:fill-color=\"{}\" draw:shadow=\"hidden\"/></style:style>",
                hex_color(couleur)
            )
        })
    }

    /// Style d'une forme : fond, trait, pointe de flèche, texte centré.
    fn forme(&mut self, o: &Object, pw: f32) -> String {
        let fond = o.fill.filter(|_| o.kind.is_closed());
        let trait_ = o.stroke.filter(|_| o.stroke_width > 0.0);
        let fleche = o.kind == ObjectKind::Arrow;
        let largeur = o.stroke_width * pw / 720.0;
        let degrade = o
            .gradient
            .as_ref()
            .filter(|_| o.kind.is_closed())
            .map(|g| self.degrade(g));
        let cle = format!("{fond:?}{trait_:?}{largeur}{fleche}{degrade:?}");
        self.style("gf", cle, |nom| {
            let mut p = String::new();
            match fond {
                _ if degrade.is_some() => {
                    let _ = write!(
                        p,
                        " draw:fill=\"gradient\" draw:fill-gradient-name=\"{}\"",
                        degrade.as_deref().unwrap_or_default()
                    );
                }
                Some(c) => {
                    let _ = write!(
                        p,
                        " draw:fill=\"solid\" draw:fill-color=\"{}\"",
                        hex_color(c)
                    );
                }
                None => p.push_str(" draw:fill=\"none\""),
            }
            match trait_ {
                Some(c) => {
                    let _ = write!(
                        p,
                        " draw:stroke=\"solid\" svg:stroke-color=\"{}\" svg:stroke-width=\"{}\" draw:stroke-linejoin=\"round\" svg:stroke-linecap=\"round\"",
                        hex_color(c),
                        cm(largeur)
                    );
                }
                None => p.push_str(" draw:stroke=\"none\""),
            }
            if fleche {
                let _ = write!(
                    p,
                    " draw:marker-end=\"Varium_20_fleche\" draw:marker-end-width=\"{}\" draw:marker-end-center=\"false\"",
                    cm((largeur * 3.5).max(8.0 * pw / 720.0))
                );
            }
            format!(
                "<style:style style:name=\"{nom}\" style:family=\"graphic\"><style:graphic-properties{p} draw:shadow=\"hidden\" draw:textarea-horizontal-align=\"justify\" draw:textarea-vertical-align=\"middle\" draw:auto-grow-height=\"false\" draw:auto-grow-width=\"false\" fo:padding-top=\"0.1cm\" fo:padding-bottom=\"0.1cm\" fo:padding-left=\"0.2cm\" fo:padding-right=\"0.2cm\" fo:wrap-option=\"wrap\"/></style:style>"
            )
        })
    }

    /// Style d'un cadre d'image : sans fond ni trait.
    fn image(&mut self) -> String {
        self.style("gi", String::new(), |nom| {
            format!(
                "<style:style style:name=\"{nom}\" style:family=\"graphic\"><style:graphic-properties draw:stroke=\"none\" draw:fill=\"none\" draw:shadow=\"hidden\"/></style:style>"
            )
        })
    }

    /// Style de page : fond et transition.
    fn page(&mut self, fond: [u8; 3], transition: Transition, duree: Option<f32>) -> String {
        let cle = format!("{fond:?}{transition:?}{duree:?}");
        self.style("dp", cle, |nom| {
            // Avance automatique : la page reste affichée `duree` secondes.
            let auto = match duree {
                Some(d) => {
                    let t = (d.max(0.0) * 10.0).round() / 10.0;
                    let s = t % 60.0;
                    let m = (t / 60.0).floor() as u64;
                    format!(
                        " presentation:transition-type=\"automatic\" presentation:duration=\"PT{:02}H{:02}M{}S\"",
                        m / 60,
                        m % 60,
                        if s.fract() == 0.0 {
                            format!("{:02}", s as u64)
                        } else {
                            format!("{s:04.1}")
                        }
                    )
                }
                None => String::new(),
            };
            let effet = match transition {
                Transition::Aucune => "",
                Transition::Fondu => {
                    " smil:type=\"fade\" smil:subtype=\"crossfade\" presentation:transition-speed=\"medium\""
                }
                Transition::Glissement => {
                    " smil:type=\"pushWipe\" smil:subtype=\"fromRight\" presentation:transition-speed=\"medium\""
                }
                Transition::Zoom => {
                    " smil:type=\"irisWipe\" smil:subtype=\"rectangle\" presentation:transition-speed=\"medium\""
                }
            };
            format!(
                "<style:style style:name=\"{nom}\" style:family=\"drawing-page\"><style:drawing-page-properties draw:background-size=\"full\" draw:fill=\"solid\" draw:fill-color=\"{}\" presentation:background-visible=\"true\" presentation:background-objects-visible=\"true\" presentation:display-footer=\"false\" presentation:display-page-number=\"false\" presentation:display-date-time=\"false\"{effet}{auto}/></style:style>",
                hex_color(fond)
            )
        })
    }
}

// ---------------------------------------------------------------------------------
// Texte
// ---------------------------------------------------------------------------------

/// Habillage d'une zone de texte.
#[derive(Debug, Clone, Copy)]
struct Habillage {
    taille: f32,
    couleur: [u8; 3],
    serif: bool,
    gras: bool,
    italique: bool,
    majuscules: bool,
    /// Approche en em.
    approche: f32,
    /// Interligne en multiple de la taille (comme en CSS).
    interligne: f32,
    align: Align,
    /// Zone de titre : pas d'espace entre les paragraphes.
    titre: bool,
    /// Contenu de la disposition Citation.
    citation: bool,
    pal: Palette,
    largeur: f32,
}

impl Habillage {
    fn pstyle(&self) -> PStyle {
        PStyle {
            align: self.align,
            taille: self.taille,
            couleur: self.couleur,
            serif: self.serif,
            gras: self.gras,
            italique: self.italique,
            interligne: interligne(self.interligne),
            marge_bas: if self.titre { 0.0 } else { 0.4 * self.taille },
            marge_gauche: 0.0,
            majuscules: self.majuscules,
            approche: self.approche * self.taille,
            taquets: Vec::new(),
        }
    }
}

/// Interligne CSS (multiple de la taille) en pourcentage ODF, qui se rapporte à
/// la hauteur naturelle d'une ligne (1,117 em pour Liberation).
fn interligne(css: f32) -> u32 {
    (css / 1.117 * 100.0).round().clamp(50.0, 400.0) as u32
}

fn runs_xml(runs: &[Run], styles: &mut Styles) -> String {
    let mut out = String::new();
    for r in runs {
        let manque = matches!(r, Run::Missing { .. });
        let mut st = r.style();
        if matches!(r, Run::Math { .. }) {
            st.italic = true;
        }
        let corps = odf_text(&r.plain());
        match styles.caracteres(st, manque) {
            Some(nom) => {
                let _ = write!(
                    out,
                    "<text:span text:style-name=\"{nom}\">{corps}</text:span>"
                );
            }
            None => out.push_str(&corps),
        }
    }
    out
}

/// Un tableau en paragraphes à tabulations (une zone de texte d'Impress ne peut pas
/// contenir de vrai tableau). Les colonnes sont placées d'après leur contenu, avec
/// les métriques du PDF ; un tableau trop large est écrit plus petit.
fn table_xml(t: &Table, hab: &Habillage, styles: &mut Styles) -> String {
    let cols = t.columns().max(1);
    let police = Police {
        serif: hab.serif,
        gras: hab.gras,
        italique: hab.italique,
    };
    let largeur = hab.largeur.max(1.0);
    let mut taille = hab.taille * 0.85;
    let (_, naturelle) = colonnes(t, police, taille, 0.75 * taille, largeur);
    if naturelle > largeur {
        taille *= (largeur / naturelle).max(0.6);
    }
    let px = 0.75 * taille;
    let (largeurs, _) = colonnes(t, police, taille, px, largeur);
    // Le texte de la première colonne commence au bord : tout est décalé de `px`.
    let mut debuts = Vec::with_capacity(cols);
    let mut x = -px;
    for w in &largeurs {
        debuts.push(x);
        x += w;
    }
    let taquet = |c: usize| match t.align(c) {
        Align::Center => Some((debuts[c] + largeurs[c] / 2.0, "center")),
        Align::Right => Some((debuts[c] + largeurs[c] - px, "right")),
        _ if c > 0 => Some((debuts[c] + px, "left")),
        _ => None,
    };
    let taquets: Vec<(f32, &'static str)> = (0..cols).filter_map(taquet).collect();
    let premiere_decalee = taquet(0).is_some();
    let mut s = String::new();
    for (i, row) in t.rows.iter().enumerate() {
        let entete = i == 0 && t.header;
        let ps = PStyle {
            taille,
            gras: entete,
            couleur: if entete { hab.pal.title } else { hab.pal.text },
            align: Align::Left,
            marge_bas: if entete { 0.35 * taille } else { 0.25 * taille },
            interligne: interligne(1.25),
            taquets: taquets.clone(),
            ..hab.pstyle()
        };
        let nom = styles.paragraphe(&ps);
        let cellules: Vec<String> = (0..cols)
            .map(|c| {
                row.get(c)
                    .map(|r| {
                        runs_xml(r, styles)
                            .replace("<text:tab/>", " ")
                            .replace("<text:line-break/>", " ")
                    })
                    .unwrap_or_default()
            })
            .collect();
        let _ = write!(
            s,
            "<text:p text:style-name=\"{nom}\">{}{}</text:p>",
            if premiere_decalee { "<text:tab/>" } else { "" },
            cellules.join("<text:tab/>")
        );
    }
    s
}

/// Le contenu mis en forme d'une zone, en paragraphes ODF.
fn paras_xml(paras: &[Para], hab: &Habillage, styles: &mut Styles) -> String {
    let mut s = String::new();
    let base = hab.pstyle();
    let taille = hab.taille;
    for p in paras {
        match p {
            Para::Text { runs, .. } if runs.iter().all(|r| r.plain().is_empty()) => {
                let ps = if hab.titre {
                    base.clone()
                } else {
                    PStyle {
                        taille: 0.45 * taille / 1.117,
                        marge_bas: 0.0,
                        interligne: 100,
                        ..base.clone()
                    }
                };
                let _ = write!(
                    s,
                    "<text:p text:style-name=\"{}\"/>",
                    styles.paragraphe(&ps)
                );
            }
            Para::Text { align, runs } => {
                let ps = PStyle {
                    align: if *align == Align::Left {
                        hab.align
                    } else {
                        *align
                    },
                    ..base.clone()
                };
                let nom = styles.paragraphe(&ps);
                let _ = write!(
                    s,
                    "<text:p text:style-name=\"{nom}\">{}</text:p>",
                    runs_xml(runs, styles)
                );
            }
            Para::Heading { level, runs } => {
                let f = [1.3, 1.15, 1.05][((*level).clamp(1, 3) - 1) as usize];
                let ps = PStyle {
                    taille: taille * f,
                    gras: true,
                    serif: hab.pal.serif_titles,
                    couleur: hab.pal.title,
                    interligne: interligne(1.15),
                    approche: -0.01 * taille * f,
                    align: Align::Left,
                    ..base.clone()
                };
                let nom = styles.paragraphe(&ps);
                let _ = write!(
                    s,
                    "<text:p text:style-name=\"{nom}\">{}</text:p>",
                    runs_xml(runs, styles)
                );
            }
            Para::Item {
                number,
                depth,
                runs,
            } => {
                let liste = styles.liste(number.is_some(), hab.pal.accent, taille, hab.serif);
                let ps = PStyle {
                    align: Align::Left,
                    ..base.clone()
                };
                let nom = styles.paragraphe(&ps);
                let profondeur = (*depth).min(5) as usize;
                let _ = write!(s, "<text:list text:style-name=\"{liste}\">");
                for _ in 0..profondeur {
                    s.push_str("<text:list-item><text:list>");
                }
                match number {
                    Some(n) => {
                        let _ = write!(s, "<text:list-item text:start-value=\"{n}\">");
                    }
                    None => s.push_str("<text:list-item>"),
                }
                let _ = write!(
                    s,
                    "<text:p text:style-name=\"{nom}\">{}</text:p></text:list-item>",
                    runs_xml(runs, styles)
                );
                for _ in 0..profondeur {
                    s.push_str("</text:list></text:list-item>");
                }
                s.push_str("</text:list>");
            }
            Para::Quote { runs } => {
                let ps = PStyle {
                    italique: true,
                    couleur: hab.pal.title,
                    marge_gauche: if hab.citation { 0.0 } else { 0.94 * taille },
                    align: Align::Left,
                    ..base.clone()
                };
                let nom = styles.paragraphe(&ps);
                let _ = write!(
                    s,
                    "<text:p text:style-name=\"{nom}\">{}</text:p>",
                    runs_xml(runs, styles)
                );
            }
            Para::Math { .. } => {
                let ps = PStyle {
                    italique: true,
                    serif: true,
                    align: Align::Center,
                    ..base.clone()
                };
                let nom = styles.paragraphe(&ps);
                let _ = write!(
                    s,
                    "<text:p text:style-name=\"{nom}\">{}</text:p>",
                    odf_text(&p.plain())
                );
            }
            Para::Table(t) => s.push_str(&table_xml(t, hab, styles)),
            Para::Rule => {
                let ps = PStyle {
                    couleur: hab.pal.rule(),
                    align: Align::Left,
                    ..base.clone()
                };
                let nom = styles.paragraphe(&ps);
                let _ = write!(s, "<text:p text:style-name=\"{nom}\">———</text:p>");
            }
            Para::PageBreak => {}
        }
    }
    if s.is_empty() {
        // Un cadre de texte contient toujours au moins un paragraphe.
        let _ = write!(
            s,
            "<text:p text:style-name=\"{}\"/>",
            styles.paragraphe(&base)
        );
    }
    s
}

// ---------------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------------

fn geometrie(f: Frame, pw: f32, ph: f32) -> String {
    let (x, y, w, h) = f.scaled(pw, ph);
    format!(
        "svg:x=\"{}\" svg:y=\"{}\" svg:width=\"{}\" svg:height=\"{}\"",
        cm(x),
        cm(y),
        cm(w),
        cm(h)
    )
}

/// Géométrie d'un objet tourné de `rotation` degrés (sens des aiguilles d'une
/// montre) autour de son centre : taille, puis `draw:transform` (ODF : rotation
/// autour de l'origine, comptée dans le sens inverse, puis translation).
fn geometrie_tournee(f: Frame, rotation: f32, pw: f32, ph: f32) -> String {
    if rotation == 0.0 {
        return geometrie(f, pw, ph);
    }
    let (x, y, w, h) = f.scaled(pw, ph);
    let (s, c) = rotation.to_radians().sin_cos();
    // Le centre du cadre reste en place : T = centre − R·(w/2, h/2).
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let tx = cx - (c * w / 2.0 - s * h / 2.0);
    let ty = cy - (s * w / 2.0 + c * h / 2.0);
    let r = -rotation.to_radians();
    format!(
        "svg:width=\"{}\" svg:height=\"{}\" draw:transform=\"rotate ({r:.6}) translate ({} {})\"",
        cm(w),
        cm(h),
        cm(tx),
        cm(ty)
    )
}

#[allow(clippy::too_many_arguments)]
fn cadre(
    out: &mut String,
    styles: &mut Styles,
    classe: Option<&str>,
    f: Frame,
    ancre: Anchor,
    contenu: &str,
    pw: f32,
    ph: f32,
) {
    match classe {
        Some(c) => {
            let st = styles.cadre("presentation", ancre);
            let _ = write!(
                out,
                "<draw:frame presentation:style-name=\"{st}\" draw:layer=\"layout\" {} presentation:class=\"{c}\" presentation:user-transformed=\"true\"><draw:text-box>{contenu}</draw:text-box></draw:frame>",
                geometrie(f, pw, ph)
            );
        }
        None => {
            let st = styles.cadre("graphic", ancre);
            let _ = write!(
                out,
                "<draw:frame draw:style-name=\"{st}\" draw:layer=\"layout\" {}><draw:text-box>{contenu}</draw:text-box></draw:frame>",
                geometrie(f, pw, ph)
            );
        }
    }
}

/// Un objet de la diapositive (image ou forme) en ODF.
#[allow(clippy::too_many_arguments)]
fn objet_xml(
    out: &mut String,
    o: &Object,
    texte: &[Para],
    qr: Option<&varium_core::qr::QrCode>,
    pal: Palette,
    styles: &mut Styles,
    deck: &Deck,
    pw: f32,
    ph: f32,
) {
    let f = o.frame;
    match o.kind {
        ObjectKind::Image => {
            let Some(m) = deck.media.get(&o.media) else {
                return;
            };
            let st = styles.image();
            let _ = write!(
                out,
                "<draw:frame draw:style-name=\"{st}\" draw:layer=\"layout\" {}><draw:image xlink:href=\"Pictures/{}.{}\" xlink:type=\"simple\" xlink:show=\"embed\" xlink:actuate=\"onLoad\" draw:mime-type=\"{}\"><text:p/></draw:image></draw:frame>",
                geometrie_tournee(f, o.rotation, pw, ph),
                o.media,
                m.extension(),
                m.mime
            );
        }
        ObjectKind::Qr => {
            let Some(q) = qr else {
                return;
            };
            let blanc = Object {
                kind: ObjectKind::Rect,
                fill: Some([255, 255, 255]),
                stroke: None,
                ..o.clone()
            };
            let st = styles.forme(&blanc, pw);
            let _ = write!(
                out,
                "<draw:custom-shape draw:style-name=\"{st}\" draw:layer=\"layout\" {}><draw:enhanced-geometry svg:viewBox=\"0 0 21600 21600\" draw:type=\"rectangle\" draw:enhanced-path=\"M 0 0 L 21600 0 21600 21600 0 21600 0 0 Z N\"/></draw:custom-shape>",
                geometrie_tournee(f, o.rotation, pw, ph)
            );
            let encre = Object {
                kind: ObjectKind::Rect,
                fill: Some(o.qr_ink()),
                stroke: None,
                ..o.clone()
            };
            let st = styles.forme(&encre, pw);
            let marge = Object::QR_MARGIN;
            let n = q.size() + 2 * marge;
            let mut d = String::new();
            for (x, y, l) in q.segments() {
                let _ = write!(d, "M{} {}h{l}v1h-{l}z", x + marge, y + marge);
            }
            let _ = write!(
                out,
                "<draw:path draw:style-name=\"{st}\" draw:layer=\"layout\" {} svg:viewBox=\"0 0 {n} {n}\" svg:d=\"{d}\"><text:p/></draw:path>",
                geometrie_tournee(f, o.rotation, pw, ph)
            );
        }
        ObjectKind::Line | ObjectKind::Arrow => {
            let st = styles.forme(o, pw);
            let ((x1, y1), (x2, y2)) = o.line_points();
            // Une ligne tournée : ses extrémités tournent autour du centre du cadre.
            let ratio = pw / ph;
            let (cx, cy) = o.frame.center();
            let tourne = |x: f32, y: f32| {
                let p = crate::objets::rotate_point((x * ratio, y), (cx * ratio, cy), o.rotation);
                (p.0 / ratio, p.1)
            };
            let ((x1, y1), (x2, y2)) = (tourne(x1, y1), tourne(x2, y2));
            let _ = write!(
                out,
                "<draw:line draw:style-name=\"{st}\" draw:layer=\"layout\" svg:x1=\"{}\" svg:y1=\"{}\" svg:x2=\"{}\" svg:y2=\"{}\"><text:p/></draw:line>",
                cm(x1 * pw),
                cm(y1 * ph),
                cm(x2 * pw),
                cm(y2 * ph)
            );
        }
        ObjectKind::Rect | ObjectKind::RoundRect | ObjectKind::Ellipse => {
            let st = styles.forme(o, pw);
            let hab = Habillage {
                taille: o.text_size * ph,
                couleur: o.text_ink(&pal),
                serif: false,
                gras: false,
                italique: false,
                majuscules: false,
                approche: 0.0,
                interligne: 1.2,
                align: Align::Center,
                titre: true,
                citation: false,
                pal,
                largeur: f.w * pw,
            };
            let contenu = if texte.is_empty() {
                String::new()
            } else {
                paras_xml(texte, &hab, styles)
            };
            // Formes d'Impress (`draw:custom-shape`) : seules à renvoyer le texte à
            // la ligne dans LibreOffice.
            let geometrie_forme = match o.kind {
                ObjectKind::Ellipse => {
                    "draw:type=\"ellipse\" draw:enhanced-path=\"U 10800 10800 10800 10800 0 360 Z N\"".to_string()
                }
                ObjectKind::RoundRect => format!(
                    "draw:type=\"round-rectangle\" draw:modifiers=\"{}\" draw:enhanced-path=\"M ?f7 0 X 0 ?f8 L 0 ?f9 Y ?f7 21600 L ?f10 21600 X 21600 ?f9 L 21600 ?f8 Y ?f10 0 Z N\"",
                    (21600.0 * Object::CORNER).round()
                ),
                _ => "draw:type=\"rectangle\" draw:enhanced-path=\"M 0 0 L 21600 0 21600 21600 0 21600 0 0 Z N\"".to_string(),
            };
            let formules = if o.kind == ObjectKind::RoundRect {
                FORMULES_ARRONDI
            } else {
                ""
            };
            let _ = write!(
                out,
                "<draw:custom-shape draw:style-name=\"{st}\" draw:layer=\"layout\" {}>{contenu}<draw:enhanced-geometry svg:viewBox=\"0 0 21600 21600\" {geometrie_forme}>{formules}</draw:enhanced-geometry></draw:custom-shape>",
                geometrie_tournee(f, o.rotation, pw, ph)
            );
        }
    }
}

/// Formules du rectangle arrondi d'Impress (rayon : premier modificateur).
const FORMULES_ARRONDI: &str = "<draw:equation draw:name=\"f0\" draw:formula=\"45\"/><draw:equation draw:name=\"f1\" draw:formula=\"$0 *sin(?f0 *(pi/180))\"/><draw:equation draw:name=\"f2\" draw:formula=\"?f1 *3163/7636\"/><draw:equation draw:name=\"f3\" draw:formula=\"left+?f2 \"/><draw:equation draw:name=\"f4\" draw:formula=\"top+?f2 \"/><draw:equation draw:name=\"f5\" draw:formula=\"right-?f2 \"/><draw:equation draw:name=\"f6\" draw:formula=\"bottom-?f2 \"/><draw:equation draw:name=\"f7\" draw:formula=\"left+$0 \"/><draw:equation draw:name=\"f8\" draw:formula=\"top+$0 \"/><draw:equation draw:name=\"f9\" draw:formula=\"bottom-$0 \"/><draw:equation draw:name=\"f10\" draw:formula=\"right-$0 \"/><draw:handle draw:handle-position=\"$0 top\" draw:handle-switched=\"true\" draw:handle-range-x-minimum=\"0\" draw:handle-range-x-maximum=\"10800\"/>";

#[allow(clippy::too_many_arguments)]
fn page_xml(
    i: usize,
    r: &Rendu,
    s: &Slide,
    styles: &mut Styles,
    deck: &Deck,
    pw: f32,
    ph: f32,
) -> String {
    let pal = r.palette;
    let b = &r.boxes;
    let mut out = String::new();
    let dp = styles.page(pal.background, s.transition, deck.auto_advance_after(i));
    let _ = write!(
        out,
        "<draw:page draw:name=\"page{}\" draw:style-name=\"{dp}\" draw:master-page-name=\"Varium\">",
        i + 1
    );
    let align = if b.centered {
        Align::Center
    } else {
        Align::Left
    };
    let grand = matches!(s.layout, Layout::Title | Layout::Section);
    let modele = Habillage {
        taille: 12.0,
        couleur: pal.text,
        serif: false,
        gras: false,
        italique: false,
        majuscules: false,
        approche: 0.0,
        interligne: BODY_LINE_HEIGHT,
        align: Align::Left,
        titre: false,
        citation: false,
        pal,
        largeur: 0.0,
    };
    if let Some(f) = b.accent {
        let st = styles.filet(pal.accent);
        let (_, _, w, h) = f.scaled(pw, ph);
        let _ = write!(
            out,
            "<draw:rect draw:style-name=\"{st}\" draw:layer=\"layout\" {} draw:corner-radius=\"{}\"/>",
            geometrie(f, pw, ph),
            cm(w.min(h) / 2.0)
        );
    }
    if let Some(f) = b.title {
        let hab = Habillage {
            taille: title_size(s.layout) * ph,
            couleur: pal.title,
            serif: pal.serif_titles,
            gras: true,
            approche: if grand { -0.03 } else { -0.02 },
            interligne: if grand { 1.05 } else { TITLE_LINE_HEIGHT },
            align,
            titre: true,
            largeur: f.w * pw,
            ..modele
        };
        let contenu = paras_xml(&r.title, &hab, styles);
        cadre(
            &mut out,
            styles,
            Some("title"),
            f,
            b.title_anchor,
            &contenu,
            pw,
            ph,
        );
    }
    if let Some(f) = b.subtitle {
        let citation = s.layout == Layout::Quote;
        let taille = subtitle_size(s.layout) * ph * if citation { 0.8 } else { 1.0 };
        let hab = Habillage {
            taille,
            couleur: pal.muted,
            serif: pal.serif_titles && !citation,
            majuscules: citation,
            approche: if citation { 0.12 } else { 0.0 },
            interligne: 1.3,
            align,
            titre: true,
            largeur: f.w * pw,
            ..modele
        };
        let contenu = paras_xml(&r.subtitle, &hab, styles);
        cadre(
            &mut out,
            styles,
            Some("subtitle"),
            f,
            b.subtitle_anchor,
            &contenu,
            pw,
            ph,
        );
    }
    let citation = s.layout == Layout::Quote;
    for (f, paras) in [(b.body, &r.body), (b.body2, &r.body2)] {
        if let Some(f) = f {
            let hab = Habillage {
                taille: body_size(s.layout) * ph,
                couleur: if citation { pal.title } else { pal.text },
                serif: citation,
                italique: citation,
                interligne: if citation { 1.22 } else { BODY_LINE_HEIGHT },
                citation,
                largeur: f.w * pw,
                ..modele
            };
            let contenu = paras_xml(paras, &hab, styles);
            cadre(&mut out, styles, None, f, b.body_anchor, &contenu, pw, ph);
        }
    }
    for ((o, texte), qr) in s.objects.iter().zip(&r.objets).zip(&r.qrs) {
        objet_xml(&mut out, o, texte, qr.as_ref(), pal, styles, deck, pw, ph);
    }
    let pied = Habillage {
        taille: FOOTER_SIZE * ph,
        couleur: pal.muted,
        approche: 0.03,
        interligne: 1.2,
        titre: true,
        ..modele
    };
    if !r.footer.is_empty() {
        let nom = styles.paragraphe(&pied.pstyle());
        let contenu = format!(
            "<text:p text:style-name=\"{nom}\">{}</text:p>",
            runs_xml(&r.footer, styles)
        );
        cadre(
            &mut out,
            styles,
            None,
            footer_frame(),
            Anchor::Middle,
            &contenu,
            pw,
            ph,
        );
    }
    if let Some(n) = r.number {
        let ps = PStyle {
            align: Align::Right,
            gras: true,
            ..pied.pstyle()
        };
        let nom = styles.paragraphe(&ps);
        let contenu = format!(
            "<text:p text:style-name=\"{nom}\"><text:page-number text:select-page=\"current\">{n}</text:page-number></text:p>"
        );
        cadre(
            &mut out,
            styles,
            None,
            number_frame(),
            Anchor::Middle,
            &contenu,
            pw,
            ph,
        );
    }
    // Notes de l'orateur : texte noir sur la page de notes (A4 portrait).
    let notes_pal = Palette {
        background: [255, 255, 255],
        text: [20, 20, 20],
        title: [0, 0, 0],
        accent: [60, 60, 60],
        muted: [110, 110, 110],
        serif_titles: false,
    };
    let hab = Habillage {
        taille: 12.0,
        couleur: notes_pal.text,
        interligne: 1.35,
        pal: notes_pal,
        largeur: 476.0,
        ..modele
    };
    let notes = paras_xml(&r.notes, &hab, styles);
    let cadre_notes = styles.cadre("presentation", Anchor::Top);
    let rapport = ph / pw;
    let _ = write!(
        out,
        "<presentation:notes draw:style-name=\"{dp}\"><draw:page-thumbnail draw:layer=\"layout\" svg:width=\"{}\" svg:height=\"{}\" svg:x=\"{}\" svg:y=\"{}\" draw:page-number=\"{}\" presentation:class=\"page\"/><draw:frame presentation:style-name=\"{cadre_notes}\" draw:layer=\"layout\" svg:width=\"{}\" svg:height=\"{}\" svg:x=\"{}\" svg:y=\"{}\" presentation:class=\"notes\"><draw:text-box>{notes}</draw:text-box></draw:frame></presentation:notes>",
        cm(476.0),
        cm(476.0 * rapport),
        cm(59.5),
        cm(60.0),
        i + 1,
        cm(476.0),
        cm(400.0),
        cm(59.5),
        cm(380.0)
    );
    out.push_str("</draw:page>");
    out
}

pub(crate) fn odp(deck: &Deck) -> Vec<u8> {
    let pw = LARGEUR;
    let ph = pw / deck.aspect.ratio();
    let rendus = deck.rendus();
    let slides = deck.exported();
    let mut styles = Styles::default();
    let pages: String = rendus
        .iter()
        .zip(slides.iter())
        .enumerate()
        .map(|(i, (r, s))| page_xml(i, r, s, &mut styles, deck, pw, ph))
        .collect();
    let content = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document-content {NS}>{POLICES}<office:automatic-styles>{}</office:automatic-styles><office:body><office:presentation>{pages}</office:presentation></office:body></office:document-content>",
        styles.xml
    );
    let fond = deck.theme.palette().background;
    let styles_xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document-styles {NS}>{POLICES}<office:styles>{}<draw:marker draw:name=\"Varium_20_fleche\" draw:display-name=\"Varium flèche\" svg:viewBox=\"0 0 20 20\" svg:d=\"M10 0l-10 20h20z\"/><style:default-style style:family=\"graphic\"><style:paragraph-properties fo:text-align=\"start\"/><style:text-properties style:font-name=\"Liberation Sans\" fo:font-size=\"18pt\" fo:language=\"fr\" fo:country=\"FR\"/></style:default-style></office:styles><office:automatic-styles><style:page-layout style:name=\"PM1\"><style:page-layout-properties fo:margin-top=\"0cm\" fo:margin-bottom=\"0cm\" fo:margin-left=\"0cm\" fo:margin-right=\"0cm\" fo:page-width=\"{}\" fo:page-height=\"{}\" style:print-orientation=\"landscape\"/></style:page-layout><style:page-layout style:name=\"PM2\"><style:page-layout-properties fo:margin-top=\"0cm\" fo:margin-bottom=\"0cm\" fo:margin-left=\"0cm\" fo:margin-right=\"0cm\" fo:page-width=\"21cm\" fo:page-height=\"29.7cm\" style:print-orientation=\"portrait\"/></style:page-layout><style:style style:name=\"Mdp1\" style:family=\"drawing-page\"><style:drawing-page-properties draw:background-size=\"full\" draw:fill=\"solid\" draw:fill-color=\"{}\"/></style:style><style:style style:name=\"Mdp2\" style:family=\"drawing-page\"><style:drawing-page-properties draw:fill=\"none\"/></style:style></office:automatic-styles><office:master-styles><draw:layer-set><draw:layer draw:name=\"layout\"/><draw:layer draw:name=\"background\"/><draw:layer draw:name=\"backgroundobjects\"/><draw:layer draw:name=\"controls\"/><draw:layer draw:name=\"measurelines\"/></draw:layer-set><style:master-page style:name=\"Varium\" style:page-layout-name=\"PM1\" draw:style-name=\"Mdp1\"><presentation:notes style:page-layout-name=\"PM2\" draw:style-name=\"Mdp2\"/></style:master-page></office:master-styles></office:document-styles>",
        styles.degrades,
        cm(pw),
        cm(ph),
        hex_color(fond)
    );
    let titre = match deck.title.trim() {
        "" => "Présentation",
        t => t,
    };
    let meta = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document-meta {NS}><office:meta><meta:generator>Varium</meta:generator><dc:title>{}</dc:title><dc:language>fr-FR</dc:language></office:meta></office:document-meta>",
        echapper(titre)
    );
    // Les images employées, une seule fois chacune, dans `Pictures/`.
    let images: Vec<(String, &crate::Media)> = deck
        .used_media()
        .into_iter()
        .filter_map(|id| {
            let m = deck.media.get(id)?;
            Some((format!("Pictures/{id}.{}", m.extension()), m))
        })
        .collect();
    let mut manifest = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<manifest:manifest xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\" manifest:version=\"1.3\"><manifest:file-entry manifest:full-path=\"/\" manifest:version=\"1.3\" manifest:media-type=\"application/vnd.oasis.opendocument.presentation\"/><manifest:file-entry manifest:full-path=\"content.xml\" manifest:media-type=\"text/xml\"/><manifest:file-entry manifest:full-path=\"styles.xml\" manifest:media-type=\"text/xml\"/><manifest:file-entry manifest:full-path=\"meta.xml\" manifest:media-type=\"text/xml\"/>",
    );
    for (chemin, m) in &images {
        let _ = write!(
            manifest,
            "<manifest:file-entry manifest:full-path=\"{chemin}\" manifest:media-type=\"{}\"/>",
            m.mime
        );
    }
    manifest.push_str("</manifest:manifest>");

    let mut zip = Zip::new();
    // `mimetype` doit être le premier fichier, stocké sans compression.
    zip.add(
        "mimetype",
        b"application/vnd.oasis.opendocument.presentation",
    );
    zip.add("content.xml", content.as_bytes());
    zip.add("styles.xml", styles_xml.as_bytes());
    zip.add("meta.xml", meta.as_bytes());
    for (chemin, m) in &images {
        zip.add(chemin, &m.data);
    }
    zip.add("META-INF/manifest.xml", manifest.as_bytes());
    zip.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn espaces_odf() {
        assert_eq!(odf_text("a  b"), "a <text:s text:c=\"1\"/>b");
        assert_eq!(
            odf_text(" a\nb"),
            "<text:s text:c=\"1\"/>a<text:line-break/>b"
        );
        assert_eq!(odf_text("<&>"), "&lt;&amp;&gt;");
    }

    #[test]
    fn unites() {
        assert_eq!(cm(720.0), "25.4cm");
        assert_eq!(cm(0.0), "0cm");
        assert_eq!(pt(18.25), "18.2pt");
        assert_eq!(interligne(1.117), 100);
    }

    #[test]
    fn styles_sans_doublon() {
        let mut s = Styles::default();
        let a = s.caracteres(
            Style {
                bold: true,
                ..Style::default()
            },
            false,
        );
        let b = s.caracteres(
            Style {
                bold: true,
                ..Style::default()
            },
            false,
        );
        assert_eq!(a, b);
        assert_eq!(s.caracteres(Style::default(), false), None);
    }
}
