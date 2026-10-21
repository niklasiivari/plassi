//! Tags are directly editable, reusable, and can be applied in bulk.

use super::harness::Ui;
use eframe::egui::{self, Key, Modifiers};
use egui_kittest::kittest::Queryable;

fn sidebar() -> Ui {
    let mut ui = Ui::with_example();
    ui.click("Seat by hand");
    ui.key(Modifiers::NONE, Key::Escape);
    ui.app_mut().selected_person = Some(0);
    ui.settle();
    ui
}

#[test]
fn sidebar_tags_support_button_enter_reuse_remove_undo_and_persistence() {
    let mut ui = sidebar();
    ui.click("Tag name");
    ui.type_text("  Speakers  ");
    ui.click("Add tag");
    assert_eq!(ui.app().project.person(0).unwrap().tags, ["Speakers"]);
    ui.click("Tag name");
    ui.type_text("speakers");
    ui.key(Modifiers::NONE, Key::Enter);
    assert_eq!(ui.app().project.person(0).unwrap().tags, ["Speakers"]);
    ui.app_mut().selected_person = Some(1);
    ui.settle();
    ui.open_combo("Use existing tag…");
    ui.type_text("speak");
    ui.key(Modifiers::NONE, Key::Enter);
    assert_eq!(ui.app().project.person(1).unwrap().tags, ["Speakers"]);
    ui.click("Speakers  ×");
    assert!(ui.app().project.person(1).unwrap().tags.is_empty());
    ui.key(Modifiers::COMMAND, Key::Z);
    assert_eq!(ui.app().project.person(1).unwrap().tags, ["Speakers"]);
    let ui = ui.reopen();
    assert_eq!(ui.app().project.person(1).unwrap().tags, ["Speakers"]);
}

#[test]
fn bulk_tags_apply_and_remove_for_all_selected_guests() {
    let mut ui = Ui::with_example();
    ui.go_to("People & wishes");
    ui.app_mut().selected_person = None;
    ui.app_mut().people.picked = [0, 1].into_iter().collect();
    ui.settle();
    ui.click("Tag name");
    ui.type_text("Volunteers");
    ui.click("Add tag");
    for id in [0, 1] {
        assert_eq!(ui.app().project.person(id).unwrap().tags, ["Volunteers"]);
    }
    assert!(ui.app().project.person(2).unwrap().tags.is_empty());
    ui.click("Volunteers (2/2)  ×");
    assert!(ui.app().project.person(0).unwrap().tags.is_empty());
    assert!(ui.app().project.person(1).unwrap().tags.is_empty());
    ui.key(Modifiers::COMMAND, Key::Z);
    assert_eq!(ui.app().project.person(0).unwrap().tags, ["Volunteers"]);
    assert_eq!(ui.app().project.person(1).unwrap().tags, ["Volunteers"]);
}

#[test]
fn people_tags_need_no_edit_toggle_and_fit_phone_widths() {
    let mut ui = sidebar();
    ui.go_to("People & wishes");
    assert!(!ui.app().people.editing_details);
    for width in [1440.0, 390.0, 320.0] {
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        ui.h.run_steps(4);
        ui.click("Tag name");
        let input = ui.h.get_by_label("Tag name").rect();
        let button = ui.h.get_by_label("Add tag").rect();
        assert!(input.left() >= 0.0 && button.right() <= width);
    }
    ui.type_text("Hosts");
    ui.key(Modifiers::NONE, Key::Enter);
    assert_eq!(ui.app().project.person(0).unwrap().tags, ["Hosts"]);
}
