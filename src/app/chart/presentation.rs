//! Attendee text, memberships and wish groups shared across chart frames.

use super::*;

pub(super) struct CategoryMetadata {
    pub(super) name: String,
    pub(super) keys: Vec<String>,
}

pub(super) struct CardMetadata {
    pub(super) attendee: usize,
    pub(super) categories: [CategoryMetadata; 4],
    pub(super) search_name: String,
    pub(super) search_affiliation: String,
}

pub(super) struct Presentation {
    pub(super) people: HashMap<usize, CardMetadata>,
}

impl Presentation {
    pub(super) fn new(project: &Project) -> Self {
        let lookups = Kind::ALL.map(|kind| project.catalog.lookup(kind));
        Self {
            people: project
                .attendees
                .iter()
                .enumerate()
                .map(|(index, attendee)| {
                    let categories = Kind::ALL.map(|kind| {
                        let raw = kind.raw(attendee);
                        let found = lookups[kind as usize].find_all(raw);
                        let list = project.catalog.list(kind);
                        let (name, keys) = if found.is_empty() {
                            let key = crate::catalog::normalize(raw);
                            (
                                raw.trim().to_string(),
                                if key.is_empty() {
                                    Vec::new()
                                } else {
                                    vec![key]
                                },
                            )
                        } else {
                            (
                                found
                                    .iter()
                                    .map(|&i| list[i].name.as_str())
                                    .collect::<Vec<_>>()
                                    .join(" / "),
                                found
                                    .iter()
                                    .map(|&i| {
                                        format!("#{}", crate::catalog::normalize(&list[i].name))
                                    })
                                    .collect(),
                            )
                        };
                        CategoryMetadata { name, keys }
                    });
                    (
                        attendee.id,
                        CardMetadata {
                            attendee: index,
                            categories,
                            search_name: attendee.name.to_lowercase(),
                            search_affiliation: project.affiliation(attendee).to_lowercase(),
                        },
                    )
                })
                .collect(),
        }
    }
}

pub(super) struct WishGroups {
    pub(super) groups: Vec<std::rc::Rc<Vec<usize>>>,
    pub(super) membership: HashMap<usize, usize>,
}

impl WishGroups {
    pub(super) fn new(project: &Project) -> Self {
        let groups: Vec<_> = crate::groups::wish_groups(project)
            .into_iter()
            .map(std::rc::Rc::new)
            .collect();
        let membership = groups
            .iter()
            .enumerate()
            .flat_map(|(i, group)| group.iter().map(move |&person| (person, i)))
            .collect();
        Self { groups, membership }
    }
}
