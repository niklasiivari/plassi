//! Phone-sized flows through the same application used on desktop.

use super::harness::Ui;
use crate::app::Page;
use eframe::egui::{self, Modifiers, PointerButton};
use egui_kittest::kittest::{NodeT, Queryable};

fn phone(width: f32) -> Ui {
    let mut ui = Ui::with_example();
    ui.h.set_size(egui::vec2(width, 844.0));
    ui.settle();
    ui
}

fn navigate(ui: &mut Ui, label: &str) {
    if !ui.has(label) {
        ui.click("Menu");
    }
    ui.click(label);
}

fn fits(ui: &Ui, label: &str, width: f32) {
    let rect = ui.node(label).rect();
    assert!(
        rect.left() >= 0.0 && rect.right() <= width + 1.0,
        "{label}: {rect:?}"
    );
}

#[test]
fn phone_navigation_and_people_editor() {
    let mut ui = phone(390.0);
    assert!(!ui.has("Collapse sidebar"));
    fits(&ui, "Menu", 390.0);
    navigate(&mut ui, "People & wishes");
    let first = ui.app().project.attendees[0].clone();
    fits(&ui, &first.name, 390.0);
    ui.click(&first.name);
    assert!(ui.has("THEIR WISHES"), "{:?}", ui.dump());
    fits(&ui, "Back to people", 390.0);
    ui.click("Back to people");
    assert!(ui.app().selected_person.is_none());
    ui.click("Select people");
    ui.click(&first.name);
    assert!(ui.app().people.picked.contains(&first.id));
    assert!(ui.app().selected_person.is_none());
    ui.click("Clear");
    ui.click("+ Add person");
    assert!(ui.has("Add a person"));
    ui.click("Back to people");
    ui.click("Import people");
    assert!(ui.app().people.import_open);
    ui.click("Back to people");
    assert!(!ui.app().people.import_open);
    for (label, page) in [
        ("Connections", Page::Graph),
        ("Categories", Page::Categories),
        ("Seating policy", Page::Policy),
        ("Room layout", Page::Room),
        ("Print", Page::Print),
    ] {
        navigate(&mut ui, label);
        assert!(ui.app().page == page);
        fits(&ui, "Menu", 390.0);
    }
    ui.open_project_menu();
    assert!(ui.has("New project"));
}

#[test]
fn phone_seats_by_tapping_between_workspaces_and_keeps_canvas_width() {
    let mut ui = phone(390.0);
    ui.click("Seat by hand");
    let count = ui.app().project.attendees.len();
    ui.click(&format!("Not seated · {count}"));
    let first = ui.app().project.attendees[0].clone();
    ui.click(&first.name);
    assert!(ui.has("Chart"));
    let canvas = ui.canvas();
    assert!(canvas.width() >= 380.0, "{canvas:?}");
    let seat = ui.app().project.layout.seats()[0].id.clone();
    let at = ui.app().seat_card_on_screen(&seat).unwrap().center();
    assert!(canvas.contains(at));
    ui.click_at(at, PointerButton::Primary, Modifiers::NONE);
    assert_eq!(ui.seat_of(first.id), seat);
    ui.click("Details");
    assert!(ui.has("Edit wishes"));
    ui.click("Edit wishes");
    assert!(ui.app().page == Page::People);
    assert!(ui.has("Back to people"));
}

#[test]
fn small_phone_and_rotation_keep_controls_reachable() {
    let mut ui = phone(320.0);
    fits(&ui, "Solve", 320.0);
    fits(&ui, "Details", 320.0);
    navigate(&mut ui, "Print");
    for field in
        ui.h.query_all_by(|n| n.role() == egui::accesskit::Role::TextInput)
    {
        assert!(field.rect().right() <= 320.0, "{:?}", field.rect());
    }
    navigate(&mut ui, "People & wishes");
    ui.click("+ Add person");
    let fields: Vec<_> =
        ui.h.query_all_by(|n| n.role() == egui::accesskit::Role::TextInput)
            .collect();
    assert!(!fields.is_empty());
    for field in fields {
        assert!(field.rect().right() <= 320.0, "{:?}", field.rect());
    }
    ui.h.set_size(egui::vec2(844.0, 390.0));
    ui.settle();
    navigate(&mut ui, "Print");
    assert!(ui.app().page == Page::Print);
    ui.h.set_size(egui::vec2(1440.0, 900.0));
    ui.settle();
    assert!(ui.has("Seating chart"));
    assert!(!ui.has("Menu"));
}

#[test]
fn two_finger_pan_and_pinch_change_the_camera_without_moving_people() {
    let mut ui = phone(390.0);
    ui.click("Seat by hand");
    let canvas = ui.canvas();
    let center = canvas.center();
    let before = ui.option().assignment.clone();
    let zoom = ui.app().chart.view.camera.zoom;
    ui.h.event(egui::Event::PointerMoved(center));
    for (id, dx) in [(1, -30.0), (2, 30.0)] {
        ui.h.event(egui::Event::Touch {
            device_id: egui::TouchDeviceId(0),
            id: egui::TouchId(id),
            phase: egui::TouchPhase::Start,
            pos: center + egui::vec2(dx, 0.0),
            force: None,
        });
    }
    ui.h.step();
    let pan = ui.app().chart.view.camera.pan;
    for (id, dx) in [(1, -50.0), (2, 70.0)] {
        ui.h.event(egui::Event::Touch {
            device_id: egui::TouchDeviceId(0),
            id: egui::TouchId(id),
            phase: egui::TouchPhase::Move,
            pos: center + egui::vec2(dx, 20.0),
            force: None,
        });
    }
    ui.h.step();
    assert!(ui.app().chart.view.camera.zoom > zoom);
    assert_ne!(ui.app().chart.view.camera.pan, pan);
    assert_eq!(ui.option().assignment, before);
    assert!(ui.app().chart.drag.is_none());
}

#[test]
fn tablet_pages_keep_desktop_navigation_and_controls_inside_the_viewport() {
    let mut ui = Ui::with_example();
    for lang in [crate::i18n::Lang::En, crate::i18n::Lang::Fi] {
        crate::i18n::set(lang);
        for width in [600.0, 720.0, 768.0, 820.0, 900.0, 1024.0, 1180.0, 1280.0] {
            ui.h.set_size(egui::vec2(width, 800.0));
            for page in [
                Page::Chart,
                Page::People,
                Page::Graph,
                Page::Categories,
                Page::Policy,
                Page::Room,
                Page::Print,
            ] {
                ui.app_mut().page = page;
                ui.settle();
                assert!(
                    ui.has(crate::i18n::tr("Seating chart")),
                    "desktop navigation at {width}"
                );
                let canvas = (page == Page::Chart).then(|| ui.canvas());
                for node in ui.h.query_all_by(|n| {
                    matches!(
                        n.role(),
                        egui::accesskit::Role::Button
                            | egui::accesskit::Role::TextInput
                            | egui::accesskit::Role::ComboBox
                            | egui::accesskit::Role::CheckBox
                            | egui::accesskit::Role::Slider
                    )
                }) {
                    let rect = node.rect();
                    if rect.bottom() < 0.0
                        || rect.top() > 800.0
                        || canvas.is_some_and(|c| c.contains(rect.center()))
                    {
                        continue;
                    }
                    assert!(
                        rect.left() >= -1.0 && rect.right() <= width + 1.0,
                        "width={width}, page={}, label={:?}, rect={rect:?}",
                        page as u8,
                        node.accesskit_node().label()
                    );
                }
            }
        }
    }
}

#[test]
fn tablet_chart_keeps_room_for_unseated_people_and_inspector() {
    let mut ui = Ui::with_example();
    ui.click("Seat by hand");
    ui.key(Modifiers::NONE, egui::Key::Escape);
    for width in [768.0, 820.0, 1024.0, 1180.0] {
        ui.h.set_size(egui::vec2(width, 800.0));
        ui.settle();
        assert!(
            ui.canvas().width() >= 320.0,
            "width={width}, canvas={:?}",
            ui.canvas()
        );
        assert!(!ui.has("Back to people"));
        assert!(!ui.has("Chart"), "no phone tabs at {width}");
    }
    ui.h.set_size(egui::vec2(768.0, 800.0));
    ui.settle();
    ui.click("Show side panel");
    assert!(ui.app().chart.floating_inspector);
    assert!(ui.canvas().width() >= 320.0);
    ui.app_mut().chart.floating_inspector = false;
    ui.click("Expand sidebar");
    ui.open_project_menu();
    assert!(ui.has("New project"));
}

#[test]
fn tablet_person_details_fields_fit_beside_the_list() {
    let mut ui = Ui::with_example();
    let first = ui.app().project.attendees[0].id;
    ui.app_mut().page = Page::People;
    ui.app_mut().selected_person = Some(first);
    ui.app_mut().people.editing_details = true;
    for width in [768.0, 820.0, 1024.0] {
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        for node in ui.h.query_all_by(|n| {
            matches!(
                n.role(),
                egui::accesskit::Role::TextInput | egui::accesskit::Role::ComboBox
            )
        }) {
            assert!(
                node.rect().right() <= width + 1.0,
                "width={width}, rect={:?}",
                node.rect()
            );
        }
    }
}

#[test]
fn tablet_rail_controls_share_the_navigation_centerline() {
    let mut ui = Ui::with_example();
    for width in [768.0, 1024.0] {
        ui.h.set_size(egui::vec2(width, 800.0));
        ui.settle();
        let center = ui.node("Seating chart").rect().center().x;
        let theme = if ui.has("Dark mode") {
            "Dark mode"
        } else {
            "Light mode"
        };
        for label in ["Expand sidebar", theme, "EN", "About Plassi"] {
            assert!(
                (ui.node(label).rect().center().x - center).abs() < 1.0,
                "width={width}, control={label}, rect={:?}, expected center={center}",
                ui.node(label).rect()
            );
        }
    }
}

#[test]
fn intermediate_widths_use_consistent_chart_controls() {
    let mut ui = Ui::with_example();
    ui.click("Seat by hand");
    ui.key(Modifiers::NONE, egui::Key::Escape);
    let count = ui.app().project.attendees.len();
    let first = ui.app().project.attendees[0].name.clone();
    for width in [600.0, 680.0, 720.0] {
        ui.h.set_size(egui::vec2(width, 800.0));
        ui.settle();
        assert!(ui.has("Details"));
        assert!(!ui.has("Show side panel"));
        ui.click(&format!("Not seated · {count}"));
        ui.click(&first);
        assert!(ui.app().chart.mobile_view == crate::app::chart::MobileChart::Chart);
        ui.click("Details");
        assert!(ui.has("Edit wishes"));
        ui.click("Chart");
    }
}

#[test]
fn undo_is_left_of_redo_in_each_toolbar_layout() {
    let mut ui = Ui::with_example();
    for width in [390.0, 680.0, 1024.0, 1440.0] {
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        let history = |prefix: &str| {
            ui.h.query_all_by(|n| n.label().is_some_and(|text| text.starts_with(prefix)))
                .next()
                .expect("history button")
                .rect()
        };
        assert!(
            history("Undo").center().x < history("Redo").center().x,
            "width={width}"
        );
    }
}

#[test]
fn page_titles_start_at_the_same_left_edge_as_their_subtitle() {
    let mut ui = Ui::with_example();
    ui.app_mut().page = Page::Print;
    for width in [390.0, 768.0, 1024.0, 1440.0] {
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        let left = |text: &str| {
            ui.h.output()
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::Shape::Text(t) = &shape.shape {
                        (t.galley.job.text == text
                            && (text != "Print"
                                || t.galley.job.sections.iter().any(|section| {
                                    section.format.font_id.size == crate::app::theme::text::PAGE
                                })))
                        .then_some(t.pos.x)
                    } else {
                        None
                    }
                })
                .expect("text is painted")
        };
        assert!(
            (left("Print") - left("A4 PDFs saved in the project's folder.")).abs() < 1.0,
            "width={width}"
        );
    }
}

#[test]
fn tap_selection_survives_phone_tablet_and_desktop_resizing() {
    let mut ui = phone(390.0);
    navigate(&mut ui, "People & wishes");
    let people: Vec<_> = ui
        .app()
        .project
        .attendees
        .iter()
        .take(4)
        .map(|p| (p.id, p.name.clone()))
        .collect();
    ui.click("Select people");
    for ((id, name), width) in people.iter().zip([390.0, 680.0, 768.0, 1440.0]) {
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        ui.click(name);
        assert!(ui.app().people.picked.contains(id), "width={width}");
        assert!(
            ui.app().selected_person.is_none(),
            "tapping must keep the selection workflow open"
        );
    }
    assert_eq!(ui.app().people.picked.len(), 4);
    ui.click("Group together");
    let expected: Vec<_> = people.iter().map(|(id, _)| *id).collect();
    assert!(ui.app().project.preferences.iter().any(|preference| {
        matches!(&preference.kind, crate::preferences::PreferenceKind::Together { people } if people == &expected)
    }));
    ui.click("Clear");
    assert!(!ui.app().people.selecting);
    ui.click(&people[0].1);
    assert_eq!(ui.app().selected_person, Some(people[0].0));
}

#[test]
fn adding_a_second_finger_cancels_a_guest_drag_without_changing_seats() {
    let mut ui = phone(390.0);
    ui.click("Seat by hand");
    let count = ui.app().project.attendees.len();
    let first = ui.app().project.attendees[0].clone();
    ui.click(&format!("Not seated · {count}"));
    ui.click(&first.name);
    let seats = ui.app().project.layout.seats();
    let seat = &seats[0].id;
    ui.click_at(
        ui.app().seat_card_on_screen(seat).unwrap().center(),
        PointerButton::Primary,
        Modifiers::NONE,
    );
    let before = ui.option().assignment.clone();
    let from = ui.card_of(first.id).center();
    ui.h.hover_at(from);
    ui.h.step();
    ui.h.drag_at(from);
    ui.h.step();
    ui.h.hover_at(from + egui::vec2(12.0, 0.0));
    ui.h.step();
    assert!(
        ui.app().chart.drag.is_some(),
        "a guest is being dragged first"
    );
    for (id, offset) in [(1, 12.0), (2, 60.0)] {
        ui.h.event(egui::Event::Touch {
            device_id: egui::TouchDeviceId(0),
            id: egui::TouchId(id),
            phase: egui::TouchPhase::Start,
            pos: from + egui::vec2(offset, 0.0),
            force: None,
        });
    }
    ui.h.step();
    assert!(ui.app().chart.drag.is_none());
    for (id, offset) in [(1, 12.0), (2, 60.0)] {
        ui.h.event(egui::Event::Touch {
            device_id: egui::TouchDeviceId(0),
            id: egui::TouchId(id),
            phase: egui::TouchPhase::End,
            pos: from + egui::vec2(offset, 0.0),
            force: None,
        });
    }
    let to = ui.app().seat_card_on_screen(&seats[1].id).unwrap().center();
    ui.h.drop_at(to);
    ui.settle();
    assert_eq!(ui.option().assignment, before);
    assert!(ui.app().chart.drag.is_none());
}

#[test]
fn resizing_the_window_keeps_the_room_fitted_after_navigation_settles() {
    let mut ui = Ui::with_example();
    for width in [1024.0, 390.0, 768.0, 1440.0] {
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.h.run_steps(90);
        let canvas = ui.canvas();
        let room = ui
            .app()
            .project
            .layout
            .seats()
            .iter()
            .map(|seat| ui.app().seat_card_on_screen(&seat.id).unwrap())
            .fold(egui::Rect::NOTHING, egui::Rect::union);
        assert!(
            canvas.expand(1.0).contains_rect(room),
            "width={width}, room={room:?}, canvas={canvas:?}"
        );
    }
}

#[test]
fn tablet_expands_the_existing_sidebar_without_duplicate_navigation() {
    let mut ui = Ui::with_example();
    for width in [768.0, 1024.0] {
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        assert_eq!(ui.count("Seating chart"), 1);
        ui.click("Expand sidebar");
        assert!(ui.has("Collapse sidebar"));
        assert!(!ui.has("Menu"));
        assert_eq!(ui.count("Seating chart"), 1);
        let expanded = ui.node("Seating chart").rect().width();
        assert!(expanded > 200.0);
        ui.open_project_menu();
        assert!(ui.has("New project"));
        ui.key(Modifiers::NONE, egui::Key::Escape);
        ui.click("People & wishes");
        assert!(ui.app().page == Page::People);
        assert_eq!(ui.count("Seating chart"), 1);
        ui.click("Collapse sidebar");
        assert!(ui.has("Expand sidebar"));
        assert!(ui.node("Seating chart").rect().width() < expanded);
    }
}
