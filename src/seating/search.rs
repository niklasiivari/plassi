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

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Outcome {
    /// Time spent searching after setup, capped at the requested budget.
    pub elapsed: Duration,
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
    hard_cost: Arc<AtomicI64>,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            best: Arc::new(AtomicI64::new(i64::MAX)),
            stop: Arc::new(AtomicBool::new(false)),
            hard_cost: Arc::new(AtomicI64::new(HARD)),
        }
    }
}

impl Progress {
    /// Records a run's new best cost.
    fn improved(&self, cost: i64) {
        self.best.fetch_min(cost, Ordering::Release);
    }

    fn reached_bound(&self, lower_bound: i64) -> bool {
        self.best.load(Ordering::Relaxed) <= lower_bound
    }

    /// The best (score, broken hard rules) so far, once a run has one.
    pub fn best(&self) -> Option<(i64, i64)> {
        let cost = self.best.load(Ordering::Acquire);
        let hard_cost = self.hard_cost.load(Ordering::Relaxed);
        (cost != i64::MAX).then_some((cost % hard_cost, cost / hard_cost))
    }

    /// Asks the search to finish now with what it has.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

/// Advances a run until its next completed iteration or the slice boundary,
/// reporting any improvement even if the row-block phase is still pending.
fn advance_run(
    run: &mut Run,
    pr: &Problem,
    rng: &mut StdRng,
    progress: &Progress,
    lower_bound: i64,
    until: Instant,
) {
    let before = run.best.cost;
    run.advance(pr, rng, || {
        Instant::now() < until && !progress.stopped() && !progress.reached_bound(lower_bound)
    });
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
    #[cfg(any(not(target_arch = "wasm32"), test))]
    pub fn new(project: &Project, options: Options) -> anyhow::Result<Self> {
        Self::new_with_progress(project, options, Progress::default())
    }

    /// Builds a search using a handle that can already be watched or stopped
    /// while a worker compiles the project and constructs its starting runs.
    pub fn new_with_progress(
        project: &Project,
        options: Options,
        progress: Progress,
    ) -> anyhow::Result<Self> {
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
        progress.hard_cost.store(pr.hard_cost, Ordering::Relaxed);
        let runs: Vec<(Run, StdRng)> = (0..options.runs.max(1))
            .take_while(|&run| run == 0 || !progress.stopped())
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
    #[cfg(any(target_arch = "wasm32", test))]
    pub fn progress(&self) -> Progress {
        self.progress.clone()
    }

    fn deadline(&self) -> Instant {
        self.started + self.budget
    }

    /// Nothing left to do: out of time, stopped, or a run proved the optimum.
    #[cfg(any(target_arch = "wasm32", test))]
    pub fn done(&self) -> bool {
        Instant::now() >= self.deadline()
            || self.progress.stopped()
            || self.progress.reached_bound(self.lower_bound)
    }

    /// Works on runs in turn, yielding within large neighborhoods as well as
    /// between iterations. Candidate order and tie-breaking stay unchanged.
    #[cfg(any(target_arch = "wasm32", test))]
    pub fn advance(&mut self, until: Instant) {
        let until = until.min(self.deadline());
        while Instant::now() < until
            && !self.progress.stopped()
            && !self.progress.reached_bound(self.lower_bound)
        {
            for (run, rng) in &mut self.runs {
                let slice = until.min(Instant::now() + Duration::from_millis(2));
                advance_run(run, &self.pr, rng, &self.progress, self.lower_bound, slice);
                if Instant::now() >= until
                    || self.progress.stopped()
                    || self.progress.reached_bound(self.lower_bound)
                {
                    return;
                }
            }
        }
    }

    /// Runs to the deadline on independent threads, sharing early termination
    /// when any run reaches the proven lower bound.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn run_to_end(&mut self) {
        let (pr, lower_bound, deadline, progress) =
            (&self.pr, self.lower_bound, self.deadline(), &self.progress);
        std::thread::scope(|scope| {
            for (run, rng) in &mut self.runs {
                scope.spawn(move || {
                    while Instant::now() < deadline
                        && !progress.reached_bound(lower_bound)
                        && !progress.stopped()
                    {
                        advance_run(run, pr, rng, progress, lower_bound, deadline);
                    }
                });
            }
        });
    }

    pub fn finish(self) -> Outcome {
        let elapsed = self.started.elapsed().min(self.budget);
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
            elapsed,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Layout, OneWish, SittingAlone, TableEnds, WithoutWishes};

    fn project() -> Project {
        let mut project = crate::tests::synthetic_project(8, 0);
        project.layout = Layout::simple(2, 2);
        project.policy.without_wishes = WithoutWishes::Indifferent;
        project.policy.sitting_alone = SittingAlone::Indifferent;
        project.policy.one_wish = OneWish::Indifferent;
        project.policy.table_ends = TableEnds::Indifferent;
        project
    }

    fn options() -> Options<'static> {
        Options {
            start: None,
            fixed: Default::default(),
            differ_from: None,
            budget: Duration::from_secs(10),
            runs: 2,
            seed: Some(1),
        }
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn native_runs_share_proven_optimal_termination() {
        let mut search = Search::new(&project(), options()).unwrap();
        assert_eq!(search.runs[0].0.best.cost, search.lower_bound);
        // A slower run has not reached the bound yet. The other run's proof
        // should stop it before it starts another neighborhood.
        search.runs[1].0.best.cost = search.lower_bound + 1;
        search.run_to_end();
        assert!(search.runs.iter().all(|(run, _)| run.iteration == 0));
    }

    #[test]
    fn preexisting_progress_handle_observes_compiled_radix_and_stop() {
        let mut project = project();
        project.rules.push(crate::import::wish_rule(
            1,
            0,
            vec![1],
            crate::model::WishKind::Avec,
            "",
        ));
        let progress = Progress::default();
        progress.stop();
        let search = Search::new_with_progress(&project, options(), progress.clone()).unwrap();
        assert_eq!(search.runs.len(), 1);
        let raw = search.runs[0].0.best.cost;
        assert_eq!(
            progress.best(),
            Some((raw % search.pr.hard_cost, raw / search.pr.hard_cost))
        );
        assert!(search.done());
    }

    #[test]
    fn elapsed_slice_does_not_start_work() {
        let mut search = Search::new(&project(), options()).unwrap();
        search.advance(Instant::now());
        assert!(search.runs.iter().all(|(run, _)| run.iteration == 0));
    }
    #[test]
    fn elapsed_work_time_is_capped_and_survives_worker_serialization() {
        let mut options = options();
        options.budget = Duration::from_secs(2);
        let mut search = Search::new(&project(), options).unwrap();
        search.started = Instant::now() - Duration::from_secs(3);
        let outcome = search.finish();
        assert_eq!(outcome.elapsed, Duration::from_secs(2));
        let json = serde_json::to_string(&outcome).unwrap();
        let decoded: Outcome = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.elapsed, outcome.elapsed);
        assert_eq!(decoded.assignment, outcome.assignment);
        assert_eq!(decoded.cost, outcome.cost);
    }
}
