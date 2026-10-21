//! The score of a finished chart, part by part, for display. It reads the
//! same compiled terms the search optimises, so the two always agree.

use std::collections::{BTreeMap, HashSet};

use crate::model::{DistanceClass, Project};

pub use super::ends::EndResult;
use super::problem::{NONE, PolicyReport, Problem, Scratch, Shape, Source};

/// How one wish or preference pair fares.
pub struct TermResult {
    pub source: Source,
    /// Distance to the target that decides the cost.
    pub class: DistanceClass,
    /// With one required target, or for an organization: the nearest one (attendee ID).
    pub nearest: Option<usize>,
    pub seated_targets: usize,
    pub required: usize,
    pub penalty: i64,
    pub hard_violation: bool,
    /// For a group: how many pieces it is in and how many rows longer than
    /// needed it is.
    pub pieces: usize,
    pub extra_rows: usize,
}

pub struct PlacementResult {
    pub rule_id: u64,
    pub seated: bool,
    pub penalty: i64,
    pub hard_violation: bool,
}

pub struct Breakdown {
    pub terms: Vec<TermResult>,
    pub placements: Vec<PlacementResult>,
    pub ends: Vec<EndResult>,
    pub policy: PolicyReport,
    /// Attendee IDs of people none of whose wishes is near them, and the cost.
    pub left_out: Vec<usize>,
    pub left_out_penalty: i64,
}

/// A project compiled once, for scoring many charts.
pub struct Scorer(Problem);

impl Scorer {
    pub fn new(project: &Project) -> Self {
        Self(Problem::new(project, &HashSet::new(), None, None))
    }

    /// The solver's cost of a chart: broken hard rules weigh the project's
    /// score radix each, on top of the score. Far cheaper than a breakdown,
    /// for ranking many charts.
    pub fn cost(&self, assignment: &BTreeMap<usize, String>) -> i64 {
        let (seat_of, occupant) = self.0.state_of(assignment);
        self.0.cost(&seat_of, &occupant)
    }

    pub fn breakdown(&self, assignment: &BTreeMap<usize, String>) -> Breakdown {
        let pr = &self.0;
        let (seat_of, occupant) = pr.state_of(assignment);
        let mut scratch = Scratch::default();
        let terms = (0..pr.terms.len())
            .map(|t| {
                let term = &pr.terms[t];
                let (class, penalty) = pr.term_outcome(t, &seat_of, &mut scratch);
                let halving = term.shape == Shape::Halving;
                let block = matches!(term.shape, Shape::Block { .. })
                    .then(|| pr.block_shape(t, &seat_of, &mut scratch));
                let nearest = (term.required == 1 || halving)
                    .then(|| {
                        term.targets
                            .iter()
                            .min_by_key(|&&x| pr.distance(&seat_of, term.requester, x))
                    })
                    .flatten()
                    .map(|&x| pr.people[x]);
                TermResult {
                    source: term.source,
                    class,
                    nearest,
                    seated_targets: term.targets.iter().filter(|&&x| seat_of[x] != NONE).count(),
                    // An organization needs one member seated, not all.
                    required: if halving { 1 } else { term.required },
                    penalty,
                    hard_violation: term.forbidden[class as usize],
                    pieces: block.as_ref().map_or(1, |b| b.pieces),
                    extra_rows: block.as_ref().map_or(0, |b| b.extra_rows),
                }
            })
            .collect();
        let placements = pr
            .placement_rules
            .iter()
            .map(|rule| {
                let seat = seat_of[rule.person];
                let cost = if seat == NONE {
                    Some(0)
                } else {
                    rule.cost[seat]
                };
                PlacementResult {
                    rule_id: rule.rule_id,
                    seated: seat != NONE,
                    penalty: cost.unwrap_or(0),
                    hard_violation: cost.is_none(),
                }
            })
            .collect();
        let left_out: Vec<usize> = (0..pr.fairness.groups.len())
            .filter(|&g| pr.left_out_cost(g, &seat_of, &mut scratch) > 0)
            .map(|g| pr.people[pr.fairness.requester[g]])
            .collect();
        Breakdown {
            left_out_penalty: left_out.len() as i64 * pr.fairness.penalty,
            left_out,
            terms,
            placements,
            ends: pr.ends.results(&seat_of, &occupant),
            policy: pr.policy_report(&occupant),
        }
    }
}
