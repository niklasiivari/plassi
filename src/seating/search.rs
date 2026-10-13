//! Running the search: independent tabu runs from different random starts.
//!
//! A [`Search`] can be advanced a slice at a time (the browser runs it between
//! frames) or run to its deadline, in parallel on every core where threads
//! exist.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use rand::SeedableRng;
use rand::rngs::StdRng;
use web_time::{Duration, Instant};

use crate::i18n::{fill, tr};
use crate::model::{Project, SolverStatus};

use super::problem::{HARD, NONE, Problem};
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
    /// Total soft penalty, without the internal hard-rule radix.
    pub cost: i64,
    pub hard_violations: usize,
    pub lower_bound: i64,
    pub runs: usize,
    pub runs_at_best: usize,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub iterations: u64,
}

impl Outcome {
    pub fn optimal(&self) -> bool {
        self.hard_violations == 0 && self.cost <= self.lower_bound
    }

    pub fn status(&self, seconds: f64) -> SolverStatus {
        if self.optimal() {
            SolverStatus::Optimal
        } else {
            SolverStatus::BestFound {
                seconds: seconds.round() as u64,
                runs_at_best: self.runs_at_best,
                runs: self.runs,
            }
        }
    }
}

/// Shared between a running search and whoever watches it: the best cost so
/// far (updated only when a run improves) and a request to stop early.
#[derive(Clone)]
pub struct Progress {
    best: Arc<AtomicI64>,
    stop: Arc<AtomicBool>,
    hard_cost: i64,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            best: Arc::new(AtomicI64::new(i64::MAX)),
            stop: Arc::new(AtomicBool::new(false)),
            hard_cost: HARD,
        }
    }
}

impl Progress {
    /// Records a run's new best cost.
    fn improved(&self, cost: i64) {
        self.best.fetch_min(cost, Ordering::Relaxed);
    }

    /// The best (score, broken hard rules) so far, once a run has one.
    pub fn best(&self) -> Option<(i64, i64)> {
        let cost = self.best.load(Ordering::Relaxed);
        (cost != i64::MAX).then_some((cost % self.hard_cost, cost / self.hard_cost))
    }

    /// Asks the search to finish now with what it has.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

/// One step of a run, reporting to `progress` only when its best improves.
fn step(run: &mut Run, pr: &Problem, rng: &mut StdRng, progress: &Progress) {
    let before = run.best.cost;
    run.step(pr, rng);
    if run.best.cost < before {
        progress.improved(run.best.cost);
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
    progress: Progress,
}

impl Search {
    pub fn new(project: &Project, options: Options) -> anyhow::Result<Self> {
        project.validate()?;
        if options
            .differ_from
            .is_some_and(|(_, minimum)| minimum > project.attendees.len())
        {
            anyhow::bail!(tr("More seat changes requested than there are people"));
        }
        let seats = project.layout.seat_count();
        if seats == 0 {
            anyhow::bail!(tr("Set up tables first"));
        }
        if seats < project.attendees.len() {
            anyhow::bail!(fill(
                "Only {} seats for {} people",
                &[&seats, &project.attendees.len()]
            ));
        }
        let mut fixed = options.fixed.clone();
        fixed.extend(project.seat_pinned_people());
        let pr = Problem::new(project, &fixed, options.differ_from, options.start);
        if let Some(p) = pr
            .placement
            .iter()
            .position(|row| row.iter().all(Option::is_none))
        {
            let name = project.person_name(pr.people[p]);
            anyhow::bail!(fill(
                "No allowed seat for {}. Check placement rules.",
                &[&name]
            ));
        }
        let start: Option<Vec<usize>> = options.start.map(|a| pr.state_of(a).0);
        let seed = options.seed.unwrap_or_else(rand::random);
        let progress = Progress {
            hard_cost: pr.hard_cost,
            ..Default::default()
        };
        let runs: Vec<(Run, StdRng)> = (0..options.runs.max(1))
            .map(|r| {
                let mut rng = StdRng::seed_from_u64(seed.wrapping_add(r as u64));
                (
                    Run::new(&pr, construct(&pr, &mut rng, start.as_deref())),
                    rng,
                )
            })
            .collect();
        for (run, _) in &runs {
            progress.improved(run.best.cost);
        }
        Ok(Self {
            lower_bound: pr.lower_bound(),
            pr,
            runs,
            started: Instant::now(),
            budget: options.budget,
            progress,
        })
    }

    /// A handle for watching and stopping this search from elsewhere.
    pub fn progress(&self) -> Progress {
        self.progress.clone()
    }

    fn deadline(&self) -> Instant {
        self.started + self.budget
    }

    /// Nothing left to do: out of time, or a run reached the lower bound.
    #[cfg(target_arch = "wasm32")]
    pub fn done(&self) -> bool {
        Instant::now() >= self.deadline()
            || self.progress.stopped()
            || self
                .runs
                .iter()
                .any(|(run, _)| run.best.cost <= self.lower_bound)
    }

    /// Works on every run in turn until `until` (or the deadline).
    #[cfg(target_arch = "wasm32")]
    pub fn advance(&mut self, until: Instant) {
        let until = until.min(self.deadline());
        let (pr, lower_bound, progress) = (&self.pr, self.lower_bound, &self.progress);
        while Instant::now() < until && !progress.stopped() {
            for (run, rng) in &mut self.runs {
                if run.best.cost > lower_bound {
                    step(run, pr, rng, progress);
                }
            }
            if self
                .runs
                .iter()
                .any(|(run, _)| run.best.cost <= lower_bound)
            {
                return;
            }
        }
    }

    /// Runs to the deadline, one thread per run where threads exist.
    pub fn run_to_end(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (pr, lower_bound, deadline, progress) =
                (&self.pr, self.lower_bound, self.deadline(), &self.progress);
            std::thread::scope(|scope| {
                for (run, rng) in &mut self.runs {
                    scope.spawn(move || {
                        while Instant::now() < deadline
                            && run.best.cost > lower_bound
                            && !progress.stopped()
                        {
                            step(run, pr, rng, progress);
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
        let best = &self
            .runs
            .iter()
            .min_by_key(|(run, _)| run.best.cost)
            .expect("at least one run")
            .0
            .best;
        debug_assert_eq!(best.cost, pr.cost(&best.seat_of, &best.occupant));
        Outcome {
            assignment: pr
                .people
                .iter()
                .zip(&best.seat_of)
                .filter(|(_, s)| **s != NONE)
                .map(|(p, s)| (*p, pr.seat_ids[*s].clone()))
                .collect(),
            cost: best.cost % pr.hard_cost,
            hard_violations: (best.cost / pr.hard_cost) as usize,
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
