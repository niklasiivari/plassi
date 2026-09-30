//! The desktop: a project file on disk, threads, files saved next to it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

use anyhow::{Context, Result};

use super::{Pdfs, Saved};
use crate::seating::{Outcome, Progress, Search};

/// The project file.
#[derive(Clone)]
pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(location: &str) -> Self {
        Self {
            path: PathBuf::from(location),
        }
    }

    pub fn location(&self) -> String {
        self.path.display().to_string()
    }

    pub fn read(&self) -> Option<Vec<u8>> {
        fs::read(&self.path).ok()
    }

    /// Writes atomically: a temporary file renamed over the old one.
    pub fn write(&self, bytes: &[u8]) -> Result<()> {
        let temp = self.path.with_extension("json.tmp");
        fs::write(&temp, bytes).with_context(|| format!("writing {}", temp.display()))?;
        fs::rename(&temp, &self.path).with_context(|| format!("saving {}", self.path.display()))
    }

    /// The folder files are saved to.
    fn folder(&self) -> PathBuf {
        self.path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }
}

/// Work on a background thread.
struct Task<T>(Receiver<T>);

impl<T: Send + 'static> Task<T> {
    fn spawn(work: impl FnOnce() -> T + Send + 'static) -> Self {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(work());
        });
        Self(receiver)
    }

    /// The result, once it is ready.
    fn poll(&mut self) -> Option<T> {
        self.0.try_recv().ok()
    }
}

/// A seating search on its own threads, one per run.
pub struct SearchRunner {
    task: Task<Outcome>,
    progress: Progress,
}

impl SearchRunner {
    pub fn start(mut search: Search) -> Self {
        let progress = search.progress();
        let task = Task::spawn(move || {
            search.run_to_end();
            search.finish()
        });
        Self { task, progress }
    }

    pub fn poll(&mut self) -> Option<Outcome> {
        self.task.poll()
    }

    /// The best (score, broken hard rules) found so far.
    pub fn progress(&self) -> Option<(i64, i64)> {
        self.progress.best()
    }

    /// Ends the search early; `poll` then delivers the best found.
    pub fn stop(&self) {
        self.progress.stop();
    }
}

/// Saves `bytes` as `name` next to the project file.
pub fn save_file(store: &Store, name: &str, bytes: &[u8]) -> Result<Saved> {
    let path = store.folder().join(name);
    fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
    Ok(Saved {
        description: path.display().to_string(),
        location: Some(path),
    })
}

/// Opens a file or folder with the system's default app.
pub fn open_location(path: &Path) -> Result<()> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(path)
        .spawn()
        .with_context(|| format!("opening {}", path.display()))?;
    Ok(())
}

/// The folder files are saved to, for "Open folder".
pub fn save_folder(store: &Store) -> Option<PathBuf> {
    Some(store.folder())
}

/// PDFs typeset on a background thread.
pub struct PdfJob<K>(Task<Pdfs<K>>);

impl<K: Send + 'static> PdfJob<K> {
    /// `prints` holds each print's template data, or why there is none.
    pub fn start(prints: Vec<(K, Result<String, String>)>) -> Self {
        Self(Task::spawn(move || {
            prints
                .into_iter()
                .map(|(kind, data)| (kind, data.and_then(|data| plassi_typeset::pdf(&data))))
                .collect()
        }))
    }

    pub fn poll(&mut self) -> Option<Pdfs<K>> {
        self.0.poll()
    }
}

/// Projects kept in the app's data folder, one JSON file each:
/// `$PLASSI_HOME` if set, otherwise the platform's usual place
/// (`~/.local/share/plassi` on Linux).
pub struct Library {
    folder: PathBuf,
    /// Earlier versions: `backups/<project>/<time>.json`.
    backups: PathBuf,
}

/// Backups kept per project on disk.
const KEEP: super::Keep = super::Keep {
    recent: 20,
    days: 14,
};

fn data_folder() -> PathBuf {
    let env = |key: &str| std::env::var_os(key).map(PathBuf::from);
    if let Some(home) = env("PLASSI_HOME") {
        return home;
    }
    if cfg!(target_os = "windows") {
        return env("APPDATA").unwrap_or_default().join("Plassi");
    }
    let home = env("HOME").unwrap_or_default();
    if cfg!(target_os = "macos") {
        return home.join("Library/Application Support/Plassi");
    }
    env("XDG_DATA_HOME")
        .unwrap_or_else(|| home.join(".local/share"))
        .join("plassi")
}

impl Library {
    pub fn open() -> Self {
        Self::at(&data_folder())
    }

    /// A library in the data folder `data`.
    pub fn at(data: &Path) -> Self {
        let folder = data.join("projects");
        let _ = fs::create_dir_all(&folder);
        Self {
            folder,
            backups: data.join("backups"),
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.folder.join(format!("{name}.json"))
    }

    /// Project names, alphabetically.
    pub fn projects(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.folder)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension()? != "json" {
                    return None;
                }
                Some(path.file_stem()?.to_string_lossy().into_owned())
            })
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names
    }

    pub fn store(&self, name: &str) -> Store {
        Store {
            path: self.path(name),
        }
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<()> {
        fs::rename(self.path(from), self.path(to))
            .with_context(|| format!("renaming {from} to {to}"))?;
        let _ = fs::rename(self.backups.join(from), self.backups.join(to));
        Ok(())
    }

    /// Keeps `bytes` as an earlier version of `name`, and drops old ones.
    pub fn backup(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let folder = self.backups.join(name);
        fs::create_dir_all(&folder).with_context(|| format!("creating {}", folder.display()))?;
        let now = super::now_stamp().max(self.backups(name).first().map_or(0, |s| s + 1));
        let path = folder.join(format!("{}.json", super::backup_plan::stamp_name(now)));
        fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
        for old in super::backup_plan::to_prune(&self.backups(name), now, KEEP) {
            let _ = fs::remove_file(self.backup_path(name, old));
        }
        Ok(())
    }

    fn backup_path(&self, name: &str, stamp: u64) -> PathBuf {
        self.backups
            .join(name)
            .join(format!("{}.json", super::backup_plan::stamp_name(stamp)))
    }

    /// The times of `name`'s backups, newest first.
    pub fn backups(&self, name: &str) -> Vec<u64> {
        let mut stamps: Vec<u64> = fs::read_dir(self.backups.join(name))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                (path.extension()? == "json").then_some(())?;
                super::backup_plan::parse_stamp(&path.file_stem()?.to_string_lossy())
            })
            .collect();
        stamps.sort_unstable_by(|a, b| b.cmp(a));
        stamps
    }

    pub fn read_backup(&self, name: &str, stamp: u64) -> Option<Vec<u8>> {
        fs::read(self.backup_path(name, stamp)).ok()
    }

    /// Names that have backups, including deleted projects.
    pub fn backed_up(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.backups)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        fs::remove_file(self.path(name)).with_context(|| format!("deleting {name}"))
    }

    /// The project open last time.
    pub fn last(&self) -> Option<String> {
        self.setting("last-project")
    }

    pub fn remember(&self, name: &str) {
        self.set_setting("last-project", name);
    }

    /// A small app setting, kept as a file in the data folder.
    pub fn setting(&self, key: &str) -> Option<String> {
        fs::read_to_string(self.folder.join("..").join(key))
            .ok()
            .map(|s| s.trim().to_string())
    }

    pub fn set_setting(&self, key: &str, value: &str) {
        let _ = fs::write(self.folder.join("..").join(key), value);
    }

    /// The folder, for "Open folder".
    pub fn folder(&self) -> &Path {
        &self.folder
    }
}

/// The system's language, e.g. "fi_FI.UTF-8".
pub fn system_locale() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()))
}

/// Reads a file dropped onto the window (at once on desktop).
pub struct DroppedRead(std::cell::Cell<Option<Result<Vec<u8>, String>>>);

impl DroppedRead {
    pub fn start(file: eframe::egui::DroppedFileHandle) -> Self {
        Self(std::cell::Cell::new(Some(file.bytes())))
    }

    /// The file's bytes, once read.
    pub fn poll(&self) -> Option<Result<Vec<u8>, String>> {
        self.0.take()
    }
}

/// Minutes the local time zone is ahead of UTC at `stamp` (milliseconds since 1970).
pub fn utc_offset_minutes(stamp: u64) -> i64 {
    use chrono::{Local, Offset, TimeZone};
    Local
        .timestamp_millis_opt(stamp as i64)
        .single()
        .map_or(0, |t| t.offset().fix().local_minus_utc() as i64 / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backups_survive_delete_and_follow_renames() {
        let data = std::env::temp_dir().join(format!(
            "plassi-test-{}-{}",
            std::process::id(),
            super::super::now_stamp()
        ));
        let library = Library::at(&data);
        library.store("A").write(b"{}").unwrap();
        library.backup("A", b"one").unwrap();
        library.backup("A", b"two").unwrap();
        let stamps = library.backups("A");
        assert_eq!(stamps.len(), 2);
        assert_eq!(
            library.read_backup("A", stamps[0]).as_deref(),
            Some(&b"two"[..])
        );
        library.rename("A", "B").unwrap();
        assert_eq!(library.backups("B").len(), 2);
        library.delete("B").unwrap();
        assert!(library.projects().is_empty());
        assert_eq!(library.backed_up(), vec!["B".to_string()]);
        let _ = fs::remove_dir_all(&data);
    }
}
