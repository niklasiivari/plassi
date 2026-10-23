//! The browser: the project in local storage, work in isolated workers,
//! files as downloads.

use std::path::Path;

use anyhow::{Result, anyhow};
use wasm_bindgen::JsCast;
use web_time::{Duration, Instant};

use super::{PdfBytes, Pdfs, Saved, SearchRequest};
use crate::i18n::{fill, tr};

/// A result filled in later by browser callbacks.
type Slot<T> = std::rc::Rc<std::cell::RefCell<Option<T>>>;
use crate::seating::{Outcome, Progress, Search};

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
        Self {
            key: format!("plassi:{location}"),
        }
    }

    pub fn location(&self) -> String {
        tr("This browser").into()
    }

    pub fn read(&self) -> Option<Vec<u8>> {
        local_storage()?
            .get_item(&self.key)
            .ok()?
            .map(String::into_bytes)
    }

    pub fn write(&self, bytes: &[u8]) -> Result<()> {
        let text = std::str::from_utf8(bytes)?;
        local_storage()
            .ok_or_else(|| anyhow!(tr("Browser storage unavailable")))?
            .set_item(&self.key, text)
            .map_err(|error| anyhow!(refusal(&error)))
    }
}

fn quota_exceeded(error: &wasm_bindgen::JsValue) -> bool {
    js_sys::Reflect::get(error, &"name".into())
        .ok()
        .and_then(|n| n.as_string())
        .is_some_and(|name| name == "QuotaExceededError")
}

/// Why the browser refused to store something, in words the user can act on.
fn refusal(error: &wasm_bindgen::JsValue) -> &'static str {
    if quota_exceeded(error) {
        crate::i18n::tr(
            "Storage full. Download each project, then clear this site's data in browser settings.",
        )
    } else {
        crate::i18n::tr("The browser blocked saving")
    }
}

thread_local! {
    /// Whether the browser promised to keep local storage; `None` until it answers.
    static PERSISTED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    static REPAINT: std::cell::RefCell<Option<eframe::egui::Context>> = const { std::cell::RefCell::new(None) };
    static EXTERNAL_DIRTY: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    static EXTERNAL_LISTENING: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

pub fn set_repaint(ctx: &eframe::egui::Context) {
    REPAINT.with(|target| *target.borrow_mut() = Some(ctx.clone()));
}

#[wasm_bindgen::prelude::wasm_bindgen(js_name = plassiWake)]
pub fn wake() {
    REPAINT.with(|target| {
        if let Some(ctx) = target.borrow().as_ref() {
            ctx.request_repaint();
        }
    });
}

pub fn request_repaint() {
    wake();
}

fn install_external_notifications() -> bool {
    EXTERNAL_LISTENING.with(|state| {
        if let Some(listening) = state.get() {
            return listening;
        }
        let install = || -> Option<()> {
            let window = web_sys::window()?;
            let document = window.document()?;
            let targets: [(&web_sys::EventTarget, &str); 3] = [
                (window.as_ref(), "storage"),
                (window.as_ref(), "focus"),
                (document.as_ref(), "visibilitychange"),
            ];
            for (target, event) in targets {
                let callback =
                    wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(|_| {
                        EXTERNAL_DIRTY.with(|dirty| dirty.set(true));
                        wake();
                    });
                target
                    .add_event_listener_with_callback(event, callback.as_ref().unchecked_ref())
                    .ok()?;
                // These three listeners live as long as the application.
                callback.forget();
            }
            Some(())
        };
        let listening = install().is_some();
        state.set(Some(listening));
        listening
    })
}

pub fn external_notifications_available() -> bool {
    install_external_notifications()
}

impl Store {
    pub fn notifications_available(&self) -> bool {
        external_notifications_available()
    }

    pub fn external_change_pending(&self) -> bool {
        !install_external_notifications() || EXTERNAL_DIRTY.with(|dirty| dirty.replace(false))
    }
}

/// Asks the browser not to clear this site's storage when space runs low.
pub fn request_persistence() {
    let Some(storage) = web_sys::window().map(|w| w.navigator().storage()) else {
        return;
    };
    wasm_bindgen_futures::spawn_local(async move {
        let ask = |promise: Result<js_sys::Promise, wasm_bindgen::JsValue>| async move {
            let answer = wasm_bindgen_futures::JsFuture::from(promise.ok()?)
                .await
                .ok()?;
            answer.as_bool()
        };
        let persisted = match ask(storage.persisted()).await {
            Some(true) => Some(true),
            _ => ask(storage.persist()).await,
        };
        PERSISTED.with(|p| p.set(Some(persisted.unwrap_or(false))));
        wake();
    });
}

/// Whether the browser keeps local storage until the user clears it.
pub fn storage_persisted() -> Option<bool> {
    PERSISTED.with(std::cell::Cell::get)
}

#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    type BrowserSearch;
    #[wasm_bindgen(js_name = plassiSearchStart, catch)]
    fn start_browser_search(data: &str) -> Result<BrowserSearch, wasm_bindgen::JsValue>;
    #[wasm_bindgen(method)]
    fn poll(this: &BrowserSearch) -> wasm_bindgen::JsValue;
    #[wasm_bindgen(method)]
    fn progress(this: &BrowserSearch) -> Option<String>;
    #[wasm_bindgen(method)]
    fn stop(this: &BrowserSearch);
    #[wasm_bindgen(method)]
    fn cancel(this: &BrowserSearch);
}

/// An isolated worker, with cooperative UI-thread execution only as a fallback
/// when a browser or deployment cannot start module workers.
pub struct SearchRunner {
    worker: Option<BrowserSearch>,
    request: Option<SearchRequest>,
    search: Option<Search>,
    progress: Progress,
    worker_progress: Option<(i64, i64)>,
}

impl SearchRunner {
    pub fn start(request: SearchRequest) -> Self {
        let worker = serde_json::to_string(&request)
            .ok()
            .and_then(|data| start_browser_search(&data).ok());
        Self {
            worker,
            request: Some(request),
            search: None,
            progress: Progress::default(),
            worker_progress: None,
        }
    }

    pub fn poll(&mut self) -> Option<Result<Outcome, super::SearchError>> {
        if let Some(worker) = &self.worker {
            if let Some(progress) = worker.progress() {
                self.worker_progress = serde_json::from_str(&progress).unwrap_or(None);
                // Once initialized, a crash is reported rather than restarting
                // and silently losing the best seating already found.
                self.request = None;
            }
            let message = worker.poll();
            if message.is_null() || message.is_undefined() {
                return None;
            }
            let field = |name: &str| {
                js_sys::Reflect::get(&message, &name.into())
                    .ok()?
                    .as_string()
            };
            let kind = field("type");
            let data = field("data").unwrap_or_default();
            if kind.as_deref() == Some("done") {
                self.worker = None;
                return Some(
                    serde_json::from_str(&data)
                        .map_err(|error| super::SearchError::Interrupted(error.to_string())),
                );
            }
            if self.request.is_none() {
                self.worker = None;
                return Some(Err(super::SearchError::Interrupted(data)));
            }
            // Worker loading failed before search initialization. The owned
            // request remains available for the supported cooperative fallback.
            self.worker = None;
        }
        if let Some(request) = self.request.take() {
            match request.build_with_progress(self.progress.clone()) {
                Ok(search) => self.search = Some(search),
                Err(error) => {
                    return Some(Err(super::SearchError::Invalid(format!("{error:#}"))));
                }
            }
        }
        let search = self.search.as_mut()?;
        // Retain the previous browser execution budget when workers are not
        // available; reducing this slice would reduce search quality for the
        // same wall-clock solve time on those browsers.
        search.advance(Instant::now() + Duration::from_millis(30));
        if search.done() {
            self.search.take().map(|search| Ok(search.finish()))
        } else {
            None
        }
    }

    pub fn progress(&self) -> Option<(i64, i64)> {
        self.worker_progress.or_else(|| self.progress.best())
    }

    /// Stop returns the best seating; dropping an invalidated job instead
    /// terminates its worker without retaining an obsolete result.
    pub fn stop(&self) {
        self.progress.stop();
        if let Some(worker) = &self.worker {
            worker.stop();
        }
    }
}

impl Drop for SearchRunner {
    fn drop(&mut self) {
        if let Some(worker) = &self.worker {
            worker.cancel();
        }
    }
}

/// Offers `bytes` as a download named `name`.
pub fn save_file(_store: &Store, name: &str, bytes: &[u8]) -> Result<Saved> {
    let array = js_sys::Uint8Array::from(bytes);
    save_js_file(name, &array)
}

fn save_js_file(name: &str, array: &js_sys::Uint8Array) -> Result<Saved> {
    let fail = |_| anyhow!(tr("The browser blocked downloading"));
    let parts = js_sys::Array::of1(array);
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(if name.ends_with(".pdf") {
        "application/pdf"
    } else if name.ends_with(".xlsx") {
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    } else if name.ends_with(".zip") {
        "application/zip"
    } else {
        "application/json"
    });
    let blob =
        web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(fail)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(fail)?;
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| anyhow!(tr("Download unavailable")))?;
    let anchor: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(fail)?
        .dyn_into()
        .map_err(|_| anyhow!(tr("Download unavailable")))?;
    anchor.set_href(&url);
    anchor.set_download(name);
    anchor.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(Saved {
        description: name.to_string(),
        location: None,
    })
}

/// Browser PDFs already live in JavaScript memory after their transferable
/// worker message. Passing them to Blob avoids a round trip through app Wasm.
pub fn save_pdf(_store: &Store, name: &str, bytes: &PdfBytes) -> Result<Saved> {
    save_js_file(name, bytes)
}

pub fn save_pdf_archive(
    _store: &Store,
    name: &str,
    files: &[(String, &PdfBytes)],
) -> Result<Saved> {
    let entries = js_sys::Array::new();
    for (name, bytes) in files {
        let entry = js_sys::Object::new();
        js_sys::Reflect::set(&entry, &"name".into(), &name.as_str().into())
            .map_err(|_| anyhow!(tr("Could not create PDF")))?;
        js_sys::Reflect::set(&entry, &"bytes".into(), bytes)
            .map_err(|_| anyhow!(tr("Could not create PDF")))?;
        entries.push(&entry);
    }
    let archive = plassi_pdf_archive(&entries).map_err(|error| {
        anyhow!(
            error
                .as_string()
                .unwrap_or_else(|| tr("Could not create PDF").into())
        )
    })?;
    save_js_file(name, &archive)
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
        let input: web_sys::HtmlInputElement =
            document.create_element("input").ok()?.dyn_into().ok()?;
        input.set_type("file");
        input.set_accept(accept);
        let slot = std::rc::Rc::new(std::cell::RefCell::new(None));
        let (target, result) = (input.clone(), slot.clone());
        let on_change = Closure::once_into_js(move || {
            let Some(file) = target.files().and_then(|files| files.get(0)) else {
                return;
            };
            wasm_bindgen_futures::spawn_local(async move {
                let read = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await;
                *result.borrow_mut() = Some(
                    read.map(|buffer| js_sys::Uint8Array::new(&buffer).to_vec())
                        .map_err(|_| tr("Could not read the file").to_string()),
                );
                wake();
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
    #[wasm_bindgen(js_name = plassiPdfArchive, catch)]
    fn plassi_pdf_archive(
        files: &js_sys::Array,
    ) -> Result<js_sys::Uint8Array, wasm_bindgen::JsValue>;
}

async fn typeset(data: &str) -> Result<PdfBytes, String> {
    let promise = plassi_typeset(data).map_err(|_| tr("PDF export unavailable").to_string())?;
    let pdf = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|error| {
            error
                .as_string()
                .or_else(|| {
                    js_sys::Reflect::get(&error, &"message".into())
                        .ok()?
                        .as_string()
                })
                .unwrap_or_else(|| tr("Could not create PDF").into())
        })?;
    pdf.dyn_into::<js_sys::Uint8Array>()
        .map_err(|_| tr("Could not create PDF").to_string())
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
            wake();
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
/// Backups: `plassi:backup:<project>:<time>`.
const BACKUP_PREFIX: &str = "plassi:backup:";
/// Local storage is small: only a few backups per project.
const KEEP: super::Keep = super::Keep { recent: 3, days: 0 };

fn storage_keys(storage: &web_sys::Storage) -> Vec<String> {
    (0..storage.length().unwrap_or(0))
        .filter_map(|i| storage.key(i).ok()?)
        .collect()
}

fn backup_key(name: &str, stamp: u64) -> String {
    format!(
        "{BACKUP_PREFIX}{name}:{}",
        super::backup_plan::stamp_name(stamp)
    )
}

/// All backups as (project, time).
fn all_backups() -> Vec<(String, u64)> {
    let Some(storage) = local_storage() else {
        return Vec::new();
    };
    storage_keys(&storage)
        .iter()
        .filter_map(|key| {
            let (name, stamp) = key.strip_prefix(BACKUP_PREFIX)?.rsplit_once(':')?;
            Some((name.to_string(), super::backup_plan::parse_stamp(stamp)?))
        })
        .collect()
}

impl Library {
    pub fn open() -> Self {
        Self
    }

    pub fn projects(&self) -> Vec<String> {
        let Some(storage) = local_storage() else {
            return Vec::new();
        };
        let mut names: Vec<String> = storage_keys(&storage)
            .into_iter()
            .filter_map(|key| key.strip_prefix(PROJECT_PREFIX).map(str::to_string))
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names
    }

    pub fn store(&self, name: &str) -> Store {
        Store::new(&format!("project:{name}"))
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<()> {
        let bytes = self
            .store(from)
            .read()
            .ok_or_else(|| anyhow!(fill("Project {} not found", &[&from])))?;
        self.store(to).write(&bytes)?;
        for stamp in self.backups(from) {
            if let (Some(storage), Some(bytes)) = (local_storage(), self.read_backup(from, stamp)) {
                let text = String::from_utf8_lossy(&bytes);
                if storage.set_item(&backup_key(to, stamp), &text).is_ok() {
                    let _ = storage.remove_item(&backup_key(from, stamp));
                }
            }
        }
        self.delete(from)
    }

    /// Keeps `bytes` as an earlier version of `name`, and drops old ones. If
    /// storage is full, the oldest backups of any project make room. If that
    /// is not enough, restore those copies before reporting the failure.
    pub fn backup(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let storage = local_storage().ok_or_else(|| anyhow!(tr("Browser storage unavailable")))?;
        let text = std::str::from_utf8(bytes)?;
        let now = super::now_stamp().max(self.backups(name).first().map_or(0, |s| s + 1));
        let mut oldest = all_backups();
        oldest.sort_by_key(|(_, stamp)| std::cmp::Reverse(*stamp));
        let mut removed = Vec::<(String, String)>::new();
        let restore = |removed: &[(String, String)]| {
            for (key, body) in removed {
                let _ = storage.set_item(key, body);
            }
        };
        while let Err(error) = storage.set_item(&backup_key(name, now), text) {
            // Removing old copies cannot fix denied writes, and the same
            // restriction could prevent restoring them afterwards.
            if !quota_exceeded(&error) {
                restore(&removed);
                return Err(anyhow!(refusal(&error)));
            }
            let Some((old_name, old)) = oldest.pop() else {
                restore(&removed);
                return Err(anyhow!(tr("No space for a backup")));
            };
            let key = backup_key(&old_name, old);
            let body = match storage.get_item(&key) {
                Ok(Some(body)) => body,
                Ok(None) => continue,
                Err(_) => {
                    restore(&removed);
                    return Err(anyhow!(tr("No space for a backup")));
                }
            };
            if storage.remove_item(&key).is_err() {
                restore(&removed);
                return Err(anyhow!(tr("No space for a backup")));
            }
            removed.push((key, body));
        }
        for old in super::backup_plan::to_prune(&self.backups(name), now, KEEP) {
            let _ = storage.remove_item(&backup_key(name, old));
        }
        Ok(())
    }

    /// The times of `name`'s backups, newest first.
    pub fn backups(&self, name: &str) -> Vec<u64> {
        let mut stamps: Vec<u64> = all_backups()
            .into_iter()
            .filter(|(n, _)| n == name)
            .map(|(_, s)| s)
            .collect();
        stamps.sort_unstable_by(|a, b| b.cmp(a));
        stamps
    }

    pub fn read_backup(&self, name: &str, stamp: u64) -> Option<Vec<u8>> {
        local_storage()?
            .get_item(&backup_key(name, stamp))
            .ok()?
            .map(String::into_bytes)
    }

    /// Names that have backups, including deleted projects.
    pub fn backed_up(&self) -> Vec<String> {
        let mut names: Vec<String> = all_backups().into_iter().map(|(n, _)| n).collect();
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup();
        names
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        local_storage()
            .and_then(|s| s.remove_item(&format!("{PROJECT_PREFIX}{name}")).ok())
            .ok_or_else(|| anyhow!(fill("Could not delete {}", &[&name])))
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

/// Reads a file dropped onto the page, in the background (browsers read files asynchronously).
pub struct DroppedRead(Slot<Result<Vec<u8>, String>>);

impl DroppedRead {
    pub fn start(file: eframe::egui::DroppedFileHandle, ctx: eframe::egui::Context) -> Self {
        let slot: Slot<Result<Vec<u8>, String>> = std::rc::Rc::new(std::cell::RefCell::new(None));
        let result = slot.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let read = file.bytes_async().await;
            *result.borrow_mut() = Some(read);
            ctx.request_repaint();
        });
        Self(slot)
    }

    /// The file's bytes, once read.
    pub fn poll(&self) -> Option<Result<Vec<u8>, String>> {
        self.0.borrow_mut().take()
    }
}

/// Browser file selection uses the same result shape as the desktop picker.
pub struct SheetPick(Slot<std::result::Result<Option<super::SheetFile>, String>>);

impl SheetPick {
    pub fn start(ctx: eframe::egui::Context) -> Self {
        let slot = Slot::default();
        let result = slot.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let picked = rfd::AsyncFileDialog::new()
                .add_filter(crate::i18n::tr("Spreadsheets"), super::SHEET_EXTENSIONS)
                .pick_file()
                .await;
            let file = match picked {
                Some(file) => Some(super::SheetFile {
                    name: file.file_name(),
                    bytes: file.read().await,
                }),
                None => None,
            };
            *result.borrow_mut() = Some(Ok(file));
            ctx.request_repaint();
        });
        Self(slot)
    }

    pub fn poll(&self) -> Option<std::result::Result<Option<super::SheetFile>, String>> {
        self.0.borrow_mut().take()
    }
}

/// Minutes the browser's time zone is ahead of UTC at `stamp` (milliseconds since 1970).
pub fn utc_offset_minutes(stamp: u64) -> i64 {
    -(js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(stamp as f64)).get_timezone_offset()
        as i64)
}
