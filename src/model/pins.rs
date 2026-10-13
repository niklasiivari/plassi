//! Pins: hard rules made in the app that hold a person on one seat or at one table.

use std::collections::{HashMap, HashSet};

use super::{Area, Project, Rule, RuleKind};

/// Where a pin holds a person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pin {
    /// On this seat; the solver never moves them.
    Seat(String),
    /// Somewhere at this table; the solver picks the seat.
    Table(String),
}

impl Pin {
    fn area(&self) -> Area {
        match self {
            Pin::Seat(seat) => Area::Seat(seat.clone()),
            Pin::Table(table) => Area::Table(table.clone()),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Pin::Seat(_) => "Pinned",
            Pin::Table(_) => "Kept at table",
        }
    }
}

impl Project {
    /// The person and pin of a rule: an enabled, hard "must sit on this seat
    /// or at this table" rule.
    pub fn pin_of(&self, rule: &Rule) -> Option<(usize, Pin)> {
        let RuleKind::Placement {
            person,
            area,
            must_be: true,
            hard: true,
            ..
        } = &rule.kind
        else {
            return None;
        };
        let pin = match area {
            Area::Seat(seat) => Pin::Seat(seat.clone()),
            Area::Table(table) => Pin::Table(table.clone()),
            _ => return None,
        };
        rule.enabled.then_some((*person, pin))
    }

    /// Every pinned person and their pin.
    pub fn pins(&self) -> HashMap<usize, Pin> {
        self.rules.iter().filter_map(|r| self.pin_of(r)).collect()
    }

    /// People held on one seat; the solver keeps them where they are.
    pub fn seat_pinned_people(&self) -> HashSet<usize> {
        self.pins()
            .into_iter()
            .filter(|(_, pin)| matches!(pin, Pin::Seat(_)))
            .map(|(person, _)| person)
            .collect()
    }

    /// Pins a person, replacing any previous pin for them.
    pub fn pin(&mut self, person: usize, pin: Pin) {
        self.unpin(person);
        let id = self.next_rule_id();
        self.rules.push(Rule {
            id,
            label: pin.label().into(),
            source: "Pinned in the app".into(),
            enabled: true,
            kind: RuleKind::Placement {
                person,
                area: pin.area(),
                must_be: true,
                hard: true,
                penalty: 0,
            },
        });
    }

    /// Removes every pin of a person.
    pub fn unpin(&mut self, person: usize) {
        let pins: Vec<u64> = self
            .rules
            .iter()
            .filter(|r| self.pin_of(r).is_some_and(|(p, _)| p == person))
            .map(|r| r.id)
            .collect();
        self.rules.retain(|r| !pins.contains(&r.id));
    }
}
