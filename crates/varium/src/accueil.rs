//! Centre de démarrage, comme celui de LibreOffice : nouveaux fichiers des trois
//! modules, modèles en vignettes, fichiers récents, « Ouvrir… » et une astuce.

use egui::{Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use varium_calc::{CellRef, Workbook};
use varium_slides::Deck;

use crate::slides_ui::SlidesApp;
use crate::suite::Module;
use crate::templates;
use crate::tr::{n_, tr};

/// Clé d'enregistrement de « Afficher au démarrage ».
pub const STORAGE_KEY: &str = "varium.accueil";

/// Durée d'affichage d'une astuce, en secondes.
const TIP_SECONDS: f64 = 9.0;

/// Les astuces, en français (traduites à l'affichage).
pub const TIPS: &[&str] = &[
    n_(
        "Ctrl+K ouvre la palette de commandes : tapez quelques lettres pour retrouver n'importe quelle action.",
    ),
    n_(
        "F1 affiche l'aide-mémoire des raccourcis du module actif, à imprimer ou à exporter en HTML.",
    ),
    n_(
        "Écrivez {{client}} dans un document : le champ se remplit une seule fois, partout où il apparaît.",
    ),
    n_(
        "Glissez un fichier sur la fenêtre : il s'ouvre tout seul dans Documents, Classeur ou Présentation.",
    ),
    n_(
        "Convertir → Document → Présentation transforme les titres de votre document en diapositives.",
    ),
    n_(
        "Sélectionnez deux colonnes d'un classeur (noms, valeurs) pour remplir d'un coup les champs du document.",
    ),
    n_("Le publipostage produit une lettre par ligne d'un tableau CSV, sans rien recopier."),
    n_(
        "{{= quantite * prix}} calcule un montant directement dans le texte, et le recalcule à chaque modification.",
    ),
    n_("F5 lance le diaporama depuis le début, Maj+F5 depuis la diapositive actuelle."),
    n_("Exportez en PDF, Word, OpenDocument, EPUB ou HTML sans aucun autre logiciel."),
    n_("Après une panne, le document interrompu vous est proposé au lancement suivant."),
    n_("Le mode concentration (Ctrl+Maj+F) ne laisse que la page et votre texte."),
    n_(
        "Le Classeur ouvre les fichiers Excel et LibreOffice, et les réenregistre dans les deux formats.",
    ),
];

/// L'astuce à montrer à l'instant `t` (secondes), décalée de `offset`.
pub fn tip_index(t: f64, offset: i64) -> usize {
    let n = TIPS.len() as i64;
    ((t / TIP_SECONDS).floor() as i64 + offset).rem_euclid(n) as usize
}

/// Ce que l'on choisit sur l'accueil.
#[derive(Clone, PartialEq, Debug)]
pub enum Choix {
    Nouveau(Module),
    /// Un modèle de document (`templates::ALL`).
    Modele(usize),
    ExempleClasseur,
    ExemplePresentation,
    Ouvrir,
    Recent(Module, String),
    Fermer,
}

pub struct Accueil {
    /// « Afficher au démarrage ».
    pub au_demarrage: bool,
    /// Décalage des astuces (boutons ◀ ▶), à partir d'une astuce tirée au lancement.
    offset: i64,
    classeur: Option<Workbook>,
    presentation: Option<Deck>,
}

impl Accueil {
    pub fn new(au_demarrage: bool) -> Self {
        let graine = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        Self {
            au_demarrage,
            offset: graine % TIPS.len() as i64,
            classeur: None,
            presentation: None,
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        slides: &mut SlidesApp,
        recents: &[(Module, String)],
    ) -> Option<Choix> {
        let mut choix = None;
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let w = ui.available_width().min(1040.0);
                    let marge = ((ui.available_width() - w) / 2.0).max(0.0);
                    ui.horizontal(|ui| {
                        ui.add_space(marge);
                        ui.vertical(|ui| {
                            ui.set_width(w);
                            self.contenu(ui, slides, recents, &mut choix);
                        });
                    });
                });
        });
        // L'astuce change toute seule.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(TIP_SECONDS / 3.0));
        choix
    }

    fn contenu(
        &mut self,
        ui: &mut egui::Ui,
        slides: &mut SlidesApp,
        recents: &[(Module, String)],
        choix: &mut Option<Choix>,
    ) {
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(tr("Bienvenue dans Varium"))
                    .size(28.0)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(tr("Fermer l'accueil"))
                    .on_hover_text(tr("Revenir au module ouvert"))
                    .clicked()
                {
                    *choix = Some(Choix::Fermer);
                }
            });
        });
        ui.label(RichText::new(tr("Un document. Toutes ses possibilités.")).weak());
        ui.add_space(14.0);

        // Les trois grands boutons, et « Ouvrir… ».
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(12.0, 12.0);
            for (m, label) in [
                (Module::Documents, tr("Nouveau document")),
                (Module::Classeur, tr("Nouveau classeur")),
                (Module::Presentation, tr("Nouvelle présentation")),
            ] {
                let b = egui::Button::new(
                    RichText::new(format!("{}  {label}", m.icon()))
                        .size(17.0)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(m.color())
                .corner_radius(10.0)
                .min_size(Vec2::new(236.0, 60.0));
                if ui.add(b).clicked() {
                    *choix = Some(Choix::Nouveau(m));
                }
            }
            let b = egui::Button::new(RichText::new(format!("📂  {}", tr("Ouvrir…"))).size(17.0))
                .corner_radius(10.0)
                .min_size(Vec2::new(170.0, 60.0));
            if ui
                .add(b)
                .on_hover_text(tr(
                    "Documents, classeurs et présentations, dans tous les formats pris en charge",
                ))
                .clicked()
            {
                *choix = Some(Choix::Ouvrir);
            }
        });

        ui.add_space(22.0);
        ui.label(RichText::new(tr("Modèles")).size(18.0).strong());
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(14.0, 14.0);
            for (i, t) in templates::ALL.iter().enumerate() {
                let r = carte(ui, tr(t.name), tr(t.description), |ui, rect| {
                    vignette_document(ui, rect, t.icon, t.source)
                });
                if r.clicked() {
                    *choix = Some(Choix::Modele(i));
                }
            }
            let wb = self.classeur.get_or_insert_with(Workbook::sample);
            let r = carte(
                ui,
                tr("Classeur d'exemple"),
                tr("Budget avec formules, formats et graphique"),
                |ui, rect| vignette_classeur(ui, rect, wb),
            );
            if r.clicked() {
                *choix = Some(Choix::ExempleClasseur);
            }
            let deck = self.presentation.get_or_insert_with(Deck::sample);
            let r = carte(
                ui,
                tr("Présentation d'exemple"),
                tr("Toutes les dispositions, des champs et un tableau"),
                |ui, rect| {
                    let h = rect.width() / deck.aspect.ratio();
                    let r = Rect::from_center_size(rect.center(), Vec2::new(rect.width(), h));
                    slides.draw_thumbnail(ui, r, deck);
                },
            );
            if r.clicked() {
                *choix = Some(Choix::ExemplePresentation);
            }
        });

        ui.add_space(22.0);
        ui.label(RichText::new(tr("Fichiers récents")).size(18.0).strong());
        ui.add_space(4.0);
        if recents.is_empty() {
            ui.label(
                RichText::new(tr(
                    "Aucun fichier récent : les fichiers que vous ouvrez ou enregistrez apparaîtront ici.",
                ))
                .weak(),
            );
        }
        for (m, path) in recents.iter().take(12) {
            let p = std::path::Path::new(path);
            let name = crate::files::short_name(path);
            let dir = p
                .parent()
                .map(|d| d.display().to_string())
                .unwrap_or_default();
            let r = ui
                .horizontal(|ui| {
                    ui.label(RichText::new(m.icon()).size(18.0).color(m.color()));
                    let r = ui
                        .add(egui::Label::new(RichText::new(&name).strong()).sense(Sense::click()));
                    ui.label(RichText::new(dir).small().weak());
                    r
                })
                .inner
                .on_hover_text(path)
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if r.clicked() {
                *choix = Some(Choix::Recent(*m, path.clone()));
            }
        }

        ui.add_space(22.0);
        ui.separator();
        let t = ui.input(|i| i.time);
        let tip = tip_index(t, self.offset);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("💡 {}", tr("Astuce"))).strong());
            ui.label(tr(TIPS[tip]));
        });
        ui.horizontal(|ui| {
            if ui
                .small_button("◀")
                .on_hover_text(tr("Astuce précédente"))
                .clicked()
            {
                self.offset -= 1;
            }
            if ui
                .small_button("▶")
                .on_hover_text(tr("Astuce suivante"))
                .clicked()
            {
                self.offset += 1;
            }
            ui.add_space(16.0);
            ui.checkbox(&mut self.au_demarrage, tr("Afficher au démarrage"));
        });
        ui.add_space(12.0);
    }
}

/// Une carte de modèle : vignette, nom, description ; cliquable.
fn carte(
    ui: &mut egui::Ui,
    nom: &str,
    description: &str,
    dessin: impl FnOnce(&mut egui::Ui, Rect),
) -> egui::Response {
    let size = Vec2::new(156.0, 214.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let resp = resp
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(description);
    let v = ui.visuals().clone();
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect.expand(4.0), 8.0, v.widgets.hovered.weak_bg_fill);
    }
    let zone = Rect::from_min_size(rect.min + Vec2::new(8.0, 4.0), Vec2::new(140.0, 160.0));
    dessin(ui, zone);
    let painter = ui.painter_at(rect.expand(4.0));
    let galley = painter.layout(
        nom.to_string(),
        FontId::proportional(13.5),
        v.text_color(),
        rect.width() - 8.0,
    );
    let gy = zone.bottom() + 8.0;
    painter.galley(
        Pos2::new(rect.center().x - galley.size().x / 2.0, gy),
        galley,
        v.text_color(),
    );
    resp
}

/// Vignette d'un modèle de document : une page, les titres en foncé, le texte en gris.
fn vignette_document(ui: &egui::Ui, zone: Rect, icon: &str, source: &str) {
    let h = zone.height();
    let w = h / 1.414;
    let page = Rect::from_center_size(zone.center(), Vec2::new(w, h));
    let painter = ui.painter_at(page.expand(2.0));
    painter.rect_filled(
        page.translate(Vec2::new(2.0, 2.0)),
        3.0,
        Color32::from_black_alpha(40),
    );
    painter.rect_filled(page, 3.0, Color32::WHITE);
    painter.rect_stroke(
        page,
        3.0,
        Stroke::new(1.0, Color32::from_gray(200)),
        egui::StrokeKind::Inside,
    );
    let left = page.left() + 10.0;
    let max = page.width() - 20.0;
    let mut y = page.top() + 12.0;
    painter.text(
        Pos2::new(page.right() - 10.0, y),
        egui::Align2::RIGHT_TOP,
        icon,
        FontId::proportional(16.0),
        Color32::from_rgb(0x1F, 0x5F, 0xBF),
    );
    for line in source.lines() {
        if y > page.bottom() - 10.0 {
            break;
        }
        let t = line.trim();
        if t.is_empty() {
            y += 4.0;
            continue;
        }
        let titre = t.starts_with('#');
        let tableau = t.starts_with('|');
        let n = t.chars().count() as f32;
        let (epais, couleur) = if titre {
            (4.0, Color32::from_rgb(0x1F, 0x3F, 0x7F))
        } else if tableau {
            (2.5, Color32::from_gray(150))
        } else {
            (2.0, Color32::from_gray(185))
        };
        let lw = if tableau {
            max
        } else {
            (n * 1.6).clamp(14.0, max)
        };
        let x = if t.starts_with("->") {
            left + (max - lw) / 2.0
        } else {
            left
        };
        painter.line_segment(
            [Pos2::new(x, y), Pos2::new(x + lw, y)],
            Stroke::new(epais, couleur),
        );
        y += if titre { 9.0 } else { 6.0 };
    }
}

/// Vignette d'un classeur : la grille et les premières valeurs.
fn vignette_classeur(ui: &egui::Ui, zone: Rect, wb: &Workbook) {
    let painter = ui.painter_at(zone.expand(2.0));
    painter.rect_filled(
        zone.translate(Vec2::new(2.0, 2.0)),
        3.0,
        Color32::from_black_alpha(40),
    );
    painter.rect_filled(zone, 3.0, Color32::WHITE);
    let (rows, cols) = (11u32, 3u32);
    let head = Color32::from_rgb(0x1E, 0x8A, 0x4C);
    let rh = zone.height() / rows as f32;
    let cw = zone.width() / cols as f32;
    painter.rect_filled(
        Rect::from_min_size(zone.min, Vec2::new(zone.width(), rh)),
        0.0,
        head.gamma_multiply(0.25),
    );
    for r in 0..rows {
        for c in 0..cols {
            let cell = Rect::from_min_size(
                zone.min + Vec2::new(c as f32 * cw, r as f32 * rh),
                Vec2::new(cw, rh),
            );
            painter.rect_stroke(
                cell,
                0.0,
                Stroke::new(0.5, Color32::from_gray(210)),
                egui::StrokeKind::Inside,
            );
            let s = wb.display(0, CellRef::new(r, c));
            if !s.is_empty() {
                let s: String = s.chars().take(9).collect();
                painter.text(
                    cell.left_center() + Vec2::new(2.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    s,
                    FontId::proportional(7.5),
                    if r == 0 { head } else { Color32::from_gray(60) },
                );
            }
        }
    }
    painter.rect_stroke(
        zone,
        3.0,
        Stroke::new(1.0, Color32::from_gray(200)),
        egui::StrokeKind::Inside,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn astuces_en_boucle() {
        assert!(TIPS.len() >= 10);
        assert_eq!(tip_index(0.0, 0), 0);
        assert_eq!(tip_index(TIP_SECONDS * 1.5, 0), 1);
        assert_eq!(tip_index(0.0, -1), TIPS.len() - 1);
        assert_eq!(tip_index(TIP_SECONDS * TIPS.len() as f64, 0), 0);
        assert!((0..100).all(|k| tip_index(k as f64 * 3.7, k - 50) < TIPS.len()));
    }
}
