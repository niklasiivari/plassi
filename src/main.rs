//! Plassi: a local seating planner.
//!
//! - [`model`]: the project file format (attendees, room, rules, options).
//! - [`catalog`]: organizations, ticket types, food and drinks as categories.
//! - [`preferences`]: the planner's own "together" / "apart" preferences.
//! - [`import`] / [`suggest`]: getting attendees and wishes in from spreadsheets.
//! - [`score`] / [`seating`] / [`groups`]: evaluating and optimising seatings.
//! - [`arrange`]: moving people by hand, one or a block at a time.
//! - [`roster`]: late changes (walk-ins, cancellations, replacements).
//! - [`print`]: A4 PDFs.
//! - [`app`]: the UI, on the desktop and in the browser; [`cli`]: headless commands.
//! - [`platform`]: storage, background work and files, per platform.

// Keep the release GUI from opening an extra console window on Windows.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod arrange;
mod catalog;
#[cfg(not(target_arch = "wasm32"))]
mod cli;
mod example;
mod export;
mod groups;
mod i18n;
mod import;
mod io;
mod model;
mod platform;
mod preferences;
mod print;
mod roster;
mod score;
mod seating;
mod suggest;
#[cfg(test)]
mod tests;
#[cfg(target_arch = "wasm32")]
mod worker;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if !cli::run(&args)? {
        app::run()?;
    }
    Ok(())
}

/// In the browser the page starts the app through [`start`].
#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    // The solver worker instantiates the same cached module in its own memory.
    // Its dedicated WorkerSearch entrypoint must not initialize a canvas or UI.
    if web_sys::window().is_some() {
        app::start_web();
    }
}
