//! Formules mathématiques : lecture d'un sous-ensemble de LaTeX (`$…$`, `$$…$$`) et
//! composition en glyphes et filets positionnés, pour la vue page et le PDF. Le même
//! arbre s'écrit en MathML (HTML, EPUB, ODT) et en OMML (Word).
//!
//! Pris en charge : lettres en italique, nombres et opérateurs droits avec leurs
//! espacements, exposants et indices (`x^2`, `a_{i,j}`), fractions (`\frac`, `\dfrac`,
//! `\tfrac`, `\binom`), racines (`\sqrt`, `\sqrt[n]`), grands opérateurs avec bornes
//! (`\sum`, `\prod`, `\int`, `\lim`…), délimiteurs ajustés (`\left( … \right)`),
//! matrices (`matrix`, `pmatrix`, `bmatrix`, `vmatrix`, `cases`), accents (`\hat`,
//! `\bar`, `\vec`, `\tilde`, `\dot`, `\overline`), lettres grecques et symboles,
//! texte droit (`\text`, `\mathrm`, `\sin`…), espaces (`\,`, `\;`, `\quad`…).

use crate::export::xml_escape;

/// Classe d'un caractère, pour l'italique et les espacements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Genre {
    /// Variable (italique).
    Ident,
    /// Nombre, lettre grecque majuscule, ponctuation (droit).
    Droit,
    /// Opérateur binaire (+, −, ×…).
    Binaire,
    /// Relation (=, <, ≤, →…).
    Relation,
    /// Ouvrant ou fermant.
    Delim,
    /// Ponctuation suivie d'une espace (`,`, `;`).
    Ponct,
}

/// Un nœud de l'arbre d'une formule.
#[derive(Debug, Clone, PartialEq)]
pub enum Noeud {
    Car(char, Genre),
    /// Texte droit (`\text{…}`, `\sin`…).
    Mot(String),
    Groupe(Vec<Noeud>),
    Scripts {
        base: Box<Noeud>,
        haut: Option<Box<Noeud>>,
        bas: Option<Box<Noeud>>,
    },
    Frac {
        num: Box<Noeud>,
        den: Box<Noeud>,
        barre: bool,
    },
    Racine {
        indice: Option<Box<Noeud>>,
        corps: Box<Noeud>,
    },
    /// Grand opérateur ; `limites` : bornes dessus et dessous en formule centrée.
    GrandOp {
        c: char,
        limites: bool,
    },
    /// Fonction à bornes (`\lim`, `\max`…).
    OpMot {
        mot: String,
    },
    Delim {
        gauche: Option<char>,
        droite: Option<char>,
        corps: Box<Noeud>,
    },
    Matrice {
        lignes: Vec<Vec<Noeud>>,
        gauche: Option<char>,
        droite: Option<char>,
        /// `cases` : colonnes alignées à gauche.
        cas: bool,
    },
    Accent {
        c: char,
        corps: Box<Noeud>,
    },
    /// Espace, en em.
    Espace(f32),
}

// ---------------------------------------------------------------------------
// Lecture
// ---------------------------------------------------------------------------

const PROFONDEUR_MAX: usize = 40;

fn genre(c: char) -> Genre {
    match c {
        '+' | '-' | '−' | '±' | '∓' | '×' | '·' | '÷' | '∪' | '∩' | '∧' | '∨' | '∘' | '⊕' | '⊗'
        | '*' => Genre::Binaire,
        '=' | '<' | '>' | '≤' | '≥' | '≠' | '≈' | '≡' | '∼' | '∝' | '∈' | '∉' | '⊂' | '⊆' | '→'
        | '←' | '⇒' | '⇐' | '⇔' | '↦' | ':' | '≃' | '≅' | '⊃' | '⊇' | '↔' => {
            Genre::Relation
        }
        '(' | ')' | '[' | ']' | '{' | '}' | '|' | '⟨' | '⟩' | '‖' => Genre::Delim,
        ',' | ';' => Genre::Ponct,
        c if c.is_ascii_alphabetic() => Genre::Ident,
        'α'..='ω' | 'ϑ' | 'ϕ' | 'ϵ' | 'ℓ' => Genre::Ident,
        _ => Genre::Droit,
    }
}

const FONCTIONS: &[&str] = &[
    "sin", "cos", "tan", "cot", "sec", "csc", "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh",
    "coth", "log", "ln", "lg", "exp", "det", "dim", "ker", "arg", "deg", "gcd", "hom", "Pr", "mod",
];
const FONCTIONS_A_BORNES: &[&str] = &["lim", "max", "min", "sup", "inf", "limsup", "liminf"];

fn symbole(cmd: &str) -> Option<char> {
    let extra: &[(&str, char)] = &[
        ("langle", '⟨'),
        ("rangle", '⟩'),
        ("lbrace", '{'),
        ("rbrace", '}'),
        ("vert", '|'),
        ("Vert", '‖'),
        ("|", '‖'),
        ("lvert", '|'),
        ("rvert", '|'),
        ("lfloor", '⌊'),
        ("rfloor", '⌋'),
        ("lceil", '⌈'),
        ("rceil", '⌉'),
        ("circ", '∘'),
        ("oplus", '⊕'),
        ("otimes", '⊗'),
        ("ast", '∗'),
        ("star", '⋆'),
        ("simeq", '≃'),
        ("cong", '≅'),
        ("supset", '⊃'),
        ("supseteq", '⊇'),
        ("leftrightarrow", '↔'),
        ("Leftrightarrow", '⇔'),
        ("longrightarrow", '⟶'),
        ("vdots", '⋮'),
        ("ddots", '⋱'),
        ("dots", '…'),
        ("prime", '′'),
        ("aleph", 'ℵ'),
        ("Upsilon", 'Υ'),
        ("upsilon", 'υ'),
        ("omicron", 'ο'),
        ("vartheta", 'ϑ'),
        ("varrho", 'ϱ'),
        ("varsigma", 'ς'),
        ("bigcup", '⋃'),
        ("bigcap", '⋂'),
        ("coprod", '∐'),
        ("iint", '∬'),
        ("iiint", '∭'),
        ("setminus", '∖'),
        ("mid", '∣'),
        ("parallel", '∥'),
        ("perp", '⊥'),
        ("angle", '∠'),
        ("triangle", '△'),
        ("square", '□'),
        ("checkmark", '✓'),
        ("$", '$'),
        ("%", '%'),
        ("&", '&'),
        ("#", '#'),
        ("_", '_'),
    ];
    if let Some((_, c)) = extra.iter().find(|(k, _)| *k == cmd) {
        return Some(*c);
    }
    let (_, v) = crate::tex::SYMBOLS.iter().find(|(k, _)| *k == cmd)?;
    let mut it = v.chars();
    let c = it.next()?;
    it.next().is_none().then_some(c)
}

#[derive(Debug, Clone, PartialEq)]
enum Jeton {
    Car(char),
    Cmd(String),
    Ouvre,
    Ferme,
    Haut,
    Bas,
    Colonne,
    Ligne,
}

fn jetons(src: &str) -> Vec<Jeton> {
    let mut out = Vec::new();
    let chars: Vec<char> = src.chars().take(20_000).collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        match c {
            '\\' => {
                let debut = i;
                while i < chars.len() && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }
                if i == debut {
                    match chars.get(i) {
                        Some('\\') => {
                            out.push(Jeton::Ligne);
                            i += 1;
                        }
                        Some(c) => {
                            out.push(Jeton::Cmd(c.to_string()));
                            i += 1;
                        }
                        None => {}
                    }
                } else {
                    out.push(Jeton::Cmd(chars[debut..i].iter().collect()));
                }
            }
            '{' => out.push(Jeton::Ouvre),
            '}' => out.push(Jeton::Ferme),
            '^' => out.push(Jeton::Haut),
            '_' => out.push(Jeton::Bas),
            '&' => out.push(Jeton::Colonne),
            '\'' => out.push(Jeton::Car('′')),
            c if c.is_whitespace() => {}
            c => out.push(Jeton::Car(c)),
        }
    }
    out
}

struct Lecteur {
    j: Vec<Jeton>,
    i: usize,
}

impl Lecteur {
    fn suivant(&mut self) -> Option<Jeton> {
        let t = self.j.get(self.i).cloned();
        self.i += 1;
        t
    }

    fn voir(&self) -> Option<&Jeton> {
        self.j.get(self.i)
    }

    /// Lit jusqu'à `}` (ou la fin), ou jusqu'à `& \\ \end \right` si `tableau`.
    fn liste(&mut self, p: usize, arret_tableau: bool) -> Vec<Noeud> {
        let mut out: Vec<Noeud> = Vec::new();
        while let Some(t) = self.voir() {
            match t {
                Jeton::Ferme => break,
                Jeton::Colonne | Jeton::Ligne if arret_tableau => break,
                Jeton::Cmd(c) if c == "end" || c == "right" => break,
                Jeton::Haut | Jeton::Bas => {
                    let haut = *t == Jeton::Haut;
                    self.i += 1;
                    let arg = self.atome(p + 1).unwrap_or(Noeud::Groupe(Vec::new()));
                    let base = out.pop().unwrap_or(Noeud::Groupe(Vec::new()));
                    let n = match base {
                        Noeud::Scripts {
                            base,
                            haut: h,
                            bas: b,
                        } if (haut && h.is_none()) || (!haut && b.is_none()) => {
                            if haut {
                                Noeud::Scripts {
                                    base,
                                    haut: Some(Box::new(arg)),
                                    bas: b,
                                }
                            } else {
                                Noeud::Scripts {
                                    base,
                                    haut: h,
                                    bas: Some(Box::new(arg)),
                                }
                            }
                        }
                        // Double exposant (`x^a^b`) : un nouvel exposant, sans imbriquer.
                        base @ Noeud::Scripts { .. } => {
                            out.push(base);
                            Noeud::Scripts {
                                base: Box::new(Noeud::Groupe(Vec::new())),
                                haut: haut.then(|| Box::new(arg.clone())),
                                bas: (!haut).then(|| Box::new(arg)),
                            }
                        }
                        base => Noeud::Scripts {
                            base: Box::new(base),
                            haut: haut.then(|| Box::new(arg.clone())),
                            bas: (!haut).then(|| Box::new(arg)),
                        },
                    };
                    out.push(n);
                }
                _ => match self.atome(p + 1) {
                    Some(n) => out.push(n),
                    None => break,
                },
            }
        }
        out
    }

    fn groupe(&mut self, p: usize) -> Noeud {
        let l = self.liste(p, false);
        if self.voir() == Some(&Jeton::Ferme) {
            self.i += 1;
        }
        if l.len() == 1 {
            l.into_iter().next().unwrap_or(Noeud::Groupe(Vec::new()))
        } else {
            Noeud::Groupe(l)
        }
    }

    /// Texte brut d'un argument `{…}` (pour `\text`, `\begin`).
    fn texte_brut(&mut self) -> String {
        let mut s = String::new();
        if self.voir() != Some(&Jeton::Ouvre) {
            if let Some(Jeton::Car(c)) = self.voir() {
                s.push(*c);
                self.i += 1;
            }
            return s;
        }
        self.i += 1;
        let mut prof = 1;
        while let Some(t) = self.suivant() {
            match t {
                Jeton::Ouvre => prof += 1,
                Jeton::Ferme => {
                    prof -= 1;
                    if prof == 0 {
                        break;
                    }
                }
                Jeton::Car(c) => s.push(c),
                Jeton::Cmd(c) => match symbole(&c) {
                    Some(ch) => s.push(ch),
                    None if c == "," || c == " " || c == ";" => s.push(' '),
                    None => s.push_str(&c),
                },
                Jeton::Haut => s.push('^'),
                Jeton::Bas => s.push('_'),
                Jeton::Colonne => s.push('&'),
                Jeton::Ligne => s.push(' '),
            }
        }
        s
    }

    fn delimiteur(&mut self) -> Option<char> {
        match self.suivant()? {
            Jeton::Car('.') => None,
            Jeton::Car(c) => Some(c),
            Jeton::Cmd(c) => match c.as_str() {
                "{" | "lbrace" => Some('{'),
                "}" | "rbrace" => Some('}'),
                "|" | "Vert" => Some('‖'),
                _ => symbole(&c),
            },
            _ => None,
        }
    }

    fn atome(&mut self, p: usize) -> Option<Noeud> {
        if p > PROFONDEUR_MAX {
            // Trop imbriqué : on consomme le reste à plat.
            let t = self.suivant()?;
            return Some(match t {
                Jeton::Car(c) => Noeud::Car(c, genre(c)),
                _ => Noeud::Groupe(Vec::new()),
            });
        }
        let t = self.suivant()?;
        Some(match t {
            Jeton::Car(c) if c.is_ascii_digit() || c == '.' => {
                let mut s = c.to_string();
                while let Some(Jeton::Car(d)) = self.voir() {
                    if d.is_ascii_digit()
                        || (*d == '.' && s.chars().last().is_some_and(|x| x.is_ascii_digit()))
                    {
                        s.push(*d);
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                if s.chars().count() == 1 {
                    Noeud::Car(c, Genre::Droit)
                } else {
                    Noeud::Mot(s)
                }
            }
            Jeton::Car(c) => Noeud::Car(if c == '-' { '−' } else { c }, genre(c)),
            Jeton::Ouvre => self.groupe(p),
            Jeton::Ferme | Jeton::Haut | Jeton::Bas | Jeton::Colonne | Jeton::Ligne => {
                Noeud::Groupe(Vec::new())
            }
            Jeton::Cmd(c) => self.commande(&c, p),
        })
    }

    fn arg(&mut self, p: usize) -> Box<Noeud> {
        Box::new(self.atome(p + 1).unwrap_or(Noeud::Groupe(Vec::new())))
    }

    fn commande(&mut self, c: &str, p: usize) -> Noeud {
        match c {
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                let num = self.arg(p);
                let den = self.arg(p);
                Noeud::Frac {
                    num,
                    den,
                    barre: true,
                }
            }
            "binom" | "dbinom" | "tbinom" => {
                let num = self.arg(p);
                let den = self.arg(p);
                Noeud::Delim {
                    gauche: Some('('),
                    droite: Some(')'),
                    corps: Box::new(Noeud::Frac {
                        num,
                        den,
                        barre: false,
                    }),
                }
            }
            "sqrt" => {
                let indice = if self.voir() == Some(&Jeton::Car('[')) {
                    self.i += 1;
                    let mut l = Vec::new();
                    while let Some(t) = self.voir() {
                        if *t == Jeton::Car(']') {
                            self.i += 1;
                            break;
                        }
                        match self.atome(p + 1) {
                            Some(n) => l.push(n),
                            None => break,
                        }
                    }
                    Some(Box::new(Noeud::Groupe(l)))
                } else {
                    None
                };
                Noeud::Racine {
                    indice,
                    corps: self.arg(p),
                }
            }
            "text" | "mathrm" | "textrm" | "operatorname" | "mbox" | "textit" | "mathit"
            | "textbf" | "mathbf" | "mathsf" | "mathtt" => {
                let t = self.texte_brut();
                if c == "mathit" || c == "textit" {
                    Noeud::Groupe(t.chars().map(|ch| Noeud::Car(ch, Genre::Ident)).collect())
                } else {
                    Noeud::Mot(t)
                }
            }
            "mathbb" | "mathcal" | "mathfrak" => {
                let t = self.texte_brut();
                let cle = format!("mathbb{{{t}}}");
                match crate::tex::SYMBOLS.iter().find(|(k, _)| *k == cle) {
                    Some((_, v)) => Noeud::Mot(v.to_string()),
                    None => Noeud::Mot(t),
                }
            }
            "left" => {
                let gauche = self.delimiteur();
                let corps = self.liste(p + 1, false);
                let droite = if self.voir() == Some(&Jeton::Cmd("right".into())) {
                    self.i += 1;
                    self.delimiteur()
                } else {
                    None
                };
                Noeud::Delim {
                    gauche,
                    droite,
                    corps: Box::new(Noeud::Groupe(corps)),
                }
            }
            "right" => Noeud::Groupe(Vec::new()),
            "big" | "Big" | "bigg" | "Bigg" | "bigl" | "bigr" | "Bigl" | "Bigr" => {
                match self.delimiteur() {
                    Some(d) => Noeud::Car(d, Genre::Delim),
                    None => Noeud::Groupe(Vec::new()),
                }
            }
            "begin" => {
                let env = self.texte_brut();
                let (gauche, droite, cas) = match env.trim() {
                    "pmatrix" => (Some('('), Some(')'), false),
                    "bmatrix" => (Some('['), Some(']'), false),
                    "Bmatrix" => (Some('{'), Some('}'), false),
                    "vmatrix" => (Some('|'), Some('|'), false),
                    "Vmatrix" => (Some('‖'), Some('‖'), false),
                    "cases" => (Some('{'), None, true),
                    _ => (None, None, false),
                };
                let mut lignes: Vec<Vec<Noeud>> = vec![Vec::new()];
                let mut garde = 0;
                loop {
                    garde += 1;
                    if garde > 10_000 {
                        break;
                    }
                    let cellule = self.liste(p + 1, true);
                    if let Some(l) = lignes.last_mut() {
                        l.push(Noeud::Groupe(cellule));
                    }
                    match self.suivant() {
                        Some(Jeton::Colonne) => {}
                        Some(Jeton::Ligne) => lignes.push(Vec::new()),
                        Some(Jeton::Cmd(e)) if e == "end" => {
                            let _ = self.texte_brut();
                            break;
                        }
                        Some(Jeton::Ferme) | Some(Jeton::Cmd(_)) => {}
                        None => break,
                        Some(_) => {}
                    }
                }
                lignes.retain(|l| {
                    !l.iter()
                        .all(|n| matches!(n, Noeud::Groupe(g) if g.is_empty()))
                });
                Noeud::Matrice {
                    lignes,
                    gauche,
                    droite,
                    cas,
                }
            }
            "end" => {
                let _ = self.texte_brut();
                Noeud::Groupe(Vec::new())
            }
            "hat" | "widehat" => self.accent('^'),
            "bar" | "overline" => self.accent('¯'),
            "vec" | "overrightarrow" => self.accent('→'),
            "tilde" | "widetilde" => self.accent('~'),
            "dot" => self.accent('˙'),
            "ddot" => self.accent('¨'),
            "," | "thinspace" => Noeud::Espace(0.17),
            ":" | ">" | "medspace" => Noeud::Espace(0.22),
            ";" | "thickspace" => Noeud::Espace(0.28),
            "!" => Noeud::Espace(-0.17),
            " " => Noeud::Espace(0.25),
            "quad" => Noeud::Espace(1.0),
            "qquad" => Noeud::Espace(2.0),
            "displaystyle" | "textstyle" | "limits" | "nolimits" => Noeud::Groupe(Vec::new()),
            "sum" | "prod" | "coprod" | "bigcup" | "bigcap" => Noeud::GrandOp {
                c: symbole(c).unwrap_or('∑'),
                limites: true,
            },
            "int" | "oint" | "iint" | "iiint" => Noeud::GrandOp {
                c: symbole(c).unwrap_or('∫'),
                limites: false,
            },
            _ if FONCTIONS_A_BORNES.contains(&c) => Noeud::OpMot { mot: c.to_string() },
            _ if FONCTIONS.contains(&c) => Noeud::Mot(c.to_string()),
            _ => match symbole(c) {
                Some(ch) => Noeud::Car(ch, genre(ch)),
                None => Noeud::Mot(c.to_string()),
            },
        }
    }

    fn accent(&mut self, c: char) -> Noeud {
        Noeud::Accent {
            c,
            corps: self.arg(0),
        }
    }
}

/// L'arbre d'une formule LaTeX (jamais d'échec : l'inconnu devient du texte).
pub fn lire(src: &str) -> Noeud {
    let mut l = Lecteur {
        j: jetons(src),
        i: 0,
    };
    let mut tout = Vec::new();
    let mut garde = 0;
    while l.i < l.j.len() && garde < 100_000 {
        garde += 1;
        let avant = l.i;
        tout.extend(l.liste(0, false));
        if l.i == avant {
            l.i += 1; // `}`, `\end` ou `\right` orphelin
        } else if matches!(l.voir(), Some(Jeton::Ferme | Jeton::Colonne | Jeton::Ligne)) {
            l.i += 1;
        }
    }
    Noeud::Groupe(tout)
}

// ---------------------------------------------------------------------------
// Composition
// ---------------------------------------------------------------------------

/// Un élément dessiné d'une formule, relatif à son origine (x vers la droite, y vers
/// le bas, ligne de base en y = 0).
#[derive(Debug, Clone, PartialEq)]
pub enum Trace {
    /// Un caractère : position de sa ligne de base, taille en points, italique.
    Glyphe {
        x: f32,
        y: f32,
        c: char,
        taille: f32,
        italique: bool,
    },
    /// Rectangle plein (barre de fraction, haut d'une racine), coin haut-gauche.
    Barre { x: f32, y: f32, l: f32, h: f32 },
    /// Ligne brisée (signe radical), épaisseur en points.
    Trait { points: Vec<(f32, f32)>, ep: f32 },
}

/// Une formule composée : largeur, hauteur au-dessus et profondeur sous la ligne de base.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Boite {
    pub largeur: f32,
    pub hauteur: f32,
    pub profondeur: f32,
    pub traces: Vec<Trace>,
}

impl Boite {
    fn decaler(&mut self, dx: f32, dy: f32) {
        for t in &mut self.traces {
            match t {
                Trace::Glyphe { x, y, .. } | Trace::Barre { x, y, .. } => {
                    *x += dx;
                    *y += dy;
                }
                Trace::Trait { points, .. } => {
                    for p in points {
                        p.0 += dx;
                        p.1 += dy;
                    }
                }
            }
        }
    }

    /// Ajoute `b` à la position (`x`, `y`) de sa ligne de base.
    fn poser(&mut self, mut b: Boite, x: f32, y: f32) {
        b.decaler(x, y);
        self.hauteur = self.hauteur.max(b.hauteur - y);
        self.profondeur = self.profondeur.max(b.profondeur + y);
        self.largeur = self.largeur.max(x + b.largeur);
        self.traces.append(&mut b.traces);
    }
}

/// Largeur d'un caractère, en em, à une taille donnée.
pub trait Mesure {
    fn avance(&self, c: char, italique: bool) -> f32;
}

impl<F: Fn(char, bool) -> f32> Mesure for F {
    fn avance(&self, c: char, italique: bool) -> f32 {
        self(c, italique)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Niveau {
    Centre,
    Texte,
    Script,
    Script2,
}

impl Niveau {
    fn script(self) -> Niveau {
        match self {
            Niveau::Centre | Niveau::Texte => Niveau::Script,
            _ => Niveau::Script2,
        }
    }
    fn fraction(self) -> Niveau {
        match self {
            Niveau::Centre => Niveau::Texte,
            n => n.script(),
        }
    }
    fn echelle(self) -> f32 {
        match self {
            Niveau::Centre | Niveau::Texte => 1.0,
            Niveau::Script => 0.7,
            Niveau::Script2 => 0.5,
        }
    }
}

struct Compo<'a> {
    m: &'a dyn Mesure,
    base: f32,
}

const ASC: f32 = 0.70;
const DESC: f32 = 0.22;
const AXE: f32 = 0.25;
const REGLE: f32 = 0.05;

impl Compo<'_> {
    fn car(&self, c: char, italique: bool, t: f32) -> Boite {
        let l = self.m.avance(c, italique) * t;
        let (h, p) = match c {
            '∑' | '∏' | '∐' | '⋃' | '⋂' => (0.75, 0.25),
            '∫' | '∮' | '∬' | '∭' => (0.85, 0.3),
            '(' | ')' | '[' | ']' | '{' | '}' | '|' | '‖' | '⟨' | '⟩' => (0.75, 0.25),
            'a'..='z' if !"bdfhklt".contains(c) => {
                (0.46, if "gjpqy".contains(c) { DESC } else { 0.0 })
            }
            _ => (
                ASC,
                if "gjpqyβζξφψμρχ".contains(c) {
                    DESC
                } else {
                    0.02
                },
            ),
        };
        Boite {
            largeur: l + if italique { 0.03 * t } else { 0.0 },
            hauteur: h * t,
            profondeur: p * t,
            traces: vec![Trace::Glyphe {
                x: 0.0,
                y: 0.0,
                c,
                taille: t,
                italique,
            }],
        }
    }

    fn mot(&self, s: &str, t: f32) -> Boite {
        let mut b = Boite::default();
        let mut x = 0.0;
        for c in s.chars() {
            let g = self.car(c, false, t);
            let l = g.largeur;
            b.poser(g, x, 0.0);
            x += l;
        }
        b.largeur = x;
        b.hauteur = b.hauteur.max(ASC * t);
        b
    }

    fn espace(l: f32) -> Boite {
        Boite {
            largeur: l,
            ..Boite::default()
        }
    }

    /// Une suite horizontale, avec les espacements des opérateurs.
    fn suite(&self, l: &[Noeud], n: Niveau) -> Boite {
        let t = self.base * n.echelle();
        let mut b = Boite::default();
        let mut x = 0.0;
        let mut prec: Option<Genre> = None;
        let serre = matches!(n, Niveau::Script | Niveau::Script2);
        let mut i = 0;
        while i < l.len() {
            let noeud = &l[i];
            let g = match noeud {
                Noeud::Car(_, g) => Some(*g),
                Noeud::Scripts { base, .. } => match base.as_ref() {
                    Noeud::Car(_, g) => Some(*g),
                    _ => None,
                },
                _ => None,
            };
            // Un binaire en tête ou après un opérateur est un signe (−x) : pas d'espace.
            let g = match (g, prec) {
                (
                    Some(Genre::Binaire),
                    None | Some(Genre::Binaire | Genre::Relation | Genre::Ponct),
                ) => Some(Genre::Droit),
                (g, _)
                    if matches!(prec, Some(Genre::Delim))
                        && g == Some(Genre::Binaire)
                        && i > 0
                        && matches!(l[i - 1], Noeud::Car('(' | '[', _)) =>
                {
                    Some(Genre::Droit)
                }
                _ => g,
            };
            let esp = if serre {
                0.0
            } else {
                match g {
                    Some(Genre::Binaire) => 0.22,
                    Some(Genre::Relation) => 0.28,
                    _ => 0.0,
                }
            };
            if matches!(noeud, Noeud::Mot(_) | Noeud::OpMot { .. })
                && matches!(prec, Some(Genre::Ident | Genre::Droit))
                && !serre
            {
                x += 0.17 * t;
            }
            x += esp * t;
            let boite = self.noeud(noeud, n);
            let lb = boite.largeur;
            b.poser(boite, x, 0.0);
            x += lb + esp * t;
            if g == Some(Genre::Ponct) && !serre {
                x += 0.17 * t;
            }
            if matches!(noeud, Noeud::Mot(m) if m.chars().all(char::is_alphabetic))
                && i + 1 < l.len()
                && matches!(l[i + 1], Noeud::Car(_, Genre::Ident) | Noeud::Mot(_))
                && !serre
            {
                x += 0.17 * t;
            }
            prec = g.or(Some(Genre::Ident));
            i += 1;
        }
        b.largeur = x;
        b
    }

    fn scripts(&self, base: &Noeud, haut: Option<&Noeud>, bas: Option<&Noeud>, n: Niveau) -> Boite {
        let t = self.base * n.echelle();
        let limites = n == Niveau::Centre
            && matches!(
                base,
                Noeud::GrandOp { limites: true, .. } | Noeud::OpMot { .. }
            );
        let b = self.noeud(base, n);
        let ns = n.script();
        let h = haut.map(|x| self.noeud(x, ns));
        let s = bas.map(|x| self.noeud(x, ns));
        let mut out = Boite::default();
        if limites {
            let w = b
                .largeur
                .max(h.as_ref().map_or(0.0, |x| x.largeur))
                .max(s.as_ref().map_or(0.0, |x| x.largeur));
            let (bh, bp, bl) = (b.hauteur, b.profondeur, b.largeur);
            out.poser(b, (w - bl) / 2.0, 0.0);
            if let Some(h) = h {
                let y = -bh - 0.12 * t - h.profondeur;
                let l = h.largeur;
                out.poser(h, (w - l) / 2.0, y);
            }
            if let Some(s) = s {
                let y = bp + 0.12 * t + s.hauteur;
                let l = s.largeur;
                out.poser(s, (w - l) / 2.0, y);
            }
            out.largeur = w;
            return out;
        }
        let integrale = matches!(base, Noeud::GrandOp { limites: false, .. });
        let (bh, bp, bl) = (b.hauteur, b.profondeur, b.largeur);
        out.poser(b, 0.0, 0.0);
        let mut droite = bl;
        let mut haut_y = None;
        if let Some(h) = h {
            let y = -(if integrale {
                bh * 0.8
            } else {
                (bh - 0.25 * t).max(0.38 * t)
            })
            .max(h.profondeur + 0.2 * t);
            let x = bl + if integrale { 0.05 * t } else { 0.02 * t };
            droite = droite.max(x + h.largeur);
            haut_y = Some(y + h.profondeur);
            out.poser(h, x, y);
        }
        if let Some(s) = s {
            let mut y = (bp + 0.05 * t).max(0.2 * t).max(s.hauteur - 0.4 * t);
            if let Some(hy) = haut_y {
                y = y.max(hy + 0.1 * t + s.hauteur);
            }
            let x = bl - if integrale { 0.15 * t } else { 0.0 };
            droite = droite.max(x + s.largeur);
            out.poser(s, x, y);
        }
        out.largeur = droite + 0.03 * t;
        out
    }

    fn frac(&self, num: &Noeud, den: &Noeud, barre: bool, n: Niveau) -> Boite {
        let t = self.base * n.echelle();
        let nf = n.fraction();
        let a = self.noeud(num, nf);
        let b = self.noeud(den, nf);
        let w = a.largeur.max(b.largeur) + 0.2 * t;
        let axe = -AXE * t;
        let ep = REGLE * t;
        let ecart = if n == Niveau::Centre { 0.15 } else { 0.08 } * t;
        let mut out = Boite::default();
        let (al, ap) = (a.largeur, a.profondeur);
        out.poser(a, (w - al) / 2.0, axe - ep / 2.0 - ecart - ap);
        let (bl, bh) = (b.largeur, b.hauteur);
        out.poser(b, (w - bl) / 2.0, axe + ep / 2.0 + ecart + bh);
        if barre {
            out.traces.push(Trace::Barre {
                x: 0.05 * t,
                y: axe - ep / 2.0,
                l: w - 0.1 * t,
                h: ep,
            });
        }
        out.largeur = w;
        out
    }

    fn racine(&self, indice: Option<&Noeud>, corps: &Noeud, n: Niveau) -> Boite {
        let t = self.base * n.echelle();
        let c = self.noeud(corps, n);
        let ep = REGLE * t;
        let haut = c.hauteur.max(0.6 * t) + 0.12 * t;
        let prof = c.profondeur.max(0.05 * t);
        let mut out = Boite::default();
        let mut x0 = 0.0;
        if let Some(i) = indice {
            let ib = self.noeud(i, Niveau::Script2);
            let l = ib.largeur;
            out.poser(ib, 0.0, -haut * 0.55);
            x0 = (l - 0.25 * t).max(0.0);
        }
        let signe = 0.55 * t;
        let pts = vec![
            (x0, -haut * 0.42),
            (x0 + 0.12 * t, -haut * 0.5),
            (x0 + 0.28 * t, prof),
            (x0 + signe, -haut),
            (x0 + signe + c.largeur + 0.1 * t, -haut),
        ];
        out.traces.push(Trace::Trait { points: pts, ep });
        let cl = c.largeur;
        out.poser(c, x0 + signe + 0.05 * t, 0.0);
        out.hauteur = out.hauteur.max(haut + ep);
        out.profondeur = out.profondeur.max(prof + ep);
        out.largeur = x0 + signe + cl + 0.15 * t;
        out
    }

    /// Délimiteur étiré à la hauteur `h` + profondeur `p` (glyphe agrandi).
    fn delim(&self, d: char, h: f32, p: f32, t: f32) -> Boite {
        let total = (h + p).max(t);
        let taille = (total / 1.0).max(t);
        let mut g = self.car(d, false, taille);
        // Centrer sur l'axe.
        let centre = -AXE * t;
        let gh = g.hauteur;
        let gp = g.profondeur;
        let dy = centre - (gp - gh) / 2.0;
        let dy = dy + (gh - gp) / 2.0 - (gh - gp) / 2.0;
        let mut out = Boite::default();
        let decal = (-(gh + gp) / 2.0 + gh) + centre;
        let _ = dy;
        g.decaler(0.0, decal);
        out.largeur = g.largeur;
        out.hauteur = gh - decal;
        out.profondeur = gp + decal;
        out.traces = g.traces;
        out
    }

    fn entoure(&self, gauche: Option<char>, droite: Option<char>, c: Boite, n: Niveau) -> Boite {
        let t = self.base * n.echelle();
        let (h, p) = (c.hauteur, c.profondeur);
        let mut out = Boite::default();
        let mut x = 0.0;
        if let Some(g) = gauche {
            let d = self.delim(g, h, p, t);
            let l = d.largeur;
            out.poser(d, 0.0, 0.0);
            x = l + 0.05 * t;
        }
        let cl = c.largeur;
        out.poser(c, x, 0.0);
        x += cl;
        if let Some(dr) = droite {
            let d = self.delim(dr, h, p, t);
            let l = d.largeur;
            out.poser(d, x + 0.05 * t, 0.0);
            x += l + 0.05 * t;
        }
        out.largeur = x;
        out
    }

    fn matrice(&self, lignes: &[Vec<Noeud>], cas: bool, n: Niveau) -> Boite {
        let t = self.base * n.echelle();
        let nm = if n == Niveau::Centre {
            Niveau::Texte
        } else {
            n
        };
        let cellules: Vec<Vec<Boite>> = lignes
            .iter()
            .map(|l| l.iter().map(|c| self.noeud(c, nm)).collect())
            .collect();
        let ncol = cellules.iter().map(Vec::len).max().unwrap_or(0);
        let mut larg = vec![0.0f32; ncol];
        for l in &cellules {
            for (j, c) in l.iter().enumerate() {
                larg[j] = larg[j].max(c.largeur);
            }
        }
        let haut: Vec<(f32, f32)> = cellules
            .iter()
            .map(|l| {
                (
                    l.iter().map(|c| c.hauteur).fold(ASC * t, f32::max),
                    l.iter().map(|c| c.profondeur).fold(DESC * t, f32::max),
                )
            })
            .collect();
        let ecart_l = 0.25 * t;
        let ecart_c = if cas { 1.0 } else { 0.8 } * t;
        let total: f32 = haut.iter().map(|(h, p)| h + p).sum::<f32>()
            + ecart_l * (haut.len().saturating_sub(1)) as f32;
        let mut out = Boite::default();
        let mut y = -total / 2.0 - AXE * t;
        for (i, l) in cellules.into_iter().enumerate() {
            y += haut[i].0;
            let mut x = 0.0;
            for (j, c) in l.into_iter().enumerate() {
                let cl = c.largeur;
                let dx = if cas { 0.0 } else { (larg[j] - cl) / 2.0 };
                out.poser(c, x + dx, y);
                x += larg[j] + ecart_c;
            }
            y += haut[i].1 + ecart_l;
        }
        out.largeur = larg.iter().sum::<f32>() + ecart_c * (ncol.saturating_sub(1)) as f32;
        out.hauteur = out.hauteur.max(total / 2.0 + AXE * t);
        out.profondeur = out.profondeur.max(total / 2.0 - AXE * t);
        out
    }

    fn noeud(&self, n: &Noeud, niv: Niveau) -> Boite {
        let t = self.base * niv.echelle();
        match n {
            Noeud::Car(c, g) => self.car(
                *c,
                *g == Genre::Ident && c.is_alphabetic() && !c.is_uppercase()
                    || (*g == Genre::Ident && c.is_ascii_uppercase()),
                t,
            ),
            Noeud::Mot(s) => self.mot(s, t),
            Noeud::OpMot { mot } => self.mot(mot, t),
            Noeud::Groupe(l) => self.suite(l, niv),
            Noeud::Espace(e) => Compo::espace(e * t),
            Noeud::Scripts { base, haut, bas } => {
                self.scripts(base, haut.as_deref(), bas.as_deref(), niv)
            }
            Noeud::Frac { num, den, barre } => self.frac(num, den, *barre, niv),
            Noeud::Racine { indice, corps } => self.racine(indice.as_deref(), corps, niv),
            Noeud::GrandOp { c, .. } => {
                let tt = if niv == Niveau::Centre {
                    t * 1.45
                } else {
                    t * 1.1
                };
                let mut b = self.car(*c, false, tt);
                // Centré sur l'axe.
                let decal = (b.hauteur - b.profondeur) / 2.0 - AXE * t;
                b.decaler(0.0, decal);
                b.hauteur -= decal;
                b.profondeur += decal;
                b.largeur += 0.1 * t;
                b
            }
            Noeud::Delim {
                gauche,
                droite,
                corps,
            } => {
                let c = self.noeud(corps, niv);
                self.entoure(*gauche, *droite, c, niv)
            }
            Noeud::Matrice {
                lignes,
                gauche,
                droite,
                cas,
            } => {
                let c = self.matrice(lignes, *cas, niv);
                self.entoure(*gauche, *droite, c, niv)
            }
            Noeud::Accent { c, corps } => {
                let b = self.noeud(corps, niv);
                let mut out = Boite::default();
                let (bl, bh) = (b.largeur, b.hauteur);
                out.poser(b, 0.0, 0.0);
                if *c == '¯' {
                    out.traces.push(Trace::Barre {
                        x: 0.0,
                        y: -bh - 0.1 * t,
                        l: bl,
                        h: REGLE * t,
                    });
                    out.hauteur = bh + 0.15 * t;
                } else {
                    let a = self.car(*c, false, t * 0.8);
                    let al = a.largeur;
                    out.poser(a, (bl - al) / 2.0, -bh + 0.42 * t);
                    out.hauteur = out.hauteur.max(bh + 0.3 * t);
                }
                out.largeur = bl;
                out
            }
        }
    }
}

/// Compose une formule à la taille `taille` (points) ; `centree` pour `$$…$$`.
pub fn composer(n: &Noeud, taille: f32, centree: bool, m: &dyn Mesure) -> Boite {
    let c = Compo { m, base: taille };
    let mut b = c.noeud(
        n,
        if centree {
            Niveau::Centre
        } else {
            Niveau::Texte
        },
    );
    if !b.largeur.is_finite() || !b.hauteur.is_finite() || !b.profondeur.is_finite() {
        return Boite::default();
    }
    b.hauteur = b.hauteur.max(ASC * taille);
    b.profondeur = b.profondeur.max(0.0);
    b
}

// ---------------------------------------------------------------------------
// MathML et OMML
// ---------------------------------------------------------------------------

fn mml(n: &Noeud, o: &mut String, centree: bool) {
    match n {
        Noeud::Car(c, g) => {
            let e = xml_escape(c.encode_utf8(&mut [0; 4]));
            match g {
                Genre::Ident => o.push_str(&format!("<mi>{e}</mi>")),
                Genre::Droit if c.is_ascii_digit() => o.push_str(&format!("<mn>{e}</mn>")),
                Genre::Droit if c.is_alphabetic() => {
                    o.push_str(&format!("<mi mathvariant=\"normal\">{e}</mi>"))
                }
                _ => o.push_str(&format!("<mo>{e}</mo>")),
            }
        }
        Noeud::Mot(s) if s.chars().all(|c| c.is_ascii_digit() || c == '.') => {
            o.push_str(&format!("<mn>{}</mn>", xml_escape(s)))
        }
        Noeud::Mot(s) if FONCTIONS.contains(&s.as_str()) => {
            o.push_str(&format!("<mi>{}</mi><mo>&#x2061;</mo>", xml_escape(s)))
        }
        Noeud::Mot(s) => o.push_str(&format!("<mtext>{}</mtext>", xml_escape(s))),
        Noeud::OpMot { mot } => o.push_str(&format!("<mo>{}</mo>", xml_escape(mot))),
        Noeud::Groupe(l) => {
            o.push_str("<mrow>");
            for x in l {
                mml(x, o, centree);
            }
            o.push_str("</mrow>");
        }
        Noeud::Espace(e) => o.push_str(&format!("<mspace width=\"{:.2}em\"/>", e.max(0.0))),
        Noeud::Scripts { base, haut, bas } => {
            let limites = centree
                && matches!(
                    base.as_ref(),
                    Noeud::GrandOp { limites: true, .. } | Noeud::OpMot { .. }
                );
            let (t3, t_bas, t_haut) = if limites {
                ("munderover", "munder", "mover")
            } else {
                ("msubsup", "msub", "msup")
            };
            let tag = match (haut, bas) {
                (Some(_), Some(_)) => t3,
                (None, Some(_)) => t_bas,
                _ => t_haut,
            };
            o.push_str(&format!("<{tag}>"));
            mml(base, o, centree);
            if let Some(b) = bas {
                mml(b, o, false);
            }
            if let Some(h) = haut {
                mml(h, o, false);
            }
            o.push_str(&format!("</{tag}>"));
        }
        Noeud::Frac { num, den, barre } => {
            o.push_str(if *barre {
                "<mfrac>"
            } else {
                "<mfrac linethickness=\"0\">"
            });
            mml(num, o, false);
            mml(den, o, false);
            o.push_str("</mfrac>");
        }
        Noeud::Racine { indice, corps } => match indice {
            Some(i) => {
                o.push_str("<mroot>");
                mml(corps, o, false);
                mml(i, o, false);
                o.push_str("</mroot>");
            }
            None => {
                o.push_str("<msqrt>");
                mml(corps, o, false);
                o.push_str("</msqrt>");
            }
        },
        Noeud::GrandOp { c, .. } => o.push_str(&format!(
            "<mo largeop=\"true\">{}</mo>",
            xml_escape(c.encode_utf8(&mut [0; 4]))
        )),
        Noeud::Delim {
            gauche,
            droite,
            corps,
        } => {
            o.push_str("<mrow>");
            if let Some(g) = gauche {
                o.push_str(&format!(
                    "<mo fence=\"true\">{}</mo>",
                    xml_escape(g.encode_utf8(&mut [0; 4]))
                ));
            }
            mml(corps, o, centree);
            if let Some(d) = droite {
                o.push_str(&format!(
                    "<mo fence=\"true\">{}</mo>",
                    xml_escape(d.encode_utf8(&mut [0; 4]))
                ));
            }
            o.push_str("</mrow>");
        }
        Noeud::Matrice {
            lignes,
            gauche,
            droite,
            cas,
        } => {
            o.push_str("<mrow>");
            if let Some(g) = gauche {
                o.push_str(&format!(
                    "<mo>{}</mo>",
                    xml_escape(g.encode_utf8(&mut [0; 4]))
                ));
            }
            o.push_str(if *cas {
                "<mtable columnalign=\"left\">"
            } else {
                "<mtable>"
            });
            for l in lignes {
                o.push_str("<mtr>");
                for c in l {
                    o.push_str("<mtd>");
                    mml(c, o, false);
                    o.push_str("</mtd>");
                }
                o.push_str("</mtr>");
            }
            o.push_str("</mtable>");
            if let Some(d) = droite {
                o.push_str(&format!(
                    "<mo>{}</mo>",
                    xml_escape(d.encode_utf8(&mut [0; 4]))
                ));
            }
            o.push_str("</mrow>");
        }
        Noeud::Accent { c, corps } => {
            o.push_str("<mover accent=\"true\">");
            mml(corps, o, centree);
            o.push_str(&format!(
                "<mo>{}</mo></mover>",
                xml_escape(c.encode_utf8(&mut [0; 4]))
            ));
        }
    }
}

/// La formule en MathML (`<math>…</math>`), avec le LaTeX d'origine en annotation.
pub fn mathml(tex: &str, centree: bool) -> String {
    let n = lire(tex);
    let mut o = String::new();
    mml(&n, &mut o, centree);
    format!(
        "<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"{}\"><semantics>{o}<annotation encoding=\"application/x-tex\">{}</annotation></semantics></math>",
        if centree { "block" } else { "inline" },
        xml_escape(tex.trim())
    )
}

fn omml_texte(s: &str, droit: bool) -> String {
    let sty = if droit {
        "<m:rPr><m:sty m:val=\"p\"/></m:rPr>"
    } else {
        ""
    };
    format!(
        "<m:r>{sty}<m:t xml:space=\"preserve\">{}</m:t></m:r>",
        xml_escape(s)
    )
}

fn omml_e(n: &Noeud, o: &mut String) {
    o.push_str("<m:e>");
    omml(n, o);
    o.push_str("</m:e>");
}

fn omml_liste(l: &[Noeud], o: &mut String) {
    let mut i = 0;
    while i < l.len() {
        // Un grand opérateur prend l'expression qui le suit comme opérande.
        let (op, haut, bas) = match &l[i] {
            Noeud::GrandOp { c, limites } => (Some((*c, *limites)), None, None),
            Noeud::Scripts { base, haut, bas } => match base.as_ref() {
                Noeud::GrandOp { c, limites } => {
                    (Some((*c, *limites)), haut.as_deref(), bas.as_deref())
                }
                _ => (None, None, None),
            },
            _ => (None, None, None),
        };
        if let Some((c, limites)) = op {
            let loc = if limites { "undOvr" } else { "subSup" };
            o.push_str(&format!(
                "<m:nary><m:naryPr><m:chr m:val=\"{}\"/><m:limLoc m:val=\"{loc}\"/>{}{}</m:naryPr><m:sub>",
                xml_escape(c.encode_utf8(&mut [0; 4])),
                if bas.is_none() { "<m:subHide m:val=\"1\"/>" } else { "" },
                if haut.is_none() { "<m:supHide m:val=\"1\"/>" } else { "" },
            ));
            if let Some(b) = bas {
                omml(b, o);
            }
            o.push_str("</m:sub><m:sup>");
            if let Some(h) = haut {
                omml(h, o);
            }
            o.push_str("</m:sup><m:e>");
            if let Some(suite) = l.get(i + 1) {
                omml(suite, o);
                i += 1;
            }
            o.push_str("</m:e></m:nary>");
        } else {
            omml(&l[i], o);
        }
        i += 1;
    }
}

fn omml(n: &Noeud, o: &mut String) {
    match n {
        Noeud::Car(c, g) => o.push_str(&omml_texte(c.encode_utf8(&mut [0; 4]), *g != Genre::Ident)),
        Noeud::Mot(s) | Noeud::OpMot { mot: s } => o.push_str(&omml_texte(s, true)),
        Noeud::Groupe(l) => omml_liste(l, o),
        Noeud::Espace(e) => {
            if *e > 0.5 {
                o.push_str(&omml_texte("\u{2003}", true));
            } else if *e > 0.0 {
                o.push_str(&omml_texte("\u{2009}", true));
            }
        }
        Noeud::Scripts { base, haut, bas } => {
            if matches!(base.as_ref(), Noeud::GrandOp { .. }) {
                omml_liste(std::slice::from_ref(n), o);
                return;
            }
            if let Noeud::OpMot { mot } = base.as_ref() {
                o.push_str("<m:func><m:fName><m:limLow><m:e>");
                o.push_str(&omml_texte(mot, true));
                o.push_str("</m:e><m:lim>");
                if let Some(b) = bas {
                    omml(b, o);
                }
                o.push_str("</m:lim></m:limLow></m:fName><m:e/></m:func>");
                return;
            }
            let tag = match (haut, bas) {
                (Some(_), Some(_)) => "m:sSubSup",
                (None, Some(_)) => "m:sSub",
                _ => "m:sSup",
            };
            o.push_str(&format!("<{tag}>"));
            omml_e(base, o);
            if let Some(b) = bas {
                o.push_str("<m:sub>");
                omml(b, o);
                o.push_str("</m:sub>");
            }
            if let Some(h) = haut {
                o.push_str("<m:sup>");
                omml(h, o);
                o.push_str("</m:sup>");
            }
            o.push_str(&format!("</{tag}>"));
        }
        Noeud::Frac { num, den, barre } => {
            o.push_str("<m:f>");
            if !barre {
                o.push_str("<m:fPr><m:type m:val=\"noBar\"/></m:fPr>");
            }
            o.push_str("<m:num>");
            omml(num, o);
            o.push_str("</m:num><m:den>");
            omml(den, o);
            o.push_str("</m:den></m:f>");
        }
        Noeud::Racine { indice, corps } => {
            o.push_str("<m:rad>");
            match indice {
                Some(i) => {
                    o.push_str("<m:deg>");
                    omml(i, o);
                    o.push_str("</m:deg>");
                }
                None => o.push_str("<m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/>"),
            }
            omml_e(corps, o);
            o.push_str("</m:rad>");
        }
        Noeud::GrandOp { .. } => omml_liste(std::slice::from_ref(n), o),
        Noeud::Delim {
            gauche,
            droite,
            corps,
        } => {
            let c = |d: &Option<char>| {
                d.map(|c| xml_escape(c.encode_utf8(&mut [0; 4])))
                    .unwrap_or_default()
            };
            o.push_str(&format!(
                "<m:d><m:dPr><m:begChr m:val=\"{}\"/><m:endChr m:val=\"{}\"/></m:dPr>",
                c(gauche),
                c(droite)
            ));
            omml_e(corps, o);
            o.push_str("</m:d>");
        }
        Noeud::Matrice {
            lignes,
            gauche,
            droite,
            ..
        } => {
            let c = |d: &Option<char>| {
                d.map(|c| xml_escape(c.encode_utf8(&mut [0; 4])))
                    .unwrap_or_default()
            };
            let avec = gauche.is_some() || droite.is_some();
            if avec {
                o.push_str(&format!(
                    "<m:d><m:dPr><m:begChr m:val=\"{}\"/><m:endChr m:val=\"{}\"/></m:dPr><m:e>",
                    c(gauche),
                    c(droite)
                ));
            }
            o.push_str("<m:m>");
            for l in lignes {
                o.push_str("<m:mr>");
                for x in l {
                    omml_e(x, o);
                }
                o.push_str("</m:mr>");
            }
            o.push_str("</m:m>");
            if avec {
                o.push_str("</m:e></m:d>");
            }
        }
        Noeud::Accent { c, corps } => {
            let ch = match c {
                '¯' => '\u{0305}',
                '^' => '\u{0302}',
                '~' => '\u{0303}',
                '→' => '\u{20D7}',
                '˙' => '\u{0307}',
                '¨' => '\u{0308}',
                c => *c,
            };
            o.push_str(&format!(
                "<m:acc><m:accPr><m:chr m:val=\"{}\"/></m:accPr>",
                ch
            ));
            omml_e(corps, o);
            o.push_str("</m:acc>");
        }
    }
}

/// La formule en OMML (formules de Word) ; `centree` pour une formule seule sur sa ligne.
pub fn omml_formule(tex: &str, centree: bool) -> String {
    let mut o = String::new();
    omml(&lire(tex), &mut o);
    if centree {
        format!("<m:oMathPara><m:oMath>{o}</m:oMath></m:oMathPara>")
    } else {
        format!("<m:oMath>{o}</m:oMath>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(_: char, _: bool) -> f32 {
        0.5
    }

    fn glyphes(b: &Boite) -> String {
        b.traces
            .iter()
            .filter_map(|t| match t {
                Trace::Glyphe { c, .. } => Some(*c),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn lecture() {
        assert_eq!(
            lire("x^2"),
            Noeud::Groupe(vec![Noeud::Scripts {
                base: Box::new(Noeud::Car('x', Genre::Ident)),
                haut: Some(Box::new(Noeud::Car('2', Genre::Droit))),
                bas: None
            }])
        );
        let n = lire("\\frac{a+b}{2}");
        assert!(matches!(&n, Noeud::Groupe(l) if matches!(l[0], Noeud::Frac { .. })));
        let n = lire("\\begin{pmatrix} 1 & 2 \\\\ 3 & 4 \\end{pmatrix}");
        let Noeud::Groupe(l) = n else { panic!() };
        let Noeud::Matrice { lignes, .. } = &l[0] else {
            panic!("{l:?}")
        };
        assert_eq!(lignes.len(), 2);
        assert_eq!(lignes[0].len(), 2);
    }

    #[test]
    fn composition() {
        let b = composer(&lire("\\sum_{i=1}^{n} \\alpha_i"), 12.0, true, &m);
        let g = glyphes(&b);
        assert!(g.contains('∑') && g.contains('α') && g.contains('n'));
        assert!(b.hauteur > 12.0 && b.profondeur > 3.0);
        let f = composer(&lire("\\frac{1}{2}"), 10.0, false, &m);
        assert!(f.traces.iter().any(|t| matches!(t, Trace::Barre { .. })));
        let r = composer(&lire("\\sqrt[3]{x}"), 10.0, false, &m);
        assert!(r.traces.iter().any(|t| matches!(t, Trace::Trait { .. })));
        // Exposant plus petit et plus haut.
        let e = composer(&lire("x^2"), 10.0, false, &m);
        let ts: Vec<(f32, f32)> = e
            .traces
            .iter()
            .filter_map(|t| match t {
                Trace::Glyphe { y, taille, .. } => Some((*y, *taille)),
                _ => None,
            })
            .collect();
        assert!(ts[1].0 < 0.0 && ts[1].1 < ts[0].1);
    }

    #[test]
    fn mathml_et_omml() {
        let x = mathml("\\frac{a}{b} + \\sqrt{x}", false);
        assert!(x.contains("<mfrac><mi>a</mi><mi>b</mi></mfrac>") && x.contains("<msqrt>"));
        let o = omml_formule("\\sum_{i=1}^n i", true);
        assert!(o.contains("<m:nary>") && o.contains("∑"));
        assert!(omml_formule("x_1^2", false).contains("<m:sSubSup>"));
    }

    #[test]
    fn fuzz_formules() {
        let mut e = 0x2545_F491_4F6C_DD1Du64;
        let bouts = [
            "\\frac",
            "{",
            "}",
            "^",
            "_",
            "x",
            "2",
            "\\sqrt",
            "[",
            "]",
            "\\left",
            "\\right",
            "(",
            ")",
            "\\begin{pmatrix}",
            "\\end{pmatrix}",
            "&",
            "\\\\",
            "\\sum",
            "\\int",
            "\\hat",
            "\\text{é}",
            "\\",
            "α",
            "😀",
            "\\begin{cases}",
            "\\end",
            "\\binom",
            "\\lim",
            ".",
            "-",
            "=",
            "\\,",
            "\\inconnu",
        ];
        for _ in 0..3000 {
            e ^= e << 13;
            e ^= e >> 7;
            e ^= e << 17;
            let n = (e % 30) as usize;
            let mut s = String::new();
            let mut k = e;
            for _ in 0..n {
                k = k
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                s.push_str(bouts[(k >> 33) as usize % bouts.len()]);
            }
            let b = composer(&lire(&s), 12.0, e % 2 == 0, &m);
            assert!(b.largeur.is_finite() && b.hauteur.is_finite());
            let _ = mathml(&s, true);
            let _ = omml_formule(&s, false);
        }
        // Imbrication extrême : pas de débordement de pile.
        let profond = "{".repeat(5000) + &"x^".repeat(3000) + &"\\frac{".repeat(2000);
        let _ = composer(&lire(&profond), 12.0, true, &m);
        let _ = mathml(&profond, true);
    }
}
