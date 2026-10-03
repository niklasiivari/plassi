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
mod roster;
mod rule_card;
pub(in crate::app) use roster::RosterEdit;
pub(in crate::app) use rule_card::costs_editor;
mod wish_row;

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
    pub(in crate::app) new_tag: String,
    /// Spreadsheet text pasted for import.
    pub(in crate::app) import_text: String,
    /// Column meanings chosen for the pasted sheet.
    pub(in crate::app) import_mapping: import::ImportMapping,
    /// A dropped sheet being read: its name and the read.
    pub(in crate::app) dropped: Option<(String, crate::platform::DroppedRead)>,
    /// Suggested wishes, keyed by project revision.
    pub(in crate::app) suggest_cache: crate::app::memo::Memo<u64, Vec<crate::suggest::Suggestion>>,
    /// An add, replace or remove being prepared.
    pub(in crate::app) roster: Option<roster::RosterEdit>,
    /// The one rule whose full editor is open.
    pub(in crate::app) editing_rule: Option<u64>,
    /// Food, drinks and allergies are being edited.
    pub(in crate::app) editing_details: bool,
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
    pub avec: bool,
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
        egui::Panel::left("people-list")
            .resizable(true)
            .default_size(theme::PANEL_WIDTH + 2.0 * theme::space::M)
            .size_range(280.0..=500.0)
            .show_separator_line(true)
            .frame(theme::floating_panel_frame(p))
            .show(ui, |ui| {
                theme::panel_card(ui, p, theme::space::L, |ui| self.people_list(ui, p))
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.bg))
            .show(ui, |ui| {
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

    fn person_names(&self) -> std::collections::BTreeMap<usize, String> {
        self.project
            .attendees
            .iter()
            .map(|a| (a.id, a.name.clone()))
            .collect()
    }
}
