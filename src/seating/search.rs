//! Running the search: independent tabu runs from different random starts.
//!
//! A [`Search`] can be advanced a slice at a time (the browser runs it between
//! frames) or run to its deadline, in parallel on every core where threads
//! exist.

use std::collections::{BTreeMap, HashSet};

use rand::rngs::StdRng;
use rand::SeedableRng;
use web_time::{Duration, Instant};

use crate::i18n::{fill, tr};
use crate::model::Project;

use super::problem::{NONE, Problem};
use super::tabu::{Run, construct};

pub struct Options<'a> {
    /// Start from this seating instead of constructing new ones.
    pub start: Option<&'a BTreeMap<usize, String>>,
    /// People who must not move (pins are always fixed).
    pub fixed: HashSet<usize>,
    /// Previous option and how many constrained people must sit elsewhere.
    pub differ_from: Option<(&'a BTreeMap<usize, String>, usize)>,
    pub budget: Duration,
    /// Independent runs.
    pub runs: usize,
    /// Random seed for reproducible runs; `None` for a fresh one.
    pub seed: Option<u64>,
}

pub struct Outcome {
    pub assignment: BTreeMap<usize, String>,
    pub cost: i64,
    pub lower_bound: i64,
    pub runs: usize,
    pub runs_at_best: usize,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub iterations: u64,
}

impl Outcome {
    pub fn optimal(&self) -> bool {
        self.cost <= self.lower_bound
    }

    pub fn status(&self, seconds: f64) -> String {
        if self.optimal() {
            tr("Optimal — no seating can score better").into()
        } else {
            fill(
                "Best found in {} s · {} of {} runs reached this score",
                &[&format!("{seconds:.0}"), &self.runs_at_best, &self.runs],
            )
        }
    }
}

/// A search in progress. It owns everything it needs, so it can move to a
/// worker thread or live in the app between frames.
pub struct Search {
    pr: Problem,
    runs: Vec<(Run, StdRng)>,
    lower_bound: i64,
    started: Instant,
    budget: Duration,
}

impl Search {
    pub fn new(project: &Project, options: Options) -> anyhow::Result<Self> {
        let seats = project.layout.seat_count();
        if seats == 0 {
            anyhow::bail!(tr("define the room layout before solving"));
        }
        if seats < project.attendees.len() {
            anyhow::bail!(fill("room has {} seats for {} attendees", &[&seats, &project.attendees.len()]));
        }
        let mut fixed = options.fixed.clone();
        fixed.extend(project.pinned_people());
        let pr = Problem::new(project, &fixed, options.differ_from, options.start);
        if let Some(p) = pr.placement.iter().position(|row| row.iter().all(Option::is_none)) {
            let name = project.person_name(pr.people[p]);
            anyhow::bail!(fill("no seat is allowed for {} by the hard placement rules", &[&name]));
        }
        let start: Option<Vec<usize>> = options.start.map(|a| pr.state_of(a).0);
        let seed = options.seed.unwrap_or_else(rand::random);
        let runs = (0..options.runs.max(1))
            .map(|r| {
                let mut rng = StdRng::seed_from_u64(seed.wrapping_add(r as u64));
                (Run::new(&pr, construct(&pr, &mut rng, start.as_deref())), rng)
            })
            .collect();
        Ok(Self { lower_bound: pr.lower_bound(), pr, runs, started: Instant::now(), budget: options.budget })
    }

    fn deadline(&self) -> Instant {
        self.started + self.budget
    }

    /// Nothing left to do: out of time, or a run reached the lower bound.
    #[cfg(target_arch = "wasm32")]
    pub fn done(&self) -> bool {
        Instant::now() >= self.deadline() || self.runs.iter().any(|(run, _)| run.best.cost <= self.lower_bound)
    }

    /// Works on every run in turn until `until` (or the deadline).
    #[cfg(target_arch = "wasm32")]
    pub fn advance(&mut self, until: Instant) {
        let until = until.min(self.deadline());
        let (pr, lower_bound) = (&self.pr, self.lower_bound);
        while Instant::now() < until {
            for (run, rng) in &mut self.runs {
                if run.best.cost > lower_bound {
                    run.step(pr, rng);
                }
            }
            if self.runs.iter().any(|(run, _)| run.best.cost <= lower_bound) {
                return;
            }
        }
    }

    /// Runs to the deadline, one thread per run where threads exist.
    pub fn run_to_end(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (pr, lower_bound, deadline) = (&self.pr, self.lower_bound, self.deadline());
            std::thread::scope(|scope| {
                for (run, rng) in &mut self.runs {
                    scope.spawn(move || {
                        while Instant::now() < deadline && run.best.cost > lower_bound {
                            run.step(pr, rng);
                        }
                    });
                }
            });
        }
        #[cfg(target_arch = "wasm32")]
        self.advance(self.deadline());
    }

    pub fn finish(self) -> Outcome {
        let pr = &self.pr;
        let scores: Vec<i64> = self.runs.iter().map(|(run, _)| run.best.cost).collect();
        let iterations = self.runs.iter().map(|(run, _)| run.iteration).sum();
        let best = &self.runs.iter().min_by_key(|(run, _)| run.best.cost).expect("at least one run").0.best;
        debug_assert_eq!(best.cost, pr.cost(&best.seat_of, &best.occupant));
        Outcome {
            assignment: pr
                .people
                .iter()
                .zip(&best.seat_of)
                .filter(|(_, s)| **s != NONE)
                .map(|(p, s)| (*p, pr.seat_ids[*s].clone()))
                .collect(),
            cost: best.cost,
            lower_bound: self.lower_bound,
            runs: scores.len(),
            runs_at_best: scores.iter().filter(|s| **s == best.cost).count(),
            iterations,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Runs a search to its deadline and returns the best seating found.
pub fn search(project: &Project, options: Options) -> anyhow::Result<Outcome> {
    let mut search = Search::new(project, options)?;
    search.run_to_end();
    Ok(search.finish())
}
