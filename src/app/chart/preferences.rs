//! The planner's own preferences on the chart: pick several people
//! (Ctrl/Shift-click), then group them or keep them apart; list and edit
//! preferences in the side panel.

use super::*;
use crate::preferences::{Closeness, PreferenceKind, Strength};
use crate::score::PreferenceResult;
use super::inspector::outcome_row;

impl PlassiApp {
    /// Floating bar over the canvas while two or more people are picked.
    pub(super) fn preference_bar(&mut self, ui: &mut egui::Ui, p: Palette, rect: Rect) {
        let picked = self.chart.multi.len();
        if picked < 2 {
            return;
        }
        let size = Vec2::new(560.0_f32.min(rect.width() - 2.0 * theme::space::L), 52.0);
        let area = Rect::from_center_size(Pos2::new(rect.center().x, rect.max.y - theme::space::XL - size.y / 2.0), size);
        let mut chosen: Option<PreferenceKind> = None;
        let mut clear = false;
        ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
            theme::floating(p).inner_margin(egui::Margin::symmetric(theme::space::L as i8, theme::space::S as i8)).show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new(fill("{} people", &[&picked])).strong());
                    ui.add_space(theme::space::S);
                    let people: Vec<usize> = self.chart.multi.iter().copied().collect();
                    if ui.add(theme::primary_button(tr("Group together"), p)).on_hover_text(tr("As if each of them wished for every other (G)")).clicked() {
                        chosen = Some(PreferenceKind::together(people.clone()));
                    }
                    ui.menu_button(tr("Keep apart"), |ui| {
                        if ui.button(tr("Not at the same table")).clicked() {
                            chosen = Some(PreferenceKind::Apart { people: people.clone(), closeness: Closeness::Table });
                            ui.close();
                        }
                        if ui.button(tr("Not beside or opposite")).clicked() {
                            chosen = Some(PreferenceKind::Apart { people: people.clone(), closeness: Closeness::Neighbours });
                            ui.close();
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        clear = theme::icon_button(ui, p, theme::Icon::Close, tr("Clear selection (Esc)"), false).clicked();
                    });
                });
            });
        });
        if let Some(kind) = chosen {
            self.project.add_preference(kind, Strength::Important);
            self.chart.multi.clear();
            self.commit(tr("Added your preference; solve or improve to apply it"));
        } else if clear {
            self.chart.multi.clear();
        }
    }

    /// "Your preferences" in the side panel, each with its result and controls.
    pub(super) fn preferences_section(&mut self, ui: &mut egui::Ui, p: Palette, results: &[PreferenceResult]) {
        ui.add_space(theme::space::XL);
        theme::section_title(ui, p, tr("Your preferences"));
        if self.project.preferences.is_empty() {
            theme::hint(ui, p, tr("Ctrl-click several people on the chart to group them or keep them apart."));
            return;
        }
        self.chart.preference_preview = None;
        let mut changed = false;
        let mut delete = None;
        for index in 0..self.project.preferences.len() {
            let id = self.project.preferences[index].id;
            let row = self.preference_row(ui, p, index, results.iter().find(|r| r.id == id));
            changed |= row.changed;
            if row.delete {
                delete = Some(index);
            }
            if row.hovered {
                self.chart.preference_preview = Some(id);
            }
        }
        if let Some(index) = delete {
            self.project.preferences.remove(index);
            changed = true;
        }
        if changed {
            self.commit(tr("Saved your preferences"));
        }
    }

    /// One preference: who, how it fares, strength and delete.
    fn preference_row(&mut self, ui: &mut egui::Ui, p: Palette, index: usize, result: Option<&PreferenceResult>) -> RowAction {
        let names = self.project.names_of(self.project.preferences[index].kind.people(), None);
        let preference = &mut self.project.preferences[index];
        let title = format!("{}: {names}", kind_title(&preference.kind));
        let color = theme::outcome_color(p, result.map(PreferenceResult::outcome));
        let mut action = RowAction::default();
        let response = theme::card(p)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    theme::status_dot(ui, color);
                    ui.add(egui::Label::new(RichText::new(title).strong()).wrap());
                });
                if let Some(result) = result {
                    theme::hint(ui, p, &result.detail);
                }
                ui.horizontal(|ui| {
                    theme::combo(("strength", preference.id), preference.strength.label())
                        .show_ui(ui, |ui| {
                            for strength in Strength::ALL {
                                action.changed |= ui.selectable_value(&mut preference.strength, strength, strength.label()).changed();
                            }
                        });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        action.delete = ui.add(theme::ghost_button(tr("Delete"), p)).clicked();
                    });
                });
                if let PreferenceKind::Together { costs, .. } = &mut preference.kind {
                    ui.push_id(preference.id, |ui| {
                        action.changed |= crate::app::people::costs_editor(ui, p, costs);
                    });
                }
            })
            .response;
        action.hovered = response.contains_pointer();
        action
    }

    /// The planner's preferences this person is part of, read-only.
    pub(super) fn person_preferences(&self, ui: &mut egui::Ui, p: Palette, scene: &Scene, id: usize) {
        let results = scene.evaluation.as_ref().map_or(&[][..], |e| e.preferences.as_slice());
        let mut mine = self.project.preferences_of(id).peekable();
        if mine.peek().is_none() {
            return;
        }
        ui.add_space(theme::space::XL);
        theme::section_title(ui, p, tr("Your preferences"));
        for preference in mine {
            let label = fill("{} with {}", &[&kind_title(&preference.kind), &self.project.names_of(preference.kind.people(), Some(id))]);
            match results.iter().find(|r| r.id == preference.id) {
                Some(r) => outcome_row(ui, p, &label, &r.detail, r.penalty, r.hard_violation),
                None => theme::hint(ui, p, label),
            }
        }
    }
}

#[derive(Default)]
struct RowAction {
    changed: bool,
    delete: bool,
    hovered: bool,
}

fn kind_title(kind: &PreferenceKind) -> &'static str {
    match kind {
        PreferenceKind::Together { .. } => tr("Together"),
        PreferenceKind::Apart { closeness: Closeness::Table, .. } => tr("Different tables"),
        PreferenceKind::Apart { .. } => tr("Not side by side"),
    }
}
