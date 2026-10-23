//! The People & wishes page: attendee list, per-person editor, review of sign-up text and spreadsheet import.

mod bulk;
mod details;
mod draft;
mod drop;
mod editor;
mod import;
mod list;
mod notes;
mod overview;
mod review;
mod roster;
mod rule_card;
pub(in crate::app) use roster::RosterEdit;
mod tags;
mod wish_row;

use crate::i18n::tr;
use rule_card::*;

use eframe::egui::{self, Color32, RichText, Sense};

use crate::model::{ImportField, RuleKind, Target};

use super::PlassiApp;
use super::theme::{self, Palette};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::app) enum PeopleFilter {
    #[default]
    All,
    ToReview,
    WithWishes,
    Unconstrained,
    /// Some wish or rule not fully met in the selected option.
    Unmet,
    /// Without a seat in the selected option.
    NotSeated,
    Pinned,
}

/// State of the People & wishes page.
#[derive(Default)]
pub(in crate::app) struct PeopleState {
    pub(in crate::app) selecting: bool,
    review: Option<Vec<usize>>,
    pub(in crate::app) scroll_to_person: bool,
    pub(in crate::app) filter: PeopleFilter,
    pub(in crate::app) search: String,
    /// People Ctrl-clicked in the list, for bulk actions.
    pub(in crate::app) picked: std::collections::BTreeSet<usize>,
    /// People in the list as last shown, for "select all".
    pub(in crate::app) shown: Vec<usize>,
    /// Search in the existing wish editor.
    pub(in crate::app) picker_filter: String,
    /// Search in the new-wish form, independent of the existing wish editor.
    pub(in crate::app) draft_picker_filter: String,
    pub(in crate::app) wish_draft: Option<WishDraft>,
    /// Spreadsheet text pasted for import.
    pub(in crate::app) import_text: String,
    /// Column meanings chosen for the pasted sheet.
    pub(in crate::app) import_mapping: import::ImportMapping,
    /// Parsed sheet and validated preview, retained until an input changes.
    import_cache: import::ImportCache,
    /// A bulk import can be opened even while a person is selected.
    pub(in crate::app) import_open: bool,
    pub(in crate::app) import_mode: crate::import::ImportMode,
    pub(in crate::app) sheet_picker: Option<crate::platform::SheetPick>,
    /// Worksheet text retained while choosing a tab in an uploaded workbook.
    pub(in crate::app) import_sheets: Vec<(String, String)>,
    pub(in crate::app) import_sheet: usize,
    /// A dropped sheet being read: its name and the read.
    pub(in crate::app) dropped: Option<(String, crate::platform::DroppedRead)>,
    /// Suggested wishes, keyed by project revision.
    pub(in crate::app) suggest_cache: crate::app::memo::Memo<u64, Vec<crate::suggest::Suggestion>>,
    /// What each note names, kept across revisions.
    pub(in crate::app) suggester: crate::suggest::Suggester,
    /// Suggestions for the note being reviewed, without rematching on every frame.
    note_suggestions: crate::app::memo::Memo<(u64, usize), Vec<crate::suggest::Suggestion>>,
    /// Attendee list values, without repeating membership scans on every frame.
    list_cache: crate::app::memo::Memo<u64, std::rc::Rc<list::ListInfo>>,
    list_matches:
        crate::app::memo::Memo<(u64, usize, PeopleFilter, String), std::rc::Rc<Vec<usize>>>,
    /// An add, replace or remove being prepared.
    pub(in crate::app) roster: Option<roster::RosterEdit>,
    /// The one rule whose full editor is open.
    pub(in crate::app) editing_rule: Option<u64>,
    /// Food, drinks and allergies are being edited.
    pub(in crate::app) editing_details: bool,
    /// Details changed while a field is still focused; save when editing ends.
    pub(in crate::app) details_dirty: bool,
}

impl PeopleState {
    pub(in crate::app) fn clear_import(&mut self) {
        self.import_text.clear();
        self.import_mapping = Default::default();
        self.import_cache = Default::default();
        self.import_open = false;
        self.import_mode = Default::default();
        self.import_sheets.clear();
        self.import_sheet = 0;
        self.sheet_picker = None;
        self.dropped = None;
    }
}

/// Who a new wish is about.
#[derive(Clone, Default, PartialEq)]
pub(in crate::app) enum DraftTarget {
    #[default]
    People,
    /// Everyone with this tag.
    Tag(String),
    /// Everyone in this organization (category or spelling).
    Organization(String),
}

/// A wish being created in the person editor.
#[derive(Clone)]
pub(in crate::app) struct WishDraft {
    pub person: usize,
    pub wish: crate::model::WishKind,
    pub near: DraftTarget,
    /// Chosen people when `near` is [`DraftTarget::People`].
    pub targets: Vec<usize>,
    pub source: String,
    /// The note this draft came from, marked reviewed on save.
    pub note: Option<usize>,
}

impl PlassiApp {
    pub(in crate::app) fn people_page(&mut self, ui: &mut egui::Ui, p: Palette) {
        self.take_dropped_sheet(ui);
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.people.picked.clear();
            self.people.selecting = false;
        }
        if ui.available_width() < theme::SPLIT_MIN {
            self.mobile_people_page(ui, p);
            return;
        }
        let list_width = (ui.available_width() * 0.4).clamp(260.0, theme::PANEL_WIDTH + 24.0);
        let max_list = (ui.available_width() - 360.0).clamp(260.0, 500.0);
        egui::Panel::left("people-list")
            .resizable(true)
            .default_size(list_width)
            .size_range(260.0..=max_list)
            .show_separator_line(true)
            .frame(theme::floating_panel_frame(p))
            .show(ui, |ui| {
                theme::panel_card(ui, p, theme::space::L, |ui| self.people_list(ui, p, true))
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.bg))
            .show(ui, |ui| {
                if self.people.import_open {
                    ui.push_id("bulk-import", |ui| {
                        crate::app::shell::page_scroll(ui, theme::CONTENT_WIDTH, |ui| {
                            self.import_panel(ui, p);
                        });
                    });
                    return;
                }
                crate::app::shell::page_scroll(ui, theme::CONTENT_WIDTH, |ui| {
                    if self.roster_form(ui, p) {
                        return;
                    }
                    match self
                        .selected_person
                        .filter(|id| self.project.person(*id).is_some())
                    {
                        Some(id) => self.person_editor(ui, p, id),
                        None => self.people_overview(ui, p),
                    }
                });
            });
    }

    fn mobile_people_page(&mut self, ui: &mut egui::Ui, p: Palette) {
        // Actions reached from other pages can open an editor or an import directly.
        let editing = self.selected_person.is_some()
            || self.people.import_open
            || self.people.roster.is_some();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.bg))
            .show(ui, |ui| {
                if !editing {
                    // Short screens need to scroll the filters and bulk actions too.
                    egui::ScrollArea::vertical()
                        .id_salt("people-mobile-list")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            egui::Frame::new()
                                .inner_margin(16)
                                .show(ui, |ui| self.people_list(ui, p, false));
                        });
                    return;
                }
                if ui.button(tr("Back to people")).clicked() {
                    self.flush_people_details();
                    self.people.import_open = false;
                    self.people.roster = None;
                    self.selected_person = None;
                }
                crate::app::shell::page_scroll(ui, theme::CONTENT_WIDTH, |ui| {
                    if self.people.import_open {
                        self.import_panel(ui, p);
                    } else if !self.roster_form(ui, p) {
                        if let Some(id) = self
                            .selected_person
                            .filter(|id| self.project.person(*id).is_some())
                        {
                            self.person_editor(ui, p, id);
                        } else {
                            self.people_overview(ui, p);
                        }
                    }
                });
            });
    }

    fn person_names(&self) -> std::collections::BTreeMap<usize, String> {
        self.project
            .attendees
            .iter()
            .map(|a| (a.id, a.name.clone()))
            .collect()
    }
}
