//! Libellés traduits des énumérations des moteurs (documents, tableur, diapositives),
//! dont les `label()` restent en français.

use crate::tr::tr;
use varium_calc::CompareOp;
use varium_core::check::Kind;
use varium_core::{Lang, Paper, VarType};
use varium_slides::{Layout, ObjectKind, Theme, Transition};

/// Nom d'une langue de document (« Anglais (Royaume-Uni) »).
pub fn lang(l: Lang) -> &'static str {
    match l {
        Lang::Fr => tr("Français (France)"),
        Lang::En => tr("Anglais (Royaume-Uni)"),
        Lang::Es => tr("Espagnol (Espagne)"),
        Lang::De => tr("Allemand (Allemagne)"),
        Lang::It => tr("Italien (Italie)"),
        Lang::Pt => tr("Portugais (Portugal)"),
    }
}

/// Le nom d'une langue au milieu d'une phrase (« dictionnaire français (france) » :
/// en minuscules en français, tel quel ailleurs).
pub fn lang_inline(l: Lang) -> String {
    if crate::tr::lang() == Lang::Fr {
        lang(l).to_lowercase()
    } else {
        lang(l).to_string()
    }
}

pub fn paper(p: Paper) -> &'static str {
    match p {
        Paper::A4 => "A4 (21 × 29,7 cm)",
        Paper::A5 => "A5 (14,8 × 21 cm)",
        Paper::Letter => tr("Lettre US"),
        Paper::Legal => tr("Légal US"),
    }
}

pub fn var_type(t: VarType) -> &'static str {
    match t {
        VarType::Text => tr("Texte"),
        VarType::Long => tr("Texte long"),
        VarType::Date => tr("Date"),
        VarType::Number => tr("Nombre"),
        VarType::Money => tr("Montant"),
        VarType::Email => tr("E-mail"),
    }
}

pub fn issue(k: Kind) -> &'static str {
    match k {
        Kind::Espace => tr("Espace"),
        Kind::Ponctuation => tr("Ponctuation"),
        Kind::Majuscule => tr("Majuscule"),
        Kind::Repetition => tr("Répétition"),
        Kind::Orthographe => tr("Orthographe"),
        Kind::Equilibre => tr("Parenthèses"),
    }
}

pub fn compare(op: CompareOp) -> &'static str {
    match op {
        CompareOp::Gt => tr("supérieure à (>)"),
        CompareOp::Ge => tr("supérieure ou égale à (≥)"),
        CompareOp::Lt => tr("inférieure à (<)"),
        CompareOp::Le => tr("inférieure ou égale à (≤)"),
        CompareOp::Eq => tr("égale à (=)"),
        CompareOp::Ne => tr("différente de (≠)"),
        CompareOp::Between => tr("comprise entre"),
        CompareOp::NotBetween => tr("non comprise entre"),
    }
}

pub fn layout(l: Layout) -> &'static str {
    match l {
        Layout::Title => tr("Titre"),
        Layout::TitleContent => tr("Titre et contenu"),
        Layout::TwoColumns => tr("Deux colonnes"),
        Layout::Section => tr("Section"),
        Layout::Quote => tr("Citation"),
        Layout::TitleOnly => tr("Titre seul"),
        Layout::Blank => tr("Vide"),
    }
}

pub fn transition(t: Transition) -> &'static str {
    match t {
        Transition::Aucune => tr("Aucune"),
        Transition::Fondu => tr("Fondu"),
        Transition::Glissement => tr("Glissement"),
        Transition::Zoom => tr("Zoom"),
    }
}

pub fn theme(t: Theme) -> &'static str {
    match t {
        Theme::Clair => tr("Clair"),
        Theme::Sombre => tr("Sombre"),
        Theme::Ocean => tr("Océan"),
        Theme::Elegant => tr("Élégant"),
        Theme::Craie => tr("Craie"),
        Theme::Corail => tr("Corail"),
        Theme::Papier => tr("Papier"),
        Theme::Minuit => tr("Minuit"),
        Theme::Foret => tr("Forêt"),
        Theme::Agrume => tr("Agrume"),
    }
}

pub fn object(k: ObjectKind) -> &'static str {
    match k {
        ObjectKind::Image => tr("Image"),
        ObjectKind::Rect => tr("Rectangle"),
        ObjectKind::RoundRect => tr("Rectangle arrondi"),
        ObjectKind::Ellipse => tr("Ellipse"),
        ObjectKind::Line => tr("Ligne"),
        ObjectKind::Arrow => tr("Flèche"),
        ObjectKind::Qr => tr("Code QR"),
    }
}
