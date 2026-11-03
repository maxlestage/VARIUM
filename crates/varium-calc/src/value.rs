//! Valeurs calculées et codes d'erreur.

use serde::{Deserialize, Serialize};

/// Erreur de calcul, affichée comme dans LibreOffice Calc en français.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CalcError {
    /// Division par zéro : `#DIV/0!`.
    Div0,
    /// Mauvais type de valeur (texte dans un calcul…) : `#VALEUR!`.
    Value,
    /// Référence invalide (cellule supprimée, feuille inconnue…) : `#REF!`.
    Ref,
    /// Fonction ou nom inconnu, formule illisible : `#NOM?`.
    Name,
    /// Référence circulaire : `#CIRC!`.
    Circular,
    /// Valeur non disponible (recherche sans résultat) : `#N/A`.
    NA,
    /// Nombre invalide ou trop grand : `#NOMBRE!`.
    Num,
}

impl CalcError {
    /// Code affiché dans la cellule.
    pub fn code(self) -> &'static str {
        match self {
            CalcError::Div0 => "#DIV/0!",
            CalcError::Value => "#VALEUR!",
            CalcError::Ref => "#REF!",
            CalcError::Name => "#NOM?",
            CalcError::Circular => "#CIRC!",
            CalcError::NA => "#N/A",
            CalcError::Num => "#NOMBRE!",
        }
    }

    /// Code anglais (exports XLSX et ODS).
    pub fn english_code(self) -> &'static str {
        match self {
            CalcError::Div0 => "#DIV/0!",
            CalcError::Value | CalcError::Circular => "#VALUE!",
            CalcError::Ref => "#REF!",
            CalcError::Name => "#NAME?",
            CalcError::NA => "#N/A",
            CalcError::Num => "#NUM!",
        }
    }

    /// Explication en français, pour la barre d'état.
    pub fn description(self) -> &'static str {
        match self {
            CalcError::Div0 => "Division par zéro",
            CalcError::Value => "Type de valeur incorrect (du texte dans un calcul ?)",
            CalcError::Ref => "Référence invalide (cellule ou feuille supprimée)",
            CalcError::Name => "Nom de fonction inconnu ou formule incorrecte",
            CalcError::Circular => "Référence circulaire : la formule dépend d'elle-même",
            CalcError::NA => "Valeur non disponible (recherche sans résultat)",
            CalcError::Num => "Nombre invalide ou hors limites",
        }
    }

    /// Reconnaît un code d'erreur français ou anglais (`#VALEUR!`, `#VALUE!`…).
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code.to_uppercase().as_str() {
            "#DIV/0!" => CalcError::Div0,
            "#VALEUR!" | "#VALUE!" | "#NULL!" => CalcError::Value,
            "#REF!" => CalcError::Ref,
            "#NOM?" | "#NAME?" => CalcError::Name,
            "#CIRC!" => CalcError::Circular,
            "#N/A" => CalcError::NA,
            "#NOMBRE!" | "#NUM!" => CalcError::Num,
            _ => return None,
        })
    }
}

impl std::fmt::Display for CalcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for CalcError {}

/// Valeur d'une cellule après calcul.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Value {
    #[default]
    Empty,
    Number(f64),
    Text(String),
    Bool(bool),
    Error(CalcError),
}

impl Value {
    pub fn is_empty(&self) -> bool {
        matches!(self, Value::Empty)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Value::Error(_))
    }

    /// Le nombre porté par la valeur (`Bool` compte pour 1 ou 0, vide pour 0).
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Value::Empty => Some(0.0),
            _ => None,
        }
    }

    /// Le texte porté par la valeur, s'il s'agit de texte.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }
}
