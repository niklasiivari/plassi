//! Single-key shortcuts on the chart.

use super::*;

impl PlassiApp {
    /// Single-key shortcuts on the chart: F fits the room, P pins or unpins
    /// the picked people (or the selected person), Delete takes them off
    /// their seats, G groups the picked people, C changes what the cards are
    /// coloured by.
    pub(super) fn chart_shortcuts(&mut self, ui: &egui::Ui) {
        if ui.ctx().memory(|m| m.focused().is_some()) {
            return;
        }
        let key = |k| ui.input(|i| i.key_pressed(k) && i.modifiers.is_none());
        self.nudge_keys(ui);
        if key(egui::Key::F) {
            self.chart.view.fit_pending = true;
        }
        if key(egui::Key::P) {
            if !self.chart.multi.is_empty() {
                let people: Vec<usize> = self.chart.multi.iter().copied().collect();
                self.toggle_seat_pins(&people);
            } else if let Some(person) = self.selected_person {
                self.toggle_seat_pin(person);
            }
        }
        if key(egui::Key::Delete) || key(egui::Key::Backspace) {
            let people: Vec<usize> = match self.selected_person {
                _ if !self.chart.multi.is_empty() => self.chart.multi.iter().copied().collect(),
                Some(person) => vec![person],
                None => Vec::new(),
            };
            let wanted: Vec<_> = people.into_iter().map(|p| (p, Spot::Unseated)).collect();
            if !wanted.is_empty() {
                self.rearrange(&wanted);
            }
        }
        if key(egui::Key::C) {
            self.next_color_by();
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
