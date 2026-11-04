//! Documents : modèles intelligents.
//!
//! - « Créer un modèle à partir de ce document » : après la détection des champs, le
//!   document devient un modèle (champs vides, une question par champ) rangé dans la
//!   galerie de l'utilisateur (accueil, « Nouveau à partir d'un modèle ») ;
//! - « Nouveau depuis un modèle » pose les questions une à une (progression,
//!   Précédent / Suivant, saisie adaptée au type), puis écrit le document rempli.
//!
//! Sous-module de `app` : il prolonge `VariumApp`.

use std::collections::BTreeMap;

use egui::{Key, RichText};
use varium_core::detection::{self, Genre};
use varium_core::formulaire::Saisie;
use varium_core::modele::{self, Modele, Question};
use varium_core::{Project, VarType, is_valid_name};

use super::{Dialog, TODAY, VariumApp, err, value_editor};
use crate::templates;
use crate::tr::tr;

// ---------------------------------------------------------------------------------
// L'assistant « Nouveau depuis un modèle »
// ---------------------------------------------------------------------------------

/// L'assistant en cours : le modèle, ses questions et les réponses.
pub(super) struct Assistant {
    modele: Modele,
    questions: Vec<(Question, Saisie)>,
    index: usize,
    reponses: BTreeMap<String, String>,
    /// Mettre le focus sur la saisie (nouvelle question).
    focus: bool,
}

impl Assistant {
    pub(super) fn new(modele: Modele) -> Self {
        let questions = modele.questions_saisies();
        Assistant {
            modele,
            questions,
            index: 0,
            reponses: BTreeMap::new(),
            focus: true,
        }
    }
}

fn type_de_saisie(s: &Saisie, ty: VarType) -> VarType {
    match s {
        Saisie::Date => VarType::Date,
        Saisie::Montant => VarType::Money,
        Saisie::Nombre => VarType::Number,
        Saisie::Email => VarType::Email,
        Saisie::TexteLong => VarType::Long,
        _ => ty,
    }
}

impl VariumApp {
    /// Ouvre l'assistant d'un modèle intelligent (ou, sans question, le document).
    pub(super) fn ouvrir_modele_intelligent(&mut self, ctx: &egui::Context, m: Modele) {
        let a = Assistant::new(m);
        if a.questions.is_empty() {
            let nom = a.modele.nom.clone();
            self.load(a.modele.remplir(&BTreeMap::new()));
            self.notify(ctx, trf!("Modèle « {name} » ouvert", name = nom), false);
        } else {
            self.dialog = Some(Dialog::Assistant(Box::new(a)));
        }
    }

    pub(super) fn assistant_dialogue(&mut self, ui: &mut egui::Ui, a: &mut Assistant) -> bool {
        let n = a.questions.len();
        a.index = a.index.min(n.saturating_sub(1));
        let fin = a.index + 1 == n;
        ui.horizontal(|ui| {
            ui.label(RichText::new(&a.modele.icone).size(22.0));
            ui.heading(&a.modele.nom);
        });
        if !a.modele.description.is_empty() {
            ui.label(RichText::new(&a.modele.description).weak());
        }
        ui.add_space(6.0);
        ui.add(
            egui::ProgressBar::new((a.index + 1) as f32 / n.max(1) as f32).text(trf!(
                "Question {i} sur {n}",
                i = a.index + 1,
                n
            )),
        );
        ui.add_space(10.0);
        let Some((q, saisie)) = a.questions.get(a.index).cloned() else {
            return false;
        };
        ui.label(RichText::new(&q.question).size(18.0).strong());
        if !q.aide.is_empty() {
            ui.label(RichText::new(&q.aide).small().weak());
        }
        ui.add_space(4.0);
        let valeurs = a.modele.projet.values();
        let ty = type_de_saisie(&saisie, valeurs.ty(&q.champ));
        let valeur = a.reponses.entry(q.champ.clone()).or_default();
        let avant_focus = a.focus;
        match &saisie {
            Saisie::Case => {
                let mut oui = !valeur.trim().is_empty();
                if ui.checkbox(&mut oui, tr("Oui")).changed() {
                    *valeur = if oui {
                        tr("oui").to_string()
                    } else {
                        String::new()
                    };
                }
            }
            Saisie::Choix(options) => {
                ui.horizontal_wrapped(|ui| {
                    for o in options {
                        if ui.selectable_label(valeur == o, o).clicked() {
                            *valeur = o.clone();
                        }
                    }
                });
                let r = ui.add(
                    egui::TextEdit::singleline(valeur)
                        .hint_text(tr("ou une autre réponse…"))
                        .desired_width(f32::INFINITY),
                );
                if avant_focus {
                    r.request_focus();
                }
            }
            Saisie::TexteLong => {
                let r = ui.add(
                    egui::TextEdit::multiline(valeur)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY),
                );
                if avant_focus {
                    r.request_focus();
                }
            }
            _ if ty == VarType::Date => value_editor(ui, &q.champ, ty, valeur),
            _ => {
                let hint = match ty {
                    VarType::Number => tr("ex. 1234,5"),
                    VarType::Money => tr("ex. 12500"),
                    VarType::Email => tr("nom@exemple.fr"),
                    _ => tr("Votre réponse…"),
                };
                let r = ui.add(
                    egui::TextEdit::singleline(valeur)
                        .hint_text(hint)
                        .desired_width(f32::INFINITY),
                );
                if avant_focus {
                    r.request_focus();
                }
            }
        }
        a.focus = false;
        let erreur = if valeur.trim().is_empty() || saisie == Saisie::Case {
            None
        } else {
            ty.validate(valeur)
        };
        if let Some(e) = erreur {
            ui.label(RichText::new(tr(e)).small().color(err(ui)));
        }
        ui.label(
            RichText::new(tr("Laissez vide pour remplir plus tard."))
                .small()
                .weak(),
        );
        ui.add_space(8.0);
        let mut garder = true;
        let entree = ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift)
            && saisie != Saisie::TexteLong;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(a.index > 0, egui::Button::new(tr("◀ Précédent")))
                .clicked()
            {
                a.index -= 1;
                a.focus = true;
            }
            let suivant = if fin {
                tr("Créer le document ✔")
            } else {
                tr("Suivant ▶")
            };
            if ui.button(suivant).clicked() || (entree && erreur.is_none()) {
                if fin {
                    let nom = a.modele.nom.clone();
                    let mut p = a.modele.remplir(&a.reponses);
                    p.title = nom.clone();
                    self.load(p);
                    let ctx = ui.ctx().clone();
                    self.notify(
                        &ctx,
                        trf!(
                            "Document créé depuis « {name} » · Ctrl+Z pour revenir",
                            name = nom
                        ),
                        false,
                    );
                    garder = false;
                } else {
                    a.index += 1;
                    a.focus = true;
                }
            }
            if ui.button(tr("Annuler")).clicked() {
                garder = false;
            }
        });
        garder
    }

    // -----------------------------------------------------------------------------
    // « Créer un modèle à partir de ce document »
    // -----------------------------------------------------------------------------

    pub(super) fn ouvrir_decoupe(&mut self) {
        let lang = crate::tr::lang();
        let mut lignes = Vec::new();
        for v in varium_core::variables(&self.source) {
            if v.name == TODAY {
                continue;
            }
            let ty = self.values.ty(&v.name);
            lignes.push(Ligne {
                texte: None,
                nom: v.name.clone(),
                ty,
                coche: true,
                question: question_auto(&v.name, ty, lang),
                auto: true,
            });
        }
        for c in detection::detecter(&self.source) {
            let ty = modele::type_de(c.genre);
            lignes.push(Ligne {
                question: question_auto(&c.nom, ty, lang),
                texte: Some(c.texte),
                nom: c.nom,
                ty,
                // Les noms qui reviennent sont proposés, mais pas cochés d'office.
                coche: c.genre != Genre::Nom || c.occurrences > 2,
                auto: true,
            });
        }
        let nom = if self.title.trim().is_empty() {
            tr("Mon modèle").to_string()
        } else {
            self.title.trim().to_string()
        };
        self.dialog = Some(Dialog::Decouper(Box::new(Decoupe {
            nom,
            description: String::new(),
            lignes,
        })));
    }

    pub(super) fn decoupe_dialogue(&mut self, ui: &mut egui::Ui, d: &mut Decoupe) -> bool {
        let lang = crate::tr::lang();
        let mut garder = true;
        ui.heading(tr("Créer un modèle à partir de ce document"));
        ui.label(tr("Les valeurs choisies deviennent des champs vides, et chacun reçoit une question. « Nouveau à partir d'un modèle » posera ces questions une à une pour écrire un nouveau document."));
        ui.add_space(4.0);
        egui::Grid::new("decoupe-nom")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label(tr("Nom du modèle"));
                ui.add(egui::TextEdit::singleline(&mut d.nom).desired_width(280.0));
                ui.end_row();
                ui.label(tr("Description"));
                ui.add(
                    egui::TextEdit::singleline(&mut d.description)
                        .hint_text(tr("facultative"))
                        .desired_width(280.0),
                );
                ui.end_row();
            });
        ui.add_space(6.0);
        if d.lignes.is_empty() {
            ui.label(
                RichText::new(tr(
                    "Ce document n'a pas encore de champ, et Varium n'y a repéré ni date, ni montant, ni e-mail, ni nom répété. Écrivez des {{champs}} (F2) puis recommencez.",
                ))
                .weak(),
            );
        } else {
            ui.label(RichText::new(tr("Champs et questions")).strong());
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .show(ui, |ui| {
                    for l in &mut d.lignes {
                        ui.horizontal(|ui| {
                            ui.add_enabled_ui(l.texte.is_some(), |ui| {
                                ui.checkbox(&mut l.coche, "");
                            });
                            if let Some(t) = &l.texte {
                                ui.label(RichText::new(t).strong());
                                ui.label("→");
                                let r = ui.add(
                                    egui::TextEdit::singleline(&mut l.nom).desired_width(110.0),
                                );
                                if r.changed() && l.auto {
                                    l.question = question_auto(&l.nom, l.ty, lang);
                                }
                                if !is_valid_name(&l.nom) {
                                    l.coche = false;
                                }
                            } else {
                                ui.label(RichText::new(format!("{{{{{}}}}}", l.nom)).monospace());
                            }
                        });
                        ui.add_enabled_ui(l.coche, |ui| {
                            ui.horizontal(|ui| {
                                ui.add_space(26.0);
                                let r = ui.add(
                                    egui::TextEdit::singleline(&mut l.question)
                                        .desired_width(f32::INFINITY),
                                );
                                if r.changed() {
                                    l.auto = false;
                                }
                            });
                        });
                        ui.add_space(2.0);
                    }
                });
        }
        let nb = d.lignes.iter().filter(|l| l.coche).count();
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let ok = !d.nom.trim().is_empty() && nb > 0;
            if ui
                .add_enabled(
                    ok,
                    egui::Button::new(trf!("Enregistrer le modèle ({n} questions)", n = nb)),
                )
                .clicked()
            {
                let ctx = ui.ctx().clone();
                self.enregistrer_decoupe(&ctx, d);
                garder = false;
            }
            if ui.button(tr("Annuler")).clicked() {
                garder = false;
            }
        });
        garder
    }

    fn enregistrer_decoupe(&mut self, ctx: &egui::Context, d: &Decoupe) {
        let choix: Vec<(String, String)> = d
            .lignes
            .iter()
            .filter(|l| l.coche && is_valid_name(&l.nom))
            .filter_map(|l| Some((l.texte.clone()?, l.nom.clone())))
            .collect();
        let source = detection::appliquer(&self.source, &choix);
        let mut valeurs = super::Values {
            values: BTreeMap::new(),
            types: self.values.types.clone(),
        };
        for l in d.lignes.iter().filter(|l| l.coche && l.ty != VarType::Text) {
            valeurs.types.insert(l.nom.clone(), l.ty);
        }
        let mut p = Project::new(d.nom.trim(), source, &valeurs);
        p.page = self.page.clone();
        let mut m = Modele::depuis_projet(&d.nom, &d.description, &p, crate::tr::lang());
        for q in &mut m.questions {
            if let Some(l) = d
                .lignes
                .iter()
                .find(|l| l.nom == q.champ && !l.question.trim().is_empty())
            {
                q.question = l.question.trim().to_string();
            }
        }
        match templates::enregistrer_modele(&templates::dossier_modeles(), &m) {
            Ok(_) => self.notify(
                ctx,
                trf!(
                    "Modèle « {name} » ajouté à vos modèles : accueil et Fichier → Nouveau à partir d'un modèle",
                    name = m.nom
                ),
                false,
            ),
            Err(e) => self.notify(ctx, trf!("Écriture impossible : {e}", e), true),
        }
    }
}

/// Une ligne du dialogue « Créer un modèle » : un champ existant (`texte` vide) ou une
/// valeur détectée qui deviendra un champ.
pub(super) struct Ligne {
    texte: Option<String>,
    nom: String,
    ty: VarType,
    coche: bool,
    question: String,
    /// La question suit encore le nom (pas modifiée à la main).
    auto: bool,
}

pub(super) struct Decoupe {
    nom: String,
    description: String,
    lignes: Vec<Ligne>,
}

fn question_auto(nom: &str, ty: VarType, lang: varium_core::Lang) -> String {
    let saisie = match ty {
        VarType::Date => Saisie::Date,
        VarType::Money => Saisie::Montant,
        VarType::Number => Saisie::Nombre,
        VarType::Email => Saisie::Email,
        VarType::Long => Saisie::TexteLong,
        _ => Saisie::Texte,
    };
    modele::question_pour(nom, &saisie, lang)
}

impl VariumApp {
    /// La section « Modèles intelligents » du dialogue « Nouveau à partir d'un
    /// modèle » : lettres types et modèles de l'utilisateur (supprimables).
    pub(super) fn modeles_intelligents(&mut self, ui: &mut egui::Ui, choisi: &mut Option<Modele>) {
        let liste = templates::intelligents();
        ui.label(RichText::new(tr("Lettres types et modèles intelligents")).strong());
        ui.label(
            RichText::new(tr(
                "Varium pose les questions, puis écrit le document. Vos modèles se créent avec Fichier → Créer un modèle à partir de ce document.",
            ))
            .small()
            .weak(),
        );
        let mut supprimer = None;
        egui::ScrollArea::vertical()
            .id_salt("modeles-intelligents")
            .max_height(180.0)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (k, i) in liste.iter().enumerate() {
                        let r = ui
                            .add(
                                egui::Button::new(format!("{}  {}", i.modele.icone, i.modele.nom))
                                    .min_size(egui::vec2(150.0, 30.0)),
                            )
                            .on_hover_text(if i.modele.description.is_empty() {
                                trf!("{n} questions", n = i.modele.questions.len())
                            } else {
                                format!(
                                    "{} · {}",
                                    i.modele.description,
                                    trf!("{n} questions", n = i.modele.questions.len())
                                )
                            });
                        if r.clicked() {
                            *choisi = Some(i.modele.clone());
                        }
                        if let Some(c) = &i.chemin {
                            r.context_menu(|ui| {
                                if ui.button(tr("Supprimer ce modèle")).clicked() {
                                    supprimer = Some((k, c.clone()));
                                    ui.close();
                                }
                            });
                        }
                    }
                });
            });
        if let Some((_, c)) = supprimer {
            let ctx = ui.ctx().clone();
            match templates::supprimer_modele(&c) {
                Ok(()) => self.notify(&ctx, tr("Modèle supprimé"), false),
                Err(e) => self.notify(&ctx, trf!("Écriture impossible : {e}", e), true),
            }
        }
    }
}
