//! Free seats for someone without one, best first.

use crate::model::{Candidate, Project, Seat};
use crate::score;

/// Free seats `person` could take in `candidate`, best first, each with the
/// score change of seating them there. Only the top free seat of each table
/// side is offered, so seating someone never leaves a gap above them.
pub fn free_seats(project: &Project, candidate: &Candidate, person: usize) -> Vec<(String, i64)> {
    let assignment = &candidate.assignment;
    let taken: std::collections::HashSet<&str> = assignment.values().map(String::as_str).collect();
    let seats = project.layout.seats();
    let free = |s: &Seat| !taken.contains(s.id.as_str());
    let top = |seat: &Seat| {
        !seats.iter().any(|s| {
            s.table_id == seat.table_id && s.side == seat.side && s.index < seat.index && free(s)
        })
    };
    let scorer = crate::seating::Scorer::new(project);
    let before = score::evaluate_with(&scorer, project, assignment).penalty();
    let mut ranked: Vec<_> = seats
        .iter()
        .filter(|s| free(s) && top(s))
        .map(|seat| {
            let mut trial = assignment.clone();
            trial.insert(person, seat.id.clone());
            let after = score::evaluate_with(&scorer, project, &trial);
            let key = (after.hard_violations, after.penalty(), seat.index);
            (key, seat.id.clone(), after.penalty() - before)
        })
        .collect();
    ranked.sort_by_key(|(key, ..)| *key);
    ranked
        .into_iter()
        .map(|(_, seat, delta)| (seat, delta))
        .collect()
}
