//! Sign-up notes: the original text, suggested wishes, and review state.
//! Reviewed notes collapse to one quiet line.

use super::*;
use crate::i18n::tr;
use crate::suggest::{self, Confidence, SuggestedTarget, Suggestion};

impl PlassiApp {
    pub(super) fn sign_up_notes(&mut self, ui: &mut egui::Ui, p: Palette, id: usize) -> bool {
        let notes: Vec<usize> = (0..self.project.notes.len())
            .filter(|&i| self.project.notes[i].person == id)
            .collect();
        if notes.is_empty() {
            return false;
        }
        theme::section_title(ui, p, tr("Sign-up notes"));
        let mut changed = false;
        for index in notes {
            changed |= self.note_card(ui, p, index);
        }
        ui.add_space(theme::space::XL);
        changed
    }

    /// One sign-up note with its suggested wishes and review controls.
    fn note_card(&mut self, ui: &mut egui::Ui, p: Palette, index: usize) -> bool {
        let note = self.project.notes[index].clone();
        if note.reviewed {
            return self.reviewed_note(ui, p, index);
        }
        let (project, people) = (&self.project, &mut self.people);
        let suggestions = people
            .note_suggestions
            .get((self.file.revision, index), || {
                people.suggester.for_note(project, index)
            })
            .clone();
        let mut changed = false;
        theme::tinted_card(p, p.warn).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                let field = match note.field {
                    ImportField::Avec => tr("Avec"),
                    ImportField::Wish => tr("Seating wish"),
                };
                theme::hint(ui, p, field);
                let (text, color) = if note.reviewed {
                    (tr("Reviewed"), p.good)
                } else {
                    (tr("Unreviewed"), p.warn)
                };
                theme::status_pill(ui, p, text, color);
            });
            ui.label(RichText::new(&note.text).size(theme::text::LEAD));
            for suggestion in &suggestions {
                if self.suggestion_row(ui, p, suggestion) {
                    suggest::accept(&mut self.project, suggestion);
                    changed = true;
                }
            }
            ui.horizontal_wrapped(|ui| {
                if ui.button(tr("+ Create wish")).clicked() {
                    self.people.draft_picker_filter.clear();
                    self.people.wish_draft = Some(WishDraft {
                        person: note.person,
                        wish: note.field.wish(),
                        near: DraftTarget::People,
                        targets: likely_people(&suggestions),
                        source: note.text.clone(),
                        note: Some(index),
                    });
                }
                let label = if note.reviewed {
                    tr("Reopen")
                } else {
                    tr("Mark reviewed")
                };
                if ui.button(label).clicked() {
                    self.project.notes[index].reviewed = !note.reviewed;
                    changed = true;
                }
                let others_left = self
                    .project
                    .notes
                    .iter()
                    .enumerate()
                    .any(|(i, n)| i != index && n.person == note.person && !n.reviewed);
                if !others_left
                    && let Some(next) = self.next_to_review(note.person)
                    && ui
                        .add(theme::primary_button(tr("Review and next"), p))
                        .on_hover_text(tr("Mark reviewed and go to the next person"))
                        .clicked()
                {
                    self.project.notes[index].reviewed = true;
                    self.selected_person = Some(next);
                    self.people.wish_draft = None;
                    changed = true;
                }
            });
        });
        changed
    }

    /// A reviewed note on one line, with Reopen.
    fn reviewed_note(&mut self, ui: &mut egui::Ui, p: Palette, index: usize) -> bool {
        let note = &self.project.notes[index];
        let field = match note.field {
            ImportField::Avec => tr("Avec"),
            ImportField::Wish => tr("Seating wish"),
        };
        let text = format!("“{}”", note.text);
        let mut reopen = false;
        ui.horizontal_wrapped(|ui| {
            theme::status_dot(ui, p.good);
            ui.label(RichText::new(field).size(theme::text::SMALL).color(p.muted));
            ui.label(RichText::new(text).size(theme::text::SMALL));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                reopen = ui.add(theme::small_button(tr("Reopen"))).clicked();
            });
        });
        if reopen {
            self.project.notes[index].reviewed = false;
        }
        reopen
    }

    /// Returns true when the suggestion's Add button was clicked.
    fn suggestion_row(&self, ui: &mut egui::Ui, p: Palette, suggestion: &Suggestion) -> bool {
        ui.horizontal_wrapped(|ui| {
            let color = match suggestion.confidence {
                Confidence::High => p.good,
                Confidence::Medium => p.accent,
                Confidence::Low => p.muted,
            };
            theme::status_pill(ui, p, suggestion.confidence.label(), color);
            ui.label(RichText::new(self.suggestion_text(suggestion)).strong());
            theme::hint(ui, p, &suggestion.reason);
            ui.add(theme::small_button(tr("+ Add"))).clicked()
        })
        .inner
    }
}

/// Single people suggested with at least "Likely match" confidence, to prefill a new wish.
fn likely_people(suggestions: &[Suggestion]) -> Vec<usize> {
    suggestions
        .iter()
        .filter(|s| s.confidence <= Confidence::Medium)
        .filter_map(|s| match &s.target {
            SuggestedTarget::People(ids) if ids.len() == 1 => Some(ids[0]),
            _ => None,
        })
        .collect()
}
