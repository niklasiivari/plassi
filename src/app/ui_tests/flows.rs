//! The flows, one test each.

use eframe::egui::{Key, Modifiers, PointerButton};
use egui_kittest::kittest::{NodeT, Queryable};

use super::harness::Ui;

#[test]
fn empty_project_offers_example() {
    let mut ui = Ui::open();
    assert!(ui.has("GET STARTED"), "{:?}", ui.dump());
    assert!(!ui.has("No notes"), "{:?}", ui.dump());
    ui.click("Try example");
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

    // Ctrl+K, type, Enter selects the match.
    ui.key(Modifiers::COMMAND, Key::K);
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
    assert!(ui.app().project.seat_pinned_people().contains(&guest));

    // F fits the whole room, so every card can be reached.
    ui.key(Modifiers::NONE, Key::F);
    let [other, third] = ui.two_unpinned();

    // Right-click menu: add to the selection.
    let card = ui.card_of(other).center();
    ui.click_at(card, PointerButton::Secondary, Modifiers::NONE);
    ui.click("Add to selection");
    assert!(ui.app().chart.multi.contains(&other));

    // With the selection cleared, dragging a card onto another swaps the
    // two; Undo restores.
    ui.key(Modifiers::NONE, Key::Escape);
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
    ui.click("Duplicate project");
    assert_eq!(ui.app().file.name, "New event 2 (copy)");

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
            "New event 2 (copy)"
        ]
    );

    ui.open_project_menu();
    ui.click("Delete project…");
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
fn print_page_exports_excel_without_running_pdf_jobs() {
    let mut ui = Ui::with_example();
    let option = ui.app().project.empty_option("Export option".into());
    ui.app_mut().project.candidates.push(option);
    ui.go_to("Print");
    ui.click("Export Excel (.xlsx)");
    let name = ui.app().file.name.clone();
    let file = crate::export::file_name(&ui.app().project, &name);
    let bytes = std::fs::read(ui.dir.path().join("projects").join(name).join(file)).unwrap();
    assert!(bytes.starts_with(b"PK"), "{}", ui.app().status);
    assert!(ui.app().printing.job.is_none());
    assert!(ui.app().printing.spreadsheet.is_some());
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
    ui.click("Save all PDFs");
    ui.wait_until(60, |app| app.printing.job.is_none());
    // In the project's own folder, named after the project.
    let name = ui.app().file.name.clone();
    let folder = ui.dir.path().join("projects").join(&name);
    let pdfs: Vec<String> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|f| f.ends_with(".pdf"))
        .collect();
    assert_eq!(pdfs.len(), 4, "{}", ui.app().status);
    assert!(
        pdfs.iter().all(|f| f.starts_with(&format!("{name} - "))),
        "{pdfs:?}"
    );
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
    assert!(ui.has("2 selected"), "{:?}", ui.dump());
    ui.click("Group together");
    let project = &ui.app().project;
    assert_eq!(project.preferences.len(), preferences + 1);
    assert!(
        ui.app().status.contains("Grouped 2 people"),
        "{}",
        ui.app().status
    );
}

#[test]
fn wish_checklist_keeps_selections_when_searching_and_saves_all_people() {
    use crate::model::{RuleKind, Target};
    use eframe::egui::accesskit::Role;

    let mut ui = Ui::with_example();
    ui.go_to("People & wishes");
    ui.click("Guest 01");
    ui.click("+ Add");
    ui.click("Sit near…");
    let before = ui.app().project.rules.len();
    assert!(
        ui.h.query_by_role_and_label(Role::CheckBox, "Guest 01")
            .is_none()
    );

    ui.h.get_by_role_and_label(Role::TextInput, "Search people")
        .click();
    ui.settle();
    ui.type_text("gUeSt 0");
    for name in ["Guest 02", "Guest 03"] {
        ui.h.get_by_role_and_label(Role::CheckBox, name).click();
        ui.settle();
    }
    assert_eq!(
        ui.app().people.wish_draft.as_ref().unwrap().targets.len(),
        2
    );
    assert_eq!(ui.app().people.draft_picker_filter, "gUeSt 0");
    ui.click("Guest 02  ×");
    assert_eq!(
        ui.app().people.wish_draft.as_ref().unwrap().targets.len(),
        1
    );

    ui.app_mut().people.draft_picker_filter = "nobody matches".into();
    ui.settle();
    assert!(ui.has("No matching people"));
    assert!(ui.has("Guest 03  ×"));
    ui.click("Guest 03  ×");
    assert!(
        ui.app()
            .people
            .wish_draft
            .as_ref()
            .unwrap()
            .targets
            .is_empty()
    );

    ui.app_mut().people.draft_picker_filter = "Guest 0".into();
    ui.settle();
    for name in ["Guest 02", "Guest 03"] {
        ui.h.get_by_role_and_label(Role::CheckBox, name).click();
        ui.settle();
    }
    ui.click("Add wish");
    assert_eq!(ui.app().project.rules.len(), before + 1);
    let rule = ui.app().project.rules.last().unwrap();
    let RuleKind::Proximity {
        target: Target::People { ids },
        wish,
        ..
    } = &rule.kind
    else {
        panic!("a seating wish for people");
    };
    assert_eq!(ids.len(), 2);
    assert_eq!(*wish, crate::model::WishKind::Seating);
    let id = rule.id;
    let saved = serde_json::to_value(rule).unwrap();
    let ui = ui.reopen();
    let restored = ui.app().project.rules.iter().find(|r| r.id == id).unwrap();
    assert_eq!(serde_json::to_value(restored).unwrap(), saved);
}

#[test]
fn existing_wish_checklist_can_change_people_but_cannot_empty_a_rule() {
    use crate::model::{RuleKind, Target};
    use eframe::egui::accesskit::Role;

    let mut ui = Ui::with_example();
    ui.go_to("People & wishes");
    ui.click("Guest 01");
    ui.choose("Edit");
    let id = ui.app().people.editing_rule.unwrap();
    let rule = ui.app().project.rules.iter().find(|r| r.id == id).unwrap();
    let RuleKind::Proximity {
        target: Target::People { ids },
        ..
    } = &rule.kind
    else {
        panic!("a wish for people");
    };
    assert_eq!(ids.len(), 1);
    let original = ui.app().project.person_name(ids[0]).to_string();
    assert!(
        ui.h.get_by_role_and_label(Role::CheckBox, &original)
            .accesskit_node()
            .is_disabled()
    );
    ui.h.get_by_role_and_label(Role::CheckBox, "Guest 03")
        .click();
    ui.settle();
    ui.h.get_by_role_and_label(Role::CheckBox, &original)
        .click();
    ui.settle();
    let rule = ui.app().project.rules.iter().find(|r| r.id == id).unwrap();
    let RuleKind::Proximity {
        target: Target::People { ids },
        ..
    } = &rule.kind
    else {
        panic!("a wish for people");
    };
    assert_eq!(ids.len(), 1);
    assert_eq!(ui.app().project.person_name(ids[0]), "Guest 03");
    assert!(
        ui.h.get_by_role_and_label(Role::CheckBox, "Guest 03")
            .accesskit_node()
            .is_disabled()
    );
    ui.app()
        .project
        .validate()
        .expect("edited project stays valid");
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
fn people_import_from_pasted_sheet() {
    let mut ui = Ui::open();
    ui.go_to("People & wishes");
    let copied = ui.click_and_copied("Copy header row");
    assert_eq!(copied, Some(crate::import::header_row()));

    ui.click_text_box(); // hinted "Paste rows with column headers…"
    ui.type_text("Full name\tOrganization\nX Y\tABC\nZ W\tXYZ\n");
    assert!(ui.has("Template column"), "{:?}", ui.dump());
    assert!(ui.has("Review mapping"), "{:?}", ui.dump());

    // No guessing: say which column holds the names.
    ui.open_combo("Kept as extra info");
    ui.choose("Name");

    // Leave the organization out.
    ui.open_combo("Organization");
    ui.choose("Ignore");
    assert!(ui.has("Selected"), "{:?}", ui.dump());
    ui.h.run_steps(3);
    assert!(ui.has("Selected"), "stays marked after the change");

    ui.click("Import");
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

#[test]
fn reopens_on_the_option_last_shown() {
    let mut ui = Ui::with_example();
    ui.solve();
    ui.solve();
    assert_eq!(ui.app().project.candidates.len(), 2);
    ui.app_mut().selected_candidate = 0;
    ui.settle();
    let ui = ui.reopen();
    assert_eq!(ui.app().selected_candidate, 0);
    let mut ui = ui;
    let undo_steps = ui.app().file.undo_stack.len();
    ui.app_mut().selected_candidate = 1;
    ui.settle();
    // Kept in the project file, but choosing an option is not an edit.
    assert_eq!(ui.app().project.shown_option, 1);
    assert_eq!(ui.app().file.undo_stack.len(), undo_steps);
    let ui = ui.reopen();
    assert_eq!(ui.app().selected_candidate, 1);
}

#[test]
fn adding_a_person_offers_existing_categories() {
    let mut ui = Ui::with_example();
    ui.go_to("People & wishes");
    ui.click("+ Add person");
    // The first dropdown beside a field is the organization's.
    ui.open_combo("");
    ui.choose("Org B");
    let Some(crate::app::people::RosterEdit::Add(person)) = ui.app().people.roster.as_ref() else {
        panic!("the add form");
    };
    assert_eq!(person.organization, "Org B");
}
