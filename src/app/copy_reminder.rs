//! In the browser the project lives only in local storage, which the browser
//! may clear. This shows where it is kept and reminds the user now and then to
//! download a copy.

// Only the browser keeps projects in local storage; desktop uses the rules in tests only.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
use super::*;
#[cfg(target_arch = "wasm32")]
use crate::i18n::tr;

/// Edits after which a copy is worth downloading.
const MANY_EDITS: u64 = 20;
/// Edits that, a day after the last download, are worth a copy.
const SOME_EDITS: u64 = 5;
const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// Work done since the last download (or dismissed reminder).
#[derive(Default)]
pub(super) struct CopyReminder {
    /// The project the counts below belong to.
    project: String,
    revision: u64,
    options: usize,
}

/// Whether to suggest downloading a copy: after many edits, after a new
/// seating option, or after a few edits once a day has passed.
fn due(edits: u64, new_options: bool, last_download: Option<u64>, now: u64) -> bool {
    let stale = last_download.is_none_or(|t| now.saturating_sub(t) >= DAY_MS);
    edits >= MANY_EDITS || (new_options && edits > 0) || (stale && edits >= SOME_EDITS)
}

fn download_key(name: &str) -> String {
    format!("downloaded:{name}")
}

#[cfg(target_arch = "wasm32")]
impl PlassiApp {
    /// Starts counting work on the open project from now.
    fn reset_copy_reminder(&mut self) {
        self.copy_reminder = CopyReminder {
            project: self.file.name.clone(),
            revision: self.file.revision,
            options: self.project.candidates.len(),
        };
    }

    /// Notes that the open project was just downloaded.
    pub(super) fn downloaded_copy(&mut self) {
        let now = platform::now_stamp().to_string();
        self.file
            .library
            .set_setting(&download_key(&self.file.name), &now);
        self.reset_copy_reminder();
    }

    fn copy_due(&mut self) -> bool {
        if self.copy_reminder.project != self.file.name {
            self.reset_copy_reminder();
        }
        let r = &self.copy_reminder;
        let last = self
            .file
            .library
            .setting(&download_key(&self.file.name))
            .and_then(|t| t.parse().ok());
        due(
            self.file.revision.saturating_sub(r.revision),
            self.project.candidates.len() > r.options,
            last,
            platform::now_stamp(),
        )
    }

    /// Storage state and, when due, the download reminder (sidebar, laid out bottom up).
    pub(super) fn browser_storage(&mut self, ui: &mut egui::Ui, p: Palette) {
        if !self.copy_due() {
            return;
        }
        ui.add_space(theme::space::L);
        theme::tinted_card(p, p.warn).show(ui, |ui| {
            // The sidebar's bottom area grows upwards, so the rows go in
            // reverse: buttons, explanation, title.
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.horizontal(|ui| {
                    if ui.add(theme::primary_button(tr("Download"), p)).clicked() {
                        self.download_project();
                    }
                    if ui.add(theme::ghost_button(tr("Not now"), p)).clicked() {
                        self.reset_copy_reminder();
                    }
                });
                ui.label(
                    RichText::new(tr("Download backup"))
                        .size(theme::text::SMALL)
                        .strong(),
                );
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reminds_after_many_edits_or_a_new_option() {
        let now = 10 * DAY_MS;
        assert!(!due(3, false, Some(now), now));
        assert!(due(MANY_EDITS, false, Some(now), now));
        assert!(due(1, true, Some(now), now));
        assert!(!due(0, true, Some(now), now));
    }

    #[test]
    fn reminds_after_a_day_with_some_edits() {
        let now = 10 * DAY_MS;
        assert!(!due(SOME_EDITS, false, Some(now - DAY_MS / 2), now));
        assert!(due(SOME_EDITS, false, Some(now - DAY_MS), now));
        assert!(due(SOME_EDITS, false, None, now));
        assert!(!due(SOME_EDITS - 1, false, None, now));
    }
}
