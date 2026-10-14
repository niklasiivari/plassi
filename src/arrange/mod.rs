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
    /// Everyone whose place changes, and where they go: the people moved on
    /// purpose first, then the people who made room for them.
    pub moves: Vec<(usize, Spot)>,
    /// How many of `moves`, at the end, made room for the others.
    pub displaced: usize,
    /// Pins that follow their person: the new pin, or `None` to unpin.
    pub repins: Vec<(usize, Option<Pin>)>,
}

impl Rearrangement {
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    /// The people moved on purpose.
    pub fn movers(&self) -> &[(usize, Spot)] {
        &self.moves[..self.moves.len() - self.displaced]
    }

    /// The option after the edit. Only people moved on purpose onto a seat
    /// count as placed by hand; people pushed aside are free for the search
    /// again, since nobody chose their new place.
    pub fn result(&self, candidate: &Candidate) -> Candidate {
        let mut next = candidate.clone();
        let movers = self.movers().len();
        for (i, (person, spot)) in self.moves.iter().enumerate() {
            if i < movers && matches!(spot, Spot::Seat(_)) {
                next.hand_placed.insert(*person);
            } else {
                next.hand_placed.remove(person);
            }
            next.place(*person, spot.clone());
        }
        next.solver_status = SolverStatus::EditedByHand;
        next
    }

    /// Applies the edit to option `index` and moves the pins along. A moved
    /// pin never takes over a seat pinned to someone else (`plan` refuses
    /// that), so nobody loses a pin they did not move.
    pub fn apply(&self, project: &mut Project, index: usize) {
        let Some(candidate) = project.candidates.get(index) else {
            return;
        };
        project.candidates[index] = self.result(candidate);
        for (person, pin) in &self.repins {
            match pin {
                Some(pin) => {
                    project.pin(*person, pin.clone());
                }
                None => project.unpin(*person),
            }
        }
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
    let repins: Vec<(usize, Option<Pin>)> = movers
        .iter()
        .filter_map(|(person, spot)| Some((*person, follow(pins.get(person)?, spot)?)))
        .collect();
    check_takeover(project, &pins, &moving, &repins)?;
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

/// Fails when a pin that follows its person would land on a seat pinned to
/// someone who is not moving: that person would silently lose their pin.
fn check_takeover(
    project: &Project,
    pins: &HashMap<usize, Pin>,
    moving: &HashSet<usize>,
    repins: &[(usize, Option<Pin>)],
) -> Result<(), String> {
    for (_, pin) in repins {
        let Some(pin @ Pin::Seat(_)) = pin else {
            continue;
        };
        if let Some((other, _)) = pins
            .iter()
            .find(|(p, held)| *held == pin && !moving.contains(p))
        {
            let name = project.person_name(*other);
            return Err(fill("That seat is pinned to {}", &[&name]));
        }
    }
    Ok(())
}

/// Whether a displaced person's pin allows them to go to `spot`. Someone
/// kept at a table may leave the seats, like people moved on purpose, but
/// not sit at another table.
fn check_pin(
    project: &Project,
    pin: Option<&Pin>,
    person: usize,
    spot: &Spot,
) -> Result<(), String> {
    let name = project.person_name(person);
    match (pin, spot) {
        (Some(Pin::Seat(_)), _) => Err(fill("{} is pinned here", &[&name])),
        (Some(Pin::Table(table)), Spot::Seat(seat)) if seat_table(seat) != table => Err(fill(
            "{} is kept at {}",
            &[&name, &project.table_label(table)],
        )),
        (Some(Pin::Table(_)), _) | (None, _) => Ok(()),
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
