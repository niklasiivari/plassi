//! Hand edits to an option: moves of one person or a block, with score
//! previews, better-seat suggestions, and closing gaps.

use super::*;
use crate::arrange::{self, Rearrangement};
use crate::model::{Spot, gaps};

/// What dropping people somewhere would do.
#[derive(Clone)]
pub(in crate::app) struct DropPreview {
    pub delta: i64,
    pub hard_delta: i64,
    /// Change in wishes fully met.
    pub met_delta: i64,
    pub blocked: Option<String>,
}

impl DropPreview {
    fn blocked(reason: String) -> Self {
        Self {
            delta: 0,
            hard_delta: 0,
            met_delta: 0,
            blocked: Some(reason),
        }
    }
}

/// Wishes fully met in an evaluation.
fn wishes_met(project: &Project, evaluation: &Evaluation) -> i64 {
    evaluation
        .results
        .iter()
        .filter(|r| project.is_wish(r.rule_id) && r.outcome() == Outcome::Met)
        .count() as i64
}

/// An improving move: (seat, its current occupant, score delta).
pub(in crate::app) type Suggestion = (String, Option<usize>, i64);

/// The checked edit that puts each person on their spot in option `candidate`.
fn plan_in(
    project: &Project,
    candidate: usize,
    wanted: &[(usize, Spot)],
) -> Option<Result<Rearrangement, String>> {
    let candidate = project.candidates.get(candidate)?;
    Some(arrange::plan(project, candidate, wanted))
}

/// The score change of putting each person on their spot. Empty seats
/// mid-table are allowed while editing; they are marked on the chart.
pub(super) fn preview(
    project: &Project,
    candidate: usize,
    wanted: &[(usize, Spot)],
) -> Option<DropPreview> {
    let edit = match plan_in(project, candidate, wanted)? {
        Ok(edit) if edit.is_empty() => return None,
        Ok(edit) => edit,
        Err(reason) => return Some(DropPreview::blocked(reason)),
    };
    let assignment = &project.candidates[candidate].assignment;
    let next = edit.result(&project.candidates[candidate]).assignment;
    let scorer = crate::seating::Scorer::new(project);
    let before = score::evaluate_with(&scorer, project, assignment);
    let after = score::evaluate_with(&scorer, project, &next);
    Some(DropPreview {
        delta: after.penalty() - before.penalty(),
        hard_delta: after.hard_violations as i64 - before.hard_violations as i64,
        met_delta: wishes_met(project, &after) - wishes_met(project, &before),
        blocked: None,
    })
}

/// Seats scored in full when looking for better seats.
const SUGGESTION_CANDIDATES: usize = 12;

/// Improving moves for a seated person, or the best free seats for someone
/// without a seat, best first. Unlike moves by hand, suggestions never leave
/// an empty seat mid-table.
pub(super) fn best_moves(project: &Project, candidate: usize, person: usize) -> Vec<Suggestion> {
    let Some(option) = project.candidates.get(candidate) else {
        return Vec::new();
    };
    if project.seat_pinned_people().contains(&person) {
        return Vec::new();
    }
    if !option.assignment.contains_key(&person) {
        return arrange::free_seats(project, option, person)
            .into_iter()
            .map(|(seat, delta)| (seat, None, delta))
            .collect();
    }
    let seats = project.layout.seats();
    let gaps_now = gaps(&seats, &option.assignment).len();
    let occupant: HashMap<&str, usize> = option
        .assignment
        .iter()
        .map(|(p, s)| (s.as_str(), *p))
        .collect();
    let scorer = crate::seating::Scorer::new(project);
    let (before, hard_before) = total_score(&scorer, project, &option.assignment);
    // The solver's incremental score ranks every seat cheaply; only the
    // most promising few are scored in full.
    let mut moves: Vec<_> = crate::seating::move_deltas(project, &option.assignment, person)
        .into_iter()
        .take_while(|(_, delta)| *delta < 0)
        .filter_map(|(seat, _)| {
            let edit = arrange::plan(project, option, &[(person, Spot::Seat(seat.clone()))]);
            let next = edit.ok()?.result(option).assignment;
            (gaps(&seats, &next).len() <= gaps_now).then_some((seat, next))
        })
        .take(SUGGESTION_CANDIDATES)
        .filter_map(|(seat, next)| {
            let (after, hard_after) = total_score(&scorer, project, &next);
            let other = occupant.get(seat.as_str()).copied();
            (after < before && hard_after <= hard_before).then_some((seat, other, after - before))
        })
        .collect();
    moves.sort_by_key(|m| m.2);
    moves
}

pub(super) fn signed(value: i64) -> String {
    match value {
        0 => tr("no change").into(),
        v if v > 0 => fill("+{} worse", &[&v]),
        v => fill("{} better", &[&v]),
    }
}

impl PlassiApp {
    /// Moves one person to a spot; whoever is there takes their place.
    pub(super) fn move_to(&mut self, person: usize, spot: Spot) {
        self.rearrange(&[(person, spot)]);
    }

    /// Puts each person on their spot; people in the way take the places
    /// they left. Pins follow the people moved.
    pub(super) fn rearrange(&mut self, wanted: &[(usize, Spot)]) {
        if self.file.external_invalid {
            return;
        }
        let edit = match plan_in(&self.project, self.selected_candidate, wanted) {
            Some(Ok(edit)) if !edit.is_empty() => edit,
            Some(Err(reason)) => {
                self.status = reason;
                return;
            }
            _ => return,
        };
        let scorer = crate::seating::Scorer::new(&self.project);
        let assignment = &self.project.candidates[self.selected_candidate].assignment;
        let (before, _) = total_score(&scorer, &self.project, assignment);
        edit.apply(&mut self.project, self.selected_candidate);
        let assignment = &self.project.candidates[self.selected_candidate].assignment;
        let (after, _) = total_score(&scorer, &self.project, assignment);
        self.chart.last_change = Some((after - before, web_time::Instant::now()));
        let what = self.describe_edit(wanted, &edit);
        self.commit(&fill(
            "{} · score {} → {} ({})",
            &[&what, &before, &after, &signed(after - before)],
        ));
    }

    /// "Moved X", "Swapped X ↔ Y", "Moved 6 people" or "Took X off the
    /// seats", for the history.
    fn describe_edit(&self, wanted: &[(usize, Spot)], edit: &Rearrangement) -> String {
        let name = |id: usize| self.project.person_name(id).to_string();
        let moved = edit.moves.len() - edit.displaced;
        if wanted.iter().all(|(_, spot)| *spot == Spot::Unseated) {
            return match wanted {
                [(person, _)] => fill("Took {} off the seats", &[&name(*person)]),
                _ => fill("Took {} people off the seats", &[&moved]),
            };
        }
        match (wanted, edit.displaced) {
            ([(person, _)], 1) => {
                let other = edit.moves.iter().map(|(p, _)| *p).find(|p| p != person);
                fill(
                    "Swapped {} ↔ {}",
                    &[&name(*person), &name(other.unwrap_or(*person))],
                )
            }
            ([(person, _)], _) => fill("Moved {}", &[&name(*person)]),
            _ => fill("Moved {} people", &[&moved]),
        }
    }

    /// Slides people up each table side so empty seats end up at the bottom.
    /// Pinned people stay put.
    pub(super) fn close_gaps(&mut self) {
        let Some(candidate) = self.project.candidates.get(self.selected_candidate) else {
            return;
        };
        let pinned = self.project.seat_pinned_people();
        let seats = self.project.layout.seats();
        let mut assignment = candidate.assignment.clone();
        let occupant: HashMap<String, usize> =
            assignment.iter().map(|(p, s)| (s.clone(), *p)).collect();
        let mut sides: BTreeMap<(String, u8), Vec<&Seat>> = BTreeMap::new();
        for seat in &seats {
            let side = if seat.side == Side::Left { 0 } else { 1 };
            sides
                .entry((seat.table_id.clone(), side))
                .or_default()
                .push(seat);
        }
        for column in sides.values_mut() {
            column.sort_by_key(|s| s.index);
            let fixed = |s: &&Seat| occupant.get(&s.id).is_some_and(|p| pinned.contains(p));
            let people: Vec<usize> = column
                .iter()
                .filter(|s| !fixed(s))
                .filter_map(|s| occupant.get(&s.id).copied())
                .collect();
            let slots: Vec<&Seat> = column.iter().filter(|s| !fixed(s)).copied().collect();
            for (person, seat) in people.into_iter().zip(slots) {
                assignment.insert(person, seat.id.clone());
            }
        }
        let scorer = crate::seating::Scorer::new(&self.project);
        let (before, _) = total_score(&scorer, &self.project, &candidate.assignment);
        let (after, _) = total_score(&scorer, &self.project, &assignment);
        let candidate = &mut self.project.candidates[self.selected_candidate];
        candidate.reseat(assignment);
        candidate.solver_status = crate::model::SolverStatus::EditedByHand;
        self.chart.last_change = Some((after - before, web_time::Instant::now()));
        self.commit(&fill(
            "Closed gaps · score {} → {} ({})",
            &[&before, &after, &signed(after - before)],
        ));
    }
}
