//! Moving people by hand: one person or a block of them at once, onto seats,
//! loose on the chart, or off the chart into the "Not seated" list.
//!
//! People sitting where the movers land take the places the movers left, so
//! dropping a block on another block swaps the two, and shifting a block by
//! a row moves the person it lands on to the row it left.

use std::collections::{HashMap, HashSet};

use crate::i18n::{fill, tr};
use crate::model::{Candidate, Pin, Project, SolverStatus, Spot, seat_table};

/// A checked hand edit, ready to preview or apply.
#[derive(Clone, Debug, Default)]
pub struct Rearrangement {
    /// Everyone whose place changes, and where they go.
    pub moves: Vec<(usize, Spot)>,
    /// How many of them made room for the people moved on purpose.
    pub displaced: usize,
    /// Pins that follow their person: the new pin, or `None` to unpin.
    pub repins: Vec<(usize, Option<Pin>)>,
}

impl Rearrangement {
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    /// The option after the edit.
    pub fn result(&self, candidate: &Candidate) -> Candidate {
        let mut next = candidate.clone();
        for (person, spot) in &self.moves {
            if matches!(spot, Spot::Seat(_)) {
                next.hand_placed.insert(*person);
            } else {
                next.hand_placed.remove(person);
            }
            next.place(*person, spot.clone());
        }
        next.solver_status = SolverStatus::EditedByHand;
        next
    }

    /// Applies the edit to option `index` and moves the pins along. Returns
    /// the people whose seat pin a moved pin took over.
    pub fn apply(&self, project: &mut Project, index: usize) -> Vec<usize> {
        let Some(candidate) = project.candidates.get(index) else {
            return Vec::new();
        };
        project.candidates[index] = self.result(candidate);
        let mut unpinned = Vec::new();
        for (person, pin) in &self.repins {
            match pin {
                Some(pin) => unpinned.extend(project.pin(*person, pin.clone())),
                None => project.unpin(*person),
            }
        }
        // Someone in the edit may have been re-pinned after losing a pin.
        let pins = project.pins();
        unpinned.retain(|p| !pins.contains_key(p));
        unpinned
    }
}

/// Checks moving each person to their spot and works out who makes room.
/// Fails with a reason when the edit would break a pin or stack two people.
pub fn plan(
    project: &Project,
    candidate: &Candidate,
    wanted: &[(usize, Spot)],
) -> Result<Rearrangement, String> {
    let movers: Vec<(usize, Spot)> = wanted
        .iter()
        .filter(|(person, spot)| candidate.spot_of(*person) != *spot)
        .cloned()
        .collect();
    // Two people aimed at one seat clash even when one of them stays put.
    landing_seats(wanted)?;
    let landing = landing_seats(&movers)?;
    let pins = project.pins();
    let moving: HashSet<usize> = movers.iter().map(|(p, _)| *p).collect();
    let occupant: HashMap<&str, usize> = candidate
        .assignment
        .iter()
        .map(|(p, s)| (s.as_str(), *p))
        .collect();
    let mut moves = movers.clone();
    let mut displaced = 0;
    for (seat, mover) in &landing {
        let Some(&occupant) = occupant.get(seat.as_str()) else {
            continue;
        };
        if moving.contains(&occupant) {
            continue;
        }
        let spot = vacated(candidate, &landing, *mover);
        check_pin(project, pins.get(&occupant), occupant, &spot)?;
        moves.push((occupant, spot));
        displaced += 1;
    }
    let repins = movers
        .iter()
        .filter_map(|(person, spot)| Some((*person, follow(pins.get(person)?, spot)?)))
        .collect();
    Ok(Rearrangement {
        moves,
        displaced,
        repins,
    })
}

/// Each seat someone moves onto, and who; fails if two land on one seat.
fn landing_seats(movers: &[(usize, Spot)]) -> Result<HashMap<String, usize>, String> {
    let mut landing = HashMap::new();
    for (person, spot) in movers {
        if let Spot::Seat(seat) = spot
            && landing.insert(seat.clone(), *person).is_some()
        {
            return Err(tr("Two people would land on one seat").into());
        }
    }
    Ok(landing)
}

/// Where someone displaced by `mover` goes: the place the mover left, or,
/// when another mover lands there too, the place that one left, and so on.
fn vacated(candidate: &Candidate, landing: &HashMap<String, usize>, mover: usize) -> Spot {
    let mut spot = candidate.spot_of(mover);
    while let Spot::Seat(seat) = &spot
        && let Some(&next) = landing.get(seat)
    {
        spot = candidate.spot_of(next);
    }
    spot
}

/// Whether a displaced person's pin allows them to go to `spot`.
fn check_pin(
    project: &Project,
    pin: Option<&Pin>,
    person: usize,
    spot: &Spot,
) -> Result<(), String> {
    let name = project.person_name(person);
    match (pin, spot) {
        (Some(Pin::Seat(_)), _) => Err(fill("{} is pinned here", &[&name])),
        (Some(Pin::Table(table)), Spot::Seat(seat)) if seat_table(seat) == table => Ok(()),
        (Some(Pin::Table(table)), _) => Err(fill(
            "{} is kept at {}",
            &[&name, &project.table_label(table)],
        )),
        (None, _) => Ok(()),
    }
}

/// How a mover's pin follows them: `Some(new pin)`, `Some(None)` to unpin,
/// or `None` when it stays as it is.
fn follow(pin: &Pin, spot: &Spot) -> Option<Option<Pin>> {
    match (pin, spot) {
        (Pin::Seat(_), Spot::Seat(seat)) => Some(Some(Pin::Seat(seat.clone()))),
        (Pin::Seat(_), _) => Some(None),
        (Pin::Table(table), Spot::Seat(seat)) => {
            let to = seat_table(seat);
            (to != table).then(|| Some(Pin::Table(to.into())))
        }
        // Off the seats, they still belong at their table.
        (Pin::Table(_), _) => None,
    }
}

mod free_seats;
pub use free_seats::free_seats;

#[cfg(test)]
mod tests;
