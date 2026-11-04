//! Remplissage instantané (Ctrl+E) et saisie semi-automatique.
//!
//! À partir de quelques exemples tapés à côté d'une ou plusieurs colonnes, on
//! cherche le « programme » le plus simple qui donne chaque exemple : une suite
//! de morceaux, chacun étant un texte fixe ou un extrait d'une colonne source
//! (mot n°k depuis le début ou la fin, champ entre délimiteurs, suite de
//! chiffres, initiale, sous-chaîne), éventuellement changé de casse.
//!
//! La recherche est un plus court chemin (Dijkstra) dans le produit des
//! positions atteintes dans chaque exemple : un morceau n'est retenu que s'il
//! convient à tous les exemples à la fois. Le coût favorise les programmes
//! courts, les extraits plutôt que les textes fixes, et les mots plutôt que les
//! sous-chaînes. Tout est borné (longueurs, nombre d'états) : aucune saisie ne
//! peut faire planter ni bloquer la recherche.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// Longueur maximale (en octets) d'un exemple ou d'une valeur source.
const MAX_LEN: usize = 256;
/// Nombre maximal d'exemples pris en compte.
const MAX_EXAMPLES: usize = 6;
/// Nombre maximal de colonnes sources.
const MAX_COLS: usize = 8;
/// Nombre maximal d'états explorés par la recherche.
const MAX_STATES: usize = 60_000;
/// Indices de jetons essayés, depuis le début et depuis la fin.
const MAX_INDEX: usize = 8;
/// Longueur maximale des sous-chaînes par position.
const MAX_SUB: usize = 24;

/// Découpage d'une valeur source en jetons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Scheme {
    /// La valeur entière.
    Whole,
    /// Suites de lettres et de chiffres.
    Words,
    /// Suites de lettres.
    Letters,
    /// Suites de chiffres.
    Digits,
    /// Champs séparés par ce caractère (espaces retirés autour).
    Split(char),
    /// Tous les chiffres de la valeur, mis bout à bout.
    DigitString,
}

/// Changement de casse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Case {
    Same,
    Upper,
    Lower,
    /// Première lettre en majuscule, le reste en minuscules.
    Capital,
    /// Première lettre de chaque mot en majuscule.
    Title,
}

/// Partie du jeton retenue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Slice {
    All,
    /// Premier caractère.
    Initial,
    /// Caractères `start..start + len`.
    Sub(u8, u8),
}

/// Un extrait d'une colonne source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Atom {
    col: usize,
    scheme: Scheme,
    /// Indice du jeton : depuis le début (0, 1…) ou la fin (-1, -2…).
    index: i32,
    case: Case,
    slice: Slice,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Part {
    Const(String),
    Atom(Atom),
}

/// Transformation trouvée par [`FlashProgram::infer`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlashProgram {
    parts: Vec<Part>,
}

const DELIMS: [char; 13] = [
    ' ', ',', ';', '@', '.', '-', '/', '_', '|', ':', '\t', '(', ')',
];

fn tokens(s: &str, scheme: Scheme) -> Vec<String> {
    let runs = |keep: fn(char) -> bool| -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        for c in s.chars() {
            if keep(c) {
                cur.push(c);
            } else if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        out
    };
    match scheme {
        Scheme::Whole => vec![s.to_string()],
        Scheme::Words => runs(char::is_alphanumeric),
        Scheme::Letters => runs(char::is_alphabetic),
        Scheme::Digits => runs(|c| c.is_ascii_digit()),
        Scheme::Split(d) => s.split(d).map(|f| f.trim().to_string()).collect(),
        Scheme::DigitString => vec![s.chars().filter(char::is_ascii_digit).collect()],
    }
}

fn capitalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut first = true;
    for c in s.chars() {
        if first && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            first = false;
        } else if first {
            out.push(c);
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

fn title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut start = true;
    for c in s.chars() {
        if c.is_alphanumeric() {
            if start {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            start = false;
        } else {
            out.push(c);
            start = c != '\'';
        }
    }
    out
}

fn apply_case(s: &str, case: Case) -> String {
    match case {
        Case::Same => s.to_string(),
        Case::Upper => s.to_uppercase(),
        Case::Lower => s.to_lowercase(),
        Case::Capital => capitalize(s),
        Case::Title => title(s),
    }
}

impl Atom {
    fn eval(&self, row: &[String]) -> Option<String> {
        let src = row.get(self.col)?;
        let toks = tokens(src, self.scheme);
        let tok = if self.index >= 0 {
            toks.get(self.index as usize)?
        } else {
            let back = self.index.unsigned_abs() as usize;
            toks.get(toks.len().checked_sub(back)?)?
        };
        let cut: String = match self.slice {
            Slice::All => tok.clone(),
            Slice::Initial => tok.chars().next()?.to_string(),
            Slice::Sub(start, len) => {
                let chars: Vec<char> = tok.chars().collect();
                let (a, n) = (usize::from(start), usize::from(len));
                if a + n > chars.len() {
                    return None;
                }
                chars[a..a + n].iter().collect()
            }
        };
        let out = apply_case(&cut, self.case);
        (!out.is_empty()).then_some(out)
    }

    /// Coût en millièmes : plus c'est simple, moins c'est cher.
    fn cost(&self) -> u32 {
        let scheme = match self.scheme {
            Scheme::Whole => 0,
            Scheme::Split(' ') => 30,
            Scheme::Words => 50,
            Scheme::Split(_) => 100,
            Scheme::Letters | Scheme::Digits => 150,
            Scheme::DigitString => 300,
        };
        let index = if self.index >= 0 {
            20 * self.index as u32
        } else {
            15 + 20 * (self.index.unsigned_abs() - 1)
        };
        let case = match self.case {
            Case::Same => 0,
            Case::Upper | Case::Lower => 100,
            Case::Capital => 120,
            Case::Title => 150,
        };
        let slice = match self.slice {
            Slice::All => 0,
            Slice::Initial => 200,
            Slice::Sub(start, _) => 300 + 10 * u32::from(start),
        };
        1000 + scheme + index + case + slice
    }
}

fn const_cost(s: &str) -> u32 {
    let alnum = s.chars().filter(|c| c.is_alphanumeric()).count() as u32;
    let other = s.chars().count() as u32 - alnum;
    if alnum > 0 {
        2000 + 1000 * alnum + 100 * other
    } else {
        600 + 100 * other
    }
}

/// Tronque une chaîne à `MAX_LEN` octets (sur une limite de caractère).
fn clip(s: &str) -> &str {
    if s.len() <= MAX_LEN {
        return s;
    }
    let mut end = MAX_LEN;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Tous les extraits possibles des colonnes d'un exemple.
fn candidate_atoms(row: &[String]) -> Vec<Atom> {
    let mut out = Vec::new();
    for (col, src) in row.iter().enumerate().take(MAX_COLS) {
        if src.is_empty() {
            continue;
        }
        let mut schemes = vec![
            Scheme::Whole,
            Scheme::Words,
            Scheme::Letters,
            Scheme::Digits,
            Scheme::DigitString,
        ];
        for d in DELIMS {
            if src.contains(d) {
                schemes.push(Scheme::Split(d));
            }
        }
        for scheme in schemes {
            let toks = tokens(src, scheme);
            let n = toks.len().min(MAX_INDEX);
            let mut indices: Vec<i32> = (0..n as i32).collect();
            indices.extend((1..=n as i32).map(|k| -k));
            for &index in &indices {
                for case in [
                    Case::Same,
                    Case::Upper,
                    Case::Lower,
                    Case::Capital,
                    Case::Title,
                ] {
                    for slice in [Slice::All, Slice::Initial] {
                        out.push(Atom {
                            col,
                            scheme,
                            index,
                            case,
                            slice,
                        });
                    }
                }
            }
            // Sous-chaînes par position : la valeur entière, ses chiffres et
            // ses mots.
            if matches!(scheme, Scheme::Whole | Scheme::DigitString | Scheme::Words) {
                for &index in &indices {
                    let Some(tok) = (if index >= 0 {
                        toks.get(index as usize)
                    } else {
                        toks.len()
                            .checked_sub(index.unsigned_abs() as usize)
                            .and_then(|i| toks.get(i))
                    }) else {
                        continue;
                    };
                    let len = tok.chars().count().min(MAX_SUB);
                    for start in 0..len {
                        for l in 1..=(len - start) {
                            if start == 0 && l == len {
                                continue;
                            }
                            for case in [Case::Same, Case::Upper, Case::Lower] {
                                out.push(Atom {
                                    col,
                                    scheme,
                                    index,
                                    case,
                                    slice: Slice::Sub(start as u8, l as u8),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

impl FlashProgram {
    /// Cherche la transformation la plus simple qui donne chaque exemple.
    /// Chaque exemple est (valeurs des colonnes sources, résultat attendu).
    pub fn infer(examples: &[(Vec<String>, String)]) -> Option<FlashProgram> {
        let examples: Vec<(Vec<String>, String)> = examples
            .iter()
            .filter(|(_, out)| !out.is_empty())
            .take(MAX_EXAMPLES)
            .map(|(row, out)| {
                (
                    row.iter()
                        .take(MAX_COLS)
                        .map(|s| clip(s).to_string())
                        .collect(),
                    clip(out).to_string(),
                )
            })
            .collect();
        let first = examples.first()?;
        let outs: Vec<&str> = examples.iter().map(|(_, o)| o.as_str()).collect();

        // Extraits du premier exemple, gardés s'ils donnent un morceau de
        // chaque résultat ; un seul (le moins cher) par suite de valeurs.
        let mut best: HashMap<Vec<String>, Atom> = HashMap::new();
        'atoms: for atom in candidate_atoms(&first.0) {
            let mut vals = Vec::with_capacity(examples.len());
            for (row, out) in &examples {
                match atom.eval(row) {
                    Some(v) if out.contains(&v) => vals.push(v),
                    _ => continue 'atoms,
                }
            }
            match best.get(&vals) {
                Some(old) if old.cost() <= atom.cost() => {}
                _ => {
                    best.insert(vals, atom);
                }
            }
        }
        let atoms: Vec<(Vec<String>, Atom)> = best.into_iter().collect();

        // Dijkstra sur les positions atteintes dans chaque résultat.
        let start = vec![0usize; examples.len()];
        let goal: Vec<usize> = outs.iter().map(|o| o.len()).collect();
        let mut dist: HashMap<Vec<usize>, u32> = HashMap::new();
        let mut back: HashMap<Vec<usize>, (Vec<usize>, Part)> = HashMap::new();
        let mut heap = BinaryHeap::new();
        dist.insert(start.clone(), 0);
        heap.push(Reverse((0u32, start)));
        let mut popped = 0usize;
        while let Some(Reverse((d, state))) = heap.pop() {
            if dist.get(&state).is_some_and(|&best| best < d) {
                continue;
            }
            if state == goal {
                let mut parts = Vec::new();
                let mut cur = state;
                while let Some((prev, part)) = back.get(&cur) {
                    parts.push(part.clone());
                    cur = prev.clone();
                }
                parts.reverse();
                return Some(FlashProgram {
                    parts: merge_consts(parts),
                });
            }
            popped += 1;
            if popped > MAX_STATES {
                return None;
            }
            let mut relax = |next: Vec<usize>, cost: u32, part: Part| {
                let nd = d.saturating_add(cost);
                if dist.get(&next).is_none_or(|&old| nd < old) {
                    dist.insert(next.clone(), nd);
                    back.insert(next.clone(), (state.clone(), part));
                    heap.push(Reverse((nd, next)));
                }
            };
            for (vals, atom) in &atoms {
                let ok = vals
                    .iter()
                    .zip(&state)
                    .zip(&outs)
                    .all(|((v, &p), o)| o[p..].starts_with(v.as_str()));
                if ok {
                    let next = state.iter().zip(vals).map(|(&p, v)| p + v.len()).collect();
                    relax(next, atom.cost(), Part::Atom(*atom));
                }
            }
            // Textes fixes : préfixes communs à tous les résultats.
            let rest0 = &outs[0][state[0]..];
            let mut common = rest0.len();
            for (o, &p) in outs.iter().zip(&state).skip(1) {
                let r = &o[p..];
                common = common.min(
                    rest0
                        .char_indices()
                        .zip(r.chars())
                        .find(|((_, a), b)| a != b)
                        .map_or(rest0.len().min(r.len()), |((i, _), _)| i),
                );
            }
            for (i, c) in rest0[..common].char_indices().take(MAX_SUB) {
                let k = i + c.len_utf8();
                let text = &rest0[..k];
                let next = state.iter().map(|&p| p + k).collect();
                relax(next, const_cost(text), Part::Const(text.to_string()));
            }
        }
        None
    }

    /// Applique la transformation à une ligne (`None` si un extrait manque).
    pub fn apply(&self, row: &[String]) -> Option<String> {
        let mut out = String::new();
        for p in &self.parts {
            match p {
                Part::Const(s) => out.push_str(s),
                Part::Atom(a) => out.push_str(&a.eval(row)?),
            }
        }
        Some(out)
    }

    /// Le programme n'est-il fait que de textes fixes ?
    pub fn is_constant(&self) -> bool {
        self.parts.iter().all(|p| matches!(p, Part::Const(_)))
    }

    /// Le programme recopie-t-il simplement une colonne telle quelle ?
    pub fn is_plain_copy(&self) -> bool {
        matches!(
            self.parts.as_slice(),
            [Part::Atom(Atom {
                scheme: Scheme::Whole,
                case: Case::Same,
                slice: Slice::All,
                ..
            })]
        )
    }
}

fn merge_consts(parts: Vec<Part>) -> Vec<Part> {
    let mut out: Vec<Part> = Vec::with_capacity(parts.len());
    for p in parts {
        match (out.last_mut(), p) {
            (Some(Part::Const(a)), Part::Const(b)) => a.push_str(&b),
            (_, p) => out.push(p),
        }
    }
    out
}

/// Remplissage instantané : déduit la transformation des exemples et
/// l'applique aux lignes données (`None` pour une ligne où elle ne s'applique
/// pas). `None` si aucune transformation ne convient à tous les exemples.
pub fn flash_fill(
    examples: &[(Vec<String>, String)],
    rows: &[Vec<String>],
) -> Option<Vec<Option<String>>> {
    let program = FlashProgram::infer(examples)?;
    Some(rows.iter().map(|r| program.apply(r)).collect())
}

/// Saisie semi-automatique : la valeur existante qui complète `prefix` (sans
/// tenir compte de la casse), comme l'« AutoSaisie » de LibreOffice. La plus
/// fréquente l'emporte, puis la plus courte, puis l'ordre alphabétique.
/// `None` si rien ne complète le début tapé.
pub fn autocomplete<'a>(
    candidates: impl IntoIterator<Item = &'a str>,
    prefix: &str,
) -> Option<String> {
    if prefix.trim().is_empty() || prefix.len() > MAX_LEN {
        return None;
    }
    let p: Vec<char> = prefix.chars().flat_map(char::to_lowercase).collect();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for c in candidates {
        if c.len() <= prefix.len() || c.len() > 4 * MAX_LEN {
            continue;
        }
        let lower: Vec<char> = c
            .chars()
            .flat_map(char::to_lowercase)
            .take(p.len() + 1)
            .collect();
        if lower.len() > p.len() && lower[..p.len()] == p[..] {
            *counts.entry(c).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .min_by(|(a, na), (b, nb)| {
            nb.cmp(na)
                .then(a.chars().count().cmp(&b.chars().count()))
                .then(a.cmp(b))
        })
        .map(|(c, _)| c.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(rows: &[(&[&str], &str)]) -> Vec<(Vec<String>, String)> {
        rows.iter()
            .map(|(r, o)| (r.iter().map(|s| s.to_string()).collect(), o.to_string()))
            .collect()
    }

    fn row(r: &[&str]) -> Vec<String> {
        r.iter().map(|s| s.to_string()).collect()
    }

    fn fill(examples: &[(&[&str], &str)], input: &[&str]) -> Option<String> {
        FlashProgram::infer(&ex(examples))?.apply(&row(input))
    }

    #[test]
    fn prenom_et_nom() {
        let e: &[(&[&str], &str)] = &[(&["Jean Dupont"], "Jean")];
        assert_eq!(fill(e, &["Marie Curie"]).as_deref(), Some("Marie"));
        let e: &[(&[&str], &str)] = &[(&["Jean Dupont"], "Dupont")];
        assert_eq!(fill(e, &["Marie Curie"]).as_deref(), Some("Curie"));
        let e: &[(&[&str], &str)] = &[(&["Jean Dupont"], "DUPONT Jean")];
        assert_eq!(fill(e, &["Marie Curie"]).as_deref(), Some("CURIE Marie"));
    }

    #[test]
    fn initiales() {
        let e: &[(&[&str], &str)] = &[(&["Jean Dupont"], "J.D.")];
        assert_eq!(fill(e, &["Marie Curie"]).as_deref(), Some("M.C."));
        let e: &[(&[&str], &str)] = &[(&["Jean Dupont"], "J. Dupont")];
        assert_eq!(fill(e, &["Marie Curie"]).as_deref(), Some("M. Curie"));
    }

    #[test]
    fn domaine_de_courriel() {
        let e: &[(&[&str], &str)] = &[(&["jean.dupont@exemple.fr"], "exemple.fr")];
        assert_eq!(
            fill(e, &["marie@labo-curie.org"]).as_deref(),
            Some("labo-curie.org")
        );
        let e: &[(&[&str], &str)] = &[
            (&["jean.dupont@exemple.fr"], "jean.dupont"),
            (&["marie@curie.org"], "marie"),
        ];
        assert_eq!(fill(e, &["a.b.c@d.e"]).as_deref(), Some("a.b.c"));
    }

    #[test]
    fn codes_postaux_et_nombres() {
        let e: &[(&[&str], &str)] = &[
            (&["12 rue de la Paix, 75002 Paris"], "75002"),
            (&["3 avenue Foch, 69006 Lyon"], "69006"),
        ];
        assert_eq!(
            fill(e, &["8 bd Victor Hugo, 06000 Nice"]).as_deref(),
            Some("06000")
        );
        let e: &[(&[&str], &str)] = &[(&["Commande n° 4512 (urgent)"], "4512")];
        assert_eq!(fill(e, &["Commande n° 77 (normal)"]).as_deref(), Some("77"));
    }

    #[test]
    fn casse() {
        let e: &[(&[&str], &str)] = &[(&["jean dupont"], "Jean Dupont")];
        assert_eq!(fill(e, &["marie curie"]).as_deref(), Some("Marie Curie"));
        let e: &[(&[&str], &str)] = &[(&["Paris"], "PARIS"), (&["Lyon"], "LYON")];
        assert_eq!(
            fill(e, &["Saint-Étienne"]).as_deref(),
            Some("SAINT-ÉTIENNE")
        );
        let e: &[(&[&str], &str)] = &[(&["BONJOUR"], "bonjour")];
        assert_eq!(fill(e, &["Au Revoir"]).as_deref(), Some("au revoir"));
    }

    #[test]
    fn concatenation_de_colonnes() {
        let e: &[(&[&str], &str)] = &[(&["Jean", "Dupont"], "Jean Dupont")];
        assert_eq!(fill(e, &["Marie", "Curie"]).as_deref(), Some("Marie Curie"));
        let e: &[(&[&str], &str)] = &[(&["Jean", "Dupont", "Paris"], "Dupont, Jean (Paris)")];
        assert_eq!(
            fill(e, &["Marie", "Curie", "Varsovie"]).as_deref(),
            Some("Curie, Marie (Varsovie)")
        );
        let e: &[(&[&str], &str)] = &[(&["Jean", "Dupont"], "jean.dupont@exemple.fr")];
        assert_eq!(
            fill(e, &["Marie", "Curie"]).as_deref(),
            Some("marie.curie@exemple.fr")
        );
    }

    #[test]
    fn dates_et_telephones() {
        let e: &[(&[&str], &str)] = &[(&["2026-03-15"], "15/03/2026")];
        assert_eq!(fill(e, &["2025-12-01"]).as_deref(), Some("01/12/2025"));
        let e: &[(&[&str], &str)] = &[(&["0612345678"], "06 12 34 56 78")];
        assert_eq!(fill(e, &["0798765432"]).as_deref(), Some("07 98 76 54 32"));
        let e: &[(&[&str], &str)] = &[(&["06.12.34.56.78"], "06 12 34 56 78")];
        assert_eq!(
            fill(e, &["01.23.45.67.89"]).as_deref(),
            Some("01 23 45 67 89")
        );
    }

    #[test]
    fn plusieurs_exemples_levent_l_ambiguite() {
        // Un seul exemple : « Martin » peut être le 2e mot ou le dernier.
        let e: &[(&[&str], &str)] = &[
            (&["Jean Martin"], "Martin"),
            (&["Anne Marie Durand"], "Durand"),
        ];
        assert_eq!(fill(e, &["Paul Henri Simon"]).as_deref(), Some("Simon"));
    }

    #[test]
    fn exemples_incoherents() {
        let e = ex(&[(&["abc"], "xyz1"), (&["def"], "uvw2")]);
        // Des textes fixes différents : rien ne convient aux deux.
        assert!(FlashProgram::infer(&e).is_none());
        assert!(FlashProgram::infer(&[]).is_none());
        let p = FlashProgram::infer(&ex(&[(&["a b"], "a")])).unwrap();
        assert_eq!(p.apply(&row(&[""])), None);
        assert!(!p.is_constant());
    }

    #[test]
    fn remplissage_complet() {
        let examples = ex(&[(&["Jean Dupont"], "Jean")]);
        let rows = vec![row(&["Marie Curie"]), row(&[""]), row(&["Victor"])];
        assert_eq!(
            flash_fill(&examples, &rows),
            Some(vec![Some("Marie".into()), None, Some("Victor".into())])
        );
    }

    #[test]
    fn saisie_semi_automatique() {
        let vals = ["Paris", "Pau", "Paris", "Lyon", "parisien"];
        assert_eq!(autocomplete(vals, "pa").as_deref(), Some("Paris"));
        assert_eq!(autocomplete(vals, "Ly").as_deref(), Some("Lyon"));
        assert_eq!(autocomplete(vals, "Lyon"), None);
        assert_eq!(autocomplete(vals, "x"), None);
        assert_eq!(autocomplete(vals, ""), None);
        assert_eq!(
            autocomplete(["Éric", "Émile"], "é").as_deref(),
            Some("Éric")
        );
    }

    /// Générateur pseudo-aléatoire déterministe (pas de dépendance externe).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn text(&mut self, max: usize) -> String {
            const ALPHABET: &[char] = &[
                'a', 'B', 'é', ' ', '.', '@', '-', '1', '9', '0', '/', ',', 'Ω', '😀', '\u{301}',
                '\t', '(', ')', 'ß', 'İ',
            ];
            let n = (self.next() % (max as u64 + 1)) as usize;
            (0..n)
                .map(|_| ALPHABET[(self.next() % ALPHABET.len() as u64) as usize])
                .collect()
        }
    }

    #[test]
    fn fuzz_sans_panique() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for _ in 0..300 {
            let cols = 1 + (rng.next() % 3) as usize;
            let n = 1 + (rng.next() % 3) as usize;
            let examples: Vec<(Vec<String>, String)> = (0..n)
                .map(|_| ((0..cols).map(|_| rng.text(14)).collect(), rng.text(10)))
                .collect();
            if let Some(p) = FlashProgram::infer(&examples) {
                for (r, out) in examples.iter().filter(|(_, o)| !o.is_empty()) {
                    assert_eq!(p.apply(r).as_deref(), Some(out.as_str()), "{examples:?}");
                }
                let other: Vec<String> = (0..cols).map(|_| rng.text(20)).collect();
                let _ = p.apply(&other);
            }
            let pre = rng.text(3);
            let cands: Vec<String> = (0..5).map(|_| rng.text(8)).collect();
            let _ = autocomplete(cands.iter().map(String::as_str), &pre);
        }
        // Entrées très longues : bornées.
        let long = "ab ".repeat(5000);
        let _ = FlashProgram::infer(&[(vec![long.clone()], long)]);
    }
}
