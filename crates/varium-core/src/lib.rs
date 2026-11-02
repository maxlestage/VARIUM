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

pub mod export;
pub mod filters;
pub mod parse;
#[cfg(feature = "serde")]
pub mod project;
pub mod render;
pub mod tex;

pub use parse::{
    Filter, Segment, VarInfo, VarRef, is_valid_name, parse, rename_variable, variables,
};
#[cfg(feature = "serde")]
pub use project::Project;
pub use render::{
    Block, Missing, Piece, Values, VarType, blocks, inline_variable, render, render_pieces,
};
