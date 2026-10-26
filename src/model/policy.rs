//! The seating policy: rules for the whole event, as plain choices. Each
//! choice stands for a weight on the same scale as rule penalties (see
//! [`super::WishKind::far_penalty`]); the solver reads the weights.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Global rules for everyone, mainly shaping where unconstrained attendees sit.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(default)]
pub struct SeatingPolicy {
    /// Relative importance of named-person and organization wishes.
    pub wish_priority: WishPriority,
    /// Whether unconstrained attendees should sit with their own organization.
    pub organization: OrganizationMixing,
    /// Whether people without wishes sit among wish groups or apart from them.
    pub without_wishes: WithoutWishes,
    /// How much it matters that nobody sits with nobody beside or opposite.
    pub sitting_alone: SittingAlone,
    /// How much it matters that everyone with wishes has one of them nearby.
    pub one_wish: OneWish,
    /// Whether table ends are kept for groups.
    pub table_ends: TableEnds,
    pub empty_seats: EmptySeatPlacement,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum WishPriority {
    #[default]
    Balanced,
    People,
    Organizations,
}

impl WishPriority {
    pub(crate) fn factor(self, target: &super::Target) -> i64 {
        match (self, target) {
            (Self::People, super::Target::People { .. })
            | (Self::Organizations, super::Target::Organization { .. }) => 3,
            _ => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum OrganizationMixing {
    SameOrganization,
    Mixed,
    #[default]
    Indifferent,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum WithoutWishes {
    #[default]
    AmongGroups,
    ApartFromGroups,
    Indifferent,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum SittingAlone {
    Never,
    #[default]
    Avoid,
    Indifferent,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum OneWish {
    Important,
    #[default]
    Preferred,
    Indifferent,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum TableEnds {
    #[default]
    GroupsOnly,
    Indifferent,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum EmptySeatPlacement {
    /// Leave spare seats at the bottom of tables, as opposite pairs where possible.
    #[default]
    TableBottoms,
    /// Any seat may stay empty.
    Anywhere,
}

impl SeatingPolicy {
    /// Cost per beside/opposite pair that goes against `organization`.
    pub fn organization_weight(&self) -> i64 {
        match self.organization {
            OrganizationMixing::Indifferent => 0,
            _ => 15,
        }
    }

    /// Positive: spread unconstrained attendees among wish groups (cost per
    /// pair of adjacent unconstrained attendees). Negative: keep them apart
    /// from wish groups (cost per unconstrained attendee adjacent to a
    /// constrained one).
    pub fn sprinkle(&self) -> i64 {
        match self.without_wishes {
            WithoutWishes::AmongGroups => 10,
            WithoutWishes::ApartFromGroups => -10,
            WithoutWishes::Indifferent => 0,
        }
    }

    /// Cost for anyone with nobody beside or opposite them.
    pub fn isolation_penalty(&self) -> i64 {
        match self.sitting_alone {
            SittingAlone::Never => 1_500,
            SittingAlone::Avoid => 400,
            SittingAlone::Indifferent => 0,
        }
    }

    /// Extra cost for anyone with wishes of whom none is beside, opposite or
    /// diagonal to them, so no one loses all their wishes to save a little
    /// elsewhere.
    pub fn left_out_penalty(&self) -> i64 {
        match self.one_wish {
            OneWish::Important => 1_000,
            OneWish::Preferred => 300,
            OneWish::Indifferent => 0,
        }
    }

    /// Cost for someone who neither wishes for anyone nor is wished for (nor
    /// is in a group) sitting at a table end, where they have the fewest
    /// people to talk to; groups take the ends instead.
    pub fn lone_end_penalty(&self) -> i64 {
        match self.table_ends {
            TableEnds::GroupsOnly => 100,
            TableEnds::Indifferent => 0,
        }
    }

    /// Every choice that adds no cost, for tests of wishes alone.
    #[cfg(test)]
    pub fn none() -> Self {
        Self {
            wish_priority: WishPriority::Balanced,
            organization: OrganizationMixing::Indifferent,
            without_wishes: WithoutWishes::Indifferent,
            sitting_alone: SittingAlone::Indifferent,
            one_wish: OneWish::Indifferent,
            table_ends: TableEnds::Indifferent,
            empty_seats: EmptySeatPlacement::TableBottoms,
        }
    }
}
