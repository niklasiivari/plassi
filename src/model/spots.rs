//! Where each person is in an option: on a seat, loose on the chart, or not
//! placed anywhere yet.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Candidate, Pin, Project, SolverStatus};

/// A position on the chart, in chart units.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// Where a person is in an option.
#[derive(Clone, Debug, PartialEq)]
pub enum Spot {
    Seat(String),
    /// Off the seats, with their card at this point on the chart.
    Loose(Point),
    /// Off the seats and off the chart: only in the "Not seated" list.
    Unseated,
}

impl Candidate {
    pub fn spot_of(&self, person: usize) -> Spot {
        if let Some(seat) = self.assignment.get(&person) {
            Spot::Seat(seat.clone())
        } else if let Some(point) = self.loose.get(&person) {
            Spot::Loose(*point)
        } else {
            Spot::Unseated
        }
    }

    /// Puts a person on a spot, leaving the one they had. Whoever else is on a
    /// target seat stays there; callers move them first.
    pub fn place(&mut self, person: usize, spot: Spot) {
        self.assignment.remove(&person);
        self.loose.remove(&person);
        match spot {
            Spot::Seat(seat) => {
                self.assignment.insert(person, seat);
            }
            Spot::Loose(point) => {
                self.loose.insert(person, point);
            }
            Spot::Unseated => {}
        }
    }

    /// Replaces the whole seating; people it seats are no longer loose.
    pub fn reseat(&mut self, assignment: BTreeMap<usize, String>) {
        self.loose
            .retain(|person, _| !assignment.contains_key(person));
        self.assignment = assignment;
    }

    /// Drops every trace of a person who left the event.
    pub fn forget(&mut self, person: usize) {
        self.place(person, Spot::Unseated);
        self.hand_placed.remove(&person);
    }

    /// People without a seat, in attendee order.
    pub fn unseated(&self, project: &Project) -> Vec<usize> {
        project
            .attendees
            .iter()
            .map(|a| a.id)
            .filter(|id| !self.assignment.contains_key(id))
            .collect()
    }
}

impl Project {
    /// "Option N", in the interface language, for the lowest N from the option count up that no option
    /// is called yet, so a deleted option's name is not repeated.
    pub fn new_option_name(&self) -> String {
        let names = (self.candidates.len() + 1..).map(|n| crate::i18n::fill("Option {}", &[&n]));
        self.free_option_name(names)
    }

    /// The first of `names` that no option is called yet.
    pub fn free_option_name(&self, mut names: impl Iterator<Item = String>) -> String {
        names
            .find(|name| self.candidates.iter().all(|c| c.name != *name))
            .expect("a free name")
    }

    /// A new option where only people pinned to a seat are seated.
    pub fn empty_option(&self, name: String) -> Candidate {
        let assignment = self
            .pins()
            .into_iter()
            .filter_map(|(person, pin)| match pin {
                Pin::Seat(seat) => Some((person, seat)),
                Pin::Table(_) => None,
            })
            .collect();
        Candidate {
            name,
            assignment,
            solver_status: SolverStatus::StartedEmpty,
            hand_placed: Default::default(),
            loose: Default::default(),
        }
    }
}
