//! Documents : écrire sans se soucier du reste. Mode concentration (paragraphe
//! courant mis en avant, défilement « machine à écrire »), objectif de mots, mots de
//! la session, temps de lecture et de parole, lisibilité, correction automatique
//! pendant la frappe et collage intelligent.
//!
//! Sous-module de `app` : il prolonge `VariumApp`.

use egui::{Color32, Pos2, RichText, Sense, Stroke, Vec2};
use varium_core::lisibilite::{self, Niveau};
use varium_core::{Lang, autocorrect, collage};

use super::{Act, Dialog, VariumApp, accent, editor, err};
use crate::tr::tr;

/// L'état des outils d'écriture du module Documents.
#[derive(Default)]
pub(super) struct Outils {
    /// Mode concentration actif.
    pub concentration: bool,
    /// Mode « Remplir comme un formulaire » actif.
    pub formulaire: bool,
    /// Le formulaire vient d'être ouvert : focus sur la première question.
    pub formulaire_focus: bool,
    /// Mots au début de la session (ouverture du document).
    pub mots_depart: Option<usize>,
    /// Position du curseur à la dernière image (défilement « machine à écrire »).
    pub dernier_curseur: Option<usize>,
    pub historique: super::historique::Etat,
}

/// Durée lisible : « 45 s », « 3 min », « 1 h 05 ».
pub(super) fn duree(secondes: u32) -> String {
    match secondes {
        0..60 => trf!("{n} s", n = secondes),
        60..3600 => trf!("{n} min", n = secondes.div_ceil(60)),
        _ => trf!(
            "{h} h {m}",
            h = secondes / 3600,
            m = format!("{:02}", (secondes % 3600) / 60)
        ),
    }
}

fn niveau_texte(n: Niveau) -> (&'static str, &'static str) {
    match n {
        Niveau::TresFacile => (
            tr("Très facile"),
            tr("Phrases courtes et mots simples : compris de tous, dès l'école primaire."),
        ),
        Niveau::Facile => (
            tr("Facile"),
            tr("Se lit sans effort : convient au grand public."),
        ),
        Niveau::Standard => (
            tr("Standard"),
            tr("Comme un article de journal : clair pour la plupart des lecteurs."),
        ),
        Niveau::Difficile => (
            tr("Difficile"),
            tr(
                "Phrases longues ou mots savants : demande de l'attention. Raccourcissez les phrases pour toucher plus de monde.",
            ),
        ),
        Niveau::TresDifficile => (
            tr("Très difficile"),
            tr(
                "Texte de spécialiste (juridique, scientifique). Coupez les longues phrases et préférez les mots courts.",
            ),
        ),
    }
}

/// Dessine un anneau de progression (0 à 1) de `diametre` pixels.
pub(super) fn anneau(ui: &mut egui::Ui, ratio: f32, diametre: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(diametre), Sense::hover());
    let p = ui.painter();
    let c = rect.center();
    let r = diametre / 2.0 - 2.0;
    let fond = ui.visuals().widgets.noninteractive.bg_stroke.color;
    p.circle_stroke(c, r, Stroke::new(3.0, fond));
    let ratio = ratio.clamp(0.0, 1.0);
    let couleur = if ratio >= 1.0 {
        Color32::from_rgb(0x2E, 0x9E, 0x4F)
    } else {
        accent(ui)
    };
    if ratio > 0.0 {
        let n = (64.0 * ratio).ceil().max(2.0) as usize;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let a = -std::f32::consts::FRAC_PI_2
                    + std::f32::consts::TAU * ratio * i as f32 / n as f32;
                c + Vec2::new(a.cos(), a.sin()) * r
            })
            .collect();
        p.add(egui::Shape::line(pts, Stroke::new(3.0, couleur)));
    }
    resp
}

impl VariumApp {
    /// Mots du document (valeurs des champs comprises).
    pub(super) fn mots(&self) -> usize {
        varium_core::stats(&varium_core::document(&self.source, &self.effective())).words
    }

    /// Le texte affiché du document, pour les mesures d'écriture.
    fn texte_affiche(&self) -> String {
        varium_core::document(&self.source, &self.effective())
            .iter()
            .map(varium_core::Para::plain)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Barre d'état : objectif (anneau), mots de la session, temps de lecture.
    pub(super) fn objectif_statut(&mut self, ui: &mut egui::Ui, mots: usize) {
        let depart = *self.outils.mots_depart.get_or_insert(mots);
        let session = mots as i64 - depart as i64;
        if let Some(obj) = self.objectif.filter(|o| *o > 0) {
            let ratio = mots as f32 / obj as f32;
            let r = anneau(ui, ratio, 16.0).on_hover_text(trf!(
                "Objectif : {mots} / {obj} mots ({pct} %)",
                mots,
                obj,
                pct = (ratio * 100.0).round() as i64
            ));
            if r.clicked() {
                self.queue.push(Act::Goal);
            }
            ui.label(format!("{mots}/{obj}"));
        }
        if session != 0 {
            ui.label(
                RichText::new(trf!(
                    "session {n}",
                    n = if session > 0 {
                        format!("+{session}")
                    } else {
                        format!("−{}", -session)
                    }
                ))
                .weak(),
            )
            .on_hover_text(tr("Mots écrits depuis l'ouverture du document"));
        }
        let lang = self.page.langue;
        ui.label(
            RichText::new(trf!(
                "lecture {t}",
                t = duree(lisibilite::secondes_lecture(mots, lang))
            ))
            .weak(),
        )
        .on_hover_text(trf!(
            "Temps de lecture ({lecture} mots/min) · à voix haute : {parole}",
            lecture = lisibilite::mots_par_minute_lecture(lang) as u32,
            parole = duree(lisibilite::secondes_parole(mots))
        ));
    }

    /// Inspecteur → Document : objectif, temps de lecture et de parole, lisibilité.
    pub(super) fn statistiques_ecriture(&mut self, ui: &mut egui::Ui) {
        let lang = self.page.langue;
        let texte = self.texte_affiche();
        let m = lisibilite::mesurer(&texte, lang);
        // Le même compte de mots que la barre d'état.
        let mots = self.mots();
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Écriture")).strong());
        egui::Grid::new("insp-ecriture")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label(RichText::new(tr("Objectif")).weak());
                ui.horizontal(|ui| {
                    match self.objectif.filter(|o| *o > 0) {
                        Some(o) => {
                            anneau(ui, mots as f32 / o as f32, 16.0);
                            ui.label(trf!("{mots} / {obj} mots", mots, obj = o));
                        }
                        None => {
                            ui.label(RichText::new(tr("aucun")).weak());
                        }
                    }
                    if ui.small_button(tr("Définir…")).clicked() {
                        self.queue.push(Act::Goal);
                    }
                });
                ui.end_row();
                ui.label(RichText::new(tr("Lecture")).weak());
                ui.label(duree(lisibilite::secondes_lecture(mots, lang)));
                ui.end_row();
                ui.label(RichText::new(tr("À voix haute")).weak());
                ui.label(duree(lisibilite::secondes_parole(mots)));
                ui.end_row();
                ui.label(RichText::new(tr("Phrases")).weak());
                ui.label(m.phrases.to_string());
                ui.end_row();
                if m.phrases > 0 {
                    ui.label(RichText::new(tr("Mots par phrase")).weak());
                    ui.label(super::fr(m.mots as f64 / m.phrases as f64));
                    ui.end_row();
                }
            });
        ui.add_space(6.0);
        ui.label(RichText::new(tr("Lisibilité")).strong());
        match m.lisibilite(lang) {
            None => {
                ui.label(RichText::new(tr("Écrivez quelques phrases pour l'estimer.")).weak());
            }
            Some(l) => {
                let (nom, explication) = niveau_texte(l.niveau);
                let couleur = match l.niveau {
                    Niveau::TresFacile | Niveau::Facile => Color32::from_rgb(0x2E, 0x9E, 0x4F),
                    Niveau::Standard => accent(ui),
                    _ => err(ui),
                };
                ui.horizontal(|ui| {
                    ui.add(
                        egui::ProgressBar::new(l.score / 100.0)
                            .desired_width(110.0)
                            .desired_height(10.0)
                            .fill(couleur),
                    );
                    ui.label(RichText::new(nom).strong().color(couleur));
                    ui.label(RichText::new(format!("{:.0}/100", l.score)).weak());
                });
                ui.label(RichText::new(explication).small());
                ui.label(
                    RichText::new(trf!(
                        "Indice {formule} pour le {langue} : plus il est haut, plus le texte est facile à lire. Il tient compte de la longueur des phrases et des mots.",
                        formule = l.formule,
                        langue = langue_minuscule(lang)
                    ))
                    .small()
                    .weak(),
                );
            }
        }
    }

    /// Mode concentration : rien que le texte, au centre ; Échap pour sortir.
    pub(super) fn concentration_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if self.dialog.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.queue.push(Act::Focus);
        }
        let fond = if ui.visuals().dark_mode {
            Color32::from_rgb(0x1C, 0x1C, 0x1E)
        } else {
            Color32::from_rgb(0xF7, 0xF5, 0xF0)
        };
        let mots = self.mots();
        egui::Panel::bottom("focus-bas")
            .frame(
                egui::Frame::new()
                    .fill(fond)
                    .inner_margin(egui::Margin::symmetric(16, 6)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    ui.label(RichText::new(trf!("{n} mots", n = mots)).weak());
                    self.objectif_statut(ui, mots);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(tr("Quitter (Échap)"))
                            .on_hover_text(tr("Revenir à l'affichage habituel"))
                            .clicked()
                        {
                            self.queue.push(Act::Focus);
                        }
                        let mut tw = self.prefs.machine_a_ecrire;
                        if ui
                            .checkbox(&mut tw, tr("Machine à écrire"))
                            .on_hover_text(tr("La ligne en cours reste au milieu de l'écran"))
                            .changed()
                        {
                            self.queue.push(Act::ToggleTypewriter);
                        }
                    });
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(fond))
            .show(ui, |ui| {
                let largeur = ui.available_width();
                let colonne = largeur.min(760.0);
                let marge = ((largeur - colonne) / 2.0).max(0.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(marge);
                    ui.vertical(|ui| {
                        ui.set_width(colonne);
                        self.text_pane_ex(ui, false, true);
                    });
                });
            });
    }

    /// Le texte affiché en mode concentration : les paragraphes autres que celui du
    /// curseur sont estompés.
    pub(super) fn estomper(job: &mut egui::text::LayoutJob, courant: std::ops::Range<usize>) {
        let mut sections = Vec::with_capacity(job.sections.len() + 2);
        for s in std::mem::take(&mut job.sections) {
            let (debut, fin) = (s.byte_range.start.0, s.byte_range.end.0);
            // Découpe la section aux bords du paragraphe courant.
            let mut bornes = vec![debut];
            for b in [courant.start, courant.end] {
                if b > debut && b < fin {
                    bornes.push(b);
                }
            }
            bornes.push(fin);
            for w in bornes.windows(2) {
                let mut t = s.clone();
                t.byte_range = egui::text::ByteIndex(w[0])..egui::text::ByteIndex(w[1]);
                if !(w[0] >= courant.start && w[1] <= courant.end) {
                    t.format.color = t.format.color.gamma_multiply(0.28);
                    t.format.background = t.format.background.gamma_multiply(0.28);
                }
                t.leading_space = if w[0] == debut { s.leading_space } else { 0.0 };
                sections.push(t);
            }
        }
        job.sections = sections;
    }

    /// Correction automatique juste après la frappe : appliquée comme une étape
    /// d'annulation à part (Ctrl+Z rend le texte tel qu'il a été tapé). Plusieurs
    /// caractères peuvent arriver dans la même image : chacun est examiné à son tour.
    pub(super) fn corriger_frappe(&mut self, avant: &str, now: f64) {
        if !self.prefs.correction_auto {
            return;
        }
        let (a, b) = self.sel;
        if a != b || self.source.len() <= avant.len() {
            return;
        }
        let curseur = editor::byte_of(&self.source, b);
        // Une insertion pure, juste avant le curseur, d'au plus 16 caractères.
        let insere = self.source.len() - avant.len();
        if insere > curseur
            || !self.source.is_char_boundary(curseur - insere)
            || self.source[..curseur - insere] != avant[..curseur - insere]
            || self.source[curseur..] != avant[curseur - insere..]
        {
            return;
        }
        let tape = &self.source[curseur - insere..curseur];
        if tape.chars().count() > 16 {
            return;
        }
        let lang = self.page.langue;
        let mut prefixe = avant[..curseur - insere].to_string();
        let mut corrige = false;
        for c in tape.chars() {
            prefixe.push(c);
            if let Some(r) = autocorrect::apres_frappe(&prefixe, prefixe.len(), lang) {
                prefixe.replace_range(r.debut..r.fin, &r.texte);
                corrige = true;
            }
        }
        if !corrige {
            return;
        }
        // L'état tapé devient une étape d'annulation, puis la correction en est une autre.
        self.history.cut = true;
        self.track(now);
        let reste = self.source[curseur..].to_string();
        let c = prefixe.chars().count();
        self.source = prefixe + &reste;
        self.sel = (c, c);
        self.pending_sel = Some((c, c));
        self.history.cut = true;
    }

    /// Collage intelligent (Ctrl+V dans le texte) : HTML, Markdown, tableur, lien.
    /// Renvoie le texte à insérer à la place de la sélection.
    pub(super) fn coller(&mut self, ctx: &egui::Context, texte: String) -> String {
        let html = arboard::Clipboard::new()
            .ok()
            .and_then(|mut c| c.get().html().ok());
        let (a, b) = (self.sel.0.min(self.sel.1), self.sel.0.max(self.sel.1));
        let (ba, bb) = (
            editor::byte_of(&self.source, a),
            editor::byte_of(&self.source, b),
        );
        let c = collage::coller(&texte, html.as_deref(), &self.source[ba..bb]);
        let mut sortie = c.texte;
        // Un tableau ou plusieurs blocs collés au milieu d'une ligne : sur leurs lignes.
        let blocs = sortie.contains('\n')
            && matches!(
                c.genre,
                collage::Genre::Tableau | collage::Genre::Html | collage::Genre::Markdown
            );
        if blocs {
            if ba > 0 && !self.source[..ba].ends_with('\n') {
                sortie.insert(0, '\n');
            }
            if bb < self.source.len() && !self.source[bb..].starts_with('\n') {
                sortie.push('\n');
            }
        }
        let msg = match c.genre {
            collage::Genre::Texte => None,
            collage::Genre::Html => Some(tr(
                "Collé avec sa mise en forme · Ctrl+Maj+V pour coller du texte brut",
            )),
            collage::Genre::Markdown => Some(tr(
                "Markdown converti · Ctrl+Maj+V pour coller du texte brut",
            )),
            collage::Genre::Tableau => Some(tr(
                "Cellules collées en tableau · Ctrl+Maj+V pour coller du texte brut",
            )),
            collage::Genre::Lien => Some(tr("Lien créé sur la sélection")),
        };
        if let Some(m) = msg {
            self.notify(ctx, m, false);
        }
        sortie
    }

    /// Ctrl+Maj+V : le texte du presse-papiers, sans aucune conversion.
    pub(super) fn coller_brut(&mut self, ctx: &egui::Context) {
        match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
            Ok(t) if !t.is_empty() => {
                let t = t.replace("\r\n", "\n");
                self.insert(&t, None);
            }
            _ => self.notify(ctx, tr("Le presse-papiers ne contient pas de texte"), true),
        }
    }

    /// Dialogue « Objectif de mots ».
    pub(super) fn objectif_dialogue(&mut self, ui: &mut egui::Ui, saisie: &mut String) -> bool {
        let mut garder = true;
        ui.heading(tr("Objectif de mots"));
        ui.label(tr("Combien de mots voulez-vous écrire ? Un anneau dans la barre d'état montre où vous en êtes."));
        let r = ui.add(
            egui::TextEdit::singleline(saisie)
                .hint_text(tr("ex. 1500"))
                .desired_width(120.0),
        );
        r.request_focus();
        ui.horizontal_wrapped(|ui| {
            for n in [300, 500, 1000, 1500, 3000] {
                if ui.small_button(n.to_string()).clicked() {
                    *saisie = n.to_string();
                }
            }
        });
        let valeur = saisie.trim().replace([' ', '\u{a0}', '\u{202f}'], "");
        let n = valeur.parse::<u32>().ok().filter(|n| *n > 0);
        if !valeur.is_empty() && n.is_none() {
            ui.label(
                RichText::new(tr("Nombre de mots attendu"))
                    .small()
                    .color(err(ui)),
            );
        }
        let entree =
            (r.lost_focus() || r.has_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.horizontal(|ui| {
            if (ui
                .add_enabled(n.is_some(), egui::Button::new(tr("Définir")))
                .clicked()
                || entree)
                && n.is_some()
            {
                self.objectif = n;
                garder = false;
            }
            if self.objectif.is_some() && ui.button(tr("Retirer l'objectif")).clicked() {
                self.objectif = None;
                garder = false;
            }
            if ui.button(tr("Annuler")).clicked() {
                garder = false;
            }
        });
        garder
    }

    /// Ouvre le dialogue de l'objectif.
    pub(super) fn ouvrir_objectif(&mut self) {
        self.dialog = Some(Dialog::Objectif(
            self.objectif.map(|n| n.to_string()).unwrap_or_default(),
        ));
    }
}

/// Le nom de la langue en minuscules, dans la langue de l'interface (« français »).
fn langue_minuscule(l: Lang) -> String {
    match l {
        Lang::Fr => tr("français"),
        Lang::En => tr("anglais"),
        Lang::Es => tr("espagnol"),
        Lang::De => tr("allemand"),
        Lang::It => tr("italien"),
        Lang::Pt => tr("portugais"),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durees() {
        assert_eq!(duree(0), "0 s");
        assert_eq!(duree(59), "59 s");
        assert_eq!(duree(61), "2 min");
        assert_eq!(duree(3600 + 5 * 60), "1 h 05");
    }

    #[test]
    fn estompage() {
        let mut job = egui::text::LayoutJob::default();
        job.append("un\ndeux\ntrois", 0.0, egui::TextFormat::default());
        VariumApp::estomper(&mut job, 3..8);
        let r: Vec<_> = job
            .sections
            .iter()
            .map(|s| s.byte_range.start.0..s.byte_range.end.0)
            .collect();
        assert_eq!(r, [0..3, 3..8, 8..13]);
        assert_eq!(
            job.sections[1].format.color,
            egui::TextFormat::default().color
        );
        assert_ne!(
            job.sections[0].format.color,
            egui::TextFormat::default().color
        );
    }
}
