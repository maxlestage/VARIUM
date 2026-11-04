//! Collage intelligent : le texte copié ailleurs (navigateur, Word, LibreOffice,
//! tableur, Markdown) devient du texte Varium avec sa mise en forme.
//!
//! - HTML : titres, gras, italique, souligné, barré, listes (imbriquées, numérotées),
//!   liens, tableaux, citations, lignes horizontales ; styles CSS en ligne compris
//!   (`font-weight`, `font-style`, `text-decoration`, listes de Word `mso-list`).
//! - Markdown : `__gras__` et `_italique_` (qui ont un autre sens dans Varium), images,
//!   code ; le reste est déjà la syntaxe de Varium.
//! - Cellules d'un tableur (tabulations) ou CSV : un tableau `| … |`.
//! - Une adresse web collée sur une sélection : un lien `[sélection](adresse)`.
//!
//! Le texte collé ne crée jamais de champ : `{{…}}` et `$` sont échappés.

/// Ce qui a été reconnu dans le presse-papiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Genre {
    /// Texte ordinaire, collé tel quel.
    Texte,
    /// Page web ou traitement de texte (HTML).
    Html,
    Markdown,
    /// Cellules de tableur ou CSV.
    Tableau,
    /// Adresse collée sur une sélection.
    Lien,
}

/// Le texte à insérer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collage {
    pub texte: String,
    pub genre: Genre,
}

/// Décide comment coller `texte` (et sa version `html` si le presse-papiers en a une)
/// à la place de `selection`.
pub fn coller(texte: &str, html: Option<&str>, selection: &str) -> Collage {
    let texte = texte.replace("\r\n", "\n").replace('\r', "\n");
    let fait = |texte: String, genre| Collage { texte, genre };
    let sel = selection.trim();
    if !sel.is_empty() && !sel.contains('\n') && est_url(texte.trim()) {
        let label = sel.replace(['[', ']'], "");
        return fait(format!("[{label}]({})", texte.trim()), Genre::Lien);
    }
    if let Some(h) = html.filter(|h| !h.trim().is_empty()) {
        let v = html_vers_varium(h);
        if !v.trim().is_empty() {
            return fait(v, Genre::Html);
        }
    }
    if let Some(t) = tableau(&texte) {
        return fait(t, Genre::Tableau);
    }
    if semble_html(&texte) {
        let v = html_vers_varium(&texte);
        if !v.trim().is_empty() {
            return fait(v, Genre::Html);
        }
    }
    if semble_markdown(&texte) {
        return fait(markdown_vers_varium(&texte), Genre::Markdown);
    }
    fait(texte, Genre::Texte)
}

/// Une adresse web ou électronique seule.
pub fn est_url(s: &str) -> bool {
    !s.is_empty()
        && !s.contains(char::is_whitespace)
        && ["https://", "http://", "www.", "mailto:", "ftp://"]
            .iter()
            .any(|p| s.len() > p.len() + 1 && s.to_ascii_lowercase().starts_with(p))
}

// ---------------------------------------------------------------------------------
// Échappement
// ---------------------------------------------------------------------------------

/// Échappe ce qui serait pris pour de la syntaxe Varium dans un texte en ligne.
fn echapper(s: &str, tableau: bool) -> String {
    let mut out = String::with_capacity(s.len());
    let cs: Vec<char> = s.chars().collect();
    for (i, &c) in cs.iter().enumerate() {
        match c {
            '*' | '_' | '~' => {
                out.push('\\');
                out.push(c);
            }
            '|' if tableau => out.push_str("\\|"),
            '$' => out.push_str("\\$"),
            '{' if cs.get(i + 1) == Some(&'{') && (i == 0 || cs[i - 1] != '{') => {
                out.push_str("\\{");
            }
            '\n' if tableau => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

/// Échappe un début de ligne qui serait pris pour un titre, une citation ou un
/// tableau (une liste reste une liste : c'est ce qu'on attend en collant).
fn echapper_debut(ligne: &str) -> String {
    let t = ligne.trim_start();
    if t.starts_with('#') || t.starts_with('>') || t.starts_with('|') {
        format!("\\{t}")
    } else {
        t.to_string()
    }
}

// ---------------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Style {
    gras: bool,
    italique: bool,
    souligne: bool,
    barre: bool,
}

/// Un morceau de texte d'un bloc.
#[derive(Debug, Clone)]
struct Seg {
    texte: String,
    style: Style,
    lien: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum Bloc {
    Para,
    Titre(u8),
    Item {
        profondeur: usize,
        numero: Option<u32>,
    },
    Citation,
    Pre,
}

/// Élément HTML ouvert.
#[derive(Debug, Clone)]
struct Ouvert {
    nom: String,
    style: Style,
    lien: Option<String>,
    /// Contenu ignoré (`mso-list:Ignore`, `<head>`…).
    ignore: bool,
}

#[derive(Debug, Clone)]
struct Liste {
    numerotee: bool,
    compteur: u32,
}

/// Une ligne produite et son groupe (les lignes d'un même groupe se suivent sans
/// ligne vide : éléments d'une liste, lignes d'un tableau).
struct Sortie {
    lignes: Vec<(String, u32)>,
    /// Numéro du groupe en cours (liste ou tableau).
    groupe: u32,
}

/// Une ligne seule (paragraphe, titre…), séparée des autres par une ligne vide.
const G_SEUL: u32 = 0;
/// Une ligne que la suivante suit sans ligne vide (`<br>`, `<pre>`).
const G_BR: u32 = 1;

impl Sortie {
    fn nouveau_groupe(&mut self) -> u32 {
        self.groupe = self.groupe.max(1) + 1;
        self.groupe
    }

    fn texte(&self) -> String {
        let mut out = String::new();
        let mut prec: Option<u32> = None;
        for (l, g) in &self.lignes {
            if l.trim().is_empty() {
                continue;
            }
            if let Some(p) = prec {
                out.push('\n');
                let colle = p == G_BR || (p == *g && *g != G_SEUL);
                if !colle {
                    out.push('\n');
                }
            }
            out.push_str(l);
            prec = Some(*g);
        }
        out
    }
}

/// Décode les entités HTML usuelles.
fn entites(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut reste = s;
    while let Some(i) = reste.find('&') {
        out.push_str(&reste[..i]);
        let apres = &reste[i + 1..];
        let fin = apres
            .char_indices()
            .take(12)
            .find(|(_, c)| *c == ';')
            .map(|(j, _)| j);
        let decode = fin.and_then(|j| {
            let nom = &apres[..j];
            let c = if let Some(n) = nom.strip_prefix("#x").or_else(|| nom.strip_prefix("#X")) {
                u32::from_str_radix(n, 16).ok().and_then(char::from_u32)
            } else if let Some(n) = nom.strip_prefix('#') {
                n.parse::<u32>().ok().and_then(char::from_u32)
            } else {
                ENTITES.iter().find(|(k, _)| *k == nom).map(|(_, c)| *c)
            };
            c.map(|c| (c, j))
        });
        match decode {
            Some((c, j)) => {
                out.push(if c == '\0' { '\u{FFFD}' } else { c });
                reste = &apres[j + 1..];
            }
            None => {
                out.push('&');
                reste = apres;
            }
        }
    }
    out.push_str(reste);
    out
}

const ENTITES: &[(&str, char)] = &[
    ("amp", '&'),
    ("lt", '<'),
    ("gt", '>'),
    ("quot", '"'),
    ("apos", '\''),
    ("nbsp", '\u{a0}'),
    ("eacute", 'é'),
    ("Eacute", 'É'),
    ("egrave", 'è'),
    ("Egrave", 'È'),
    ("ecirc", 'ê'),
    ("euml", 'ë'),
    ("agrave", 'à'),
    ("Agrave", 'À'),
    ("acirc", 'â'),
    ("auml", 'ä'),
    ("ccedil", 'ç'),
    ("Ccedil", 'Ç'),
    ("icirc", 'î'),
    ("iuml", 'ï'),
    ("ocirc", 'ô'),
    ("ouml", 'ö'),
    ("ugrave", 'ù'),
    ("ucirc", 'û'),
    ("uuml", 'ü'),
    ("ntilde", 'ñ'),
    ("aacute", 'á'),
    ("iacute", 'í'),
    ("oacute", 'ó'),
    ("uacute", 'ú'),
    ("atilde", 'ã'),
    ("otilde", 'õ'),
    ("szlig", 'ß'),
    ("oelig", 'œ'),
    ("laquo", '«'),
    ("raquo", '»'),
    ("lsquo", '‘'),
    ("rsquo", '’'),
    ("ldquo", '“'),
    ("rdquo", '”'),
    ("bdquo", '„'),
    ("hellip", '…'),
    ("mdash", '—'),
    ("ndash", '–'),
    ("euro", '€'),
    ("pound", '£'),
    ("copy", '©'),
    ("reg", '®'),
    ("trade", '™'),
    ("deg", '°'),
    ("times", '×'),
    ("middot", '·'),
    ("bull", '•'),
    ("sect", '§'),
    ("shy", '\u{ad}'),
    ("thinsp", '\u{2009}'),
    ("ensp", '\u{2002}'),
    ("emsp", '\u{2003}'),
];

/// Attributs d'une balise : `(nom en minuscules, valeur décodée)`.
fn attributs(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        while i < cs.len() && (cs[i].is_whitespace() || cs[i] == '/') {
            i += 1;
        }
        let debut = i;
        while i < cs.len() && !cs[i].is_whitespace() && cs[i] != '=' && cs[i] != '/' {
            i += 1;
        }
        if i == debut {
            i += 1;
            continue;
        }
        let nom: String = cs[debut..i].iter().collect::<String>().to_lowercase();
        while i < cs.len() && cs[i].is_whitespace() {
            i += 1;
        }
        let mut valeur = String::new();
        if i < cs.len() && cs[i] == '=' {
            i += 1;
            while i < cs.len() && cs[i].is_whitespace() {
                i += 1;
            }
            if i < cs.len() && (cs[i] == '"' || cs[i] == '\'') {
                let q = cs[i];
                i += 1;
                let d = i;
                while i < cs.len() && cs[i] != q {
                    i += 1;
                }
                valeur = cs[d..i.min(cs.len())].iter().collect();
                i += 1;
            } else {
                let d = i;
                while i < cs.len() && !cs[i].is_whitespace() {
                    i += 1;
                }
                valeur = cs[d..i].iter().collect();
            }
        }
        out.push((nom, entites(&valeur)));
    }
    out
}

fn css(style: &str, base: Style) -> (Style, bool) {
    let mut s = base;
    let mut ignore = false;
    for decl in style.split(';') {
        let Some((k, v)) = decl.split_once(':') else {
            continue;
        };
        let (k, v) = (k.trim().to_lowercase(), v.trim().to_lowercase());
        match k.as_str() {
            "font-weight" => {
                s.gras = v.starts_with("bold")
                    || v.parse::<u32>().is_ok_and(|n| n >= 600)
                    || (v == "bolder");
            }
            "font-style" => s.italique = v.starts_with("italic") || v.starts_with("oblique"),
            "text-decoration" | "text-decoration-line" => {
                if v.contains("none") {
                    s.souligne = false;
                    s.barre = false;
                }
                s.souligne |= v.contains("underline");
                s.barre |= v.contains("line-through");
            }
            "mso-list" if v.contains("ignore") => ignore = true,
            "display" if v.starts_with("none") => ignore = true,
            _ => {}
        }
    }
    (s, ignore)
}

fn niveau_mso(style: &str) -> Option<usize> {
    let l = style.to_lowercase();
    let i = l.find("mso-list:")?;
    let reste = &l[i..];
    let j = reste.find("level")?;
    let n: String = reste[j + 5..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    n.parse::<usize>().ok().map(|n| n.saturating_sub(1))
}

struct Convertisseur {
    sortie: Sortie,
    pile: Vec<Ouvert>,
    listes: Vec<Liste>,
    bloc: Bloc,
    segs: Vec<Seg>,
    /// Ligne en cours d'un tableau, et le tableau.
    tableau: Option<Vec<(Vec<String>, bool)>>,
    cellule: Option<(Vec<Seg>, bool)>,
    /// Profondeur de tableaux imbriqués (seul le plus extérieur compte).
    tables: usize,
    /// Le texte ignoré d'un paragraphe de liste Word (puce ou numéro).
    puce_mso: String,
    mso: Option<usize>,
    /// Groupe des éléments de la liste en cours.
    liste_groupe: u32,
}

impl Convertisseur {
    fn style(&self) -> Style {
        self.pile.last().map(|o| o.style).unwrap_or_default()
    }

    fn lien(&self) -> Option<String> {
        self.pile.iter().rev().find_map(|o| o.lien.clone())
    }

    fn ignore(&self) -> bool {
        self.pile.iter().any(|o| o.ignore)
    }

    fn texte(&mut self, t: &str) {
        if self.ignore() {
            if self.mso.is_some() {
                self.puce_mso.push_str(&entites(t));
            }
            return;
        }
        let t = entites(t);
        let t = if self.bloc == Bloc::Pre {
            t
        } else {
            let mut s = String::with_capacity(t.len());
            let mut espace = false;
            for c in t.chars() {
                if c.is_whitespace() && c != '\u{a0}' {
                    espace = true;
                } else {
                    if espace {
                        s.push(' ');
                    }
                    espace = false;
                    s.push(c);
                }
            }
            if espace {
                s.push(' ');
            }
            s
        };
        if t.is_empty() {
            return;
        }
        let seg = Seg {
            texte: t,
            style: self.style(),
            lien: self.lien(),
        };
        match &mut self.cellule {
            Some((segs, _)) => segs.push(seg),
            None => self.segs.push(seg),
        }
    }

    /// Le texte Varium d'une suite de morceaux.
    fn rendre(segs: &[Seg], tableau: bool) -> String {
        // Morceaux voisins de même style et même lien fusionnés.
        let mut fus: Vec<Seg> = Vec::new();
        for s in segs {
            match fus.last_mut() {
                Some(l) if l.style == s.style && l.lien == s.lien => l.texte.push_str(&s.texte),
                _ => fus.push(s.clone()),
            }
        }
        let mut out = String::new();
        for s in &fus {
            let t = &s.texte;
            let coeur = t.trim_matches(|c: char| c.is_whitespace() && c != '\u{a0}');
            if coeur.is_empty() {
                if !out.ends_with(' ') && !out.is_empty() && t.contains(' ') {
                    out.push(' ');
                }
                continue;
            }
            let debut = &t[..t.find(coeur).unwrap_or(0)];
            let fin = &t[debut.len() + coeur.len()..];
            if !debut.is_empty() && !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
            let mut c = match &s.lien {
                Some(url) if est_url(url) => {
                    let label = coeur.replace(['[', ']'], "");
                    format!("[{}]({url})", echapper(&label, tableau))
                }
                _ => echapper(coeur, tableau),
            };
            let st = s.style;
            for (on, m) in [
                (st.barre, "~~"),
                (st.souligne, "__"),
                (st.italique, "*"),
                (st.gras, "**"),
            ] {
                if on {
                    c = format!("{m}{c}{m}");
                }
            }
            out.push_str(&c);
            if !fin.is_empty() {
                out.push(' ');
            }
        }
        out.trim_end().to_string()
    }

    /// Termine le bloc en cours.
    fn finir(&mut self, groupe_br: bool) {
        let segs = std::mem::take(&mut self.segs);
        if self.bloc == Bloc::Pre {
            let brut: String = segs.iter().map(|s| s.texte.as_str()).collect();
            for l in brut.trim_matches('\n').lines() {
                let l = echapper(l, false);
                self.sortie.lignes.push((echapper_debut(&l), G_BR));
            }
            if let Some(last) = self.sortie.lignes.iter_mut().rev().find(|_| true) {
                last.1 = G_SEUL;
            }
            return;
        }
        let corps = Self::rendre(&segs, false);
        let corps = corps.trim();
        if corps.is_empty() || corps.chars().all(|c| c == '\u{a0}') {
            return;
        }
        let corps = corps.trim_matches('\u{a0}');
        let (ligne, g) = match &self.bloc {
            Bloc::Titre(n) => (format!("{} {corps}", "#".repeat(*n as usize)), G_SEUL),
            Bloc::Item { profondeur, numero } => {
                let marque = match numero {
                    Some(n) => format!("{n}. "),
                    None => "- ".into(),
                };
                (
                    format!("{}{marque}{corps}", "  ".repeat(*profondeur)),
                    self.liste_groupe,
                )
            }
            Bloc::Citation => (format!("> {corps}"), G_SEUL),
            _ => (echapper_debut(corps), G_SEUL),
        };
        let g = if groupe_br { G_BR } else { g };
        self.sortie.lignes.push((ligne, g));
    }

    fn ouvrir_bloc(&mut self, b: Bloc) {
        // Un paragraphe dans un élément de liste continue l'élément.
        if matches!(self.bloc, Bloc::Item { .. }) && b == Bloc::Para && self.segs.is_empty() {
            return;
        }
        self.finir(false);
        self.bloc = b;
    }

    fn balise(&mut self, nom: &str, attrs: &str, fermante: bool, auto: bool) {
        let a = attributs(attrs);
        let attr = |k: &str| a.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        if fermante {
            self.fermer(nom);
            return;
        }
        let mut base = self.style();
        let mut ignore_balise = false;
        match nom {
            "b" | "strong" => base.gras = true,
            "i" | "em" | "cite" | "dfn" | "var" => base.italique = true,
            "u" | "ins" => base.souligne = true,
            "s" | "strike" | "del" => base.barre = true,
            "head" | "style" | "script" | "title" | "template" | "noscript" | "svg" | "math" => {
                ignore_balise = true
            }
            _ => {}
        }
        // Le style CSS l'emporte sur la balise (Google Docs : <b style="font-weight:normal">).
        let (style, ignore_css) = css(attr("style").unwrap_or(""), base);
        let mut ignore = ignore_balise || ignore_css;
        if self.tables == 0 || self.cellule.is_none() || matches!(nom, "table" | "tr" | "td" | "th")
        {
            self.blocs(nom, &a, &mut ignore);
        } else if nom == "br" {
            self.texte(" ");
        }
        if auto
            || matches!(
                nom,
                "br" | "hr" | "img" | "meta" | "link" | "input" | "col" | "wbr"
            )
        {
            return;
        }
        self.pile.push(Ouvert {
            nom: nom.to_string(),
            style,
            lien: if nom == "a" {
                attr("href")
                    .map(str::trim)
                    .filter(|h| est_url(h))
                    .map(str::to_string)
            } else {
                None
            },
            ignore,
        });
    }

    fn blocs(&mut self, nom: &str, a: &[(String, String)], ignore: &mut bool) {
        let attr = |k: &str| a.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        match nom {
            "p" | "div" | "section" | "article" | "header" | "footer" | "main" | "aside"
            | "figure" | "figcaption" | "dd" | "dt" | "address" | "center" | "nav" => {
                let style = attr("style").unwrap_or("");
                match niveau_mso(style) {
                    Some(n) if !style.to_lowercase().contains("mso-list:none") => {
                        self.finir(false);
                        self.bloc = Bloc::Item {
                            profondeur: n.min(5),
                            numero: None,
                        };
                        self.mso = Some(n);
                        self.puce_mso.clear();
                        if self
                            .sortie
                            .lignes
                            .last()
                            .is_none_or(|l| l.1 != self.liste_groupe)
                        {
                            self.liste_groupe = self.sortie.nouveau_groupe();
                        }
                    }
                    _ => self.ouvrir_bloc(Bloc::Para),
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let n = nom[1..].parse::<u8>().unwrap_or(1).min(3);
                self.ouvrir_bloc(Bloc::Titre(n));
            }
            "blockquote" => self.ouvrir_bloc(Bloc::Citation),
            "pre" => self.ouvrir_bloc(Bloc::Pre),
            "ul" | "ol" | "menu" => {
                self.finir(false);
                self.bloc = Bloc::Para;
                if self.listes.is_empty() {
                    self.liste_groupe = self.sortie.nouveau_groupe();
                }
                self.listes.push(Liste {
                    numerotee: nom == "ol",
                    compteur: attr("start")
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or(1),
                });
            }
            "li" => {
                self.finir(false);
                let profondeur = self.listes.len().saturating_sub(1).min(5);
                let numero = match self.listes.last_mut() {
                    Some(l) if l.numerotee => {
                        let n = l.compteur;
                        l.compteur += 1;
                        Some(n)
                    }
                    _ => None,
                };
                self.bloc = Bloc::Item { profondeur, numero };
            }
            "br" => {
                if self.bloc == Bloc::Pre {
                    self.texte("\n");
                } else {
                    self.finir(true);
                }
            }
            "hr" => {
                self.finir(false);
                self.sortie.lignes.push(("---".into(), G_SEUL));
                self.bloc = Bloc::Para;
            }
            "table" => {
                self.tables += 1;
                if self.tables == 1 {
                    self.finir(false);
                    self.tableau = Some(Vec::new());
                }
            }
            "tr" if self.tables == 1 => {
                self.fin_cellule();
                if let Some(t) = &mut self.tableau {
                    t.push((Vec::new(), false));
                }
            }
            "td" | "th" if self.tables == 1 => {
                self.fin_cellule();
                self.cellule = Some((Vec::new(), nom == "th"));
            }
            "img" => {
                if let Some(alt) = attr("alt").filter(|a| !a.trim().is_empty()) {
                    let alt = alt.to_string();
                    self.texte(&alt);
                }
            }
            "o:p" => *ignore = false,
            _ => {}
        }
    }

    fn fin_cellule(&mut self) {
        if let Some((segs, th)) = self.cellule.take() {
            let t = Self::rendre(&segs, true);
            if let Some(tab) = &mut self.tableau {
                if tab.is_empty() {
                    tab.push((Vec::new(), false));
                }
                if let Some(ligne) = tab.last_mut() {
                    ligne.0.push(t.trim().to_string());
                    ligne.1 |= th;
                }
            }
        }
    }

    fn fin_tableau(&mut self) {
        self.fin_cellule();
        let Some(t) = self.tableau.take() else {
            return;
        };
        let lignes: Vec<(Vec<String>, bool)> =
            t.into_iter().filter(|(c, _)| !c.is_empty()).collect();
        let cols = lignes.iter().map(|(c, _)| c.len()).max().unwrap_or(0);
        if cols == 0 {
            return;
        }
        let g = self.sortie.nouveau_groupe();
        for (i, (mut cellules, th)) in lignes.into_iter().enumerate() {
            cellules.resize(cols, String::new());
            let l = format!("| {} |", cellules.join(" | "));
            self.sortie.lignes.push((l, g));
            if i == 0 && th {
                self.sortie
                    .lignes
                    .push((format!("|{}", "---|".repeat(cols)), g));
            }
        }
    }

    fn fermer(&mut self, nom: &str) {
        let Some(pos) = self.pile.iter().rposition(|o| o.nom == nom) else {
            // Fermeture sans ouverture : seuls les blocs comptent.
            self.fin_bloc(nom);
            return;
        };
        let ouverts: Vec<Ouvert> = self.pile.drain(pos..).collect();
        for o in ouverts.iter().rev() {
            self.fin_bloc(&o.nom);
        }
    }

    fn fin_bloc(&mut self, nom: &str) {
        match nom {
            "p" | "div" | "section" | "article" | "header" | "footer" | "main" | "aside"
            | "figure" | "figcaption" | "dd" | "dt" | "address" | "center" | "nav" => {
                if self.cellule.is_none()
                    && !matches!(self.bloc, Bloc::Item { .. } if self.mso.is_none())
                {
                    self.finir(false);
                    self.bloc = Bloc::Para;
                }
                if self.mso.take().is_some() {
                    // Liste Word : numéro si la puce ignorée était « 1. », « a) »…
                    let p = self.puce_mso.trim().trim_matches('\u{a0}');
                    if p.ends_with('.') || p.ends_with(')') {
                        let n: String = p.chars().take_while(char::is_ascii_digit).collect();
                        if let (Ok(n), Some((l, _))) =
                            (n.parse::<u32>(), self.sortie.lignes.last_mut())
                        {
                            let t = l.trim_start();
                            let ind = l.len() - t.len();
                            if let Some(r) = t.strip_prefix("- ") {
                                *l = format!("{}{n}. {r}", &l[..ind]);
                            }
                        }
                    }
                    self.bloc = Bloc::Para;
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "blockquote" | "pre" | "li" => {
                self.finir(false);
                self.bloc = Bloc::Para;
            }
            "ul" | "ol" | "menu" => {
                self.finir(false);
                self.listes.pop();
                self.bloc = Bloc::Para;
            }
            "td" | "th" if self.tables == 1 => self.fin_cellule(),
            "table" => {
                if self.tables == 1 {
                    self.fin_tableau();
                }
                self.tables = self.tables.saturating_sub(1);
            }
            _ => {}
        }
    }
}

/// Convertit du HTML (presse-papiers d'un navigateur, de Word, de LibreOffice) en
/// texte Varium. Tolérant : balises mal fermées, fragments, commentaires.
pub fn html_vers_varium(html: &str) -> String {
    // Fragment du presse-papiers Windows (« CF_HTML ») : seulement le fragment.
    let html = match (
        html.find("<!--StartFragment-->"),
        html.find("<!--EndFragment-->"),
    ) {
        (Some(a), Some(b)) if a < b => &html[a + 20..b],
        _ => html,
    };
    let mut c = Convertisseur {
        sortie: Sortie {
            lignes: Vec::new(),
            groupe: 2,
        },
        pile: Vec::new(),
        listes: Vec::new(),
        bloc: Bloc::Para,
        segs: Vec::new(),
        tableau: None,
        cellule: None,
        tables: 0,
        puce_mso: String::new(),
        mso: None,
        liste_groupe: 2,
    };
    let mut reste = html;
    while !reste.is_empty() {
        let Some(i) = reste.find('<') else {
            c.texte(reste);
            break;
        };
        if i > 0 {
            c.texte(&reste[..i]);
        }
        let tag = &reste[i..];
        if let Some(r) = tag.strip_prefix("<!--") {
            reste = r.find("-->").map_or("", |j| &r[j + 3..]);
            continue;
        }
        if tag.starts_with("<!") || tag.starts_with("<?") {
            reste = tag.find('>').map_or("", |j| &tag[j + 1..]);
            continue;
        }
        let fermante = tag.starts_with("</");
        let corps = &tag[if fermante { 2 } else { 1 }..];
        let nom_fin = corps
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .unwrap_or(corps.len());
        let nom = corps[..nom_fin].to_ascii_lowercase();
        if nom.is_empty() || !nom.starts_with(|c: char| c.is_ascii_alphabetic()) {
            // Un « < » ordinaire.
            c.texte("<");
            reste = &tag[1..];
            continue;
        }
        let Some(fin) = corps.find('>') else {
            break;
        };
        let attrs = &corps[nom_fin..fin];
        let auto = attrs.trim_end().ends_with('/');
        reste = &corps[fin + 1..];
        // Contenu brut (script, style) : jusqu'à la balise fermante.
        if !fermante && matches!(nom.as_str(), "script" | "style" | "title" | "textarea") {
            let fermeture = format!("</{nom}");
            let bas = reste.to_ascii_lowercase();
            reste = match bas.find(&fermeture) {
                Some(j) => {
                    let r = &reste[j..];
                    r.find('>').map_or("", |k| &r[k + 1..])
                }
                None => "",
            };
            continue;
        }
        c.balise(&nom, attrs, fermante, auto);
    }
    // Ce qui reste ouvert est fermé.
    while let Some(o) = c.pile.pop() {
        c.fin_bloc(&o.nom);
    }
    if c.tableau.is_some() {
        c.tables = 1;
        c.fin_tableau();
    }
    c.finir(false);
    c.sortie.texte().trim().to_string()
}

fn semble_html(t: &str) -> bool {
    let t = t.trim_start();
    if !t.starts_with('<') {
        return false;
    }
    let bas = t.to_ascii_lowercase();
    [
        "<html",
        "<body",
        "<p>",
        "<p ",
        "<div",
        "<table",
        "<ul",
        "<ol",
        "<h1",
        "<h2",
        "<h3",
        "<br",
        "<b>",
        "<strong",
        "<span",
        "<!doctype",
        "<meta",
    ]
    .iter()
    .any(|m| bas.contains(m))
        && bas.contains('>')
}

// ---------------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------------

fn semble_markdown(t: &str) -> bool {
    let mut indices = 0;
    let mut fort = false;
    for l in t.lines() {
        let s = l.trim_start();
        let titre = s.starts_with('#') && s.trim_start_matches('#').starts_with(' ');
        if titre || s.starts_with("```") || s.starts_with("|---") || s.starts_with("| ---") {
            fort = true;
        }
        if s.starts_with("- ") || s.starts_with("* ") || s.starts_with("> ") || s.starts_with("1. ")
        {
            indices += 1;
        }
        if s.contains("**") || s.contains("](") || s.contains("__") || s.contains('`') {
            indices += 1;
        }
    }
    fort || indices >= 2
}

/// Convertit le Markdown en texte Varium (les deux syntaxes sont proches).
pub fn markdown_vers_varium(md: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut code = false;
    for l in md.replace("\r\n", "\n").lines() {
        let t = l.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            code = !code;
            continue;
        }
        if code {
            out.push(echapper_debut(&echapper(l, false)));
            continue;
        }
        out.push(ligne_markdown(l));
    }
    out.join("\n")
}

fn ligne_markdown(l: &str) -> String {
    let cs: Vec<char> = l.chars().collect();
    let mut out = String::with_capacity(l.len());
    let mut i = 0;
    let alnum = |c: Option<&char>| c.is_some_and(|c| c.is_alphanumeric());
    let mut italique_ouvert = false;
    while i < cs.len() {
        let c = cs[i];
        // Image : ![alt](url) → alt
        if c == '!' && cs.get(i + 1) == Some(&'[') {
            let reste: String = cs[i + 2..].iter().collect();
            if let Some(f) = reste.find("](")
                && let Some(g) = reste[f..].find(')')
            {
                out.push_str(&reste[..f]);
                i += 2 + reste[..f + g + 1].chars().count();
                continue;
            }
        }
        // Code en ligne : `x` → x (échappé)
        if c == '`' {
            let reste: String = cs[i + 1..].iter().collect();
            if let Some(f) = reste.find('`') {
                out.push_str(&echapper(&reste[..f], false));
                i += 2 + reste[..f].chars().count();
                continue;
            }
        }
        // __gras__ → **gras**
        if c == '_' && cs.get(i + 1) == Some(&'_') {
            out.push_str("**");
            i += 2;
            continue;
        }
        // _italique_ → *italique* (jamais au milieu d'un mot : snake_case reste tel quel)
        if c == '_' {
            let avant = i.checked_sub(1).and_then(|j| cs.get(j));
            let apres = cs.get(i + 1);
            if !italique_ouvert && !alnum(avant) && apres.is_some_and(|c| !c.is_whitespace()) {
                italique_ouvert = true;
                out.push('*');
            } else if italique_ouvert && !alnum(apres) {
                italique_ouvert = false;
                out.push('*');
            } else {
                out.push_str("\\_");
            }
            i += 1;
            continue;
        }
        if c == '{' && cs.get(i + 1) == Some(&'{') {
            out.push_str("\\{");
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------------
// Tableaux (tableur, CSV)
// ---------------------------------------------------------------------------------

/// Découpe une ligne CSV (guillemets doublés compris).
fn champs_csv(l: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut guillemets = false;
    let mut it = l.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' if guillemets && it.peek() == Some(&'"') => {
                cur.push('"');
                it.next();
            }
            '"' if guillemets => guillemets = false,
            '"' if cur.trim().is_empty() => {
                cur.clear();
                guillemets = true;
            }
            c if c == sep && !guillemets => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// Cellules de tableur (tabulations) ou CSV régulier → tableau Varium, la première
/// ligne en en-tête. `None` si ce n'est pas un tableau.
pub fn tableau(t: &str) -> Option<String> {
    let lignes: Vec<&str> = t
        .trim_matches('\n')
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .collect();
    if lignes.is_empty() {
        return None;
    }
    let sep = if lignes.iter().all(|l| l.contains('\t')) {
        '\t'
    } else if lignes.len() >= 2 {
        // CSV : seulement si toutes les lignes ont le même nombre de champs courts.
        *[';', ','].iter().find(|&&s| {
            let n = champs_csv(lignes[0], s).len();
            n >= 2
                && lignes.iter().all(|l| {
                    let c = champs_csv(l, s);
                    c.len() == n && c.iter().all(|x| x.chars().count() <= 60)
                })
                && !lignes[0].trim_end().ends_with('.')
        })?
    } else {
        return None;
    };
    let rangees: Vec<Vec<String>> = lignes
        .iter()
        .map(|l| {
            champs_csv(l, sep)
                .into_iter()
                .map(|c| echapper(c.trim(), true))
                .collect()
        })
        .collect();
    let cols = rangees.iter().map(Vec::len).max()?;
    if cols < 2 && rangees.len() < 2 {
        return None;
    }
    let mut out = String::new();
    for (i, mut r) in rangees.into_iter().enumerate() {
        r.resize(cols, String::new());
        out.push_str(&format!("| {} |\n", r.join(" | ")));
        if i == 0 {
            out.push_str(&format!("|{}\n", "---|".repeat(cols)));
        }
    }
    Some(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Para, Values, document};

    #[test]
    fn html_navigateur() {
        let h = r#"<meta charset="utf-8"><h1 class="x">Titre <em>un</em></h1>
            <p>Un <b>gras</b>, un <i>italique</i>, <u>souligné</u> et <s>barré</s>&nbsp;!
            Voir <a href="https://exemple.fr/a?b=1&amp;c=2">le site</a>.</p>
            <ul><li>un</li><li>deux<ul><li>sous</li></ul></li></ul>
            <ol start="3"><li><p>trois</p></li><li>quatre</li></ol>
            <blockquote>Citation</blockquote><hr>
            <table><tr><th>Nom</th><th>Prix</th></tr><tr><td>Pomme | verte</td><td>2 €</td></tr></table>
            <p>{{pas_un_champ}} et 5 $</p><script>alert(1)</script>"#;
        let v = html_vers_varium(h);
        assert_eq!(
            v,
            "# Titre *un*\n\n\
             Un **gras**, un *italique*, __souligné__ et ~~barré~~\u{a0}! Voir [le site](https://exemple.fr/a?b=1&c=2).\n\n\
             - un\n- deux\n  - sous\n\n\
             3. trois\n4. quatre\n\n\
             > Citation\n\n---\n\n\
             | Nom | Prix |\n|---|---|\n| Pomme \\| verte | 2 € |\n\n\
             \\{{pas\\_un\\_champ}} et 5 \\$"
        );
        // Aucun champ créé, et le document se lit comme prévu.
        assert!(crate::variables(&v).is_empty());
        let d = document(&v, &Values::default());
        assert!(matches!(d[0], Para::Heading { level: 1, .. }));
        assert!(
            d.iter()
                .any(|p| matches!(p, Para::Table(t) if t.header && t.rows.len() == 2))
        );
        assert!(
            d.iter()
                .any(|p| p.runs().iter().any(|r| r.lien().is_some()))
        );
    }

    #[test]
    fn html_google_docs_et_word() {
        // Google Docs : <b style="font-weight:normal"> enveloppe tout.
        let g = r#"<b style="font-weight:normal;" id="docs-internal-guid-1"><p dir="ltr"><span style="font-weight:700">Fort</span><span style="font-weight:400"> et </span><span style="font-style:italic;text-decoration:underline">penché</span></p></b>"#;
        assert_eq!(html_vers_varium(g), "**Fort** et *__penché__*");
        // Word : listes en paragraphes « mso-list », puce ignorée.
        let w = r#"<html xmlns:o="urn:schemas-microsoft-com:office:office"><head><style>p {color:red}</style></head><body>
<!--StartFragment--><p class=MsoListParagraphCxSpFirst style='mso-list:l0 level1 lfo1'><![if !supportLists]><span style='mso-list:Ignore'>1.<span>&nbsp;&nbsp; </span></span><![endif]>Premier<o:p></o:p></p>
<p class=MsoListParagraphCxSpLast style='mso-list:l0 level2 lfo1'><span style='mso-list:Ignore'>·<span>&nbsp;</span></span>Second<o:p></o:p></p>
<p class=MsoNormal><b>Gras</b><o:p>&nbsp;</o:p></p><p class=MsoNormal><o:p>&nbsp;</o:p></p><!--EndFragment--></body></html>"#;
        assert_eq!(html_vers_varium(w), "1. Premier\n  - Second\n\n**Gras**");
        // LibreOffice : <font>, paragraphes, <br>.
        let lo = "<p><font face=\"Liberation Serif\">Ligne 1<br/>Ligne 2</font></p><p># pas un titre</p>";
        assert_eq!(html_vers_varium(lo), "Ligne 1\nLigne 2\n\n\\# pas un titre");
    }

    #[test]
    fn markdown() {
        let md = "## Titre\n\nDu __gras__, de l'_italique_ et snake_case, `code*`.\n\n![logo](x.png) [lien](https://x.fr)\n\n```\n# pas un titre\n```";
        assert!(semble_markdown(md));
        assert_eq!(
            markdown_vers_varium(md),
            "## Titre\n\nDu **gras**, de l'*italique* et snake\\_case, code\\*.\n\nlogo [lien](https://x.fr)\n\n\\# pas un titre"
        );
        assert!(!semble_markdown("Une phrase simple.\nUne autre."));
    }

    #[test]
    fn tableurs_et_csv() {
        assert_eq!(
            tableau("Nom\tPrix\nPomme\t2,5\n").as_deref(),
            Some("| Nom | Prix |\n|---|---|\n| Pomme | 2,5 |")
        );
        assert_eq!(
            tableau("a;b;c\n1;\"2;3\";4").as_deref(),
            Some("| a | b | c |\n|---|---|---|\n| 1 | 2;3 | 4 |")
        );
        // De la prose avec des virgules n'est pas un tableau.
        assert_eq!(tableau("Bonjour, Marc.\nÀ demain, Julie."), None);
        assert_eq!(tableau("une seule ligne"), None);
    }

    #[test]
    fn decisions() {
        let c = coller("https://varium.fr", None, "notre site");
        assert_eq!(c.genre, Genre::Lien);
        assert_eq!(c.texte, "[notre site](https://varium.fr)");
        assert_eq!(coller("https://varium.fr", None, "").genre, Genre::Texte);
        assert_eq!(coller("a\tb\nc\td", None, "").genre, Genre::Tableau);
        let c = coller("Gras", Some("<b>Gras</b>"), "");
        assert_eq!((c.genre, c.texte.as_str()), (Genre::Html, "**Gras**"));
        assert_eq!(coller("<p>Un <b>x</b></p>", None, "").texte, "Un **x**");
        assert_eq!(coller("Bonjour {{nom}}", None, "").texte, "Bonjour {{nom}}");
        assert_eq!(coller("a\r\nb", None, "").texte, "a\nb");
        assert!(est_url("www.exemple.fr"));
        assert!(!est_url("www."));
        assert!(!est_url("voir https://x.fr"));
    }

    #[test]
    fn entites_html() {
        assert_eq!(
            entites("&lt;a&gt; &amp;amp; &#233;&#x1F600; &inconnu; & &;"),
            "<a> &amp; é😀 &inconnu; & &;"
        );
        assert_eq!(entites("&#0;"), "\u{FFFD}");
    }

    /// Générateur pseudo-aléatoire déterministe (xorshift).
    fn alea(etat: &mut u64) -> usize {
        *etat ^= *etat << 13;
        *etat ^= *etat >> 7;
        *etat ^= *etat << 17;
        (*etat >> 11) as usize
    }

    #[test]
    fn fuzz_html() {
        const ATOMES: &[&str] = &[
            "<p>",
            "</p>",
            "<b>",
            "</b>",
            "<i>",
            "</i>",
            "<u>",
            "</u>",
            "<li>",
            "</li>",
            "<ul>",
            "</ul>",
            "<ol>",
            "</ol>",
            "<table>",
            "</table>",
            "<tr>",
            "<td>",
            "</td>",
            "<th>",
            "<br>",
            "<hr/>",
            "<a href=\"https://x.fr\">",
            "</a>",
            "<h2>",
            "</h2>",
            "<pre>",
            "</pre>",
            "<!--",
            "-->",
            "<",
            ">",
            "&",
            "&amp;",
            "&#x1F600;",
            "&#99999999;",
            "{{x}}",
            "$",
            "*",
            "_",
            "|",
            "\n",
            " ",
            "texte",
            "é😀",
            "<span style=\"font-weight:bold\">",
            "</span>",
            "<p style='mso-list:l0 level9 lfo1'>",
            "<span style='mso-list:Ignore'>",
            "<script>",
            "</script>",
            "<blockquote>",
            "<b style=\"font-weight:normal\">",
            "\t",
            "<!--StartFragment-->",
            "<!--EndFragment-->",
            "<x:y a=b c='d' e=\"f\">",
            "</",
        ];
        let mut etat = 0x5DEE_CE66_D1CE_4E5B_u64;
        for _ in 0..3000 {
            let n = alea(&mut etat) % 40;
            let h: String = (0..n)
                .map(|_| ATOMES[alea(&mut etat) % ATOMES.len()])
                .collect();
            let v = html_vers_varium(&h);
            // Jamais de champ, jamais trois lignes vides, et un document lisible.
            assert!(crate::variables(&v).is_empty(), "{h:?} → {v:?}");
            assert!(!v.contains("\n\n\n"), "{h:?} → {v:?}");
            let _ = document(&v, &Values::default());
            let _ = coller(&h, Some(&h), "sel");
            let _ = markdown_vers_varium(&h);
            let _ = tableau(&h);
        }
    }
}
