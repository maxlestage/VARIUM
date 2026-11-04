//! « Remplir comme un formulaire » : les champs d'un document présentés comme les
//! questions d'un formulaire, avec un libellé lisible et le bon type de saisie.
//!
//! - le libellé vient du nom (`date_rdv` → « Date rdv », `nomClient` → « Nom client ») ;
//! - le type de saisie vient du type du champ (date, nombre, montant, e-mail, texte
//!   long) ;
//! - un champ comparé dans des `{{si statut = payé}}…{{si statut = en attente}}` devient
//!   une liste de choix (payé, en attente) ;
//! - un champ seulement testé par `{{si remise}}` (jamais affiché) devient une case à
//!   cocher (« oui » ou vide).

use crate::conditions;
use crate::parse::{self, Segment};
use crate::render::{Values, VarType};

/// La manière de saisir une réponse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Saisie {
    Texte,
    TexteLong,
    Date,
    Nombre,
    Montant,
    Email,
    /// Une valeur parmi celles-ci (la saisie libre reste possible).
    Choix(Vec<String>),
    /// Oui ou non (`{{si champ}}`).
    Case,
}

/// Une question du formulaire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// Nom du champ dans le document.
    pub nom: String,
    /// Libellé lisible.
    pub libelle: String,
    pub saisie: Saisie,
    /// Nombre d'endroits où le champ apparaît (ou est testé).
    pub occurrences: usize,
}

/// Libellé lisible d'un nom de champ : séparateurs en espaces, mots collés en
/// « camelCase » séparés, première lettre en capitale.
pub fn libelle(nom: &str) -> String {
    let mut out = String::with_capacity(nom.len() + 4);
    let mut prec: Option<char> = None;
    for c in nom.chars() {
        match c {
            '_' | '-' | '.' => {
                if !out.ends_with(' ') && !out.is_empty() {
                    out.push(' ');
                }
            }
            c if c.is_uppercase() && prec.is_some_and(char::is_lowercase) => {
                out.push(' ');
                out.extend(c.to_lowercase());
            }
            c => out.push(c),
        }
        prec = Some(c);
    }
    let out = out.trim();
    let mut cs = out.chars();
    match cs.next() {
        Some(f) => f.to_uppercase().chain(cs).collect(),
        None => String::new(),
    }
}

/// Les questions du formulaire, dans l'ordre d'apparition des champs.
pub fn questions(src: &str, values: &Values) -> Vec<Question> {
    // Champs affichés (ou utilisés dans un calcul) quelque part.
    let mut affiches: Vec<String> = Vec::new();
    for seg in parse::parse(src) {
        if let Segment::Var(v) = seg {
            match &v.formula {
                Some(f) => affiches.extend(crate::calc::identifiers(f)),
                None => affiches.push(v.name),
            }
        }
    }
    let testes = conditions::valeurs_testees(src);
    parse::variables(src)
        .into_iter()
        .map(|info| {
            let choix = testes
                .iter()
                .find(|(n, _)| *n == info.name)
                .map(|(_, v)| v.clone());
            let ty = values.ty(&info.name);
            let saisie = match (choix, ty) {
                (_, VarType::Date) => Saisie::Date,
                (_, VarType::Number) => Saisie::Nombre,
                (_, VarType::Money) => Saisie::Montant,
                (_, VarType::Email) => Saisie::Email,
                (Some(c), _) if !c.is_empty() => Saisie::Choix(c),
                (Some(_), _) if !affiches.contains(&info.name) => Saisie::Case,
                (_, VarType::Long) => Saisie::TexteLong,
                _ => Saisie::Texte,
            };
            Question {
                libelle: libelle(&info.name),
                nom: info.name,
                saisie,
                occurrences: info.count,
            }
        })
        .collect()
}

/// Message d'erreur d'une réponse, s'il y en a un (vide : jamais d'erreur).
pub fn erreur(q: &Question, values: &Values, valeur: &str) -> Option<&'static str> {
    if q.saisie == Saisie::Case {
        return None;
    }
    values.ty(&q.nom).validate(valeur)
}

/// Combien de questions ont une réponse.
pub fn remplies(qs: &[Question], values: &Values) -> usize {
    qs.iter()
        .filter(|q| !values.get(&q.nom).trim().is_empty())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libelles() {
        assert_eq!(libelle("date_rdv"), "Date rdv");
        assert_eq!(libelle("prénom"), "Prénom");
        assert_eq!(libelle("nomClient"), "Nom client");
        assert_eq!(libelle("code-postal.ville"), "Code postal ville");
        assert_eq!(libelle("TVA"), "TVA");
        assert_eq!(libelle("__x__"), "X");
        assert_eq!(libelle(""), "");
    }

    #[test]
    fn types_de_saisie() {
        let src = "{{si statut = payé}}Merci{{sinon}}{{si statut = \"en attente\"}}Patience{{fin}}{{fin}}\n\
                   {{si remise}}Remise accordée{{fin}}\n\
                   {{si urgent}}URGENT {{urgent}}{{fin}}\n\
                   Le {{date_rdv}}, {{montant}} pour {{nomClient}} ({{courriel}}).\n\
                   {{= montant * 2}} {{qr:x}} {{notes}}";
        let mut v = Values::default();
        v.types.insert("date_rdv".into(), VarType::Date);
        v.types.insert("montant".into(), VarType::Money);
        v.types.insert("courriel".into(), VarType::Email);
        v.types.insert("notes".into(), VarType::Long);
        let q = questions(src, &v);
        let vue: Vec<(&str, &str, Saisie)> = q
            .iter()
            .map(|q| (q.nom.as_str(), q.libelle.as_str(), q.saisie.clone()))
            .collect();
        assert_eq!(
            vue,
            [
                (
                    "statut",
                    "Statut",
                    Saisie::Choix(vec!["payé".into(), "en attente".into()])
                ),
                ("remise", "Remise", Saisie::Case),
                ("urgent", "Urgent", Saisie::Texte),
                ("date_rdv", "Date rdv", Saisie::Date),
                ("montant", "Montant", Saisie::Montant),
                ("nomClient", "Nom client", Saisie::Texte),
                ("courriel", "Courriel", Saisie::Email),
                ("notes", "Notes", Saisie::TexteLong),
            ]
        );
        assert_eq!(q[4].occurrences, 2);
        assert_eq!(
            erreur(&q[3], &v, "demain"),
            Some("Date attendue : AAAA-MM-JJ")
        );
        assert_eq!(erreur(&q[3], &v, "2026-03-01"), None);
        assert_eq!(erreur(&q[1], &v, "n'importe quoi"), None);
        v.set("statut", "payé");
        assert_eq!(remplies(&q, &v), 1);
        assert!(conditions::est_vrai("oui") && !conditions::est_vrai("non"));
    }
}
