//! Fichier de présentation `.varium-slides.json` : écriture, et lecture tolérante.
//!
//! La lecture n'échoue que si le texte n'est pas du JSON, n'est pas un objet, est
//! d'un autre format ou d'une version plus récente. Tout champ absent, nul ou d'un
//! type inattendu prend sa valeur par défaut : un fichier ancien ou abîmé s'ouvre.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::{Deck, Frame, Layout, Media, Object, Slide};

/// Nom du format, écrit dans le champ `format` du fichier.
pub const FORMAT: &str = "varium-slides";
/// Version du format écrite par cette version de Varium.
pub const VERSION: u32 = 1;

/// Forme normalisée d'un identifiant : minuscules, lettres et chiffres seulement.
pub(crate) fn cle(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Serialize)]
struct Fichier<'a> {
    format: &'static str,
    version: u32,
    #[serde(flatten)]
    deck: &'a Deck,
}

pub(crate) fn to_json(deck: &Deck) -> String {
    serde_json::to_string_pretty(&Fichier {
        format: FORMAT,
        version: VERSION,
        deck,
    })
    // Impossible en pratique (clés textuelles, aucun flottant) : jamais de panique.
    .unwrap_or_else(|_| format!("{{\"format\":\"{FORMAT}\",\"version\":{VERSION}}}"))
}

pub(crate) fn from_json(s: &str) -> Result<Deck, String> {
    let v: Value = serde_json::from_str(s).map_err(|e| format!("Présentation illisible : {e}"))?;
    let Some(obj) = v.as_object() else {
        return Err("Présentation illisible : un objet JSON est attendu".into());
    };
    match obj.get("format") {
        None | Some(Value::Null) => {}
        Some(Value::String(f)) if f == FORMAT => {}
        Some(Value::String(f)) if f == "varium" => {
            return Err("Ce fichier est un document Varium, pas une présentation".into());
        }
        Some(autre) => {
            let nom = autre
                .as_str()
                .map(str::to_string)
                .unwrap_or(autre.to_string());
            return Err(format!(
                "Format inconnu : « {} »",
                crate::texte::tronquer(&nom, 40)
            ));
        }
    }
    if let Some(version) = obj.get("version").and_then(Value::as_u64)
        && version > VERSION as u64
    {
        return Err(format!(
            "Présentation créée par une version plus récente de Varium (format v{version})"
        ));
    }
    let mut deck = Deck::deserialize(v).map_err(|e| format!("Présentation illisible : {e}"))?;
    if deck.slides.is_empty() {
        deck.slides.push(Slide::new(Layout::Title));
    }
    Ok(deck)
}

// ---------------------------------------------------------------------------------
// Lecture tolérante des champs
// ---------------------------------------------------------------------------------

/// Texte : une chaîne, ou un nombre / booléen écrit en texte ; vide sinon.
pub(crate) fn texte<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    })
}

/// Booléen, vrai s'il est absent ou illisible.
pub(crate) fn vrai_par_defaut<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Bool(b) => b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !matches!(
            cle(&s).as_str(),
            "false" | "faux" | "non" | "no" | "0" | "off"
        ),
        _ => true,
    })
}

/// Valeurs des champs : un objet dont les valeurs simples sont gardées en texte.
pub(crate) fn valeurs<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    let Value::Object(obj) = Value::deserialize(d)? else {
        return Ok(BTreeMap::new());
    };
    Ok(obj
        .into_iter()
        .filter_map(|(k, v)| {
            let v = match v {
                Value::String(s) => s,
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => return None,
            };
            Some((k, v))
        })
        .collect())
}

/// Diapositives : chaque objet du tableau en est une ; le reste est ignoré.
pub(crate) fn diapositives<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Slide>, D::Error> {
    let Value::Array(items) = Value::deserialize(d)? else {
        return Ok(Vec::new());
    };
    Ok(items
        .into_iter()
        .filter(Value::is_object)
        .map(|v| Slide::deserialize(v).unwrap_or_default())
        .collect())
}

/// Couleur : `"#rrggbb"`, `"#rgb"` ou `[r, g, b]` ; absente sinon.
pub(crate) fn couleur<'de, D: Deserializer<'de>>(d: D) -> Result<Option<[u8; 3]>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) => crate::texte::parse_color(&s),
        Value::Array(a) if a.len() == 3 => {
            let canal = |v: &Value| v.as_f64().map(|x| x.clamp(0.0, 255.0).round() as u8);
            match (canal(&a[0]), canal(&a[1]), canal(&a[2])) {
                (Some(r), Some(g), Some(b)) => Some([r, g, b]),
                _ => None,
            }
        }
        _ => None,
    })
}

pub(crate) fn couleur_ser<S: Serializer>(c: &Option<[u8; 3]>, s: S) -> Result<S::Ok, S::Error> {
    match c {
        Some(c) => s.serialize_str(&crate::texte::hex_color(*c)),
        None => s.serialize_none(),
    }
}

/// Nombre : un nombre JSON (ou un texte qui en contient un) ; `NaN` sinon, que
/// [`Object::normalized`] remplace par une valeur par défaut.
pub(crate) fn nombre<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    Ok(valeur_nombre(&Value::deserialize(d)?).unwrap_or(f32::NAN))
}

fn valeur_nombre(v: &Value) -> Option<f32> {
    match v {
        Value::Number(n) => n.as_f64().map(|x| x as f32),
        Value::String(s) => s.trim().replace(',', ".").parse::<f32>().ok(),
        _ => None,
    }
    .filter(|x| x.is_finite())
}

/// Booléen, faux s'il est absent ou illisible.
pub(crate) fn booleen<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Bool(b) => b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0),
        Value::String(s) => matches!(cle(&s).as_str(), "true" | "vrai" | "oui" | "yes" | "1"),
        _ => false,
    })
}

/// Cadre : `{"x":…, "y":…, "w":…, "h":…}` ou `[x, y, w, h]`.
pub(crate) fn cadre<'de, D: Deserializer<'de>>(d: D) -> Result<Frame, D::Error> {
    let v = Value::deserialize(d)?;
    let n = |v: Option<&Value>| v.and_then(valeur_nombre).unwrap_or(f32::NAN);
    Ok(match &v {
        Value::Array(a) => Frame::new(n(a.first()), n(a.get(1)), n(a.get(2)), n(a.get(3))),
        Value::Object(o) => Frame::new(n(o.get("x")), n(o.get("y")), n(o.get("w")), n(o.get("h"))),
        _ => Frame::new(f32::NAN, f32::NAN, f32::NAN, f32::NAN),
    })
}

/// Objets d'une diapositive : chaque objet lisible, ramené dans des bornes sûres.
pub(crate) fn objets<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Object>, D::Error> {
    let Value::Array(items) = Value::deserialize(d)? else {
        return Ok(Vec::new());
    };
    Ok(items
        .into_iter()
        .filter(Value::is_object)
        .filter_map(|v| Object::deserialize(v).ok())
        .map(Object::normalized)
        .collect())
}

/// Médias : un objet `identifiant → image` ; une image illisible est ignorée.
pub(crate) fn medias<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, Media>, D::Error> {
    let Value::Object(obj) = Value::deserialize(d)? else {
        return Ok(BTreeMap::new());
    };
    Ok(obj
        .into_iter()
        .filter_map(|(k, v)| Some((k, crate::objets::media_de_valeur(&v)?)))
        .collect())
}
