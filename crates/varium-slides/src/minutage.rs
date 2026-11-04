//! Minutage répété : durée de chaque diapositive, pour l'avance automatique.

use crate::Deck;

/// Une durée en « m:ss » (ou « h:mm:ss »).
pub fn format_duration(secs: f32) -> String {
    let t = if secs.is_finite() {
        secs.max(0.0).round() as u64
    } else {
        0
    };
    if t >= 3600 {
        format!("{}:{:02}:{:02}", t / 3600, t / 60 % 60, t % 60)
    } else {
        format!("{}:{:02}", t / 60, t % 60)
    }
}

impl Deck {
    /// Enregistre un minutage répété : `secs[i]` est la durée de la diapositive
    /// `i` (arrondie au dixième ; une durée nulle, négative ou absente efface
    /// celle de la diapositive).
    pub fn set_timings(&mut self, secs: &[f64]) {
        for (i, s) in self.slides.iter_mut().enumerate() {
            s.duration = secs
                .get(i)
                .copied()
                .filter(|d| d.is_finite() && *d >= 0.05)
                .map(|d| ((d.min(86_400.0) * 10.0).round() / 10.0) as f32);
        }
    }

    /// Efface le minutage de toutes les diapositives.
    pub fn clear_timings(&mut self) {
        for s in &mut self.slides {
            s.duration = None;
        }
        self.auto_advance = false;
    }

    /// Durée totale du minutage (secondes) ; `None` si aucune diapositive n'en a.
    pub fn total_duration(&self) -> Option<f32> {
        let mut total = None;
        for d in self.slides.iter().filter_map(|s| s.duration) {
            *total.get_or_insert(0.0) += d;
        }
        total
    }

    /// Durée d'affichage automatique de la diapositive `i`, si l'avance
    /// automatique est active et que la diapositive a un minutage.
    pub fn auto_advance_after(&self, i: usize) -> Option<f32> {
        if !self.auto_advance {
            return None;
        }
        self.slides
            .get(i)
            .and_then(|s| s.duration)
            .filter(|d| d.is_finite() && *d > 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Layout, Slide};

    #[test]
    fn minutage() {
        let mut d = Deck::new();
        d.slides = vec![Slide::new(Layout::Title); 3];
        d.set_timings(&[12.345, 0.0, f64::NAN]);
        assert_eq!(d.slides[0].duration, Some(12.3));
        assert_eq!(d.slides[1].duration, None);
        assert_eq!(d.total_duration(), Some(12.3));
        assert_eq!(d.auto_advance_after(0), None);
        d.auto_advance = true;
        assert_eq!(d.auto_advance_after(0), Some(12.3));
        assert_eq!(d.auto_advance_after(1), None);
        assert_eq!(format_duration(75.4), "1:15");
        assert_eq!(format_duration(3725.0), "1:02:05");
        assert_eq!(format_duration(f32::NAN), "0:00");
        // Fichier : écrit seulement s'il existe, relu de façon tolérante.
        let json = d.to_json();
        assert!(json.contains("\"duration\": 12.3"));
        assert!(json.contains("\"auto_advance\": true"));
        let r = Deck::from_json(&json).unwrap();
        assert_eq!(r.slides[0].duration, Some(12.3));
        assert!(r.auto_advance);
        let ancien = Deck::new().to_json();
        assert!(!ancien.contains("duration") && !ancien.contains("auto_advance"));
        let abime = r#"{"slides":[{"duration":"7,5"},{"duration":-3},{"duration":"x"}],"auto_advance":"oui"}"#;
        let r = Deck::from_json(abime).unwrap();
        assert_eq!(r.slides[0].duration, Some(7.5));
        assert_eq!(r.slides[1].duration, None);
        assert_eq!(r.slides[2].duration, None);
        assert!(r.auto_advance);
        d.clear_timings();
        assert_eq!(d.total_duration(), None);
    }
}
