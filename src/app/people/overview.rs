//! Shown when nobody is selected: review progress, suggested wishes and spreadsheet import.

use super::*;
use crate::i18n::{fill, tr};

impl PlassiApp {
    pub(super) fn people_overview(&mut self, ui: &mut egui::Ui, p: Palette) {
        let summary = fill(
            "{} people · {} with wishes · {} rules",
            &[
                &self.project.attendees.len(),
                &self.people_list_info().active.len(),
                &self.project.rules.len(),
            ],
        );
        if self.project.attendees.is_empty() {
            return self.first_import(ui, p);
        }
        crate::app::help::page_header(
            ui,
            p,
            crate::app::help::HelpPage::People,
            tr("Overview"),
            Some(&summary),
        );
        self.review_progress(ui, p);
        ui.add_space(theme::space::L);
        self.suggestions_card(ui, p);
        self.import_panel(ui, p);
    }

    /// An empty project: how to get people in, or try the example.
    fn first_import(&mut self, ui: &mut egui::Ui, p: Palette) {
        crate::app::help::page_header(
            ui,
            p,
            crate::app::help::HelpPage::People,
            tr("Add people"),
            None,
        );
        self.import_panel(ui, p);
        ui.add_space(theme::space::L);
        ui.horizontal(|ui| {
            theme::hint(ui, p, tr("No guest list?"));
            if ui
                .button(tr("Try example"))
                .on_hover_text(tr("48 guests, wishes and tables"))
                .clicked()
            {
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
            theme::responsive_row(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(tr("Sign-up notes"))
                            .size(theme::text::LEAD)
                            .strong(),
                    );
                    let status = if total == 0 {
                        tr("No notes").to_string()
                    } else if pending == 0 {
                        tr("All notes reviewed").to_string()
                    } else {
                        fill("{} reviewed · {} left", &[&(total - pending), &pending])
                    };
                    theme::hint(ui, p, status);
                });
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if pending > 0
                            && ui
                                .add(theme::primary_button(tr("Review notes"), p))
                                .clicked()
                        {
                            self.start_review();
                        }
                    });
                });
            });
            if total > 0 {
                ui.add_space(theme::space::M);
                let done = (total - pending) as f32 / total as f32;
                ui.add(
                    egui::ProgressBar::new(done)
                        .desired_height(6.0)
                        .fill(p.accent),
                );
            }
        });
    }

    /// Wishes found in sign-up text, with a one-click add for full-name matches.
    fn suggestions_card(&mut self, ui: &mut egui::Ui, p: Palette) {
        let (project, people) = (&self.project, &mut self.people);
        let suggestions = people
            .suggest_cache
            .get(self.file.revision, || people.suggester.suggestions(project))
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
            theme::responsive_row(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(fill("{} suggested wishes", &[&suggestions.len()]))
                            .size(theme::text::LEAD)
                            .strong(),
                    );
                    theme::hint(ui, p, fill("{} full-name matches", &[&sure.len()]));
                });
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = fill("Add {}", &[&sure.len()]);
                        let add = theme::primary_button(&label, p);
                        if !sure.is_empty()
                            && ui
                                .add(add)
                                .on_hover_text(tr("Add wishes for all full-name matches"))
                                .clicked()
                        {
                            for s in &sure {
                                crate::suggest::accept(&mut self.project, s);
                            }
                            self.commit(&fill(
                                "Added {} wishes from full-name matches",
                                &[&sure.len()],
                            ));
                        }
                    });
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
                let text = match s.wish {
                    crate::model::WishKind::Avec => "Avec {}",
                    crate::model::WishKind::Seating => "Sit near {}",
                };
                fill(text, &[&names])
            }
            crate::suggest::SuggestedTarget::Organization(name) => {
                fill("Sit near people from {}", &[name])
            }
        }
    }
}
