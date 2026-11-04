//! Lecture d'un document OpenDocument texte (`.odt`, LibreOffice Writer) : voir
//! `import`.

use std::collections::HashMap;

use crate::doc::Align;
use crate::import::{self, Bloc, Brouillon, Genre, Seg};
use crate::xmlarbre::{self, Child, Node};
use crate::zip::ZipReader;

/// Ce qu'un style ODF apporte (les valeurs absentes viennent du style parent).
#[derive(Debug, Clone, Default)]
struct StyleOdt {
    parent: Option<String>,
    gras: Option<bool>,
    italique: Option<bool>,
    souligne: Option<bool>,
    barre: Option<bool>,
    /// `Some(None)` : noir ou transparent, qui annule le style parent.
    couleur: Option<Option<String>>,
    fond: Option<Option<String>>,
    align: Option<Align>,
    saut_avant: bool,
    saut_apres: bool,
    retrait: Option<f32>,
    bordure_bas: bool,
    titre: Option<u8>,
    citation: bool,
}

/// Les styles connus, par famille puis par nom.
#[derive(Default)]
struct Styles {
    styles: HashMap<(String, String), StyleOdt>,
    /// Pour chaque style de liste : niveau (1 à 10) numéroté ?
    listes: HashMap<String, Vec<(u8, bool)>>,
    /// Page maîtresse demandée par un style de paragraphe ou de tableau.
    maitres: HashMap<String, String>,
}

/// Le style résolu (parents compris).
#[derive(Debug, Clone, Default)]
struct Resolu {
    gras: Option<bool>,
    italique: Option<bool>,
    souligne: Option<bool>,
    barre: Option<bool>,
    couleur: Option<Option<String>>,
    fond: Option<Option<String>>,
    align: Option<Align>,
    saut_avant: bool,
    saut_apres: bool,
    retrait: f32,
    bordure_bas: bool,
    titre: Option<u8>,
    citation: bool,
}

fn vrai_gras(v: &str) -> bool {
    v == "bold" || v.parse::<u32>().is_ok_and(|n| n >= 600)
}

fn alignement(v: &str) -> Option<Align> {
    match v {
        "center" => Some(Align::Center),
        "end" | "right" => Some(Align::Right),
        "justify" => Some(Align::Justify),
        "start" | "left" => Some(Align::Left),
        _ => None,
    }
}

/// Le niveau de titre que suggère le nom d'un style (`Heading_20_2`, « Titre 1 »).
fn niveau_par_nom(nom: &str) -> Option<u8> {
    let n = nom.replace("_20_", " ").to_lowercase();
    if n == "title" || n == "titre" {
        return Some(1);
    }
    if n == "subtitle" || n == "sous-titre" {
        return Some(2);
    }
    for p in [
        "heading ",
        "titre ",
        "überschrift ",
        "título ",
        "titolo ",
        "encabezado ",
    ] {
        if let Some(r) = n.strip_prefix(p) {
            return r.trim().parse::<u8>().ok().filter(|l| (1..=9).contains(l));
        }
    }
    None
}

fn est_citation(nom: &str) -> bool {
    let n = nom.replace("_20_", " ").to_lowercase();
    matches!(
        n.as_str(),
        "quotations" | "quote" | "citation" | "citations"
    )
}

impl Styles {
    fn lire(&mut self, conteneur: &Node) {
        for s in conteneur.elems() {
            if s.is("style") || s.is("default-style") {
                let famille = s.attr("style:family").unwrap_or("").to_string();
                let nom = if s.is("default-style") {
                    "\u{0}defaut".to_string()
                } else {
                    s.attr("style:name").unwrap_or("").to_string()
                };
                let mut st = StyleOdt {
                    parent: s.attr("style:parent-style-name").map(str::to_string),
                    ..StyleOdt::default()
                };
                let affiche = s.attr("style:display-name").unwrap_or(&nom).to_string();
                if let Some(m) = s.attr("style:master-page-name").filter(|m| !m.is_empty()) {
                    self.maitres.insert(nom.clone(), m.to_string());
                }
                if let Some(l) = s
                    .attr("style:default-outline-level")
                    .and_then(|v| v.parse::<u8>().ok())
                    .filter(|l| *l >= 1)
                {
                    st.titre = Some(l);
                } else {
                    st.titre = niveau_par_nom(&nom).or_else(|| niveau_par_nom(&affiche));
                }
                st.citation = est_citation(&nom) || est_citation(&affiche);
                if let Some(t) = s.child("text-properties") {
                    st.gras = t.attr("fo:font-weight").map(vrai_gras);
                    st.italique = t
                        .attr("fo:font-style")
                        .map(|v| v == "italic" || v == "oblique");
                    st.souligne = t.attr("style:text-underline-style").map(|v| v != "none");
                    st.barre = t.attr("style:text-line-through-style").map(|v| v != "none");
                    st.couleur = t.attr("fo:color").map(import::couleur_hex);
                    st.fond = t.attr("fo:background-color").map(import::couleur_hex);
                }
                if let Some(p) = s.child("paragraph-properties") {
                    st.align = p.attr("fo:text-align").and_then(alignement);
                    st.saut_avant = p.attr("fo:break-before") == Some("page");
                    st.saut_apres = p.attr("fo:break-after") == Some("page");
                    st.retrait = p.attr("fo:margin-left").and_then(import::longueur_mm);
                    st.bordure_bas = p
                        .attr("fo:border-bottom")
                        .or_else(|| p.attr("fo:border"))
                        .is_some_and(|b| b != "none" && !b.is_empty());
                }
                self.styles.insert((famille, nom), st);
            } else if s.is("list-style") {
                let nom = s.attr("style:name").unwrap_or("").to_string();
                let niveaux = s
                    .elems()
                    .filter_map(|l| {
                        let n = l.attr("text:level")?.parse::<u8>().ok()?;
                        Some((n, l.is("list-level-style-number")))
                    })
                    .collect();
                self.listes.insert(nom, niveaux);
            }
        }
    }

    fn resoudre(&self, famille: &str, nom: Option<&str>) -> Resolu {
        let mut r = Resolu::default();
        let (mut gras, mut ital, mut soul, mut barre) = (None, None, None, None);
        let (mut couleur, mut fond) = (None, None);
        let mut align = None;
        let mut retrait = None;
        let mut titre = None;
        let mut courant = nom.map(str::to_string);
        let mut premier = true;
        for _ in 0..24 {
            let Some(n) = courant.take() else { break };
            let Some(st) = self.styles.get(&(famille.to_string(), n)) else {
                break;
            };
            gras = gras.or(st.gras);
            ital = ital.or(st.italique);
            soul = soul.or(st.souligne);
            barre = barre.or(st.barre);
            couleur = couleur.or(st.couleur.clone());
            fond = fond.or(st.fond.clone());
            align = align.or(st.align);
            retrait = retrait.or(st.retrait);
            titre = titre.or(st.titre);
            r.citation |= st.citation;
            if premier {
                // Sauts et filets ne s'héritent pas d'un style de base.
                r.saut_avant = st.saut_avant;
                r.saut_apres = st.saut_apres;
                r.bordure_bas = st.bordure_bas;
                premier = false;
            }
            courant = st.parent.clone();
        }
        r.gras = gras;
        r.italique = ital;
        r.souligne = soul;
        r.barre = barre;
        r.couleur = couleur;
        r.fond = fond;
        r.align = align;
        r.retrait = retrait.unwrap_or(0.0);
        r.titre = titre;
        r
    }

    fn numerotee(&self, liste: &str, niveau: u8) -> bool {
        self.listes.get(liste).is_some_and(|v| {
            v.iter()
                .find(|(n, _)| *n == niveau)
                .is_some_and(|(_, num)| *num)
        })
    }
}

/// La forme des caractères en cours.
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

    /// La forme héritée, modifiée par ce que précise le style.
    fn avec(&self, r: &Resolu) -> Forme {
        Forme {
            gras: r.gras.unwrap_or(self.gras),
            italique: r.italique.unwrap_or(self.italique),
            souligne: r.souligne.unwrap_or(self.souligne),
            barre: r.barre.unwrap_or(self.barre),
            lien: self.lien.clone(),
            couleur: r.couleur.clone().unwrap_or_else(|| self.couleur.clone()),
            fond: r.fond.clone().unwrap_or_else(|| self.fond.clone()),
        }
    }
}

use crate::import::MARQUE_PAGE as PAGE;

struct Lecteur<'a> {
    styles: &'a Styles,
    b: Brouillon,
    /// Compteurs des listes numérotées, par niveau.
    compteurs: Vec<u32>,
}

impl Lecteur<'_> {
    /// Le texte d'un paragraphe, mis en forme.
    fn en_ligne(&mut self, n: &Node, forme: &Forme, out: &mut Vec<Seg>, profondeur: usize) {
        if profondeur > 200 {
            return;
        }
        for c in &n.children {
            match c {
                Child::Text(t) => {
                    // Les espaces ODF se replient (text:s les rend explicites).
                    let mut s = String::with_capacity(t.len());
                    let mut espace = out.last().is_some_and(|x| x.texte.ends_with(' '));
                    for ch in t.chars() {
                        if ch == ' ' || ch == '\n' || ch == '\r' || ch == '\t' {
                            if !espace {
                                s.push(' ');
                            }
                            espace = true;
                        } else {
                            s.push(ch);
                            espace = false;
                        }
                    }
                    if !s.is_empty() {
                        out.push(forme.seg(&s));
                    }
                }
                Child::Elem(e) => self.element(e, forme, out, profondeur),
            }
        }
    }

    fn element(&mut self, e: &Node, forme: &Forme, out: &mut Vec<Seg>, profondeur: usize) {
        if e.is("span") {
            let r = self.styles.resoudre("text", e.attr("text:style-name"));
            self.en_ligne(e, &forme.avec(&r), out, profondeur + 1);
        } else if e.is("a") {
            let mut f = forme.clone();
            f.lien = e.attr("xlink:href").map(str::to_string);
            self.en_ligne(e, &f, out, profondeur + 1);
        } else if e.is("s") {
            let n = e
                .attr("text:c")
                .and_then(|c| c.parse::<usize>().ok())
                .unwrap_or(1)
                .clamp(1, 64);
            out.push(forme.seg(&" ".repeat(n)));
        } else if e.is("tab") {
            out.push(forme.seg("\t"));
        } else if e.is("line-break") {
            out.push(forme.seg("\n"));
        } else if e.is("note") {
            let mut segs = Vec::new();
            if let Some(corps) = e.child("note-body") {
                for p in corps.elems() {
                    if !segs.is_empty() {
                        segs.push(Seg {
                            texte: " ".into(),
                            ..Seg::default()
                        });
                    }
                    self.en_ligne(p, &Forme::default(), &mut segs, profondeur + 1);
                }
            }
            for s in &mut segs {
                if !s.brut {
                    s.texte = s.texte.replace('\n', " ");
                }
            }
            let m = self.b.note(&segs);
            out.push(m);
        } else if e.is("annotation") {
            let auteur = e.child("creator").map(|n| n.text()).unwrap_or_default();
            let date = e
                .child("date")
                .map(|n| n.text().chars().take(10).collect::<String>())
                .unwrap_or_default();
            let texte: Vec<String> = e
                .children_named("p")
                .map(|p| p.text().trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            let texte = texte.join(" ");
            if !texte.is_empty() {
                out.push(Seg::brut(crate::critic::commentaire(
                    &auteur, &date, &texte,
                )));
            }
        } else if e.is("frame") {
            self.cadre(e, forme, out, profondeur);
        } else if e.is("page-number") {
            out.push(Seg::brut(PAGE));
        } else if !(e.is("note-citation")
            || e.is("bookmark")
            || e.is("bookmark-start")
            || e.is("bookmark-end")
            || e.is("soft-page-break")
            || e.is("annotation-end")
            || e.is("change-start")
            || e.is("change-end")
            || e.is("change")
            || e.is("sequence-decls")
            || e.name.starts_with("office:forms"))
        {
            // Autres éléments (champs, formes…) : leur texte.
            self.en_ligne(e, forme, out, profondeur + 1);
        }
    }

    /// Un cadre : code QR de Varium, image (non importée) ou zone de texte.
    fn cadre(&mut self, e: &Node, forme: &Forme, out: &mut Vec<Seg>, profondeur: usize) {
        // Formule de Varium : objet LibreOffice Math, la source LaTeX dans svg:desc.
        let desc = e.child("desc").map(|d| d.text()).unwrap_or_default();
        if e.child("object").is_some() && !desc.trim().is_empty() && !desc.contains('$') {
            out.push(Seg::brut(format!("${}$", desc.trim())));
            return;
        }
        if e.child("image").is_some() {
            let titre = e.child("title").map(|t| t.text()).unwrap_or_default();
            let nom = e.attr("draw:name").unwrap_or("");
            if nom.starts_with("QR") && !titre.is_empty() && !titre.contains("}}") {
                out.push(Seg::brut(format!("{{{{qr:{}}}}}", titre.trim())));
            } else {
                let m = self.b.image();
                out.push(m);
            }
        } else if let Some(tb) = e.child("text-box") {
            for p in tb.elems() {
                self.en_ligne(p, forme, out, profondeur + 1);
                out.push(forme.seg(" "));
            }
        } else {
            let m = self.b.image();
            out.push(m);
        }
    }

    fn paragraphe(&mut self, p: &Node, titre: Option<u8>, item: Option<Genre>) {
        let r = self.styles.resoudre("paragraph", p.attr("text:style-name"));
        if r.saut_avant {
            self.b.saut();
        }
        let base = Forme::default().avec(&r);
        let mut segs = Vec::new();
        self.en_ligne(p, &base, &mut segs, 0);
        // Une formule seule dans son paragraphe : formule centrée.
        let pleins: Vec<usize> = (0..segs.len())
            .filter(|&i| !segs[i].texte.trim().is_empty())
            .collect();
        if let [i] = pleins[..]
            && segs[i].brut
            && segs[i].texte.starts_with('$')
            && !segs[i].texte.starts_with("$$")
        {
            let t = segs[i].texte.trim_matches('$').to_string();
            segs = vec![Seg::brut(format!("$${t}$$"))];
        }
        let vide = import::texte_de(&segs).trim().is_empty();
        let genre = if let Some(g) = item {
            g
        } else if let Some(l) = titre.or(r.titre) {
            Genre::Titre(l.min(3))
        } else if r.citation {
            Genre::Citation
        } else {
            Genre::Corps
        };
        if vide && r.bordure_bas && genre == Genre::Corps && segs.iter().all(|s| !s.brut) {
            self.b.blocs.push(Bloc::Filet);
        } else if !(vide && (r.saut_avant || r.saut_apres) && segs.iter().all(|s| !s.brut)) {
            self.b
                .paragraphe(genre, r.align.unwrap_or(Align::Left), segs, r.retrait);
        }
        if r.saut_apres {
            self.b.saut_en_attente = true;
        }
    }

    fn liste(&mut self, l: &Node, profondeur: u8, style: Option<&str>) {
        let style = l.attr("text:style-name").or(style).map(str::to_string);
        if self.compteurs.len() <= profondeur as usize {
            self.compteurs.resize(profondeur as usize + 1, 0);
        }
        if l.attr("text:continue-numbering") != Some("true")
            && l.attr("text:continue-list").is_none()
        {
            self.compteurs[profondeur as usize] = 0;
        }
        for it in l.elems() {
            if !(it.is("list-item") || it.is("list-header")) {
                continue;
            }
            let mut premier = true;
            for c in it.elems() {
                if c.is("list") {
                    self.liste(c, (profondeur + 1).min(5), style.as_deref());
                } else if c.is("p") || c.is("h") {
                    let item = if premier && it.is("list-item") {
                        premier = false;
                        let numero = style
                            .as_deref()
                            .is_some_and(|s| self.styles.numerotee(s, profondeur + 1))
                            .then(|| {
                                let k = &mut self.compteurs[profondeur as usize];
                                *k = it
                                    .attr("text:start-value")
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(*k + 1);
                                *k
                            });
                        // Les niveaux plus profonds recommencent.
                        for k in self.compteurs.iter_mut().skip(profondeur as usize + 1) {
                            *k = 0;
                        }
                        Some(Genre::Item { profondeur, numero })
                    } else {
                        Some(Genre::Corps)
                    };
                    self.paragraphe(c, None, item);
                } else if c.is("table") {
                    self.tableau(c);
                }
            }
        }
    }

    fn cellule(&mut self, c: &Node) -> (Vec<Seg>, Option<Align>) {
        let mut segs = Vec::new();
        let mut align = None;
        for p in c.elems() {
            if p.is("p") || p.is("h") {
                let r = self.styles.resoudre("paragraph", p.attr("text:style-name"));
                align = align.or(r.align);
                if !segs.is_empty() {
                    segs.push(Seg {
                        texte: " ".into(),
                        ..Seg::default()
                    });
                }
                self.en_ligne(p, &Forme::default().avec(&r), &mut segs, 0);
            } else if p.is("list") {
                segs.push(Seg {
                    texte: format!(" {}", p.text()),
                    ..Seg::default()
                });
            }
        }
        for s in &mut segs {
            if !s.brut {
                s.texte = s.texte.replace(['\n', '\t'], " ");
            }
        }
        (segs, align)
    }

    fn tableau(&mut self, t: &Node) {
        let mut lignes: Vec<Vec<Vec<Seg>>> = Vec::new();
        let mut aligns: Vec<Option<Align>> = Vec::new();
        let mut entete = false;
        let mut rangs: Vec<(&Node, bool)> = Vec::new();
        for c in t.elems() {
            if c.is("table-header-rows") {
                entete = true;
                rangs.extend(c.children_named("table-row").map(|r| (r, true)));
            } else if c.is("table-row") {
                rangs.push((c, false));
            } else if c.is("table-rows") {
                rangs.extend(c.children_named("table-row").map(|r| (r, false)));
            }
        }
        // L'en-tête est toujours la première ligne.
        rangs.sort_by_key(|(_, h)| !*h);
        for (rang, _) in rangs.iter().take(10_000) {
            let mut l = Vec::new();
            for c in rang.elems() {
                if !(c.is("table-cell") || c.is("covered-table-cell")) {
                    continue;
                }
                let rep = c
                    .attr("table:number-columns-repeated")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(1)
                    .clamp(1, 64);
                let (segs, a) = if c.is("covered-table-cell") {
                    (Vec::new(), None)
                } else {
                    self.cellule(c)
                };
                for _ in 0..rep {
                    if l.len() >= 64 {
                        break;
                    }
                    if lignes.len() == usize::from(entete) {
                        if aligns.len() <= l.len() {
                            aligns.resize(l.len() + 1, None);
                        }
                        aligns[l.len()] = aligns[l.len()].or(a);
                    }
                    l.push(segs.clone());
                }
            }
            lignes.push(l);
        }
        if std::mem::take(&mut self.b.saut_en_attente) {
            self.b.blocs.push(Bloc::Saut);
        }
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
                self.paragraphe(c, None, None);
            } else if c.is("h") {
                let l = c
                    .attr("text:outline-level")
                    .and_then(|v| v.parse::<u8>().ok())
                    .unwrap_or(1)
                    .clamp(1, 9);
                self.paragraphe(c, Some(l), None);
            } else if c.is("list") {
                self.compteurs.clear();
                self.liste(c, 0, None);
            } else if c.is("table") {
                self.tableau(c);
            } else if c.is("table-of-content") {
                self.b.blocs.push(Bloc::Sommaire);
            } else if c.is("section") || c.is("index-body") || c.is("text") {
                self.corps(c, profondeur + 1);
            } else if c.is("frame") {
                let mut segs = Vec::new();
                self.cadre(c, &Forme::default(), &mut segs, 0);
                self.b.paragraphe(Genre::Corps, Align::Left, segs, 0.0);
            }
        }
    }
}

fn arbre(zip: &ZipReader, nom: &str) -> Result<Option<Node>, String> {
    match zip.text(nom) {
        Some(t) => xmlarbre::parse(&t)
            .map(Some)
            .map_err(|e| format!("{nom} : {e}")),
        None => Ok(None),
    }
}

/// Le texte Varium d'un en-tête ou d'un pied de page, et la présence d'un numéro de page.
fn bandeau(styles: &Styles, n: &Node) -> (String, bool) {
    let mut l = Lecteur {
        styles,
        b: Brouillon::default(),
        compteurs: Vec::new(),
    };
    let mut segs = Vec::new();
    for p in n.elems() {
        if !segs.is_empty() {
            segs.push(Seg {
                texte: " ".into(),
                ..Seg::default()
            });
        }
        l.en_ligne(p, &Forme::default(), &mut segs, 0);
    }
    import::pied(segs)
}

/// Lit le document.
pub(crate) fn lire(zip: &ZipReader) -> Result<Brouillon, String> {
    let contenu = arbre(zip, "content.xml")?.ok_or("content.xml manquant")?;
    let styles_xml = arbre(zip, "styles.xml").unwrap_or(None);
    let mut styles = Styles::default();
    if let Some(s) = &styles_xml {
        for c in s.elems() {
            if c.is("styles") || c.is("automatic-styles") {
                styles.lire(c);
            }
        }
    }
    // Les styles automatiques du contenu ont priorité sur ceux de styles.xml.
    for c in contenu.elems() {
        if c.is("automatic-styles") || c.is("styles") {
            styles.lire(c);
        }
    }
    let mut l = Lecteur {
        styles: &styles,
        b: Brouillon::default(),
        compteurs: Vec::new(),
    };
    // Mise en page, langue, police, en-tête et pied de page.
    // La page maîtresse du début du document (sinon « Standard »).
    let maitre = contenu
        .child("body")
        .and_then(|b| b.child("text"))
        .and_then(|t| {
            t.elems()
                .filter_map(|e| {
                    e.attr("text:style-name")
                        .or_else(|| e.attr("table:style-name"))
                })
                .take(3)
                .find_map(|n| styles.maitres.get(n).cloned())
        });
    if let Some(s) = &styles_xml {
        mise_en_page(s, &styles, &mut l.b, maitre.as_deref());
    }
    if let Some(m) = arbre(zip, "meta.xml").unwrap_or(None)
        && let Some(meta) = m.child("meta")
    {
        l.b.titre = meta.child("title").map(|t| t.text()).unwrap_or_default();
        if let Some(lg) = meta
            .child("language")
            .and_then(|t| import::langue(&t.text()))
            && l.b.page.langue == Default::default()
        {
            l.b.page.langue = lg;
        }
    }
    if let Some(corps) = contenu.child("body") {
        for t in corps.elems() {
            if t.is("text") {
                l.corps(t, 0);
            }
        }
    }
    Ok(l.b)
}

fn mise_en_page(s: &Node, styles: &Styles, b: &mut Brouillon, maitre: Option<&str>) {
    // Langue, police et taille par défaut.
    let mut lang = None;
    let mut police = None;
    let mut taille = None;
    if let Some(st) = s.child("styles") {
        for d in st.elems() {
            let defaut = d.is("default-style") && d.attr("style:family") == Some("paragraph");
            let standard = d.is("style") && d.attr("style:name") == Some("Standard");
            if !(defaut || standard) {
                continue;
            }
            if let Some(t) = d.child("text-properties") {
                if let Some(lg) = t.attr("fo:language").and_then(import::langue) {
                    lang = Some(lg);
                }
                if let Some(f) = t.attr("style:font-name") {
                    police = Some(import::police(f));
                }
                if let Some(v) = t.attr("fo:font-size") {
                    let pt = import::longueur_mm(v).map(|mm| mm * 72.0 / 25.4);
                    taille = pt.and_then(import::taille).or(taille);
                }
            }
        }
    }
    if let Some(lg) = lang {
        b.page.langue = lg;
    }
    if let Some(p) = police {
        b.page.font = p;
    }
    if let Some(t) = taille {
        b.page.font_size = t;
    }
    // Page maîtresse « Standard » (sinon la première) et sa mise en page.
    let Some(maitres) = s.child("master-styles") else {
        return;
    };
    let pages: Vec<&Node> = maitres.children_named("master-page").collect();
    let Some(mp) = pages
        .iter()
        .find(|m| maitre.is_some() && m.attr("style:name") == maitre)
        .or_else(|| {
            pages
                .iter()
                .find(|m| m.attr("style:name") == Some("Standard"))
        })
        .or(pages.first())
    else {
        return;
    };
    if let Some(nom) = mp.attr("style:page-layout-name") {
        let props = s
            .elems()
            .filter(|c| c.is("automatic-styles") || c.is("styles"))
            .flat_map(|c| c.children_named("page-layout"))
            .find(|pl| pl.attr("style:name") == Some(nom))
            .and_then(|pl| pl.child("page-layout-properties"));
        if let Some(p) = props {
            let mesure = |k: &str| p.attr(k).and_then(import::longueur_mm);
            if let (Some(w), Some(h)) = (mesure("fo:page-width"), mesure("fo:page-height"))
                && let Some((papier, paysage)) = import::papier(w, h)
            {
                b.page.paper = papier;
                b.page.landscape = paysage;
            }
            if let Some(m) = mesure("fo:margin-top").and_then(import::marge) {
                b.page.margin_top = m;
            }
            if let Some(m) = mesure("fo:margin-bottom").and_then(import::marge) {
                b.page.margin_bottom = m;
            }
            if let Some(m) = mesure("fo:margin-left").and_then(import::marge) {
                b.page.margin_left = m;
            }
            if let Some(m) = mesure("fo:margin-right").and_then(import::marge) {
                b.page.margin_right = m;
            }
        }
    }
    if let Some(h) = mp.child("header") {
        let (t, n) = bandeau(styles, h);
        b.page.header = t;
        b.page.page_numbers |= n;
    }
    if let Some(f) = mp.child("footer") {
        let (t, n) = bandeau(styles, f);
        b.page.footer = t;
        b.page.page_numbers |= n;
    }
}
