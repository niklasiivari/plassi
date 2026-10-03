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
                "Solve: new chart. Improve: keep manual seats.",
                "Choose an option. Lower scores are better.",
                "Drag to move or swap. Ctrl-click to select.",
                "Select a person to see wishes.",
            ],
            HelpPage::People => &[
                "Paste rows or drop a CSV/TSV file.",
                "Select a person to edit wishes.",
                "Review notes and add wishes.",
            ],
            HelpPage::Graph => &[
                "Colours show whether wishes are met.",
                "Drag to rearrange the view.",
                "Scroll to zoom; drag the background to pan.",
            ],
            HelpPage::Categories => &[
                "Group meals, drinks and other entries.",
                "Merge different names for the same thing.",
                "Categories are used in PDFs.",
            ],
            HelpPage::Policy => &[
                "Set rules for the whole event.",
                "Solve again to apply changes.",
                "Reset to restore defaults.",
            ],
            HelpPage::Room => &[
                "Set up tables and seats.",
                "Quick setup creates equal tables.",
                "Changing seats clears seating options.",
                "Set tags and custom distances for special seats.",
            ],
            HelpPage::Print => &[
                "Create A4 PDFs for the selected option.",
                "Event details appear on each PDF.",
                "Browser: download. Desktop: save beside the project.",
            ],
        }
    }

    /// Keyboard shortcuts shown under the bullets: (keys, what they do).
    pub(super) fn shortcuts(self) -> &'static [(&'static str, &'static str)] {
        match self {
            HelpPage::Chart => &[
                ("Ctrl+F", "Find person"),
                ("F", "Fit to window"),
                ("P", "Pin person"),
                ("G", "Group selection"),
                ("Esc", "Cancel / clear selection"),
                ("Ctrl+Z / Ctrl+Shift+Z", "Undo / redo"),
            ],
            HelpPage::Graph
            | HelpPage::People
            | HelpPage::Categories
            | HelpPage::Policy
            | HelpPage::Room => &[("Ctrl+Z / Ctrl+Shift+Z", "Undo / redo")],
            HelpPage::Print => &[],
        }
    }
}

/// A page title (with optional subtitle) with the help button on its right.
pub(super) fn page_header(
    ui: &mut egui::Ui,
    p: Palette,
    page: HelpPage,
    title: &str,
    subtitle: Option<&str>,
) {
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
                theme::floating(p)
                    .inner_margin(egui::Margin::same(theme::space::M as i8))
                    .show(ui, |ui| {
                        ui.set_width(320.0);
                        tour = panel(ui, p, page);
                    });
            });
        let clicked_outside = ui.input(|i| i.pointer.any_click())
            && !button.clicked()
            && !ui
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|pos| area.response.rect.contains(pos));
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
            for (keys, what) in shortcuts {
                theme::key_hint(ui, p, keys, tr(what));
            }
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
