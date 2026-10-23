//! What differs between the desktop app and the browser: where the project is
//! kept, how work runs in the background, and how files reach the user.
//!
//! Both sides offer the same items: `Store`, `SearchRunner`, `PdfJob`,
//! `save_file`, `open_location`, `save_folder`, `DroppedRead`, and a `Library` of projects
//! with their backups.

mod backup_plan;
pub use backup_plan::{Keep, now_stamp};
mod search_request;
pub use search_request::SearchRequest;

/// Keep expected input failures separate from an interrupted background job.
#[derive(Debug)]
pub enum SearchError {
    Invalid(String),
    Interrupted(String),
}

/// A picked spreadsheet, read without changing the open project.
pub struct SheetFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

const SHEET_EXTENSIONS: &[&str] = &["xlsx", "xls", "xlsm", "xlsb", "ods", "csv", "tsv", "txt"];

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::*;

/// Where a saved file went, for the status line (and opening it on desktop).
pub struct Saved {
    pub description: String,
    pub location: Option<std::path::PathBuf>,
}

/// Typeset prints: each kind with its PDF, or why it failed.
pub type Pdfs<K> = Vec<(K, Result<PdfBytes, String>)>;

#[cfg(not(target_arch = "wasm32"))]
pub type PdfBytes = Vec<u8>;
#[cfg(target_arch = "wasm32")]
pub type PdfBytes = js_sys::Uint8Array;
