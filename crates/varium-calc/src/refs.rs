//! Références de cellules (`B12`, `$B$12`) et plages (`A1:C3`).

use serde::{Deserialize, Serialize};

/// Nombre de lignes d'une feuille (comme Excel et LibreOffice récents).
pub const MAX_ROWS: u32 = 1_048_576;
/// Nombre de colonnes d'une feuille (`A` à `XFD`).
pub const MAX_COLS: u32 = 16_384;

/// Position d'une cellule, à partir de 0 (`A1` = ligne 0, colonne 0).
///
/// L'ordre suit les lignes puis les colonnes (ordre de lecture).
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct CellRef {
    pub row: u32,
    pub col: u32,
}

impl CellRef {
    pub fn new(row: u32, col: u32) -> Self {
        Self { row, col }
    }

    /// Lit `B12`, `$B$12`, `b12`… (espaces autour ignorés).
    pub fn parse(s: &str) -> Option<Self> {
        parse_addr(s.trim()).map(|a| a.cell())
    }

    /// Nom de la cellule : `B12`.
    pub fn name(&self) -> String {
        format!("{}{}", col_name(self.col), u64::from(self.row) + 1)
    }

    /// Cellule décalée, ou `None` hors de la feuille.
    pub fn offset(&self, drow: i64, dcol: i64) -> Option<Self> {
        let row = i64::from(self.row) + drow;
        let col = i64::from(self.col) + dcol;
        if (0..i64::from(MAX_ROWS)).contains(&row) && (0..i64::from(MAX_COLS)).contains(&col) {
            Some(Self::new(row as u32, col as u32))
        } else {
            None
        }
    }
}

impl std::fmt::Display for CellRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name())
    }
}

/// Nom d'une colonne : 0 → `A`, 25 → `Z`, 26 → `AA`.
pub fn col_name(col: u32) -> String {
    let mut n = u64::from(col) + 1;
    let mut out = Vec::new();
    while n > 0 {
        n -= 1;
        out.push(b'A' + (n % 26) as u8);
        n /= 26;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// Index d'une colonne d'après ses lettres (`AA` → 26), dans les limites de la feuille.
pub fn col_index(letters: &str) -> Option<u32> {
    if letters.is_empty() || letters.len() > 3 {
        return None;
    }
    let mut n: u32 = 0;
    for b in letters.bytes() {
        if !b.is_ascii_alphabetic() {
            return None;
        }
        n = n * 26 + u32::from(b.to_ascii_uppercase() - b'A') + 1;
    }
    let col = n - 1;
    (col < MAX_COLS).then_some(col)
}

/// Adresse lue dans une formule : position et marqueurs `$`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Addr {
    pub row: u32,
    pub col: u32,
    pub abs_row: bool,
    pub abs_col: bool,
}

impl Addr {
    pub fn cell(&self) -> CellRef {
        CellRef::new(self.row, self.col)
    }

    /// Texte de l'adresse avec ses `$`.
    pub fn text(&self) -> String {
        format!(
            "{}{}{}{}",
            if self.abs_col { "$" } else { "" },
            col_name(self.col),
            if self.abs_row { "$" } else { "" },
            u64::from(self.row) + 1
        )
    }
}

/// Lit une adresse complète (`$A$1`, `b7`…), sans espaces.
pub(crate) fn parse_addr(s: &str) -> Option<Addr> {
    let b = s.as_bytes();
    let mut i = 0;
    let abs_col = b.first() == Some(&b'$');
    if abs_col {
        i += 1;
    }
    let ls = i;
    while i < b.len() && b[i].is_ascii_alphabetic() {
        i += 1;
    }
    let col = col_index(s.get(ls..i)?)?;
    let abs_row = b.get(i) == Some(&b'$');
    if abs_row {
        i += 1;
    }
    let ds = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i != b.len() || i == ds || i - ds > 7 {
        return None;
    }
    let row: u32 = s.get(ds..i)?.parse().ok()?;
    if row == 0 || row > MAX_ROWS {
        return None;
    }
    Some(Addr {
        row: row - 1,
        col,
        abs_row,
        abs_col,
    })
}

/// Plage rectangulaire de cellules, bornes incluses, toujours normalisée
/// (`start` en haut à gauche, `end` en bas à droite).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Range {
    pub start: CellRef,
    pub end: CellRef,
}

impl Range {
    /// Plage entre deux coins quelconques (normalisée).
    pub fn new(a: CellRef, b: CellRef) -> Self {
        Self {
            start: CellRef::new(a.row.min(b.row), a.col.min(b.col)),
            end: CellRef::new(a.row.max(b.row), a.col.max(b.col)),
        }
    }

    /// Plage d'une seule cellule.
    pub fn cell(c: CellRef) -> Self {
        Self { start: c, end: c }
    }

    /// Lit `A1:B3` (ou `B3:A1`, ou une cellule seule `A1`).
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        match s.split_once(':') {
            Some((a, b)) => Some(Self::new(CellRef::parse(a)?, CellRef::parse(b)?)),
            None => CellRef::parse(s).map(Self::cell),
        }
    }

    /// Nom de la plage : `A1:B3`, ou `A1` pour une seule cellule.
    pub fn name(&self) -> String {
        if self.start == self.end {
            self.start.name()
        } else {
            format!("{}:{}", self.start.name(), self.end.name())
        }
    }

    /// Nombre de lignes.
    pub fn rows(&self) -> u32 {
        self.end.row - self.start.row + 1
    }

    /// Nombre de colonnes.
    pub fn cols(&self) -> u32 {
        self.end.col - self.start.col + 1
    }

    /// Nombre de cellules.
    pub fn area(&self) -> u64 {
        u64::from(self.rows()) * u64::from(self.cols())
    }

    pub fn contains(&self, c: CellRef) -> bool {
        (self.start.row..=self.end.row).contains(&c.row)
            && (self.start.col..=self.end.col).contains(&c.col)
    }

    /// Les cellules de la plage, ligne par ligne.
    pub fn iter(&self) -> impl Iterator<Item = CellRef> + use<> {
        let r = *self;
        (r.start.row..=r.end.row)
            .flat_map(move |row| (r.start.col..=r.end.col).map(move |col| CellRef::new(row, col)))
    }
}

impl std::fmt::Display for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name())
    }
}

/// Le nom d'une feuille peut-il s'écrire sans apostrophes dans une formule ?
fn sheet_name_is_plain(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_alphabetic() || first == '_')
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
        && parse_addr(name).is_none()
        && !matches!(
            name.to_uppercase().as_str(),
            "VRAI" | "FAUX" | "TRUE" | "FALSE"
        )
}

/// Nom de feuille tel qu'il s'écrit dans une formule : `Feuille2` ou `'Feuille 2'`.
pub fn quote_sheet_name(name: &str) -> String {
    if sheet_name_is_plain(name) {
        name.to_string()
    } else {
        always_quote_sheet_name(name)
    }
}

/// Nom de feuille toujours entre apostrophes (exports).
pub(crate) fn always_quote_sheet_name(name: &str) -> String {
    format!("'{}'", name.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_de_colonnes() {
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(25), "Z");
        assert_eq!(col_name(26), "AA");
        assert_eq!(col_name(701), "ZZ");
        assert_eq!(col_name(702), "AAA");
        assert_eq!(col_name(MAX_COLS - 1), "XFD");
        assert_eq!(col_index("XFD"), Some(MAX_COLS - 1));
        assert_eq!(col_index("XFE"), None);
        for c in [0, 1, 25, 26, 27, 700, 701, 702, 16_383] {
            assert_eq!(col_index(&col_name(c)), Some(c));
        }
    }

    #[test]
    fn lecture_des_references() {
        assert_eq!(CellRef::parse("B12"), Some(CellRef::new(11, 1)));
        assert_eq!(CellRef::parse("$b$12"), Some(CellRef::new(11, 1)));
        assert_eq!(CellRef::parse(" a1 "), Some(CellRef::new(0, 0)));
        assert_eq!(CellRef::parse("A0"), None);
        assert_eq!(CellRef::parse("A"), None);
        assert_eq!(CellRef::parse("12"), None);
        assert_eq!(CellRef::parse("A1048577"), None);
        assert_eq!(CellRef::parse("A1048576"), Some(CellRef::new(MAX_ROWS - 1, 0)));
        assert_eq!(CellRef::parse("AAAA1"), None);
        assert_eq!(CellRef::parse("A1B"), None);
        assert_eq!(CellRef::parse("é1"), None);
        assert_eq!(CellRef::new(11, 1).name(), "B12");
    }

    #[test]
    fn plages() {
        let r = Range::parse("C3:A1").unwrap();
        assert_eq!(r.start, CellRef::new(0, 0));
        assert_eq!(r.end, CellRef::new(2, 2));
        assert_eq!(r.name(), "A1:C3");
        assert_eq!((r.rows(), r.cols()), (3, 3));
        assert_eq!(r.iter().count(), 9);
        assert_eq!(r.iter().nth(1), Some(CellRef::new(0, 1)));
        assert_eq!(Range::parse("B2").unwrap().name(), "B2");
        assert!(r.contains(CellRef::new(1, 1)));
        assert!(!r.contains(CellRef::new(3, 1)));
    }

    #[test]
    fn noms_de_feuilles() {
        assert_eq!(quote_sheet_name("Feuille2"), "Feuille2");
        assert_eq!(quote_sheet_name("Paramètres"), "Paramètres");
        assert_eq!(quote_sheet_name("Feuille 2"), "'Feuille 2'");
        assert_eq!(quote_sheet_name("AB12"), "'AB12'");
        assert_eq!(quote_sheet_name("L'été"), "'L''été'");
        assert_eq!(quote_sheet_name("2026"), "'2026'");
    }
}
