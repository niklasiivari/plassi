//! Spellings that don't belong to a category yet, and suggested merges.

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
        let suggestions = crate::catalog::merge_suggestions(&self.project, kind);
        if suggestions.is_empty() {
            return false;
        }
        let mut apply = None;
        let mut apply_all = false;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(tr("Suggested merges"))
                        .size(theme::text::LEAD)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    apply_all = ui.add(theme::primary_button(tr("Apply all"), p)).clicked();
                });
            });
            theme::hint(
                ui,
                p,
                tr("Spellings that look like something already known, or several things in one."),
            );
            ui.add_space(theme::space::S);
            for (index, merge) in suggestions.iter().enumerate() {
                ui.horizontal(|ui| {
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

    /// Spellings with no category, each with "new category" or "add to …".
    pub(super) fn unmapped_card(&mut self, ui: &mut egui::Ui, p: Palette, kind: Kind) -> bool {
        let unmapped: Vec<(String, usize)> = self
            .project
            .spellings(kind)
            .into_iter()
            .filter(|(s, _)| self.project.catalog.find(kind, s).is_none())
            .collect();
        if unmapped.is_empty() {
            return false;
        }
        let names: Vec<String> = self
            .project
            .catalog
            .list(kind)
            .iter()
            .map(|c| c.name.clone())
            .collect();
        let mut action: Option<(String, Option<usize>)> = None;
        let mut create_all = false;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(tr("Not mapped yet"))
                        .size(theme::text::LEAD)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    create_all = ui
                        .button(tr("Make each its own category"))
                        .on_hover_text(tr("Create a category for every spelling below as written"))
                        .clicked();
                });
            });
            ui.add_space(theme::space::S);
            for (spelling, count) in &unmapped {
                if let Some(chosen) = unmapped_row(ui, p, spelling, *count, &names) {
                    action = Some((spelling.clone(), chosen));
                }
            }
        });
        ui.add_space(theme::space::L);
        let catalog = &mut self.project.catalog;
        if create_all {
            for (spelling, _) in &unmapped {
                catalog.create(kind, spelling);
            }
            return true;
        }
        match action {
            Some((spelling, None)) => {
                catalog.create(kind, &spelling);
                true
            }
            Some((spelling, Some(index))) => {
                catalog.assign(kind, &spelling, index);
                true
            }
            None => false,
        }
    }
}

/// One spelling: name and count on the left, actions on the right. Returns
/// `Some(None)` for "new category" and `Some(Some(i))` for "add to category i".
fn unmapped_row(
    ui: &mut egui::Ui,
    p: Palette,
    spelling: &str,
    count: usize,
    names: &[String],
) -> Option<Option<usize>> {
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.set_min_height(theme::CONTROL_HEIGHT);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !names.is_empty() {
                egui::ComboBox::from_id_salt(("map", spelling))
                    .selected_text(tr("Add to…"))
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        for (i, name) in names.iter().enumerate() {
                            if ui.selectable_label(false, name).clicked() {
                                chosen = Some(Some(i));
                            }
                        }
                    });
            }
            if ui
                .add(theme::small_button(tr("New")))
                .on_hover_text(tr("Make this spelling its own category"))
                .clicked()
            {
                chosen = Some(None);
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(RichText::new(spelling).strong()).truncate());
                theme::hint(ui, p, people_count(count));
            });
        });
    });
    chosen
}

pub(super) fn people_count(count: usize) -> String {
    if count == 1 {
        tr("1 person").into()
    } else {
        fill("{} people", &[&count])
    }
}
