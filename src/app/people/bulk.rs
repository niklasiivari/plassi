//! Actions on several people at once, selected by tapping or Ctrl-clicking.

use super::*;
use crate::i18n::{fill, tr};

impl PlassiApp {
    /// With people picked: review, pin, unpin, group, remove, clear.
    /// Otherwise a hint and tr("Select all shown").
    pub(super) fn bulk_bar(&mut self, ui: &mut egui::Ui) {
        let picked: Vec<usize> = self.people.picked.iter().copied().collect();
        if picked.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.toggle_value(&mut self.people.selecting, tr("Select people"));
                if ui
                    .add(theme::small_button(tr("Select all shown")))
                    .clicked()
                {
                    self.people.picked = self.people.shown.iter().copied().collect();
                    self.people.selecting = true;
                }
            });
            return;
        }
        let has_seat = self
            .project
            .candidates
            .get(self.selected_candidate)
            .is_some_and(|c| picked.iter().any(|id| c.assignment.contains_key(id)));
        let pins = self.project.pins();
        let has_pin = picked.iter().any(|id| pins.contains_key(id));
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(fill("{} selected", &[&picked.len()])).strong());
            if ui
                .add(theme::small_button(tr("Mark notes reviewed")))
                .clicked()
            {
                self.bulk_mark_reviewed(&picked);
            }
            if ui
                .add_enabled(has_seat, theme::small_button(tr("Pin to current seats")))
                .on_disabled_hover_text(tr("Seat selected people in an option before pinning."))
                .clicked()
            {
                self.bulk_pin(&picked, true);
            }
            if ui
                .add_enabled(has_pin, theme::small_button(tr("Unpin")))
                .on_disabled_hover_text(tr("Selected people have no pins."))
                .clicked()
            {
                self.bulk_pin(&picked, false);
            }
            if picked.len() >= 2 && ui.add(theme::small_button(tr("Group together"))).clicked() {
                self.project.add_preference(
                    crate::preferences::PreferenceKind::together(picked.clone()),
                    crate::preferences::Strength::Important,
                );
                self.commit(tr("Preference added; solve again to apply"));
            }
            if ui
                .add(theme::small_button(tr("Remove from event")))
                .clicked()
            {
                self.remove_people(&picked);
            }
            if ui.add(theme::small_button(tr("Clear"))).clicked() {
                self.people.picked.clear();
                self.people.selecting = false;
            }
        });
        if !self.people.picked.is_empty() && self.tags_editor(ui, theme::palette(ui.ctx()), &picked)
        {
            self.commit(tr("Saved changes"));
        }
    }

    fn bulk_mark_reviewed(&mut self, picked: &[usize]) {
        let mut count = 0;
        for note in self
            .project
            .notes
            .iter_mut()
            .filter(|n| picked.contains(&n.person) && !n.reviewed)
        {
            note.reviewed = true;
            count += 1;
        }
        self.commit(&fill("Reviewed {} notes", &[&count]));
    }

    /// Pins everyone picked to their seat in the selected option, or unpins them.
    fn bulk_pin(&mut self, picked: &[usize], pin: bool) {
        let seats = self
            .project
            .candidates
            .get(self.selected_candidate)
            .map(|c| c.assignment.clone())
            .unwrap_or_default();
        let pins = self.project.pins();
        let mut affected = Vec::new();
        for &id in picked {
            match (pin, seats.get(&id)) {
                (true, Some(seat)) => {
                    self.project.pin(id, crate::model::Pin::Seat(seat.clone()));
                    affected.push(id);
                }
                (true, None) => {}
                (false, _) if pins.contains_key(&id) => {
                    self.project.unpin(id);
                    affected.push(id);
                }
                (false, _) => {}
            }
        }
        if affected.is_empty() {
            return;
        }
        let message = match (pin, affected.as_slice()) {
            (true, [one]) => fill("Pinned {}", &[&self.project.person_name(*one)]),
            (false, [one]) => fill("Unpinned {}", &[&self.project.person_name(*one)]),
            (true, _) => fill("Pinned {} people", &[&affected.len()]),
            (false, _) => fill("Unpinned {} people", &[&affected.len()]),
        };
        self.commit(&message);
    }
}
