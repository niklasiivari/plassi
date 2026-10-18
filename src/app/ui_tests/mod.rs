//! Click-through tests of the real app, run headless with `egui_kittest`.
//! Each test builds the app on its own temporary library; they run one at a
//! time because the UI language is global.

mod harness;

mod arranging;
mod flows;
mod importing;
mod placing;
mod views;
