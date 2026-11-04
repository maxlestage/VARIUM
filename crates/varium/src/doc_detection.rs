//! Documents : « Détecter les champs… » — repère les dates, montants, e-mails,
//! téléphones et noms qui reviennent dans un document déjà écrit, et en fait des
//! `{{champs}}` en un clic. Le document reste identique : chaque champ reçoit sa
//! valeur d'origine.
//!
//! Sous-module de `app` : il prolonge `VariumApp`.

use egui::RichText;
use varium_core::detection::{self, Candidat, Genre};
use varium_core::is_valid_name;

use super::VariumApp;
use crate::tr::tr;

/// Ce que contient le dialogue : les propositions, cochées ou non, avec leur nom.
#[derive(Clone, Default)]
pub(super) struct DetectionSaisie {
    pub lignes: Vec<(Candidat, bool)>,
}

impl DetectionSaisie {
    pub(super) fn analyser(source: &str) -> Self {
        Self {
            lignes: detection::detecter(source)
                .into_iter()
                .map(|c| (c, true))
                .collect(),
        }
    }
}

fn genre_libelle(g: Genre) -> &'static str {
    match g {
        Genre::Date => tr("Date"),
        Genre::Montant => tr("Montant"),
        Genre::Email => tr("E-mail"),
        Genre::Telephone => tr("Téléphone"),
        Genre::Nom => tr("Nom"),
    }
}

impl VariumApp {
    pub(super) fn detection_dialogue(
        &mut self,
        ui: &mut egui::Ui,
        s: &mut DetectionSaisie,
    ) -> bool {
        let mut garder = true;
        ui.heading(tr("Détecter les champs"));
        ui.label(tr("Varium a repéré des valeurs qui peuvent devenir des champs {{…}} : on les remplit ensuite une seule fois, ou depuis un tableur, pour tous les documents du même type."));
        if s.lignes.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(tr("Rien à proposer : ni date, ni montant, ni e-mail, ni téléphone, ni nom répété.")).weak());
            return !ui.button(tr("Fermer")).clicked();
        }
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                for (c, coche) in &mut s.lignes {
                    ui.horizontal(|ui| {
                        ui.checkbox(coche, "");
                        ui.label(RichText::new(genre_libelle(c.genre)).small().weak());
                        ui.label(RichText::new(&c.texte).strong());
                        ui.label(
                            RichText::new(trf!("× {n}", n = c.occurrences))
                                .small()
                                .weak(),
                        );
                        ui.label("→");
                        ui.add(egui::TextEdit::singleline(&mut c.nom).desired_width(110.0));
                        if !is_valid_name(&c.nom) {
                            *coche = false;
                        }
                    });
                }
            });
        let choix: Vec<(String, String)> = s
            .lignes
            .iter()
            .filter(|(c, on)| *on && is_valid_name(&c.nom))
            .map(|(c, _)| (c.texte.clone(), c.nom.clone()))
            .collect();
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !choix.is_empty(),
                    egui::Button::new(trf!("Créer {n} champ(s)", n = choix.len())),
                )
                .clicked()
            {
                self.creer_champs_detectes(ui.ctx(), &choix);
                garder = false;
            }
            if ui.button(tr("Annuler")).clicked() {
                garder = false;
            }
        });
        garder
    }

    fn creer_champs_detectes(&mut self, ctx: &egui::Context, choix: &[(String, String)]) {
        self.prendre_version(varium_core::versions::Motif::Avant, "Détecter les champs");
        self.history.cut = true;
        self.source = detection::appliquer(&self.source, choix);
        // Chaque champ garde la valeur qu'avait le texte : le document ne change pas.
        for (texte, nom) in choix {
            self.values.set(nom.clone(), texte.clone());
        }
        self.notify(
            ctx,
            trf!(
                "{n} champ(s) créé(s) · Ctrl+Z pour annuler",
                n = choix.len()
            ),
            false,
        );
    }
}
