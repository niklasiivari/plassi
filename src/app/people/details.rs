//! Food, drinks, allergies, extra columns and custom tags — shown as values,
//! editable on request.

use super::*;
use crate::i18n::tr;

impl PlassiApp {
    /// The details block. Returns true if anything was saved.
    pub(super) fn person_details_block(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        id: usize,
    ) -> bool {
        ui.horizontal(|ui| {
            theme::section_title(ui, p, tr("Details"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if self.people.editing_details {
                    tr("Done")
                } else {
                    tr("Edit")
                };
                if ui.add(theme::small_button(label)).clicked() {
                    self.people.editing_details = !self.people.editing_details;
                }
            });
        });
        let changed = if self.people.editing_details {
            self.details_fields(ui, p, id) | self.tags_editor(ui, p, id)
        } else {
            self.details_values(ui, p, id);
            false
        };
        ui.add_space(theme::space::XL);
        changed
    }

    fn details_values(&self, ui: &mut egui::Ui, p: Palette, id: usize) {
        let Some(a) = self.project.person(id) else {
            return;
        };
        let food = self.project.category_name(crate::catalog::Kind::Food, a);
        let drink = self.project.category_name(crate::catalog::Kind::Drink, a);
        let custom_tags: Vec<&str> = a
            .tags
            .iter()
            .filter(|t| **t != a.organization && **t != a.participant_group)
            .map(String::as_str)
            .collect();
        let tags = custom_tags.join(", ");
        let price = self
            .project
            .attendee_price_cents(a)
            .map(|cents| {
                format!(
                    "{}.{:02} €{}",
                    cents / 100,
                    cents % 100,
                    if a.price_override_cents.is_some() {
                        tr(" (override)")
                    } else {
                        ""
                    }
                )
            })
            .unwrap_or_else(|| tr("Not set").to_string());
        let mut rows = vec![
            (tr("Price"), price.as_str()),
            (tr("Food"), food.as_str()),
            (tr("Drinks"), drink.as_str()),
            (tr("Allergies / diet"), a.allergies.as_str()),
        ];
        rows.extend(a.extra.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        if !tags.is_empty() {
            rows.push((tr("Tags"), tags.as_str()));
        }
        egui::Grid::new("details")
            .num_columns(2)
            .spacing([theme::space::XL, theme::space::S])
            .show(ui, |ui| {
                for (label, value) in rows {
                    ui.label(RichText::new(label).color(p.muted));
                    if value.trim().is_empty() {
                        ui.label(RichText::new("—").color(p.muted));
                    } else {
                        ui.label(value);
                    }
                    ui.end_row();
                }
            });
    }

    fn details_fields(&mut self, ui: &mut egui::Ui, p: Palette, id: usize) -> bool {
        let Some(a) = self.project.attendees.iter_mut().find(|a| a.id == id) else {
            return false;
        };
        let mut changed = false;
        egui::Grid::new("details-edit")
            .num_columns(2)
            .spacing([theme::space::XL, theme::space::S])
            .show(ui, |ui| {
                ui.label(RichText::new(tr("Price")).color(p.muted));
                changed |=
                    theme::price_input(ui, tr("Override total"), &mut a.price_override_cents);
                ui.end_row();
                for (label, value) in [
                    (tr("Food"), &mut a.food),
                    (tr("Drinks"), &mut a.drink),
                    (tr("Allergies / diet"), &mut a.allergies),
                ] {
                    ui.label(RichText::new(label).color(p.muted));
                    changed |= ui
                        .add(theme::field(value).desired_width(360.0))
                        .lost_focus();
                    ui.end_row();
                }
            });
        changed
    }

    /// Custom tags (organization and ticket tags are automatic and not shown).
    fn tags_editor(&mut self, ui: &mut egui::Ui, p: Palette, id: usize) -> bool {
        let Some(index) = self.project.attendees.iter().position(|a| a.id == id) else {
            return false;
        };
        let mut changed = false;
        ui.add_space(theme::space::S);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(tr("Tags")).color(p.muted));
            let person = &mut self.project.attendees[index];
            let automatic = [
                person.organization.clone(),
                person.participant_group.clone(),
            ];
            let mut remove = None;
            for tag in person.tags.iter().filter(|t| !automatic.contains(t)) {
                let chip = theme::tag_button(tag);
                if ui.add(chip).on_hover_text(tr("Remove tag")).clicked() {
                    remove = Some(tag.clone());
                }
            }
            if let Some(tag) = remove {
                person.tags.retain(|t| *t != tag);
                changed = true;
            }
            let edit = ui.add(
                theme::field(&mut self.people.new_tag)
                    .hint_text(tr("Add tag"))
                    .desired_width(140.0),
            );
            let tag = self.people.new_tag.trim().to_string();
            if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !tag.is_empty()
            {
                if !person.tags.contains(&tag) {
                    person.tags.push(tag);
                    changed = true;
                }
                self.people.new_tag.clear();
            }
        });
        changed
    }
}
