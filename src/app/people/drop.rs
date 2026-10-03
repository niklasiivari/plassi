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
        "xlsx" | "xls" | "xlsm" | "ods" => DropKind::Excel,
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

/// Text of a dropped sheet; a byte-order mark is dropped and invalid UTF-8 replaced.
fn sheet_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    text.trim_start_matches('\u{feff}').to_string()
}

impl PlassiApp {
    /// True while a file is dragged over the window.
    pub(super) fn file_hovering(ui: &egui::Ui) -> bool {
        ui.input(|i| !i.raw.hovered_files.is_empty())
    }

    /// Starts reading a dropped .csv/.tsv/.txt, and once read puts it in the
    /// import box, showing the overview so the preview appears before
    /// anything is applied.
    pub(super) fn take_dropped_sheet(&mut self, ui: &egui::Ui) {
        if let Some(file) = ui.input(|i| i.raw.dropped_files.first().cloned()) {
            let name = file_name(file.as_ref());
            match kind_of(&name) {
                DropKind::Text => {
                    self.people.dropped = Some((name, crate::platform::DroppedRead::start(file)))
                }
                DropKind::Excel => {
                    self.status = tr("Save Excel as CSV/TSV, or copy and paste the rows.").into();
                }
                DropKind::Other => {
                    self.status = fill("{} is not a .csv, .tsv or .txt file", &[&name])
                }
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
                self.people.import_text = sheet_text(&bytes);
                self.selected_person = None;
                self.status = fill("Loaded {}. Check the preview and import.", &[&name]);
            }
            Err(error) => self.status = fill("Could not read {}: {}", &[&name, &error]),
        }
        ui.ctx().request_repaint();
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

    #[test]
    fn byte_order_mark_is_removed() {
        assert_eq!(sheet_text("\u{feff}Name\tAvec".as_bytes()), "Name\tAvec");
    }
}
