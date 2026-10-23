//! Search input, bounded chart overlays, and readable inspector suggestions.

use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::kittest::Queryable;

use super::harness::Ui;
use crate::app::chart::MobileChart;

fn chart(size: egui::Vec2, long: bool) -> Ui {
    let mut ui = Ui::with_example();
    let mut project = crate::model::Project::new();
    project.attendees = ui.app().project.attendees[..12].to_vec();
    for (i, person) in project.attendees.iter_mut().enumerate() {
        person.id = i;
        person.name = format!(
            "Person {:02}{}",
            i + 1,
            if long {
                " — Long placeholder placeholder placeholder"
            } else {
                ""
            }
        );
        person.organization = if i % 2 == 0 { "XYZ" } else { "ABC" }.into();
        person.participant_group.clear();
    }
    project.layout = serde_json::from_value(serde_json::json!({
        "rows": [{ "id": "R1", "label": "Row 1", "tables": [
            { "id": "T1", "label": "Table 1", "left": 3, "right": 3 },
            { "id": "T2", "label": "Table 2", "left": 3, "right": 3 }
        ] }]
    }))
    .unwrap();
    project.rules = serde_json::from_value(serde_json::json!([{
        "id": 1, "label": "Seating wish", "source": "", "enabled": true,
        "kind": {"Proximity": {"requester": 0, "target": {"People": {"ids": [6]}},
            "wish": "Seating", "importance": "Normal", "max_class": null}}
    }]))
    .unwrap();
    let mut candidate = project.empty_option("Option 1".into());
    candidate.assignment = project
        .layout
        .seats()
        .into_iter()
        .enumerate()
        .map(|(i, s)| (i, s.id))
        .collect();
    project.candidates.push(candidate);
    ui.app_mut().project = project;
    ui.app_mut().selected_candidate = 0;
    ui.app_mut().selected_person = None;
    ui.app_mut().commit("Saved changes");
    ui.h.set_size(size);
    ui.settle();
    ui
}

fn queue_key(ui: &Ui, key: Key) {
    // Browser key events update modifiers too. In particular Ctrl+K must not
    // leave COMMAND set and accidentally disable the canvas shortcut control.
    ui.h.event(Event::ModifiersChanged(Modifiers::NONE));
    for pressed in [true, false] {
        ui.h.event(Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
    }
}

#[test]
fn submitting_chart_search_never_replays_typing_as_canvas_actions() {
    for size in [egui::vec2(1440.0, 900.0), egui::vec2(390.0, 844.0)] {
        for action in [Key::P, Key::Backspace, Key::ArrowRight] {
            let mut ui = chart(size, false);
            ui.app_mut().selected_person = Some(0);
            let assignment = ui.option().assignment.clone();
            let revision = ui.app().file.revision;
            ui.key(Modifiers::COMMAND, Key::K);
            match action {
                Key::P => {
                    queue_key(&ui, Key::P);
                    ui.h.event(Event::Text("Person 01".into()));
                }
                Key::Backspace => {
                    ui.type_text("Person 01X");
                    queue_key(&ui, Key::Backspace);
                }
                _ => {
                    ui.type_text("Person 01");
                    queue_key(&ui, action);
                }
            }
            queue_key(&ui, Key::Enter);
            ui.settle();
            assert_eq!(ui.app().selected_person, Some(0), "{size:?} {action:?}");
            assert_eq!(ui.option().assignment, assignment, "{size:?} {action:?}");
            assert!(
                ui.app().project.seat_pinned_people().is_empty(),
                "{size:?} {action:?}"
            );
            assert_eq!(
                ui.app().file.revision,
                revision,
                "search does not edit the project"
            );
            // The guard lasts one frame: a separate intentional shortcut works.
            ui.key(Modifiers::NONE, Key::P);
            assert!(ui.app().project.seat_pinned_people().contains(&0));
        }
    }
}

#[test]
fn short_chart_search_scrolls_to_keyboard_selection_and_allows_last_result() {
    let mut ui = chart(egui::vec2(390.0, 640.0), true);
    for (size, lang) in [
        (egui::vec2(390.0, 640.0), crate::i18n::Lang::Fi),
        (egui::vec2(568.0, 320.0), crate::i18n::Lang::En),
    ] {
        crate::i18n::set(lang);
        ui.h.set_size(size);
        ui.settle();
        ui.key(Modifiers::COMMAND, Key::K);
        ui.type_text("Person");
        let list = ui.app().chart.search_list.expect("search popup");
        assert!(
            list.left() >= 0.0
                && list.right() <= size.x
                && list.top() >= 0.0
                && list.bottom() <= size.y,
            "{size:?}: {list:?}"
        );
        for _ in 0..7 {
            ui.key(Modifiers::NONE, Key::ArrowDown);
        }
        let last =
            ui.h.query_all_by(|n| {
                n.label()
                    .is_some_and(|s| s.starts_with("Person 08") && s.contains(" · "))
            })
            .next()
            .expect("last result")
            .rect();
        let list = ui.app().chart.search_list.expect("search popup");
        assert!(
            list.contains_rect(last),
            "active result must be visible: {last:?} in {list:?}"
        );
        ui.key(Modifiers::NONE, Key::Enter);
        assert_eq!(ui.app().selected_person, Some(7));
        assert!(ui.app().chart.search.is_empty());
    }
}

fn card_name_color(ui: &Ui, person: usize) -> egui::Color32 {
    let name = ui.app().project.person_name(person);
    ui.h.output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(t) if t.galley.job.text == name => {
                Some(t.galley.job.sections[0].format.color)
            }
            _ => None,
        })
        .expect("painted person name")
}

#[test]
fn organization_search_keeps_matching_canvas_cards_undimmed() {
    let mut ui = chart(egui::vec2(1440.0, 900.0), false);
    let baseline = [card_name_color(&ui, 0), card_name_color(&ui, 1)];
    ui.key(Modifiers::COMMAND, Key::K);
    ui.type_text("xYz");
    assert!(ui.has_text("Person 01 · XYZ"));
    assert_eq!(card_name_color(&ui, 0), baseline[0]);
    assert_ne!(card_name_color(&ui, 1), baseline[1]);
}

#[test]
fn long_seat_labels_leave_wish_status_readable_on_phone() {
    for width in [320.0, 390.0] {
        for language in [crate::i18n::Lang::En, crate::i18n::Lang::Fi] {
            let mut ui = chart(egui::vec2(width, 844.0), true);
            for table in &mut ui.app_mut().project.layout.rows[0].tables {
                table.label.push_str(&" — Long placeholder".repeat(5));
            }
            ui.app_mut().selected_person = Some(0);
            ui.app_mut().chart.mobile_view = MobileChart::Details;
            ui.app_mut().commit("Saved changes");
            crate::i18n::set(language);
            ui.settle();
            let status = ui.node(crate::i18n::tr("Partly met")).rect();
            assert!(
                status.width() >= 50.0 && status.height() < 30.0,
                "{language:?}, width {width}: status should stay on one line: {status:?}"
            );
            assert!(status.left() >= 0.0 && status.right() <= width);
        }
    }
}

#[test]
fn long_better_seat_labels_leave_actions_readable_on_phone() {
    let mut ui = chart(egui::vec2(390.0, 844.0), true);
    ui.app_mut().selected_person = Some(0);
    ui.app_mut().chart.mobile_view = MobileChart::Details;
    ui.settle();
    let label =
        ui.h.query_all_by(|n| {
            n.value().is_some_and(|s| s.starts_with("Swap with Person"))
                || n.label().is_some_and(|s| s.starts_with("Swap with Person"))
        })
        .next()
        .expect("swap suggestion");
    label.scroll_to_me();
    ui.settle();
    let actions: Vec<_> = ui.h.query_all_by_label("Apply").map(|n| n.rect()).collect();
    let labels: Vec<_> =
        ui.h.query_all_by(|n| {
            n.value().is_some_and(|s| s.starts_with("Swap with Person"))
                || n.label().is_some_and(|s| s.starts_with("Swap with Person"))
        })
        .map(|n| n.rect())
        .collect();
    assert!(!actions.is_empty() && !labels.is_empty());
    for label in labels {
        assert!(
            label.width() < 300.0,
            "label reserves action space: {label:?}"
        );
        assert!(
            !actions.iter().any(|action| action.intersects(label)),
            "label {label:?} overlaps an action"
        );
    }
    let assignment = ui.option().assignment.clone();
    ui.click("Apply");
    assert_ne!(ui.option().assignment, assignment);
}
