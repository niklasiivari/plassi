//! Managing options: a new empty one, duplicating and deleting.

use super::*;

impl PlassiApp {
    /// Adds an option with only the people pinned to a seat seated, for
    /// seating the rest by hand or with "Seat the rest".
    pub(super) fn add_empty_option(&mut self) {
        let option = self.project.empty_option(self.project.new_option_name());
        self.project.candidates.push(option);
        self.selected_candidate = self.project.candidates.len() - 1;
        self.commit(tr("Added an empty option"));
    }

    pub(super) fn duplicate_option(&mut self, index: usize) {
        let Some(mut copy) = self.project.candidates.get(index).cloned() else {
            return;
        };
        copy.name = fill("{} (copy)", &[&copy.name]);
        self.project.candidates.insert(index + 1, copy);
        self.selected_candidate = index + 1;
        if let Some(compare) = self.chart.compare_with.as_mut().filter(|c| **c > index) {
            *compare += 1;
        }
        self.commit(tr("Duplicated option"));
    }

    /// Forgets who was placed by hand in option `index`, so Improve may move
    /// them again. Pins stay.
    pub(super) fn release_hand_placed(&mut self, index: usize) {
        let Some(candidate) = self.project.candidates.get_mut(index) else {
            return;
        };
        candidate.hand_placed.clear();
        let name = candidate.name.clone();
        self.commit(&fill("Improve may move everyone in {}", &[&name]));
    }

    pub(super) fn delete_option(&mut self, index: usize) {
        if index >= self.project.candidates.len() {
            return;
        }
        let name = self.project.candidates.remove(index).name;
        // Keep showing the same option when an earlier one goes.
        let shift = |i: usize| if i > index { i - 1 } else { i };
        if self.selected_candidate != index {
            self.selected_candidate = shift(self.selected_candidate);
        }
        self.chart.compare_with = self.chart.compare_with.filter(|c| *c != index).map(shift);
        self.commit(&fill("Deleted {}", &[&name]));
    }
}
