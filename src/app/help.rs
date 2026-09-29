//! Short per-page help: a (?) button that opens a floating panel of bullets.

use eframe::egui::{self, RichText};

use super::theme::{self, Palette};
use crate::i18n::tr;

/// Pages that have help.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum HelpPage {
    Chart,
    People,
    Graph,
    Categories,
    Policy,
    Room,
    Print,
}

impl HelpPage {
    #[cfg(test)]
    pub(super) const ALL: [HelpPage; 7] = [
        HelpPage::Chart,
        HelpPage::People,
        HelpPage::Graph,
        HelpPage::Categories,
        HelpPage::Policy,
        HelpPage::Room,
        HelpPage::Print,
    ];

    /// What the page is for and its main actions (untranslated keys).
    pub(super) fn bullets(self) -> &'static [&'static str] {
        match self {
            HelpPage::Chart => &[
                "The chart shows one option: who sits where.",
                "Solve finds a chart; Different option and Improve make new options from it.",
                "Each chip is an option with its wishes met. Its score counts what is unmet, so lower is better.",
                "Drag a card to move or swap people. Ctrl-click several people to group them (G) or keep them apart.",
                "Click a person to see their wishes in the side panel.",
            ],
            HelpPage::People => &[
                "The guest list and each person's wishes and rules.",
                "Paste spreadsheet rows, or drop a .csv/.tsv file, to add or update people.",
                "Select a person on the left to edit their wishes.",
                "Review sign-up text to turn notes into wishes.",
            ],
            HelpPage::Graph => &[
                "Who wished to sit with whom, as a graph.",
                "Line colours show how well each wish is met in the selected option.",
                "Drag people to rearrange the picture; click to select.",
                "Scroll to zoom, drag the background to pan.",
            ],
            HelpPage::Categories => &[
                "Categories such as food, drinks and allergies, taken from the sheet.",
                "Merge spellings of the same value into one entry.",
                "The counts go on the printed lists.",
            ],
            HelpPage::Policy => &[
                "Rules for everyone, mostly for people without wishes of their own.",
                "They are weighed against wishes when you solve.",
                "Changes apply at the next Solve or Improve.",
            ],
            HelpPage::Room => &[
                "The tables and seats of the room.",
                "Quick setup makes a room of equal tables in one step.",
                "Changing tables or seats removes the existing options.",
                "Seat tags and relationships are for special seats.",
            ],
            HelpPage::Print => &[
                "A4 PDFs of the selected option: seating chart, place cards and lists.",
                "Event details appear on the prints.",
                "In the browser the files are downloaded; on desktop they are saved next to the project file.",
            ],
        }
    }

    /// Keyboard shortcuts shown under the bullets: (keys, what they do).
    pub(super) fn shortcuts(self) -> &'static [(&'static str, &'static str)] {
        match self {
            HelpPage::Chart => &[
                ("Ctrl+F", "Find a person"),
                ("F", "Fit the room to the window"),
                ("P", "Pin the selected person"),
                ("G", "Group the picked people"),
                ("Esc", "Cancel or clear the selection"),
                ("Ctrl+Z / Ctrl+Shift+Z", "Undo / redo"),
            ],
            HelpPage::Graph | HelpPage::People | HelpPage::Categories | HelpPage::Policy | HelpPage::Room => {
                &[("Ctrl+Z / Ctrl+Shift+Z", "Undo / redo")]
            }
            HelpPage::Print => &[],
        }
    }
}

/// A page title (with optional subtitle) with the help button on its right.
pub(super) fn page_header(ui: &mut egui::Ui, p: Palette, page: HelpPage, title: &str, subtitle: Option<&str>) {
    theme::page_header(ui, p, title, subtitle, |ui| {
        help_button(ui, p, page);
    });
}

/// The (?) button and its panel. Returns true when "Show the tour" was
/// clicked (offered on the chart only).
pub(super) fn help_button(ui: &mut egui::Ui, p: Palette, page: HelpPage) -> bool {
    let id = ui.id().with(("help", page as u8));
    let mut open = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let button = theme::icon_button(ui, p, theme::Icon::Help, tr("Help for this page"), open);
    if button.clicked() {
        open = !open;
    }
    let mut tour = false;
    if open {
        let area = egui::Area::new(id.with("panel"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::RIGHT_TOP)
            .fixed_pos(button.rect.right_bottom() + egui::vec2(0.0, theme::space::XS))
            .show(ui.ctx(), |ui| {
                theme::floating(p).show(ui, |ui| {
                    ui.set_width(320.0);
                    tour = panel(ui, p, page);
                });
            });
        let clicked_outside = ui.input(|i| i.pointer.any_click())
            && !button.clicked()
            && !ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| area.response.rect.contains(pos));
        if tour || clicked_outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            open = false;
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, open));
    tour
}

fn panel(ui: &mut egui::Ui, p: Palette, page: HelpPage) -> bool {
    ui.add_space(theme::space::XS);
    for bullet in page.bullets() {
        ui.horizontal_top(|ui| {
            ui.label(RichText::new("•").color(p.muted));
            ui.add(egui::Label::new(tr(bullet)).wrap());
        });
    }
    let shortcuts = page.shortcuts();
    if !shortcuts.is_empty() {
        ui.add_space(theme::space::M);
        theme::section_title(ui, p, tr("Keys"));
        theme::text_rows(ui, |ui| {
            egui::Grid::new("help-keys").num_columns(2).spacing([theme::space::M, theme::space::XS]).show(ui, |ui| {
                for (keys, what) in shortcuts {
                    ui.label(RichText::new(*keys).monospace().size(theme::text::SMALL));
                    theme::hint(ui, p, tr(what));
                    ui.end_row();
                }
            });
        });
    }
    page == HelpPage::Chart && {
        ui.add_space(theme::space::M);
        ui.add(theme::small_button(tr("Show the tour"))).clicked()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_has_a_few_short_bullets() {
        for page in HelpPage::ALL {
            let n = page.bullets().len();
            assert!((3..=6).contains(&n), "{page:?} has {n} bullets");
            for bullet in page.bullets() {
                assert!(bullet.len() < 140, "too long: {bullet}");
            }
        }
    }
}
