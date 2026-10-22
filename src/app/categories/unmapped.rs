//! Mapping imported entries to one or more categories, and suggested merges.

use super::*;
use crate::i18n::{fill, tr};

impl PlassiApp {
    /// Obvious merges (same letters, or an abbreviation) with one-click apply.
    pub(super) fn merge_suggestions_card(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        kind: Kind,
    ) -> bool {
        let project = &self.project;
        let suggestions = self
            .categories
            .merges
            .get((self.file.revision, kind), || {
                crate::catalog::merge_suggestions(project, kind)
            })
            .clone();
        if suggestions.is_empty() {
            return false;
        }
        let mut prepared = suggestions.clone();
        let mut ready = vec![true; suggestions.len()];
        for (i, merge) in prepared.iter_mut().enumerate() {
            for (part, alternatives) in merge.alternatives.iter().enumerate() {
                if alternatives.is_empty() {
                    continue;
                }
                let key = (kind.key().into(), merge.spelling.clone(), part);
                if let Some(choice) = self
                    .categories
                    .choices
                    .get(&key)
                    .filter(|c| alternatives.contains(c))
                {
                    merge.targets[part] = choice.clone();
                } else {
                    ready[i] = false;
                }
            }
        }
        let mut apply = None;
        let mut apply_all = false;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(tr("Suggested merges"))
                        .size(theme::text::LEAD)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    apply_all = ui.add_enabled(ready.iter().any(|r| *r), theme::primary_button(tr("Apply all"), p)).clicked();
                });
            });
            theme::hint(ui, p, tr("Similar names and combined entries."));
            if ready.iter().any(|r| !*r) { theme::hint(ui, p, tr("Choose ambiguous matches individually. Apply all skips unresolved choices.")); }
            ui.add_space(theme::space::S);
            for (index, merge) in prepared.iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&merge.spelling).strong());
                    let relation = if merge.targets.len() > 1 {
                        tr("belongs to")
                    } else {
                        tr("is the same as")
                    };
                    theme::hint(ui, p, relation);
                    for (part, target) in merge.targets.iter().enumerate() {
                        let existing = self.project.catalog.find(kind, target).is_some();
                        let label = fill(
                            if existing {
                                "{} · Existing"
                            } else {
                                "{} · New"
                            },
                            &[target],
                        );
                        theme::status_pill(ui, p, label, if existing { p.accent } else { p.good });
                        theme::hint(ui, p, merge.reasons[part].label());
                        let alternatives = &merge.alternatives[part];
                        if !alternatives.is_empty() {
                            let key = (kind.key().into(), merge.spelling.clone(), part);
                            let chosen = self.categories.choices.get(&key).cloned();
                            theme::combo(("match-choice", index, part), chosen.as_deref().unwrap_or(tr("Choose a match…"))).show_ui(ui, |ui| {
                                for choice in alternatives { if ui.action(chosen.as_ref() == Some(choice), choice) { self.categories.choices.insert(key.clone(), choice.clone()); } }
                            });
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(ready[index], theme::small_button(tr("Merge"))).clicked() {
                            apply = Some(index);
                        }
                    });
                });
            }
        });
        ui.add_space(theme::space::L);
        let chosen: Vec<_> = match (apply_all, apply) {
            (true, _) => prepared
                .into_iter()
                .enumerate()
                .filter(|(i, _)| ready[*i])
                .map(|(_, m)| m)
                .collect(),
            (false, Some(i)) => vec![prepared[i].clone()],
            _ => return false,
        };
        if chosen.is_empty() {
            return false;
        }
        let catalog = &mut self.project.catalog;
        for merge in chosen {
            for target in &merge.targets {
                let index = catalog.create(kind, target);
                catalog.assign(kind, &merge.spelling, index);
            }
        }
        true
    }

    /// Every imported spelling stays editable after its first mapping, so a
    /// combined entry can gain a second organization without finding aliases.
    pub(super) fn imported_entries(&mut self, ui: &mut egui::Ui, p: Palette, kind: Kind) -> bool {
        let spellings = self.project.spellings(kind);
        if spellings.is_empty() {
            return false;
        }
        let lookup = self.project.catalog.lookup(kind);
        let unmapped: Vec<_> = spellings
            .iter()
            .filter(|(s, _)| {
                lookup.find_all(s).is_empty() && !self.project.catalog.is_ignored(kind, s)
            })
            .collect();
        let mut changed = false;
        let mut create_all = false;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(tr("Imported entries"))
                        .size(theme::text::LEAD)
                        .strong(),
                );
                if !unmapped.is_empty() {
                    create_all = ui
                        .button(tr("Create all categories"))
                        .on_hover_text(tr("One category per entry"))
                        .clicked();
                }
            });
            ui.add_space(theme::space::S);
            let search_label = tr("Find imported entry");
            let response = ui.add(
                theme::field(&mut self.categories.mapping_search)
                    .hint_text(search_label)
                    .desired_width(ui.available_width()),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, search_label)
            });
            let search = crate::catalog::normalize(&self.categories.mapping_search);
            ui.horizontal_wrapped(|ui| {
                for (filter, label) in [
                    (MappingFilter::All, "All entries"),
                    (MappingFilter::Unresolved, "Unresolved"),
                    (MappingFilter::Resolved, "Resolved"),
                ] {
                    ui.selectable_value(&mut self.categories.mapping_filter, filter, tr(label));
                }
            });
            ui.add_space(theme::space::S);
            for (spelling, count) in &spellings {
                let ignored = self.project.catalog.is_ignored(kind, spelling);
                let resolved = ignored || !lookup.find_all(spelling).is_empty();
                if matches!(self.categories.mapping_filter, MappingFilter::Unresolved) && resolved
                    || matches!(self.categories.mapping_filter, MappingFilter::Resolved)
                        && !resolved
                {
                    continue;
                }
                if !crate::catalog::normalize(spelling).contains(&search) {
                    continue;
                }
                let row = ui.push_id(spelling, |ui| {
                    ui.add(egui::Label::new(RichText::new(spelling).strong()).wrap());
                    if ui
                        .link(people_count(*count))
                        .on_hover_text(tr("Show people with this entry"))
                        .clicked()
                    {
                        let key = crate::catalog::normalize(spelling);
                        let ids = self
                            .project
                            .attendees
                            .iter()
                            .filter(|a| crate::catalog::normalize(kind.raw(a)) == key)
                            .map(|a| a.id)
                            .collect();
                        self.categories.entry_people = Some((spelling.clone(), ids));
                    }
                    if ignored {
                        theme::status_pill(ui, p, tr("Ignored"), p.muted);
                        if ui.button(tr("Reopen")).clicked() {
                            self.project.catalog.ignore(kind, spelling, false);
                            changed = true;
                        }
                    } else {
                        let mapped = lookup.find_all(spelling);
                        changed |= mapping_picker(
                            ui,
                            p,
                            &mut self.project.catalog,
                            kind,
                            (spelling, mapped),
                        );
                        if mapped.is_empty()
                            && ui.add(theme::small_button(tr("Ignore entry"))).clicked()
                        {
                            self.project.catalog.ignore(kind, spelling, true);
                            changed = true;
                        }
                    }
                    ui.add_space(theme::space::S);
                    ui.separator();
                    ui.add_space(theme::space::S);
                });
                if self
                    .categories
                    .focus_spelling
                    .as_ref()
                    .is_some_and(|target| {
                        crate::catalog::normalize(target) == crate::catalog::normalize(spelling)
                    })
                {
                    row.response.scroll_to_me(Some(egui::Align::Center));
                    self.categories.focus_spelling = None;
                }
            }
        });
        ui.add_space(theme::space::L);
        if create_all {
            for (spelling, _) in unmapped {
                self.project.catalog.create(kind, spelling);
            }
            changed = true;
        }
        changed
    }
}

/// Checkboxes keep all existing memberships visible while adding another.
fn mapping_picker(
    ui: &mut egui::Ui,
    p: Palette,
    catalog: &mut crate::catalog::Catalog,
    kind: Kind,
    (spelling, mapped): (&str, &[usize]),
) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        if !catalog.list(kind).is_empty() {
            let label = if kind == Kind::Organization {
                tr("Choose organizations…")
            } else {
                tr("Choose categories…")
            };
            theme::combo("memberships", label)
                .width(ui.available_width().min(220.0))
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show_ui(ui, |ui| {
                    for index in catalog.sorted_indices(kind) {
                        let category = &catalog.list(kind)[index];
                        let mut selected = mapped.contains(&index);
                        let own_name = crate::catalog::normalize(&category.name)
                            == crate::catalog::normalize(spelling);
                        if ui.checkbox(!own_name, &mut selected, &category.name) {
                            if selected {
                                catalog.assign(kind, spelling, index);
                            } else {
                                catalog.unassign(kind, spelling, index);
                            }
                            changed = true;
                        }
                    }
                });
        }
        if mapped.is_empty()
            && ui
                .add(theme::small_button(tr("Create from entry")))
                .clicked()
        {
            catalog.create(kind, spelling);
            changed = true;
        }
    });
    let names: Vec<_> = mapped
        .iter()
        .filter_map(|&i| catalog.list(kind).get(i))
        .map(|c| c.name.clone())
        .collect();
    if !names.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for name in names {
                theme::status_pill(ui, p, name, p.accent);
            }
        });
    } else {
        theme::hint(ui, p, tr("Uncategorized"));
    }
    changed
}

pub(super) fn people_count(count: usize) -> String {
    if count == 1 {
        tr("1 person").into()
    } else {
        fill("{} people", &[&count])
    }
}
