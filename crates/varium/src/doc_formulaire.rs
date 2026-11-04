//! Documents : « Remplir comme un formulaire » (les champs du document en questions,
//! avec l'aperçu de la page à côté) et le dialogue « Insertion → Code QR… ».
//!
//! Sous-module de `app` : il prolonge `VariumApp`.

use egui::{Color32, RichText, Sense, Vec2};
use varium_core::formulaire::{self, Question, Saisie};
use varium_core::qr::QrCode;

use super::{Act, TODAY, VariumApp, accent, err, value_editor};
use crate::tr::tr;

/// Ce que contient le dialogue « Code QR ».
#[derive(Clone, Default)]
pub(super) struct QrSaisie {
    pub texte: String,
    /// Un champ du document plutôt qu'un texte fixe.
    pub champ: Option<String>,
}

/// Dessine un code QR dans `cote` pixels.
fn dessiner_qr(ui: &mut egui::Ui, q: &QrCode, cote: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(cote), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, Color32::WHITE);
    let n = q.size() as f32 + 4.0;
    let m = cote / n;
    for (x, y, l) in q.segments() {
        let r = egui::Rect::from_min_size(
            rect.min + Vec2::new((x as f32 + 2.0) * m, (y as f32 + 2.0) * m),
            Vec2::new(l as f32 * m, m),
        );
        p.rect_filled(r.expand(0.25), 0.0, Color32::BLACK);
    }
}

impl VariumApp {
    /// Dialogue « Insertion → Code QR… ».
    pub(super) fn qr_dialogue(&mut self, ui: &mut egui::Ui, s: &mut QrSaisie) -> bool {
        let mut garder = true;
        ui.heading(tr("Insérer un code QR"));
        ui.label(tr("Un code QR qu'on lit avec l'appareil photo d'un téléphone : adresse d'un site, texte, numéro… Il s'imprime dans tous les exports (PDF, Word, LibreOffice, page web)."));
        let noms = self.variable_names();
        ui.horizontal(|ui| {
            ui.selectable_value(&mut s.champ, None, tr("Texte fixe"));
            if !noms.is_empty()
                && ui
                    .selectable_label(s.champ.is_some(), tr("Valeur d'un champ"))
                    .clicked()
                && s.champ.is_none()
            {
                s.champ = noms.first().cloned();
            }
        });
        let contenu = match &mut s.champ {
            None => {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut s.texte)
                        .hint_text("https://exemple.fr")
                        .desired_width(f32::INFINITY),
                );
                r.request_focus();
                s.texte.clone()
            }
            Some(nom) => {
                egui::ComboBox::from_id_salt("qr-champ")
                    .selected_text(nom.as_str())
                    .show_ui(ui, |ui| {
                        for n in &noms {
                            ui.selectable_value(nom, n.clone(), n);
                        }
                    });
                self.effective().get(nom).to_string()
            }
        };
        let interdit = s.champ.is_none() && (s.texte.contains("}}") || s.texte.contains('\n'));
        ui.horizontal(|ui| match QrCode::encode(&contenu) {
            Ok(q) if !contenu.is_empty() => {
                dessiner_qr(ui, &q, 132.0);
                ui.label(
                    RichText::new(trf!(
                        "Version {v} · {n} × {n} modules · correction M (15 %)",
                        v = q.version(),
                        n = q.size()
                    ))
                    .small()
                    .weak(),
                );
            }
            Ok(_) => {
                ui.label(RichText::new(tr("L'aperçu apparaît ici.")).weak());
            }
            Err(e) => {
                ui.label(RichText::new(e.to_string()).color(err(ui)));
            }
        });
        if interdit {
            ui.label(
                RichText::new(tr(
                    "Le texte ne peut contenir ni « }} » ni retour à la ligne.",
                ))
                .small()
                .color(err(ui)),
            );
        }
        let syntaxe = match &s.champ {
            Some(n) => format!("{{{{{n} | qr}}}}"),
            None => format!("{{{{qr:{}}}}}", s.texte.trim()),
        };
        ui.label(
            RichText::new(trf!("Dans le texte : {code}", code = syntaxe.as_str()))
                .small()
                .monospace(),
        );
        ui.horizontal(|ui| {
            let ok = !interdit && (s.champ.is_some() || !s.texte.trim().is_empty());
            if ui
                .add_enabled(ok, egui::Button::new(tr("Insérer")))
                .clicked()
            {
                self.insert_block(&syntaxe);
                garder = false;
            }
            if ui.button(tr("Annuler")).clicked() {
                garder = false;
            }
        });
        garder
    }

    /// Une question du formulaire ; renvoie `true` si la réponse est à vérifier.
    fn question_ui(&mut self, ui: &mut egui::Ui, q: &Question) -> bool {
        let depuis_publipostage = self
            .merge
            .as_ref()
            .is_some_and(|m| m.sheet.column_for(&q.nom).is_some());
        let mut erreur = false;
        egui::Frame::group(ui.style())
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&q.libelle).strong().size(15.0));
                    if q.occurrences > 1 {
                        ui.label(
                            RichText::new(trf!("utilisé {n} fois", n = q.occurrences))
                                .small()
                                .weak(),
                        );
                    }
                });
                if depuis_publipostage {
                    let v = self.effective().get(&q.nom).to_string();
                    ui.label(RichText::new(format!("📄 {v}")).italics())
                        .on_hover_text(tr("Valeur lue dans le fichier du publipostage"));
                    return;
                }
                let valeur = self.values.values.entry(q.nom.clone()).or_default();
                match &q.saisie {
                    Saisie::Case => {
                        let mut oui = varium_core::conditions::est_vrai(valeur);
                        if ui.checkbox(&mut oui, tr("Oui")).changed() {
                            *valeur = if oui {
                                tr("oui").to_string()
                            } else {
                                String::new()
                            };
                        }
                    }
                    Saisie::Choix(choix) => {
                        ui.horizontal_wrapped(|ui| {
                            for c in choix {
                                let on = valeur.trim().to_lowercase() == c.to_lowercase();
                                if ui.selectable_label(on, c).clicked() {
                                    *valeur = if on { String::new() } else { c.clone() };
                                }
                            }
                        });
                        ui.add(
                            egui::TextEdit::singleline(valeur)
                                .hint_text(tr("ou une autre valeur…"))
                                .desired_width(f32::INFINITY),
                        );
                    }
                    _ => {
                        let ty = self.values.types.get(&q.nom).copied().unwrap_or_default();
                        let valeur = self.values.values.entry(q.nom.clone()).or_default();
                        value_editor(ui, &q.nom, ty, valeur);
                    }
                }
                let valeur = self.values.get(&q.nom).to_string();
                if let Some(msg) = formulaire::erreur(q, &self.values, &valeur) {
                    erreur = true;
                    ui.label(RichText::new(msg).small().color(err(ui)));
                } else if q.nom == TODAY && valeur.is_empty() {
                    ui.label(
                        RichText::new(tr("date du jour, automatique"))
                            .small()
                            .weak(),
                    );
                }
            });
        erreur
    }

    /// Le mode « Remplir comme un formulaire » : les questions à gauche, la page à
    /// droite, mise à jour à chaque frappe.
    pub(super) fn formulaire_ui(&mut self, ui: &mut egui::Ui) {
        let questions = formulaire::questions(&self.source, &self.values);
        let eff = self.effective();
        let remplies = formulaire::remplies(&questions, &eff);
        let largeur = ui.available_width();
        egui::Panel::top("form-haut").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(tr("📝 Remplir le formulaire"))
                        .strong()
                        .size(16.0),
                );
                ui.label(RichText::new(self.title.trim()).weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(RichText::new(tr("Terminer")).strong())
                        .on_hover_text(tr("Revenir au texte (Échap)"))
                        .clicked()
                    {
                        self.queue.push(Act::FillForm);
                    }
                    if !questions.is_empty() {
                        ui.add(
                            egui::ProgressBar::new(remplies as f32 / questions.len() as f32)
                                .desired_width(160.0)
                                .text(trf!(
                                    "{n} / {total} renseignés",
                                    n = remplies,
                                    total = questions.len()
                                )),
                        );
                    }
                });
            });
            ui.add_space(4.0);
        });
        if self.dialog.is_none() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.queue.push(Act::FillForm);
        }
        egui::Panel::left("form-questions")
            .resizable(true)
            .default_size((largeur * 0.42).clamp(320.0, 560.0))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("form-scroll")
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        ui.add_space(8.0);
                        if questions.is_empty() {
                            ui.label(tr("Ce document n'a pas de champ à remplir. Écrivez {{nom}} dans le texte pour créer une question."));
                            return;
                        }
                        ui.label(
                            RichText::new(tr("Répondez aux questions : la page se remplit à mesure. Tab passe à la question suivante."))
                                .small()
                                .weak(),
                        );
                        ui.add_space(6.0);
                        let mut erreurs = 0;
                        for q in &questions {
                            if self.question_ui(ui, q) {
                                erreurs += 1;
                            }
                            ui.add_space(4.0);
                        }
                        // Focus sur le premier champ de saisie à l'ouverture.
                        if std::mem::take(&mut self.outils.formulaire_focus) {
                            ui.ctx().memory_mut(|m| m.move_focus(egui::FocusDirection::Next));
                        }
                        ui.add_space(6.0);
                        if erreurs > 0 {
                            ui.label(
                                RichText::new(trf!("{n} réponse(s) à vérifier", n = erreurs))
                                    .color(err(ui)),
                            );
                        } else if remplies == questions.len() {
                            ui.label(
                                RichText::new(tr("✔ Tout est rempli. Exportez le document (Fichier → Exporter) ou terminez."))
                                    .color(accent(ui)),
                            );
                        }
                    });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.page_pane(ui));
    }
}
