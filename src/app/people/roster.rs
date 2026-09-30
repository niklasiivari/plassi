//! Adding a walk-in, removing a cancellation, or replacing one person with another.

use super::*;
use crate::i18n::{fill, tr};
use crate::roster::{self, NewAttendee};

/// A roster change being prepared in the People page.
#[derive(Clone)]
pub(in crate::app) enum RosterEdit {
    Add(NewAttendee),
    /// Replace this attendee with a new one in the same seat.
    Replace(usize, NewAttendee),
}

enum FormAction {
    Submit,
    Cancel,
}

impl PlassiApp {
    /// Shows the open roster form, if any. Returns true if it took the page.
    pub(super) fn roster_form(&mut self, ui: &mut egui::Ui, p: Palette) -> bool {
        let Some(edit) = self.people.roster.as_mut() else {
            return false;
        };
        let action = match edit {
            RosterEdit::Add(person) => attendee_form(ui, p, tr("Add a person"), tr("Add"), person),
            RosterEdit::Replace(old, person) => {
                let old_name = self.project.person_name(*old).to_string();
                let title = fill("Replace {}", &[&old_name]);
                ui.label(
                    RichText::new(fill(
                        "The new person takes {}'s seat in every option. Wishes by or about {} are removed.",
                        &[&old_name, &old_name],
                    ))
                    .color(p.muted),
                );
                attendee_form(ui, p, &title, tr("Replace"), person)
            }
        };
        match action {
            Some(FormAction::Submit) => self.apply_roster_edit(),
            Some(FormAction::Cancel) => self.people.roster = None,
            None => {}
        }
        true
    }

    /// Removes people from the event; their seats become free. Undo brings them back.
    pub(in crate::app) fn remove_people(&mut self, ids: &[usize]) {
        let names: Vec<String> = ids
            .iter()
            .map(|id| self.project.person_name(*id).to_string())
            .collect();
        for &id in ids {
            roster::remove(&mut self.project, id);
        }
        let template = if ids.len() == 1 {
            "Removed {}; their seat is free (Improve or Close gaps on the chart)"
        } else {
            "Removed {}; their seats are free (Improve or Close gaps on the chart)"
        };
        self.commit(&fill(template, &[&names.join(", ")]));
    }

    fn apply_roster_edit(&mut self) {
        let Some(edit) = self.people.roster.take() else {
            return;
        };
        let retry = edit.clone();
        let result = match edit {
            RosterEdit::Add(person) => {
                let name = person.name.clone();
                roster::add(&mut self.project, person)
                    .map(|id| (id, fill("Added {} to the best free seat", &[&name])))
            }
            RosterEdit::Replace(old, person) => {
                let name = person.name.clone();
                roster::replace(&mut self.project, old, person)
                    .map(|id| (id, fill("{} took over the seat", &[&name])))
            }
        };
        match result {
            Ok((id, message)) => {
                self.selected_person = Some(id);
                self.commit(&message);
            }
            Err(error) => {
                self.people.roster = Some(retry);
                self.status = format!("{error:#}");
            }
        }
    }
}

/// Name and details of a person. Returns the button pressed, if any.
fn attendee_form(
    ui: &mut egui::Ui,
    p: Palette,
    title: &str,
    submit: &str,
    person: &mut NewAttendee,
) -> Option<FormAction> {
    let mut action = None;
    theme::raised_card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).strong().size(theme::text::LEAD));
        egui::Grid::new("attendee-form")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                for (label, value) in [
                    (tr("Name"), &mut person.name),
                    (tr("Organization"), &mut person.organization),
                    (tr("Ticket / group"), &mut person.participant_group),
                    (tr("Food"), &mut person.food),
                    (tr("Drinks"), &mut person.drink),
                    (tr("Allergies / diet"), &mut person.allergies),
                ] {
                    ui.label(RichText::new(label).color(p.muted));
                    ui.add(theme::field(value).desired_width(340.0));
                    ui.end_row();
                }
            });
        ui.horizontal(|ui| {
            let ready = !person.name.trim().is_empty();
            if ui
                .add_enabled(ready, theme::primary_button(submit, p))
                .clicked()
            {
                action = Some(FormAction::Submit);
            }
            if ui.button(tr("Cancel")).clicked() {
                action = Some(FormAction::Cancel);
            }
        });
    });
    action
}
