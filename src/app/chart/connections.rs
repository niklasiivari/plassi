//! Wish relationships resolved once with the scene, independent of pointer movement.
use std::collections::{HashMap, HashSet};

pub(super) struct WishConnection {
    pub(super) rule: u64,
    pub(super) requester: usize,
    pub(super) targets: Vec<usize>,
}

pub(super) struct WishConnections {
    wishes: Vec<WishConnection>,
    outgoing: HashMap<usize, Vec<usize>>,
    incoming: HashMap<usize, Vec<usize>>,
}

impl WishConnections {
    pub(super) fn new(scorer: &crate::seating::Scorer) -> Self {
        let wishes: Vec<_> = scorer
            .resolved_wishes()
            .map(|(rule, requester, targets)| WishConnection {
                rule,
                requester,
                targets,
            })
            .collect();
        let mut outgoing: HashMap<_, Vec<_>> = HashMap::new();
        let mut incoming: HashMap<_, Vec<_>> = HashMap::new();
        for (index, wish) in wishes.iter().enumerate() {
            outgoing.entry(wish.requester).or_default().push(index);
            let mut seen = HashSet::new();
            for &person in &wish.targets {
                if seen.insert(person) {
                    incoming.entry(person).or_default().push(index);
                }
            }
        }
        Self {
            wishes,
            outgoing,
            incoming,
        }
    }

    pub(super) fn outgoing(&self, person: usize) -> impl Iterator<Item = &WishConnection> {
        self.outgoing
            .get(&person)
            .into_iter()
            .flatten()
            .map(|&i| &self.wishes[i])
    }

    pub(super) fn incoming(&self, person: usize) -> impl Iterator<Item = &WishConnection> {
        self.incoming
            .get(&person)
            .into_iter()
            .flatten()
            .map(|&i| &self.wishes[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Kind;
    use crate::model::{Project, RuleKind, Target, WishKind};

    fn check(project: &Project) {
        let connections = WishConnections::new(&crate::seating::Scorer::new(project));
        for person in &project.attendees {
            let mut incoming = Vec::new();
            let mut outgoing = Vec::new();
            for rule in project.rules.iter().filter(|r| r.enabled) {
                let RuleKind::Proximity {
                    requester, target, ..
                } = &rule.kind
                else {
                    continue;
                };
                let targets = project.target_ids(*requester, target);
                if *requester == person.id {
                    outgoing.push(rule.id);
                    assert_eq!(
                        connections
                            .outgoing(person.id)
                            .find(|w| w.rule == rule.id)
                            .unwrap()
                            .targets,
                        targets
                    );
                }
                if targets.contains(&person.id) {
                    incoming.push(rule.id);
                }
            }
            assert_eq!(
                connections
                    .incoming(person.id)
                    .map(|w| w.rule)
                    .collect::<Vec<_>>(),
                incoming
            );
            assert_eq!(
                connections
                    .outgoing(person.id)
                    .map(|w| w.rule)
                    .collect::<Vec<_>>(),
                outgoing
            );
        }
    }

    #[test]
    fn resolved_connections_follow_aliases_tags_disabled_rules_and_membership_edits() {
        let mut project = crate::example::project();
        project.attendees.truncate(6);
        project.rules.clear();
        project.notes.clear();
        project.catalog = Default::default();
        for (id, person) in project.attendees.iter_mut().enumerate() {
            person.organization = ["ABC", "AB-C", "ABC / XYZ", "XYZ", "XYZ", "ABC"][id].into();
            person.tags = if id % 2 == 0 {
                vec!["X".into()]
            } else {
                vec![]
            };
        }
        let abc = project
            .catalog
            .create_named(Kind::Organization, "ABC")
            .unwrap();
        let xyz = project
            .catalog
            .create_named(Kind::Organization, "XYZ")
            .unwrap();
        project.catalog.assign(Kind::Organization, "AB-C", abc);
        for category in [abc, xyz] {
            project
                .catalog
                .assign(Kind::Organization, "ABC / XYZ", category);
        }
        for (id, target) in [
            Target::Organization { name: "ABC".into() },
            Target::Organization {
                name: "AB-C".into(),
            },
            Target::Tag {
                tag: "X".into(),
                required: 1,
            },
            Target::People { ids: vec![0, 1, 2] },
        ]
        .into_iter()
        .enumerate()
        {
            let mut rule =
                crate::import::wish_rule(id as u64 + 1, 0, vec![], WishKind::Seating, "");
            if let RuleKind::Proximity { target: t, .. } = &mut rule.kind {
                *t = target;
            }
            project.rules.push(rule);
        }
        check(&project);
        project.rules[1].enabled = false;
        project.catalog.organizations[abc]
            .aliases
            .retain(|a| a != "AB-C");
        project.catalog.assign(Kind::Organization, "AB-C", xyz);
        project.attendees[5].organization = "XYZ".into();
        project.attendees[1].tags.push("X".into());
        check(&project);
    }
}
