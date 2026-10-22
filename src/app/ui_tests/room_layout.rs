//! Regression coverage for large rooms and moving whole tables.
use super::harness::Ui;
use crate::model::Layout;
use eframe::egui::{self, Key, Modifiers};
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn many_tables_stay_inside_the_room_editor_on_desktop_and_phone() {
    let mut ui = Ui::with_example();
    ui.app_mut().project.layout = Layout::simple(40, 4);
    ui.go_to("Room layout");
    for width in [1440.0, 1024.0, 850.0, 768.0, 390.0, 320.0] {
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        ui.h.run_steps(4);
        let outside: Vec<_> =
            ui.h.query_all_by(|n| {
                matches!(
                    n.role(),
                    egui::accesskit::Role::Button
                        | egui::accesskit::Role::TextInput
                        | egui::accesskit::Role::ComboBox
                )
            })
            .filter_map(|n| {
                let rect = n.rect();
                (rect.right() > width + 1.0 || rect.left() < -1.0)
                    .then(|| format!("{:?}: {rect:?}", n.accesskit_node().label()))
            })
            .collect();
        assert!(outside.is_empty(), "width={width}: {outside:?}");
    }
}

#[test]
fn table_row_selector_keeps_seating_and_free_canvas_drag_is_saved() {
    let mut ui = Ui::with_example();
    ui.solve();
    ui.key(Modifiers::NONE, Key::Escape);
    let assignment = ui.option().assignment.clone();
    ui.go_to("Room layout");
    ui.click("+ Add row");
    ui.open_combo("Row 1");
    ui.choose("Row 2");
    assert_eq!(ui.app().project.layout.rows[1].tables[0].id, "table-1");
    assert_eq!(ui.option().assignment, assignment);
    ui.click("Free layout");
    let original = ui.app().project.layout.table_positions["table-1"];
    let label = format!("Move {}", ui.app().project.table_label("table-1"));
    ui.node(&label).scroll_to_me();
    ui.settle();
    let start = ui.node(&label).rect().center();
    ui.drag(start, start + egui::vec2(73.0, 57.0));
    let at = ui.app().project.layout.table_positions["table-1"];
    assert_ne!(at, original);
    let table = ui.app().project.layout.table("table-1").unwrap();
    let (w, h) = table.half_size();
    let anchor = at.place(-w, -h);
    let grid = crate::model::SEAT_PITCH;
    for value in [anchor.0, anchor.1] {
        assert!(value.rem_euclid(grid).min(grid - value.rem_euclid(grid)) < 0.01);
    }
    assert_eq!(ui.option().assignment, assignment);
    let disk: crate::model::Project =
        crate::io::parse(&ui.app().file.store.read().unwrap()).unwrap();
    assert_eq!(disk.layout.table_positions["table-1"], at);
    assert!(disk.layout.free_layout);
    ui.key(Modifiers::COMMAND, Key::Z);
    assert_eq!(ui.app().project.layout.table_positions["table-1"], original);
    ui.key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    assert_eq!(ui.app().project.layout.table_positions["table-1"], at);
    ui.click("Snap to grid");
    let start = ui.node(&label).rect().center();
    ui.drag(start, start + egui::vec2(31.0, 23.0));
    let at = ui.app().project.layout.table_positions["table-1"];
    assert!(at.x % 56.0 != 0.0 || at.y % 56.0 != 0.0);
    let start = ui.node(&label).rect().center();
    ui.h.drag_at(start);
    ui.h.step();
    ui.h.hover_at(start + egui::vec2(60.0, 30.0));
    ui.h.step();
    ui.key(Modifiers::NONE, Key::Escape);
    ui.h.drop_at(start + egui::vec2(60.0, 30.0));
    ui.settle();
    assert_eq!(ui.app().project.layout.table_positions["table-1"], at);
    ui.click("Rows");
    assert!(!ui.app().project.layout.free_layout);
    ui.click("Free layout");
    assert_eq!(ui.app().project.layout.table_positions["table-1"], at);
}

#[test]
fn free_canvas_has_local_editing_and_adds_tables_at_the_context_click() {
    use crate::model::TABLE_SPAN;
    use egui::PointerButton;
    let mut ui = Ui::with_example();
    ui.go_to("Room layout");
    ui.click("Free layout");
    assert!(!ui.has("+ Add row"));
    assert!(!ui.has("Quick setup"));
    ui.click("Move Table 1");
    assert!(ui.has("Table name"));
    ui.click("+");
    assert_eq!(
        ui.app().project.layout.rows[0].tables[0].left,
        ui.app().project.layout.rows[0].tables[0].right + 1
    );
    ui.click("Deselect");
    ui.click("Snap to grid");
    let table = &ui.app().project.layout.rows[0].tables[0];
    let at = ui.app().project.layout.table_frames()[&table.id];
    let middle = egui::pos2(at.x, at.y);
    let table_rect = ui.node("Move Table 1").rect();
    let zoom = (table_rect.width() - 16.0) / TABLE_SPAN;
    let canvas = ui.node("Room canvas").rect();
    let cursor = canvas.min + egui::vec2(35.0, 55.0);
    let expected = middle + (cursor - table_rect.center()) / zoom;
    let before = ui
        .app()
        .project
        .layout
        .rows
        .iter()
        .map(|r| r.tables.len())
        .sum::<usize>();
    ui.click_at(cursor, PointerButton::Secondary, Modifiers::NONE);
    ui.click("Add table here");
    let project = &ui.app().project;
    assert_eq!(
        project
            .layout
            .rows
            .iter()
            .map(|r| r.tables.len())
            .sum::<usize>(),
        before + 1
    );
    let added = project.layout.rows[0].tables.last().unwrap();
    let position = project.layout.table_positions[&added.id];
    assert!(
        (position.x - expected.x).abs() < 0.1,
        "{position:?} vs {expected:?}"
    );
    assert!(
        (position.y - expected.y).abs() < 0.1,
        "{position:?} vs {expected:?}"
    );
    assert!(ui.has("Table name"));
    ui.click("Remove table");
    assert_eq!(
        ui.app()
            .project
            .layout
            .rows
            .iter()
            .map(|r| r.tables.len())
            .sum::<usize>(),
        before
    );
    ui.click("+ Add table");
    assert_eq!(
        ui.app()
            .project
            .layout
            .rows
            .iter()
            .map(|r| r.tables.len())
            .sum::<usize>(),
        before + 1
    );
    for width in [390.0, 320.0] {
        ui.h.set_size(egui::vec2(width, 844.0));
        ui.settle();
        ui.h.run_steps(4);
        let panel = ui.node("Remove table").rect();
        assert!(panel.right() <= width && panel.left() >= 0.0, "{panel:?}");
    }
}

#[test]
fn free_tables_turn_become_round_and_warn_when_their_cards_overlap() {
    use crate::model::{TablePosition, TableShape};
    let mut ui = Ui::with_example();
    // Tall enough that the table editor is never pushed up to stay on screen.
    ui.h.set_size(egui::vec2(1440.0, 1400.0));
    ui.go_to("Room layout");
    ui.click("Free layout");
    ui.click("Move Table 1");
    let editor = ui.node("Table name").rect();
    ui.click("Round");
    // The editor stays where it opened while the table changes shape and turns.
    assert_eq!(ui.node("Table name").rect(), editor);
    let table = |ui: &Ui| ui.app().project.layout.rows[0].tables[0].clone();
    assert_eq!(table(&ui).shape, TableShape::Round);
    ui.key(Modifiers::NONE, Key::R);
    assert_eq!(
        ui.app().project.layout.table_positions["table-1"].rotation,
        45.0
    );
    assert_eq!(ui.node("Table name").rect(), editor);
    ui.key(Modifiers::SHIFT, Key::R);
    ui.key(Modifiers::SHIFT, Key::R);
    assert_eq!(
        ui.app().project.layout.table_positions["table-1"].rotation,
        315.0
    );
    ui.click("Long");
    assert_eq!(table(&ui).shape, TableShape::Long);
    // Dragging the handle to the right of the centre points the table's top there.
    let handle = ui.node("Turn Table 1").rect().center();
    let centre = ui.node("Move Table 1").rect().center();
    let right = centre + egui::vec2(centre.distance(handle), 0.0);
    ui.drag(handle, right);
    assert_eq!(
        ui.app().project.layout.table_positions["table-1"].rotation,
        90.0
    );
    ui.key(Modifiers::COMMAND, Key::Z);
    assert_eq!(
        ui.app().project.layout.table_positions["table-1"].rotation,
        315.0
    );
    // Slanted, the long table reaches into its neighbour's cards.
    assert!(ui.has_text("overlap another table"));
    ui.key(Modifiers::NONE, Key::R);
    assert_eq!(
        ui.app().project.layout.table_positions["table-1"].rotation,
        0.0
    );
    ui.click("Deselect");
    assert!(!ui.has_text("Name cards overlap"));
    let first = ui.app().project.layout.table_positions["table-1"];
    let next = TablePosition {
        x: first.x + 120.0,
        ..first
    };
    let id = ui.app().project.layout.rows[0].tables[1].id.clone();
    ui.app_mut().project.layout.table_positions.insert(id, next);
    ui.settle();
    assert!(ui.has_text("Name cards overlap"));

    ui.click("Room walls");
    let room = ui
        .app()
        .project
        .layout
        .room
        .expect("walls around the tables");
    assert!(room.width > 0.0 && room.height > 0.0);
    let disk: crate::model::Project =
        crate::io::parse(&ui.app().file.store.read().unwrap()).unwrap();
    assert_eq!(disk.layout.room, Some(room));
    assert!(ui.has("Resize room walls"));
    ui.click("Room walls");
    assert!(ui.app().project.layout.room.is_none());
}

#[test]
fn room_text_can_be_edited_moved_cancelled_removed_and_restored() {
    let mut ui = Ui::with_example();
    ui.go_to("Room layout");
    ui.click("Free layout");
    ui.click("+ Add text");
    ui.click("Label text");
    ui.key(Modifiers::COMMAND, Key::A);
    ui.type_text("X & Y");
    ui.key(Modifiers::NONE, Key::Tab);
    assert_eq!(ui.app().project.layout.labels[0].text, "X & Y");
    let original = ui.app().project.layout.labels[0].clone();
    let start = ui.node("Move text: X & Y").rect().center();
    ui.drag(start, start + egui::vec2(50.0, -50.0));
    let moved = ui.app().project.layout.labels[0].clone();
    assert!(moved.x != original.x || moved.y != original.y);
    ui.key(Modifiers::COMMAND, Key::Z);
    assert_eq!(ui.app().project.layout.labels[0].x, original.x);
    ui.key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    assert_eq!(ui.app().project.layout.labels[0].x, moved.x);
    let start = ui.node("Move text: X & Y").rect().center();
    ui.h.drag_at(start);
    ui.h.step();
    ui.h.hover_at(start + egui::vec2(30.0, 30.0));
    ui.h.step();
    ui.key(Modifiers::NONE, Key::Escape);
    ui.h.drop_at(start + egui::vec2(30.0, 30.0));
    ui.settle();
    assert_eq!(ui.app().project.layout.labels[0].x, moved.x);
    ui.click("Move text: X & Y");
    for width in [390.0, 320.0] {
        ui.h.set_size(egui::vec2(width, 844.0));
        ui.settle();
        let field = ui.node("Label text").rect();
        assert!(field.left() >= 0.0 && field.right() <= width);
    }
    ui.click("Remove text");
    assert!(ui.app().project.layout.labels.is_empty());
    ui.key(Modifiers::COMMAND, Key::Z);
    assert_eq!(ui.app().project.layout.labels[0].text, "X & Y");
    let ui = ui.reopen();
    assert_eq!(ui.app().project.layout.labels[0].text, "X & Y");
}
