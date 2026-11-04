//! Documents : « Prêt à envoyer… » — un dernier contrôle (champs vides, modifications
//! ou commentaires jamais traités, remarques de révision) puis l'export en PDF sous un
//! nom de fichier propre, sans accents ni espaces.
//!
//! Sous-module de `app` : il prolonge `VariumApp`.

use egui::{Color32, RichText};
use varium_core::envoi::{self, Point};

use super::{Act, Dialog, Format, VariumApp};
use crate::tr::tr;

impl VariumApp {
    /// Contrôle le document et ouvre le dialogue.
    pub(super) fn ouvrir_envoi(&mut self) {
        let valeurs = self.effective();
        let points = envoi::controler(&self.source, &valeurs, self.page.langue);
        self.dialog = Some(Dialog::Envoi(points));
    }

    /// Le nom de fichier proposé pour l'envoi (publipostage compris).
    pub(super) fn nom_envoi(&self, extension: &str) -> String {
        let titre = if self.merge.is_some() {
            format!("{} publipostage", self.title)
        } else {
            self.title.clone()
        };
        envoi::nom_de_fichier(&titre, extension)
    }

    pub(super) fn envoi_dialogue(&mut self, ui: &mut egui::Ui, points: &[Point]) -> bool {
        let mut garder = true;
        ui.heading(tr("Prêt à envoyer ?"));
        ui.add_space(4.0);
        if points.is_empty() {
            ui.label(
                RichText::new(tr("✔ Rien à signaler : le document est prêt."))
                    .color(Color32::from_rgb(40, 150, 70)),
            );
        } else {
            ui.label(tr(
                "Avant d'envoyer, voici ce que votre destinataire pourrait remarquer :",
            ));
            ui.add_space(4.0);
            for p in points {
                let (icone, couleur) = if p.grave() {
                    ("⚠", Color32::from_rgb(200, 120, 20))
                } else {
                    ("ℹ", Color32::GRAY)
                };
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(icone).color(couleur));
                    ui.label(point_texte(p));
                });
            }
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(trf!("Fichier : {nom}", nom = self.nom_envoi("pdf")))
                .small()
                .weak(),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let libelle = if points.iter().any(Point::grave) {
                tr("Exporter en PDF quand même")
            } else {
                tr("Exporter en PDF")
            };
            if ui.button(libelle).clicked() {
                self.queue.push(Act::ExportPropre(Format::Pdf));
                garder = false;
            }
            if ui.button(tr("Fermer")).clicked() {
                garder = false;
            }
        });
        garder
    }
}

fn point_texte(p: &Point) -> String {
    match p {
        Point::DocumentVide => tr("Le document est vide.").to_string(),
        Point::ChampVide(nom) => trf!(
            "Le champ {champ} n'est pas rempli.",
            champ = format!("{{{{{nom}}}}}")
        ),
        Point::ModificationsEnAttente(n) => trf!(
            "{n} modification(s) proposée(s) attendent d'être acceptées ou refusées.",
            n = n
        ),
        Point::CommentairesEnAttente(n) => {
            trf!(
                "{n} commentaire(s) de relecture restent dans le texte.",
                n = n
            )
        }
        Point::Remarques(n) => trf!(
            "{n} remarque(s) de révision (orthographe, espaces, majuscules…).",
            n = n
        ),
    }
}
