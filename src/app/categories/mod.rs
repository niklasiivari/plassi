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
}

impl Default for CategoriesState {
    fn default() -> Self {
        Self {
            kind: Kind::Organization,
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
        let mut changed = false;
        theme::two_columns(ui, |ui, column| {
            if column == 0 {
                changed |= self.merge_suggestions_card(ui, p, kind);
                changed |= self.unmapped_card(ui, p, kind);
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
                let unmapped = self
                    .project
                    .spellings(kind)
                    .iter()
                    .filter(|(s, _)| self.project.catalog.find(kind, s).is_none())
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
                }
            }
        });
    }
}
