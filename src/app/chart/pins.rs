//! Pinning people to their seat or keeping them at a table.

use super::*;
use crate::model::{Pin, seat_table};

impl PlassiApp {
    /// Pins a person, or unpins them with `None`.
    pub(in crate::app) fn set_pin(&mut self, person: usize, pin: Option<Pin>) {
        if self.file.external_invalid {
            return;
        }
        let name = self.project.person_name(person).to_string();
        let message = match &pin {
            Some(Pin::Seat(_)) => fill("Pinned {}", &[&name]),
            Some(Pin::Table(table)) => {
                fill("Kept {} at {}", &[&name, &self.project.table_label(table)])
            }
            None => fill("Unpinned {}", &[&name]),
        };
        match pin {
            Some(pin) => self.project.pin(person, pin),
            None => self.project.unpin(person),
        }
        self.commit(&message);
    }

    /// Unpins a pinned person, or pins a seated one to their seat.
    pub(super) fn toggle_seat_pin(&mut self, person: usize) {
        if self.project.pins().contains_key(&person) {
            self.set_pin(person, None);
        } else if let Some(seat) = self.seat_in_option(person) {
            self.set_pin(person, Some(Pin::Seat(seat)));
        }
    }

    /// Keeps people at a table: the solver may change their seats there.
    pub(super) fn keep_at_table(&mut self, people: &[usize], table: &str) {
        if self.file.external_invalid || people.is_empty() {
            return;
        }
        for &person in people {
            self.project.pin(person, Pin::Table(table.into()));
        }
        let label = self.project.table_label(table).to_string();
        let message = match people {
            [one] => fill("Kept {} at {}", &[&self.project.person_name(*one), &label]),
            _ => fill("Kept {} people at {}", &[&people.len(), &label]),
        };
        self.commit(&message);
    }

    /// "Keep at table" with every table to choose from, the one the people
    /// already share first. Returns the table chosen.
    pub(super) fn keep_at_table_menu(&self, ui: &mut egui::Ui, people: &[usize]) -> Option<String> {
        let shared = self.shared_table(people);
        let mut tables: Vec<(String, String)> = self
            .project
            .layout
            .rows
            .iter()
            .flat_map(|r| &r.tables)
            .map(|t| (t.id.clone(), t.label.clone()))
            .collect();
        tables.sort_by_key(|(id, _)| Some(id) != shared.as_ref());
        let mut chosen = None;
        egui::containers::menu::MenuButton::new(tr("Keep at table"))
            .config(egui::containers::menu::MenuConfig::new().style(theme::menu_style))
            .ui(ui, |ui| {
                for (id, label) in tables {
                    let text = if Some(&id) == shared.as_ref() {
                        fill("{} (where they sit)", &[&label])
                    } else {
                        label
                    };
                    if ui.button(text).clicked() {
                        chosen = Some(id);
                        ui.close();
                    }
                }
            });
        chosen
    }

    /// The table where all of these people sit, if they share one.
    fn shared_table(&self, people: &[usize]) -> Option<String> {
        let mut tables = people
            .iter()
            .map(|p| self.seat_in_option(*p).map(|s| seat_table(&s).to_string()));
        let first = tables.next()??;
        tables
            .all(|t| t.as_deref() == Some(&first))
            .then_some(first)
    }

    /// The person's seat in the selected option.
    pub(super) fn seat_in_option(&self, person: usize) -> Option<String> {
        self.project
            .candidates
            .get(self.selected_candidate)?
            .assignment
            .get(&person)
            .cloned()
    }
}
