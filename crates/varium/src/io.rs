//! Ouverture, enregistrement et impression, sur le bureau (rfd) et sur le web
//! (rfd asynchrone, téléchargement par Blob, fenêtre d'impression).

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

#[cfg(not(target_arch = "wasm32"))]
pub const OPEN_EXTENSIONS: &[&str] = &["json", "varium", "txt", "md", "tex"];
#[cfg(not(target_arch = "wasm32"))]
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
        #[cfg(target_arch = "wasm32")]
        {
            // Un vrai sélecteur de fichier du navigateur (pas de fenêtre intermédiaire).
            if let Err(e) = pick_web(self.inbox.clone(), ctx.clone(), kind) {
                Self::push(
                    &self.inbox,
                    ctx,
                    Event::Error(format!("Ouverture impossible : {e:?}")),
                );
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
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
    }

    pub fn save(
        &self,
        ctx: &egui::Context,
        file_name: String,
        mime: &'static str,
        content: Vec<u8>,
    ) {
        #[cfg(target_arch = "wasm32")]
        {
            let ev = match download(&file_name, mime, &content) {
                Ok(()) => Event::Saved(file_name),
                Err(e) => Event::Error(format!("Téléchargement impossible : {e:?}")),
            };
            Self::push(&self.inbox, ctx, ev);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
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
    }

    /// Ouvre le document HTML dans le navigateur et lance l'impression
    /// (« Enregistrer au format PDF » dans la fenêtre d'impression).
    pub fn print(&self, ctx: &egui::Context, file_name: String, html: String) {
        let html = html.replacen(
            "</body>",
            "<script>window.addEventListener('load',()=>setTimeout(()=>window.print(),600));</script>\n</body>",
            1,
        );
        #[cfg(target_arch = "wasm32")]
        {
            let ev = match open_window(&html) {
                Ok(true) => Event::Info(
                    "Impression : choisissez « Enregistrer au format PDF » pour obtenir un PDF"
                        .into(),
                ),
                Ok(false) => match download(&file_name, "text/html", html.as_bytes()) {
                    Ok(()) => Event::Info(format!(
                        "Fenêtre bloquée : ouvrez « {file_name} » puis imprimez-le"
                    )),
                    Err(e) => Event::Error(format!("Impression impossible : {e:?}")),
                },
                Err(e) => Event::Error(format!("Impression impossible : {e:?}")),
            };
            Self::push(&self.inbox, ctx, ev);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
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

#[cfg(target_arch = "wasm32")]
fn pick_web(
    inbox: Arc<Mutex<Vec<Event>>>,
    ctx: egui::Context,
    kind: OpenKind,
) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("pas de document")?;
    let input: web_sys::HtmlInputElement = document.create_element("input")?.dyn_into()?;
    input.set_type("file");
    input.set_accept(match kind {
        OpenKind::Document => ".json,.varium,.txt,.md,.tex,application/json,text/plain",
        OpenKind::Data => ".csv,.tsv,.txt,text/csv,text/plain",
    });
    input.set_attribute("style", "display:none")?;
    document.body().ok_or("pas de body")?.append_child(&input)?;
    let target = input.clone();
    let on_change = Closure::once_into_js(move || {
        if let Some(file) = target.files().and_then(|l| l.get(0)) {
            let name = file.name();
            wasm_bindgen_futures::spawn_local(async move {
                let ev = match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
                    Ok(buf) => decode(kind, name, js_sys::Uint8Array::new(&buf).to_vec()),
                    Err(e) => Event::Error(format!("Lecture impossible : {e:?}")),
                };
                Io::push(&inbox, &ctx, ev);
            });
        }
        target.remove();
    });
    input.set_onchange(Some(on_change.unchecked_ref()));
    input.click();
    Ok(())
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
fn spawn(fut: impl std::future::Future<Output = ()> + Send + 'static) {
    std::thread::spawn(move || pollster_block_on(fut));
}

/// Exécuteur minimal pour les dialogues natifs asynchrones de rfd.
#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(target_arch = "wasm32")]
fn blob_url(mime: &str, content: &[u8]) -> Result<String, wasm_bindgen::JsValue> {
    let bytes = js_sys::Uint8Array::from(content);
    let parts = js_sys::Array::of1(&bytes);
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts)?;
    web_sys::Url::create_object_url_with_blob(&blob)
}

#[cfg(target_arch = "wasm32")]
fn download(file_name: &str, mime: &str, content: &[u8]) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;

    let url = blob_url(mime, content)?;
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("pas de document")?;
    let a: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
    a.set_href(&url);
    a.set_download(file_name);
    a.click();
    // Laisse au navigateur le temps de lire le fichier avant de libérer l'adresse.
    let revoke = wasm_bindgen::closure::Closure::once_into_js(move || {
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    if let Some(w) = web_sys::window() {
        let _ =
            w.set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 60_000);
    }
    Ok(())
}

/// Ouvre le HTML dans un nouvel onglet. `Ok(false)` si le navigateur l'a bloqué.
#[cfg(target_arch = "wasm32")]
fn open_window(html: &str) -> Result<bool, wasm_bindgen::JsValue> {
    let url = blob_url("text/html;charset=utf-8", html.as_bytes())?;
    let window = web_sys::window().ok_or("pas de fenêtre")?;
    Ok(window.open_with_url_and_target(&url, "_blank")?.is_some())
}
