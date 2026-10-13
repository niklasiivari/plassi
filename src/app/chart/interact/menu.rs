//! The right-click menu of a person's card.

use super::*;
use crate::model::seat_table;

impl PlassiApp {
    /// The right-click menu of a person's card.
    pub(super) fn card_menu(
        &self,
        ui: &mut egui::Ui,
        scene: &Scene,
        place: Place,
        id: usize,
        actions: &mut CardActions,
    ) {
        let pin = scene.pins.get(&id);
        let people = self.acting_on(id);
        match (place, pin) {
            _ if people.len() > 1 => {
                let label = if self.would_unpin(&people) {
                    fill("Unpin {} people (P)", &[&people.len()])
                } else {
                    fill("Pin {} people to their seats (P)", &[&people.len()])
                };
                if ui.button(label).clicked() {
                    actions.pin_all = Some(people.clone());
                }
            }
            (_, Some(_)) => {
                if ui.button(tr("Unpin (P)")).clicked() {
                    actions.pin = Some((id, None));
                }
            }
            (Place::Seat(seat), None) => {
                if ui.button(tr("Pin to this seat (P)")).clicked() {
                    actions.pin = Some((id, Some(Pin::Seat(seat.id.clone()))));
                }
            }
            (Place::Loose(..), None) => {}
        }
        if let Place::Seat(seat) = place
            && !matches!(pin, Some(Pin::Table(_)))
        {
            let label = match people.len() {
                1 => tr("Keep at this table").to_string(),
                n => fill("Keep {} people at this table", &[&n]),
            };
            if ui
                .button(label)
                .on_hover_text(tr("The solver may change their seats at this table"))
                .clicked()
            {
                actions.keep = Some((people.clone(), seat_table(&seat.id).to_string()));
            }
        }
        if ui.button(tr("Swap with…")).clicked() {
            actions.start_swap = Some(id);
        }
        let off = match (place, people.len()) {
            (Place::Seat(_), 1) => tr("Take off the seat").to_string(),
            (Place::Seat(_), n) => fill("Take {} people off their seats", &[&n]),
            (Place::Loose(..), 1) => tr("Move to the Not seated list").to_string(),
            (Place::Loose(..), n) => fill("Move {} people to the Not seated list", &[&n]),
        };
        if ui.button(off).clicked() {
            actions.moves = Some(people.iter().map(|p| (*p, Spot::Unseated)).collect());
        }
        self.selection_items(ui, id, actions);
    }

    /// Selection, details and roster changes for a person's card.
    fn selection_items(&self, ui: &mut egui::Ui, id: usize, actions: &mut CardActions) {
        let pick = if self.chart.multi.contains(&id) {
            tr("Remove from selection")
        } else {
            tr("Add to selection")
        };
        if ui
            .button(pick)
            .on_hover_text(tr("Ctrl-click to select multiple people."))
            .clicked()
        {
            actions.pick = Some(id);
        }
        if ui.button(tr("Show details")).clicked() {
            actions.clicked = Some(Some(id));
        }
        ui.separator();
        if ui.button(tr("Replace with someone else…")).clicked() {
            actions.roster = Some((id, RosterEdit::Replace(id, Default::default())));
        }
        if ui.button(tr("Remove from event")).clicked() {
            actions.remove = Some(id);
        }
    }

    /// The selection when the person is in it, else just the person.
    fn acting_on(&self, person: usize) -> Vec<usize> {
        if self.chart.multi.contains(&person) {
            self.chart.multi.iter().copied().collect()
        } else {
            vec![person]
        }
    }
}
