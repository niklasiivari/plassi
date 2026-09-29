//! The browser: the project in local storage, work in slices between frames,
//! files as downloads.

use std::path::Path;

use anyhow::{Result, anyhow};
use wasm_bindgen::JsCast;
use web_time::{Duration, Instant};

use super::{Pdfs, Saved};

/// A result filled in later by browser callbacks.
type Slot<T> = std::rc::Rc<std::cell::RefCell<Option<T>>>;
use crate::seating::{Outcome, Search};

/// The project, kept in the browser's local storage.
#[derive(Clone)]
pub struct Store {
    key: String,
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

impl Store {
    pub fn new(location: &str) -> Self {
        Self { key: format!("plassi:{location}") }
    }

    pub fn location(&self) -> String {
        "this browser".into()
    }

    pub fn read(&self) -> Option<Vec<u8>> {
        local_storage()?.get_item(&self.key).ok()?.map(String::into_bytes)
    }

    pub fn write(&self, bytes: &[u8]) -> Result<()> {
        let text = std::str::from_utf8(bytes)?;
        local_storage()
            .ok_or_else(|| anyhow!("this browser has no local storage"))?
            .set_item(&self.key, text)
            .map_err(|_| anyhow!("the browser refused to store the project (storage full?)"))
    }
}

/// A seating search advanced a little every frame, so the page stays responsive.
pub struct SearchRunner(Option<Search>);

/// Search time per frame.
const SLICE: Duration = Duration::from_millis(30);

impl SearchRunner {
    pub fn start(search: Search) -> Self {
        Self(Some(search))
    }

    pub fn poll(&mut self) -> Option<Outcome> {
        let search = self.0.as_mut()?;
        search.advance(Instant::now() + SLICE);
        if search.done() { self.0.take().map(Search::finish) } else { None }
    }
}

/// Offers `bytes` as a download named `name`.
pub fn save_file(_store: &Store, name: &str, bytes: &[u8]) -> Result<Saved> {
    let fail = |_| anyhow!("the browser refused the download");
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::of1(&array);
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(if name.ends_with(".pdf") { "application/pdf" } else { "application/json" });
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(fail)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(fail)?;
    let document = web_sys::window().and_then(|w| w.document()).ok_or_else(|| anyhow!("no document"))?;
    let anchor: web_sys::HtmlAnchorElement = document.create_element("a").map_err(fail)?.dyn_into().map_err(|_| anyhow!("no anchor"))?;
    anchor.set_href(&url);
    anchor.set_download(name);
    anchor.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(Saved { description: format!("downloaded {name}"), location: None })
}

pub fn open_location(_path: &Path) -> Result<()> {
    Ok(())
}

pub fn save_folder(_store: &Store) -> Option<std::path::PathBuf> {
    None
}

/// A file the user picked in the browser's file dialog, read in the background.
pub struct FilePick(Slot<Result<Vec<u8>, String>>);

impl FilePick {
    /// Opens the file dialog for files matching `accept`, e.g. ".json".
    pub fn open(accept: &str) -> Option<Self> {
        use wasm_bindgen::closure::Closure;
        let document = web_sys::window()?.document()?;
        let input: web_sys::HtmlInputElement = document.create_element("input").ok()?.dyn_into().ok()?;
        input.set_type("file");
        input.set_accept(accept);
        let slot = std::rc::Rc::new(std::cell::RefCell::new(None));
        let (target, result) = (input.clone(), slot.clone());
        let on_change = Closure::once_into_js(move || {
            let Some(file) = target.files().and_then(|files| files.get(0)) else { return };
            wasm_bindgen_futures::spawn_local(async move {
                let read = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await;
                *result.borrow_mut() = Some(
                    read.map(|buffer| js_sys::Uint8Array::new(&buffer).to_vec()).map_err(|_| "could not read the file".to_string()),
                );
            });
        });
        input.set_onchange(Some(on_change.unchecked_ref()));
        input.click();
        Some(Self(slot))
    }

    /// The file's bytes, once read.
    pub fn poll(&self) -> Option<Result<Vec<u8>, String>> {
        self.0.borrow_mut().take()
    }
}

#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    /// Defined by index.html: loads the typesetting module on first use and
    /// returns a PDF (Uint8Array) for the template data.
    #[wasm_bindgen(js_name = plassiTypeset, catch)]
    fn plassi_typeset(data: &str) -> Result<js_sys::Promise, wasm_bindgen::JsValue>;
}

async fn typeset(data: &str) -> Result<Vec<u8>, String> {
    let promise = plassi_typeset(data).map_err(|_| "printing is not available on this page".to_string())?;
    let pdf = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|error| error.as_string().unwrap_or_else(|| "typesetting failed".into()))?;
    Ok(js_sys::Uint8Array::new(&pdf).to_vec())
}

/// PDFs typeset by the separately loaded typesetting module.
pub struct PdfJob<K>(Slot<Pdfs<K>>);

impl<K: 'static> PdfJob<K> {
    /// `prints` holds each print's template data, or why there is none.
    pub fn start(prints: Vec<(K, Result<String, String>)>) -> Self {
        let slot = std::rc::Rc::new(std::cell::RefCell::new(None));
        let result = slot.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let mut pdfs = Vec::new();
            for (kind, data) in prints {
                let pdf = match data {
                    Ok(data) => typeset(&data).await,
                    Err(error) => Err(error),
                };
                pdfs.push((kind, pdf));
            }
            *result.borrow_mut() = Some(pdfs);
        });
        Self(slot)
    }

    pub fn poll(&mut self) -> Option<Pdfs<K>> {
        self.0.borrow_mut().take()
    }
}

/// Projects kept in the browser's local storage.
pub struct Library;

const PROJECT_PREFIX: &str = "plassi:project:";

impl Library {
    pub fn open() -> Self {
        Self
    }

    pub fn projects(&self) -> Vec<String> {
        let Some(storage) = local_storage() else { return Vec::new() };
        let count = storage.length().unwrap_or(0);
        let mut names: Vec<String> = (0..count)
            .filter_map(|i| storage.key(i).ok()?)
            .filter_map(|key| key.strip_prefix(PROJECT_PREFIX).map(str::to_string))
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names
    }

    pub fn store(&self, name: &str) -> Store {
        Store::new(&format!("project:{name}"))
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<()> {
        let bytes = self.store(from).read().ok_or_else(|| anyhow!("no project named {from}"))?;
        self.store(to).write(&bytes)?;
        self.delete(from)
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        local_storage()
            .and_then(|s| s.remove_item(&format!("{PROJECT_PREFIX}{name}")).ok())
            .ok_or_else(|| anyhow!("could not delete {name}"))
    }

    pub fn last(&self) -> Option<String> {
        self.setting("last-project")
    }

    pub fn remember(&self, name: &str) {
        self.set_setting("last-project", name);
    }

    /// A small app setting in local storage.
    pub fn setting(&self, key: &str) -> Option<String> {
        local_storage()?.get_item(&format!("plassi:{key}")).ok()?
    }

    pub fn set_setting(&self, key: &str, value: &str) {
        if let Some(storage) = local_storage() {
            let _ = storage.set_item(&format!("plassi:{key}"), value);
        }
    }
}

/// The browser's language, e.g. "fi-FI".
pub fn system_locale() -> Option<String> {
    web_sys::window()?.navigator().language()
}
