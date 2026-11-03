//! Import PowerPoint (`.pptx`, Office Open XML).
//!
//! Lit l'ordre des diapositives (`ppt/presentation.xml` et ses relations), le
//! format (`p:sldSz`), puis pour chaque diapositive : fond uni (de la diapositive,
//! de sa disposition ou de son masque), zones réservées (titre, sous-titre,
//! contenu, pied de page, numéro), zones de texte libres, images, formes simples
//! (rectangle, rectangle arrondi, ellipse, ligne, flèche) et notes de l'orateur.
//! Les positions sont en EMU (914 400 par pouce).

use std::collections::HashMap;

use varium_core::zip::ZipReader;

use crate::import::{Ligne, Marques, Medias, Page, Paragraphe, Role, Zone, construire};
use crate::objets::{Object, ObjectKind};
use crate::xml::{Enfant, Noeud, lire};
use crate::{Aspect, Deck, Frame, Transition};

pub(crate) fn pptx(bytes: &[u8]) -> Result<Deck, String> {
    let z = ZipReader::new(bytes)?;
    depuis_zip(&z)
}

/// Dossier d'une partie (`ppt/slides/slide1.xml` → `ppt/slides`).
fn dossier(partie: &str) -> &str {
    partie.rsplit_once('/').map_or("", |(d, _)| d)
}

/// Résout une cible de relation relative au dossier `base`.
fn resoudre(base: &str, cible: &str) -> String {
    if let Some(abs) = cible.strip_prefix('/') {
        return abs.to_string();
    }
    let mut parts: Vec<&str> = base.split('/').filter(|s| !s.is_empty()).collect();
    for seg in cible.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// Relations d'une partie : identifiant → (type, chemin résolu).
fn relations(z: &ZipReader, partie: &str) -> HashMap<String, (String, String)> {
    let d = dossier(partie);
    let nom = partie.rsplit('/').next().unwrap_or(partie);
    let chemin = if d.is_empty() {
        format!("_rels/{nom}.rels")
    } else {
        format!("{d}/_rels/{nom}.rels")
    };
    let Some(r) = z.text(&chemin).and_then(|t| lire(&t).ok()) else {
        return HashMap::new();
    };
    r.noeuds()
        .filter(|n| n.est("Relationship"))
        .filter(|n| n.attr("TargetMode") != Some("External"))
        .filter_map(|n| {
            Some((
                n.attr("Id")?.to_string(),
                (
                    n.attr("Type").unwrap_or("").to_string(),
                    resoudre(d, n.attr("Target")?),
                ),
            ))
        })
        .collect()
}

fn type_rel(t: &str, fin: &str) -> bool {
    t.rsplit('/').next() == Some(fin)
}

/// Couleur `a:srgbClr` (ou `a:sysClr` avec `lastClr`) d'un remplissage uni.
fn couleur_unie(n: &Noeud) -> Option<[u8; 3]> {
    if let Some(c) = n.enfant("srgbClr") {
        return crate::texte::parse_color(c.attr("val")?);
    }
    if let Some(c) = n.enfant("sysClr") {
        return crate::texte::parse_color(c.attr("lastClr")?);
    }
    if let Some(c) = n.enfant("prstClr") {
        return match c.attr("val")? {
            "black" => Some([0, 0, 0]),
            "white" => Some([255, 255, 255]),
            "red" => Some([255, 0, 0]),
            "blue" => Some([0, 0, 255]),
            "green" => Some([0, 128, 0]),
            _ => None,
        };
    }
    None
}

/// Fond uni d'une partie (diapositive, disposition ou masque).
fn fond(racine: &Noeud) -> Option<[u8; 3]> {
    let bg = racine.chemin(&["cSld", "bg"])?;
    bg.chemin(&["bgPr", "solidFill"]).and_then(couleur_unie)
}

pub(crate) fn depuis_zip(z: &ZipReader) -> Result<Deck, String> {
    // Partie principale : d'après les relations du paquet, sinon l'emplacement usuel.
    let principale = relations(z, "")
        .into_values()
        .find(|(t, _)| type_rel(t, "officeDocument"))
        .map(|(_, c)| c)
        .unwrap_or_else(|| "ppt/presentation.xml".to_string());
    let texte = z
        .text(&principale)
        .ok_or("Présentation PowerPoint endommagée : presentation.xml est absent.")?;
    let pres = lire(&texte).map_err(|e| format!("Présentation PowerPoint illisible : {e}"))?;
    let rels = relations(z, &principale);

    let emu = |n: Option<&Noeud>, a: &str| {
        n.and_then(|n| n.attr(a))
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| *v > 0.0 && v.is_finite())
    };
    let taille = pres.enfant("sldSz");
    let (pw, ph) = match (emu(taille, "cx"), emu(taille, "cy")) {
        (Some(w), Some(h)) => (w as f32, h as f32),
        _ => (12_192_000.0, 6_858_000.0),
    };
    let aspect = if (pw / ph - 4.0 / 3.0).abs() < (pw / ph - 16.0 / 9.0).abs() {
        Aspect::Standard
    } else {
        Aspect::Large
    };

    let mut chemins: Vec<String> = pres
        .enfant("sldIdLst")
        .map(|l| {
            l.noeuds()
                .filter_map(|s| s.attr("r:id"))
                .filter_map(|id| rels.get(id))
                .map(|(_, c)| c.clone())
                .collect()
        })
        .unwrap_or_default();
    if chemins.is_empty() {
        // Liste absente : les diapositives du dossier, dans l'ordre de leur numéro.
        let mut v: Vec<(u32, String)> = z
            .names()
            .filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))
            .map(|n| {
                let k = n
                    .trim_start_matches("ppt/slides/slide")
                    .trim_end_matches(".xml")
                    .parse()
                    .unwrap_or(u32::MAX);
                (k, n.to_string())
            })
            .collect();
        v.sort();
        chemins = v.into_iter().map(|(_, n)| n).collect();
    }

    let mut medias = Medias::new();
    let mut pages = Vec::new();
    for chemin in chemins.iter().take(2000) {
        let Some(xml) = z.text(chemin) else {
            continue;
        };
        let Ok(racine) = lire(&xml) else {
            continue;
        };
        let rels = relations(z, chemin);
        // Fond : diapositive, sinon sa disposition, sinon le masque.
        let fond_page = fond(&racine).or_else(|| {
            let (_, dispo) = rels.values().find(|(t, _)| type_rel(t, "slideLayout"))?;
            let dx = lire(&z.text(dispo)?).ok()?;
            fond(&dx).or_else(|| {
                let r2 = relations(z, dispo);
                let (_, maitre) = r2.values().find(|(t, _)| type_rel(t, "slideMaster"))?;
                fond(&lire(&z.text(maitre)?).ok()?)
            })
        });
        let mut page = Page {
            fond: fond_page,
            transition: transition(&racine),
            ..Page::default()
        };
        if let Some(arbre) = racine.chemin(&["cSld", "spTree"]) {
            let ctx = Contexte {
                z,
                rels: &rels,
                pw,
                ph,
            };
            ctx.formes(arbre, &mut page, &mut medias, 0);
        }
        // Notes de l'orateur.
        if let Some((_, notes)) = rels.values().find(|(t, _)| type_rel(t, "notesSlide"))
            && let Some(nx) = z.text(notes).and_then(|t| lire(&t).ok())
        {
            for sp in nx.descendants("sp") {
                let ph = sp.chemin(&["nvSpPr", "nvPr", "ph"]);
                if ph.and_then(|p| p.attr("type")) == Some("body")
                    && let Some(tx) = sp.enfant("txBody")
                {
                    page.notes.extend(paragraphes(tx, false, false));
                }
            }
        }
        pages.push(page);
    }
    let titre = z
        .text("docProps/core.xml")
        .and_then(|t| lire(&t).ok())
        .and_then(|c| c.enfant("title").map(Noeud::texte))
        .unwrap_or_default();
    Ok(construire(
        titre.trim().to_string(),
        aspect,
        pages,
        medias.deck,
    ))
}

struct Contexte<'a> {
    z: &'a ZipReader,
    rels: &'a HashMap<String, (String, String)>,
    pw: f32,
    ph: f32,
}

impl Contexte<'_> {
    /// Position (`a:xfrm`) d'une forme, en fractions de la diapositive, et ses
    /// retournements.
    fn cadre(&self, sp_pr: Option<&Noeud>) -> Option<(Frame, bool, bool)> {
        let x = sp_pr?.enfant("xfrm")?;
        let n = |n: Option<&Noeud>, a: &str| {
            n.and_then(|n| n.attr(a))
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite())
                .map(|v| v as f32)
        };
        let off = x.enfant("off");
        let ext = x.enfant("ext");
        let f = Frame::new(
            n(off, "x")? / self.pw,
            n(off, "y")? / self.ph,
            n(ext, "cx")? / self.pw,
            n(ext, "cy")? / self.ph,
        );
        let vrai = |a: &str| matches!(x.attr(a), Some("1" | "true"));
        Some((f, vrai("flipH"), vrai("flipV")))
    }

    /// Remplissage et trait d'une forme (`p:spPr`).
    fn style(&self, sp_pr: Option<&Noeud>) -> (Option<[u8; 3]>, Option<[u8; 3]>, f32, bool) {
        let Some(sp) = sp_pr else {
            return (None, None, 1.0, false);
        };
        let fill = sp.enfant("solidFill").and_then(couleur_unie);
        let ln = sp.enfant("ln");
        let (stroke, w) = match ln {
            Some(l) if l.enfant("noFill").is_some() => (None, 0.0),
            Some(l) => (
                l.enfant("solidFill").and_then(couleur_unie),
                l.attr("w")
                    .and_then(|w| w.parse::<f32>().ok())
                    .filter(|w| w.is_finite())
                    .map(|w| (w / 12_700.0 * 720.0 / (self.pw / 12_700.0)).max(0.5))
                    .unwrap_or(1.0),
            ),
            None => (None, 1.0),
        };
        let fleche = ln.is_some_and(|l| {
            ["tailEnd", "headEnd"].iter().any(|e| {
                l.enfant(e)
                    .and_then(|t| t.attr("type"))
                    .is_some_and(|t| t != "none")
            })
        });
        (fill, stroke, w, fleche)
    }

    fn formes(&self, parent: &Noeud, page: &mut Page, medias: &mut Medias, profondeur: usize) {
        if profondeur > 16 {
            return;
        }
        for n in parent.noeuds() {
            match n.local() {
                "sp" => self.forme(n, page),
                "cxnSp" => {
                    let sp_pr = n.enfant("spPr");
                    let Some((f, fh, fv)) = self.cadre(sp_pr) else {
                        continue;
                    };
                    let (_, stroke, w, fleche) = self.style(sp_pr);
                    page.objets.push(Object {
                        kind: if fleche {
                            ObjectKind::Arrow
                        } else {
                            ObjectKind::Line
                        },
                        frame: f,
                        flip_h: fh,
                        flip_v: fv,
                        stroke: stroke.or(Some([0, 0, 0])),
                        stroke_width: w,
                        ..Object::default()
                    });
                }
                "pic" => {
                    let Some((f, _, _)) = self.cadre(n.enfant("spPr")) else {
                        continue;
                    };
                    let Some(id) = n
                        .descendant("blip")
                        .and_then(|b| b.attr("r:embed"))
                        .and_then(|r| self.rels.get(r))
                        .and_then(|(_, c)| medias.ajouter(c, self.z.get(c)))
                    else {
                        continue;
                    };
                    page.objets.push(Object {
                        kind: ObjectKind::Image,
                        frame: f,
                        media: id,
                        ..Object::default()
                    });
                }
                "grpSp" => self.formes(n, page, medias, profondeur + 1),
                "AlternateContent" => {
                    // Variante compatible : `mc:Fallback` (ou le premier choix).
                    if let Some(c) = n.enfant("Fallback").or_else(|| n.enfant("Choice")) {
                        self.formes(c, page, medias, profondeur + 1);
                    }
                }
                _ => {}
            }
        }
    }

    fn forme(&self, n: &Noeud, page: &mut Page) {
        let sp_pr = n.enfant("spPr");
        let cadre = self.cadre(sp_pr);
        let ph = n.chemin(&["nvSpPr", "nvPr", "ph"]);
        let texte = n.enfant("txBody");
        if let Some(ph) = ph {
            let role = match ph.attr("type") {
                Some("title") => Role::Titre,
                Some("ctrTitle") => Role::TitreCentre,
                Some("subTitle") => Role::SousTitre,
                Some("ftr") => Role::Pied,
                Some("sldNum") => Role::Numero,
                Some("dt" | "hdr" | "sldImg" | "chart" | "tbl" | "clipArt" | "dgm" | "media") => {
                    return;
                }
                _ => Role::Corps,
            };
            let puces = role == Role::Corps;
            let titre = matches!(role, Role::Titre | Role::TitreCentre);
            let lignes = texte
                .map(|t| paragraphes(t, puces, titre))
                .unwrap_or_default();
            page.zones.push(Zone {
                role,
                frame: cadre.map(|c| c.0),
                lignes,
            });
            return;
        }
        let geom = sp_pr
            .and_then(|s| s.enfant("prstGeom"))
            .and_then(|g| g.attr("prst"))
            .unwrap_or("rect");
        let (fill, stroke, w, fleche) = self.style(sp_pr);
        let zone_texte = n
            .chemin(&["nvSpPr", "cNvSpPr"])
            .and_then(|c| c.attr("txBox"))
            .is_some_and(|v| v == "1" || v == "true")
            || (geom == "rect" && fill.is_none() && stroke.is_none());
        let Some((f, fh, fv)) = cadre else {
            return;
        };
        if zone_texte {
            if let Some(t) = texte {
                let numero = t
                    .descendants("fld")
                    .iter()
                    .any(|f| f.attr("type") == Some("slidenum"));
                page.zones.push(Zone {
                    role: if numero { Role::Numero } else { Role::Corps },
                    frame: Some(f),
                    lignes: paragraphes(t, false, false),
                });
            }
            return;
        }
        let kind = match geom {
            "ellipse" | "circle" => ObjectKind::Ellipse,
            "roundRect" | "round1Rect" | "round2SameRect" => ObjectKind::RoundRect,
            "line" | "straightConnector1" => {
                if fleche {
                    ObjectKind::Arrow
                } else {
                    ObjectKind::Line
                }
            }
            _ => ObjectKind::Rect,
        };
        let txt = texte
            .map(|t| crate::import::titre_markup(&paragraphes(t, false, false)))
            .unwrap_or_default();
        // Taille du texte (centièmes de point) en fraction de la hauteur.
        let taille = texte
            .and_then(|t| {
                t.descendants("rPr")
                    .into_iter()
                    .find_map(|r| r.attr("sz")?.parse::<f32>().ok())
            })
            .map(|sz| sz / 100.0 / (self.ph / 12_700.0))
            .unwrap_or(0.04);
        page.objets.push(Object {
            kind,
            frame: f,
            flip_h: fh,
            flip_v: fv,
            fill: if kind.is_line() { None } else { fill },
            stroke: if kind.is_line() {
                stroke.or(Some([0, 0, 0]))
            } else {
                stroke
            },
            stroke_width: w,
            text: if kind.is_line() { String::new() } else { txt },
            text_size: taille,
            ..Object::default()
        });
    }
}

/// Transition d'une diapositive (`p:transition`, éventuellement dans un
/// `mc:AlternateContent`).
fn transition(racine: &Noeud) -> Option<Transition> {
    let t = racine.enfant("transition").or_else(|| {
        let alt = racine.enfant("AlternateContent")?;
        alt.enfant("Fallback")
            .or_else(|| alt.enfant("Choice"))?
            .enfant("transition")
    })?;
    Some(match t.noeuds().next().map(Noeud::local) {
        Some("push" | "wipe" | "cover" | "pull" | "split" | "strips") => Transition::Glissement,
        Some("zoom" | "circle" | "diamond" | "plus" | "wedge") => Transition::Zoom,
        _ => Transition::Fondu,
    })
}

/// Les paragraphes d'un `p:txBody`. `puces` : zone de contenu, dont les
/// paragraphes ont une puce sauf mention contraire (`a:buNone`) ; `titre` : le
/// gras de tout un paragraphe est retiré.
fn paragraphes(tx: &Noeud, puces: bool, titre: bool) -> Vec<Ligne> {
    let mut out = Vec::new();
    for p in tx.noeuds().filter(|n| n.est("p")).take(10_000) {
        let ppr = p.enfant("pPr");
        let niveau = ppr
            .and_then(|x| x.attr("lvl"))
            .and_then(|l| l.parse::<u8>().ok())
            .unwrap_or(0)
            .min(5);
        let sans = ppr.is_some_and(|x| x.enfant("buNone").is_some());
        let numero = ppr.is_some_and(|x| x.enfant("buAutoNum").is_some());
        let puce = ppr.is_some_and(|x| x.enfant("buChar").is_some() || numero);
        let mut para = Paragraphe::default();
        for c in p.noeuds() {
            match c.local() {
                "r" | "fld" => {
                    let rpr = c.enfant("rPr");
                    let vrai = |a: &str| {
                        rpr.and_then(|r| r.attr(a))
                            .is_some_and(|v| v == "1" || v == "true")
                    };
                    let m = Marques {
                        gras: vrai("b"),
                        italique: vrai("i"),
                        souligne: rpr.and_then(|r| r.attr("u")).is_some_and(|u| u != "none"),
                        barre: rpr
                            .and_then(|r| r.attr("strike"))
                            .is_some_and(|s| s != "noStrike"),
                    };
                    if let Some(t) = c.enfant("t") {
                        let s: String = t
                            .enfants
                            .iter()
                            .filter_map(|e| match e {
                                Enfant::Texte(s) => Some(s.as_str()),
                                Enfant::Noeud(_) => None,
                            })
                            .collect();
                        para.pousser(&s.replace(['\r', '\n'], " "), m);
                    }
                }
                "br" => para.pousser("\n", Marques::default()),
                _ => {}
            }
        }
        let vide = para.est_vide();
        let liste = ((puces && !sans) || puce) && !vide;
        out.push(Ligne {
            texte: if titre {
                para.markup_titre()
            } else {
                para.markup()
            },
            liste: liste.then_some((niveau, numero)),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chemins_relatifs() {
        assert_eq!(
            resoudre("ppt/slides", "../media/image1.png"),
            "ppt/media/image1.png"
        );
        assert_eq!(resoudre("", "ppt/presentation.xml"), "ppt/presentation.xml");
        assert_eq!(resoudre("ppt", "/ppt/slides/s.xml"), "ppt/slides/s.xml");
        assert_eq!(resoudre("a", "../../../x"), "x");
    }

    #[test]
    fn paragraphes_a_puces() {
        let x = lire(
            r#"<p:txBody xmlns:p="p" xmlns:a="a"><a:p><a:r><a:rPr b="1"/><a:t>Gras</a:t></a:r><a:r><a:t> et normal</a:t></a:r></a:p><a:p><a:pPr lvl="1"/><a:r><a:t>Sous</a:t></a:r></a:p><a:p><a:pPr><a:buNone/></a:pPr><a:r><a:t>Libre</a:t></a:r></a:p></p:txBody>"#,
        )
        .unwrap();
        let l = paragraphes(&x, true, false);
        assert_eq!(l[0].texte, "**Gras** et normal");
        assert_eq!(l[0].liste, Some((0, false)));
        assert_eq!(l[1].liste, Some((1, false)));
        assert_eq!(l[2].liste, None);
    }
}
