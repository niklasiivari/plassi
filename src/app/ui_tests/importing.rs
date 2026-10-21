//! Repeated sheet imports, independent mappings, and explicit name matching.

use eframe::egui::{self, Key, Modifiers};
use egui_kittest::kittest::Queryable;

use super::harness::Ui;

#[test]
fn dropdown_typing_filters_and_keyboard_selects_without_changing_on_no_match() {
    let mut ui = Ui::open();
    ui.go_to("People & wishes");
    ui.app_mut()
        .load_sheet_file("guests.tsv", b"Name\tMeal notes\nExample Guest\tAvoid nuts");
    ui.settle();
    ui.open_combo("Kept as extra info");
    // The search field gets focus as soon as the dropdown opens.
    ui.type_text("nothing matches this");
    assert!(ui.has_text("No matching options"));
    ui.key(Modifiers::NONE, Key::Enter);
    assert!(ui.has_text("No matching options"));
    ui.key(Modifiers::COMMAND, Key::A);
    ui.type_text("NAME");
    ui.key(Modifiers::NONE, Key::ArrowDown);
    ui.key(Modifiers::NONE, Key::Enter);
    assert!(ui.has("First name"), "{:?}", ui.dump());
    // Reopening resets the filter, then another query selects the intended field.
    ui.open_combo("First name");
    assert!(!ui.has_text("No matching options"));
    ui.type_text("ALLERG");
    ui.key(Modifiers::NONE, Key::Enter);
    ui.click("Import");
    assert_eq!(ui.app().project.attendees[0].allergies, "Avoid nuts");
}

#[test]
fn long_import_headers_and_samples_keep_mapping_controls_inside_the_panel() {
    let mut ui = Ui::with_example();
    ui.go_to("People & wishes");
    let header = "Tell us about all dietary requirements and allergies that the kitchen should know before preparing your meal";
    let unbroken = "VeryLongSpreadsheetColumnWithoutAnySpaces".repeat(8);
    let sample =
        "Please prepare a separate meal and avoid any traces of the following ingredients. "
            .repeat(5);
    let sheet = format!("Name\t{header}\t{unbroken}\nExample guest\t{sample}\t{sample}");
    ui.app_mut()
        .load_sheet_file("long-headers.tsv", sheet.as_bytes());
    for width in [1440.0, 1180.0, 1024.0, 820.0, 600.0, 390.0, 320.0] {
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        ui.h.run_steps(4);
        for node in
            ui.h.query_all_by(|n| n.role() == egui::accesskit::Role::ComboBox)
        {
            let rect = node.rect();
            assert!(
                rect.left() >= 0.0 && rect.right() <= width,
                "mapping escapes at {width}: {rect:?}"
            );
            assert!(
                rect.width() >= 150.0,
                "mapping squeezed at {width}: {rect:?}"
            );
        }
    }
    ui.open_combo("Kept as extra info");
    ui.choose("Allergies / diet");
    ui.click("Import");
    let guest = ui.app().project.attendees.last().unwrap();
    assert_eq!(guest.name, "Example guest");
    assert_eq!(guest.allergies, sample.trim());
    assert_eq!(guest.extra[&unbroken], sample.trim());
}

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

#[test]
fn long_pasted_sheet_keeps_mapping_nearby_and_extra_info_can_be_confirmed() {
    let mut ui = Ui::open();
    ui.go_to("People & wishes");
    let rows = (1..=1000)
        .map(|i| format!("Guest {i:04}\tAdditional details\n"))
        .collect::<String>();
    ui.app_mut()
        .load_sheet_file("import.tsv", format!("Name\tNotes\n{rows}").as_bytes());
    for width in [1440.0, 390.0] {
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        ui.h.run_steps(4);
        let mapping =
            ui.h.query_all_by(|node| {
                node.role() == egui::accesskit::Role::ComboBox
                    && node.value().as_deref() == Some("Name")
            })
            .next()
            .expect("name mapping")
            .rect();
        assert!(
            mapping.top() < 800.0,
            "mapping too far below source at {width}: {mapping:?}"
        );
    }
    ui.open_combo("Kept as extra info");
    ui.choose("Kept as extra info");
    assert!(
        !ui.has_text("Review mapping"),
        "explicit extra-info choice must be confirmed"
    );
    assert!(ui.has_text("Selected"));
    ui.click("Import");
    assert_eq!(ui.app().project.attendees.len(), 1000);
    assert_eq!(
        ui.app().project.attendees[0].extra["Notes"],
        "Additional details"
    );
}
