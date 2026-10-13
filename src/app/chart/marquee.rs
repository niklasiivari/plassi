//! Box selection: Shift- or Ctrl-drag on the chart floor adds everyone whose
//! card the box touches to the selection.

use super::*;
use crate::app::camera::Viewport;

impl PlassiApp {
    /// Starts a box when a drag on the floor begins with Shift or Ctrl held.
    /// Called before the view pans, so that drag does not pan it.
    pub(super) fn start_marquee(
        &mut self,
        ui: &egui::Ui,
        background: &egui::Response,
        view: &Viewport,
    ) {
        let adding = ui.input(|i| i.modifiers.command || i.modifiers.shift);
        if background.drag_started_by(egui::PointerButton::Primary)
            && adding
            && let Some(at) = ui.input(|i| i.pointer.press_origin())
        {
            self.chart.marquee = Some(view.world(at));
        }
    }

    /// Draws the box while it is dragged and selects when it is let go.
    pub(super) fn marquee(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        view: &Viewport,
        p: Palette,
        scene: &Scene,
    ) {
        let Some(start) = self.chart.marquee else {
            return;
        };
        let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) else {
            return;
        };
        let world = Rect::from_two_pos(start, view.world(pointer));
        if ui.input(|i| i.pointer.primary_down()) {
            painter.rect(
                view.rect(world),
                CornerRadius::same(theme::radius::SM),
                p.accent.gamma_multiply(0.12),
                Stroke::new(1.0, p.accent),
                StrokeKind::Inside,
            );
            return;
        }
        self.chart.marquee = None;
        self.chart.multi.extend(people_in(scene, world));
    }
}

/// Everyone on the chart whose card overlaps `area`, in chart units.
fn people_in(scene: &Scene, area: Rect) -> Vec<usize> {
    let size = Vec2::new(CARD_W, CARD_H);
    scene
        .seat_of
        .keys()
        .chain(scene.loose.keys())
        .copied()
        .filter(|person| {
            scene
                .card_centre(*person)
                .is_some_and(|c| Rect::from_center_size(c, size).intersects(area))
        })
        .collect()
}
