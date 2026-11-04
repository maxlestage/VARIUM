//! Ouvrir par glisser-déposer, partout : chaque fichier déposé sur la fenêtre s'ouvre
//! dans le bon module (Documents, Classeur ou Présentation), et un voile translucide
//! annonce, pendant le survol, quel module va l'ouvrir. Sert aussi au dialogue
//! « Ouvrir… » de l'accueil, qui accepte tous les formats.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use egui::{Align2, Color32, FontId};

use crate::suite::Module;
use crate::tr::tr;

/// Où va un fichier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cible {
    Module(Module),
    /// Une image : elle s'insère dans la diapositive (Présentation).
    Image,
    /// Un document Word ou OpenDocument texte : Varium les écrit mais ne les lit pas.
    TexteBureautique,
    NonPrisEnCharge,
}

/// Extensions proposées par le dialogue « Ouvrir… » de l'accueil.
pub const EXTENSIONS: &[&str] = &[
    "json", "varium", "md", "txt", "tex", "xlsx", "xlsm", "ods", "csv", "tsv", "odp", "pptx",
];

/// Le module d'un fichier, d'après son nom et, pour un `.json`, son début.
pub fn route(name: &str, head: &[u8]) -> Cible {
    let n = name.to_lowercase();
    let ext = n.rsplit_once('.').map_or("", |(_, e)| e);
    if n.ends_with(".varium-calc.json") {
        return Cible::Module(Module::Classeur);
    }
    if n.ends_with(".varium-slides.json") {
        return Cible::Module(Module::Presentation);
    }
    match ext {
        "json" => {
            let t = String::from_utf8_lossy(head);
            // La première clé connue rencontrée décide.
            let pos = |k: &str| t.find(&format!("\"{k}\"")).unwrap_or(usize::MAX);
            let (doc, calc, deck) = (pos("source"), pos("sheets"), pos("slides"));
            if calc < doc.min(deck) {
                Cible::Module(Module::Classeur)
            } else if deck < doc.min(calc) {
                Cible::Module(Module::Presentation)
            } else {
                Cible::Module(Module::Documents)
            }
        }
        "varium" | "md" | "markdown" | "txt" | "tex" => Cible::Module(Module::Documents),
        "xlsx" | "xlsm" | "ods" | "csv" | "tsv" => Cible::Module(Module::Classeur),
        "odp" | "pptx" => Cible::Module(Module::Presentation),
        "png" | "jpg" | "jpeg" => Cible::Image,
        "odt" | "docx" | "doc" | "rtf" | "pages" => Cible::TexteBureautique,
        _ => Cible::NonPrisEnCharge,
    }
}

/// Le début d'un fichier (pour reconnaître un `.json`).
pub fn head(path: &Path) -> Vec<u8> {
    use std::io::Read;
    let mut buf = vec![0u8; 4096];
    match std::fs::File::open(path).and_then(|mut f| f.read(&mut buf)) {
        Ok(n) => {
            buf.truncate(n);
            buf
        }
        Err(_) => Vec::new(),
    }
}

/// Le nom d'un chemin, pour les messages.
pub fn nom(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Message pour un fichier qui ne s'ouvre pas.
pub fn refus(cible: Cible, name: &str) -> String {
    match cible {
        Cible::TexteBureautique => trf!(
            "« {name} » : Varium exporte en Word et en OpenDocument texte, mais ne sait pas encore les ouvrir",
            name
        ),
        _ => trf!(
            "« {name} » : type de fichier non pris en charge (documents .varium.json, .md, .txt ; classeurs .xlsx, .ods, .csv ; présentations .odp, .pptx)",
            name
        ),
    }
}

/// Le texte du voile pendant le survol.
pub fn annonce(cible: Cible, actif: Module, n: usize) -> String {
    let base = match cible {
        Cible::Module(m) => trf!("Déposer pour ouvrir dans {module}", module = m.label()),
        Cible::Image if actif == Module::Presentation => {
            tr("Déposer pour insérer l'image dans la diapositive").to_string()
        }
        Cible::Image => tr("Déposer pour insérer l'image dans une diapositive").to_string(),
        Cible::TexteBureautique | Cible::NonPrisEnCharge => {
            tr("Ce type de fichier n'est pas pris en charge").to_string()
        }
    };
    if n > 1 {
        format!("{base}  (+{})", n - 1)
    } else {
        base
    }
}

/// Le voile translucide pendant le survol de la fenêtre par des fichiers.
pub fn voile(ctx: &egui::Context, cible: Cible, texte: &str) {
    let rect = ctx.content_rect();
    let couleur = match cible {
        Cible::Module(m) => m.color(),
        Cible::Image => Module::Presentation.color(),
        _ => Color32::from_rgb(0x80, 0x80, 0x80),
    };
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::unique("varium-depot"),
    ));
    let [r, g, b, _] = couleur.to_array();
    painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(r, g, b, 215));
    painter.rect_stroke(
        rect.shrink(12.0),
        12.0,
        egui::Stroke::new(3.0, Color32::WHITE),
        egui::StrokeKind::Inside,
    );
    let icone = match cible {
        Cible::Module(m) => m.icon(),
        Cible::Image => "🖼",
        _ => "⛔",
    };
    painter.text(
        rect.center() - egui::vec2(0.0, 30.0),
        Align2::CENTER_CENTER,
        icone,
        FontId::proportional(64.0),
        Color32::WHITE,
    );
    painter.text(
        rect.center() + egui::vec2(0.0, 36.0),
        Align2::CENTER_CENTER,
        texte,
        FontId::proportional(24.0),
        Color32::WHITE,
    );
}

/// Dialogue « Ouvrir… » de tous les formats ; le fichier choisi arrive dans `boite`.
pub fn choisir(ctx: &egui::Context, boite: &Arc<Mutex<Vec<PathBuf>>>) {
    let boite = boite.clone();
    let ctx = ctx.clone();
    let dialog = rfd::AsyncFileDialog::new()
        .set_title(tr("Ouvrir"))
        .add_filter(tr("Fichiers Varium, Office et OpenDocument"), EXTENSIONS);
    std::thread::spawn(move || {
        if let Some(f) = block_on(dialog.pick_file()) {
            boite
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(f.path().to_path_buf());
            ctx.request_repaint();
        }
    });
}

/// Exécuteur minimal pour le dialogue asynchrone de rfd.
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};
    struct Reveil(std::thread::Thread);
    impl Wake for Reveil {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let mut fut = std::pin::pin!(fut);
    let waker = Waker::from(Arc::new(Reveil(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_fichier_va_dans_son_module() {
        let doc = Cible::Module(Module::Documents);
        let calc = Cible::Module(Module::Classeur);
        let deck = Cible::Module(Module::Presentation);
        assert_eq!(
            route("Lettre.varium.json", b"{\"title\":\"x\",\"source\":\"\"}"),
            doc
        );
        assert_eq!(route("notes.MD", b""), doc);
        assert_eq!(route("a.txt", b""), doc);
        assert_eq!(route("budget.varium-calc.json", b""), calc);
        assert_eq!(route("Budget.XLSX", b"PK"), calc);
        assert_eq!(route("b.ods", b""), calc);
        assert_eq!(route("c.csv", b""), calc);
        assert_eq!(route("d.odp", b""), deck);
        assert_eq!(route("d.pptx", b""), deck);
        assert_eq!(route("d.varium-slides.json", b""), deck);
        assert_eq!(route("photo.JPG", b""), Cible::Image);
        assert_eq!(route("lettre.docx", b""), Cible::TexteBureautique);
        assert_eq!(route("lettre.odt", b""), Cible::TexteBureautique);
        assert_eq!(route("archive.zip", b""), Cible::NonPrisEnCharge);
        assert_eq!(route("sans-extension", b""), Cible::NonPrisEnCharge);
    }

    #[test]
    fn un_json_est_reconnu_a_son_contenu() {
        let calc = br#"{"title":"Budget","sheets":[{"name":"Feuille 1"}]}"#;
        let deck = br#"{"format":"varium-slides","title":"x","slides":[]}"#;
        let doc = br#"{"title":"x","source":"Bonjour","values":{}}"#;
        assert_eq!(route("x.json", calc), Cible::Module(Module::Classeur));
        assert_eq!(route("x.json", deck), Cible::Module(Module::Presentation));
        assert_eq!(route("x.json", doc), Cible::Module(Module::Documents));
        assert_eq!(route("x.json", b"[]"), Cible::Module(Module::Documents));
    }

    #[test]
    fn debut_de_fichier() {
        let dir = std::env::temp_dir().join(format!("varium-depot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.json");
        std::fs::write(&f, vec![b'x'; 10_000]).unwrap();
        assert_eq!(head(&f).len(), 4096);
        assert!(head(&dir.join("absent")).is_empty());
        assert_eq!(nom(&f), "a.json");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
