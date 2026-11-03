//! Interface traduite : français (langue de référence), anglais, espagnol, allemand,
//! italien et portugais.
//!
//! Le texte français sert de clé : `tr("Enregistrer")` rend « Save » en anglais, et le
//! français lui-même s'il manque une traduction. `trf!("{n} diapositives", n)` traduit
//! puis remplace les `{nom}` (`{{` et `}}` donnent une accolade).
//!
//! Les traductions sont dans `tr/<code>.rs`, triées par clé (recherche dichotomique).
//! Un test vérifie que chaque `tr("…")`, `trf!("…")` et `n_("…")` des sources a sa
//! traduction dans les cinq tables, avec les mêmes `{…}`.
//!
//! La langue de l'interface est indépendante de celle du document (`page.langue`).

use std::fmt::Display;
use std::sync::atomic::{AtomicU8, Ordering};

use varium_core::Lang;

mod de;
mod en;
mod es;
mod it;
mod pt;

/// Clé d'enregistrement du choix (eframe).
pub const STORAGE_KEY: &str = "varium.ui_lang";

const AUTO: u8 = u8::MAX;

/// Langue appliquée (indice dans `Lang::ALL`).
static LANG: AtomicU8 = AtomicU8::new(0);
/// Choix de l'utilisateur : `AUTO` ou indice dans `Lang::ALL`.
static CHOICE: AtomicU8 = AtomicU8::new(AUTO);

fn index(l: Lang) -> u8 {
    Lang::ALL.iter().position(|&x| x == l).unwrap_or(0) as u8
}

/// Langue actuelle de l'interface.
pub fn lang() -> Lang {
    Lang::ALL[LANG.load(Ordering::Relaxed) as usize % Lang::ALL.len()]
}

/// Le choix de l'utilisateur (`None` : celle du système).
pub fn choice() -> Option<Lang> {
    match CHOICE.load(Ordering::Relaxed) {
        AUTO => None,
        i => Lang::ALL.get(i as usize).copied(),
    }
}

/// Choisit la langue de l'interface (`None` : celle du système) et l'applique.
pub fn set_choice(c: Option<Lang>) {
    CHOICE.store(c.map_or(AUTO, index), Ordering::Relaxed);
    LANG.store(index(c.unwrap_or_else(system)), Ordering::Relaxed);
}

/// Code enregistré pour un choix : `auto`, `fr`, `en`…
pub fn choice_code(c: Option<Lang>) -> &'static str {
    c.map_or("auto", Lang::code)
}

/// Relit un code enregistré (`auto` ou inconnu : `None`).
pub fn parse_choice(code: &str) -> Option<Lang> {
    Lang::ALL.into_iter().find(|l| l.code() == code)
}

/// La langue d'une étiquette de paramètres régionaux (`de_DE.UTF-8`, `pt-BR`…).
fn from_locale(s: &str) -> Option<Lang> {
    let code = s.get(..2)?.to_ascii_lowercase();
    if s.len() > 2 && s.as_bytes()[2].is_ascii_alphabetic() {
        return None;
    }
    parse_choice(&code)
}

/// La langue du système ; l'anglais si elle n'est pas proposée.
pub fn system() -> Lang {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let found = sys_locale::get_locales().find_map(|l| from_locale(&l));
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let found = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .and_then(|v| from_locale(&v));
    found.unwrap_or(Lang::En)
}

/// Nom d'une langue dans cette langue, pour le menu de choix.
pub fn choices() -> impl Iterator<Item = (Option<Lang>, &'static str)> {
    std::iter::once((None, tr("Automatique (système)")))
        .chain(Lang::ALL.into_iter().map(|l| (Some(l), l.native())))
}

/// Le sous-menu « Langue de l'interface » des menus egui.
pub fn menu(ui: &mut egui::Ui) {
    ui.menu_button(tr("Langue de l'interface"), |ui| {
        let current = choice();
        for (c, name) in choices() {
            if ui.radio(current == c, name).clicked() {
                set_choice(c);
                ui.close();
            }
        }
    });
}

/// La table de traductions d'une langue (`None` pour le français).
fn table(l: Lang) -> Option<&'static [(&'static str, &'static str)]> {
    match l {
        Lang::Fr => None,
        Lang::En => Some(en::T),
        Lang::Es => Some(es::T),
        Lang::De => Some(de::T),
        Lang::It => Some(it::T),
        Lang::Pt => Some(pt::T),
    }
}

/// Traduit `fr` dans la langue `l`.
pub fn tr_in(l: Lang, fr: &'static str) -> &'static str {
    match table(l) {
        None => fr,
        Some(t) => match t.binary_search_by(|(k, _)| (*k).cmp(fr)) {
            Ok(i) => t[i].1,
            Err(_) => fr,
        },
    }
}

/// Traduit un texte français de l'interface dans la langue actuelle.
pub fn tr(fr: &'static str) -> &'static str {
    tr_in(lang(), fr)
}

/// Marque un texte à traduire là où `tr` ne peut pas être appelé (constantes) ;
/// il est traduit à l'affichage par `tr`.
pub const fn n_(fr: &'static str) -> &'static str {
    fr
}

/// Remplace les `{nom}` de `template` par les valeurs données.
pub fn fill(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            out.push_str(&tail[..1]);
            rest = &tail[2..];
        } else if let Some(end) = tail.find('}').filter(|_| tail.starts_with('{')) {
            let name = &tail[1..end];
            match args.iter().find(|(k, _)| *k == name) {
                Some((_, v)) => out.push_str(&v.to_string()),
                None => out.push_str(&tail[..=end]),
            }
            rest = &tail[end + 1..];
        } else {
            out.push_str(&tail[..1]);
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// `trf!("{n} remarques", n)` ou `trf!("Version {v}", v = env!("CARGO_PKG_VERSION"))` :
/// traduit le modèle puis remplace les `{nom}`.
macro_rules! trf {
    (@v $k:ident) => { $k };
    (@v $k:ident = $v:expr) => { $v };
    ($s:literal $(, $k:ident $(= $v:expr)?)* $(,)?) => {
        $crate::tr::fill(
            $crate::tr::tr($s),
            &[$((stringify!($k), &trf!(@v $k $(= $v)?) as &dyn std::fmt::Display)),*],
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn remplissage() {
        let n = 3;
        assert_eq!(fill("{n} mots, {{x}}", &[("n", &n)]), "3 mots, {x}");
        assert_eq!(fill("{inconnu} }", &[]), "{inconnu} }");
        LANG.store(index(Lang::En), Ordering::Relaxed);
        assert_eq!(trf!("Version {v}", v = "1.0"), "Version 1.0");
        LANG.store(0, Ordering::Relaxed);
    }

    #[test]
    fn locales() {
        assert_eq!(from_locale("de_DE.UTF-8"), Some(Lang::De));
        assert_eq!(from_locale("pt-BR"), Some(Lang::Pt));
        assert_eq!(from_locale("fr"), Some(Lang::Fr));
        assert_eq!(from_locale("C.UTF-8"), None);
        assert_eq!(from_locale("ja_JP"), None);
        assert_eq!(from_locale("eng"), None);
    }

    /// Décode le contenu d'un littéral de chaîne Rust (échappements usuels).
    fn unescape(raw: &str) -> String {
        let mut out = String::new();
        let mut it = raw.chars().peekable();
        while let Some(c) = it.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('0') => out.push('\0'),
                Some('u') => {
                    let hex: String = it
                        .by_ref()
                        .skip_while(|&c| c == '{')
                        .take_while(|&c| c != '}')
                        .collect();
                    out.extend(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32));
                }
                Some('\n') => {
                    while it.peek().is_some_and(|c| c.is_whitespace()) {
                        it.next();
                    }
                }
                Some(c) => out.push(c),
                None => {}
            }
        }
        out
    }

    /// Les littéraux passés à `tr(`, `trf!(` et `n_(` dans les sources du crate.
    fn keys() -> BTreeMap<String, String> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "rs"))
            .collect();
        files.sort();
        let mut keys = BTreeMap::new();
        for f in files {
            let full = std::fs::read_to_string(&f).unwrap();
            // Sans les tests ni les commentaires.
            let code = full.split("#[cfg(test)]\nmod tests").next().unwrap_or_default();
            let src: String = code
                .lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .flat_map(|l| [l, "\n"])
                .collect();
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            for marker in ["tr(", "trf!(", "n_("] {
                let mut from = 0;
                while let Some(i) = src[from..].find(marker) {
                    let at = from + i;
                    from = at + marker.len();
                    // L'identifiant doit commencer là (pas `str(`, `attr(`…).
                    if src[..at]
                        .chars()
                        .next_back()
                        .is_some_and(|c| c.is_alphanumeric() || c == '_')
                    {
                        continue;
                    }
                    let rest = &src[from..];
                    let trimmed = rest.trim_start();
                    if !trimmed.starts_with('"') {
                        continue;
                    }
                    let start = from + (rest.len() - trimmed.len()) + 1;
                    let bytes = src.as_bytes();
                    let mut j = start;
                    while bytes[j] != b'"' {
                        j += if bytes[j] == b'\\' { 2 } else { 1 };
                    }
                    let key = unescape(&src[start..j]);
                    if key != "…" {
                        keys.entry(key).or_insert_with(|| name.clone());
                    }
                }
            }
        }
        keys
    }

    fn placeholders(s: &str) -> BTreeSet<String> {
        let s = s.replace("{{", "").replace("}}", "");
        let mut out = BTreeSet::new();
        let mut rest = s.as_str();
        while let Some(i) = rest.find('{') {
            rest = &rest[i + 1..];
            if let Some(end) = rest.find('}') {
                let name = &rest[..end];
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    out.insert(name.to_string());
                }
            }
        }
        out
    }

    #[test]
    fn toutes_les_chaines_sont_traduites() {
        let keys = keys();
        assert!(keys.len() > 500, "{} clés seulement ?", keys.len());
        let mut errors = Vec::new();
        for l in Lang::ALL {
            let Some(t) = table(l) else { continue };
            for w in t.windows(2) {
                if w[0].0 >= w[1].0 {
                    errors.push(format!("{}: table non triée vers « {} »", l.code(), w[1].0));
                }
            }
            let map: BTreeMap<_, _> = t.iter().copied().collect();
            for (k, file) in &keys {
                match map.get(k.as_str()) {
                    None => errors.push(format!("{}: manque « {k} » ({file})", l.code())),
                    Some(v) if placeholders(k) != placeholders(v) => errors.push(format!(
                        "{}: accolades différentes pour « {k} » → « {v} »",
                        l.code()
                    )),
                    Some(v) if v.trim().is_empty() => {
                        errors.push(format!("{}: traduction vide pour « {k} »", l.code()))
                    }
                    _ => {}
                }
            }
            for k in map.keys() {
                if !keys.contains_key(*k) {
                    errors.push(format!("{}: clé inutilisée « {k} »", l.code()));
                }
            }
        }
        assert!(
            errors.is_empty(),
            "{} problème(s) :\n{}",
            errors.len(),
            errors.join("\n")
        );
    }
}
