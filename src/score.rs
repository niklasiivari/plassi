//! A chart's score in words: each rule and preference with its penalty and a
//! short explanation. The numbers come from the solver's own breakdown.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::i18n::{fill, tr};
use crate::model::{Area, Project, Rule, RuleKind};
use crate::preferences::{Preference, PreferenceKind};
use crate::seating::{Breakdown, PolicyReport, Scorer, Source, TermResult};

#[derive(Clone, Debug)]
pub struct RuleResult {
    pub rule_id: u64,
    pub label: String,
    pub detail: String,
    pub penalty: i64,
    pub hard_violation: bool,
}

/// How one of the planner's preferences fares in a seating.
#[derive(Clone, Debug)]
pub struct PreferenceResult {
    pub id: u64,
    pub penalty: i64,
    pub hard_violation: bool,
    /// E.g. "everyone close", "2 of 3 pairs at different tables".
    pub detail: String,
}

/// How well a rule or preference is met.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Outcome {
    Met,
    Partial,
    Broken,
}

impl Outcome {
    pub fn of(hard_violation: bool, penalty: i64) -> Self {
        match () {
            _ if hard_violation => Outcome::Broken,
            _ if penalty > 0 => Outcome::Partial,
            _ => Outcome::Met,
        }
    }
}

impl RuleResult {
    pub fn outcome(&self) -> Outcome {
        Outcome::of(self.hard_violation, self.penalty)
    }
}

impl PreferenceResult {
    pub fn outcome(&self) -> Outcome {
        Outcome::of(self.hard_violation, self.penalty)
    }
}

#[derive(Clone, Debug)]
pub struct Evaluation {
    /// Penalty of attendees' wishes and rules.
    pub wish_penalty: i64,
    /// Hard rules and "must" preferences broken.
    pub hard_violations: usize,
    /// Enabled rules, broken and costly first.
    pub results: Vec<RuleResult>,
    /// The planner's preferences, kept apart from wishes.
    pub preferences: Vec<PreferenceResult>,
    pub preference_penalty: i64,
    pub policy: PolicyReport,
    /// People none of whose wishes is near them, and what that costs.
    pub left_out: Vec<usize>,
    pub left_out_penalty: i64,
    pub assignment_errors: Vec<String>,
}

impl Evaluation {
    /// The score: wishes, the planner's preferences and seating policy. Lower is better.
    pub fn penalty(&self) -> i64 {
        self.wish_penalty + self.preference_penalty + self.policy_penalty()
    }

    /// Seating policy, including "nobody loses every wish".
    pub fn policy_penalty(&self) -> i64 {
        self.policy.total() + self.left_out_penalty
    }
}

pub fn evaluate(project: &Project, assignment: &BTreeMap<usize, String>) -> Evaluation {
    evaluate_with(&Scorer::new(project), project, assignment)
}

/// Like [`evaluate`], reusing a compiled scorer when scoring many charts.
pub fn evaluate_with(
    scorer: &Scorer,
    project: &Project,
    assignment: &BTreeMap<usize, String>,
) -> Evaluation {
    let breakdown = scorer.breakdown(assignment);
    let mut results: Vec<RuleResult> = project
        .rules
        .iter()
        .filter(|r| r.enabled)
        .filter_map(|rule| rule_result(project, rule, &breakdown))
        .collect();
    results.sort_by_key(|r| (!r.hard_violation, -r.penalty));
    let preferences: Vec<PreferenceResult> = project
        .active_preferences()
        .map(|p| preference_result(p, &breakdown))
        .collect();
    Evaluation {
        wish_penalty: results.iter().map(|r| r.penalty).sum(),
        hard_violations: results.iter().filter(|r| r.hard_violation).count()
            + preferences.iter().filter(|p| p.hard_violation).count(),
        preference_penalty: preferences.iter().map(|p| p.penalty).sum(),
        preferences,
        results,
        policy: breakdown.policy,
        left_out: breakdown.left_out,
        left_out_penalty: breakdown.left_out_penalty,
        assignment_errors: assignment_errors(project, assignment),
    }
}

fn rule_result(project: &Project, rule: &Rule, breakdown: &Breakdown) -> Option<RuleResult> {
    let (penalty, hard_violation, detail) = match &rule.kind {
        RuleKind::Placement { area, must_be, .. } => {
            let r = breakdown.placements.iter().find(|r| r.rule_id == rule.id)?;
            let detail = match (r.seated, r.penalty == 0 && !r.hard_violation) {
                (false, _) => tr("not seated").into(),
                (true, true) => tr("placement met").into(),
                (true, false) => missed_area(area, *must_be),
            };
            (r.penalty, r.hard_violation, detail)
        }
        RuleKind::Proximity {
            requester, target, ..
        } => {
            let t = breakdown
                .terms
                .iter()
                .find(|t| t.source == Source::Rule(rule.id))?;
            (
                t.penalty,
                t.hard_violation,
                proximity_detail(project, *requester, target, t),
            )
        }
        RuleKind::AvoidTableEnd { .. } => {
            let r = breakdown.ends.iter().find(|r| r.rule_id == rule.id)?;
            let detail = match r.level {
                None => tr("not seated"),
                Some(2) => tr("at a table end"),
                Some(1) => tr("next to a table end"),
                Some(_) => tr("away from the ends"),
            };
            (r.penalty, false, detail.into())
        }
    };
    Some(RuleResult {
        rule_id: rule.id,
        label: crate::i18n::tr_any(&rule.label).to_string(),
        detail,
        penalty,
        hard_violation,
    })
}

fn proximity_detail(
    project: &Project,
    requester: usize,
    target: &crate::model::Target,
    t: &TermResult,
) -> String {
    if t.seated_targets < t.required {
        fill(
            "only {} of {} wished people seated",
            &[&t.seated_targets, &t.required],
        )
    } else if let crate::model::Target::People { ids, .. } = target
        && ids.len() > 1
    {
        let names = project.names_of(ids, Some(requester));
        fill("all of {}: {}", &[&names, &t.class.label()])
    } else if let Some(nearest) = t.nearest {
        format!("{}: {}", project.person_name(nearest), t.class.label())
    } else {
        fill("{} nearest: {}", &[&t.required, &t.class.label()])
    }
}

/// Sums a preference's terms and says how it fares.
fn preference_result(preference: &Preference, breakdown: &Breakdown) -> PreferenceResult {
    let pairs: Vec<&TermResult> = breakdown
        .terms
        .iter()
        .filter(|t| t.source == Source::Preference(preference.id))
        .collect();
    let missed = pairs
        .iter()
        .filter(|t| t.penalty > 0 || t.hard_violation)
        .count();
    let penalty = pairs.iter().map(|t| t.penalty).sum();
    let detail = match (&preference.kind, pairs.first()) {
        (PreferenceKind::Together { .. }, Some(t)) if t.pieces > 1 => {
            fill("split into {} parts", &[&t.pieces])
        }
        (PreferenceKind::Together { .. }, Some(t)) if t.extra_rows > 0 => {
            fill("{} rows longer than needed", &[&t.extra_rows])
        }
        (PreferenceKind::Together { .. }, _) => tr("together").into(),
        (PreferenceKind::Apart { .. }, _) if missed == 0 => tr("kept apart").into(),
        (PreferenceKind::Apart { .. }, _) if missed == 1 => tr("1 pair too close").into(),
        (PreferenceKind::Apart { .. }, _) => fill("{} pairs too close", &[&missed]),
    };
    PreferenceResult {
        id: preference.id,
        penalty,
        hard_violation: pairs.iter().any(|t| t.hard_violation),
        detail,
    }
}

/// Unseated people, unknown seats and doubly assigned seats.
fn assignment_errors(project: &Project, assignment: &BTreeMap<usize, String>) -> Vec<String> {
    let seats: HashSet<String> = project.layout.seats().into_iter().map(|s| s.id).collect();
    let mut used = HashMap::new();
    let mut errors = Vec::new();
    for p in &project.attendees {
        match assignment.get(&p.id) {
            Some(id) if !seats.contains(id) => {
                errors.push(format!("{} has unknown seat {id}", p.name))
            }
            Some(id) if used.insert(id, p.id).is_some() => {
                errors.push(format!("seat {id} is assigned twice"))
            }
            Some(_) => {}
            None => errors.push(format!("{} has no seat", p.name)),
        }
    }
    errors
}

/// Where a missed placement rule wanted the person (`must_be`) or not.
fn missed_area(area: &Area, must_be: bool) -> String {
    match (area, must_be) {
        (Area::Seat(x), true) => fill("outside seat {}", &[x]),
        (Area::Table(x), true) => fill("outside table {}", &[x]),
        (Area::Row(x), true) => fill("outside row {}", &[x]),
        (Area::RowCenter(x), true) => fill("outside center of row {}", &[x]),
        (Area::Tag(x), true) => fill("outside seat tag {}", &[x]),
        (Area::Seat(x), false) => fill("in forbidden seat {}", &[x]),
        (Area::Table(x), false) => fill("at forbidden table {}", &[x]),
        (Area::Row(x), false) => fill("in forbidden row {}", &[x]),
        (Area::RowCenter(x), false) => fill("in forbidden row centre {}", &[x]),
        (Area::Tag(x), false) => fill("in forbidden seats tagged {}", &[x]),
    }
}
