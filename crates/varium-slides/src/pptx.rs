//! Import PowerPoint (`.pptx`, Office Open XML).
//!
//! Lit l'ordre des diapositives (`ppt/presentation.xml` et ses relations), le
//! format (`p:sldSz`), puis pour chaque diapositive : fond (de la diapositive,
//! de sa disposition ou de son masque), zones réservées (titre, sous-titre,
//! contenu, pied de page, numéro), zones de texte libres, images, formes simples
//! (rectangle, rectangle arrondi, ellipse, ligne, flèche) avec leur rotation et
//! leurs dégradés (linéaires ou radiaux), tableaux (`a:tbl`, en tableau Varium
//! dans le contenu), groupes (repère des enfants) et notes de l'orateur.
//!
//! Les couleurs du thème (`a:schemeClr` : `accent1`, `tx1`, `bg2`…) sont lues
//! dans le thème du masque (`ppt/theme/themeN.xml`) à travers sa table de
//! correspondance (`p:clrMap`), avec leurs modificateurs (`lumMod`, `lumOff`,
//! `tint`, `shade`). Les positions sont en EMU (914 400 par pouce).

use std::collections::HashMap;

use varium_core::zip::ZipReader;

use crate::import::{Ligne, Marques, Medias, Page, Paragraphe, Role, Zone, construire};
use crate::objets::{Gradient, GradientStop, Object, ObjectKind};
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

// ---------------------------------------------------------------------------------
// Couleurs : thème, modificateurs, remplissages
// ---------------------------------------------------------------------------------

/// Couleurs du thème d'un masque : `dk1`, `lt1`, `accent1`… et la table de
/// correspondance du masque (`bg1` → `lt1`, `tx1` → `dk1`…).
#[derive(Debug, Clone, Default)]
pub(crate) struct Schema {
    couleurs: HashMap<String, [u8; 3]>,
    correspondance: HashMap<String, String>,
}

impl Schema {
    /// Le schéma d'un thème (`a:theme`) et d'un masque (`p:sldMaster`, pour sa
    /// `p:clrMap`). Ce qui manque prend les valeurs d'Office.
    pub(crate) fn lire(theme: Option<&Noeud>, maitre: Option<&Noeud>) -> Schema {
        let mut couleurs: HashMap<String, [u8; 3]> = [
            ("dk1", [0, 0, 0]),
            ("lt1", [255, 255, 255]),
            ("dk2", [0x44, 0x54, 0x6A]),
            ("lt2", [0xE7, 0xE6, 0xE6]),
            ("accent1", [0x44, 0x72, 0xC4]),
            ("accent2", [0xED, 0x7D, 0x31]),
            ("accent3", [0xA5, 0xA5, 0xA5]),
            ("accent4", [0xFF, 0xC0, 0x00]),
            ("accent5", [0x5B, 0x9B, 0xD5]),
            ("accent6", [0x70, 0xAD, 0x47]),
            ("hlink", [0x05, 0x63, 0xC1]),
            ("folHlink", [0x95, 0x4F, 0x72]),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        let vide = Schema::default();
        if let Some(cs) = theme.and_then(|t| t.descendant("clrScheme")) {
            for c in cs.noeuds().take(32) {
                if let Some(v) = couleur_unie(c, &vide) {
                    couleurs.insert(c.local().to_string(), v);
                }
            }
        }
        let mut correspondance: HashMap<String, String> = [
            ("bg1", "lt1"),
            ("tx1", "dk1"),
            ("bg2", "lt2"),
            ("tx2", "dk2"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        if let Some(m) = maitre.and_then(|m| m.enfant("clrMap")) {
            for k in [
                "bg1", "tx1", "bg2", "tx2", "accent1", "accent2", "accent3", "accent4", "accent5",
                "accent6", "hlink", "folHlink",
            ] {
                if let Some(v) = m.attr(k) {
                    correspondance.insert(k.to_string(), v.to_string());
                }
            }
        }
        Schema {
            couleurs,
            correspondance,
        }
    }

    /// La couleur d'un nom du thème (`accent1`, `tx1`, `phClr`…).
    fn couleur(&self, nom: &str) -> Option<[u8; 3]> {
        let nom = self.correspondance.get(nom).map_or(nom, String::as_str);
        self.couleurs.get(nom).copied()
    }
}

/// RVB → TSL (teinte, saturation, luminosité ; de 0 à 1).
fn tsl(c: [u8; 3]) -> (f32, f32, f32) {
    let [r, g, b] = c.map(|v| v as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if max - min < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, l)
}

/// TSL → RVB.
fn rvb(h: f32, s: f32, l: f32) -> [u8; 3] {
    let (h, s, l) = (h.rem_euclid(1.0), s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
    if s < 1e-6 {
        let v = (l * 255.0).round() as u8;
        return [v, v, v];
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let canal = |t: f32| {
        let t = t.rem_euclid(1.0);
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    [canal(h + 1.0 / 3.0), canal(h), canal(h - 1.0 / 3.0)]
}

/// Applique les modificateurs d'une couleur DrawingML (`lumMod`, `lumOff`,
/// `tint`, `shade`, `satMod`), en cent-millièmes.
fn modifier(mut c: [u8; 3], n: &Noeud) -> [u8; 3] {
    let val = |m: &Noeud| {
        m.attr("val")
            .and_then(|v| v.trim_end_matches('%').parse::<f32>().ok())
            .filter(|v| v.is_finite())
            .map(|v| (v / 100_000.0).clamp(-10.0, 10.0))
    };
    for m in n.noeuds().take(16) {
        let Some(v) = val(m) else {
            continue;
        };
        match m.local() {
            "lumMod" => {
                let (h, s, l) = tsl(c);
                c = rvb(h, s, l * v);
            }
            "lumOff" => {
                let (h, s, l) = tsl(c);
                c = rvb(h, s, l + v);
            }
            "satMod" => {
                let (h, s, l) = tsl(c);
                c = rvb(h, s * v, l);
            }
            // Éclaircir vers le blanc, assombrir vers le noir.
            "tint" => c = crate::texte::melange([255, 255, 255], c, v.clamp(0.0, 1.0)),
            "shade" => c = crate::texte::melange([0, 0, 0], c, v.clamp(0.0, 1.0)),
            _ => {}
        }
    }
    c
}

/// Couleur d'un élément de couleur DrawingML lui-même (`a:srgbClr`,
/// `a:schemeClr`…), modificateurs compris.
fn couleur_simple(n: &Noeud, sch: &Schema, profondeur: usize) -> Option<[u8; 3]> {
    if profondeur > 4 {
        return None;
    }
    let c = match n.local() {
        "srgbClr" => crate::texte::parse_color(n.attr("val")?)?,
        "sysClr" => crate::texte::parse_color(n.attr("lastClr")?).or(match n.attr("val") {
            Some("windowText") => Some([0, 0, 0]),
            Some("window") => Some([255, 255, 255]),
            _ => None,
        })?,
        "schemeClr" => sch.couleur(n.attr("val")?)?,
        "scrgbClr" => {
            let k = |a: &str| {
                n.attr(a)
                    .and_then(|v| v.trim_end_matches('%').parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .map(|v| (v / 100_000.0).clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0)
                    .unwrap_or(0.0)
                    .round() as u8
            };
            [k("r"), k("g"), k("b")]
        }
        "hslClr" => {
            let k = |a: &str, d: f32| {
                n.attr(a)
                    .and_then(|v| v.trim_end_matches('%').parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .unwrap_or(0.0)
                    / d
            };
            rvb(
                k("hue", 21_600_000.0),
                k("sat", 100_000.0),
                k("lum", 100_000.0),
            )
        }
        "prstClr" => match n.attr("val")? {
            "black" => [0, 0, 0],
            "white" => [255, 255, 255],
            "red" => [255, 0, 0],
            "blue" => [0, 0, 255],
            "green" => [0, 128, 0],
            "yellow" => [255, 255, 0],
            "gray" | "grey" => [128, 128, 128],
            "orange" => [255, 165, 0],
            _ => return None,
        },
        _ => return None,
    };
    Some(modifier(c, n))
}

/// La couleur contenue dans `n` (un `a:solidFill`, un `a:gs`, un `a:fillRef`…) :
/// son premier enfant de couleur.
fn couleur_unie(n: &Noeud, sch: &Schema) -> Option<[u8; 3]> {
    n.noeuds().take(8).find_map(|c| couleur_simple(c, sch, 0))
}

/// Un dégradé `a:gradFill` : arrêts `a:gs` (positions en cent-millièmes),
/// linéaire (`a:lin`, angle en 60 000es de degré) ou radial (`a:path`).
fn degrade(n: &Noeud, sch: &Schema) -> Option<Gradient> {
    let stops = n
        .enfant("gsLst")?
        .noeuds()
        .filter(|g| g.est("gs"))
        .take(64)
        .filter_map(|g| {
            let pos = g
                .attr("pos")
                .and_then(|p| p.trim_end_matches('%').parse::<f32>().ok())
                .filter(|p| p.is_finite())?
                / 100_000.0;
            Some(GradientStop {
                pos,
                color: couleur_unie(g, sch)?,
            })
        })
        .collect();
    let radial = n.enfant("path").is_some();
    let angle = n
        .enfant("lin")
        .and_then(|l| l.attr("ang"))
        .and_then(|a| a.parse::<f32>().ok())
        .filter(|a| a.is_finite())
        .map(|a| a / 60_000.0)
        .unwrap_or(if radial { 0.0 } else { 90.0 });
    Gradient {
        radial,
        angle,
        stops,
    }
    .normalized()
}

/// Remplissage d'un `p:spPr` ou d'un `p:bgPr` : uni, dégradé, aucun (`noFill`)
/// ou non précisé (`None`, le style de la forme décide).
#[derive(Debug, Clone, PartialEq)]
enum Remplissage {
    Aucun,
    Uni([u8; 3]),
    Degrade(Gradient),
}

fn remplissage(n: &Noeud, sch: &Schema) -> Option<Remplissage> {
    if n.enfant("noFill").is_some() {
        return Some(Remplissage::Aucun);
    }
    if let Some(c) = n.enfant("solidFill").and_then(|f| couleur_unie(f, sch)) {
        return Some(Remplissage::Uni(c));
    }
    if let Some(g) = n.enfant("gradFill").and_then(|f| degrade(f, sch)) {
        return Some(Remplissage::Degrade(g));
    }
    None
}

/// Fond d'une partie (diapositive, disposition ou masque) : une couleur unie (la
/// moyenne d'un dégradé).
fn fond(racine: &Noeud, sch: &Schema) -> Option<[u8; 3]> {
    let bg = racine.chemin(&["cSld", "bg"])?;
    if let Some(pr) = bg.enfant("bgPr") {
        return match remplissage(pr, sch)? {
            Remplissage::Uni(c) => Some(c),
            Remplissage::Degrade(g) => Some(g.average()),
            Remplissage::Aucun => None,
        };
    }
    // Fond du thème (`p:bgRef`) : sa couleur.
    bg.enfant("bgRef").and_then(|r| couleur_unie(r, sch))
}

/// Le masque d'une diapositive (par sa disposition) et son schéma de couleurs,
/// lu une fois par masque.
fn schema_de(
    z: &ZipReader,
    rels: &HashMap<String, (String, String)>,
    cache: &mut HashMap<String, Schema>,
) -> (Option<String>, Option<String>, Schema) {
    let dispo = rels
        .values()
        .find(|(t, _)| type_rel(t, "slideLayout"))
        .map(|(_, c)| c.clone());
    let maitre = dispo.as_ref().and_then(|d| {
        relations(z, d)
            .into_values()
            .find(|(t, _)| type_rel(t, "slideMaster"))
            .map(|(_, c)| c)
    });
    let cle = maitre.clone().unwrap_or_default();
    if let Some(s) = cache.get(&cle) {
        return (dispo, maitre, s.clone());
    }
    let mx = maitre
        .as_ref()
        .and_then(|m| z.text(m))
        .and_then(|t| lire(&t).ok());
    let theme = maitre
        .as_ref()
        .and_then(|m| {
            relations(z, m)
                .into_values()
                .find(|(t, _)| type_rel(t, "theme"))
        })
        .and_then(|(_, c)| z.text(&c))
        .and_then(|t| lire(&t).ok());
    let s = Schema::lire(theme.as_ref(), mx.as_ref());
    cache.insert(cle, s.clone());
    (dispo, maitre, s)
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
    let mut schemas: HashMap<String, Schema> = HashMap::new();
    for chemin in chemins.iter().take(2000) {
        let Some(xml) = z.text(chemin) else {
            continue;
        };
        let Ok(racine) = lire(&xml) else {
            continue;
        };
        let rels = relations(z, chemin);
        let (dispo, maitre, schema) = schema_de(z, &rels, &mut schemas);
        // Fond : diapositive, sinon sa disposition, sinon le masque.
        let fond_page = fond(&racine, &schema)
            .or_else(|| fond(&lire(&z.text(dispo.as_ref()?)?).ok()?, &schema))
            .or_else(|| fond(&lire(&z.text(maitre.as_ref()?)?).ok()?, &schema));
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
                schema: &schema,
                repere: Repere::IDENTITE,
                rotation: 0.0,
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

/// Repère d'un groupe : position EMU des enfants → position EMU sur la
/// diapositive (`x' = x × sx + tx`).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Repere {
    sx: f32,
    sy: f32,
    tx: f32,
    ty: f32,
}

impl Repere {
    const IDENTITE: Repere = Repere {
        sx: 1.0,
        sy: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    /// Le repère des enfants d'un groupe (`a:off`, `a:ext`, `a:chOff`, `a:chExt`)
    /// dans le repère `self`.
    fn groupe(&self, xfrm: Option<&Noeud>) -> Repere {
        let Some(x) = xfrm else {
            return *self;
        };
        let n = |e: &str, a: &str| {
            x.enfant(e)
                .and_then(|n| n.attr(a))
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite())
                .map(|v| v as f32)
        };
        let (Some(ox), Some(oy), Some(w), Some(h)) =
            (n("off", "x"), n("off", "y"), n("ext", "cx"), n("ext", "cy"))
        else {
            return *self;
        };
        let (cox, coy) = (n("chOff", "x").unwrap_or(ox), n("chOff", "y").unwrap_or(oy));
        let (cw, ch) = (n("chExt", "cx").unwrap_or(w), n("chExt", "cy").unwrap_or(h));
        let ex = if cw.abs() > 1.0 { w / cw } else { 1.0 };
        let ey = if ch.abs() > 1.0 { h / ch } else { 1.0 };
        let (ex, ey) = (ex.clamp(-1e4, 1e4), ey.clamp(-1e4, 1e4));
        // Enfant (x) → groupe : ox + (x − cox) × ex, puis le repère parent.
        Repere {
            sx: self.sx * ex,
            sy: self.sy * ey,
            tx: self.sx * (ox - cox * ex) + self.tx,
            ty: self.sy * (oy - coy * ey) + self.ty,
        }
    }
}

struct Contexte<'a> {
    z: &'a ZipReader,
    rels: &'a HashMap<String, (String, String)>,
    pw: f32,
    ph: f32,
    schema: &'a Schema,
    /// Repère du groupe courant.
    repere: Repere,
    /// Rotation des groupes englobants, en degrés.
    rotation: f32,
}

/// Rotation d'un `a:xfrm` (60 000es de degré), en degrés.
fn rotation_xfrm(x: Option<&Noeud>) -> f32 {
    x.and_then(|x| x.attr("rot"))
        .and_then(|r| r.parse::<f64>().ok())
        .filter(|r| r.is_finite())
        .map(|r| ((r / 60_000.0) % 360.0) as f32)
        .unwrap_or(0.0)
}

/// Remplissage et trait lus d'une forme.
struct Style {
    fill: Option<[u8; 3]>,
    gradient: Option<Gradient>,
    stroke: Option<[u8; 3]>,
    width: f32,
    arrow: bool,
}

impl Contexte<'_> {
    /// Position (`a:xfrm` d'un `p:spPr`, ou `p:xfrm` d'un `p:graphicFrame`) d'une
    /// forme, en fractions de la diapositive, et ses retournements.
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
        let r = self.repere;
        let f = Frame::new(
            (n(off, "x")? * r.sx + r.tx) / self.pw,
            (n(off, "y")? * r.sy + r.ty) / self.ph,
            n(ext, "cx")? * r.sx / self.pw,
            n(ext, "cy")? * r.sy / self.ph,
        );
        let vrai = |a: &str| matches!(x.attr(a), Some("1" | "true"));
        Some((f, vrai("flipH"), vrai("flipV")))
    }

    /// Rotation totale d'une forme (la sienne et celle de ses groupes), en degrés.
    fn rotation(&self, sp_pr: Option<&Noeud>) -> f32 {
        let r = self.rotation + rotation_xfrm(sp_pr.and_then(|s| s.enfant("xfrm")));
        if r.is_finite() {
            r.rem_euclid(360.0)
        } else {
            0.0
        }
    }

    /// Remplissage et trait d'une forme (`p:spPr`, à défaut le style `p:style` du
    /// thème) : fond, dégradé, trait, épaisseur, pointe de flèche.
    fn style(&self, forme: &Noeud) -> Style {
        let sp_pr = forme.enfant("spPr");
        let style = forme.enfant("style");
        // Couleur d'une référence au style du thème (`a:fillRef idx="1"` : oui ;
        // `idx="0"` : pas de remplissage).
        let reference = |nom: &str| {
            style
                .and_then(|s| s.enfant(nom))
                .filter(|r| r.attr("idx").is_some_and(|i| i != "0"))
                .and_then(|r| couleur_unie(r, self.schema))
        };
        let (fill, gradient) = match sp_pr.and_then(|s| remplissage(s, self.schema)) {
            Some(Remplissage::Uni(c)) => (Some(c), None),
            Some(Remplissage::Degrade(g)) => (Some(g.average()), Some(g)),
            Some(Remplissage::Aucun) => (None, None),
            None => (reference("fillRef"), None),
        };
        let ln = sp_pr.and_then(|s| s.enfant("ln"));
        let largeur = |l: &Noeud| {
            l.attr("w")
                .and_then(|w| w.parse::<f32>().ok())
                .filter(|w| w.is_finite())
                .map(|w| (w / 12_700.0 * 720.0 / (self.pw / 12_700.0)).clamp(0.5, 72.0))
                .unwrap_or(1.0)
        };
        let (stroke, w) = match ln {
            Some(l) if l.enfant("noFill").is_some() => (None, 0.0),
            Some(l) => (
                l.enfant("solidFill")
                    .and_then(|f| couleur_unie(f, self.schema))
                    .or_else(|| {
                        l.enfant("gradFill")
                            .and_then(|f| degrade(f, self.schema))
                            .map(|g| g.average())
                    })
                    .or_else(|| reference("lnRef")),
                largeur(l),
            ),
            None => (reference("lnRef"), 1.0),
        };
        let fleche = ln.is_some_and(|l| {
            ["tailEnd", "headEnd"].iter().any(|e| {
                l.enfant(e)
                    .and_then(|t| t.attr("type"))
                    .is_some_and(|t| t != "none")
            })
        });
        Style {
            fill,
            gradient,
            stroke,
            width: w,
            arrow: fleche,
        }
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
                    let st = self.style(n);
                    page.objets.push(Object {
                        kind: if st.arrow {
                            ObjectKind::Arrow
                        } else {
                            ObjectKind::Line
                        },
                        frame: f,
                        flip_h: fh,
                        flip_v: fv,
                        stroke: st.stroke.or(Some([0, 0, 0])),
                        stroke_width: st.width,
                        rotation: self.rotation(sp_pr),
                        ..Object::default()
                    });
                }
                "graphicFrame" => self.tableau(n, page),
                "pic" => {
                    let Some((f, _, _)) = self.cadre(n.enfant("spPr")) else {
                        continue;
                    };
                    let rotation = self.rotation(n.enfant("spPr"));
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
                        rotation,
                        ..Object::default()
                    });
                }
                "grpSp" => {
                    let gx = n.chemin(&["grpSpPr", "xfrm"]);
                    let enfant = Contexte {
                        repere: self.repere.groupe(gx),
                        rotation: self.rotation + rotation_xfrm(gx),
                        ..*self
                    };
                    enfant.formes(n, page, medias, profondeur + 1);
                }
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

    /// Un tableau (`p:graphicFrame` contenant un `a:tbl`) : une zone de contenu
    /// dont chaque ligne a ses cellules séparées par des tabulations, que la
    /// construction écrit en tableau Varium (`| a | b |`, la première ligne en
    /// en-tête). Graphiques et diagrammes sont ignorés.
    fn tableau(&self, n: &Noeud, page: &mut Page) {
        let Some(tbl) = n
            .chemin(&["graphic", "graphicData", "tbl"])
            .or_else(|| n.descendant("tbl"))
        else {
            return;
        };
        let mut lignes = Vec::new();
        for tr in tbl.noeuds().filter(|t| t.est("tr")).take(500) {
            let cellules: Vec<String> = tr
                .noeuds()
                .filter(|c| c.est("tc"))
                .take(100)
                .map(|tc| {
                    // Cellule fusionnée avec sa voisine : vide.
                    if tc.attr("hMerge").is_some() || tc.attr("vMerge").is_some() {
                        return String::new();
                    }
                    tc.enfant("txBody")
                        .map(|tx| {
                            paragraphes(tx, false, false)
                                .iter()
                                .map(|l| l.texte.replace(['\t', '\n'], " ").trim().to_string())
                                .filter(|t| !t.is_empty())
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .unwrap_or_default()
                })
                .collect();
            if cellules.is_empty() {
                continue;
            }
            // Colonne vide en tête : retirée à la construction, elle garantit
            // qu'une ligne d'une seule cellule reste une ligne de tableau.
            lignes.push(Ligne {
                texte: format!("\t{}", cellules.join("\t")),
                liste: None,
            });
        }
        if lignes.iter().all(|l| l.texte.trim().is_empty()) {
            return;
        }
        page.zones.push(Zone {
            role: Role::Corps,
            frame: self.cadre(Some(n)).map(|c| c.0),
            lignes,
        });
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
        let st = self.style(n);
        let (fill, stroke, w, fleche) = (st.fill, st.stroke, st.width, st.arrow);
        let zone_texte = n
            .chemin(&["nvSpPr", "cNvSpPr"])
            .and_then(|c| c.attr("txBox"))
            .is_some_and(|v| v == "1" || v == "true")
            || (geom == "rect" && fill.is_none() && stroke.is_none() && st.gradient.is_none());
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
        // Couleur du texte : celle du premier morceau qui en a une.
        let text_color = texte.and_then(|t| {
            t.descendants("rPr")
                .into_iter()
                .chain(t.descendants("defRPr"))
                .find_map(|r| r.enfant("solidFill"))
                .and_then(|f| couleur_unie(f, self.schema))
        });
        page.objets.push(Object {
            kind,
            frame: f,
            flip_h: fh,
            flip_v: fv,
            rotation: self.rotation(sp_pr),
            gradient: if kind.is_closed() { st.gradient } else { None },
            text_color: if kind.is_closed() { text_color } else { None },
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
