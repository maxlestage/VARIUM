//! Import et export CSV.

use crate::format::format_value;
use crate::refs::{CellRef, MAX_COLS, MAX_ROWS};
use crate::workbook::Workbook;

fn quote(field: &str, sep: char) -> String {
    let needs = field.contains([sep, '"', '\n', '\r'])
        || field.starts_with(char::is_whitespace)
        || field.ends_with(char::is_whitespace);
    if needs {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Feuille en CSV : séparateur `;`, valeurs telles qu'affichées (virgule
/// décimale), une ligne par ligne de la feuille jusqu'à la dernière occupée.
pub(crate) fn to_csv(wb: &Workbook, sheet: usize) -> String {
    let (rows, cols) = wb.used_size(sheet);
    let computed = wb.snapshot();
    let (Some(values), Some(sh)) = (computed.values.get(sheet), wb.sheets.get(sheet)) else {
        return String::new();
    };
    // Lignes de même largeur, sauf feuille démesurée (une cellule isolée tout au
    // bout de la feuille) : on s'arrête alors au dernier champ non vide.
    let rectangular = u64::from(rows) * u64::from(cols) <= 20_000_000;
    let mut out = String::new();
    for r in 0..rows {
        let mut fields: Vec<(u32, String)> = Vec::new();
        for (&at, cell) in sh
            .cells
            .range(CellRef::new(r, 0)..=CellRef::new(r, u32::MAX))
        {
            if let Some(v) = values.get(&at) {
                fields.push((at.col, quote(&format_value(v, cell.format.number), ';')));
            }
        }
        let width = if rectangular {
            cols
        } else {
            fields.last().map(|(c, _)| c + 1).unwrap_or(0)
        };
        // Le champ d'index k est précédé d'un « ; » (sauf le premier).
        let mut next = 0;
        for (c, text) in fields {
            while next < c {
                if next > 0 {
                    out.push(';');
                }
                next += 1;
            }
            if c > 0 {
                out.push(';');
            }
            out.push_str(&text);
            next = c + 1;
        }
        while next < width {
            if next > 0 {
                out.push(';');
            }
            next += 1;
        }
        out.push('\n');
    }
    out
}

/// Séparateur le plus probable d'après la première ligne non vide : `;` s'il
/// y en a (habituel en français, où la virgule est décimale), sinon tabulation,
/// sinon virgule.
fn detect_separator(text: &str) -> char {
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut in_quotes = false;
    let (mut semi, mut tab) = (0, 0);
    for c in first.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            ';' if !in_quotes => semi += 1,
            '\t' if !in_quotes => tab += 1,
            _ => {}
        }
    }
    if semi > 0 {
        ';'
    } else if tab > 0 {
        '\t'
    } else {
        ','
    }
}

/// Découpe un texte CSV en lignes de champs (les lignes vides sont gardées
/// pour respecter la position des données).
pub(crate) fn records(text: &str, sep: char) -> Result<Vec<Vec<String>>, String> {
    let mut out: Vec<Vec<String>> = Vec::new();
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
            out.push(std::mem::take(&mut row));
        } else {
            field.push(c);
        }
    }
    if in_quotes {
        return Err("Guillemet non fermé dans le fichier CSV".into());
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        out.push(row);
    }
    Ok(out)
}

/// Classeur d'une feuille à partir d'un CSV. Les nombres (`1 234,5`, `12.5`,
/// `12 %`, `15,00 €`), dates (`15/03/2026`) et valeurs logiques sont reconnus ;
/// un champ commençant par `=` reste du texte (pas de formule cachée).
pub(crate) fn from_csv(title: &str, text: &str) -> Result<Workbook, String> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let sep = detect_separator(text);
    let recs = records(text, sep)?;
    let mut wb = Workbook::new();
    let title = title.trim();
    if !title.is_empty() {
        wb.title = title.to_string();
    }
    for (r, rec) in recs.iter().enumerate().take(MAX_ROWS as usize) {
        for (c, field) in rec.iter().enumerate().take(MAX_COLS as usize) {
            if field.is_empty() {
                continue;
            }
            let at = CellRef::new(r as u32, c as u32);
            if field.starts_with('=') || field.starts_with('\'') {
                wb.set_input(0, at, &format!("'{field}"));
            } else {
                wb.set_input(0, at, field);
            }
        }
    }
    Ok(wb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separateurs() {
        assert_eq!(detect_separator("a;b,c\n"), ';');
        assert_eq!(detect_separator("a\tb,c\n"), '\t');
        assert_eq!(detect_separator("a,b\n"), ',');
        assert_eq!(detect_separator("\"a;b\",c\n"), ',');
    }

    #[test]
    fn champs_entre_guillemets() {
        let r = records("a;\"b;c\";\"d\"\"e\"\n\nf\r\n", ';').unwrap();
        assert_eq!(r[0], vec!["a", "b;c", "d\"e"]);
        assert_eq!(r[1], vec![""]);
        assert_eq!(r[2], vec!["f"]);
        assert!(records("\"abc", ';').is_err());
    }
}
