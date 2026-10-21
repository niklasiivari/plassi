//! The selected table's settings in the free layout's side panel: name,
//! shape, seats, turning and removal.
use super::*;
use crate::model::normal_degrees;

/// How far one turn button or key press turns a table, in degrees.
const TURN_STEP: f32 = 45.0;

impl PlassiApp {
    pub(super) fn table_settings(&mut self, ui: &mut egui::Ui, p: Palette, id: &str) -> RoomEdit {
        let mut edit = RoomEdit::default();
        let mut rotation = self.table_rotation(id);
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
        let mut close = false;
        ui.horizontal(|ui| {
            theme::section_title(ui, p, tr("Table"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                close =
                    theme::icon_button(ui, p, theme::Icon::Close, tr("Deselect"), false).clicked();
            });
        });
        let label = ui.label(tr("Table name"));
        edit.labels |= ui
            .add(theme::field(&mut table.label).desired_width(ui.available_width()))
            .labelled_by(label.id)
            .changed();
        ui.add_space(theme::space::S);
        edit.geometry |= seat_controls(ui, p, table);
        ui.add_space(theme::space::M);
        turn_controls(ui, &mut rotation);
        if self.room.canvas.crowded.contains(id) {
            ui.add_space(theme::space::M);
            ui.label(
                RichText::new(tr("Name cards overlap another table on the chart"))
                    .size(theme::text::SMALL)
                    .color(p.bad),
            );
        }
        ui.add_space(theme::space::L);
        let remove = ui.add(theme::ghost_button(tr("Remove table"), p)).clicked();
        if rotation != self.table_rotation(id) {
            self.set_table_rotation(id, rotation);
            edit.labels = true;
        }
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

    /// R turns the selected table clockwise, Shift+R back. Returns whether it turned.
    pub(super) fn rotate_by_key(&mut self, ui: &egui::Ui) -> bool {
        let Some(id) = self.room.canvas.selected.clone() else {
            return false;
        };
        if ui.ctx().memory(|m| m.focused().is_some()) {
            return false;
        }
        let Some(back) = ui.input(|i| {
            i.key_pressed(egui::Key::R)
                .then_some(i.modifiers.shift)
                .filter(|_| !i.modifiers.command)
        }) else {
            return false;
        };
        let step = if back { -TURN_STEP } else { TURN_STEP };
        let rotation = self.table_rotation(&id) + step;
        self.set_table_rotation(&id, rotation);
        true
    }

    fn table_rotation(&self, id: &str) -> f32 {
        self.project
            .layout
            .table_frames()
            .get(id)
            .map_or(0.0, |f| f.rotation)
    }

    fn set_table_rotation(&mut self, id: &str, degrees: f32) {
        let layout = &mut self.project.layout;
        let Some(&frame) = layout.table_frames().get(id) else {
            return;
        };
        let at = layout
            .table_positions
            .entry(id.to_string())
            .or_insert(frame);
        at.rotation = normal_degrees(degrees);
        self.chart.view.fit_pending = true;
    }
}

/// "Turn" with a step back, the angle, and a step on.
fn turn_controls(ui: &mut egui::Ui, rotation: &mut f32) {
    ui.label(tr("Turn"));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme::space::XS;
        if ui
            .add(theme::glyph_button("↺"))
            .on_hover_text(tr("Turn 45° anticlockwise (Shift+R)"))
            .clicked()
        {
            *rotation = normal_degrees(*rotation - TURN_STEP);
        }
        ui.add(
            egui::DragValue::new(rotation)
                .range(0.0..=359.0)
                .speed(1.0)
                .max_decimals(0)
                .suffix("°"),
        );
        if ui
            .add(theme::glyph_button("↻"))
            .on_hover_text(tr("Turn 45° clockwise (R)"))
            .clicked()
        {
            *rotation = normal_degrees(*rotation + TURN_STEP);
        }
    });
}
