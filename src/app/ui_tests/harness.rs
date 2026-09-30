//! Building the app for a test and the steps most flows share.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::{Harness, Node};

use super::super::*;
use crate::i18n::{self, Lang};

/// Held by the running test: the language and other process-wide state are shared.
static SERIAL: Mutex<()> = Mutex::new(());

/// Search time in tests, instead of the half minute a real solve gets.
const TEST_SEARCH: Duration = Duration::from_millis(600);

/// A folder removed when the test ends.
pub(super) struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("plassi-ui-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("temp folder");
        Self(path)
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The app in a harness, on a library of its own.
pub(super) struct Ui {
    pub(super) h: Harness<'static, PlassiApp>,
    pub(super) dir: TempDir,
    _serial: MutexGuard<'static, ()>,
}

impl Ui {
    /// The app on an empty library, in English.
    pub(super) fn open() -> Self {
        let serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = TempDir::new();
        let library = platform::Library::at(dir.path());
        let h = Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_eframe(move |cc| build(cc, library));
        let mut ui = Self { h, dir, _serial: serial };
        ui.settle();
        ui
    }

    /// The app with the example project open.
    pub(super) fn with_example() -> Self {
        let mut ui = Self::open();
        ui.click("Try with example data");
        ui
    }

    pub(super) fn app(&self) -> &PlassiApp {
        self.h.state()
    }

    pub(super) fn app_mut(&mut self) -> &mut PlassiApp {
        self.h.state_mut()
    }

    /// Runs frames until the app is idle (a few frames while a search runs).
    pub(super) fn settle(&mut self) {
        if self.h.try_run().is_err() {
            self.h.run_steps(4);
        }
    }

    /// The first widget labelled `label`.
    pub(super) fn node<'a>(&'a self, label: &'a str) -> Node<'a> {
        let found = self.h.query_all_by_label(label).next();
        found.unwrap_or_else(|| panic!("no {label:?} among {:?}", self.dump()))
    }

    /// Widgets and text showing exactly `text` (buttons carry it as a
    /// label, plain text as a value).
    pub(super) fn count(&self, text: &str) -> usize {
        self.h.query_all_by(|n| n.label().as_deref() == Some(text) || n.value().as_deref() == Some(text)).count()
    }

    pub(super) fn has(&self, label: &str) -> bool {
        self.count(label) > 0
    }

    /// Whether any widget or text contains `part`.
    pub(super) fn has_text(&self, part: &str) -> bool {
        self.dump().iter().any(|t| t.contains(part))
    }

    pub(super) fn click(&mut self, label: &str) {
        // Scrolled into view first: pages taller than the window scroll.
        self.node(label).scroll_to_me();
        self.settle();
        self.node(label).click();
        self.settle();
    }

    /// Opens the project switcher (the button named after the open project).
    pub(super) fn open_project_menu(&mut self) {
        let name = self.app().file.name.clone();
        self.click(&name);
    }

    /// Runs frames until `done` holds, for at most `seconds`.
    pub(super) fn wait_until(&mut self, seconds: u64, mut done: impl FnMut(&PlassiApp) -> bool) {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
        while !done(self.app()) {
            assert!(std::time::Instant::now() < until, "timed out after {seconds} s");
            self.h.step();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        self.settle();
    }

    /// Clicks Solve and waits for the new option.
    pub(super) fn solve(&mut self) {
        let before = self.app().project.candidates.len();
        self.click("Solve");
        self.wait_until(20, |app| app.solving.is_none() && app.project.candidates.len() > before);
    }

    /// Builds the app again on the same library, as after a restart.
    pub(super) fn reopen(self) -> Self {
        let Self { h, dir, _serial } = self;
        drop(h);
        let library = platform::Library::at(dir.path());
        let h = Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_eframe(move |cc| build(cc, library));
        let mut ui = Self { h, dir, _serial };
        ui.settle();
        ui
    }

    /// Where the card of `person` is on screen.
    pub(super) fn card_of(&self, person: usize) -> egui::Rect {
        let app = self.app();
        let seat = &app.project.candidates[app.selected_candidate].assignment[&person];
        app.seat_card_on_screen(seat).expect("seat on screen")
    }

    /// Two seated people who are not pinned, the first ones in seat order.
    pub(super) fn two_unpinned(&self) -> [usize; 2] {
        let app = self.app();
        let pinned = app.project.pinned_people();
        let seated = &app.project.candidates[app.selected_candidate].assignment;
        let mut people = app.project.layout.seats().into_iter().filter_map(|seat| {
            seated.iter().find(|(_, s)| **s == seat.id).map(|(&p, _)| p).filter(|p| !pinned.contains(p))
        });
        [people.next().expect("a person"), people.next().expect("another person")]
    }

    /// The seat of `person` in the shown option.
    pub(super) fn seat_of(&self, person: usize) -> String {
        let app = self.app();
        app.project.candidates[app.selected_candidate].assignment[&person].clone()
    }

    /// Presses and releases a mouse button at `pos`.
    pub(super) fn click_at(&mut self, pos: egui::Pos2, button: egui::PointerButton, modifiers: egui::Modifiers) {
        self.h.hover_at(pos);
        self.h.step();
        for pressed in [true, false] {
            self.h.event(egui::Event::PointerButton { pos, button, pressed, modifiers });
            self.h.step();
        }
        self.settle();
    }

    /// Drags from `from` to `to` with the primary button, in a few moves.
    pub(super) fn drag(&mut self, from: egui::Pos2, to: egui::Pos2) {
        self.h.hover_at(from);
        self.h.step();
        self.h.drag_at(from);
        self.h.step();
        for t in [0.1, 0.5, 0.9, 1.0] {
            self.h.hover_at(from.lerp(to, t));
            self.h.step();
        }
        self.h.drop_at(to);
        self.h.step();
        self.settle();
    }

    /// Presses a key with modifiers (none for a plain key).
    pub(super) fn key(&mut self, modifiers: egui::Modifiers, key: egui::Key) {
        self.h.key_press_modifiers(modifiers, key);
        self.settle();
    }

    /// Opens the dropdown currently showing `shown`.
    pub(super) fn open_combo(&mut self, shown: &str) {
        let found = self.h.query_all_by(|n| n.role() == egui::accesskit::Role::ComboBox && n.value().as_deref() == Some(shown)).next();
        let found = found.unwrap_or_else(|| panic!("no dropdown showing {shown:?}"));
        found.scroll_to_me();
        self.settle();
        let found = self.h.query_all_by(|n| n.role() == egui::accesskit::Role::ComboBox && n.value().as_deref() == Some(shown)).next();
        let found = found.expect("the dropdown");
        found.click();
        self.settle();
    }

    /// Picks `label` in the open dropdown: its list is drawn last, so this is
    /// the last widget with that label (the page may show the same word).
    pub(super) fn choose(&mut self, label: &str) {
        let found = self.h.query_all_by_label(label).last();
        found.unwrap_or_else(|| panic!("no {label:?} in the open dropdown")).scroll_to_me();
        self.settle();
        self.h.query_all_by_label(label).last().expect("the option").click();
        self.settle();
    }

    /// Clicks `label` and returns the text the click copied, if any.
    pub(super) fn click_and_copied(&mut self, label: &str) -> Option<String> {
        self.node(label).click();
        let mut copied = None;
        for _ in 0..3 {
            self.h.step();
            copied = copied.or_else(|| {
                self.h.output().platform_output.commands.iter().find_map(|c| match c {
                    egui::OutputCommand::CopyText(text) => Some(text.clone()),
                    _ => None,
                })
            });
        }
        self.settle();
        copied
    }

    /// Clicks into the one multi-line text box on the page (egui does not
    /// expose hint texts to AccessKit).
    pub(super) fn click_text_box(&mut self) {
        {
            let mut found = self.h.query_all_by(|n| n.role() == egui::accesskit::Role::MultilineTextInput);
            let field = found.next().expect("a text box");
            assert!(found.next().is_none(), "one text box");
            field.click();
        }
        self.settle();
    }

    /// Goes to a page through the sidebar.
    pub(super) fn go_to(&mut self, page: &str) {
        self.click(page);
    }

    /// Types text into the focused field.
    pub(super) fn type_text(&mut self, text: &str) {
        self.h.event(egui::Event::Text(text.into()));
        self.settle();
    }

    /// Every text in the accessibility tree, for asserts and failure messages.
    pub(super) fn dump(&self) -> Vec<String> {
        self.h.query_all_by(|n| n.label().is_some() || n.value().is_some()).filter_map(|n| {
            let n = n.accesskit_node();
            n.label().or_else(|| n.value())
        }).collect()
    }
}

/// The app on `library` with short searches, in English.
fn build(cc: &mut eframe::CreationContext<'_>, library: platform::Library) -> PlassiApp {
    theme::install(&cc.egui_ctx);
    let mut app = PlassiApp::with_library(library);
    app.solve_time = TEST_SEARCH;
    app.improve_time = TEST_SEARCH;
    i18n::set(Lang::En);
    app
}
