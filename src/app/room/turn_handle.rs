//! The handle above the selected table in the free layout: dragging it
//! turns the table around its centre.
use super::*;
use crate::app::camera::Viewport;
use crate::app::turned::TableTop;
use crate::model::{TablePosition, normal_degrees};
use egui::{Pos2, Rect, Sense, Stroke, Vec2};

/// Degrees a handle turn snaps to while snapping to the grid.
const TURN_SNAP: f32 = 15.0;
/// Screen pixels between the table's end and the handle.
const HANDLE_GAP: f32 = 28.0;

impl PlassiApp {
    /// The table being turned and how far the pointer has turned it.
    pub(super) fn turned_table(
        &self,
        ui: &egui::Ui,
        view: &Viewport,
        geometry: &super::geometry::RoomGeometry,
    ) -> Option<(String, TablePosition)> {
        let id = self.room.canvas.turning.as_ref()?;
        let frame = geometry.table(id)?.base;
        let pointer = view.world(ui.input(|i| i.pointer.interact_pos())?);
        let rotation = pointer_rotation(frame, pointer, self.room.canvas.snap);
        Some((id.clone(), TablePosition { rotation, ..frame }))
    }

    /// Paints a table's turn handle and starts a turn when it is dragged.
    pub(super) fn turn_handle(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        view: &Viewport,
        p: Palette,
        (table, top): (&TableSpec, &TableTop),
    ) {
        let id = table.id.as_str();
        let centre = view.point(top.top.centre);
        let up = Vec2::angled(top.top.angle - std::f32::consts::FRAC_PI_2);
        let reach = top.top.size.y / 2.0 * view.zoom + HANDLE_GAP;
        let at = centre + up * reach;
        let target = if theme::mobile(ui.ctx()) || ui.input(|i| i.has_touch_screen()) {
            44.0
        } else {
            18.0
        };
        let handle = Rect::from_center_size(at, Vec2::splat(target));
        let response = ui
            .interact(handle, ui.id().with(("turn-table", id)), Sense::drag())
            .on_hover_text(tr("Drag to turn"))
            .on_hover_cursor(egui::CursorIcon::Alias);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                fill("Turn {}", &[&table.label]),
            )
        });
        let stroke = Stroke::new(1.5, p.accent);
        let end = centre + up * (reach - HANDLE_GAP);
        painter.line_segment([end, at - up * 7.0], stroke);
        painter.circle(at, 7.0, p.surface_hi, stroke);
        if response.drag_started() {
            self.room.canvas.turning = Some(id.to_string());
        }
    }
}

/// The rotation that points the table's top end at `pointer`.
fn pointer_rotation(frame: TablePosition, pointer: Pos2, snap: bool) -> f32 {
    let (dx, dy) = (pointer.x - frame.x, pointer.y - frame.y);
    let degrees = dy.atan2(dx).to_degrees() + 90.0;
    let degrees = if snap {
        (degrees / TURN_SNAP).round() * TURN_SNAP
    } else {
        degrees.round()
    };
    normal_degrees(degrees)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_handle_points_the_table_top_at_the_pointer() {
        let frame = TablePosition {
            x: 100.0,
            y: 100.0,
            rotation: 0.0,
        };
        let turn = |x, y, snap| pointer_rotation(frame, Pos2::new(x, y), snap);
        assert_eq!(turn(100.0, 0.0, true), 0.0);
        assert_eq!(turn(200.0, 100.0, true), 90.0);
        assert_eq!(turn(100.0, 200.0, true), 180.0);
        assert_eq!(turn(0.0, 100.0, true), 270.0);
        assert_eq!(turn(200.0, 0.0, true), 45.0);
        // 50° snaps to 45°, or stays 50° without snapping.
        let (sin, cos) = 50_f32.to_radians().sin_cos();
        let (x, y) = (100.0 + 100.0 * sin, 100.0 - 100.0 * cos);
        assert_eq!(turn(x, y, true), 45.0);
        assert_eq!(turn(x, y, false), 50.0);
    }
}
