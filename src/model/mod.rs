use crate::i18n::{fill, tr};
use std::collections::{BTreeMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod layout;
mod validate;

pub use layout::*;

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
    /// Other spreadsheet columns, by header.
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Layout {
    pub rows: Vec<RowSpec>,
    #[serde(default)]
    pub seat_tags: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub relations: Vec<SeatRelation>,
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
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SeatRelation {
    pub a: String,
    pub b: String,
    pub class: DistanceClass,
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
    pub tags: Vec<String>,
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

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Project {
    pub version: u32,
    pub attendees: Vec<Attendee>,
    pub layout: Layout,
    pub rules: Vec<Rule>,
    pub notes: Vec<ImportNote>,
    #[serde(default)]
    pub candidates: Vec<Candidate>,
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

/// Global rules for everyone, mainly shaping where unconstrained attendees sit.
/// Weights use the same scale as rule penalties (see [`ProximityCosts::far_penalty`]).
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(default)]
pub struct SeatingPolicy {
    /// Whether unconstrained attendees should sit with their own organization.
    pub organization: OrganizationMixing,
    /// Cost per beside/opposite pair that goes against `organization`.
    pub organization_weight: i64,
    /// Positive: spread unconstrained attendees among wish groups (cost per pair of
    /// adjacent unconstrained attendees). Negative: keep them apart from wish groups
    /// (cost per unconstrained attendee adjacent to a constrained one).
    pub sprinkle: i64,
    /// Cost for anyone with nobody beside or opposite them.
    pub isolation_penalty: i64,
    /// Extra cost for anyone with wishes of whom none is near them, so no
    /// one loses all their wishes to save a little elsewhere.
    pub left_out_penalty: i64,
    pub empty_seats: EmptySeatPlacement,
}

impl Default for SeatingPolicy {
    fn default() -> Self {
        Self {
            organization: OrganizationMixing::Indifferent,
            organization_weight: 15,
            sprinkle: 10,
            isolation_penalty: 400,
            left_out_penalty: 300,
            empty_seats: EmptySeatPlacement::TableBottoms,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum OrganizationMixing {
    SameOrganization,
    Mixed,
    Indifferent,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum EmptySeatPlacement {
    /// Leave spare seats at the bottom of tables, as opposite pairs where possible.
    TableBottoms,
    /// Any seat may stay empty.
    Anywhere,
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
    Proximity {
        requester: usize,
        target: Target,
        costs: ProximityCosts,
        weight: i64,
        #[serde(default)]
        weight_mode: WeightMode,
        max_class: Option<DistanceClass>,
    },
}

fn yes() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum WeightMode {
    #[default]
    Fixed,
    ShareRequesterBudget,
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
    Organization {
        name: String,
        required: usize,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum Area {
    Seat(String),
    Table(String),
    Row(String),
    RowCenter(String),
    Tag(String),
}

/// Penalty per distance class, closest first (see `DistanceClass::ALL`).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProximityCosts(pub [i64; CLASSES]);

impl ProximityCosts {
    pub const AVEC: Self = Self([0, 3, 18, 22, 25, 40]);
    pub const WISH: Self = Self([0, 0, 2, 8, 15, 40]);
    /// Default importance of an avec and of an ordinary wish.
    pub const AVEC_WEIGHT: i64 = 100;
    pub const WISH_WEIGHT: i64 = 10;

    /// Penalty of a default wish or avec seated at another table, for explaining scale.
    pub fn far_penalty(avec: bool) -> i64 {
        if avec {
            Self::AVEC[DistanceClass::Far] * Self::AVEC_WEIGHT
        } else {
            Self::WISH[DistanceClass::Far] * Self::WISH_WEIGHT
        }
    }

    /// Costs times `weight`.
    pub fn scaled(self, weight: i64) -> [i64; CLASSES] {
        self.0.map(|c| c.saturating_mul(weight))
    }

    /// Costs times `weight / share`, rounded once at the end.
    pub fn shared(self, weight: i64, share: i64) -> [i64; CLASSES] {
        self.0.map(|c| (c * weight + share / 2) / share)
    }

    /// Avec-strength costs (opposite already costs something and far away is expensive).
    pub fn is_avec(&self) -> bool {
        self[DistanceClass::Far] >= Self::AVEC[DistanceClass::Far]
            && self[DistanceClass::Opposite] > 0
    }

    /// Whether a farther distance never costs less than a nearer one.
    pub fn is_ordered(self) -> bool {
        self.0.windows(2).all(|w| w[0] <= w[1])
    }

    /// Raises costs where needed so they never fall with distance.
    pub fn make_ordered(&mut self) {
        for i in 1..self.0.len() {
            self.0[i] = self.0[i].max(self.0[i - 1]);
        }
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
}

impl SolverStatus {
    /// The status as a sentence in the current language.
    pub fn describe(&self) -> String {
        match self {
            Self::Optimal => tr("Optimal — no seating can score better").into(),
            Self::BestFound {
                seconds,
                runs_at_best,
                runs,
            } => fill(
                "Best found in {} s · {} of {} runs reached this score",
                &[seconds, runs_at_best, runs],
            ),
            Self::Improved { from, hand_placed } => fill(
                "Improved from {}, keeping {} hand-placed people",
                &[from, hand_placed],
            ),
            Self::Continued { from, seconds } => {
                fill("Searched {} s more from {}", &[seconds, from])
            }
            Self::EditedByHand => tr("Edited by hand").into(),
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
        let label = self
            .layout
            .rows
            .iter()
            .flat_map(|r| &r.tables)
            .find(|t| t.id == table)
            .map_or(table, |t| t.label.as_str());
        Some((label, side, index + 1))
    }

    /// Human-readable seat name, e.g. "Table 2 · left 3".
    pub fn seat_label(&self, seat_id: &str) -> String {
        match self.seat_place(seat_id) {
            Some((table, Side::Left, row)) => fill("{} · left {}", &[&table, &row]),
            Some((table, Side::Right, row)) => fill("{} · right {}", &[&table, &row]),
            None => seat_id.to_string(),
        }
    }

    /// People held on one seat by an enabled hard seat rule.
    pub fn pinned_people(&self) -> HashSet<usize> {
        self.rules
            .iter()
            .filter_map(|r| self.pin_seat(r).map(|(person, _)| person))
            .collect()
    }

    /// The (person, seat) of a pin: an enabled, hard "must sit on this seat" rule.
    pub fn pin_seat<'a>(&self, rule: &'a Rule) -> Option<(usize, &'a str)> {
        match &rule.kind {
            RuleKind::Placement {
                person,
                area: Area::Seat(seat),
                must_be: true,
                hard: true,
                ..
            } if rule.enabled => Some((*person, seat.as_str())),
            _ => None,
        }
    }

    /// Pins a person to a seat, replacing any previous pin for them.
    pub fn pin(&mut self, person: usize, seat: &str) {
        self.unpin(person);
        let id = self.next_rule_id();
        self.rules.push(Rule {
            id,
            label: "Pinned".into(),
            source: "Pinned in the app".into(),
            enabled: true,
            kind: RuleKind::Placement {
                person,
                area: Area::Seat(seat.into()),
                must_be: true,
                hard: true,
                penalty: 0,
            },
        });
    }

    pub fn unpin(&mut self, person: usize) {
        let pinned: Vec<u64> = self
            .rules
            .iter()
            .filter(|r| self.pin_seat(r).is_some_and(|(p, _)| p == person))
            .map(|r| r.id)
            .collect();
        self.rules.retain(|r| !pinned.contains(&r.id));
    }

    pub fn next_rule_id(&self) -> u64 {
        self.rules.iter().map(|r| r.id).max().unwrap_or(0) + 1
    }

    pub fn person(&self, id: usize) -> Option<&Attendee> {
        self.attendees.iter().find(|p| p.id == id)
    }

    /// The person's name, or "?" for an unknown ID.
    /// Whether a rule is a wish to sit near someone.
    pub fn is_wish(&self, rule_id: u64) -> bool {
        self.rules
            .iter()
            .any(|r| r.id == rule_id && matches!(r.kind, RuleKind::Proximity { .. }))
    }

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
                self.attendees
                    .iter()
                    .filter(|p| p.id != requester && self.in_category(kind, p, &key))
                    .map(|p| p.id)
                    .collect()
            }
        }
    }

    pub fn effective_weight(&self, rule: &Rule) -> i64 {
        let (weight, share) = self.weight_share(rule);
        weight / share
    }

    /// A wish's importance and how many wishes share it: a person with a
    /// shared budget splits it over all their wishes.
    pub fn weight_share(&self, rule: &Rule) -> (i64, i64) {
        let RuleKind::Proximity {
            requester,
            weight,
            weight_mode,
            ..
        } = &rule.kind
        else {
            return (0, 1);
        };
        match weight_mode {
            WeightMode::Fixed => (*weight, 1),
            WeightMode::ShareRequesterBudget => {
                let count = self.rules.iter().filter(|other| other.enabled && matches!(other.kind,
                    RuleKind::Proximity { requester: p, weight_mode: WeightMode::ShareRequesterBudget, .. } if p == *requester)).count();
                (*weight, count.max(1) as i64)
            }
        }
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
    pub fn required(&self) -> usize {
        match self {
            Target::People { ids, .. } => ids.len(),
            Target::Tag { required, .. } | Target::Organization { required, .. } => *required,
        }
    }
}
