//! The flows, one test each.

use eframe::egui::{Key, Modifiers, PointerButton};

use super::harness::Ui;

#[test]
fn empty_project_offers_example() {
    let mut ui = Ui::open();
    assert!(ui.has("BEFORE THE FIRST CHART"), "{:?}", ui.dump());
    assert!(!ui.has("Sign-up notes: none to review"), "{:?}", ui.dump());
    ui.click("Try with example data");
    assert_eq!(ui.app().project.attendees.len(), 48);
    assert!(!ui.app().project.layout.seats().is_empty());
    assert_eq!(ui.app().file.name, "Example event");
    ui.open_project_menu();
    // The switcher button and its entry in the list.
    assert!(ui.count("Example event") >= 2, "{:?}", ui.dump());
    assert!(ui.has("New event"));
}

#[test]
fn solve_shows_option_score_and_tour_once() {
    let mut ui = Ui::with_example();
    ui.solve();
    let options = &ui.app().project.candidates;
    assert_eq!(options.len(), 1);
    let chip = format!("{}  ", options[0].name);
    assert!(
        ui.dump()
            .iter()
            .any(|t| t.starts_with(&chip) && t.contains(" met")),
        "{:?}",
        ui.dump()
    );
    assert!(ui.has("SCORE"), "{:?}", ui.dump());
    for step in 1..=5 {
        assert!(
            ui.has(&format!("Step {step} of 5")),
            "step {step}: {:?}",
            ui.dump()
        );
        ui.click(if step == 5 { "Done" } else { "Next" });
    }
    assert!(!ui.has_text("Step "));
    let ui = ui.reopen();
    assert!(!ui.app().project.candidates.is_empty());
    assert!(!ui.has_text("Step "), "the tour shows only once");
}

#[test]
fn chart_search_pin_select_and_drag() {
    let mut ui = Ui::with_example();
    ui.solve();
    ui.key(Modifiers::NONE, Key::Escape); // ends the tour

    // Ctrl+F, type, Enter selects the match.
    ui.key(Modifiers::COMMAND, Key::F);
    ui.type_text("Guest 07");
    assert!(ui.has_text("Guest 07 · "), "{:?}", ui.dump());
    ui.key(Modifiers::NONE, Key::Enter);
    let guest = ui
        .app()
        .project
        .attendees
        .iter()
        .find(|a| a.name == "Guest 07")
        .unwrap()
        .id;
    assert_eq!(ui.app().selected_person, Some(guest));

    // P pins the selected person to their seat.
    ui.key(Modifiers::NONE, Key::P);
    assert!(ui.app().project.pinned_people().contains(&guest));

    // F fits the whole room, so every card can be reached.
    ui.key(Modifiers::NONE, Key::F);
    let [other, third] = ui.two_unpinned();

    // Right-click menu: add to the selection.
    let card = ui.card_of(other).center();
    ui.click_at(card, PointerButton::Secondary, Modifiers::NONE);
    ui.click("Add to selection");
    assert!(ui.app().chart.multi.contains(&other));

    // Dragging a card onto another swaps them; Undo restores.
    let (a, b) = (other, third);
    let (seat_a, seat_b) = (ui.seat_of(a), ui.seat_of(b));
    let (from, to) = (ui.card_of(a).center(), ui.card_of(b).center());
    ui.drag(from, to);
    assert_eq!(
        (ui.seat_of(a), ui.seat_of(b)),
        (seat_b.clone(), seat_a.clone())
    );
    ui.key(Modifiers::COMMAND, Key::Z);
    assert_eq!((ui.seat_of(a), ui.seat_of(b)), (seat_a, seat_b));
}

#[test]
fn chart_group_selection_includes_the_existing_selection() {
    let mut ui = Ui::with_example();
    ui.solve();
    ui.key(Modifiers::NONE, Key::Escape);
    let ids: std::collections::BTreeMap<_, _> = ui
        .app()
        .project
        .attendees
        .iter()
        .map(|a| (a.name.clone(), a.id))
        .collect();
    let guest30 = ids["Guest 30"];
    ui.node("Guest 30").click();
    ui.settle();
    assert_eq!(
        ui.app().selected_person,
        Some(guest30),
        "first pick: {:?}",
        ui.dump()
    );
    for name in ["Guest 44", "Guest 23", "Guest 36"] {
        ui.node(name).click_modifiers(Modifiers::COMMAND);
        ui.settle();
        assert!(
            ui.app().chart.multi.contains(&guest30),
            "after {name}: {:?}",
            ui.dump()
        );
    }
    let group = &ui.app().chart.multi;
    assert_eq!(group.len(), 4);
    for id in [guest30, ids["Guest 44"], ids["Guest 23"], ids["Guest 36"]] {
        assert!(group.contains(&id));
    }
}

#[test]
fn project_menu_new_rename_duplicate_delete_restore() {
    let mut ui = Ui::with_example();
    ui.open_project_menu();
    ui.click("New project");
    assert_eq!(ui.app().file.name, "New event 2");
    assert!(ui.app().project.attendees.is_empty());

    ui.open_project_menu();
    ui.click("Duplicate this project");
    assert_eq!(ui.app().file.name, "New event 2 copy");

    ui.open_project_menu();
    ui.click("Example event");
    assert_eq!(ui.app().file.name, "Example event");
    assert_eq!(ui.app().project.attendees.len(), 48);
    let names = ui.app().file.library.projects();
    assert_eq!(
        names,
        [
            "Example event",
            "New event",
            "New event 2",
            "New event 2 copy"
        ]
    );

    ui.open_project_menu();
    ui.click("Delete this project…");
    assert!(ui.has_text("Delete Example event?"), "{:?}", ui.dump());
    ui.click("Delete");
    assert!(
        !ui.app()
            .file
            .library
            .projects()
            .contains(&"Example event".to_string())
    );
    assert_ne!(ui.app().file.name, "Example event");

    ui.open_project_menu();
    ui.click("Deleted projects…");
    assert!(ui.has("Example event"), "{:?}", ui.dump());
    ui.click("Restore");
    assert_eq!(ui.app().file.name, "Example event");
    assert_eq!(ui.app().project.attendees.len(), 48);
}

#[test]
fn project_can_be_renamed_from_the_project_menu() {
    let mut ui = Ui::with_example();
    ui.open_project_menu();
    ui.click("Rename project");
    ui.key(Modifiers::COMMAND, Key::A);
    ui.type_text("Banquet");
    ui.key(Modifiers::NONE, Key::Enter);
    assert_eq!(ui.app().file.name, "Banquet");
}

#[test]
fn earlier_version_restores_and_undo_returns() {
    let mut ui = Ui::with_example();
    // A backup of the example as loaded; the five-minute timer is not waited for.
    ui.app_mut().backup_now();
    ui.solve();
    ui.key(Modifiers::NONE, Key::Escape);
    assert_eq!(ui.app().project.candidates.len(), 1);

    ui.open_project_menu();
    ui.click("Earlier versions…");
    assert!(ui.has("48 people, 0 options"), "{:?}", ui.dump());
    let restore = ui.dump().iter().filter(|t| *t == "Restore").count();
    assert!(restore >= 1);
    // Newest first; every backup here is of the example without options.
    ui.click("Restore");
    assert!(ui.app().project.candidates.is_empty());
    assert!(!ui.has("Earlier versions"), "the window closes");

    ui.key(Modifiers::COMMAND, Key::Z);
    assert_eq!(ui.app().project.candidates.len(), 1);
}

#[test]
fn language_switches_to_finnish_and_back() {
    let mut ui = Ui::with_example();
    assert!(ui.has("Seating chart"));
    ui.click("EN");
    assert!(ui.has("Istumajärjestys"), "{:?}", ui.dump());
    assert_eq!(
        ui.app().file.library.setting("language").as_deref(),
        Some("fi")
    );
    ui.click("FI");
    assert!(ui.has("Seating chart"));
    assert!(!ui.has("Istumajärjestys"));
}

#[test]
fn print_page_saves_four_pdfs() {
    let mut ui = Ui::with_example();
    ui.solve();
    ui.key(Modifiers::NONE, Key::Escape);
    ui.go_to("Print");
    assert!(
        ui.has_text("Ready: everyone seated")
            || ui.has_text("hard rules broken")
            || ui.has_text("empty seats"),
        "{:?}",
        ui.dump()
    );
    ui.click("Save all four PDFs");
    ui.wait_until(60, |app| app.printing.job.is_none());
    let folder = ui.dir.path().join("projects");
    let pdfs = std::fs::read_dir(&folder)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "pdf")
        })
        .count();
    assert_eq!(pdfs, 4, "{}", ui.app().status);
}

#[test]
fn people_filters_and_bulk_group() {
    let mut ui = Ui::with_example();
    ui.go_to("People & wishes");
    assert!(ui.has("All 48"), "{:?}", ui.dump());

    ui.click("No wishes");
    let active = ui.app().project.active_people();
    let shown = ui.app().people.shown.clone();
    assert!(!shown.is_empty() && shown.len() < 48);
    assert!(shown.iter().all(|id| !active.contains(id)));
    ui.click("All 48");
    assert_eq!(ui.app().people.shown.len(), 48);

    let preferences = ui.app().project.preferences.len();
    for name in ["Guest 01", "Guest 02"] {
        ui.node(name).click_modifiers(Modifiers::COMMAND);
        ui.settle();
    }
    assert!(ui.has("2 picked"), "{:?}", ui.dump());
    ui.click("Group together");
    let project = &ui.app().project;
    assert_eq!(project.preferences.len(), preferences + 1);
    assert!(
        ui.app().status.contains("Grouped 2 people together"),
        "{}",
        ui.app().status
    );
}

#[test]
fn invalid_late_change_keeps_the_form_and_entered_details() {
    let mut ui = Ui::with_example();
    ui.go_to("People & wishes");
    ui.app_mut().people.roster = Some(crate::app::people::RosterEdit::Add(
        crate::roster::NewAttendee {
            name: "Guest 01".into(),
            food: "Vegetarian".into(),
            ..Default::default()
        },
    ));
    ui.settle();
    ui.click("Add");
    let Some(crate::app::people::RosterEdit::Add(person)) = &ui.app().people.roster else {
        panic!("the invalid add form should stay open");
    };
    assert_eq!(person.name, "Guest 01");
    assert_eq!(person.food, "Vegetarian");
    assert!(ui.app().status.contains("already on the list"));
}

#[test]
fn opening_advanced_room_editor_does_not_create_empty_seat_tags() {
    let mut ui = Ui::with_example();
    ui.go_to("Room layout");
    ui.click("Advanced: seat tags and seat relationships");
    ui.settle();

    assert!(ui.app().project.layout.seat_tags.is_empty());
}

#[test]
fn people_import_from_pasted_sheet() {
    let mut ui = Ui::open();
    ui.go_to("People & wishes");
    let copied = ui.click_and_copied("Copy header row");
    assert_eq!(copied, Some(crate::import::header_row()));

    ui.click_text_box(); // hinted "Paste the rows here, header row included…"
    ui.type_text("Full name\tOrganization\nX Y\tABC\nZ W\tXYZ\n");
    assert!(ui.has("Template column"), "{:?}", ui.dump());
    assert!(
        ui.has("Not in the template: choose what it is"),
        "{:?}",
        ui.dump()
    );

    // No guessing: say which column holds the names.
    ui.open_combo("Kept as extra info");
    ui.choose("Name");

    // Leave the organization out.
    ui.open_combo("Organization");
    ui.choose("Ignore");
    assert!(ui.has("Set by you"), "{:?}", ui.dump());
    ui.h.run_steps(3);
    assert!(ui.has("Set by you"), "stays marked after the change");

    ui.click("Update attendees");
    let project = &ui.app().project;
    let names: Vec<&str> = project.attendees.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["X Y", "Z W"]);
    assert!(
        project
            .attendees
            .iter()
            .all(|a| project.affiliation(a).is_empty()),
        "organization ignored"
    );
    assert!(ui.has("X Y") && ui.has("Z W"), "{:?}", ui.dump());
}
