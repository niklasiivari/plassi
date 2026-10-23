//! Regression coverage for pending Print edits and destructive project actions.

use eframe::egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::Queryable;

use super::harness::Ui;

fn edit_title(ui: &mut Ui, value: &str) {
    ui.go_to("Print");
    ui.h.query_all_by(|node| node.role() == Role::TextInput)
        .next()
        .expect("event title")
        .click();
    ui.settle();
    ui.key(Modifiers::COMMAND, Key::A);
    ui.type_text(value);
}

#[test]
fn print_text_commits_once_before_navigation_and_survives_reopen() {
    let mut ui = Ui::with_example();
    let history = ui.app().file.undo_stack.len();
    edit_title(&mut ui, "<event>");
    assert!(ui.app().printing.details_dirty);
    assert_eq!(ui.app().file.undo_stack.len(), history);
    ui.go_to("Connections");
    assert!(!ui.app().printing.details_dirty);
    assert_eq!(ui.app().file.undo_stack.len(), history + 1);
    assert_eq!(ui.app().project.event.title, "<event>");
    let reopened = ui.reopen();
    assert_eq!(reopened.app().project.event.title, "<event>");
}

#[test]
fn changing_projects_saves_pending_print_text_to_its_original_project() {
    let mut ui = Ui::with_example();
    let original = ui.app().file.store.clone();
    edit_title(&mut ui, "<event>");
    ui.app_mut()
        .create_project("<project>", crate::model::Project::new());
    let saved = crate::io::parse(&original.read().unwrap()).unwrap();
    assert_eq!(saved.event.title, "<event>");
    assert!(ui.app().project.event.title.is_empty());
    assert!(!ui.app().printing.details_dirty);
}

#[test]
fn deletion_keeps_the_project_when_its_recovery_copy_fails() {
    let mut ui = Ui::with_example();
    let name = ui.app().file.name.clone();
    let store = ui.app().file.store.clone();
    let before = store.read().unwrap();
    let blocked = crate::platform::save_folder(&store)
        .unwrap()
        .join("backups");
    if blocked.is_dir() {
        std::fs::remove_dir_all(&blocked).unwrap();
    }
    std::fs::write(&blocked, b"<blocked>").unwrap();

    ui.app_mut().delete_project();

    assert_eq!(ui.app().file.name, name);
    assert_eq!(store.read().unwrap(), before);
    assert!(ui.app().file.library.projects().contains(&name));
    assert!(ui.app().status.starts_with("Could not delete"));
    assert!(ui.app().status.contains("backup could not be saved"));
}

#[test]
fn edit_wishes_closes_the_import_panel_for_the_selected_person() {
    for mode in ["Import people", "+ Add person"] {
        let mut ui = Ui::with_example();
        ui.go_to("People & wishes");
        ui.click(mode);
        ui.go_to("Connections");
        let person = ui.app().project.attendees[0].id;
        ui.app_mut().selected_person = Some(person);
        ui.settle();
        ui.click("Edit wishes");
        assert!(!ui.app().people.import_open);
        assert!(ui.app().people.roster.is_none());
        assert_eq!(ui.app().selected_person, Some(person));
        assert!(!ui.has("Choose file…"));
    }
}
