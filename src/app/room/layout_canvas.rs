//! Room designer. Table positions belong to the project; camera and in-progress
//! gestures belong to the editor, so a drag is saved once on release.
use super::*;
use crate::app::camera::{Camera, Viewport};
use crate::app::turned::TableTop;
use crate::model::TablePosition;
use egui::{Align2, FontId, Pos2, Rect, Sense, Stroke, Vec2};

pub(super) struct LayoutCanvas {
    camera: Camera,
    pub(super) fit_pending: bool,
    pub(super) snap: bool,
    drag: Option<(String, TablePosition, Pos2)>,
    pub(super) outline_drag: Option<outline::OutlineDrag>,
    /// The table whose turn handle is being dragged.
    pub(super) turning: Option<String>,
    project: String,
    size: Option<Vec2>,
    pub(super) selected: Option<String>,
    pub(super) selected_label: Option<u64>,
    pub(super) label_drag: Option<(u64, crate::model::Point, Pos2)>,
    /// Tables too close for their name cards on the chart, as last drawn.
    pub(super) crowded: std::collections::BTreeSet<String>,
    context_at: Option<TablePosition>,
    pub(super) center: Option<TablePosition>,
}

impl Default for LayoutCanvas {
    fn default() -> Self {
        Self {
            camera: Camera::new(0.02, 3.0),
            fit_pending: true,
            snap: true,
            drag: None,
            outline_drag: None,
            turning: None,
            project: String::new(),
            size: None,
            selected: None,
            selected_label: None,
            label_drag: None,
            crowded: Default::default(),
            context_at: None,
            center: None,
        }
    }
}

impl LayoutCanvas {
    pub(super) fn camera_navigation(&self) -> bool {
        self.camera.is_touch_navigation()
    }
}

pub(super) const GRID: f32 = crate::model::SEAT_PITCH;
const CARD_MARGIN: f32 = 190.0;
/// How far a long table's top reaches past its end seats.
const TABLE_END: f32 = 28.0;

pub(super) fn snapped(value: f32, snap: bool) -> f32 {
    if snap {
        (value / GRID).round() * GRID
    } else {
        value
    }
}

/// Anchor the first seat row / upper-left edge, so unlike-length tables can
/// line up their end seats instead of their centres.
pub(super) fn snap_table(table: &TableSpec, position: TablePosition, snap: bool) -> TablePosition {
    if !snap {
        return position;
    }
    let (across, along) = table.half_size();
    let (x, y) = position.place(-across, -along);
    TablePosition {
        x: position.x + snapped(x, snap) - x,
        y: position.y + snapped(y, snap) - y,
        ..position
    }
}

fn at(point: Pos2) -> TablePosition {
    TablePosition {
        x: point.x,
        y: point.y,
        rotation: 0.0,
    }
}

impl PlassiApp {
    pub(super) fn layout_controls(&mut self, ui: &mut egui::Ui) -> RoomEdit {
        let before = self.project.layout.free_layout;
        let mut free = before;
        let mut add = false;
        let mut edit = RoomEdit::default();
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut free, false, tr("Rows"));
            ui.selectable_value(&mut free, true, tr("Free layout"));
            if free {
                add = ui.button(tr("+ Add table")).clicked();
                if ui.button(tr("+ Add text")).clicked() {
                    edit.labels |= self.add_room_label();
                }
                ui.checkbox(&mut self.room.canvas.snap, tr("Snap to grid"))
                    .on_hover_text(tr("Align table edges and end seats to the grid"));
                if ui.button(tr("Fit room")).clicked() {
                    self.room.canvas.fit_pending = true;
                }
                edit.labels |= self.outline_controls(ui);
            }
        });
        if free != before {
            self.project.layout.set_free_layout(free);
            self.room.canvas.fit_pending = true;
            self.room.canvas.drag = None;
        }
        if add {
            let at = self.room.canvas.center.unwrap_or(at(Pos2::ZERO));
            self.add_free_table(at);
        }
        edit.geometry |= free != before || add;
        edit
    }

    pub(super) fn layout_canvas(&mut self, ui: &mut egui::Ui, p: Palette) -> RoomEdit {
        let (rect, background) =
            ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        background.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, tr("Room canvas"))
        });
        let state = &mut self.room.canvas;
        if state.project != self.file.name || state.size != Some(rect.size()) {
            if state.project != self.file.name {
                state.selected = None;
                state.selected_label = None;
            }
            state.project = self.file.name.clone();
            state.size = Some(rect.size());
            state.fit_pending = true;
            state.drag = None;
            state.outline_drag = None;
            state.turning = None;
            state.label_drag = None;
        }
        // A second finger yields the drag to camera navigation, without changing the room.
        if ui.input(|i| i.multi_touch().is_some()) {
            state.drag = None;
            state.outline_drag = None;
            state.turning = None;
            state.label_drag = None;
        }
        let view = state.camera.viewport(rect);
        // The room as it is drawn: with a dragged or turned table where the
        // pointer has it.
        let mut layout = self.project.layout.clone();
        let dragged = self
            .dragged_table(ui, &view)
            .or_else(|| self.turned_table(ui, &view));
        if let Some((id, position)) = &dragged {
            layout.table_positions.insert(id.clone(), *position);
        }
        let frames = layout.table_frames();
        let tables: Vec<_> = layout
            .rows
            .iter()
            .flat_map(|r| &r.tables)
            .map(|t| (t, TableTop::new(t, frames[&t.id], TABLE_END)))
            .collect();
        let prints = footprint::footprints(&layout, &tables);
        let crowded = footprint::crowded(&prints);
        let state = &mut self.room.canvas;
        let room = layout.room.map(outline::world_rect);
        let world = tables
            .iter()
            .map(|(_, top)| top.bounds().expand2(Vec2::new(CARD_MARGIN, 42.0)))
            .chain(room)
            .chain(layout.labels.iter().map(crate::app::room_labels::bounds))
            .fold(Rect::NOTHING, Rect::union);
        if state.fit_pending && world.is_finite() {
            state.camera.fit(rect.shrink(20.0), world, 1.0);
            state.fit_pending = false;
        }
        state.camera.animate(ui.ctx());
        let view = state.camera.viewport(rect);
        state.center = Some(at(view.world(rect.center())));
        if background.clicked() {
            state.selected = None;
            state.selected_label = None;
        }
        let mut add_at = None;
        add_menu(&background, &view, &mut state.context_at, &mut add_at);
        let painter = ui.painter_at(rect);
        paint_grid(&painter, &view, rect, room, p);
        for (id, print) in &prints {
            let color = if crowded.contains(id) { p.bad } else { p.muted };
            print.paint(&painter, &view, color);
        }
        let navigation =
            state.camera.is_touch_navigation() || ui.input(|i| i.multi_touch().is_some());
        let seats = layout.seats();
        for (table, top) in &tables {
            let response = ui.interact(
                view.rect(top.bounds()).expand(8.0).intersect(rect),
                ui.id().with(("move-table", &table.id)),
                Sense::click_and_drag(),
            );
            if response.clicked() {
                state.selected = Some(table.id.clone());
                state.selected_label = None;
            }
            add_menu(&response, &view, &mut state.context_at, &mut add_at);
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    true,
                    fill("Move {}", &[&table.label]),
                )
            });
            if !navigation
                && response.drag_started()
                && let Some(at) = response.interact_pointer_pos()
            {
                // press_origin keeps the table attached to the initial grab point.
                let start = ui.input(|i| i.pointer.press_origin()).unwrap_or(at);
                state.drag = Some((table.id.clone(), frames[&table.id], view.world(start)));
            }
            let active = state
                .drag
                .as_ref()
                .is_some_and(|(id, _, _)| id == &table.id)
                || state.selected.as_deref() == Some(&table.id);
            let dots = seats
                .iter()
                .filter(|s| s.table_id == table.id)
                .map(|s| Pos2::new(s.x, s.y) + 12.0 * Vec2::angled(s.out));
            paint_table(&painter, &view, p, (table, top), dots, active);
            response.on_hover_cursor(egui::CursorIcon::Grab);
        }
        if self.room.canvas.drag.is_none() {
            for (table, top) in &tables {
                self.turn_handle(ui, &painter, &view, p, (table, top));
            }
        }
        let label_changed = self.room_label_canvas(ui, &painter, &view, p);
        let mut moved = dragged.filter(|_| ui.input(|i| i.pointer.any_released()));
        let outline_changed = self.outline_handles(ui, &painter, &view, p);
        let state = &mut self.room.canvas;
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            state.drag = None;
            state.turning = None;
            state.selected = None;
            state.selected_label = None;
            state.label_drag = None;
            moved = None;
        }
        state.camera.handle_input(
            ui,
            rect,
            &background,
            state.drag.is_none()
                && state.outline_drag.is_none()
                && state.turning.is_none()
                && state.label_drag.is_none(),
        );
        if ui.input(|i| i.pointer.any_released()) {
            state.drag = None;
            state.turning = None;
        }
        if state.crowded != crowded {
            // The side panel, drawn before the canvas, shows this next frame.
            ui.ctx().request_repaint();
            state.crowded = crowded;
        }
        let mut edit = RoomEdit {
            labels: outline_changed || label_changed,
            geometry: false,
        };
        if let Some(at) = add_at {
            self.add_free_table(at);
            edit.geometry = true;
        }
        if let Some((id, position)) = moved {
            self.project.layout.table_positions.insert(id, position);
            self.chart.view.fit_pending = true;
            edit.labels = true;
        }
        edit.labels |= self.rotate_by_key(ui);
        edit
    }

    /// The table being dragged and where the pointer has it now.
    fn dragged_table(&self, ui: &egui::Ui, view: &Viewport) -> Option<(String, TablePosition)> {
        let (id, original, start) = self.room.canvas.drag.as_ref()?;
        let delta = view.world(ui.input(|i| i.pointer.interact_pos())?) - *start;
        let snap = self.room.canvas.snap;
        let position = TablePosition {
            x: original.x + delta.x,
            y: original.y + delta.y,
            ..*original
        };
        Some((
            id.clone(),
            snap_table(self.project.layout.table(id)?, position, snap),
        ))
    }

    fn add_free_table(&mut self, at: TablePosition) {
        let ids: Vec<_> = self
            .project
            .layout
            .rows
            .iter()
            .flat_map(|r| r.tables.iter().map(|t| t.id.clone()))
            .collect();
        if self.project.layout.rows.is_empty() {
            self.project.layout.rows.push(RowSpec {
                id: "row-1".into(),
                label: fill("Row {}", &[&1]),
                tables: Vec::new(),
            });
        }
        let row = &mut self.project.layout.rows[0];
        add_table(row, &ids);
        let id = row.tables.last().unwrap().id.clone();
        let position = snap_table(row.tables.last().unwrap(), at, self.room.canvas.snap);
        self.project
            .layout
            .table_positions
            .insert(id.clone(), position);
        self.room.canvas.selected = Some(id);
        self.room.canvas.selected_label = None;
    }
}

/// Which tables are too close for their name cards on the chart.
pub(super) fn crowded_note(
    ui: &mut egui::Ui,
    p: Palette,
    layout: &crate::model::Layout,
    crowded: &std::collections::BTreeSet<String>,
) {
    if crowded.is_empty() {
        return;
    }
    let names: Vec<_> = crowded
        .iter()
        .filter_map(|id| layout.table(id).map(|t| t.label.as_str()))
        .collect();
    ui.label(
        RichText::new(fill(
            "Name cards overlap on the chart: {}",
            &[&names.join(", ")],
        ))
        .size(theme::text::SMALL)
        .color(p.bad),
    );
}

/// Dots on the grid, inside the room's walls when it has them.
fn paint_grid(
    painter: &egui::Painter,
    view: &Viewport,
    rect: Rect,
    room: Option<Rect>,
    p: Palette,
) {
    let spacing = GRID * view.zoom;
    if spacing < 8.0 {
        return;
    }
    let area = room.map_or(rect, |room| view.rect(room).intersect(rect));
    let first = |min: f32, origin: f32| {
        let offset = (origin - min).rem_euclid(spacing);
        min + if offset > 0.0 { offset } else { 0.0 }
    };
    let mut x = first(area.min.x, view.origin.x);
    while x <= area.max.x {
        let mut y = first(area.min.y, view.origin.y);
        while y <= area.max.y {
            painter.circle_filled(Pos2::new(x, y), 1.0, p.grid);
            y += spacing;
        }
        x += spacing;
    }
}

/// A table's top with its name on it, and its seats as dots.
fn paint_table(
    painter: &egui::Painter,
    view: &Viewport,
    p: Palette,
    (table, top): (&TableSpec, &TableTop),
    seats: impl Iterator<Item = Pos2>,
    active: bool,
) {
    let stroke = Stroke::new(
        if active { 2.0 } else { 1.0 },
        if active { p.accent } else { p.muted },
    );
    top.paint(painter, view, p.table, stroke, 4);
    painter.text(
        view.point(top.top.centre),
        Align2::CENTER_CENTER,
        &table.label,
        FontId::proportional((14.0 * view.zoom).max(9.0)),
        p.text,
    );
    for seat in seats {
        painter.circle_filled(view.point(seat), (5.0 * view.zoom).max(1.5), p.accent);
    }
}

fn add_menu(
    response: &egui::Response,
    view: &Viewport,
    origin: &mut Option<TablePosition>,
    add: &mut Option<TablePosition>,
) {
    if response.secondary_clicked()
        && let Some(point) = response.interact_pointer_pos()
    {
        *origin = Some(at(view.world(point)));
    }
    response.context_menu(|ui| {
        if ui.button(tr("Add table here")).clicked() {
            *add = *origin;
            ui.close();
        }
    });
}

#[cfg(test)]
mod snapping_tests {
    use super::*;
    #[test]
    fn unequal_tables_snap_their_first_seat_rows_and_edges_including_after_turning() {
        for rotation in [0.0, 45.0, 90.0, 180.0, 270.0] {
            let mut anchors = Vec::new();
            for length in [4, 5, 12, 17] {
                let table = TableSpec {
                    id: "t".into(),
                    label: "X".into(),
                    left: length,
                    right: length,
                    shape: TableShape::Long,
                };
                let (w, h) = table.half_size();
                let frame = TablePosition {
                    x: 0.0,
                    y: 0.0,
                    rotation,
                };
                let offset = frame.place(-w, -h);
                let at = TablePosition {
                    x: 83.0 - offset.0,
                    y: 121.0 - offset.1,
                    rotation,
                };
                let snapped = snap_table(&table, at, true);
                let anchor = snapped.place(-w, -h);
                assert!((anchor.0 - 56.0).abs() < 0.01 && (anchor.1 - 112.0).abs() < 0.01);
                assert_eq!(snap_table(&table, at, false), at);
                anchors.push(anchor);
            }
            assert!(
                anchors
                    .windows(2)
                    .all(|a| (a[0].0 - a[1].0).abs() < 0.01 && (a[0].1 - a[1].1).abs() < 0.01)
            );
        }
    }
}
