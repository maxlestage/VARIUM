//! Modèles prêts à l'emploi (Fichier → Nouveau à partir d'un modèle).

use crate::tr::n_;
use varium_core::{PageSetup, Project, Values, VarType};

pub struct Template {
    pub name: &'static str,
    pub icon: &'static str,
    pub description: &'static str,
    pub title: &'static str,
    pub source: &'static str,
    pub values: fn() -> Values,
    pub page: fn() -> PageSetup,
}

impl Template {
    pub fn project(&self) -> Project {
        let mut p = Project::new(self.title, self.source, &(self.values)());
        p.page = (self.page)();
        p
    }
}

fn typed(pairs: &[(&str, &str)], types: &[(&str, VarType)]) -> Values {
    let mut v = Values::default();
    for (k, val) in pairs {
        v.set(*k, *val);
    }
    for (k, t) in types {
        v.types.insert((*k).into(), *t);
    }
    v
}

fn page_default() -> PageSetup {
    PageSetup::default()
}

fn page_numbered() -> PageSetup {
    PageSetup {
        page_numbers: true,
        ..PageSetup::default()
    }
}

pub const LETTRE: &str = r#"**{{entreprise | upper}}**
{{adresse}}

-> {{ville}}, le {{date_courrier | date:long}} ->

**Objet : dossier {{référence}}**

{{civilité | default:"Madame, Monsieur"}},

Votre dossier {{référence}} est en cours de traitement. Le montant retenu s'élève à {{montant}}, soit une mensualité de {{= montant / durée | money}} sur {{durée}} mois.

Pour mémoire, le capital suit la formule :
$$ C_n = C_0 \times (1 + t)^n $$

Nous restons à votre disposition, {{prénom}}, pour toute question.

Je vous prie d'agréer, {{civilité | default:"Madame, Monsieur"}}, l'expression de mes salutations distinguées.

-> {{signataire | default:"Le service client"}} ->
-> {{entreprise}} ->"#;

fn lettre_values() -> Values {
    typed(
        &[
            ("entreprise", "Atelier Lumen"),
            ("adresse", "12 rue des Lilas\n69003 Lyon"),
            ("ville", "Lyon"),
            ("date_courrier", "2026-11-03"),
            ("référence", "REF-2026-015"),
            ("civilité", "Monsieur"),
            ("prénom", "Marc"),
            ("montant", "12500"),
            ("durée", "24"),
        ],
        &[
            ("date_courrier", VarType::Date),
            ("montant", VarType::Money),
            ("durée", VarType::Number),
            ("adresse", VarType::Long),
        ],
    )
}

const FACTURE: &str = r#"**{{entreprise | upper}}**
{{adresse_entreprise}}
SIRET {{siret}}

-> **{{client}}** ->
-> {{adresse_client}} ->

# Facture n° {{numéro}}

Date : {{date_facture}} · Échéance : {{échéance}}

| Désignation | Quantité | Prix unitaire HT | Total HT |
|---|:---:|---:|---:|
| {{prestation_1}} | {{quantité_1}} | {{prix_1}} | {{= quantité_1 * prix_1}} |
| {{prestation_2}} | {{quantité_2}} | {{prix_2}} | {{= quantité_2 * prix_2}} |

-> Total HT : **{{= quantité_1 * prix_1 + quantité_2 * prix_2}}** ->
-> TVA {{taux_tva}} % : {{= (quantité_1 * prix_1 + quantité_2 * prix_2) * taux_tva / 100 | money}} ->
-> **Total TTC : {{= (quantité_1 * prix_1 + quantité_2 * prix_2) * (1 + taux_tva / 100) | money}}** ->

---
Paiement par virement à réception · IBAN {{iban}}
*En cas de retard, une pénalité de trois fois le taux d'intérêt légal sera appliquée, ainsi qu'une indemnité forfaitaire de 40 € pour frais de recouvrement.*"#;

fn facture_values() -> Values {
    typed(
        &[
            ("entreprise", "Atelier Lumen"),
            ("adresse_entreprise", "12 rue des Lilas, 69003 Lyon"),
            ("siret", "123 456 789 00012"),
            ("client", "Société Horizon"),
            ("adresse_client", "4 place Bellecour\n69002 Lyon"),
            ("numéro", "F-2026-042"),
            ("date_facture", "2026-11-03"),
            ("échéance", "2026-12-03"),
            ("prestation_1", "Conception du site"),
            ("quantité_1", "3"),
            ("prix_1", "450"),
            ("prestation_2", "Hébergement (1 an)"),
            ("quantité_2", "1"),
            ("prix_2", "120"),
            ("taux_tva", "20"),
            ("iban", "FR76 3000 6000 0112 3456 7890 189"),
        ],
        &[
            ("date_facture", VarType::Date),
            ("échéance", VarType::Date),
            ("prix_1", VarType::Money),
            ("prix_2", VarType::Money),
            ("quantité_1", VarType::Number),
            ("quantité_2", VarType::Number),
            ("taux_tva", VarType::Number),
            ("adresse_client", VarType::Long),
        ],
    )
}

const DEVIS: &str = r#"# Devis n° {{numéro}}

**{{entreprise}}** — {{adresse_entreprise}}
Pour : **{{client}}**
Établi le {{date_devis}}, valable jusqu'au {{validité}}.

## Prestations

| Poste | Jours | Tarif journalier | Montant |
|---|:---:|---:|---:|
| {{poste_1}} | {{jours_1}} | {{tarif}} | {{= jours_1 * tarif}} |
| {{poste_2}} | {{jours_2}} | {{tarif}} | {{= jours_2 * tarif}} |

-> **Total HT : {{= (jours_1 + jours_2) * tarif}}** ->

## Conditions

- Acompte de {{acompte}} % à la commande, soit {{= (jours_1 + jours_2) * tarif * acompte / 100 | money}}.
- Solde à la livraison.

Bon pour accord, le ____________________

Signature du client :"#;

fn devis_values() -> Values {
    typed(
        &[
            ("numéro", "D-2026-007"),
            ("entreprise", "Atelier Lumen"),
            ("adresse_entreprise", "12 rue des Lilas, 69003 Lyon"),
            ("client", "Société Horizon"),
            ("date_devis", "2026-11-03"),
            ("validité", "2026-12-03"),
            ("poste_1", "Ateliers et maquettes"),
            ("jours_1", "4"),
            ("poste_2", "Développement"),
            ("jours_2", "10"),
            ("tarif", "550"),
            ("acompte", "30"),
        ],
        &[
            ("date_devis", VarType::Date),
            ("validité", VarType::Date),
            ("tarif", VarType::Money),
            ("jours_1", VarType::Number),
            ("jours_2", VarType::Number),
            ("acompte", VarType::Number),
        ],
    )
}

const ATTESTATION: &str = r#"-> # Attestation de {{objet | default:"présence"}} <-

Je soussigné(e), **{{signataire}}**, {{fonction}} de {{organisme}}, atteste que :

-> **{{prénom}} {{nom | upper}}** <-
-> né(e) le {{date_naissance}} <-

a {{action | default:"participé à la formation"}} « *{{intitulé}}* » du {{date_début}} au {{date_fin}}, pour une durée de {{heures}} heures.

Cette attestation est délivrée pour servir et valoir ce que de droit.

-> Fait à {{ville}}, le {{aujourdhui}} ->

-> {{signataire}} ->"#;

fn attestation_values() -> Values {
    typed(
        &[
            ("signataire", "Claire Martin"),
            ("fonction", "directrice"),
            ("organisme", "Formations Lumen"),
            ("prénom", "Marc"),
            ("nom", "Dupont"),
            ("date_naissance", "1990-04-12"),
            ("intitulé", "Rédiger des documents efficaces"),
            ("date_début", "2026-10-12"),
            ("date_fin", "2026-10-16"),
            ("heures", "35"),
            ("ville", "Lyon"),
        ],
        &[
            ("date_naissance", VarType::Date),
            ("date_début", VarType::Date),
            ("date_fin", VarType::Date),
            ("aujourdhui", VarType::Date),
            ("heures", VarType::Number),
        ],
    )
}

const CONTRAT: &str = r#"-> # Contrat de prestation de services <-

**Entre les soussignés :**

{{prestataire}}, {{adresse_prestataire}}, ci-après « le Prestataire »,

**et**

{{client}}, {{adresse_client}}, ci-après « le Client ».

## Article 1 — Objet

Le Prestataire s'engage à réaliser pour le Client : {{mission}}.

## Article 2 — Durée

Le contrat prend effet le {{date_début}} pour une durée de {{durée}} mois.

## Article 3 — Prix

La prestation est facturée {{prix}} HT, soit {{= prix * 1,2 | money}} TTC, payable en {{échéances}} échéances de {{= prix * 1,2 / échéances | money}}.

## Article 4 — Résiliation

Chaque partie peut résilier le contrat par lettre recommandée avec un préavis de {{préavis | default:"30"}} jours.

\newpage
## Signatures

Fait en deux exemplaires à {{ville}}, le {{aujourdhui}}.

| Le Prestataire | Le Client |
|---|---|
| {{prestataire}} | {{client}} |
| Lu et approuvé | Lu et approuvé |"#;

fn contrat_values() -> Values {
    typed(
        &[
            ("prestataire", "Atelier Lumen"),
            ("adresse_prestataire", "12 rue des Lilas, 69003 Lyon"),
            ("client", "Société Horizon"),
            ("adresse_client", "4 place Bellecour, 69002 Lyon"),
            ("mission", "la refonte de son site internet"),
            ("date_début", "2026-12-01"),
            ("durée", "6"),
            ("prix", "9000"),
            ("échéances", "3"),
            ("ville", "Lyon"),
        ],
        &[
            ("date_début", VarType::Date),
            ("aujourdhui", VarType::Date),
            ("prix", VarType::Money),
            ("durée", VarType::Number),
            ("échéances", VarType::Number),
        ],
    )
}

const CONVOCATION: &str = r#"**{{entreprise}}**
Service des ressources humaines

-> {{prénom}} {{nom}} ->
-> {{adresse}} ->

**Objet : convocation à un entretien**

Bonjour {{prénom}},

Nous avons le plaisir de vous convier à un entretien pour le poste de **{{poste}}** :

- **Date :** {{date_entretien | date:full}}
- **Heure :** {{heure}}
- **Lieu :** {{lieu}}
- **Avec :** {{recruteur}}

Merci de vous munir d'une pièce d'identité. En cas d'empêchement, prévenez-nous à {{email_contact}}.

Cordialement,

{{recruteur}}"#;

fn convocation_values() -> Values {
    typed(
        &[
            ("entreprise", "Atelier Lumen"),
            ("prénom", "Léa"),
            ("nom", "Bernard"),
            ("adresse", "8 rue Garibaldi\n69006 Lyon"),
            ("poste", "chargée de communication"),
            ("date_entretien", "2026-11-18"),
            ("heure", "10 h 30"),
            ("lieu", "12 rue des Lilas, 69003 Lyon"),
            ("recruteur", "Claire Martin"),
            ("email_contact", "rh@atelier-lumen.fr"),
        ],
        &[
            ("date_entretien", VarType::Date),
            ("email_contact", VarType::Email),
            ("adresse", VarType::Long),
        ],
    )
}

pub const ALL: &[Template] = &[
    Template {
        name: n_("Lettre"),
        icon: "✉",
        description: n_("Courrier avec objet, montant et mensualité calculée"),
        title: "Courrier de suivi",
        source: LETTRE,
        values: lettre_values,
        page: page_default,
    },
    Template {
        name: n_("Facture"),
        icon: "🧾",
        description: n_("Tableau des prestations, totaux HT, TVA et TTC calculés"),
        title: "Facture",
        source: FACTURE,
        values: facture_values,
        page: page_default,
    },
    Template {
        name: n_("Devis"),
        icon: "📝",
        description: n_("Postes, total et acompte calculés automatiquement"),
        title: "Devis",
        source: DEVIS,
        values: devis_values,
        page: page_default,
    },
    Template {
        name: n_("Attestation"),
        icon: "🎓",
        description: n_("Attestation de formation datée du jour"),
        title: "Attestation",
        source: ATTESTATION,
        values: attestation_values,
        page: page_default,
    },
    Template {
        name: n_("Contrat"),
        icon: "📑",
        description: n_("Contrat de prestation en articles, sur deux pages"),
        title: "Contrat de prestation",
        source: CONTRAT,
        values: contrat_values,
        page: page_numbered,
    },
    Template {
        name: n_("Convocation"),
        icon: "📅",
        description: n_("Invitation à un entretien, idéale pour le publipostage"),
        title: "Convocation",
        source: CONVOCATION,
        values: convocation_values,
        page: page_default,
    },
];

/// Le document ouvert au premier lancement.
pub fn first() -> Project {
    ALL[0].project()
}

// ---------------------------------------------------------------------------------
// Modèles intelligents : lettres types et modèles de l'utilisateur
// ---------------------------------------------------------------------------------

/// Un modèle intelligent de la galerie : une lettre type (sans fichier) ou un modèle
/// enregistré par l'utilisateur.
#[derive(Clone, Debug)]
pub struct Intelligent {
    pub modele: varium_core::modele::Modele,
    pub chemin: Option<std::path::PathBuf>,
}

/// Le dossier des modèles de l'utilisateur (`…/varium/modeles`).
pub fn dossier_modeles() -> std::path::PathBuf {
    crate::secours::dossier_donnees().join("modeles")
}

/// Les modèles enregistrés dans `dossier`, triés par nom. Un fichier illisible est
/// ignoré (jamais d'erreur).
pub fn galerie(dossier: &std::path::Path) -> Vec<Intelligent> {
    let Ok(entrees) = std::fs::read_dir(dossier) else {
        return Vec::new();
    };
    let mut out: Vec<Intelligent> = entrees
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(varium_core::modele::EXTENSION))
        })
        .filter_map(|p| {
            let t = std::fs::read_to_string(&p).ok()?;
            let modele = varium_core::modele::Modele::from_json(&t).ok()?;
            Some(Intelligent {
                modele,
                chemin: Some(p),
            })
        })
        .collect();
    out.sort_by_key(|i| i.modele.nom.to_lowercase());
    out
}

/// Enregistre un modèle dans `dossier`, sous un nom de fichier libre.
pub fn enregistrer_modele(
    dossier: &std::path::Path,
    m: &varium_core::modele::Modele,
) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(dossier)?;
    let base = m.nom_de_fichier();
    let racine = base
        .strip_suffix(&format!(".{}", varium_core::modele::EXTENSION))
        .unwrap_or(&base)
        .to_string();
    let mut chemin = dossier.join(&base);
    let mut k = 2;
    while chemin.exists() {
        chemin = dossier.join(format!("{racine}-{k}.{}", varium_core::modele::EXTENSION));
        k += 1;
    }
    crate::io::write_atomic(&chemin, m.to_json().as_bytes())?;
    oublier_cache();
    Ok(chemin)
}

/// Supprime un modèle de l'utilisateur.
pub fn supprimer_modele(chemin: &std::path::Path) -> std::io::Result<()> {
    let r = std::fs::remove_file(chemin);
    oublier_cache();
    r
}

type Cache = Option<(std::time::Instant, varium_core::Lang, Vec<Intelligent>)>;
static CACHE: std::sync::Mutex<Cache> = std::sync::Mutex::new(None);

fn oublier_cache() {
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Les modèles intelligents proposés : les lettres types dans la langue de
/// l'interface, puis ceux de l'utilisateur (relus au plus toutes les 3 secondes).
pub fn intelligents() -> Vec<Intelligent> {
    let lang = crate::tr::lang();
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((t, l, v)) = cache.as_ref()
        && *l == lang
        && t.elapsed() < std::time::Duration::from_secs(3)
    {
        return v.clone();
    }
    let mut v: Vec<Intelligent> = varium_core::lettres::lettres(lang)
        .into_iter()
        .map(|modele| Intelligent {
            modele,
            chemin: None,
        })
        .collect();
    v.extend(galerie(&dossier_modeles()));
    *cache = Some((std::time::Instant::now(), lang, v.clone()));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn galerie_enregistrer_relire_supprimer() {
        let dir = std::env::temp_dir().join(format!("varium-modeles-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(galerie(&dir).is_empty());
        let p = ALL[0].project();
        let m =
            varium_core::modele::Modele::depuis_projet("Ma lettre", "", &p, varium_core::Lang::Fr);
        let a = enregistrer_modele(&dir, &m).unwrap();
        let b = enregistrer_modele(&dir, &m).unwrap();
        assert_ne!(a, b, "deux fichiers distincts");
        std::fs::write(dir.join("abime.varium-modele.json"), "{pas du json").unwrap();
        std::fs::write(dir.join("autre.txt"), "x").unwrap();
        let g = galerie(&dir);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].modele.nom, "Ma lettre");
        assert!(g[0].modele.projet.values.is_empty());
        supprimer_modele(&a).unwrap();
        assert_eq!(galerie(&dir).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn modeles_d_exemple_ouvrent_et_reviennent_par_odt_et_docx() {
        // Chaque modèle exporté en ODT et en DOCX puis rouvert garde ses titres, ses
        // listes, ses tableaux et son texte.
        use varium_core::{Values, document, docx, export, import, odt};
        fn resume(paras: &[varium_core::Para]) -> Vec<String> {
            paras
                .iter()
                .map(|p| {
                    let genre = match p {
                        varium_core::Para::Heading { level, .. } => format!("H{level}"),
                        varium_core::Para::Item { depth, number, .. } => {
                            format!("L{depth}{}", number.is_some())
                        }
                        varium_core::Para::Table(t) => format!("T{}", t.rows.len()),
                        varium_core::Para::Text { align, .. } => format!("{align:?}"),
                        // Une formule centrée s'exporte en texte centré.
                        varium_core::Para::Math { .. } => "Center".to_string(),
                        other => format!("{other:?}").chars().take(4).collect(),
                    };
                    let t: String = p.plain().chars().filter(|c| c.is_alphanumeric()).collect();
                    format!("{genre}:{t}")
                })
                .filter(|s| !s.ends_with(':') || s.starts_with('T'))
                .collect()
        }
        for t in ALL {
            let p = t.project();
            let mut page = p.page.clone();
            page.typographie = false;
            let r = export::prepare(&p.source, &p.values(), &page);
            let attendu = resume(&r.paras);
            for (nom, octets) in [
                ("m.odt", odt::odt(t.title, std::slice::from_ref(&r), &page)),
                (
                    "m.docx",
                    docx::docx(t.title, std::slice::from_ref(&r), &page),
                ),
            ] {
                let i = import::importer(nom, &octets).unwrap();
                let relu = resume(&document(&i.source, &Values::default()));
                assert_eq!(relu, attendu, "{} ({nom}) :\n{}", t.title, i.source);
            }
        }
    }
}
