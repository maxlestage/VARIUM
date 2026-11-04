//! Ouvrir un document Word (`.docx`) ou OpenDocument texte (`.odt`) : il devient un
//! texte Varium (titres, paragraphes, gras, italique, souligné, barré, listes à
//! plusieurs niveaux, tableaux, liens, alignement, sauts de page, notes de bas de
//! page `[^n]`), avec sa mise en page (format du papier, marges, police, langue).
//!
//! - les images ne sont pas importées : un commentaire `{>>Image non importée<<}`
//!   marque leur place (il n'apparaît pas dans les exports) ;
//! - les codes QR écrits par Varium redeviennent des `{{qr:…}}` ;
//! - les commentaires et modifications suivies de Word et de LibreOffice deviennent
//!   des marques de révision (`{>>…<<}`, `{++…++}`, `{--…--}`, voir `critic`) ;
//! - un `{{champ}}` écrit dans le document reste un champ.
//!
//! Les fichiers abîmés (archive tronquée, XML invalide) donnent une erreur en
//! français, jamais un plantage.

use crate::doc::Align;
use crate::lang::Lang;
use crate::page::{DocFont, PageSetup, Paper};

/// Un document importé.
#[derive(Debug, Clone, PartialEq)]
pub struct Importe {
    /// Titre (métadonnées du fichier ; vide s'il n'y en a pas).
    pub titre: String,
    /// Le texte Varium.
    pub source: String,
    pub page: PageSetup,
    /// Ce qui n'a pas pu être repris (images…), en français.
    pub remarques: Vec<String>,
}

/// Le format d'un fichier de traitement de texte, d'après son contenu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Odt,
    Docx,
}

/// Taille maximale d'un fichier à importer.
const MAX_OCTETS: usize = 200 * 1024 * 1024;

/// Ouvre un `.odt` ou un `.docx` (reconnu à son contenu, sinon à son nom).
pub fn importer(nom: &str, octets: &[u8]) -> Result<Importe, String> {
    if octets.len() > MAX_OCTETS {
        return Err(format!("« {nom} » est trop volumineux pour être ouvert."));
    }
    if !octets.starts_with(b"PK") {
        let n = nom.to_lowercase();
        return Err(if n.ends_with(".doc") {
            format!(
                "« {nom} » est un ancien document Word (.doc) : enregistrez-le en .docx ou en .odt pour l'ouvrir dans Varium."
            )
        } else {
            format!(
                "« {nom} » n'est pas un document Word ou OpenDocument valide (archive illisible)."
            )
        });
    }
    let zip =
        crate::zip::ZipReader::new(octets).map_err(|e| format!("« {nom} » est endommagé : {e}"))?;
    let format = if zip.get("word/document.xml").is_some()
        || zip
            .names()
            .any(|n| n.ends_with("document.xml") && n.contains("word"))
    {
        Format::Docx
    } else if zip.get("content.xml").is_some() {
        Format::Odt
    } else {
        return Err(format!(
            "« {nom} » n'est ni un document Word (.docx) ni un document OpenDocument texte (.odt)."
        ));
    };
    let brouillon = match format {
        Format::Odt => crate::import_odt::lire(&zip),
        Format::Docx => crate::import_docx::lire(&zip),
    }
    .map_err(|e| format!("« {nom} » est illisible : {e}"))?;
    Ok(brouillon.finir())
}

// ---------------------------------------------------------------------------------
// Modèle intermédiaire, commun aux deux formats
// ---------------------------------------------------------------------------------

/// Un morceau de texte et sa mise en forme.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Seg {
    pub texte: String,
    pub gras: bool,
    pub italique: bool,
    pub souligne: bool,
    pub barre: bool,
    pub lien: Option<String>,
    /// Déjà du texte Varium (`[^1]`, `{{qr:…}}`, `{>>…<<}`) : écrit tel quel.
    pub brut: bool,
}

impl Seg {
    pub fn brut(t: impl Into<String>) -> Self {
        Seg {
            texte: t.into(),
            brut: true,
            ..Seg::default()
        }
    }

    fn meme_forme(&self, o: &Seg) -> bool {
        self.gras == o.gras
            && self.italique == o.italique
            && self.souligne == o.souligne
            && self.barre == o.barre
            && self.lien == o.lien
            && self.brut == o.brut
    }
}

/// Le genre d'un paragraphe.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Genre {
    #[default]
    Corps,
    Titre(u8),
    Citation,
    Item {
        profondeur: u8,
        numero: Option<u32>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Bloc {
    Para {
        genre: Genre,
        align: Align,
        segs: Vec<Seg>,
    },
    Tableau {
        lignes: Vec<Vec<Vec<Seg>>>,
        entete: bool,
        aligns: Vec<Align>,
    },
    Saut,
    Filet,
    Sommaire,
}

/// Le document en cours de lecture.
#[derive(Debug, Clone, Default)]
pub(crate) struct Brouillon {
    pub blocs: Vec<Bloc>,
    /// Texte Varium des notes de bas de page, dans l'ordre (`[^1]` est la première).
    pub notes: Vec<String>,
    pub images: usize,
    pub page: PageSetup,
    pub titre: String,
    /// Paragraphes de texte justifiés, et paragraphes de texte en tout.
    pub justifies: usize,
    pub paragraphes: usize,
    /// Le paragraphe suivant commence sur une nouvelle page.
    pub saut_en_attente: bool,
}

impl Brouillon {
    /// Ajoute une note de bas de page ; renvoie la marque `[^n]` à placer dans le texte.
    pub fn note(&mut self, segs: &[Seg]) -> Seg {
        self.notes.push(en_ligne(segs, false));
        Seg::brut(format!("[^{}]", self.notes.len()))
    }

    /// La marque d'une image non importée.
    pub fn image(&mut self) -> Seg {
        self.images += 1;
        Seg::brut("{>>Image non importée<<}")
    }

    /// Ajoute un paragraphe : les retours à la ligne (`\n`) le coupent en plusieurs,
    /// une puce ou un numéro écrits au début (« •→texte », « 2.→texte ») en font un
    /// élément de liste.
    pub fn paragraphe(&mut self, genre: Genre, align: Align, segs: Vec<Seg>, retrait_mm: f32) {
        if std::mem::take(&mut self.saut_en_attente) {
            self.blocs.push(Bloc::Saut);
        }
        let mut lignes: Vec<Vec<Seg>> = vec![Vec::new()];
        for s in segs {
            if s.brut || !s.texte.contains('\n') {
                if let Some(l) = lignes.last_mut() {
                    l.push(s);
                }
                continue;
            }
            for (i, part) in s.texte.split('\n').enumerate() {
                if i > 0 {
                    lignes.push(Vec::new());
                }
                if !part.is_empty()
                    && let Some(l) = lignes.last_mut()
                {
                    l.push(Seg {
                        texte: part.to_string(),
                        ..s.clone()
                    });
                }
            }
        }
        for (i, l) in lignes.into_iter().enumerate() {
            let mut l = fusionner(l);
            let mut g = genre;
            if i > 0 && matches!(g, Genre::Item { .. }) {
                g = Genre::Corps;
            }
            if g == Genre::Corps
                && let Some(item) = puce_ecrite(&mut l, retrait_mm)
            {
                g = item;
            }
            if g == Genre::Corps && !texte_de(&l).trim().is_empty() {
                self.paragraphes += 1;
                if align == Align::Justify {
                    self.justifies += 1;
                }
            }
            self.blocs.push(Bloc::Para {
                genre: g,
                align,
                segs: l,
            });
        }
    }

    pub fn saut(&mut self) {
        self.saut_en_attente = false;
        self.blocs.push(Bloc::Saut);
    }

    /// Le document Varium.
    pub fn finir(mut self) -> Importe {
        if self.paragraphes > 0 && self.justifies * 2 > self.paragraphes {
            self.page.justify = true;
        }
        let justifie = self.page.justify;
        let mut lignes: Vec<String> = Vec::new();
        for b in &self.blocs {
            match b {
                Bloc::Para { genre, align, segs } => {
                    lignes.push(ligne(*genre, *align, segs, justifie));
                }
                Bloc::Tableau {
                    lignes: rangs,
                    entete,
                    aligns,
                } => lignes.extend(tableau(rangs, *entete, aligns)),
                Bloc::Saut => lignes.push("\\newpage".into()),
                Bloc::Filet => lignes.push("---".into()),
                Bloc::Sommaire => lignes.push("\\sommaire".into()),
            }
        }
        // Pas plus d'une ligne vide de suite, ni au début ni à la fin.
        let mut source = String::new();
        let mut vides = 0;
        for l in lignes {
            if l.trim().is_empty() {
                vides += 1;
                continue;
            }
            if !source.is_empty() {
                source.push('\n');
                if vides > 0 {
                    source.push('\n');
                }
            }
            vides = 0;
            source.push_str(&l);
        }
        if !self.notes.is_empty() {
            source.push_str("\n\n");
            let defs: Vec<String> = self
                .notes
                .iter()
                .enumerate()
                .map(|(i, n)| format!("[^{}]: {}", i + 1, n.trim()))
                .collect();
            source.push_str(&defs.join("\n"));
        }
        let mut remarques = Vec::new();
        if self.images > 0 {
            remarques.push(if self.images == 1 {
                "1 image n'a pas été importée (sa place est marquée d'un commentaire)".to_string()
            } else {
                format!(
                    "{} images n'ont pas été importées (leur place est marquée d'un commentaire)",
                    self.images
                )
            });
        }
        Importe {
            titre: self.titre.trim().to_string(),
            source,
            page: self.page,
            remarques,
        }
    }
}

/// Marque du champ « numéro de page » (en-tête et pied de page).
pub(crate) const MARQUE_PAGE: &str = "\u{1}page";
const PAGE: &str = MARQUE_PAGE;

/// Sépare le texte d'un pied de page de son numéro de page (« Page 1 sur 3 »).
pub(crate) fn pied(segs: Vec<Seg>) -> (String, bool) {
    let numero = segs.iter().any(|s| s.brut && s.texte == PAGE);
    if !numero {
        let segs: Vec<Seg> = segs
            .into_iter()
            .filter(|s| !s.brut || s.texte != PAGE)
            .collect();
        return (en_ligne(&segs, false), false);
    }
    // Le texte qui précède la tabulation ou le numéro.
    let mut garde = Vec::new();
    for s in segs {
        if s.brut && s.texte == PAGE {
            break;
        }
        if !s.brut && s.texte.contains('\t') {
            let avant = s.texte.split('\t').next().unwrap_or("").to_string();
            garde.push(Seg { texte: avant, ..s });
            break;
        }
        garde.push(s);
    }
    let t = en_ligne(&garde, false);
    let t = t
        .trim_end_matches(|c: char| c.is_whitespace() || c == '-' || c == '–')
        .to_string();
    let seul = ["page", "seite", "página", "pagina", "pág.", "p."];
    if seul.contains(&t.to_lowercase().as_str()) {
        return (String::new(), true);
    }
    (t, true)
}

/// Fusionne les morceaux voisins de même forme (« • », tabulation, texte).
fn fusionner(segs: Vec<Seg>) -> Vec<Seg> {
    let mut out: Vec<Seg> = Vec::with_capacity(segs.len());
    for s in segs {
        match out.last_mut() {
            Some(l) if !s.brut && !l.brut && l.meme_forme(&s) => l.texte.push_str(&s.texte),
            _ => out.push(s),
        }
    }
    out
}

/// Tout le texte de morceaux (sans mise en forme).
pub(crate) fn texte_de(segs: &[Seg]) -> String {
    segs.iter().map(|s| s.texte.as_str()).collect()
}

/// Puce ou numéro écrits à la main au début d'un paragraphe en retrait (c'est ainsi
/// que Varium et beaucoup d'outils exportent les listes) : on en fait un élément.
fn puce_ecrite(segs: &mut Vec<Seg>, retrait_mm: f32) -> Option<Genre> {
    let premier = segs.iter().position(|s| !s.texte.is_empty())?;
    if segs[premier].brut {
        return None;
    }
    let t = segs[premier].texte.trim_start().to_string();
    let profondeur = ((retrait_mm / 6.35).round() as i32 - 1).clamp(0, 5) as u8;
    let mut cs = t.char_indices();
    let (_, c) = cs.next()?;
    let (genre, reste) = if matches!(c, '•' | '◦' | '▪' | '‣' | '○' | '■') {
        // Une vraie puce : un élément de liste, même sans retrait. Sans retrait, la
        // puce donne le niveau (• ◦ ▪, comme les exports de Varium).
        let r = &t[c.len_utf8()..];
        let profondeur = if retrait_mm > 0.5 {
            profondeur
        } else {
            match c {
                '◦' | '○' => 1,
                '▪' | '■' => 2,
                _ => 0,
            }
        };
        (
            Genre::Item {
                profondeur,
                numero: None,
            },
            r.trim_start_matches(['\t', ' ']),
        )
    } else if matches!(c, '–' | '-' | '·') {
        let r = &t[c.len_utf8()..];
        if !(r.starts_with('\t') || (retrait_mm > 0.5 && r.starts_with(' '))) {
            return None;
        }
        (
            Genre::Item {
                profondeur,
                numero: None,
            },
            r.trim_start_matches(['\t', ' ']),
        )
    } else if c.is_ascii_digit() {
        let n = t.chars().take_while(char::is_ascii_digit).count();
        let r = &t[n..];
        let r = r.strip_prefix('.').or_else(|| r.strip_prefix(')'))?;
        if n > 4 || !(r.starts_with('\t') || (retrait_mm > 0.5 && r.starts_with(' '))) {
            return None;
        }
        (
            Genre::Item {
                profondeur,
                numero: t[..n].parse().ok(),
            },
            r.trim_start_matches(['\t', ' ']),
        )
    } else {
        return None;
    };
    segs[premier].texte = reste.to_string();
    Some(genre)
}

// ---------------------------------------------------------------------------------
// Écriture du texte Varium
// ---------------------------------------------------------------------------------

/// Échappe ce qui serait pris pour de la syntaxe Varium (les `{{champs}}` restent).
fn echapper(s: &str, tableau: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '*' | '_' | '~' => {
                out.push('\\');
                out.push(c);
            }
            '$' => out.push_str("\\$"),
            '|' if tableau => out.push_str("\\|"),
            '\t' => out.push(' '),
            '\n' | '\r' => out.push(' '),
            c if c.is_control() => {}
            _ => out.push(c),
        }
    }
    out
}

/// Le texte Varium d'une suite de morceaux.
pub(crate) fn en_ligne(segs: &[Seg], tableau: bool) -> String {
    let mut fus: Vec<Seg> = Vec::new();
    for s in segs {
        if s.texte.is_empty() || (s.brut && s.texte == PAGE) {
            continue;
        }
        match fus.last_mut() {
            Some(l) if l.meme_forme(s) => l.texte.push_str(&s.texte),
            _ => fus.push(s.clone()),
        }
    }
    let mut out = String::new();
    for s in &fus {
        if s.brut {
            out.push_str(&s.texte);
            continue;
        }
        let t = echapper(&s.texte, tableau);
        let coeur = t.trim_matches(|c: char| c.is_whitespace() && c != '\u{a0}');
        if coeur.is_empty() {
            if !t.is_empty() && !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
            continue;
        }
        let debut = t.find(coeur).unwrap_or(0);
        let fin = &t[debut + coeur.len()..];
        if debut > 0 && !out.ends_with(' ') && !out.is_empty() {
            out.push(' ');
        }
        let lien_valide = s.lien.as_deref().is_some_and(crate::collage::est_url);
        let mut c = match &s.lien {
            Some(url) if lien_valide => {
                let label = coeur.replace(['[', ']'], "");
                format!("[{label}]({})", url.replace([' ', ')'], ""))
            }
            _ => coeur.to_string(),
        };
        // Un lien est déjà souligné.
        let souligne = s.souligne && !lien_valide;
        for (on, m) in [
            (s.barre, "~~"),
            (souligne, "__"),
            (s.italique, "*"),
            (s.gras, "**"),
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
    out.trim().to_string()
}

/// Protège un début de ligne qui serait pris pour un titre, une liste, une citation,
/// un tableau, un alignement ou une directive.
fn proteger_debut(t: &str) -> String {
    if t.starts_with('#') || t.starts_with('>') || t.starts_with('|') {
        return format!("\\{t}");
    }
    let chiffres = t.chars().take_while(char::is_ascii_digit).count();
    let liste = ["- ", "* ", "• ", "+ "].iter().any(|p| t.starts_with(p))
        || ((1..=4).contains(&chiffres)
            && (t[chiffres..].starts_with(". ") || t[chiffres..].starts_with(") ")));
    let regle = t.len() >= 3 && ['-', '*', '_'].iter().any(|c| t.chars().all(|x| x == *c));
    if liste
        || regle
        || t.starts_with("->")
        || t.starts_with("$$")
        || t.starts_with("[^")
        || t == "\\newpage"
        || t == "\\sommaire"
    {
        // Une espace sans chasse : invisible, mais la ligne n'est plus une marque.
        return format!("\u{200B}{t}");
    }
    t.to_string()
}

fn ligne(genre: Genre, align: Align, segs: &[Seg], justifie: bool) -> String {
    // Les titres sont déjà gras, les citations en italique : on n'écrit pas
    // `# **Titre**` ni `> *citation*`.
    let segs: Vec<Seg> = segs
        .iter()
        .map(|s| Seg {
            gras: s.gras && !matches!(genre, Genre::Titre(_)),
            italique: s.italique && genre != Genre::Citation,
            ..s.clone()
        })
        .collect();
    let t = en_ligne(&segs, false);
    if t.is_empty() {
        return String::new();
    }
    let corps = match genre {
        Genre::Titre(n) => format!("{} {t}", "#".repeat(n.clamp(1, 3) as usize)),
        Genre::Citation => format!("> {t}"),
        Genre::Item { profondeur, numero } => {
            let retrait = "  ".repeat(profondeur.min(5) as usize);
            match numero {
                Some(n) => format!("{retrait}{n}. {t}"),
                None => format!("{retrait}- {t}"),
            }
        }
        Genre::Corps => proteger_debut(&t),
    };
    match align {
        Align::Center if !matches!(genre, Genre::Item { .. }) => format!("-> {corps} <-"),
        Align::Right if !matches!(genre, Genre::Item { .. }) => format!("-> {corps} ->"),
        Align::Justify if !justifie && genre == Genre::Corps => corps,
        _ => corps,
    }
}

fn tableau(lignes: &[Vec<Vec<Seg>>], entete: bool, aligns: &[Align]) -> Vec<String> {
    let cols = lignes.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let mut out = Vec::new();
    for (i, rang) in lignes.iter().enumerate() {
        let mut l = String::from("|");
        for c in 0..cols {
            // L'en-tête est déjà en gras.
            let t = rang
                .get(c)
                .map(|segs| {
                    let segs: Vec<Seg> = segs
                        .iter()
                        .map(|s| Seg {
                            gras: s.gras && !(i == 0 && entete),
                            ..s.clone()
                        })
                        .collect();
                    en_ligne(&segs, true)
                })
                .unwrap_or_default();
            l.push(' ');
            l.push_str(&t);
            l.push_str(" |");
        }
        out.push(l);
        if i == 0 && entete {
            let mut sep = String::from("|");
            for c in 0..cols {
                sep.push_str(match aligns.get(c).copied().unwrap_or_default() {
                    Align::Center => ":---:|",
                    Align::Right => "---:|",
                    _ => "---|",
                });
            }
            out.push(sep);
        }
    }
    out
}

// ---------------------------------------------------------------------------------
// Utilitaires de mise en page
// ---------------------------------------------------------------------------------

/// Une longueur ODF (`2cm`, `20.5mm`, `1in`, `12pt`) en millimètres.
pub(crate) fn longueur_mm(s: &str) -> Option<f32> {
    let s = s.trim();
    let fin = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .unwrap_or(s.len());
    let v: f32 = s[..fin].parse().ok()?;
    let mm = match s[fin..].trim() {
        "mm" => v,
        "cm" => v * 10.0,
        "in" | "inch" => v * 25.4,
        "pt" => v * 25.4 / 72.0,
        "pc" => v * 25.4 / 6.0,
        "px" => v * 25.4 / 96.0,
        "" => v,
        _ => return None,
    };
    mm.is_finite().then_some(mm)
}

/// Une marge plausible (0 à 100 mm).
pub(crate) fn marge(mm: f32) -> Option<f32> {
    (mm.is_finite() && (0.0..=100.0).contains(&mm)).then_some(mm)
}

/// Le format de papier le plus proche de `largeur × hauteur` (mm), et l'orientation.
pub(crate) fn papier(largeur: f32, hauteur: f32) -> Option<(Paper, bool)> {
    if !(largeur.is_finite() && hauteur.is_finite()) || largeur < 50.0 || hauteur < 50.0 {
        return None;
    }
    let (petit, grand) = (largeur.min(hauteur), largeur.max(hauteur));
    let mut meilleur = (Paper::A4, f32::MAX);
    for p in Paper::ALL {
        let (w, h) = p.size_mm();
        let d = (w - petit).abs() + (h - grand).abs();
        if d < meilleur.1 {
            meilleur = (p, d);
        }
    }
    Some((meilleur.0, largeur > hauteur + 1.0))
}

/// La langue d'une étiquette (`fr-FR`, `de_DE`, `en`).
pub(crate) fn langue(code: &str) -> Option<Lang> {
    let c = code.trim().get(..2)?.to_ascii_lowercase();
    Lang::ALL.into_iter().find(|l| l.code() == c)
}

/// La police Varium la plus proche d'une famille.
pub(crate) fn police(nom: &str) -> DocFont {
    let n = nom.to_lowercase();
    let connue = [
        "arial",
        "helvetica",
        "calibri",
        "verdana",
        "aptos",
        "segoe",
        "tahoma",
        "carlito",
        "open",
        "roboto",
        "trebuchet",
    ]
    .iter()
    .any(|k| n.contains(k));
    if n.contains("sans") || (connue && !n.contains("serif")) {
        DocFont::Sans
    } else {
        DocFont::Serif
    }
}

/// Une taille de police plausible (6 à 72 pt).
pub(crate) fn taille(pt: f32) -> Option<f32> {
    (pt.is_finite() && (6.0..=72.0).contains(&pt)).then_some(pt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(t: &str) -> Seg {
        Seg {
            texte: t.into(),
            ..Seg::default()
        }
    }

    #[test]
    fn morceaux_mis_en_forme() {
        let segs = vec![
            seg("Un "),
            Seg {
                texte: "mot gras".into(),
                gras: true,
                ..Seg::default()
            },
            seg(" et "),
            Seg {
                texte: "un lien".into(),
                lien: Some("https://varium.fr".into()),
                ..Seg::default()
            },
            seg(" à 5 * 3 $ près_"),
        ];
        assert_eq!(
            en_ligne(&segs, false),
            "Un **mot gras** et [un lien](https://varium.fr) à 5 \\* 3 \\$ près\\_"
        );
    }

    #[test]
    fn debuts_de_ligne_proteges() {
        for t in ["# x", "> x", "| x |"] {
            assert!(proteger_debut(t).starts_with('\\'));
        }
        for t in ["- x", "1. x", "---", "-> x", "\\newpage"] {
            assert!(proteger_debut(t).starts_with('\u{200B}'), "{t}");
        }
        assert_eq!(proteger_debut("Bonjour"), "Bonjour");
        // Le texte protégé reste un paragraphe ordinaire.
        let src = format!(
            "{}\n{}",
            proteger_debut("- pas une liste"),
            proteger_debut("# pas un titre")
        );
        let doc = crate::doc::document(&src, &crate::render::Values::default());
        assert!(
            doc.iter()
                .all(|p| matches!(p, crate::doc::Para::Text { .. }))
        );
    }

    #[test]
    fn puces_ecrites_a_la_main() {
        let mut s = vec![seg("•\tPremier")];
        assert_eq!(
            puce_ecrite(&mut s, 12.7),
            Some(Genre::Item {
                profondeur: 1,
                numero: None
            })
        );
        assert_eq!(s[0].texte, "Premier");
        let mut s = vec![seg("3.\tTrois")];
        assert_eq!(
            puce_ecrite(&mut s, 6.35),
            Some(Genre::Item {
                profondeur: 0,
                numero: Some(3)
            })
        );
        let mut s = vec![seg("2026 fut une année")];
        assert_eq!(puce_ecrite(&mut s, 0.0), None);
        let mut s = vec![seg("- tiret sans retrait")];
        assert_eq!(puce_ecrite(&mut s, 0.0), None);
        let mut s = vec![seg("◦imbriqué")];
        assert_eq!(
            puce_ecrite(&mut s, 0.0),
            Some(Genre::Item {
                profondeur: 1,
                numero: None
            })
        );
        assert_eq!(s[0].texte, "imbriqué");
    }

    #[test]
    fn mesures_et_formats() {
        assert_eq!(longueur_mm("2cm"), Some(20.0));
        assert_eq!(longueur_mm("1in"), Some(25.4));
        assert!(longueur_mm("abc").is_none());
        assert!(longueur_mm("12furlong").is_none());
        assert_eq!(papier(210.0, 297.0), Some((Paper::A4, false)));
        assert_eq!(papier(297.0, 210.0), Some((Paper::A4, true)));
        assert_eq!(papier(215.9, 279.4), Some((Paper::Letter, false)));
        assert_eq!(papier(0.0, 1.0), None);
        assert_eq!(langue("de-DE"), Some(Lang::De));
        assert_eq!(langue("pt_BR"), Some(Lang::Pt));
        assert_eq!(langue("x"), None);
        assert_eq!(police("Liberation Sans"), DocFont::Sans);
        assert_eq!(police("Times New Roman"), DocFont::Serif);
        assert_eq!(police("Liberation Serif"), DocFont::Serif);
    }

    #[test]
    fn fichiers_qui_ne_sont_pas_des_documents() {
        assert!(importer("a.docx", b"").is_err());
        assert!(
            importer("a.doc", b"\xD0\xCF\x11\xE0")
                .unwrap_err()
                .contains(".doc")
        );
        let mut z = crate::zip::Zip::new();
        z.add("autre.txt", b"x");
        let e = importer("a.odt", &z.finish()).unwrap_err();
        assert!(e.contains("ni un document Word"), "{e}");
    }
}
