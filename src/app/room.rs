use crate::i18n::{fill, tr};
use std::collections::HashSet;

use eframe::egui::{self, RichText};

use crate::model::{RowSpec, TableShape, TableSpec};

use super::PlassiApp;
use super::theme::{self, Palette};

mod arrange;
mod footprint;
mod free_page;
mod labels;
mod layout_canvas;
mod outline;
mod table_editor;
mod turn_handle;

const TABLE_CARD_WIDTH: f32 = 192.0 + 2.0 * theme::space::M;

/// State of the Room page.
pub(super) struct RoomState {
    pub(super) quick_tables: usize,
    pub(super) quick_seats: usize,
    pub(super) quick_columns: usize,
    canvas: layout_canvas::LayoutCanvas,
}

impl RoomState {
    pub(in crate::app) fn focus_tables(&mut self, ids: &[String]) {
        self.canvas.selected_tables = ids.iter().cloned().collect();
        self.canvas.selected = ids.first().cloned();
        self.canvas.selected_label = None;
        self.canvas.fit_pending = true;
    }
}

impl Default for RoomState {
    fn default() -> Self {
        Self {
            quick_tables: 3,
            quick_seats: 15,
            quick_columns: 4,
            canvas: Default::default(),
        }
    }
}

/// What an edit on the Room page touched. Geometry changes fit options to the new room.
#[derive(Default)]
struct RoomEdit {
    labels: bool,
    geometry: bool,
}

impl RoomEdit {
    fn merge(&mut self, other: RoomEdit) {
        self.labels |= other.labels;
        self.geometry |= other.geometry;
    }
}

impl PlassiApp {
    pub(super) fn room_page(&mut self, ui: &mut egui::Ui, p: Palette) {
        self.room_header(ui, p);
        if self.file.external_invalid {
            ui.colored_label(
                p.warn,
                tr("Repair the JSON file before editing in the app."),
            );
            return;
        }
        ui.add_space(theme::space::XL);
        let mut edit = RoomEdit::default();
        edit.geometry |= self.quick_setup(ui, p);
        ui.add_space(theme::space::S);
        edit.merge(self.layout_controls(ui));
        edit.merge(self.row_cards(ui, p));
        ui.add_space(theme::space::L);
        self.room_problem(ui, p);
        self.save_room(edit);
    }

    /// What is wrong with the room, if anything.
    fn room_problem(&self, ui: &mut egui::Ui, p: Palette) {
        if let Err(error) = self.project.validate() {
            ui.colored_label(
                p.warn,
                fill("Invalid project: {}", &[&format!("{error:#}")]),
            );
        }
    }

    /// Title with seat and people counts.
    fn room_header(&self, ui: &mut egui::Ui, p: Palette) {
        let seats = self.project.layout.seat_count();
        let people = self.project.attendees.len();
        crate::app::help::page_header(
            ui,
            p,
            crate::app::help::HelpPage::Room,
            tr("Room layout"),
            None,
        );
        ui.horizontal_wrapped(|ui| {
            theme::pill(ui, fill("{} seats", &[&seats]), p.text, p.surface_hi);
            theme::pill(ui, fill("{} people", &[&people]), p.text, p.surface_hi);
            let (text, color) = match seats.cmp(&people) {
                std::cmp::Ordering::Less => (fill("{} seats short", &[&(people - seats)]), p.bad),
                std::cmp::Ordering::Equal if people == 0 => (tr("Empty").to_string(), p.muted),
                std::cmp::Ordering::Equal => (tr("No spare seats").to_string(), p.good),
                std::cmp::Ordering::Greater => (fill("{} spare", &[&(seats - people)]), p.muted),
            };
            theme::status_pill(ui, p, text, color);
        });
    }

    /// "N tables with M seats per side" in one click. Returns true if the room was replaced.
    fn quick_setup(&mut self, ui: &mut egui::Ui, p: Palette) -> bool {
        let people = self.project.attendees.len();
        let mut replace = false;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(tr("Quick setup"))
                    .strong()
                    .size(theme::text::LEAD),
            );
            ui.add_space(theme::space::XS);
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::DragValue::new(&mut self.room.quick_tables).range(1..=40));
                ui.label(tr("tables with"));
                ui.add(egui::DragValue::new(&mut self.room.quick_seats).range(1..=100));
                ui.label(tr("seats per side"));
                ui.add(egui::DragValue::new(&mut self.room.quick_columns).range(1..=40));
                ui.label(tr("tables per row"));
                let total = self.room.quick_tables * self.room.quick_seats * 2;
                let color = if total >= people { p.good } else { p.bad };
                ui.label(RichText::new(fill("= {} seats", &[&total])).color(color));
                let empty = self.project.layout.seat_count() == 0;
                replace = ui
                    .add(theme::primary_button(
                        if empty {
                            tr("Create room")
                        } else {
                            tr("Replace room")
                        },
                        p,
                    ))
                    .on_hover_text(tr(
                        "Replaces all tables. People keep seats that still exist.",
                    ))
                    .clicked();
            });
            if people > 0 {
                let (tables, per_side) = crate::model::Layout::suggested(people);
                if (tables, per_side) != (self.room.quick_tables, self.room.quick_seats)
                    && ui
                        .link(fill(
                            "For {} people: {} tables with {} seats per side",
                            &[&people, &tables, &per_side],
                        ))
                        .clicked()
                {
                    (self.room.quick_tables, self.room.quick_seats) = (tables, per_side);
                }
            }
        });
        if replace {
            self.project.layout = crate::model::Layout::grid(
                self.room.quick_tables,
                self.room.quick_seats,
                self.room.quick_columns,
            );
            self.room.canvas.fit_pending = true;
        }
        replace
    }

    /// One card per row with its tables, plus "Add row".
    fn row_cards(&mut self, ui: &mut egui::Ui, p: Palette) -> RoomEdit {
        let mut edit = RoomEdit::default();
        let table_ids: Vec<String> = self
            .project
            .layout
            .rows
            .iter()
            .flat_map(|row| row.tables.iter().map(|table| table.id.clone()))
            .collect();
        let rows: Vec<_> = self
            .project
            .layout
            .rows
            .iter()
            .map(|r| (r.id.clone(), r.label.clone()))
            .collect();
        let row_count = rows.len();
        let mut move_table = None;
        let mut remove_row = None;
        for (index, row) in self.project.layout.rows.iter_mut().enumerate() {
            let (row_edit, remove) = ui
                .push_id(row.id.clone(), |ui| {
                    row_card(
                        ui,
                        p,
                        row,
                        &table_ids,
                        &rows,
                        &mut move_table,
                        row_count > 1,
                    )
                })
                .inner;
            edit.merge(row_edit);
            if remove {
                remove_row = Some(index);
            }
            ui.add_space(theme::space::S);
        }
        if let Some((table, row)) = move_table {
            edit.geometry |= self.project.layout.move_table(&table, &row);
        }
        if let Some(index) = remove_row {
            self.project.layout.rows.remove(index);
            edit.geometry = true;
        }
        if ui.button(tr("+ Add row")).clicked() {
            let rows = &mut self.project.layout.rows;
            let id = next_id("row", rows.iter().map(|row| row.id.as_str()));
            let label = fill("Row {}", &[&(rows.len() + 1)]);
            rows.push(RowSpec {
                id,
                label,
                tables: Vec::new(),
            });
            edit.geometry = true;
        }
        edit
    }

    fn save_room(&mut self, edit: RoomEdit) {
        if edit.geometry {
            let ids: HashSet<_> = self
                .project
                .layout
                .rows
                .iter()
                .flat_map(|r| r.tables.iter().map(|t| t.id.clone()))
                .collect();
            self.project
                .layout
                .table_positions
                .retain(|id, _| ids.contains(id));
            // Free placement keeps the camera still so adding at the pointer
            // or editing a table does not move the table away from the user.
            self.room.canvas.fit_pending |= !self.project.layout.free_layout;
            let fit = self.project.fit_to_room();
            self.chart.view.fit_pending = true;
            self.commit(&room_message(&fit, self.selected_candidate));
        } else if edit.labels {
            self.commit(tr("Saved room layout"));
        }
    }
}

/// A row's label, its table cards and "Add table". Returns the edit and
/// whether the row should be removed.
fn row_card(
    ui: &mut egui::Ui,
    p: Palette,
    row: &mut RowSpec,
    table_ids: &[String],
    rows: &[(String, String)],
    move_table: &mut Option<(String, String)>,
    removable: bool,
) -> (RoomEdit, bool) {
    let mut edit = RoomEdit::default();
    let mut remove_row = false;
    theme::card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        theme::responsive_row(ui, |ui| {
            let label = theme::field(&mut row.label)
                .font(egui::TextStyle::Heading)
                .desired_width(220.0);
            edit.labels |= ui.add(label).changed();
            ui.label(RichText::new(fill("{} tables", &[&row.tables.len()])).color(p.muted));
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    remove_row =
                        removable && ui.add(theme::ghost_button(tr("Remove row"), p)).clicked();
                    if ui.add(theme::secondary_button(tr("+ Add table"))).clicked() {
                        add_table(row, table_ids);
                        edit.geometry = true;
                    }
                });
            });
        });
        ui.add_space(theme::space::S);
        let mut remove_table = None;
        // Explicitly bounded lines: wrapping after an auto-sized child has grown
        // the parent lets a large generated row widen the entire page.
        let columns = ((ui.available_width() + ui.spacing().item_spacing.x)
            / (TABLE_CARD_WIDTH + ui.spacing().item_spacing.x))
            .floor()
            .max(1.0) as usize;
        for (line, tables) in row.tables.chunks_mut(columns).enumerate() {
            ui.horizontal_top(|ui| {
                for (column, table) in tables.iter_mut().enumerate() {
                    let (table_edit, remove) = table_card(ui, p, table, &row.id, rows, move_table);
                    edit.merge(table_edit);
                    if remove {
                        remove_table = Some(line * columns + column);
                    }
                }
            });
        }
        if let Some(index) = remove_table {
            row.tables.remove(index);
            edit.geometry = true;
        }
    });
    (edit, remove_row)
}

/// Appends a table copying the previous table's seat counts.
fn add_table(row: &mut RowSpec, table_ids: &[String]) {
    let used = table_ids.iter().chain(row.tables.iter().map(|t| &t.id));
    let id = next_id("table", used.map(String::as_str));
    let (left, right, shape) = row
        .tables
        .last()
        .map_or((8, 8, TableShape::Long), |t| (t.left, t.right, t.shape));
    row.tables.push(TableSpec {
        label: fill("Table {}", &[&(table_ids.len() + 1)]),
        id,
        left,
        right,
        shape,
    });
}

/// A table's label, preview, seat steppers and custom seats. Returns the
/// edit and whether the table should be removed.
fn table_card(
    ui: &mut egui::Ui,
    p: Palette,
    table: &mut TableSpec,
    row_id: &str,
    rows: &[(String, String)],
    move_table: &mut Option<(String, String)>,
) -> (RoomEdit, bool) {
    let mut edit = RoomEdit::default();
    let mut remove = false;
    ui.push_id(table.id.clone(), |ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(TABLE_CARD_WIDTH, 10.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                theme::inset(p).show(ui, |ui| {
                    theme::inset_controls(ui, p);
                    ui.set_width(192.0);
                    ui.horizontal_wrapped(|ui| {
                        edit.labels |= ui
                            .add(theme::field(&mut table.label).desired_width(110.0))
                            .changed();
                        remove = ui
                            .add(theme::glyph_button("×"))
                            .on_hover_text(tr("Remove table"))
                            .clicked();
                    });
                    let mut target = row_id.to_string();
                    let label = rows
                        .iter()
                        .find(|(id, _)| id == row_id)
                        .map(|(_, label)| label.as_str())
                        .unwrap_or(row_id);
                    theme::combo("table-row", label)
                        .width(160.0)
                        .show_ui(ui, |ui| {
                            for (id, label) in rows {
                                ui.value(&mut target, id.clone(), label);
                            }
                        });
                    if target != row_id {
                        *move_table = Some((table.id.clone(), target));
                    }
                    edit.geometry |= seat_controls(ui, p, table);
                });
            },
        );
    });
    (edit, remove)
}

/// "−  n  +" control; returns whether the value changed.
fn stepper(ui: &mut egui::Ui, value: &mut usize) -> bool {
    let before = *value;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = theme::space::XS;
        if ui.add(theme::glyph_button("−")).clicked() {
            *value = value.saturating_sub(1);
        }
        ui.add(egui::DragValue::new(value).range(0..=200));
        if ui.add(theme::glyph_button("+")).clicked() {
            *value = (*value + 1).min(200);
        }
    });
    *value != before
}

/// Long or round, and the seats: per side down a long table, in total
/// around a round one.
fn seat_controls(ui: &mut egui::Ui, p: Palette, table: &mut TableSpec) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        for (shape, label) in [
            (TableShape::Long, tr("Long")),
            (TableShape::Round, tr("Round")),
        ] {
            changed |= ui
                .selectable_value(&mut table.shape, shape, label)
                .changed();
        }
    });
    let caption = |ui: &mut egui::Ui, text: &str| {
        ui.label(
            RichText::new(text)
                .size(theme::text::CAPTION)
                .color(p.muted),
        );
    };
    ui.horizontal_wrapped(|ui| {
        table_preview(ui, p, table);
        ui.vertical(|ui| match table.shape {
            TableShape::Long => {
                caption(ui, tr("Left"));
                changed |= ui.push_id("left", |ui| stepper(ui, &mut table.left)).inner;
                caption(ui, tr("Right"));
                changed |= ui
                    .push_id("right", |ui| stepper(ui, &mut table.right))
                    .inner;
            }
            TableShape::Round => {
                caption(ui, tr("Seats"));
                let mut seats = table.left + table.right;
                if ui.push_id("seats", |ui| stepper(ui, &mut seats)).inner {
                    (table.left, table.right) = (seats.div_ceil(2), seats / 2);
                    changed = true;
                }
            }
        });
    });
    changed
}

/// Tiny drawing of a table with its seats.
fn table_preview(ui: &mut egui::Ui, p: Palette, table: &TableSpec) {
    if table.shape == TableShape::Round {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::hover());
        let seats = table.left + table.right;
        ui.painter().circle_filled(rect.center(), 12.0, p.table);
        for i in 0..seats {
            let angle = (i as f32 + 0.5) / seats as f32 * std::f32::consts::TAU;
            let at = rect.center() + 18.0 * egui::vec2(angle.sin(), -angle.cos());
            ui.painter().circle_filled(at, 2.2, p.accent);
        }
        return;
    }
    let (left, right) = (table.left, table.right);
    let rows = left.max(right).max(1);
    let pitch = (90.0 / rows as f32).min(9.0);
    let height = (rows as f32 * pitch + 8.0).max(40.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(44.0, height), egui::Sense::hover());
    let body = egui::Rect::from_min_max(
        egui::pos2(rect.center().x - 8.0, rect.min.y + 2.0),
        egui::pos2(rect.center().x + 8.0, rect.max.y - 2.0),
    );
    ui.painter().rect_filled(body, 3.0, p.table);
    let dot = (pitch * 0.35).clamp(1.5, 3.2);
    for (count, x) in [(left, body.min.x - 7.0), (right, body.max.x + 7.0)] {
        for i in 0..count {
            let y = body.min.y + 4.0 + (i as f32 + 0.5) * pitch;
            ui.painter().circle_filled(egui::pos2(x, y), dot, p.accent);
        }
    }
}

fn next_id<'a>(prefix: &str, existing: impl Iterator<Item = &'a str>) -> String {
    let used: HashSet<_> = existing.collect();
    (1..)
        .map(|index| format!("{prefix}-{index}"))
        .find(|id| !used.contains(id.as_str()))
        .unwrap()
}

/// The status after a room change, saying what it took away: counted in
/// the option shown, with a note when only other options lost people.
fn room_message(fit: &crate::model::RoomFit, shown: usize) -> String {
    let mut parts = vec![tr("Saved room layout").to_string()];
    let elsewhere = fit.unseated.iter().any(|n| *n > 0);
    match fit.unseated.get(shown).copied().unwrap_or(0) {
        0 if elsewhere => parts.push(tr("people in other options lost their seats").into()),
        0 => {}
        1 => parts.push(tr("1 person lost their seat and is not seated").into()),
        n => parts.push(fill("{} people lost their seats and are not seated", &[&n])),
    }
    match fit.rules_removed {
        0 => {}
        1 => parts.push(tr("1 rule about a removed place was deleted").into()),
        n => parts.push(fill("{} rules about removed places were deleted", &[&n])),
    }
    parts.join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_room_change_counts_the_people_of_the_option_shown() {
        crate::i18n::set(crate::i18n::Lang::En);
        let fit = |unseated: Vec<usize>| crate::model::RoomFit {
            unseated,
            rules_removed: 0,
        };
        let shown_lost = room_message(&fit(vec![5, 3]), 1);
        assert!(
            shown_lost.contains("3 people lost their seats"),
            "{shown_lost}"
        );
        let other_lost = room_message(&fit(vec![5, 0]), 1);
        assert!(other_lost.contains("other options"), "{other_lost}");
        assert_eq!(room_message(&fit(vec![0, 0]), 0), "Saved room layout");
    }
}
