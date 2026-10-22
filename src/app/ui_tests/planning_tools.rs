//! Cross-page history and room grouping.
use super::harness::Ui;
use eframe::egui::{self, Key, Modifiers, PointerButton};

#[test]
fn activity_links_back_to_the_reviewed_person_survives_undo_and_reopens() {
    let mut ui = Ui::with_example();
    ui.go_to("People & wishes");
    ui.click("Review notes");
    let first = ui.app().selected_person.unwrap();
    let name = ui.app().project.person_name(first).to_string();
    ui.click("Mark reviewed & next");
    let message = format!("Reviewed {name}");
    assert!(
        ui.app()
            .project
            .activity
            .iter()
            .any(|a| a.message == message)
    );
    let count = ui.app().project.activity.len();
    ui.key(Modifiers::COMMAND, Key::Z);
    ui.key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    assert_eq!(ui.app().project.activity.len(), count + 2);
    let mut ui = ui.reopen();
    ui.open_project_menu();
    ui.click("Activity history");
    ui.click(&message);
    assert_eq!(ui.app().selected_person, Some(first));
}

#[test]
fn selected_tables_align_move_together_and_undo_as_one_edit() {
    let mut ui = Ui::with_example();
    ui.go_to("Room layout");
    ui.click("Free layout");
    ui.click("Move Table 1");
    let at = ui.node("Move Table 2").rect().center();
    ui.click_at(at, PointerButton::Primary, Modifiers::SHIFT);
    assert!(ui.has("2 TABLES SELECTED"), "{:?}", ui.dump());
    let before = ui.app().project.layout.table_positions.clone();
    let start = ui.node("Move Table 1").rect().center();
    ui.drag(start, start + egui::vec2(38.0, 44.0));
    let after = ui.app().project.layout.table_positions.clone();
    let delta = |id: &str| egui::vec2(after[id].x - before[id].x, after[id].y - before[id].y);
    assert!(delta("table-1").length() > 1.0);
    assert!((delta("table-1") - delta("table-2")).length() < 0.01);
    assert_eq!(after["table-3"], before["table-3"]);
    ui.key(Modifiers::COMMAND, Key::Z);
    assert_eq!(ui.app().project.layout.table_positions, before);
    ui.click("Align bottom ends");
    let seats = ui.app().project.layout.seats();
    let bottom = |id: &str| {
        seats
            .iter()
            .filter(|s| s.table_id == id)
            .map(|s| s.y)
            .reduce(f32::max)
            .unwrap()
    };
    assert!((bottom("table-1") - bottom("table-2")).abs() < 0.01);
    ui.click("Clear table selection");
    assert!(!ui.has("2 TABLES SELECTED"));
}

#[test]
fn group_controls_and_activity_history_fit_narrow_screens() {
    use egui_kittest::kittest::Queryable;
    let mut ui = Ui::with_example();
    ui.go_to("Room layout");
    ui.click("Free layout");
    ui.click("Select tables");
    ui.click("Move Table 1");
    ui.click("Move Table 2");
    assert!(ui.has("2 TABLES SELECTED"));
    for width in [1440.0, 390.0, 320.0] {
        ui.h.set_size(egui::vec2(width, 844.0));
        ui.settle();
        ui.h.run_steps(4);
        for node in
            ui.h.query_all_by(|n| n.role() == egui::accesskit::Role::Button)
        {
            let rect = node.rect();
            if rect.bottom() > 0.0 && rect.top() < 844.0 {
                assert!(
                    rect.left() >= -1.0 && rect.right() <= width + 1.0,
                    "width={width}: {rect:?}"
                );
            }
        }
    }
    for i in 0..200 {
        ui.app_mut().project.record_activity(
            i,
            format!("X {i:03}: X X X X X X X X X X X X X X X X X X"),
            None,
        );
    }
    ui.app_mut().commit("Saved changes");
    ui.open_project_menu();
    ui.click("Activity history");
    for node in
        ui.h.query_all_by(|n| n.role() == egui::accesskit::Role::TextInput)
    {
        let rect = node.rect();
        assert!(rect.left() >= 0.0 && rect.right() <= 320.0);
    }
}
