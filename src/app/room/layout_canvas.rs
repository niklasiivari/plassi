//! Room designer. Table positions belong to the project; camera and in-progress
//! gestures belong to the editor, so a drag is saved once on release.
use super::*;
use crate::app::camera::Camera;
use crate::model::{SEAT_PITCH, TABLE_SPAN, TablePosition};
use egui::{Align2, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

pub(super) struct LayoutCanvas {
    camera: Camera,
    pub(super) fit_pending: bool,
    snap: bool,
    drag: Option<(String, TablePosition, Pos2)>,
    project: String,
    size: Option<Vec2>,
    selected: Option<String>,
    context_at: Option<TablePosition>,
    center: Option<TablePosition>,
}

impl Default for LayoutCanvas {
    fn default() -> Self {
        Self {
            camera: Camera::new(0.02, 3.0),
            fit_pending: true,
            snap: true,
            drag: None,
            project: String::new(),
            size: None,
            selected: None,
            context_at: None,
            center: None,
        }
    }
}

const GRID: f32 = 40.0;
const CARD_MARGIN: f32 = 190.0;

fn snapped(value: f32, snap: bool) -> f32 {
    if snap {
        (value / GRID).round() * GRID
    } else {
        value
    }
}

impl PlassiApp {
    pub(super) fn layout_controls(&mut self, ui: &mut egui::Ui, p: Palette) -> RoomEdit {
        let before = self.project.layout.free_layout;
        let mut free = before;
        let mut add = false;
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut free, false, tr("Rows"));
            ui.selectable_value(&mut free, true, tr("Free layout"));
            if free {
                add = ui.button(tr("+ Add table")).clicked();
                ui.checkbox(&mut self.room.canvas.snap, tr("Snap to grid"));
                if ui.button(tr("Fit room")).clicked() {
                    self.room.canvas.fit_pending = true;
                }
            }
        });
        if free != before {
            self.project.layout.set_free_layout(free);
            self.room.canvas.fit_pending = true;
            self.room.canvas.drag = None;
        }
        if free {
            theme::hint(
                ui,
                p,
                tr(
                    "Click a table to edit it; drag to move. Right-click to add a table. Drag the background to pan; scroll or pinch to zoom.",
                ),
            );
        }
        if add {
            let at = self
                .room
                .canvas
                .center
                .unwrap_or(TablePosition { x: 0.0, y: 0.0 });
            self.add_free_table(at);
        }
        RoomEdit {
            labels: false,
            geometry: free != before || add,
        }
    }

    pub(super) fn layout_canvas(&mut self, ui: &mut egui::Ui, p: Palette) -> RoomEdit {
        let (rect, background) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), 420.0),
            Sense::click_and_drag(),
        );
        background.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, tr("Room canvas"))
        });
        let state = &mut self.room.canvas;
        if state.project != self.file.name || state.size != Some(rect.size()) {
            if state.project != self.file.name {
                state.selected = None;
            }
            state.project = self.file.name.clone();
            state.size = Some(rect.size());
            state.fit_pending = true;
            state.drag = None;
        }
        let origins = self.project.layout.table_origins();
        let tables: Vec<_> = self
            .project
            .layout
            .rows
            .iter()
            .flat_map(|r| &r.tables)
            .map(|t| {
                let at = origins[&t.id];
                let height = (t.left.max(t.right).max(1) - 1) as f32 * SEAT_PITCH + 56.0;
                let body = Rect::from_min_size(
                    Pos2::new(at.x, at.y - 28.0),
                    Vec2::new(TABLE_SPAN, height),
                );
                (t, body)
            })
            .collect();
        let world = tables
            .iter()
            .map(|(_, r)| r.expand2(Vec2::new(CARD_MARGIN, 42.0)))
            .fold(Rect::NOTHING, Rect::union);
        if state.fit_pending && world.is_finite() {
            state.camera.fit(rect.shrink(20.0), world, 1.0);
            state.fit_pending = false;
        }
        state.camera.animate(ui.ctx());
        let view = state.camera.viewport(rect);
        let center = view.world(rect.center());
        state.center = Some(TablePosition {
            x: center.x,
            y: center.y,
        });
        if background.clicked() {
            state.selected = None;
        }
        let mut add_at = None;
        add_menu(&background, &view, &mut state.context_at, &mut add_at);
        let mut anchors = std::collections::HashMap::new();
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, p.surface_hi);
        let spacing = GRID * view.zoom;
        if spacing >= 8.0 {
            let mut x = rect.min.x + (view.origin.x - rect.min.x).rem_euclid(spacing);
            while x < rect.max.x {
                let mut y = rect.min.y + (view.origin.y - rect.min.y).rem_euclid(spacing);
                while y < rect.max.y {
                    painter.circle_filled(Pos2::new(x, y), 1.0, p.grid);
                    y += spacing;
                }
                x += spacing;
            }
        }
        // A second finger yields the drag to camera navigation, without changing the room.
        if ui.input(|i| i.multi_touch().is_some()) {
            state.drag = None;
        }
        let navigation =
            state.camera.is_touch_navigation() || ui.input(|i| i.multi_touch().is_some());
        let mut moved = None;
        for (table, body) in tables {
            let response = ui.interact(
                view.rect(body).expand(8.0).intersect(rect),
                ui.id().with(("move-table", &table.id)),
                Sense::click_and_drag(),
            );
            if response.clicked() {
                state.selected = Some(table.id.clone());
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
                state.drag = Some((table.id.clone(), origins[&table.id], view.world(start)));
            }
            let mut offset = Vec2::ZERO;
            if let Some((id, original, start)) = &state.drag
                && id == &table.id
                && let Some(at) = ui.input(|i| i.pointer.interact_pos())
            {
                let delta = view.world(at) - *start;
                let position = TablePosition {
                    x: snapped(original.x + delta.x, state.snap),
                    y: snapped(original.y + delta.y, state.snap),
                };
                offset = Vec2::new(position.x - original.x, position.y - original.y);
                if ui.input(|i| i.pointer.any_released()) {
                    moved = Some((id.clone(), position));
                }
            }
            let body = body.translate(offset);
            let active = state
                .drag
                .as_ref()
                .is_some_and(|(id, _, _)| id == &table.id)
                || state.selected.as_deref() == Some(&table.id);
            anchors.insert(table.id.clone(), view.rect(body));
            painter.rect(
                view.rect(body),
                4.0,
                p.table,
                Stroke::new(
                    if active { 2.0 } else { 1.0 },
                    if active { p.accent } else { p.muted },
                ),
                StrokeKind::Outside,
            );
            painter.text(
                view.point(Pos2::new(body.center().x, body.min.y - 18.0)),
                Align2::CENTER_CENTER,
                &table.label,
                FontId::proportional((14.0 * view.zoom).max(9.0)),
                p.text,
            );
            for (count, x) in [
                (table.left, body.min.x - 12.0),
                (table.right, body.max.x + 12.0),
            ] {
                for i in 0..count {
                    painter.circle_filled(
                        view.point(Pos2::new(x, body.min.y + 28.0 + i as f32 * SEAT_PITCH)),
                        (5.0 * view.zoom).max(1.5),
                        p.accent,
                    );
                }
            }
            response.on_hover_cursor(egui::CursorIcon::Grab);
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            state.drag = None;
            state.selected = None;
            moved = None;
        }
        state
            .camera
            .handle_input(ui, rect, &background, state.drag.is_none());
        if ui.input(|i| i.pointer.any_released()) {
            state.drag = None;
        }
        let mut edit = RoomEdit::default();
        if let Some(at) = add_at {
            self.add_free_table(at);
            edit.geometry = true;
        }
        if let Some((id, position)) = moved {
            self.project.layout.table_positions.insert(id, position);
            self.chart.view.fit_pending = true;
            edit.labels = true;
        }
        if let Some(id) = self.room.canvas.selected.clone()
            && let Some(body) = anchors.get(&id)
            && self.room.canvas.drag.is_none()
        {
            edit.merge(self.free_table_editor(ui, p, &id, *body));
        }
        edit
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
        self.project.layout.table_positions.insert(
            id.clone(),
            TablePosition {
                x: snapped(at.x, self.room.canvas.snap),
                y: snapped(at.y, self.room.canvas.snap),
            },
        );
        self.room.canvas.selected = Some(id);
    }

    fn free_table_editor(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        id: &str,
        body: Rect,
    ) -> RoomEdit {
        let mut edit = RoomEdit::default();
        let Some(table) = self
            .project
            .layout
            .rows
            .iter_mut()
            .flat_map(|r| &mut r.tables)
            .find(|t| t.id == id)
        else {
            return edit;
        };
        let mut remove = false;
        let mut close = false;
        let position = Pos2::new(body.max.x + 12.0, body.min.y);
        egui::Area::new(ui.id().with(("free-table-editor", id)))
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .constrain_to(ui.ctx().content_rect().shrink(8.0))
            .show(ui.ctx(), |ui| {
                theme::card(p).show(ui, |ui| {
                    ui.set_width(200.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tr("Table")).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            close = theme::icon_button(
                                ui,
                                p,
                                theme::Icon::Close,
                                tr("Deselect"),
                                false,
                            )
                            .clicked();
                        });
                    });
                    let label = ui.label(tr("Table name"));
                    edit.labels |= ui
                        .add(theme::field(&mut table.label).desired_width(200.0))
                        .labelled_by(label.id)
                        .changed();
                    ui.label(tr("Left"));
                    edit.geometry |= ui.push_id("left", |ui| stepper(ui, &mut table.left)).inner;
                    ui.label(tr("Right"));
                    edit.geometry |= ui
                        .push_id("right", |ui| stepper(ui, &mut table.right))
                        .inner;
                    ui.add_space(theme::space::S);
                    remove = ui.add(theme::ghost_button(tr("Remove table"), p)).clicked();
                });
            });
        if close {
            self.room.canvas.selected = None;
        }
        if remove {
            for row in &mut self.project.layout.rows {
                row.tables.retain(|t| t.id != id);
            }
            self.room.canvas.selected = None;
            edit.geometry = true;
        }
        edit
    }
}

fn add_menu(
    response: &egui::Response,
    view: &crate::app::camera::Viewport,
    origin: &mut Option<TablePosition>,
    add: &mut Option<TablePosition>,
) {
    if response.secondary_clicked()
        && let Some(at) = response.interact_pointer_pos()
    {
        let at = view.world(at);
        *origin = Some(TablePosition { x: at.x, y: at.y });
    }
    response.context_menu(|ui| {
        if ui.button(tr("Add table here")).clicked() {
            *add = *origin;
            ui.close();
        }
    });
}
