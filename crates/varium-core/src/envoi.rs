//! « Prêt à envoyer » : le contrôle de dernière minute avant d'envoyer un document, et
//! un nom de fichier propre (sans accents, espaces ni caractères gênants pour les
//! messageries).
//!
//! Le contrôle ne bloque jamais : il liste ce qu'un destinataire verrait de travers
//! (champs restés vides, modifications ou commentaires jamais traités, remarques de
//! révision restantes) pour que l'utilisateur décide en connaissance de cause.

use crate::critic::{self, Genre};
use crate::{Lang, Values, check, parse};

/// Ce que le contrôle a trouvé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Point {
    /// Un document sans aucun texte.
    DocumentVide,
    /// Un champ `{{nom}}` sans valeur : il resterait tel quel (ou vide) à l'arrivée.
    ChampVide(String),
    /// Ajouts, suppressions ou remplacements proposés mais ni acceptés ni refusés.
    ModificationsEnAttente(usize),
    /// Commentaires de relecture encore présents dans le texte.
    CommentairesEnAttente(usize),
    /// Remarques de révision (espaces, majuscules, mots répétés…).
    Remarques(usize),
}

impl Point {
    /// Vrai pour ce qui rend l'envoi franchement risqué (le reste est un conseil).
    pub fn grave(&self) -> bool {
        matches!(
            self,
            Point::DocumentVide | Point::ChampVide(_) | Point::ModificationsEnAttente(_)
        )
    }
}

/// Contrôle `src` avec les `valeurs` saisies. Liste vide : prêt à envoyer.
pub fn controler(src: &str, valeurs: &Values, langue: Lang) -> Vec<Point> {
    let mut points = Vec::new();
    let final_ = critic::pour_export(src);
    if final_.trim().is_empty() {
        return vec![Point::DocumentVide];
    }
    // Champs sans valeur (ni valeur par défaut) : on regarde chaque occurrence.
    let mut vus: Vec<String> = Vec::new();
    for seg in parse::parse(src) {
        if let parse::Segment::Var(v) = seg
            && v.formula.is_none()
            && !v.name.starts_with(parse::QR)
            && !v.name.starts_with(parse::SIGNATURE)
            && valeurs.resolve(&v).is_none_or(|r| r.trim().is_empty())
            && !vus.contains(&v.name)
        {
            vus.push(v.name.clone());
        }
    }
    points.extend(vus.into_iter().map(Point::ChampVide));
    let marques = critic::marques(src);
    let modifs = marques
        .iter()
        .filter(|m| {
            matches!(
                m.genre,
                Genre::Ajout | Genre::Suppression | Genre::Substitution
            )
        })
        .count();
    if modifs > 0 {
        points.push(Point::ModificationsEnAttente(modifs));
    }
    let comms = marques
        .iter()
        .filter(|m| {
            matches!(m.genre, Genre::Commentaire | Genre::Surlignage) || m.commentaire.is_some()
        })
        .count();
    if comms > 0 {
        points.push(Point::CommentairesEnAttente(comms));
    }
    let remarques = check::check(&final_, langue).len();
    if remarques > 0 {
        points.push(Point::Remarques(remarques));
    }
    points
}

/// Un nom de fichier sûr pour un envoi : accents retirés, séparateurs en `-`, longueur
/// raisonnable, jamais vide. `nom_de_fichier("Lettre de relance — Éric", "pdf")` donne
/// `Lettre-de-relance-Eric.pdf`.
pub fn nom_de_fichier(titre: &str, extension: &str) -> String {
    let mut s = String::new();
    for c in titre.trim().chars() {
        let c = sans_accent(c);
        match c {
            "" => {}
            c if c.chars().all(|x| x.is_ascii_alphanumeric() || x == '_') => s.push_str(c),
            _ => {
                if !s.ends_with('-') {
                    s.push('-');
                }
            }
        }
    }
    let s: String = s.trim_matches('-').chars().take(60).collect();
    let s = s.trim_matches('-');
    let base = if s.is_empty() { "document" } else { s };
    let ext = extension.trim_start_matches('.');
    if ext.is_empty() {
        base.to_string()
    } else {
        format!("{base}.{ext}")
    }
}

/// La lettre sans son accent (ligatures développées) ; les autres alphabets sont
/// conservés tels quels s'ils sont ASCII, sinon retirés.
fn sans_accent(c: char) -> &'static str {
    // Table minimale mais couvrant les six langues de l'interface.
    const T: &[(&str, &str)] = &[
        ("àáâãäåāă", "a"),
        ("ÀÁÂÃÄÅĀĂ", "A"),
        ("çćč", "c"),
        ("ÇĆČ", "C"),
        ("èéêëēę", "e"),
        ("ÈÉÊËĒĘ", "E"),
        ("ìíîïī", "i"),
        ("ÌÍÎÏĪ", "I"),
        ("ñń", "n"),
        ("ÑŃ", "N"),
        ("òóôõöōø", "o"),
        ("ÒÓÔÕÖŌØ", "O"),
        ("ùúûüū", "u"),
        ("ÙÚÛÜŪ", "U"),
        ("ýÿ", "y"),
        ("ÝŸ", "Y"),
        ("œ", "oe"),
        ("Œ", "OE"),
        ("æ", "ae"),
        ("Æ", "AE"),
        ("ß", "ss"),
    ];
    for (liste, rempl) in T {
        if liste.contains(c) {
            return rempl;
        }
    }
    // Les caractères ASCII passent par `char::to_string` côté appelant : on les
    // renvoie via une table statique de 128 entrées.
    if c.is_ascii() {
        const ASCII: &str = "\0\u{1}\u{2}\u{3}\u{4}\u{5}\u{6}\u{7}\u{8}\u{9}\u{a}\u{b}\u{c}\u{d}\u{e}\u{f}\u{10}\u{11}\u{12}\u{13}\u{14}\u{15}\u{16}\u{17}\u{18}\u{19}\u{1a}\u{1b}\u{1c}\u{1d}\u{1e}\u{1f} !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~\u{7f}";
        let i = c as usize;
        return &ASCII[i..=i];
    }
    ""
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nom_propre() {
        assert_eq!(
            nom_de_fichier("Lettre de relance — Éric", "pdf"),
            "Lettre-de-relance-Eric.pdf"
        );
        assert_eq!(
            nom_de_fichier("  Cœur d'été : n°3 !  ", ".pdf"),
            "Coeur-d-ete-n3.pdf"
        );
        assert_eq!(nom_de_fichier("???", "pdf"), "document.pdf");
        assert_eq!(nom_de_fichier("日本語", "pdf"), "document.pdf");
        assert_eq!(nom_de_fichier("Straße_2", ""), "Strasse_2");
        let long = "a".repeat(200);
        assert_eq!(nom_de_fichier(&long, "pdf").len(), 64);
    }

    #[test]
    fn document_vide() {
        assert_eq!(
            controler("  \n ", &Values::default(), Lang::Fr),
            vec![Point::DocumentVide]
        );
    }

    #[test]
    fn champs_vides_une_fois() {
        let mut v = Values::default();
        v.set("nom", "Marc");
        let p = controler("Bonjour {{nom}}, {{ville}} et {{ville}}.", &v, Lang::Fr);
        assert_eq!(p, vec![Point::ChampVide("ville".into())]);
        assert!(p[0].grave());
    }

    #[test]
    fn valeur_par_defaut_suffit() {
        let p = controler(
            "Bonjour {{nom|default:\"ami\"}}.",
            &Values::default(),
            Lang::Fr,
        );
        assert!(p.is_empty(), "{p:?}");
    }

    #[test]
    fn revision_en_attente() {
        let mut v = Values::default();
        v.set("x", "1");
        let src = "Voici {++un ajout++} et {>>à vérifier<<} {{x}}.";
        let p = controler(src, &v, Lang::Fr);
        assert!(p.contains(&Point::ModificationsEnAttente(1)), "{p:?}");
        assert!(p.contains(&Point::CommentairesEnAttente(1)), "{p:?}");
    }

    #[test]
    fn document_propre() {
        let mut v = Values::default();
        v.set("nom", "Marc");
        assert!(controler("Bonjour {{nom}}.", &v, Lang::Fr).is_empty());
    }
}
