//! Table ends in the search: "avoid table ends" rules, and the seating
//! policy keeping people without wishes off the ends. Whether a seat is an
//! end depends on who sits around it, so a swap re-scores everyone within
//! two seats.

use std::collections::{HashMap, HashSet};

use crate::model::{Project, RuleKind, Seat, beside_lists, end_level};

use super::problem::NONE;

struct EndTerm {
    rule_id: u64,
    person: usize,
    end: i64,
    near: i64,
}

/// How an "avoid table ends" rule fares.
pub struct EndResult {
    pub rule_id: u64,
    /// 2 at an end, 1 next to one, 0 away; `None` without a seat.
    pub level: Option<u8>,
    pub penalty: i64,
}

pub(super) struct Ends {
    terms: Vec<EndTerm>,
    /// Per person index: neither wishes nor is wished for, nor in a group.
    lone: Vec<bool>,
    /// Policy cost for a lone person at an end.
    lone_penalty: i64,
    /// End terms per person index.
    of_person: Vec<Vec<usize>>,
    beside: Vec<Vec<usize>>,
    /// Seats whose end level can change when this seat's occupant changes.
    pub(super) reach: Vec<Vec<usize>>,
}

impl Ends {
    pub(super) fn new(project: &Project, seats: &[Seat], index: &HashMap<usize, usize>) -> Self {
        let mut terms = Vec::new();
        let mut of_person = vec![Vec::new(); index.len()];
        for rule in project.rules.iter().filter(|r| r.enabled) {
            if let RuleKind::AvoidTableEnd {
                person,
                end_penalty,
                near_end_penalty,
            } = &rule.kind
                && let Some(&p) = index.get(person)
            {
                of_person[p].push(terms.len());
                terms.push(EndTerm {
                    rule_id: rule.id,
                    person: p,
                    end: *end_penalty,
                    near: *near_end_penalty,
                });
            }
        }
        let lone = lone_people(project, index);
        let lone_penalty = if lone.contains(&true) {
            project.policy.lone_end_penalty()
        } else {
            0
        };
        let beside = if terms.is_empty() && lone_penalty == 0 {
            vec![Vec::new(); seats.len()]
        } else {
            beside_lists(seats)
        };
        let reach = (0..seats.len())
            .map(|s| {
                let mut near: Vec<usize> = vec![s];
                for &a in &beside[s] {
                    near.push(a);
                    near.extend(beside[a].iter().copied());
                }
                near.sort_unstable();
                near.dedup();
                near
            })
            .collect();
        Self {
            terms,
            lone,
            lone_penalty,
            of_person,
            beside,
            reach,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.terms.is_empty() && self.lone_penalty == 0
    }

    /// Penalty for whoever sits on `seat`: their rules and the policy.
    pub(super) fn cost_at(&self, seat: usize, occupant: &[usize]) -> i64 {
        self.rule_cost_at(seat, occupant) + self.lone_cost_at(seat, occupant)
    }

    fn rule_cost_at(&self, seat: usize, occupant: &[usize]) -> i64 {
        let person = occupant[seat];
        if person == NONE || self.of_person[person].is_empty() {
            return 0;
        }
        let level = end_level(seat, &self.beside, &|s| occupant[s] != NONE);
        self.of_person[person]
            .iter()
            .map(|&t| match level {
                2 => self.terms[t].end,
                1 => self.terms[t].near,
                _ => 0,
            })
            .sum()
    }

    /// The policy cost of a person without wishes sitting on `seat`.
    pub(super) fn lone_cost_at(&self, seat: usize, occupant: &[usize]) -> i64 {
        let person = occupant[seat];
        if self.lone_penalty == 0 || person == NONE || !self.lone[person] {
            return 0;
        }
        match end_level(seat, &self.beside, &|s| occupant[s] != NONE) {
            2 => self.lone_penalty,
            _ => 0,
        }
    }

    pub(super) fn results(&self, seat_of: &[usize], occupant: &[usize]) -> Vec<EndResult> {
        self.terms
            .iter()
            .map(|term| {
                let seat = seat_of[term.person];
                let level =
                    (seat != NONE).then(|| end_level(seat, &self.beside, &|s| occupant[s] != NONE));
                let penalty = match level {
                    Some(2) => term.end,
                    Some(1) => term.near,
                    _ => 0,
                };
                EndResult {
                    rule_id: term.rule_id,
                    level,
                    penalty,
                }
            })
            .collect()
    }

    /// The rules' part; the policy's part is in the policy report.
    pub(super) fn rules_total(&self, occupant: &[usize]) -> i64 {
        if self.terms.is_empty() {
            return 0;
        }
        (0..occupant.len())
            .map(|s| self.rule_cost_at(s, occupant))
            .sum()
    }
}

/// Per person index: whether they neither wish for anyone nor are wished
/// for, and aren't in a group. Pinned people are left out: they can't move.
fn lone_people(project: &Project, index: &HashMap<usize, usize>) -> Vec<bool> {
    let mut connected: HashSet<usize> = project.seat_pinned_people();
    for rule in project.rules.iter().filter(|r| r.enabled) {
        if let RuleKind::Proximity {
            requester, target, ..
        } = &rule.kind
        {
            connected.insert(*requester);
            connected.extend(project.target_ids(*requester, target));
        }
    }
    for preference in project.active_preferences() {
        if let crate::preferences::PreferenceKind::Together { people } = &preference.kind {
            connected.extend(people.iter().copied());
        }
    }
    let mut lone = vec![false; index.len()];
    for (id, &p) in index {
        lone[p] = !connected.contains(id);
    }
    lone
}
