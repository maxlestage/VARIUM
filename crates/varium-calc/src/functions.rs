//! Catalogue des fonctions : noms français et anglais, syntaxe et aide.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Fiche d'une fonction, pour l'aide et l'autocomplétion de l'interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FunctionInfo {
    /// Nom français (celui de LibreOffice Calc en français).
    pub fr: &'static str,
    /// Nom anglais (exports XLSX, et accepté à la saisie).
    pub en: &'static str,
    /// Syntaxe, arguments séparés par `;`.
    pub syntax: &'static str,
    /// Description en français.
    pub description: &'static str,
}

/// Identifiant interne d'une fonction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum F {
    Somme,
    Moyenne,
    Min,
    Max,
    Nb,
    NbVal,
    NbVide,
    Si,
    SiErreur,
    Et,
    Ou,
    Non,
    Vrai,
    Faux,
    Arrondi,
    ArrondiSup,
    ArrondiInf,
    Ent,
    Abs,
    Racine,
    Puissance,
    Mod,
    Pi,
    Produit,
    Mediane,
    Ecartype,
    Signe,
    Exp,
    Ln,
    Log10,
    SommeSi,
    NbSi,
    MoyenneSi,
    SommeSiEns,
    NbSiEns,
    SommeProd,
    RechercheV,
    Index,
    Equiv,
    Concat,
    Concatener,
    Gauche,
    Droite,
    Stxt,
    NbCar,
    Majuscule,
    Minuscule,
    SupprEspace,
    Texte,
    Trouve,
    Cherche,
    Substitue,
    Aujourdhui,
    Date,
    Annee,
    Mois,
    Jour,
    Vpm,
    EstVide,
    EstNum,
    EstTexte,
    EstErreur,
    Na,
    Convertir,
    Devise,
}

macro_rules! catalogue {
    ($( $id:ident, $fr:literal, $en:literal, $syntax:literal, $desc:literal; )*) => {
        static FUNCTIONS: &[FunctionInfo] = &[
            $( FunctionInfo { fr: $fr, en: $en, syntax: $syntax, description: $desc }, )*
        ];
        static IDS: &[F] = &[ $( F::$id, )* ];
    };
}

catalogue! {
    Somme, "SOMME", "SUM", "SOMME(nombre1; nombre2; …)",
        "Additionne les nombres et les plages de cellules.";
    Moyenne, "MOYENNE", "AVERAGE", "MOYENNE(nombre1; nombre2; …)",
        "Moyenne arithmétique des nombres (le texte et les cellules vides sont ignorés).";
    Min, "MIN", "MIN", "MIN(nombre1; nombre2; …)",
        "Plus petit des nombres (0 s'il n'y en a aucun).";
    Max, "MAX", "MAX", "MAX(nombre1; nombre2; …)",
        "Plus grand des nombres (0 s'il n'y en a aucun).";
    Nb, "NB", "COUNT", "NB(valeur1; valeur2; …)",
        "Compte les nombres (les dates sont des nombres).";
    NbVal, "NBVAL", "COUNTA", "NBVAL(valeur1; valeur2; …)",
        "Compte les cellules non vides.";
    NbVide, "NB.VIDE", "COUNTBLANK", "NB.VIDE(plage)",
        "Compte les cellules vides d'une plage.";
    Si, "SI", "IF", "SI(test; valeur_si_vrai; valeur_si_faux)",
        "Renvoie une valeur si le test est vrai, une autre sinon.";
    SiErreur, "SI.ERREUR", "IFERROR", "SI.ERREUR(valeur; valeur_si_erreur)",
        "Renvoie la valeur, ou la seconde si la première est une erreur.";
    Et, "ET", "AND", "ET(test1; test2; …)",
        "VRAI si tous les tests sont vrais.";
    Ou, "OU", "OR", "OU(test1; test2; …)",
        "VRAI si au moins un test est vrai.";
    Non, "NON", "NOT", "NON(test)",
        "Inverse une valeur logique.";
    Vrai, "VRAI", "TRUE", "VRAI()",
        "La valeur logique VRAI.";
    Faux, "FAUX", "FALSE", "FAUX()",
        "La valeur logique FAUX.";
    Arrondi, "ARRONDI", "ROUND", "ARRONDI(nombre; décimales)",
        "Arrondit au plus proche (0,5 s'arrondit en s'éloignant de zéro).";
    ArrondiSup, "ARRONDI.SUP", "ROUNDUP", "ARRONDI.SUP(nombre; décimales)",
        "Arrondit en s'éloignant de zéro.";
    ArrondiInf, "ARRONDI.INF", "ROUNDDOWN", "ARRONDI.INF(nombre; décimales)",
        "Arrondit vers zéro (tronque).";
    Ent, "ENT", "INT", "ENT(nombre)",
        "Partie entière (arrondi à l'entier inférieur).";
    Abs, "ABS", "ABS", "ABS(nombre)",
        "Valeur absolue.";
    Racine, "RACINE", "SQRT", "RACINE(nombre)",
        "Racine carrée d'un nombre positif.";
    Puissance, "PUISSANCE", "POWER", "PUISSANCE(nombre; exposant)",
        "Élève un nombre à une puissance (comme l'opérateur ^).";
    Mod, "MOD", "MOD", "MOD(dividende; diviseur)",
        "Reste de la division (du signe du diviseur).";
    Pi, "PI", "PI", "PI()",
        "Le nombre π (3,14159…).";
    Produit, "PRODUIT", "PRODUCT", "PRODUIT(nombre1; nombre2; …)",
        "Multiplie les nombres entre eux.";
    Mediane, "MEDIANE", "MEDIAN", "MEDIANE(nombre1; nombre2; …)",
        "Valeur médiane des nombres.";
    Ecartype, "ECARTYPE", "STDEV", "ECARTYPE(nombre1; nombre2; …)",
        "Écart type d'un échantillon (division par n − 1).";
    Signe, "SIGNE", "SIGN", "SIGNE(nombre)",
        "1 si le nombre est positif, −1 s'il est négatif, 0 sinon.";
    Exp, "EXP", "EXP", "EXP(nombre)",
        "e élevé à la puissance donnée.";
    Ln, "LN", "LN", "LN(nombre)",
        "Logarithme népérien.";
    Log10, "LOG10", "LOG10", "LOG10(nombre)",
        "Logarithme décimal.";
    SommeSi, "SOMME.SI", "SUMIF", "SOMME.SI(plage; critère; plage_somme)",
        "Additionne les cellules qui répondent au critère (\">10\", \"<>x\", \"texte\", 5…).";
    NbSi, "NB.SI", "COUNTIF", "NB.SI(plage; critère)",
        "Compte les cellules qui répondent au critère.";
    MoyenneSi, "MOYENNE.SI", "AVERAGEIF", "MOYENNE.SI(plage; critère; plage_moyenne)",
        "Moyenne des cellules qui répondent au critère.";
    SommeSiEns, "SOMME.SI.ENS", "SUMIFS", "SOMME.SI.ENS(plage_somme; plage1; critère1; …)",
        "Additionne les cellules qui répondent à tous les critères.";
    NbSiEns, "NB.SI.ENS", "COUNTIFS", "NB.SI.ENS(plage1; critère1; plage2; critère2; …)",
        "Compte les lignes qui répondent à tous les critères.";
    SommeProd, "SOMMEPROD", "SUMPRODUCT", "SOMMEPROD(plage1; plage2; …)",
        "Somme des produits terme à terme de plages de même taille.";
    RechercheV, "RECHERCHEV", "VLOOKUP", "RECHERCHEV(valeur; tableau; colonne; trié)",
        "Cherche une valeur dans la première colonne d'un tableau et renvoie celle de la colonne indiquée (trié = FAUX pour une correspondance exacte).";
    Index, "INDEX", "INDEX", "INDEX(plage; ligne; colonne)",
        "Valeur de la cellule à la ligne et à la colonne données dans la plage.";
    Equiv, "EQUIV", "MATCH", "EQUIV(valeur; plage; type)",
        "Position d'une valeur dans une plage (type 0 : correspondance exacte).";
    Concat, "CONCAT", "CONCAT", "CONCAT(texte1; texte2; …)",
        "Met bout à bout des textes et le contenu de plages.";
    Concatener, "CONCATENER", "CONCATENATE", "CONCATENER(texte1; texte2; …)",
        "Met bout à bout des textes.";
    Gauche, "GAUCHE", "LEFT", "GAUCHE(texte; nombre)",
        "Premiers caractères d'un texte.";
    Droite, "DROITE", "RIGHT", "DROITE(texte; nombre)",
        "Derniers caractères d'un texte.";
    Stxt, "STXT", "MID", "STXT(texte; début; nombre)",
        "Partie d'un texte, à partir de la position donnée (1 pour le premier caractère).";
    NbCar, "NBCAR", "LEN", "NBCAR(texte)",
        "Nombre de caractères d'un texte.";
    Majuscule, "MAJUSCULE", "UPPER", "MAJUSCULE(texte)",
        "Met un texte en majuscules.";
    Minuscule, "MINUSCULE", "LOWER", "MINUSCULE(texte)",
        "Met un texte en minuscules.";
    SupprEspace, "SUPPRESPACE", "TRIM", "SUPPRESPACE(texte)",
        "Supprime les espaces au début et à la fin, et les espaces doubles.";
    Texte, "TEXTE", "TEXT", "TEXTE(valeur; format)",
        "Met un nombre en forme de texte : \"0,00\", \"# ##0 €\", \"0 %\", \"jj/mm/aaaa\"…";
    Trouve, "TROUVE", "FIND", "TROUVE(cherché; texte; début)",
        "Position d'un texte dans un autre (respecte la casse).";
    Cherche, "CHERCHE", "SEARCH", "CHERCHE(cherché; texte; début)",
        "Position d'un texte dans un autre (sans tenir compte de la casse).";
    Substitue, "SUBSTITUE", "SUBSTITUTE", "SUBSTITUE(texte; ancien; nouveau; occurrence)",
        "Remplace un texte par un autre (toutes les occurrences, ou seulement celle indiquée).";
    Aujourdhui, "AUJOURDHUI", "TODAY", "AUJOURDHUI()",
        "Date du jour.";
    Date, "DATE", "DATE", "DATE(année; mois; jour)",
        "Date à partir de l'année, du mois et du jour.";
    Annee, "ANNEE", "YEAR", "ANNEE(date)",
        "Année d'une date.";
    Mois, "MOIS", "MONTH", "MOIS(date)",
        "Mois d'une date (1 à 12).";
    Jour, "JOUR", "DAY", "JOUR(date)",
        "Jour du mois d'une date (1 à 31).";
    Vpm, "VPM", "PMT", "VPM(taux; nombre_périodes; valeur_actuelle; valeur_future; type)",
        "Remboursement périodique d'un emprunt à taux constant (négatif : c'est un paiement).";
    EstVide, "ESTVIDE", "ISBLANK", "ESTVIDE(valeur)",
        "VRAI si la cellule est vide.";
    EstNum, "ESTNUM", "ISNUMBER", "ESTNUM(valeur)",
        "VRAI si la valeur est un nombre.";
    EstTexte, "ESTTEXTE", "ISTEXT", "ESTTEXTE(valeur)",
        "VRAI si la valeur est du texte.";
    EstErreur, "ESTERREUR", "ISERROR", "ESTERREUR(valeur)",
        "VRAI si la valeur est une erreur.";
    Na, "NA", "NA", "NA()",
        "Renvoie l'erreur #N/A (valeur non disponible).";
    Convertir, "CONVERTIR", "CONVERT", "CONVERTIR(nombre; de_unité; vers_unité)",
        "Convertit une mesure d'une unité dans une autre : longueurs (\"m\", \"km\", \"mi\", \"pouce\"), masses (\"kg\", \"lbm\"), volumes (\"l\", \"gal\"), températures (\"C\", \"F\", \"K\"), durées, surfaces, vitesses, énergies, données (\"Mo\", \"GiB\").";
    Devise, "DEVISE", "CURRENCY", "DEVISE(montant; de; vers; date)",
        "Convertit un montant d'une monnaie dans une autre (\"EUR\", \"USD\"…) avec la table des taux de change du classeur (Données → Taux de change) ; la date choisit des taux enregistrés à cette date.";
}

/// Toutes les fonctions connues, dans l'ordre du catalogue.
pub fn functions() -> &'static [FunctionInfo] {
    FUNCTIONS
}

/// Catégorie d'une fonction (assistant de formule).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FunctionCategory {
    Math,
    Statistics,
    Logic,
    Text,
    Date,
    Lookup,
    Finance,
    Information,
}

impl FunctionCategory {
    /// Toutes les catégories, dans l'ordre des menus.
    pub const ALL: [FunctionCategory; 8] = [
        FunctionCategory::Math,
        FunctionCategory::Statistics,
        FunctionCategory::Logic,
        FunctionCategory::Text,
        FunctionCategory::Date,
        FunctionCategory::Lookup,
        FunctionCategory::Finance,
        FunctionCategory::Information,
    ];

    /// Libellé français.
    pub fn label(self) -> &'static str {
        match self {
            FunctionCategory::Math => "Mathématiques",
            FunctionCategory::Statistics => "Statistiques",
            FunctionCategory::Logic => "Logique",
            FunctionCategory::Text => "Texte",
            FunctionCategory::Date => "Date et heure",
            FunctionCategory::Lookup => "Recherche",
            FunctionCategory::Finance => "Finances",
            FunctionCategory::Information => "Information",
        }
    }
}

fn category_of(id: F) -> FunctionCategory {
    use FunctionCategory as C;
    match id {
        F::Somme
        | F::Arrondi
        | F::ArrondiSup
        | F::ArrondiInf
        | F::Ent
        | F::Abs
        | F::Racine
        | F::Puissance
        | F::Mod
        | F::Pi
        | F::Produit
        | F::Signe
        | F::Exp
        | F::Ln
        | F::Log10
        | F::SommeSi
        | F::SommeSiEns
        | F::SommeProd
        | F::Convertir => C::Math,
        F::Moyenne
        | F::Min
        | F::Max
        | F::Nb
        | F::NbVal
        | F::NbVide
        | F::Mediane
        | F::Ecartype
        | F::NbSi
        | F::MoyenneSi
        | F::NbSiEns => C::Statistics,
        F::Si | F::SiErreur | F::Et | F::Ou | F::Non | F::Vrai | F::Faux => C::Logic,
        F::Concat
        | F::Concatener
        | F::Gauche
        | F::Droite
        | F::Stxt
        | F::NbCar
        | F::Majuscule
        | F::Minuscule
        | F::SupprEspace
        | F::Texte
        | F::Trouve
        | F::Cherche
        | F::Substitue => C::Text,
        F::Aujourdhui | F::Date | F::Annee | F::Mois | F::Jour => C::Date,
        F::RechercheV | F::Index | F::Equiv => C::Lookup,
        F::Vpm | F::Devise => C::Finance,
        F::EstVide | F::EstNum | F::EstTexte | F::EstErreur | F::Na => C::Information,
    }
}

impl FunctionInfo {
    /// Catégorie de la fonction.
    pub fn category(&self) -> FunctionCategory {
        lookup(self.fr).map_or(FunctionCategory::Information, category_of)
    }

    /// Noms des arguments, d'après la syntaxe (`SOMME(nombre1; nombre2; …)` →
    /// `["nombre1", "nombre2", "…"]`).
    pub fn arguments(&self) -> Vec<&'static str> {
        let Some(open) = self.syntax.find('(') else {
            return Vec::new();
        };
        let inner = self.syntax[open + 1..].trim_end_matches(')');
        inner
            .split(';')
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .collect()
    }
}

/// Autres noms acceptés à la saisie (variantes de LibreOffice ou d'Excel).
const ALIASES: &[(&str, F)] = &[
    ("SIERREUR", F::SiErreur),
    ("ECARTYPE.STANDARD", F::Ecartype),
    ("STDEV.S", F::Ecartype),
    ("_XLFN.CONCAT", F::Concat),
    ("COM.MICROSOFT.CONCAT", F::Concat),
    ("_XLFN.STDEV.S", F::Ecartype),
];

/// Majuscules sans accents : `Médiane` → `MEDIANE`.
pub(crate) fn normalize_name(name: &str) -> String {
    name.chars()
        .flat_map(char::to_uppercase)
        .map(|c| match c {
            'À' | 'Â' | 'Ä' | 'Á' => 'A',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'Î' | 'Ï' | 'Í' => 'I',
            'Ô' | 'Ö' | 'Ó' => 'O',
            'Ù' | 'Û' | 'Ü' | 'Ú' => 'U',
            'Ç' => 'C',
            'Ÿ' => 'Y',
            c => c,
        })
        .collect()
}

fn table() -> &'static HashMap<String, F> {
    static TABLE: OnceLock<HashMap<String, F>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut m = HashMap::new();
        for (info, id) in FUNCTIONS.iter().zip(IDS) {
            m.insert(normalize_name(info.fr), *id);
            m.insert(normalize_name(info.en), *id);
        }
        for (name, id) in ALIASES {
            m.insert((*name).to_string(), *id);
        }
        m
    })
}

/// Cherche une fonction par son nom français ou anglais (casse et accents ignorés).
pub(crate) fn lookup(name: &str) -> Option<F> {
    table().get(&normalize_name(name)).copied()
}

/// Fiche d'une fonction d'après son identifiant.
pub(crate) fn info(id: F) -> &'static FunctionInfo {
    let i = IDS.iter().position(|x| *x == id).unwrap_or(0);
    &FUNCTIONS[i]
}

/// Cherche la fiche d'une fonction par son nom français ou anglais.
pub fn function_info(name: &str) -> Option<&'static FunctionInfo> {
    lookup(name).map(info)
}

/// Nom anglais tel qu'il s'écrit dans un fichier XLSX (préfixe `_xlfn.` des
/// fonctions récentes d'Excel).
pub(crate) fn xlsx_name(id: F) -> String {
    match id {
        F::Concat => "_xlfn.CONCAT".into(),
        _ => info(id).en.into(),
    }
}

/// Nom OpenFormula (ODS).
pub(crate) fn odf_name(id: F) -> String {
    match id {
        F::Concat => "COM.MICROSOFT.CONCAT".into(),
        _ => info(id).en.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_coherent() {
        assert_eq!(FUNCTIONS.len(), IDS.len());
        for (info_, id) in FUNCTIONS.iter().zip(IDS) {
            assert_eq!(lookup(info_.fr), Some(*id), "{}", info_.fr);
            assert_eq!(lookup(info_.en), Some(*id), "{}", info_.en);
            assert_eq!(info(*id).fr, info_.fr);
            assert!(info_.syntax.starts_with(info_.fr), "{}", info_.syntax);
            assert!(!info_.description.is_empty());
        }
    }

    #[test]
    fn noms_sans_accents_ni_casse() {
        assert_eq!(lookup("médiane"), Some(F::Mediane));
        assert_eq!(lookup("Année"), Some(F::Annee));
        assert_eq!(lookup("sierreur"), Some(F::SiErreur));
        assert_eq!(lookup("si.erreur"), Some(F::SiErreur));
        assert_eq!(lookup("vlookup"), Some(F::RechercheV));
        assert_eq!(lookup("INCONNUE"), None);
        assert_eq!(function_info("somme").map(|i| i.en), Some("SUM"));
    }

    #[test]
    fn categories_et_arguments() {
        let somme = function_info("SOMME").unwrap();
        assert_eq!(somme.category(), FunctionCategory::Math);
        assert_eq!(somme.arguments(), vec!["nombre1", "nombre2", "…"]);
        assert!(function_info("PI").unwrap().arguments().is_empty());
        assert_eq!(
            function_info("RECHERCHEV").unwrap().category(),
            FunctionCategory::Lookup
        );
        for f in FUNCTIONS {
            let _ = f.category();
        }
    }
}
