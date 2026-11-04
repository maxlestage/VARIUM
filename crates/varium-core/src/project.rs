//! Fichier projet `.varium.json` : source + valeurs + types.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::page::PageSetup;
use crate::render::{Values, VarType};

pub const FORMAT: &str = "varium";
/// Version du format : 1 (texte, valeurs, types), 2 (+ mise en page), 3 (+ objectif
/// de mots). Les fichiers plus anciens se lisent toujours.
pub const VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    #[serde(default = "format_name")]
    pub format: String,
    #[serde(default = "version")]
    pub version: u32,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    #[serde(default)]
    pub types: BTreeMap<String, VarType>,
    /// Mise en page (depuis la version 2 ; les projets v1 prennent celle par défaut).
    #[serde(default)]
    pub page: PageSetup,
    /// Objectif de mots (depuis la version 3 ; absent : pas d'objectif).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objectif: Option<u32>,
}

fn format_name() -> String {
    FORMAT.into()
}
fn version() -> u32 {
    VERSION
}

impl Default for Project {
    fn default() -> Self {
        Self {
            format: FORMAT.into(),
            version: VERSION,
            title: String::new(),
            source: String::new(),
            values: BTreeMap::new(),
            types: BTreeMap::new(),
            page: PageSetup::default(),
            objectif: None,
        }
    }
}

impl Project {
    pub fn new(title: impl Into<String>, source: impl Into<String>, values: &Values) -> Self {
        Self {
            title: title.into(),
            source: source.into(),
            values: values.values.clone(),
            types: values.types.clone(),
            ..Self::default()
        }
    }

    pub fn values(&self) -> Values {
        Values {
            values: self.values.clone(),
            types: self.types.clone(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("sérialisation du projet")
    }

    pub fn from_json(s: &str) -> Result<Self, String> {
        let p: Project = serde_json::from_str(s).map_err(|e| format!("Projet illisible : {e}"))?;
        if p.format != FORMAT {
            return Err(format!("Format inconnu : « {} »", p.format));
        }
        if p.version > VERSION {
            return Err(format!(
                "Projet créé par une version plus récente (v{})",
                p.version
            ));
        }
        Ok(p)
    }

    /// Récupération tolérante d'un projet abîmé (JSON tronqué, champ au mauvais
    /// type, format inconnu ou version trop récente). Renvoie le projet reconstitué
    /// et la liste, en français, de ce qui a été perdu ou réparé ; `None` si rien
    /// d'exploitable (pas même une source).
    pub fn recuperer(s: &str) -> Option<(Project, Vec<String>)> {
        use serde_json::Value;
        let mut notes = Vec::new();
        let valeur: Value = match serde_json::from_str(s) {
            Ok(v) => v,
            Err(_) => {
                // Fichier tronqué : on tente de refermer les chaînes et objets ouverts.
                let base = s.trim_end();
                let v = ["\"}", "}", "\"}}", "}}", "\"\"}", "\"\"}}", "\"]}}", "]}}"]
                    .iter()
                    .find_map(|fin| serde_json::from_str::<Value>(&format!("{base}{fin}")).ok())?;
                notes.push("Fichier tronqué : la fin a été refermée automatiquement".to_string());
                v
            }
        };
        let objet = valeur.as_object()?;
        let source = objet.get("source")?.as_str()?.to_string();
        let mut p = Project {
            source,
            ..Project::default()
        };
        match objet.get("format").and_then(Value::as_str) {
            Some(f) if f != FORMAT => notes.push(format!("Format inconnu : « {f} »")),
            _ => {}
        }
        if let Some(v) = objet.get("version").and_then(Value::as_u64)
            && v > u64::from(VERSION)
        {
            notes.push(format!(
                "Projet créé par une version plus récente (v{v}) : certains réglages peuvent manquer"
            ));
        }
        match objet.get("title") {
            Some(Value::String(t)) => p.title = t.clone(),
            Some(_) => notes.push("Titre illisible : ignoré".into()),
            None => {}
        }
        match objet.get("values") {
            Some(Value::Object(m)) => {
                for (k, v) in m {
                    match v {
                        Value::String(t) => {
                            p.values.insert(k.clone(), t.clone());
                        }
                        Value::Number(n) => {
                            p.values.insert(k.clone(), n.to_string());
                        }
                        Value::Bool(b) => {
                            p.values.insert(k.clone(), b.to_string());
                        }
                        _ => notes.push(format!("Valeur du champ « {k} » illisible : ignorée")),
                    }
                }
            }
            Some(_) => notes.push("Valeurs illisibles : ignorées".into()),
            None => {}
        }
        if let Some(Value::Object(m)) = objet.get("types") {
            for (k, v) in m {
                match serde_json::from_value::<VarType>(v.clone()) {
                    Ok(t) => {
                        p.types.insert(k.clone(), t);
                    }
                    Err(_) => notes.push(format!("Type du champ « {k} » illisible : ignoré")),
                }
            }
        } else if objet.contains_key("types") {
            notes.push("Types illisibles : ignorés".into());
        }
        if let Some(n) = objet.get("objectif").and_then(Value::as_u64) {
            p.objectif = u32::try_from(n).ok();
        }
        if let Some(v) = objet.get("page") {
            match serde_json::from_value::<PageSetup>(v.clone()) {
                Ok(pg) => p.page = pg,
                Err(_) => notes.push("Mise en page illisible : réglages par défaut".into()),
            }
        }
        Some((p, notes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anciens_formats_toujours_lisibles() {
        // Version 1 : ni mise en page ni objectif.
        let v1 = r#"{"format":"varium","version":1,"title":"Lettre","source":"Bonjour {{nom}}","values":{"nom":"Marc"},"types":{"nom":"text"}}"#;
        let p = Project::from_json(v1).unwrap();
        assert_eq!(p.source, "Bonjour {{nom}}");
        assert_eq!(p.page, PageSetup::default());
        assert_eq!(p.objectif, None);
        // Version 2 : mise en page, pas d'objectif.
        let v2 = r#"{"format":"varium","version":2,"title":"T","source":"x","page":{"paper":"a5","landscape":true}}"#;
        let p = Project::from_json(v2).unwrap();
        assert!(p.page.landscape);
        assert_eq!(p.objectif, None);
        // Version actuelle : aller-retour, objectif compris.
        let mut p = Project::new("T", "Un texte", &Values::default());
        p.objectif = Some(500);
        let json = p.to_json();
        assert!(json.contains("\"version\": 3"));
        assert_eq!(Project::from_json(&json).unwrap(), p);
        // Sans objectif, le champ n'est pas écrit.
        assert!(!Project::default().to_json().contains("objectif"));
        // Plus récent : refusé proprement, mais récupérable.
        let futur = r#"{"format":"varium","version":99,"source":"x","objectif":42}"#;
        assert!(Project::from_json(futur).is_err());
        let (r, notes) = Project::recuperer(futur).unwrap();
        assert_eq!(r.objectif, Some(42));
        assert_eq!(notes.len(), 1);
    }
}
