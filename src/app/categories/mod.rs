//! The Categories page: organizations, ticket types, food and drinks as
//! explicit categories, and which spellings people wrote map to which.

mod entries;
mod unmapped;

use crate::i18n::{fill, tr};
use eframe::egui::{self, RichText};

use crate::catalog::Kind;

use super::PlassiApp;
use super::theme::{self, Palette};

/// State of the Categories page.
pub(in crate::app) struct CategoriesState {
    kind: Kind,
    new_name: String,
    mapping_search: String,
    mapping_filter: MappingFilter,
    choices: std::collections::BTreeMap<(String, String, usize), String>,
    entry_people: Option<(String, Vec<usize>)>,
    manage: bool,
    /// Suggested merges, by project revision and kind.
    merges: crate::app::memo::Memo<(u64, Kind), Vec<crate::catalog::Merge>>,
}

impl CategoriesState {
    pub(in crate::app) fn forget_project(&mut self) {
        self.entry_people = None;
        self.choices.clear();
    }
}

impl Default for CategoriesState {
    fn default() -> Self {
        Self {
            kind: Kind::Organization,
            new_name: String::new(),
            mapping_search: String::new(),
            mapping_filter: MappingFilter::All,
            choices: Default::default(),
            entry_people: None,
            manage: false,
            merges: Default::default(),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum MappingFilter {
    All,
    Unresolved,
    Resolved,
}

impl PlassiApp {
    pub(in crate::app) fn categories_page(&mut self, ui: &mut egui::Ui, p: Palette) {
        crate::app::help::page_header(
            ui,
            p,
            crate::app::help::HelpPage::Categories,
            tr("Categories"),
            None,
        );
        self.kind_tabs(ui, p);
        ui.add_space(theme::space::L);
        let kind = self.categories.kind;
        let mut changed = false;
        let compact = ui.available_width() < theme::TWO_COLUMN_MIN;
        if compact {
            ui.horizontal_wrapped(|ui| {
                for (manage, label) in [(false, "Review entries"), (true, "Manage categories")] {
                    if ui
                        .add(theme::chip(tr(label), p, self.categories.manage == manage))
                        .clicked()
                    {
                        self.categories.manage = manage;
                    }
                }
            });
            ui.add_space(theme::space::L);
            if self.categories.manage {
                changed |= self.category_management(ui, p, kind);
            } else {
                self.category_review_header(ui, p, kind);
                changed |= self.merge_suggestions_card(ui, p, kind);
                changed |= self.imported_entries(ui, p, kind);
            }
        } else {
            theme::two_columns(ui, |ui, column| {
                if column == 0 {
                    self.category_review_header(ui, p, kind);
                    changed |= self.merge_suggestions_card(ui, p, kind);
                    changed |= self.imported_entries(ui, p, kind);
                } else {
                    changed |= self.category_management(ui, p, kind);
                }
            });
        }
        if changed {
            self.commit(tr("Saved categories"));
        }
        self.entry_people_window(ui, p);
    }

    fn category_management(&mut self, ui: &mut egui::Ui, p: Palette, kind: Kind) -> bool {
        ui.label(
            RichText::new(tr("Manage categories"))
                .size(theme::text::LEAD)
                .strong(),
        );
        ui.add_space(theme::space::M);
        let mut changed = self.create_category_card(ui, p, kind);
        ui.add_space(theme::space::L);
        changed |= self.category_entries(ui, p, kind);
        changed
    }

    fn category_review_header(&self, ui: &mut egui::Ui, p: Palette, kind: Kind) {
        let spellings = self.project.spellings(kind);
        let lookup = self.project.catalog.lookup(kind);
        let pending = spellings
            .iter()
            .filter(|(name, _)| {
                lookup.find_all(name).is_empty() && !self.project.catalog.is_ignored(kind, name)
            })
            .count();
        ui.label(
            RichText::new(tr("Review imported entries"))
                .size(theme::text::LEAD)
                .strong(),
        );
        theme::hint(ui, p, tr("Original sign-up text is kept."));
        ui.add_space(theme::space::M);
        if spellings.is_empty() {
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(tr("No imported entries for this type yet."));
            });
        } else {
            ui.horizontal_wrapped(|ui| {
                theme::status_pill(
                    ui,
                    p,
                    if pending == 0 {
                        tr("All entries categorized").to_string()
                    } else {
                        fill("{} entries to categorize", &[&pending])
                    },
                    if pending == 0 { p.good } else { p.warn },
                );
                theme::hint(ui, p, fill("{} imported entries", &[&spellings.len()]));
            });
        }
        ui.add_space(theme::space::L);
    }

    /// One tab per kind; the count shows spellings still to map.
    fn kind_tabs(&mut self, ui: &mut egui::Ui, p: Palette) {
        ui.horizontal_wrapped(|ui| {
            for kind in Kind::ALL {
                let lookup = self.project.catalog.lookup(kind);
                let unmapped = self
                    .project
                    .spellings(kind)
                    .iter()
                    .filter(|(s, _)| {
                        lookup.find_all(s).is_empty() && !self.project.catalog.is_ignored(kind, s)
                    })
                    .count();
                let active = self.categories.kind == kind;
                let mut label = egui::text::LayoutJob::default();
                let color = if active { p.accent } else { p.text };
                label.append(
                    kind.plural(),
                    0.0,
                    egui::TextFormat::simple(egui::FontId::proportional(theme::text::BODY), color),
                );
                if unmapped > 0 {
                    let count = egui::TextFormat::simple(
                        egui::FontId::proportional(theme::text::SMALL),
                        p.warn,
                    );
                    label.append(&format!("  {unmapped}"), 0.0, count);
                }
                let tab = theme::chip(label, p, active);
                let hint = if unmapped > 0 {
                    fill("{} uncategorized entries", &[&unmapped])
                } else {
                    tr("All categorized").into()
                };
                if ui.add(tab).on_hover_text(hint).clicked() {
                    self.categories.kind = kind;
                    self.categories.new_name.clear();
                    self.categories.mapping_search.clear();
                }
            }
        });
    }

    fn create_category_card(&mut self, ui: &mut egui::Ui, p: Palette, kind: Kind) -> bool {
        let mut create = false;
        let organization = kind == Kind::Organization;
        let label = if organization {
            tr("Create organization")
        } else {
            tr("Create category")
        };
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(label).strong().size(theme::text::LEAD));
            ui.add_space(theme::space::S);
            let name = &mut self.categories.new_name;
            let key = crate::catalog::normalize(name);
            let duplicate = self
                .project
                .catalog
                .list(kind)
                .iter()
                .any(|c| crate::catalog::normalize(&c.name) == key);
            ui.horizontal_wrapped(|ui| {
                let field_label = if organization {
                    tr("Organization name")
                } else {
                    tr("Category name")
                };
                let width = ui.available_width().min(320.0);
                let response = ui.add(
                    theme::field(name)
                        .hint_text(field_label)
                        .desired_width(width),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, field_label)
                });
                let ready = !key.is_empty() && !duplicate;
                create = ui
                    .add_enabled(ready, theme::primary_button(label, p))
                    .clicked()
                    || (ready
                        && response.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            });
            if !key.is_empty() && duplicate {
                theme::hint(ui, p, tr("A category with this name already exists."));
            }
        });
        if create
            && self
                .project
                .catalog
                .create_named(kind, &self.categories.new_name)
                .is_some()
        {
            self.categories.new_name.clear();
            return true;
        }
        false
    }
}

impl PlassiApp {
    fn entry_people_window(&mut self, ui: &mut egui::Ui, p: Palette) {
        let Some((entry, ids)) = self.categories.entry_people.clone() else {
            return;
        };
        let mut open = true;
        let mut selected = None;
        egui::Window::new(tr("People with this entry"))
            .id(egui::Id::new("entry-people"))
            .open(&mut open)
            .default_width(320.0)
            .max_width((ui.ctx().content_rect().width() - 32.0).max(1.0))
            .show(ui.ctx(), |ui| {
                ui.add(egui::Label::new(RichText::new(&entry).strong()).wrap());
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .show(ui, |ui| {
                        for id in ids {
                            if let Some(person) = self.project.person(id) {
                                if ui.link(&person.name).clicked() {
                                    selected = Some(id);
                                }
                                theme::hint(ui, p, self.project.affiliation(person));
                            }
                        }
                    });
            });
        if let Some(id) = selected {
            self.people.end_review();
            self.page = crate::app::Page::People;
            self.selected_person = Some(id);
            self.people.import_open = false;
            self.people.roster = None;
            self.people.wish_draft = None;
            open = false;
        }
        if !open {
            self.categories.entry_people = None;
        }
    }
}
