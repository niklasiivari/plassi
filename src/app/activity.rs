//! A searchable change log; following a link never changes project data.
use super::*;
use crate::i18n::{fill, tr};
use crate::model::{ActivityTarget, RuleKind};

#[derive(Default)]
pub(super) struct ActivityState {
    pub(super) open: bool,
    search: String,
}

fn differs(a: &impl serde::Serialize, b: &impl serde::Serialize) -> bool {
    serde_json::to_value(a).ok() != serde_json::to_value(b).ok()
}

pub(super) fn describe(
    before: &Project,
    after: &Project,
    option: usize,
    fallback: &str,
) -> (String, Option<ActivityTarget>) {
    let changed_notes: std::collections::BTreeMap<_, _> = after
        .notes
        .iter()
        .filter(|note| {
            before
                .notes
                .iter()
                .find(|n| n.person == note.person && n.field == note.field && n.text == note.text)
                .is_some_and(|old| old.reviewed != note.reviewed)
        })
        .map(|note| (note.person, note.reviewed))
        .collect();
    if changed_notes.len() == 1 {
        let (&id, &reviewed) = changed_notes.first_key_value().unwrap();
        return (
            fill(
                if reviewed {
                    "Reviewed {}"
                } else {
                    "Reopened notes for {}"
                },
                &[&after.person_name(id)],
            ),
            Some(ActivityTarget::Person { id }),
        );
    } else if !changed_notes.is_empty() {
        return (
            fill("Updated notes for {} people", &[&changed_notes.len()]),
            Some(ActivityTarget::People {
                ids: changed_notes.into_keys().collect(),
            }),
        );
    }
    if before.catalog != after.catalog {
        for kind in crate::catalog::Kind::ALL {
            if before.catalog.list(kind) != after.catalog.list(kind)
                || before.catalog.ignored.get(kind.key()) != after.catalog.ignored.get(kind.key())
            {
                let mappings: Vec<_> = after
                    .catalog
                    .list(kind)
                    .iter()
                    .flat_map(|category| {
                        let old = before
                            .catalog
                            .list(kind)
                            .iter()
                            .find(|c| c.name == category.name);
                        category
                            .aliases
                            .iter()
                            .filter(move |s| old.is_none_or(|c| !c.aliases.contains(s)))
                            .map(move |alias| (alias, &category.name))
                    })
                    .collect();
                if mappings.len() == 1 {
                    let (alias, category) = mappings[0];
                    return (
                        fill("Mapped {} to {}", &[alias, category]),
                        Some(ActivityTarget::Category {
                            category_kind: kind.key().into(),
                            spelling: alias.clone(),
                        }),
                    );
                } else if !mappings.is_empty() {
                    return (
                        fill("Updated {} category mappings", &[&mappings.len()]),
                        Some(ActivityTarget::Category {
                            category_kind: kind.key().into(),
                            spelling: String::new(),
                        }),
                    );
                }
                let spelling = after
                    .catalog
                    .ignored
                    .get(kind.key())
                    .into_iter()
                    .flatten()
                    .find(|s| !before.catalog.is_ignored(kind, s))
                    .cloned()
                    .unwrap_or_default();
                return (
                    fill("Updated {}", &[&kind.plural()]),
                    Some(ActivityTarget::Category {
                        category_kind: kind.key().into(),
                        spelling,
                    }),
                );
            }
        }
    }
    if differs(&before.rules, &after.rules) {
        let owners: std::collections::BTreeSet<_> = after
            .rules
            .iter()
            .filter(|r| {
                before
                    .rules
                    .iter()
                    .find(|o| o.id == r.id)
                    .is_none_or(|o| differs(o, *r))
            })
            .chain(
                before
                    .rules
                    .iter()
                    .filter(|r| !after.rules.iter().any(|n| n.id == r.id)),
            )
            .filter_map(|r| match r.kind {
                RuleKind::Proximity { requester, .. } => Some(requester),
                _ => None,
            })
            .collect();
        if owners.len() == 1 {
            let id = *owners.first().unwrap();
            return (
                fill("Updated wishes for {}", &[&after.person_name(id)]),
                Some(ActivityTarget::Person { id }),
            );
        }
    }
    for person in &after.attendees {
        if let Some(old) = before.person(person.id)
            && differs(old, person)
        {
            return (
                fill("Updated {}", &[&person.name]),
                Some(ActivityTarget::Person { id: person.id }),
            );
        }
    }
    let old_frames = before.layout.table_frames();
    let ids: Vec<_> = after
        .layout
        .table_frames()
        .into_iter()
        .filter(|(id, at)| old_frames.get(id) != Some(at))
        .map(|(id, _)| id)
        .collect();
    if !ids.is_empty() {
        return (
            fill("Updated {} tables", &[&ids.len()]),
            Some(ActivityTarget::Tables { ids }),
        );
    }
    if before
        .candidates
        .get(option)
        .zip(after.candidates.get(option))
        .is_some_and(|(a, b)| differs(a, b))
    {
        return (
            fallback.into(),
            Some(ActivityTarget::Option { index: option }),
        );
    }
    (fallback.into(), None)
}

impl PlassiApp {
    pub(super) fn activity_window(&mut self, ctx: &egui::Context, p: Palette) {
        if !self.activity.open {
            return;
        }
        let mut open = true;
        let mut target = None;
        egui::Window::new(tr("Activity history"))
            .open(&mut open)
            .default_width(480.0)
            .max_width((ctx.content_rect().width() - 32.0).max(1.0))
            .max_height((ctx.content_rect().height() - 48.0).max(100.0))
            .show(ctx, |ui| {
                ui.add(
                    theme::field(&mut self.activity.search)
                        .hint_text(tr("Find a change"))
                        .desired_width(ui.available_width()),
                );
                theme::hint(
                    ui,
                    p,
                    tr("Recent changes are saved with this project. Open an item to return to it."),
                );
                let needle = search::Needle::new(&self.activity.search);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.project.activity.is_empty() {
                        theme::hint(ui, p, tr("No changes recorded yet"));
                    }
                    for entry in self
                        .project
                        .activity
                        .iter()
                        .rev()
                        .filter(|a| needle.matches(&a.message))
                    {
                        ui.push_id(entry.id, |ui| {
                            if let Some(link) = &entry.target {
                                if ui.add(egui::Button::new(&entry.message).wrap()).clicked() {
                                    target = Some(link.clone());
                                }
                            } else {
                                ui.add(egui::Label::new(&entry.message).wrap());
                            }
                            let minutes = platform::now_stamp().saturating_sub(entry.at) / 60_000;
                            theme::hint(
                                ui,
                                p,
                                if minutes == 0 {
                                    tr("Just now").into()
                                } else {
                                    fill("{} minutes ago", &[&minutes])
                                },
                            );
                            ui.separator();
                        });
                    }
                });
            });
        if let Some(target) = target {
            self.follow_activity(target);
            open = false;
        }
        self.activity.open = open;
    }

    fn follow_activity(&mut self, target: ActivityTarget) {
        match target {
            ActivityTarget::Person { id } if self.project.person(id).is_some() => {
                self.page = Page::People;
                self.selected_person = Some(id);
                self.people.wish_draft = None;
                self.people.scroll_to_person = true;
                self.people.end_review();
                self.people.clear_import();
                self.people.roster = None;
            }
            ActivityTarget::People { ids }
                if ids.iter().any(|id| self.project.person(*id).is_some()) =>
            {
                self.page = Page::People;
                self.selected_person = None;
                self.people.clear_import();
                self.people.end_review();
                self.people.roster = None;
                self.people.search.clear();
                self.people.filter = people::PeopleFilter::All;
                self.people.picked = ids
                    .into_iter()
                    .filter(|id| self.project.person(*id).is_some())
                    .collect();
                self.people.selecting = true;
            }
            ActivityTarget::Category {
                category_kind,
                spelling,
            } => {
                if let Some(kind) = crate::catalog::Kind::from_key(&category_kind) {
                    self.page = Page::Categories;
                    self.categories.focus_entry(kind, &spelling);
                }
            }
            ActivityTarget::Tables { ids }
                if ids.iter().any(|id| self.project.layout.table(id).is_some()) =>
            {
                self.page = Page::Room;
                self.room.focus_tables(&ids);
            }
            ActivityTarget::Option { index } if index < self.project.candidates.len() => {
                self.page = Page::Chart;
                self.selected_candidate = index;
                self.chart.view.fit_pending = true;
            }
            _ => {
                self.status = tr("This item is no longer in the project").into();
            }
        }
    }
}
