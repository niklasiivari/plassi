//! Arrow keys: move people one seat along their table side, or across it.

use super::*;

/// Which way an arrow key moves people.
#[derive(Clone, Copy)]
enum Step {
    Up,
    Down,
    To(Side),
}

impl PlassiApp {
    /// Moves the picked people, or the selected person, one seat with the
    /// arrow keys; whoever sits there takes the seat they left.
    pub(super) fn nudge_keys(&mut self, ui: &egui::Ui) {
        let key = |k| ui.input(|i| i.key_pressed(k) && i.modifiers.is_none());
        let step = match () {
            _ if key(egui::Key::ArrowUp) => Step::Up,
            _ if key(egui::Key::ArrowDown) => Step::Down,
            _ if key(egui::Key::ArrowLeft) => Step::To(Side::Left),
            _ if key(egui::Key::ArrowRight) => Step::To(Side::Right),
            _ => return,
        };
        let people: Vec<usize> = match self.selected_person {
            _ if !self.chart.multi.is_empty() => self.chart.multi.iter().copied().collect(),
            Some(person) => vec![person],
            None => return,
        };
        let seats = self.project.layout.seats();
        let mut wanted = Vec::new();
        for person in people {
            let Some(seat) = self.seat_in_option(person) else {
                continue;
            };
            let Some(next) = next_seat(&seats, &seat, step) else {
                self.status = tr("No seat there").into();
                return;
            };
            wanted.push((person, Spot::Seat(next)));
        }
        if !wanted.is_empty() {
            self.rearrange(&wanted);
        }
    }
}

/// The seat one step from `seat` at the same table.
fn next_seat(seats: &[Seat], seat: &str, step: Step) -> Option<String> {
    let from = seats.iter().find(|s| s.id == seat)?;
    let (side, index) = match step {
        Step::Up => (from.side, from.index.checked_sub(1)?),
        Step::Down => (from.side, from.index + 1),
        Step::To(side) if side != from.side => (side, from.index),
        Step::To(_) => return None,
    };
    seats
        .iter()
        .find(|s| s.table_id == from.table_id && s.side == side && s.index == index)
        .map(|s| s.id.clone())
}
