//! Import de présentations : OpenDocument (`.odp`) ici, PowerPoint (`.pptx`) dans
//! [`crate::pptx`], et la construction commune de la présentation Varium.
//!
//! Chaque page est d'abord lue en zones de texte (titre, sous-titre, contenus,
//! pied de page), notes, objets (images, formes) et couleur de fond ; puis la
//! disposition Varium la plus proche est choisie d'après les zones présentes et
//! leur position. Le texte est converti dans la syntaxe Varium : puces et niveaux,
//! listes numérotées, gras, italique, souligné, barré. Les caractères qui auraient
//! un sens dans cette syntaxe (`*`, `|`, `{{`…) sont échappés.
//!
//! Un fichier endommagé donne une erreur en français, jamais une panique.

use std::collections::{BTreeMap, HashMap};

use crate::objets::{Media, Object, ObjectKind};
use crate::texte::contraste;
use crate::xml::{Enfant, Noeud, lire};
use crate::{Aspect, Deck, Frame, Layout, Slide, Theme, Transition, boxes};

// ---------------------------------------------------------------------------------
// Représentation intermédiaire
// ---------------------------------------------------------------------------------

/// Rôle d'une zone de texte importée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    Titre,
    /// Titre d'une diapositive de titre (PowerPoint `ctrTitle`).
    TitreCentre,
    SousTitre,
    Corps,
    Pied,
    Numero,
}

/// Un paragraphe importé : texte déjà dans la syntaxe des caractères Varium
/// (échappé), et sa place dans une liste (profondeur, numérotée).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Ligne {
    pub texte: String,
    pub liste: Option<(u8, bool)>,
}

#[derive(Debug, Clone)]
pub(crate) struct Zone {
    pub role: Role,
    pub frame: Option<Frame>,
    pub lignes: Vec<Ligne>,
}

impl Zone {
    fn vide(&self) -> bool {
        self.lignes.iter().all(|l| l.texte.trim().is_empty())
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Page {
    pub zones: Vec<Zone>,
    pub notes: Vec<Ligne>,
    pub objets: Vec<Object>,
    pub fond: Option<[u8; 3]>,
    pub transition: Option<Transition>,
}

/// Morceau de texte et sa mise en forme.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Marques {
    pub gras: bool,
    pub italique: bool,
    pub souligne: bool,
    pub barre: bool,
}

/// Construit un paragraphe à partir de morceaux mis en forme : les morceaux de
/// même forme sont fusionnés, le texte échappé, les marques posées sans espace
/// intérieur (sinon la syntaxe ne les reconnaîtrait pas).
#[derive(Debug, Default)]
pub(crate) struct Paragraphe {
    morceaux: Vec<(String, Marques)>,
}

impl Paragraphe {
    pub fn pousser(&mut self, t: &str, m: Marques) {
        if t.is_empty() {
            return;
        }
        match self.morceaux.last_mut() {
            Some((s, mm)) if *mm == m => s.push_str(t),
            _ => self.morceaux.push((t.to_string(), m)),
        }
    }

    pub fn est_vide(&self) -> bool {
        self.morceaux.iter().all(|(t, _)| t.trim().is_empty())
    }

    /// Le texte d'un titre : le gras qui couvre tout le paragraphe est retiré (un
    /// titre est gras par nature).
    pub fn markup_titre(&self) -> String {
        let plein = |t: &str| !t.trim().is_empty();
        let tout_gras = self
            .morceaux
            .iter()
            .filter(|(t, _)| plein(t))
            .all(|(_, m)| m.gras);
        if !tout_gras {
            return self.markup();
        }
        let sans = Paragraphe {
            morceaux: self
                .morceaux
                .iter()
                .map(|(t, m)| {
                    (
                        t.clone(),
                        Marques {
                            gras: false,
                            ..*m
                        },
                    )
                })
                .collect(),
        };
        sans.markup()
    }

    /// Le texte dans la syntaxe Varium. Les retours à la ligne sont conservés.
    pub fn markup(&self) -> String {
        let mut out = String::new();
        for (t, m) in &self.morceaux {
            // Les retours à la ligne coupent les marques : chaque ligne est enveloppée.
            let lignes: Vec<String> = t.split('\n').map(|l| envelopper(l, *m)).collect();
            out.push_str(&lignes.join("\n"));
        }
        out
    }
}

/// Échappe ce qui aurait un sens dans la syntaxe des caractères Varium.
pub(crate) fn echapper_markup(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '*' | '|' | '$' => {
                out.push('\\');
                out.push(c);
            }
            '_' | '~' if chars.get(i + 1) == Some(&c) || (i > 0 && chars[i - 1] == c) => {
                out.push('\\');
                out.push(c);
            }
            // Caractères de contrôle : retirés (sauf tabulation, changée en espace).
            '\t' => out.push(' '),
            c if c.is_control() && c != '\n' => {}
            c => out.push(c),
        }
    }
    varium_core::parse::escape_text(&out)
}

/// Un texte mis en forme : marques ouvrantes et fermantes autour du texte, les
/// espaces du début et de la fin laissés à l'extérieur.
fn envelopper(t: &str, m: Marques) -> String {
    let coeur = t.trim();
    if coeur.is_empty() {
        return t.replace('\t', " ");
    }
    let debut = &t[..t.len() - t.trim_start().len()];
    let fin = &t[t.trim_end().len()..];
    let mut ouvre = String::new();
    if m.gras {
        ouvre.push_str("**");
    }
    if m.italique {
        ouvre.push('*');
    }
    if m.souligne {
        ouvre.push_str("__");
    }
    if m.barre {
        ouvre.push_str("~~");
    }
    let ferme: String = {
        let mut f = String::new();
        if m.barre {
            f.push_str("~~");
        }
        if m.souligne {
            f.push_str("__");
        }
        if m.italique {
            f.push('*');
        }
        if m.gras {
            f.push_str("**");
        }
        f
    };
    format!(
        "{}{ouvre}{}{ferme}{}",
        debut.replace('\t', " "),
        echapper_markup(coeur),
        fin.replace('\t', " ")
    )
}

/// Échappe le début d'une ligne qui serait pris pour un titre, une citation ou
/// une liste.
fn debut_sur(l: &str) -> String {
    let t = l.trim_start();
    if t.starts_with('#') || t.starts_with('>') {
        format!("\\{t}")
    } else {
        t.to_string()
    }
}

/// Le contenu (syntaxe Varium) de paragraphes importés.
pub(crate) fn corps_markup(lignes: &[Ligne]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut compteurs = [0u32; 8];
    let mut numerotes = [false; 8];
    for l in lignes {
        match l.liste {
            Some((d, num)) => {
                let d = (d as usize).min(5);
                let texte = l.texte.replace('\n', " ");
                let texte = texte.trim();
                if texte.is_empty() {
                    continue;
                }
                if numerotes[d] != num {
                    compteurs[d] = 0;
                    numerotes[d] = num;
                }
                compteurs[d] += 1;
                for c in compteurs.iter_mut().skip(d + 1) {
                    *c = 0;
                }
                let marque = if num {
                    format!("{}.", compteurs[d])
                } else {
                    "-".into()
                };
                out.push(format!("{}{marque} {texte}", "  ".repeat(d)));
            }
            None => {
                compteurs = [0; 8];
                for part in l.texte.split('\n') {
                    out.push(debut_sur(part.trim_end()));
                }
            }
        }
    }
    // Pas plus d'une ligne vide de suite, aucune au début ni à la fin.
    let mut propre: Vec<String> = Vec::new();
    for l in out {
        if l.trim().is_empty() && propre.last().is_none_or(|p: &String| p.trim().is_empty()) {
            continue;
        }
        propre.push(l);
    }
    while propre.last().is_some_and(|l| l.trim().is_empty()) {
        propre.pop();
    }
    propre.join("\n")
}

/// Un titre (une ligne par paragraphe, sans puce).
pub(crate) fn titre_markup(lignes: &[Ligne]) -> String {
    let t: Vec<String> = lignes
        .iter()
        .flat_map(|l| l.texte.split('\n').map(|s| debut_sur(s.trim())))
        .filter(|s| !s.is_empty())
        .collect();
    t.join("\n")
}

// ---------------------------------------------------------------------------------
// Construction de la présentation
// ---------------------------------------------------------------------------------

/// Distance entre deux cadres (somme des écarts).
fn distance(a: Frame, b: Frame) -> f32 {
    (a.x - b.x).abs() + (a.y - b.y).abs() + (a.w - b.w).abs() + (a.h - b.h).abs()
}

/// Les médias lus dans le fichier, par chemin : ajoutés à la présentation à la
/// première utilisation (une image illisible est ignorée).
pub(crate) struct Medias {
    pub deck: BTreeMap<String, Media>,
    cache: HashMap<String, Option<String>>,
}

impl Medias {
    pub fn new() -> Self {
        Medias {
            deck: BTreeMap::new(),
            cache: HashMap::new(),
        }
    }

    /// L'identifiant du média du fichier `chemin`, s'il est une image lisible.
    pub fn ajouter(&mut self, chemin: &str, octets: Option<&[u8]>) -> Option<String> {
        if let Some(r) = self.cache.get(chemin) {
            return r.clone();
        }
        let r = octets.and_then(|b| Media::from_bytes(b).ok()).map(|m| {
            let id = m.id();
            self.deck.entry(id.clone()).or_insert(m);
            id
        });
        self.cache.insert(chemin.to_string(), r.clone());
        r
    }
}

/// Le thème dont le fond est la couleur la plus fréquente des pages.
fn deviner_theme(pages: &[Page]) -> (Theme, Option<[u8; 3]>) {
    let mut compte: Vec<([u8; 3], usize)> = Vec::new();
    for p in pages {
        if let Some(c) = p.fond {
            match compte.iter_mut().find(|(k, _)| *k == c) {
                Some((_, n)) => *n += 1,
                None => compte.push((c, 1)),
            }
        }
    }
    let Some(&(fond, _)) = compte.iter().max_by_key(|(_, n)| *n) else {
        return (Theme::Clair, None);
    };
    if let Some(t) = Theme::ALL
        .into_iter()
        .find(|t| t.palette().background == fond)
    {
        return (t, Some(fond));
    }
    // Aucun thème n'a ce fond : le plus proche en clarté, fond conservé par page.
    let sombre = contraste(fond, [255, 255, 255]) > contraste(fond, [0, 0, 0]);
    (
        if sombre { Theme::Sombre } else { Theme::Clair },
        None,
    )
}

/// Construit la présentation à partir des pages importées.
pub(crate) fn construire(
    titre: String,
    aspect: Aspect,
    pages: Vec<Page>,
    media: BTreeMap<String, Media>,
) -> Deck {
    let (theme, fond_theme) = deviner_theme(&pages);
    let mut deck = Deck {
        title: titre,
        theme,
        aspect,
        footer: String::new(),
        slide_numbers: false,
        values: BTreeMap::new(),
        slides: Vec::new(),
        media,
    };
    for page in pages {
        let s = diapositive(&mut deck, page, theme, fond_theme);
        deck.slides.push(s);
    }
    if deck.slides.is_empty() {
        deck.slides.push(Slide::new(Layout::Title));
    }
    deck.prune_media();
    deck
}

fn diapositive(
    deck: &mut Deck,
    page: Page,
    theme: Theme,
    fond_theme: Option<[u8; 3]>,
) -> Slide {
    let mut titres = Vec::new();
    let mut sous_titres = Vec::new();
    let mut corps: Vec<Zone> = Vec::new();
    let mut centre = false;
    for z in page.zones {
        if z.vide() {
            if z.role == Role::Numero {
                deck.slide_numbers = true;
            }
            continue;
        }
        match z.role {
            Role::Titre | Role::TitreCentre => {
                centre |= z.role == Role::TitreCentre;
                titres.push(z)
            }
            Role::SousTitre => sous_titres.push(z),
            Role::Corps => {
                // Petites zones en bas de page : pied de page ou numéro.
                let bas = z.frame.is_some_and(|f| f.y >= 0.88 && f.h <= 0.1);
                let texte = titre_markup(&z.lignes);
                let court = texte.chars().count() <= 120 && !texte.contains('\n');
                if bas && court && texte.chars().all(|c| c.is_ascii_digit()) {
                    deck.slide_numbers = true;
                } else if bas && court {
                    if deck.footer.is_empty() {
                        deck.footer = texte;
                    }
                } else {
                    corps.push(z);
                }
            }
            Role::Pied => {
                if deck.footer.is_empty() {
                    deck.footer = titre_markup(&z.lignes);
                }
            }
            Role::Numero => deck.slide_numbers = true,
        }
    }
    // Zones supplémentaires : rangées avec le contenu.
    while titres.len() > 1 {
        let z = titres.remove(1);
        corps.push(Zone {
            role: Role::Corps,
            ..z
        });
    }
    while sous_titres.len() > 1 {
        let z = sous_titres.remove(1);
        corps.push(z);
    }
    // Deux colonnes : de gauche à droite.
    corps.sort_by(|a, b| {
        let x = |z: &Zone| z.frame.map_or(0.0, |f| f.x);
        let y = |z: &Zone| z.frame.map_or(0.0, |f| f.y);
        (x(a), y(a))
            .partial_cmp(&(x(b), y(b)))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let titre = titres.first();
    let sous = sous_titres.first();
    let layout = deviner_layout(titre, sous, &corps, centre);
    let b = boxes(layout);

    let mut s = Slide::new(layout);
    let mut corps1: Vec<String> = Vec::new();
    let mut corps2: Vec<String> = Vec::new();
    let mut reste: Vec<String> = Vec::new();
    for (k, z) in corps.iter().enumerate() {
        let m = corps_markup(&z.lignes);
        if k == 0 || (k > 0 && b.body2.is_none()) {
            corps1.push(m);
        } else {
            corps2.push(m);
        }
    }
    if let Some(z) = titre {
        let t = titre_markup(&z.lignes);
        if b.title.is_some() {
            s.title = t;
        } else {
            reste.push(format!("**{}**", t.replace('\n', " ")));
        }
    }
    if let Some(z) = sous {
        let t = titre_markup(&z.lignes);
        if b.subtitle.is_some() {
            s.subtitle = t;
        } else {
            reste.push(t);
        }
    }
    // Texte sans zone dans la disposition : ajouté en tête du contenu.
    if b.body.is_some() {
        reste.extend(corps1);
        s.body = reste.join("\n\n");
    } else {
        reste.extend(corps1);
        let t = reste.join("\n");
        if b.subtitle.is_some() && s.subtitle.is_empty() {
            s.subtitle = t;
        } else {
            s.body = t;
        }
    }
    s.body2 = corps2.join("\n\n");
    s.notes = corps_markup(&page.notes);
    s.transition = page.transition.unwrap_or_default();
    s.background = match page.fond {
        Some(f) if Some(f) == fond_theme => None,
        Some(f) if fond_theme.is_none() && f == theme.palette().background => None,
        autre => autre,
    };
    // Objets : sans le filet décoratif de la disposition (exports de Varium).
    s.objects = page
        .objets
        .into_iter()
        .filter(|o| {
            let filet = matches!(o.kind, ObjectKind::Rect | ObjectKind::RoundRect)
                && o.text.is_empty()
                && o.stroke.is_none()
                && b.accent.is_some_and(|a| distance(a, o.frame) < 0.02);
            !filet
        })
        .map(Object::normalized)
        .collect();
    s
}

/// La disposition la plus proche des zones présentes.
fn deviner_layout(
    titre: Option<&Zone>,
    sous: Option<&Zone>,
    corps: &[Zone],
    centre: bool,
) -> Layout {
    let mut meilleur = (f32::INFINITY, Layout::TitleContent);
    for l in Layout::ALL {
        let b = boxes(l);
        let mut score = 0.0f32;
        let mut role = |zone: Option<&Zone>, cadre: Option<Frame>| match (zone, cadre) {
            (Some(z), Some(c)) => {
                score += z.frame.map_or(0.0, |f| distance(f, c));
            }
            (Some(_), None) => score += 10.0,
            (None, Some(_)) => score += 0.6,
            (None, None) => {}
        };
        role(titre, b.title);
        role(sous, b.subtitle);
        role(corps.first(), b.body);
        if corps.len() >= 2 {
            role(corps.get(1), b.body2);
        } else if b.body2.is_some() {
            score += 0.6;
        }
        if centre && l == Layout::Title {
            score -= 0.5;
        }
        if score < meilleur.0 {
            meilleur = (score, l);
        }
    }
    meilleur.1
}

// ---------------------------------------------------------------------------------
// Ouverture d'un fichier quelconque
// ---------------------------------------------------------------------------------

pub(crate) fn ouvrir(bytes: &[u8]) -> Result<Deck, String> {
    if bytes.starts_with(b"PK") {
        let z = varium_core::zip::ZipReader::new(bytes)?;
        let noms: Vec<&str> = z.names().collect();
        if noms.contains(&"ppt/presentation.xml")
            || noms.iter().any(|n| n.starts_with("ppt/slides/"))
        {
            return crate::pptx::depuis_zip(&z);
        }
        if noms.contains(&"content.xml") {
            return odp_zip(&z);
        }
        return Err(
            "Ce fichier n'est pas une présentation (ODP ou PPTX attendu).".to_string(),
        );
    }
    let texte = std::str::from_utf8(bytes)
        .map_err(|_| "Ce fichier n'est pas une présentation lisible.".to_string())?;
    Deck::from_json(texte.trim_start_matches('\u{feff}'))
}

// ---------------------------------------------------------------------------------
// OpenDocument
// ---------------------------------------------------------------------------------

/// Longueur ODF (`2.54cm`, `10mm`, `1in`, `72pt`, `96px`) en centimètres.
pub(crate) fn longueur_cm(s: &str) -> Option<f32> {
    let s = s.trim();
    let fin = s
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')))
        .unwrap_or(s.len());
    // `e` d'une unité (`em`) : on recule si le nombre ne se lit pas.
    let (mut n, mut u) = (&s[..fin], &s[fin..]);
    if n.parse::<f32>().is_err()
        && let Some(i) = n.rfind(['e', 'E'])
    {
        (n, u) = (&s[..i], &s[i..]);
    }
    let v: f32 = n.parse().ok()?;
    let k = match u.trim() {
        "cm" => 1.0,
        "mm" => 0.1,
        "in" | "inch" => 2.54,
        "pt" => 2.54 / 72.0,
        "pc" => 2.54 / 6.0,
        "px" => 2.54 / 96.0,
        "" => 1.0,
        _ => return None,
    };
    Some(v * k).filter(|x| x.is_finite())
}

/// Les styles nommés d'un document ODF (automatiques et communs).
struct Styles<'a> {
    par_nom: HashMap<&'a str, &'a Noeud>,
    listes: HashMap<&'a str, &'a Noeud>,
}

impl<'a> Styles<'a> {
    fn new(racines: &[&'a Noeud]) -> Self {
        let mut par_nom = HashMap::new();
        let mut listes = HashMap::new();
        for r in racines {
            for bloc in ["automatic-styles", "styles"] {
                if let Some(b) = r.enfant(bloc) {
                    for n in b.noeuds() {
                        if let Some(nom) = n.attr("style:name") {
                            if n.est("list-style") {
                                listes.insert(nom, n);
                            } else if n.est("style") || n.est("page-layout") {
                                par_nom.entry(nom).or_insert(n);
                            }
                        }
                    }
                }
            }
        }
        Styles { par_nom, listes }
    }

    /// Une propriété d'un style (enfant `propriétés`), en suivant les styles parents.
    fn prop(&self, nom: &str, proprietes: &str, attr: &str) -> Option<&'a str> {
        let mut nom = nom;
        for _ in 0..8 {
            let st = self.par_nom.get(nom)?;
            if let Some(v) = st.enfant(proprietes).and_then(|p| p.attr(attr)) {
                return Some(v);
            }
            nom = st.attr("style:parent-style-name")?;
        }
        None
    }

    fn marques(&self, nom: Option<&str>, mut m: Marques) -> Marques {
        let Some(nom) = nom else {
            return m;
        };
        let p = |a: &str| self.prop(nom, "text-properties", a);
        if let Some(w) = p("fo:font-weight") {
            m.gras = w == "bold" || w.parse::<u32>().is_ok_and(|n| n >= 600);
        }
        if let Some(s) = p("fo:font-style") {
            m.italique = s == "italic" || s == "oblique";
        }
        if let Some(s) = p("style:text-underline-style") {
            m.souligne = s != "none";
        }
        if let Some(s) = p("style:text-line-through-style") {
            m.barre = s != "none";
        }
        m
    }

    /// La liste `nom` est numérotée au niveau `niveau` (0 pour le premier).
    fn numerotee(&self, nom: Option<&str>, niveau: usize) -> bool {
        let Some(l) = nom.and_then(|n| self.listes.get(n)) else {
            return false;
        };
        let voulu = (niveau + 1).to_string();
        l.noeuds()
            .find(|n| n.attr("text:level") == Some(voulu.as_str()))
            .is_some_and(|n| n.est("list-level-style-number"))
    }
}

/// Lecteur des paragraphes ODF.
struct TexteOdf<'s, 'a> {
    styles: &'s Styles<'a>,
    /// Zone de titre : le gras de tout un paragraphe est retiré.
    titre: std::cell::Cell<bool>,
}

impl TexteOdf<'_, '_> {
    /// Les morceaux d'un paragraphe (`text:p`, `text:span`, `text:a`…).
    fn morceaux(&self, n: &Noeud, m: Marques, p: &mut Paragraphe, profondeur: usize) {
        if profondeur > 64 {
            return;
        }
        for e in &n.enfants {
            match e {
                Enfant::Texte(t) => {
                    // Les espaces consécutives ne comptent qu'une fois en ODF.
                    let mut s = String::with_capacity(t.len());
                    let mut espace = false;
                    for c in t.chars() {
                        if c.is_whitespace() {
                            if !espace {
                                s.push(' ');
                            }
                            espace = true;
                        } else {
                            s.push(c);
                            espace = false;
                        }
                    }
                    p.pousser(&s, m);
                }
                Enfant::Noeud(c) => match c.local() {
                    "span" => {
                        let mm = self.styles.marques(c.attr("text:style-name"), m);
                        self.morceaux(c, mm, p, profondeur + 1);
                    }
                    "s" => {
                        let k = c
                            .attr("text:c")
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(1)
                            .min(64);
                        p.pousser(&" ".repeat(k), m);
                    }
                    "tab" => p.pousser(" ", m),
                    "line-break" => p.pousser("\n", m),
                    // Annotations, repères : sans texte affiché.
                    "annotation" | "note" | "bookmark" | "bookmark-start" | "bookmark-end"
                    | "soft-page-break" => {}
                    _ => self.morceaux(c, m, p, profondeur + 1),
                },
            }
        }
    }

    /// Les paragraphes d'une zone de texte (`draw:text-box` ou forme).
    fn paragraphes(
        &self,
        n: &Noeud,
        liste: Option<(usize, Option<&str>)>,
        out: &mut Vec<Ligne>,
        profondeur: usize,
    ) {
        if profondeur > 64 || out.len() > 10_000 {
            return;
        }
        for c in n.noeuds() {
            match c.local() {
                "p" | "h" => {
                    let mut p = Paragraphe::default();
                    // La mise en forme du paragraphe entier est ignorée (les titres
                    // sont gras par nature) : seule celle des morceaux compte.
                    self.morceaux(c, Marques::default(), &mut p, 0);
                    let texte = if self.titre.get() {
                        p.markup_titre()
                    } else {
                        p.markup()
                    };
                    out.push(Ligne {
                        texte,
                        liste: liste.map(|(d, style)| {
                            (d.min(5) as u8, self.styles.numerotee(style, d))
                        }),
                    });
                }
                "list" => {
                    let (d, style) = match liste {
                        Some((d, s)) => (d + 1, c.attr("text:style-name").or(s)),
                        None => (0, c.attr("text:style-name")),
                    };
                    for item in c.noeuds() {
                        match item.local() {
                            "list-item" => {
                                self.paragraphes(item, Some((d, style)), out, profondeur + 1)
                            }
                            // En-tête de liste : paragraphes sans puce.
                            "list-header" => self.paragraphes(item, None, out, profondeur + 1),
                            _ => {}
                        }
                    }
                }
                // Une liste dans un élément de liste : un niveau plus bas.
                _ if liste.is_some() => {}
                "section" | "text-box" => self.paragraphes(c, liste, out, profondeur + 1),
                _ => {}
            }
        }
    }
}

/// Géométrie ODF d'un élément, en fractions de la page.
fn cadre_odf(n: &Noeud, pw: f32, ph: f32) -> Option<Frame> {
    let l = |a: &str| n.attr(a).and_then(longueur_cm);
    let (mut x, mut y) = (l("svg:x"), l("svg:y"));
    if (x.is_none() || y.is_none())
        && let Some(t) = n.attr("draw:transform")
        && let Some(i) = t.find("translate")
    {
        let args = &t[i + 9..];
        let args = args.trim_start().trim_start_matches('(');
        let args = &args[..args.find(')').unwrap_or(args.len())];
        let v: Vec<f32> = args.split_whitespace().filter_map(longueur_cm).collect();
        if v.len() >= 2 {
            x = Some(v[0]);
            y = Some(v[1]);
        }
    }
    let (w, h) = (l("svg:width")?, l("svg:height")?);
    Some(Frame::new(x? / pw, y? / ph, w / pw, h / ph))
}

/// Couleur `#rrggbb` d'une propriété de style.
fn couleur(v: Option<&str>) -> Option<[u8; 3]> {
    v.and_then(crate::texte::parse_color)
}

pub(crate) fn odp(bytes: &[u8]) -> Result<Deck, String> {
    let z = varium_core::zip::ZipReader::new(bytes)?;
    odp_zip(&z)
}

fn odp_zip(z: &varium_core::zip::ZipReader) -> Result<Deck, String> {
    let contenu = z
        .text("content.xml")
        .ok_or("Présentation ODP endommagée : content.xml est absent.")?;
    let content = lire(&contenu).map_err(|e| format!("Présentation ODP illisible : {e}"))?;
    let styles_doc = z.text("styles.xml").and_then(|s| lire(&s).ok());
    let mut racines = vec![&content];
    if let Some(s) = &styles_doc {
        racines.push(s);
    }
    let styles = Styles::new(&racines);
    let presentation = content
        .chemin(&["body", "presentation"])
        .ok_or("Ce fichier OpenDocument n'est pas une présentation.")?;

    // Taille des pages : mise en page de la première page maîtresse employée.
    let maitres: HashMap<&str, &Noeud> = styles_doc
        .iter()
        .flat_map(|s| s.descendants("master-page"))
        .filter_map(|m| Some((m.attr("style:name")?, m)))
        .collect();
    let pages_xml: Vec<&Noeud> = presentation.noeuds().filter(|n| n.est("page")).collect();
    let premier_maitre = pages_xml
        .first()
        .and_then(|p| p.attr("draw:master-page-name"))
        .and_then(|m| maitres.get(m))
        .or_else(|| maitres.values().next());
    let (pw, ph) = premier_maitre
        .and_then(|m| m.attr("style:page-layout-name"))
        .and_then(|pl| {
            let w = styles.prop(pl, "page-layout-properties", "fo:page-width")?;
            let h = styles.prop(pl, "page-layout-properties", "fo:page-height")?;
            Some((longueur_cm(w)?, longueur_cm(h)?))
        })
        .filter(|(w, h)| *w > 0.1 && *h > 0.1)
        .unwrap_or((28.0, 15.75));
    let aspect = if (pw / ph - 4.0 / 3.0).abs() < (pw / ph - 16.0 / 9.0).abs() {
        Aspect::Standard
    } else {
        Aspect::Large
    };

    let fond_style = |nom: Option<&str>| -> Option<[u8; 3]> {
        let nom = nom?;
        let fill = styles.prop(nom, "drawing-page-properties", "draw:fill");
        if fill.is_some_and(|f| f != "solid") {
            return None;
        }
        couleur(styles.prop(nom, "drawing-page-properties", "draw:fill-color"))
    };

    let lecteur = TexteOdf {
        styles: &styles,
        titre: std::cell::Cell::new(false),
    };
    let mut medias = Medias::new();
    let mut pages = Vec::new();
    for p in pages_xml {
        let effet = p.attr("draw:style-name").and_then(|st| {
            styles
                .prop(st, "drawing-page-properties", "smil:type")
                .or_else(|| styles.prop(st, "drawing-page-properties", "presentation:transition-type"))
        });
        let mut page = Page {
            transition: effet.map(|t| match t {
                "pushWipe" | "slideWipe" | "barWipe" | "push" | "cover" | "uncover" => {
                    Transition::Glissement
                }
                "irisWipe" | "zoom" | "ellipseWipe" => Transition::Zoom,
                _ => Transition::Fondu,
            }),
            fond: fond_style(p.attr("draw:style-name")).or_else(|| {
                p.attr("draw:master-page-name")
                    .and_then(|m| maitres.get(m))
                    .and_then(|m| fond_style(m.attr("draw:style-name")))
            }),
            ..Page::default()
        };
        formes_odf(p, &lecteur, &mut medias, z, pw, ph, &mut page, 0);
        if let Some(notes) = p.enfant("notes") {
            for f in notes.descendants("frame") {
                if f.attr("presentation:class") == Some("notes")
                    && let Some(tb) = f.enfant("text-box")
                {
                    lecteur.paragraphes(tb, None, &mut page.notes, 0);
                }
            }
        }
        pages.push(page);
    }
    let titre = z
        .text("meta.xml")
        .and_then(|m| lire(&m).ok())
        .and_then(|m| m.descendant("title").map(Noeud::texte))
        .unwrap_or_default();
    Ok(construire(titre.trim().to_string(), aspect, pages, medias.deck))
}

/// Lit les éléments dessinés d'une page (ou d'un groupe).
#[allow(clippy::too_many_arguments)]
fn formes_odf(
    parent: &Noeud,
    lecteur: &TexteOdf<'_, '_>,
    medias: &mut Medias,
    z: &varium_core::zip::ZipReader,
    pw: f32,
    ph: f32,
    page: &mut Page,
    profondeur: usize,
) {
    if profondeur > 16 {
        return;
    }
    let styles = lecteur.styles;
    for n in parent.noeuds() {
        let style = n.attr("draw:style-name");
        let gp = |a: &str| style.and_then(|s| styles.prop(s, "graphic-properties", a));
        let remplissage = || match gp("draw:fill") {
            Some("none") => None,
            Some("solid") | None => couleur(gp("draw:fill-color")),
            Some(_) => couleur(gp("draw:fill-color")),
        };
        let trait_ = || match gp("draw:stroke") {
            Some("none") => (None, 0.0),
            _ => (
                couleur(gp("svg:stroke-color")).or(Some([52, 101, 164])),
                gp("svg:stroke-width")
                    .and_then(longueur_cm)
                    .map(|cm| (cm / pw * 720.0).max(0.5))
                    .unwrap_or(1.0),
            ),
        };
        let texte_forme = || {
            let mut lignes = Vec::new();
            lecteur.paragraphes(n, None, &mut lignes, 0);
            titre_markup(&lignes)
        };
        // Taille du texte d'une forme : celle du premier paragraphe, en fraction
        // de la hauteur de la page.
        let taille_texte = || {
            n.descendant("p")
                .and_then(|p| p.attr("text:style-name"))
                .and_then(|st| styles.prop(st, "text-properties", "fo:font-size"))
                .and_then(longueur_cm)
                .map(|cm| cm / ph)
                .unwrap_or(0.04)
        };
        match n.local() {
            "frame" => {
                let classe = n.attr("presentation:class");
                let cadre = cadre_odf(n, pw, ph);
                if let Some(img) = n.enfant("image")
                    && n.enfant("text-box").is_none()
                {
                    let href = img.attr("xlink:href").unwrap_or("");
                    let chemin = href.trim_start_matches("./");
                    // Une image peut avoir des remplaçantes (SVG + PNG) : la première lisible.
                    let id = n
                        .noeuds()
                        .filter(|c| c.est("image"))
                        .filter_map(|c| c.attr("xlink:href"))
                        .map(|h| h.trim_start_matches("./"))
                        .find_map(|h| medias.ajouter(h, z.get(h)))
                        .or_else(|| medias.ajouter(chemin, z.get(chemin)));
                    if let (Some(id), Some(f)) = (id, cadre) {
                        page.objets.push(Object {
                            kind: ObjectKind::Image,
                            frame: f,
                            media: id,
                            ..Object::default()
                        });
                    }
                    continue;
                }
                let Some(tb) = n.enfant("text-box") else {
                    continue;
                };
                let numero = tb.descendant("page-number").is_some()
                    && tb.texte().trim().chars().all(|c| c.is_ascii_digit() || c == '<' || c == '>');
                let role = match classe {
                    _ if numero => Role::Numero,
                    Some("title") => Role::Titre,
                    Some("subtitle") => Role::SousTitre,
                    Some("footer") => Role::Pied,
                    Some("page-number") => Role::Numero,
                    Some("date-time") | Some("header") | Some("notes") | Some("handout") => {
                        continue;
                    }
                    _ => Role::Corps,
                };
                let mut lignes = Vec::new();
                lecteur.titre.set(role == Role::Titre);
                lecteur.paragraphes(tb, None, &mut lignes, 0);
                lecteur.titre.set(false);
                page.zones.push(Zone {
                    role,
                    frame: cadre,
                    lignes,
                });
            }
            "rect" | "ellipse" | "circle" | "custom-shape" => {
                let Some(f) = cadre_odf(n, pw, ph) else {
                    continue;
                };
                let kind = match n.local() {
                    "rect" => {
                        if n
                            .attr("draw:corner-radius")
                            .and_then(longueur_cm)
                            .is_some_and(|r| r > 0.0)
                        {
                            ObjectKind::RoundRect
                        } else {
                            ObjectKind::Rect
                        }
                    }
                    "ellipse" | "circle" => ObjectKind::Ellipse,
                    _ => match n
                        .enfant("enhanced-geometry")
                        .and_then(|g| g.attr("draw:type"))
                    {
                        Some("ellipse" | "circle") => ObjectKind::Ellipse,
                        Some("round-rectangle" | "round-square") => ObjectKind::RoundRect,
                        _ => ObjectKind::Rect,
                    },
                };
                let (stroke, w) = trait_();
                let fill = if n.local() == "custom-shape" && gp("draw:fill").is_none() {
                    couleur(gp("draw:fill-color")).or(Some([114, 159, 207]))
                } else {
                    remplissage()
                };
                page.objets.push(Object {
                    kind,
                    frame: f,
                    fill,
                    stroke,
                    stroke_width: w,
                    text: texte_forme(),
                    text_size: taille_texte(),
                    ..Object::default()
                });
            }
            "line" | "connector" => {
                let l = |a: &str| n.attr(a).and_then(longueur_cm);
                let (Some(x1), Some(y1), Some(x2), Some(y2)) =
                    (l("svg:x1"), l("svg:y1"), l("svg:x2"), l("svg:y2"))
                else {
                    continue;
                };
                let (mut x1, mut y1, mut x2, mut y2) = (x1 / pw, y1 / ph, x2 / pw, y2 / ph);
                let fin = gp("draw:marker-end").is_some_and(|m| !m.is_empty());
                let debut = gp("draw:marker-start").is_some_and(|m| !m.is_empty());
                if debut && !fin {
                    std::mem::swap(&mut x1, &mut x2);
                    std::mem::swap(&mut y1, &mut y2);
                }
                let (stroke, w) = trait_();
                page.objets.push(Object {
                    kind: if fin || debut {
                        ObjectKind::Arrow
                    } else {
                        ObjectKind::Line
                    },
                    frame: Frame::new(x1.min(x2), y1.min(y2), (x2 - x1).abs(), (y2 - y1).abs()),
                    flip_h: x1 > x2,
                    flip_v: y1 > y2,
                    stroke: stroke.or(Some([0, 0, 0])),
                    stroke_width: w,
                    ..Object::default()
                });
            }
            "g" => formes_odf(n, lecteur, medias, z, pw, ph, page, profondeur + 1),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echappement_de_la_syntaxe() {
        assert_eq!(echapper_markup("a*b | c"), "a\\*b \\| c");
        assert_eq!(echapper_markup("x__y"), "x\\_\\_y");
        assert_eq!(echapper_markup("snake_case"), "snake_case");
        assert_eq!(echapper_markup("{{nom}}"), "\\{{nom}}");
        assert_eq!(echapper_markup("10 $"), "10 \\$");
    }

    #[test]
    fn marques_sans_espace_interieur() {
        let m = Marques {
            gras: true,
            ..Marques::default()
        };
        assert_eq!(envelopper(" mot ", m), " **mot** ");
        let mut p = Paragraphe::default();
        p.pousser("Un ", Marques::default());
        p.pousser("seul", m);
        p.pousser(" format", Marques::default());
        assert_eq!(p.markup(), "Un **seul** format");
        let runs = varium_core::doc::inline(&p.markup(), &Default::default());
        assert!(runs.iter().any(|r| r.style().bold && r.plain() == "seul"));
    }

    #[test]
    fn listes_et_niveaux() {
        let l = |t: &str, liste| Ligne {
            texte: t.into(),
            liste,
        };
        let m = corps_markup(&[
            l("Intro", None),
            l("a", Some((0, false))),
            l("b", Some((1, false))),
            l("un", Some((0, true))),
            l("deux", Some((0, true))),
            l("# pas un titre", None),
        ]);
        assert_eq!(m, "Intro\n- a\n  - b\n1. un\n2. deux\n\\# pas un titre");
    }

    #[test]
    fn longueurs() {
        assert_eq!(longueur_cm("2.54cm"), Some(2.54));
        assert_eq!(longueur_cm("10mm"), Some(1.0));
        assert!((longueur_cm("1in").unwrap() - 2.54).abs() < 1e-6);
        assert!((longueur_cm("72pt").unwrap() - 2.54).abs() < 1e-5);
        assert_eq!(longueur_cm("abc"), None);
        assert_eq!(longueur_cm("1e3cm"), Some(1000.0));
    }
}
