//! Comparaison de deux textes mot à mot, pour l'historique des versions.
//!
//! L'algorithme est celui de Myers (« An O(ND) Difference Algorithm », 1986), en
//! espace linéaire : on cherche le « serpent du milieu » puis on recommence de part
//! et d'autre. Les préfixes et suffixes communs sont retirés d'abord. Un budget borne
//! le travail sur des textes énormes et entièrement différents : au-delà, la portion
//! restante est donnée comme retirée puis ajoutée (résultat correct, moins fin).
//!
//! ```
//! use varium_core::diff::{Op, diff_mots};
//! let d = diff_mots("Bonjour Marc, à demain.", "Bonjour Julie, à demain !");
//! let ajouts: String = d.iter().filter(|m| m.op == Op::Ajout).map(|m| m.texte.as_str()).collect();
//! assert_eq!(ajouts, "Julie !");
//! ```

use std::ops::Range;

/// Nature d'un morceau de la comparaison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// Présent dans les deux textes.
    Egal,
    /// Seulement dans le nouveau texte.
    Ajout,
    /// Seulement dans l'ancien texte.
    Retrait,
}

/// Un morceau de texte et sa nature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Morceau {
    pub op: Op,
    pub texte: String,
}

/// Une opération sur des plages : `a` dans l'ancienne suite, `b` dans la nouvelle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edition {
    pub op: Op,
    pub a: Range<usize>,
    pub b: Range<usize>,
}

/// Travail maximal (cases explorées) avant de renoncer à la finesse.
const BUDGET: usize = 40_000_000;

/// Comparaison de deux suites. Le résultat couvre les deux suites dans l'ordre ;
/// les opérations consécutives de même nature sont fusionnées.
pub fn diff<T: PartialEq>(a: &[T], b: &[T]) -> Vec<Edition> {
    let mut out = Vec::new();
    let mut budget = BUDGET;
    rec(a, b, 0, 0, &mut out, &mut budget);
    fusionner(out)
}

fn pousser(out: &mut Vec<Edition>, op: Op, a: Range<usize>, b: Range<usize>) {
    if a.is_empty() && b.is_empty() {
        return;
    }
    out.push(Edition { op, a, b });
}

fn rec<T: PartialEq>(
    a: &[T],
    b: &[T],
    da: usize,
    db: usize,
    out: &mut Vec<Edition>,
    budget: &mut usize,
) {
    // Préfixe commun.
    let p = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    pousser(out, Op::Egal, da..da + p, db..db + p);
    let (a, b, da, db) = (&a[p..], &b[p..], da + p, db + p);
    // Suffixe commun.
    let s = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (am, bm) = (&a[..a.len() - s], &b[..b.len() - s]);
    if am.is_empty() || bm.is_empty() {
        pousser(out, Op::Retrait, da..da + am.len(), db..db);
        pousser(
            out,
            Op::Ajout,
            da + am.len()..da + am.len(),
            db..db + bm.len(),
        );
    } else {
        match milieu(am, bm, budget) {
            Some((x, y)) => {
                rec(&am[..x], &bm[..y], da, db, out, budget);
                rec(&am[x..], &bm[y..], da + x, db + y, out, budget);
            }
            None => {
                pousser(out, Op::Retrait, da..da + am.len(), db..db);
                pousser(
                    out,
                    Op::Ajout,
                    da + am.len()..da + am.len(),
                    db..db + bm.len(),
                );
            }
        }
    }
    let (ea, eb) = (da + am.len(), db + bm.len());
    pousser(out, Op::Egal, ea..ea + s, eb..eb + s);
}

/// Point de coupe (x, y) sur un chemin d'édition minimal (serpent du milieu).
/// `None` si les suites n'ont rien en commun ou si le budget est épuisé.
fn milieu<T: PartialEq>(a: &[T], b: &[T], budget: &mut usize) -> Option<(usize, usize)> {
    let (n, m) = (a.len() as isize, b.len() as isize);
    let max_d = (n + m + 1) / 2;
    let off = max_d;
    let len = 2 * max_d + 2;
    let mut v1 = vec![-1isize; len as usize];
    let mut v2 = vec![-1isize; len as usize];
    v1[(off + 1) as usize] = 0;
    v2[(off + 1) as usize] = 0;
    let delta = n - m;
    let avant = delta % 2 != 0;
    let (mut k1s, mut k1e, mut k2s, mut k2e) = (0isize, 0isize, 0isize, 0isize);
    for d in 0..max_d {
        if *budget < (2 * d as usize + 1) * 2 {
            return None;
        }
        *budget -= (2 * d as usize + 1) * 2;
        let mut k1 = -d + k1s;
        while k1 <= d - k1e {
            let i = (off + k1) as usize;
            let mut x1 = if k1 == -d || (k1 != d && v1[i - 1] < v1[i + 1]) {
                v1[i + 1]
            } else {
                v1[i - 1] + 1
            };
            let mut y1 = x1 - k1;
            while x1 < n && y1 < m && a[x1 as usize] == b[y1 as usize] {
                x1 += 1;
                y1 += 1;
            }
            v1[i] = x1;
            if x1 > n {
                k1e += 2;
            } else if y1 > m {
                k1s += 2;
            } else if avant {
                let j = off + delta - k1;
                if j >= 0 && j < len && v2[j as usize] != -1 && x1 >= n - v2[j as usize] {
                    return Some((x1 as usize, y1 as usize));
                }
            }
            k1 += 2;
        }
        let mut k2 = -d + k2s;
        while k2 <= d - k2e {
            let i = (off + k2) as usize;
            let mut x2 = if k2 == -d || (k2 != d && v2[i - 1] < v2[i + 1]) {
                v2[i + 1]
            } else {
                v2[i - 1] + 1
            };
            let mut y2 = x2 - k2;
            while x2 < n && y2 < m && a[(n - x2 - 1) as usize] == b[(m - y2 - 1) as usize] {
                x2 += 1;
                y2 += 1;
            }
            v2[i] = x2;
            if x2 > n {
                k2e += 2;
            } else if y2 > m {
                k2s += 2;
            } else if !avant {
                let j = off + delta - k2;
                if j >= 0 && j < len && v1[j as usize] != -1 {
                    let x1 = v1[j as usize];
                    let y1 = off + x1 - j;
                    if x1 >= n - x2 {
                        return Some((x1 as usize, y1 as usize));
                    }
                }
            }
            k2 += 2;
        }
    }
    None
}

/// Fusionne les opérations voisines : entre deux passages égaux, un seul retrait
/// suivi d'un seul ajout.
fn fusionner(eds: Vec<Edition>) -> Vec<Edition> {
    let mut out: Vec<Edition> = Vec::new();
    let mut retrait: Option<Edition> = None;
    let mut ajout: Option<Edition> = None;
    let vider = |out: &mut Vec<Edition>, r: &mut Option<Edition>, a: &mut Option<Edition>| {
        if let Some(r) = r.take() {
            out.push(r);
        }
        if let Some(mut a) = a.take() {
            // L'ajout se place après le retrait dans l'ancienne suite.
            if let Some(last) = out.last()
                && last.op == Op::Retrait
            {
                a.a = last.a.end..last.a.end;
            }
            out.push(a);
        }
    };
    for e in eds {
        match e.op {
            Op::Egal => {
                vider(&mut out, &mut retrait, &mut ajout);
                if let Some(last) = out.last_mut()
                    && last.op == Op::Egal
                {
                    last.a.end = e.a.end;
                    last.b.end = e.b.end;
                } else {
                    out.push(e);
                }
            }
            Op::Retrait => match &mut retrait {
                Some(r) => r.a.end = e.a.end,
                None => {
                    let b = ajout.as_ref().map_or(e.b.start, |a| a.b.start);
                    retrait = Some(Edition {
                        op: Op::Retrait,
                        a: e.a,
                        b: b..b,
                    });
                }
            },
            Op::Ajout => match &mut ajout {
                Some(a) => a.b.end = e.b.end,
                None => ajout = Some(e),
            },
        }
    }
    vider(&mut out, &mut retrait, &mut ajout);
    out
}

/// Découpe en jetons : mots (lettres, chiffres, apostrophes et tirets internes),
/// suites d'espaces, autres caractères un par un. Leur concaténation redonne le texte.
pub fn jetons(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let mut fin = i + c.len_utf8();
        if c.is_alphanumeric() {
            while let Some(&(j, d)) = it.peek() {
                let interne = matches!(d, '\'' | '’' | '-')
                    && s[j + d.len_utf8()..]
                        .chars()
                        .next()
                        .is_some_and(char::is_alphanumeric);
                if d.is_alphanumeric() || interne {
                    fin = j + d.len_utf8();
                    it.next();
                } else {
                    break;
                }
            }
        } else if c.is_whitespace() && c != '\n' {
            while let Some(&(j, d)) = it.peek() {
                if d.is_whitespace() && d != '\n' {
                    fin = j + d.len_utf8();
                    it.next();
                } else {
                    break;
                }
            }
        }
        out.push(&s[i..fin]);
    }
    out
}

/// Comparaison mot à mot de deux textes, en morceaux prêts à afficher.
pub fn diff_mots(avant: &str, apres: &str) -> Vec<Morceau> {
    let a = jetons(avant);
    let b = jetons(apres);
    diff(&a, &b)
        .into_iter()
        .map(|e| Morceau {
            op: e.op,
            texte: match e.op {
                Op::Retrait => a[e.a].concat(),
                _ => b[e.b].concat(),
            },
        })
        .collect()
}

/// Ce qui a changé, en bref.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Resume {
    /// Mots ajoutés.
    pub ajoutes: usize,
    /// Mots retirés.
    pub retires: usize,
    /// Un extrait du premier changement (ajout de préférence), sur une ligne.
    pub extrait: Option<String>,
}

fn compte_mots(s: &str) -> usize {
    jetons(s)
        .iter()
        .filter(|t| t.chars().next().is_some_and(char::is_alphanumeric))
        .count()
}

/// Résumé du passage de `avant` à `apres`.
pub fn resume(avant: &str, apres: &str) -> Resume {
    let d = diff_mots(avant, apres);
    let mut r = Resume::default();
    for m in &d {
        match m.op {
            Op::Ajout => r.ajoutes += compte_mots(&m.texte),
            Op::Retrait => r.retires += compte_mots(&m.texte),
            Op::Egal => {}
        }
    }
    let premier = d
        .iter()
        .find(|m| m.op == Op::Ajout && m.texte.trim().chars().any(char::is_alphanumeric))
        .or_else(|| {
            d.iter()
                .find(|m| m.op == Op::Retrait && m.texte.trim().chars().any(char::is_alphanumeric))
        });
    r.extrait = premier.map(|m| {
        let une: String = m.texte.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut c: String = une.chars().take(48).collect();
        if une.chars().count() > 48 {
            c.push('…');
        }
        c
    });
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reconstruit les deux suites depuis les opérations et vérifie la couverture.
    fn verifie<T: PartialEq + Clone + std::fmt::Debug>(a: &[T], b: &[T]) -> usize {
        let d = diff(a, b);
        let (mut ra, mut rb) = (Vec::new(), Vec::new());
        let (mut ia, mut ib) = (0, 0);
        let mut cout = 0;
        for e in &d {
            assert_eq!(e.a.start, ia, "{d:?}");
            assert_eq!(e.b.start, ib, "{d:?}");
            match e.op {
                Op::Egal => {
                    assert_eq!(e.a.len(), e.b.len());
                    assert_eq!(&a[e.a.clone()], &b[e.b.clone()]);
                    ra.extend_from_slice(&a[e.a.clone()]);
                    rb.extend_from_slice(&b[e.b.clone()]);
                }
                Op::Retrait => {
                    assert!(e.b.is_empty());
                    ra.extend_from_slice(&a[e.a.clone()]);
                    cout += e.a.len();
                }
                Op::Ajout => {
                    assert!(e.a.is_empty());
                    rb.extend_from_slice(&b[e.b.clone()]);
                    cout += e.b.len();
                }
            }
            ia = e.a.end;
            ib = e.b.end;
        }
        assert_eq!(ra, a);
        assert_eq!(rb, b);
        cout
    }

    /// Distance d'édition (insertions + suppressions) par programmation dynamique.
    fn distance<T: PartialEq>(a: &[T], b: &[T]) -> usize {
        let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
        for i in (0..a.len()).rev() {
            for j in (0..b.len()).rev() {
                lcs[i][j] = if a[i] == b[j] {
                    lcs[i + 1][j + 1] + 1
                } else {
                    lcs[i + 1][j].max(lcs[i][j + 1])
                };
            }
        }
        a.len() + b.len() - 2 * lcs[0][0]
    }

    #[test]
    fn exemples_classiques() {
        // L'exemple de l'article de Myers : ABCABBA → CBABAC, D = 5.
        let a: Vec<char> = "ABCABBA".chars().collect();
        let b: Vec<char> = "CBABAC".chars().collect();
        assert_eq!(verifie(&a, &b), 5);
        assert_eq!(verifie::<char>(&[], &[]), 0);
        assert_eq!(verifie(&['a'], &[]), 1);
        assert_eq!(verifie(&[], &['a', 'b']), 2);
        assert_eq!(verifie(&['x'; 50], &['x'; 50]), 0);
    }

    #[test]
    fn minimal_sur_des_suites_aleatoires() {
        let mut etat = 0x9E37_79B9_7F4A_7C15_u64;
        let mut alea = || {
            etat ^= etat << 13;
            etat ^= etat >> 7;
            etat ^= etat << 17;
            etat
        };
        for _ in 0..400 {
            let la = (alea() % 30) as usize;
            let lb = (alea() % 30) as usize;
            let alphabet = 1 + alea() % 5;
            let a: Vec<u64> = (0..la).map(|_| alea() % alphabet).collect();
            let b: Vec<u64> = (0..lb).map(|_| alea() % alphabet).collect();
            assert_eq!(verifie(&a, &b), distance(&a, &b), "{a:?} → {b:?}");
        }
    }

    #[test]
    fn mots() {
        assert_eq!(
            jetons("L'été, c’est  peut-être\n- là 😀"),
            [
                "L'été",
                ",",
                " ",
                "c’est",
                "  ",
                "peut-être",
                "\n",
                "-",
                " ",
                "là",
                " ",
                "😀"
            ]
        );
        let d = diff_mots("Bonjour Marc, à demain.", "Bonjour Julie, à demain !");
        assert_eq!(
            d,
            [
                Morceau {
                    op: Op::Egal,
                    texte: "Bonjour ".into()
                },
                Morceau {
                    op: Op::Retrait,
                    texte: "Marc".into()
                },
                Morceau {
                    op: Op::Ajout,
                    texte: "Julie".into()
                },
                Morceau {
                    op: Op::Egal,
                    texte: ", à demain".into()
                },
                Morceau {
                    op: Op::Retrait,
                    texte: ".".into()
                },
                Morceau {
                    op: Op::Ajout,
                    texte: " !".into()
                },
            ]
        );
    }

    #[test]
    fn resumes() {
        let r = resume("Un deux trois.", "Un deux trois quatre cinq.");
        assert_eq!((r.ajoutes, r.retires), (2, 0));
        assert_eq!(r.extrait.as_deref(), Some("quatre cinq"));
        let r = resume("Un deux trois.", "Un trois.");
        assert_eq!((r.ajoutes, r.retires), (0, 1));
        assert_eq!(r.extrait.as_deref(), Some("deux"));
        assert_eq!(resume("a", "a"), Resume::default());
    }

    #[test]
    fn gros_textes_bornes() {
        // Deux textes de 30 000 mots sans rien en commun : réponse rapide et correcte.
        let a: Vec<u32> = (0..30_000).collect();
        let b: Vec<u32> = (30_000..60_000).collect();
        let debut = std::time::Instant::now();
        assert_eq!(verifie(&a, &b), 60_000);
        assert!(debut.elapsed().as_secs() < 20);
        // Une petite retouche au milieu d'un long texte reste fine.
        let mut c = a.clone();
        c[15_000] = 99_999;
        assert_eq!(verifie(&a, &c), 2);
    }
}
