//! # varium-core
//!
//! Moteur de documents à variables de **Varium** — « Un document. Toutes ses possibilités. »
//!
//! ```
//! use varium_core::{Values, Missing, render};
//!
//! let src = "Bonjour {{prénom}} {{nom}}, dossier {{réf}}. — {{prénom}}";
//! let mut v = Values::default();
//! v.set("prénom", "Marc");
//! // Seules les occurrences de {{prénom}} changent ; {{nom}} et {{réf}} restent intacts.
//! assert_eq!(render(src, &v, Missing::Keep), "Bonjour Marc {{nom}}, dossier {{réf}}. — Marc");
//! ```

pub mod autocorrect;
pub mod calc;
pub mod check;
pub mod collage;
pub mod conditions;
pub mod critic;
pub mod detection;
pub mod diff;
pub mod doc;
pub mod docx;
pub mod epub;
pub mod export;
pub mod filters;
pub mod formulaire;
pub mod formule;
pub mod import;
mod import_docx;
mod import_odt;
pub mod lang;
#[cfg(feature = "serde")]
pub mod lettres;
pub mod lisibilite;
pub mod merge;
pub mod mise_en_page;
#[cfg(feature = "serde")]
pub mod modele;
pub mod odt;
pub mod page;
pub mod parse;
pub mod pdf;
pub mod police;
#[cfg(feature = "serde")]
pub mod project;
pub mod qr;
pub mod render;
pub mod rtf;
pub mod signature;
pub mod spell;
pub mod tex;
pub mod typo;
#[cfg(feature = "serde")]
pub mod versions;
mod xmlarbre;
pub mod zip;

pub use parse::{
    Filter, Segment, VarInfo, VarRef, formulas, is_valid_name, parse, rename_variable, variables,
};
#[cfg(feature = "serde")]
pub use project::Project;
pub use render::{
    Block, Missing, Piece, Values, VarType, blocks, inline_variable, render, render_pieces,
};

pub use doc::{Align, Para, Run, Stats, Style, Table, document, stats};
pub use lang::Lang;
pub use page::{DocFont, PageSetup, Paper};
