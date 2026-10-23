//! A short first-run tour of the chart: a few floating cards, each pointing
//! at one area. Shown once (remembered in the library settings) and replayable
//! from the chart's help.

use eframe::egui::{self, Pos2, Rect, RichText, Vec2};

use super::PlassiApp;
use super::theme::{self, Palette};
use crate::i18n::{fill, tr};

/// Library setting that records the tour as seen.
const DONE_KEY: &str = "tour-done";
const CARD_WIDTH: f32 = 300.0;

/// Areas of the chart page the tour points at, recorded while drawing it.
#[derive(Clone, Copy, Default)]
pub(super) struct Anchors {
    pub(super) options: Option<Rect>,
    pub(super) canvas: Option<Rect>,
    pub(super) panel: Option<Rect>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Target {
    Options,
    Canvas,
    Panel,
}

/// (area, title, text) per step; text is untranslated keys.
const STEPS: [(Target, &str, &str); 5] = [
    (
        Target::Options,
        "Options and scores",
        "Lower scores are better.",
    ),
    (
        Target::Canvas,
        "Move and swap",
        "Drag to move or swap. Scores update immediately.",
    ),
    (
        Target::Panel,
        "Status colours",
        "Stripes show wishes and rule violations.",
    ),
    (
        Target::Canvas,
        "Group or keep apart",
        "{}-click to select, then group (G) or keep apart.",
    ),
    (
        Target::Panel,
        "Side panel",
        "Select a person for wishes and suggested swaps.",
    ),
];

#[derive(Default)]
pub(super) struct TourState {
    /// The step shown, if the tour is running.
    step: Option<usize>,
    /// The first-run check was made this session.
    checked: bool,
    pub(super) anchors: Anchors,
}

impl TourState {
    pub(super) fn start(&mut self) {
        self.step = Some(0);
        self.checked = true;
    }
}

/// The step after `step`, or `None` past the last one.
fn next(step: usize) -> Option<usize> {
    (step + 1 < STEPS.len()).then_some(step + 1)
}

fn back(step: usize) -> usize {
    step.saturating_sub(1)
}

/// Where a card of `size` goes next to `anchor`, kept inside `screen`:
/// below a short area (a toolbar), left of an area on the right (a side
/// panel), otherwise inside the area near its top.
fn place(anchor: Rect, screen: Rect, size: Vec2) -> Pos2 {
    let gap = theme::space::M;
    let pos = if anchor.height() < 120.0 {
        Pos2::new(anchor.left(), anchor.bottom() + gap)
    } else if anchor.left() > screen.center().x {
        Pos2::new(
            anchor.left() - gap - size.x,
            anchor.top() + theme::space::XXL,
        )
    } else {
        Pos2::new(
            anchor.center().x - size.x / 2.0,
            anchor.top() + theme::space::XXL,
        )
    };
    let max = (screen.max - size).max(screen.min);
    pos.clamp(screen.min, max)
}

impl PlassiApp {
    /// Draws the tour over the chart page, starting it on first sight of an option.
    pub(super) fn chart_tour(&mut self, ctx: &egui::Context, p: Palette) {
        if !self.tour.checked && !self.project.candidates.is_empty() {
            self.tour.checked = true;
            if self.file.library.setting(DONE_KEY).is_none() {
                self.tour.step = Some(0);
            }
        }
        let Some(step) = self.tour.step else { return };
        let (target, title, text) = STEPS[step];
        let anchors = self.tour.anchors;
        let anchor = match target {
            Target::Options => anchors.options,
            Target::Canvas => anchors.canvas,
            Target::Panel => anchors.panel.or(anchors.canvas),
        };
        let screen = ctx.content_rect().shrink(theme::space::L);
        let anchor = anchor.unwrap_or(screen);
        let size = ctx
            .memory(|m| m.area_rect(egui::Id::new("chart-tour")).map(|r| r.size()))
            .unwrap_or(Vec2::new(CARD_WIDTH, 160.0));
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("chart-tour-mark"),
        ))
        .rect_stroke(
            anchor.expand(2.0),
            theme::radius::MD,
            egui::Stroke::new(2.0, p.accent),
            egui::StrokeKind::Outside,
        );
        let mut choice = None;
        egui::Area::new(egui::Id::new("chart-tour"))
            .order(egui::Order::Foreground)
            .fixed_pos(place(anchor, screen, size))
            .show(ctx, |ui| {
                theme::floating(p)
                    .inner_margin(egui::Margin::same(theme::space::L as i8))
                    .show(ui, |ui| {
                        ui.set_width(CARD_WIDTH);
                        choice = card(ui, p, step, title, text);
                    });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            choice = Some(Choice::Skip);
        }
        match choice {
            Some(Choice::Next) => self.tour.step = next(step),
            Some(Choice::Back) => self.tour.step = Some(back(step)),
            Some(Choice::Skip) => self.tour.step = None,
            None => {}
        }
        if self.tour.step.is_none() {
            self.file.library.set_setting(DONE_KEY, "1");
        }
    }
}

enum Choice {
    Next,
    Back,
    Skip,
}

fn card(
    ui: &mut egui::Ui,
    p: Palette,
    step: usize,
    title: &'static str,
    text: &'static str,
) -> Option<Choice> {
    let mut choice = None;
    ui.label(RichText::new(tr(title)).size(theme::text::LEAD).strong());
    ui.add_space(theme::space::XS);
    let modifier = ui.ctx().format_modifiers(egui::Modifiers::COMMAND);
    ui.add(egui::Label::new(fill(text, &[&modifier])).wrap());
    ui.add_space(theme::space::M);
    ui.horizontal(|ui| {
        theme::hint(ui, p, fill("Step {} of {}", &[&(step + 1), &STEPS.len()]));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let last = next(step).is_none();
            if ui
                .add(theme::primary_button(
                    if last { tr("Done") } else { tr("Next") },
                    p,
                ))
                .clicked()
            {
                choice = Some(Choice::Next);
            }
            if step > 0 && ui.add(theme::secondary_button(tr("Back"))).clicked() {
                choice = Some(Choice::Back);
            }
            if !last && ui.add(theme::ghost_button(tr("Skip"), p)).clicked() {
                choice = Some(Choice::Skip);
            }
        });
    });
    choice
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_advance_and_end() {
        assert_eq!(next(0), Some(1));
        assert_eq!(next(STEPS.len() - 1), None);
        assert_eq!(back(0), 0);
        assert_eq!(back(3), 2);
    }

    #[test]
    fn cards_go_below_toolbars_and_left_of_side_panels() {
        let screen = Rect::from_min_max(Pos2::ZERO, Pos2::new(1400.0, 900.0));
        let size = Vec2::new(300.0, 150.0);
        let toolbar = Rect::from_min_max(Pos2::new(200.0, 10.0), Pos2::new(600.0, 40.0));
        assert_eq!(
            place(toolbar, screen, size),
            Pos2::new(200.0, 40.0 + theme::space::M)
        );
        let panel = Rect::from_min_max(Pos2::new(1100.0, 60.0), Pos2::new(1400.0, 900.0));
        assert!(place(panel, screen, size).x + size.x <= panel.left());
        // Never off screen.
        let edge = Rect::from_min_max(Pos2::new(1300.0, 860.0), Pos2::new(1400.0, 880.0));
        let at = place(edge, screen, size);
        assert!(screen.contains(at + size));
    }
}
