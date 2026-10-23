//! Compiled scoring inputs shared by evaluations and interactive previews.

use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use crate::{model::Project, score, seating::Scorer};

#[derive(Default)]
pub(super) struct ScoringCache {
    checked: Option<(u64, &'static str)>,
    inputs: Vec<u8>,
    scorer: Option<Rc<Scorer>>,
    evaluations: HashMap<usize, Evaluated>,
}

struct Evaluated {
    revision: u64,
    assignment: BTreeMap<usize, String>,
    value: Rc<score::Evaluation>,
}

impl ScoringCache {
    pub(super) fn scorer(&mut self, project: &Project, revision: u64) -> Rc<Scorer> {
        let language = crate::i18n::current().code();
        if self.checked != Some((revision, language)) {
            // Exact serialized equality avoids hash collisions. Options, import
            // notes and event/view metadata do not change the compiled problem.
            let inputs = serde_json::to_vec(&(
                &project.attendees,
                &project.layout,
                &project.rules,
                &project.policy,
                &project.catalog,
                &project.preferences,
                language,
            ))
            .expect("serializable scoring inputs");
            if self.scorer.is_none() || inputs != self.inputs {
                self.scorer = Some(Rc::new(Scorer::new(project)));
                self.inputs = inputs;
                self.evaluations.clear();
            }
            self.checked = Some((revision, language));
        }
        self.scorer.as_ref().expect("compiled scorer").clone()
    }

    pub(super) fn evaluation(
        &mut self,
        project: &Project,
        revision: u64,
        option: usize,
    ) -> Option<Rc<score::Evaluation>> {
        let candidate = project.candidates.get(option)?;
        let scorer = self.scorer(project, revision);
        self.evaluations
            .retain(|index, _| *index < project.candidates.len());
        if let Some(saved) = self.evaluations.get_mut(&option)
            && (saved.revision == revision || saved.assignment == candidate.assignment)
        {
            saved.revision = revision;
            return Some(saved.value.clone());
        }
        let value = Rc::new(score::evaluate_with(
            &scorer,
            project,
            &candidate.assignment,
        ));
        self.evaluations.insert(
            option,
            Evaluated {
                revision,
                assignment: candidate.assignment.clone(),
                value: value.clone(),
            },
        );
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_and_option_changes_reuse_compilation() {
        let mut project = crate::tests::two_tables();
        let mut cache = ScoringCache::default();
        let initial = cache.scorer(&project, 0);
        project.event.title = "<event>".into();
        project.shown_option = 1;
        assert!(Rc::ptr_eq(&initial, &cache.scorer(&project, 1)));
        project.policy = crate::model::SeatingPolicy::default();
        project.rules.clear();
        // Roster changes always invalidate compiled person indices.
        project.attendees.pop();
        assert!(!Rc::ptr_eq(&initial, &cache.scorer(&project, 2)));
    }
    #[test]
    fn option_evaluations_survive_view_changes_and_invalidate_independently() {
        let mut project = crate::tests::two_tables();
        let seats = project.layout.seats();
        let assignment: BTreeMap<_, _> = project
            .attendees
            .iter()
            .zip(&seats)
            .map(|(p, s)| (p.id, s.id.clone()))
            .collect();
        let option = crate::model::Candidate {
            name: "Option 1".into(),
            assignment,
            solver_status: crate::model::SolverStatus::EditedByHand,
            hand_placed: Default::default(),
            loose: Default::default(),
        };
        project.candidates = vec![option.clone(), option];
        let mut cache = ScoringCache::default();
        let initial = cache.scorer(&project, 0);
        let first = cache.evaluation(&project, 0, 0).unwrap();
        let second = cache.evaluation(&project, 0, 1).unwrap();
        project.shown_option = 1;
        project.event.title = "<event>".into();
        assert!(Rc::ptr_eq(
            &first,
            &cache.evaluation(&project, 1, 0).unwrap()
        ));
        assert!(Rc::ptr_eq(
            &second,
            &cache.evaluation(&project, 1, 1).unwrap()
        ));
        project.candidates[0].assignment.remove(&0);
        assert!(!Rc::ptr_eq(
            &first,
            &cache.evaluation(&project, 2, 0).unwrap()
        ));
        assert!(Rc::ptr_eq(
            &second,
            &cache.evaluation(&project, 2, 1).unwrap()
        ));
        assert!(Rc::ptr_eq(&initial, &cache.scorer(&project, 2)));
        project.attendees[0].name = "X".into();
        assert!(!Rc::ptr_eq(&initial, &cache.scorer(&project, 3)));
        assert!(!Rc::ptr_eq(
            &second,
            &cache.evaluation(&project, 3, 1).unwrap()
        ));
    }
}
