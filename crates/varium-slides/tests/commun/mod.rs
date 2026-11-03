//! Outils communs aux tests : lecture d'une archive ZIP stockée et vérification
//! qu'un XML est bien formé (sans dépendance).

#![allow(dead_code)]

/// Les fichiers d'une archive ZIP sans compression, dans l'ordre de l'archive.
pub fn zip_entries(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    let u16_at = |o: usize| u16::from_le_bytes([data[o], data[o + 1]]) as usize;
    let u32_at =
        |o: usize| u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]) as usize;
    let mut out = Vec::new();
    let mut o = 0;
    while o + 30 <= data.len() && u32_at(o) == 0x0403_4b50 {
        assert_eq!(u16_at(o + 8), 0, "fichier compressé inattendu");
        let taille = u32_at(o + 18);
        assert_eq!(taille, u32_at(o + 22), "tailles incohérentes");
        let (nom_len, extra) = (u16_at(o + 26), u16_at(o + 28));
        let nom = String::from_utf8(data[o + 30..o + 30 + nom_len].to_vec()).unwrap();
        let debut = o + 30 + nom_len + extra;
        out.push((nom, data[debut..debut + taille].to_vec()));
        o = debut + taille;
    }
    out
}

/// Le contenu texte d'un fichier de l'archive.
pub fn zip_text(data: &[u8], nom: &str) -> String {
    let (_, contenu) = zip_entries(data)
        .into_iter()
        .find(|(n, _)| n == nom)
        .unwrap_or_else(|| panic!("{nom} absent de l'archive"));
    String::from_utf8(contenu).expect("XML en UTF-8")
}

fn entite_valide(e: &str) -> bool {
    match e {
        "amp" | "lt" | "gt" | "quot" | "apos" => true,
        _ => {
            let n = if let Some(h) = e.strip_prefix("#x") {
                u32::from_str_radix(h, 16).ok()
            } else if let Some(d) = e.strip_prefix('#') {
                d.parse().ok()
            } else {
                None
            };
            n.and_then(char::from_u32).is_some()
        }
    }
}

/// Vérifie le texte entre deux balises ou une valeur d'attribut.
fn texte_valide(s: &str) -> Result<(), String> {
    let mut reste = s;
    while let Some(i) = reste.find(['&', '<']) {
        if reste.as_bytes()[i] == b'<' {
            return Err(format!("« < » non échappé près de {:?}", &reste[i..]));
        }
        let fin = reste[i..]
            .find(';')
            .ok_or_else(|| format!("entité non fermée : {:?}", &reste[i..]))?;
        let e = &reste[i + 1..i + fin];
        if !entite_valide(e) {
            return Err(format!("entité inconnue : &{e};"));
        }
        reste = &reste[i + fin + 1..];
    }
    if s.chars().any(|c| c < ' ' && !matches!(c, '\n' | '\r' | '\t')) {
        return Err("caractère de contrôle interdit en XML".into());
    }
    Ok(())
}

fn nom_valide(n: &str) -> bool {
    !n.is_empty()
        && n
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ':' | '_' | '-' | '.'))
}

/// Vérifie qu'un document XML est bien formé : balises équilibrées, attributs entre
/// guillemets et uniques, entités connues, une seule racine.
pub fn xml_bien_forme(xml: &str) -> Result<(), String> {
    let mut s = xml.trim_start();
    if let Some(r) = s.strip_prefix("<?xml") {
        let fin = r.find("?>").ok_or("déclaration XML non fermée")?;
        s = &r[fin + 2..];
    }
    let mut pile: Vec<String> = Vec::new();
    let mut racines = 0;
    while !s.is_empty() {
        let Some(i) = s.find('<') else {
            if !pile.is_empty() {
                return Err(format!("balises non fermées : {pile:?}"));
            }
            if !s.trim().is_empty() {
                return Err("texte après la racine".into());
            }
            break;
        };
        let texte = &s[..i];
        if pile.is_empty() && !texte.trim().is_empty() {
            return Err("texte hors de la racine".into());
        }
        texte_valide(texte)?;
        s = &s[i..];
        if let Some(r) = s.strip_prefix("<!--") {
            let fin = r.find("-->").ok_or("commentaire non fermé")?;
            s = &r[fin + 3..];
            continue;
        }
        let fin = s.find('>').ok_or("balise non fermée")?;
        let balise = &s[1..fin];
        s = &s[fin + 1..];
        if let Some(nom) = balise.strip_prefix('/') {
            let ouvert = pile.pop().ok_or_else(|| format!("</{nom}> sans ouverture"))?;
            if ouvert != nom.trim() {
                return Err(format!("<{ouvert}> fermée par </{nom}>"));
            }
            continue;
        }
        let (corps, vide) = match balise.strip_suffix('/') {
            Some(c) => (c, true),
            None => (balise, false),
        };
        let nom_fin = corps.find(char::is_whitespace).unwrap_or(corps.len());
        let nom = &corps[..nom_fin];
        if !nom_valide(nom) {
            return Err(format!("nom de balise invalide : {nom:?}"));
        }
        // Attributs : nom="valeur", séparés par des espaces.
        let mut attrs = corps[nom_fin..].trim();
        let mut vus: Vec<&str> = Vec::new();
        while !attrs.is_empty() {
            let eg = attrs
                .find('=')
                .ok_or_else(|| format!("attribut sans valeur dans <{nom}>"))?;
            let an = attrs[..eg].trim();
            if !nom_valide(an) {
                return Err(format!("nom d'attribut invalide : {an:?}"));
            }
            if vus.contains(&an) {
                return Err(format!("attribut {an} répété dans <{nom}>"));
            }
            vus.push(an);
            let r = attrs[eg + 1..].trim_start();
            let q = r.chars().next().ok_or("valeur d'attribut absente")?;
            if q != '"' && q != '\'' {
                return Err(format!("valeur sans guillemets pour {an}"));
            }
            let fin_v = r[1..]
                .find(q)
                .ok_or_else(|| format!("valeur non fermée pour {an}"))?;
            texte_valide(&r[1..1 + fin_v])?;
            attrs = r[1 + fin_v + 1..].trim_start();
        }
        if pile.is_empty() {
            racines += 1;
            if racines > 1 {
                return Err("plusieurs racines".into());
            }
        }
        if !vide {
            pile.push(nom.to_string());
        }
    }
    if !pile.is_empty() {
        return Err(format!("balises non fermées : {pile:?}"));
    }
    if racines != 1 {
        return Err("aucune racine".into());
    }
    Ok(())
}

/// Nombre d'occurrences d'un motif dans un texte.
pub fn compte(texte: &str, motif: &str) -> usize {
    texte.matches(motif).count()
}
