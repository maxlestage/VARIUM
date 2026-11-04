//! Apparence de l'interface : thème (automatique, clair, sombre, sépia, contraste élevé)
//! et zoom de l'interface. Le choix est mémorisé et s'applique à toute la suite.

use egui::{Color32, Context, Stroke, Theme, ThemePreference, Visuals};

use crate::tr::tr;

pub const THEME_KEY: &str = "varium_apparence";
pub const ZOOM_KEY: &str = "varium_zoom_interface";
pub const ZOOM_MIN: f32 = 0.8;
pub const ZOOM_MAX: f32 = 2.0;
pub const ZOOM_PAS: f32 = 0.1;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Apparence {
    #[default]
    Automatique,
    Clair,
    Sombre,
    Sepia,
    Contraste,
}

impl Apparence {
    pub const TOUTES: [Apparence; 5] = [
        Apparence::Automatique,
        Apparence::Clair,
        Apparence::Sombre,
        Apparence::Sepia,
        Apparence::Contraste,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Apparence::Automatique => "auto",
            Apparence::Clair => "clair",
            Apparence::Sombre => "sombre",
            Apparence::Sepia => "sepia",
            Apparence::Contraste => "contraste",
        }
    }

    /// Un code inconnu (fichier de préférences d'une autre version) donne `None`.
    pub fn depuis_code(code: &str) -> Option<Self> {
        Self::TOUTES.into_iter().find(|a| a.code() == code)
    }

    pub fn nom(self) -> &'static str {
        match self {
            Apparence::Automatique => tr("Automatique"),
            Apparence::Clair => tr("Clair"),
            Apparence::Sombre => tr("Sombre"),
            Apparence::Sepia => tr("Sépia"),
            Apparence::Contraste => tr("Contraste élevé"),
        }
    }
}

/// Zoom borné ; une valeur absurde (NaN, fichier modifié à la main) revient à 100 %.
pub fn zoom_valide(z: f32) -> f32 {
    if z.is_finite() {
        z.clamp(ZOOM_MIN, ZOOM_MAX)
    } else {
        1.0
    }
}

fn sepia() -> Visuals {
    let mut v = Visuals::light();
    let papier = Color32::from_rgb(244, 236, 216);
    let creux = Color32::from_rgb(232, 221, 196);
    let encre = Color32::from_rgb(67, 52, 34);
    v.panel_fill = papier;
    v.window_fill = papier;
    v.extreme_bg_color = Color32::from_rgb(250, 244, 228);
    v.faint_bg_color = creux;
    v.code_bg_color = creux;
    v.override_text_color = Some(encre);
    v.widgets.noninteractive.bg_fill = papier;
    v.widgets.noninteractive.weak_bg_fill = papier;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(210, 196, 166));
    v.widgets.inactive.bg_fill = creux;
    v.widgets.inactive.weak_bg_fill = creux;
    v.widgets.hovered.bg_fill = Color32::from_rgb(222, 207, 175);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(222, 207, 175);
    v.selection.bg_fill = Color32::from_rgb(196, 150, 84);
    v.hyperlink_color = Color32::from_rgb(140, 70, 20);
    v
}

fn contraste() -> Visuals {
    let mut v = Visuals::dark();
    let blanc = Color32::WHITE;
    let jaune = Color32::from_rgb(255, 221, 0);
    v.panel_fill = Color32::BLACK;
    v.window_fill = Color32::BLACK;
    v.extreme_bg_color = Color32::BLACK;
    v.faint_bg_color = Color32::from_gray(20);
    v.code_bg_color = Color32::from_gray(20);
    v.override_text_color = Some(blanc);
    v.window_stroke = Stroke::new(2.0, blanc);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.bg_stroke = Stroke::new(1.5, blanc);
        w.fg_stroke = Stroke::new(1.5, blanc);
    }
    v.widgets.noninteractive.bg_fill = Color32::BLACK;
    v.widgets.noninteractive.weak_bg_fill = Color32::BLACK;
    v.widgets.inactive.bg_fill = Color32::from_gray(30);
    v.widgets.inactive.weak_bg_fill = Color32::from_gray(30);
    v.widgets.hovered.bg_stroke = Stroke::new(2.0, jaune);
    v.widgets.active.bg_stroke = Stroke::new(2.0, jaune);
    v.selection.bg_fill = Color32::from_rgb(0, 90, 200);
    v.selection.stroke = Stroke::new(2.0, jaune);
    v.hyperlink_color = Color32::from_rgb(120, 200, 255);
    v
}

/// Les couleurs de l'interface pour un thème ; `None` : on laisse le thème de base d'egui.
pub fn visuels(a: Apparence) -> Option<Visuals> {
    match a {
        Apparence::Sepia => Some(sepia()),
        Apparence::Contraste => Some(contraste()),
        _ => None,
    }
}

/// Applique le thème et le zoom. Appelé seulement quand l'un des deux change.
pub fn appliquer(ctx: &Context, a: Apparence, zoom: f32) {
    // On repart toujours des thèmes d'origine : passer de sépia à clair doit tout rétablir.
    ctx.set_visuals_of(Theme::Light, Visuals::light());
    ctx.set_visuals_of(Theme::Dark, Visuals::dark());
    match a {
        Apparence::Automatique => ctx.set_theme(ThemePreference::System),
        Apparence::Clair => ctx.set_theme(ThemePreference::Light),
        Apparence::Sombre => ctx.set_theme(ThemePreference::Dark),
        Apparence::Sepia => {
            ctx.set_visuals_of(Theme::Light, visuels(a).unwrap_or_else(Visuals::light));
            ctx.set_theme(ThemePreference::Light);
        }
        Apparence::Contraste => {
            ctx.set_visuals_of(Theme::Dark, visuels(a).unwrap_or_else(Visuals::dark));
            ctx.set_theme(ThemePreference::Dark);
        }
    }
    ctx.set_zoom_factor(zoom_valide(zoom));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Color32) -> f32 {
        let f = |v: u8| {
            let s = v as f32 / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    fn rapport(a: Color32, b: Color32) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn les_codes_font_l_aller_retour() {
        for a in Apparence::TOUTES {
            assert_eq!(Apparence::depuis_code(a.code()), Some(a));
        }
        assert_eq!(Apparence::depuis_code("inconnu"), None);
    }

    #[test]
    fn le_zoom_est_borne_et_robuste() {
        assert_eq!(zoom_valide(0.1), ZOOM_MIN);
        assert_eq!(zoom_valide(9.0), ZOOM_MAX);
        assert_eq!(zoom_valide(f32::NAN), 1.0);
        assert_eq!(zoom_valide(f32::INFINITY), 1.0);
        assert_eq!(zoom_valide(1.3), 1.3);
    }

    #[test]
    fn le_contraste_eleve_respecte_aaa() {
        let v = visuels(Apparence::Contraste).unwrap();
        let texte = v.override_text_color.unwrap();
        assert!(rapport(texte, v.panel_fill) >= 7.0);
        assert!(rapport(v.hyperlink_color, v.panel_fill) >= 7.0);
    }

    #[test]
    fn la_sepia_reste_lisible() {
        let v = visuels(Apparence::Sepia).unwrap();
        let texte = v.override_text_color.unwrap();
        assert!(rapport(texte, v.panel_fill) >= 7.0);
        assert!(rapport(texte, v.faint_bg_color) >= 4.5);
    }
}
