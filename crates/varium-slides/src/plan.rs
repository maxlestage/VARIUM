//! Mode plan : toute la présentation sous forme d'un texte en retrait, comme le
//! mode Plan d'Impress ou de Keynote.
//!
//! Chaque ligne sans retrait est le titre d'une diapositive ; les lignes en
//! retrait (une tabulation par niveau, ou quatre espaces) sont son contenu :
//! niveau 1 pour une puce, niveau 2 pour une sous-puce… Pour une diapositive
//! de titre ou un intercalaire, les lignes en retrait sont le sous-titre ; pour
//! une citation, la citation. Une ligne `||` en retrait sépare les deux colonnes.
//!
//! [`Deck::apply_outline`] relit un plan modifié : les diapositives sont
//! retrouvées par leur titre (plus longue sous-suite commune, puis par position
//! entre deux titres reconnus), si bien qu'une diapositive renommée, déplacée ou
//! complétée garde ses images, formes, notes, fond et disposition ; et ce qui n'a
//! pas changé dans le plan reste tel quel, à l'octet près.

use crate::{Deck, Layout, Slide};

/// Séparateur des deux colonnes dans le plan.
pub const COLUMN_BREAK: &str = "||";
/// Niveau de retrait maximal.
const NIVEAU_MAX: usize = 8;
/// Au-delà de ce nombre de diapositives (anciennes × nouvelles), la
/// correspondance se fait par position seulement (coût borné).
const LCS_MAX: usize = 4_000_000;

/// Une diapositive du plan : son titre et ses lignes `(niveau ≥ 1, texte)`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutlineEntry {
    pub title: String,
    pub lines: Vec<(usize, String)>,
}

/// La marque de liste au début d'une ligne (sans son retrait) : `- `, `* `, `+ `
/// ou `12. ` ; renvoie sa longueur en octets.
pub(crate) fn marque_de_liste(t: &str) -> Option<usize> {
    for m in ["- ", "* ", "+ "] {
        if t.starts_with(m) {
            return Some(m.len());
        }
    }
    let chiffres = t.bytes().take_while(u8::is_ascii_digit).count();
    if chiffres > 0 && chiffres <= 9 && t[chiffres..].starts_with(". ") {
        return Some(chiffres + 2);
    }
    None
}

/// Une ligne du contenu : son niveau dans le plan (1 + retrait / 2) et son texte
/// sans retrait ni puce `- ` (une liste numérotée garde son numéro).
pub(crate) fn ligne_de_corps(l: &str) -> (usize, String) {
    let espaces = l.len() - l.trim_start_matches(' ').len();
    let t = l.trim();
    let niveau = (1 + espaces / 2).min(NIVEAU_MAX);
    let texte = match marque_de_liste(t) {
        Some(n) if !t.as_bytes()[0].is_ascii_digit() => t[n..].trim_start().to_string(),
        _ => t.to_string(),
    };
    (niveau, texte)
}

/// Ligne de contenu (source Varium) d'une ligne du plan.
fn corps_de_ligne(niveau: usize, texte: &str) -> String {
    let retrait = "  ".repeat(niveau.saturating_sub(1).min(NIVEAU_MAX));
    let t = texte.trim();
    let deja = marque_de_liste(t).is_some()
        || t.starts_with('#')
        || t.starts_with('>')
        || t.starts_with('|')
        || t.starts_with("->")
        || t == "---";
    if deja {
        format!("{retrait}{t}")
    } else {
        format!("{retrait}- {t}")
    }
}

/// Les lignes d'un texte, non vides, toutes au niveau 1.
fn lignes_simples(src: &str) -> Vec<(usize, String)> {
    src.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| (1, l.to_string()))
        .collect()
}

fn lignes_de_corps(src: &str) -> Vec<(usize, String)> {
    src.lines()
        .filter(|l| !l.trim().is_empty())
        .map(ligne_de_corps)
        .filter(|(_, t)| !t.is_empty())
        .collect()
}

/// Le titre d'une diapositive sur une ligne.
fn titre_du_plan(s: &Slide) -> String {
    s.title
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

impl OutlineEntry {
    /// L'entrée du plan d'une diapositive.
    pub fn of(s: &Slide) -> OutlineEntry {
        let lines = match s.layout {
            Layout::Title | Layout::Section => lignes_simples(&s.subtitle),
            Layout::Quote => lignes_simples(&s.body),
            _ => {
                let mut v = lignes_de_corps(&s.body);
                if s.layout == Layout::TwoColumns && !s.body2.trim().is_empty() {
                    v.push((1, COLUMN_BREAK.to_string()));
                    v.extend(lignes_de_corps(&s.body2));
                }
                v
            }
        };
        OutlineEntry {
            title: titre_du_plan(s),
            lines,
        }
    }

    /// Écrit l'entrée dans la diapositive (titre et contenu selon la disposition).
    fn ecrire(&self, s: &mut Slide) {
        let ancienne = OutlineEntry::of(s);
        if ancienne.title != self.title {
            s.title = self.title.clone();
            // Un titre sur une diapositive qui n'en montre pas : on le montre.
            if s.layout == Layout::Blank && !self.title.is_empty() {
                s.layout = Layout::TitleContent;
            }
        }
        if ancienne.lines == self.lines {
            return;
        }
        let textes = |ls: &[(usize, String)]| {
            ls.iter()
                .map(|(_, t)| t.trim().to_string())
                .collect::<Vec<_>>()
                .join("\n")
        };
        match s.layout {
            Layout::Title | Layout::Section => s.subtitle = textes(&self.lines),
            Layout::Quote => s.body = textes(&self.lines),
            _ => {
                let coupe = self
                    .lines
                    .iter()
                    .position(|(n, t)| *n == 1 && t.trim() == COLUMN_BREAK);
                let (un, deux) = match coupe {
                    Some(k) => (&self.lines[..k], &self.lines[k + 1..]),
                    None => (&self.lines[..], &[][..]),
                };
                let corps = |ls: &[(usize, String)]| {
                    ls.iter()
                        .filter(|(_, t)| t.trim() != COLUMN_BREAK)
                        .map(|(n, t)| corps_de_ligne(*n, t))
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                s.body = corps(un);
                if coupe.is_some() {
                    s.layout = Layout::TwoColumns;
                    s.body2 = corps(deux);
                } else if s.layout == Layout::TwoColumns {
                    s.body2.clear();
                }
                if s.layout == Layout::TitleOnly && !s.body.is_empty() {
                    s.layout = Layout::TitleContent;
                }
            }
        }
    }
}

/// Le niveau d'une ligne du plan (tabulations, ou groupes de quatre espaces) et
/// son texte.
fn niveau_de(l: &str) -> (usize, &str) {
    let mut niveau = 0;
    let mut espaces = 0;
    let mut fin = 0;
    for (i, c) in l.char_indices() {
        match c {
            '\t' => {
                niveau += 1;
                espaces = 0;
            }
            ' ' => {
                espaces += 1;
                if espaces == 4 {
                    niveau += 1;
                    espaces = 0;
                }
            }
            _ => break,
        }
        fin = i + c.len_utf8();
    }
    (niveau.min(NIVEAU_MAX), l[fin..].trim())
}

/// Lit un plan. Une ligne vide sans retrait est une diapositive sans titre ; une
/// ligne vide en retrait est ignorée ; des lignes en retrait avant le premier
/// titre forment une diapositive sans titre. La dernière ligne vide (celle que
/// l'on vient de créer avec Entrée) compte aussi.
pub fn parse_outline(text: &str) -> Vec<OutlineEntry> {
    let mut out: Vec<OutlineEntry> = Vec::new();
    if text.is_empty() {
        return out;
    }
    for l in text.split('\n') {
        let l = l.strip_suffix('\r').unwrap_or(l);
        let (niveau, t) = niveau_de(l);
        if niveau == 0 {
            out.push(OutlineEntry {
                title: t.to_string(),
                lines: Vec::new(),
            });
        } else if !t.is_empty() {
            if out.is_empty() {
                out.push(OutlineEntry::default());
            }
            if let Some(e) = out.last_mut() {
                e.lines.push((niveau, t.to_string()));
            }
        }
    }
    out
}

/// Écrit des entrées en texte de plan.
pub fn outline_text(entries: &[OutlineEntry]) -> String {
    let mut out = String::new();
    for (k, e) in entries.iter().enumerate() {
        if k > 0 {
            out.push('\n');
        }
        out.push_str(&e.title);
        for (n, t) in &e.lines {
            out.push('\n');
            for _ in 0..(*n).clamp(1, NIVEAU_MAX) {
                out.push('\t');
            }
            out.push_str(t);
        }
    }
    out
}

/// Pour chaque nouvelle entrée, l'ancienne diapositive qui lui correspond.
fn correspondances(anciennes: &[String], nouvelles: &[String]) -> Vec<Option<usize>> {
    let (n, m) = (anciennes.len(), nouvelles.len());
    let mut paires: Vec<(usize, usize)> = Vec::new();
    if n.saturating_mul(m) <= LCS_MAX {
        // Plus longue sous-suite commune des titres.
        let mut t = vec![0u32; (n + 1) * (m + 1)];
        let idx = |i: usize, j: usize| i * (m + 1) + j;
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                t[idx(i, j)] = if anciennes[i] == nouvelles[j] {
                    t[idx(i + 1, j + 1)] + 1
                } else {
                    t[idx(i + 1, j)].max(t[idx(i, j + 1)])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n && j < m {
            if anciennes[i] == nouvelles[j] {
                paires.push((i, j));
                i += 1;
                j += 1;
            } else if t[idx(i + 1, j)] >= t[idx(i, j + 1)] {
                i += 1;
            } else {
                j += 1;
            }
        }
    }
    let mut res = vec![None; m];
    let mut prise = vec![false; n];
    for &(i, j) in &paires {
        res[j] = Some(i);
        prise[i] = true;
    }
    // Une diapositive déplacée : même titre, ailleurs.
    for j in 0..m {
        if res[j].is_none()
            && let Some(i) = (0..n).find(|&i| !prise[i] && anciennes[i] == nouvelles[j])
        {
            res[j] = Some(i);
            prise[i] = true;
        }
    }
    // Entre deux titres reconnus, les diapositives restantes s'associent dans
    // l'ordre (un titre modifié garde sa diapositive).
    let mut bornes = paires;
    bornes.push((n, m));
    let (mut i0, mut j0) = (0, 0);
    for (i1, j1) in bornes {
        let libres_i: Vec<usize> = (i0..i1).filter(|&i| !prise[i]).collect();
        let libres_j: Vec<usize> = (j0..j1).filter(|&j| res[j].is_none()).collect();
        for (&a, &b) in libres_i.iter().zip(&libres_j) {
            res[b] = Some(a);
            prise[a] = true;
        }
        i0 = i1 + 1;
        j0 = j1 + 1;
    }
    res
}

impl Deck {
    /// Le plan de la présentation (voir le module).
    pub fn to_outline(&self) -> String {
        let entries: Vec<OutlineEntry> = self.slides.iter().map(OutlineEntry::of).collect();
        outline_text(&entries)
    }

    /// La présentation d'après un plan modifié : diapositives créées, supprimées,
    /// réordonnées et modifiées ; chaque diapositive retrouvée garde ce que le plan
    /// ne montre pas (objets, notes, fond, disposition, sous-titre…). Toujours au
    /// moins une diapositive.
    pub fn apply_outline(&self, text: &str) -> Deck {
        let entries = parse_outline(text);
        let anciennes: Vec<String> = self.slides.iter().map(titre_du_plan).collect();
        let nouvelles: Vec<String> = entries.iter().map(|e| e.title.clone()).collect();
        let liens = correspondances(&anciennes, &nouvelles);
        let mut deck = self.clone();
        deck.slides = Vec::with_capacity(entries.len().max(1));
        for (e, lien) in entries.iter().zip(liens) {
            let mut s = match lien.and_then(|i| self.slides.get(i)) {
                Some(s) => s.clone(),
                None => {
                    let transition = deck.slides.last().map(|s| s.transition).unwrap_or_default();
                    let layout = if deck.slides.is_empty() && self.slides.is_empty() {
                        Layout::Title
                    } else {
                        Layout::TitleContent
                    };
                    Slide {
                        transition,
                        ..Slide::new(layout)
                    }
                }
            };
            e.ecrire(&mut s);
            deck.slides.push(s);
        }
        if deck.slides.is_empty() {
            let layout = self.slides.first().map_or(Layout::Title, |s| s.layout);
            deck.slides.push(Slide::new(layout));
        }
        deck
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Object, ObjectKind};

    #[test]
    fn exemple_aller_retour_exact() {
        let deck = Deck::sample();
        let plan = deck.to_outline();
        assert!(plan.starts_with("Varium\n\tLa suite bureautique"));
        assert!(plan.contains("Pourquoi Varium ?\n\tUn document"));
        assert!(plan.contains("\n\t\t*gras*"));
        // Le plan relu tel quel ne change rien.
        assert_eq!(deck.apply_outline(&plan), deck);
        assert_eq!(deck.apply_outline(&plan).to_outline(), plan);
    }

    #[test]
    fn creer_renommer_supprimer_reordonner() {
        let mut deck = Deck::new();
        deck.slides = vec![
            Slide {
                title: "Un".into(),
                body: "- a".into(),
                notes: "note 1".into(),
                objects: vec![Object::default()],
                ..Slide::new(Layout::TitleContent)
            },
            Slide {
                title: "Deux".into(),
                body: "- b".into(),
                notes: "note 2".into(),
                ..Slide::new(Layout::TitleContent)
            },
            Slide {
                title: "Trois".into(),
                notes: "note 3".into(),
                objects: vec![Object {
                    kind: ObjectKind::Ellipse,
                    ..Object::default()
                }],
                ..Slide::new(Layout::TitleOnly)
            },
        ];
        // Réordonner : les objets suivent leur diapositive.
        let d = deck.apply_outline("Trois\nUn\n\ta\nDeux\n\tb");
        assert_eq!(d.slides[0].notes, "note 3");
        assert_eq!(d.slides[0].objects[0].kind, ObjectKind::Ellipse);
        assert_eq!(d.slides[1].notes, "note 1");
        assert_eq!(d.slides[1].objects.len(), 1);
        // Renommer : la diapositive garde ses notes.
        let d = deck.apply_outline("Un\n\ta\nDeux bis\n\tb\nTrois");
        assert_eq!(d.slides[1].title, "Deux bis");
        assert_eq!(d.slides[1].notes, "note 2");
        // Créer.
        let d = deck.apply_outline("Un\n\ta\nNouvelle\nDeux\n\tb\nTrois");
        assert_eq!(d.slides.len(), 4);
        assert_eq!(d.slides[1].notes, "");
        assert_eq!(d.slides[2].notes, "note 2");
        // Supprimer.
        let d = deck.apply_outline("Un\nTrois");
        assert_eq!(d.slides.len(), 2);
        assert_eq!(d.slides[1].notes, "note 3");
        // Remplacer un titre : c'est un renommage.
        let d = deck.apply_outline("Un\n\ta\n\t\tsous-point\nNouvelle\n\tx\nTrois\n\tdu contenu");
        assert_eq!(d.slides.len(), 3);
        assert_eq!(d.slides[0].body, "- a\n  - sous-point");
        assert_eq!(d.slides[1].title, "Nouvelle");
        assert_eq!(d.slides[1].notes, "note 2");
        assert_eq!(d.slides[1].body, "- x");
        assert_eq!(d.slides[1].layout, Layout::TitleContent);
        assert_eq!(d.slides[2].notes, "note 3");
        // « Titre seul » qui reçoit du contenu : il devient visible.
        assert_eq!(d.slides[2].layout, Layout::TitleContent);
        assert_eq!(d.slides[2].body, "- du contenu");
        // Tout effacer : il reste une diapositive vide.
        let d = deck.apply_outline("");
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.slides[0].title, "");
    }

    #[test]
    fn colonnes_titre_et_citation() {
        let mut deck = Deck::new();
        deck.slides = vec![
            Slide {
                title: "Titre".into(),
                subtitle: "Ligne 1\nLigne 2".into(),
                ..Slide::new(Layout::Title)
            },
            Slide {
                title: "Deux".into(),
                body: "- a".into(),
                body2: "- b".into(),
                ..Slide::new(Layout::TwoColumns)
            },
        ];
        let plan = deck.to_outline();
        assert_eq!(plan, "Titre\n\tLigne 1\n\tLigne 2\nDeux\n\ta\n\t||\n\tb");
        let d =
            deck.apply_outline("Titre\n\tAutre\nDeux\n\ta\n\tc\n\t||\n\tb\nTrois\n\tx\n\t||\n\ty");
        assert_eq!(d.slides[0].subtitle, "Autre");
        assert_eq!(d.slides[1].body, "- a\n- c");
        assert_eq!(d.slides[1].body2, "- b");
        assert_eq!(d.slides[2].layout, Layout::TwoColumns);
        assert_eq!(d.slides[2].body2, "- y");
    }

    #[test]
    fn niveaux() {
        assert_eq!(niveau_de("\t\tx"), (2, "x"));
        assert_eq!(niveau_de("    x"), (1, "x"));
        assert_eq!(niveau_de("x"), (0, "x"));
        assert_eq!(ligne_de_corps("  - sous"), (2, "sous".into()));
        assert_eq!(ligne_de_corps("1. premier"), (1, "1. premier".into()));
        assert_eq!(corps_de_ligne(2, "x"), "  - x");
        assert_eq!(corps_de_ligne(1, "1. x"), "1. x");
        let e = parse_outline("\tsans titre\nA\n\n\t\nB");
        assert_eq!(e.len(), 4);
        assert_eq!(e[0].title, "");
        assert_eq!(e[0].lines, vec![(1, "sans titre".to_string())]);
        assert_eq!(e[2].title, "");
    }

    #[test]
    fn plans_aleatoires_sans_panique() {
        let mut graine: u64 = 0x2545_F491_4F6C_DD1D;
        let mut alea = || {
            graine ^= graine << 13;
            graine ^= graine >> 7;
            graine ^= graine << 17;
            graine
        };
        let morceaux = [
            "\t", "\n", " ", "    ", "||", "- ", "1. ", "é", "🎉", "#", "\r", "x", "Titre",
            "\u{0}", "  ", "**", "{{a}}", "|", ">",
        ];
        let deck = Deck::sample();
        for _ in 0..400 {
            let n = (alea() % 60) as usize;
            let texte: String = (0..n)
                .map(|_| morceaux[(alea() % morceaux.len() as u64) as usize])
                .collect();
            let d = deck.apply_outline(&texte);
            assert!(!d.slides.is_empty());
            // Le plan d'une présentation relu redonne la même présentation.
            let plan = d.to_outline();
            assert_eq!(d.apply_outline(&plan).to_outline(), plan, "{texte:?}");
        }
    }
}
