//! Publipostage : un tableau CSV (une ligne par destinataire) → un document par ligne.
//!
//! La première ligne du CSV donne les noms des colonnes. Une colonne remplit la
//! variable de même nom ; la casse, les espaces, les tirets et les accents des
//! en-têtes ne comptent pas (`Prénom`, `prenom` et `PRÉNOM` remplissent `{{prénom}}`).
//! Le séparateur (`;`, `,` ou tabulation) est détecté, comme les exports d'Excel
//! ou de LibreOffice Calc.

use crate::render::{Values, VarType};

/// Un tableau de données.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sheet {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

fn detect_separator(first_line: &str) -> char {
    let mut best = (',', 0);
    for sep in [';', ',', '\t'] {
        let mut in_quotes = false;
        let n = first_line
            .chars()
            .filter(|c| {
                if *c == '"' {
                    in_quotes = !in_quotes;
                }
                !in_quotes && *c == sep
            })
            .count();
        if n > best.1 {
            best = (sep, n);
        }
    }
    best.0
}

/// Lit un CSV. Les guillemets (`"a; b"`, `""` pour un guillemet) sont gérés.
pub fn parse_csv(text: &str) -> Result<Sheet, String> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let sep = detect_separator(first);

    let mut records: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' && field.trim().is_empty() {
            field.clear();
            in_quotes = true;
        } else if c == sep {
            row.push(std::mem::take(&mut field));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut field));
            if row.iter().any(|f| !f.trim().is_empty()) {
                records.push(std::mem::take(&mut row));
            } else {
                row.clear();
            }
        } else {
            field.push(c);
        }
    }
    if in_quotes {
        return Err("Guillemet non fermé dans le fichier CSV".into());
    }
    row.push(field);
    if row.iter().any(|f| !f.trim().is_empty()) {
        records.push(row);
    }

    let mut it = records.into_iter();
    let headers: Vec<String> = it
        .next()
        .ok_or("Le fichier CSV est vide")?
        .into_iter()
        .map(|h| h.trim().to_string())
        .collect();
    if headers.iter().all(String::is_empty) {
        return Err("La première ligne du CSV doit donner les noms des colonnes".into());
    }
    let width = headers.len();
    let rows = it
        .map(|mut r| {
            r.iter_mut().for_each(|f| *f = f.trim().to_string());
            r.resize(width, String::new());
            r
        })
        .collect();
    Ok(Sheet { headers, rows })
}

/// Forme comparable d'un nom : minuscules, sans accents, `_` pour les séparateurs.
pub fn normalize(name: &str) -> String {
    name.trim()
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' | 'ì' => 'i',
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            'ÿ' => 'y',
            'ñ' => 'n',
            ' ' | '-' | '.' | '\'' | '’' => '_',
            other => other,
        })
        .collect()
}

/// `15/03/2026` → `2026-03-15` (les tableurs français écrivent les dates ainsi).
fn french_date_to_iso(v: &str) -> Option<String> {
    let mut it = v.trim().split('/');
    let (d, m, y) = (it.next()?, it.next()?, it.next()?);
    if it.next().is_some() || y.len() != 4 {
        return None;
    }
    let (d, m): (u32, u32) = (d.parse().ok()?, m.parse().ok()?);
    Some(format!("{y}-{m:02}-{d:02}"))
}

impl Sheet {
    /// La colonne qui remplit une variable, si elle existe.
    pub fn column_for(&self, variable: &str) -> Option<usize> {
        let n = normalize(variable);
        self.headers.iter().position(|h| normalize(h) == n)
    }

    /// Les variables du document qu'aucune colonne ne remplit.
    pub fn unmatched<'a>(&self, variables: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        variables
            .into_iter()
            .filter(|v| self.column_for(v).is_none())
            .map(str::to_string)
            .collect()
    }

    /// Les valeurs d'une ligne : celles de `base`, remplacées par les colonnes du CSV.
    pub fn values_for(&self, row: usize, base: &Values, variables: &[String]) -> Values {
        let mut v = base.clone();
        let Some(cells) = self.rows.get(row) else {
            return v;
        };
        for name in variables {
            if let Some(col) = self.column_for(name) {
                let mut cell = cells.get(col).cloned().unwrap_or_default();
                if v.ty(name) == VarType::Date
                    && let Some(iso) = french_date_to_iso(&cell)
                {
                    cell = iso;
                }
                v.set(name.clone(), cell);
            }
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_francais() {
        let s = parse_csv("\u{FEFF}Prénom;Nom;Ville\r\nMarc;\"Du; pont\";Lyon\r\n\r\nLéa;\"Il dit \"\"oui\"\"\";\n")
            .unwrap();
        assert_eq!(s.headers, ["Prénom", "Nom", "Ville"]);
        assert_eq!(s.rows.len(), 2);
        assert_eq!(s.rows[0][1], "Du; pont");
        assert_eq!(s.rows[1][1], "Il dit \"oui\"");
        assert_eq!(s.rows[1][2], "");
    }

    #[test]
    fn csv_virgules_et_correspondance() {
        let s = parse_csv("prenom,date naissance\nAnne,15/03/1990").unwrap();
        assert_eq!(s.column_for("prénom"), Some(0));
        assert_eq!(s.column_for("date_naissance"), Some(1));
        let mut base = Values::default();
        base.types.insert("date_naissance".into(), VarType::Date);
        let vars = vec![
            "prénom".to_string(),
            "date_naissance".to_string(),
            "x".to_string(),
        ];
        let v = s.values_for(0, &base, &vars);
        assert_eq!(v.get("prénom"), "Anne");
        assert_eq!(v.get("date_naissance"), "1990-03-15");
        assert_eq!(s.unmatched(vars.iter().map(String::as_str)), ["x"]);
    }

    #[test]
    fn csv_vide() {
        assert!(parse_csv("").is_err());
        assert!(parse_csv("a;\"b").is_err());
    }
}
