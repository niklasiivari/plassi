//! Room editing keeps usable canvas space and preserves valid seat capacities.
use super::harness::Ui;
use crate::i18n::{self, Lang};
use crate::model::{Layout, TableShape};
use eframe::egui::{self, Rect, Shape};
use egui_kittest::kittest::Queryable;
use std::collections::BTreeSet;

#[test]
fn short_phone_keeps_room_canvas_and_touch_handles_usable() {
    let mut ui = Ui::with_example();
    ui.go_to("Room layout");
    ui.click("Free layout");
    for language in [Lang::En, Lang::Fi] {
        i18n::set(language);
        for (width, height) in [(390.0, 844.0), (320.0, 568.0)] {
            ui.h.set_size(egui::vec2(width, height));
            ui.settle();
            let canvas = ui.node(i18n::tr("Room canvas")).rect();
            assert!(
                canvas.height() >= 140.0,
                "{language:?}, {width}x{height}: {canvas:?}"
            );
            let table = &ui.app().project.layout.rows[0].tables[0];
            let handle = ui.node(&i18n::fill("Turn {}", &[&table.label])).rect();
            assert!(
                handle.width() >= 44.0 && handle.height() >= 44.0,
                "{handle:?}"
            );
        }
    }
    ui.click(i18n::tr("Room walls"));
    let grip = ui.node(i18n::tr("Resize room walls")).rect();
    assert!(grip.width() >= 44.0 && grip.height() >= 44.0, "{grip:?}");
}

#[test]
fn opening_room_and_changing_shape_preserve_large_valid_capacities() {
    let mut ui = Ui::with_example();
    ui.app_mut().project.layout = Layout::simple(1, 300);
    ui.app_mut().project.layout.rows[0].tables[0].right = 200;
    assert!(ui.app().project.validate().is_ok());
    ui.app_mut().commit("Loaded project");
    let revision = ui.app().file.revision;
    let original: BTreeSet<_> = ui
        .app()
        .project
        .layout
        .seats()
        .into_iter()
        .map(|s| s.id)
        .collect();
    ui.go_to("Room layout");
    assert_eq!(ui.app().project.layout.seat_count(), 500);
    assert_eq!(
        ui.app().file.revision,
        revision,
        "viewing the room must not save an edit"
    );
    ui.click("Round");
    assert_eq!(
        ui.app().project.layout.rows[0].tables[0].shape,
        TableShape::Round
    );
    assert_eq!(ui.app().project.layout.seat_count(), 500);
    assert_eq!(
        ui.app()
            .project
            .layout
            .seats()
            .into_iter()
            .map(|s| s.id)
            .collect::<BTreeSet<_>>(),
        original,
    );
    ui.click("+");
    assert_eq!(ui.app().project.layout.seat_count(), 501);
    let expanded: BTreeSet<_> = ui
        .app()
        .project
        .layout
        .seats()
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert!(
        original.is_subset(&expanded),
        "adding one round-table seat must preserve all existing IDs"
    );
    assert!(ui.app().project.validate().is_ok());
    let saved: crate::model::Project =
        crate::io::parse(&ui.app().file.store.read().unwrap()).unwrap();
    assert_eq!(saved.layout.seat_count(), 501);
}

#[test]
fn typing_a_capacity_does_not_apply_incomplete_numbers_or_unseat_people() {
    let mut ui = Ui::with_example();
    let app = ui.app_mut();
    app.project.layout = Layout::simple(1, 30);
    let mut option = app.project.empty_option("Option 1".into());
    option.assignment = app
        .project
        .attendees
        .iter()
        .zip(app.project.layout.seats())
        .map(|(person, seat)| (person.id, seat.id))
        .collect();
    app.project.candidates.push(option);
    app.selected_candidate = 0;
    app.commit("Saved seating");
    let assignment = ui.option().assignment.clone();
    ui.go_to("Room layout");
    ui.h.query_all_by(|n| n.numeric_value() == Some(30.0))
        .next()
        .expect("left capacity")
        .click();
    ui.settle();
    ui.key(egui::Modifiers::COMMAND, egui::Key::A);
    for digit in ["3", "5"] {
        ui.type_text(digit);
        assert_eq!(ui.app().project.layout.rows[0].tables[0].left, 30);
        assert_eq!(ui.option().assignment, assignment);
    }
    ui.key(egui::Modifiers::NONE, egui::Key::Enter);
    assert_eq!(ui.app().project.layout.rows[0].tables[0].left, 35);
    assert_eq!(ui.option().assignment, assignment);
}

#[test]
fn submitting_a_table_name_does_not_replay_a_rotation_key() {
    let mut ui = Ui::with_example();
    ui.go_to("Room layout");
    ui.click("Free layout");
    ui.click("Move Table 1");
    let table = ui.app().project.layout.rows[0].tables[0].id.clone();
    let rotation = ui.app().project.layout.table_positions[&table].rotation;
    ui.h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Table name")
        .click();
    ui.settle();
    for event in [
        egui::Event::Key {
            key: egui::Key::R,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::Text("r".into()),
        egui::Event::Key {
            key: egui::Key::R,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
    ] {
        ui.h.event(event);
    }
    ui.settle();
    assert!(
        ui.app()
            .project
            .layout
            .table(&table)
            .unwrap()
            .label
            .ends_with('r')
    );
    assert_eq!(
        ui.app().project.layout.table_positions[&table].rotation,
        rotation
    );
    ui.key(egui::Modifiers::NONE, egui::Key::R);
    assert_eq!(
        ui.app().project.layout.table_positions[&table].rotation,
        (rotation + 45.0) % 360.0
    );
    ui.key(egui::Modifiers::SHIFT, egui::Key::R);
    assert_eq!(
        ui.app().project.layout.table_positions[&table].rotation,
        rotation
    );
}

fn label_shapes(shape: &Shape, labels: &[String], found: &mut Vec<(Rect, bool)>) {
    match shape {
        Shape::Text(text) if labels.contains(&text.galley.job.text) => {
            found.push((
                Rect::from_min_size(text.pos, text.galley.size()),
                text.galley.elided,
            ));
        }
        Shape::Vec(shapes) => {
            for shape in shapes {
                label_shapes(shape, labels, found);
            }
        }
        _ => {}
    }
}

#[test]
fn room_table_captions_do_not_overlap_with_long_names() {
    let mut ui = Ui::with_example();
    ui.app_mut().project.layout = Layout::simple(3, 4);
    let labels: Vec<_> = (1..=3)
        .map(|n| format!("Table {n} — ABC — Group {n} — Guest {n}"))
        .collect();
    for (table, name) in ui.app_mut().project.layout.rows[0]
        .tables
        .iter_mut()
        .zip(&labels)
    {
        table.label.clone_from(name);
    }
    ui.app_mut().commit("Saved room layout");
    ui.go_to("Room layout");
    ui.click("Free layout");
    for width in [390.0, 768.0] {
        ui.h.set_size(egui::vec2(width, 844.0));
        ui.settle();
        let canvas = ui.node("Room canvas").rect();
        let mut captions = Vec::new();
        for shape in &ui.h.output().shapes {
            label_shapes(&shape.shape, &labels, &mut captions);
        }
        assert_eq!(captions.len(), 3);
        assert!(
            captions.iter().all(|(rect, _)| canvas.contains_rect(*rect)),
            "{captions:?}"
        );
        assert!(
            captions.iter().all(|(_, elided)| *elided),
            "long names must be bounded"
        );
        for (i, (first, _)) in captions.iter().enumerate() {
            for (second, _) in &captions[i + 1..] {
                assert!(!first.intersects(*second), "{first:?} overlaps {second:?}");
            }
        }
    }
}
