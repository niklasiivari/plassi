//! A review queue stays available for backtracking after notes are marked reviewed.
use super::*;
use crate::i18n::fill;

impl PeopleState {
    pub(in crate::app) fn end_review(&mut self) {
        self.review = None;
    }
}

impl PlassiApp {
    pub(super) fn start_review(&mut self) {
        let pending: std::collections::HashSet<_> = self
            .project
            .notes
            .iter()
            .filter(|n| !n.reviewed)
            .map(|n| n.person)
            .collect();
        let queue: Vec<_> = self
            .project
            .attendees
            .iter()
            .filter(|a| pending.contains(&a.id))
            .map(|a| a.id)
            .collect();
        self.selected_person = queue.first().copied();
        self.people.review = Some(queue);
        self.people.filter = PeopleFilter::ToReview;
    }

    pub(super) fn review_navigation(&mut self, ui: &mut egui::Ui, p: Palette, current: usize) {
        let queue = self.people.review.clone().unwrap_or_else(|| {
            let with_notes: std::collections::HashSet<_> =
                self.project.notes.iter().map(|n| n.person).collect();
            self.project
                .attendees
                .iter()
                .filter(|a| with_notes.contains(&a.id))
                .map(|a| a.id)
                .collect()
        });
        let queue: Vec<_> = queue
            .into_iter()
            .filter(|id| self.project.person(*id).is_some())
            .collect();
        let Some(pos) = queue.iter().position(|id| *id == current) else {
            return;
        };
        let mut go = None;
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(pos > 0, egui::Button::new(tr("Previous person")))
                .clicked()
            {
                go = Some(queue[pos - 1]);
            }
            theme::hint(ui, p, fill("Person {} of {}", &[&(pos + 1), &queue.len()]));
            if ui
                .add_enabled(pos + 1 < queue.len(), egui::Button::new(tr("Next person")))
                .on_hover_text(tr("Continue without marking reviewed"))
                .clicked()
            {
                go = Some(queue[pos + 1]);
            }
            if self.people.review.is_some() && ui.button(tr("Finish review")).clicked() {
                self.people.review = None;
                self.selected_person = None;
            }
        });
        if let Some(id) = go {
            self.selected_person = Some(id);
            self.people.wish_draft = None;
        }
        ui.add_space(theme::space::M);
    }
}
