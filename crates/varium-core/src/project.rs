//! Fichier projet `.varium.json` : source + valeurs + types.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::render::{Values, VarType};

pub const FORMAT: &str = "varium";
pub const VERSION: u32 = 1;

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
}
