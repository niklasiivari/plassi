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

    pub(super) fn review_navigation(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        current: usize,
    ) -> bool {
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
            return false;
        };
        let typing = ui
            .memory(|m| m.focused())
            .is_some_and(|id| egui::TextEdit::load_state(ui.ctx(), id).is_some());
        let (previous, next, review) = if !typing && !egui::Popup::is_any_open(ui.ctx()) {
            ui.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::NONE, egui::Key::K),
                    i.consume_key(egui::Modifiers::NONE, egui::Key::J),
                    i.consume_key(egui::Modifiers::NONE, egui::Key::R),
                )
            })
        } else {
            (false, false, false)
        };
        let mut go = if previous && pos > 0 {
            Some(queue[pos - 1])
        } else if next && pos + 1 < queue.len() {
            Some(queue[pos + 1])
        } else {
            None
        };
        let mut mark = review
            && self
                .project
                .notes
                .iter()
                .any(|n| n.person == current && !n.reviewed);
        let mut changed = false;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(tr("Review sign-up notes")).strong())
                    .on_hover_text(tr("K: previous · J: next · R: review and next"));
                theme::hint(ui, p, fill("Person {} of {}", &[&(pos + 1), &queue.len()]));
            });
            theme::hint(
                ui,
                p,
                tr("Read the note below and add any missing wishes before marking it reviewed."),
            );
            ui.add_space(theme::space::S);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(pos > 0, egui::Button::new(tr("Previous person")))
                    .clicked()
                {
                    go = Some(queue[pos - 1]);
                }
                if ui
                    .add_enabled(pos + 1 < queue.len(), egui::Button::new(tr("Next person")))
                    .on_hover_text(tr("Continue without marking reviewed"))
                    .clicked()
                {
                    go = Some(queue[pos + 1]);
                }
                let pending = self
                    .project
                    .notes
                    .iter()
                    .any(|n| n.person == current && !n.reviewed);
                mark |= ui
                    .add_enabled(
                        pending,
                        theme::primary_button(tr("Mark reviewed & next"), p),
                    )
                    .clicked();
                if self.people.review.is_some() && ui.button(tr("Finish review")).clicked() {
                    self.people.review = None;
                    self.selected_person = None;
                }
            });
        });
        if mark {
            for note in self
                .project
                .notes
                .iter_mut()
                .filter(|n| n.person == current)
            {
                note.reviewed = true;
            }
            self.people.wish_draft = None;
            if pos + 1 < queue.len() {
                go = Some(queue[pos + 1]);
            } else {
                self.people.review = None;
                self.selected_person = None;
            }
            changed = true;
        }
        if let Some(id) = go {
            self.selected_person = Some(id);
            self.people.wish_draft = None;
        }
        ui.add_space(theme::space::M);
        changed
    }
}
