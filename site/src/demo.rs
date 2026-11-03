//! La démo : le vrai moteur `varium-core`, exécuté par le serveur.
//!
//! Le formulaire envoie la source et une valeur par variable ; le serveur détecte les
//! variables, applique les filtres et renvoie le document rendu. Aucun JavaScript :
//! la page fonctionne partout, et la Content-Security-Policy n'autorise aucun script.

use std::collections::BTreeMap;

use paquets::prelude::*;
use paquets::serde::{Deserialize, Serialize};
use varium_core::{Align, Para, Piece, Run, Values, VarType, parse, render, render_pieces, tex};

use crate::{Options, layout};

/// Taille maximale d'une source, en caractères.
pub const LIMITE: usize = 20_000;

/// Le document d'exemple, ouvert par défaut dans la démo.
pub const EXEMPLE: &str = r#"{{entreprise | upper}}
{{adresse}}

{{ville}}, le {{date_courrier}}

Objet : dossier {{référence}}

Bonjour {{prénom}} {{nom}},

Votre dossier {{référence}} est en cours de traitement. Le montant retenu s'élève à {{montant}}, soit une mensualité de $m = \frac{M}{n}$ sur {{durée}} mois.

Le taux appliqué suit la formule :
$$ C_n = C_0 \times (1 + t)^n $$

Nous restons à votre disposition, {{prénom | capitalize}}, pour toute question.

Cordialement,

{{signataire | default:"Le service client"}}
{{entreprise}}"#;

/// Les valeurs et les types de l'exemple. `{{nom}}` reste volontairement vide.
pub fn valeurs_exemple() -> Values {
    let mut v = Values::default();
    v.set("entreprise", "Atelier Lumen")
        .set("adresse", "12 rue des Lilas, 69003 Lyon")
        .set("ville", "Lyon")
        .set("date_courrier", "2026-11-02")
        .set("référence", "REF-2026-015")
        .set("prénom", "marc")
        .set("montant", "12500")
        .set("durée", "24");
    v.types.insert("date_courrier".into(), VarType::Date);
    v.types.insert("montant".into(), VarType::Money);
    v.types.insert("durée".into(), VarType::Number);
    v.types.insert("adresse".into(), VarType::Long);
    v
}

/// Identifiant d'un type, pour les formulaires et le JSON.
fn type_id(t: VarType) -> &'static str {
    match t {
        VarType::Text => "text",
        VarType::Long => "long",
        VarType::Date => "date",
        VarType::Number => "number",
        VarType::Money => "money",
        VarType::Email => "email",
    }
}

fn type_depuis(id: &str) -> VarType {
    VarType::ALL
        .into_iter()
        .find(|t| type_id(*t) == id)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------------
// Rendu du document en vues
// ---------------------------------------------------------------------------------

/// La source, avec chaque `{{…}}` mis en évidence.
pub fn source_coloree(src: &str) -> Element {
    pre()
        .class("source")
        .child(code().children(parse(src).into_iter().map(|s| -> Node {
            match s {
                varium_core::Segment::Text(t) => text(t),
                varium_core::Segment::Var(v) => {
                    span().class("var").attr("title", v.name).text(v.raw).into()
                }
            }
        })))
}

/// Un morceau de texte mis en forme : valeurs surlignées, champs sans valeur signalés,
/// formules converties en Unicode.
fn morceau_doc(r: Run) -> Node {
    let st = r.style();
    let mut n: Node = match r {
        Run::Text { text: t, .. } => text(t),
        Run::Value { name, text: t, .. } => mark()
            .class("valeur")
            .attr("title", format!("{{{{{name}}}}}"))
            .text(t)
            .into(),
        Run::Missing { name, raw, .. } => span()
            .class("manquante")
            .attr("title", format!("« {name} » n'a pas encore de valeur"))
            .text(raw)
            .into(),
        Run::Math { tex: t, .. } => span()
            .class("math")
            .attr("title", t.clone())
            .text(tex::to_unicode(&t))
            .into(),
    };
    if st.bold {
        n = strong().child(n).into();
    }
    if st.italic {
        n = em().child(n).into();
    }
    if st.underline {
        n = u().child(n).into();
    }
    if st.strike {
        n = s().child(n).into();
    }
    n
}

fn morceaux(runs: Vec<Run>) -> Vec<Node> {
    runs.into_iter().map(morceau_doc).collect()
}

fn classe(a: Align) -> &'static str {
    match a {
        Align::Center => "c",
        Align::Right => "r",
        Align::Justify => "j",
        Align::Left => "g",
    }
}

fn paragraphe(para: Para) -> Node {
    match para {
        Para::Text { runs, .. } if runs.is_empty() => p().class("vide").child(br()).into(),
        Para::Text { align, runs } => p().class(classe(align)).children(morceaux(runs)).into(),
        Para::Heading { level, runs } => {
            let titre = match level {
                1 => h3(),
                2 => h4(),
                _ => h5(),
            };
            titre.class("titre").children(morceaux(runs)).into()
        }
        Para::Item {
            number,
            depth,
            runs,
        } => {
            let puce = match number {
                Some(n) => format!("{n}."),
                None => ["•", "◦", "▪"][depth as usize % 3].to_string(),
            };
            p().class("li")
                .style(format!("margin-left:{}em", 1.4 * f32::from(depth)))
                .child(span().class("puce").text(puce))
                .children(morceaux(runs))
                .into()
        }
        Para::Quote { runs } => blockquote().children(morceaux(runs)).into(),
        Para::Math { tex: t } => p()
            .class("math bloc")
            .attr("title", t.clone())
            .text(tex::to_unicode(t.trim()))
            .into(),
        Para::Table(t) => {
            let colonnes = t.columns();
            table()
                .children(t.rows.iter().enumerate().map(|(i, ligne)| {
                    tr().children((0..colonnes).map(|c| {
                        let cellule = if i == 0 && t.header { th() } else { td() };
                        cellule
                            .class(classe(t.align(c)))
                            .children(morceaux(ligne.get(c).cloned().unwrap_or_default()))
                    }))
                }))
                .into()
        }
        Para::Rule => hr().into(),
        Para::PageBreak => p().class("saut").text("saut de page").into(),
    }
}

/// Le document mis en forme par le moteur de Varium (titres, gras, listes, tableaux…).
pub fn document(src: &str, valeurs: &Values) -> Element {
    div().class("document").children(
        varium_core::document(src, valeurs)
            .into_iter()
            .map(paragraphe),
    )
}

// ---------------------------------------------------------------------------------
// Page et formulaire
// ---------------------------------------------------------------------------------

fn champ(nom: &str, valeurs: &Values) -> Element {
    let ty = valeurs.ty(nom);
    let valeur = valeurs.get(nom).to_string();
    let erreur = ty.validate(&valeur);
    let saisie = if ty == VarType::Long {
        textarea()
            .attr("name", format!("val.{nom}"))
            .attr("rows", "2")
            .text(valeur)
    } else {
        let html_type = match ty {
            VarType::Date => "date",
            VarType::Email => "email",
            _ => "text",
        };
        input()
            .attr("type", html_type)
            .attr("name", format!("val.{nom}"))
            .attr("value", valeur)
            .attr(
                "placeholder",
                if ty == VarType::Date {
                    "AAAA-MM-JJ"
                } else {
                    ""
                },
            )
    };
    let choix = select()
        .attr("name", format!("type.{nom}"))
        .attr("aria-label", format!("Type de {nom}"))
        .children(VarType::ALL.into_iter().map(|t| {
            let o = option().attr("value", type_id(t)).text(t.label());
            if t == ty {
                o.attr("selected", "selected")
            } else {
                o
            }
        }));
    div()
        .class("champ")
        .child(
            label()
                .child(code().text(format!("{{{{{nom}}}}}")))
                .child(saisie),
        )
        .child(choix)
        .child(erreur.map(|e| p().class("erreur").text(e)))
}

/// Les variables de `src`, leur nombre total d'occurrences et le nombre de variables
/// sans valeur.
fn compte(src: &str, valeurs: &Values) -> (Vec<varium_core::VarInfo>, usize, usize) {
    let vars = varium_core::variables(src);
    let total: usize = vars.iter().map(|v| v.count).sum();
    let manquantes = vars
        .iter()
        .filter(|v| !renseignee(&v.name, valeurs))
        .count();
    (vars, total, manquantes)
}

/// `true` quand la variable s'affiche avec une valeur (les filtres `default` comptent).
fn renseignee(nom: &str, valeurs: &Values) -> bool {
    !render_pieces(&format!("{{{{{nom}}}}}"), valeurs)
        .iter()
        .any(|p| matches!(p, Piece::Missing { .. }))
}

/// Le formulaire de la démo, envoyé au serveur.
fn formulaire(o: &Options, src: &str, valeurs: &Values) -> Element {
    let vars = varium_core::variables(src);
    form()
        .attr("method", "post")
        .attr("action", o.lien("/essayer/"))
        .class("stack")
        .child(
            label()
                .attr("for", "source")
                .child(strong().text("Modèle"))
                .child(
                    span()
                        .class("muted")
                        .text(" · écrivez {{une_variable}} n'importe où"),
                ),
        )
        .child(
            textarea()
                .id("source")
                .attr("name", "source")
                .attr("rows", "16")
                .attr("spellcheck", "false")
                .class("mono")
                .text(src.to_string()),
        )
        .child(h3().text(format!("Variables ({})", vars.len())))
        .child(if vars.is_empty() {
            p().class("muted").text(
                "Aucune variable pour l'instant : ajoutez {{prénom}} au modèle puis mettez à jour.",
            )
        } else {
            div()
                .class("champs")
                .children(vars.iter().map(|v| champ(&v.name, valeurs)))
        })
        .child(
            div()
                .class("row")
                .child(
                    button()
                        .attr("type", "submit")
                        .text("Mettre à jour le document"),
                )
                .child(
                    a().class("button ghost")
                        .href(o.lien("/essayer/"))
                        .text("Réinitialiser"),
                ),
        )
}

/// Le modèle en lecture seule, pour l'export statique : la source colorée et la valeur
/// de chaque variable.
fn modele_lu(src: &str, valeurs: &Values) -> Element {
    let vars = varium_core::variables(src);
    div()
        .class("stack")
        .child(
            div()
                .class("row entre")
                .child(strong().text("Modèle"))
                .child(span().class("muted").text("lecture seule")),
        )
        .child(source_coloree(src))
        .child(h3().text(format!("Variables ({})", vars.len())))
        .child(
            table()
                .child(
                    thead().child(
                        tr().child(th().text("Variable"))
                            .child(th().text("Type"))
                            .child(th().text("Valeur")),
                    ),
                )
                .child(tbody().children(vars.iter().map(|v| {
                    let valeur = valeurs.get(&v.name);
                    tr().child(td().child(code().text(format!("{{{{{}}}}}", v.name))))
                        .child(td().text(valeurs.ty(&v.name).label()))
                        .child(td().child(if valeur.is_empty() {
                            span().class("manquante").text("à remplir")
                        } else {
                            span().text(valeur.to_string())
                        }))
                }))),
        )
}

/// Le document rendu, avec ses compteurs et le texte brut.
fn resultat(src: &str, valeurs: &Values) -> Element {
    let (vars, total, manquantes) = compte(src, valeurs);
    div()
        .class("stack")
        .child(
            div()
                .class("row entre")
                .child(strong().text("Document"))
                .child(span().class("muted").text(format!(
                    "{} variable{} · {} occurrence{} · {} à remplir",
                    vars.len(),
                    if vars.len() > 1 { "s" } else { "" },
                    total,
                    if total > 1 { "s" } else { "" },
                    manquantes
                ))),
        )
        .child(document(src, valeurs))
        .child(
            details()
                .child(summary().text("Texte brut"))
                .child(pre().child(code().text(render(src, valeurs, varium_core::Missing::Keep)))),
        )
}

fn vue(req: &Request, src: &str, valeurs: &Values, alerte: Option<String>) -> Page {
    let o = Options::de(req);
    // Un hôte statique ne reçoit pas de formulaire : l'exemple est montré tel quel, et
    // l'éditeur complet est l'application web.
    let (explication, gauche, description) = if o.est_statique() {
        (
            "Le courrier d'exemple de Varium : le modèle et ses valeurs à gauche, le document calculé par le vrai moteur (varium-core) à droite.",
            modele_lu(src, valeurs),
            "Un modèle avec des {{variables}}, leurs valeurs et le document rendu par Varium. L'éditeur complet tourne dans le navigateur.",
        )
    } else {
        (
            "Modifiez le modèle ou les valeurs puis mettez le document à jour. Le rendu est calculé par le vrai moteur de Varium (varium-core), sur le serveur.",
            formulaire(o, src, valeurs),
            "Écrivez un modèle avec des {{variables}}, donnez-leur une valeur et voyez le document se mettre à jour : la démo en ligne de Varium.",
        )
    };
    let application = o.est_statique().then(|| {
        div()
            .class("card ouvrir")
            .child(
                p().child(strong().text("Pour écrire vos propres modèles, ouvrez l'application."))
                    .child(text(" L'éditeur complet de Varium tourne dans votre navigateur, compilé en WebAssembly : variables typées, filtres, formules, exports et sauvegarde automatique, sans rien installer.")),
            )
            .child(
                div()
                    .class("row")
                    .child(a().class("button").href(o.lien_application()).text("Ouvrir l'application"))
                    .child(a().class("button ghost").href(o.lien("/syntaxe/")).text("Lire la syntaxe")),
            )
    });

    let contenu = div()
        .child(
            section()
                .class("intro")
                .child(h1().text("Essayer Varium"))
                .child(p().class("muted").text(explication)),
        )
        .child(application)
        .child(alerte.map(|m| p().class("alerte").text(m)))
        .child(
            div()
                .class("demo")
                .child(div().class("card").child(gauche))
                .child(div().class("card").child(resultat(src, valeurs))),
        );

    layout::page(req, "Essayer", description, "/essayer/", contenu.into())
}

/// `GET /essayer` : la démo, ouverte sur le courrier d'exemple.
pub async fn page(req: Request) -> Page {
    vue(&req, EXEMPLE, &valeurs_exemple(), None)
}

/// `POST /essayer` : le formulaire renvoyé, rendu de nouveau.
pub async fn soumettre(req: Request) -> Result<Page> {
    let champs: BTreeMap<String, String> = req.form().await?;
    let mut src = champs
        .get("source")
        .cloned()
        .unwrap_or_default()
        .replace("\r\n", "\n");
    let mut alerte = None;
    if src.chars().count() > LIMITE {
        src = src.chars().take(LIMITE).collect();
        alerte = Some(format!("Le modèle a été tronqué à {LIMITE} caractères."));
    }
    let mut valeurs = Values::default();
    for (cle, valeur) in &champs {
        if let Some(nom) = cle.strip_prefix("val.") {
            valeurs.set(nom, valeur.replace("\r\n", "\n"));
        } else if let Some(nom) = cle.strip_prefix("type.") {
            valeurs.types.insert(nom.to_string(), type_depuis(valeur));
        }
    }
    Ok(vue(&req, &src, &valeurs, alerte))
}

// ---------------------------------------------------------------------------------
// API JSON
// ---------------------------------------------------------------------------------

/// Corps de `POST /api/rendu`.
#[derive(Deserialize)]
#[serde(crate = "paquets::serde")]
pub struct Demande {
    pub source: String,
    #[serde(default)]
    pub valeurs: BTreeMap<String, String>,
    /// `text`, `long`, `date`, `number`, `money` ou `email`.
    #[serde(default)]
    pub types: BTreeMap<String, String>,
}

#[derive(Serialize)]
#[serde(crate = "paquets::serde")]
pub struct Variable {
    pub nom: String,
    pub occurrences: usize,
    pub renseignee: bool,
}

/// Réponse de `POST /api/rendu`.
#[derive(Serialize)]
#[serde(crate = "paquets::serde")]
pub struct Reponse {
    pub texte: String,
    pub html: String,
    pub variables: Vec<Variable>,
}

/// `POST /api/rendu` : le moteur, en JSON.
pub async fn api(req: Request) -> Result<Json<Reponse>> {
    let d: Demande = req.json().await?;
    if d.source.chars().count() > LIMITE {
        return Err(Error::bad_request(format!(
            "source trop longue (plus de {LIMITE} caractères)"
        )));
    }
    let mut valeurs = Values::default();
    valeurs.values = d.valeurs;
    valeurs.types = d
        .types
        .iter()
        .map(|(k, v)| (k.clone(), type_depuis(v)))
        .collect();
    let variables = varium_core::variables(&d.source)
        .into_iter()
        .map(|v| Variable {
            renseignee: !valeurs.get(&v.name).is_empty(),
            nom: v.name,
            occurrences: v.count,
        })
        .collect();
    Ok(Json(Reponse {
        texte: render(&d.source, &valeurs, varium_core::Missing::Keep),
        html: varium_core::export::to_html("Document Varium", &d.source, &valeurs),
        variables,
    }))
}
