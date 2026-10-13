//! Single-key shortcuts on the chart.

use super::*;

impl PlassiApp {
    /// Single-key shortcuts on the chart: F fits the room, P pins or unpins
    /// the selected person, G groups the picked people.
    pub(super) fn chart_shortcuts(&mut self, ui: &egui::Ui) {
        if ui.ctx().memory(|m| m.focused().is_some()) {
            return;
        }
        let key = |k| ui.input(|i| i.key_pressed(k) && i.modifiers.is_none());
        if key(egui::Key::F) {
            self.chart.view.fit_pending = true;
        }
        if key(egui::Key::P)
            && let Some(person) = self.selected_person
        {
            self.toggle_seat_pin(person);
        }
        if key(egui::Key::G) && self.chart.multi.len() >= 2 {
            let people = std::mem::take(&mut self.chart.multi).into_iter().collect();
            self.project.add_preference(
                crate::preferences::PreferenceKind::together(people),
                crate::preferences::Strength::Important,
            );
            self.commit(tr("Preference added; solve again to apply"));
        }
    }
}
