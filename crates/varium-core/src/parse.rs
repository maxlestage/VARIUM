//! Analyse de la source : découpe en segments texte / variable.
//!
//! Syntaxe :
//! - `{{nom}}` : variable simple (les noms acceptent les accents : `{{prénom}}`)
//! - `{{montant | money}}` : variable + filtres chaînés
//! - `{{date | date:full}}`, `{{t | default:"Madame, Monsieur"}}` : filtre avec argument
//! - `\{{nom}}` : accolades littérales, non interprétées
//! - `{{si …}}`, `{{sinon}}`, `{{fin}}` : texte conditionnel (voir `conditions`) ; `sinon` et
//!   `fin` sont des mots réservés, on ne peut pas les employer comme noms de champ
//! - `{{qr:texte}}` : code QR du texte écrit tel quel (nommé `qr:texte`, ce n'est pas un
//!   champ à remplir) ; `{{site | qr}}` : code QR de la valeur d'un champ

/// Un filtre appliqué à une variable, ex. `money:usd`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub name: String,
    pub arg: Option<String>,
}

/// Une occurrence de variable dans la source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarRef {
    pub name: String,
    pub filters: Vec<Filter>,
    /// Texte exact de l'occurrence, ex. `{{ nom | upper }}`.
    pub raw: String,
    /// Plage en octets dans la source.
    pub range: std::ops::Range<usize>,
    /// Champ calculé `{{= a * b}}` : la formule (sans le `=`). `name` vaut alors `= formule`.
    pub formula: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Text(String),
    Var(VarRef),
}

/// Un nom de variable valide : commence par une lettre, un chiffre ou `_`,
/// puis lettres, chiffres, `_`, `.` ou `-`.
pub fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_alphanumeric() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

fn parse_filters(s: &str) -> Vec<Filter> {
    s.split('|')
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .map(|f| match f.split_once(':') {
            None => Filter {
                name: f.to_lowercase(),
                arg: None,
            },
            Some((n, a)) => {
                let a = a.trim();
                let unquoted = ['"', '\'']
                    .iter()
                    .find_map(|q| a.strip_prefix(*q).and_then(|r| r.strip_suffix(*q)))
                    .unwrap_or(a);
                Filter {
                    name: n.trim().to_lowercase(),
                    arg: Some(unquoted.to_string()),
                }
            }
        })
        .collect()
}

/// Préfixe du nom d'un code QR, littéral (`qr:texte`) ou de champ (`qr:nom`).
pub const QR: &str = "qr:";

fn qr_litteral(inner: &str) -> Option<&str> {
    let t = inner.trim_start();
    let tete = t.get(..3)?;
    tete.eq_ignore_ascii_case(QR).then(|| t[3..].trim())
}

/// Essaie de lire une variable commençant à `start` (qui pointe sur `{{`).
/// Renvoie (nom, filtres, fin) si la syntaxe est valide.
fn try_token(src: &str, start: usize) -> Option<(String, Vec<Filter>, usize)> {
    let inner_start = start + 2;
    // Le contenu ne peut contenir ni `{`, ni `}`, ni saut de ligne : on s'arrête au
    // premier de ces caractères (parcours borné, pas quadratique sur `{{{{{{…`).
    let reste = src.get(inner_start..)?;
    let off = reste.find(['{', '}', '\n'])?;
    if !reste[off..].starts_with("}}") {
        return None;
    }
    let close = inner_start + off;
    let inner = &reste[..off];
    // `{{qr:…}}` : tout ce qui suit est le contenu du code, `|` compris.
    if let Some(contenu) = qr_litteral(inner) {
        return (!contenu.is_empty()).then(|| (format!("{QR}{contenu}"), Vec::new(), close + 2));
    }
    let (name, filters) = match inner.split_once('|') {
        Some((n, f)) => (n.trim(), parse_filters(f)),
        None => (inner.trim(), Vec::new()),
    };
    if let Some(formula) = name.strip_prefix('=') {
        let formula = formula.trim();
        return (!formula.is_empty()).then(|| (format!("= {formula}"), filters, close + 2));
    }
    // `{{sinon}}` et `{{fin}}` sont des directives du texte conditionnel, pas des champs.
    let reserve = matches!(name.to_lowercase().as_str(), "sinon" | "fin");
    (is_valid_name(name) && !reserve).then(|| (name.to_string(), filters, close + 2))
}

/// Découpe la source en segments. Les textes adjacents sont fusionnés.
pub fn parse(src: &str) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    let bytes = src.as_bytes();
    let mut last = 0;

    while let Some(off) = src[i..].find("{{") {
        let pos = i + off;
        let escaped = pos > 0 && bytes[pos - 1] == b'\\';
        match try_token(src, pos) {
            Some((name, filters, end)) => {
                if escaped {
                    text.push_str(&src[last..pos - 1]);
                    text.push_str(&src[pos..end]);
                } else {
                    text.push_str(&src[last..pos]);
                    if !text.is_empty() {
                        out.push(Segment::Text(std::mem::take(&mut text)));
                    }
                    let formula = name.strip_prefix("= ").map(str::to_string);
                    out.push(Segment::Var(VarRef {
                        name,
                        filters,
                        raw: src[pos..end].to_string(),
                        range: pos..end,
                        formula,
                    }));
                }
                last = end;
                i = end;
            }
            None => i = pos + 1,
        }
    }
    text.push_str(&src[last..]);
    if !text.is_empty() {
        out.push(Segment::Text(text));
    }
    out
}

/// Variable détectée et son nombre d'occurrences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarInfo {
    pub name: String,
    pub count: usize,
}

/// Variables de la source, dans l'ordre de première apparition. Les variables
/// utilisées dans un champ calculé `{{= a * b}}` sont comptées ; le champ lui-même,
/// qui n'a pas de valeur à saisir, ne l'est pas.
pub fn variables(src: &str) -> Vec<VarInfo> {
    // (position, nom) : variables du texte et champs testés par `{{si …}}`, dans l'ordre.
    let mut found: Vec<(usize, String)> = crate::conditions::tested_names(src);
    for seg in parse(src) {
        if let Segment::Var(v) = seg {
            match &v.formula {
                Some(f) => found.extend(
                    crate::calc::identifiers(f)
                        .into_iter()
                        .filter(|n| is_valid_name(n))
                        .map(|n| (v.range.start, n)),
                ),
                // Un code QR littéral n'a rien à remplir.
                None if v.name.starts_with(QR) => {}
                None => found.push((v.range.start, v.name)),
            }
        }
    }
    found.sort_by_key(|(p, _)| *p);
    let mut out: Vec<VarInfo> = Vec::new();
    for (_, name) in found {
        match out.iter_mut().find(|i| i.name == name) {
            Some(i) => i.count += 1,
            None => out.push(VarInfo { name, count: 1 }),
        }
    }
    out
}

/// Les champs calculés de la source (`= formule`), sans doublon.
pub fn formulas(src: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for seg in parse(src) {
        if let Segment::Var(VarRef {
            formula: Some(f), ..
        }) = seg
            && !out.contains(&f)
        {
            out.push(f);
        }
    }
    out
}

/// Ré-échappe un texte littéral pour qu'il ne soit pas réinterprété comme variable.
pub fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(off) = text[i..].find("{{") {
        let pos = i + off;
        out.push_str(&text[i..pos]);
        let already = pos > 0 && text.as_bytes()[pos - 1] == b'\\';
        if !already && try_token(text, pos).is_some() {
            out.push('\\');
        }
        out.push_str("{{");
        i = pos + 2;
    }
    out.push_str(&text[i..]);
    out
}

/// Renomme une variable partout, sans toucher aux autres variables ni au texte.
/// Les filtres et l'espacement de chaque occurrence sont conservés.
pub fn rename_variable(src: &str, old: &str, new: &str) -> Result<String, String> {
    if !is_valid_name(new) {
        return Err(format!("Nom de variable invalide : « {new} »"));
    }
    Ok(rewrite(src, |v| match &v.formula {
        Some(f) => {
            let renamed = crate::calc::rename_in(f, old, new);
            (renamed != *f).then(|| v.raw.replacen(f.as_str(), &renamed, 1))
        }
        None => (v.name == old).then(|| v.raw.replacen(old, new, 1)),
    }))
}

/// Réécrit la source en remplaçant certaines occurrences de variables.
/// `f` renvoie `Some(remplacement brut)` ou `None` pour garder l'occurrence.
pub(crate) fn rewrite(src: &str, mut f: impl FnMut(&VarRef) -> Option<String>) -> String {
    let mut out = String::with_capacity(src.len());
    let mut last = 0;
    for seg in parse(src) {
        if let Segment::Var(v) = seg {
            out.push_str(&src[last..v.range.start]);
            out.push_str(&f(&v).unwrap_or_else(|| v.raw.clone()));
            last = v.range.end;
        }
    }
    out.push_str(&src[last..]);
    out
}
