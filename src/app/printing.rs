//! The Print page: event details and the four A4 PDFs, typeset in the background.

use crate::i18n::{fill, tr};
use eframe::egui::{self, RichText};

use crate::model::Attendee;
use crate::print::PrintKind;

use super::PlassiApp;
use super::theme::{self, Palette};

/// PDFs being typeset: whether to open them afterwards, and each PDF.
pub(super) type PrintJob = (bool, crate::platform::PdfJob<PrintKind>);

/// State of the Print page.
#[derive(Default)]
pub(super) struct PrintState {
    pub(super) job: Option<PrintJob>,
    /// Last saved PDF per print kind, and the project revision it shows.
    pub(super) saved: std::collections::HashMap<PrintKind, (String, u64)>,
    /// File, project revision and selected option of the last Excel snapshot.
    pub(super) spreadsheet: Option<(String, u64, usize)>,
}

impl PlassiApp {
    pub(super) fn print_page(&mut self, ui: &mut egui::Ui, p: Palette) {
        self.poll_print();
        crate::app::help::page_header(
            ui,
            p,
            crate::app::help::HelpPage::Print,
            tr("Print"),
            Some(if cfg!(target_arch = "wasm32") {
                tr("Download A4 PDFs.")
            } else {
                tr("A4 PDFs saved in the project's folder.")
            }),
        );
        if self.event_settings(ui, p) {
            self.commit(tr("Saved event details"));
        }
        ui.add_space(theme::space::L);
        self.spreadsheet_card(ui, p);
        ui.add_space(theme::space::L);
        let mut start = None;
        ui.horizontal_wrapped(|ui| {
            let ready = !self.project.candidates.is_empty() && self.printing.job.is_none();
            if ui
                .add_enabled(
                    ready,
                    theme::primary_button(
                        if cfg!(target_arch = "wasm32") {
                            tr("Download all PDFs")
                        } else {
                            tr("Save all PDFs")
                        },
                        p,
                    ),
                )
                .clicked()
            {
                start = Some((PrintKind::ALL.to_vec(), false));
            }
            if let Some(folder) = crate::platform::save_folder(&self.file.store)
                && ui.add(theme::secondary_button(tr("Open folder"))).clicked()
            {
                self.open_file(&folder);
            }
            if self.printing.job.is_some() {
                ui.spinner();
                ui.label(tr("Creating PDFs…"));
            }
        });
        ui.add_space(theme::space::S);
        ui.spacing_mut().item_spacing.x = theme::space::L;
        if theme::narrow(ui) {
            for kind in PrintKind::ALL {
                if let Some(open) = self.print_card(ui, p, kind) {
                    start = Some((vec![kind], open));
                }
                ui.add_space(theme::space::S);
            }
        } else {
            ui.columns(2, |cols| {
                for (i, kind) in PrintKind::ALL.into_iter().enumerate() {
                    if let Some(open) = self.print_card(&mut cols[i % 2], p, kind) {
                        start = Some((vec![kind], open));
                    }
                    cols[i % 2].add_space(theme::space::S);
                }
            });
        }
        if let Some((kinds, open)) = start {
            self.start_print(kinds, open);
        }
    }

    fn spreadsheet_card(&mut self, ui: &mut egui::Ui, p: Palette) {
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(tr("Excel workbook")).strong().size(theme::text::LEAD));
            theme::hint(ui, p, tr("Seating order, meals, drinks, allergies and extra guest details, with catering totals by table. Includes unseated guests and empty seats."));
            let mut request = None;
            ui.horizontal_wrapped(|ui| {
                let ready = !self.project.candidates.is_empty();
                if ui.add_enabled(ready, theme::primary_button(tr("Export Excel (.xlsx)"), p)).clicked() {
                    request = Some(false);
                }
                if !cfg!(target_arch = "wasm32") && ui.add_enabled(ready, theme::secondary_button(tr("Save Excel and open"))).clicked() {
                    request = Some(true);
                }
            });
            if let Some(open) = request {
                let result = crate::export::workbook(&self.project, self.selected_candidate)
                    .and_then(|bytes| {
                        let name = crate::export::file_name(&self.project, &self.file.name);
                        crate::platform::save_file(&self.file.store, &name, &bytes)
                    });
                match result {
                    Ok(saved) => {
                        if open && let Some(path) = saved.location { self.open_file(&path); }
                        self.status = fill(if cfg!(target_arch = "wasm32") { "Downloaded {}" } else { "Saved {}" }, &[&saved.description]);
                        self.printing.spreadsheet = Some((saved.description, self.file.revision, self.selected_candidate));
                    }
                    Err(error) => self.status = format!("{error:#}"),
                }
            }
            if let Some((path, revision, option)) = &self.printing.spreadsheet {
                let stale = *revision != self.file.revision || *option != self.selected_candidate;
                let text = if stale { fill("{} · out of date", &[path]) } else { path.clone() };
                ui.label(RichText::new(text).size(theme::text::CAPTION).color(if stale { p.warn } else { p.muted }));
            }
        });
    }

    /// Title, subtitle, language and which option to print. Returns true if edited.
    fn event_settings(&mut self, ui: &mut egui::Ui, p: Palette) -> bool {
        let mut changed = false;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(tr("Event")).strong());
            let mobile = theme::narrow(ui);
            let field_width = if mobile { ui.available_width() } else { 380.0 };
            egui::Grid::new("event")
                .num_columns(if mobile { 1 } else { 2 })
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    let event = &mut self.project.event;
                    for (label, value, hint) in [
                        (tr("Title"), &mut event.title, tr("Event name")),
                        (tr("Subtitle"), &mut event.subtitle, tr("Date and venue")),
                    ] {
                        ui.label(RichText::new(label).color(p.muted));
                        if mobile {
                            ui.end_row();
                        }
                        let field = theme::field(value)
                            .hint_text(hint)
                            .desired_width(field_width);
                        changed |= ui.add(field).lost_focus();
                        ui.end_row();
                    }
                    ui.label(RichText::new(tr("Language")).color(p.muted));
                    if mobile {
                        ui.end_row();
                    }
                    ui.horizontal_wrapped(|ui| {
                        use crate::model::PrintLanguage;
                        for (value, label) in [
                            (PrintLanguage::SameAsApp, tr("App language")),
                            (PrintLanguage::Finnish, "Suomi"),
                            (PrintLanguage::English, "English"),
                        ] {
                            changed |= ui
                                .selectable_value(&mut event.print_language, value, label)
                                .changed();
                        }
                    });
                    ui.end_row();
                    ui.label(RichText::new(tr("Option")).color(p.muted));
                    if mobile {
                        ui.end_row();
                    }
                    ui.vertical(|ui| {
                        self.option_picker(ui, p);
                        self.option_check(ui, p);
                    });
                    ui.end_row();
                });
        });
        changed
    }

    fn option_picker(&mut self, ui: &mut egui::Ui, p: Palette) {
        if self.project.candidates.is_empty() {
            ui.label(RichText::new(tr("Create a seating chart first")).color(p.warn));
            return;
        }
        self.selected_candidate = self
            .selected_candidate
            .min(self.project.candidates.len() - 1);
        theme::combo(
            "print-option",
            &self.project.candidates[self.selected_candidate].name,
        )
        .show_ui(ui, |ui| {
            for (i, c) in self.project.candidates.iter().enumerate() {
                ui.value(&mut self.selected_candidate, i, &c.name);
            }
        });
    }

    /// Whether the option is fit to print: broken rules, gaps, unseated people.
    fn option_check(&mut self, ui: &mut egui::Ui, p: Palette) {
        let Some(evaluation) = self.evaluation() else {
            return;
        };
        let Some(candidate) = self.project.candidates.get(self.selected_candidate) else {
            return;
        };
        let gaps = crate::model::gaps(&self.project.layout.seats(), &candidate.assignment).len();
        let problems: Vec<String> = [
            (evaluation.hard_violations, "{} hard rules broken"),
            (evaluation.assignment_errors.len(), "{} seating errors"),
            (evaluation.unseated.len(), "{} not seated"),
            (gaps, "{} empty seats mid-table"),
        ]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, what)| fill(what, &[&n]))
        .collect();
        let (text, color) = if problems.is_empty() {
            (
                fill("Ready to print · score {}", &[&evaluation.penalty()]),
                p.good,
            )
        } else {
            (problems.join(" · "), p.warn)
        };
        ui.label(RichText::new(text).size(theme::text::SMALL).color(color));
    }

    /// One print with its buttons. Returns `Some(open_afterwards)` when requested.
    fn print_card(&self, ui: &mut egui::Ui, p: Palette, kind: PrintKind) -> Option<bool> {
        let ready = !self.project.candidates.is_empty() && self.printing.job.is_none();
        let mut request = None;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(kind.title()).strong().size(theme::text::LEAD));
            theme::hint(ui, p, kind.description());
            if let Some(field) = self.missing_data(kind) {
                let text = match field {
                    "food" => tr("No meal data. Add it on People & wishes."),
                    _ => tr("No drink data. Add it on People & wishes."),
                };
                ui.label(RichText::new(text).size(theme::text::SMALL).color(p.warn));
            }
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        ready,
                        theme::primary_button(
                            if cfg!(target_arch = "wasm32") {
                                tr("Download PDF")
                            } else {
                                tr("Save PDF")
                            },
                            p,
                        ),
                    )
                    .clicked()
                {
                    request = Some(false);
                }
                if !cfg!(target_arch = "wasm32")
                    && ui
                        .add_enabled(ready, theme::secondary_button(tr("Save and open")))
                        .clicked()
                {
                    request = Some(true);
                }
            });
            if let Some((path, revision)) = self.printing.saved.get(&kind) {
                let (text, color) = if *revision == self.file.revision {
                    (path.clone(), p.muted)
                } else {
                    (fill("{} · out of date", &[path]), p.warn)
                };
                ui.label(RichText::new(text).size(theme::text::CAPTION).color(color));
            }
        });
        request
    }

    /// The column a print relies on, if nobody has it filled in.
    fn missing_data(&self, kind: PrintKind) -> Option<&'static str> {
        let (field, get): (_, fn(&Attendee) -> &str) = match kind {
            PrintKind::Kitchen => ("food", |a| &a.food),
            PrintKind::Bar => ("drink", |a| &a.drink),
            _ => return None,
        };
        let any = self
            .project
            .attendees
            .iter()
            .any(|a| !get(a).trim().is_empty());
        (!any).then_some(field)
    }

    fn start_print(&mut self, kinds: Vec<PrintKind>, open: bool) {
        let prints = kinds
            .into_iter()
            .map(|kind| {
                let data = crate::print::data(&self.project, self.selected_candidate, kind);
                (
                    kind,
                    data.map(|d| d.to_string()).map_err(|e| format!("{e:#}")),
                )
            })
            .collect();
        self.printing.job = Some((open, crate::platform::PdfJob::start(prints)));
    }

    /// Opens a file or folder with the system's default app.
    fn open_file(&mut self, path: &std::path::Path) {
        if let Err(error) = crate::platform::open_location(path) {
            self.status = format!("{error:#}");
        }
    }

    fn poll_print(&mut self) {
        let Some((open, pdfs)) = self
            .printing
            .job
            .as_mut()
            .and_then(|(open, job)| Some((*open, job.poll()?)))
        else {
            return;
        };
        self.printing.job = None;
        let mut saved = Vec::new();
        let mut failed = Vec::new();
        for (kind, pdf) in pdfs {
            let result = pdf.and_then(|bytes| {
                let name = crate::print::file_name(&self.project, &self.file.name, kind);
                crate::platform::save_file(&self.file.store, &name, &bytes)
                    .map_err(|e| format!("{e:#}"))
            });
            match result {
                Ok(file) => {
                    if open && let Some(path) = &file.location {
                        self.open_file(path);
                    }
                    saved.push(kind.title());
                    self.printing
                        .saved
                        .insert(kind, (file.description, self.file.revision));
                }
                Err(error) => failed.push(fill("{} failed: {}", &[&kind.title(), &error])),
            }
        }
        self.status = match (saved.is_empty(), failed.is_empty()) {
            (_, false) => failed.join("; "),
            (false, true) => fill(
                if cfg!(target_arch = "wasm32") {
                    "Downloaded {}"
                } else {
                    "Saved {}"
                },
                &[&saved.join(", ")],
            ),
            (true, true) => String::new(),
        };
    }
}
