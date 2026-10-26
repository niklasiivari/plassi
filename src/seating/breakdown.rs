//! The score of a finished chart, part by part, for display. It reads the
//! same compiled terms the search optimises, so the two always agree.

use std::collections::{BTreeMap, HashMap, HashSet};

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
pub struct Scorer {
    problem: Problem,
    preference_of_term: Vec<Option<usize>>,
    preferences: usize,
}

impl Scorer {
    /// Enabled wish rules with attendee IDs already resolved by the solver.
    /// Chart relationships reuse these without expanding organizations again.
    pub(crate) fn resolved_wishes(&self) -> impl Iterator<Item = (u64, usize, Vec<usize>)> + '_ {
        let problem = &self.problem;
        problem.terms.iter().filter_map(|term| {
            let Source::Rule(id) = term.source else {
                return None;
            };
            Some((
                id,
                problem.people[term.requester],
                term.targets.iter().map(|&i| problem.people[i]).collect(),
            ))
        })
    }

    pub fn new(project: &Project) -> Self {
        let problem = Problem::new(project, &HashSet::new(), None, None);
        let preferences: HashMap<_, _> = project
            .active_preferences()
            .enumerate()
            .map(|(index, preference)| (preference.id, index))
            .collect();
        let preference_of_term = problem
            .terms
            .iter()
            .map(|term| match term.source {
                Source::Preference(id) => preferences.get(&id).copied(),
                Source::Rule(_) => None,
            })
            .collect();
        Self {
            problem,
            preference_of_term,
            preferences: preferences.len(),
        }
    }

    /// The solver's cost of a chart: broken hard rules weigh the project's
    /// score radix each, on top of the score. Far cheaper than a breakdown,
    /// for ranking many charts.
    pub fn cost(&self, assignment: &BTreeMap<usize, String>) -> i64 {
        let (seat_of, occupant) = self.problem.state_of(assignment);
        self.problem.cost(&seat_of, &occupant)
    }

    /// The displayed (soft penalty, broken hard rules), without explanations.
    /// A required preference counts once even when several of its pairs fail;
    /// placement rules each keep their own soft and hard contribution.
    pub fn totals(&self, assignment: &BTreeMap<usize, String>) -> (i64, usize) {
        let pr = &self.problem;
        let (seat_of, occupant) = pr.state_of(assignment);
        let mut scratch = Scratch::default();
        let mut classes = Vec::with_capacity(pr.terms.len());
        let mut broken_preferences = vec![false; self.preferences];
        let mut penalty = 0;
        let mut hard_violations = 0;
        for (index, term) in pr.terms.iter().enumerate() {
            let (class, cost) = pr.term_outcome(index, &seat_of, &mut scratch);
            classes.push(class);
            penalty += cost;
            if term.forbidden[class as usize] {
                match self.preference_of_term[index] {
                    Some(preference) => broken_preferences[preference] = true,
                    None => hard_violations += 1,
                }
            }
        }
        for rule in &pr.placement_rules {
            let seat = seat_of[rule.person];
            if seat != NONE {
                match rule.cost[seat] {
                    Some(cost) => penalty += cost,
                    None => hard_violations += 1,
                }
            }
        }
        for group in &pr.fairness.groups {
            if !group
                .iter()
                .any(|&term| classes[term] <= DistanceClass::Diagonal)
            {
                penalty += pr.fairness.penalty;
            }
        }
        penalty += pr.policy_cost(&occupant) + pr.ends.results_total(&seat_of, &occupant);
        hard_violations += broken_preferences
            .into_iter()
            .filter(|broken| *broken)
            .count();
        (penalty, hard_violations)
    }

    pub fn breakdown(&self, assignment: &BTreeMap<usize, String>) -> Breakdown {
        let pr = &self.problem;
        let (seat_of, occupant) = pr.state_of(assignment);
        let mut scratch = Scratch::default();
        let terms: Vec<TermResult> = (0..pr.terms.len())
            .map(|t| {
                let term = &pr.terms[t];
                let block = matches!(term.shape, Shape::Block { .. })
                    .then(|| pr.block_shape(t, &seat_of, &mut scratch));
                let (class, penalty) = match (term.shape, block.as_ref()) {
                    (Shape::Block { stretch }, Some(block)) => (
                        block.widest,
                        block.gap_cost + block.extra_rows as i64 * stretch,
                    ),
                    _ => pr.term_outcome(t, &seat_of, &mut scratch),
                };
                let halving = term.shape == Shape::Halving;
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
            .filter(|&g| {
                !pr.fairness.groups[g]
                    .iter()
                    .any(|&term| terms[term].class <= DistanceClass::Diagonal)
            })
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

#[cfg(test)]
mod tests {
    use rand::seq::SliceRandom;
    use rand::{Rng, SeedableRng};

    use super::*;
    use crate::model::{Area, Attendee, Importance, Layout, Rule, RuleKind, Target, WishKind};
    use crate::preferences::{Closeness, PreferenceKind, Strength};

    fn project(people: usize) -> Project {
        let mut project = Project::new();
        project.layout = Layout::simple(3, people.div_ceil(3));
        project.attendees = (0..people)
            .map(|id| Attendee {
                id,
                name: format!("Person {id}"),
                organization: ["ABC", "XYZ"][id % 2].into(),
                participant_group: String::new(),
                tags: vec!["group".into()],
                food: String::new(),
                drink: String::new(),
                allergies: String::new(),
                price_override_cents: None,
                extra: BTreeMap::new(),
            })
            .collect();
        project
    }

    fn add_rule(project: &mut Project, kind: RuleKind) {
        project.rules.push(Rule {
            id: project.next_rule_id(),
            label: String::new(),
            source: String::new(),
            enabled: true,
            kind,
        });
    }

    #[test]
    fn numeric_totals_preserve_displayed_scores_for_partial_charts() {
        let mut project = project(18);
        for requester in 0..18 {
            let target = match requester % 3 {
                0 => Target::Organization { name: "ABC".into() },
                1 => Target::Tag {
                    tag: "group".into(),
                    required: 4,
                },
                _ => Target::People {
                    ids: vec![(requester + 1) % 18, (requester + 5) % 18],
                },
            };
            add_rule(
                &mut project,
                RuleKind::Proximity {
                    requester,
                    target,
                    wish: if requester % 4 == 0 {
                        WishKind::Avec
                    } else {
                        WishKind::Seating
                    },
                    importance: Importance::Normal,
                    max_class: (requester % 5 == 0).then_some(DistanceClass::Diagonal),
                },
            );
        }
        for hard in [true, false] {
            add_rule(
                &mut project,
                RuleKind::Placement {
                    person: 0,
                    area: Area::Table("table-2".into()),
                    must_be: true,
                    hard,
                    penalty: if hard { 0 } else { 73 },
                },
            );
        }
        add_rule(
            &mut project,
            RuleKind::AvoidTableEnd {
                person: 1,
                end_penalty: 300,
                near_end_penalty: 80,
            },
        );
        project.add_preference(PreferenceKind::together(vec![0, 1, 2, 3]), Strength::Must);
        project.add_preference(
            PreferenceKind::Apart {
                people: vec![3, 4, 5, 6],
                closeness: Closeness::Table,
            },
            Strength::Must,
        );
        project.add_preference(
            PreferenceKind::together(vec![7, 8, 9, 10]),
            Strength::Important,
        );
        project.add_preference(
            PreferenceKind::Apart {
                people: vec![11, 12, 13],
                closeness: Closeness::Neighbours,
            },
            Strength::NiceToHave,
        );
        project.rules[2].enabled = false;
        project.preferences[3].enabled = false;
        let scorer = Scorer::new(&project);
        let mut rng = rand::rngs::StdRng::seed_from_u64(101);
        let mut seats: Vec<_> = project
            .layout
            .seats()
            .into_iter()
            .map(|seat| seat.id)
            .collect();
        for snapshot in 0..160 {
            seats.shuffle(&mut rng);
            let mut assignment: BTreeMap<_, _> = project
                .attendees
                .iter()
                .zip(&seats)
                .filter(|_| rng.random_bool(0.75))
                .map(|(person, seat)| (person.id, seat.clone()))
                .collect();
            if snapshot % 5 == 0 {
                assignment.insert(0, "missing-seat".into());
            }
            if snapshot % 7 == 0
                && let Some(seat) = assignment.get(&2).cloned()
            {
                assignment.insert(1, seat);
            }
            let evaluation = crate::score::evaluate_with(&scorer, &project, &assignment);
            assert_eq!(
                scorer.totals(&assignment),
                (evaluation.penalty(), evaluation.hard_violations),
                "snapshot={snapshot}"
            );
        }
    }

    #[test]
    fn numeric_totals_keep_each_placement_and_count_a_preference_once() {
        let mut project = project(4);
        for hard in [true, true, false] {
            add_rule(
                &mut project,
                RuleKind::Placement {
                    person: 0,
                    area: Area::Table("table-2".into()),
                    must_be: true,
                    hard,
                    penalty: if hard { 0 } else { 77 },
                },
            );
        }
        project.add_preference(
            PreferenceKind::Apart {
                people: vec![0, 1, 2, 3],
                closeness: Closeness::Table,
            },
            Strength::Must,
        );
        let seats = project.layout.seats();
        let mut assignment = (0..4).map(|id| (id, seats[id].id.clone())).collect();
        let scorer = Scorer::new(&project);
        let evaluation = crate::score::evaluate_with(&scorer, &project, &assignment);
        assert_eq!(evaluation.hard_violations, 3);
        assert_eq!(evaluation.wish_penalty, 77);
        assert_eq!(evaluation.preferences.len(), 1);
        assert_eq!(
            scorer.totals(&assignment),
            (evaluation.penalty(), evaluation.hard_violations)
        );
        assignment.remove(&0);
        assert_eq!(scorer.totals(&assignment).1, 1);
        assert_eq!(scorer.totals(&BTreeMap::new()).1, 0);
    }

    #[test]
    fn compiled_wish_counts_keep_original_weighting() {
        let mut project = project(4);
        for (requester, wish) in [
            (0, WishKind::Seating),
            (0, WishKind::Seating),
            (0, WishKind::Avec),
            (1, WishKind::Seating),
            (0, WishKind::Seating),
        ] {
            add_rule(
                &mut project,
                RuleKind::Proximity {
                    requester,
                    target: Target::People { ids: vec![3] },
                    wish,
                    importance: Importance::High,
                    max_class: None,
                },
            );
        }
        project.rules.last_mut().unwrap().enabled = false;
        let scorer = Scorer::new(&project);
        for term in &scorer.problem.terms {
            let Source::Rule(id) = term.source else {
                continue;
            };
            let rule = project.rules.iter().find(|rule| rule.id == id).unwrap();
            assert_eq!(Some(term.costs), project.wish_costs(rule));
        }
    }
}
