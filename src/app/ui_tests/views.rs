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
    let mut ui = ui.reopen();
    assert_eq!(ui.app().chart.color_by, Some(Kind::Food));

    // C steps through the choices and back to none.
    ui.key(Modifiers::NONE, Key::C);
    assert_eq!(ui.app().chart.color_by, Some(Kind::Drink));
    ui.key(Modifiers::NONE, Key::C);
    assert_eq!(ui.app().chart.color_by, None);
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

#[test]
fn a_table_summary_lists_wishes_that_leave_the_table() {
    use crate::model::{Importance, Rule, RuleKind, Target, WishKind};
    let mut ui = solved();
    let tables: Vec<String> = ui.app().project.layout.rows[0]
        .tables
        .iter()
        .map(|t| t.id.clone())
        .collect();
    let at = |ui: &Ui, table: &str| {
        *ui.option()
            .assignment
            .iter()
            .find(|(_, s)| s.starts_with(&format!("{table}:")))
            .unwrap()
            .0
    };
    let (from, to) = (at(&ui, &tables[0]), at(&ui, &tables[1]));
    let app = ui.app_mut();
    let id = app.project.next_rule_id();
    app.project.rules.push(Rule {
        id,
        label: "Seating wish".into(),
        source: "test".into(),
        enabled: true,
        kind: RuleKind::Proximity {
            requester: from,
            target: Target::People { ids: vec![to] },
            wish: WishKind::Seating,
            importance: Importance::Normal,
            max_class: None,
        },
    });
    app.commit("Added a wish");
    ui.settle();

    let point = ui.app().table_on_screen(&tables[0]).unwrap();
    ui.click_at(point, PointerButton::Primary, Modifiers::NONE);
    let line = ui
        .dump()
        .into_iter()
        .find(|t| t.contains('→'))
        .expect("the wish is listed");
    ui.click(&line);
    assert_eq!(ui.app().selected_person, Some(to));
}

#[test]
fn removing_a_table_with_pinned_loose_and_moved_people() {
    let mut ui = solved();
    let table = first_table(&ui);
    let seat = |i: usize| format!("{table}:left:{i}");
    let on = |ui: &Ui, seat: &str| {
        *ui.option()
            .assignment
            .iter()
            .find(|(_, s)| *s == seat)
            .unwrap()
            .0
    };
    let (pinned, loose, moved) = (on(&ui, &seat(0)), on(&ui, &seat(1)), on(&ui, &seat(2)));
    {
        let app = ui.app_mut();
        app.project.pin(pinned, crate::model::Pin::Seat(seat(0)));
        let option = &mut app.project.candidates[app.selected_candidate];
        option.place(
            loose,
            Spot::Loose(crate::model::Point { x: -200.0, y: 0.0 }),
        );
        option.hand_placed.insert(moved);
        app.commit("Arranged by hand");
    }
    ui.settle();

    ui.go_to("Room layout");
    ui.click("×");
    let option = ui.option();
    assert_eq!(option.spot_of(pinned), Spot::Unseated);
    assert!(matches!(option.spot_of(loose), Spot::Loose(_)));
    assert_eq!(option.spot_of(moved), Spot::Unseated);
    assert!(
        !option.hand_placed.contains(&moved),
        "nobody keeps a seat they lost"
    );
    assert!(!ui.app().project.pins().contains_key(&pinned));
    ui.app().project.validate().unwrap();

    // Undo brings the table, the pin and the seats back.
    ui.key(Modifiers::COMMAND, Key::Z);
    assert_eq!(ui.option().spot_of(pinned), Spot::Seat(seat(0)));
    assert!(ui.app().project.pins().contains_key(&pinned));
}

#[test]
fn removing_every_table_leaves_everyone_not_seated() {
    let mut ui = solved();
    ui.go_to("Room layout");
    while ui.has("×") {
        ui.click("×");
    }
    let app = ui.app();
    assert!(app.project.layout.seats().is_empty());
    assert_eq!(app.project.candidates.len(), 1);
    assert!(ui.option().assignment.is_empty());
    app.project.validate().unwrap();
    ui.go_to("Seating chart");
}
