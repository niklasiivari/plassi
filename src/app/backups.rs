//! Earlier versions of projects: taken automatically every few minutes while
//! editing and before risky operations, listed and restored from the project
//! switcher. Storage is in `platform::Library`.

use super::*;
use crate::i18n::{fill, tr};

/// At most one automatic backup this often while editing.
const EVERY: Duration = Duration::from_secs(5 * 60);

/// The backup list being shown, and when the last automatic backup was taken.
#[derive(Default)]
pub(super) struct BackupsState {
    view: Option<View>,
    last_auto: Option<Instant>,
}

enum View {
    /// The open project's backups.
    Versions(Vec<Version>),
    /// Projects that are gone but have backups.
    Deleted(Vec<String>),
}

struct Version {
    stamp: u64,
    people: usize,
    options: usize,
}

/// "2026-09-30 17:05" in local time, for milliseconds since 1970.
pub(super) fn format_stamp(stamp: u64) -> String {
    format_at_offset(stamp, platform::utc_offset_minutes(stamp))
}

/// The time `offset_minutes` ahead of UTC.
fn format_at_offset(stamp: u64, offset_minutes: i64) -> String {
    let minutes = (stamp / 60_000) as i64 + offset_minutes;
    let (days, minute_of_day) = (minutes.div_euclid(1440), minutes.rem_euclid(1440));
    let (year, month, day) = civil_date(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        minute_of_day / 60,
        minute_of_day % 60
    )
}

/// Year, month and day of a day count since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_date(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

impl PlassiApp {
    /// Keeps the last saved state as a backup, e.g. before a risky operation.
    pub(super) fn backup_now(&mut self) {
        if let Some(bytes) = self.file.last_bytes.clone() {
            self.backup_bytes(&bytes);
        }
    }

    pub(super) fn backup_bytes(&mut self, bytes: &[u8]) {
        if self.file.library.backup(&self.file.name, bytes).is_ok() {
            self.backups.last_auto = Some(Instant::now());
        }
    }

    /// A backup of the last saved state if none was taken for a while.
    pub(super) fn auto_backup(&mut self) {
        if self
            .backups
            .last_auto
            .is_none_or(|at| at.elapsed() >= EVERY)
        {
            self.backup_now();
        }
    }

    pub(super) fn backup_menu_items(&mut self, ui: &mut egui::Ui) {
        if ui.button(tr("Earlier versions…")).clicked() {
            self.show_versions();
            ui.close();
        }
        let projects = self.file.library.projects();
        let deleted: Vec<String> = self
            .file
            .library
            .backed_up()
            .into_iter()
            .filter(|n| !projects.contains(n))
            .collect();
        if !deleted.is_empty() && ui.button(tr("Deleted projects…")).clicked() {
            self.backups.view = Some(View::Deleted(deleted));
            ui.close();
        }
    }

    fn show_versions(&mut self) {
        let library = &self.file.library;
        let versions = library
            .backups(&self.file.name)
            .into_iter()
            .filter_map(|stamp| {
                let project = io::parse(&library.read_backup(&self.file.name, stamp)?).ok()?;
                Some(Version {
                    stamp,
                    people: project.attendees.len(),
                    options: project.candidates.len(),
                })
            })
            .collect();
        self.backups.view = Some(View::Versions(versions));
    }

    /// The backup list, when open.
    pub(super) fn backups_window(&mut self, ctx: &egui::Context, p: Palette) {
        let Some(view) = &self.backups.view else {
            return;
        };
        let title = match view {
            View::Versions(_) => tr("Earlier versions"),
            View::Deleted(_) => tr("Deleted projects"),
        };
        let mut open = true;
        let mut action = None;
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .frame(theme::card(p))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_min_width(360.0);
                egui::ScrollArea::vertical()
                    .max_height(420.0)
                    .show(ui, |ui| {
                        action = match view {
                            View::Versions(versions) => versions_list(ui, p, versions),
                            View::Deleted(names) => deleted_list(ui, p, names),
                        };
                    });
            });
        match action {
            Some(Restore::Version(stamp)) => self.restore_version(stamp),
            Some(Restore::Deleted(name)) => self.restore_deleted(&name),
            None if open => return,
            None => {}
        }
        self.backups.view = None;
    }

    /// Replaces the project with a backup, as an undoable change.
    fn restore_version(&mut self, stamp: u64) {
        let bytes = self.file.library.read_backup(&self.file.name, stamp);
        match bytes
            .ok_or_else(|| anyhow::anyhow!("the backup is gone"))
            .and_then(|b| io::parse(&b))
        {
            Ok(project) => {
                self.backup_now();
                self.project = project;
                self.selected_candidate = 0;
                self.commit(&fill(
                    "Restored the version from {}",
                    &[&format_stamp(stamp)],
                ));
            }
            Err(error) => self.status = fill("Could not restore: {}", &[&format!("{error:#}")]),
        }
    }

    /// Brings back a deleted project from its latest backup.
    fn restore_deleted(&mut self, name: &str) {
        let library = &self.file.library;
        let bytes = library
            .backups(name)
            .first()
            .and_then(|&stamp| library.read_backup(name, stamp));
        match bytes
            .ok_or_else(|| anyhow::anyhow!("no backup of {name}"))
            .and_then(|b| io::parse(&b))
        {
            Ok(project) => self.create_project(name, project),
            Err(error) => self.status = fill("Could not restore: {}", &[&format!("{error:#}")]),
        }
    }
}

enum Restore {
    Version(u64),
    Deleted(String),
}

fn versions_list(ui: &mut egui::Ui, p: Palette, versions: &[Version]) -> Option<Restore> {
    if versions.is_empty() {
        theme::hint(
            ui,
            p,
            tr("No earlier versions yet. One is kept every few minutes while you edit."),
        );
    }
    let mut picked = None;
    for version in versions {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(format_stamp(version.stamp));
                let counts = fill(
                    "{} people, {} options",
                    &[&version.people, &version.options],
                );
                ui.label(
                    RichText::new(counts)
                        .color(p.muted)
                        .size(theme::text::SMALL),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(theme::secondary_button(tr("Restore"))).clicked() {
                    picked = Some(Restore::Version(version.stamp));
                }
            });
        });
        ui.add_space(theme::space::S);
    }
    picked
}

fn deleted_list(ui: &mut egui::Ui, p: Palette, names: &[String]) -> Option<Restore> {
    theme::hint(ui, p, tr("Restoring opens the project's latest backup."));
    let mut picked = None;
    for name in names {
        ui.horizontal(|ui| {
            ui.label(name);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(theme::secondary_button(tr("Restore"))).clicked() {
                    picked = Some(Restore::Deleted(name.clone()));
                }
            });
        });
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_times_with_offset() {
        assert_eq!(format_at_offset(0, 0), "1970-01-01 00:00");
        assert_eq!(
            format_at_offset(951_782_400_000 + 61_000, 0),
            "2000-02-29 00:01"
        );
        assert_eq!(format_at_offset(1_790_000_000_000, 180), "2026-09-21 17:13");
        assert_eq!(format_at_offset(0, -60), "1969-12-31 23:00");
    }
}
