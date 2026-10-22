//! Text landmarks: create, edit, move and remove, without changing seating.
use super::*;
use crate::app::{camera::Viewport, room_labels};
use crate::model::{Point, RoomLabel};
use egui::{Sense, Stroke, StrokeKind};

impl PlassiApp {
    pub(super) fn add_room_label(&mut self) -> bool {
        let labels = &mut self.project.layout.labels;
        let id = labels.iter().map(|l| l.id).max().unwrap_or(0) + 1;
        let centre = self.room.canvas.center;
        labels.push(RoomLabel {
            id,
            text: tr("Label").into(),
            x: centre.map_or(0.0, |p| p.x),
            y: centre.map_or(0.0, |p| p.y),
        });
        self.room.canvas.selected_label = Some(id);
        self.room.canvas.selected = None;
        true
    }

    pub(super) fn room_label_settings(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        id: u64,
    ) -> RoomEdit {
        let Some(label) = self.project.layout.labels.iter_mut().find(|l| l.id == id) else {
            self.room.canvas.selected_label = None;
            return RoomEdit::default();
        };
        theme::section_title(ui, p, tr("Room text"));
        let title = ui.label(tr("Label text"));
        let changed = ui
            .add(
                theme::text_area(&mut label.text)
                    .char_limit(200)
                    .desired_rows(3)
                    .desired_width(ui.available_width()),
            )
            .labelled_by(title.id)
            .changed();
        theme::hint(
            ui,
            p,
            tr("Drag the text to place it. Included on the chart and printed room map."),
        );
        if ui.button(tr("Deselect")).clicked() {
            self.room.canvas.selected_label = None;
        }
        let remove = ui.add(theme::ghost_button(tr("Remove text"), p)).clicked();
        if remove {
            self.project.layout.labels.retain(|l| l.id != id);
            self.room.canvas.selected_label = None;
        }
        RoomEdit {
            labels: changed || remove,
            geometry: false,
        }
    }

    pub(super) fn room_label_canvas(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        view: &Viewport,
        p: Palette,
    ) -> bool {
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let release = ui.input(|i| i.pointer.any_released());
        let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape) || i.multi_touch().is_some());
        let navigation = self.room.canvas.camera_navigation();
        let mut moved = None;
        for saved in &self.project.layout.labels {
            let mut label = saved.clone();
            if let Some((id, original, start)) = self.room.canvas.label_drag
                && id == label.id
                && !cancel
                && let Some(pointer) = pointer
            {
                let delta = view.world(pointer) - start;
                label.x = layout_canvas::snapped(original.x + delta.x, self.room.canvas.snap);
                label.y = layout_canvas::snapped(original.y + delta.y, self.room.canvas.snap);
                if release {
                    moved = Some((id, label.x, label.y));
                }
            }
            let rect = room_labels::paint(painter, view, &label, p.muted);
            let response = ui.interact(
                rect.intersect(view.clip),
                ui.id().with(("room-label", label.id)),
                Sense::click_and_drag(),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    true,
                    fill("Move text: {}", &[&label.text]),
                )
            });
            if (response.clicked() || response.drag_started())
                && let Some(focused) = ui.memory(|m| m.focused())
            {
                ui.memory_mut(|m| m.surrender_focus(focused));
            }
            if response.clicked() {
                self.room.canvas.selected_label = Some(label.id);
                self.room.canvas.selected = None;
            }
            if !navigation
                && !cancel
                && response.drag_started()
                && let Some(pointer) = pointer
            {
                let start = ui.input(|i| i.pointer.press_origin()).unwrap_or(pointer);
                self.room.canvas.label_drag = Some((
                    label.id,
                    Point {
                        x: saved.x,
                        y: saved.y,
                    },
                    view.world(start),
                ));
                self.room.canvas.selected_label = Some(label.id);
                self.room.canvas.selected = None;
            }
            if response.hovered() || self.room.canvas.selected_label == Some(label.id) {
                painter.rect_stroke(rect, 4.0, Stroke::new(1.0, p.accent), StrokeKind::Outside);
            }
            response.on_hover_cursor(egui::CursorIcon::Grab);
        }
        if cancel || release {
            self.room.canvas.label_drag = None;
        }
        if let Some((id, x, y)) = moved
            && let Some(label) = self.project.layout.labels.iter_mut().find(|l| l.id == id)
        {
            let changed = label.x != x || label.y != y;
            label.x = x;
            label.y = y;
            return changed;
        }
        false
    }
}
