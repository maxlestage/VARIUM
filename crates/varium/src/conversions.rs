//! Conversions en un clic entre les modules (logique pure) : un tableau du classeur
//! en tableau Varium `| a | b |`, deux colonnes en valeurs de champs. La conversion
//! d'un document en présentation est dans `varium_slides::deck_from_document`.

/// Une cellule dans un tableau Varium : les `|` sont protégés, les retours à la
/// ligne deviennent des espaces.
fn cellule(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('|', "\\|")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Ressemble à un nombre affiché (`1 234,50 €`, `12 %`, `-3.5`) ?
fn numerique(s: &str) -> bool {
    let t: String = s
        .chars()
        .filter(|c| !matches!(c, ' ' | '\u{a0}' | '\u{202f}' | '€' | '$' | '%' | '£'))
        .collect();
    !t.is_empty()
        && t.chars().any(|c| c.is_ascii_digit())
        && t.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, ',' | '.' | '-' | '+' | 'E' | 'e'))
}

/// Les lignes sans lignes ni colonnes vides au bord.
fn rogner(rows: &[Vec<String>]) -> Vec<Vec<String>> {
    let vide = |r: &Vec<String>| r.iter().all(|c| c.trim().is_empty());
    let debut = rows.iter().position(|r| !vide(r)).unwrap_or(rows.len());
    let fin = rows.iter().rposition(|r| !vide(r)).map_or(debut, |i| i + 1);
    let rows = &rows[debut..fin];
    let cols = rows
        .iter()
        .map(|r| {
            r.iter()
                .rposition(|c| !c.trim().is_empty())
                .map_or(0, |i| i + 1)
        })
        .max()
        .unwrap_or(0);
    let gauche = (0..cols)
        .find(|&c| {
            rows.iter()
                .any(|r| r.get(c).is_some_and(|x| !x.trim().is_empty()))
        })
        .unwrap_or(0);
    rows.iter()
        .map(|r| {
            (gauche..cols)
                .map(|c| r.get(c).cloned().unwrap_or_default())
                .collect()
        })
        .collect()
}

/// Un tableau Varium : la première ligne sert d'en-tête, les colonnes de nombres
/// sont alignées à droite. Vide si la plage l'est.
pub fn markup_table(rows: &[Vec<String>]) -> String {
    let rows = rogner(rows);
    let Some(cols) = rows.iter().map(Vec::len).max().filter(|&c| c > 0) else {
        return String::new();
    };
    let ligne = |r: &Vec<String>| {
        let cells: Vec<String> = (0..cols)
            .map(|c| cellule(r.get(c).map_or("", String::as_str)))
            .collect();
        format!("| {} |", cells.join(" | "))
    };
    let mut out = vec![ligne(&rows[0])];
    let sep: Vec<&str> = (0..cols)
        .map(|c| {
            let corps: Vec<&str> = rows[1..]
                .iter()
                .filter_map(|r| r.get(c).map(String::as_str))
                .filter(|s| !s.trim().is_empty())
                .collect();
            if !corps.is_empty() && corps.iter().all(|s| numerique(s)) {
                "---:"
            } else {
                "---"
            }
        })
        .collect();
    out.push(format!("|{}|", sep.join("|")));
    out.extend(rows[1..].iter().map(ligne));
    out.join("\n")
}

/// Deux colonnes (noms, valeurs) → paires de champs. Les noms sont normalisés comme
/// les en-têtes du publipostage (« Nom du client » → `nom_du_client`) ; les lignes
/// sans nom sont ignorées.
pub fn field_pairs(rows: &[Vec<String>]) -> Result<Vec<(String, String)>, String> {
    let rows = rogner(rows);
    if rows.iter().map(Vec::len).max().unwrap_or(0) < 2 {
        return Err(crate::tr::tr(
            "Sélectionnez deux colonnes : les noms des champs, puis leurs valeurs",
        )
        .to_string());
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for r in &rows {
        let nom = varium_core::merge::normalize(r.first().map_or("", String::as_str));
        let nom = nom.trim_matches('_').to_string();
        if nom.is_empty() || !varium_core::is_valid_name(&nom) {
            continue;
        }
        let valeur = r.get(1).cloned().unwrap_or_default().trim().to_string();
        out.retain(|(k, _)| *k != nom);
        out.push((nom, valeur));
    }
    if out.is_empty() {
        return Err(
            crate::tr::tr("Aucun nom de champ valable dans la première colonne").to_string(),
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(rows: &[&[&str]]) -> Vec<Vec<String>> {
        rows.iter()
            .map(|r| r.iter().map(|s| s.to_string()).collect())
            .collect()
    }

    #[test]
    fn tableau_avec_en_tete_et_nombres_a_droite() {
        let rows = t(&[
            &["Produit", "Prix", ""],
            &["Pomme", "1,50 €", ""],
            &["Poire | bio", "2,00 €", ""],
            &["", "", ""],
        ]);
        assert_eq!(
            markup_table(&rows),
            "| Produit | Prix |\n|---|---:|\n| Pomme | 1,50 € |\n| Poire \\| bio | 2,00 € |"
        );
        assert_eq!(markup_table(&t(&[&["", ""]])), "");
        assert_eq!(markup_table(&[]), "");
        // Lignes et colonnes vides au bord retirées.
        assert_eq!(
            markup_table(&t(&[&["", "", ""], &["", "a", "12 %"]])),
            "| a | 12 % |\n|---|---|"
        );
        // Un tableau Varium relu par le moteur.
        let src = markup_table(&rows);
        let doc = varium_core::document(&src, &varium_core::Values::default());
        assert!(matches!(doc[0], varium_core::Para::Table(_)));
    }

    #[test]
    fn deux_colonnes_en_champs() {
        let rows = t(&[
            &["Nom du client", " Dupont "],
            &["", "ignoré"],
            &["montant", "1 200"],
            &["montant", "1 300"],
        ]);
        let p = field_pairs(&rows).unwrap();
        assert_eq!(
            p,
            [
                ("nom_du_client".to_string(), "Dupont".to_string()),
                ("montant".to_string(), "1 300".to_string())
            ]
        );
        assert!(field_pairs(&t(&[&["a"], &["b"]])).is_err());
        assert!(field_pairs(&t(&[&["", "x"]])).is_err());
    }
}
