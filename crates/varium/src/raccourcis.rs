//! Aide-mémoire des raccourcis (F1 ou Ctrl+/) : tous les raccourcis du module actif,
//! tirés des mêmes listes que la palette de commandes (donc toujours exacts), avec
//! une recherche, et à imprimer ou exporter en HTML.

use egui::{Color32, RichText};

use crate::palette::fold;
use crate::tr::tr;

/// Une ligne de l'aide-mémoire : (groupe, libellé, raccourci), déjà traduits.
pub type Ligne = (String, String, String);

/// Ce que l'on demande depuis l'aide-mémoire.
pub enum Demande {
    ExportHtml,
    Imprimer,
}

/// Les lignes qui correspondent à la recherche (libellé, groupe ou touches).
pub fn filtrer<'a>(lignes: &'a [Ligne], query: &str) -> Vec<&'a Ligne> {
    let q = fold(query);
    let compact = |s: &str| s.to_lowercase().replace(' ', "");
    let touches = compact(query);
    lignes
        .iter()
        .filter(|(g, l, k)| {
            q.is_empty() || {
                let hay = fold(&format!("{l} {g}"));
                q.split(' ').all(|t| hay.contains(t)) || compact(k).contains(&touches)
            }
        })
        .collect()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Page HTML autonome, prête à imprimer : un tableau par groupe.
pub fn html(titre: &str, lignes: &[Ligne]) -> String {
    let mut out = format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>{t}</title>\n<style>\
body{{font-family:system-ui,sans-serif;margin:2em;color:#222}}\
h1{{font-size:1.5em}}h2{{font-size:1.1em;margin-top:1.4em;border-bottom:1px solid #ccc}}\
table{{border-collapse:collapse;width:100%}}td{{padding:.25em .5em}}\
td.k{{text-align:right;white-space:nowrap}}kbd{{border:1px solid #bbb;border-radius:3px;\
padding:0 .35em;background:#f4f4f4;font-family:inherit}}\
@media print{{body{{margin:1cm}}h2{{break-after:avoid}}}}\
</style></head><body>\n<h1>{t}</h1>\n",
        t = esc(titre)
    );
    let mut groupe: Option<&str> = None;
    for (g, l, k) in lignes {
        if groupe != Some(g.as_str()) {
            if groupe.is_some() {
                out.push_str("</table>\n");
            }
            out.push_str(&format!("<h2>{}</h2>\n<table>\n", esc(g)));
            groupe = Some(g);
        }
        out.push_str(&format!(
            "<tr><td>{}</td><td class=\"k\"><kbd>{}</kbd></td></tr>\n",
            esc(l),
            esc(k)
        ));
    }
    if groupe.is_some() {
        out.push_str("</table>\n");
    }
    out.push_str("</body></html>\n");
    out
}

#[derive(Default)]
pub struct AideMemoire {
    pub open: bool,
    query: String,
}

impl AideMemoire {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.query.clear();
    }

    /// Affiche l'aide-mémoire du module `titre`.
    pub fn show(&mut self, ctx: &egui::Context, titre: &str, lignes: &[Ligne]) -> Option<Demande> {
        if !self.open {
            return None;
        }
        let mut demande = None;
        let width = (ctx.content_rect().width() - 32.0).clamp(280.0, 700.0);
        let height = (ctx.content_rect().height() - 220.0).clamp(160.0, 560.0);
        let modal = egui::Modal::new(egui::Id::unique("varium-aide-memoire"))
            .backdrop_color(Color32::from_black_alpha(70))
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.horizontal(|ui| {
                    ui.heading(titre);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("✕").on_hover_text(tr("Fermer")).clicked() {
                            ui.close();
                        }
                    });
                });
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text(tr("Rechercher un raccourci…"))
                        .desired_width(f32::INFINITY),
                );
                r.request_focus();
                ui.add_space(4.0);
                let vues = filtrer(lignes, &self.query);
                egui::ScrollArea::vertical()
                    .max_height(height)
                    .min_scrolled_height(height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if vues.is_empty() {
                            ui.label(RichText::new(tr("Aucun raccourci ne correspond")).weak());
                        }
                        let mut groupe: Option<&str> = None;
                        for (g, l, k) in vues {
                            if groupe != Some(g.as_str()) {
                                ui.add_space(6.0);
                                ui.label(RichText::new(g).strong());
                                groupe = Some(g);
                            }
                            ui.horizontal(|ui| {
                                ui.label(l);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.add_space(14.0);
                                        ui.label(RichText::new(k).monospace());
                                    },
                                );
                            });
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr("Exporter en HTML…")).clicked() {
                        demande = Some(Demande::ExportHtml);
                    }
                    if ui.button(tr("Imprimer…")).clicked() {
                        demande = Some(Demande::Imprimer);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(tr("Fermer")).clicked() {
                            ui.close();
                        }
                    });
                });
            });
        if modal.should_close() {
            self.open = false;
        }
        demande
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lignes() -> Vec<Ligne> {
        vec![
            ("Fichier".into(), "Enregistrer".into(), "Ctrl+S".into()),
            ("Fichier".into(), "Ouvrir…".into(), "Ctrl+O".into()),
            ("Format".into(), "Gras <b>".into(), "Ctrl+B".into()),
        ]
    }

    #[test]
    fn recherche_dans_l_aide_memoire() {
        let l = lignes();
        assert_eq!(filtrer(&l, "").len(), 3);
        assert_eq!(filtrer(&l, "ctrl+s").len(), 1);
        assert_eq!(filtrer(&l, "fichier").len(), 2);
        assert_eq!(filtrer(&l, "ÉNREG").len(), 1);
        assert!(filtrer(&l, "zzz").is_empty());
    }

    #[test]
    fn export_html_groupe_et_echappe() {
        let h = html("Raccourcis — Documents", &lignes());
        assert!(h.starts_with("<!DOCTYPE html>"));
        assert_eq!(h.matches("<h2>").count(), 2);
        assert_eq!(h.matches("<table>").count(), h.matches("</table>").count());
        assert!(h.contains("Gras &lt;b&gt;"));
        assert!(h.contains("<kbd>Ctrl+S</kbd>"));
        assert!(h.contains("<title>Raccourcis — Documents</title>"));
    }
}
