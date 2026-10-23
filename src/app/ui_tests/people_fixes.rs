//! Saving focused details, long attendee values, and accurate bulk/roster feedback.

use super::harness::Ui;
use crate::app::{Page, people::RosterEdit};
use crate::i18n::{self, Lang, fill, tr};
use crate::model::{Candidate, Pin, SolverStatus};
use crate::roster::NewAttendee;
use eframe::egui::{self, Key, Modifiers};
use egui_kittest::kittest::{NodeT, Queryable};

fn edit_food(ui: &mut Ui) {
    ui.click(tr("Edit details"));
    ui.h.query_all_by(|n| {
        n.role() == egui::accesskit::Role::TextInput && n.value().as_deref() == Some("ABC")
    })
    .next()
    .expect("food input")
    .scroll_to_me();
    ui.settle();
    ui.h.query_all_by(|n| {
        n.role() == egui::accesskit::Role::TextInput && n.value().as_deref() == Some("ABC")
    })
    .next()
    .expect("food input")
    .click();
    ui.settle();
    ui.key(Modifiers::COMMAND, Key::A);
    ui.type_text("XYZ");
}

fn food_editor(width: f32, language: Lang) -> Ui {
    let mut ui = Ui::with_example();
    let id = ui.app().project.attendees[0].id;
    ui.app_mut().project.attendees[0].food = "ABC".into();
    ui.app_mut().commit("Saved changes");
    i18n::set(language);
    ui.h.set_size(egui::vec2(width, 844.0));
    ui.app_mut().page = Page::People;
    ui.app_mut().selected_person = Some(id);
    ui.settle();
    ui
}

#[test]
fn people_navigation_saves_pending_details_once_without_per_keystroke_writes() {
    for language in Lang::ALL {
        for width in [320.0, 390.0, 1440.0] {
            let actions: &[&str] = if width < 640.0 {
                &["Back to people"]
            } else {
                &[
                    "Room layout",
                    "Import people",
                    "+ Add person",
                    "Overview",
                    "Guest 02",
                ]
            };
            for action in actions {
                let mut ui = food_editor(width, language);
                let id = ui.app().selected_person.unwrap();
                let history = ui.app().file.undo_stack.len();
                edit_food(&mut ui);
                assert!(ui.app().people.details_dirty);
                if width == 1440.0 {
                    assert!(ui.has(tr("Unsaved changes")));
                }
                assert_eq!(ui.app().file.undo_stack.len(), history);
                let stored = crate::io::parse(&ui.app().file.store.read().unwrap()).unwrap();
                assert_eq!(stored.person(id).unwrap().food, "ABC");
                ui.click(tr(action));
                assert!(!ui.app().people.details_dirty, "{action}");
                assert_eq!(ui.app().file.undo_stack.len(), history + 1, "{action}");
                let stored = crate::io::parse(&ui.app().file.store.read().unwrap()).unwrap();
                assert_eq!(stored.person(id).unwrap().food, "XYZ", "{action}");
                ui.app_mut().undo(false);
                assert_eq!(ui.app().project.person(id).unwrap().food, "ABC", "{action}");
            }
        }
    }
}

#[test]
fn project_changes_flush_people_details_to_the_original_project_and_clear_pending_state() {
    let mut ui = food_editor(1440.0, Lang::En);
    let id = ui.app().selected_person.unwrap();
    let original = ui.app().file.store.clone();
    let original_name = ui.app().file.name.clone();
    edit_food(&mut ui);
    ui.app_mut()
        .create_project("<project>", crate::model::Project::new());
    let stored = crate::io::parse(&original.read().unwrap()).unwrap();
    assert_eq!(stored.person(id).unwrap().food, "XYZ");
    assert!(ui.app().project.attendees.is_empty());
    assert!(!ui.app().people.details_dirty);
    ui.app_mut().switch_project(&original_name);
    assert_eq!(ui.app().project.person(id).unwrap().food, "XYZ");
    drop(ui);

    let mut ui = food_editor(1440.0, Lang::En);
    let original = ui.app().file.store.clone();
    edit_food(&mut ui);
    ui.app_mut().replace_project(crate::model::Project::new());
    assert!(!ui.app().people.details_dirty);
    assert_eq!(
        crate::io::parse(&original.read().unwrap())
            .unwrap()
            .person(id)
            .unwrap()
            .food,
        "ABC"
    );
}

#[test]
fn another_save_includes_pending_people_details_without_an_extra_navigation_undo() {
    let mut ui = food_editor(1440.0, Lang::En);
    let history = ui.app().file.undo_stack.len();
    edit_food(&mut ui);
    ui.app_mut().commit("Saved changes");
    assert!(!ui.app().people.details_dirty);
    ui.go_to("Room layout");
    assert_eq!(ui.app().file.undo_stack.len(), history + 1);
}

#[test]
fn details_done_saves_the_focused_field_and_can_be_undone() {
    for language in Lang::ALL {
        for width in [320.0, 390.0, 1440.0] {
            let mut ui = Ui::with_example();
            let id = ui.app().project.attendees[0].id;
            ui.app_mut().project.attendees[0].food = "ABC".into();
            ui.app_mut().commit("Saved changes");
            i18n::set(language);
            ui.h.set_size(egui::vec2(width, 844.0));
            ui.app_mut().page = Page::People;
            ui.app_mut().selected_person = Some(id);
            ui.settle();
            ui.click(tr("Edit details"));
            let food = || {
                ui.h.query_all_by(|n| {
                    n.role() == egui::accesskit::Role::TextInput
                        && n.value().as_deref() == Some("ABC")
                })
                .next()
                .expect("food input")
            };
            food().scroll_to_me();
            ui.settle();
            ui.h.query_all_by(|n| {
                n.role() == egui::accesskit::Role::TextInput && n.value().as_deref() == Some("ABC")
            })
            .next()
            .expect("food input")
            .click();
            ui.settle();
            ui.key(Modifiers::COMMAND, Key::A);
            ui.type_text("XYZ");
            ui.click(tr("Done"));
            assert!(!ui.app().people.editing_details);
            let stored = crate::io::parse(&ui.app().file.store.read().unwrap()).unwrap();
            assert_eq!(stored.person(id).unwrap().food, "XYZ");
            ui.key(Modifiers::COMMAND, Key::Z);
            assert_eq!(ui.app().project.person(id).unwrap().food, "ABC");
            ui.key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
            let ui = ui.reopen();
            assert_eq!(ui.app().project.person(id).unwrap().food, "XYZ");
        }
    }
}

fn horizontal_overflow(ui: &Ui, width: f32) -> Vec<String> {
    ui.h.query_all_by(|n| {
        matches!(
            n.role(),
            egui::accesskit::Role::Button
                | egui::accesskit::Role::TextInput
                | egui::accesskit::Role::ComboBox
                | egui::accesskit::Role::CheckBox
                | egui::accesskit::Role::Label
        )
    })
    .filter_map(|node| {
        let rect = node.rect();
        (rect.left() < -1.0 || rect.right() > width + 1.0).then(|| {
            format!(
                "label={:?}, value={:?}, rect={rect:?}",
                node.accesskit_node().label(),
                node.accesskit_node().value()
            )
        })
    })
    .collect()
}

fn row_text_shapes(shape: &egui::Shape, values: &[String], found: &mut Vec<(egui::Rect, bool)>) {
    match shape {
        egui::Shape::Text(text) if values.contains(&text.galley.job.text) => {
            found.push((
                egui::Rect::from_min_size(text.pos, text.galley.size()),
                text.galley.elided,
            ));
        }
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                row_text_shapes(shape, values, found);
            }
        }
        _ => {}
    }
}

#[test]
fn long_people_list_names_and_subtitles_leave_the_review_badge_clear() {
    for language in Lang::ALL {
        for width in [320.0, 390.0, 1440.0] {
            let mut ui = Ui::with_example();
            let id = ui.app().project.attendees[0].id;
            let name = "Person 1 ".repeat(8);
            let organization = "ABCXYZ".repeat(20);
            ui.app_mut().project.attendees[0].name.clone_from(&name);
            ui.app_mut().project.attendees[0]
                .organization
                .clone_from(&organization);
            ui.app_mut().project.rules.clear();
            ui.app_mut().project.notes.push(crate::model::ImportNote {
                person: id,
                field: crate::model::ImportField::Wish,
                text: "XYZ".into(),
                reviewed: false,
            });
            ui.app_mut().commit("Saved changes");
            i18n::set(language);
            ui.h.set_size(egui::vec2(width, 844.0));
            ui.app_mut().page = Page::People;
            ui.settle();
            ui.node(&name).scroll_to_me();
            ui.settle();
            let row = ui.node(&name).rect();
            let mut painted = Vec::new();
            for shape in &ui.h.output().shapes {
                row_text_shapes(
                    &shape.shape,
                    &[name.clone(), organization.clone()],
                    &mut painted,
                );
            }
            assert_eq!(painted.len(), 2, "{language:?} {width}: {painted:?}");
            for (rect, elided) in painted {
                assert!(elided, "long row text should show an ellipsis");
                assert!(row.contains_rect(rect), "{rect:?} outside {row:?}");
                assert!(
                    rect.right() <= row.right() - 40.0,
                    "{rect:?}: review badge space"
                );
            }
        }
    }
}

#[test]
fn long_people_names_details_and_tags_keep_actions_within_the_viewport() {
    for language in Lang::ALL {
        for width in [320.0, 390.0, 1440.0] {
            let mut ui = Ui::with_example();
            let id = ui.app().project.attendees[0].id;
            let person = &mut ui.app_mut().project.attendees[0];
            person.name = "Person 1 ".repeat(8);
            person.organization = "ABC".into();
            person.allergies = "ABC XYZ ".repeat(10);
            person.extra.insert("XYZ".repeat(30), "ABC".repeat(30));
            person.tags.push("ABC".repeat(30));
            ui.app_mut().commit("Saved changes");
            i18n::set(language);
            ui.h.set_size(egui::vec2(width, 844.0));
            ui.app_mut().page = Page::People;
            ui.app_mut().selected_person = Some(id);
            ui.settle();
            ui.h.run_steps(4);
            let overflow = horizontal_overflow(&ui, width);
            assert!(overflow.is_empty(), "{language:?} {width}: {overflow:?}");
            for action in ["More", "Show on chart", "Edit details", "Add tag"] {
                let rect = ui.node(tr(action)).rect();
                assert!(
                    rect.width() > 40.0 && rect.height() < 60.0,
                    "{action}: {rect:?}"
                );
            }
            ui.click(tr("Edit details"));
            let overflow = horizontal_overflow(&ui, width);
            assert!(overflow.is_empty(), "{language:?} {width}: {overflow:?}");
        }
    }
}

fn empty_option() -> Candidate {
    Candidate {
        name: "Option 1".into(),
        assignment: Default::default(),
        solver_status: SolverStatus::StartedEmpty,
        hand_placed: Default::default(),
        loose: Default::default(),
    }
}

#[test]
fn bulk_pins_require_current_seats_and_report_only_affected_people() {
    let mut ui = Ui::with_example();
    let ids: Vec<_> = ui
        .app()
        .project
        .attendees
        .iter()
        .take(2)
        .map(|a| a.id)
        .collect();
    ui.app_mut().page = Page::People;
    ui.app_mut().people.picked = ids.iter().copied().collect();
    ui.settle();
    assert!(
        ui.node("Pin to current seats")
            .accesskit_node()
            .is_disabled()
    );
    assert!(ui.node("Unpin").accesskit_node().is_disabled());
    let mut option = empty_option();
    let seat = ui.app().project.layout.seats()[0].id.clone();
    option.assignment.insert(ids[0], seat.clone());
    ui.app_mut().project.candidates.push(option);
    ui.app_mut().commit("Saved changes");
    ui.settle();
    ui.click("Pin to current seats");
    assert_eq!(ui.app().project.pins().get(&ids[0]), Some(&Pin::Seat(seat)));
    assert!(!ui.app().project.pins().contains_key(&ids[1]));
    assert_eq!(
        ui.app().status,
        fill("Pinned {}", &[&ui.app().project.person_name(ids[0])])
    );
    ui.click("Unpin");
    assert!(!ui.app().project.pins().contains_key(&ids[0]));
    assert_eq!(
        ui.app().status,
        fill("Unpinned {}", &[&ui.app().project.person_name(ids[0])])
    );
    let before = ui.app().project.candidates[0].assignment.clone();
    let preferences = ui.app().project.preferences.len();
    ui.click("Group together");
    assert_eq!(ui.app().project.preferences.len(), preferences + 1);
    assert_eq!(ui.app().project.candidates[0].assignment, before);
    assert_eq!(ui.app().status, "Preference added; solve again to apply");
}

#[test]
fn roster_feedback_only_claims_a_seat_when_the_shown_option_has_one() {
    for seated in [false, true] {
        let mut ui = Ui::with_example();
        ui.app_mut().project.layout.rows[0].tables[0].left += 1;
        if seated {
            ui.app_mut().project.candidates.push(empty_option());
        }
        ui.app_mut().commit("Saved changes");
        ui.app_mut().page = Page::People;
        ui.app_mut().people.roster = Some(RosterEdit::Add(NewAttendee {
            name: "Person 100".into(),
            ..Default::default()
        }));
        ui.settle();
        ui.click("Add");
        let id = ui.app().selected_person.unwrap();
        let added = if seated {
            "Added {} to the best free seat"
        } else {
            "Added {} to the guest list"
        };
        assert_eq!(ui.app().status, fill(added, &[&"Person 100"]));
        ui.app_mut().people.roster = Some(RosterEdit::Replace(
            id,
            NewAttendee {
                name: "Person 101".into(),
                ..Default::default()
            },
        ));
        ui.settle();
        ui.click("Replace");
        let expected = if seated {
            fill("{} took over the seat", &[&"Person 101"])
        } else {
            fill("Replaced {} with {}", &[&"Person 100", &"Person 101"])
        };
        assert_eq!(ui.app().status, expected);
        let ui = ui.reopen();
        assert!(
            ui.app()
                .project
                .attendees
                .iter()
                .any(|a| a.name == "Person 101")
        );
        assert!(
            !ui.app()
                .project
                .attendees
                .iter()
                .any(|a| a.name == "Person 100")
        );
    }
}
