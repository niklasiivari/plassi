//! The project file: opening, importing, live reload of external edits, atomic saves and undo/redo.

use super::*;
use crate::i18n::{fill, tr};
use std::io::{Read, Write};
use std::rc::Rc;

/// Undo steps kept.
const HISTORY: usize = 200;
/// Name of a new project.
const NEW_PROJECT: &str = "New event";

/// The open project in the library (a file on desktop, local storage in the
/// browser) and its edit history.
pub(super) struct ProjectFile {
    pub(super) library: platform::Library,
    /// The open project's name in the library.
    pub(super) name: String,
    pub(super) store: platform::Store,
    /// Bytes last written or read, to detect external edits.
    pub(super) last_bytes: Option<Rc<[u8]>>,
    pub(super) last_check: Instant,
    /// The stored project is invalid; the app shows the last valid state and won't save.
    pub(super) external_invalid: bool,
    /// Increments on every change, so cached results know when they are stale.
    pub(super) revision: u64,
    pub(super) undo_stack: Vec<Step>,
    pub(super) redo_stack: Vec<Step>,
    /// The option shown at the start of this frame, so undo can show it
    /// again. Its index is into the options of the last saved state, which
    /// is the state an undo step saves, so it needs no name to find it.
    pub(super) shown_option: Option<usize>,
    /// A project file being opened from outside the library.
    pub(super) upload: Option<platform::FilePick>,
}

/// A saved state in the history and the change that leads away from it.
pub(super) struct Step {
    bytes: Snapshot,
    /// E.g. "Moved X to Table 2"; shown as "Undo: …".
    pub(super) change: String,
    /// The option shown in this state, as its index there.
    pub(super) option: Option<usize>,
}

/// Lossless history storage; incompressible snapshots retain their shared bytes.
enum Snapshot {
    Raw(Rc<[u8]>),
    Pending {
        bytes: Rc<[u8]>,
        encoder: flate2::write::DeflateEncoder<Vec<u8>>,
        offset: usize,
    },
    Deflated(Vec<u8>),
}

impl Snapshot {
    fn new(bytes: Rc<[u8]>) -> Self {
        if bytes.len() <= 512 {
            return Self::Raw(bytes);
        }
        Self::Pending {
            bytes,
            encoder: flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast()),
            offset: 0,
        }
    }

    fn pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    /// One bounded piece of compression; pending snapshots remain restorable.
    fn advance(&mut self) {
        let Self::Pending {
            bytes,
            encoder,
            offset,
        } = self
        else {
            return;
        };
        let end = (*offset + 4096).min(bytes.len());
        if encoder.write_all(&bytes[*offset..end]).is_err() {
            *self = Self::Raw(bytes.clone());
            return;
        }
        *offset = end;
        if end != bytes.len() {
            return;
        }
        let raw = bytes.clone();
        let completed = std::mem::replace(self, Self::Raw(raw));
        if let Self::Pending { encoder, bytes, .. } = completed
            && let Ok(compressed) = encoder.finish()
            && compressed.len() < bytes.len()
        {
            *self = Self::Deflated(compressed);
        }
    }

    fn decode(&self) -> std::io::Result<Vec<u8>> {
        match self {
            Self::Raw(bytes) | Self::Pending { bytes, .. } => Ok(bytes.to_vec()),
            Self::Deflated(bytes) => {
                let mut decoded = Vec::new();
                flate2::read::DeflateDecoder::new(bytes.as_slice()).read_to_end(&mut decoded)?;
                Ok(decoded)
            }
        }
    }
}

/// A name that works as a file name: no path separators or reserved characters.
pub(super) fn project_name(text: &str) -> String {
    let name: String = text
        .trim()
        .chars()
        .map(|c| {
            if "/\\:*?\"<>|".contains(c) || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    if name.is_empty() {
        tr(NEW_PROJECT).to_string()
    } else {
        name
    }
}

impl ProjectFile {
    /// Opens the project used last time, or starts a new one.
    pub(super) fn open_default(library: platform::Library) -> (Self, Project, String) {
        let projects = library.projects();
        let name = library
            .last()
            .filter(|n| projects.contains(n))
            .or_else(|| projects.first().cloned());
        let name = name.unwrap_or_else(|| {
            let name = tr(NEW_PROJECT).to_string();
            let _ = io::encode(&Project::new()).and_then(|b| library.store(&name).write(&b));
            name
        });
        let store = library.store(&name);
        let stored = store.read();
        let (project, status, external_invalid) = match stored.as_deref().map(io::parse) {
            Some(Ok(project)) => (project, fill("Opened {}", &[&name]), false),
            Some(Err(error)) => (
                Project::new(),
                fill("{} needs repair: {}", &[&name, &format!("{error:#}")]),
                true,
            ),
            None => (Project::new(), String::new(), false),
        };
        library.remember(&name);
        let file = Self {
            external_invalid,
            last_bytes: stored.map(Rc::from),
            last_check: Instant::now(),
            revision: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            shown_option: None,
            upload: None,
            library,
            name,
            store,
        };
        (file, project, status)
    }

    /// `base`, or `base 2`, `base 3`… whichever is free.
    pub(super) fn free_name(&self, base: &str) -> String {
        let base = project_name(base);
        let taken = self.library.projects();
        std::iter::once(base.to_string())
            .chain((2..).map(|n| format!("{base} {n}")))
            .find(|name| !taken.contains(name))
            .expect("some name is free")
    }
}

impl PlassiApp {
    /// Compress history after drawing, without making a large commit block a
    /// whole frame. All snapshots can be restored while this work is pending.
    pub(super) fn compress_history(&mut self, ctx: &egui::Context) {
        let until = Instant::now() + Duration::from_millis(2);
        'history: for step in self
            .file
            .undo_stack
            .iter_mut()
            .chain(&mut self.file.redo_stack)
        {
            while step.bytes.pending() {
                step.bytes.advance();
                if Instant::now() >= until {
                    break 'history;
                }
            }
        }
        if self
            .file
            .undo_stack
            .iter()
            .chain(&self.file.redo_stack)
            .any(|step| step.bytes.pending())
        {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }

    /// Offers the project as a JSON file.
    #[cfg(target_arch = "wasm32")]
    pub(super) fn download_project(&mut self) {
        let result = io::encode(&self.project)
            .and_then(|bytes| platform::save_file(&self.file.store, "plassi.json", &bytes));
        self.status = match result {
            Ok(saved) => {
                self.downloaded_copy();
                fill("Downloaded project: {}", &[&saved.description])
            }
            Err(error) => format!("{error:#}"),
        };
    }

    /// Replaces the project with an opened file, once it has been read.
    pub(super) fn poll_upload(&mut self) {
        let Some(result) = self.file.upload.as_ref().and_then(platform::FilePick::poll) else {
            return;
        };
        self.file.upload = None;
        match result
            .map_err(anyhow::Error::msg)
            .and_then(|bytes| io::parse(&bytes))
        {
            Ok(project) => {
                self.backup_now();
                self.project = project;
                self.selected_candidate = 0;
                self.commit(tr("Project file loaded"));
            }
            Err(error) => {
                self.status = fill("Could not load that file: {}", &[&format!("{error:#}")])
            }
        }
    }

    /// Opens the made-up example event as a project of its own.
    pub(super) fn load_example(&mut self) {
        self.create_project(tr("Example event"), crate::example::project());
    }

    /// Switches to another project in the library.
    pub(super) fn switch_project(&mut self, name: &str) {
        let store = self.file.library.store(name);
        match store
            .read()
            .ok_or_else(|| anyhow::anyhow!(fill("Project {} not found", &[&name])))
            .and_then(|b| io::parse(&b).map(|p| (p, b)))
        {
            Ok((project, bytes)) => {
                self.file.name = name.to_string();
                self.file.store = store;
                self.file.last_bytes = Some(bytes.into());
                self.file.external_invalid = false;
                self.file.undo_stack.clear();
                self.file.redo_stack.clear();
                self.file.library.remember(name);
                self.backups = Default::default();
                self.selected_candidate = saved_option(&project);
                self.selected_person = None;
                self.people.clear_import();
                self.chart.selected_table = None;
                self.chart.view.fit_pending = true;
                self.replace_project(project);
                self.status = fill("Opened {}", &[&name]);
            }
            Err(error) => {
                self.status = fill("Could not open {}: {}", &[&name, &format!("{error:#}")])
            }
        }
    }

    /// Adds a project to the library under a free name based on `name`, and opens it.
    pub(super) fn create_project(&mut self, name: &str, project: Project) {
        let name = self.file.free_name(name);
        match io::encode(&project).and_then(|bytes| self.file.library.store(&name).write(&bytes)) {
            Ok(()) => self.switch_project(&name),
            Err(error) => {
                self.status = fill("Could not create {}: {}", &[&name, &format!("{error:#}")])
            }
        }
    }

    pub(super) fn rename_project(&mut self, to: &str) {
        if to.trim().is_empty() {
            return;
        }
        let to = project_name(to);
        let to = to.as_str();
        if to == self.file.name {
            return;
        }
        if self.file.library.projects().iter().any(|n| n == to) {
            self.status = fill("A project named {} already exists", &[&to]);
            return;
        }
        match self.file.library.rename(&self.file.name, to) {
            Ok(()) => {
                self.file.name = to.to_string();
                self.file.store = self.file.library.store(to);
                self.file.library.remember(to);
                self.status = fill("Renamed to {}", &[&to]);
            }
            Err(error) => self.status = format!("{error:#}"),
        }
    }

    /// Deletes the open project, keeping a backup, and opens another (or a new one).
    pub(super) fn delete_project(&mut self) {
        let name = self.file.name.clone();
        self.backup_now();
        if let Err(error) = self.file.library.delete(&name) {
            self.status = format!("{error:#}");
            return;
        }
        match self.file.library.projects().first().cloned() {
            Some(next) => self.switch_project(&next),
            None => self.create_project(tr(NEW_PROJECT), Project::new()),
        }
        self.status = fill("Deleted {}", &[&name]);
    }

    /// Picks up edits made to the file outside the app. A valid external
    /// edit becomes an undo step of its own.
    pub(super) fn sync_external(&mut self) {
        if !self.file.store.notifications_available()
            && self.file.last_check.elapsed() < Duration::from_millis(350)
        {
            return;
        }
        self.file.last_check = Instant::now();
        if !self.file.store.external_change_pending() {
            return;
        }
        let Some(bytes) = self.file.store.read() else {
            return;
        };
        self.load_external_bytes(bytes);
    }

    /// A detected save conflict already supplies the bytes; notifications must
    /// never gate loading a change that was confirmed by the content check.
    fn load_external_bytes(&mut self, bytes: Vec<u8>) {
        if self.file.last_bytes.as_deref() == Some(bytes.as_slice()) {
            self.file.external_invalid = false;
            return;
        }
        let parsed = serde_json::from_slice::<Project>(&bytes)
            .map_err(anyhow::Error::from)
            .and_then(|project| project.validate().map(|()| project));
        match parsed {
            Ok(project) => {
                if let Some(before) = &self.file.last_bytes {
                    self.backup_bytes(&before.clone());
                }
                if let Some(before) = self.file.last_bytes.replace(bytes.into()) {
                    self.push_undo(before, tr("External edits").into());
                }
                self.file.redo_stack.clear();
                self.file.external_invalid = false;
                self.replace_project(project);
                self.status = fill("Loaded changes from {}", &[&self.file.store.location()]);
            }
            Err(error) => {
                self.file.external_invalid = true;
                self.status = fill(
                    "Invalid file; last saved version shown: {}",
                    &[&format!("{error:#}")],
                );
            }
        }
    }

    /// Saves the edited project as an undo step. If it can't be saved, the
    /// edit is discarded so the app never shows unsaved changes.
    pub(super) fn commit(&mut self, message: &str) {
        let before = self.file.last_bytes.clone();
        if self.save(message) {
            if let Some(before) = before {
                self.push_undo(before, message.into());
            }
            self.file.redo_stack.clear();
        }
    }

    fn push_undo(&mut self, bytes: Rc<[u8]>, change: String) {
        let option = self.file.shown_option;
        self.file.undo_stack.push(Step {
            bytes: Snapshot::new(bytes),
            change,
            option,
        });
        if self.file.undo_stack.len() > HISTORY {
            self.file.undo_stack.remove(0);
        }
    }

    /// Restores the previous (`redo == false`) or next saved state.
    pub(super) fn undo(&mut self, redo: bool) {
        let from = if redo {
            &self.file.redo_stack
        } else {
            &self.file.undo_stack
        };
        let Some(step) = from.last() else { return };
        let Ok(bytes) = step.bytes.decode() else {
            return;
        };
        let Ok(project) = serde_json::from_slice::<Project>(&bytes) else {
            return;
        };
        let change = step.change.clone();
        let option = step.option;
        let current = self.file.last_bytes.clone();
        self.project = project;
        // Back on the option shown in that state.
        if let Some(index) = option.filter(|i| *i < self.project.candidates.len()) {
            self.selected_candidate = index;
        }
        let message = if redo {
            fill("Redid: {}", &[&change])
        } else {
            fill("Undid: {}", &[&change])
        };
        if !self.save(&message) {
            return;
        }
        let (from, to) = if redo {
            (&mut self.file.redo_stack, &mut self.file.undo_stack)
        } else {
            (&mut self.file.undo_stack, &mut self.file.redo_stack)
        };
        from.pop();
        let option = self.file.shown_option;
        to.extend(current.map(|bytes| Step {
            bytes: Snapshot::new(bytes),
            change,
            option,
        }));
    }

    /// Writes the project; returns whether it was saved. On failure the
    /// project goes back to the last saved state.
    pub(super) fn save(&mut self, message: &str) -> bool {
        if self.file.external_invalid {
            self.revert(tr("Fix the project file before saving"));
            return false;
        }
        let disk_bytes = self.file.store.read();
        // A missing file is simply written again; any other difference is an external edit.
        if let Some(bytes) = disk_bytes
            && Some(bytes.as_slice()) != self.file.last_bytes.as_deref()
        {
            self.revert(tr("File changed; loading external edits"));
            self.load_external_bytes(bytes);
            return false;
        }
        self.auto_backup();
        match io::encode(&self.project)
            .and_then(|bytes| self.file.store.write(&bytes).map(|()| bytes))
        {
            Ok(bytes) => {
                self.file.last_bytes = Some(bytes.into());
                self.after_change();
                self.status = message.into();
                true
            }
            Err(error) => {
                self.revert(&fill(
                    "Save failed; restored last saved state: {}",
                    &[&format!("{error:#}")],
                ));
                false
            }
        }
    }

    /// Discards unsaved edits: back to the last saved state.
    fn revert(&mut self, message: &str) {
        let saved = self
            .file
            .last_bytes
            .as_deref()
            .and_then(|b| serde_json::from_slice::<Project>(b).ok());
        if let Some(project) = saved {
            self.replace_project(project);
        }
        self.status = message.into();
    }

    /// Installs a project and drops everything that referred to the old one.
    pub(super) fn replace_project(&mut self, project: Project) {
        self.project = project;
        self.people.end_review();
        self.categories.forget_project();
        self.after_change();
    }

    /// Marks cached results stale and drops state that referred to people or
    /// options that are gone.
    fn after_change(&mut self) {
        self.file.revision += 1;
        self.cancel_stale_search();
        platform::request_repaint();
        self.selected_candidate = self
            .selected_candidate
            .min(self.project.candidates.len().saturating_sub(1));
        let exists = |id: &usize| self.project.person(*id).is_some();
        if !self.selected_person.as_ref().is_none_or(exists) {
            self.selected_person = None;
        }
        self.chart.forget_people(&exists);
        self.people.picked.retain(exists);
        if self
            .people
            .wish_draft
            .as_ref()
            .is_some_and(|d| !exists(&d.person))
        {
            self.people.wish_draft = None;
        }
        // A draft's note is an index; drop it if it now points at someone else's note.
        let notes = &self.project.notes;
        if let Some(draft) = &mut self.people.wish_draft
            && draft
                .note
                .is_some_and(|i| notes.get(i).is_none_or(|n| n.person != draft.person))
        {
            draft.note = None;
        }
        let candidates = self.project.candidates.len();
        if self.chart.compare_with.is_some_and(|c| c >= candidates) {
            self.chart.compare_with = None;
        }
    }
}

/// The option this project was last showing, if it still exists; else the first.
pub(super) fn saved_option(project: &Project) -> usize {
    if project.shown_option < project.candidates.len() {
        project.shown_option
    } else {
        0
    }
}

impl PlassiApp {
    /// Keeps the shown option in the project file, so the project reopens on
    /// it. Saved quietly: choosing an option is not an edit to undo.
    pub(super) fn remember_option(&mut self) {
        if self.project.shown_option != self.selected_candidate
            && self.selected_candidate < self.project.candidates.len()
        {
            self.project.shown_option = self.selected_candidate;
            let status = self.status.clone();
            self.save(&status);
        }
    }
}

#[cfg(test)]
mod history_tests {
    use super::*;

    #[test]
    fn snapshots_preserve_all_bytes_and_compress_repeated_project_data() {
        let bytes: Rc<[u8]> = (0..200)
            .flat_map(|_| b"{\"name\":\"Person 1\",\"seat\":\"1\"}\n".iter().copied())
            .collect::<Vec<_>>()
            .into();
        let mut saved = Snapshot::new(bytes.clone());
        assert!(saved.pending());
        assert_eq!(saved.decode().unwrap().as_slice(), bytes.as_ref());
        saved.advance();
        assert!(
            saved.pending(),
            "one chunk cannot consume this whole snapshot"
        );
        assert_eq!(saved.decode().unwrap().as_slice(), bytes.as_ref());
        while saved.pending() {
            saved.advance();
        }
        assert!(
            matches!(&saved, Snapshot::Deflated(compressed) if compressed.len() < bytes.len() / 4)
        );
        assert_eq!(saved.decode().unwrap().as_slice(), bytes.as_ref());
        // All undo entries remain independently restorable, in either direction.
        let snapshots: Vec<_> = (0..HISTORY)
            .map(|n| Snapshot::new(format!("{{\"shown_option\":{n}}}").into_bytes().into()))
            .collect();
        for (n, saved) in snapshots.iter().enumerate().rev() {
            assert_eq!(
                saved.decode().unwrap(),
                format!("{{\"shown_option\":{n}}}").as_bytes()
            );
        }
        assert_eq!(Snapshot::new(Rc::from(&b""[..])).decode().unwrap(), b"");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn pre_save_conflicts_load_confirmed_bytes_and_keep_undo_and_invalid_file_protection() {
        let folder =
            std::env::temp_dir().join(format!("plassi-conflict-{}", rand::random::<u64>()));
        let mut app = PlassiApp::with_library(platform::Library::at(&folder));
        app.project = crate::example::project();
        app.commit("Imported people");
        let before = app.file.last_bytes.clone().unwrap();
        let mut external = app.project.clone();
        external.event.title = "<event>".into();
        let changed = io::encode(&external).unwrap();
        app.file.store.write(&changed).unwrap();
        // Saving must resolve a confirmed conflict independently of whether
        // the notification has already been consumed or has not arrived yet.
        let _ = app.file.store.external_change_pending();
        app.project.event.subtitle = "X".into();
        app.commit("Edited details");
        assert_eq!(app.project.event.title, "<event>");
        assert_eq!(app.file.last_bytes.as_deref(), Some(changed.as_slice()));
        app.undo(false);
        assert_eq!(app.file.last_bytes.as_deref(), Some(before.as_ref()));
        app.file.store.write(b"{").unwrap();
        app.project.event.subtitle = "Y".into();
        app.commit("Edited details");
        assert!(app.file.external_invalid);
        assert_eq!(app.file.store.read().unwrap(), b"{");
        assert_eq!(
            io::encode(&app.project).unwrap().as_slice(),
            before.as_ref()
        );
        drop(app);
        let _ = std::fs::remove_dir_all(folder);
    }

    #[test]
    fn incompressible_history_keeps_original_bytes_after_incremental_work() {
        let mut state = 7_u64;
        let bytes: Rc<[u8]> = (0..65_536)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 32) as u8
            })
            .collect::<Vec<_>>()
            .into();
        let mut saved = Snapshot::new(bytes.clone());
        while saved.pending() {
            assert_eq!(saved.decode().unwrap().as_slice(), bytes.as_ref());
            saved.advance();
        }
        assert!(matches!(saved, Snapshot::Raw(_)));
        assert_eq!(saved.decode().unwrap().as_slice(), bytes.as_ref());
    }
}
