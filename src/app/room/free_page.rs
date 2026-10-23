//! The free layout as a workspace like the seating chart: a toolbar, the
//! room canvas filling the page, and a side panel with the selected table's
//! settings or an overview of the room.
use super::*;

/// What the pointer and keys do on the canvas: keys, then the action.
const CONTROLS: [(&str, &str); 5] = [
    ("Drag ○", "Turn a table"),
    ("R / Shift+R", "Turn the selected table"),
    ("Right-click", "Add a table there"),
    ("Drag background / Scroll", "Pan / zoom"),
    ("Esc", "Cancel / deselect"),
];

impl PlassiApp {
    pub(in crate::app) fn free_room_page(&mut self, ui: &mut egui::Ui, p: Palette) {
        if ui.ctx().memory(|m| m.focused().is_some()) {
            self.room.text_input_frame = Some(ui.ctx().cumulative_frame_nr());
        }
        let mut edit = RoomEdit::default();
        egui::Panel::top("room-toolbar")
            .frame(theme::panel_frame(p).inner_margin(egui::Margin::symmetric(
                theme::space::L as i8,
                theme::space::M as i8,
            )))
            .show(ui, |ui| edit.merge(self.layout_controls(ui)));
        if edit.geometry || edit.labels {
            self.room.problem.clear();
        }
        // Narrow screens keep the settings under the canvas.
        let panel = if ui.available_width() < theme::SPLIT_MIN {
            // Clamp a previously resized panel too: a desktop-sized inspector
            // must not consume the whole workspace after a phone resize.
            let maximum = (ui.available_height() * 0.4).clamp(0.0, 320.0);
            egui::Panel::bottom("room-inspector")
                .default_size(maximum)
                .size_range(maximum.min(64.0)..=maximum)
        } else {
            egui::Panel::right("room-inspector")
                .default_size(theme::PANEL_WIDTH + 2.0 * theme::space::M)
                .size_range(300.0..=480.0)
        };
        panel
            .resizable(true)
            .show_separator_line(true)
            .frame(theme::floating_panel_frame(p))
            .show(ui, |ui| {
                theme::panel_card(ui, p, theme::space::L, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| edit.merge(self.room_inspector(ui, p)));
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.canvas))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                // Toolbar/inspector edits have not been committed yet, so the
                // project revision still describes the previous room.
                if edit.geometry || edit.labels {
                    self.room.canvas.geometry = None;
                    self.room.problem.clear();
                }
                edit.merge(self.layout_canvas(ui, p));
                theme::panel_edge_shadows(ui, p, rect);
            });
        self.save_room(edit);
    }

    /// The selected table's settings, or the room overview.
    fn room_inspector(&mut self, ui: &mut egui::Ui, p: Palette) -> RoomEdit {
        if self.room.canvas.selected_tables.len() > 1 {
            return self.table_arrangement(ui, p);
        }
        if let Some(id) = self.room.canvas.selected_label {
            return self.room_label_settings(ui, p, id);
        }
        if let Some(id) = self.room.canvas.selected.clone()
            && self.project.layout.table(&id).is_some()
        {
            return self.table_settings(ui, p, &id);
        }
        self.room_header(ui, p);
        for label in &self.project.layout.labels {
            let text = if label.text.trim().is_empty() {
                tr("Label")
            } else {
                &label.text
            };
            if ui.add(egui::Button::new(text).wrap()).clicked() {
                self.room.canvas.selected_label = Some(label.id);
            }
        }
        ui.add_space(theme::space::L);
        theme::section_title(ui, p, tr("Controls"));
        theme::text_rows(ui, |ui| {
            for (keys, action) in CONTROLS {
                theme::key_hint(ui, p, tr(keys), tr(action));
            }
        });
        ui.add_space(theme::space::L);
        layout_canvas::crowded_note(ui, p, &self.project.layout, &self.room.canvas.crowded);
        self.room_problem(ui, p);
        RoomEdit::default()
    }
}
