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
    /// Suggested merges, by project revision and kind.
    merges: crate::app::memo::Memo<(u64, Kind), Vec<crate::catalog::Merge>>,
}

impl Default for CategoriesState {
    fn default() -> Self {
        Self {
            kind: Kind::Organization,
            new_name: String::new(),
            mapping_search: String::new(),
            merges: Default::default(),
        }
    }
}

impl PlassiApp {
    pub(in crate::app) fn categories_page(&mut self, ui: &mut egui::Ui, p: Palette) {
        crate::app::help::page_header(
            ui,
            p,
            crate::app::help::HelpPage::Categories,
            tr("Categories"),
            Some(tr("Merge names and set prices.")),
        );
        self.kind_tabs(ui, p);
        ui.add_space(theme::space::XL);
        let kind = self.categories.kind;
        let mut changed = self.create_category_card(ui, p, kind);
        ui.add_space(theme::space::L);
        theme::two_columns(ui, |ui, column| {
            if column == 0 {
                changed |= self.merge_suggestions_card(ui, p, kind);
                changed |= self.imported_entries(ui, p, kind);
            } else {
                changed |= self.category_entries(ui, p, kind);
            }
        });
        if changed {
            self.commit(tr("Saved categories"));
        }
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
                    .filter(|(s, _)| lookup.find_all(s).is_empty())
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
