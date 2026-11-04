//! Petit arbre XML pour les imports (ODT, DOCX), construit avec `quick-xml`.
//!
//! Les noms gardent leur préfixe (`table:table-cell`) ; [`Node::is`] compare
//! le nom local quand on ne donne pas de préfixe. Les tailles sont bornées :
//! un fichier trop gros ou trop imbriqué donne une erreur, jamais un plantage.

use quick_xml::Reader;
use quick_xml::events::Event;

/// Nombre maximal d'éléments dans un document.
const MAX_NODES: usize = 8_000_000;
/// Profondeur maximale d'imbrication.
const MAX_DEPTH: usize = 512;

#[derive(Debug, Default, Clone)]
pub(crate) struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Child>,
}

#[derive(Debug, Clone)]
pub(crate) enum Child {
    Elem(Node),
    Text(String),
}

fn local(name: &str) -> &str {
    name.rsplit_once(':').map_or(name, |(_, l)| l)
}

impl Node {
    /// Le nom correspond-il ? `"c"` compare le nom local, `"table:table"` le nom complet.
    pub fn is(&self, name: &str) -> bool {
        if name.contains(':') {
            self.name == name
        } else {
            local(&self.name) == name
        }
    }

    /// Valeur d'un attribut (même règle de nom que [`Node::is`]).
    pub fn attr(&self, name: &str) -> Option<&str> {
        let exact = name.contains(':');
        self.attrs
            .iter()
            .find(|(k, _)| if exact { k == name } else { local(k) == name })
            .map(|(_, v)| v.as_str())
    }

    /// Les éléments enfants.
    pub fn elems(&self) -> impl Iterator<Item = &Node> {
        self.children.iter().filter_map(|c| match c {
            Child::Elem(n) => Some(n),
            Child::Text(_) => None,
        })
    }

    /// Premier enfant de ce nom.
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.elems().find(|n| n.is(name))
    }

    /// Enfants de ce nom.
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.elems().filter(move |n| n.is(name))
    }

    /// Tout le texte contenu (descendants compris), dans l'ordre.
    pub fn text(&self) -> String {
        let mut out = String::new();
        let mut stack: Vec<std::slice::Iter<'_, Child>> = vec![self.children.iter()];
        while let Some(top) = stack.last_mut() {
            match top.next() {
                Some(Child::Text(t)) => out.push_str(t),
                Some(Child::Elem(n)) => stack.push(n.children.iter()),
                None => {
                    stack.pop();
                }
            }
        }
        out
    }
}

fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let n = name.strip_prefix('#')?;
            let code = match n.strip_prefix(['x', 'X']) {
                Some(h) => u32::from_str_radix(h, 16).ok()?,
                None => n.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

fn push_text(node: &mut Node, t: &str) {
    if t.is_empty() {
        return;
    }
    if let Some(Child::Text(prev)) = node.children.last_mut() {
        prev.push_str(t);
    } else {
        node.children.push(Child::Text(t.to_string()));
    }
}

fn element(e: &quick_xml::events::BytesStart<'_>) -> Node {
    let name = e.name().as_ref().to_string();
    let attrs = e
        .attributes()
        .with_checks(false)
        .flatten()
        .map(|a| {
            let k = a.key.as_ref().to_string();
            let v = a
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map(|v| v.into_owned())
                .unwrap_or_else(|_| a.value.to_string());
            (k, v)
        })
        .collect();
    Node {
        name,
        attrs,
        children: Vec::new(),
    }
}

/// Lit un document XML et renvoie son élément racine.
pub(crate) fn parse(text: &str) -> Result<Node, String> {
    let text = text.trim_start_matches('\u{FEFF}');
    let mut reader = Reader::from_str(text);
    {
        let cfg = reader.config_mut();
        cfg.check_end_names = false;
        cfg.allow_unmatched_ends = true;
    }
    let mut stack: Vec<Node> = vec![Node::default()];
    let mut count = 0usize;
    loop {
        let ev = reader
            .read_event()
            .map_err(|e| format!("XML illisible : {e}"))?;
        match ev {
            Event::Start(e) => {
                count += 1;
                if count > MAX_NODES || stack.len() > MAX_DEPTH {
                    return Err("document trop volumineux".into());
                }
                stack.push(element(&e));
            }
            Event::Empty(e) => {
                count += 1;
                if count > MAX_NODES {
                    return Err("document trop volumineux".into());
                }
                let n = element(&e);
                if let Some(top) = stack.last_mut() {
                    top.children.push(Child::Elem(n));
                }
            }
            Event::End(_) => {
                if stack.len() > 1 {
                    let n = stack.pop().unwrap_or_default();
                    if let Some(top) = stack.last_mut() {
                        top.children.push(Child::Elem(n));
                    }
                }
            }
            Event::Text(t) => {
                if let Some(top) = stack.last_mut() {
                    push_text(top, &t.xml10_content());
                }
            }
            Event::CData(t) => {
                if let Some(top) = stack.last_mut() {
                    let s = t.into_inner();
                    push_text(top, &s);
                }
            }
            Event::GeneralRef(r) => {
                if let Some(top) = stack.last_mut()
                    && let Some(c) = entity(&r)
                {
                    push_text(top, c.encode_utf8(&mut [0u8; 4]));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    // Document tronqué : on referme les éléments restés ouverts.
    while stack.len() > 1 {
        let n = stack.pop().unwrap_or_default();
        if let Some(top) = stack.last_mut() {
            top.children.push(Child::Elem(n));
        }
    }
    let root = stack.pop().unwrap_or_default();
    root.children
        .into_iter()
        .find_map(|c| match c {
            Child::Elem(n) => Some(n),
            Child::Text(_) => None,
        })
        .ok_or_else(|| "document XML vide".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arbre_simple() {
        let n = parse(
            "<?xml version=\"1.0\"?><a x=\"1&amp;2\"><b:c y='z'>t&lt;1&#233;<![CDATA[<k>]]></b:c><d/></a>",
        )
        .unwrap();
        assert!(n.is("a"));
        assert_eq!(n.attr("x"), Some("1&2"));
        let c = n.child("c").unwrap();
        assert!(c.is("b:c"));
        assert_eq!(c.attr("y"), Some("z"));
        assert_eq!(c.text(), "t<1é<k>");
        assert!(n.child("d").is_some());
    }

    #[test]
    fn documents_abimes() {
        assert!(parse("").is_err());
        assert!(parse("<a><b>").is_ok());
        let _ = parse("<a x=\"></a>");
        let _ = parse("<<<>>>");
        let _ = parse("<a>&bogus;</a>");
    }
}
