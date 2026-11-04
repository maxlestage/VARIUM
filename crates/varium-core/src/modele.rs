//! Modèles intelligents : un document dont les champs sont vides, et une question
//! par champ (« Quel est le nom du client ? »). « Nouveau depuis un modèle » pose les
//! questions une à une, puis produit le document rempli.
//!
//! Un modèle s'enregistre dans un fichier `.varium-modele.json` (galerie de modèles de
//! l'utilisateur) ; les lettres types fournies avec Varium sont dans `lettres`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::formulaire::{self, Saisie};
use crate::lang::Lang;
use crate::project::Project;
use crate::render::{Values, VarType};

pub const FORMAT: &str = "varium-modele";
/// Version 1 : nom, description, icône, langue, projet, questions.
pub const VERSION: u32 = 1;
/// Extension des fichiers de modèle.
pub const EXTENSION: &str = "varium-modele.json";

/// La question posée pour un champ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub champ: String,
    pub question: String,
    /// Une précision sous la question (facultative).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub aide: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Modele {
    #[serde(default = "format_name")]
    pub format: String,
    #[serde(default = "version")]
    pub version: u32,
    #[serde(default)]
    pub nom: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "icone_defaut")]
    pub icone: String,
    #[serde(default)]
    pub langue: Lang,
    /// Le document, champs vides (les types restent).
    pub projet: Project,
    #[serde(default)]
    pub questions: Vec<Question>,
}

fn format_name() -> String {
    FORMAT.into()
}
fn version() -> u32 {
    VERSION
}
fn icone_defaut() -> String {
    "✨".into()
}

impl Modele {
    /// Fait un modèle d'un document : les valeurs sont vidées (les types restent) et
    /// chaque champ reçoit une question dans la langue `lang`.
    pub fn depuis_projet(nom: &str, description: &str, projet: &Project, lang: Lang) -> Modele {
        let valeurs = projet.values();
        let questions = formulaire::questions(&projet.source, &valeurs)
            .into_iter()
            .filter(|q| q.nom != "aujourdhui")
            .map(|q| Question {
                question: question_pour(&q.nom, &q.saisie, lang),
                champ: q.nom,
                aide: String::new(),
            })
            .collect();
        let mut p = projet.clone();
        p.values.clear();
        p.title = if nom.trim().is_empty() {
            projet.title.clone()
        } else {
            nom.trim().to_string()
        };
        Modele {
            format: FORMAT.into(),
            version: VERSION,
            nom: nom.trim().to_string(),
            description: description.trim().to_string(),
            icone: icone_defaut(),
            langue: lang,
            projet: p,
            questions,
        }
    }

    /// Les questions dans l'ordre, avec leur manière de répondre (date, montant,
    /// liste de choix…). Un champ du document sans question en reçoit une.
    pub fn questions_saisies(&self) -> Vec<(Question, Saisie)> {
        let valeurs = self.projet.values();
        let saisies = formulaire::questions(&self.projet.source, &valeurs);
        let mut out: Vec<(Question, Saisie)> = Vec::new();
        for q in &self.questions {
            if let Some(s) = saisies.iter().find(|s| s.nom == q.champ) {
                out.push((q.clone(), s.saisie.clone()));
            }
        }
        for s in saisies {
            if s.nom != "aujourdhui" && !out.iter().any(|(q, _)| q.champ == s.nom) {
                out.push((
                    Question {
                        question: question_pour(&s.nom, &s.saisie, self.langue),
                        champ: s.nom,
                        aide: String::new(),
                    },
                    s.saisie,
                ));
            }
        }
        out
    }

    /// Le document rempli avec ces réponses.
    pub fn remplir(&self, reponses: &BTreeMap<String, String>) -> Project {
        let mut p = self.projet.clone();
        for (k, v) in reponses {
            if !v.trim().is_empty() {
                p.values.insert(k.clone(), v.clone());
            }
        }
        if p.title.trim().is_empty() {
            p.title = self.nom.clone();
        }
        p
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn from_json(s: &str) -> Result<Modele, String> {
        let m: Modele = serde_json::from_str(s).map_err(|e| format!("Modèle illisible : {e}"))?;
        if m.format != FORMAT {
            return Err(format!("Format inconnu : « {} »", m.format));
        }
        if m.version > VERSION {
            return Err(format!(
                "Modèle créé par une version plus récente (v{})",
                m.version
            ));
        }
        Ok(m)
    }

    /// Un nom de fichier sûr (`lettre-de-relance.varium-modele.json`).
    pub fn nom_de_fichier(&self) -> String {
        let mut s = String::new();
        for c in self.nom.trim().chars().flat_map(char::to_lowercase) {
            if c.is_alphanumeric() {
                s.push(c);
            } else if !s.ends_with('-') {
                s.push('-');
            }
        }
        let s: String = s.trim_matches('-').chars().take(60).collect();
        let s = if s.is_empty() {
            "modele".to_string()
        } else {
            s
        };
        format!("{s}.{EXTENSION}")
    }
}

// ---------------------------------------------------------------------------------
// Questions : « Quel est le nom du client ? »
// ---------------------------------------------------------------------------------

/// Les mots d'un nom de champ (`nom_client`, `dateRdv` → « nom », « client »).
fn mots(nom: &str) -> Vec<String> {
    formulaire::libelle(nom)
        .split_whitespace()
        .map(str::to_lowercase)
        .collect()
}

/// Genre des noms français courants dans les champs : (masculin, élidé devant voyelle).
fn genre_fr(mot: &str) -> Option<bool> {
    const MASC: &[&str] = &[
        "nom",
        "prénom",
        "prenom",
        "montant",
        "numéro",
        "numero",
        "client",
        "destinataire",
        "expéditeur",
        "expediteur",
        "objet",
        "prix",
        "total",
        "téléphone",
        "telephone",
        "code",
        "motif",
        "poste",
        "salaire",
        "contrat",
        "employeur",
        "signataire",
        "titre",
        "mois",
        "jour",
        "lieu",
        "délai",
        "delai",
        "solde",
        "email",
        "courriel",
        "courrier",
        "rdv",
        "rendez-vous",
        "dossier",
        "projet",
        "produit",
        "service",
        "taux",
        "pays",
        "site",
        "responsable",
        "fournisseur",
        "locataire",
        "propriétaire",
        "bénéficiaire",
        "beneficiaire",
        "acompte",
        "loyer",
        "nombre",
        "logement",
        "contact",
        "candidat",
        "employé",
        "patient",
        "élève",
        "sujet",
        "message",
        "commentaire",
        "intitulé",
        "compte",
        "iban",
        "siret",
        "département",
        "secteur",
        "diplôme",
        "véhicule",
        "abonnement",
        "remboursement",
    ];
    const FEM: &[&str] = &[
        "date",
        "adresse",
        "ville",
        "référence",
        "reference",
        "entreprise",
        "société",
        "societe",
        "civilité",
        "civilite",
        "durée",
        "duree",
        "raison",
        "facture",
        "quantité",
        "quantite",
        "fonction",
        "période",
        "periode",
        "signature",
        "banque",
        "description",
        "commande",
        "remise",
        "réduction",
        "mission",
        "formation",
        "personne",
        "demande",
        "année",
        "annee",
        "heure",
        "somme",
        "tva",
        "région",
        "region",
        "rue",
        "agence",
        "structure",
        "association",
        "école",
        "classe",
        "matière",
        "note",
        "offre",
        "annonce",
        "prestation",
        "livraison",
        "résidence",
        "salle",
        "réunion",
        "échéance",
        "mensualité",
        "caution",
        "assurance",
    ];
    if MASC.contains(&mot) {
        Some(true)
    } else if FEM.contains(&mot) {
        Some(false)
    } else {
        None
    }
}

fn voyelle(mot: &str) -> bool {
    mot.chars().next().is_some_and(|c| {
        matches!(
            c,
            'a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'h' | 'é' | 'è' | 'ê' | 'â' | 'î' | 'ô' | 'û'
        )
    })
}

/// Compléments sans article : « date de naissance », « date d'échéance ».
const SANS_ARTICLE: &[&str] = &[
    "naissance",
    "livraison",
    "paiement",
    "début",
    "debut",
    "fin",
    "départ",
    "depart",
    "retour",
    "signature",
    "résiliation",
    "resiliation",
    "embauche",
    "facturation",
    "échéance",
    "echeance",
    "émission",
    "emission",
    "validité",
    "expiration",
    "arrivée",
    "envoi",
    "réception",
    "travail",
    "sortie",
    "entrée",
    "effet",
    "rendu",
];

/// « le nom », « l'adresse », « la ville ».
fn avec_article(mot: &str, masc: bool) -> String {
    if voyelle(mot) && !mot.starts_with("ha") {
        format!("l'{mot}")
    } else if masc {
        format!("le {mot}")
    } else {
        format!("la {mot}")
    }
}

/// « du client », « de la société », « de l'entreprise », « de naissance ».
fn complement(mot: &str) -> Option<String> {
    if SANS_ARTICLE.contains(&mot) {
        return Some(if voyelle(mot) {
            format!("d'{mot}")
        } else {
            format!("de {mot}")
        });
    }
    let masc = genre_fr(mot)?;
    Some(if voyelle(mot) && !mot.starts_with("ha") {
        format!("de l'{mot}")
    } else if masc {
        format!("du {mot}")
    } else {
        format!("de la {mot}")
    })
}

fn question_fr(nom: &str, saisie: &Saisie) -> String {
    let m = mots(nom);
    let lib = formulaire::libelle(nom);
    if *saisie == Saisie::Case {
        return format!("« {lib} » : oui ou non ?");
    }
    if let Some(premier) = m.first()
        && let Some(masc) = genre_fr(premier)
    {
        let mut groupe = avec_article(premier, masc);
        let reste = &m[1..];
        if let Some(second) = reste.first() {
            match complement(second) {
                Some(c) => {
                    groupe.push(' ');
                    groupe.push_str(&c);
                    for r in &reste[1..] {
                        groupe.push(' ');
                        groupe.push_str(r);
                    }
                }
                None => {
                    for r in reste {
                        groupe.push(' ');
                        groupe.push_str(r);
                    }
                }
            }
        }
        let quel = if masc { "Quel" } else { "Quelle" };
        if let Saisie::Choix(_) = saisie {
            return format!("{quel} est {groupe} ? (choisissez)");
        }
        return format!("{quel} est {groupe} ?");
    }
    match saisie {
        Saisie::Date => format!("Quelle date pour « {lib} » ?"),
        Saisie::Montant => format!("Quel montant pour « {lib} » ?"),
        Saisie::Nombre => format!("Combien pour « {lib} » ?"),
        Saisie::Email => format!("Quelle adresse e-mail pour « {lib} » ?"),
        Saisie::Choix(_) => format!("Que choisir pour « {lib} » ?"),
        _ => format!("Que faut-il écrire pour « {lib} » ?"),
    }
}

/// La question posée pour un champ, dans la langue `lang`.
pub fn question_pour(nom: &str, saisie: &Saisie, lang: Lang) -> String {
    let lib = formulaire::libelle(nom);
    let bas = lib.to_lowercase();
    match lang {
        Lang::Fr => question_fr(nom, saisie),
        Lang::En => match saisie {
            Saisie::Case => format!("“{lib}”: yes or no?"),
            Saisie::Choix(_) => format!("Which option for “{lib}”?"),
            Saisie::Montant => format!("What amount for “{lib}”?"),
            Saisie::Nombre => format!("What number for “{lib}”?"),
            Saisie::Email => format!("What email address for “{lib}”?"),
            Saisie::Date if !bas.contains("date") => format!("Which date for “{lib}”?"),
            _ => format!("What is the {bas}?"),
        },
        Lang::Es => match saisie {
            Saisie::Case => format!("«{lib}»: ¿sí o no?"),
            Saisie::Choix(_) => format!("¿Qué elegir para «{lib}»?"),
            Saisie::Montant => format!("¿Qué importe para «{lib}»?"),
            Saisie::Nombre => format!("¿Qué número para «{lib}»?"),
            Saisie::Email => format!("¿Qué correo electrónico para «{lib}»?"),
            Saisie::Date => format!("¿Qué fecha para «{lib}»?"),
            _ => format!("¿Qué hay que escribir en «{lib}»?"),
        },
        Lang::De => match saisie {
            Saisie::Case => format!("„{lib}“: ja oder nein?"),
            Saisie::Choix(_) => format!("Was wählen Sie für „{lib}“?"),
            Saisie::Montant => format!("Welcher Betrag für „{lib}“?"),
            Saisie::Nombre => format!("Welche Zahl für „{lib}“?"),
            Saisie::Email => format!("Welche E-Mail-Adresse für „{lib}“?"),
            Saisie::Date => format!("Welches Datum für „{lib}“?"),
            _ => format!("Was gehört in „{lib}“?"),
        },
        Lang::It => match saisie {
            Saisie::Case => format!("«{lib}»: sì o no?"),
            Saisie::Choix(_) => format!("Cosa scegliere per «{lib}»?"),
            Saisie::Montant => format!("Quale importo per «{lib}»?"),
            Saisie::Nombre => format!("Quale numero per «{lib}»?"),
            Saisie::Email => format!("Quale indirizzo e-mail per «{lib}»?"),
            Saisie::Date => format!("Quale data per «{lib}»?"),
            _ => format!("Cosa scrivere in «{lib}»?"),
        },
        Lang::Pt => match saisie {
            Saisie::Case => format!("«{lib}»: sim ou não?"),
            Saisie::Choix(_) => format!("O que escolher para «{lib}»?"),
            Saisie::Montant => format!("Que montante para «{lib}»?"),
            Saisie::Nombre => format!("Que número para «{lib}»?"),
            Saisie::Email => format!("Que endereço de e-mail para «{lib}»?"),
            Saisie::Date => format!("Que data para «{lib}»?"),
            _ => format!("O que escrever em «{lib}»?"),
        },
    }
}

/// Le type d'un champ d'après sa manière de répondre (pour un champ détecté).
pub fn type_de(genre: crate::detection::Genre) -> VarType {
    use crate::detection::Genre as G;
    match genre {
        G::Date => VarType::Date,
        G::Montant => VarType::Money,
        G::Email => VarType::Email,
        G::Telephone | G::Nom => VarType::Text,
    }
}

/// Valeurs vides pour tous les champs (types gardés).
pub fn sans_valeurs(v: &Values) -> Values {
    Values {
        values: BTreeMap::new(),
        types: v.types.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn questions_en_francais() {
        let q = |n: &str, s: Saisie| question_pour(n, &s, Lang::Fr);
        assert_eq!(
            q("nom_client", Saisie::Texte),
            "Quel est le nom du client ?"
        );
        assert_eq!(q("adresse", Saisie::TexteLong), "Quelle est l'adresse ?");
        assert_eq!(
            q("date_naissance", Saisie::Date),
            "Quelle est la date de naissance ?"
        );
        assert_eq!(
            q("adresseEntreprise", Saisie::Texte),
            "Quelle est l'adresse de l'entreprise ?"
        );
        assert_eq!(q("montant", Saisie::Montant), "Quel est le montant ?");
        assert_eq!(q("remise", Saisie::Case), "« Remise » : oui ou non ?");
        assert_eq!(q("xyz", Saisie::Date), "Quelle date pour « Xyz » ?");
        assert_eq!(
            question_pour("client_name", &Saisie::Texte, Lang::En),
            "What is the client name?"
        );
        for l in Lang::ALL {
            for s in [
                Saisie::Texte,
                Saisie::Date,
                Saisie::Case,
                Saisie::Choix(vec!["a".into()]),
                Saisie::Montant,
                Saisie::Nombre,
                Saisie::Email,
                Saisie::TexteLong,
            ] {
                assert!(!question_pour("", &s, l).is_empty());
            }
        }
    }

    #[test]
    fn modele_vide_puis_rempli() {
        let mut v = Values::default();
        v.set("nom_client", "Marc");
        v.set("montant", "120");
        v.types.insert("montant".into(), VarType::Money);
        let p = Project::new(
            "Relance",
            "Bonjour {{nom_client}}, merci de régler {{montant}} ({{aujourdhui}}).",
            &v,
        );
        let m = Modele::depuis_projet("Relance", "Une relance", &p, Lang::Fr);
        assert!(m.projet.values.is_empty());
        assert_eq!(m.projet.types.get("montant"), Some(&VarType::Money));
        assert_eq!(m.questions.len(), 2);
        assert_eq!(m.questions[0].question, "Quel est le nom du client ?");
        let qs = m.questions_saisies();
        assert_eq!(qs[1].1, Saisie::Montant);
        let mut r = BTreeMap::new();
        r.insert("nom_client".to_string(), "Léa".to_string());
        let rempli = m.remplir(&r);
        assert_eq!(
            rempli.values.get("nom_client").map(String::as_str),
            Some("Léa")
        );
        // Aller-retour JSON, et fichiers abîmés refusés proprement.
        let j = m.to_json();
        assert_eq!(Modele::from_json(&j).unwrap(), m);
        assert!(Modele::from_json(&j[..j.len() / 2]).is_err());
        assert!(Modele::from_json("{\"format\":\"autre\",\"projet\":{}}").is_err());
        assert_eq!(m.nom_de_fichier(), "relance.varium-modele.json");
        let mut sans = m.clone();
        sans.nom = "  ".into();
        assert_eq!(sans.nom_de_fichier(), "modele.varium-modele.json");
    }
}
