//! The score of a finished chart, part by part, for display. It reads the
//! same compiled terms the search optimises, so the two always agree.

use std::collections::{BTreeMap, HashSet};

use crate::model::{DistanceClass, Project};

pub use super::ends::EndResult;
use super::problem::{NONE, PolicyReport, Problem, Source};

/// How one wish or preference pair fares.
pub struct TermResult {
    pub source: Source,
    /// Distance to the target that decides the cost.
    pub class: DistanceClass,
    /// With one required target: the nearest one (attendee ID).
    pub nearest: Option<usize>,
    pub seated_targets: usize,
    pub required: usize,
    pub penalty: i64,
    pub hard_violation: bool,
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

    pub fn breakdown(&self, assignment: &BTreeMap<usize, String>) -> Breakdown {
        let pr = &self.0;
        let (seat_of, occupant) = pr.state_of(assignment);
        let mut scratch = Vec::new();
        let terms = (0..pr.terms.len())
            .map(|t| {
                let term = &pr.terms[t];
                let (class, penalty) = pr.term_outcome(t, &seat_of, &mut scratch);
                let nearest = (term.required == 1)
                    .then(|| term.targets.iter().min_by_key(|&&x| pr.distance(&seat_of, term.requester, x)))
                    .flatten()
                    .map(|&x| pr.people[x]);
                TermResult {
                    source: term.source,
                    class,
                    nearest,
                    seated_targets: term.targets.iter().filter(|&&x| seat_of[x] != NONE).count(),
                    required: term.required,
                    penalty,
                    hard_violation: term.forbidden[class as usize],
                }
            })
            .collect();
        let placements = pr
            .placement_rules
            .iter()
            .map(|rule| {
                let seat = seat_of[rule.person];
                let cost = if seat == NONE { Some(0) } else { rule.cost[seat] };
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
