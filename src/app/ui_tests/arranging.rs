//! Arranging people by hand: blocks, loose people, empty options and table pins.

use eframe::egui::{Key, Modifiers, PointerButton, Pos2};

use super::harness::Ui;
use crate::model::{Pin, Spot};

/// A solved example with the whole room in view.
fn solved() -> Ui {
    let mut ui = Ui::with_example();
    ui.solve();
    ui.key(Modifiers::NONE, Key::Escape); // ends the tour
    ui.key(Modifiers::NONE, Key::F);
    ui
}

/// Who sits on the first two left seats of the first two tables.
fn two_blocks(ui: &Ui) -> [[usize; 2]; 2] {
    let tables: Vec<String> = ui
        .app()
        .project
        .layout
        .rows
        .iter()
        .flat_map(|r| &r.tables)
        .map(|t| t.id.clone())
        .collect();
    let at = |table: &str, row: usize| {
        let seat = format!("{table}:left:{row}");
        let option = ui.option();
        *option
            .assignment
            .iter()
            .find(|(_, s)| **s == seat)
            .expect("seat taken")
            .0
    };
    [
        [at(&tables[0], 0), at(&tables[0], 1)],
        [at(&tables[1], 0), at(&tables[1], 1)],
    ]
}

/// A point on the chart floor below the tables.
fn floor(ui: &Ui) -> Pos2 {
    let canvas = ui.canvas();
    Pos2::new(canvas.center().x, canvas.bottom() - 24.0)
}

#[test]
fn a_selected_block_dropped_on_another_block_swaps_them() {
    let mut ui = solved();
    let [[a, b], [c, d]] = two_blocks(&ui);
    let seats = [a, b, c, d].map(|p| ui.seat_of(p));
    let primary = PointerButton::Primary;
    ui.click_at(ui.card_of(a).center(), primary, Modifiers::NONE);
    ui.click_at(ui.card_of(b).center(), primary, Modifiers::COMMAND);
    assert_eq!(ui.app().chart.multi, [a, b].into());
    ui.drag(ui.card_of(a).center(), ui.card_of(c).center());
    let now = [a, b, c, d].map(|p| ui.seat_of(p));
    assert_eq!(
        now,
        [&seats[2], &seats[3], &seats[0], &seats[1]].map(|s| s.clone())
    );
}

#[test]
fn people_dragged_off_their_seat_stay_loose_until_dropped_on_one() {
    let mut ui = solved();
    let [[a, b], _] = two_blocks(&ui);
    let (seat_a, seat_b) = (ui.seat_of(a), ui.seat_of(b));
    ui.drag(ui.card_of(a).center(), floor(&ui));
    assert!(matches!(ui.option().spot_of(a), Spot::Loose(_)));
    assert!(ui.has_text("Option 1  1 not seated"), "{:?}", ui.dump());
    // The empty seat is allowed while editing; nobody slides up.
    assert_eq!(ui.seat_of(b), seat_b);
    assert!(!ui.option().assignment.values().any(|s| *s == seat_a));
    // Dropped on a taken seat, its occupant takes the loose spot. The list
    // panel now takes room, so the room is fitted again first.
    ui.key(Modifiers::NONE, Key::F);
    let loose = ui.option().spot_of(a);
    ui.drag(ui.card_of(a).center(), ui.card_of(b).center());
    assert_eq!(ui.seat_of(a), seat_b);
    assert_eq!(ui.option().spot_of(b), loose);
}

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
    ui.drag(ui.node(&first.name).rect().center(), to);
    assert_eq!(ui.seat_of(first.id), seat);
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
fn a_person_can_be_kept_at_their_table() {
    let mut ui = solved();
    let [[a, _], _] = two_blocks(&ui);
    let table = crate::model::seat_table(&ui.seat_of(a)).to_string();
    ui.click_at(
        ui.card_of(a).center(),
        PointerButton::Secondary,
        Modifiers::NONE,
    );
    ui.click("Keep at this table");
    assert_eq!(ui.app().project.pins()[&a], Pin::Table(table));
}
