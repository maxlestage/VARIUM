//! Lecture d'un document Word (`.docx`, Office Open XML) : voir `import`.

use std::collections::HashMap;

use crate::doc::Align;
use crate::import::{self, Bloc, Brouillon, Genre, Seg};
use crate::xmlarbre::{self, Child, Node};
use crate::zip::ZipReader;

/// Ce qu'un style Word apporte (les valeurs absentes viennent de `basedOn`).
#[derive(Debug, Clone, Default)]
struct StyleW {
    parent: Option<String>,
    gras: Option<bool>,
    italique: Option<bool>,
    souligne: Option<bool>,
    barre: Option<bool>,
    /// `Some(None)` : couleur automatique (noir), qui annule celle du style.
    couleur: Option<Option<String>>,
    fond: Option<Option<String>>,
    align: Option<Align>,
    titre: Option<u8>,
    citation: bool,
    saut_avant: bool,
    bordure_bas: bool,
    num: Option<(String, u8)>,
}

/// Une propriété « bascule » de Word : `<w:b/>` (oui) ou `<w:b w:val="0"/>` (non).
fn bascule(n: &Node) -> bool {
    !matches!(n.attr("w:val"), Some("0" | "false" | "off" | "none"))
}

fn proprietes_caracteres(rpr: &Node, st: &mut StyleW) {
    for c in rpr.elems() {
        if c.is("b") {
            st.gras = Some(bascule(c));
        } else if c.is("i") {
            st.italique = Some(bascule(c));
        } else if c.is("u") {
            st.souligne = Some(bascule(c));
        } else if c.is("strike") || c.is("dstrike") {
            st.barre = Some(bascule(c));
        } else if c.is("color") {
            st.couleur = Some(c.attr("w:val").and_then(import::couleur_hex));
        } else if c.is("highlight") {
            st.fond = Some(c.attr("w:val").and_then(surlignage_word));
        } else if c.is("shd") && st.fond.is_none() {
            st.fond = Some(c.attr("w:fill").and_then(import::couleur_hex));
        }
    }
}

/// Les couleurs de surlignage de Word (`w:highlight`).
fn surlignage_word(nom: &str) -> Option<String> {
    let h = match nom {
        "yellow" => "ffff00",
        "green" => "00ff00",
        "cyan" => "00ffff",
        "magenta" => "ff00ff",
        "blue" => "0000ff",
        "red" => "ff0000",
        "darkBlue" => "000080",
        "darkCyan" => "008080",
        "darkGreen" => "008000",
        "darkMagenta" => "800080",
        "darkRed" => "800000",
        "darkYellow" => "808000",
        "darkGray" => "808080",
        "lightGray" => "c0c0c0",
        _ => return None,
    };
    Some(h.to_string())
}

fn alignement(v: &str) -> Option<Align> {
    match v {
        "center" => Some(Align::Center),
        "right" | "end" => Some(Align::Right),
        "both" | "distribute" | "justify" => Some(Align::Justify),
        "left" | "start" => Some(Align::Left),
        _ => None,
    }
}

fn numero(ppr: &Node) -> Option<(String, u8)> {
    let np = ppr.child("numPr")?;
    let id = np.child("numId")?.attr("w:val")?.to_string();
    let lvl = np
        .child("ilvl")
        .and_then(|n| n.attr("w:val"))
        .and_then(|v| v.parse::<u8>().ok())
        .unwrap_or(0);
    Some((id, lvl.min(8)))
}

/// Le niveau de titre que suggère un style (`Heading2`, « heading 2 », `Titre1`).
fn niveau_par_nom(id: &str, nom: &str) -> Option<u8> {
    for n in [id.to_lowercase(), nom.to_lowercase()] {
        if n == "title" || n == "titre" {
            return Some(1);
        }
        if n == "subtitle" || n == "sous-titre" {
            return Some(2);
        }
        for p in ["heading", "titre", "überschrift", "título", "titolo"] {
            if let Some(r) = n.strip_prefix(p) {
                return r.trim().parse::<u8>().ok().filter(|l| (1..=9).contains(l));
            }
        }
    }
    None
}

#[derive(Default)]
struct Styles {
    styles: HashMap<String, StyleW>,
    defaut_paragraphe: Option<String>,
    /// numId → (abstractNumId, débuts imposés par niveau).
    nums: HashMap<String, (String, HashMap<u8, u32>)>,
    /// abstractNumId → niveau → (numéroté, début).
    abstraits: HashMap<String, HashMap<u8, (bool, u32)>>,
}

impl Styles {
    fn lire(&mut self, racine: &Node) {
        for s in racine.children_named("style") {
            let id = s.attr("w:styleId").unwrap_or("").to_string();
            let nom = s
                .child("name")
                .and_then(|n| n.attr("w:val"))
                .unwrap_or("")
                .to_string();
            let mut st = StyleW {
                parent: s
                    .child("basedOn")
                    .and_then(|n| n.attr("w:val"))
                    .map(str::to_string),
                titre: niveau_par_nom(&id, &nom),
                citation: [
                    "quote",
                    "intensequote",
                    "citation",
                    "quotations",
                    "blockquotation",
                ]
                .contains(&id.to_lowercase().as_str())
                    || [
                        "quote",
                        "intense quote",
                        "citation",
                        "quotations",
                        "block quotation",
                    ]
                    .contains(&nom.to_lowercase().as_str()),
                ..StyleW::default()
            };
            if let Some(ppr) = s.child("pPr") {
                st.align = ppr
                    .child("jc")
                    .and_then(|j| j.attr("w:val"))
                    .and_then(alignement);
                if let Some(l) = ppr
                    .child("outlineLvl")
                    .and_then(|o| o.attr("w:val"))
                    .and_then(|v| v.parse::<u8>().ok())
                    .filter(|l| *l < 9)
                {
                    st.titre = Some(l + 1);
                }
                st.saut_avant = ppr.child("pageBreakBefore").is_some_and(bascule);
                st.bordure_bas = ppr
                    .child("pBdr")
                    .and_then(|b| b.child("bottom"))
                    .is_some_and(|b| {
                        b.attr("w:val") != Some("none") && b.attr("w:val") != Some("nil")
                    });
                st.num = numero(ppr);
            }
            if let Some(rpr) = s.child("rPr") {
                proprietes_caracteres(rpr, &mut st);
            }
            if s.attr("w:type") == Some("paragraph") && s.attr("w:default") == Some("1") {
                self.defaut_paragraphe = Some(id.clone());
            }
            self.styles.insert(id, st);
        }
    }

    fn lire_numerotation(&mut self, racine: &Node) {
        for a in racine.children_named("abstractNum") {
            let id = a.attr("w:abstractNumId").unwrap_or("").to_string();
            let mut niveaux = HashMap::new();
            for l in a.children_named("lvl") {
                let Some(n) = l.attr("w:ilvl").and_then(|v| v.parse::<u8>().ok()) else {
                    continue;
                };
                let fmt = l
                    .child("numFmt")
                    .and_then(|f| f.attr("w:val"))
                    .unwrap_or("decimal");
                let debut = l
                    .child("start")
                    .and_then(|f| f.attr("w:val"))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1);
                niveaux.insert(n, (!matches!(fmt, "bullet" | "none"), debut));
            }
            self.abstraits.insert(id, niveaux);
        }
        for n in racine.children_named("num") {
            let id = n.attr("w:numId").unwrap_or("").to_string();
            let abs = n
                .child("abstractNumId")
                .and_then(|a| a.attr("w:val"))
                .unwrap_or("")
                .to_string();
            let mut debuts = HashMap::new();
            for o in n.children_named("lvlOverride") {
                if let (Some(l), Some(v)) = (
                    o.attr("w:ilvl").and_then(|v| v.parse::<u8>().ok()),
                    o.child("startOverride")
                        .and_then(|s| s.attr("w:val"))
                        .and_then(|v| v.parse::<u32>().ok()),
                ) {
                    debuts.insert(l, v);
                }
            }
            self.nums.insert(id, (abs, debuts));
        }
    }

    /// Le style résolu (chaîne `basedOn`), à partir de `id`.
    fn resoudre(&self, id: Option<&str>) -> StyleW {
        let mut r = StyleW::default();
        let mut courant = id.map(str::to_string);
        let mut premier = true;
        for _ in 0..24 {
            let Some(n) = courant.take() else { break };
            let Some(st) = self.styles.get(&n) else { break };
            r.gras = r.gras.or(st.gras);
            r.italique = r.italique.or(st.italique);
            r.souligne = r.souligne.or(st.souligne);
            r.barre = r.barre.or(st.barre);
            r.couleur = r.couleur.clone().or(st.couleur.clone());
            r.fond = r.fond.clone().or(st.fond.clone());
            r.align = r.align.or(st.align);
            r.titre = r.titre.or(st.titre);
            r.num = r.num.clone().or(st.num.clone());
            r.citation |= st.citation;
            if premier {
                r.saut_avant = st.saut_avant;
                r.bordure_bas = st.bordure_bas;
                premier = false;
            }
            courant = st.parent.clone();
        }
        r
    }

    /// Niveau numéroté ? Et son premier numéro.
    fn niveau(&self, num: &str, lvl: u8) -> Option<(bool, u32)> {
        let (abs, debuts) = self.nums.get(num)?;
        let (numerote, debut) = self.abstraits.get(abs)?.get(&lvl).copied()?;
        Some((numerote, debuts.get(&lvl).copied().unwrap_or(debut)))
    }
}

#[derive(Debug, Clone, Default)]
struct Forme {
    gras: bool,
    italique: bool,
    souligne: bool,
    barre: bool,
    lien: Option<String>,
    couleur: Option<String>,
    fond: Option<String>,
}

impl Forme {
    fn seg(&self, t: &str) -> Seg {
        Seg {
            texte: t.to_string(),
            gras: self.gras,
            italique: self.italique,
            souligne: self.souligne,
            barre: self.barre,
            lien: self.lien.clone(),
            couleur: self.couleur.clone(),
            fond: self.fond.clone(),
            brut: false,
        }
    }

    fn avec(&self, st: &StyleW) -> Forme {
        Forme {
            gras: st.gras.unwrap_or(self.gras),
            italique: st.italique.unwrap_or(self.italique),
            souligne: st.souligne.unwrap_or(self.souligne),
            barre: st.barre.unwrap_or(self.barre),
            lien: self.lien.clone(),
            couleur: st.couleur.clone().unwrap_or_else(|| self.couleur.clone()),
            fond: st.fond.clone().unwrap_or_else(|| self.fond.clone()),
        }
    }
}

use crate::import::MARQUE_PAGE as PAGE;
/// Marque d'un saut de page au milieu d'un paragraphe.
const SAUT: &str = "\u{1}saut";

struct Lecteur<'a> {
    styles: &'a Styles,
    /// Cible des liens (`r:id` → adresse).
    liens: HashMap<String, String>,
    notes: HashMap<String, &'a Node>,
    notes_fin: HashMap<String, &'a Node>,
    commentaires: HashMap<String, &'a Node>,
    b: Brouillon,
    /// Compteurs : (numId, niveau) → dernier numéro.
    compteurs: HashMap<(String, u8), u32>,
    /// Les notes se lisent sans s'imbriquer.
    dans_une_note: bool,
}

impl<'a> Lecteur<'a> {
    fn runs(&mut self, n: &Node, forme: &Forme, out: &mut Vec<Seg>, profondeur: usize) {
        if profondeur > 200 {
            return;
        }
        for c in n.elems() {
            if c.is("r") {
                self.run(c, forme, out);
            } else if c.is("hyperlink") {
                let mut f = forme.clone();
                f.lien = c
                    .attr("r:id")
                    .and_then(|id| self.liens.get(id).cloned())
                    .filter(|u| !u.is_empty());
                self.runs(c, &f, out, profondeur + 1);
            } else if c.is("ins") || c.is("moveTo") {
                out.push(Seg::brut("{++"));
                self.runs(c, forme, out, profondeur + 1);
                out.push(Seg::brut("++}"));
                if let Some(m) = auteur_date(c) {
                    out.push(Seg::brut(m));
                }
            } else if c.is("del") || c.is("moveFrom") {
                out.push(Seg::brut("{--"));
                self.runs(c, forme, out, profondeur + 1);
                out.push(Seg::brut("--}"));
                if let Some(m) = auteur_date(c) {
                    out.push(Seg::brut(m));
                }
            } else if c.is("fldSimple") {
                let instr = c.attr("w:instr").unwrap_or("").to_uppercase();
                if instr.split_whitespace().next() == Some("PAGE") {
                    out.push(Seg::brut(PAGE));
                } else {
                    self.runs(c, forme, out, profondeur + 1);
                }
            } else if c.is("oMathPara") {
                // Formule centrée (OMML) : relue en LaTeX.
                for m in c.children_named("oMath") {
                    let t = omml_latex(m, 0);
                    if !t.trim().is_empty() {
                        out.push(Seg::brut(format!("$${}$$", t.trim())));
                    }
                }
            } else if c.is("oMath") {
                let t = omml_latex(c, 0);
                if !t.trim().is_empty() {
                    out.push(Seg::brut(format!("${}$", t.trim())));
                }
            } else if c.is("AlternateContent") {
                if let Some(ch) = c.child("Choice").or_else(|| c.child("Fallback")) {
                    self.runs(ch, forme, out, profondeur + 1);
                }
            } else if c.is("smartTag")
                || c.is("customXml")
                || c.is("sdt")
                || c.is("sdtContent")
                || c.is("bdo")
                || c.is("dir")
            {
                self.runs(c, forme, out, profondeur + 1);
            }
        }
    }

    fn run(&mut self, r: &Node, forme: &Forme, out: &mut Vec<Seg>) {
        let mut f = forme.clone();
        if let Some(rpr) = r.child("rPr") {
            if let Some(rs) = rpr.child("rStyle").and_then(|s| s.attr("w:val")) {
                f = f.avec(&self.styles.resoudre(Some(rs)));
            }
            let mut st = StyleW::default();
            proprietes_caracteres(rpr, &mut st);
            f = f.avec(&st);
        }
        for c in &r.children {
            let Child::Elem(e) = c else { continue };
            if e.is("t") || e.is("delText") {
                let t = e.text();
                if !t.is_empty() {
                    out.push(f.seg(&t));
                }
            } else if e.is("tab") || e.is("ptab") {
                out.push(f.seg("\t"));
            } else if e.is("br") || e.is("cr") {
                if e.attr("w:type") == Some("page") {
                    out.push(Seg::brut(SAUT));
                } else {
                    out.push(f.seg("\n"));
                }
            } else if e.is("noBreakHyphen") {
                out.push(f.seg("\u{2011}"));
            } else if e.is("sym") {
                if let Some(ch) = e
                    .attr("w:char")
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .and_then(char::from_u32)
                    .filter(|c| !('\u{F000}'..='\u{F0FF}').contains(c))
                {
                    out.push(f.seg(ch.encode_utf8(&mut [0; 4])));
                }
            } else if e.is("instrText") {
                let i = e.text().to_uppercase();
                if i.split_whitespace().next() == Some("PAGE") {
                    out.push(Seg::brut(PAGE));
                }
            } else if e.is("footnoteReference") || e.is("endnoteReference") {
                self.note(e, out);
            } else if e.is("commentReference") {
                if let Some(c) = e.attr("w:id").and_then(|id| self.commentaires.get(id)) {
                    let auteur = c.attr("w:author").unwrap_or("");
                    let date: String = c.attr("w:date").unwrap_or("").chars().take(10).collect();
                    let texte: Vec<String> = c
                        .children_named("p")
                        .map(|p| p.text().trim().to_string())
                        .filter(|t| !t.is_empty())
                        .collect();
                    let texte = texte.join(" ");
                    if !texte.is_empty() {
                        out.push(Seg::brut(crate::critic::commentaire(auteur, &date, &texte)));
                    }
                }
            } else if e.is("drawing") || e.is("pict") || e.is("object") {
                out.push(self.dessin(e));
            } else if e.is("AlternateContent")
                && let Some(ch) = e.child("Choice").or_else(|| e.child("Fallback"))
            {
                for d in ch.elems() {
                    if d.is("drawing") || d.is("pict") {
                        out.push(self.dessin(d));
                    }
                }
            }
        }
    }

    /// Une image : un code QR de Varium (`{{qr:…}}`) ou une marque « non importée ».
    fn dessin(&mut self, d: &Node) -> Seg {
        let mut pile = vec![d];
        while let Some(n) = pile.pop() {
            if n.is("docPr") {
                let nom = n.attr("name").unwrap_or("");
                let descr = n.attr("descr").unwrap_or("");
                if nom.starts_with("QR") && !descr.is_empty() && !descr.contains("}}") {
                    return Seg::brut(format!("{{{{qr:{}}}}}", descr.trim()));
                }
                break;
            }
            pile.extend(n.elems());
        }
        self.b.image()
    }

    fn note(&mut self, e: &Node, out: &mut Vec<Seg>) {
        if self.dans_une_note {
            return;
        }
        let id = e.attr("w:id").unwrap_or("");
        let corps = if e.is("footnoteReference") {
            self.notes.get(id).copied()
        } else {
            self.notes_fin.get(id).copied()
        };
        let Some(corps) = corps else { return };
        self.dans_une_note = true;
        let mut segs = Vec::new();
        for p in corps.children_named("p") {
            if !segs.is_empty() {
                segs.push(Seg {
                    texte: " ".into(),
                    ..Seg::default()
                });
            }
            self.runs(p, &Forme::default(), &mut segs, 0);
        }
        self.dans_une_note = false;
        for s in &mut segs {
            if !s.brut {
                s.texte = s.texte.replace(['\n', '\t'], " ");
            }
        }
        // Word répète l'appel de note au début de la note : déjà retiré (marque vide).
        segs.retain(|s| !(s.brut && (s.texte == PAGE || s.texte == SAUT)));
        let m = self.b.note(&segs);
        out.push(m);
    }

    fn paragraphe(&mut self, p: &Node) {
        let ppr = p.child("pPr");
        let style_id = ppr
            .and_then(|x| x.child("pStyle"))
            .and_then(|s| s.attr("w:val"))
            .map(str::to_string)
            .or_else(|| self.styles.defaut_paragraphe.clone());
        let st = self.styles.resoudre(style_id.as_deref());
        let align = ppr
            .and_then(|x| x.child("jc"))
            .and_then(|j| j.attr("w:val"))
            .and_then(alignement)
            .or(st.align)
            .unwrap_or(Align::Left);
        let saut_avant = ppr
            .and_then(|x| x.child("pageBreakBefore"))
            .map_or(st.saut_avant, bascule);
        let retrait_tw = ppr
            .and_then(|x| x.child("ind"))
            .and_then(|i| i.attr("w:left").or_else(|| i.attr("w:start")))
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.0);
        let bordure = ppr
            .and_then(|x| x.child("pBdr"))
            .map_or(st.bordure_bas, |b| b.child("bottom").is_some());
        let num = ppr.and_then(numero).or(st.num.clone());
        if saut_avant {
            self.b.saut();
        }
        // Les caractères du style de paragraphe (sauf le gras des titres).
        let mut base = Forme::default().avec(&st);
        if st.titre.is_some() {
            base.gras = false;
        }
        let mut segs = Vec::new();
        self.runs(p, &base, &mut segs, 0);
        let genre = if let Some((id, lvl)) = num.filter(|(id, _)| id != "0") {
            match self.styles.niveau(&id, lvl) {
                Some((numerote, debut)) => {
                    let k = self.compteurs.entry((id.clone(), lvl)).or_insert(debut - 1);
                    *k += 1;
                    let n = *k;
                    // Les niveaux plus profonds recommencent.
                    for ((i, l), v) in self.compteurs.iter_mut() {
                        if *i == id && *l > lvl {
                            *v = self.styles.niveau(i, *l).map_or(0, |(_, d)| d - 1);
                        }
                    }
                    Genre::Item {
                        profondeur: lvl.min(5),
                        numero: numerote.then_some(n),
                    }
                }
                None => Genre::Item {
                    profondeur: lvl.min(5),
                    numero: None,
                },
            }
        } else if let Some(l) = st.titre {
            Genre::Titre(l.min(3))
        } else if st.citation {
            Genre::Citation
        } else {
            Genre::Corps
        };
        // Un saut de page au milieu du paragraphe le coupe en deux.
        let mut morceaux: Vec<Vec<Seg>> = vec![Vec::new()];
        for s in segs {
            if s.brut && s.texte == SAUT {
                morceaux.push(Vec::new());
            } else if !(s.brut && s.texte == PAGE)
                && let Some(m) = morceaux.last_mut()
            {
                m.push(s);
            }
        }
        let n = morceaux.len();
        for (i, m) in morceaux.into_iter().enumerate() {
            if i > 0 {
                self.b.saut();
            }
            let vide = import::texte_de(&m).trim().is_empty() && m.iter().all(|s| !s.brut);
            if vide && n > 1 {
                continue;
            }
            if vide && bordure && genre == Genre::Corps {
                self.b.blocs.push(Bloc::Filet);
                continue;
            }
            let g = if i > 0 && matches!(genre, Genre::Item { .. }) {
                Genre::Corps
            } else {
                genre
            };
            self.b.paragraphe(g, align, m, retrait_tw / 56.692_9);
        }
    }

    fn tableau(&mut self, t: &Node) {
        let mut lignes: Vec<Vec<Vec<Seg>>> = Vec::new();
        let mut aligns: Vec<Option<Align>> = Vec::new();
        let mut entete = false;
        for (i, tr) in t.children_named("tr").take(10_000).enumerate() {
            if i == 0 {
                entete = tr
                    .child("trPr")
                    .and_then(|p| p.child("tblHeader"))
                    .is_some_and(bascule);
            }
            let mut l = Vec::new();
            for tc in tr.children_named("tc") {
                let pr = tc.child("tcPr");
                let span = pr
                    .and_then(|p| p.child("gridSpan"))
                    .and_then(|g| g.attr("w:val"))
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(1)
                    .clamp(1, 64);
                let suite = pr
                    .and_then(|p| p.child("vMerge"))
                    .is_some_and(|v| v.attr("w:val") != Some("restart"));
                let mut segs = Vec::new();
                let mut align = None;
                if !suite {
                    for p in tc.elems() {
                        if p.is("p") {
                            align = align.or_else(|| {
                                p.child("pPr")
                                    .and_then(|x| x.child("jc"))
                                    .and_then(|j| j.attr("w:val"))
                                    .and_then(alignement)
                            });
                            if !segs.is_empty() {
                                segs.push(Seg {
                                    texte: " ".into(),
                                    ..Seg::default()
                                });
                            }
                            self.runs(p, &Forme::default(), &mut segs, 0);
                        } else if p.is("tbl") {
                            segs.push(Seg {
                                texte: format!(" {}", p.text()),
                                ..Seg::default()
                            });
                        }
                    }
                }
                segs.retain(|s| !(s.brut && (s.texte == PAGE || s.texte == SAUT)));
                for s in &mut segs {
                    if !s.brut {
                        s.texte = s.texte.replace(['\n', '\t'], " ");
                    }
                }
                for k in 0..span {
                    if l.len() >= 64 {
                        break;
                    }
                    if lignes.len() == usize::from(entete) {
                        if aligns.len() <= l.len() {
                            aligns.resize(l.len() + 1, None);
                        }
                        aligns[l.len()] = aligns[l.len()].or(align);
                    }
                    l.push(if k == 0 { segs.clone() } else { Vec::new() });
                }
            }
            lignes.push(l);
        }
        if std::mem::take(&mut self.b.saut_en_attente) {
            self.b.blocs.push(Bloc::Saut);
        }
        // Varium (et Word) ajoutent un en-tête gras et grisé : la première ligne est
        // un en-tête si elle est marquée comme telle.
        if !lignes.is_empty() {
            self.b.blocs.push(Bloc::Tableau {
                lignes,
                entete,
                aligns: aligns.into_iter().map(Option::unwrap_or_default).collect(),
            });
        }
    }

    fn corps(&mut self, n: &Node, profondeur: usize) {
        if profondeur > 64 {
            return;
        }
        for c in n.elems() {
            if c.is("p") {
                self.paragraphe(c);
            } else if c.is("tbl") {
                self.tableau(c);
            } else if c.is("sdt") || c.is("sdtContent") || c.is("customXml") {
                // Une table des matières de Word devient `\sommaire`.
                let toc = c
                    .child("sdtPr")
                    .and_then(|p| p.child("docPartObj"))
                    .and_then(|d| d.child("docPartGallery"))
                    .and_then(|g| g.attr("w:val"))
                    .is_some_and(|v| v.contains("Table of Contents"));
                if toc {
                    self.b.blocs.push(Bloc::Sommaire);
                } else {
                    self.corps(c, profondeur + 1);
                }
            } else if c.is("ins") || c.is("moveTo") {
                self.corps(c, profondeur + 1);
            }
        }
    }
}

/// Une formule OMML (Word) relue en LaTeX, au mieux : fractions, exposants, indices,
/// racines, délimiteurs, sommes et intégrales, fonctions, matrices.
fn omml_latex(n: &Node, profondeur: usize) -> String {
    if profondeur > 64 {
        return String::new();
    }
    let enfant = |nom: &str| {
        n.child(nom)
            .map(|c| omml_latex(c, profondeur + 1))
            .unwrap_or_default()
    };
    let groupe = |t: String| {
        if t.chars().count() == 1 {
            t
        } else {
            format!("{{{t}}}")
        }
    };
    let val = |pr: &str, k: &str| {
        n.child(pr)
            .and_then(|p| p.child(k))
            .and_then(|c| c.attr("m:val"))
            .map(str::to_string)
    };
    match n.name.rsplit_once(':').map_or(n.name.as_str(), |(_, l)| l) {
        "r" => n
            .children_named("t")
            .map(|t| tex_de_texte(&t.text()))
            .collect(),
        "f" => format!("\\frac{{{}}}{{{}}}", enfant("num"), enfant("den")),
        "sSup" => format!("{}^{}", groupe(enfant("e")), groupe(enfant("sup"))),
        "sSub" => format!("{}_{}", groupe(enfant("e")), groupe(enfant("sub"))),
        "sSubSup" => format!(
            "{}_{}^{}",
            groupe(enfant("e")),
            groupe(enfant("sub")),
            groupe(enfant("sup"))
        ),
        "rad" => {
            let deg = enfant("deg");
            if deg.trim().is_empty() {
                format!("\\sqrt{{{}}}", enfant("e"))
            } else {
                format!("\\sqrt[{deg}]{{{}}}", enfant("e"))
            }
        }
        "d" => {
            let ouv = val("dPr", "begChr").unwrap_or_else(|| "(".into());
            let fer = val("dPr", "endChr").unwrap_or_else(|| ")".into());
            let morceaux: Vec<String> = n
                .children_named("e")
                .map(|e| omml_latex(e, profondeur + 1))
                .collect();
            let echap = |c: &str| match c {
                "{" => "\\{".to_string(),
                "}" => "\\}".to_string(),
                "" => ".".to_string(),
                c => c.to_string(),
            };
            format!(
                "\\left{} {} \\right{}",
                echap(&ouv),
                morceaux.join(", "),
                echap(&fer)
            )
        }
        "nary" => {
            let op = match val("naryPr", "chr").as_deref() {
                Some("∑") => "\\sum",
                Some("∏") => "\\prod",
                Some("∬") => "\\iint",
                Some("∮") => "\\oint",
                None | Some("∫") => "\\int",
                Some(_) => "\\sum",
            };
            let (bas, haut) = (enfant("sub"), enfant("sup"));
            let mut s = op.to_string();
            if !bas.trim().is_empty() {
                s.push_str(&format!("_{}", groupe(bas)));
            }
            if !haut.trim().is_empty() {
                s.push_str(&format!("^{}", groupe(haut)));
            }
            format!("{s} {}", enfant("e"))
        }
        "func" => format!("{} {}", enfant("fName"), enfant("e")),
        "acc" => {
            let cmd = match val("accPr", "chr").as_deref() {
                Some("\u{305}" | "¯") => "\\bar",
                Some("\u{2192}" | "\u{20D7}") => "\\vec",
                Some("~" | "\u{303}") => "\\tilde",
                _ => "\\hat",
            };
            format!("{cmd}{{{}}}", enfant("e"))
        }
        "bar" => format!("\\overline{{{}}}", enfant("e")),
        "m" => {
            let lignes: Vec<String> = n
                .children_named("mr")
                .map(|r| {
                    r.children_named("e")
                        .map(|e| omml_latex(e, profondeur + 1))
                        .collect::<Vec<_>>()
                        .join(" & ")
                })
                .collect();
            format!(
                "\\begin{{matrix}} {} \\end{{matrix}}",
                lignes.join(" \\\\ ")
            )
        }
        _ => n
            .elems()
            .filter(|c| !c.name.ends_with("Pr"))
            .map(|c| omml_latex(c, profondeur + 1))
            .collect(),
    }
}

/// Le texte d'une formule en LaTeX : lettres grecques et opérateurs nommés.
fn tex_de_texte(t: &str) -> String {
    let mut s = String::new();
    for c in t.chars() {
        let nom = match c {
            '×' => "\\times ",
            '÷' => "\\div ",
            '·' | '⋅' => "\\cdot ",
            '≤' => "\\leq ",
            '≥' => "\\geq ",
            '≠' => "\\neq ",
            '±' => "\\pm ",
            '∞' => "\\infty ",
            '→' => "\\to ",
            'α' => "\\alpha ",
            'β' => "\\beta ",
            'γ' => "\\gamma ",
            'δ' => "\\delta ",
            'ε' => "\\epsilon ",
            'θ' => "\\theta ",
            'λ' => "\\lambda ",
            'μ' => "\\mu ",
            'π' => "\\pi ",
            'σ' => "\\sigma ",
            'φ' => "\\phi ",
            'ω' => "\\omega ",
            'Δ' => "\\Delta ",
            'Σ' => "\\Sigma ",
            'Ω' => "\\Omega ",
            '{' => "\\{",
            '}' => "\\}",
            '$' => "",
            '\\' => "\\backslash ",
            _ => {
                s.push(c);
                continue;
            }
        };
        s.push_str(nom);
    }
    s
}

/// `{>>Auteur · date<<}` d'une modification suivie de Word.
fn auteur_date(n: &Node) -> Option<String> {
    let a = n.attr("w:author").unwrap_or("").trim();
    let d: String = n.attr("w:date").unwrap_or("").chars().take(10).collect();
    crate::critic::signature(a, &d)
}

fn arbre(zip: &ZipReader, nom: &str) -> Result<Option<Node>, String> {
    match zip.text(nom) {
        Some(t) => xmlarbre::parse(&t)
            .map(Some)
            .map_err(|e| format!("{nom} : {e}")),
        None => Ok(None),
    }
}

/// Les relations d'une partie (`word/_rels/document.xml.rels`) : id → cible.
fn relations(zip: &ZipReader, partie: &str) -> HashMap<String, (String, String)> {
    let (dossier, fichier) = partie.rsplit_once('/').unwrap_or(("", partie));
    let nom = if dossier.is_empty() {
        format!("_rels/{fichier}.rels")
    } else {
        format!("{dossier}/_rels/{fichier}.rels")
    };
    let mut out = HashMap::new();
    if let Ok(Some(r)) = arbre(zip, &nom) {
        for rel in r.children_named("Relationship") {
            if let (Some(id), Some(cible)) = (rel.attr("Id"), rel.attr("Target")) {
                out.insert(
                    id.to_string(),
                    (
                        rel.attr("Type").unwrap_or("").to_string(),
                        cible.to_string(),
                    ),
                );
            }
        }
    }
    out
}

/// Chemin d'une cible relative à `word/`.
fn chemin(dossier: &str, cible: &str) -> String {
    match cible.strip_prefix('/') {
        Some(abs) => abs.to_string(),
        None if dossier.is_empty() => cible.to_string(),
        None => format!("{dossier}/{cible}"),
    }
}

/// Les éléments `nom` d'une partie, par identifiant (`w:id`).
fn collecter<'n>(racine: &'n Option<Node>, nom: &'static str) -> HashMap<String, &'n Node> {
    let mut m = HashMap::new();
    if let Some(r) = racine {
        for n in r.children_named(nom) {
            if let Some(id) = n.attr("w:id") {
                m.insert(id.to_string(), n);
            }
        }
    }
    m
}

/// Lit le document.
pub(crate) fn lire(zip: &ZipReader) -> Result<Brouillon, String> {
    // La partie principale (d'après `_rels/.rels`, sinon `word/document.xml`).
    let principale = relations(zip, "")
        .into_values()
        .find(|(t, _)| t.ends_with("/officeDocument"))
        .map(|(_, c)| c.trim_start_matches('/').to_string())
        .filter(|c| zip.get(c).is_some())
        .unwrap_or_else(|| "word/document.xml".to_string());
    let dossier = principale
        .rsplit_once('/')
        .map_or("", |(d, _)| d)
        .to_string();
    let document = arbre(zip, &principale)?.ok_or("document.xml manquant")?;
    let rels = relations(zip, &principale);
    let partie = |genre: &str, defaut: &str| {
        rels.values()
            .find(|(t, _)| t.ends_with(genre))
            .map(|(_, c)| chemin(&dossier, c))
            .unwrap_or_else(|| format!("{dossier}/{defaut}"))
    };
    let styles_xml = arbre(zip, &partie("/styles", "styles.xml")).unwrap_or(None);
    let numerotation = arbre(zip, &partie("/numbering", "numbering.xml")).unwrap_or(None);
    let notes_xml = arbre(zip, &partie("/footnotes", "footnotes.xml")).unwrap_or(None);
    let fins_xml = arbre(zip, &partie("/endnotes", "endnotes.xml")).unwrap_or(None);
    let comm_xml = arbre(zip, &partie("/comments", "comments.xml")).unwrap_or(None);

    let mut styles = Styles::default();
    if let Some(s) = &styles_xml {
        styles.lire(s);
    }
    if let Some(n) = &numerotation {
        styles.lire_numerotation(n);
    }
    let mut l = Lecteur {
        styles: &styles,
        liens: rels
            .iter()
            .filter(|(_, (t, _))| t.ends_with("/hyperlink"))
            .map(|(id, (_, c))| (id.clone(), c.clone()))
            .collect(),
        notes: collecter(&notes_xml, "footnote"),
        notes_fin: collecter(&fins_xml, "endnote"),
        commentaires: collecter(&comm_xml, "comment"),
        b: Brouillon::default(),
        compteurs: HashMap::new(),
        dans_une_note: false,
    };

    // Langue, police et taille par défaut.
    if let Some(rpr) = styles_xml
        .as_ref()
        .and_then(|s| s.child("docDefaults"))
        .and_then(|d| d.child("rPrDefault"))
        .and_then(|d| d.child("rPr"))
    {
        if let Some(lg) = rpr
            .child("lang")
            .and_then(|x| x.attr("w:val"))
            .and_then(import::langue)
        {
            l.b.page.langue = lg;
        }
        if let Some(f) = rpr.child("rFonts") {
            if let Some(nom) = f.attr("w:ascii").or_else(|| f.attr("w:hAnsi")) {
                l.b.page.font = import::police(nom);
            } else if f.attr("w:asciiTheme").is_some() {
                // Police du thème (Calibri, Aptos) : sans empattement.
                l.b.page.font = crate::page::DocFont::Sans;
            }
        }
        if let Some(t) = rpr
            .child("sz")
            .and_then(|x| x.attr("w:val"))
            .and_then(|v| v.parse::<f32>().ok())
            .and_then(|v| import::taille(v / 2.0))
        {
            l.b.page.font_size = t;
        }
    }
    if let Some(core) = arbre(zip, "docProps/core.xml").unwrap_or(None) {
        l.b.titre = core.child("title").map(|t| t.text()).unwrap_or_default();
        if styles_xml.is_none()
            && let Some(lg) = core
                .child("language")
                .and_then(|t| import::langue(&t.text()))
        {
            l.b.page.langue = lg;
        }
    }

    let Some(corps) = document.child("body") else {
        return Ok(l.b);
    };
    l.corps(corps, 0);

    // Mise en page : la dernière section.
    let sect = corps.child("sectPr").or_else(|| {
        corps
            .children_named("p")
            .filter_map(|p| p.child("pPr").and_then(|x| x.child("sectPr")))
            .last()
    });
    if let Some(s) = sect {
        let tw = |n: Option<&Node>, k: &str| {
            n.and_then(|x| x.attr(k))
                .and_then(|v| v.parse::<f32>().ok())
                .map(|v| v / 56.692_9)
        };
        let sz = s.child("pgSz");
        if let (Some(w), Some(h)) = (tw(sz, "w:w"), tw(sz, "w:h"))
            && let Some((papier, paysage)) = import::papier(w, h)
        {
            l.b.page.paper = papier;
            l.b.page.landscape =
                paysage || sz.and_then(|x| x.attr("w:orient")) == Some("landscape");
        }
        let mar = s.child("pgMar");
        if let Some(m) = tw(mar, "w:top").map(f32::abs).and_then(import::marge) {
            l.b.page.margin_top = m;
        }
        if let Some(m) = tw(mar, "w:bottom").map(f32::abs).and_then(import::marge) {
            l.b.page.margin_bottom = m;
        }
        if let Some(m) = tw(mar, "w:left").and_then(import::marge) {
            l.b.page.margin_left = m;
        }
        if let Some(m) = tw(mar, "w:right").and_then(import::marge) {
            l.b.page.margin_right = m;
        }
        // En-tête et pied de page par défaut.
        for (balise, entete) in [("headerReference", true), ("footerReference", false)] {
            let Some(r) = s
                .children_named(balise)
                .find(|r| r.attr("w:type").is_none_or(|t| t == "default"))
            else {
                continue;
            };
            let Some((_, cible)) = r.attr("r:id").and_then(|id| rels.get(id)) else {
                continue;
            };
            let Ok(Some(x)) = arbre(zip, &chemin(&dossier, cible)) else {
                continue;
            };
            let mut segs = Vec::new();
            for p in x.elems().filter(|p| p.is("p")) {
                if !segs.is_empty() {
                    segs.push(Seg {
                        texte: " ".into(),
                        ..Seg::default()
                    });
                }
                l.runs(p, &Forme::default(), &mut segs, 0);
            }
            segs.retain(|s| !(s.brut && s.texte == SAUT));
            let (texte, numeros) = import::pied(segs);
            if entete {
                l.b.page.header = texte;
            } else {
                l.b.page.footer = texte;
            }
            l.b.page.page_numbers |= numeros;
        }
    }
    Ok(l.b)
}
