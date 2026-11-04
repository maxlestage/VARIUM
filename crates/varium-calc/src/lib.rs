//! Moteur de classeur de Varium : cellules, formules, fonctions, exports
//! XLSX, ODS, CSV et JSON.
//!
//! # Saisie
//!
//! Une saisie qui commence par `=` est une formule. Sinon, les nombres sont
//! reconnus à la française ou à l'anglaise (`12`, `1 234,5`, `12.5`), comme les
//! pourcentages (`12,5 %` → 0,125, la cellule prend le format pourcentage si
//! elle n'en avait pas), les montants (`12,50 €`), les dates (`15/03/2026`,
//! `2026-03-15`) et les valeurs logiques (`VRAI`, `FAUX`). Tout le reste est du
//! texte ; une apostrophe en tête (`'12`) force le texte.
//!
//! # Formules
//!
//! Le langage est celui de LibreOffice Calc en français :
//!
//! - nombres avec `,` ou `.` comme séparateur décimal ;
//! - **arguments séparés uniquement par `;`** (la virgule est décimale :
//!   `=SOMME(1,5;2)` vaut 3,5) ;
//! - opérateurs `+ - * / ^`, `-` unaire, `%` postfixé, `&` (concaténation),
//!   comparaisons `= <> < > <= >=`, parenthèses ;
//! - textes entre guillemets (`""` pour un guillemet) ;
//! - références `A1`, `$A$1`, `A$1`, plages `A1:B5`, autres feuilles
//!   `Feuille2!A1`, `'Feuille 2'!A1:B2` (ou `Feuille2.A1` à la LibreOffice) ;
//! - fonctions en français ou en anglais, sans tenir compte de la casse ni des
//!   accents (voir [`functions`]).
//!
//! Le calcul suit les dépendances (une formule est évaluée après les cellules
//! qu'elle utilise) et repère les références circulaires (`#CIRC!`). Les
//! erreurs se propagent : `#DIV/0!`, `#VALEUR!`, `#REF!`, `#NOM?`, `#N/A`,
//! `#NOMBRE!`. Les formules sont limitées à 8 192 caractères et 64 niveaux
//! d'imbrication : aucune saisie ne peut faire planter le moteur.
//!
//! # Mise en forme conditionnelle, volets figés, imports
//!
//! Chaque feuille porte ses règles de mise en forme conditionnelle
//! ([`CondFormat`] : comparaisons, texte, vide, erreur, N premiers, moyenne,
//! échelles de couleurs, barres de données ; voir [`CondFormatter`] pour
//! l'ordre d'application) et ses volets figés (`frozen_rows`,
//! `frozen_cols`). Les deux sont enregistrés en JSON et exportés en XLSX et
//! ODS. [`Workbook::from_xlsx`] et [`Workbook::from_ods`] relisent ces
//! formats (valeurs, formules traduites, formats, largeurs, règles, volets) ;
//! un fichier abîmé donne une erreur en français.
//!
//! Les dates sont des numéros de série comptés depuis le 30/12/1899, comme
//! dans les tableurs.

mod analysis;
mod cond;
mod csv;
mod eval;
mod explain;
mod filter;
mod flash;
mod format;
mod functions;
mod import;
mod lexer;
mod ods;
mod ods_read;
mod parser;
mod paste;
mod question;
mod refs;
mod rewrite;
mod sample;
mod value;
mod workbook;
mod xlsx;
mod xlsx_read;
mod xml;
mod xmltree;

pub use analysis::{DataShape, QuickFormat, QuickTotal};
pub use cond::{CompareOp, CondFormat, CondFormatter, CondPaint, CondRule, CondStyle};
pub use explain::{BinOp, ErrorCause, FormulaNode};
pub use filter::{AutoFilter, ColumnFilter, FilterCondition};
pub use flash::{FlashProgram, autocomplete, flash_fill};
pub use format::{
    CellFormat, HAlign, NBSP, NumberFormat, THOUSANDS_SEP, date_serial, format_general,
    format_number, format_value, parse_number, serial_to_ymd, today_serial,
};
pub use functions::{FunctionCategory, FunctionInfo, function_info, functions};
pub use parser::check_formula;
pub use paste::{PasteOptions, PasteSource, PasteWhat, parse_table, smart_input};
pub use question::{Answer, MAX_GROUPS, Operation, QuestionError};
pub use refs::{CellRef, MAX_COLS, MAX_ROWS, Range, col_index, col_name, quote_sheet_name};
pub use rewrite::{shift_formula, translate_to_english};
pub use value::{CalcError, Value};
pub use workbook::{
    Cell, Clip, ClipCell, DEFAULT_COL_WIDTH, DEFAULT_ROW_HEIGHT, MAX_SHEET_NAME, Sheet, Workbook,
};
