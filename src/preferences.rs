//! The planner's own seating preferences ("these three together", "keep these
//! two apart"), kept separate from attendees' self-described wishes.
//!
//! They compile into the same scoring terms as wishes, so the solver treats
//! them alike, but they are stored, listed and edited on their own.

use crate::i18n::{fill, tr};
use std::collections::HashSet;

use anyhow::{Result, bail};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::{DistanceClass, Project, ProximityCosts, WishKind};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Preference {
    pub id: u64,
    pub kind: PreferenceKind,
    pub strength: Strength,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
pub enum PreferenceKind {
    /// These people sit as one compact block: everyone beside, opposite or
    /// diagonal to someone else in it, over as few rows as they need.
    Together { people: Vec<usize> },
    /// These people don't sit within `closeness` of each other.
    Apart {
        people: Vec<usize>,
        closeness: Closeness,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum Closeness {
    /// Nobody beside or opposite another.
    Neighbours,
    /// No two at the same table.
    Table,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum Strength {
    NiceToHave,
    Important,
    /// Never broken by the solver.
    Must,
}

impl Strength {
    pub const ALL: [Strength; 3] = [Strength::NiceToHave, Strength::Important, Strength::Must];

    pub fn label(self) -> &'static str {
        match self {
            Strength::NiceToHave => tr("Optional"),
            Strength::Important => tr("Important"),
            Strength::Must => tr("Required"),
        }
    }

    /// Weight on the same scale as wish importance.
    pub fn weight(self) -> i64 {
        match self {
            Strength::NiceToHave => WishKind::Seating.weight(),
            Strength::Important | Strength::Must => WishKind::Seating.weight() * 3,
        }
    }

    pub fn hard(self) -> bool {
        self == Strength::Must
    }
}

impl Closeness {
    /// The farthest distance that counts as too close.
    pub fn limit(self) -> DistanceClass {
        match self {
            Closeness::Neighbours => DistanceClass::Opposite,
            Closeness::Table => DistanceClass::SameTable,
        }
    }
}

impl PreferenceKind {
    pub fn people(&self) -> &[usize] {
        match self {
            PreferenceKind::Together { people, .. } | PreferenceKind::Apart { people, .. } => {
                people
            }
        }
    }

    pub fn together(people: Vec<usize>) -> Self {
        PreferenceKind::Together { people }
    }

    pub fn people_mut(&mut self) -> &mut Vec<usize> {
        match self {
            PreferenceKind::Together { people, .. } | PreferenceKind::Apart { people, .. } => {
                people
            }
        }
    }
}

/// What one pair of an "apart" preference costs at each distance: the
/// solver's term and the report's score both come from this.
pub struct PairTerm {
    pub a: usize,
    pub b: usize,
    /// Penalty per distance class, strength included.
    pub costs: [i64; crate::model::CLASSES],
    /// Distances that break a "must".
    pub forbidden: [bool; crate::model::CLASSES],
}

impl Preference {
    /// For "together": the cost of each break between pieces of the group by
    /// its width, the cost per extra row, and the breaks a "must" forbids.
    pub fn group_costs(&self) -> Option<GroupCosts> {
        let PreferenceKind::Together { .. } = self.kind else {
            return None;
        };
        let weight = self.strength.weight();
        let must = self.strength.hard();
        Some(GroupCosts {
            gap: ProximityCosts::GROUP.scaled(weight),
            stretch: ProximityCosts::GROUP_STRETCH * weight,
            forbidden: DistanceClass::ALL.map(|c| must && c > DistanceClass::Diagonal),
        })
    }

    /// For "apart": one term per unordered pair of members.
    pub fn pair_terms(&self) -> impl Iterator<Item = PairTerm> + '_ {
        let weight = self.strength.weight();
        let must = self.strength.hard();
        let PreferenceKind::Apart { people, closeness } = &self.kind else {
            return None.into_iter().flatten();
        };
        let close = DistanceClass::ALL.map(|c| c <= closeness.limit());
        let costs = close.map(|c| {
            if c {
                ProximityCosts::WISH[DistanceClass::Far] * weight
            } else {
                0
            }
        });
        let forbidden = close.map(|c| must && c);
        Some(people.iter().enumerate().flat_map(move |(i, &a)| {
            people[i + 1..].iter().map(move |&b| PairTerm {
                a,
                b,
                costs,
                forbidden,
            })
        }))
        .into_iter()
        .flatten()
    }
}

/// How a "together" preference is scored, strength included.
pub struct GroupCosts {
    pub gap: [i64; crate::model::CLASSES],
    pub stretch: i64,
    pub forbidden: [bool; crate::model::CLASSES],
}

impl Project {
    pub fn next_preference_id(&self) -> u64 {
        self.preferences.iter().map(|p| p.id).max().unwrap_or(0) + 1
    }

    /// Adds a preference for these people; returns its ID.
    pub fn add_preference(&mut self, kind: PreferenceKind, strength: Strength) -> u64 {
        let id = self.next_preference_id();
        self.preferences.push(Preference {
            id,
            kind,
            strength,
            enabled: true,
        });
        id
    }

    /// Names of these people, except `except`, joined with commas.
    pub fn names_of(&self, people: &[usize], except: Option<usize>) -> String {
        people
            .iter()
            .filter(|p| Some(**p) != except)
            .filter_map(|p| self.person(*p).map(|a| a.name.as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Preferences in force.
    pub fn active_preferences(&self) -> impl Iterator<Item = &Preference> {
        self.preferences.iter().filter(|p| p.enabled)
    }

    /// Preferences in force that include `person`.
    pub fn preferences_of(&self, person: usize) -> impl Iterator<Item = &Preference> {
        self.active_preferences()
            .filter(move |p| p.kind.people().contains(&person))
    }

    /// Takes a person out of every preference; drops preferences left with fewer than two.
    pub fn remove_from_preferences(&mut self, person: usize) {
        for preference in &mut self.preferences {
            preference.kind.people_mut().retain(|p| *p != person);
        }
        self.preferences.retain(|p| p.kind.people().len() >= 2);
    }

    pub(crate) fn validate_preferences(&self, people: &HashSet<usize>) -> Result<()> {
        let mut ids = HashSet::new();
        for preference in &self.preferences {
            if preference.id == u64::MAX {
                bail!(tr("Invalid preference ID"));
            }
            if !ids.insert(preference.id) {
                bail!(fill("Duplicate preference ID: {}", &[&preference.id]));
            }
            let members = preference.kind.people();
            let distinct: HashSet<_> = members.iter().collect();
            if members.len() < 2 || distinct.len() != members.len() {
                bail!(fill(
                    "Preference {} needs at least two different people",
                    &[&preference.id]
                ));
            }
            if members.iter().any(|p| !people.contains(p)) {
                bail!(fill("Preference {}: person is missing", &[&preference.id]));
            }
        }
        Ok(())
    }
}
