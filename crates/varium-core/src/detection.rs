//! Détection automatique des champs : dans un document déjà écrit, repère les dates,
//! montants, adresses e-mail, numéros de téléphone et noms qui reviennent, et propose
//! d'en faire des `{{champs}}`.
//!
//! - `detecter` ne regarde que le texte hors des `{{…}}` existants ;
//! - `appliquer` remplace les valeurs choisies par leurs champs ; l'application
//!   remet ensuite chaque valeur d'origine, si bien que le document reste identique
//!   à l'écran jusqu'à ce qu'on change une valeur.

use crate::parse::{self, Segment};
use std::collections::BTreeSet;

/// Ce qui a été reconnu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Genre {
    Date,
    Montant,
    Email,
    Telephone,
    /// Un nom propre (au moins deux mots en capitale) qui revient.
    Nom,
}

/// Une valeur du document qui pourrait devenir un champ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidat {
    /// Le texte tel qu'il est écrit dans le document.
    pub texte: String,
    pub genre: Genre,
    /// Nombre d'endroits où il apparaît.
    pub occurrences: usize,
    /// Nom de champ proposé (valide et encore inutilisé).
    pub nom: String,
}

const ESPACES_FINS: [char; 2] = ['\u{a0}', '\u{202f}'];

fn est_espace_chiffres(c: char) -> bool {
    c == ' ' || ESPACES_FINS.contains(&c)
}

/// Les morceaux de texte ordinaire (hors champs), pour ne rien détecter dans `{{…}}`.
fn morceaux(src: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut dernier = 0;
    for seg in parse::parse(src) {
        if let Segment::Var(v) = seg {
            out.push(&src[dernier..v.range.start]);
            dernier = v.range.end;
        }
    }
    out.push(&src[dernier..]);
    out
}

fn ponctuation(c: char) -> bool {
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | '(' | ')' | '<' | '>' | '"' | '«' | '»' | '\''
    )
}

fn courriel(mot: &str) -> Option<String> {
    let m = mot.trim_matches(ponctuation);
    let (local, domaine) = m.split_once('@')?;
    let bon = |c: char| c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | '+');
    let ok = !local.is_empty()
        && local.chars().all(bon)
        && domaine.contains('.')
        && domaine.chars().all(bon)
        && domaine.split('.').all(|p| !p.is_empty());
    ok.then(|| m.to_string())
}

/// `jj/mm/aaaa`, `jj.mm.aa`, `jj-mm-aaaa` ou `aaaa-mm-jj`.
fn date(mot: &str) -> Option<String> {
    let m = mot.trim_matches(ponctuation);
    let sep = ['/', '.', '-'].into_iter().find(|s| m.contains(*s))?;
    let parts: Vec<&str> = m.split(sep).collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    let n: Vec<u32> = parts.iter().map(|p| p.parse().unwrap_or(0)).collect();
    let (j, mo, a) = if parts[0].len() == 4 {
        (n[2], n[1], parts[0].len())
    } else {
        (n[0], n[1], parts[2].len())
    };
    let ok_jour = (1..=31).contains(&j);
    let ok_mois = (1..=12).contains(&mo);
    let ok_annee = if parts[0].len() == 4 {
        parts[1].len() <= 2 && parts[2].len() <= 2
    } else {
        parts[0].len() <= 2 && parts[1].len() <= 2 && (a == 2 || a == 4)
    };
    (ok_jour && ok_mois && ok_annee).then(|| m.to_string())
}

/// Numéro de téléphone : 10 à 13 chiffres, commence par 0 ou +, séparés par des
/// espaces, points ou tirets.
fn telephones(texte: &str, out: &mut Vec<(String, Genre)>) {
    let car: Vec<(usize, char)> = texte.char_indices().collect();
    let mut i = 0;
    while i < car.len() {
        let (debut, c) = car[i];
        if !(c == '+' || c == '0') || (i > 0 && car[i - 1].1.is_alphanumeric()) {
            i += 1;
            continue;
        }
        let mut j = i;
        let mut chiffres = 0;
        let mut fin = i;
        while j < car.len() {
            let c = car[j].1;
            if c.is_ascii_digit() {
                chiffres += 1;
                fin = j + 1;
            } else if !(est_espace_chiffres(c) || matches!(c, '.' | '-' | '+')) {
                break;
            }
            j += 1;
        }
        let apres = car.get(fin).map(|x| x.0).unwrap_or(texte.len());
        // Un chiffre ou une lettre collé derrière : ce n'est pas un numéro isolé.
        let colle = car.get(j).is_some_and(|x| x.1.is_alphanumeric()) && j == fin;
        if (10..=13).contains(&chiffres) && !colle {
            out.push((texte[debut..apres].to_string(), Genre::Telephone));
        }
        i = j.max(i + 1);
    }
}

/// Montants : un nombre suivi de « € », « EUR », « euros », « $ » ou « £ ».
fn montants(texte: &str, out: &mut Vec<(String, Genre)>) {
    let car: Vec<(usize, char)> = texte.char_indices().collect();
    for (k, &(pos, c)) in car.iter().enumerate() {
        let longueur_symbole = if matches!(c, '€' | '$' | '£') {
            c.len_utf8()
        } else if texte[pos..].starts_with("EUR") && !precede_lettre(&car, k) {
            3
        } else if texte[pos..].to_lowercase().starts_with("euros") && !precede_lettre(&car, k) {
            5
        } else {
            continue;
        };
        let mut b = k;
        while b > 0 && est_espace_chiffres(car[b - 1].1) {
            b -= 1;
        }
        let fin_nombre = b;
        let mut d = b;
        while d > 0 {
            let p = car[d - 1].1;
            let suivi_chiffre = car.get(d).is_some_and(|x| x.1.is_ascii_digit());
            if p.is_ascii_digit()
                || (matches!(p, ',' | '.') && suivi_chiffre)
                || (est_espace_chiffres(p)
                    && suivi_chiffre
                    && d >= 2
                    && car[d - 2].1.is_ascii_digit())
            {
                d -= 1;
            } else {
                break;
            }
        }
        if d == fin_nombre || !car[d].1.is_ascii_digit() {
            continue;
        }
        if d > 0 && car[d - 1].1.is_alphanumeric() {
            continue;
        }
        out.push((
            texte[car[d].0..pos + longueur_symbole].to_string(),
            Genre::Montant,
        ));
    }
}

fn precede_lettre(car: &[(usize, char)], k: usize) -> bool {
    k > 0 && car[k - 1].1.is_alphabetic()
}

fn mot_capitale(mot: &str) -> bool {
    let mut it = mot.chars();
    match it.next() {
        Some(c) if c.is_uppercase() => {}
        _ => return false,
    }
    let reste: Vec<char> = it.collect();
    reste.len() >= 2
        && reste
            .iter()
            .all(|c| c.is_alphabetic() || matches!(c, '-' | '\''))
        && reste.iter().any(|c| c.is_lowercase())
}

/// Suites d'au moins deux mots en capitale (« Marc Dupont », « Société Martin »).
fn noms(texte: &str, out: &mut Vec<(String, Genre)>) {
    for ligne in texte.lines() {
        let mut suite: Vec<&str> = Vec::new();
        let mut vide = |suite: &mut Vec<&str>| {
            if suite.len() >= 2 {
                out.push((suite.join(" "), Genre::Nom));
            }
            suite.clear();
        };
        for mot in ligne.split(' ') {
            let propre = mot.trim_matches(ponctuation);
            let ponct_fin = mot.len() != mot.trim_end_matches(ponctuation).len();
            if mot_capitale(propre) {
                suite.push(propre);
                if ponct_fin {
                    vide(&mut suite);
                }
            } else {
                vide(&mut suite);
            }
        }
        vide(&mut suite);
    }
}

fn occurrences(morceaux: &[&str], texte: &str) -> usize {
    morceaux.iter().map(|m| m.matches(texte).count()).sum()
}

fn nom_de(genre: Genre) -> &'static str {
    match genre {
        Genre::Date => "date",
        Genre::Montant => "montant",
        Genre::Email => "email",
        Genre::Telephone => "téléphone",
        Genre::Nom => "nom",
    }
}

/// Propose les champs d'un document : dates, montants, e-mails, téléphones (même
/// s'ils n'apparaissent qu'une fois) et noms qui reviennent au moins deux fois.
pub fn detecter(src: &str) -> Vec<Candidat> {
    let parts = morceaux(src);
    let mut trouves: Vec<(String, Genre)> = Vec::new();
    for m in &parts {
        for mot in m.split(|c: char| c.is_whitespace()) {
            if let Some(e) = courriel(mot) {
                trouves.push((e, Genre::Email));
            } else if let Some(d) = date(mot) {
                trouves.push((d, Genre::Date));
            }
        }
        telephones(m, &mut trouves);
        montants(m, &mut trouves);
        noms(m, &mut trouves);
    }
    let mut utilises: BTreeSet<String> =
        parse::variables(src).into_iter().map(|v| v.name).collect();
    let mut vus = BTreeSet::new();
    let mut out = Vec::new();
    for (texte, genre) in trouves {
        if !vus.insert(texte.clone()) {
            continue;
        }
        let n = occurrences(&parts, &texte);
        if n == 0 || (genre == Genre::Nom && n < 2) {
            continue;
        }
        let base = nom_de(genre);
        let mut nom = base.to_string();
        let mut k = 2;
        while utilises.contains(&nom) {
            nom = format!("{base}_{k}");
            k += 1;
        }
        utilises.insert(nom.clone());
        out.push(Candidat {
            texte,
            genre,
            occurrences: n,
            nom,
        });
    }
    out
}

/// Remplace chaque texte choisi par son champ, hors des `{{…}}` existants. Les textes
/// les plus longs sont reconnus en premier ; un nom invalide ou vide est ignoré.
/// Renvoie la nouvelle source.
pub fn appliquer(src: &str, choix: &[(String, String)]) -> String {
    let mut valides: Vec<(&str, String)> = choix
        .iter()
        .filter(|(t, n)| !t.is_empty() && parse::is_valid_name(n))
        .map(|(t, n)| (t.as_str(), format!("{{{{{n}}}}}")))
        .collect();
    valides.sort_by_key(|(t, _)| std::cmp::Reverse(t.len()));
    let remplacer = |gap: &str, out: &mut String| {
        let mut i = 0;
        'pos: while i < gap.len() {
            for (t, champ) in &valides {
                if gap[i..].starts_with(t) {
                    out.push_str(champ);
                    i += t.len();
                    continue 'pos;
                }
            }
            let c = gap[i..].chars().next().map_or(1, char::len_utf8);
            out.push_str(&gap[i..i + c]);
            i += c;
        }
    };
    let mut out = String::with_capacity(src.len() + 16);
    let mut dernier = 0;
    for seg in parse::parse(src) {
        if let Segment::Var(v) = seg {
            remplacer(&src[dernier..v.range.start], &mut out);
            out.push_str(&v.raw);
            dernier = v.range.end;
        }
    }
    remplacer(&src[dernier..], &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn textes(src: &str) -> Vec<(String, Genre)> {
        detecter(src)
            .into_iter()
            .map(|c| (c.texte, c.genre))
            .collect()
    }

    #[test]
    fn reconnait_dates_montants_courriels_telephones() {
        let src = "Facture du 12/03/2026. Total : 1 250,50 € à régler avant le 2026-04-01.\n\
                   Contact : marc.dupont@exemple.fr ou 06 12 34 56 78.";
        let t = textes(src);
        assert!(t.contains(&("12/03/2026".into(), Genre::Date)));
        assert!(t.contains(&("2026-04-01".into(), Genre::Date)));
        assert!(t.contains(&("1 250,50 €".into(), Genre::Montant)));
        assert!(t.contains(&("marc.dupont@exemple.fr".into(), Genre::Email)));
        assert!(t.contains(&("06 12 34 56 78".into(), Genre::Telephone)));
    }

    #[test]
    fn ignore_les_faux_positifs() {
        let t = textes("Version 1.2.3, le 31/13/2026 (mois 13), page 12, code 123456.");
        assert!(t.is_empty(), "{t:?}");
        assert!(textes("a@b et @tag et x@y.").is_empty());
    }

    #[test]
    fn noms_seulement_s_ils_reviennent() {
        let un = textes("Bonjour Marc Dupont.");
        assert!(un.iter().all(|(_, g)| *g != Genre::Nom));
        let deux =
            textes("Madame Claire Martin, nous écrivons à Claire Martin pour Claire Martin.");
        assert!(deux.contains(&("Claire Martin".into(), Genre::Nom)));
    }

    #[test]
    fn rien_dans_les_champs_existants_et_noms_uniques() {
        let src = "Le 01/02/2026 et {{date_x | date}} puis {{date}} le 03/04/2026.";
        let c = detecter(src);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].nom, "date_2");
        assert_eq!(c[1].nom, "date_3");
        assert!(!c.iter().any(|x| x.texte.contains("date_x")));
    }

    #[test]
    fn appliquer_remplace_toutes_les_occurrences_hors_champs() {
        let src = "Marc Dupont le 12/03/2026. {{nom}} 12/03/2026. Marc Dupont.";
        let choix = vec![
            ("12/03/2026".to_string(), "date".to_string()),
            ("Marc Dupont".to_string(), "client".to_string()),
            ("x".to_string(), "invalide nom".to_string()),
        ];
        assert_eq!(
            appliquer(src, &choix),
            "{{client}} le {{date}}. {{nom}} {{date}}. {{client}}."
        );
    }

    #[test]
    fn appliquer_prefere_le_texte_le_plus_long_et_garde_les_accents() {
        let choix = vec![
            ("Élodie".to_string(), "prénom".to_string()),
            ("Élodie Évrard".to_string(), "client".to_string()),
        ];
        assert_eq!(
            appliquer("Élodie Évrard, chère Élodie", &choix),
            "{{client}}, chère {{prénom}}"
        );
    }

    #[test]
    fn jamais_de_panique() {
        for s in [
            "",
            "€",
            "0",
            "+",
            "@",
            "1/1/1",
            "é€é",
            "10 000 000 000 00€",
            "{{",
            "\u{a0}€",
        ] {
            let _ = detecter(s);
            let _ = appliquer(s, &[(s.to_string(), "x".to_string())]);
        }
    }
}
