//! Behavior retained by virtual attendee rows and background print completion.

use super::harness::Ui;
use crate::print::PrintKind;

#[test]
fn virtual_attendee_list_keeps_bulk_selection_and_search_complete() {
    let mut ui = Ui::with_example();
    ui.go_to("People & wishes");
    assert_eq!(ui.app().people.shown.len(), 48);
    ui.click("Select all shown");
    assert_eq!(ui.app().people.picked.len(), 48);
    ui.click("Clear selection");
    ui.app_mut().people.search = "gUeSt 48".into();
    ui.settle();
    let expected = ui.app().project.attendees[47].id;
    assert_eq!(ui.app().people.shown, [expected]);
    ui.click("Guest 48");
    assert_eq!(ui.app().selected_person, Some(expected));
    ui.app_mut().people.search.clear();
    ui.settle();
    assert_eq!(ui.app().people.shown.len(), 48);
}

#[test]
fn completed_print_uses_its_original_project_destination() {
    let mut ui = Ui::with_example();
    let kind = PrintKind::Chart;
    let name = crate::print::file_name(&ui.app().project, &ui.app().file.name, kind);
    let original_folder = crate::platform::save_folder(&ui.app().file.store).unwrap();
    let target = ui.app().print_target(&[kind]);
    ui.app_mut()
        .create_project("<project>", crate::model::Project::new());
    let other_folder = crate::platform::save_folder(&ui.app().file.store).unwrap();
    let status = ui.app().status.clone();
    let bytes = b"%PDF-1.7\n<snapshot>\n".to_vec();
    ui.app_mut()
        .finish_print(target, true, vec![(kind, Ok(bytes.clone()))]);
    assert_eq!(std::fs::read(original_folder.join(&name)).unwrap(), bytes);
    assert!(!other_folder.join(&name).exists());
    assert!(ui.app().printing.saved.is_empty());
    assert_eq!(ui.app().status, status);
}

#[test]
fn completed_print_does_not_mark_another_option_or_revision_fresh() {
    let mut ui = Ui::with_example();
    ui.click("Seat by hand");
    let target = ui.app().print_target(&[PrintKind::Chart]);
    let other = ui.option().clone();
    ui.app_mut().project.candidates.push(other);
    ui.app_mut().selected_candidate = 1;
    let status = ui.app().status.clone();
    ui.app_mut().finish_print(
        target,
        true,
        vec![(PrintKind::Chart, Ok(b"%PDF-1.7\n".to_vec()))],
    );
    assert!(ui.app().printing.saved.is_empty());
    assert_eq!(ui.app().status, status);

    let target = ui.app().print_target(&[PrintKind::Chart]);
    let revision = ui.app().file.revision;
    ui.app_mut().commit("Saved changes");
    let status = ui.app().status.clone();
    ui.app_mut().finish_print(
        target,
        true,
        vec![(PrintKind::Chart, Ok(b"%PDF-1.7\n".to_vec()))],
    );
    let (_, saved_revision, option) = &ui.app().printing.saved[&PrintKind::Chart];
    assert_eq!(*saved_revision, revision);
    assert_eq!(*option, 1);
    assert_ne!(*saved_revision, ui.app().file.revision);
    assert_eq!(ui.app().status, status);
}
