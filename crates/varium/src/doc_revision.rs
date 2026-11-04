//! Documents : commentaires et suivi des modifications (marques CriticMarkup
//! `{++…++}`, `{--…--}`, `{~~…~>…~~}`, `{==…==}{>>…<<}`), et badge « champs
//! possibles » de la barre d'état.
//!
//! - « Suivre les modifications » : la frappe devient des ajouts et des suppressions
//!   proposés, signés du nom de l'auteur (préférences) et de la date ;
//! - le panneau « Modifications » de l'inspecteur liste commentaires et
//!   modifications : Accepter / Refuser, Tout accepter / Tout refuser, aller au texte,
//!   commenter la sélection ;
//! - les exports montrent la version finale, ou les marques si on le demande.
//!
//! Sous-module de `app` : il prolonge `VariumApp`.

use egui::{Color32, RichText};
use varium_core::critic::{self, Genre, Marque, Vue};
use varium_core::detection;

use super::{Act, Dialog, VariumApp, accent, editor};
use crate::tr::tr;

/// L'état du module : badge de détection et lecture à voix haute.
#[derive(Default)]
pub(super) struct Etat {
    /// « 3 champs possibles » : nombre de valeurs repérées après un collage ou une
    /// ouverture (le badge ouvre « Détecter les champs »).
    pub badge: Option<usize>,
    pub voix: super::voix::Lecteur,
}

/// Le nom de l'auteur par défaut : celui de la session.
pub(super) fn auteur_systeme() -> String {
    let brut = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default();
    let mut cs = brut.trim().chars();
    match cs.next() {
        Some(c) if brut != "root" => c.to_uppercase().chain(cs).collect(),
        _ => String::new(),
    }
}

fn libelle(g: Genre) -> &'static str {
    match g {
        Genre::Ajout => tr("Ajout"),
        Genre::Suppression => tr("Suppression"),
        Genre::Substitution => tr("Remplacement"),
        Genre::Surlignage => tr("Commentaire"),
        Genre::Commentaire => tr("Commentaire"),
    }
}

fn couleur(g: Genre, sombre: bool) -> Color32 {
    match (g, sombre) {
        (Genre::Ajout, false) => Color32::from_rgb(0x1E, 0x7A, 0x3C),
        (Genre::Ajout, true) => Color32::from_rgb(0x81, 0xC9, 0x95),
        (Genre::Suppression, false) => Color32::from_rgb(0xB3, 0x26, 0x1E),
        (Genre::Suppression, true) => Color32::from_rgb(0xF2, 0x8B, 0x82),
        (Genre::Substitution, false) => Color32::from_rgb(0x8A, 0x5A, 0x00),
        (Genre::Substitution, true) => Color32::from_rgb(0xF2, 0xC0, 0x4B),
        (_, false) => Color32::from_rgb(0x1F, 0x5F, 0xBF),
        (_, true) => Color32::from_rgb(0x8A, 0xB4, 0xF8),
    }
}

/// Un extrait court, sur une ligne.
fn extrait(t: &str, n: usize) -> String {
    let t = t.replace('\n', " ⏎ ");
    if t.chars().count() > n {
        format!("{}…", t.chars().take(n).collect::<String>())
    } else {
        t
    }
}

impl VariumApp {
    /// Le nom signé sur les modifications et les commentaires.
    pub(super) fn auteur(&self) -> String {
        let a = self.prefs.auteur.trim();
        if a.is_empty() {
            auteur_systeme()
        } else {
            a.to_string()
        }
    }

    /// Ce que montrent les exports : la version finale, ou les marques.
    pub(super) fn vue_export(&self) -> Vue {
        if self.prefs.export_marques {
            Vue::Marquee
        } else {
            Vue::Finale
        }
    }

    /// Ce que montre la page à l'écran : les marques pendant le suivi.
    pub(super) fn vue_page(&self) -> Vue {
        if self.prefs.suivi || self.prefs.export_marques {
            Vue::Marquee
        } else {
            Vue::Finale
        }
    }

    /// Suivi des modifications : la frappe qui vient de changer `avant` devient un
    /// ajout ou une suppression proposés.
    pub(super) fn suivre_frappe(&mut self, avant: &str, curseur_avant: usize) {
        if !self.prefs.suivi || avant == self.source {
            return;
        }
        let date = super::today();
        if let Some((s, c)) =
            critic::suivre(avant, &self.source, curseur_avant, &self.auteur(), &date)
        {
            self.source = s;
            self.sel = (c, c);
            self.pending_sel = Some((c, c));
        }
    }

    pub(super) fn basculer_suivi(&mut self, ctx: &egui::Context) {
        self.prefs.suivi = !self.prefs.suivi;
        self.notify(
            ctx,
            if self.prefs.suivi {
                trf!(
                    "Suivi des modifications activé · signé « {auteur} »",
                    auteur = self.auteur()
                )
            } else {
                tr("Suivi des modifications désactivé").to_string()
            },
            false,
        );
    }

    /// Accepte (ou refuse) la marque qui commence à l'octet `debut`.
    fn decider(&mut self, debut: usize, accepter: bool) {
        let Some(m) = critic::marques(&self.source)
            .into_iter()
            .find(|m| m.plage.start == debut)
        else {
            return;
        };
        self.history.cut = true;
        self.source = if accepter {
            critic::accepter(&self.source, &m)
        } else {
            critic::refuser(&self.source, &m)
        };
        let c = editor::char_of(&self.source, m.plage.start);
        self.sel = (c, c);
        self.pending_sel = Some((c, c));
    }

    /// Tout accepter ou tout refuser.
    pub(super) fn decider_tout(&mut self, ctx: &egui::Context, accepter: bool) {
        let n = critic::marques(&self.source).len();
        if n == 0 {
            self.notify(ctx, tr("Aucune modification ni aucun commentaire"), false);
            return;
        }
        self.prendre_version(
            varium_core::versions::Motif::Avant,
            if accepter {
                "Tout accepter"
            } else {
                "Tout refuser"
            },
        );
        self.history.cut = true;
        self.source = if accepter {
            critic::tout_accepter(&self.source)
        } else {
            critic::tout_refuser(&self.source)
        };
        let n_car = self.source.chars().count();
        self.sel = (self.sel.0.min(n_car), self.sel.1.min(n_car));
        self.pending_sel = Some(self.sel);
        self.notify(
            ctx,
            if accepter {
                trf!("{n} marque(s) acceptée(s) · Ctrl+Z pour annuler", n)
            } else {
                trf!("{n} marque(s) refusée(s) · Ctrl+Z pour annuler", n)
            },
            false,
        );
    }

    /// Ajoute un commentaire sur la sélection (ou au curseur).
    pub(super) fn commenter(&mut self, texte: &str) {
        let (a, b) = (self.sel.0.min(self.sel.1), self.sel.0.max(self.sel.1));
        let (ba, bb) = (
            editor::byte_of(&self.source, a),
            editor::byte_of(&self.source, b),
        );
        let (s, note) =
            critic::commenter(&self.source, ba..bb, &self.auteur(), &super::today(), texte);
        self.history.cut = true;
        self.source = s;
        // Le curseur passe après le commentaire (`<<}`).
        let fin = editor::char_of(&self.source, note.end + 3);
        self.sel = (fin, fin);
        self.pending_sel = Some((fin, fin));
    }

    /// « Commenter la sélection » : le texte du commentaire.
    pub(super) fn commentaire_dialogue(&mut self, ui: &mut egui::Ui, texte: &mut String) -> bool {
        let mut garder = true;
        ui.heading(tr("Ajouter un commentaire"));
        let (a, b) = (self.sel.0.min(self.sel.1), self.sel.0.max(self.sel.1));
        if a != b {
            let (ba, bb) = (
                editor::byte_of(&self.source, a),
                editor::byte_of(&self.source, b),
            );
            ui.label(
                RichText::new(format!("« {} »", extrait(&self.source[ba..bb], 80)))
                    .italics()
                    .weak(),
            );
        }
        let r = ui.add(
            egui::TextEdit::multiline(texte)
                .hint_text(tr("Votre remarque…"))
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        r.request_focus();
        ui.label(
            RichText::new(trf!(
                "Signé « {auteur} » · le nom se change dans l'inspecteur, onglet Modifications",
                auteur = self.auteur()
            ))
            .small()
            .weak(),
        );
        ui.horizontal(|ui| {
            let ok = !texte.trim().is_empty();
            let entree = ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command);
            if ui
                .add_enabled(ok, egui::Button::new(tr("Commenter")))
                .clicked()
                || (ok && entree)
            {
                let t = texte.trim().to_string();
                self.commenter(&t);
                garder = false;
            }
            if ui.button(tr("Annuler")).clicked() {
                garder = false;
            }
        });
        garder
    }

    /// L'onglet « Modifications » de l'inspecteur.
    pub(super) fn revision_tab(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let sombre = ui.visuals().dark_mode;
        let mut suivi = self.prefs.suivi;
        if ui
            .checkbox(&mut suivi, RichText::new(tr("Suivre les modifications")).strong())
            .on_hover_text(tr(
                "Ce que vous tapez devient des ajouts et des suppressions proposés, à accepter ou refuser",
            ))
            .changed()
        {
            self.basculer_suivi(&ctx);
        }
        ui.horizontal(|ui| {
            ui.label(tr("Auteur"));
            let defaut = auteur_systeme();
            ui.add(
                egui::TextEdit::singleline(&mut self.prefs.auteur)
                    .hint_text(if defaut.is_empty() {
                        tr("Votre nom")
                    } else {
                        &defaut
                    })
                    .desired_width(f32::INFINITY),
            );
        });
        ui.checkbox(
            &mut self.prefs.export_marques,
            tr("Exporter avec les marques (sinon : version finale)"),
        );
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(tr("💬 Commenter la sélection"))
                .on_hover_text(tr("Ajoute un commentaire sur le texte sélectionné"))
                .clicked()
            {
                self.queue.push(Act::Commenter);
            }
        });
        let marques = critic::marques(&self.source);
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!marques.is_empty(), |ui| {
                if ui.button(tr("✔ Tout accepter")).clicked() {
                    self.queue.push(Act::ToutAccepter);
                }
                if ui.button(tr("✖ Tout refuser")).clicked() {
                    self.queue.push(Act::ToutRefuser);
                }
            });
        });
        ui.separator();
        if marques.is_empty() {
            ui.label(
                RichText::new(tr(
                    "Aucun commentaire ni aucune modification. Activez « Suivre les modifications » ou commentez une sélection.",
                ))
                .weak(),
            );
            return;
        }
        ui.label(
            RichText::new(trf!("{n} marque(s)", n = marques.len()))
                .small()
                .weak(),
        );
        let mut action: Option<(usize, Option<bool>)> = None;
        egui::ScrollArea::vertical()
            .id_salt("insp-revision")
            .auto_shrink(false)
            .show(ui, |ui| {
                for m in &marques {
                    self.carte_marque(ui, m, sombre, &mut action);
                }
            });
        match action {
            Some((debut, Some(accepter))) => self.decider(debut, accepter),
            Some((debut, None)) => {
                if let Some(m) = marques.iter().find(|m| m.plage.start == debut) {
                    let a = editor::char_of(&self.source, m.plage.start);
                    let b = editor::char_of(&self.source, m.plage.end);
                    self.sel = (a, b);
                    self.pending_sel = Some((a, b));
                }
            }
            None => {}
        }
    }

    fn carte_marque(
        &self,
        ui: &mut egui::Ui,
        m: &Marque,
        sombre: bool,
        action: &mut Option<(usize, Option<bool>)>,
    ) {
        let c = couleur(m.genre, sombre);
        let note = m.note();
        egui::Frame::group(ui.style())
            .inner_margin(6.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(libelle(m.genre)).strong().color(c));
                    let qui = [note.auteur.as_str(), note.date.as_str()]
                        .iter()
                        .filter(|x| !x.is_empty())
                        .copied()
                        .collect::<Vec<_>>()
                        .join(" · ");
                    if !qui.is_empty() {
                        ui.label(RichText::new(qui).small().weak());
                    }
                });
                match m.genre {
                    Genre::Ajout => {
                        ui.label(RichText::new(extrait(&m.texte, 90)).color(c).underline());
                    }
                    Genre::Suppression => {
                        ui.label(
                            RichText::new(extrait(&m.texte, 90))
                                .color(c)
                                .strikethrough(),
                        );
                    }
                    Genre::Substitution => {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(extrait(&m.texte, 40))
                                    .strikethrough()
                                    .color(couleur(Genre::Suppression, sombre)),
                            );
                            ui.label("→");
                            ui.label(
                                RichText::new(extrait(&m.nouveau, 40))
                                    .underline()
                                    .color(couleur(Genre::Ajout, sombre)),
                            );
                        });
                    }
                    Genre::Surlignage => {
                        ui.label(
                            RichText::new(format!("« {} »", extrait(&m.texte, 80)))
                                .italics()
                                .background_color(Color32::from_rgba_unmultiplied(
                                    0xF2, 0xC0, 0x4B, 70,
                                )),
                        );
                    }
                    Genre::Commentaire => {}
                }
                if !note.texte.is_empty() {
                    ui.label(RichText::new(format!("💬 {}", extrait(&note.texte, 200))));
                }
                ui.horizontal(|ui| {
                    let commentaire = matches!(m.genre, Genre::Commentaire | Genre::Surlignage);
                    if commentaire {
                        if ui
                            .small_button(tr("Résoudre"))
                            .on_hover_text(tr("Retire le commentaire, garde le texte"))
                            .clicked()
                        {
                            *action = Some((m.plage.start, Some(true)));
                        }
                    } else {
                        if ui.small_button(tr("✔ Accepter")).clicked() {
                            *action = Some((m.plage.start, Some(true)));
                        }
                        if ui.small_button(tr("✖ Refuser")).clicked() {
                            *action = Some((m.plage.start, Some(false)));
                        }
                    }
                    if ui
                        .small_button(tr("Aller au texte"))
                        .on_hover_text(tr("Sélectionne la marque dans le texte"))
                        .clicked()
                    {
                        *action = Some((m.plage.start, None));
                    }
                });
            });
    }

    // ---------------- Badge « champs possibles » ----------------

    /// Après un collage ou une ouverture : combien de champs pourraient être créés ?
    pub(super) fn signaler_champs(&mut self, texte: &str) {
        let n = if texte.len() > 2_000_000 {
            0
        } else {
            detection::detecter(texte).len()
        };
        self.etat.badge = (n > 0).then_some(n);
    }

    /// Le badge de la barre d'état.
    pub(super) fn badge_statut(&mut self, ui: &mut egui::Ui) {
        let Some(n) = self.etat.badge else { return };
        ui.separator();
        let texte = if n > 1 {
            trf!("✨ {n} champs possibles", n)
        } else {
            tr("✨ 1 champ possible").to_string()
        };
        let b = egui::Button::new(RichText::new(texte).color(accent(ui)).small())
            .corner_radius(8.0)
            .stroke(egui::Stroke::new(1.0, accent(ui)));
        let r = ui.add(b).on_hover_text(tr(
            "Des dates, montants, e-mails ou noms qui reviennent : Varium peut en faire des {{champs}}",
        ));
        if r.clicked() {
            self.etat.badge = None;
            self.queue.push(Act::DetectFields);
        }
        if ui.small_button("×").on_hover_text(tr("Masquer")).clicked() {
            self.etat.badge = None;
        }
    }

    pub(super) fn ouvrir_commentaire(&mut self) {
        self.dialog = Some(Dialog::Commentaire(String::new()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extraits_courts() {
        assert_eq!(extrait("abc", 5), "abc");
        assert_eq!(extrait("abcdef", 3), "abc…");
        assert_eq!(extrait("a\nb", 9), "a ⏎ b");
    }
}
