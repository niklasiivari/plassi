//! Wishes can be added in the chart inspector without leaving the seating.

use super::harness::Ui;
use crate::app::{Page, chart::MobileChart};
use crate::model::{RuleKind, Target};
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

fn pick_person(ui: &mut Ui, name: &str) {
    ui.h.get_by_role_and_label(egui::accesskit::Role::CheckBox, name)
        .scroll_to_me();
    ui.settle();
    ui.h.get_by_role_and_label(egui::accesskit::Role::CheckBox, name)
        .click();
    ui.settle();
}

#[test]
fn sidebar_adds_a_wish_without_changing_seats_and_supports_undo_and_persistence() {
    let mut ui = sidebar();
    let seats = ui.option().assignment.clone();
    let before = ui.app().project.rules.len();
    ui.click("+ Add wish");
    assert!(
        ui.h.query_by_role_and_label(egui::accesskit::Role::CheckBox, "Guest 01")
            .is_none()
    );
    for name in ["Guest 02", "Guest 03"] {
        pick_person(&mut ui, name);
    }
    assert_eq!(ui.app().people.wish_draft.as_ref().unwrap().targets, [1, 2]);
    ui.click("Add wish");
    assert!(ui.app().page == Page::Chart);
    assert_eq!(ui.option().assignment, seats);
    assert_eq!(ui.app().project.rules.len(), before + 1);
    let rule = ui.app().project.rules.last().unwrap();
    assert_eq!(rule.owner(), 0);
    assert!(
        matches!(&rule.kind, RuleKind::Proximity { target: Target::People { ids, .. }, .. } if ids == &[1, 2])
    );
    let id = rule.id;
    ui.key(Modifiers::COMMAND, Key::Z);
    assert_eq!(ui.app().project.rules.len(), before);
    ui.key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    assert_eq!(ui.app().project.rules.len(), before + 1);
    let ui = ui.reopen();
    assert!(
        ui.app()
            .project
            .rules
            .iter()
            .any(|rule| rule.id == id && rule.owner() == 0)
    );
}

#[test]
fn sidebar_can_add_an_organization_wish_with_a_searchable_dropdown() {
    let mut ui = sidebar();
    ui.click("+ Add wish");
    ui.click("Organization");
    ui.open_combo("Choose…");
    ui.type_text("org c");
    ui.key(Modifiers::NONE, Key::Enter);
    ui.click("Add wish");
    assert!(ui.app().page == Page::Chart);
    assert!(matches!(&ui.app().project.rules.last().unwrap().kind,
        RuleKind::Proximity { requester: 0, target: Target::Organization { name }, .. } if name == "Org C"));
}

#[test]
fn sidebar_draft_changes_owner_safely_and_cancel_keeps_the_project_unchanged() {
    let mut ui = sidebar();
    let before = serde_json::to_value(&ui.app().project).unwrap();
    ui.click("+ Add wish");
    pick_person(&mut ui, "Guest 03");
    assert_eq!(ui.app().people.wish_draft.as_ref().unwrap().targets, [2]);
    ui.app_mut().selected_person = Some(1);
    ui.settle();
    assert!(!ui.has("New wish"));
    ui.click("+ Add wish");
    let draft = ui.app().people.wish_draft.as_ref().unwrap();
    assert_eq!(draft.person, 1);
    assert!(draft.targets.is_empty());
    ui.click("Cancel");
    assert!(ui.app().people.wish_draft.is_none());
    assert_eq!(serde_json::to_value(&ui.app().project).unwrap(), before);
}

#[test]
fn sidebar_wish_form_fits_docked_floating_and_phone_layouts() {
    let mut ui = sidebar();
    ui.click("+ Add wish");
    for width in [1440.0, 1024.0, 390.0, 320.0] {
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.app_mut().chart.floating_inspector = true;
        ui.app_mut().chart.mobile_view = MobileChart::Details;
        ui.settle();
        ui.h.run_steps(4);
        assert!(ui.has("New wish"), "form missing at {width}");
        for node in ui.h.query_all_by(|n| {
            matches!(
                n.role(),
                egui::accesskit::Role::TextInput | egui::accesskit::Role::CheckBox
            )
        }) {
            let rect = node.rect();
            assert!(
                rect.left() >= -1.0 && rect.right() <= width + 1.0,
                "width={width}, rect={rect:?}"
            );
        }
    }
    ui.click("Organization");
    ui.open_combo("Choose…");
    ui.choose("Org B");
    ui.click("Add wish");
    assert!(ui.app().page == Page::Chart);
}
