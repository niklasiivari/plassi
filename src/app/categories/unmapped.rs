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
                    apply_all = ui.add(theme::primary_button(tr("Apply all"), p)).clicked();
                });
            });
            theme::hint(ui, p, tr("Similar names and combined entries."));
            ui.add_space(theme::space::S);
            for (index, merge) in suggestions.iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&merge.spelling).strong());
                    let relation = if merge.targets.len() > 1 {
                        tr("belongs to")
                    } else {
                        tr("is the same as")
                    };
                    theme::hint(ui, p, relation);
                    ui.label(merge.targets.join(tr(" and ")));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(theme::small_button(tr("Merge"))).clicked() {
                            apply = Some(index);
                        }
                    });
                });
            }
        });
        ui.add_space(theme::space::L);
        let chosen: Vec<_> = match (apply_all, apply) {
            (true, _) => suggestions,
            (false, Some(i)) => vec![suggestions[i].clone()],
            _ => return false,
        };
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
            .filter(|(s, _)| lookup.find_all(s).is_empty())
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
            ui.add_space(theme::space::S);
            for (spelling, count) in &spellings {
                if !crate::catalog::normalize(spelling).contains(&search) {
                    continue;
                }
                ui.push_id(spelling, |ui| {
                    ui.add(egui::Label::new(RichText::new(spelling).strong()).wrap());
                    theme::hint(ui, p, people_count(*count));
                    let mapped = lookup.find_all(spelling);
                    changed |=
                        mapping_picker(ui, p, &mut self.project.catalog, kind, (spelling, mapped));
                    ui.add_space(theme::space::S);
                    ui.separator();
                    ui.add_space(theme::space::S);
                });
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
                    for index in 0..catalog.list(kind).len() {
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
