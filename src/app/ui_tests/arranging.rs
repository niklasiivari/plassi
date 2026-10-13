//! Arranging people by hand: blocks, loose people, empty options and table pins.

use eframe::egui::{self, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::kittest::Queryable;

use super::harness::Ui;
use crate::model::{Pin, Spot};

/// A solved example with the whole room in view.
pub(super) fn solved() -> Ui {
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
pub(super) fn floor(ui: &Ui) -> Pos2 {
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
fn shift_dragging_a_box_on_the_floor_selects_the_people_in_it() {
    let mut ui = solved();
    let [[a, b], [c, _]] = two_blocks(&ui);
    let (top, bottom) = (ui.card_of(a), ui.card_of(b));
    let pan = ui.app().chart.view.camera.pan;
    // From beside the first card to inside the second: both are touched.
    let from = top.left_top() - egui::vec2(12.0, 12.0);
    ui.drag_holding(from, bottom.center(), Modifiers::SHIFT);
    assert_eq!(ui.app().chart.multi, [a, b].into());
    assert_eq!(ui.app().chart.view.camera.pan, pan, "the box does not pan");
    assert!(!ui.app().chart.multi.contains(&c));
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

#[test]
fn a_group_move_seats_a_loose_member_and_the_project_stays_valid() {
    let mut ui = solved();
    let pinned = ui.app().project.seat_pinned_people();
    let person = ui
        .app()
        .project
        .attendees
        .iter()
        .map(|a| a.id)
        .find(|id| {
            crate::groups::group_of(&ui.app().project, *id)
                .is_some_and(|g| g.len() > 1 && g.iter().all(|p| !pinned.contains(p)))
        })
        .expect("an unpinned wish group");
    ui.drag(ui.card_of(person).center(), floor(&ui));
    assert!(matches!(ui.option().spot_of(person), Spot::Loose(_)));
    ui.key(Modifiers::NONE, Key::F);
    ui.click_at(
        ui.card_of(person).center(),
        PointerButton::Primary,
        Modifiers::NONE,
    );
    ui.click("Find places for this group");
    ui.click("Move");
    assert!(matches!(ui.option().spot_of(person), Spot::Seat(_)));
    assert!(ui.app().project.validate().is_ok());
}

#[test]
fn p_pins_every_picked_person_and_then_unpins_them() {
    let mut ui = solved();
    let [a, b] = ui.two_unpinned();
    let primary = PointerButton::Primary;
    ui.click_at(ui.card_of(a).center(), primary, Modifiers::COMMAND);
    ui.click_at(ui.card_of(b).center(), primary, Modifiers::COMMAND);
    ui.key(Modifiers::NONE, Key::P);
    let pins = ui.app().project.pins();
    assert_eq!(pins[&a], Pin::Seat(ui.seat_of(a)));
    assert_eq!(pins[&b], Pin::Seat(ui.seat_of(b)));
    ui.key(Modifiers::NONE, Key::P);
    let pins = ui.app().project.pins();
    assert!(!pins.contains_key(&a) && !pins.contains_key(&b));
}

#[test]
fn taking_someone_off_the_seats_says_so_in_the_history() {
    let mut ui = solved();
    let [a, _] = ui.two_unpinned();
    let name = ui.app().project.person_name(a).to_string();
    ui.click_at(
        ui.card_of(a).center(),
        PointerButton::Secondary,
        Modifiers::NONE,
    );
    ui.click("Take off the seat");
    assert_eq!(ui.option().spot_of(a), Spot::Unseated);
    let said = format!("Took {name} off the seats");
    assert!(ui.has_text(&said), "{:?}", ui.dump());
}

#[test]
fn a_block_with_someone_from_the_list_keeps_its_shape() {
    let mut ui = solved();
    let [[a, b], [c, d]] = two_blocks(&ui);
    let u = *ui.option().assignment.keys().max().expect("someone seated");
    assert!(![a, b, c, d].contains(&u));
    let secondary = PointerButton::Secondary;
    ui.click_at(ui.card_of(u).center(), secondary, Modifiers::NONE);
    ui.click("Take off the seat");
    ui.key(Modifiers::NONE, Key::F);
    let primary = PointerButton::Primary;
    for person in [a, b] {
        ui.click_at(ui.card_of(person).center(), primary, Modifiers::COMMAND);
    }
    let name = ui.app().project.person_name(u).to_string();
    ui.click_at(ui.node(&name).rect().center(), primary, Modifiers::COMMAND);
    assert_eq!(ui.app().chart.multi, [a, b, u].into());
    let (seat_c, seat_d) = (ui.seat_of(c), ui.seat_of(d));
    // The person from the list stacks below the block, not onto its seats.
    ui.drag(ui.card_of(a).center(), ui.card_of(c).center());
    assert_eq!(ui.seat_of(a), seat_c);
    assert_eq!(ui.seat_of(b), seat_d);
    assert!(ui.option().assignment.contains_key(&u));
}

#[test]
fn an_option_can_forget_who_was_placed_by_hand() {
    let mut ui = solved();
    let [[a, _], [c, _]] = two_blocks(&ui);
    ui.drag(ui.card_of(a).center(), ui.card_of(c).center());
    assert!(!ui.option().hand_placed.is_empty());
    let chip = ui.app().project.candidates[0].name.clone();
    let at =
        ui.h.query_all_by(|n| n.label().is_some_and(|l| l.starts_with(&chip)))
            .next()
            .expect("option chip")
            .rect()
            .center();
    ui.click_at(at, PointerButton::Secondary, Modifiers::NONE);
    ui.click("Let Improve move everyone");
    assert!(ui.option().hand_placed.is_empty());
}
