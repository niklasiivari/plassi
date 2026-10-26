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

#[test]
fn confirmed_organization_wishes_bulk_add_separately_deduplicate_and_undo_on_all_widths() {
    use crate::catalog::Kind;
    use crate::model::{ImportField, ImportNote};
    for width in [1440.0, 768.0, 390.0, 320.0] {
        let mut ui = Ui::with_example();
        let mut project = crate::example::project();
        project.attendees.truncate(4);
        project.rules.clear();
        project.catalog = Default::default();
        for (id, organization) in ["ABC", "ABC", "XYZ", "XYZ"].into_iter().enumerate() {
            project.attendees[id].organization = organization.into();
        }
        let abc = project
            .catalog
            .create_named(Kind::Organization, "ABC")
            .unwrap();
        project.catalog.assign(Kind::Organization, "AB-C", abc);
        project.notes = [
            (0, "AB-C and Guest 02", false),
            (0, "ABC", false),
            (2, "AB-C", false),
            (3, "XYZ", false),
            (1, "ABC", true),
        ]
        .into_iter()
        .map(|(person, text, reviewed)| ImportNote {
            person,
            field: ImportField::Wish,
            text: text.into(),
            reviewed,
        })
        .collect();
        ui.app_mut().replace_project(project);
        ui.app_mut().commit("Saved changes");
        ui.go_to("People & wishes");
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        assert!(
            ui.has("2 confirmed organization matches"),
            "width={width}: {:?}",
            ui.dump()
        );
        assert!(ui.has("1 full-name matches"));
        // The count represents distinct wishes, even across several notes.
        ui.node("Add 2 organization wishes").scroll_to_me();
        ui.settle();
        let button = ui.node("Add 2 organization wishes").rect();
        assert!(
            button.left() >= 0.0 && button.right() <= width,
            "width={width}: {button:?}"
        );
        let notes = serde_json::to_value(&ui.app().project.notes).unwrap();
        ui.click("Add 2 organization wishes");
        assert_eq!(ui.app().project.rules.len(), 2);
        for requester in [0, 2] {
            assert_eq!(
                ui.app()
                    .project
                    .rules
                    .iter()
                    .filter(|rule| matches!(&rule.kind,
                RuleKind::Proximity { requester: q, target: Target::Organization { name }, .. }
                    if *q == requester && name == "ABC"))
                    .count(),
                1
            );
        }
        assert_eq!(
            serde_json::to_value(&ui.app().project.notes).unwrap(),
            notes
        );
        assert!(!ui.has("Add 2 organization wishes"));
        assert!(ui.has("1 full-name matches"));
        ui.key(Modifiers::COMMAND, Key::Z);
        assert!(ui.app().project.rules.is_empty());
        assert!(ui.has("Add 2 organization wishes"));
        ui.key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        assert_eq!(ui.app().project.rules.len(), 2);
        // The full-name action still adds only a person wish.
        ui.click("Add 1");
        assert_eq!(ui.app().project.rules.len(), 3);
        assert!(
            ui.app()
                .project
                .rules
                .iter()
                .any(|rule| matches!(&rule.kind,
            RuleKind::Proximity { requester: 0, target: Target::People { ids }, .. }
                if ids == &[1]))
        );
        let ui = ui.reopen();
        assert_eq!(ui.app().project.rules.len(), 3);
        assert_eq!(
            serde_json::to_value(&ui.app().project.notes).unwrap(),
            notes
        );
    }
}

#[test]
fn confirming_a_category_refreshes_the_overview_and_individual_wish_badge() {
    use crate::catalog::Kind;
    use crate::model::{ImportField, ImportNote};
    let mut ui = Ui::with_example();
    let mut project = crate::example::project();
    project.rules.clear();
    project.catalog = Default::default();
    for person in &mut project.attendees {
        person.organization = "AB-C".into();
    }
    project.notes = vec![ImportNote {
        person: 0,
        field: ImportField::Wish,
        text: "AB-C".into(),
        reviewed: false,
    }];
    ui.app_mut().replace_project(project);
    ui.app_mut().commit("Saved changes");
    ui.go_to("People & wishes");
    assert!(!ui.has("1 confirmed organization matches"));
    ui.go_to("Categories");
    ui.click("Organization name");
    ui.type_text("ABC");
    ui.choose("Create organization");
    ui.open_combo("Choose organizations…");
    ui.choose("ABC");
    ui.key(Modifiers::NONE, Key::Escape);
    assert!(
        ui.app()
            .project
            .catalog
            .find(Kind::Organization, "AB-C")
            .is_some()
    );
    ui.go_to("People & wishes");
    assert!(ui.has("1 confirmed organization matches"));
    ui.app_mut().selected_person = Some(0);
    ui.settle();
    assert!(ui.has("Confirmed category"));
    assert!(ui.has("Sit near people from ABC"));
}
