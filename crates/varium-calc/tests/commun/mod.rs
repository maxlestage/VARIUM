//! Outils partagés des tests : lecture d'une archive ZIP non compressée et
//! vérification qu'un XML est bien formé (sans dépendance externe).

#![allow(dead_code)]

fn u16_at(d: &[u8], i: usize) -> Option<usize> {
    Some(u16::from_le_bytes(d.get(i..i + 2)?.try_into().ok()?) as usize)
}

fn u32_at(d: &[u8], i: usize) -> Option<usize> {
    Some(u32::from_le_bytes(d.get(i..i + 4)?.try_into().ok()?) as usize)
}

/// Entrées d'une archive ZIP stockée (sans compression), dans l'ordre.
pub fn zip_entries(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut i = 0;
    while u32_at(data, i) == Some(0x0403_4b50) {
        assert_eq!(u16_at(data, i + 8), Some(0), "entrée compressée inattendue");
        let size = u32_at(data, i + 18).expect("taille");
        let name_len = u16_at(data, i + 26).expect("longueur du nom");
        let extra = u16_at(data, i + 28).expect("champ extra");
        let name = String::from_utf8(data[i + 30..i + 30 + name_len].to_vec()).expect("nom UTF-8");
        let start = i + 30 + name_len + extra;
        out.push((name, data[start..start + size].to_vec()));
        i = start + size;
    }
    assert_eq!(
        u32_at(data, i),
        Some(0x0201_4b50),
        "répertoire central manquant"
    );
    let end = data.len() - 22;
    assert_eq!(
        u32_at(data, end),
        Some(0x0605_4b50),
        "fin d'archive manquante"
    );
    assert_eq!(u16_at(data, end + 10), Some(out.len()), "nombre d'entrées");
    out
}

/// Contenu texte d'une entrée.
pub fn entry(entries: &[(String, Vec<u8>)], name: &str) -> String {
    let (_, data) = entries
        .iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("entrée {name} absente"));
    String::from_utf8(data.clone()).expect("XML en UTF-8")
}

fn check_entities(s: &str) -> Result<(), String> {
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        let after = &rest[i + 1..];
        let end = after.find(';').ok_or("entité non terminée")?;
        let name = &after[..end];
        let ok = matches!(name, "amp" | "lt" | "gt" | "quot" | "apos")
            || name
                .strip_prefix("#x")
                .is_some_and(|h| !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit()))
            || name
                .strip_prefix('#')
                .is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()));
        if !ok {
            return Err(format!("entité inconnue &{name};"));
        }
        rest = &after[end + 1..];
    }
    Ok(())
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, ':' | '_' | '-' | '.')
}

/// Vérifie qu'un document XML est bien formé : balises équilibrées, attributs
/// entre guillemets et uniques, entités connues, un seul élément racine,
/// aucun caractère interdit.
pub fn check_xml(doc: &str) -> Result<(), String> {
    if let Some(c) = doc.chars().find(|&c| {
        (c < ' ' && !matches!(c, '\t' | '\n' | '\r')) || c == '\u{FFFE}' || c == '\u{FFFF}'
    }) {
        return Err(format!("caractère interdit U+{:04X}", c as u32));
    }
    let mut s = doc;
    if s.starts_with("<?xml") {
        let end = s.find("?>").ok_or("déclaration XML non fermée")?;
        s = &s[end + 2..];
    }
    let mut stack: Vec<String> = Vec::new();
    let mut roots = 0;
    loop {
        let Some(lt) = s.find('<') else {
            check_entities(s)?;
            if !s.trim().is_empty() {
                return Err("texte après la racine".into());
            }
            break;
        };
        let text = &s[..lt];
        check_entities(text)?;
        if stack.is_empty() && !text.trim().is_empty() {
            return Err(format!("texte hors de la racine : {text:?}"));
        }
        if text.contains('>') && false {
            return Err("chevron fermant isolé".into());
        }
        s = &s[lt + 1..];
        if let Some(rest) = s.strip_prefix("!--") {
            let end = rest.find("-->").ok_or("commentaire non fermé")?;
            s = &rest[end + 3..];
            continue;
        }
        // Lecture de la balise jusqu'au `>` qui n'est pas dans un attribut.
        let mut quote: Option<char> = None;
        let mut end = None;
        for (i, c) in s.char_indices() {
            match quote {
                Some(q) if c == q => quote = None,
                Some(_) if c == '<' => return Err("« < » dans un attribut".into()),
                Some(_) => {}
                None if c == '"' || c == '\'' => quote = Some(c),
                None if c == '>' => {
                    end = Some(i);
                    break;
                }
                None if c == '<' => return Err("« < » dans une balise".into()),
                None => {}
            }
        }
        let end = end.ok_or("balise non fermée")?;
        let tag = &s[..end];
        s = &s[end + 1..];
        if let Some(name) = tag.strip_prefix('/') {
            let name = name.trim();
            match stack.pop() {
                Some(open) if open == name => {}
                other => return Err(format!("</{name}> ne ferme pas {other:?}")),
            }
            continue;
        }
        let self_closing = tag.ends_with('/');
        let body = tag.strip_suffix('/').unwrap_or(tag);
        let name: String = body.chars().take_while(|&c| is_name_char(c)).collect();
        if name.is_empty() {
            return Err(format!("balise sans nom : <{tag}>"));
        }
        // Attributs : nom="valeur", sans doublon.
        let mut attrs = &body[name.len()..];
        let mut seen: Vec<String> = Vec::new();
        loop {
            attrs = attrs.trim_start();
            if attrs.is_empty() {
                break;
            }
            let an: String = attrs.chars().take_while(|&c| is_name_char(c)).collect();
            if an.is_empty() {
                return Err(format!("attribut illisible dans <{name}> : {attrs:?}"));
            }
            if seen.contains(&an) {
                return Err(format!("attribut {an} en double dans <{name}>"));
            }
            seen.push(an.clone());
            attrs = attrs[an.len()..].trim_start();
            attrs = attrs
                .strip_prefix('=')
                .ok_or("« = » manquant")?
                .trim_start();
            let q = attrs.chars().next().ok_or("valeur manquante")?;
            if q != '"' && q != '\'' {
                return Err(format!("valeur sans guillemets pour {an}"));
            }
            let close = attrs[1..].find(q).ok_or("guillemet non fermé")?;
            check_entities(&attrs[1..1 + close])?;
            attrs = &attrs[close + 2..];
        }
        if stack.is_empty() {
            roots += 1;
        }
        if !self_closing {
            stack.push(name);
        }
    }
    if !stack.is_empty() {
        return Err(format!("balises non fermées : {stack:?}"));
    }
    if roots != 1 {
        return Err(format!("{roots} éléments racines"));
    }
    Ok(())
}

#[test]
fn verificateur_xml() {
    assert!(check_xml("<?xml version=\"1.0\"?><a x=\"1\"><b/>t&amp;t</a>").is_ok());
    assert!(check_xml("<a><b></a></b>").is_err());
    assert!(check_xml("<a x=1/>").is_err());
    assert!(check_xml("<a x=\"1\" x=\"2\"/>").is_err());
    assert!(check_xml("<a>&nbsp;</a>").is_err());
    assert!(check_xml("<a/><b/>").is_err());
    assert!(check_xml("<a>\u{1}</a>").is_err());
    assert!(check_xml("<a>").is_err());
}
