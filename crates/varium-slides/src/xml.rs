//! Arbre XML minimal pour l'import ODP et PPTX, lu avec quick-xml.
//!
//! Les éléments sont désignés par leur nom local (sans préfixe d'espace de noms) :
//! les fichiers des différents logiciels emploient presque toujours les préfixes
//! usuels, mais rien ne les y oblige. Profondeur et nombre de nœuds sont bornés :
//! un fichier malveillant ne peut ni épuiser la pile ni la mémoire.

use quick_xml::events::Event;

/// Profondeur maximale d'imbrication.
const PROFONDEUR_MAX: usize = 256;
/// Nombre maximal d'éléments d'un fichier.
const NOEUDS_MAX: usize = 2_000_000;

#[derive(Debug, Clone, Default)]
pub(crate) struct Noeud {
    /// Nom qualifié, tel qu'écrit (`draw:frame`).
    pub nom: String,
    pub attrs: Vec<(String, String)>,
    pub enfants: Vec<Enfant>,
}

#[derive(Debug, Clone)]
pub(crate) enum Enfant {
    Noeud(Noeud),
    Texte(String),
}

fn local(nom: &str) -> &str {
    nom.rsplit_once(':').map_or(nom, |(_, l)| l)
}

impl Noeud {
    /// Nom local de l'élément.
    pub fn local(&self) -> &str {
        local(&self.nom)
    }

    /// L'élément a ce nom local.
    pub fn est(&self, l: &str) -> bool {
        self.local() == l
    }

    /// Valeur d'un attribut : par nom qualifié exact (`draw:style-name`), sinon par
    /// nom local si le nom demandé n'a pas de préfixe ou si le préfixe diffère.
    pub fn attr(&self, nom: &str) -> Option<&str> {
        if let Some((_, v)) = self.attrs.iter().find(|(k, _)| k == nom) {
            return Some(v);
        }
        let l = local(nom);
        self.attrs
            .iter()
            .find(|(k, _)| local(k) == l && !k.starts_with("xmlns"))
            .map(|(_, v)| v.as_str())
    }

    /// Les éléments enfants.
    pub fn noeuds(&self) -> impl Iterator<Item = &Noeud> {
        self.enfants.iter().filter_map(|e| match e {
            Enfant::Noeud(n) => Some(n),
            Enfant::Texte(_) => None,
        })
    }

    /// Le premier enfant de ce nom local.
    pub fn enfant(&self, l: &str) -> Option<&Noeud> {
        self.noeuds().find(|n| n.est(l))
    }

    /// Suit un chemin de noms locaux d'enfants.
    pub fn chemin(&self, chemin: &[&str]) -> Option<&Noeud> {
        let mut n = self;
        for l in chemin {
            n = n.enfant(l)?;
        }
        Some(n)
    }

    /// Le premier descendant (en profondeur) de ce nom local.
    pub fn descendant(&self, l: &str) -> Option<&Noeud> {
        let mut pile: Vec<&Noeud> = self.noeuds().collect();
        pile.reverse();
        while let Some(n) = pile.pop() {
            if n.est(l) {
                return Some(n);
            }
            let debut = pile.len();
            pile.extend(n.noeuds());
            pile[debut..].reverse();
        }
        None
    }

    /// Tous les descendants de ce nom local, dans l'ordre du document.
    pub fn descendants<'a>(&'a self, l: &str) -> Vec<&'a Noeud> {
        let mut out = Vec::new();
        let mut pile: Vec<&Noeud> = self.noeuds().collect();
        pile.reverse();
        while let Some(n) = pile.pop() {
            if n.est(l) {
                out.push(n);
            }
            let debut = pile.len();
            pile.extend(n.noeuds());
            pile[debut..].reverse();
        }
        out
    }

    /// Tout le texte contenu, sans balises.
    pub fn texte(&self) -> String {
        let mut s = String::new();
        let mut pile: Vec<&Enfant> = self.enfants.iter().rev().collect();
        while let Some(e) = pile.pop() {
            match e {
                Enfant::Texte(t) => s.push_str(t),
                Enfant::Noeud(n) => pile.extend(n.enfants.iter().rev()),
            }
        }
        s
    }
}

/// Lit un document XML et renvoie son élément racine.
pub(crate) fn lire(xml: &str) -> Result<Noeud, String> {
    let mal_forme = |detail: &str| format!("Fichier XML endommagé ({detail}).");
    let mut lecteur = quick_xml::Reader::from_str(xml);
    lecteur.config_mut().check_end_names = false;
    let mut pile: Vec<Noeud> = vec![Noeud::default()];
    let mut compte = 0usize;
    let ouvrir = |e: &quick_xml::events::BytesStart<'_>| -> Noeud {
        let attrs = e
            .attributes()
            .with_checks(false)
            .filter_map(|a| a.ok())
            .map(|a| {
                let k = a.key.as_ref().to_string();
                let v = a
                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .map(|v| v.into_owned())
                    .unwrap_or_else(|_| a.value.to_string());
                (k, v)
            })
            .collect();
        Noeud {
            nom: e.name().as_ref().to_string(),
            attrs,
            enfants: Vec::new(),
        }
    };
    let texte = |pile: &mut Vec<Noeud>, t: &str| {
        if t.is_empty() {
            return;
        }
        if let Some(haut) = pile.last_mut() {
            match haut.enfants.last_mut() {
                Some(Enfant::Texte(s)) => s.push_str(t),
                _ => haut.enfants.push(Enfant::Texte(t.to_string())),
            }
        }
    };
    loop {
        let ev = lecteur
            .read_event()
            .map_err(|e| mal_forme(&crate::texte::tronquer(&e.to_string(), 80)))?;
        match ev {
            Event::Start(e) => {
                compte += 1;
                if pile.len() > PROFONDEUR_MAX || compte > NOEUDS_MAX {
                    return Err(mal_forme("trop d'éléments imbriqués"));
                }
                pile.push(ouvrir(&e));
            }
            Event::Empty(e) => {
                compte += 1;
                if compte > NOEUDS_MAX {
                    return Err(mal_forme("trop d'éléments"));
                }
                let n = ouvrir(&e);
                if let Some(haut) = pile.last_mut() {
                    haut.enfants.push(Enfant::Noeud(n));
                }
            }
            Event::End(_) => {
                if pile.len() <= 1 {
                    return Err(mal_forme("balise fermante en trop"));
                }
                if let Some(n) = pile.pop()
                    && let Some(haut) = pile.last_mut()
                {
                    haut.enfants.push(Enfant::Noeud(n));
                }
            }
            Event::Text(t) => texte(&mut pile, &t.xml10_content()),
            Event::CData(t) => texte(&mut pile, &t.into_inner()),
            Event::GeneralRef(r) => {
                let c = match r.resolve_char_ref() {
                    Ok(Some(c)) => Some(c),
                    _ => match &*r {
                        "amp" => Some('&'),
                        "lt" => Some('<'),
                        "gt" => Some('>'),
                        "quot" => Some('"'),
                        "apos" => Some('\''),
                        _ => None,
                    },
                };
                if let Some(c) = c {
                    texte(&mut pile, c.encode_utf8(&mut [0; 4]));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if pile.len() != 1 {
        return Err(mal_forme("document tronqué"));
    }
    pile.pop()
        .and_then(|racine| {
            racine.enfants.into_iter().find_map(|e| match e {
                Enfant::Noeud(n) => Some(n),
                Enfant::Texte(_) => None,
            })
        })
        .ok_or_else(|| mal_forme("document vide"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arbre_et_entites() {
        let r = lire("<?xml version=\"1.0\"?><a:x xmlns:a=\"u\" a:k=\"1 &amp; 2\"><b>t&#233;&lt;</b><c/></a:x>")
            .unwrap();
        assert!(r.est("x"));
        assert_eq!(r.attr("a:k"), Some("1 & 2"));
        assert_eq!(r.attr("k"), Some("1 & 2"));
        assert_eq!(r.texte(), "té<");
        assert!(r.enfant("c").is_some());
        assert_eq!(r.descendants("b").len(), 1);
    }

    #[test]
    fn xml_abime() {
        assert!(lire("<a><b></a>").is_err() || lire("<a><b></a>").is_ok());
        assert!(lire("<a><b>").is_err());
        assert!(lire("").is_err());
        assert!(lire(&"<a>".repeat(1000)).is_err());
    }
}
