//! The room's walls in the free layout: a rectangle around the tables,
//! resized from any corner or side.
use super::layout_canvas::{GRID, snapped};
use super::*;
use crate::app::camera::Viewport;
use crate::app::turned::TableTop;
use crate::model::RoomOutline;
use egui::{Pos2, Rect, Sense, Stroke, Vec2};

/// The grip being dragged, the walls before the drag, and where in the
/// world the drag started.
pub(super) struct OutlineDrag {
    grip: Grip,
    original: RoomOutline,
    start: Pos2,
}

/// Which edges a handle moves: -1 the left or top, 1 the right or bottom,
/// 0 neither.
#[derive(Clone, Copy, PartialEq)]
struct Grip {
    x: i8,
    y: i8,
}

const GRIPS: [Grip; 8] = [
    Grip { x: -1, y: -1 },
    Grip { x: 0, y: -1 },
    Grip { x: 1, y: -1 },
    Grip { x: 1, y: 0 },
    Grip { x: 1, y: 1 },
    Grip { x: 0, y: 1 },
    Grip { x: -1, y: 1 },
    Grip { x: -1, y: 0 },
];

/// Smallest side of the room, in world units.
const MIN_SIDE: f32 = 2.0 * GRID;
/// Room left between the tables and new walls, in world units.
const WALL_MARGIN: f32 = 4.0 * GRID;

pub(super) fn world_rect(room: RoomOutline) -> Rect {
    Rect::from_min_size(
        Pos2::new(room.x, room.y),
        Vec2::new(room.width, room.height),
    )
}

impl Grip {
    fn point(self, walls: Rect) -> Pos2 {
        let pick = |side: i8, min: f32, max: f32| match side {
            -1 => min,
            1 => max,
            _ => (min + max) / 2.0,
        };
        Pos2::new(
            pick(self.x, walls.min.x, walls.max.x),
            pick(self.y, walls.min.y, walls.max.y),
        )
    }

    fn cursor(self) -> egui::CursorIcon {
        use egui::CursorIcon::*;
        match (self.x, self.y) {
            (0, _) => ResizeVertical,
            (_, 0) => ResizeHorizontal,
            (x, y) if x == y => ResizeNwSe,
            _ => ResizeNeSw,
        }
    }
}

impl PlassiApp {
    /// The walls switch. Returns whether it changed the project.
    pub(super) fn outline_controls(&mut self, ui: &mut egui::Ui) -> bool {
        let before = self.project.layout.room;
        let mut on = before.is_some();
        ui.checkbox(&mut on, tr("Room walls"));
        match (on, before.is_some()) {
            (true, false) => {
                self.project.layout.room = Some(self.room_around_tables());
                self.room.canvas.fit_pending = true;
            }
            (false, true) => self.project.layout.room = None,
            _ => {}
        }
        self.project.layout.room != before
    }

    /// Walls a little clear of the tables, on the grid.
    fn room_around_tables(&self) -> RoomOutline {
        let layout = &self.project.layout;
        let frames = layout.table_frames();
        let tables = layout
            .rows
            .iter()
            .flat_map(|r| &r.tables)
            .map(|t| TableTop::new(t, frames[&t.id], 0.0).bounds())
            .fold(Rect::NOTHING, Rect::union);
        let area = if tables.is_finite() {
            tables.expand(WALL_MARGIN)
        } else {
            Rect::from_min_size(Pos2::ZERO, Vec2::splat(20.0 * GRID))
        };
        let (min, max) = (
            (area.min / GRID).floor() * GRID,
            (area.max / GRID).ceil() * GRID,
        );
        RoomOutline {
            x: min.x,
            y: min.y,
            width: max.x - min.x,
            height: max.y - min.y,
        }
    }

    /// Paints the walls and their handles, and resizes them. Returns
    /// whether a finished drag changed them.
    pub(super) fn outline_handles(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        view: &Viewport,
        p: Palette,
    ) -> bool {
        let Some(saved) = self.project.layout.room else {
            self.room.canvas.outline_drag = None;
            return false;
        };
        let snap = self.room.canvas.snap;
        let pointer = ui
            .input(|i| i.pointer.interact_pos())
            .map(|at| view.world(at));
        let room = match (&self.room.canvas.outline_drag, pointer) {
            (Some(drag), Some(at)) => dragged(drag, at, snap),
            _ => saved,
        };
        let walls = view.rect(world_rect(room));
        painter.rect_stroke(
            walls,
            2.0,
            Stroke::new(2.0, p.muted),
            egui::StrokeKind::Inside,
        );
        for grip in GRIPS {
            let handle = Rect::from_center_size(grip.point(walls), Vec2::splat(14.0));
            let response = ui
                .interact(
                    handle,
                    ui.id().with(("room-grip", grip.x, grip.y)),
                    Sense::drag(),
                )
                .on_hover_cursor(grip.cursor());
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tr("Resize room walls"))
            });
            painter.rect_filled(handle.shrink(3.0), 2.0, p.accent);
            if response.drag_started()
                && let Some(start) = pointer
            {
                self.room.canvas.outline_drag = Some(OutlineDrag {
                    grip,
                    original: saved,
                    start,
                });
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.room.canvas.outline_drag = None;
            return false;
        }
        if self.room.canvas.outline_drag.is_some() && ui.input(|i| i.pointer.any_released()) {
            self.room.canvas.outline_drag = None;
            self.project.layout.room = Some(room);
            return room != saved;
        }
        false
    }
}

/// The walls with the dragged edges moved to follow `at`.
fn dragged(drag: &OutlineDrag, at: Pos2, snap: bool) -> RoomOutline {
    let delta = at - drag.start;
    let walls = world_rect(drag.original);
    let (mut min, mut max) = (walls.min, walls.max);
    match drag.grip.x {
        -1 => min.x = snapped(min.x + delta.x, snap).min(max.x - MIN_SIDE),
        1 => max.x = snapped(max.x + delta.x, snap).max(min.x + MIN_SIDE),
        _ => {}
    }
    match drag.grip.y {
        -1 => min.y = snapped(min.y + delta.y, snap).min(max.y - MIN_SIDE),
        1 => max.y = snapped(max.y + delta.y, snap).max(min.y + MIN_SIDE),
        _ => {}
    }
    RoomOutline {
        x: min.x,
        y: min.y,
        width: max.x - min.x,
        height: max.y - min.y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_grip_moves_only_its_own_edges() {
        let original = RoomOutline {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 200.0,
        };
        let drag = |x, y, by: Vec2| {
            let drag = OutlineDrag {
                grip: Grip { x, y },
                original,
                start: Pos2::ZERO,
            };
            dragged(&drag, Pos2::ZERO + by, true)
        };
        let room = drag(-1, -1, Vec2::new(40.0, -80.0));
        assert_eq!(
            (room.x, room.y, room.width, room.height),
            (56.0, -56.0, 344.0, 256.0)
        );
        let room = drag(0, 1, Vec2::new(120.0, 40.0));
        assert_eq!(
            (room.x, room.y, room.width, room.height),
            (0.0, 0.0, 400.0, 224.0)
        );
        let room = drag(1, 0, Vec2::new(-1000.0, 0.0));
        assert_eq!(room.width, MIN_SIDE);
    }
}
