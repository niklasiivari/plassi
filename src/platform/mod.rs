//! What differs between the desktop app and the browser: where the project is
//! kept, how work runs in the background, and how files reach the user.
//!
//! Both sides offer the same items: `Store`, `SearchRunner`, `PdfJob`,
//! `save_file`, `open_location` and `save_folder`.

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
pub type Pdfs<K> = Vec<(K, Result<Vec<u8>, String>)>;
