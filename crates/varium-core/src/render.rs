//! Rendu : substitution des valeurs et repérage des formules LaTeX.

use std::collections::BTreeMap;

use crate::filters;
use crate::parse::{self, Filter, Segment, VarRef};

/// Type d'une variable : choisit le champ de saisie et le format par défaut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum VarType {
    #[default]
    Text,
    Long,
    Date,
    Number,
    Money,
    Email,
}

impl VarType {
    pub const ALL: [VarType; 6] = [
        VarType::Text,
        VarType::Long,
        VarType::Date,
        VarType::Number,
        VarType::Money,
        VarType::Email,
    ];

    pub fn label(self) -> &'static str {
        match self {
            VarType::Text => "Texte",
            VarType::Long => "Texte long",
            VarType::Date => "Date",
            VarType::Number => "Nombre",
            VarType::Money => "Montant",
            VarType::Email => "E-mail",
        }
    }

    /// Filtre appliqué quand l'occurrence n'en précise aucun.
    fn implicit_filter(self) -> Option<Filter> {
        let name = match self {
            VarType::Date => "date",
            VarType::Number => "number",
            VarType::Money => "money",
            _ => return None,
        };
        Some(Filter {
            name: name.into(),
            arg: None,
        })
    }

    /// Vérifie une saisie ; renvoie un message d'erreur si elle est invalide.
    pub fn validate(self, value: &str) -> Option<&'static str> {
        if value.trim().is_empty() {
            return None;
        }
        match self {
            VarType::Date if filters::parse_date(value).is_none() => {
                Some("Date attendue : AAAA-MM-JJ")
            }
            VarType::Number | VarType::Money if filters::parse_number(value).is_none() => {
                Some("Nombre attendu")
            }
            VarType::Email if !is_email(value) => Some("Adresse e-mail invalide"),
            _ => None,
        }
    }
}

fn is_email(v: &str) -> bool {
    let v = v.trim();
    match v.split_once('@') {
        Some((user, domain)) => {
            !user.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !v.contains(char::is_whitespace)
        }
        None => false,
    }
}

/// Valeurs et types des variables.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Values {
    pub values: BTreeMap<String, String>,
    pub types: BTreeMap<String, VarType>,
}

impl Values {
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.values.insert(name.into(), value.into());
        self
    }

    pub fn get(&self, name: &str) -> &str {
        self.values.get(name).map(String::as_str).unwrap_or("")
    }

    pub fn ty(&self, name: &str) -> VarType {
        self.types.get(name).copied().unwrap_or_default()
    }

    /// Valeur finale d'une occurrence, ou `None` si elle n'est pas renseignée.
    pub fn resolve(&self, v: &VarRef) -> Option<String> {
        let raw = self.get(&v.name);
        let has_default = v.filters.iter().any(|f| f.name == "default");
        if raw.is_empty() && !has_default {
            return None;
        }
        if v.filters.is_empty()
            && let Some(f) = self.ty(&v.name).implicit_filter() {
                return Some(filters::apply(raw, &[f]));
            }
        Some(filters::apply(raw, &v.filters))
    }
}

/// Que faire d'une variable sans valeur au rendu texte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Missing {
    /// Laisse `{{nom}}` tel quel.
    #[default]
    Keep,
    /// N'écrit rien.
    Empty,
}

/// Morceau de document rendu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// Texte ordinaire de la source.
    Text(String),
    /// Valeur substituée à une variable.
    Value { name: String, text: String },
    /// Variable sans valeur.
    Missing { name: String, raw: String },
}

impl Piece {
    pub fn text(&self) -> &str {
        match self {
            Piece::Text(t) | Piece::Value { text: t, .. } | Piece::Missing { raw: t, .. } => t,
        }
    }
}

pub fn render_pieces(src: &str, values: &Values) -> Vec<Piece> {
    parse::parse(src)
        .into_iter()
        .map(|s| match s {
            Segment::Text(t) => Piece::Text(t),
            Segment::Var(v) => match values.resolve(&v) {
                Some(text) => Piece::Value { name: v.name, text },
                None => Piece::Missing {
                    name: v.name,
                    raw: v.raw,
                },
            },
        })
        .collect()
}

/// Document final en texte.
pub fn render(src: &str, values: &Values, missing: Missing) -> String {
    render_pieces(src, values)
        .iter()
        .map(|p| match (p, missing) {
            (Piece::Missing { .. }, Missing::Empty) => "",
            _ => p.text(),
        })
        .collect()
}

/// Remplace chaque occurrence d'une variable par sa valeur figée dans la source.
pub fn inline_variable(src: &str, name: &str, values: &Values) -> String {
    parse::rewrite(src, |v| {
        if v.name != name {
            return None;
        }
        values.resolve(v).map(|t| parse::escape_text(&t))
    })
}

/// Bloc de document : texte courant ou formule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Prose(Vec<Piece>),
    /// Formule LaTeX ; `display` pour `$$…$$`, sinon `$…$` en ligne.
    Math {
        display: bool,
        pieces: Vec<Piece>,
    },
}

/// Repère les formules `$…$` et `$$…$$`. Seuls les `$` du texte source
/// comptent comme délimiteurs : un `$` venant d'une valeur (ex. « 10 $US »)
/// ne déclenche jamais une formule. `\$` donne un dollar littéral.
pub fn blocks(pieces: Vec<Piece>) -> Vec<Block> {
    let mut out = Vec::new();
    let mut cur: Vec<Piece> = Vec::new();
    let mut math: Option<bool> = None; // Some(display) quand on est dans une formule

    fn push_text(cur: &mut Vec<Piece>, s: &str) {
        if s.is_empty() {
            return;
        }
        if let Some(Piece::Text(t)) = cur.last_mut() {
            t.push_str(s);
        } else {
            cur.push(Piece::Text(s.to_string()));
        }
    }

    for piece in pieces {
        let Piece::Text(text) = piece else {
            cur.push(piece);
            continue;
        };
        let mut rest = text.as_str();
        while !rest.is_empty() {
            let Some(pos) = rest.find(['$', '\\']) else {
                push_text(&mut cur, rest);
                break;
            };
            push_text(&mut cur, &rest[..pos]);
            let tail = &rest[pos..];
            if let Some(after) = tail.strip_prefix("\\$") {
                push_text(&mut cur, if math.is_some() { "\\$" } else { "$" });
                rest = after;
            } else if let Some(after) = tail.strip_prefix('\\') {
                push_text(&mut cur, "\\");
                rest = after;
            } else {
                let double = tail.starts_with("$$");
                rest = &tail[if double { 2 } else { 1 }..];
                match math {
                    None => {
                        out.push(Block::Prose(std::mem::take(&mut cur)));
                        math = Some(double);
                    }
                    Some(d) if d == double => {
                        out.push(Block::Math {
                            display: d,
                            pieces: std::mem::take(&mut cur),
                        });
                        math = None;
                    }
                    Some(_) => push_text(&mut cur, if double { "$$" } else { "$" }),
                }
            }
        }
    }
    match math {
        // Formule non fermée : on la rend telle quelle.
        Some(d) => {
            let mut p = vec![Piece::Text(if d { "$$".into() } else { "$".into() })];
            p.append(&mut cur);
            out.push(Block::Prose(p));
        }
        None => out.push(Block::Prose(cur)),
    }
    out.retain(|b| !matches!(b, Block::Prose(p) if p.is_empty()));
    out
}
