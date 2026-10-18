//! Getting people in from a spreadsheet: the template (copy its header row,
//! download it, export the current list in it), the pasted sheet's preview
//! with each column's meaning, and applying it.

use super::*;
use crate::i18n::{fill, tr};
use crate::import::{Column, ImportMode, Match, Sheet};

/// The pasted text's column meanings as the person set them, kept while the
/// header row stays the same.
#[derive(Default)]
pub(in crate::app) struct ImportMapping {
    header_line: String,
    columns: Vec<Column>,
}

impl PlassiApp {
    pub(super) fn import_panel(&mut self, ui: &mut egui::Ui, p: Palette) {
        if self.people.import_open
            && !self.project.attendees.is_empty()
            && ui
                .add(theme::ghost_button(tr("Back to overview"), p))
                .clicked()
        {
            self.people.import_open = false;
            self.selected_person = None;
        }
        let hovering = Self::file_hovering(ui);
        let frame = if hovering {
            theme::tinted_card(p, p.accent)
        } else {
            theme::card(p)
        };
        let card = frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(tr("Import people"))
                    .size(theme::text::LEAD)
                    .strong(),
            );
            ui.add_space(theme::space::XS);
            theme::hint(ui, p, tr("Import another sheet at any time. Review its columns before adding people."));
            ui.add_space(theme::space::S);
            if ui.add_enabled(self.people.sheet_picker.is_none(), theme::secondary_button(tr("Choose file…"))).clicked() {
                self.people.dropped = None;
                self.people.sheet_picker = Some(crate::platform::SheetPick::start(ui.ctx().clone()));
            }
            if self.people.import_sheets.len() > 1 {
                ui.horizontal(|ui| {
                    ui.label(tr("Worksheet"));
                    let previous = self.people.import_sheet;
                    theme::combo("import-worksheet", &self.people.import_sheets[previous].0)
                        .show_ui(ui, |ui| {
                            for (index, (name, _)) in self.people.import_sheets.iter().enumerate() {
                                ui.selectable_value(&mut self.people.import_sheet, index, name);
                            }
                        });
                    if previous != self.people.import_sheet {
                        self.people.import_text = self.people.import_sheets[self.people.import_sheet].1.clone();
                        self.people.import_mapping = Default::default();
                    }
                });
            }
            ui.add_space(theme::space::S);
            self.template_bar(ui, p);
            ui.add_space(theme::space::M);
            ui.add(
                theme::text_area(&mut self.people.import_text)
                    .hint_text(tr("Paste rows with column headers…"))
                    .desired_rows(5)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace),
            );
            if hovering {
                ui.label(RichText::new(tr("Drop to import")).color(p.accent).strong());
            } else {
                theme::hint(ui, p, tr("Choose or drop an Excel, CSV, TSV or ODS file, or paste rows with headers."));
            }
            if !self.people.import_text.trim().is_empty() {
                self.sheet_preview(ui, p);
            }
        });
        if hovering {
            let stroke = egui::Stroke::new(2.0, p.accent);
            ui.painter().rect_stroke(
                card.response.rect,
                theme::radius::LG,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
    }

    /// The template's columns with what goes in each, and the three ways to use it.
    fn template_bar(&mut self, ui: &mut egui::Ui, p: Palette) {
        theme::hint(
            ui,
            p,
            tr("Name required. Choose what each column means for this sheet."),
        );
        ui.add_space(theme::space::S);
        egui::CollapsingHeader::new(tr("Columns")).show(ui, |ui| {
            template_table(ui, p);
        });
        ui.add_space(theme::space::S);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add(theme::secondary_button(tr("Copy header row")))
                .on_hover_text(tr("Use as the sheet's first row"))
                .clicked()
            {
                ui.ctx().copy_text(crate::import::header_row());
                self.status = tr("Header row copied").into();
            }
            if ui
                .add(theme::secondary_button(tr("Download template")))
                .on_hover_text(tr("CSV with headers and an example"))
                .clicked()
            {
                self.save_sheet_file(tr("plassi-template.csv"), crate::import::template_csv());
            }
            let has_people = !self.project.attendees.is_empty();
            let export = ui.add_enabled(has_people, theme::secondary_button(tr("Export CSV")));
            if export
                .on_hover_text(tr("Edit in a spreadsheet and import back"))
                .clicked()
            {
                self.save_sheet_file(
                    tr("plassi-people.csv"),
                    crate::import::export_csv(&self.project),
                );
            }
        });
    }

    fn save_sheet_file(&mut self, name: &str, text: String) {
        self.status = match crate::platform::save_file(&self.file.store, name, text.as_bytes()) {
            Ok(saved) => fill(
                if cfg!(target_arch = "wasm32") {
                    "Downloaded {}"
                } else {
                    "Saved {}"
                },
                &[&saved.description],
            ),
            Err(error) => format!("{error:#}"),
        };
    }

    /// Each column of the pasted sheet with what it becomes, then Apply.
    fn sheet_preview(&mut self, ui: &mut egui::Ui, p: Palette) {
        let mut sheet = match crate::import::parse_sheet(&self.people.import_text) {
            Ok(sheet) => sheet,
            Err(error) => {
                ui.label(RichText::new(format!("{error:#}")).color(p.warn));
                return;
            }
        };
        sheet.remember(&self.project.import_columns);
        let detected = sheet.columns.clone();
        self.apply_mapping(&mut sheet);
        ui.add_space(theme::space::M);
        theme::section_title(
            ui,
            p,
            &fill("{} rows · check column mapping", &[&sheet.rows.len()]),
        );
        let changed = column_rows(ui, p, &mut sheet, &detected);
        if changed {
            self.people.import_mapping.columns = sheet.columns.clone();
        }
        ui.add_space(theme::space::M);
        if !sheet.has_name() {
            ui.label(
                RichText::new(fill("Choose the “{}” column.", &[&Column::Name.label()]))
                    .color(p.warn),
            );
        }
        if !self.project.attendees.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.radio_value(
                    &mut self.people.import_mode,
                    ImportMode::Add,
                    tr("Add as new people"),
                );
                ui.radio_value(
                    &mut self.people.import_mode,
                    ImportMode::UpdateMatching,
                    tr("Update matching names"),
                );
            });
            theme::hint(
                ui,
                p,
                match self.people.import_mode {
                    ImportMode::Add => tr(
                        "Each row adds a separate person. Existing people and seats stay unchanged.",
                    ),
                    ImportMode::UpdateMatching => tr(
                        "Matching names update existing people. New names are added; nobody is removed.",
                    ),
                },
            );
        }
        let prepared = sheet
            .has_name()
            .then(|| crate::import::prepare(&self.project, &sheet, self.people.import_mode));
        if let Some(result) = &prepared {
            match result {
                Ok((next, summary)) => {
                    ui.label(fill(
                        "To add: {} · to update: {}",
                        &[&summary.added, &summary.updated],
                    ));
                    theme::hint(
                        ui,
                        p,
                        fill("People after import: {}", &[&next.attendees.len()]),
                    );
                }
                Err(error) => {
                    ui.label(RichText::new(format!("{error:#}")).color(p.warn));
                }
            }
        }
        ui.horizontal(|ui| {
            let ready = prepared.as_ref().is_some_and(|result| result.is_ok())
                && !self.file.external_invalid;
            let apply = ui.add_enabled(ready, theme::primary_button(tr("Import"), p));
            if apply.clicked()
                && let Some(Ok((next, summary))) = prepared
            {
                self.project = next;
                self.finish_import(summary);
            }
            if ui.add(theme::ghost_button(tr("Clear"), p)).clicked() {
                self.people.clear_import();
                self.people.import_open = true;
            }
        });
    }

    /// Uses the person's column choices while the header row is unchanged.
    fn apply_mapping(&mut self, sheet: &mut Sheet) {
        let header_line = sheet.headers.join("\t");
        let mapping = &mut self.people.import_mapping;
        if mapping.header_line != header_line || mapping.columns.len() != sheet.columns.len() {
            *mapping = ImportMapping {
                header_line,
                columns: sheet.columns.clone(),
            };
        }
        sheet.columns = mapping.columns.clone();
    }

    fn finish_import(&mut self, summary: crate::import::MergeSummary) {
        self.people.clear_import();
        self.people.search.clear();
        self.people.filter = PeopleFilter::All;
        self.people.picked.clear();
        self.selected_person = None;
        let mut message = fill(
            "Imported: {} updated, {} added, {} new notes, {} wishes matched",
            &[
                &summary.updated,
                &summary.added,
                &summary.new_notes,
                &summary.new_rules,
            ],
        );
        if !summary.duplicate_names.is_empty() {
            message += &fill(
                " · duplicate names kept separate: {}",
                &[&summary.duplicate_names.join(", ")],
            );
        }
        self.commit(&message);
    }
}

/// One row per column: its header, what it becomes, how that was found, and
/// sample values. Returns whether a choice changed.
fn column_rows(ui: &mut egui::Ui, p: Palette, sheet: &mut Sheet, detected: &[Column]) -> bool {
    let mut changed = false;
    egui::Grid::new("import-columns")
        .num_columns(4)
        .spacing([theme::space::M, theme::space::S])
        .show(ui, |ui| {
            for header in [tr("Source column"), tr("Imported as"), "", tr("Preview")] {
                ui.label(
                    RichText::new(header)
                        .size(theme::text::CAPTION)
                        .color(p.muted),
                );
            }
            ui.end_row();
            for i in 0..sheet.headers.len() {
                ui.label(RichText::new(&sheet.headers[i]).strong());
                let column = &mut sheet.columns[i];
                theme::combo(("import-column", i), column.label())
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for choice in Column::CHOICES {
                            changed |= ui
                                .selectable_value(column, choice, choice.label())
                                .changed();
                        }
                    });
                // Set by the person: differs from what was found in the header.
                let set_by_you = detected.get(i) != Some(&sheet.columns[i]);
                match_pill(ui, p, sheet.matches[i], set_by_you);
                ui.label(
                    RichText::new(samples(sheet, i))
                        .size(theme::text::SMALL)
                        .color(p.muted),
                );
                ui.end_row();
            }
        });
    changed
}

/// Whether the column is a template column, was set on an earlier import,
/// or still needs a decision.
fn match_pill(ui: &mut egui::Ui, p: Palette, found: Match, set_by_you: bool) {
    let (label, color) = match found {
        Match::Exact => (tr("Template column"), p.good),
        Match::Remembered => (tr("Previous choice"), p.good),
        Match::Unknown => (tr("Choose a field"), p.warn),
    };
    if set_by_you {
        theme::status_pill(ui, p, tr("Selected"), p.accent);
    } else {
        theme::status_pill(ui, p, label, color);
    }
}

/// The first few non-empty values of a column.
fn samples(sheet: &Sheet, column: usize) -> String {
    let values: Vec<&str> = sheet
        .rows
        .iter()
        .filter_map(|row| row.get(column).map(String::as_str))
        .filter(|v| !v.is_empty())
        .take(3)
        .collect();
    values.join(" · ")
}

/// The template's columns: name, whether it's required, and what goes in it.
fn template_table(ui: &mut egui::Ui, p: Palette) {
    theme::inset(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        theme::text_rows(ui, |ui| {
            egui::Grid::new("import-template")
                .num_columns(3)
                .spacing([theme::space::L, theme::space::S])
                .show(ui, |ui| {
                    for column in Column::TEMPLATE {
                        ui.label(RichText::new(column.header()).strong());
                        let required = if column == Column::Name {
                            tr("required")
                        } else {
                            tr("optional")
                        };
                        ui.label(
                            RichText::new(required)
                                .size(theme::text::SMALL)
                                .color(p.muted),
                        );
                        ui.label(RichText::new(column.explain()).size(theme::text::SMALL));
                        ui.end_row();
                    }
                });
        });
    });
}
