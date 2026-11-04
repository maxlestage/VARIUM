//! Palette de commandes (Ctrl+K, ⌘K ou Ctrl+Maj+P) : une boîte de recherche au-dessus
//! du module actif, qui retrouve n'importe quelle commande par son nom, dans la langue
//! de l'interface comme en français, sans tenir compte des accents ni de la casse.
//! Les commandes récemment utilisées viennent en premier.

use egui::{Color32, Key, Modifiers, RichText};
use varium_core::Lang;

use crate::app::Act;
use crate::calc_ui::CalcAction;
use crate::slides_ui::SlideAction;
use crate::suite::Module;
use crate::tr::{n_, tr};

/// Libellés de commandes de la Présentation propres à la palette (les sources
/// placées après les tests d'un fichier échappent au contrôle des traductions).
pub const LIBELLE_IMAGE: &str = n_("Image…");
pub const LIBELLE_DUPLIQUER_DIAPO: &str = n_("Dupliquer la diapositive");
pub const LIBELLE_SUPPRIMER_DIAPO: &str = n_("Supprimer la diapositive");

/// Nombre de commandes récentes retenues.
pub const MAX_RECENT: usize = 8;

/// Les conversions d'un module à l'autre.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Conversion {
    /// Le document devient une présentation.
    DocToDeck,
    /// La sélection du classeur devient un tableau, au curseur du document.
    CalcToDoc,
    /// La sélection du classeur devient un tableau, dans un nouveau document.
    CalcToNewDoc,
    /// Deux colonnes (noms, valeurs) donnent leurs valeurs aux champs du document.
    CalcToFields,
}

impl Conversion {
    pub const ALL: [Conversion; 4] = [
        Conversion::DocToDeck,
        Conversion::CalcToDoc,
        Conversion::CalcToNewDoc,
        Conversion::CalcToFields,
    ];

    /// Libellé en français (traduit à l'affichage).
    pub fn label_fr(self) -> &'static str {
        match self {
            Conversion::DocToDeck => crate::tr::n_("Document → Présentation"),
            Conversion::CalcToDoc => crate::tr::n_("Classeur → Document (tableau au curseur)"),
            Conversion::CalcToNewDoc => crate::tr::n_("Classeur → nouveau document"),
            Conversion::CalcToFields => crate::tr::n_("Classeur → Champs du document"),
        }
    }
}

/// Ce que lance une entrée de la palette.
#[derive(Clone, PartialEq, Debug)]
pub enum Cmd {
    Doc(Act),
    Calc(CalcAction),
    Slides(SlideAction),
    Module(Module),
    Accueil,
    Shortcuts,
    UiLang(Option<Lang>),
    /// Nouveau fichier vierge dans un module.
    New(Module),
    /// Dialogue d'ouverture (tous les formats pris en charge).
    OpenAny,
    OpenRecent(Module, String),
    Convert(Conversion),
}

/// Une commande de la palette.
#[derive(Clone, Debug)]
pub struct Entry {
    pub cmd: Cmd,
    /// Identifiant stable (commandes récentes).
    pub id: String,
    /// Libellé dans la langue de l'interface.
    pub label: String,
    /// Libellé en français.
    pub fr: String,
    /// Groupe (menu) dans la langue de l'interface, et en français.
    pub group: String,
    pub group_fr: String,
    /// Raccourci clavier mis en forme (`Ctrl+S`, `⌘S`), ou vide.
    pub keys: String,
}

// ---------------------------------------------------------------------------------
// Recherche approximative
// ---------------------------------------------------------------------------------

/// Minuscules sans accents ; tout ce qui n'est ni lettre ni chiffre devient une espace.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        let r: &str = match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => "a",
            'æ' => "ae",
            'ç' | 'ć' | 'č' => "c",
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => "e",
            'ì' | 'í' | 'î' | 'ï' | 'ī' => "i",
            'ñ' | 'ń' => "n",
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' => "o",
            'œ' => "oe",
            'ù' | 'ú' | 'û' | 'ü' | 'ū' => "u",
            'ý' | 'ÿ' => "y",
            'ß' => "ss",
            c if c.is_alphanumeric() => {
                out.push(c);
                continue;
            }
            _ => " ",
        };
        out.push_str(r);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Score d'un mot cherché dans un texte déjà normalisé (`None` : absent).
fn token_score(tok: &str, hay: &str) -> Option<i32> {
    if hay.starts_with(tok) {
        return Some(130 + tok.len() as i32);
    }
    if hay.contains(&format!(" {tok}")) {
        return Some(100 + tok.len() as i32);
    }
    if hay.contains(tok) {
        return Some(70);
    }
    // Lettres dans l'ordre, pas forcément voisines (« enrs » → « enregistrer sous »).
    let hay: Vec<char> = hay.chars().collect();
    let mut score = 20;
    let mut pos = 0usize;
    let mut prev: Option<usize> = None;
    for c in tok.chars() {
        let i = (pos..hay.len()).find(|&i| hay[i] == c)?;
        if i == 0 || hay[i - 1] == ' ' {
            score += 8;
        } else if prev == Some(i.wrapping_sub(1)) {
            score += 5;
        } else {
            score += 1;
        }
        if let Some(p) = prev {
            score -= ((i - p - 1) as i32).min(6);
        }
        prev = Some(i);
        pos = i + 1;
    }
    // Les lettres trop éparpillées ne comptent pas.
    (score > 20 + tok.chars().count() as i32).then_some(score)
}

/// Score d'une recherche : chaque mot doit se retrouver dans l'un des textes.
/// `None` si la commande ne correspond pas ; une recherche vide correspond à tout.
pub fn score(query: &str, label: &str, fr: &str, group: &str, group_fr: &str) -> Option<i32> {
    let q = fold(query);
    if q.is_empty() {
        return Some(0);
    }
    let (label, fr) = (fold(label), fold(fr));
    let all = format!("{label} {fr} {} {}", fold(group), fold(group_fr));
    let mut best: Option<i32> = None;
    for hay in [&label, &fr] {
        let s: Option<i32> = q.split(' ').map(|t| token_score(t, hay)).sum();
        best = best.max(s.map(|s| s + 20));
    }
    // Les mots peuvent aussi se répartir entre le libellé et son menu.
    let s: Option<i32> = q.split(' ').map(|t| token_score(t, &all)).sum();
    best = best.max(s);
    best.map(|s| s - (label.len() as i32) / 12)
}

/// Les entrées à afficher, dans l'ordre : sans recherche, les récentes puis les
/// autres ; avec une recherche, par score (les récentes un peu favorisées).
pub fn rank(entries: &[Entry], query: &str, recents: &[String]) -> Vec<usize> {
    let recent_rank = |e: &Entry| recents.iter().position(|r| *r == e.id);
    if fold(query).is_empty() {
        let mut out: Vec<usize> = recents
            .iter()
            .filter_map(|r| entries.iter().position(|e| e.id == *r))
            .collect();
        let rest: Vec<usize> = (0..entries.len()).filter(|i| !out.contains(i)).collect();
        out.extend(rest);
        return out;
    }
    let mut scored: Vec<(i32, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let s = score(query, &e.label, &e.fr, &e.group, &e.group_fr)?;
            let bonus = recent_rank(e).map_or(0, |r| 30 - 3 * r as i32);
            Some((s + bonus, i))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).collect()
}

/// Place une commande en tête des récentes.
pub fn push_recent(list: &mut Vec<String>, id: &str) {
    list.retain(|x| x != id);
    list.insert(0, id.to_string());
    list.truncate(MAX_RECENT);
}

// ---------------------------------------------------------------------------------
// Interface
// ---------------------------------------------------------------------------------

#[derive(Default)]
pub struct Palette {
    pub open: bool,
    query: String,
    selected: usize,
}

impl Palette {
    pub fn toggle(&mut self) {
        if self.open {
            self.open = false;
        } else {
            self.open = true;
            self.query.clear();
            self.selected = 0;
        }
    }

    /// Affiche la palette ; renvoie la position de l'entrée choisie.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        entries: &[Entry],
        recents: &[String],
    ) -> Option<usize> {
        if !self.open {
            return None;
        }
        let order = rank(entries, &self.query, recents);
        let n = order.len();
        // Les flèches et Entrée vont à la palette, pas au champ de recherche.
        let (down, up, enter) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        if n > 0 {
            if down {
                self.selected = (self.selected + 1) % n;
            }
            if up {
                self.selected = (self.selected + n - 1) % n;
            }
        }
        self.selected = self.selected.min(n.saturating_sub(1));
        let mut chosen = (enter && n > 0).then(|| order[self.selected]);
        let width = (ctx.content_rect().width() - 32.0).clamp(280.0, 620.0);
        let id = egui::Id::unique("varium-palette");
        let modal = egui::Modal::new(id)
            .area(egui::Modal::default_area(id).anchor(egui::Align2::CENTER_TOP, [0.0, 70.0]))
            .backdrop_color(Color32::from_black_alpha(70))
            .show(ctx, |ui| {
                ui.set_width(width);
                let before = self.query.clone();
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text(tr("Rechercher une commande, un fichier, une langue…"))
                        .font(egui::TextStyle::Heading)
                        .desired_width(f32::INFINITY),
                );
                r.request_focus();
                if self.query != before {
                    self.selected = 0;
                }
                ui.add_space(4.0);
                let h = (ctx.content_rect().height() - 220.0).clamp(120.0, 420.0);
                egui::ScrollArea::vertical()
                    .max_height(h)
                    .min_scrolled_height(h.min(n.max(1) as f32 * 26.0 + 24.0))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if n == 0 {
                            ui.label(RichText::new(tr("Aucune commande ne correspond")).weak());
                        }
                        let recent_head = self.query.trim().is_empty()
                            && order
                                .first()
                                .is_some_and(|&i| recents.contains(&entries[i].id));
                        if recent_head {
                            ui.label(RichText::new(tr("Récemment utilisées")).small().weak());
                        }
                        for (k, &i) in order.iter().enumerate().take(80) {
                            let e = &entries[i];
                            if recent_head
                                && k > 0
                                && recents.contains(&entries[order[k - 1]].id)
                                && !recents.contains(&e.id)
                            {
                                ui.separator();
                            }
                            let on = k == self.selected;
                            let resp = row(ui, e, on);
                            if on && (up || down) {
                                resp.scroll_to_me(None);
                            }
                            if resp.clicked() {
                                chosen = Some(i);
                            }
                            if resp.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO)
                            {
                                self.selected = k;
                            }
                        }
                    });
                ui.add_space(2.0);
                ui.label(
                    RichText::new(tr(
                        "↑ ↓ pour choisir · Entrée pour lancer · Échap pour fermer",
                    ))
                    .small()
                    .weak(),
                );
            });
        if modal.should_close() || chosen.is_some() {
            self.open = false;
        }
        chosen
    }
}

/// Une ligne de la palette : libellé, groupe, raccourci.
fn row(ui: &mut egui::Ui, e: &Entry, on: bool) -> egui::Response {
    let h = 26.0;
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click());
    let v = ui.visuals();
    if on {
        ui.painter()
            .rect_filled(rect, 4.0, v.selection.bg_fill.gamma_multiply(0.6));
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(rect, 4.0, v.widgets.hovered.weak_bg_fill);
    }
    let text = v.text_color();
    let weak = v.weak_text_color();
    let font = egui::FontId::proportional(14.0);
    let small = egui::FontId::proportional(12.0);
    let left = rect.left_center() + egui::vec2(8.0, 0.0);
    let g = ui.painter().layout_no_wrap(e.label.clone(), font, text);
    let lw = g.size().x;
    ui.painter()
        .galley(left - egui::vec2(0.0, g.size().y / 2.0), g, text);
    if !e.group.is_empty() {
        ui.painter().text(
            left + egui::vec2(lw + 10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &e.group,
            small.clone(),
            weak,
        );
    }
    if !e.keys.is_empty() {
        let g = ui.painter().layout_no_wrap(e.keys.clone(), small, text);
        let r = egui::Rect::from_center_size(
            rect.right_center() - egui::vec2(10.0 + g.size().x / 2.0, 0.0),
            g.size() + egui::vec2(10.0, 4.0),
        );
        ui.painter().rect_filled(r, 3.0, v.widgets.inactive.bg_fill);
        ui.painter().galley(r.min + egui::vec2(5.0, 2.0), g, text);
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(id: &str, label: &str, fr: &str, group: &str) -> Entry {
        Entry {
            cmd: Cmd::Accueil,
            id: id.into(),
            label: label.into(),
            fr: fr.into(),
            group: group.into(),
            group_fr: group.into(),
            keys: String::new(),
        }
    }

    #[test]
    fn normalisation_sans_accents_ni_casse() {
        assert_eq!(fold("Éléments À Insérer"), "elements a inserer");
        assert_eq!(fold("Größe – Œuvre"), "grosse oeuvre");
        assert_eq!(fold("  Ouvrir…  (CSV) "), "ouvrir csv");
    }

    #[test]
    fn recherche_approximative() {
        // Préfixe, début de mot, sous-chaîne, lettres dans l'ordre.
        assert!(score("enreg", "Enregistrer", "Enregistrer", "", "").is_some());
        assert!(score("sous", "Enregistrer sous…", "Enregistrer sous…", "", "").is_some());
        assert!(score("enrsous", "Enregistrer sous…", "Enregistrer sous…", "", "").is_some());
        assert!(score("zzz", "Enregistrer", "Enregistrer", "", "").is_none());
        // Accents et casse ignorés, dans les deux sens.
        assert!(score("PRESENTATION", "Présentation", "Présentation", "", "").is_some());
        assert!(score("insérer", "Inserer", "Inserer", "", "").is_some());
        // Langue de l'interface ou français.
        assert!(
            score(
                "save as",
                "Save As…",
                "Enregistrer sous…",
                "File",
                "Fichier"
            )
            .is_some()
        );
        assert!(
            score(
                "enregistrer",
                "Save As…",
                "Enregistrer sous…",
                "File",
                "Fichier"
            )
            .is_some()
        );
        // Mots répartis entre le libellé et le menu.
        assert!(
            score(
                "fichier pdf",
                "Exporter en PDF",
                "Exporter en PDF",
                "Fichier",
                "Fichier"
            )
            .is_some()
        );
        // Le préfixe l'emporte sur les lettres éparpillées.
        let a = score("gras", "Gras", "Gras", "", "").unwrap();
        let b = score("gras", "Largeur de la page sans", "Largeur", "", "").unwrap_or(0);
        assert!(a > b);
    }

    #[test]
    fn classement_et_recentes() {
        let entries = vec![
            e("a", "Gras", "Gras", "Format"),
            e("b", "Italique", "Italique", "Format"),
            e("c", "Grille", "Grille", "Affichage"),
        ];
        assert_eq!(rank(&entries, "", &[]), [0, 1, 2]);
        assert_eq!(rank(&entries, "", &["c".into()]), [2, 0, 1]);
        assert_eq!(rank(&entries, "ital", &[]), [1]);
        let r = rank(&entries, "gr", &[]);
        assert_eq!(r.len(), 2);
        // Une commande récente passe devant à score égal.
        let r = rank(&entries, "gr", &["c".into()]);
        assert_eq!(r[0], 2);
        let mut rec = Vec::new();
        for i in 0..12 {
            push_recent(&mut rec, &i.to_string());
        }
        push_recent(&mut rec, "5");
        assert_eq!(rec.len(), MAX_RECENT);
        assert_eq!(rec[0], "5");
        assert_eq!(rec.iter().filter(|x| *x == "5").count(), 1);
    }
}
