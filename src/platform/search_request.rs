//! Owned input for building a search away from the interface thread.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use web_time::Duration;

use crate::model::Project;
use crate::seating::{Options, Progress, Search};

#[derive(Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub project: Project,
    pub start: Option<BTreeMap<usize, String>>,
    pub fixed: HashSet<usize>,
    pub differ_from: Option<(BTreeMap<usize, String>, usize)>,
    pub budget: Duration,
    pub language: String,
    pub seed: Option<u64>,
}

impl SearchRequest {
    pub fn new(project: &Project, budget: Duration) -> Self {
        Self {
            project: project.clone(),
            start: None,
            fixed: HashSet::new(),
            differ_from: None,
            budget,
            language: crate::i18n::current().code().into(),
            seed: None,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn build(self) -> anyhow::Result<Search> {
        self.build_with_progress(Progress::default())
    }

    pub fn build_with_progress(self, progress: Progress) -> anyhow::Result<Search> {
        #[cfg(any(target_arch = "wasm32", test))]
        crate::i18n::set(
            crate::i18n::Lang::from_locale(&self.language).unwrap_or(crate::i18n::Lang::En),
        );
        Search::new_with_progress(
            &self.project,
            Options {
                start: self.start.as_ref(),
                fixed: self.fixed,
                differ_from: self.differ_from.as_ref().map(|(a, n)| (a, *n)),
                budget: self.budget,
                runs: if cfg!(target_arch = "wasm32") {
                    1
                } else {
                    std::thread::available_parallelism().map_or(2, |n| n.get())
                },
                seed: self.seed,
            },
            progress,
        )
    }
}
