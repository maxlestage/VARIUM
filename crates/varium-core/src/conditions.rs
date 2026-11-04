//! Texte conditionnel : `{{si champ}}…{{sinon}}…{{fin}}`.
//!
//! Syntaxe (insensible à la casse pour les mots-clés) :
//! - `{{si nom}}` : garde le bloc si le champ est renseigné (non vide, et différent de
//!   « non », « faux », « false », « 0 ») ;
//! - `{{si non nom}}` : l'inverse ;
//! - `{{si nom = valeur}}` / `{{si nom != valeur}}` : comparaison (guillemets facultatifs) ;
//! - `{{sinon}}` : branche alternative ; `{{fin}}` : ferme le bloc.
//!
//! Les blocs s'imbriquent. Un `{{si}}` jamais fermé court jusqu'à la fin du document ;
//! un `{{sinon}}` ou `{{fin}}` sans `{{si}}` est laissé tel quel (visible, donc repérable).
//! Une directive seule sur sa ligne disparaît avec son saut de ligne. `\{{si x}}` écrit
//! la directive littéralement.

use std::ops::Range;

use crate::render::Values;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Cond {
    Truthy(String, bool),
    Eq(String, String, bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    If(Cond),
    Else,
    End,
}

#[derive(Debug, Clone)]
struct Directive {
    kind: Kind,
    /// Plage de la directive (étendue à la ligne entière si elle est seule).
    range: Range<usize>,
    /// Plage exacte de `{{…}}`.
    raw: Range<usize>,
    escaped: bool,
}

fn unquote(s: &str) -> &str {
    let s = s.trim();
    ['"', '\'']
        .iter()
        .find_map(|q| s.strip_prefix(*q).and_then(|r| r.strip_suffix(*q)))
        .unwrap_or(s)
}

fn parse_inner(inner: &str) -> Option<Kind> {
    let inner = inner.trim();
    let lower = inner.to_lowercase();
    match lower.as_str() {
        "sinon" => return Some(Kind::Else),
        "fin" => return Some(Kind::End),
        _ => {}
    }
    // `to_lowercase` peut changer les longueurs : on travaille sur l'original par mots.
    let mut words = inner.splitn(2, char::is_whitespace);
    if words.next()?.to_lowercase() != "si" {
        return None;
    }
    let rest = words.next()?.trim();
    let (negated, rest) = match rest.split_once(char::is_whitespace) {
        Some((w, r)) if w.to_lowercase() == "non" && !r.trim().is_empty() => (true, r.trim()),
        _ => (false, rest),
    };
    for (op, neg) in [("!=", true), ("=", false)] {
        if let Some((n, v)) = rest.split_once(op) {
            let n = n.trim();
            return crate::parse::is_valid_name(n).then(|| {
                Kind::If(Cond::Eq(
                    n.to_string(),
                    unquote(v).to_string(),
                    neg ^ negated,
                ))
            });
        }
    }
    crate::parse::is_valid_name(rest).then(|| Kind::If(Cond::Truthy(rest.to_string(), negated)))
}

fn directives(src: &str) -> Vec<Directive> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(off) = src[i..].find("{{") {
        let pos = i + off;
        let inner_start = pos + 2;
        let Some(len) = src[inner_start..].find(['{', '}', '\n']) else {
            break;
        };
        if !src[inner_start + len..].starts_with("}}") {
            i = pos + 1;
            continue;
        }
        let end = inner_start + len + 2;
        i = end;
        let Some(kind) = parse_inner(&src[inner_start..inner_start + len]) else {
            continue;
        };
        let escaped = pos > 0 && src.as_bytes()[pos - 1] == b'\\';
        let mut range = pos..end;
        if !escaped {
            let line_start = src[..pos].rfind('\n').map_or(0, |p| p + 1);
            let line_end = src[end..].find('\n').map_or(src.len(), |p| end + p);
            if src[line_start..pos].trim().is_empty() && src[end..line_end].trim().is_empty() {
                range = line_start..(line_end + 1).min(src.len());
            }
        }
        out.push(Directive {
            kind,
            range,
            raw: pos..end,
            escaped,
        });
    }
    out
}

/// Plages (en octets) des directives `{{si}}`, `{{sinon}}`, `{{fin}}`, pour la coloration.
pub fn directive_ranges(src: &str) -> Vec<Range<usize>> {
    directives(src)
        .into_iter()
        .filter(|d| !d.escaped)
        .map(|d| d.raw)
        .collect()
}

/// Champs testés par les `{{si …}}`, avec leur position dans la source.
pub(crate) fn tested_names(src: &str) -> Vec<(usize, String)> {
    directives(src)
        .into_iter()
        .filter(|d| !d.escaped)
        .filter_map(|d| match d.kind {
            Kind::If(Cond::Truthy(n, _) | Cond::Eq(n, _, _)) => Some((d.raw.start, n)),
            _ => None,
        })
        .collect()
}

/// Pour chaque champ testé par un `{{si}}` : les valeurs auxquelles il est comparé
/// (`{{si statut = payé}}`), dans l'ordre, sans doublon ; vide s'il n'est testé que
/// pour sa présence (`{{si remise}}`).
pub fn valeurs_testees(src: &str) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for d in directives(src).into_iter().filter(|d| !d.escaped) {
        let (nom, valeur) = match d.kind {
            Kind::If(Cond::Truthy(n, _)) => (n, None),
            Kind::If(Cond::Eq(n, v, _)) => (n, Some(v)),
            _ => continue,
        };
        let i = match out.iter().position(|(n, _)| *n == nom) {
            Some(i) => i,
            None => {
                out.push((nom, Vec::new()));
                out.len() - 1
            }
        };
        if let Some(v) = valeur.filter(|v| !v.is_empty())
            && !out[i]
                .1
                .iter()
                .any(|x| x.to_lowercase() == v.to_lowercase())
        {
            out[i].1.push(v);
        }
    }
    out
}

/// La valeur rend-elle `{{si champ}}` vrai ? (non vide, ni « non », « faux », « false », « 0 »)
pub fn est_vrai(v: &str) -> bool {
    truthy(v)
}

fn truthy(v: &str) -> bool {
    let v = v.trim();
    !(v.is_empty() || ["non", "faux", "false", "0"].contains(&v.to_lowercase().as_str()))
}

fn holds(c: &Cond, values: &Values) -> bool {
    match c {
        Cond::Truthy(n, neg) => truthy(values.get(n)) ^ neg,
        Cond::Eq(n, v, neg) => (values.get(n).trim().to_lowercase() == v.to_lowercase()) ^ neg,
    }
}

/// Anomalies de structure (en français), vide si les blocs sont bien formés.
pub fn problems(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack: Vec<(bool, usize)> = Vec::new(); // (sinon déjà vu, ligne)
    let line = |p: usize| src[..p].matches('\n').count() + 1;
    for d in directives(src).into_iter().filter(|d| !d.escaped) {
        match d.kind {
            Kind::If(_) => stack.push((false, line(d.raw.start))),
            Kind::Else => match stack.last_mut() {
                None => out.push(format!(
                    "ligne {} : {{{{sinon}}}} sans {{{{si}}}}",
                    line(d.raw.start)
                )),
                Some((true, _)) => out.push(format!(
                    "ligne {} : deux {{{{sinon}}}} dans le même bloc",
                    line(d.raw.start)
                )),
                Some(top) => top.0 = true,
            },
            Kind::End => {
                if stack.pop().is_none() {
                    out.push(format!(
                        "ligne {} : {{{{fin}}}} sans {{{{si}}}}",
                        line(d.raw.start)
                    ));
                }
            }
        }
    }
    for (_, l) in stack {
        out.push(format!(
            "ligne {l} : {{{{si}}}} jamais fermé par {{{{fin}}}}"
        ));
    }
    out
}

/// Applique les conditions : renvoie la source sans les blocs écartés ni les directives.
pub fn resolve(src: &str, values: &Values) -> String {
    let ds = directives(src);
    if ds.is_empty() {
        return src.to_string();
    }
    let mut out = String::with_capacity(src.len());
    // Pile de (branche active, condition du parent active, sinon déjà vu).
    let mut stack: Vec<(bool, bool, bool)> = Vec::new();
    let active = |s: &[(bool, bool, bool)]| s.last().is_none_or(|t| t.0 && t.1);
    let mut last = 0;
    for d in ds {
        if d.range.start < last {
            continue;
        }
        if active(&stack) {
            out.push_str(&src[last..d.range.start]);
        }
        last = d.range.end;
        if d.escaped {
            // `\{{si x}}` : directive littérale, sans l'antislash.
            if active(&stack) {
                out.truncate(out.len().saturating_sub(1));
                out.push_str(&src[d.raw.clone()]);
            }
            continue;
        }
        let parent = active(&stack);
        match d.kind {
            Kind::If(c) => stack.push((holds(&c, values), parent, false)),
            Kind::Else => match stack.last_mut() {
                Some(top) if !top.2 => {
                    top.0 = !top.0;
                    top.2 = true;
                }
                // `sinon` superflu ou orphelin : laissé tel quel.
                _ => {
                    if parent {
                        out.push_str(&src[d.range.clone()]);
                    }
                }
            },
            Kind::End => {
                if stack.pop().is_none() {
                    out.push_str(&src[d.range.clone()]);
                }
            }
        }
    }
    if active(&stack) {
        out.push_str(&src[last..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(pairs: &[(&str, &str)]) -> Values {
        let mut v = Values::default();
        for (k, x) in pairs {
            v.set(*k, *x);
        }
        v
    }

    #[test]
    fn garde_ou_retire() {
        let s = "A {{si tva}}avec TVA{{fin}} B";
        assert_eq!(resolve(s, &v(&[("tva", "20")])), "A avec TVA B");
        assert_eq!(resolve(s, &v(&[])), "A  B");
        assert_eq!(resolve(s, &v(&[("tva", "non")])), "A  B");
    }

    #[test]
    fn sinon_et_negation() {
        let s = "{{si x}}oui{{sinon}}non{{fin}}";
        assert_eq!(resolve(s, &v(&[("x", "1")])), "oui");
        assert_eq!(resolve(s, &v(&[])), "non");
        assert_eq!(resolve("{{si non x}}vide{{fin}}", &v(&[])), "vide");
    }

    #[test]
    fn comparaison() {
        let s = "{{si statut = \"payé\"}}P{{sinon}}N{{fin}}";
        assert_eq!(resolve(s, &v(&[("statut", "Payé")])), "P");
        assert_eq!(resolve(s, &v(&[("statut", "dû")])), "N");
        assert_eq!(resolve("{{si a != b}}x{{fin}}", &v(&[("a", "c")])), "x");
    }

    #[test]
    fn imbrication() {
        let s = "{{si a}}1{{si b}}2{{sinon}}3{{fin}}4{{sinon}}5{{fin}}";
        assert_eq!(resolve(s, &v(&[("a", "1"), ("b", "1")])), "124");
        assert_eq!(resolve(s, &v(&[("a", "1")])), "134");
        assert_eq!(resolve(s, &v(&[("b", "1")])), "5");
    }

    #[test]
    fn directive_seule_sur_sa_ligne() {
        let s = "a\n{{si x}}\nligne\n{{fin}}\nb";
        assert_eq!(resolve(s, &v(&[("x", "1")])), "a\nligne\nb");
        assert_eq!(resolve(s, &v(&[])), "a\nb");
    }

    #[test]
    fn non_ferme_et_orphelins() {
        assert_eq!(resolve("a{{si x}}b", &v(&[])), "a");
        assert_eq!(resolve("a{{si x}}b", &v(&[("x", "1")])), "ab");
        assert_eq!(resolve("a{{fin}}b", &v(&[])), "a{{fin}}b");
        assert_eq!(resolve("a{{sinon}}b", &v(&[])), "a{{sinon}}b");
        assert_eq!(problems("a{{si x}}b").len(), 1);
        assert_eq!(problems("{{fin}}").len(), 1);
        assert!(problems("{{si x}}{{sinon}}{{fin}}").is_empty());
        assert_eq!(problems("{{si x}}{{sinon}}{{sinon}}{{fin}}").len(), 1);
    }

    #[test]
    fn echappement_et_variables() {
        assert_eq!(resolve("\\{{si x}}", &v(&[])), "{{si x}}");
        let s = "{{si x}}{{nom}}{{fin}}";
        assert_eq!(resolve(s, &v(&[("x", "1")])), "{{nom}}");
    }

    #[test]
    fn pas_de_panique() {
        for s in [
            "{{si",
            "{{si }}",
            "{{si non}}",
            "{{si =}}",
            "é{{si é}}{{fin}}é",
            "{{{{si x}}",
        ] {
            let _ = resolve(s, &v(&[]));
            let _ = problems(s);
        }
    }
}
