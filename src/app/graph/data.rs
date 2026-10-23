//! Building the graph from the project: people, wishes and how well each is met.

use std::collections::{HashMap, HashSet};

use super::{Edge, Graph};
use crate::model::{Project, RuleKind, Target, WishKind};
use crate::score::Outcome;

impl Graph {
    pub(super) fn build(
        project: &Project,
        evaluation: Option<&crate::score::Evaluation>,
        everyone: bool,
    ) -> Self {
        let outcomes: HashMap<_, _> = evaluation
            .into_iter()
            .flat_map(|e| &e.results)
            .map(|r| (r.rule_id, r.outcome()))
            .collect();
        let mut pairs: Vec<(usize, usize, bool, Option<Outcome>)> = Vec::new();
        for rule in project.rules.iter().filter(|r| r.enabled) {
            if let RuleKind::Proximity {
                requester,
                target: Target::People { ids, .. },
                wish,
                ..
            } = &rule.kind
            {
                for &to in ids {
                    pairs.push((
                        *requester,
                        to,
                        *wish == WishKind::Avec,
                        outcomes.get(&rule.id).copied(),
                    ));
                }
            }
        }
        let linked: HashSet<usize> = pairs.iter().flat_map(|(a, b, _, _)| [*a, *b]).collect();
        let people: Vec<usize> = project
            .attendees
            .iter()
            .map(|a| a.id)
            .filter(|id| everyone || linked.contains(id))
            .collect();
        let index: HashMap<usize, usize> =
            people.iter().enumerate().map(|(i, p)| (*p, i)).collect();
        let directed: HashSet<(usize, usize)> = pairs.iter().map(|(a, b, _, _)| (*a, *b)).collect();
        let mut edges: Vec<Edge> = Vec::new();
        let mut connections = HashMap::new();
        for (from, to, avec, outcome) in pairs {
            let mutual = directed.contains(&(to, from));
            let key = (from.min(to), from.max(to));
            if let Some(&existing) = connections.get(&key) {
                let edge: &mut Edge = &mut edges[existing];
                // A mutual connection represents both wishes. Do not hide the
                // stricter direction or an Avec because of numeric person IDs.
                edge.avec |= avec;
                edge.outcome = edge.outcome.max(outcome);
                continue;
            }
            if let (Some(&a), Some(&b)) = (index.get(&from), index.get(&to)) {
                connections.insert(key, edges.len());
                edges.push(Edge {
                    from: a,
                    to: b,
                    avec,
                    mutual,
                    outcome,
                });
            }
        }
        let mut degree = vec![0; people.len()];
        let mut neighbours = vec![HashSet::new(); people.len()];
        for e in &edges {
            degree[e.from] += 1;
            degree[e.to] += 1;
            neighbours[e.from].insert(e.to);
            neighbours[e.to].insert(e.from);
        }
        let attendee_indexes: Vec<_> = project
            .attendees
            .iter()
            .enumerate()
            .filter(|(_, a)| index.contains_key(&a.id))
            .map(|(i, _)| i)
            .collect();
        let color_keys = attendee_indexes
            .iter()
            .map(|&i| {
                project.category_key(crate::catalog::Kind::Organization, &project.attendees[i])
            })
            .collect();
        // The summary describes only connections actually drawn. Organizer
        // group preferences are not edges in this view.
        let mut visited = vec![false; people.len()];
        let (mut groups, mut largest_group) = (0, 0);
        for first in 0..people.len() {
            if visited[first] {
                continue;
            }
            let mut stack = vec![first];
            let mut members = 0;
            while let Some(person) = stack.pop() {
                if std::mem::replace(&mut visited[person], true) {
                    continue;
                }
                members += 1;
                stack.extend(neighbours[person].iter().copied());
            }
            if members > 1 {
                groups += 1;
                largest_group = largest_group.max(members);
            }
        }
        Self {
            groups,
            largest_group,
            people,
            index,
            edges,
            degree,
            attendee_indexes,
            color_keys,
            neighbours,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Candidate, DistanceClass, Importance, Layout, SolverStatus};

    fn pair(first: usize, second: usize) -> Project {
        let mut project = Project::new();
        project.layout = Layout::simple(2, 1);
        project.attendees = vec![
            serde_json::from_value(serde_json::json!({
                "id": first, "name": "X", "organization": "ABC", "participant_group": "", "tags": []
            }))
            .unwrap(),
            serde_json::from_value(serde_json::json!({
                "id": second, "name": "Y", "organization": "XYZ", "participant_group": "", "tags": []
            }))
            .unwrap(),
        ];
        project.rules = vec![
            crate::model::Rule {
                id: 1,
                label: "Seating wish".into(),
                source: String::new(),
                enabled: true,
                kind: RuleKind::Proximity {
                    requester: first,
                    target: Target::People { ids: vec![second] },
                    wish: WishKind::Seating,
                    importance: Importance::Normal,
                    max_class: None,
                },
            },
            crate::model::Rule {
                id: 2,
                label: "Avec".into(),
                source: String::new(),
                enabled: true,
                kind: RuleKind::Proximity {
                    requester: second,
                    target: Target::People { ids: vec![first] },
                    wish: WishKind::Avec,
                    importance: Importance::Normal,
                    max_class: Some(DistanceClass::Beside),
                },
            },
        ];
        project.candidates.push(Candidate {
            name: "Option 1".into(),
            assignment: [
                (first, "table-1:left:0".into()),
                (second, "table-2:left:0".into()),
            ]
            .into_iter()
            .collect(),
            solver_status: SolverStatus::EditedByHand,
            hand_placed: Default::default(),
            loose: Default::default(),
        });
        project
    }

    #[test]
    fn mutual_connections_keep_the_strictest_wish_independent_of_ids_and_rule_order() {
        for ids in [(0, 1), (1, 0)] {
            for reverse in [false, true] {
                let mut project = pair(ids.0, ids.1);
                if reverse {
                    project.rules.reverse();
                }
                let evaluation =
                    crate::score::evaluate(&project, &project.candidates[0].assignment);
                assert_eq!(evaluation.hard_violations, 1);
                let graph = Graph::build(&project, Some(&evaluation), false);
                assert_eq!(graph.edges.len(), 1);
                let edge = &graph.edges[0];
                assert!(edge.mutual);
                assert!(edge.avec);
                assert_eq!(edge.outcome, Some(Outcome::Broken));
                assert_eq!(graph.groups, 1);
                assert_eq!(graph.largest_group, 2);
            }
        }
    }

    #[test]
    fn group_summary_counts_only_connections_shown_in_the_graph() {
        let mut project = pair(0, 1);
        project.rules.clear();
        project.preferences.push(crate::preferences::Preference {
            id: 1,
            enabled: true,
            strength: crate::preferences::Strength::Important,
            kind: crate::preferences::PreferenceKind::Together { people: vec![0, 1] },
        });
        let graph = Graph::build(&project, None, true);
        assert!(graph.edges.is_empty());
        assert_eq!(graph.groups, 0);
        assert_eq!(graph.largest_group, 0);
    }
}
