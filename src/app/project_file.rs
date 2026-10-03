//! The project file: opening, importing, live reload of external edits, atomic saves and undo/redo.

use super::*;
use crate::i18n::{fill, tr};

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
    pub(super) last_bytes: Option<Vec<u8>>,
    pub(super) last_check: Instant,
    /// The stored project is invalid; the app shows the last valid state and won't save.
    pub(super) external_invalid: bool,
    /// Increments on every change, so cached results know when they are stale.
    pub(super) revision: u64,
    pub(super) undo_stack: Vec<Step>,
    pub(super) redo_stack: Vec<Step>,
    /// A project file being uploaded in the browser.
    #[cfg(target_arch = "wasm32")]
    pub(super) upload: Option<platform::FilePick>,
}

/// A saved state in the history and the change that leads away from it.
pub(super) struct Step {
    pub(super) bytes: Vec<u8>,
    /// E.g. "Moved X to Table 2"; shown as "Undo: …".
    pub(super) change: String,
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
        let (project, status) = match stored.as_deref().map(io::parse) {
            Some(Ok(project)) => (project, fill("Opened {}", &[&name])),
            Some(Err(error)) => (
                Project::new(),
                fill("{} needs repair: {}", &[&name, &format!("{error:#}")]),
            ),
            None => (Project::new(), String::new()),
        };
        library.remember(&name);
        let file = Self {
            external_invalid: stored.as_deref().is_some_and(|b| io::parse(b).is_err()),
            last_bytes: stored,
            last_check: Instant::now(),
            revision: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            #[cfg(target_arch = "wasm32")]
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
    pub(super) fn import_tsv(&mut self) {
        match import::import_tsv(Path::new(&self.room.import_input)) {
            Ok(project) => {
                self.backup_now();
                self.project = project;
                self.commit(tr("Project replaced from TSV"));
            }
            Err(error) => self.status = fill("Import failed: {}", &[&format!("{error:#}")]),
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

    /// Replaces the project with an uploaded file, once it has been read.
    #[cfg(target_arch = "wasm32")]
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
                self.file.last_bytes = Some(bytes);
                self.file.external_invalid = false;
                self.file.undo_stack.clear();
                self.file.redo_stack.clear();
                self.file.library.remember(name);
                self.backups = Default::default();
                self.selected_candidate = 0;
                self.selected_person = None;
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
        if self.file.last_check.elapsed() < Duration::from_millis(350) {
            return;
        }
        self.file.last_check = Instant::now();
        let Some(bytes) = self.file.store.read() else {
            return;
        };
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
                if let Some(before) = self.file.last_bytes.replace(bytes) {
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

    fn push_undo(&mut self, bytes: Vec<u8>, change: String) {
        self.file.undo_stack.push(Step { bytes, change });
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
        let Ok(project) = serde_json::from_slice::<Project>(&step.bytes) else {
            return;
        };
        let change = step.change.clone();
        let current = self.file.last_bytes.clone();
        self.project = project;
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
        to.extend(current.map(|bytes| Step { bytes, change }));
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
        if disk_bytes.is_some() && disk_bytes.as_deref() != self.file.last_bytes.as_deref() {
            self.revert(tr("File changed; loading external edits"));
            self.file.last_check = Instant::now() - Duration::from_secs(1);
            self.sync_external();
            return false;
        }
        self.auto_backup();
        match io::encode(&self.project)
            .and_then(|bytes| self.file.store.write(&bytes).map(|()| bytes))
        {
            Ok(bytes) => {
                self.file.last_bytes = Some(bytes);
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
        self.after_change();
    }

    /// Marks cached results stale and drops state that referred to people or
    /// options that are gone.
    fn after_change(&mut self) {
        self.file.revision += 1;
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
