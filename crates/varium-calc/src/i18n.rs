//! Textes du moteur dans les six langues de l'interface : libellés écrits
//! dans les feuilles (modèles, tableaux croisés, relevés bancaires,
//! périodes), messages d'erreur des formules et des imports.
//!
//! Le français sert de clé : [`loc`] rend la traduction, ou le français s'il
//! en manque une. [`message_in`] traduit un message complet du moteur, même
//! avec des parties variables (« Parenthèse fermante manquante pour SOMME »).

use std::collections::HashMap;
use std::sync::OnceLock;

use varium_core::Lang;

#[path = "i18n_table.rs"]
mod table;

fn index(lang: Lang) -> usize {
    match lang {
        Lang::Fr => 0,
        Lang::En => 1,
        Lang::Es => 2,
        Lang::De => 3,
        Lang::It => 4,
        Lang::Pt => 5,
    }
}

fn map() -> &'static HashMap<&'static str, &'static [&'static str; 6]> {
    static MAP: OnceLock<HashMap<&'static str, &'static [&'static str; 6]>> = OnceLock::new();
    MAP.get_or_init(|| table::TABLE.iter().map(|t| (t[0], t)).collect())
}

/// Le texte français `fr` dans la langue donnée (le français s'il manque).
pub fn loc(lang: Lang, fr: &'static str) -> &'static str {
    match map().get(fr) {
        Some(t) => t[index(lang)],
        None => fr,
    }
}

/// Nom du mois (1 à 12) dans la langue donnée.
pub fn month_name(lang: Lang, m: u32) -> &'static str {
    const MOIS: [&str; 12] = [
        "janvier",
        "février",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "août",
        "septembre",
        "octobre",
        "novembre",
        "décembre",
    ];
    loc(lang, MOIS[(m.clamp(1, 12) - 1) as usize])
}

/// Découpe `s` selon un modèle où `{}` marque une partie variable ; renvoie
/// les parties variables.
fn match_template<'a>(template: &str, s: &'a str) -> Option<Vec<&'a str>> {
    let parts: Vec<&str> = template.split("{}").collect();
    if parts.len() == 1 {
        return (template == s).then(Vec::new);
    }
    let first = parts[0];
    let last = parts[parts.len() - 1];
    let mut rest = s.strip_prefix(first)?;
    rest = rest.strip_suffix(last)?;
    let mut args = Vec::new();
    for mid in &parts[1..parts.len() - 1] {
        let i = rest.find(mid)?;
        args.push(&rest[..i]);
        rest = &rest[i + mid.len()..];
    }
    args.push(rest);
    Some(args)
}

/// Traduit un message du moteur (erreur de formule, d'import, de calcul) :
/// le message français tel que le moteur le donne, rendu dans `lang`. Les
/// parties variables sont gardées (et traduites elles-mêmes si ce sont des
/// messages connus). Un message inconnu est rendu tel quel.
pub fn message_in(lang: Lang, fr: &str) -> String {
    message_depth(lang, fr, 0)
}

fn message_depth(lang: Lang, fr: &str, depth: u32) -> String {
    if lang == Lang::Fr || depth > 4 {
        return fr.to_string();
    }
    if let Some(t) = map().get(fr) {
        return t[index(lang)].to_string();
    }
    for t in table::TEMPLATES {
        if let Some(args) = match_template(t[0], fr) {
            let mut out = String::new();
            let target = t[index(lang)];
            let mut pieces = target.split("{}");
            if let Some(p) = pieces.next() {
                out.push_str(p);
            }
            for (i, p) in pieces.enumerate() {
                if let Some(a) = args.get(i) {
                    out.push_str(&message_depth(lang, a, depth + 1));
                }
                out.push_str(p);
            }
            return out;
        }
    }
    fr.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_completes() {
        for t in table::TABLE.iter().chain(table::TEMPLATES) {
            for (i, s) in t.iter().enumerate() {
                assert!(!s.trim().is_empty(), "traduction vide : {} ({i})", t[0]);
                assert_eq!(
                    s.matches("{}").count(),
                    t[0].matches("{}").count(),
                    "parties variables : {s}"
                );
            }
        }
        let mut keys: Vec<&str> = table::TABLE.iter().map(|t| t[0]).collect();
        let n = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), n, "clé en double");
    }

    /// Chaque `loc(…, "…")` des sources a sa traduction.
    #[test]
    fn toutes_les_cles_sont_traduites() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut missing = Vec::new();
        for e in std::fs::read_dir(dir).unwrap() {
            let path = e.unwrap().path();
            if path.extension().is_none_or(|x| x != "rs") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            let code = src.split("#[cfg(test)]").next().unwrap_or_default();
            let mut from = 0;
            while let Some(i) = code[from..].find("loc(") {
                let at = from + i + 4;
                from = at;
                if code[..at - 4]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
                {
                    continue;
                }
                // Premier argument (la langue), puis un littéral.
                let rest = &code[at..];
                let Some(comma) = rest.find(',') else {
                    continue;
                };
                let lit = rest[comma + 1..].trim_start();
                if !lit.starts_with('"') {
                    continue;
                }
                let end = lit[1..].find('"').map(|j| j + 1).unwrap_or(0);
                let key = &lit[1..end];
                if !key.contains('\\') && !map().contains_key(key) {
                    missing.push(format!("{key} ({})", path.display()));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "sans traduction :\n{}",
            missing.join("\n")
        );
    }

    #[test]
    fn messages_avec_parties_variables() {
        assert_eq!(
            message_in(Lang::En, "Parenthèse fermante manquante pour SOMME"),
            "Missing closing parenthesis for SOMME"
        );
        assert_eq!(
            message_in(Lang::De, "Fichier Excel illisible : aucune feuille trouvée"),
            "Excel-Datei nicht lesbar: keine Tabelle gefunden"
        );
        assert_eq!(message_in(Lang::Fr, "Formule vide"), "Formule vide");
        assert_eq!(message_in(Lang::It, "texte inconnu"), "texte inconnu");
        assert_eq!(loc(Lang::Pt, "mars"), "março");
        assert_eq!(
            crate::CalcError::Div0.description_in(Lang::De),
            "Division durch null"
        );
        assert_eq!(month_name(Lang::Es, 13), "diciembre");
        for l in Lang::ALL {
            for s in ["", "{}", "Caractère inattendu : « ", "»", "💥"] {
                let _ = message_in(l, s);
            }
        }
    }
}
