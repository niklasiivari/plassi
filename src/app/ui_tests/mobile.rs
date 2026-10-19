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

fn overflowing_controls(ui: &Ui, width: f32) -> Vec<String> {
    ui.h.query_all_by(|n| {
        matches!(
            n.role(),
            egui::accesskit::Role::Button
                | egui::accesskit::Role::TextInput
                | egui::accesskit::Role::ComboBox
                | egui::accesskit::Role::CheckBox
                | egui::accesskit::Role::Slider
                | egui::accesskit::Role::RadioButton
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

#[test]
fn phone_project_menu_rename_and_about_fit_with_a_long_project_name() {
    let mut ui = phone(320.0);
    ui.app_mut().rename_project(&"<project> ".repeat(12));
    ui.settle();
    for width in [320.0, 390.0] {
        ui.h.set_size(egui::vec2(width, 844.0));
        ui.settle();
        ui.open_project_menu();
        assert!(
            overflowing_controls(&ui, width).is_empty(),
            "{:?}",
            overflowing_controls(&ui, width)
        );
        ui.click("Rename project");
        fits(&ui, "Save project name", width);
        fits(&ui, "Cancel rename", width);
        assert!(
            overflowing_controls(&ui, width).is_empty(),
            "{:?}",
            overflowing_controls(&ui, width)
        );
        ui.click("Cancel rename");
        ui.click("Menu");
        ui.click("About Plassi");
        fits(&ui, "Close about window", width);
        assert!(
            overflowing_controls(&ui, width).is_empty(),
            "{:?}",
            overflowing_controls(&ui, width)
        );
        ui.click("Close about window");
    }
}

#[test]
fn expanded_third_party_notices_fit_phone_and_tablet_screens() {
    let mut ui = phone(320.0);
    ui.click("Menu");
    ui.click("About Plassi");
    for lang in [crate::i18n::Lang::En, crate::i18n::Lang::Fi] {
        crate::i18n::set(lang);
        for size in [
            egui::vec2(320.0, 568.0),
            egui::vec2(390.0, 844.0),
            egui::vec2(844.0, 390.0),
            egui::vec2(768.0, 1024.0),
        ] {
            ui.h.set_size(size);
            ui.settle();
            for section in include_str!("../../../THIRD_PARTY_NOTICES.md")
                .lines()
                .filter_map(|line| line.strip_prefix("## "))
            {
                let label = crate::i18n::tr_any(section);
                ui.click(label);
                let overflow = overflowing_controls(&ui, size.x);
                assert!(
                    overflow.is_empty(),
                    "size={size:?}, section={section}, {overflow:?}"
                );
                let close = ui.node(crate::i18n::tr("Close about window")).rect();
                assert!(
                    egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(close),
                    "close={close:?}, size={size:?}"
                );
                ui.click(label);
            }
        }
    }
}

#[test]
fn chart_group_actions_wrap_inside_the_canvas() {
    let mut ui = phone(320.0);
    ui.click("Seat by hand");
    ui.key(Modifiers::NONE, egui::Key::Escape);
    ui.app_mut().chart.multi = ui
        .app()
        .project
        .attendees
        .iter()
        .take(2)
        .map(|a| a.id)
        .collect();
    for lang in [crate::i18n::Lang::En, crate::i18n::Lang::Fi] {
        crate::i18n::set(lang);
        for width in [320.0, 390.0, 768.0] {
            ui.h.set_size(egui::vec2(width, 844.0));
            ui.settle();
            for label in [
                "Group together",
                "Keep apart",
                "Keep at table",
                "Clear selection (Esc)",
            ] {
                let rect = ui.node(crate::i18n::tr(label)).rect();
                assert!(
                    ui.canvas().contains_rect(rect),
                    "width={width}, label={label}, rect={rect:?}, canvas={:?}",
                    ui.canvas()
                );
            }
        }
    }
    crate::i18n::set(crate::i18n::Lang::En);
    ui.settle();
    ui.click("Clear selection (Esc)");
    assert!(ui.app().chart.multi.is_empty());
}

#[test]
fn phone_import_maps_long_headers_and_adds_people() {
    let mut ui = phone(320.0);
    ui.app_mut().page = Page::People;
    let before = ui.app().project.attendees.len();
    let sheet = format!(
        "{},{}\nX,ABC\nY,XYZ",
        "Column 1 ".repeat(12),
        "Column 2 ".repeat(12)
    );
    ui.app_mut().load_sheet_file("import.csv", sheet.as_bytes());
    ui.settle();
    ui.click("Columns");
    assert!(
        overflowing_controls(&ui, 320.0).is_empty(),
        "{:?}",
        overflowing_controls(&ui, 320.0)
    );
    ui.open_combo("Kept as extra info");
    assert!(
        overflowing_controls(&ui, 320.0).is_empty(),
        "{:?}",
        overflowing_controls(&ui, 320.0)
    );
    ui.choose("Name");
    ui.open_combo("Kept as extra info");
    ui.choose("Organization");
    assert!(ui.has_text("To add: 2 · to update: 0"), "{:?}", ui.dump());
    ui.click("Import");
    assert_eq!(
        ui.app().project.attendees.len(),
        before + 2,
        "status={}, widgets={:?}",
        ui.app().status,
        ui.dump()
    );
    assert_eq!(ui.app().project.attendees[before].organization, "ABC");
}

#[test]
fn short_phone_keeps_people_reachable_with_bulk_actions_open() {
    let mut ui = Ui::with_example();
    let people: Vec<_> = ui
        .app()
        .project
        .attendees
        .iter()
        .take(3)
        .map(|a| (a.id, a.name.clone()))
        .collect();
    let app = ui.app_mut();
    app.page = Page::People;
    app.people.selecting = true;
    app.people.picked = people.iter().map(|(id, _)| *id).collect();
    ui.h.set_size(egui::vec2(320.0, 568.0));
    ui.settle();
    ui.node(&people[0].1).scroll_to_me();
    ui.settle();
    let row = ui.node(&people[0].1).rect();
    assert!(row.top() >= 0.0 && row.bottom() <= 568.0, "{row:?}");
    ui.click(&people[0].1);
    assert!(!ui.app().people.picked.contains(&people[0].0));
    ui.click("Clear");
    assert!(ui.app().people.picked.is_empty());
}

#[test]
fn phone_editors_keep_controls_inside_the_viewport() {
    let mut ui = Ui::with_example();
    let first = ui
        .app()
        .project
        .rules
        .iter()
        .find(|r| matches!(r.kind, crate::model::RuleKind::Proximity { .. }))
        .expect("a wish")
        .owner();
    ui.app_mut()
        .project
        .notes
        .extend([false, true].map(|reviewed| crate::model::ImportNote {
            person: first,
            field: crate::model::ImportField::Wish,
            text: "Guest 02".into(),
            reviewed,
        }));
    let mut overflows = Vec::new();
    for lang in [crate::i18n::Lang::En, crate::i18n::Lang::Fi] {
        crate::i18n::set(lang);
        for width in [320.0, 390.0, 599.0, 768.0] {
            ui.h.set_size(egui::vec2(width, 1000.0));
            for state in 0..7 {
                let app = ui.app_mut();
                app.page = Page::People;
                app.people.import_open = state == 0;
                app.people.import_text =
                    "Name,Organization,Food,Drinks\nX,ABC,V,0\nY,XYZ,V,0".into();
                app.selected_person = (1..=3).contains(&state).then_some(first);
                app.people.editing_details = state == 1;
                app.people.editing_rule = if state == 2 {
                    app.project
                        .rules
                        .iter()
                        .find(|r| r.owner() == first)
                        .map(|r| r.id)
                } else {
                    None
                };
                app.people.wish_draft = (state == 3).then(|| crate::app::people::WishDraft {
                    person: first,
                    wish: crate::model::WishKind::Seating,
                    near: Default::default(),
                    targets: vec![],
                    source: String::new(),
                    note: None,
                });
                if state == 4 {
                    app.page = Page::Graph;
                }
                if state >= 5 {
                    app.page = Page::Chart;
                    app.chart.mobile_view = crate::app::chart::MobileChart::Details;
                    if state == 6 {
                        app.selected_person = Some(first);
                    }
                }
                ui.settle();
                if state == 4 && ui.has(crate::i18n::tr("Details")) {
                    ui.click(crate::i18n::tr("Details"));
                }
                for overflow in overflowing_controls(&ui, width) {
                    overflows.push(format!(
                        "lang={lang:?}, width={width}, state={state}, {overflow}"
                    ));
                }
            }
        }
    }
    assert!(overflows.is_empty(), "{}", overflows.join("\n"));
}

#[test]
fn phone_pages_keep_controls_inside_the_viewport() {
    let mut ui = Ui::with_example();
    let mut overflows = Vec::new();
    for lang in [crate::i18n::Lang::En, crate::i18n::Lang::Fi] {
        crate::i18n::set(lang);
        for width in [320.0, 360.0, 390.0, 430.0, 599.0] {
            ui.h.set_size(egui::vec2(width, 844.0));
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
                for overflow in overflowing_controls(&ui, width) {
                    overflows.push(format!(
                        "lang={lang:?}, width={width}, page={}, {overflow}",
                        page as u8
                    ));
                }
            }
        }
    }
    assert!(overflows.is_empty(), "{}", overflows.join("\n"));
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

/// Touch events and the mouse-like events emitted alongside them by the web backend.
fn browser_touch(
    ui: &mut Ui,
    id: u64,
    phase: egui::TouchPhase,
    pos: egui::Pos2,
    primary: egui::Pos2,
) {
    match phase {
        egui::TouchPhase::Start | egui::TouchPhase::End => {
            ui.h.event(egui::Event::PointerButton {
                pos: primary,
                button: PointerButton::Primary,
                pressed: phase == egui::TouchPhase::Start,
                modifiers: Modifiers::NONE,
            });
            if phase == egui::TouchPhase::End {
                ui.h.event(egui::Event::PointerGone);
            }
        }
        egui::TouchPhase::Move => ui.h.event(egui::Event::PointerMoved(primary)),
        egui::TouchPhase::Cancel => ui.h.event(egui::Event::PointerGone),
    }
    ui.h.event(egui::Event::Touch {
        device_id: egui::TouchDeviceId(0),
        id: egui::TouchId(id),
        phase,
        pos,
        force: None,
    });
}

#[test]
fn a_pan_can_turn_into_a_pinch_and_back_without_lifting_both_fingers() {
    let mut ui = phone(390.0);
    ui.click("Seat by hand");
    let canvas = ui.canvas();
    let mut first = canvas.left_bottom() + egui::vec2(5.0, -5.0);
    ui.h.hover_at(first);
    ui.h.step();
    let initial_pan = ui.app().chart.view.camera.pan;
    let initial_zoom = ui.app().chart.view.camera.zoom;
    browser_touch(&mut ui, 1, egui::TouchPhase::Start, first, first);
    ui.h.step();
    for _ in 0..3 {
        first += egui::vec2(15.0, -15.0);
        browser_touch(&mut ui, 1, egui::TouchPhase::Move, first, first);
        ui.h.step();
    }
    assert_ne!(ui.app().chart.view.camera.pan, initial_pan);
    assert_eq!(ui.app().chart.view.camera.zoom, initial_zoom);

    let before = ui.app().chart.view.camera.target();
    let mut second = first + egui::vec2(90.0, -70.0);
    browser_touch(&mut ui, 2, egui::TouchPhase::Start, second, first);
    ui.h.step();
    assert_eq!(
        ui.app().chart.view.camera.target(),
        before,
        "adding a finger must not move the view"
    );
    for _ in 0..3 {
        let previous_center = first.lerp(second, 0.5);
        let camera = &ui.app().chart.view.camera;
        let anchor = camera.viewport(canvas).world(previous_center);
        let zoom = camera.zoom;
        let distance = first.distance(second);
        first += egui::vec2(-5.0, 5.0);
        second += egui::vec2(15.0, -5.0);
        browser_touch(&mut ui, 1, egui::TouchPhase::Move, first, first);
        browser_touch(&mut ui, 2, egui::TouchPhase::Move, second, first);
        ui.h.step();
        let camera = &ui.app().chart.view.camera;
        assert!((camera.zoom / zoom - first.distance(second) / distance).abs() < 0.001);
        assert!(
            camera
                .to_screen(canvas, anchor)
                .distance(first.lerp(second, 0.5))
                < 0.01,
            "the point under the fingers must follow their center"
        );
    }
    let before = ui.app().chart.view.camera.target();
    browser_touch(&mut ui, 2, egui::TouchPhase::End, second, first);
    ui.h.step();
    assert_eq!(ui.app().chart.view.camera.target(), before);
    first += egui::vec2(10.0, -10.0);
    browser_touch(&mut ui, 1, egui::TouchPhase::Move, first, first);
    ui.h.step();
    assert!(
        (ui.app().chart.view.camera.pan - before.1 - egui::vec2(10.0, -10.0)).length() < 0.01,
        "the remaining finger must resume panning"
    );
}

#[test]
fn pinch_keeps_the_canvas_captured_after_panning_over_its_edge() {
    let mut ui = phone(390.0);
    ui.click("Seat by hand");
    let canvas = ui.canvas();
    let mut first = canvas.left_top() + egui::vec2(5.0, 5.0);
    ui.h.hover_at(first);
    ui.h.step();
    browser_touch(&mut ui, 1, egui::TouchPhase::Start, first, first);
    ui.h.step();
    for _ in 0..3 {
        first.y -= 10.0;
        browser_touch(&mut ui, 1, egui::TouchPhase::Move, first, first);
        ui.h.step();
    }
    assert!(!canvas.contains(first));
    let mut second = canvas.left_top() + egui::vec2(150.0, 100.0);
    let before = ui.app().chart.view.camera.target();
    browser_touch(&mut ui, 2, egui::TouchPhase::Start, second, first);
    ui.h.step();
    assert_eq!(ui.app().chart.view.camera.target(), before);
    let anchor = ui
        .app()
        .chart
        .view
        .camera
        .viewport(canvas)
        .world(first.lerp(second, 0.5));
    // Re-enter the canvas while the pinch center stays fixed. Falling back
    // to mouse-style panning at this point would make the view jump.
    first += egui::vec2(20.0, 50.0);
    second -= egui::vec2(20.0, 50.0);
    assert!(canvas.contains(first));
    browser_touch(&mut ui, 1, egui::TouchPhase::Move, first, first);
    browser_touch(&mut ui, 2, egui::TouchPhase::Move, second, first);
    ui.h.step();
    let camera = &ui.app().chart.view.camera;
    assert!(camera.zoom < before.0);
    assert!(
        camera
            .to_screen(canvas, anchor)
            .distance(first.lerp(second, 0.5))
            < 0.01
    );
}

#[test]
fn lifting_the_first_finger_keeps_panning_and_does_not_click_the_chart() {
    let mut ui = phone(390.0);
    ui.click("Seat by hand");
    let selected = ui.app().project.attendees[0].id;
    ui.app_mut().selected_person = Some(selected);
    let mut first = ui.canvas().left_bottom() + egui::vec2(5.0, -5.0);
    ui.h.hover_at(first);
    ui.h.step();
    browser_touch(&mut ui, 1, egui::TouchPhase::Start, first, first);
    ui.h.step();
    first += egui::vec2(20.0, -20.0);
    browser_touch(&mut ui, 1, egui::TouchPhase::Move, first, first);
    ui.h.step();
    let mut second = first + egui::vec2(90.0, -70.0);
    browser_touch(&mut ui, 2, egui::TouchPhase::Start, second, first);
    ui.h.step();
    // Only the second finger moves: the browser can mistake its release for
    // a click because the synthetic pointer has stayed at the first finger.
    second += egui::vec2(30.0, -10.0);
    browser_touch(&mut ui, 2, egui::TouchPhase::Move, second, first);
    ui.h.step();
    let before = ui.app().chart.view.camera.target();
    browser_touch(&mut ui, 1, egui::TouchPhase::End, first, first);
    ui.h.step();
    assert_eq!(ui.app().selected_person, Some(selected));
    assert_eq!(ui.app().chart.view.camera.target(), before);
    second += egui::vec2(12.0, -8.0);
    browser_touch(&mut ui, 2, egui::TouchPhase::Move, second, second);
    ui.h.step();
    assert!((ui.app().chart.view.camera.pan - before.1 - egui::vec2(12.0, -8.0)).length() < 0.01);
    let before = ui.app().chart.view.camera.target();
    let third = second + egui::vec2(40.0, -20.0);
    browser_touch(&mut ui, 3, egui::TouchPhase::Start, third, second);
    ui.h.step();
    assert_eq!(ui.app().chart.view.camera.target(), before);
    browser_touch(&mut ui, 3, egui::TouchPhase::Cancel, third, second);
    ui.h.step();
    assert_eq!(ui.app().chart.view.camera.target(), before);
    browser_touch(&mut ui, 2, egui::TouchPhase::End, second, second);
    ui.h.step();
    assert_eq!(ui.app().selected_person, Some(selected));
    assert!(ui.option().assignment.is_empty());
    let canvas = ui.canvas();
    let seat = ui
        .app()
        .project
        .layout
        .seats()
        .into_iter()
        .find(|seat| canvas.contains(ui.app().seat_card_on_screen(&seat.id).unwrap().center()))
        .expect("a visible empty seat")
        .id;
    let at = ui.app().seat_card_on_screen(&seat).unwrap().center();
    browser_touch(&mut ui, 4, egui::TouchPhase::Start, at, at);
    ui.h.step();
    browser_touch(&mut ui, 4, egui::TouchPhase::End, at, at);
    ui.h.step();
    assert_eq!(
        ui.seat_of(selected),
        seat,
        "a fresh tap still seats a person"
    );
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

#[test]
fn sidebar_controls_stay_pinned_through_the_width_animation() {
    let mut ui = Ui::open_with_step_dt(1.0 / 60.0);
    ui.click("Try example");
    for size in [egui::vec2(1024.0, 768.0), egui::vec2(844.0, 390.0)] {
        ui.h.set_size(size);
        ui.settle();
        let theme = if ui.has("Dark mode") {
            "Dark mode"
        } else {
            "Light mode"
        };
        let controls = [theme, "EN", "About Plassi"];
        let positions: Vec<_> = controls
            .iter()
            .map(|label| ui.node(label).rect().center())
            .collect();
        let toggle = ui.node("Expand sidebar").rect().center();
        assert!((size.y - 26.0 - toggle.y).abs() < 1.0);
        let nav_y = ui.node("Seating chart").rect().top();
        for (click, next) in [
            ("Expand sidebar", "Collapse sidebar"),
            ("Collapse sidebar", "Expand sidebar"),
        ] {
            ui.node(click).click();
            let mut widths = Vec::new();
            for _ in 0..25 {
                ui.h.step();
                let nav = ui.node("Seating chart").rect();
                widths.push(nav.width());
                assert!((nav.top() - nav_y).abs() < 1.0);
                let button = if ui.has(next) { next } else { click };
                assert!(ui.node(button).rect().center().distance(toggle) < 1.0);
                for (label, position) in controls.iter().zip(&positions) {
                    assert!(ui.node(label).rect().center().distance(*position) < 1.0);
                }
            }
            assert!(
                widths
                    .windows(2)
                    .all(|pair| (pair[1] - pair[0]).abs() < 40.0),
                "{widths:?}"
            );
            assert!(ui.has(next));
        }
    }
}

#[test]
fn project_header_fits_long_names_and_renames_from_the_rail() {
    let mut ui = Ui::with_example();
    ui.app_mut()
        .rename_project("<project> <project> <project> <project>");
    let name = ui.app().file.name.clone();
    ui.h.set_size(egui::vec2(1024.0, 768.0));
    ui.settle();
    let compact = ui.node(&name).rect();
    assert!(compact.top() < 20.0 && compact.right() <= 84.0);
    ui.open_project_menu();
    assert!(ui.has("New project"));
    ui.click("Rename project");
    assert!(ui.has("Collapse sidebar"));
    fits(&ui, "Save project name", 244.0);
    fits(&ui, "Cancel rename", 244.0);
    ui.click("Save project name");
    let expanded = ui.node(&name).rect();
    assert!(expanded.top() < 20.0 && expanded.right() <= 244.0);
    assert!(expanded.width() > 200.0);
    let footer = ui.node("Collapse sidebar").rect().center();
    ui.click("Collapse sidebar");
    assert!(ui.node("Expand sidebar").rect().center().distance(footer) < 1.0);
    assert!((ui.node(&name).rect().top() - compact.top()).abs() < 1.0);
}
