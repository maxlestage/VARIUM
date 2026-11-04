//! Documents : historique des versions, comme Pages ou Time Machine.
//!
//! Une version est prise à chaque enregistrement, toutes les cinq minutes pendant
//! l'écriture, avant les grosses opérations (« Tout corriger », « Tout remplacer »,
//! renommage, conversion, restauration) et à la demande (« Marquer cette version… »).
//! Le panneau liste les versions (heure, mots, ce qui a changé) et compare la version
//! choisie au document actuel, mot à mot.
//!
//! Sous-module de `app` : il prolonge `VariumApp`.

use std::collections::HashMap;
use std::path::PathBuf;

use egui::{Color32, RichText, Stroke};
use varium_core::diff::{self, Morceau, Op};
use varium_core::versions::{self, Historique, Motif, Version};

use super::{Dialog, Snapshot, VariumApp, accent, err, secours};
use crate::tr::tr;

/// Comparer la version choisie avec…
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Reference {
    /// … le document tel qu'il est maintenant.
    #[default]
    Actuel,
    /// … la version qui la précède.
    Precedente,
}

/// L'état du panneau et des versions du document ouvert.
#[derive(Default)]
pub(super) struct Etat {
    /// Versions du document ouvert (chargées à la demande).
    histo: Option<Historique>,
    /// Heure (horloge d'egui) de la dernière version automatique.
    derniere_auto: Option<f64>,
    pub ouvert: bool,
    choisie: Option<String>,
    reference: Reference,
    cote_a_cote: bool,
    /// Résumés « ce qui a changé » déjà calculés (par identifiant).
    resumes: HashMap<String, String>,
    /// Dernière comparaison affichée : (clé, morceaux).
    comparaison: Option<(u64, Vec<Morceau>)>,
    renommer: Option<(String, String)>,
}

/// Secondes depuis 1970, maintenant.
fn maintenant() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// « 04/11/2026 14:32 », à l'heure locale.
fn date_heure(secondes: u64) -> String {
    let Ok(t) = jiff::Timestamp::from_second(secondes as i64) else {
        return String::new();
    };
    let z = t.to_zoned(jiff::tz::TimeZone::system());
    let aujourdhui = jiff::Zoned::now().date();
    if z.date() == aujourdhui {
        trf!("aujourd'hui {h}", h = z.strftime("%H:%M").to_string())
    } else if aujourdhui.yesterday().ok() == Some(z.date()) {
        trf!("hier {h}", h = z.strftime("%H:%M").to_string())
    } else {
        z.strftime("%d/%m/%Y %H:%M").to_string()
    }
}

fn motif(v: &Version) -> String {
    match v.motif {
        Motif::Enregistrement => tr("Enregistrement").to_string(),
        Motif::Auto => tr("Automatique").to_string(),
        Motif::Avant if v.detail.is_empty() => tr("Avant une modification").to_string(),
        Motif::Avant => trf!("Avant « {op} »", op = tr_detail(&v.detail)),
        Motif::Restauration => tr("Avant une restauration").to_string(),
        Motif::Manuel => tr("Marquée").to_string(),
    }
}

/// Les opérations sont enregistrées en français (clé de traduction).
fn tr_detail(d: &str) -> &str {
    OPERATIONS.iter().find(|o| **o == d).map_or(d, |o| tr(o))
}

/// Les noms d'opérations qui précèdent une version (enregistrés en français).
pub(super) const OPERATIONS: &[&str] = &[
    crate::tr::n_("Tout corriger"),
    crate::tr::n_("Tout remplacer"),
    crate::tr::n_("Renommer partout"),
    crate::tr::n_("Figer un champ"),
    crate::tr::n_("Effacer toutes les valeurs"),
    crate::tr::n_("Remplacement du document"),
];

fn resume_texte(avant: Option<&Version>, v: &Version) -> String {
    let Some(a) = avant else {
        return tr("Première version").to_string();
    };
    let r = diff::resume(&a.projet.source, &v.projet.source);
    let mut parts = Vec::new();
    if r.ajoutes > 0 {
        parts.push(format!("+{}", r.ajoutes));
    }
    if r.retires > 0 {
        parts.push(format!("−{}", r.retires));
    }
    let mut s = if parts.is_empty() {
        if a.projet.values != v.projet.values {
            tr("valeurs des champs modifiées").to_string()
        } else if a.projet.page != v.projet.page || a.projet.title != v.projet.title {
            tr("mise en page ou titre modifiés").to_string()
        } else {
            tr("retouches").to_string()
        }
    } else {
        trf!("{n} mots", n = parts.join(" "))
    };
    if let Some(e) = r.extrait {
        s.push_str(&format!(" · « {e} »"));
    }
    s
}

/// Un texte comparé, en couleurs : ajouts en vert, retraits en rouge barrés.
fn texte_compare(ui: &egui::Ui, morceaux: &[Morceau], montrer: &[Op]) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let font = egui::FontId::proportional(14.0);
    let sombre = ui.visuals().dark_mode;
    let (vert, rouge) = if sombre {
        (
            Color32::from_rgba_unmultiplied(0x3F, 0xB9, 0x50, 70),
            Color32::from_rgba_unmultiplied(0xF8, 0x51, 0x49, 70),
        )
    } else {
        (
            Color32::from_rgb(0xD5, 0xF5, 0xDC),
            Color32::from_rgb(0xFD, 0xDC, 0xDA),
        )
    };
    let encre = ui.visuals().text_color();
    for m in morceaux.iter().filter(|m| montrer.contains(&m.op)) {
        let mut f = egui::TextFormat {
            font_id: font.clone(),
            color: encre,
            ..Default::default()
        };
        match m.op {
            Op::Egal => f.color = ui.visuals().weak_text_color().gamma_multiply(1.2),
            Op::Ajout => {
                f.background = vert;
                f.color = if sombre {
                    Color32::from_rgb(0xAF, 0xF5, 0xB4)
                } else {
                    Color32::from_rgb(0x11, 0x63, 0x29)
                };
            }
            Op::Retrait => {
                f.background = rouge;
                f.color = if sombre {
                    Color32::from_rgb(0xFF, 0xB8, 0xB0)
                } else {
                    Color32::from_rgb(0x8E, 0x1B, 0x12)
                };
                f.strikethrough = Stroke::new(1.0, f.color);
            }
        }
        job.append(&m.texte, 0.0, f);
    }
    job
}

impl VariumApp {
    fn dossier_versions(&self) -> PathBuf {
        versions::dossier(&secours::dossier_donnees(), self.path.as_deref())
    }

    /// Les versions du document ouvert (rechargées si le document a changé de fichier).
    fn histo(&mut self) -> &mut Historique {
        let d = self.dossier_versions();
        let e = &mut self.outils.historique;
        if e.histo.as_ref().is_none_or(|h| h.dossier != d) {
            e.histo = Some(Historique::ouvrir(&d));
            e.resumes.clear();
            e.comparaison = None;
            e.choisie = None;
        }
        e.histo.get_or_insert_with(Historique::default)
    }

    /// Prend une version de l'état actuel (sans doublon). `detail` : opération en
    /// français, parmi [`OPERATIONS`].
    pub(super) fn prendre_version(&mut self, m: Motif, detail: &str) {
        if self.source.trim().is_empty() && self.values.values.is_empty() {
            return;
        }
        let p = self.project();
        if let Err(e) = self.histo().ajouter(&p, m, detail, maintenant()) {
            log::warn!("historique des versions : {e}");
        }
    }

    /// Version automatique toutes les cinq minutes d'écriture.
    pub(super) fn versions_tick(&mut self, ctx: &egui::Context, now: f64) {
        let e = &mut self.outils.historique;
        let derniere = *e.derniere_auto.get_or_insert(now);
        if now - derniere >= versions::INTERVALLE_AUTO as f64 {
            e.derniere_auto = Some(now);
            if self.history.last_edit > derniere {
                self.prendre_version(Motif::Auto, "");
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(versions::INTERVALLE_AUTO));
    }

    /// Ouvre le panneau de l'historique.
    pub(super) fn ouvrir_historique(&mut self) {
        let _ = self.histo();
        let e = &mut self.outils.historique;
        e.ouvert = true;
        if e.choisie.is_none() {
            e.choisie = e.histo.as_ref().and_then(|h| h.derniere().map(Version::id));
        }
    }

    /// « Marquer cette version… » : nomme l'état actuel.
    pub(super) fn marquer_version(&mut self, ctx: &egui::Context, nom: &str) {
        let p = self.project();
        match self.histo().marquer(&p, nom, maintenant()) {
            Ok(()) => self.notify(
                ctx,
                trf!("Version « {nom} » marquée", nom = nom.trim()),
                false,
            ),
            Err(e) => self.notify(ctx, trf!("Historique inaccessible : {e}", e), true),
        }
        self.outils.historique.resumes.clear();
    }

    fn restaurer_version(&mut self, ctx: &egui::Context, v: &Version) {
        self.prendre_version(Motif::Restauration, "");
        self.history.cut = true;
        let p = v.projet.clone();
        let values = p.values();
        self.restore(Snapshot {
            title: p.title,
            source: p.source,
            values,
            page: p.page,
            objectif: p.objectif,
        });
        self.history.cut = true;
        self.pending_sel = Some(self.sel);
        self.notify(
            ctx,
            trf!(
                "Version du {date} restaurée · Ctrl+Z pour revenir",
                date = date_heure(v.horodatage)
            ),
            false,
        );
    }

    /// Le panneau « Historique des versions ».
    pub(super) fn historique_panneau(&mut self, ctx: &egui::Context) {
        if !self.outils.historique.ouvert {
            return;
        }
        let versions: Vec<Version> = self.histo().versions.clone();
        let mut fermer = false;
        let mut restaurer: Option<Version> = None;
        let mut supprimer: Option<String> = None;
        let mut nommer: Option<(String, String)> = None;
        let ecran = ctx.content_rect();
        let largeur = (ecran.width() - 60.0).clamp(320.0, 1180.0);
        let hauteur = (ecran.height() - 120.0).max(260.0);
        let modal = egui::Modal::new(egui::Id::unique("varium-historique")).show(ctx, |ui| {
            ui.set_width(largeur);
            ui.horizontal(|ui| {
                ui.heading(tr("Historique des versions"));
                ui.label(
                    RichText::new(match versions.len() {
                        0 => tr("aucune version").to_string(),
                        1 => tr("1 version").to_string(),
                        n => trf!("{n} versions", n),
                    })
                    .weak(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tr("Fermer")).clicked() {
                        fermer = true;
                    }
                    if ui
                        .button(tr("Marquer l'état actuel…"))
                        .on_hover_text(tr("Garder l'état actuel sous un nom (jamais effacé)"))
                        .clicked()
                    {
                        self.dialog = Some(Dialog::Marquer(String::new()));
                    }
                });
            });
            ui.separator();
            if versions.is_empty() {
                ui.label(tr("Pas encore de version. Varium en prend une à chaque enregistrement, toutes les cinq minutes pendant l'écriture et avant les grosses opérations (Tout corriger, Tout remplacer…)."));
                return;
            }
            let e = &mut self.outils.historique;
            if e.choisie.as_ref().is_none_or(|c| !versions.iter().any(|v| v.id() == *c)) {
                e.choisie = versions.last().map(Version::id);
            }
            ui.horizontal_top(|ui| {
                // Liste des versions, la plus récente en haut.
                ui.vertical(|ui| {
                    ui.set_width(300.0_f32.min(largeur * 0.4));
                    egui::ScrollArea::vertical()
                        .id_salt("hist-liste")
                        .max_height(hauteur)
                        .min_scrolled_height(hauteur)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (i, v) in versions.iter().enumerate().rev() {
                                let id = v.id();
                                let resume = self
                                    .outils
                                    .historique
                                    .resumes
                                    .entry(id.clone())
                                    .or_insert_with(|| resume_texte(i.checked_sub(1).map(|j| &versions[j]), v))
                                    .clone();
                                let choisie = self.outils.historique.choisie.as_deref() == Some(&id);
                                let fond = if choisie {
                                    ui.visuals().selection.bg_fill.gamma_multiply(0.5)
                                } else {
                                    Color32::TRANSPARENT
                                };
                                let r = egui::Frame::new()
                                    .fill(fond)
                                    .corner_radius(4.0)
                                    .inner_margin(6.0)
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(date_heure(v.horodatage)).strong());
                                            ui.label(RichText::new(trf!("{n} mots", n = v.mots)).small().weak());
                                        });
                                        if let Some(nom) = &v.nom {
                                            ui.label(RichText::new(format!("🏷 {nom}")).color(accent(ui)));
                                        }
                                        ui.label(RichText::new(motif(v)).small().weak());
                                        ui.label(RichText::new(resume).small());
                                    })
                                    .response
                                    .interact(egui::Sense::click());
                                if r.clicked() {
                                    self.outils.historique.choisie = Some(id);
                                    self.outils.historique.renommer = None;
                                }
                            }
                        });
                });
                ui.separator();
                // La version choisie, comparée.
                ui.vertical(|ui| {
                    let Some(i) = versions
                        .iter()
                        .position(|v| Some(v.id()) == self.outils.historique.choisie)
                    else {
                        return;
                    };
                    let v = &versions[i];
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(date_heure(v.horodatage)).strong().size(16.0));
                        if let Some(nom) = &v.nom {
                            ui.label(RichText::new(format!("🏷 {nom}")).color(accent(ui)));
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .button(tr("Restaurer cette version"))
                            .on_hover_text(tr("Le document reprend ce texte (l'état actuel est gardé dans l'historique, Ctrl+Z pour revenir)"))
                            .clicked()
                        {
                            restaurer = Some(v.clone());
                        }
                        if ui.button(tr("Copier le texte")).clicked() {
                            ui.ctx().copy_text(v.projet.source.clone());
                        }
                        if ui.button(tr("Nommer…")).clicked() {
                            self.outils.historique.renommer =
                                Some((v.id(), v.nom.clone().unwrap_or_default()));
                        }
                        if ui
                            .button(RichText::new(tr("Supprimer")).color(err(ui)))
                            .clicked()
                        {
                            supprimer = Some(v.id());
                        }
                    });
                    if let Some((id, nom)) = self.outils.historique.renommer.as_mut()
                        && *id == v.id()
                    {
                        ui.horizontal(|ui| {
                            let r = ui.add(
                                egui::TextEdit::singleline(nom)
                                    .hint_text(tr("ex. Envoyée au client"))
                                    .desired_width(220.0),
                            );
                            let ok = ui.button(tr("Enregistrer le nom")).clicked()
                                || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                            if ok {
                                nommer = Some((id.clone(), nom.clone()));
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label(tr("Comparer avec"));
                        let e = &mut self.outils.historique;
                        ui.selectable_value(&mut e.reference, Reference::Actuel, tr("le document actuel"));
                        ui.add_enabled_ui(i > 0, |ui| {
                            ui.selectable_value(&mut e.reference, Reference::Precedente, tr("la version précédente"));
                        });
                        ui.separator();
                        ui.selectable_value(&mut e.cote_a_cote, false, tr("Dans le texte"));
                        ui.selectable_value(&mut e.cote_a_cote, true, tr("Côte à côte"));
                    });
                    let reference = if self.outils.historique.reference == Reference::Precedente && i > 0 {
                        Reference::Precedente
                    } else {
                        Reference::Actuel
                    };
                    let (avant, apres) = match reference {
                        Reference::Precedente => (versions[i - 1].projet.source.clone(), v.projet.source.clone()),
                        Reference::Actuel => (v.projet.source.clone(), self.source.clone()),
                    };
                    let cle = versions::fnv(format!("{}\u{0}{avant}\u{0}{apres}", v.id()).as_bytes());
                    let e = &mut self.outils.historique;
                    if e.comparaison.as_ref().is_none_or(|(k, _)| *k != cle) {
                        e.comparaison = Some((cle, diff::diff_mots(&avant, &apres)));
                    }
                    let morceaux = e.comparaison.as_ref().map(|(_, m)| m.clone()).unwrap_or_default();
                    let (ajoutes, retires): (usize, usize) = morceaux.iter().fold((0, 0), |(a, r), m| match m.op {
                        Op::Ajout => (a + m.texte.split_whitespace().count(), r),
                        Op::Retrait => (a, r + m.texte.split_whitespace().count()),
                        Op::Egal => (a, r),
                    });
                    let (gauche, droite) = match reference {
                        Reference::Precedente => (tr("Version précédente"), tr("Cette version")),
                        Reference::Actuel => (tr("Cette version"), tr("Document actuel")),
                    };
                    ui.label(
                        RichText::new(if ajoutes + retires == 0 {
                            tr("Aucune différence dans le texte.").to_string()
                        } else {
                            trf!(
                                "{a} mot(s) ajouté(s) en vert, {r} retiré(s) en rouge : de « {gauche} » à « {droite} ».",
                                a = ajoutes,
                                r = retires,
                                gauche,
                                droite
                            )
                        })
                        .small()
                        .weak(),
                    );
                    let h = hauteur - 110.0;
                    if self.outils.historique.cote_a_cote {
                        ui.columns(2, |cols| {
                            for (k, (titre, ops)) in [
                                (gauche, [Op::Egal, Op::Retrait]),
                                (droite, [Op::Egal, Op::Ajout]),
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                cols[k].label(RichText::new(titre).strong());
                                egui::ScrollArea::vertical()
                                    .id_salt(("hist-cote", k))
                                    .max_height(h)
                                    .min_scrolled_height(h)
                                    .auto_shrink([false, false])
                                    .show(&mut cols[k], |ui| {
                                        let job = texte_compare(ui, &morceaux, &ops);
                                        ui.add(egui::Label::new(job).wrap());
                                    });
                            }
                        });
                    } else {
                        egui::ScrollArea::vertical()
                            .id_salt("hist-texte")
                            .max_height(h)
                                    .min_scrolled_height(h)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let job = texte_compare(ui, &morceaux, &[Op::Egal, Op::Ajout, Op::Retrait]);
                                ui.add(egui::Label::new(job).wrap());
                            });
                    }
                });
            });
        });
        if modal.should_close() && self.dialog.is_none() {
            fermer = true;
        }
        if let Some((id, nom)) = nommer {
            if let Err(e) = self.histo().nommer(&id, &nom) {
                self.notify(ctx, trf!("Historique inaccessible : {e}", e), true);
            }
            self.outils.historique.renommer = None;
        }
        if let Some(id) = supprimer {
            self.histo().supprimer(&id);
            self.outils.historique.choisie = None;
            self.outils.historique.resumes.clear();
        }
        if let Some(v) = restaurer {
            self.restaurer_version(ctx, &v);
            self.outils.historique.resumes.clear();
            fermer = true;
        }
        if fermer {
            self.outils.historique.ouvert = false;
            self.outils.historique.renommer = None;
        }
    }

    /// Dialogue « Marquer cette version… ».
    pub(super) fn marquer_dialogue(&mut self, ui: &mut egui::Ui, nom: &mut String) -> bool {
        let mut garder = true;
        ui.heading(tr("Marquer cette version"));
        ui.label(tr("L'état actuel du document est gardé dans l'historique sous ce nom, et ne sera jamais effacé automatiquement."));
        let r = ui.add(
            egui::TextEdit::singleline(nom)
                .hint_text(tr("ex. Envoyée au client"))
                .desired_width(f32::INFINITY),
        );
        r.request_focus();
        let ok = !nom.trim().is_empty();
        let entree =
            (r.lost_focus() || r.has_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.horizontal(|ui| {
            if (ui
                .add_enabled(ok, egui::Button::new(tr("Marquer")))
                .clicked()
                || entree)
                && ok
            {
                let n = nom.clone();
                let ctx = ui.ctx().clone();
                self.marquer_version(&ctx, &n);
                garder = false;
            }
            if ui.button(tr("Annuler")).clicked() {
                garder = false;
            }
        });
        garder
    }
}
