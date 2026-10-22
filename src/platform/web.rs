//! The browser: the project in local storage, work in slices between frames,
//! files as downloads.

use std::path::Path;

use anyhow::{Result, anyhow};
use wasm_bindgen::JsCast;
use web_time::{Duration, Instant};

use super::{Pdfs, Saved};
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

/// Why the browser refused to store something, in words the user can act on.
fn refusal(error: &wasm_bindgen::JsValue) -> &'static str {
    let name = js_sys::Reflect::get(error, &"name".into())
        .ok()
        .and_then(|n| n.as_string())
        .unwrap_or_default();
    if name.contains("Quota") {
        crate::i18n::tr(
            "Browser storage full. Download a backup, then delete old projects or backups.",
        )
    } else {
        crate::i18n::tr("The browser blocked saving")
    }
}

thread_local! {
    /// Whether the browser promised to keep local storage; `None` until it answers.
    static PERSISTED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
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
    });
}

/// Whether the browser keeps local storage until the user clears it.
pub fn storage_persisted() -> Option<bool> {
    PERSISTED.with(std::cell::Cell::get)
}

/// A seating search advanced a little every frame, so the page stays responsive.
pub struct SearchRunner {
    search: Option<Search>,
    progress: Progress,
}

/// Search time per frame.
const SLICE: Duration = Duration::from_millis(30);

impl SearchRunner {
    pub fn start(search: Search) -> Self {
        Self {
            progress: search.progress(),
            search: Some(search),
        }
    }

    pub fn poll(&mut self) -> Option<Outcome> {
        let search = self.search.as_mut()?;
        search.advance(Instant::now() + SLICE);
        if search.done() {
            self.search.take().map(Search::finish)
        } else {
            None
        }
    }

    /// The best (score, broken hard rules) found so far.
    pub fn progress(&self) -> Option<(i64, i64)> {
        self.progress.best()
    }

    /// Ends the search early; the next `poll` delivers the best found.
    pub fn stop(&self) {
        self.progress.stop();
    }
}

/// Offers `bytes` as a download named `name`.
pub fn save_file(_store: &Store, name: &str, bytes: &[u8]) -> Result<Saved> {
    let fail = |_| anyhow!(tr("The browser blocked downloading"));
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::of1(&array);
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
    let promise = plassi_typeset(data).map_err(|_| tr("PDF export unavailable").to_string())?;
    let pdf = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|error| {
            error
                .as_string()
                .unwrap_or_else(|| tr("Could not create PDF").into())
        })?;
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
    /// storage is full, the oldest backups of any project make room; if that
    /// is not enough, no backup is taken.
    pub fn backup(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let storage = local_storage().ok_or_else(|| anyhow!(tr("Browser storage unavailable")))?;
        let text = std::str::from_utf8(bytes)?;
        let now = super::now_stamp().max(self.backups(name).first().map_or(0, |s| s + 1));
        let mut oldest = all_backups();
        oldest.sort_by_key(|(_, stamp)| std::cmp::Reverse(*stamp));
        while storage.set_item(&backup_key(name, now), text).is_err() {
            let (old_name, old) = oldest
                .pop()
                .ok_or_else(|| anyhow!(tr("No space for a backup")))?;
            let _ = storage.remove_item(&backup_key(&old_name, old));
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
