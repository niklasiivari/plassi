//! Repeated sheet imports, independent mappings, and explicit name matching.

use eframe::egui::{Key, Modifiers};

use super::harness::Ui;

#[test]
fn fifty_people_then_twenty_more_with_different_columns_and_undo() {
    let mut ui = Ui::open();
    ui.go_to("People & wishes");
    ui.click_text_box();
    let first = (1..=50)
        .map(|i| format!("Guest {i:02}\tABC\n"))
        .collect::<String>();
    ui.type_text(&format!("Name\tOrganization\n{first}"));
    ui.click("Import");
    let original = serde_json::to_value(&ui.app().project.attendees).unwrap();
    ui.app_mut().people.search = "Guest 01".into();
    ui.settle();
    ui.click("Guest 01");
    ui.click("Import people");
    assert!(ui.has("Choose file…"));
    ui.click_text_box();
    let second = (51..=70)
        .map(|i| format!("Guest {i:02}\tXYZ\n"))
        .collect::<String>();
    ui.type_text(&format!("Attendee\tClub\n{second}"));
    ui.open_combo("Kept as extra info");
    ui.choose("Name");
    ui.open_combo("Kept as extra info");
    ui.choose("Organization");
    assert!(ui.has_text("To add: 20 · to update: 0"));
    assert!(ui.has_text("People after import: 70"));
    ui.click("Import");
    let people = &ui.app().project.attendees;
    assert_eq!(people.len(), 70);
    assert_eq!(serde_json::to_value(&people[..50]).unwrap(), original);
    assert!(
        people[50..]
            .iter()
            .all(|person| person.organization == "XYZ")
    );
    assert_eq!(
        ui.app().people.shown.len(),
        70,
        "the new batch is not hidden by an old filter"
    );
    ui.key(Modifiers::COMMAND, Key::Z);
    assert_eq!(ui.app().project.attendees.len(), 50);
    ui.key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    assert_eq!(ui.app().project.attendees.len(), 70);
}

#[test]
fn selected_excel_worksheets_can_be_mapped_separately() {
    let mut ui = Ui::open();
    ui.go_to("People & wishes");
    ui.app_mut().load_sheet_file(
        "import.xlsx",
        include_bytes!("../../../tests/fixtures/import.xlsx"),
    );
    ui.settle();
    assert!(ui.has("Worksheet"));
    ui.open_combo("Sheet 1");
    ui.choose("Sheet 2");
    ui.open_combo("Kept as extra info");
    ui.choose("Name");
    ui.open_combo("Kept as extra info");
    ui.choose("Food");
    ui.click("Import");
    assert_eq!(ui.app().project.attendees.len(), 1);
    assert_eq!(ui.app().project.attendees[0].name, "Z");
    assert_eq!(ui.app().project.attendees[0].food, "Vegetarian");
}

#[test]
fn another_file_can_add_same_named_people_or_explicitly_update_them() {
    let mut ui = Ui::open();
    ui.go_to("People & wishes");
    ui.app_mut()
        .load_sheet_file("first.csv", b"Name,Organization\nX,ABC");
    ui.settle();
    ui.click("Import");
    ui.app_mut()
        .load_sheet_file("second.csv", b"Name,Organization\nX,XYZ\nY,XYZ");
    ui.settle();
    assert!(ui.has_text("To add: 2 · to update: 0"));
    ui.click("Update matching names");
    assert!(ui.has_text("To add: 1 · to update: 1"));
    ui.click("Import");
    assert_eq!(ui.app().project.attendees.len(), 2);
    assert_eq!(ui.app().project.attendees[0].organization, "XYZ");
    // Starting a new file resets the choice to adding separate attendees.
    ui.app_mut()
        .load_sheet_file("third.csv", b"Name,Organization\nX,ABC");
    ui.settle();
    assert!(ui.has_text("To add: 1 · to update: 0"));
    ui.click("Import");
    assert_eq!(ui.app().project.attendees.len(), 3);
    assert_eq!(ui.app().project.attendees[0].organization, "XYZ");
}

#[test]
fn switching_projects_discards_the_previous_import_draft() {
    let mut ui = Ui::with_example();
    ui.go_to("People & wishes");
    ui.app_mut().load_sheet_file("import.csv", b"Name\nX");
    ui.settle();
    assert!(!ui.app().people.import_text.is_empty());
    ui.app_mut()
        .create_project("Project 2", crate::model::Project::new());
    ui.settle();
    assert!(ui.app().people.import_text.is_empty());
    assert!(ui.app().people.import_sheets.is_empty());
    assert!(!ui.app().people.import_open);
    assert!(ui.app().project.attendees.is_empty());
}
