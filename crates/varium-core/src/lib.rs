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

pub mod calc;
pub mod doc;
pub mod docx;
pub mod export;
pub mod filters;
pub mod merge;
pub mod odt;
pub mod page;
pub mod parse;
#[cfg(feature = "serde")]
pub mod project;
pub mod render;
pub mod tex;
mod zip;

pub use parse::{
    Filter, Segment, VarInfo, VarRef, formulas, is_valid_name, parse, rename_variable, variables,
};
#[cfg(feature = "serde")]
pub use project::Project;
pub use render::{
    Block, Missing, Piece, Values, VarType, blocks, inline_variable, render, render_pieces,
};

pub use doc::{Align, Para, Run, Stats, Style, Table, document, stats};
pub use page::{DocFont, PageSetup, Paper};
