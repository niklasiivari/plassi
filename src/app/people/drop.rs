//! Spreadsheet files dropped onto the People page go into the import box.

use super::*;
use crate::i18n::{fill, tr};

/// What to do with a dropped file, judged by its name.
#[derive(Debug, PartialEq, Eq)]
enum DropKind {
    Text,
    Excel,
    Other,
}

fn kind_of(name: &str) -> DropKind {
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "tsv" | "csv" | "txt" => DropKind::Text,
        "xlsx" | "xls" | "xlsm" | "xlsb" | "ods" => DropKind::Excel,
        _ => DropKind::Other,
    }
}

/// The file's name (on the web the path is just the name).
fn file_name(file: &dyn egui::DroppedFile) -> String {
    file.path()
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl PlassiApp {
    /// True while a file is dragged over the window.
    pub(super) fn file_hovering(ui: &egui::Ui) -> bool {
        ui.input(|i| !i.raw.hovered_files.is_empty())
    }

    /// Reads picked or dropped spreadsheets into the mapping preview, without
    /// applying anything to the project.
    pub(super) fn take_dropped_sheet(&mut self, ui: &egui::Ui) {
        if let Some(file) = ui.input(|i| i.raw.dropped_files.first().cloned()) {
            let name = file_name(file.as_ref());
            match kind_of(&name) {
                DropKind::Text | DropKind::Excel => {
                    self.people.sheet_picker = None;
                    self.people.dropped = Some((
                        name,
                        crate::platform::DroppedRead::start(file, ui.ctx().clone()),
                    ))
                }
                DropKind::Other => {
                    self.status = tr("Choose an Excel, CSV, TSV or ODS file").into();
                }
            }
        }
        if let Some(picked) = self
            .people
            .sheet_picker
            .as_ref()
            .and_then(|picker| picker.poll())
        {
            self.people.sheet_picker = None;
            match picked {
                Ok(Some(file)) => self.load_sheet_file(&file.name, &file.bytes),
                Ok(None) => {}
                Err(error) => self.status = fill("Import failed: {}", &[&error]),
            }
        }
        let Some(read) = self
            .people
            .dropped
            .as_ref()
            .and_then(|(_, read)| read.poll())
        else {
            return;
        };
        let (name, _) = self.people.dropped.take().expect("a read in progress");
        match read {
            Ok(bytes) => {
                self.load_sheet_file(&name, &bytes);
            }
            Err(error) => self.status = fill("Could not read {}: {}", &[&name, &error]),
        }
        ui.ctx().request_repaint();
    }

    /// File contents enter the same editable mapping preview as pasted rows.
    pub(in crate::app) fn load_sheet_file(&mut self, name: &str, bytes: &[u8]) {
        match crate::import::read_sheets(name, bytes) {
            Ok(sheets) => {
                self.flush_people_details();
                self.people.import_text = sheets[0].1.clone();
                self.people.import_sheets = sheets;
                self.people.import_sheet = 0;
                self.people.import_mapping = Default::default();
                self.people.import_mode = Default::default();
                self.people.import_open = true;
                self.people.roster = None;
                self.status = fill("Loaded {}. Check the preview and import.", &[&name]);
            }
            Err(error) => self.status = fill("Import failed: {}", &[&format!("{error:#}")]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_judged_by_extension() {
        assert_eq!(kind_of("guests.CSV"), DropKind::Text);
        assert_eq!(kind_of("guests.tsv"), DropKind::Text);
        assert_eq!(kind_of("guests.xlsx"), DropKind::Excel);
        assert_eq!(kind_of("guests"), DropKind::Other);
    }
}
