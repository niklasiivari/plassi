//! Solves and improvements on background threads or browser workers,
//! delivered back as new options.

use super::*;
use crate::i18n::{fill, tr};
use crate::model::SolverStatus;

/// A search in progress and what to do with its result.
pub(super) struct SolveJob {
    pub(super) runner: platform::SearchRunner,
    /// Project revision it started from; a changed project discards the result.
    revision: u64,
    pub(super) started: Instant,
    /// Initialization finished; the budget starts with search work.
    pub(super) ready: bool,
    pub(super) budget: Duration,
    kind: JobKind,
}

enum JobKind {
    New,
    /// Improving an option: its name and the people placed by hand.
    Improve {
        from: String,
        hand_placed: std::collections::BTreeSet<usize>,
    },
    /// Searching longer from an option: its name and (broken hard rules, score).
    Continue {
        from: String,
        before: (usize, i64),
    },
}

impl PlassiApp {
    pub(super) fn start_solve(&mut self, alternative: bool) {
        if self.solving.is_some() || self.file.external_invalid {
            return;
        }
        let prior = alternative
            .then(|| self.project.candidates.get(self.selected_candidate))
            .flatten();
        let min_changed = if prior.is_some() {
            (self.project.active_people().len() / 10).max(1)
        } else {
            0
        };
        let mut request = platform::SearchRequest::new(&self.project, self.solve_time);
        request.differ_from = prior.map(|c| (c.assignment.clone(), min_changed));
        self.start_job(request, self.solve_time, JobKind::New);
        self.status = tr("Solving…").into();
    }

    /// How many people Improve and "Seat the rest" keep where they are in the
    /// selected option: pinned to a seat or placed by hand.
    pub(super) fn kept_by_improve(&self) -> usize {
        let Some(current) = self.project.candidates.get(self.selected_candidate) else {
            return 0;
        };
        let mut kept = self.project.seat_pinned_people();
        kept.extend(current.hand_placed.iter().copied());
        kept.retain(|p| current.assignment.contains_key(p));
        kept.len()
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
        let mut fixed = self.project.seat_pinned_people();
        fixed.extend(current.hand_placed.iter().copied());
        let kind = JobKind::Improve {
            from: current.name.clone(),
            hand_placed: current.hand_placed.clone(),
        };
        let mut request = platform::SearchRequest::new(&self.project, self.improve_time);
        request.start = Some(current.assignment.clone());
        request.fixed = fixed;
        self.start_job(request, self.improve_time, kind);
        self.status = tr("Improving…").into();
    }

    /// Searches from the current option for another full solve time, keeping
    /// only pins fixed. The result becomes a new option only if it is better.
    pub(super) fn start_continue(&mut self) {
        if self.solving.is_some() || self.file.external_invalid {
            return;
        }
        let Some(current) = self.project.candidates.get(self.selected_candidate) else {
            return;
        };
        let evaluation = score::evaluate(&self.project, &current.assignment);
        let kind = JobKind::Continue {
            from: current.name.clone(),
            before: (evaluation.hard_violations, evaluation.penalty()),
        };
        let mut request = platform::SearchRequest::new(&self.project, self.solve_time);
        request.start = Some(current.assignment.clone());
        self.start_job(request, self.solve_time, kind);
        self.status = tr("Searching…").into();
    }

    /// Ends the running search early, keeping the best seating found so far.
    pub(super) fn stop_search(&mut self) {
        if let Some(job) = &self.solving {
            job.runner.stop();
        }
    }

    /// Edits make a job obsolete immediately, rather than after its budget.
    pub(super) fn cancel_stale_search(&mut self) {
        if self
            .solving
            .as_ref()
            .is_some_and(|job| job.revision != self.file.revision)
            && let Some(job) = self.solving.take()
        {
            job.runner.stop();
        }
    }

    fn start_job(&mut self, request: platform::SearchRequest, budget: Duration, kind: JobKind) {
        self.solving = Some(SolveJob {
            runner: platform::SearchRunner::start(request),
            revision: self.file.revision,
            started: Instant::now(),
            ready: false,
            budget,
            kind,
        });
    }

    pub(super) fn poll_solver(&mut self) {
        if self
            .solving
            .as_ref()
            .is_some_and(|job| job.revision != self.file.revision)
        {
            if let Some(job) = self.solving.take() {
                job.runner.stop();
            }
            self.status = tr("Project changed; search result discarded").into();
            return;
        }
        let outcome = self.solving.as_mut().and_then(|job| {
            let outcome = job.runner.poll();
            if !job.ready && job.runner.progress().is_some() {
                job.ready = true;
                job.started = Instant::now();
            }
            outcome
        });
        let Some(outcome) = outcome else {
            return;
        };
        let job = self.solving.take().expect("job just polled");
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                self.status = search_failure_status(&job.kind, error);
                return;
            }
        };
        let elapsed = outcome.elapsed;
        let mut candidate = seating::candidate(&self.project, outcome, elapsed);
        let evaluation = score::evaluate(&self.project, &candidate.assignment);
        match job.kind {
            JobKind::New => {}
            JobKind::Improve { from, hand_placed } => {
                candidate.solver_status = SolverStatus::Improved {
                    from,
                    hand_placed: hand_placed.len(),
                };
                candidate.hand_placed = hand_placed;
            }
            JobKind::Continue { from, before } => {
                if (evaluation.hard_violations, evaluation.penalty()) >= before {
                    self.status = fill("No better seating found in {} s", &[&elapsed.as_secs()]);
                    return;
                }
                candidate.solver_status = SolverStatus::Continued {
                    from,
                    seconds: elapsed.as_secs(),
                };
            }
        }
        self.project.candidates.push(candidate);
        self.selected_candidate = self.project.candidates.len() - 1;
        self.page = Page::Chart;
        self.commit(&fill(
            "Saved: score {} · {} hard rules broken",
            &[&evaluation.penalty(), &evaluation.hard_violations],
        ));
    }
}

fn search_failure_status(kind: &JobKind, error: platform::SearchError) -> String {
    match error {
        platform::SearchError::Invalid(reason) => fill(
            match kind {
                JobKind::New => "Cannot solve: {}",
                JobKind::Improve { .. } => "Cannot improve: {}",
                JobKind::Continue { .. } => "Cannot continue searching: {}",
            },
            &[&reason],
        ),
        platform::SearchError::Interrupted(detail) => {
            #[cfg(not(target_arch = "wasm32"))]
            eprintln!("Seating search failed: {detail}");
            #[cfg(target_arch = "wasm32")]
            web_sys::console::error_1(&format!("Seating search failed: {detail}").into());
            tr(match kind {
                JobKind::New => "The seating search stopped unexpectedly. Try again.",
                JobKind::Improve { .. } => {
                    "The seating improvement stopped unexpectedly. Try again."
                }
                JobKind::Continue { .. } => "The continued search stopped unexpectedly. Try again.",
            })
            .into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_failures_use_localized_recovery_and_keep_validation_reasons() {
        crate::i18n::set(crate::i18n::Lang::Fi);
        for (kind, expected) in [
            (
                JobKind::New,
                "The seating search stopped unexpectedly. Try again.",
            ),
            (
                JobKind::Improve {
                    from: "Existing".into(),
                    hand_placed: Default::default(),
                },
                "The seating improvement stopped unexpectedly. Try again.",
            ),
            (
                JobKind::Continue {
                    from: "Existing".into(),
                    before: (0, 100),
                },
                "The continued search stopped unexpectedly. Try again.",
            ),
        ] {
            let message = search_failure_status(
                &kind,
                platform::SearchError::Interrupted("technical worker diagnostic".into()),
            );
            assert_eq!(message, tr(expected));
            assert_ne!(message, expected, "missing Finnish translation");
            assert!(!message.contains("technical worker diagnostic"));
            let reason = tr("Add enough seats for everyone");
            let invalid =
                search_failure_status(&kind, platform::SearchError::Invalid(reason.into()));
            assert!(invalid.ends_with(&format!(": {reason}")), "{invalid}");
            assert!(!invalid.contains("Cannot"), "{invalid}");
        }
    }
}
