//! Ouverture, enregistrement et impression (dialogues natifs rfd).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Ce que l'utilisateur ouvre.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OpenKind {
    /// Un projet `.varium.json` ou un modèle texte.
    Document,
    /// Un tableau CSV pour le publipostage.
    Data,
    /// Un classeur (`.varium-calc.json` ou CSV).
    Workbook,
    /// Une présentation (`.varium-slides.json`).
    Deck,
}

pub enum Event {
    Opened {
        kind: OpenKind,
        name: String,
        content: String,
        /// Chemin du fichier (toujours connu avec les dialogues natifs).
        path: Option<PathBuf>,
    },
    /// Un fichier a été écrit ; `project` : c'est le document lui-même
    /// (et non un export), il devient le fichier courant.
    Saved {
        path: PathBuf,
        project: bool,
    },
    Info(String),
    Error(String),
}

#[derive(Clone, Default)]
pub struct Io {
    inbox: Arc<Mutex<Vec<Event>>>,
}

pub const OPEN_EXTENSIONS: &[&str] = &["json", "varium", "txt", "md", "tex"];
pub const DATA_EXTENSIONS: &[&str] = &["csv", "tsv", "txt"];

impl Io {
    pub fn drain(&self) -> Vec<Event> {
        std::mem::take(&mut *self.inbox.lock().unwrap_or_else(|e| e.into_inner()))
    }

    fn push(inbox: &Arc<Mutex<Vec<Event>>>, ctx: &egui::Context, e: Event) {
        inbox.lock().unwrap_or_else(|e| e.into_inner()).push(e);
        ctx.request_repaint();
    }

    pub fn open(&self, ctx: &egui::Context, kind: OpenKind) {
        let inbox = self.inbox.clone();
        let ctx = ctx.clone();
        let dialog = match kind {
            OpenKind::Document => rfd::AsyncFileDialog::new()
                .set_title("Ouvrir un document Varium")
                .add_filter("Varium / texte", OPEN_EXTENSIONS),
            OpenKind::Data => rfd::AsyncFileDialog::new()
                .set_title("Choisir les données du publipostage")
                .add_filter("Tableau CSV", DATA_EXTENSIONS),
            OpenKind::Workbook => rfd::AsyncFileDialog::new()
                .set_title("Ouvrir un classeur")
                .add_filter(
                    "Classeur (Varium, Excel, OpenDocument, CSV)",
                    &["json", "xlsx", "xlsm", "ods", "csv", "tsv", "txt"],
                ),
            OpenKind::Deck => rfd::AsyncFileDialog::new()
                .set_title("Ouvrir une présentation")
                .add_filter(
                    "Présentation (Varium, Impress, PowerPoint)",
                    &["json", "odp", "pptx"],
                )
                .add_filter("Présentation Varium", &["json"])
                .add_filter("LibreOffice Impress", &["odp"])
                .add_filter("PowerPoint", &["pptx"]),
        };
        spawn(async move {
            let Some(file) = dialog.pick_file().await else {
                return;
            };
            let name = file.file_name();
            let path = Some(file.path().to_path_buf());
            let bytes = file.read().await;
            Self::push(&inbox, &ctx, decode(kind, name, path, bytes));
        });
    }

    /// Ouvre directement un fichier connu (documents récents).
    pub fn open_path(&self, ctx: &egui::Context, kind: OpenKind, path: &Path) {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into(),
        );
        let ev = match std::fs::read(path) {
            Ok(bytes) => decode(kind, name, Some(path.to_path_buf()), bytes),
            Err(e) => Event::Error(format!("Ouverture impossible de « {name} » : {e}")),
        };
        Self::push(&self.inbox, ctx, ev);
    }

    pub fn save(
        &self,
        ctx: &egui::Context,
        file_name: String,
        mime: &'static str,
        content: Vec<u8>,
        project: bool,
    ) {
        let _ = mime;
        let inbox = self.inbox.clone();
        let ctx = ctx.clone();
        let dialog = rfd::AsyncFileDialog::new()
            .set_title("Enregistrer")
            .set_file_name(&file_name);
        spawn(async move {
            let Some(handle) = dialog.save_file().await else {
                return;
            };
            let path = handle.path().to_path_buf();
            let ev = match write_atomic(&path, &content) {
                Ok(()) => Event::Saved { path, project },
                Err(e) => Event::Error(format!("Écriture impossible : {e}")),
            };
            Self::push(&inbox, &ctx, ev);
        });
    }

    /// Réécrit un fichier déjà connu (Enregistrer), sans dialogue.
    pub fn write_to(&self, ctx: &egui::Context, path: PathBuf, content: Vec<u8>) {
        let ev = match write_atomic(&path, &content) {
            Ok(()) => Event::Saved {
                path,
                project: true,
            },
            Err(e) => Event::Error(format!("Écriture impossible : {e}")),
        };
        Self::push(&self.inbox, ctx, ev);
    }

    /// Ouvre le document HTML dans le navigateur et lance l'impression
    /// (« Enregistrer au format PDF » dans la fenêtre d'impression).
    pub fn print(&self, ctx: &egui::Context, file_name: String, html: String) {
        let html = html.replacen(
            "</body>",
            "<script>window.addEventListener('load',()=>setTimeout(()=>window.print(),600));</script>\n</body>",
            1,
        );
        let path = std::env::temp_dir().join(file_name);
        let ev = match std::fs::write(&path, html) {
            Ok(()) => match open_in_browser(&path) {
                Ok(()) => Event::Info(
                    "Document ouvert dans le navigateur : imprimez-le ou enregistrez-le en PDF"
                        .into(),
                ),
                Err(e) => Event::Error(format!(
                    "Navigateur introuvable ({e}) : ouvrez {}",
                    path.display()
                )),
            },
            Err(e) => Event::Error(format!("Écriture impossible : {e}")),
        };
        Self::push(&self.inbox, ctx, ev);
    }
}

/// Le contenu d'un fichier ouvert, en texte.
fn decode(kind: OpenKind, name: String, path: Option<PathBuf>, bytes: Vec<u8>) -> Event {
    // Présentations ODP et PPTX (archives ZIP) : transmises octet pour caractère
    // (U+0000 à U+00FF), que le module Présentation reconvertit en octets.
    if kind == OpenKind::Deck && (bytes.starts_with(b"PK\x03\x04") || is_zip_deck(&name)) {
        return Event::Opened {
            kind,
            name,
            content: bytes.iter().map(|&b| b as char).collect(),
            path,
        };
    }
    match String::from_utf8(bytes) {
        Ok(content) => Event::Opened {
            kind,
            name,
            content,
            path,
        },
        // Les CSV d'Excel sont souvent en Windows-1252 : on les convertit.
        Err(e) if matches!(kind, OpenKind::Data | OpenKind::Workbook) => Event::Opened {
            kind,
            name,
            content: e.into_bytes().iter().map(|&b| cp1252(b)).collect(),
            path,
        },
        Err(_) => Event::Error(format!("« {name} » n'est pas un fichier texte UTF-8")),
    }
}

/// Présentation enregistrée dans une archive (`.odp`, `.pptx`).
fn is_zip_deck(name: &str) -> bool {
    let n = name.to_lowercase();
    n.ends_with(".odp") || n.ends_with(".pptx")
}

/// Écrit dans un fichier temporaire voisin puis le renomme : un plantage ou un
/// disque plein ne laisse jamais un document à moitié écrit.
pub fn write_atomic(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".varium-tmp");
    let tmp = PathBuf::from(tmp);
    let res = std::fs::write(&tmp, content).and_then(|()| std::fs::rename(&tmp, path));
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

/// Windows-1252 → caractère (pour les CSV enregistrés par Excel).
fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž',
        '\u{8F}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}',
        'ž', 'Ÿ',
    ];
    match b {
        0x80..=0x9F => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

fn open_in_browser(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(path).spawn().map(|_| ())
}

fn spawn(fut: impl std::future::Future<Output = ()> + Send + 'static) {
    std::thread::spawn(move || pollster_block_on(fut));
}

/// Exécuteur minimal pour les dialogues natifs asynchrones de rfd.
fn pollster_block_on<F: std::future::Future>(fut: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};

    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    let mut fut = std::pin::pin!(fut);
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
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
    fn ecriture_atomique_remplace_le_fichier() {
        let dir = std::env::temp_dir().join(format!("varium-io-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("doc.varium.json");
        write_atomic(&f, b"un").unwrap();
        write_atomic(&f, b"deux").unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"deux");
        assert!(!dir.join("doc.varium.json.varium-tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ecriture_dans_un_dossier_absent_echoue_proprement() {
        let f = std::env::temp_dir().join("varium-absent-xyz/dossier/doc.json");
        assert!(write_atomic(&f, b"x").is_err());
    }

    #[test]
    fn decode_transmet_les_presentations_binaires() {
        let octets = vec![b'P', b'K', 3, 4, 0xff, 0x80, 0];
        match decode(OpenKind::Deck, "a.pptx".into(), None, octets.clone()) {
            Event::Opened { content, .. } => {
                let relu: Vec<u8> = content.chars().map(|c| c as u32 as u8).collect();
                assert_eq!(relu, octets);
            }
            _ => panic!("présentation binaire attendue"),
        }
    }

    #[test]
    fn decode_refuse_le_binaire_mais_accepte_le_csv_windows() {
        assert!(matches!(
            decode(OpenKind::Document, "a.txt".into(), None, vec![0xff, 0xfe]),
            Event::Error(_)
        ));
        match decode(OpenKind::Data, "a.csv".into(), None, vec![b'a', 0x80]) {
            Event::Opened { content, .. } => assert_eq!(content, "a€"),
            _ => panic!("CSV Windows-1252 attendu"),
        }
    }
}
