//! Langue du document : dates, nombres et montants, typographie et vérification
//! suivent les conventions de la langue choisie (Document → Langue).

use std::cell::Cell;

/// Une langue d'écriture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Lang {
    #[default]
    Fr,
    En,
    Es,
    De,
    It,
    Pt,
}

impl Lang {
    pub const ALL: [Lang; 6] = [Lang::Fr, Lang::En, Lang::Es, Lang::De, Lang::It, Lang::Pt];

    /// Code ISO 639-1 (`fr`).
    pub fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
            Lang::Es => "es",
            Lang::De => "de",
            Lang::It => "it",
            Lang::Pt => "pt",
        }
    }

    /// Pays par défaut (ISO 3166-1), pour ODT et DOCX.
    pub fn country(self) -> &'static str {
        match self {
            Lang::Fr => "FR",
            Lang::En => "GB",
            Lang::Es => "ES",
            Lang::De => "DE",
            Lang::It => "IT",
            Lang::Pt => "PT",
        }
    }

    /// Étiquette BCP 47 (`fr-FR`).
    pub fn tag(self) -> String {
        format!("{}-{}", self.code(), self.country())
    }

    /// Nom de la langue dans la langue elle-même.
    pub fn native(self) -> &'static str {
        match self {
            Lang::Fr => "Français",
            Lang::En => "English",
            Lang::Es => "Español",
            Lang::De => "Deutsch",
            Lang::It => "Italiano",
            Lang::Pt => "Português",
        }
    }

    /// Nom de la langue en français (interface).
    pub fn label(self) -> &'static str {
        match self {
            Lang::Fr => "Français (France)",
            Lang::En => "Anglais (Royaume-Uni)",
            Lang::Es => "Espagnol (Espagne)",
            Lang::De => "Allemand (Allemagne)",
            Lang::It => "Italien (Italie)",
            Lang::Pt => "Portugais (Portugal)",
        }
    }

    /// Nom de la langue (et du pays) dans la langue de l'interface `ui`.
    pub fn label_in(self, ui: Lang) -> &'static str {
        let t: [&'static str; 6] = match self {
            Lang::Fr => [
                "Français (France)",
                "French (France)",
                "Francés (Francia)",
                "Französisch (Frankreich)",
                "Francese (Francia)",
                "Francês (França)",
            ],
            Lang::En => [
                "Anglais (Royaume-Uni)",
                "English (United Kingdom)",
                "Inglés (Reino Unido)",
                "Englisch (Vereinigtes Königreich)",
                "Inglese (Regno Unito)",
                "Inglês (Reino Unido)",
            ],
            Lang::Es => [
                "Espagnol (Espagne)",
                "Spanish (Spain)",
                "Español (España)",
                "Spanisch (Spanien)",
                "Spagnolo (Spagna)",
                "Espanhol (Espanha)",
            ],
            Lang::De => [
                "Allemand (Allemagne)",
                "German (Germany)",
                "Alemán (Alemania)",
                "Deutsch (Deutschland)",
                "Tedesco (Germania)",
                "Alemão (Alemanha)",
            ],
            Lang::It => [
                "Italien (Italie)",
                "Italian (Italy)",
                "Italiano (Italia)",
                "Italienisch (Italien)",
                "Italiano (Italia)",
                "Italiano (Itália)",
            ],
            Lang::Pt => [
                "Portugais (Portugal)",
                "Portuguese (Portugal)",
                "Portugués (Portugal)",
                "Portugiesisch (Portugal)",
                "Portoghese (Portogallo)",
                "Português (Portugal)",
            ],
        };
        t[Lang::ALL.iter().position(|&l| l == ui).unwrap_or(0)]
    }

    pub fn months(self) -> [&'static str; 12] {
        match self {
            Lang::Fr => [
                "janvier",
                "février",
                "mars",
                "avril",
                "mai",
                "juin",
                "juillet",
                "août",
                "septembre",
                "octobre",
                "novembre",
                "décembre",
            ],
            Lang::En => [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ],
            Lang::Es => [
                "enero",
                "febrero",
                "marzo",
                "abril",
                "mayo",
                "junio",
                "julio",
                "agosto",
                "septiembre",
                "octubre",
                "noviembre",
                "diciembre",
            ],
            Lang::De => [
                "Januar",
                "Februar",
                "März",
                "April",
                "Mai",
                "Juni",
                "Juli",
                "August",
                "September",
                "Oktober",
                "November",
                "Dezember",
            ],
            Lang::It => [
                "gennaio",
                "febbraio",
                "marzo",
                "aprile",
                "maggio",
                "giugno",
                "luglio",
                "agosto",
                "settembre",
                "ottobre",
                "novembre",
                "dicembre",
            ],
            Lang::Pt => [
                "janeiro",
                "fevereiro",
                "março",
                "abril",
                "maio",
                "junho",
                "julho",
                "agosto",
                "setembro",
                "outubro",
                "novembro",
                "dezembro",
            ],
        }
    }

    /// Jours de la semaine, dimanche en premier.
    pub fn weekdays(self) -> [&'static str; 7] {
        match self {
            Lang::Fr => [
                "dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi",
            ],
            Lang::En => [
                "Sunday",
                "Monday",
                "Tuesday",
                "Wednesday",
                "Thursday",
                "Friday",
                "Saturday",
            ],
            Lang::Es => [
                "domingo",
                "lunes",
                "martes",
                "miércoles",
                "jueves",
                "viernes",
                "sábado",
            ],
            Lang::De => [
                "Sonntag",
                "Montag",
                "Dienstag",
                "Mittwoch",
                "Donnerstag",
                "Freitag",
                "Samstag",
            ],
            Lang::It => [
                "domenica",
                "lunedì",
                "martedì",
                "mercoledì",
                "giovedì",
                "venerdì",
                "sabato",
            ],
            Lang::Pt => [
                "domingo",
                "segunda-feira",
                "terça-feira",
                "quarta-feira",
                "quinta-feira",
                "sexta-feira",
                "sábado",
            ],
        }
    }

    /// Séparateur décimal.
    pub fn decimal(self) -> char {
        match self {
            Lang::En => '.',
            _ => ',',
        }
    }

    /// Séparateur de milliers.
    pub fn group(self) -> char {
        match self {
            Lang::Fr => crate::filters::THIN_NBSP,
            Lang::En => ',',
            Lang::Es | Lang::De | Lang::It | Lang::Pt => '.',
        }
    }

    /// Guillemets (ouvrant, fermant) et espace placée à l'intérieur.
    pub fn quotes(self) -> (char, char, Option<char>) {
        match self {
            Lang::Fr => ('«', '»', Some(crate::filters::NBSP)),
            Lang::En | Lang::Pt => ('“', '”', None),
            Lang::De => ('„', '“', None),
            Lang::Es | Lang::It => ('«', '»', None),
        }
    }

    /// Les règles d'écriture de la langue, pour l'aide (Aide → Écrire en plusieurs langues).
    pub fn rules(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Lang::Fr => &[
                (
                    "Guillemets",
                    "« texte », avec espaces insécables à l'intérieur",
                ),
                (
                    "Ponctuation haute",
                    "espace fine insécable avant ; ! ? et insécable avant :",
                ),
                ("Apostrophe", "l’été (apostrophe typographique ’)"),
                (
                    "Nombres",
                    "12 500,50 — espace fine pour les milliers, virgule décimale",
                ),
                ("Montants", "12 500,00 €"),
                (
                    "Dates",
                    "1er mars 2026 · dimanche 1er mars 2026 · 01/03/2026",
                ),
                (
                    "Majuscules",
                    "les mois et les jours s'écrivent en minuscules",
                ),
            ],
            Lang::En => &[
                ("Quotes", "“text” and ‘text’ (curly quotes)"),
                ("Punctuation", "no space before ; : ! ?"),
                (
                    "Articles",
                    "a + consonant sound, an + vowel sound (an hour, a user)",
                ),
                ("Pronoun", "I is always a capital letter"),
                (
                    "Numbers",
                    "12,500.50 — comma for thousands, point for decimals",
                ),
                ("Money", "€12,500.00 · $12,500.00 · £12,500.00"),
                ("Dates", "1 March 2026 · Sunday 1 March 2026 · 01/03/2026"),
            ],
            Lang::Es => &[
                ("Comillas", "«texto», sin espacios interiores"),
                (
                    "Interrogación",
                    "¿Pregunta? y ¡exclamación! (signos de apertura obligatorios)",
                ),
                ("Puntuación", "sin espacio antes de ; : ! ?"),
                ("Números", "12.500,50 — punto para los miles, coma decimal"),
                ("Importes", "12.500,00 €"),
                (
                    "Fechas",
                    "1 de marzo de 2026 · domingo, 1 de marzo de 2026 · 01/03/2026",
                ),
                (
                    "Mayúsculas",
                    "los meses y los días se escriben en minúscula",
                ),
            ],
            Lang::De => &[
                ("Anführungszeichen", "„Text“ und ‚Text‘"),
                ("Satzzeichen", "kein Leerzeichen vor ; : ! ?"),
                ("Substantive", "Substantive werden großgeschrieben"),
                (
                    "Zahlen",
                    "12.500,50 — Punkt für Tausender, Komma für Dezimalstellen",
                ),
                ("Beträge", "12.500,00 €"),
                ("Datum", "1. März 2026 · Sonntag, 1. März 2026 · 01.03.2026"),
            ],
            Lang::It => &[
                ("Virgolette", "«testo», senza spazi interni"),
                (
                    "Accenti",
                    "perché, poiché, affinché (accento acuto); po’ con l'apostrofo",
                ),
                ("Punteggiatura", "nessuno spazio prima di ; : ! ?"),
                (
                    "Numeri",
                    "12.500,50 — punto per le migliaia, virgola decimale",
                ),
                ("Importi", "12.500,00 €"),
                (
                    "Date",
                    "1º marzo 2026 · domenica 1º marzo 2026 · 01/03/2026",
                ),
            ],
            Lang::Pt => &[
                ("Aspas", "“texto” (ou «texto» em Portugal)"),
                ("Pontuação", "sem espaço antes de ; : ! ?"),
                (
                    "Números",
                    "12.500,50 — ponto para os milhares, vírgula decimal",
                ),
                ("Valores", "12.500,00 €"),
                (
                    "Datas",
                    "1 de março de 2026 · domingo, 1 de março de 2026 · 01/03/2026",
                ),
                ("Maiúsculas", "os meses e os dias escrevem-se com minúscula"),
            ],
        }
    }
}

thread_local! {
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::Fr) };
}

/// La langue utilisée en ce moment par les filtres (`date`, `money`, `number`).
pub fn current() -> Lang {
    CURRENT.with(Cell::get)
}

/// Exécute `f` avec `lang` comme langue des filtres, puis rétablit la précédente.
pub fn with_lang<R>(lang: Lang, f: impl FnOnce() -> R) -> R {
    struct Restore(Lang);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.with(|c| c.set(self.0));
        }
    }
    let _restore = Restore(CURRENT.with(|c| c.replace(lang)));
    f()
}
