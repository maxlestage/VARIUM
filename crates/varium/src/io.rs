//! Ouverture, enregistrement et impression (dialogues natifs rfd).

use std::sync::{Arc, Mutex};

/// Ce que l'utilisateur ouvre.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OpenKind {
    /// Un projet `.varium.json` ou un modèle texte.
    Document,
    /// Un tableau CSV pour le publipostage.
    Data,
}

pub enum Event {
    Opened {
        kind: OpenKind,
        name: String,
        content: String,
    },
    Saved(String),
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
        std::mem::take(&mut *self.inbox.lock().unwrap())
    }

    fn push(inbox: &Arc<Mutex<Vec<Event>>>, ctx: &egui::Context, e: Event) {
        inbox.lock().unwrap().push(e);
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
        };
        spawn(async move {
            let Some(file) = dialog.pick_file().await else {
                return;
            };
            let name = file.file_name();
            let bytes = file.read().await;
            Self::push(&inbox, &ctx, decode(kind, name, bytes));
        });
    }

    pub fn save(
        &self,
        ctx: &egui::Context,
        file_name: String,
        mime: &'static str,
        content: Vec<u8>,
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
            let ev = match std::fs::write(&path, content) {
                Ok(()) => Event::Saved(path.display().to_string()),
                Err(e) => Event::Error(format!("Écriture impossible : {e}")),
            };
            Self::push(&inbox, &ctx, ev);
        });
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
fn decode(kind: OpenKind, name: String, bytes: Vec<u8>) -> Event {
    match String::from_utf8(bytes) {
        Ok(content) => Event::Opened {
            kind,
            name,
            content,
        },
        // Les CSV d'Excel sont souvent en Windows-1252 : on les convertit.
        Err(e) if kind == OpenKind::Data => Event::Opened {
            kind,
            name,
            content: e.into_bytes().iter().map(|&b| cp1252(b)).collect(),
        },
        Err(_) => Event::Error(format!("« {name} » n'est pas un fichier texte UTF-8")),
    }
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
