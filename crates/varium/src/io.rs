//! Ouverture / enregistrement de fichiers, sur bureau (rfd) et sur le web
//! (rfd asynchrone + téléchargement via Blob).

use std::sync::{Arc, Mutex};

pub enum Event {
    Opened { name: String, content: String },
    Saved(String),
    Error(String),
}

#[derive(Clone, Default)]
pub struct Io {
    inbox: Arc<Mutex<Vec<Event>>>,
}

pub const OPEN_EXTENSIONS: &[&str] = &["json", "varium", "txt", "md", "tex"];

impl Io {
    pub fn drain(&self) -> Vec<Event> {
        std::mem::take(&mut *self.inbox.lock().unwrap())
    }

    fn push(inbox: &Arc<Mutex<Vec<Event>>>, ctx: &egui::Context, e: Event) {
        inbox.lock().unwrap().push(e);
        ctx.request_repaint();
    }

    pub fn open(&self, ctx: &egui::Context) {
        let inbox = self.inbox.clone();
        let ctx = ctx.clone();
        let dialog = rfd::AsyncFileDialog::new()
            .set_title("Ouvrir un document Varium")
            .add_filter("Varium / texte", OPEN_EXTENSIONS);
        spawn(async move {
            let Some(file) = dialog.pick_file().await else {
                return;
            };
            let name = file.file_name();
            let bytes = file.read().await;
            let ev = match String::from_utf8(bytes) {
                Ok(content) => Event::Opened { name, content },
                Err(_) => Event::Error(format!("« {name} » n'est pas un fichier texte UTF-8")),
            };
            Self::push(&inbox, &ctx, ev);
        });
    }

    pub fn save(
        &self,
        ctx: &egui::Context,
        file_name: String,
        mime: &'static str,
        content: String,
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
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn(fut: impl std::future::Future<Output = ()> + Send + 'static) {
    std::thread::spawn(move || pollster_block_on(fut));
}

#[cfg(target_arch = "wasm32")]
fn spawn(fut: impl std::future::Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(fut);
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
fn download(file_name: &str, mime: &str, content: &str) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;

    let parts = js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(content));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(&format!("{mime};charset=utf-8"));
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("pas de document")?;
    let a: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
    a.set_href(&url);
    a.set_download(file_name);
    a.click();
    web_sys::Url::revoke_object_url(&url)?;
    Ok(())
}
