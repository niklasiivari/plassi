use crate::i18n::{fill, tr};
use std::collections::HashSet;

use eframe::egui::{self, RichText};

use crate::model::{DistanceClass, RowSpec, Seat, SeatRelation, TableSpec};

use super::PlassiApp;
use super::theme::{self, Palette};

/// State of the Room page.
pub(super) struct RoomState {
    pub(super) quick_tables: usize,
    pub(super) quick_seats: usize,
    /// Seat whose tags are being edited.
    pub(super) selected_seat: String,
    pub(super) new_tag: String,
    pub(super) relation_a: String,
    pub(super) relation_b: String,
    pub(super) relation_class: DistanceClass,
    /// TSV path for replacing the project.
    pub(super) import_input: String,
}

impl Default for RoomState {
    fn default() -> Self {
        Self {
            quick_tables: 3,
            quick_seats: 15,
            selected_seat: String::new(),
            new_tag: String::new(),
            relation_a: String::new(),
            relation_b: String::new(),
            relation_class: DistanceClass::Beside,
            import_input: "data/data.tsv".into(),
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
        edit.merge(self.row_cards(ui, p));
        ui.add_space(theme::space::L);
        egui::CollapsingHeader::new(RichText::new(tr("Advanced: tags and distances")).strong())
            .id_salt("advanced-room")
            .show(ui, |ui| edit.labels |= self.advanced_room(ui, p));
        ui.add_space(theme::space::S);
        self.replace_from_tsv(ui, p);
        self.save_room(edit);
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
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(tr("Room layout"))
                    .size(theme::text::PAGE)
                    .strong(),
            );
            ui.add_space(theme::space::S);
            theme::pill(ui, fill("{} seats", &[&seats]), p.text, p.surface_hi);
            theme::pill(ui, fill("{} people", &[&people]), p.text, p.surface_hi);
            let (text, color) = match seats.cmp(&people) {
                std::cmp::Ordering::Less => (fill("{} seats short", &[&(people - seats)]), p.bad),
                std::cmp::Ordering::Equal if people == 0 => (tr("Empty").to_string(), p.muted),
                std::cmp::Ordering::Equal => (tr("No spare seats").to_string(), p.good),
                std::cmp::Ordering::Greater => (fill("{} spare", &[&(seats - people)]), p.muted),
            };
            theme::status_pill(ui, p, text, color);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    crate::app::help::help_button(ui, p, crate::app::help::HelpPage::Room);
                });
            });
        });
        ui.label(RichText::new(tr("Long tables with seats on both sides.")).color(p.muted));
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
            self.project.layout =
                crate::model::Layout::simple(self.room.quick_tables, self.room.quick_seats);
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
        let row_count = self.project.layout.rows.len();
        let mut remove_row = None;
        for (index, row) in self.project.layout.rows.iter_mut().enumerate() {
            let (row_edit, remove) = row_card(ui, p, row, &table_ids, row_count > 1);
            edit.merge(row_edit);
            if remove {
                remove_row = Some(index);
            }
            ui.add_space(theme::space::S);
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

    fn replace_from_tsv(&mut self, ui: &mut egui::Ui, p: Palette) {
        if cfg!(target_arch = "wasm32") {
            return;
        }
        egui::CollapsingHeader::new(RichText::new(tr("Replace project from TSV")).strong())
            .id_salt("import-room")
            .show(ui, |ui| {
                ui.label(
                    RichText::new(tr("Replaces the entire project. To update people, use Import on People & wishes."))
                        .color(p.warn),
                );
                ui.horizontal_wrapped(|ui| {
                    ui.text_edit_singleline(&mut self.room.import_input);
                    if ui.button(tr("Import TSV")).clicked() {
                        self.import_tsv();
                    }
                });
            });
    }

    fn save_room(&mut self, edit: RoomEdit) {
        if edit.geometry {
            let fit = self.project.fit_to_room();
            self.chart.view.fit_pending = true;
            self.commit(&room_message(&fit, self.selected_candidate));
        } else if edit.labels {
            self.commit(tr("Saved room layout"));
        }
    }

    /// Returns whether anything changed.
    fn advanced_room(&mut self, ui: &mut egui::Ui, p: Palette) -> bool {
        let seats = self.project.layout.seats();
        if seats.is_empty() {
            ui.label(RichText::new(tr("Add tables first.")).color(p.muted));
            return false;
        }
        let tags = self.tag_editor(ui, p, &seats);
        ui.add_space(theme::space::S);
        tags | self.relation_editor(ui, p, &seats)
    }

    /// Tags on one seat, for placement rules.
    fn tag_editor(&mut self, ui: &mut egui::Ui, p: Palette, seats: &[Seat]) -> bool {
        if self.room.selected_seat.is_empty() {
            self.room.selected_seat = seats[0].id.clone();
        }
        theme::hint(ui, p, tr("Mark special seats, e.g. accessible or aisle."));
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            seat_picker(ui, "tag-seat", &mut self.room.selected_seat, seats);
            if let Some(tags) = self
                .project
                .layout
                .seat_tags
                .get_mut(&self.room.selected_seat)
            {
                tags.retain(|tag| {
                    let keep = !ui.add(theme::tag_button(tag)).clicked();
                    changed |= !keep;
                    keep
                });
            }
            ui.add(
                theme::field(&mut self.room.new_tag)
                    .hint_text(tr("new tag"))
                    .desired_width(120.0),
            );
            let tag = self.room.new_tag.trim().to_owned();
            if ui.button(tr("Add tag")).clicked() && !tag.is_empty() {
                let tags = self
                    .project
                    .layout
                    .seat_tags
                    .entry(self.room.selected_seat.clone())
                    .or_default();
                if !tags.contains(&tag) {
                    tags.push(tag);
                    changed = true;
                }
                self.room.new_tag.clear();
            }
        });
        self.project
            .layout
            .seat_tags
            .retain(|_, tags| !tags.is_empty());
        changed
    }

    /// Hand-set distances between two seats.
    fn relation_editor(&mut self, ui: &mut egui::Ui, p: Palette, seats: &[Seat]) -> bool {
        if self.room.relation_a.is_empty() {
            self.room.relation_a = seats[0].id.clone();
        }
        if self.room.relation_b.is_empty() {
            self.room.relation_b = seats[seats.len() - 1].id.clone();
        }
        theme::hint(ui, p, tr("Set a custom distance between two seats."));
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            seat_picker(ui, "relation-a", &mut self.room.relation_a, seats);
            seat_picker(ui, "relation-b", &mut self.room.relation_b, seats);
            theme::combo("relation-class", self.room.relation_class.label()).show_ui(ui, |ui| {
                for class in DistanceClass::ALL {
                    ui.selectable_value(&mut self.room.relation_class, class, class.label());
                }
            });
            let (a, b) = (&self.room.relation_a, &self.room.relation_b);
            if ui.button(tr("Save distance")).clicked() && a != b {
                self.project
                    .layout
                    .relations
                    .retain(|r| !same_pair(&r.a, &r.b, a, b));
                self.project.layout.relations.push(SeatRelation {
                    a: a.clone(),
                    b: b.clone(),
                    class: self.room.relation_class,
                });
                changed = true;
            }
        });
        self.project.layout.relations.retain(|relation| {
            let keep = ui
                .horizontal(|ui| {
                    ui.label(format!(
                        "{} ↔ {}: {}",
                        relation.a,
                        relation.b,
                        relation.class.label()
                    ));
                    !ui.add(theme::ghost_button(tr("Remove"), p)).clicked()
                })
                .inner;
            changed |= !keep;
            keep
        });
        changed
    }
}

/// A row's label, its table cards and "Add table". Returns the edit and
/// whether the row should be removed.
fn row_card(
    ui: &mut egui::Ui,
    p: Palette,
    row: &mut RowSpec,
    table_ids: &[String],
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
        let layout = if ui.available_width() < 540.0 {
            egui::Layout::top_down(egui::Align::Min)
        } else {
            egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true)
        };
        ui.with_layout(layout, |ui| {
            for (index, table) in row.tables.iter_mut().enumerate() {
                let (table_edit, remove) = table_card(ui, p, table);
                edit.merge(table_edit);
                if remove {
                    remove_table = Some(index);
                }
            }
        });
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
    let (left, right) = row.tables.last().map_or((8, 8), |t| (t.left, t.right));
    row.tables.push(TableSpec {
        label: fill("Table {}", &[&(table_ids.len() + 1)]),
        id,
        left,
        right,
    });
}

/// A table's label, preview, seat steppers and custom seats. Returns the
/// edit and whether the table should be removed.
fn table_card(ui: &mut egui::Ui, p: Palette, table: &mut TableSpec) -> (RoomEdit, bool) {
    let mut edit = RoomEdit::default();
    let mut remove = false;
    ui.push_id(table.id.clone(), |ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(214.0, 10.0),
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
                    ui.horizontal_wrapped(|ui| {
                        table_preview(ui, p, table.left, table.right);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(tr("Left"))
                                    .size(theme::text::CAPTION)
                                    .color(p.muted),
                            );
                            edit.geometry |= stepper(ui, &mut table.left);
                            ui.label(
                                RichText::new(tr("Right"))
                                    .size(theme::text::CAPTION)
                                    .color(p.muted),
                            );
                            edit.geometry |= stepper(ui, &mut table.right);
                        });
                    });
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

/// Tiny vertical drawing of a table with its seats.
fn table_preview(ui: &mut egui::Ui, p: Palette, left: usize, right: usize) {
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

fn seat_picker(ui: &mut egui::Ui, salt: &str, selected: &mut String, seats: &[Seat]) {
    theme::combo(salt, selected.as_str())
        .height(300.0)
        .show_ui(ui, |ui| {
            for seat in seats {
                ui.selectable_value(selected, seat.id.clone(), &seat.id);
            }
        });
}

fn same_pair(a: &str, b: &str, c: &str, d: &str) -> bool {
    (a == c && b == d) || (a == d && b == c)
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
