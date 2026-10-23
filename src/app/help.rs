//! Short per-page help: a (?) button that opens a floating panel of bullets.

use eframe::egui::{self, RichText};

use super::theme::{self, Palette};
use crate::i18n::{fill, tr};

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
                "Lower scores are better.",
                "Drag to move or swap. Ctrl-click or Shift-drag to select.",
                "Drag a selection to move it as a block.",
                "Drag people off the tables to seat them later.",
            ],
            HelpPage::People => &["Import people from another file or paste rows at any time."],
            HelpPage::Graph => &[
                "Colours show whether wishes are met.",
                "Drag to rearrange the view.",
                "Scroll to zoom; drag the background to pan.",
            ],
            HelpPage::Categories => &[
                "Merge different names for the same thing.",
                "Categories are used in PDFs.",
            ],
            HelpPage::Policy => &[
                "Policies are preferences; some may be left unmet.",
                "Solve again to apply changes.",
            ],
            HelpPage::Room => &[
                "Quick setup creates equal tables.",
                "People keep their seats when tables change.",
                "Free layout places, turns and rounds tables on a canvas.",
            ],
            HelpPage::Print => &[
                "Create A4 PDFs for the selected option.",
                "Event details appear on each PDF.",
                "Browser: download. Desktop: save in the project's folder.",
            ],
        }
    }

    /// Keyboard shortcuts shown under the bullets: (keys, what they do).
    pub(super) fn shortcuts(self) -> &'static [(&'static str, &'static str)] {
        match self {
            HelpPage::Chart => &[
                ("Ctrl+K", "Find person"),
                ("F", "Fit to window"),
                ("P", "Pin person or selection"),
                ("Delete", "Take person or selection off the seats"),
                ("↑ ↓ ← →", "Move person or selection one seat"),
                ("G", "Group selection"),
                ("C", "Change card colours"),
                ("Esc", "Cancel / clear selection"),
                ("Ctrl+Z / Ctrl+Shift+Z", "Undo / redo"),
            ],
            HelpPage::Graph | HelpPage::People | HelpPage::Categories | HelpPage::Policy => {
                &[("Ctrl+Z / Ctrl+Shift+Z", "Undo / redo")]
            }
            HelpPage::Room => &[
                ("R / Shift+R", "Turn the selected table"),
                ("Ctrl+Z / Ctrl+Shift+Z", "Undo / redo"),
            ],
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
    toggle_button(ui, p, page);
    show_panel(ui.ctx(), p, page)
}

/// The mobile toolbar keeps this button in More, but draws its panel after
/// that menu has closed. Page-scoped state survives the menu's lifetime.
pub(super) fn toggle_button(ui: &mut egui::Ui, p: Palette, page: HelpPage) {
    let id = egui::Id::new(("page-help", page as u8));
    let mut open = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let button = theme::icon_button(ui, p, theme::Icon::Help, tr("Help for this page"), open);
    if button.clicked() {
        open = !open;
    }
    ui.ctx().data_mut(|d| {
        d.insert_temp(id, open);
        d.insert_temp(id.with("anchor"), button.rect);
    });
}

pub(super) fn show_panel(ctx: &egui::Context, p: Palette, page: HelpPage) -> bool {
    let id = egui::Id::new(("page-help", page as u8));
    let (mut open, anchor) = ctx.data(|d| {
        (
            d.get_temp::<bool>(id).unwrap_or(false),
            d.get_temp::<egui::Rect>(id.with("anchor")),
        )
    });
    let mut tour = false;
    if let Some(anchor) = anchor.filter(|_| open) {
        let area = egui::Area::new(id.with("panel"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::RIGHT_TOP)
            .fixed_pos(anchor.right_bottom() + egui::vec2(0.0, theme::space::XS))
            .show(ctx, |ui| {
                theme::floating(p)
                    .inner_margin(egui::Margin::same(theme::space::M as i8))
                    .show(ui, |ui| {
                        ui.set_width(320.0_f32.min(ui.ctx().content_rect().width() - 64.0));
                        let height = (ctx.content_rect().height()
                            - 2.0 * theme::space::L
                            - 2.0 * theme::space::M)
                            .max(80.0);
                        egui::ScrollArea::vertical()
                            .id_salt("help-content")
                            .max_height(height)
                            .show(ui, |ui| tour = panel(ui, p, page));
                    });
            });
        let clicked_outside = ctx.input(|i| {
            i.pointer.any_click()
                && !i
                    .pointer
                    .interact_pos()
                    .is_some_and(|pos| anchor.contains(pos) || area.response.rect.contains(pos))
        });
        if tour || clicked_outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            open = false;
            ctx.request_repaint();
        }
    }
    ctx.data_mut(|d| d.insert_temp(id, open));
    tour
}

fn panel(ui: &mut egui::Ui, p: Palette, page: HelpPage) -> bool {
    ui.add_space(theme::space::XS);
    for bullet in page.bullets() {
        ui.horizontal_top(|ui| {
            ui.label(RichText::new("•").color(p.muted));
            let text = if *bullet == "Drag to move or swap. Ctrl-click or Shift-drag to select." {
                fill(
                    "Drag to move or swap. {}-click or Shift-drag to select.",
                    &[&ui.ctx().format_modifiers(egui::Modifiers::COMMAND)],
                )
            } else {
                tr(bullet).to_owned()
            };
            ui.add(egui::Label::new(text).wrap());
        });
    }
    let shortcuts = page.shortcuts();
    if !shortcuts.is_empty() {
        ui.add_space(theme::space::M);
        theme::section_title(ui, p, tr("Keys"));
        theme::text_rows(ui, |ui| {
            for (keys, what) in shortcuts {
                theme::key_hint(ui, p, &shortcut_keys(ui.ctx(), keys), tr(what));
            }
        });
    }
    page == HelpPage::Chart && ui.ctx().content_rect().width() >= theme::SPLIT_MIN + 84.0 && {
        ui.add_space(theme::space::M);
        ui.add(theme::small_button(tr("Show the tour"))).clicked()
    }
}

/// The documented command shortcuts use the same platform modifier as input.
pub(super) fn shortcut_keys(ctx: &egui::Context, keys: &str) -> String {
    let command = |key, shift| {
        ctx.format_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND
                | if shift {
                    egui::Modifiers::SHIFT
                } else {
                    egui::Modifiers::NONE
                },
            key,
        ))
    };
    match keys {
        "Ctrl+K" => command(egui::Key::K, false),
        "Ctrl+Z / Ctrl+Shift+Z" => format!(
            "{} / {}",
            command(egui::Key::Z, false),
            command(egui::Key::Z, true)
        ),
        _ => keys.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_stays_concise_without_requiring_filler() {
        for page in HelpPage::ALL {
            let n = page.bullets().len();
            assert!(n <= 6, "{page:?} has {n} bullets");
            for bullet in page.bullets() {
                assert!(bullet.len() < 140, "too long: {bullet}");
            }
        }
    }

    /// Help texts are kept untranslated and passed to `tr` when shown, so the
    /// catalog check, which reads the texts written inside `tr` calls, does
    /// not see them.
    #[test]
    fn every_help_text_has_a_finnish_entry() {
        crate::i18n::set(crate::i18n::Lang::Fi);
        for page in HelpPage::ALL {
            let what = page.shortcuts().iter().map(|(_, what)| *what);
            for text in page.bullets().iter().copied().chain(what) {
                assert_ne!(tr(text), text, "not translated: {text}");
            }
        }
    }
}
