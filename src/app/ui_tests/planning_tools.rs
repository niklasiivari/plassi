//! Canonical category choices and room grouping.
use super::harness::Ui;
use eframe::egui::{self, Key, Modifiers, PointerButton};

#[test]
fn canonical_category_choice_does_not_follow_another_category_alias() {
    use crate::catalog::Kind;
    for action in ["Merge", "Apply ready matches"] {
        let mut ui = Ui::with_example();
        let mut project = crate::model::Project::new();
        project.attendees = crate::example::project()
            .attendees
            .into_iter()
            .take(1)
            .collect();
        project.attendees[0].organization = "XYZ / ABC 2".into();
        let first = project
            .catalog
            .create_named(Kind::Organization, "ABC")
            .unwrap();
        project.catalog.assign(Kind::Organization, "XYZ", first);
        project
            .catalog
            .create_named(Kind::Organization, "XYZ")
            .unwrap();
        ui.app_mut().replace_project(project);
        ui.app_mut().commit("Saved changes");
        ui.go_to("Categories");
        ui.open_combo("Choose a match…");
        ui.choose("XYZ");
        ui.click(action);
        let names = ui
            .app()
            .project
            .category_names(Kind::Organization, &ui.app().project.attendees[0]);
        assert!(
            names.contains(&"XYZ".to_string()),
            "chosen category missing: {names:?}"
        );
        assert!(
            !names.contains(&"ABC".to_string()),
            "unselected category used: {names:?}"
        );
        ui.key(Modifiers::COMMAND, Key::Z);
        assert!(
            ui.app()
                .project
                .catalog
                .find(Kind::Organization, "XYZ / ABC 2")
                .is_none()
        );
        ui.key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        let ui = ui.reopen();
        assert_eq!(
            ui.app()
                .project
                .category_names(Kind::Organization, &ui.app().project.attendees[0]),
            names
        );
    }
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
fn group_controls_fit_narrow_screens() {
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
}
