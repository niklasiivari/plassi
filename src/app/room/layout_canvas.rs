//! Room designer. Table positions belong to the project; camera and in-progress
//! gestures belong to the editor, so a drag is saved once on release.
use super::*;
use crate::app::camera::{Camera, Viewport};
use crate::app::turned::TableTop;
use crate::model::TablePosition;
use egui::{FontId, Pos2, Rect, Sense, Stroke, Vec2};
use std::collections::BTreeMap;

use super::geometry::{RoomGeometry, TABLE_END};

pub(super) struct LayoutCanvas {
    camera: Camera,
    pub(super) fit_pending: bool,
    pub(super) snap: bool,
    pub(super) drag: Option<(String, TablePosition, Pos2)>,
    group_drag: std::collections::BTreeMap<String, TablePosition>,
    pub(super) selected_tables: std::collections::BTreeSet<String>,
    pub(super) selection_mode: bool,
    pub(super) marquee: Option<(Pos2, Pos2)>,
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
    /// Geometry and exact collision results survive pointer and camera frames.
    pub(super) geometry: Option<(u64, RoomGeometry)>,
}

impl Default for LayoutCanvas {
    fn default() -> Self {
        Self {
            camera: Camera::new(0.02, 3.0),
            fit_pending: true,
            snap: true,
            drag: None,
            group_drag: Default::default(),
            selected_tables: Default::default(),
            selection_mode: false,
            marquee: None,
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
            geometry: None,
        }
    }
}

impl LayoutCanvas {
    pub(super) fn camera_navigation(&self) -> bool {
        self.camera.is_touch_navigation()
    }
}

pub(super) const GRID: f32 = crate::model::SEAT_PITCH;

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
                ui.toggle_value(&mut self.room.canvas.selection_mode, tr("Select tables"))
                    .on_hover_text(tr("Tap tables or drag a box to select several. Shift-click also adds to the selection."));
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
                state.selected_tables.clear();
            }
            state.project = self.file.name.clone();
            state.size = Some(rect.size());
            state.fit_pending = true;
            state.drag = None;
            state.outline_drag = None;
            state.turning = None;
            state.label_drag = None;
            state.marquee = None;
            state.group_drag.clear();
        }
        // A second finger yields the drag to camera navigation, without changing the room.
        if ui.input(|i| i.multi_touch().is_some()) {
            state.drag = None;
            state.outline_drag = None;
            state.turning = None;
            state.label_drag = None;
            state.marquee = None;
            state.group_drag.clear();
        }
        let view = state.camera.viewport(rect);
        let mut geometry = state
            .geometry
            .take()
            .filter(|(revision, _)| *revision == self.file.revision)
            .map(|(_, geometry)| geometry)
            .unwrap_or_else(|| RoomGeometry::new(&self.project.layout));
        // The room as it is drawn: with a dragged or turned table where the
        // pointer has it.
        let dragged = self
            .dragged_table(ui, &view, &geometry)
            .or_else(|| self.turned_table(ui, &view, &geometry));
        let mut preview = BTreeMap::new();
        if let Some((id, position)) = &dragged
            && self.project.layout.free_layout
        {
            preview.insert(id.clone(), *position);
            if self.room.canvas.drag.is_some() {
                let original = self.room.canvas.group_drag.get(id).unwrap_or(position);
                for (other, at) in &self.room.canvas.group_drag {
                    preview.insert(
                        other.clone(),
                        TablePosition {
                            x: at.x + position.x - original.x,
                            y: at.y + position.y - original.y,
                            ..*at
                        },
                    );
                }
            }
        }
        geometry.preview(&preview);
        let tables: Vec<_> = geometry
            .tables
            .iter()
            .map(|table| (&table.spec, table.top))
            .collect();
        let state = &mut self.room.canvas;
        let room = self.project.layout.room.map(outline::world_rect);
        let world = room
            .into_iter()
            .chain(
                self.project
                    .layout
                    .labels
                    .iter()
                    .map(crate::app::room_labels::bounds),
            )
            .fold(geometry.bounds, Rect::union);
        if state.fit_pending && world.is_finite() {
            state.camera.fit(rect.shrink(20.0), world, 1.0);
            state.fit_pending = false;
        }
        state.camera.animate(ui.ctx());
        let view = state.camera.viewport(rect);
        state.center = Some(at(view.world(rect.center())));
        state
            .selected_tables
            .retain(|id| geometry.by_id.contains_key(id));
        let over_table = background.interact_pointer_pos().is_some_and(|point| {
            tables
                .iter()
                .any(|(_, top)| view.rect(top.bounds()).expand(8.0).contains(point))
        });
        if background.clicked() && !over_table {
            state.selected_tables.clear();
            state.selected = None;
            state.selected_label = None;
        }
        let mut add_at = None;
        add_menu(&background, &view, &mut state.context_at, &mut add_at);
        let painter = ui.painter_at(rect);
        paint_grid(&painter, &view, rect, room, p);
        for &index in geometry.by_id.values() {
            let table = &geometry.tables[index];
            let color = if geometry.crowded.contains(&table.spec.id) {
                p.bad
            } else {
                p.muted
            };
            table.footprint.paint(&painter, &view, color);
        }
        let navigation =
            state.camera.is_touch_navigation() || ui.input(|i| i.multi_touch().is_some());
        for (table, top) in &tables {
            let response = ui.interact(
                view.rect(top.bounds()).expand(8.0).intersect(rect),
                ui.id().with(("move-table", &table.id)),
                Sense::click_and_drag(),
            );
            if response.clicked() {
                if state.selection_mode || ui.input(|i| i.modifiers.shift || i.modifiers.command) {
                    if !state.selected_tables.insert(table.id.clone()) {
                        state.selected_tables.remove(&table.id);
                    }
                } else {
                    state.selected_tables.clear();
                    state.selected_tables.insert(table.id.clone());
                }
                state.selected = if state.selected_tables.contains(&table.id) {
                    Some(table.id.clone())
                } else {
                    state.selected_tables.first().cloned()
                };
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
                if !state.selected_tables.contains(&table.id) {
                    state.selected_tables.clear();
                    state.selected_tables.insert(table.id.clone());
                }
                state.selected = Some(table.id.clone());
                state.selected_label = None;
                state.group_drag = state
                    .selected_tables
                    .iter()
                    .map(|id| {
                        (
                            id.clone(),
                            geometry.table(id).expect("selected table").frame,
                        )
                    })
                    .collect();
                state.drag = Some((
                    table.id.clone(),
                    geometry.table(&table.id).expect("shown table").frame,
                    view.world(start),
                ));
                if let Some(focused) = ui.memory(|m| m.focused()) {
                    ui.memory_mut(|m| m.surrender_focus(focused));
                }
            }
            let active = state
                .drag
                .as_ref()
                .is_some_and(|(id, _, _)| id == &table.id)
                || state.selected_tables.contains(&table.id)
                || state.selected.as_deref() == Some(&table.id);
            let shown = geometry.table(&table.id).expect("shown table");
            let label = paint_table(
                &painter,
                &view,
                p,
                (table, top),
                shown.dots.iter().copied(),
                active,
                shown.footprint.bounds(),
            );
            ui.interact(
                label.intersect(rect),
                ui.id().with(("table-label", &table.id)),
                Sense::hover(),
            )
            .on_hover_text(&table.label);
            response
                .on_hover_text(&table.label)
                .on_hover_cursor(egui::CursorIcon::Grab);
        }
        self.room_selection_box(ui, &painter, &view, &background, &tables, p);
        self.paint_alignment_guides(&painter, &view, &geometry, dragged.as_ref(), p);
        if self.room.canvas.drag.is_none() && self.room.canvas.selected_tables.len() <= 1 {
            for (table, top) in &tables {
                self.turn_handle(ui, &painter, &view, p, (table, top));
            }
        }
        let label_changed = self.room_label_canvas(ui, &painter, &view, p);
        let mut moved = dragged.filter(|_| ui.input(|i| i.pointer.any_released()));
        let outline_changed = self.outline_handles(ui, &painter, &view, p);
        let state = &mut self.room.canvas;
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if !preview.is_empty() {
                ui.ctx().request_repaint();
            }
            state.drag = None;
            state.turning = None;
            state.selected = None;
            state.selected_label = None;
            state.label_drag = None;
            state.marquee = None;
            state.selected_tables.clear();
            state.group_drag.clear();
            moved = None;
        }
        state.camera.handle_input(
            ui,
            rect,
            &background,
            state.drag.is_none()
                && state.outline_drag.is_none()
                && state.turning.is_none()
                && state.label_drag.is_none()
                && state.marquee.is_none(),
        );
        if ui.input(|i| i.pointer.any_released()) {
            state.drag = None;
            state.turning = None;
        }
        if state.crowded != geometry.crowded {
            // The side panel, drawn before the canvas, shows this next frame.
            ui.ctx().request_repaint();
            state.crowded.clone_from(&geometry.crowded);
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
            if !self.room.canvas.group_drag.is_empty() {
                for other in self.room.canvas.group_drag.keys() {
                    self.project.layout.table_positions.insert(
                        other.clone(),
                        geometry.table(other).expect("dragged table").frame,
                    );
                }
            } else {
                self.project.layout.table_positions.insert(id, position);
            }
            self.chart.view.fit_pending = true;
            edit.labels = true;
        }
        if ui.input(|i| i.pointer.any_released()) {
            self.room.canvas.group_drag.clear();
        }
        if self.room.canvas.selected_tables.len() <= 1
            && self.room.text_input_frame != Some(ui.ctx().cumulative_frame_nr())
        {
            edit.labels |= self.rotate_by_key(ui);
        }
        self.room.canvas.geometry = Some((self.file.revision, geometry));
        edit
    }

    /// The table being dragged and where the pointer has it now.
    fn dragged_table(
        &self,
        ui: &egui::Ui,
        view: &Viewport,
        geometry: &RoomGeometry,
    ) -> Option<(String, TablePosition)> {
        let (id, original, start) = self.room.canvas.drag.as_ref()?;
        let delta = view.world(ui.input(|i| i.pointer.interact_pos())?) - *start;
        let snap = self.room.canvas.snap;
        let position = TablePosition {
            x: original.x + delta.x,
            y: original.y + delta.y,
            ..*original
        };
        let table = &geometry.table(id)?.spec;
        let mut result = snap_table(table, position, snap);
        if snap {
            let top = TableTop::new(table, position, TABLE_END).bounds();
            let others = geometry
                .by_id
                .iter()
                .filter(|(id, _)| !self.room.canvas.selected_tables.contains(id.as_str()))
                .map(|(_, &i)| geometry.tables[i].base_bounds);
            let (delta, guides) = super::arrange::magnetic(top, others, 8.0 / view.zoom);
            if guides.0.is_some() {
                result.x = position.x + delta.x;
            }
            if guides.1.is_some() {
                result.y = position.y + delta.y;
            }
        }
        Some((id.clone(), result))
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
        self.room.canvas.selected_tables.clear();
        self.room.canvas.selected_tables.insert(id.clone());
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

/// A table's top, its seats as dots and a bounded caption below its footprint.
fn paint_table(
    painter: &egui::Painter,
    view: &Viewport,
    p: Palette,
    (table, top): (&TableSpec, &TableTop),
    seats: impl Iterator<Item = Pos2>,
    active: bool,
    footprint: Rect,
) -> Rect {
    let stroke = Stroke::new(
        if active { 2.0 } else { 1.0 },
        if active { p.accent } else { p.muted },
    );
    top.paint(painter, view, p.table, stroke, 4);
    for seat in seats {
        painter.circle_filled(view.point(seat), (5.0 * view.zoom).max(1.5), p.accent);
    }
    // Names must not cross neighbouring tables or obscure seat markers. The
    // full value remains available on hover and in the selected table editor.
    let bounds = view.rect(footprint);
    let left = (bounds.left() + 4.0).max(painter.clip_rect().left());
    let right = (bounds.right() - 4.0).min(painter.clip_rect().right());
    let width = (right - left).max(0.0);
    let mut job = egui::text::LayoutJob::simple_singleline(
        table.label.clone(),
        FontId::proportional((14.0 * view.zoom).max(9.0)),
        p.text,
    );
    job.wrap.max_width = width;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let name = painter.layout_job(job);
    let at = Pos2::new(left + (width - name.size().x) / 2.0, bounds.bottom() + 4.0);
    let label = Rect::from_min_size(at, name.size());
    painter.galley(at, name, p.text);
    label
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
