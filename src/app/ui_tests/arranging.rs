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
fn taking_someone_off_the_seats_names_the_list_in_the_history() {
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
    let said = format!("Moved {name} to the Not seated list");
    assert!(ui.has_text(&said), "{:?}", ui.dump());
}

#[test]
fn delete_takes_the_picked_people_off_their_seats() {
    let mut ui = solved();
    let [a, b] = ui.two_unpinned();
    let primary = PointerButton::Primary;
    ui.click_at(ui.card_of(a).center(), primary, Modifiers::COMMAND);
    ui.click_at(ui.card_of(b).center(), primary, Modifiers::COMMAND);
    ui.key(Modifiers::NONE, Key::Delete);
    assert_eq!(ui.option().spot_of(a), Spot::Unseated);
    assert_eq!(ui.option().spot_of(b), Spot::Unseated);
}

#[test]
fn arrow_keys_move_the_selected_person_one_seat() {
    let mut ui = solved();
    let seats = ui.app().project.layout.seats();
    let top = &seats[0];
    let below = seats
        .iter()
        .find(|s| s.table_id == top.table_id && s.side == top.side && s.index == top.index + 1)
        .expect("seat below")
        .id
        .clone();
    let occupant = |ui: &Ui, seat: &str| {
        let option = ui.option();
        option
            .assignment
            .iter()
            .find(|(_, s)| *s == seat)
            .map(|(p, _)| *p)
    };
    let a = occupant(&ui, &top.id).expect("someone on the top seat");
    let b = occupant(&ui, &below).expect("someone below");
    ui.click_at(
        ui.card_of(a).center(),
        PointerButton::Primary,
        Modifiers::NONE,
    );
    ui.key(Modifiers::NONE, Key::ArrowDown);
    assert_eq!(ui.seat_of(a), below);
    assert_eq!(ui.seat_of(b), top.id);
    // Nothing above the top seat.
    ui.key(Modifiers::NONE, Key::ArrowUp);
    ui.key(Modifiers::NONE, Key::ArrowUp);
    assert_eq!(ui.seat_of(a), top.id);
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

#[test]
fn a_box_takes_in_whoever_was_selected_like_a_ctrl_click() {
    let mut ui = solved();
    let [[a, b], [c, _]] = two_blocks(&ui);
    ui.click_at(
        ui.card_of(c).center(),
        PointerButton::Primary,
        Modifiers::NONE,
    );
    let (top, bottom) = (ui.card_of(a), ui.card_of(b));
    let from = top.left_top() - egui::vec2(12.0, 12.0);
    ui.drag_holding(from, bottom.center(), Modifiers::SHIFT);
    assert_eq!(ui.app().chart.multi, [a, b, c].into());
}

#[test]
fn p_on_a_selection_with_someone_unseated_pins_the_seated() {
    let mut ui = solved();
    let [a, b] = ui.two_unpinned();
    let primary = PointerButton::Primary;
    ui.click_at(
        ui.card_of(b).center(),
        PointerButton::Secondary,
        Modifiers::NONE,
    );
    ui.click("Take off the seat");
    ui.click_at(ui.card_of(a).center(), primary, Modifiers::COMMAND);
    ui.key(Modifiers::NONE, Key::P);
    assert!(ui.app().project.pins().contains_key(&a));
    // With b in the selection, P still unpins a.
    let name = ui.app().project.person_name(b).to_string();
    ui.click_at(ui.node(&name).rect().center(), primary, Modifiers::COMMAND);
    assert_eq!(ui.app().chart.multi, [a, b].into());
    ui.key(Modifiers::NONE, Key::P);
    assert!(!ui.app().project.pins().contains_key(&a));
}

#[test]
fn the_list_appearing_keeps_a_zoomed_in_view() {
    let mut ui = solved();
    let [a, b] = ui.two_unpinned();
    let primary = PointerButton::Primary;
    // Fitted, the room is fitted again when the list appears.
    let fitted = ui.app().chart.view.camera.target();
    ui.click_at(ui.card_of(a).center(), primary, Modifiers::NONE);
    ui.key(Modifiers::NONE, Key::Delete);
    assert_ne!(ui.app().chart.view.camera.target(), fitted);
    // Zoomed in by hand, the view stays when the list appears.
    ui.key(Modifiers::COMMAND, Key::Z);
    ui.app_mut().chart.view.camera.zoom_by(2.0);
    ui.settle();
    let zoomed = ui.app().chart.view.camera.target();
    ui.click_at(ui.card_of(b).center(), primary, Modifiers::NONE);
    ui.key(Modifiers::NONE, Key::Delete);
    assert_eq!(ui.app().chart.view.camera.target(), zoomed);
}

#[test]
fn a_pin_moved_onto_a_seat_pinned_to_someone_else_says_so() {
    let mut ui = solved();
    let [a, b] = ui.two_unpinned();
    let seat_b = ui.seat_of(b);
    let other = ui
        .app()
        .project
        .attendees
        .iter()
        .map(|x| x.id)
        .find(|id| ![a, b].contains(id) && !ui.app().project.pins().contains_key(id))
        .expect("a third person");
    // Someone pinned to b's seat while sitting elsewhere, as after pinning
    // in another option.
    let app = ui.app_mut();
    app.project.pin(
        a,
        Pin::Seat(app.project.candidates[0].assignment[&a].clone()),
    );
    app.project.pin(other, Pin::Seat(seat_b.clone()));
    app.commit("pins");
    ui.settle();
    ui.drag(ui.card_of(a).center(), ui.card_of(b).center());
    assert_eq!(ui.seat_of(a), seat_b);
    let name = ui.app().project.person_name(other).to_string();
    let said = format!("{name} is no longer pinned to that seat");
    assert!(ui.has_text(&said), "{:?}", ui.dump());
}

#[test]
fn a_loose_card_dropped_on_another_does_not_cover_it() {
    let mut ui = solved();
    let [a, b] = ui.two_unpinned();
    let app = ui.app_mut();
    let below = app
        .project
        .layout
        .seats()
        .iter()
        .map(|s| s.y)
        .fold(0.0, f32::max)
        + 200.0;
    let option = &mut app.project.candidates[0];
    option.place(a, Spot::Loose(crate::model::Point { x: 0.0, y: below }));
    option.place(b, Spot::Loose(crate::model::Point { x: 300.0, y: below }));
    app.commit("loose");
    ui.key(Modifiers::NONE, Key::F);
    ui.drag(ui.card_of(a).center(), ui.card_of(b).center());
    assert!(matches!(ui.option().spot_of(a), Spot::Loose(_)));
    let (ra, rb) = (ui.card_of(a), ui.card_of(b));
    assert!(!ra.intersects(rb), "{ra:?} {rb:?}");
}

#[test]
fn seating_someone_on_a_seat_pinned_to_another_person_warns() {
    let mut ui = solved();
    let [a, b] = ui.two_unpinned();
    let seat = ui.seat_of(a);
    // Pinned to that seat but without a seat in this option, as after
    // pinning in another option.
    let app = ui.app_mut();
    app.project.pin(a, Pin::Seat(seat.clone()));
    app.project.candidates[0].place(a, Spot::Unseated);
    app.commit("pin");
    ui.key(Modifiers::NONE, Key::F);
    let to = ui.app().seat_card_on_screen(&seat).expect("seat").center();
    ui.drag(ui.card_of(b).center(), to);
    assert_eq!(ui.seat_of(b), seat);
    let name = ui.app().project.person_name(a).to_string();
    let said = format!("the seat is pinned to {name}");
    assert!(ui.has_text(&said), "{:?}", ui.dump());
}

#[test]
fn a_block_of_loose_cards_dropped_on_a_loose_card_keeps_clear() {
    let mut ui = solved();
    let people: Vec<usize> = ui.option().assignment.keys().copied().take(3).collect();
    let [a, b, c] = [people[0], people[1], people[2]];
    let app = ui.app_mut();
    let below = app
        .project
        .layout
        .seats()
        .iter()
        .map(|s| s.y)
        .fold(0.0, f32::max)
        + 200.0;
    let option = &mut app.project.candidates[0];
    let at = |x, y| Spot::Loose(crate::model::Point { x, y });
    option.place(a, at(0.0, below));
    option.place(b, at(0.0, below + 56.0));
    option.place(c, at(400.0, below));
    app.commit("loose");
    ui.key(Modifiers::NONE, Key::F);
    let primary = PointerButton::Primary;
    ui.click_at(ui.card_of(a).center(), primary, Modifiers::COMMAND);
    ui.click_at(ui.card_of(b).center(), primary, Modifiers::COMMAND);
    ui.drag(ui.card_of(a).center(), ui.card_of(c).center());
    let cards = [ui.card_of(a), ui.card_of(b), ui.card_of(c)];
    for (i, x) in cards.iter().enumerate() {
        for y in &cards[i + 1..] {
            assert!(!x.intersects(*y), "{cards:?}");
        }
    }
}

#[test]
fn shifting_a_pinned_block_reports_no_lost_pin() {
    let mut ui = solved();
    let seats = ui.app().project.layout.seats();
    let (top, next) = (seats[0].id.clone(), seats[1].id.clone());
    let on = |ui: &Ui, seat: &str| {
        let option = ui.option();
        option
            .assignment
            .iter()
            .find(|(_, s)| *s == seat)
            .map(|(p, _)| *p)
    };
    let (x, y) = (on(&ui, &top).expect("x"), on(&ui, &next).expect("y"));
    let app = ui.app_mut();
    app.project.pin(x, Pin::Seat(top));
    app.project.pin(y, Pin::Seat(next));
    app.commit("pins");
    let primary = PointerButton::Primary;
    ui.click_at(ui.card_of(x).center(), primary, Modifiers::COMMAND);
    ui.click_at(ui.card_of(y).center(), primary, Modifiers::COMMAND);
    ui.key(Modifiers::NONE, Key::ArrowDown);
    assert!(!ui.has_text("no longer pinned"), "{:?}", ui.dump());
    let pins = ui.app().project.pins();
    assert_eq!(pins[&x], Pin::Seat(ui.seat_of(x)));
    assert_eq!(pins[&y], Pin::Seat(ui.seat_of(y)));
}

#[test]
fn the_pin_item_for_a_selection_counts_only_who_it_pins() {
    let mut ui = solved();
    let [a, b] = ui.two_unpinned();
    let primary = PointerButton::Primary;
    ui.click_at(ui.card_of(a).center(), primary, Modifiers::NONE);
    ui.key(Modifiers::NONE, Key::P);
    ui.click_at(ui.card_of(a).center(), primary, Modifiers::COMMAND);
    ui.click_at(ui.card_of(b).center(), primary, Modifiers::COMMAND);
    ui.click_at(
        ui.card_of(a).center(),
        PointerButton::Secondary,
        Modifiers::NONE,
    );
    let name = ui.app().project.person_name(b).to_string();
    ui.click(&format!("Pin {name} to their seat (P)"));
    assert_eq!(ui.app().project.pins()[&b], Pin::Seat(ui.seat_of(b)));
}
