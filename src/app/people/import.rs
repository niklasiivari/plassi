//! Getting people in from a spreadsheet: the template (copy its header row,
//! download it, export the current list in it), the pasted sheet's preview
//! with each column's meaning, and applying it.

use super::*;
use crate::i18n::{fill, tr};
use crate::import::{Column, Match, Sheet};

/// The pasted text's column meanings as the person set them, kept while the
/// header row stays the same.
#[derive(Default)]
pub(in crate::app) struct ImportMapping {
    header_line: String,
    columns: Vec<Column>,
}

impl PlassiApp {
    pub(super) fn import_panel(&mut self, ui: &mut egui::Ui, p: Palette) {
        let hovering = Self::file_hovering(ui);
        let frame = if hovering { theme::tinted_card(p, p.accent) } else { theme::card(p) };
        let card = frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(tr("Import or update from a spreadsheet")).size(theme::text::LEAD).strong());
            ui.add_space(theme::space::XS);
            self.template_bar(ui, p);
            ui.add_space(theme::space::M);
            ui.add(
                theme::text_area(&mut self.people.import_text)
                    .hint_text(tr("Paste the rows here, header row included…"))
                    .desired_rows(5)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace),
            );
            if hovering {
                ui.label(RichText::new(tr("Drop the file to load it here")).color(p.accent).strong());
            } else {
                theme::hint(ui, p, tr("Or drop a .csv/.tsv file here"));
            }
            if !self.people.import_text.trim().is_empty() {
                self.sheet_preview(ui, p);
            }
        });
        if hovering {
            let stroke = egui::Stroke::new(2.0, p.accent);
            ui.painter().rect_stroke(card.response.rect, theme::radius::LG, stroke, egui::StrokeKind::Inside);
        }
    }

    /// The template's columns with what goes in each, and the three ways to use it.
    fn template_bar(&mut self, ui: &mut egui::Ui, p: Palette) {
        theme::hint(
            ui,
            p,
            tr("Use these columns, in any order. Only the name is required; other columns are kept as extra info. People are matched by name, so importing again updates them and keeps rules, room and options."),
        );
        ui.add_space(theme::space::S);
        template_table(ui, p);
        ui.add_space(theme::space::S);
        ui.horizontal_wrapped(|ui| {
            if ui.add(theme::secondary_button(tr("Copy header row"))).on_hover_text(tr("Paste it into the first row of your spreadsheet")).clicked() {
                ui.ctx().copy_text(crate::import::header_row());
                self.status = tr("Copied the header row; paste it into your spreadsheet").into();
            }
            if ui.add(theme::secondary_button(tr("Download template"))).on_hover_text(tr("A .csv file with the columns and one example row")).clicked() {
                self.save_sheet_file(tr("plassi-template.csv"), crate::import::template_csv());
            }
            let has_people = !self.project.attendees.is_empty();
            let export = ui.add_enabled(has_people, theme::secondary_button(tr("Export people")));
            if export.on_hover_text(tr("Everyone in these columns, to edit in a spreadsheet and import back")).clicked() {
                self.save_sheet_file(tr("plassi-people.csv"), crate::import::export_csv(&self.project));
            }
        });
    }

    fn save_sheet_file(&mut self, name: &str, text: String) {
        self.status = match crate::platform::save_file(&self.file.store, name, text.as_bytes()) {
            Ok(saved) => fill("Saved {}", &[&saved.description]),
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
        theme::section_title(ui, p, &fill("{} rows · check what each column becomes", &[&sheet.rows.len()]));
        let changed = column_rows(ui, p, &mut sheet, &detected);
        if changed {
            self.people.import_mapping.columns = sheet.columns.clone();
        }
        ui.add_space(theme::space::M);
        if !sheet.has_name() {
            ui.label(RichText::new(fill("Set one column to “{}” to import.", &[&Column::Name.label()])).color(p.warn));
        }
        ui.horizontal(|ui| {
            let apply = ui.add_enabled(sheet.has_name(), theme::primary_button(tr("Update attendees"), p));
            if apply.clicked() {
                self.merge_sheet(&sheet);
            }
            if ui.add(theme::ghost_button(tr("Clear"), p)).clicked() {
                self.people.import_text.clear();
            }
        });
    }

    /// Uses the person's column choices while the header row is unchanged.
    fn apply_mapping(&mut self, sheet: &mut Sheet) {
        let header_line = sheet.headers.join("\t");
        let mapping = &mut self.people.import_mapping;
        if mapping.header_line != header_line || mapping.columns.len() != sheet.columns.len() {
            *mapping = ImportMapping { header_line, columns: sheet.columns.clone() };
        }
        sheet.columns = mapping.columns.clone();
    }

    fn merge_sheet(&mut self, sheet: &Sheet) {
        match crate::import::merge(&mut self.project, sheet) {
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
}

/// One row per column: its header, what it becomes, how that was found, and
/// sample values. Returns whether a choice changed.
fn column_rows(ui: &mut egui::Ui, p: Palette, sheet: &mut Sheet, detected: &[Column]) -> bool {
    let mut changed = false;
    egui::Grid::new("import-columns").num_columns(4).spacing([theme::space::M, theme::space::S]).show(ui, |ui| {
        for header in [tr("Column in your sheet"), tr("Imported as"), "", tr("First values")] {
            ui.label(RichText::new(header).size(theme::text::CAPTION).color(p.muted));
        }
        ui.end_row();
        for i in 0..sheet.headers.len() {
            ui.label(RichText::new(&sheet.headers[i]).strong());
            let column = &mut sheet.columns[i];
            theme::combo(("import-column", i), column.label()).width(180.0).show_ui(ui, |ui| {
                for choice in Column::CHOICES {
                    changed |= ui.selectable_value(column, choice, choice.label()).changed();
                }
            });
            // Set by the person: differs from what was found in the header.
            let set_by_you = detected.get(i) != Some(&sheet.columns[i]);
            match_pill(ui, p, sheet.matches[i], set_by_you);
            ui.label(RichText::new(samples(sheet, i)).size(theme::text::SMALL).color(p.muted));
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
        Match::Remembered => (tr("Your earlier choice"), p.good),
        Match::Unknown => (tr("Not in the template: choose what it is"), p.warn),
    };
    if set_by_you {
        theme::status_pill(ui, p, tr("Set by you"), p.accent);
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
            egui::Grid::new("import-template").num_columns(3).spacing([theme::space::L, theme::space::S]).show(ui, |ui| {
                for column in Column::TEMPLATE {
                    ui.label(RichText::new(column.header()).strong());
                    let required = if column == Column::Name { tr("required") } else { tr("optional") };
                    ui.label(RichText::new(required).size(theme::text::SMALL).color(p.muted));
                    ui.label(RichText::new(column.explain()).size(theme::text::SMALL));
                    ui.end_row();
                }
            });
        });
    });
}
