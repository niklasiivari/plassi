//! Seating optimisation.
//!
//! - [`problem`]: the project compiled to indices and the score.
//! - [`spare_seats`]: which seats stay empty.
//! - [`ends`]: "avoid table ends" rules, which depend on who sits around.
//! - [`tabu`]: one tabu search run and its moves.
//! - [`search`]: independent runs, in parallel or a slice at a time.
//! - [`breakdown`]: a chart's score part by part, for display.
//!
//! [`solve`] builds a new option; [`improve`] refines an existing one.

mod breakdown;
mod ends;
mod problem;
mod search;
pub(crate) mod spare_seats;
mod tabu;

#[cfg(test)]
use std::collections::{BTreeMap, HashSet};

#[cfg(any(not(target_arch = "wasm32"), test))]
use anyhow::Result;
use web_time::Duration;

use crate::model::{Candidate, Project};

pub use breakdown::{Breakdown, Scorer, TermResult};
pub use problem::{PolicyReport, Source};
#[cfg(not(target_arch = "wasm32"))]
pub use search::search;
pub use search::{Options, Outcome, Progress, Search};
#[cfg(test)]
pub(crate) use tabu::check_deltas;
pub use tabu::move_deltas;

/// Search time for a new option. Quality matters more than waiting: longer
/// runs find clearly better charts.
pub const SOLVE_TIME: Duration = Duration::from_secs(30);
/// Search time for "Improve", which starts from a good seating.
pub const IMPROVE_TIME: Duration = Duration::from_secs(10);

/// Independent search runs per solve: one per CPU core, one in the browser.
#[cfg(any(not(target_arch = "wasm32"), test))]
fn runs() -> usize {
    if cfg!(target_arch = "wasm32") {
        1
    } else {
        std::thread::available_parallelism().map_or(2, |n| n.get())
    }
}

/// The search for a new option. With `prior`, at least `min_changed` people
/// with wishes must sit somewhere else than in that option.
#[cfg(not(target_arch = "wasm32"))]
pub fn solve_search(
    project: &Project,
    prior: Option<&Candidate>,
    min_changed: usize,
    budget: Duration,
) -> Result<Search> {
    Search::new(
        project,
        Options {
            start: None,
            fixed: Default::default(),
            differ_from: prior.map(|c| (&c.assignment, min_changed)),
            budget,
            runs: runs(),
            seed: None,
        },
    )
}

/// The search that improves an existing seating, keeping `fixed` people (and pins) in place.
#[cfg(test)]
pub fn improve_search(
    project: &Project,
    assignment: &BTreeMap<usize, String>,
    fixed: &HashSet<usize>,
    budget: Duration,
) -> Result<Search> {
    Search::new(
        project,
        Options {
            start: Some(assignment),
            fixed: fixed.clone(),
            differ_from: None,
            budget,
            runs: runs(),
            seed: None,
        },
    )
}

/// A new option with a name no other option has.
pub fn candidate(project: &Project, outcome: Outcome, budget: Duration) -> Candidate {
    Candidate {
        name: project.new_option_name(),
        solver_status: outcome.status(budget.as_secs_f64()),
        assignment: outcome.assignment,
        hand_placed: Default::default(),
        loose: Default::default(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Solves from scratch, running the search to its end.
pub fn solve(
    project: &Project,
    prior: Option<&Candidate>,
    min_changed: usize,
    budget: Duration,
) -> Result<Candidate> {
    let mut search = solve_search(project, prior, min_changed, budget)?;
    search.run_to_end();
    Ok(candidate(project, search.finish(), budget))
}
