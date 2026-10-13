//! Actions on several people at once: Ctrl-click them in the list.

use super::*;
use crate::i18n::{fill, tr};

impl PlassiApp {
    /// With people picked: review, pin, unpin, group, remove, clear.
    /// Otherwise a hint and tr("Select all shown").
    pub(super) fn bulk_bar(&mut self, ui: &mut egui::Ui, p: Palette) {
        let picked: Vec<usize> = self.people.picked.iter().copied().collect();
        if picked.is_empty() {
            ui.horizontal(|ui| {
                theme::hint(ui, p, tr("Ctrl-click to select multiple people."));
                if ui
                    .add(theme::small_button(tr("Select all shown")))
                    .clicked()
                {
                    self.people.picked = self.people.shown.iter().copied().collect();
                }
            });
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(fill("{} selected", &[&picked.len()])).strong());
            if ui
                .add(theme::small_button(tr("Mark notes reviewed")))
                .clicked()
            {
                self.bulk_mark_reviewed(&picked);
            }
            if ui
                .add(theme::small_button(tr("Pin to current seats")))
                .clicked()
            {
                self.bulk_pin(&picked, true);
            }
            if ui.add(theme::small_button(tr("Unpin"))).clicked() {
                self.bulk_pin(&picked, false);
            }
            if picked.len() >= 2 && ui.add(theme::small_button(tr("Group together"))).clicked() {
                self.project.add_preference(
                    crate::preferences::PreferenceKind::together(picked.clone()),
                    crate::preferences::Strength::Important,
                );
                self.commit(&fill("Grouped {} people", &[&picked.len()]));
            }
            if ui
                .add(theme::small_button(tr("Remove from event")))
                .clicked()
            {
                self.remove_people(&picked);
            }
            if ui.add(theme::small_button(tr("Clear"))).clicked() {
                self.people.picked.clear();
            }
        });
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
        for &id in picked {
            match (pin, seats.get(&id)) {
                (true, Some(seat)) => {
                    self.project.pin(id, crate::model::Pin::Seat(seat.clone()));
                }
                (true, None) => {}
                (false, _) => self.project.unpin(id),
            }
        }
        let template = if pin {
            "Pinned {} people"
        } else {
            "Unpinned {} people"
        };
        self.commit(&fill(template, &[&picked.len()]));
    }
}
