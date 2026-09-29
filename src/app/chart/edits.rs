//! Hand edits to an option: moves and swaps with score previews, pins, closing gaps, duplicating and deleting options.

use super::*;
use crate::model::gaps;

/// What dropping a person on a seat would do.
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
        Self { delta: 0, hard_delta: 0, met_delta: 0, blocked: Some(reason) }
    }
}

/// Wishes fully met in an evaluation.
fn wishes_met(project: &Project, evaluation: &Evaluation) -> i64 {
    evaluation.results.iter().filter(|r| project.is_wish(r.rule_id) && r.outcome() == Outcome::Met).count() as i64
}

/// An improving move: (seat, its current occupant, score delta).
pub(in crate::app) type Suggestion = (String, Option<usize>, i64);

/// Assignment after moving `person` to `target` (swapping with its occupant).
pub(super) fn apply_move(
    assignment: &BTreeMap<usize, String>,
    person: usize,
    target: &str,
) -> Option<(BTreeMap<usize, String>, Option<usize>)> {
    let old = assignment.get(&person)?.clone();
    if old == target {
        return None;
    }
    let other = assignment
        .iter()
        .find(|(_, seat)| seat.as_str() == target)
        .map(|(id, _)| *id);
    let mut next = assignment.clone();
    next.insert(person, target.into());
    if let Some(other) = other {
        next.insert(other, old);
    }
    Some((next, other))
}

pub(super) fn preview_move(
    project: &Project,
    candidate: usize,
    person: usize,
    target: &str,
) -> Option<DropPreview> {
    let assignment = &project.candidates.get(candidate)?.assignment;
    let (next, other) = apply_move(assignment, person, target)?;
    if let Some(other) = other
        && project.pinned_people().contains(&other)
    {
        let name = project
            .person_name(other);
        return Some(DropPreview::blocked(fill("{} is pinned here", &[&name])));
    }
    let seats = project.layout.seats();
    if gaps(&seats, &next).len() > gaps(&seats, assignment).len() {
        return Some(DropPreview::blocked(tr("Would leave an empty seat mid-table; swap with someone instead").into()));
    }
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

/// Improving moves for one person, best first.
pub(super) fn best_moves(project: &Project, candidate: usize, person: usize) -> Vec<Suggestion> {
    if project.pinned_people().contains(&person) {
        return Vec::new();
    }
    let Some(assignment) = project.candidates.get(candidate).map(|c| &c.assignment) else {
        return Vec::new();
    };
    let seats = project.layout.seats();
    let pinned = project.pinned_people();
    let gaps_now = gaps(&seats, assignment).len();
    let scorer = crate::seating::Scorer::new(project);
    let (before, hard_before) = total_score(&scorer, project, assignment);
    // The solver's incremental score ranks every seat cheaply; only the
    // most promising few are scored in full.
    let mut moves: Vec<_> = crate::seating::move_deltas(project, assignment, person)
        .into_iter()
        .take_while(|(_, delta)| *delta < 0)
        .filter_map(|(seat, _)| {
            let (next, other) = apply_move(assignment, person, &seat)?;
            let blocked = other.is_some_and(|o| pinned.contains(&o)) || gaps(&seats, &next).len() > gaps_now;
            (!blocked).then_some((seat, next, other))
        })
        .take(SUGGESTION_CANDIDATES)
        .filter_map(|(seat, next, other)| {
            let (after, hard_after) = total_score(&scorer, project, &next);
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
    pub(super) fn move_or_swap(&mut self, person: usize, target: &str) {
        if self.file.external_invalid {
            return;
        }
        let Some(preview) = preview_move(&self.project, self.selected_candidate, person, target)
        else {
            return;
        };
        if let Some(reason) = preview.blocked {
            self.status = reason;
            return;
        }
        let candidate = &self.project.candidates[self.selected_candidate];
        let scorer = crate::seating::Scorer::new(&self.project);
        let (before, _) = total_score(&scorer, &self.project, &candidate.assignment);
        let Some((next, other)) = apply_move(&candidate.assignment, person, target) else {
            return;
        };
        let pinned = self.project.pinned_people();
        let candidate = &mut self.project.candidates[self.selected_candidate];
        candidate.assignment = next;
        candidate.solver_status = crate::model::SolverStatus::EditedByHand;
        candidate.hand_placed.insert(person);
        if let Some(other) = other {
            candidate.hand_placed.insert(other);
        }
        if pinned.contains(&person) {
            self.project.pin(person, target);
        }
        let name = |id: usize| {
            self.project
                .person_name(id).to_string()
        };
        let what = match other {
            Some(other) => fill("Swapped {} ↔ {}", &[&name(person), &name(other)]),
            None => fill("Moved {}", &[&name(person)]),
        };
        let after = before + preview.delta;
        self.chart.last_change = Some((preview.delta, web_time::Instant::now()));
        self.commit(&fill(
            "{} · score {} → {} ({})",
            &[&what, &before, &after, &signed(preview.delta)],
        ));
    }

    /// Slides people up each table side so empty seats end up at the bottom.
    /// Pinned people stay put.
    pub(super) fn close_gaps(&mut self) {
        let Some(candidate) = self.project.candidates.get(self.selected_candidate) else {
            return;
        };
        let pinned = self.project.pinned_people();
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
        candidate.assignment = assignment;
        candidate.solver_status = crate::model::SolverStatus::EditedByHand;
        self.chart.last_change = Some((after - before, web_time::Instant::now()));
        self.commit(&fill(
            "Closed gaps · score {} → {} ({})",
            &[&before, &after, &signed(after - before)],
        ));
    }

    /// Pins a person to `seat`, or unpins them when `seat` is `None`.
    pub(in crate::app) fn set_pin(&mut self, person: usize, seat: Option<&str>) {
        if self.file.external_invalid {
            return;
        }
        let name = self
            .project
            .person_name(person).to_string();
        match seat {
            Some(seat) => {
                self.project.pin(person, seat);
                self.commit(&fill("Pinned {} — the solver won't move them", &[&name]));
            }
            None => {
                self.project.unpin(person);
                self.commit(&fill("Unpinned {}", &[&name]));
            }
        }
    }

    pub(super) fn duplicate_option(&mut self, index: usize) {
        let Some(mut copy) = self.project.candidates.get(index).cloned() else {
            return;
        };
        copy.name = fill("{} (copy)", &[&copy.name]);
        self.project.candidates.insert(index + 1, copy);
        self.selected_candidate = index + 1;
        if let Some(compare) = self.chart.compare_with.as_mut().filter(|c| **c > index) {
            *compare += 1;
        }
        self.commit(tr("Duplicated option"));
    }

    pub(super) fn delete_option(&mut self, index: usize) {
        if index >= self.project.candidates.len() {
            return;
        }
        let name = self.project.candidates.remove(index).name;
        // Keep showing the same option when an earlier one goes.
        let shift = |i: usize| if i > index { i - 1 } else { i };
        if self.selected_candidate != index {
            self.selected_candidate = shift(self.selected_candidate);
        }
        self.chart.compare_with = self.chart.compare_with.filter(|c| *c != index).map(shift);
        self.commit(&fill("Deleted {}", &[&name]));
    }

    /// Single-key shortcuts on the chart: F fits the room, P pins or unpins
    /// the selected person, G groups the picked people.
    pub(super) fn chart_shortcuts(&mut self, ui: &egui::Ui) {
        if ui.ctx().memory(|m| m.focused().is_some()) {
            return;
        }
        let key = |k| ui.input(|i| i.key_pressed(k) && i.modifiers.is_none());
        if key(egui::Key::F) {
            self.chart.view.fit_pending = true;
        }
        if key(egui::Key::P)
            && let Some(person) = self.selected_person
        {
            let pinned = self.project.pinned_people().contains(&person);
            let seat = self.project.candidates.get(self.selected_candidate).and_then(|c| c.assignment.get(&person)).cloned();
            match (pinned, seat) {
                (true, _) => self.set_pin(person, None),
                (false, Some(seat)) => self.set_pin(person, Some(&seat)),
                (false, None) => {}
            }
        }
        if key(egui::Key::G) && self.chart.multi.len() >= 2 {
            let people = std::mem::take(&mut self.chart.multi).into_iter().collect();
            self.project.add_preference(crate::preferences::PreferenceKind::together(people), crate::preferences::Strength::Important);
            self.commit(tr("Added your preference; solve or improve to apply it"));
        }
    }
}
