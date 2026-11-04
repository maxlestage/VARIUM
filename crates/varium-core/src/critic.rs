//! Commentaires et suivi des modifications, écrits dans le texte à la manière de
//! CriticMarkup :
//!
//! | Écriture | Sens |
//! |---|---|
//! | `{++texte++}` | ajout proposé |
//! | `{--texte--}` | suppression proposée |
//! | `{~~ancien~>nouveau~~}` | remplacement proposé |
//! | `{==texte==}{>>remarque<<}` | passage surligné et son commentaire |
//! | `{>>remarque<<}` | commentaire seul |
//!
//! Un commentaire collé derrière une marque lui appartient : `{++oui++}{>>Max ·
//! 2026-11-04<<}` dit qui a proposé l'ajout, et quand (« auteur · date », puis
//! éventuellement « : remarque »).
//!
//! Les exports montrent par défaut la **version finale** (ajouts acceptés,
//! suppressions faites, commentaires masqués) ; [`avec_vue`] permet de garder les
//! marques visibles (ajouts soulignés, suppressions barrées, commentaires en notes).

use std::borrow::Cow;
use std::cell::Cell;
use std::ops::Range;

/// Le genre d'une marque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Genre {
    Ajout,
    Suppression,
    Substitution,
    Surlignage,
    Commentaire,
}

/// Une marque de révision trouvée dans le texte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marque {
    pub genre: Genre,
    /// Toute la marque (octets), commentaire attaché compris.
    pub plage: Range<usize>,
    /// Le texte concerné : ajouté, supprimé, remplacé (l'ancien), surligné, ou le
    /// commentaire lui-même.
    pub texte: String,
    /// Le nouveau texte d'un remplacement (vide sinon).
    pub nouveau: String,
    /// Où l'on écrit dans la marque (octets) : le texte ajouté, le nouveau texte d'un
    /// remplacement, le texte d'un commentaire.
    pub interieur: Range<usize>,
    /// Le commentaire attaché (sans `{>>` ni `<<}`).
    pub commentaire: Option<String>,
}

impl Marque {
    /// L'auteur, la date et la remarque du commentaire (attaché, ou la marque même).
    pub fn note(&self) -> Note {
        match (&self.commentaire, self.genre) {
            (Some(c), _) => lire_note(c),
            (None, Genre::Commentaire) => lire_note(&self.texte),
            _ => Note::default(),
        }
    }
}

/// Auteur, date et remarque d'un commentaire.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Note {
    pub auteur: String,
    pub date: String,
    pub texte: String,
}

const OUVERTURES: [(&str, &str, Genre); 5] = [
    ("{++", "++}", Genre::Ajout),
    ("{--", "--}", Genre::Suppression),
    ("{~~", "~~}", Genre::Substitution),
    ("{==", "==}", Genre::Surlignage),
    ("{>>", "<<}", Genre::Commentaire),
];

/// Le texte contient-il des marques de révision ?
pub fn a_des_marques(src: &str) -> bool {
    OUVERTURES.iter().any(|(o, _, _)| src.contains(o))
}

/// La marque qui commence à l'octet `i`, si elle est bien fermée.
fn marque_en(src: &str, i: usize) -> Option<Marque> {
    let reste = src.get(i..)?;
    let (o, f, genre) = OUVERTURES.iter().find(|(o, _, _)| reste.starts_with(o))?;
    let debut_int = i + o.len();
    let fin_int = debut_int + src[debut_int..].find(f)?;
    let interieur_brut = &src[debut_int..fin_int];
    let mut fin = fin_int + f.len();
    let (texte, nouveau, interieur) = match genre {
        Genre::Substitution => {
            let (a, n) = interieur_brut.split_once("~>")?;
            let d = debut_int + a.len() + 2;
            (a.to_string(), n.to_string(), d..fin_int)
        }
        _ => (
            interieur_brut.to_string(),
            String::new(),
            debut_int..fin_int,
        ),
    };
    let mut commentaire = None;
    if *genre != Genre::Commentaire
        && src[fin..].starts_with("{>>")
        && let Some(k) = src[fin + 3..].find("<<}")
    {
        commentaire = Some(src[fin + 3..fin + 3 + k].to_string());
        fin += 3 + k + 3;
    }
    Some(Marque {
        genre: *genre,
        plage: i..fin,
        texte,
        nouveau,
        interieur,
        commentaire,
    })
}

/// Toutes les marques, dans l'ordre du texte (sans chevauchement).
pub fn marques(src: &str) -> Vec<Marque> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(k) = src[i..].find('{') {
        let p = i + k;
        let echappe = p > 0 && src.as_bytes()[p - 1] == b'\\';
        match (!echappe).then(|| marque_en(src, p)).flatten() {
            Some(m) => {
                i = m.plage.end;
                out.push(m);
            }
            None => i = p + 1,
        }
    }
    out
}

/// Ce que devient une marque acceptée.
fn acceptee(m: &Marque) -> &str {
    match m.genre {
        Genre::Ajout | Genre::Surlignage => &m.texte,
        Genre::Substitution => &m.nouveau,
        Genre::Suppression | Genre::Commentaire => "",
    }
}

/// Ce que devient une marque refusée.
fn refusee(m: &Marque) -> &str {
    match m.genre {
        Genre::Suppression | Genre::Substitution | Genre::Surlignage => &m.texte,
        Genre::Ajout | Genre::Commentaire => "",
    }
}

fn remplacer(src: &str, m: &Marque, par: &str) -> String {
    let mut s = String::with_capacity(src.len());
    s.push_str(&src[..m.plage.start]);
    s.push_str(par);
    s.push_str(&src[m.plage.end..]);
    s
}

/// Accepte une marque (un commentaire est simplement retiré).
pub fn accepter(src: &str, m: &Marque) -> String {
    remplacer(src, m, acceptee(m))
}

/// Refuse une marque (un commentaire est simplement retiré).
pub fn refuser(src: &str, m: &Marque) -> String {
    remplacer(src, m, refusee(m))
}

fn toutes(src: &str, f: fn(&Marque) -> &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut dernier = 0;
    for m in marques(src) {
        out.push_str(&src[dernier..m.plage.start]);
        out.push_str(f(&m));
        dernier = m.plage.end;
    }
    out.push_str(&src[dernier..]);
    out
}

/// Accepte toutes les modifications et retire les commentaires : la version finale.
pub fn tout_accepter(src: &str) -> String {
    toutes(src, acceptee)
}

/// Refuse toutes les modifications et retire les commentaires : la version d'origine.
pub fn tout_refuser(src: &str) -> String {
    toutes(src, refusee)
}

/// Les marques visibles dans un export : ajouts soulignés, suppressions barrées,
/// commentaires en notes numérotées.
pub fn vue_marquee(src: &str) -> String {
    let entourer = |t: &str, m: &str| -> String {
        t.split('\n')
            .map(|l| {
                if l.trim().is_empty() {
                    l.to_string()
                } else {
                    format!("{m}{}{m}", l.trim())
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let note = |c: &str| -> String {
        let n = lire_note(c);
        if n.texte.is_empty() {
            return String::new();
        }
        let qui = [n.auteur.as_str(), n.date.as_str()]
            .iter()
            .filter(|x| !x.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(", ");
        let t = if qui.is_empty() {
            n.texte
        } else {
            format!("{} ({qui})", n.texte)
        };
        format!("\\note{{{}}}", t.replace(['{', '}', '\n'], " "))
    };
    let mut out = String::with_capacity(src.len());
    let mut dernier = 0;
    for m in marques(src) {
        out.push_str(&src[dernier..m.plage.start]);
        match m.genre {
            Genre::Ajout => out.push_str(&entourer(&m.texte, "__")),
            Genre::Suppression => out.push_str(&entourer(&m.texte, "~~")),
            Genre::Substitution => {
                out.push_str(&entourer(&m.texte, "~~"));
                out.push(' ');
                out.push_str(&entourer(&m.nouveau, "__"));
            }
            Genre::Surlignage => out.push_str(&m.texte),
            Genre::Commentaire => out.push_str(&note(&m.texte)),
        }
        if let Some(c) = &m.commentaire {
            out.push_str(&note(c));
        }
        dernier = m.plage.end;
    }
    out.push_str(&src[dernier..]);
    out
}

/// Ce que montrent les exports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Vue {
    /// La version finale (par défaut).
    #[default]
    Finale,
    /// Les marques visibles.
    Marquee,
    /// Le texte tel quel, marques comprises (aperçu).
    Brute,
}

thread_local! {
    static VUE: Cell<Vue> = const { Cell::new(Vue::Finale) };
}

/// Exécute `f` (un export) avec cette vue des marques de révision.
pub fn avec_vue<R>(vue: Vue, f: impl FnOnce() -> R) -> R {
    let avant = VUE.with(|v| v.replace(vue));
    struct Remise(Vue);
    impl Drop for Remise {
        fn drop(&mut self) {
            VUE.with(|v| v.set(self.0));
        }
    }
    let _remise = Remise(avant);
    f()
}

/// Le texte à exporter, selon la vue en cours (voir [`avec_vue`]).
pub fn pour_export(src: &str) -> Cow<'_, str> {
    if !a_des_marques(src) {
        return Cow::Borrowed(src);
    }
    match VUE.with(Cell::get) {
        Vue::Finale => Cow::Owned(tout_accepter(src)),
        Vue::Marquee => Cow::Owned(vue_marquee(src)),
        Vue::Brute => Cow::Borrowed(src),
    }
}

// ---------------------------------------------------------------------------------
// Commentaires : « auteur · date : remarque »
// ---------------------------------------------------------------------------------

/// Retire d'un texte ce qui fermerait la marque trop tôt (`<<}` devient `< < }`).
fn nettoyer(t: &str, fermeture: &str) -> String {
    let espace: String = fermeture
        .chars()
        .map(String::from)
        .collect::<Vec<_>>()
        .join(" ");
    let mut s = t.to_string();
    for _ in 0..8 {
        if !s.contains(fermeture) {
            break;
        }
        s = s.replace(fermeture, &espace);
    }
    s
}

fn est_date(d: &str) -> bool {
    let b = d.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// Lit « auteur · date : remarque » (chaque partie est facultative).
pub fn lire_note(c: &str) -> Note {
    let c = c.trim();
    if let Some((avant, apres)) = c.split_once(" · ") {
        let (date, texte) = match apres.split_once(" : ") {
            Some((d, t)) if est_date(d.trim()) => (d.trim(), t.trim()),
            _ if est_date(apres.trim()) => (apres.trim(), ""),
            _ => {
                return Note {
                    texte: c.to_string(),
                    ..Note::default()
                };
            }
        };
        return Note {
            auteur: avant.trim().to_string(),
            date: date.to_string(),
            texte: texte.to_string(),
        };
    }
    Note {
        texte: c.to_string(),
        ..Note::default()
    }
}

/// La signature `{>>auteur · date<<}` d'une modification (rien sans auteur ni date).
pub fn signature(auteur: &str, date: &str) -> Option<String> {
    let a = nettoyer(auteur.trim(), "<<}").replace(" · ", " - ");
    let d = date.trim();
    match (a.is_empty(), d.is_empty()) {
        (true, true) => None,
        (false, true) => Some(format!("{{>>{a}<<}}")),
        (true, false) => Some(format!("{{>> · {d}<<}}")),
        (false, false) => Some(format!("{{>>{a} · {d}<<}}")),
    }
}

/// Un commentaire complet : `{>>auteur · date : remarque<<}`.
pub fn commentaire(auteur: &str, date: &str, texte: &str) -> String {
    let t = nettoyer(&texte.replace('\n', " "), "<<}");
    let a = nettoyer(auteur.trim(), "<<}").replace(" · ", " - ");
    let d = date.trim();
    if a.is_empty() && !est_date(d) {
        return format!("{{>>{t}<<}}");
    }
    format!("{{>>{a} · {d} : {t}<<}}")
}

/// Commente la sélection `sel` (octets) : `{==sélection==}{>>…<<}`, ou un commentaire
/// seul si rien n'est sélectionné. Renvoie le texte et la plage (octets) où écrire la
/// remarque, déjà sélectionnée si elle est vide.
pub fn commenter(
    src: &str,
    sel: Range<usize>,
    auteur: &str,
    date: &str,
    texte: &str,
) -> (String, Range<usize>) {
    let a = sel.start.min(src.len());
    let b = sel.end.clamp(a, src.len());
    let (a, b) = (floor(src, a), floor(src, b));
    let choisi = &src[a..b];
    let c = commentaire(auteur, date, texte);
    // Où commence la remarque dans `c` (après « : ») ?
    let debut_note = c.rfind(" : ").map_or(3, |k| k + 3);
    let mut out = String::with_capacity(src.len() + c.len() + 6);
    out.push_str(&src[..a]);
    let base = if choisi.is_empty() || choisi.contains('\n') || a_des_marques(choisi) {
        out.push_str(choisi);
        out.len()
    } else {
        out.push_str("{==");
        out.push_str(&nettoyer(choisi, "==}"));
        out.push_str("==}");
        out.len()
    };
    out.push_str(&c);
    let note = base + debut_note..base + c.len() - 3;
    out.push_str(&src[b..]);
    (out, note)
}

fn floor(s: &str, mut i: usize) -> usize {
    i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

// ---------------------------------------------------------------------------------
// Suivi des modifications pendant la frappe
// ---------------------------------------------------------------------------------

#[cfg(test)]
fn octet(s: &str, ch: usize) -> usize {
    s.char_indices().nth(ch).map_or(s.len(), |(b, _)| b)
}

fn caractere(s: &str, b: usize) -> usize {
    s[..floor(s, b)].chars().count()
}

/// Suivi des modifications : `avant` est devenu `apres` par une frappe (curseur en
/// caractères avant et après). Renvoie le texte où l'ajout est marqué `{++…++}`, la
/// suppression `{--…--}` (le texte supprimé reste), le remplacement `{~~…~>…~~}`, avec
/// la signature de l'auteur, et la nouvelle position du curseur. `None` : rien à
/// marquer (pas de changement, ou frappe dans une marque, comme son propre ajout).
pub fn suivre(
    avant: &str,
    apres: &str,
    curseur_avant: usize,
    auteur: &str,
    date: &str,
) -> Option<(String, usize)> {
    if avant == apres {
        return None;
    }
    // Préfixe et suffixe communs (sur des frontières de caractères).
    let mut p = avant
        .bytes()
        .zip(apres.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    while !(avant.is_char_boundary(p) && apres.is_char_boundary(p)) {
        p -= 1;
    }
    let max_s = avant.len().min(apres.len()) - p;
    let mut s = avant
        .bytes()
        .rev()
        .zip(apres.bytes().rev())
        .take(max_s)
        .take_while(|(a, b)| a == b)
        .count();
    while !(avant.is_char_boundary(avant.len() - s) && apres.is_char_boundary(apres.len() - s)) {
        s -= 1;
    }
    let fin_avant = avant.len() - s;
    let supprime = &avant[p..fin_avant];
    let ajoute = &apres[p..apres.len() - s];
    // Une frappe dans une marque (son propre ajout, un commentaire…) reste telle quelle,
    // comme une suppression qui en touche une.
    let ms = marques(avant);
    if ms.iter().any(|m| {
        if supprime.is_empty() {
            m.plage.start < p && p < m.plage.end
        } else {
            m.plage.start < fin_avant && p < m.plage.end
        }
    }) || a_des_marques(ajoute)
        || ["++}", "--}", "~~}", "<<}", "==}", "~>"]
            .iter()
            .any(|f| supprime.contains(f) || ajoute.contains(f))
    {
        return None;
    }
    let sig = signature(auteur, date).unwrap_or_default();
    let sig_int = sig
        .strip_prefix("{>>")
        .and_then(|x| x.strip_suffix("<<}"))
        .map(str::to_string);
    let meme = |m: &Marque| m.commentaire == sig_int;
    let (debut, fin) = (&avant[..p], &avant[fin_avant..]);
    let mut out = String::with_capacity(apres.len() + 32);
    let curseur;
    if supprime.is_empty() {
        // Ajout collé à son propre ajout : il s'y prolonge.
        if let Some(m) = ms
            .iter()
            .find(|m| m.genre == Genre::Ajout && m.plage.end == p && meme(m))
        {
            out.push_str(&avant[..m.interieur.end]);
            out.push_str(ajoute);
            let c = out.len();
            out.push_str(&avant[m.interieur.end..]);
            curseur = caractere(&out, c);
            return Some((out, curseur));
        }
        out.push_str(debut);
        out.push_str("{++");
        out.push_str(ajoute);
        let c = out.len();
        out.push_str("++}");
        out.push_str(&sig);
        out.push_str(fin);
        return Some((out.clone(), caractere(&out, c)));
    }
    // Suppression vers l'avant (touche Suppr) : le curseur était au début.
    let vers_l_avant = curseur_avant == caractere(avant, p);
    if ajoute.is_empty() {
        // Suppression collée à la sienne : elle s'y ajoute.
        if let Some(m) = ms
            .iter()
            .find(|m| m.genre == Genre::Suppression && m.plage.start == fin_avant && meme(m))
        {
            out.push_str(debut);
            out.push_str("{--");
            out.push_str(supprime);
            out.push_str(&avant[m.interieur.start..]);
            let c = if vers_l_avant {
                p + 3 + supprime.len() + (m.plage.end - m.interieur.start)
            } else {
                p
            };
            return Some((out.clone(), caractere(&out, c)));
        }
        if let Some(m) = ms
            .iter()
            .find(|m| m.genre == Genre::Suppression && m.plage.end == p && meme(m))
        {
            out.push_str(&avant[..m.interieur.end]);
            out.push_str(supprime);
            out.push_str(&avant[m.interieur.end..m.plage.end]);
            out.push_str(fin);
            let c = if vers_l_avant {
                m.plage.end + supprime.len()
            } else {
                m.plage.start
            };
            return Some((out.clone(), caractere(&out, c)));
        }
        out.push_str(debut);
        out.push_str("{--");
        out.push_str(supprime);
        out.push_str("--}");
        out.push_str(&sig);
        let c = if vers_l_avant { out.len() } else { p };
        out.push_str(fin);
        return Some((out.clone(), caractere(&out, c)));
    }
    // Remplacement d'une sélection.
    out.push_str(debut);
    out.push_str("{~~");
    out.push_str(supprime);
    out.push_str("~>");
    out.push_str(ajoute);
    let c = out.len();
    out.push_str("~~}");
    out.push_str(&sig);
    out.push_str(fin);
    Some((out.clone(), caractere(&out, c)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lire_les_marques() {
        let src = "Un {++ajout++}{>>Max · 2026-11-04<<}, {--retrait--}, {~~vieux~>neuf~~} et {==ceci==}{>>à revoir<<}. {>>seul<<} \\{++non++}";
        let m = marques(src);
        assert_eq!(m.len(), 5);
        assert_eq!(m[0].genre, Genre::Ajout);
        assert_eq!(m[0].texte, "ajout");
        assert_eq!(m[0].note().auteur, "Max");
        assert_eq!(m[0].note().date, "2026-11-04");
        assert_eq!(m[2].texte, "vieux");
        assert_eq!(m[2].nouveau, "neuf");
        assert_eq!(&src[m[2].interieur.clone()], "neuf");
        assert_eq!(m[3].commentaire.as_deref(), Some("à revoir"));
        assert_eq!(m[4].genre, Genre::Commentaire);
        assert_eq!(tout_accepter(src), "Un ajout, , neuf et ceci.  \\{++non++}");
        assert_eq!(
            tout_refuser(src),
            "Un , retrait, vieux et ceci.  \\{++non++}"
        );
        // Non fermées : du texte ordinaire.
        assert!(marques("{++ouvert {--").is_empty());
        assert!(marques("{~~sans flèche~~}").is_empty());
    }

    #[test]
    fn accepter_et_refuser_une_marque() {
        let src = "a {++b++} c";
        let m = &marques(src)[0];
        assert_eq!(accepter(src, m), "a b c");
        assert_eq!(refuser(src, m), "a  c");
    }

    #[test]
    fn vues_d_export() {
        let src = "Bonjour {++cher++}{>>Max · 2026-11-04 : plus chaleureux<<} {--ami--}{~~Paul~>Pierre~~}.";
        assert_eq!(pour_export(src), "Bonjour cher Pierre.");
        let m = avec_vue(Vue::Marquee, || pour_export(src).into_owned());
        assert!(m.contains("__cher__"), "{m}");
        assert!(
            m.contains("\\note{plus chaleureux (Max, 2026-11-04)}"),
            "{m}"
        );
        assert!(m.contains("~~ami~~"));
        assert!(m.contains("~~Paul~~ __Pierre__"));
        // La vue revient à la version finale ensuite.
        assert_eq!(pour_export(src), "Bonjour cher Pierre.");
        assert!(matches!(pour_export("rien"), Cow::Borrowed(_)));
        // L'export complet passe par la version finale.
        let r = crate::export::prepare(src, &Default::default(), &Default::default());
        let t = crate::export::text(&[r]);
        assert!(t.contains("Bonjour cher Pierre."), "{t}");
        assert!(!t.contains("ami"));
    }

    #[test]
    fn notes_et_signatures() {
        assert_eq!(
            lire_note("Max · 2026-11-04 : à voir"),
            Note {
                auteur: "Max".into(),
                date: "2026-11-04".into(),
                texte: "à voir".into()
            }
        );
        assert_eq!(lire_note("juste un mot").texte, "juste un mot");
        assert_eq!(lire_note("a · b").texte, "a · b");
        assert_eq!(
            signature("Max", "2026-11-04").unwrap(),
            "{>>Max · 2026-11-04<<}"
        );
        assert_eq!(signature("", ""), None);
        assert_eq!(commentaire("", "", "x<<}y"), "{>>x< < }y<<}");
        let (s, r) = commenter("Le chat dort", 3..7, "Max", "2026-11-04", "");
        assert_eq!(s, "Le {==chat==}{>>Max · 2026-11-04 : <<} dort");
        assert_eq!(&s[r.clone()], "");
        assert_eq!(r.start, s.find(" : ").unwrap() + 3);
        let (debut, fin) = (9, 2);
        let (s, _) = commenter("abc", debut..fin, "", "", "note");
        assert_eq!(s, "abc{>>note<<}");
    }

    fn taper(src: &str, cur: usize, t: &str) -> (String, usize) {
        let b = octet(src, cur);
        let mut s = src.to_string();
        s.insert_str(b, t);
        suivre(src, &s, cur, "Max", "2026-11-04").unwrap_or((s, cur + t.chars().count()))
    }

    fn effacer_arriere(src: &str, cur: usize) -> (String, usize) {
        let b = octet(src, cur);
        let a = octet(src, cur - 1);
        let mut s = src.to_string();
        s.replace_range(a..b, "");
        suivre(src, &s, cur, "Max", "2026-11-04").unwrap_or((s, cur - 1))
    }

    #[test]
    fn la_frappe_est_suivie() {
        // Taper « très » devant « bien » : un seul ajout, prolongé lettre après lettre.
        let mut s = "C'est bien.".to_string();
        let mut c = 6;
        for ch in ["t", "r", "è", "s", " "] {
            (s, c) = taper(&s, c, ch);
        }
        assert_eq!(s, "C'est {++très ++}{>>Max · 2026-11-04<<}bien.");
        assert_eq!(tout_accepter(&s), "C'est très bien.");
        // Retour arrière sur « bien » : une seule suppression, qui grandit.
        let fin = s.chars().count() - 1;
        let (mut s2, mut c2) = (s.clone(), fin);
        for _ in 0..4 {
            (s2, c2) = effacer_arriere(&s2, c2);
        }
        assert_eq!(
            s2,
            "C'est {++très ++}{>>Max · 2026-11-04<<}{--bien--}{>>Max · 2026-11-04<<}."
        );
        assert_eq!(tout_accepter(&s2), "C'est très .");
        assert_eq!(tout_refuser(&s2), "C'est bien.");
        // Effacer dans son propre ajout : simplement effacé.
        let dans = s.find("très").unwrap() + "très".len();
        let cur = s[..dans].chars().count();
        let (s3, _) = effacer_arriere(&s, cur);
        assert_eq!(s3, "C'est {++trè ++}{>>Max · 2026-11-04<<}bien.");
        // Remplacer une sélection.
        let avant = "Le chat dort";
        let r = suivre(avant, "Le chien dort", 3, "Max", "2026-11-04").unwrap();
        assert_eq!(r.0, "Le ch{~~at~>ien~~}{>>Max · 2026-11-04<<} dort");
        // Supprimer vers l'avant : le curseur passe après la marque.
        let r = suivre("abc", "ac", 1, "", "").unwrap();
        assert_eq!(r, ("a{--b--}c".to_string(), 8));
        assert!(suivre("x", "x", 0, "", "").is_none());
    }

    #[test]
    fn fuzz_sans_panique() {
        let mut graine = 7u64;
        let mut alea = |n: usize| {
            graine = graine
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (graine >> 33) as usize % n.max(1)
        };
        let morceaux = [
            "{++", "++}", "{--", "--}", "{~~", "~>", "~~}", "{==", "==}", "{>>", "<<}", "é", "a",
            " ", "\n", "\\", "{", "}",
        ];
        for _ in 0..3000 {
            let n = alea(14);
            let s: String = (0..n).map(|_| morceaux[alea(morceaux.len())]).collect();
            let _ = tout_accepter(&s);
            let _ = tout_refuser(&s);
            let _ = vue_marquee(&s);
            for m in marques(&s) {
                assert!(s.get(m.plage.clone()).is_some());
                let _ = accepter(&s, &m);
            }
            let k = s.len();
            let (a, b) = (alea(k + 2), alea(k + 2));
            let _ = commenter(&s, a..b, "M", "2026-01-01", "x");
            let t: String = (0..alea(6))
                .map(|_| morceaux[alea(morceaux.len())])
                .collect();
            let c = alea(s.chars().count() + 1);
            let mut apres = s.clone();
            apres.insert_str(octet(&s, c), &t);
            if let Some((r, cur)) = suivre(&s, &apres, c, "M", "2026-01-01") {
                assert!(cur <= r.chars().count());
            }
            let mut moins = s.clone();
            if !moins.is_empty() {
                let x = floor(&s, alea(s.len()));
                let y = floor(&s, alea(s.len() + 1)).max(x);
                moins.replace_range(x..y, "");
                let _ = suivre(&s, &moins, c, "M", "2026-01-01");
            }
        }
    }
}
