//! Documents : mise en forme du paragraphe (inspecteur Format : retraits, interligne,
//! espacements, couleur et surlignage) et signatures manuscrites (Insertion →
//! Signature… : un pavé pour dessiner à la souris ou au pavé tactile).
//!
//! Les signatures sont enregistrées hors du document, dans le dossier de données de
//! Varium (`signatures/*.sig`), et réutilisables d'un document à l'autre ; on peut
//! aussi les intégrer au document (valeur réservée `signature:nom`).
//!
//! Sous-module de `app` : il prolonge `VariumApp`.

use std::path::PathBuf;

use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use varium_core::doc::{self, Attributs, Couleur, Interligne};
use varium_core::signature::{self, Signature};

use super::VariumApp;
use crate::editor;
use crate::tr::tr;

/// Couleurs proposées (noms de la syntaxe `{couleur:…}`).
const PALETTE: [&str; 10] = [
    "noir",
    "gris",
    "rouge",
    "orange",
    "vert",
    "turquoise",
    "bleu",
    "violet",
    "rose",
    "marron",
];
const PALETTE_SURLIGNAGE: [&str; 7] = ["jaune", "vert", "bleu", "rose", "orange", "violet", "gris"];

fn c32(c: Couleur) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

/// Dossier des signatures de l'utilisateur.
fn dossier() -> PathBuf {
    crate::secours::dossier_donnees().join("signatures")
}

fn fichier(nom: &str) -> PathBuf {
    let n = if nom.is_empty() { "signature" } else { nom };
    dossier().join(format!("{n}.sig"))
}

/// Charge les signatures enregistrées (au démarrage) : elles deviennent disponibles
/// pour la vue page et les exports.
pub(super) fn charger_signatures() {
    let Ok(lecture) = std::fs::read_dir(dossier()) else {
        return;
    };
    for e in lecture.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("sig") {
            continue;
        }
        let Some(stem) = p.file_stem().and_then(|x| x.to_str()) else {
            continue;
        };
        let nom = if stem == "signature" { "" } else { stem };
        if let Some(s) = std::fs::read_to_string(&p)
            .ok()
            .and_then(|t| Signature::lire(&t))
        {
            signature::definir(nom, s);
        }
    }
}

/// Le pavé de signature (gardé dans la mémoire d'egui).
#[derive(Clone, Default)]
struct Pave {
    ouvert: bool,
    traits: Vec<Vec<(f32, f32)>>,
    en_cours: Vec<(f32, f32)>,
    nom: String,
    integrer: bool,
    erreur: Option<String>,
}

fn id_pave() -> egui::Id {
    egui::Id::unique("pave-signature")
}

/// Ouvre la fenêtre « Signature » (menu Insertion).
pub(super) fn ouvrir_signature(ctx: &egui::Context) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Pave>(id_pave()).ouvert = true);
}

/// Aperçu d'une signature dans `r`.
fn dessiner(p: &egui::Painter, s: &Signature, r: Rect, couleur: Color32) {
    let k = (r.width() / s.largeur.max(1.0)).min(r.height() / s.hauteur.max(1.0));
    let o = r.center() - Vec2::new(s.largeur * k / 2.0, s.hauteur * k / 2.0);
    let st = Stroke::new((s.epaisseur() * k).max(1.0), couleur);
    for t in &s.traits {
        let pts: Vec<Pos2> = t.iter().map(|(x, y)| o + Vec2::new(x * k, y * k)).collect();
        if pts.len() == 1 {
            p.circle_filled(pts[0], st.width / 2.0, couleur);
        } else {
            p.add(egui::Shape::line(pts, st));
        }
    }
}

impl VariumApp {
    /// Les lignes (octets) touchées par la sélection.
    fn lignes_selection(&self) -> (usize, usize) {
        let (a, b) = (self.sel.0.min(self.sel.1), self.sel.0.max(self.sel.1));
        (
            editor::byte_of(&self.source, a),
            editor::byte_of(&self.source, b),
        )
    }

    /// Les attributs `{: …}` du paragraphe où se trouve le curseur.
    fn attributs_courants(&self) -> Attributs {
        let b = editor::byte_of(&self.source, self.sel.1);
        let debut = self.source[..b].rfind('\n').map_or(0, |i| i + 1);
        let fin = self.source[b..]
            .find('\n')
            .map_or(self.source.len(), |i| b + i);
        doc::attributs_de_ligne(&self.source[debut..fin])
            .map(|(a, _)| a)
            .unwrap_or_default()
    }

    /// Modifie les attributs des paragraphes sélectionnés.
    fn changer_attributs(&mut self, f: impl Fn(&mut Attributs)) {
        let (d, fin) = self.lignes_selection();
        let nouvelle = doc::modifier_attributs(&self.source, d, fin, &|mut a| {
            f(&mut a);
            a
        });
        if nouvelle == self.source {
            return;
        }
        // Le curseur reste dans le même paragraphe, au début de son texte.
        let ligne = self.source[..d].matches('\n').count();
        self.edit(move |s, _| {
            *s = nouvelle;
            let debut = s
                .match_indices('\n')
                .nth(ligne.wrapping_sub(1))
                .map_or(0, |(i, _)| i + 1);
            let debut = if ligne == 0 { 0 } else { debut };
            let fin_l = s[debut..].find('\n').map_or(s.len(), |i| debut + i);
            let texte = doc::attributs_de_ligne(&s[debut..fin_l]).map_or(0, |(_, n)| n);
            let c = editor::char_of(s, debut + texte);
            (c, c)
        });
    }

    /// Entoure la sélection d'une couleur ou d'un surlignage en ligne.
    fn colorer_selection(&mut self, cle: &str, nom: &str) {
        let (a, b) = (self.sel.0.min(self.sel.1), self.sel.0.max(self.sel.1));
        let ouvre = format!("{{{cle}:{nom}}}");
        self.edit(move |s, _| {
            let (ba, bb) = (editor::byte_of(s, a), editor::byte_of(s, b));
            s.insert_str(bb, "{/}");
            s.insert_str(ba, &ouvre);
            let n = ouvre.chars().count();
            (a + n, b + n)
        });
    }

    /// Section « Paragraphe » de l'onglet Format.
    pub(super) fn format_paragraphe(&mut self, ui: &mut egui::Ui) {
        let a = self.attributs_courants();
        let f = a.format;
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Paragraphe")).strong())
            .on_hover_text(tr("S'applique au paragraphe du curseur ou aux paragraphes sélectionnés (écriture {: …} en début de ligne)."));
        let cm = |v: f64, _| format!("{} cm", super::fr(v));
        egui::Grid::new("insp-para")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                let ligne = |ui: &mut egui::Ui,
                             libelle: &str,
                             valeur: Option<f32>,
                             min: f64,
                             max: f64|
                 -> Option<Option<f32>> {
                    ui.label(libelle);
                    let mut v = valeur.unwrap_or(0.0) as f64 / 10.0;
                    let r = ui.add(
                        egui::DragValue::new(&mut v)
                            .range(min..=max)
                            .speed(0.05)
                            .custom_formatter(cm),
                    );
                    ui.end_row();
                    r.changed()
                        .then(|| (v.abs() > 0.0001).then_some((v * 10.0) as f32))
                };
                if let Some(v) = ligne(ui, tr("Retrait à gauche"), f.gauche, 0.0, 15.0) {
                    self.changer_attributs(|a| a.format.gauche = v);
                }
                if let Some(v) = ligne(ui, tr("Retrait à droite"), f.droite, 0.0, 15.0) {
                    self.changer_attributs(|a| a.format.droite = v);
                }
                if let Some(v) = ligne(ui, tr("Première ligne"), f.alinea, -5.0, 5.0) {
                    self.changer_attributs(|a| a.format.alinea = v);
                }
                let pt = |v: f64, _| format!("{} pt", super::fr(v));
                for (libelle, valeur, avant) in [
                    (tr("Espace avant"), f.avant, true),
                    (tr("Espace après"), f.apres, false),
                ] {
                    ui.label(libelle);
                    let mut v = valeur.unwrap_or(0.0) as f64;
                    if ui
                        .add(
                            egui::DragValue::new(&mut v)
                                .range(0.0..=96.0)
                                .speed(0.5)
                                .custom_formatter(pt),
                        )
                        .changed()
                    {
                        let n = (v > 0.0001).then_some(v as f32);
                        self.changer_attributs(move |a| {
                            if avant {
                                a.format.avant = n;
                            } else {
                                a.format.apres = n;
                            }
                        });
                    }
                    ui.end_row();
                }
                ui.label(tr("Interligne"));
                let texte = match f.interligne {
                    None => tr("Celui du document").to_string(),
                    Some(Interligne::Multiple(m)) if (m - 1.0).abs() < 0.01 => {
                        tr("Simple").to_string()
                    }
                    Some(Interligne::Multiple(m)) if (m - 2.0).abs() < 0.01 => {
                        tr("Double").to_string()
                    }
                    Some(Interligne::Multiple(m)) => super::fr(m as f64),
                    Some(Interligne::Exact(p)) => {
                        format!("{} {} pt", tr("Exact"), super::fr(p as f64))
                    }
                };
                let mut choix: Option<Option<Interligne>> = None;
                egui::ComboBox::from_id_salt("insp-interligne")
                    .selected_text(texte)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(f.interligne.is_none(), tr("Celui du document"))
                            .clicked()
                        {
                            choix = Some(None);
                        }
                        for (m, l) in [
                            (1.0, tr("Simple")),
                            (1.15, "1,15"),
                            (1.5, "1,5"),
                            (2.0, tr("Double")),
                        ] {
                            let on = f.multiple().is_some_and(|x| (x - m).abs() < 0.01);
                            if ui.selectable_label(on, l).clicked() {
                                choix = Some(Some(Interligne::Multiple(m)));
                            }
                        }
                        let exact = matches!(f.interligne, Some(Interligne::Exact(_)));
                        if ui.selectable_label(exact, tr("Exact")).clicked() {
                            choix = Some(Some(Interligne::Exact(self.page.font_size * 1.2)));
                        }
                    });
                ui.end_row();
                if let Some(Interligne::Exact(p)) = f.interligne {
                    ui.label("");
                    let mut v = p as f64;
                    if ui
                        .add(
                            egui::DragValue::new(&mut v)
                                .range(4.0..=96.0)
                                .speed(0.2)
                                .custom_formatter(pt),
                        )
                        .changed()
                    {
                        choix = Some(Some(Interligne::Exact(v as f32)));
                    }
                    ui.end_row();
                }
                if let Some(c) = choix {
                    self.changer_attributs(move |a| a.format.interligne = c);
                }
            });
        // Couleurs : la sélection si elle existe, sinon tout le paragraphe.
        let selection = self.sel.0 != self.sel.1;
        let portee = if selection {
            tr("la sélection")
        } else {
            tr("le paragraphe")
        };
        for (titre, surl) in [(tr("Couleur du texte"), false), (tr("Surlignage"), true)] {
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{titre} ({portee})")).small());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(3.0);
                let noms: &[&str] = if surl { &PALETTE_SURLIGNAGE } else { &PALETTE };
                for nom in noms {
                    let c = if surl {
                        Couleur::lire_surlignage(nom)
                    } else {
                        Couleur::lire(nom)
                    };
                    let Some(c) = c else { continue };
                    let actuel = if surl { a.surlignage } else { a.couleur };
                    let (r, rep) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
                    ui.painter().rect_filled(r, 3.0, c32(c));
                    let bord = if actuel == Some(c) {
                        Stroke::new(2.0, ui.visuals().strong_text_color())
                    } else {
                        Stroke::new(1.0, Color32::from_gray(150))
                    };
                    ui.painter()
                        .rect_stroke(r, 3.0, bord, egui::StrokeKind::Inside);
                    if rep.on_hover_text(*nom).clicked() {
                        if selection {
                            self.colorer_selection(
                                if surl { "surlignage" } else { "couleur" },
                                nom,
                            );
                        } else {
                            self.changer_attributs(move |a| {
                                if surl {
                                    a.surlignage = Some(c);
                                } else {
                                    a.couleur = Some(c);
                                }
                            });
                        }
                    }
                }
                if !selection
                    && (if surl { a.surlignage } else { a.couleur }).is_some()
                    && ui.small_button("✕").on_hover_text(tr("Aucune")).clicked()
                {
                    self.changer_attributs(move |a| {
                        if surl {
                            a.surlignage = None;
                        } else {
                            a.couleur = None;
                        }
                    });
                }
            });
        }
        if !a.est_vide()
            && ui
                .button(tr("Effacer la mise en forme du paragraphe"))
                .clicked()
        {
            self.changer_attributs(|a| *a = Attributs::default());
        }
    }

    /// La fenêtre « Signature » : signatures enregistrées et pavé pour en dessiner une.
    pub(super) fn signature_fenetre(&mut self, ctx: &egui::Context) {
        let mut pave: Pave = ctx.data_mut(|d| d.get_temp::<Pave>(id_pave()).unwrap_or_default());
        if !pave.ouvert {
            return;
        }
        let mut ouvert = true;
        let mut inserer: Option<String> = None;
        egui::Window::new(tr("Signature"))
            .open(&mut ouvert)
            .collapsible(false)
            .resizable(false)
            .default_width(520.0)
            .show(ctx, |ui| {
                let noms = signature::noms();
                if !noms.is_empty() {
                    ui.label(RichText::new(tr("Signatures enregistrées")).strong());
                    let mut supprimer = None;
                    ui.horizontal_wrapped(|ui| {
                        for nom in &noms {
                            let Some(s) = signature::obtenir(nom) else {
                                continue;
                            };
                            ui.vertical(|ui| {
                                let (r, rep) =
                                    ui.allocate_exact_size(Vec2::new(140.0, 50.0), Sense::click());
                                ui.painter().rect_filled(r, 4.0, Color32::WHITE);
                                ui.painter().rect_stroke(
                                    r,
                                    4.0,
                                    Stroke::new(1.0, Color32::from_gray(190)),
                                    egui::StrokeKind::Inside,
                                );
                                dessiner(
                                    ui.painter(),
                                    &s,
                                    r.shrink(4.0),
                                    Color32::from_rgb(0x1A, 0x2A, 0x5A),
                                );
                                if rep.on_hover_text(tr("Insérer cette signature")).clicked() {
                                    inserer = Some(nom.clone());
                                }
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(if nom.is_empty() {
                                            tr("par défaut")
                                        } else {
                                            nom.as_str()
                                        })
                                        .small(),
                                    );
                                    if ui
                                        .small_button("🗑")
                                        .on_hover_text(tr("Supprimer"))
                                        .clicked()
                                    {
                                        supprimer = Some(nom.clone());
                                    }
                                });
                            });
                        }
                    });
                    if let Some(n) = supprimer {
                        signature::retirer(&n);
                        let _ = std::fs::remove_file(fichier(&n));
                    }
                    ui.separator();
                }
                ui.label(RichText::new(tr("Nouvelle signature : dessinez dans le cadre")).strong());
                let taille = Vec2::new(ui.available_width().clamp(300.0, 520.0), 170.0);
                let (rep, painter) = ui.allocate_painter(taille, Sense::drag());
                let r = rep.rect;
                painter.rect_filled(r, 6.0, Color32::WHITE);
                painter.rect_stroke(
                    r,
                    6.0,
                    Stroke::new(1.0, Color32::from_gray(170)),
                    egui::StrokeKind::Inside,
                );
                // Ligne de signature.
                painter.line_segment(
                    [
                        Pos2::new(r.left() + 20.0, r.bottom() - 40.0),
                        Pos2::new(r.right() - 20.0, r.bottom() - 40.0),
                    ],
                    Stroke::new(1.0, Color32::from_gray(215)),
                );
                if let Some(p) = rep.interact_pointer_pos()
                    && rep.dragged()
                    && r.contains(p)
                {
                    let q = (p.x - r.left(), p.y - r.top());
                    if pave.en_cours.last() != Some(&q) {
                        pave.en_cours.push(q);
                    }
                }
                if (rep.drag_stopped() || (!rep.dragged() && !pave.en_cours.is_empty()))
                    && !pave.en_cours.is_empty()
                {
                    let t = signature::lisser(&std::mem::take(&mut pave.en_cours));
                    pave.traits.push(t);
                }
                let encre = Color32::from_rgb(0x1A, 0x2A, 0x5A);
                for t in pave.traits.iter().chain(std::iter::once(&pave.en_cours)) {
                    let pts: Vec<Pos2> = t.iter().map(|(x, y)| r.min + Vec2::new(*x, *y)).collect();
                    match pts.len() {
                        0 => {}
                        1 => {
                            painter.circle_filled(pts[0], 1.5, encre);
                        }
                        _ => {
                            painter.add(egui::Shape::line(pts, Stroke::new(2.6, encre)));
                        }
                    }
                }
                ui.horizontal(|ui| {
                    if ui.button(tr("Effacer")).clicked() {
                        pave.traits.clear();
                        pave.en_cours.clear();
                    }
                    if ui
                        .add_enabled(
                            !pave.traits.is_empty(),
                            egui::Button::new(tr("Annuler le dernier trait")),
                        )
                        .clicked()
                    {
                        pave.traits.pop();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(tr("Nom"));
                    ui.add(
                        egui::TextEdit::singleline(&mut pave.nom)
                            .hint_text(tr("par défaut"))
                            .desired_width(160.0),
                    );
                });
                ui.checkbox(
                    &mut pave.integrer,
                    tr("Intégrer au document (sinon, gardée seulement sur cet ordinateur)"),
                );
                if let Some(e) = &pave.erreur {
                    ui.colored_label(super::err(ui), e);
                }
                let nom = pave.nom.trim().to_string();
                if ui
                    .add_enabled(
                        !pave.traits.is_empty(),
                        egui::Button::new(tr("Enregistrer et insérer")),
                    )
                    .clicked()
                {
                    if !nom.is_empty() && !varium_core::is_valid_name(&nom) {
                        pave.erreur = Some(
                            tr("Nom invalide : lettres, chiffres, - et _ seulement.").to_string(),
                        );
                    } else if let Some(s) = Signature::depuis_traits(&pave.traits) {
                        let _ = std::fs::create_dir_all(dossier());
                        if let Err(e) = std::fs::write(fichier(&nom), s.ecrire()) {
                            log::warn!("signature non enregistrée : {e}");
                        }
                        if pave.integrer {
                            self.values.set(signature::cle_valeur(&nom), s.ecrire());
                        }
                        signature::definir(&nom, s);
                        pave.traits.clear();
                        pave.erreur = None;
                        inserer = Some(nom);
                    }
                }
            });
        if let Some(nom) = inserer {
            let texte = if nom.is_empty() {
                "{{signature}}".to_string()
            } else {
                format!("{{{{signature:{nom}}}}}")
            };
            self.insert_block(&texte);
            ouvert = false;
        }
        pave.ouvert = ouvert;
        ctx.data_mut(|d| d.insert_temp(id_pave(), pave));
    }
}
