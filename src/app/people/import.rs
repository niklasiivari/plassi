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
    reviewed: Vec<bool>,
}

/// Parsing and a prepared import survive repaints. The prepared project is
/// moved into the app on Apply, rather than cloned again.
#[derive(Default)]
pub(in crate::app) struct ImportCache {
    parsed: Option<ParsedImport>,
    prepared: Option<(PreparedKey, PreparedImport)>,
    #[cfg(test)]
    parses: usize,
    #[cfg(test)]
    preparations: usize,
}

struct ParsedImport {
    text: String,
    language: crate::i18n::Lang,
    result: Result<ParsedSheet, String>,
}

struct ParsedSheet {
    sheet: Sheet,
    columns: Vec<Column>,
    matches: Vec<Match>,
}

#[derive(PartialEq)]
struct PreparedKey {
    revision: u64,
    columns: Vec<Column>,
    mode: ImportMode,
}

type PreparedImport = Result<(crate::model::Project, crate::import::MergeSummary), String>;

impl ImportCache {
    fn parse(&mut self, text: &str) {
        let language = crate::i18n::current();
        if self
            .parsed
            .as_ref()
            .is_some_and(|parsed| parsed.text == text && parsed.language == language)
        {
            return;
        }
        self.parsed = Some(ParsedImport {
            text: text.to_string(),
            language,
            result: crate::import::parse_sheet(text)
                .map(|sheet| ParsedSheet {
                    columns: sheet.columns.clone(),
                    matches: sheet.matches.clone(),
                    sheet,
                })
                .map_err(|error| format!("{error:#}")),
        });
        self.prepared = None;
        #[cfg(test)]
        {
            self.parses += 1;
        }
    }

    fn sheet(&mut self) -> Result<&mut Sheet, &str> {
        let parsed = self.parsed.as_mut().expect("parsed before showing");
        match &mut parsed.result {
            Ok(parsed) => {
                // Remembered mappings can change with the project. Start from
                // the parser's metadata, while retaining the sheet's rows.
                parsed.sheet.columns.clone_from(&parsed.columns);
                parsed.sheet.matches.clone_from(&parsed.matches);
                Ok(&mut parsed.sheet)
            }
            Err(error) => Err(error),
        }
    }

    fn prepare(&mut self, project: &crate::model::Project, revision: u64, mode: ImportMode) {
        let sheet = &self
            .parsed
            .as_ref()
            .expect("parsed before preparing")
            .result
            .as_ref()
            .expect("only valid sheets are prepared")
            .sheet;
        if !sheet.has_name() {
            self.prepared = None;
            return;
        }
        let key = PreparedKey {
            revision,
            columns: sheet.columns.clone(),
            mode,
        };
        if self.prepared.as_ref().is_some_and(|(old, _)| *old == key) {
            return;
        }
        self.prepared = Some((
            key,
            crate::import::prepare(project, sheet, mode).map_err(|error| format!("{error:#}")),
        ));
        #[cfg(test)]
        {
            self.preparations += 1;
        }
    }
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
            ui.add_space(theme::space::S);
            if ui
                .add_enabled(
                    self.people.sheet_picker.is_none(),
                    theme::secondary_button(tr("Choose file…")),
                )
                .clicked()
            {
                self.people.dropped = None;
                self.people.sheet_picker =
                    Some(crate::platform::SheetPick::start(ui.ctx().clone()));
            }
            if self.people.import_sheets.len() > 1 {
                theme::responsive_row(ui, |ui| {
                    ui.label(tr("Worksheet"));
                    let previous = self.people.import_sheet;
                    theme::combo("import-worksheet", &self.people.import_sheets[previous].0)
                        .show_ui(ui, |ui| {
                            for (index, (name, _)) in self.people.import_sheets.iter().enumerate() {
                                ui.value(&mut self.people.import_sheet, index, name);
                            }
                        });
                    if previous != self.people.import_sheet {
                        self.people.import_text = self.people.import_sheets
                            [self.people.import_sheet]
                            .1
                            .clone();
                        self.people.import_mapping = Default::default();
                    }
                });
            }
            ui.add_space(theme::space::S);
            if self.people.import_text.trim().is_empty() {
                self.template_bar(ui, p);
            } else {
                egui::CollapsingHeader::new(tr("Template and export"))
                    .show(ui, |ui| self.template_bar(ui, p));
            }
            ui.add_space(theme::space::M);
            egui::ScrollArea::vertical()
                .id_salt("import-source")
                .max_height(140.0)
                .show(ui, |ui| {
                    ui.add(
                        theme::text_area(&mut self.people.import_text)
                            .hint_text(tr("Paste rows with column headers…"))
                            .desired_rows(3)
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Monospace),
                    );
                });
            if hovering {
                ui.label(RichText::new(tr("Drop to import")).color(p.accent).strong());
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
        let people = &mut self.people;
        let cache = &mut people.import_cache;
        cache.parse(&people.import_text);
        let sheet = match cache.sheet() {
            Ok(sheet) => sheet,
            Err(error) => {
                ui.label(RichText::new(error).color(p.warn));
                return;
            }
        };
        sheet.remember(&self.project.import_columns);
        Self::apply_mapping(&mut people.import_mapping, sheet);
        ui.add_space(theme::space::M);
        theme::section_title(
            ui,
            p,
            &fill("{} rows · check column mapping", &[&sheet.rows.len()]),
        );
        ui.add_space(theme::space::S);
        let changed = column_rows(ui, p, sheet, &mut people.import_mapping.reviewed);
        if changed {
            people.import_mapping.columns = sheet.columns.clone();
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
                    &mut people.import_mode,
                    ImportMode::Add,
                    tr("Add as new people"),
                );
                ui.radio_value(
                    &mut people.import_mode,
                    ImportMode::UpdateMatching,
                    tr("Update matching names"),
                );
            });
        }
        cache.prepare(&self.project, self.file.revision, people.import_mode);
        let prepared = cache.prepared.as_ref().map(|(_, result)| result);
        if let Some(result) = prepared {
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
        let mut apply = false;
        let mut clear = false;
        ui.horizontal(|ui| {
            let ready =
                prepared.is_some_and(|result| result.is_ok()) && !self.file.external_invalid;
            apply = ui
                .add_enabled(ready, theme::primary_button(tr("Import"), p))
                .clicked();
            clear = ui.add(theme::ghost_button(tr("Clear"), p)).clicked();
        });
        if apply && let Some((_, Ok((next, summary)))) = cache.prepared.take() {
            self.project = next;
            self.finish_import(summary);
        }
        if clear {
            self.people.clear_import();
            self.people.import_open = true;
        }
    }

    /// Uses the person's column choices while the header row is unchanged.
    fn apply_mapping(mapping: &mut ImportMapping, sheet: &mut Sheet) {
        let header_line = sheet.headers.join("\t");
        if mapping.header_line != header_line || mapping.columns.len() != sheet.columns.len() {
            *mapping = ImportMapping {
                header_line,
                columns: sheet.columns.clone(),
                reviewed: vec![false; sheet.columns.len()],
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
fn column_rows(ui: &mut egui::Ui, p: Palette, sheet: &mut Sheet, reviewed: &mut [bool]) -> bool {
    let mut changed = false;
    let width = ui.available_width();
    let stacked = width < 640.0;
    let gap = theme::space::M;
    let mapping_width = 190.0;
    let text_width = (width - mapping_width - 2.0 * gap) / 2.0;
    ui.spacing_mut().item_spacing.x = gap;
    if !stacked {
        ui.horizontal(|ui| {
            for (label, part_width) in [
                ("Source column", text_width),
                ("Imported as", mapping_width),
                ("Preview", text_width),
            ] {
                ui.allocate_ui_with_layout(
                    egui::vec2(part_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(part_width);
                        theme::hint(ui, p, tr(label));
                    },
                );
            }
        });
        ui.separator();
    }
    for (i, reviewed) in reviewed.iter_mut().enumerate() {
        if i > 0 {
            ui.add_space(theme::space::S);
            ui.separator();
            ui.add_space(theme::space::S);
        }
        let preview = samples(sheet, i);
        let header = sheet.headers[i].clone();
        let source = |ui: &mut egui::Ui| {
            if stacked {
                theme::hint(ui, p, tr("Source column"));
            }
            ui.add(egui::Label::new(RichText::new(&header).strong()).wrap());
        };
        let sample = |ui: &mut egui::Ui| {
            if stacked {
                theme::hint(ui, p, tr("Preview"));
            }
            ui.add(
                egui::Label::new(
                    RichText::new(&preview)
                        .size(theme::text::SMALL)
                        .color(p.muted),
                )
                .wrap(),
            );
        };
        if stacked {
            source(ui);
            ui.add_space(theme::space::S);
            theme::hint(ui, p, tr("Imported as"));
            changed |= column_choice(ui, p, i, sheet, reviewed);
            ui.add_space(theme::space::S);
            sample(ui);
        } else {
            ui.spacing_mut().item_spacing.x = gap;
            ui.horizontal_top(|ui| {
                for (part, part_width) in [text_width, mapping_width, text_width]
                    .into_iter()
                    .enumerate()
                {
                    ui.allocate_ui_with_layout(
                        egui::vec2(part_width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_width(part_width);
                            match part {
                                0 => source(ui),
                                1 => changed |= column_choice(ui, p, i, sheet, reviewed),
                                _ => sample(ui),
                            }
                        },
                    );
                }
            });
        }
    }
    changed
}

/// Mapping and its origin stay together, independently of source text length.
fn column_choice(
    ui: &mut egui::Ui,
    p: Palette,
    i: usize,
    sheet: &mut Sheet,
    reviewed: &mut bool,
) -> bool {
    let mut changed = false;
    let column = &mut sheet.columns[i];
    theme::combo(("import-column", i), column.label())
        .width(180.0_f32.min(ui.available_width()))
        .show_ui(ui, |ui| {
            for choice in Column::CHOICES {
                if ui.action(*column == choice, choice.label()) {
                    *column = choice;
                    *reviewed = true;
                    changed = true;
                }
            }
        });
    match_pill(ui, p, sheet.matches[i], *reviewed);
    changed
}

/// Whether the column is a template column, was set on an earlier import,
/// or still needs a decision.
fn match_pill(ui: &mut egui::Ui, p: Palette, found: Match, set_by_you: bool) {
    let (label, color) = match found {
        Match::Exact => (tr("Template column"), p.good),
        Match::Remembered => (tr("Previous choice"), p.good),
        Match::Unknown => (tr("Review mapping"), p.muted),
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
    values
        .into_iter()
        .map(|value| {
            let mut chars = value.chars();
            let preview: String = chars.by_ref().take(80).collect();
            if chars.next().is_some() {
                format!("{preview}…")
            } else {
                preview
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The template's columns: name, whether it's required, and what goes in it.
fn template_table(ui: &mut egui::Ui, p: Palette) {
    theme::inset(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if theme::narrow(ui) {
            for column in Column::TEMPLATE {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(column.header()).strong());
                    theme::hint(
                        ui,
                        p,
                        if column == Column::Name {
                            tr("required")
                        } else {
                            tr("optional")
                        },
                    );
                });
                ui.add(
                    egui::Label::new(RichText::new(column.explain()).size(theme::text::SMALL))
                        .wrap(),
                );
                ui.add_space(theme::space::S);
            }
            return;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;

    fn refresh(
        cache: &mut ImportCache,
        mapping: &mut ImportMapping,
        text: &str,
        project: &Project,
        revision: u64,
        mode: ImportMode,
    ) {
        cache.parse(text);
        let sheet = cache.sheet().unwrap();
        sheet.remember(&project.import_columns);
        PlassiApp::apply_mapping(mapping, sheet);
        cache.prepare(project, revision, mode);
    }

    fn prepared(cache: &ImportCache) -> &(Project, crate::import::MergeSummary) {
        cache.prepared.as_ref().unwrap().1.as_ref().unwrap()
    }

    #[test]
    fn preview_reuses_unchanged_input_and_refreshes_every_import_input() {
        crate::i18n::set(crate::i18n::Lang::En);
        let source = "Name\tOther\nPerson 1\tABC";
        let mut project = crate::import::prepare(
            &Project::new(),
            &crate::import::parse_sheet("Name\nPerson 1").unwrap(),
            ImportMode::Add,
        )
        .unwrap()
        .0;
        let mut cache = ImportCache::default();
        let mut mapping = ImportMapping::default();
        refresh(
            &mut cache,
            &mut mapping,
            source,
            &project,
            0,
            ImportMode::Add,
        );
        assert_eq!(prepared(&cache).0.attendees.len(), 2);
        assert_eq!(prepared(&cache).0.attendees[1].extra["Other"], "ABC");
        refresh(
            &mut cache,
            &mut mapping,
            source,
            &project,
            0,
            ImportMode::Add,
        );
        assert_eq!((cache.parses, cache.preparations), (1, 1));

        mapping.columns[1] = Column::Food;
        refresh(
            &mut cache,
            &mut mapping,
            source,
            &project,
            0,
            ImportMode::Add,
        );
        assert_eq!(prepared(&cache).0.attendees[1].food, "ABC");
        assert!(prepared(&cache).0.attendees[1].extra.is_empty());
        assert_eq!((cache.parses, cache.preparations), (1, 2));

        let changed = "Name\tOther\nPerson 1\tXYZ";
        refresh(
            &mut cache,
            &mut mapping,
            changed,
            &project,
            0,
            ImportMode::Add,
        );
        assert_eq!(prepared(&cache).0.attendees[1].food, "XYZ");
        assert_eq!((cache.parses, cache.preparations), (2, 3));

        refresh(
            &mut cache,
            &mut mapping,
            changed,
            &project,
            0,
            ImportMode::UpdateMatching,
        );
        assert_eq!(prepared(&cache).0.attendees.len(), 1);
        assert_eq!(prepared(&cache).0.attendees[0].food, "XYZ");
        assert_eq!((cache.parses, cache.preparations), (2, 4));

        project.attendees.clear();
        refresh(
            &mut cache,
            &mut mapping,
            changed,
            &project,
            1,
            ImportMode::UpdateMatching,
        );
        assert_eq!(
            (prepared(&cache).1.added, prepared(&cache).1.updated),
            (1, 0)
        );
        assert_eq!((cache.parses, cache.preparations), (2, 5));

        mapping.columns[0] = Column::Ignore;
        refresh(
            &mut cache,
            &mut mapping,
            changed,
            &project,
            1,
            ImportMode::Add,
        );
        assert!(cache.prepared.is_none(), "a missing name disables Apply");
    }

    #[test]
    fn preview_refreshes_localized_errors_and_clear_releases_cached_projects() {
        let project = Project::new();
        let mut cache = ImportCache::default();
        let mut mapping = ImportMapping::default();
        for language in crate::i18n::Lang::ALL {
            crate::i18n::set(language);
            refresh(
                &mut cache,
                &mut mapping,
                "Name\n",
                &project,
                0,
                ImportMode::Add,
            );
            let actual = cache.prepared.as_ref().unwrap().1.as_ref().unwrap_err();
            let expected = crate::import::prepare(
                &project,
                &crate::import::parse_sheet("Name\n").unwrap(),
                ImportMode::Add,
            )
            .unwrap_err();
            assert_eq!(actual, &format!("{expected:#}"));
        }
        assert_eq!((cache.parses, cache.preparations), (2, 2));
        let mut state = PeopleState {
            import_cache: cache,
            ..Default::default()
        };
        state.clear_import();
        assert!(state.import_cache.parsed.is_none());
        assert!(state.import_cache.prepared.is_none());
    }
}
