//! Adding a walk-in, removing a cancellation, or replacing one person with another.

use super::*;
use crate::catalog::Kind;
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
        if self.people.roster.is_none() {
            return false;
        }
        let choices = [Kind::Organization, Kind::Ticket, Kind::Food, Kind::Drink]
            .map(|kind| self.category_choices(kind));
        let Some(edit) = self.people.roster.as_mut() else {
            return false;
        };
        let action = match edit {
            RosterEdit::Add(person) => {
                attendee_form(ui, p, tr("Add a person"), tr("Add"), person, &choices)
            }
            RosterEdit::Replace(old, person) => {
                let old_name = self.project.person_name(*old).to_string();
                let title = fill("Replace {}", &[&old_name]);
                ui.label(
                    RichText::new(fill(
                        "Takes {}'s seats in all options; removes wishes involving {}.",
                        &[&old_name, &old_name],
                    ))
                    .color(p.muted),
                );
                attendee_form(ui, p, &title, tr("Replace"), person, &choices)
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
            "Removed {}; seat freed"
        } else {
            "Removed {}; seats freed"
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

/// Name and details of a person, with the existing organizations, tickets,
/// meals and drinks to pick from. Returns the button pressed, if any.
fn attendee_form(
    ui: &mut egui::Ui,
    p: Palette,
    title: &str,
    submit: &str,
    person: &mut NewAttendee,
    choices: &[Vec<String>; 4],
) -> Option<FormAction> {
    let mut action = None;
    theme::raised_card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).strong().size(theme::text::LEAD));
        egui::Grid::new("attendee-form")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                let none: &[String] = &[];
                for (label, value, options) in [
                    (tr("Name"), &mut person.name, none),
                    (
                        tr("Organization"),
                        &mut person.organization,
                        &choices[0][..],
                    ),
                    (
                        tr("Ticket / group"),
                        &mut person.participant_group,
                        &choices[1][..],
                    ),
                    (tr("Food"), &mut person.food, &choices[2][..]),
                    (tr("Drinks"), &mut person.drink, &choices[3][..]),
                    (tr("Allergies / diet"), &mut person.allergies, none),
                ] {
                    ui.label(RichText::new(label).color(p.muted));
                    field_with_choices(ui, label, value, options, 340.0);
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

/// A text field with a dropdown of known values beside it; picking one fills
/// the field, and any other text can still be typed. True when the text was
/// committed (focus left) or a value was picked.
pub(super) fn field_with_choices(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut String,
    choices: &[String],
    width: f32,
) -> bool {
    ui.horizontal(|ui| {
        let mut changed = ui
            .add(theme::field(value).desired_width(width))
            .lost_focus();
        if !choices.is_empty() {
            theme::combo(("choices", id), "")
                .width(28.0)
                .show_ui(ui, |ui| {
                    for choice in choices {
                        if ui.selectable_label(value == choice, choice).clicked() {
                            value.clone_from(choice);
                            changed = true;
                        }
                    }
                });
        }
        changed
    })
    .inner
}
