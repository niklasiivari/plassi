//! In the browser the project lives only in local storage, which the browser
//! may clear. This shows where it is kept and reminds the user now and then to
//! download a copy.

// Only the browser keeps projects in local storage; desktop uses the rules in tests only.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(any(target_arch = "wasm32", test))]
use super::*;
#[cfg(any(target_arch = "wasm32", test))]
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

    /// Shows a download reminder in the sidebar or mobile navigation menu.
    pub(super) fn browser_storage(&mut self, ui: &mut egui::Ui, p: Palette) {
        if !self.copy_due() {
            return;
        }
        ui.add_space(theme::space::S);
        match reminder_card(ui, p) {
            Some(ReminderAction::Download) => self.download_project(),
            Some(ReminderAction::Later) => self.reset_copy_reminder(),
            None => {}
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReminderAction {
    Download,
    Later,
}

/// A compact notice with complete labels, even in the sidebar's narrow column.
#[cfg(any(target_arch = "wasm32", test))]
fn reminder_card(ui: &mut egui::Ui, p: Palette) -> Option<ReminderAction> {
    let mut action = None;
    theme::tinted_card(p, p.warn)
        .shadow(egui::Shadow::NONE)
        .corner_radius(theme::radius::SM)
        .inner_margin(egui::Margin::same(theme::space::S as i8))
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.spacing_mut().item_spacing.y = theme::space::XS;
                ui.label(
                    RichText::new(tr("Download backup"))
                        .size(theme::text::SMALL)
                        .strong(),
                );
                let size = egui::vec2(ui.available_width(), theme::CONTROL_HEIGHT);
                if ui
                    .add_sized(size, theme::primary_button(tr("Download"), p))
                    .clicked()
                {
                    action = Some(ReminderAction::Download);
                }
                if ui
                    .add_sized(size, theme::ghost_button(tr("Not now"), p))
                    .clicked()
                {
                    action = Some(ReminderAction::Later);
                }
            });
        });
    action
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

#[cfg(test)]
mod layout_tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn narrow_backup_notice_keeps_both_actions_readable_and_clickable() {
        for lang in [crate::i18n::Lang::En, crate::i18n::Lang::Fi] {
            crate::i18n::set(lang);
            let mut harness = Harness::builder()
                .with_size(egui::vec2(220.0, 300.0))
                .build_ui_state(
                    |ui, action| {
                        theme::install(ui.ctx());
                        ui.set_width(152.0);
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        *action = reminder_card(ui, theme::palette(ui.ctx())).or(*action);
                    },
                    None,
                );
            harness.run();
            for label in [tr("Download backup"), tr("Download"), tr("Not now")] {
                let node = harness.get_by_label(label);
                let rect = node.rect();
                assert!(
                    rect.right() <= 160.0 && rect.width() >= 70.0,
                    "{label}: {rect:?}"
                );
                assert!(rect.height() <= 40.0, "{label}: {rect:?}");
            }
            harness.get_by_label(tr("Not now")).click();
            harness.run();
            assert_eq!(*harness.state(), Some(ReminderAction::Later));
            harness.get_by_label(tr("Download")).click();
            harness.run();
            assert_eq!(*harness.state(), Some(ReminderAction::Download));
        }
    }
}
