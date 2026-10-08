//! Building the graph from the project: people, wishes and how well each is met.

use std::collections::{HashMap, HashSet};

use super::{Edge, Graph};
use crate::model::{Project, RuleKind, Target, WishKind};
use crate::score::Outcome;

impl Graph {
    pub(super) fn build(project: &Project, candidate: Option<usize>, everyone: bool) -> Self {
        let evaluation = candidate
            .and_then(|i| project.candidates.get(i))
            .map(|c| crate::score::evaluate(project, &c.assignment));
        let outcome_of = |rule_id: u64| {
            evaluation
                .as_ref()?
                .results
                .iter()
                .find(|r| r.rule_id == rule_id)
                .map(|r| r.outcome())
        };
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
                    pairs.push((*requester, to, *wish == WishKind::Avec, outcome_of(rule.id)));
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
        let mut edges = Vec::new();
        for (from, to, avec, outcome) in pairs {
            let mutual = directed.contains(&(to, from));
            // Draw a mutual pair once.
            if mutual && from > to {
                continue;
            }
            if let (Some(&a), Some(&b)) = (index.get(&from), index.get(&to)) {
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
        for e in &edges {
            degree[e.from] += 1;
            degree[e.to] += 1;
        }
        let components = crate::groups::wish_groups(project);
        Self {
            groups: components.len(),
            largest_group: components.first().map_or(0, Vec::len),
            people,
            index,
            edges,
            degree,
        }
    }
}
