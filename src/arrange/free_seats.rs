//! Free seats for someone without one, best first.

use crate::model::{Candidate, Pin, Project, Seat};
use crate::seating::Scorer;

/// Free seats `person` could take in `candidate`, best first, each with the
/// score change of seating them there. Only the top free seat of each table
/// side is offered, so seating someone never leaves a gap above them, and
/// never a seat pinned to someone else: pins hold in every option, so the
/// solver would want that seat for its owner.
pub fn free_seats(
    project: &Project,
    scorer: &Scorer,
    candidate: &Candidate,
    person: usize,
) -> Vec<(String, i64)> {
    let assignment = &candidate.assignment;
    let taken: std::collections::HashSet<&str> = assignment.values().map(String::as_str).collect();
    let pinned: std::collections::HashSet<String> = project
        .pins()
        .into_iter()
        .filter(|(p, _)| *p != person)
        .filter_map(|(_, pin)| match pin {
            Pin::Seat(seat) => Some(seat),
            Pin::Table(_) => None,
        })
        .collect();
    let seats = project.layout.seats();
    let free = |s: &Seat| !taken.contains(s.id.as_str());
    let top = |seat: &Seat| {
        !seats.iter().any(|s| {
            s.table_id == seat.table_id && s.side == seat.side && s.index < seat.index && free(s)
        })
    };
    // The cost holds broken hard rules above the score, so ranking by it
    // ranks by (hard rules, score).
    let radix = project.score_radix();
    let before = scorer.cost(assignment);
    let mut ranked: Vec<_> = seats
        .iter()
        .filter(|s| free(s) && top(s) && !pinned.contains(&s.id))
        .map(|seat| {
            let mut trial = assignment.clone();
            trial.insert(person, seat.id.clone());
            let after = scorer.cost(&trial);
            let delta = after.rem_euclid(radix) - before.rem_euclid(radix);
            ((after, seat.index), seat.id.clone(), delta)
        })
        .collect();
    ranked.sort_by_key(|(key, ..)| *key);
    ranked
        .into_iter()
        .map(|(_, seat, delta)| (seat, delta))
        .collect()
}
