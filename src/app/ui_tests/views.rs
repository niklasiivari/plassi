//! Room changes that keep the seating, colouring by category and table summaries.

use eframe::egui::{Key, Modifiers, PointerButton};

use super::harness::Ui;
use crate::catalog::Kind;
use crate::model::Spot;

/// A solved example with the whole room in view.
fn solved() -> Ui {
    let mut ui = Ui::with_example();
    ui.solve();
    ui.key(Modifiers::NONE, Key::Escape); // ends the tour
    ui.key(Modifiers::NONE, Key::F);
    ui
}

fn first_table(ui: &Ui) -> String {
    ui.app().project.layout.rows[0].tables[0].id.clone()
}

#[test]
fn removing_a_table_keeps_everyone_else_seated() {
    let mut ui = solved();
    let table = first_table(&ui);
    let before = ui.option().assignment.clone();
    let at_table: Vec<usize> = before
        .iter()
        .filter(|(_, seat)| seat.starts_with(&format!("{table}:")))
        .map(|(p, _)| *p)
        .collect();
    assert!(!at_table.is_empty());

    ui.go_to("Room layout");
    ui.click("×"); // the first table's remove button
    assert_eq!(ui.app().project.candidates.len(), 1);
    for (person, seat) in &before {
        let expected = if at_table.contains(person) {
            Spot::Unseated
        } else {
            Spot::Seat(seat.clone())
        };
        assert_eq!(ui.option().spot_of(*person), expected);
    }
    assert!(
        ui.app().status.contains("lost their seats"),
        "{}",
        ui.app().status
    );
}

#[test]
fn cards_can_be_coloured_by_food() {
    let mut ui = solved();
    ui.open_combo("Nothing");
    ui.choose("Food");
    assert_eq!(ui.app().chart.color_by, Some(Kind::Food));
    let project = &ui.app().project;
    let food = project
        .attendees
        .iter()
        .map(|a| project.category_name(Kind::Food, a))
        .find(|name| !name.is_empty())
        .expect("someone chose a meal");
    assert!(ui.has(&food), "{:?}", ui.dump());

    // Hovering a category in the legend picks it out.
    ui.node(&food).hover();
    ui.settle();
    let key = {
        let project = &ui.app().project;
        let a = project
            .attendees
            .iter()
            .find(|a| project.category_name(Kind::Food, a) == food)
            .unwrap();
        project.category_key(Kind::Food, a)
    };
    assert_eq!(ui.app().chart.color_hover, Some(key));

    // The choice is remembered.
    let ui = ui.reopen();
    assert_eq!(ui.app().chart.color_by, Some(Kind::Food));
}

#[test]
fn clicking_a_table_shows_its_summary() {
    let mut ui = solved();
    let table = first_table(&ui);
    let at = ui.app().table_on_screen(&table).expect("table on screen");
    ui.click_at(at, PointerButton::Primary, Modifiers::NONE);
    assert_eq!(
        ui.app().chart.selected_table.as_deref(),
        Some(table.as_str())
    );
    assert!(ui.has_text("seats taken"), "{:?}", ui.dump());

    let someone = *ui
        .option()
        .assignment
        .iter()
        .find(|(_, s)| s.starts_with(&format!("{table}:")))
        .unwrap()
        .0;
    let name = ui.app().project.person(someone).unwrap().name.clone();
    ui.click(&name);
    assert_eq!(ui.app().selected_person, Some(someone));
    assert_eq!(ui.app().chart.selected_table, None);
}
