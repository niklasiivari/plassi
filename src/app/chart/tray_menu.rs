//! The right-click menu of a card in the "Not seated" list.

use super::*;
use crate::app::people::RosterEdit;

/// What was picked in a list card's menu.
pub(super) enum TrayAction {
    /// Seat each person in the best free seat left for them.
    SeatBest(Vec<usize>),
    Keep(Vec<usize>, String),
    Pick(usize),
    Details(usize),
    Replace(usize),
    Remove(usize),
}

impl PlassiApp {
    /// The menu for `person`, acting on everyone selected in the list when
    /// they are selected.
    pub(super) fn tray_menu(
        &self,
        ui: &mut egui::Ui,
        scene: &Scene,
        person: usize,
    ) -> Option<TrayAction> {
        // Only the people in the list; seated ones in the selection stay put.
        let people: Vec<usize> = if self.chart.multi.contains(&person) {
            let multi = &self.chart.multi;
            scene
                .unseated
                .iter()
                .copied()
                .filter(|p| multi.contains(p))
                .collect()
        } else {
            vec![person]
        };
        let mut action = None;
        let seat = match people.len() {
            1 => tr("Seat in the best free seat").to_string(),
            n => fill("Seat {} people in the best free seats", &[&n]),
        };
        if ui.button(seat).clicked() {
            action = Some(TrayAction::SeatBest(people.clone()));
        }
        if let Some(table) = self.keep_at_table_menu(ui, &people) {
            action = Some(TrayAction::Keep(people, table));
        }
        let pick = if self.chart.multi.contains(&person) {
            tr("Remove from selection")
        } else {
            tr("Add to selection")
        };
        if ui.button(pick).clicked() {
            action = Some(TrayAction::Pick(person));
        }
        if ui.button(tr("Show details")).clicked() {
            action = Some(TrayAction::Details(person));
        }
        ui.separator();
        if ui.button(tr("Replace with someone else…")).clicked() {
            action = Some(TrayAction::Replace(person));
        }
        if ui.button(tr("Remove from event")).clicked() {
            action = Some(TrayAction::Remove(person));
        }
        action
    }

    pub(super) fn apply_tray_action(&mut self, action: TrayAction, scene: &Scene) {
        match action {
            TrayAction::SeatBest(people) => self.seat_in_best_free_seats(&people),
            TrayAction::Keep(people, table) => self.keep_at_table(&people, &table),
            TrayAction::Pick(person) => self.toggle_pick(person),
            TrayAction::Details(person) => {
                self.selected_person = Some(person);
                self.focus_person(person, scene);
            }
            TrayAction::Replace(person) => {
                self.selected_person = Some(person);
                self.people.roster = Some(RosterEdit::Replace(person, Default::default()));
                self.page = crate::app::Page::People;
            }
            TrayAction::Remove(person) => self.remove_people(&[person]),
        }
    }

    /// Seats people one after another, each in the best seat still free,
    /// as one edit.
    fn seat_in_best_free_seats(&mut self, people: &[usize]) {
        let Some(option) = self.project.candidates.get(self.selected_candidate) else {
            return;
        };
        let mut option = option.clone();
        let mut wanted = Vec::new();
        // Compiled once: the project stays the same while seats fill up.
        let scorer = crate::seating::Scorer::new(&self.project);
        for &person in people {
            let free = crate::arrange::free_seats(&self.project, &scorer, &option, person);
            let Some((seat, _)) = free.into_iter().next() else {
                continue;
            };
            option.place(person, Spot::Seat(seat.clone()));
            wanted.push((person, Spot::Seat(seat)));
        }
        if wanted.is_empty() {
            self.status = tr("No free seat left").into();
            return;
        }
        self.rearrange(&wanted);
    }
}
