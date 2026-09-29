//! Shown when nobody is selected: review progress, suggested wishes and spreadsheet import.

use super::*;
use crate::i18n::{fill, tr};

impl PlassiApp {
    pub(super) fn people_overview(&mut self, ui: &mut egui::Ui, p: Palette) {
        let summary = fill(
            "{} people · {} with wishes · {} rules",
            &[
                &self.project.attendees.len(),
                &self.project.active_people().len(),
                &self.project.rules.len(),
            ],
        );
        if self.project.attendees.is_empty() {
            return self.first_import(ui, p);
        }
        crate::app::help::page_header(ui, p, crate::app::help::HelpPage::People, tr("Wishes overview"), Some(&summary));
        self.review_progress(ui, p);
        ui.add_space(theme::space::L);
        self.suggestions_card(ui, p);
        self.import_panel(ui, p);
    }

    /// An empty project: how to get people in, or try the example.
    fn first_import(&mut self, ui: &mut egui::Ui, p: Palette) {
        crate::app::help::page_header(ui, p, crate::app::help::HelpPage::People, tr("Add your guests"), Some(tr("Paste your sign-up sheet to start. Everything else builds on the guest list.")));
        self.import_panel(ui, p);
        ui.add_space(theme::space::L);
        ui.horizontal(|ui| {
            theme::hint(ui, p, tr("No sheet at hand?"));
            if ui.button(tr("Try with example data")).on_hover_text(tr("48 made-up guests with wishes and a room, to see how everything works")).clicked() {
                self.load_example();
            }
        });
    }

    /// How much sign-up text is reviewed, with a button to continue.
    fn review_progress(&mut self, ui: &mut egui::Ui, p: Palette) {
        let total = self.project.notes.len();
        let pending = self.project.notes.iter().filter(|n| !n.reviewed).count();
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(tr("Sign-up text"))
                            .size(theme::text::LEAD)
                            .strong(),
                    );
                    let status = if pending == 0 {
                        tr("Everything is reviewed.").to_string()
                    } else {
                        fill(
                            "{} of {} notes reviewed · {} to go",
                            &[&(total - pending), &total, &pending],
                        )
                    };
                    theme::hint(ui, p, status);
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if pending > 0
                        && ui
                            .add(theme::primary_button(tr("Start reviewing"), p))
                            .clicked()
                    {
                        self.people.filter = PeopleFilter::ToReview;
                        self.selected_person = self
                            .project
                            .notes
                            .iter()
                            .find(|n| !n.reviewed)
                            .map(|n| n.person);
                    }
                });
            });
            ui.add_space(theme::space::M);
            let done = if total == 0 {
                1.0
            } else {
                (total - pending) as f32 / total as f32
            };
            ui.add(
                egui::ProgressBar::new(done)
                    .desired_height(6.0)
                    .fill(p.accent),
            );
        });
    }

    /// Wishes found in sign-up text, with a one-click add for full-name matches.
    fn suggestions_card(&mut self, ui: &mut egui::Ui, p: Palette) {
        let project = &self.project;
        let suggestions = self
            .people
            .suggest_cache
            .get(self.file.revision, || crate::suggest::suggestions(project))
            .clone();
        if suggestions.is_empty() {
            return;
        }
        let sure: Vec<_> = suggestions
            .iter()
            .filter(|s| s.confidence == crate::suggest::Confidence::High)
            .cloned()
            .collect();
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(fill("{} suggested wishes", &[&suggestions.len()])).size(theme::text::LEAD).strong());
                    theme::hint(
                        ui,
                        p,
                        fill("{} are full names found in the text. The rest are shown on each person to check.", &[&sure.len()]),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = fill("Add {}", &[&sure.len()]);
                    let add = theme::primary_button(&label, p);
                    if !sure.is_empty() && ui.add(add).on_hover_text(tr("Add every full-name match as a wish")).clicked() {
                        for s in &sure {
                            crate::suggest::accept(&mut self.project, s);
                        }
                        self.commit(&fill("Added {} wishes from full-name matches", &[&sure.len()]));
                    }
                });
            });
        });
        ui.add_space(theme::space::L);
    }

    pub(super) fn suggestion_text(&self, s: &crate::suggest::Suggestion) -> String {
        match &s.target {
            crate::suggest::SuggestedTarget::People(ids) => {
                let names = ids
                    .iter()
                    .filter_map(|i| self.project.person(*i).map(|a| a.name.clone()))
                    .collect::<Vec<_>>()
                    .join(tr(" or "));
                fill(if s.avec { "Avec {}" } else { "Sit near {}" }, &[&names])
            }
            crate::suggest::SuggestedTarget::Organization(name) => {
                fill("Sit with {} people", &[name])
            }
        }
    }
}
