//! Solves and improvements in the background (threads on the desktop, short
//! slices between frames in the browser), delivered back as new options.

use super::*;
use crate::i18n::{fill, tr};

/// A search in progress and what to do with its result.
pub(super) struct SolveJob {
    runner: platform::SearchRunner,
    /// Project revision it started from; a changed project discards the result.
    revision: u64,
    pub(super) started: Instant,
    pub(super) budget: Duration,
    kind: JobKind,
}

enum JobKind {
    New,
    /// Improving an option: its name and the people placed by hand.
    Improve { from: String, hand_placed: std::collections::BTreeSet<usize> },
}

impl PlassiApp {
    pub(super) fn start_solve(&mut self, alternative: bool) {
        if self.solving.is_some() || self.file.external_invalid {
            return;
        }
        let prior = alternative.then(|| self.project.candidates.get(self.selected_candidate)).flatten();
        let min_changed = if prior.is_some() { (self.project.active_people().len() / 10).max(1) } else { 0 };
        match seating::solve_search(&self.project, prior, min_changed, seating::SOLVE_TIME) {
            Ok(search) => {
                self.start_job(search, seating::SOLVE_TIME, JobKind::New);
                self.status = tr("Solving…").into();
            }
            Err(error) => self.status = fill("Cannot solve: {}", &[&format!("{error:#}")]),
        }
    }

    /// Re-optimizes the current option, keeping pinned and hand-placed people
    /// where they are. The result becomes a new option.
    pub(super) fn start_improve(&mut self) {
        if self.solving.is_some() || self.file.external_invalid {
            return;
        }
        let Some(current) = self.project.candidates.get(self.selected_candidate) else {
            return;
        };
        let mut fixed = self.project.pinned_people();
        fixed.extend(current.hand_placed.iter().copied());
        let kind = JobKind::Improve { from: current.name.clone(), hand_placed: current.hand_placed.clone() };
        match seating::improve_search(&self.project, &current.assignment, &fixed, seating::IMPROVE_TIME) {
            Ok(search) => {
                self.start_job(search, seating::IMPROVE_TIME, kind);
                self.status = tr("Improving around your edits…").into();
            }
            Err(error) => self.status = fill("Cannot improve: {}", &[&format!("{error:#}")]),
        }
    }

    fn start_job(&mut self, search: seating::Search, budget: Duration, kind: JobKind) {
        self.solving = Some(SolveJob {
            runner: platform::SearchRunner::start(search),
            revision: self.file.revision,
            started: Instant::now(),
            budget,
            kind,
        });
    }

    pub(super) fn poll_solver(&mut self) {
        let Some(outcome) = self.solving.as_mut().and_then(|job| job.runner.poll()) else {
            return;
        };
        let job = self.solving.take().expect("job just polled");
        if job.revision != self.file.revision {
            self.status = tr("The project changed during the search; its result was discarded").into();
            return;
        }
        let mut candidate = seating::candidate(&self.project, outcome, job.budget);
        if let JobKind::Improve { from, hand_placed } = job.kind {
            candidate.solver_status = fill("Improved from {}, keeping {} hand-placed people", &[&from, &hand_placed.len()]);
            candidate.hand_placed = hand_placed;
        }
        let evaluation = score::evaluate(&self.project, &candidate.assignment);
        self.project.candidates.push(candidate);
        self.selected_candidate = self.project.candidates.len() - 1;
        self.page = Page::Chart;
        self.commit(&fill("Saved option: score {}, hard violations {}", &[&evaluation.penalty(), &evaluation.hard_violations]));
    }
}
