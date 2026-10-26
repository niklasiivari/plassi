use crate::i18n::{fill, tr};
use std::collections::{BTreeMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod layout;
mod pins;
mod policy;
mod refit;
mod spots;
mod tables;
mod validate;

pub use layout::*;
pub use pins::Pin;
pub use policy::*;
pub use refit::RoomFit;
pub use spots::{Point, Spot};
pub use tables::*;

/// Seats on one side of a table, at most; shared by validation and editors.
pub(crate) const MAX_SIDE_SEATS: usize = 500;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Attendee {
    pub id: usize,
    pub name: String,
    pub organization: String,
    pub participant_group: String,
    pub tags: Vec<String>,
    #[serde(default)]
    pub food: String,
    #[serde(default)]
    pub drink: String,
    #[serde(default)]
    pub allergies: String,
    /// Override the attendee's total charge in euro cents.
    #[serde(default)]
    pub price_override_cents: Option<u32>,
    /// Other spreadsheet columns, by header.
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Layout {
    pub rows: Vec<RowSpec>,
    /// Movable landmarks shown on the room plan and chart.
    #[serde(default)]
    pub labels: Vec<RoomLabel>,
    /// Use explicit table origins instead of the row geometry.
    #[serde(default)]
    pub free_layout: bool,
    #[serde(default)]
    pub table_positions: BTreeMap<String, TablePosition>,
    /// The room's walls in the free layout, to see how the tables fit.
    #[serde(default)]
    pub room: Option<RoomOutline>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RoomLabel {
    pub id: u64,
    pub text: String,
    pub x: f32,
    pub y: f32,
}

/// Where a table stands: its centre, and how far it is turned.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TablePosition {
    pub x: f32,
    pub y: f32,
    /// Degrees clockwise.
    #[serde(default)]
    pub rotation: f32,
}

/// A rectangle around the tables, in the same units as table positions.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RoomOutline {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RowSpec {
    pub id: String,
    pub label: String,
    pub tables: Vec<TableSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TableSpec {
    pub id: String,
    pub label: String,
    /// Seats down the left side, numbered from the top of the table.
    pub left: usize,
    /// Seats down the right side, numbered from the top of the table.
    pub right: usize,
    #[serde(default)]
    pub shape: TableShape,
}

/// A long table has seats down two sides. A round table has the same seats
/// around it: the right side clockwise from the top, the left side on back up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TableShape {
    #[default]
    Long,
    Round,
}

#[derive(Clone, Debug)]
pub struct Seat {
    pub id: String,
    pub row_id: String,
    pub table_id: String,
    pub x: f32,
    pub y: f32,
    pub side: Side,
    pub index: usize,
    /// The direction away from the table, in radians clockwise from +x.
    pub out: f32,
    /// The seat's place around a round table.
    pub ring: Option<Ring>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ImportNote {
    pub person: usize,
    pub field: ImportField,
    pub text: String,
    #[serde(default)]
    pub reviewed: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum ImportField {
    Avec,
    Wish,
}

impl ImportField {
    /// The kind of wish a note in this column asks for.
    pub fn wish(self) -> WishKind {
        match self {
            ImportField::Avec => WishKind::Avec,
            ImportField::Wish => WishKind::Seating,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Project {
    pub version: u32,
    pub attendees: Vec<Attendee>,
    pub layout: Layout,
    pub rules: Vec<Rule>,
    pub notes: Vec<ImportNote>,
    #[serde(default)]
    pub candidates: Vec<Candidate>,
    /// The option (index into `candidates`) last shown, to reopen on.
    #[serde(default)]
    pub shown_option: usize,
    #[serde(default)]
    pub policy: SeatingPolicy,
    #[serde(default)]
    pub event: EventInfo,
    /// Categories that free-text organizations, tickets, food and drinks map to.
    #[serde(default)]
    pub catalog: crate::catalog::Catalog,
    /// The planner's own seating preferences, separate from attendees' wishes.
    #[serde(default)]
    pub preferences: Vec<crate::preferences::Preference>,
    /// What spreadsheet columns outside the import template were imported as,
    /// by normalised header, so the next import of the same sheet maps them.
    #[serde(default)]
    pub import_columns: BTreeMap<String, crate::import::Column>,
}

/// Printed on exported charts and lists.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(default)]
pub struct EventInfo {
    pub title: String,
    /// E.g. date and venue.
    pub subtitle: String,
    /// Language of the prints.
    pub print_language: PrintLanguage,
}

/// The language prints are made in.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrintLanguage {
    /// Whatever the app is showing when printing.
    #[default]
    SameAsApp,
    Finnish,
    English,
}

impl PrintLanguage {
    /// The language code to print in, resolving "same as the app".
    pub fn code(self) -> &'static str {
        match self {
            Self::SameAsApp => crate::i18n::current().code(),
            Self::Finnish => "fi",
            Self::English => "en",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Rule {
    pub id: u64,
    pub label: String,
    pub source: String,
    pub enabled: bool,
    pub kind: RuleKind,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum RuleKind {
    Placement {
        person: usize,
        area: Area,
        #[serde(default = "yes")]
        must_be: bool,
        hard: bool,
        penalty: i64,
    },
    /// Keep away from the ends of a table side: a larger penalty for an end
    /// seat (nobody beside on one side), a smaller one for the seat next to it.
    /// Ends follow who actually sits where, so spare seats move them.
    AvoidTableEnd {
        person: usize,
        end_penalty: i64,
        near_end_penalty: i64,
    },
    /// A wish to sit near someone. Its distance costs and weight follow
    /// from `wish` and `importance`, so every wish of a kind scores alike.
    Proximity {
        requester: usize,
        target: Target,
        wish: WishKind,
        importance: Importance,
        max_class: Option<DistanceClass>,
    },
}

fn yes() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum WishKind {
    /// A partner: beside is best, opposite fine, diagonal unwanted. Keeps its
    /// full weight however many wishes the person has.
    Avec,
    /// Beside, opposite and diagonal are all good. A person's seating
    /// wishes are damped by how many they have (see `Project::wish_costs`).
    Seating,
}

impl WishKind {
    pub fn costs(self) -> ProximityCosts {
        match self {
            WishKind::Avec => ProximityCosts::AVEC,
            WishKind::Seating => ProximityCosts::WISH,
        }
    }

    pub fn weight(self) -> i64 {
        match self {
            WishKind::Avec => 100,
            WishKind::Seating => 10,
        }
    }

    /// What a normal wish of this kind costs at another table, for explaining scale.
    pub fn far_penalty(self) -> i64 {
        self.costs()[DistanceClass::Far] * self.weight()
    }

    /// The rule's label when it is made.
    pub fn label(self) -> &'static str {
        match self {
            WishKind::Avec => "Avec",
            WishKind::Seating => "Seating wish",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum Importance {
    Normal,
    High,
}

impl Importance {
    pub const ALL: [Importance; 2] = [Importance::Normal, Importance::High];

    pub fn factor(self) -> i64 {
        match self {
            Importance::Normal => 1,
            Importance::High => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Importance::Normal => tr("Normal"),
            Importance::High => tr("High"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum Target {
    /// Every named person is part of the wish.
    People {
        ids: Vec<usize>,
    },
    Tag {
        tag: String,
        required: usize,
    },
    /// Everyone whose organization matches (case-insensitive).
    /// Everyone in the organization counts, so there is no count to meet.
    Organization {
        name: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum Area {
    Seat(String),
    Table(String),
    Row(String),
    RowCenter(String),
}

/// Penalty per distance class, closest first (see `DistanceClass::ALL`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProximityCosts(pub [i64; CLASSES]);

impl ProximityCosts {
    // Only beside, opposite and diagonal are together; a few seats down the
    // same table is nearly as bad as another table.
    pub const AVEC: Self = Self([0, 3, 18, 32, 39, 40]);
    pub const WISH: Self = Self([0, 0, 2, 25, 38, 40]);
    /// A break between two pieces of a group, by how far apart they are.
    pub const GROUP: Self = Self([0, 0, 0, 25, 38, 40]);
    /// A group's cost per row it is longer than needed, before strength.
    pub const GROUP_STRETCH: i64 = 10;

    /// Costs times `weight`.
    pub fn scaled(self, weight: i64) -> [i64; CLASSES] {
        self.0.map(|c| c.saturating_mul(weight))
    }

    /// Costs times `weight / share`, rounded once at the end.
    pub fn shared(self, weight: i64, share: i64) -> [i64; CLASSES] {
        self.0.map(|c| (c * weight + share / 2) / share)
    }
}

impl std::ops::Index<DistanceClass> for ProximityCosts {
    type Output = i64;
    fn index(&self, class: DistanceClass) -> &i64 {
        &self.0[class as usize]
    }
}

impl std::ops::IndexMut<DistanceClass> for ProximityCosts {
    fn index_mut(&mut self, class: DistanceClass) -> &mut i64 {
        &mut self.0[class as usize]
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum DistanceClass {
    Beside,
    Opposite,
    Diagonal,
    /// Same table, at most `NEAR_ROWS` rows apart.
    Near,
    SameTable,
    Far,
}

/// Number of distance classes.
pub const CLASSES: usize = 6;
pub(crate) const MAX_DISTANCE_COST: i64 = 100_000;
/// Rows apart that still count as `Near` on a long table.
pub const NEAR_ROWS: usize = 2;

impl DistanceClass {
    /// Closest first; `class as usize` indexes this.
    pub const ALL: [DistanceClass; CLASSES] = [
        DistanceClass::Beside,
        DistanceClass::Opposite,
        DistanceClass::Diagonal,
        DistanceClass::Near,
        DistanceClass::SameTable,
        DistanceClass::Far,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DistanceClass::Beside => tr("beside"),
            DistanceClass::Opposite => tr("opposite"),
            DistanceClass::Diagonal => tr("diagonal"),
            DistanceClass::Near => tr("near (same table)"),
            DistanceClass::SameTable => tr("same table"),
            DistanceClass::Far => tr("another table"),
        }
    }

    /// As an upper limit: "opposite or closer".
    pub fn limit_label(self) -> &'static str {
        match self {
            DistanceClass::Beside => tr("beside"),
            DistanceClass::Opposite => tr("opposite or closer"),
            DistanceClass::Diagonal => tr("diagonal or closer"),
            DistanceClass::Near => tr("within two rows"),
            DistanceClass::SameTable => tr("same table"),
            DistanceClass::Far => tr("anywhere"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Candidate {
    pub name: String,
    pub assignment: BTreeMap<usize, String>,
    pub solver_status: SolverStatus,
    /// People placed by hand in this option; "Improve around my edits" keeps them.
    #[serde(default)]
    pub hand_placed: std::collections::BTreeSet<usize>,
    /// People taken off the seats and left on the chart, by where their card is.
    #[serde(default)]
    pub loose: BTreeMap<usize, Point>,
}

/// How an option came about, stored as data and put into words in the
/// current language when shown.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SolverStatus {
    /// No seating can score better.
    Optimal,
    /// The best seating found within the time.
    BestFound {
        seconds: u64,
        runs_at_best: usize,
        runs: usize,
    },
    /// Improved from another option, keeping people placed by hand.
    Improved {
        from: String,
        hand_placed: usize,
    },
    /// A longer search continued from another option.
    Continued {
        from: String,
        seconds: u64,
    },
    EditedByHand,
    /// Made empty, to be seated by hand.
    StartedEmpty,
    /// People lost their seats when the room changed.
    RoomChanged,
}

impl SolverStatus {
    /// The status as a sentence in the current language.
    pub fn describe(&self) -> String {
        match self {
            Self::Optimal => tr("Optimal solution").into(),
            Self::BestFound {
                seconds,
                runs_at_best,
                runs,
            } => {
                if *runs == 1 {
                    fill("Searched {} s", &[seconds])
                } else {
                    fill(
                        "Searched {} s · best in {}/{} runs",
                        &[seconds, runs_at_best, runs],
                    )
                }
            }
            Self::Improved { from, hand_placed } => {
                fill("Improved {} · {} manual seats kept", &[from, hand_placed])
            }
            Self::Continued { from, seconds } => fill("Searched {} s more · {}", &[seconds, from]),
            Self::EditedByHand => tr("Edited by hand").into(),
            Self::StartedEmpty => tr("Started empty").into(),
            Self::RoomChanged => tr("Seats removed with a room change").into(),
        }
    }
}

impl Project {
    pub fn new() -> Self {
        Self {
            version: 1,
            attendees: Vec::new(),
            layout: Layout::default(),
            rules: Vec::new(),
            notes: Vec::new(),
            candidates: Vec::new(),
            shown_option: 0,
            policy: SeatingPolicy::default(),
            event: EventInfo::default(),
            catalog: Default::default(),
            preferences: Vec::new(),
            import_columns: BTreeMap::new(),
        }
    }

    /// A seat's table label, side and row (from 1), from its ID.
    pub fn seat_place<'a>(&'a self, seat_id: &'a str) -> Option<(&'a str, Side, usize)> {
        let mut parts = seat_id.split(':');
        let (table, side, index) = (
            parts.next()?,
            parts.next()?,
            parts.next()?.parse::<usize>().ok()?,
        );
        let side = match side {
            "left" => Side::Left,
            "right" => Side::Right,
            _ => return None,
        };
        Some((self.table_label(table), side, index + 1))
    }

    /// Human-readable seat name, e.g. "Table 2 · left 3".
    pub fn seat_label(&self, seat_id: &str) -> String {
        let place = self.seat_place(seat_id);
        let round = self
            .layout
            .table(seat_table(seat_id))
            .filter(|t| t.shape == TableShape::Round);
        if let (Some(spec), Some((table, side, row))) = (round, place) {
            let seat = spec.ring_position(side, row - 1) + 1;
            return fill("{} · seat {}", &[&table, &seat]);
        }
        match place {
            Some((table, Side::Left, row)) => fill("{} · left {}", &[&table, &row]),
            Some((table, Side::Right, row)) => fill("{} · right {}", &[&table, &row]),
            None => seat_id.to_string(),
        }
    }

    /// A table's label, or its ID if it is not in the room.
    pub fn table_label<'a>(&'a self, table_id: &'a str) -> &'a str {
        self.layout
            .rows
            .iter()
            .flat_map(|r| &r.tables)
            .find(|t| t.id == table_id)
            .map_or(table_id, |t| t.label.as_str())
    }

    pub fn next_rule_id(&self) -> u64 {
        self.rules.iter().map(|r| r.id).max().unwrap_or(0) + 1
    }

    pub fn person(&self, id: usize) -> Option<&Attendee> {
        self.attendees.iter().find(|p| p.id == id)
    }

    /// The person's name, or "?" for an unknown ID.
    pub fn person_name(&self, id: usize) -> &str {
        self.person(id).map_or("?", |a| a.name.as_str())
    }

    pub fn target_ids(&self, requester: usize, target: &Target) -> Vec<usize> {
        match target {
            Target::People { ids, .. } => {
                ids.iter().copied().filter(|id| *id != requester).collect()
            }
            Target::Tag { tag, .. } => self
                .attendees
                .iter()
                .filter(|p| p.id != requester && p.tags.iter().any(|t| t == tag))
                .map(|p| p.id)
                .collect(),
            Target::Organization { name, .. } => {
                let kind = crate::catalog::Kind::Organization;
                let key = self.category_key_of(kind, name);
                // Index spellings once instead of scanning every category and
                // alias again for each attendee in this organization wish.
                let lookup = self.catalog.lookup(kind);
                let categories: HashSet<_> = self
                    .catalog
                    .list(kind)
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| format!("#{}", crate::catalog::normalize(&c.name)) == key)
                    .map(|(i, _)| i)
                    .collect();
                self.attendees
                    .iter()
                    .filter(|p| {
                        if p.id == requester {
                            return false;
                        }
                        let found = lookup.find_all(&p.organization);
                        if found.is_empty() {
                            crate::catalog::normalize(&p.organization) == key && !key.is_empty()
                        } else {
                            found.iter().any(|i| categories.contains(i))
                        }
                    })
                    .map(|p| p.id)
                    .collect()
            }
        }
    }

    /// A wish's distance costs: its kind's, times its weight and importance.
    /// A seating wish is divided by the square root of how many seating
    /// wishes its requester has: four wishes count half each, so a long
    /// list is damped without making each wish cheap to give up.
    #[cfg(test)]
    pub fn wish_costs(&self, rule: &Rule) -> Option<[i64; CLASSES]> {
        let RuleKind::Proximity {
            requester, wish, ..
        } = &rule.kind
        else {
            return None;
        };
        let wishes = match wish {
            WishKind::Avec => 1,
            WishKind::Seating => self
                .rules
                .iter()
                .filter(|other| {
                    other.enabled
                        && matches!(other.kind, RuleKind::Proximity {
                            requester: p,
                            wish: WishKind::Seating,
                            ..
                        } if p == *requester)
                })
                .count()
                .max(1),
        };
        self.wish_costs_with_count(rule, wishes)
    }

    /// As `wish_costs`, with the requester's enabled seating wishes counted once
    /// by a caller compiling many rules.
    pub(crate) fn wish_costs_with_count(
        &self,
        rule: &Rule,
        seating_wishes: usize,
    ) -> Option<[i64; CLASSES]> {
        let RuleKind::Proximity {
            wish,
            importance,
            ref target,
            ..
        } = rule.kind
        else {
            return None;
        };
        let wishes = if wish == WishKind::Avec {
            1
        } else {
            seating_wishes.max(1)
        };
        // In thousandths, so the square root stays exact enough.
        let share = ((wishes as f64).sqrt() * 1000.0).round() as i64;
        Some(wish.costs().shared(
            wish.weight() * importance.factor() * self.policy.wish_priority.factor(target) * 1000,
            share,
        ))
    }

    pub fn active_people(&self) -> HashSet<usize> {
        let mut active = HashSet::new();
        for rule in self.rules.iter().filter(|rule| rule.enabled) {
            match &rule.kind {
                RuleKind::Placement { person, .. } | RuleKind::AvoidTableEnd { person, .. } => {
                    active.insert(*person);
                }
                RuleKind::Proximity {
                    requester, target, ..
                } => {
                    active.insert(*requester);
                    active.extend(self.target_ids(*requester, target));
                }
            }
        }
        for preference in self.active_preferences() {
            active.extend(preference.kind.people().iter().copied());
        }
        active
    }
}

impl Rule {
    /// The person this rule belongs to.
    pub fn owner(&self) -> usize {
        match &self.kind {
            RuleKind::Placement { person, .. } | RuleKind::AvoidTableEnd { person, .. } => *person,
            RuleKind::Proximity { requester, .. } => *requester,
        }
    }
}

impl Target {
    /// How many targets must be near; an organization means all of them,
    /// capped by the caller at how many there are.
    pub fn required(&self) -> usize {
        match self {
            Target::People { ids, .. } => ids.len(),
            Target::Tag { required, .. } => *required,
            Target::Organization { .. } => usize::MAX,
        }
    }
}
