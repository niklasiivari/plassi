//! Placing people without a seat: from the "Not seated" list, by clicking
//! an empty seat, and from suggested free seats.

use eframe::egui::{self, Key, Modifiers, PointerButton};
use egui_kittest::kittest::Queryable;

use super::arranging::{floor, solved};
use super::harness::Ui;
use crate::model::{Pin, Spot};

#[test]
fn an_empty_option_is_seated_by_hand_and_then_the_rest() {
    let mut ui = Ui::with_example();
    ui.click("Seat by hand");
    let people = ui.app().project.attendees.len();
    assert_eq!(ui.app().project.candidates.len(), 1);
    assert_eq!(ui.option().unseated(&ui.app().project).len(), people);
    ui.key(Modifiers::NONE, Key::Escape);
    ui.key(Modifiers::NONE, Key::F);
    // From the list onto a seat.
    let first = ui.app().project.attendees[0].clone();
    let seat = ui.app().project.layout.seats()[0].id.clone();
    let to = ui
        .app()
        .seat_card_on_screen(&seat)
        .expect("seat on screen")
        .center();
    // Unfinished, the overview counts people without a seat instead of
    // scoring them, and lists nothing about them.
    assert!(
        ui.has_text(&format!("{people} not seated")),
        "{:?}",
        ui.dump()
    );
    assert!(!ui.has_text("SCORE"));
    assert!(!ui.has_text("No wished-for people near"));
    assert!(
        ui.has_text("Nothing for the people seated so far"),
        "{:?}",
        ui.dump()
    );
    ui.drag(ui.node(&first.name).rect().center(), to);
    assert_eq!(ui.seat_of(first.id), seat);
    // Their partner has no seat yet, so that wish is not flagged.
    assert!(ui.has_text("Waiting for people not seated"));
    ui.key(Modifiers::NONE, Key::Escape);
    assert!(!ui.has_text("No wished-for people near"));
    assert!(ui.has_text("Nothing for the people seated so far"));
    // "Seat the rest" adds an option with everyone seated, keeping the
    // person placed by hand.
    ui.click("Seat the rest");
    ui.wait_until(20, |app| {
        app.solving.is_none() && app.project.candidates.len() == 2
    });
    let filled = &ui.app().project.candidates[1];
    assert_eq!(filled.assignment.len(), people);
    assert_eq!(filled.assignment[&first.id], seat);
}

#[test]
fn someone_without_a_seat_is_offered_the_best_free_seats() {
    let mut ui = Ui::with_example();
    ui.click("Seat by hand");
    ui.key(Modifiers::NONE, Key::Escape);
    let first = ui.app().project.attendees[0].clone();
    ui.click(&first.name);
    assert!(ui.has_text("Seat here"), "{:?}", ui.dump());
    // Without a seat their wishes are not judged: no status, no penalty.
    assert!(!ui.has_text("Waiting for people not seated"));
    assert!(!ui.has_text("−4000"));
    // Their wish lines start at their card in the list.
    let anchor = ui.app().chart.tray_anchor.map(|(person, _)| person);
    assert_eq!(anchor, Some(first.id));
    let best = {
        let app = ui.app();
        crate::arrange::free_seats(&app.project, ui.option(), first.id)[0]
            .0
            .clone()
    };
    // Pointing at a suggestion marks its seat on the chart.
    let row = ui.node("Seat here").rect().center();
    ui.h.hover_at(row);
    ui.settle();
    assert_eq!(ui.app().chart.group_preview, Some(vec![best.clone()]));
    ui.click("Seat here");
    assert_eq!(ui.seat_of(first.id), best);
    // Seated, they get swaps instead.
    assert!(!ui.has_text("Seat here"));
}

#[test]
fn people_dragged_together_from_the_list_fill_a_table_side() {
    let mut ui = Ui::with_example();
    ui.click("Seat by hand");
    ui.key(Modifiers::NONE, Key::Escape);
    ui.key(Modifiers::NONE, Key::F);
    let side: Vec<String> = {
        let seats = ui.app().project.layout.seats();
        let first = seats[0].clone();
        seats
            .into_iter()
            .filter(|s| s.table_id == first.table_id && s.side == first.side)
            .map(|s| s.id)
            .collect()
    };
    let unseated = ui.option().unseated(&ui.app().project);
    let people: Vec<_> = unseated[..side.len()]
        .iter()
        .map(|id| ui.app().project.person(*id).expect("person").clone())
        .collect();
    for person in &people {
        let at = ui.node(&person.name).rect().center();
        ui.click_at(at, PointerButton::Primary, Modifiers::COMMAND);
    }
    let to = ui
        .app()
        .seat_card_on_screen(&side[0])
        .expect("seat on screen")
        .center();
    ui.drag(ui.node(&people[0].name).rect().center(), to);
    let seated: Vec<String> = people.iter().map(|p| ui.seat_of(p.id)).collect();
    assert_eq!(seated, side);
}

#[test]
fn clicking_an_empty_seat_seats_the_person_selected_in_the_list() {
    let mut ui = Ui::with_example();
    ui.click("Seat by hand");
    ui.key(Modifiers::NONE, Key::Escape);
    ui.key(Modifiers::NONE, Key::F);
    let first = ui.app().project.attendees[0].clone();
    ui.click(&first.name);
    let seat = ui.app().project.layout.seats()[3].id.clone();
    let at = ui.app().seat_card_on_screen(&seat).expect("seat").center();
    ui.click_at(at, PointerButton::Primary, Modifiers::NONE);
    assert_eq!(ui.seat_of(first.id), seat);
    // Seated now, a click on another empty seat only clears the selection.
    let other = ui.app().project.layout.seats()[5].id.clone();
    let at = ui.app().seat_card_on_screen(&other).expect("seat").center();
    ui.click_at(at, PointerButton::Primary, Modifiers::NONE);
    assert_eq!(ui.seat_of(first.id), seat);
    assert_eq!(ui.app().selected_person, None);
}

#[test]
fn someone_without_a_seat_can_be_kept_at_a_chosen_table() {
    let mut ui = Ui::with_example();
    ui.click("Seat by hand");
    ui.key(Modifiers::NONE, Key::Escape);
    let first = ui.app().project.attendees[0].clone();
    ui.click(&first.name);
    let table = ui.app().project.layout.rows[0].tables[1].clone();
    ui.click("Keep at table");
    ui.click(&table.label);
    assert_eq!(ui.app().project.pins()[&first.id], Pin::Table(table.id));
}

#[test]
fn a_right_button_drag_in_the_list_moves_nobody() {
    let mut ui = solved();
    let [a, _] = ui.two_unpinned();
    ui.drag(ui.card_of(a).center(), floor(&ui));
    let loose = ui.option().spot_of(a);
    assert!(matches!(loose, Spot::Loose(_)));
    ui.key(Modifiers::NONE, Key::F);
    let name = ui.app().project.person_name(a).to_string();
    // Their card in the list, not the one on the chart.
    let tray = ui.app().chart.tray_rect.expect("list shown");
    let from =
        ui.h.query_all_by_label(&name)
            .map(|n| n.rect().center())
            .find(|at| tray.contains(*at))
            .expect("card in the list");
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    ui.h.hover_at(from);
    ui.h.step();
    ui.h.event(press(from, true));
    ui.h.step();
    for dy in [5.0, 15.0, 30.0] {
        ui.h.hover_at(from + egui::vec2(0.0, dy));
        ui.h.step();
    }
    ui.h.event(press(from + egui::vec2(0.0, 30.0), false));
    ui.h.step();
    ui.settle();
    assert_eq!(ui.option().spot_of(a), loose);
}

#[test]
fn finding_someone_without_a_seat_scrolls_the_list_to_them() {
    let mut ui = Ui::with_example();
    ui.click("Seat by hand");
    ui.key(Modifiers::NONE, Key::Escape);
    let last = ui.app().project.attendees.last().expect("people").clone();
    ui.key(Modifiers::COMMAND, Key::F);
    ui.type_text(&last.name);
    ui.key(Modifiers::NONE, Key::Enter);
    assert_eq!(ui.app().selected_person, Some(last.id));
    // Selected and in view in the list, their wish lines start there.
    let anchor = ui.app().chart.tray_anchor.map(|(person, _)| person);
    assert_eq!(anchor, Some(last.id));
}

#[test]
fn the_list_menu_seats_a_selection_in_the_best_free_seats() {
    let mut ui = Ui::with_example();
    ui.click("Seat by hand");
    ui.key(Modifiers::NONE, Key::Escape);
    let people: Vec<_> = ui.app().project.attendees[..3].to_vec();
    for person in &people {
        let at = ui.node(&person.name).rect().center();
        ui.click_at(at, PointerButton::Primary, Modifiers::COMMAND);
    }
    let at = ui.node(&people[0].name).rect().center();
    ui.click_at(at, PointerButton::Secondary, Modifiers::NONE);
    ui.click("Seat 3 people in the best free seats");
    let seats: std::collections::BTreeSet<String> =
        people.iter().map(|p| ui.seat_of(p.id)).collect();
    assert_eq!(seats.len(), 3);
    // One edit, so one undo puts them all back.
    ui.key(Modifiers::COMMAND, Key::Z);
    let unseated = ui.option().unseated(&ui.app().project);
    assert!(people.iter().all(|p| unseated.contains(&p.id)));
}

#[test]
fn the_list_menu_leaves_seated_people_in_the_selection_where_they_are() {
    let mut ui = solved();
    let [a, b] = ui.two_unpinned();
    let seat_b = ui.seat_of(b);
    let primary = PointerButton::Primary;
    ui.click_at(ui.card_of(a).center(), primary, Modifiers::NONE);
    ui.key(Modifiers::NONE, Key::Delete);
    assert_eq!(ui.option().spot_of(a), Spot::Unseated);
    ui.key(Modifiers::NONE, Key::Escape);
    ui.key(Modifiers::NONE, Key::F);
    ui.click_at(ui.card_of(b).center(), primary, Modifiers::COMMAND);
    let name = ui.app().project.person_name(a).to_string();
    let tray = ui.app().chart.tray_rect.expect("list shown");
    let at =
        ui.h.query_all_by_label(&name)
            .map(|n| n.rect().center())
            .find(|at| tray.contains(*at))
            .expect("card in the list");
    ui.click_at(at, primary, Modifiers::COMMAND);
    ui.click_at(at, PointerButton::Secondary, Modifiers::NONE);
    ui.click("Seat in the best free seat");
    assert_eq!(ui.seat_of(b), seat_b);
    assert!(matches!(ui.option().spot_of(a), Spot::Seat(_)));
}
