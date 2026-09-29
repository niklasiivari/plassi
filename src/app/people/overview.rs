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
        theme::page_header(ui, p, tr("Wishes overview"), Some(&summary));
        self.review_progress(ui, p);
        ui.add_space(theme::space::L);
        self.suggestions_card(ui, p);
        self.import_panel(ui, p);
    }

    /// An empty project: how to get people in, or try the example.
    fn first_import(&mut self, ui: &mut egui::Ui, p: Palette) {
        theme::page_header(ui, p, tr("Add your guests"), Some(tr("Paste your sign-up sheet to start. Everything else builds on the guest list.")));
        self.import_panel(ui, p);
        ui.add_space(theme::space::M);
        theme::hint(ui, p, tr("A header row like this works (any order, extra columns are kept):"));
        // Headers the importer recognises, in the interface language.
        let header = match crate::i18n::current() {
            crate::i18n::Lang::Fi => "Nimi\tJärjestö\tAvec\tPöytätoive\tRuoka\tJuoma\tAllergiat",
            crate::i18n::Lang::En => "Name\tOrganization\tAvec\tSeating wish\tFood\tDrinks\tAllergies",
        };
        ui.label(RichText::new(header).monospace().color(p.muted));
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

    pub(super) fn import_panel(&mut self, ui: &mut egui::Ui, p: Palette) {
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(tr("Import or update from a spreadsheet")).size(theme::text::LEAD).strong());
            ui.label(
                RichText::new(tr(
                    "Copy the rows (with the header row) from Excel or Sheets and paste them here. Columns are recognised by name: \
                     name, organization, participant group, avec, seating wish, food, drinks, allergies; others are kept as extra info. \
                     People are matched by name — rules, room and options are kept.",
                ))
                .size(theme::text::SMALL)
                .color(p.muted),
            );
            ui.add(
                theme::text_area(&mut self.people.import_text)
                    .hint_text(tr("Paste spreadsheet rows here…"))
                    .desired_rows(5)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace),
            );
            if self.people.import_text.trim().is_empty() {
                return;
            }
            match crate::import::parse_sheet(&self.people.import_text) {
                Ok(sheet) => {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(fill("{} rows ·", &[&sheet.rows.len()])).color(p.muted));
                        for (header, column) in sheet.headers.iter().zip(&sheet.columns) {
                            let color = if *column == crate::import::Column::Other { p.muted } else { p.good };
                            theme::status_pill(ui, p, format!("{header} → {}", column.label()), color);
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.add(theme::primary_button(tr("Update attendees"), p)).clicked() {
                            match crate::import::merge(&mut self.project, &sheet) {
                                Ok(summary) => {
                                    self.people.import_text.clear();
                                    let mut message = fill(
                                        "Imported: {} updated, {} added, {} new notes, {} wishes matched",
                                        &[&summary.updated, &summary.added, &summary.new_notes, &summary.new_rules],
                                    );
                                    if !summary.duplicate_names.is_empty() {
                                        message += &fill(
                                            " · same name on several rows, kept as separate people: {}",
                                            &[&summary.duplicate_names.join(", ")],
                                        );
                                    }
                                    self.commit(&message);
                                }
                                Err(error) => self.status = fill("Import failed: {}", &[&format!("{error:#}")]),
                            }
                        }
                        if ui.button(tr("Clear")).clicked() {
                            self.people.import_text.clear();
                        }
                    });
                }
                Err(error) => {
                    ui.label(RichText::new(format!("{error:#}")).color(p.warn));
                }
            }
        });
    }
}
