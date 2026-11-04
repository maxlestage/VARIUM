//! Signatures manuscrites : des traits dessinés à la souris ou au pavé tactile,
//! insérés par `{{signature}}` ou `{{signature:nom}}`.
//!
//! Une signature est enregistrée hors du document (dossier de l'utilisateur, mis à
//! disposition des exports par [`definir`]) ou dans le document lui-même, comme valeur
//! du champ réservé `signature:nom` (voir [`cle_valeur`]). Elle se dessine en traits
//! vectoriels dans la page et le PDF, en SVG en HTML, et en image PNG dans les
//! fichiers ODT et DOCX.

use std::collections::BTreeMap;
use std::sync::RwLock;

/// Hauteur d'une signature dans la page, en millimètres.
pub const HAUTEUR_MM: f32 = 16.0;
/// Largeur maximale, en millimètres.
pub const LARGEUR_MAX_MM: f32 = 70.0;

/// Une signature : des traits (lignes brisées) dans un cadre `largeur` × `hauteur`
/// (unités arbitraires, origine en haut à gauche).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Signature {
    pub largeur: f32,
    pub hauteur: f32,
    pub traits: Vec<Vec<(f32, f32)>>,
}

impl Signature {
    /// Recadre des traits bruts (en pixels) sur leur encombrement, hauteur ramenée à 100.
    pub fn depuis_traits(traits: &[Vec<(f32, f32)>]) -> Option<Signature> {
        let pts = traits
            .iter()
            .flatten()
            .filter(|p| p.0.is_finite() && p.1.is_finite());
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        let mut n = 0;
        for p in pts {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
            n += 1;
        }
        if n == 0 {
            return None;
        }
        let marge = ((x1 - x0).max(y1 - y0) * 0.04).max(1.0);
        let (x0, y0) = (x0 - marge, y0 - marge);
        let h = (y1 - y0 + marge).max((x1 - x0 + marge) / 6.0).max(1.0);
        let k = 100.0 / h;
        let traits = traits
            .iter()
            .map(|t| {
                t.iter()
                    .filter(|p| p.0.is_finite() && p.1.is_finite())
                    .map(|p| ((p.0 - x0) * k, (p.1 - y0) * k))
                    .collect::<Vec<_>>()
            })
            .filter(|t: &Vec<(f32, f32)>| !t.is_empty())
            .collect();
        Some(Signature {
            largeur: ((x1 - x0 + marge) * k).clamp(1.0, 2000.0),
            hauteur: 100.0,
            traits,
        })
    }

    /// Écriture compacte : `sig1;largeur;hauteur;x,y x,y|x,y …`.
    pub fn ecrire(&self) -> String {
        let traits: Vec<String> = self
            .traits
            .iter()
            .map(|t| {
                t.iter()
                    .map(|(x, y)| format!("{:.1},{:.1}", x, y))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        format!(
            "sig1;{:.1};{:.1};{}",
            self.largeur,
            self.hauteur,
            traits.join("|")
        )
    }

    /// Relit [`Signature::ecrire`] ; `None` si le texte n'est pas une signature.
    pub fn lire(s: &str) -> Option<Signature> {
        let mut it = s.trim().splitn(4, ';');
        if it.next()? != "sig1" {
            return None;
        }
        let num = |v: &str| v.trim().parse::<f32>().ok().filter(|v| v.is_finite());
        let largeur = num(it.next()?)?.clamp(1.0, 2000.0);
        let hauteur = num(it.next()?)?.clamp(1.0, 2000.0);
        let mut traits = Vec::new();
        for t in it.next().unwrap_or("").split('|').take(10_000) {
            let pts: Vec<(f32, f32)> = t
                .split_whitespace()
                .take(100_000)
                .filter_map(|p| {
                    let (x, y) = p.split_once(',')?;
                    Some((
                        num(x)?.clamp(-1.0, largeur + 1.0),
                        num(y)?.clamp(-1.0, hauteur + 1.0),
                    ))
                })
                .collect();
            if !pts.is_empty() {
                traits.push(pts);
            }
        }
        Some(Signature {
            largeur,
            hauteur,
            traits,
        })
    }

    /// Taille dans la page, en millimètres (hauteur fixe, largeur bornée).
    pub fn taille_mm(&self) -> (f32, f32) {
        let k = HAUTEUR_MM / self.hauteur.max(1.0);
        let l = self.largeur * k;
        if l > LARGEUR_MAX_MM {
            (LARGEUR_MAX_MM, HAUTEUR_MM * LARGEUR_MAX_MM / l)
        } else {
            (l.max(1.0), HAUTEUR_MM)
        }
    }

    /// Épaisseur du trait, dans les unités de la signature.
    pub fn epaisseur(&self) -> f32 {
        self.hauteur * 0.028
    }

    /// Les traits à l'échelle d'un cadre `l` × `h`.
    pub fn a_l_echelle(&self, l: f32, h: f32) -> Vec<Vec<(f32, f32)>> {
        let (kx, ky) = (l / self.largeur.max(1.0), h / self.hauteur.max(1.0));
        self.traits
            .iter()
            .map(|t| t.iter().map(|(x, y)| (x * kx, y * ky)).collect())
            .collect()
    }

    /// La signature en SVG, de `mm` millimètres de haut.
    pub fn svg(&self, nom: &str) -> String {
        let (l, h) = self.taille_mm();
        let mut chemins = String::new();
        for t in &self.traits {
            if t.len() == 1 {
                chemins.push_str(&format!(
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\"/>",
                    t[0].0,
                    t[0].1,
                    self.epaisseur() / 2.0
                ));
                continue;
            }
            let d: Vec<String> = t
                .iter()
                .enumerate()
                .map(|(i, (x, y))| format!("{}{:.1} {:.1}", if i == 0 { 'M' } else { 'L' }, x, y))
                .collect();
            chemins.push_str(&format!("<path d=\"{}\"/>", d.join(" ")));
        }
        format!(
            "<svg class=\"signature\" xmlns=\"http://www.w3.org/2000/svg\" width=\"{:.1}mm\" height=\"{:.1}mm\" viewBox=\"0 0 {:.1} {:.1}\" role=\"img\" aria-label=\"{}\"><g fill=\"none\" stroke=\"#1a2a5a\" stroke-width=\"{:.2}\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{chemins}</g></svg>",
            l,
            h,
            self.largeur,
            self.hauteur,
            crate::export::xml_escape(if nom.is_empty() { "Signature" } else { nom }),
            self.epaisseur()
        )
    }

    /// La signature en image PNG (niveaux de gris, fond blanc), `px` pixels de haut.
    pub fn png(&self, px: u32) -> Vec<u8> {
        let px = px.clamp(8, 1000);
        let k = px as f32 / self.hauteur.max(1.0);
        let w = ((self.largeur * k).ceil() as u32).clamp(8, 6000);
        let h = px;
        let mut img = vec![255u8; (w * h) as usize];
        let r = (self.epaisseur() * k / 2.0).max(0.8);
        for t in &self.traits {
            let pts: Vec<(f32, f32)> = t.iter().map(|(x, y)| (x * k, y * k)).collect();
            let segs: Vec<((f32, f32), (f32, f32))> = if pts.len() == 1 {
                vec![(pts[0], pts[0])]
            } else {
                pts.windows(2).map(|p| (p[0], p[1])).collect()
            };
            for (a, b) in segs {
                let (xa, xb) = (
                    (a.0.min(b.0) - r - 1.0).max(0.0),
                    (a.0.max(b.0) + r + 1.0).min(w as f32),
                );
                let (ya, yb) = (
                    (a.1.min(b.1) - r - 1.0).max(0.0),
                    (a.1.max(b.1) + r + 1.0).min(h as f32),
                );
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let l2 = dx * dx + dy * dy;
                for y in ya as u32..yb as u32 {
                    for x in xa as u32..xb as u32 {
                        let (px_, py) = (x as f32 + 0.5, y as f32 + 0.5);
                        let t = if l2 > 0.0 {
                            (((px_ - a.0) * dx + (py - a.1) * dy) / l2).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        let (cx, cy) = (a.0 + t * dx, a.1 + t * dy);
                        let d = ((px_ - cx).powi(2) + (py - cy).powi(2)).sqrt();
                        let couverture = (r + 0.5 - d).clamp(0.0, 1.0);
                        let v = (255.0 * (1.0 - couverture * 0.9)) as u8;
                        let i = (y * w + x) as usize;
                        img[i] = img[i].min(v);
                    }
                }
            }
        }
        let mut lignes = Vec::with_capacity(((w + 1) * h) as usize);
        for y in 0..h {
            lignes.push(0);
            lignes.extend_from_slice(&img[(y * w) as usize..((y + 1) * w) as usize]);
        }
        crate::qr::png_gris(w, h, &lignes)
    }
}

/// Lisse une suite de points (coupes de Chaikin), pour un trait régulier.
pub fn lisser(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut p: Vec<(f32, f32)> = Vec::with_capacity(points.len());
    // Points trop proches retirés (tremblements).
    for &q in points {
        if p.last()
            .is_none_or(|l: &(f32, f32)| (l.0 - q.0).abs() + (l.1 - q.1).abs() > 0.8)
        {
            p.push(q);
        }
    }
    for _ in 0..2 {
        if p.len() < 3 {
            break;
        }
        let mut q = Vec::with_capacity(p.len() * 2);
        q.push(p[0]);
        for w in p.windows(2) {
            let (a, b) = (w[0], w[1]);
            q.push((0.75 * a.0 + 0.25 * b.0, 0.75 * a.1 + 0.25 * b.1));
            q.push((0.25 * a.0 + 0.75 * b.0, 0.25 * a.1 + 0.75 * b.1));
        }
        q.push(p[p.len() - 1]);
        p = q;
    }
    p
}

/// Nom de la valeur qui intègre la signature `nom` au document.
pub fn cle_valeur(nom: &str) -> String {
    format!("{}{nom}", crate::parse::SIGNATURE)
}

static REGISTRE: RwLock<BTreeMap<String, Signature>> = RwLock::new(BTreeMap::new());
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Compteur des changements du registre (pour les caches de l'affichage).
pub fn generation() -> u64 {
    GENERATION.load(std::sync::atomic::Ordering::Relaxed)
}

fn changement() {
    GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Rend une signature disponible pour l'affichage et les exports (signatures de
/// l'utilisateur, hors du document).
pub fn definir(nom: &str, s: Signature) {
    if let Ok(mut r) = REGISTRE.write() {
        r.insert(nom.to_string(), s);
    }
    changement();
}

/// Retire une signature du registre.
pub fn retirer(nom: &str) {
    if let Ok(mut r) = REGISTRE.write() {
        r.remove(nom);
    }
    changement();
}

/// Une signature du registre.
pub fn obtenir(nom: &str) -> Option<Signature> {
    REGISTRE.read().ok()?.get(nom).cloned()
}

/// Les noms des signatures du registre.
pub fn noms() -> Vec<String> {
    REGISTRE
        .read()
        .map(|r| r.keys().cloned().collect())
        .unwrap_or_default()
}

/// La signature `nom` : celle intégrée au document (dans `values`), sinon celle de
/// l'utilisateur.
pub fn trouver(nom: &str, values: &crate::render::Values) -> Option<Signature> {
    Signature::lire(values.get(&cle_valeur(nom))).or_else(|| obtenir(nom))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exemple() -> Signature {
        Signature::depuis_traits(&[
            vec![(10.0, 50.0), (30.0, 20.0), (50.0, 60.0), (90.0, 30.0)],
            vec![(60.0, 70.0)],
        ])
        .unwrap()
    }

    #[test]
    fn aller_retour() {
        let s = exemple();
        assert_eq!(s.hauteur, 100.0);
        let t = s.ecrire();
        let r = Signature::lire(&t).unwrap();
        assert_eq!(r.traits.len(), 2);
        assert!((r.largeur - s.largeur).abs() < 0.1);
        assert!(Signature::lire("pas une signature").is_none());
        assert!(Signature::lire("sig1;x;y;").is_none());
        assert!(Signature::lire("sig1;10;10;1,2 3,nan|,,").is_some());
        assert!(Signature::depuis_traits(&[]).is_none());
    }

    #[test]
    fn rendus() {
        let s = exemple();
        assert!(s.svg("moi").contains("<path d=\"M"));
        let png = s.png(60);
        assert!(png.starts_with(b"\x89PNG"));
        let (l, h) = s.taille_mm();
        assert!(l <= LARGEUR_MAX_MM && h <= HAUTEUR_MM);
        assert!(lisser(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]).len() > 3);
    }

    #[test]
    fn registre_et_valeurs() {
        definir("essai-registre", exemple());
        assert!(noms().contains(&"essai-registre".to_string()));
        let mut v = crate::render::Values::default();
        assert!(trouver("essai-registre", &v).is_some());
        assert!(trouver("absente", &v).is_none());
        v.set(cle_valeur("absente"), exemple().ecrire());
        assert!(trouver("absente", &v).is_some());
        retirer("essai-registre");
        assert!(obtenir("essai-registre").is_none());
    }
}
