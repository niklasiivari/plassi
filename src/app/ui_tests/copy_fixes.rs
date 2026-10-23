//! Copy must describe the scope and effect of the action shown beside it.
use super::harness::Ui;
use crate::app::Page;
use crate::catalog::Kind;
use crate::i18n::{self, Lang, tr};
use crate::model::{ImportField, ImportNote, SittingAlone};

#[test]
fn policy_choices_describe_preferences_in_both_languages() {
    let mut ui = Ui::with_example();
    ui.app_mut().page = Page::Policy;
    for language in [Lang::En, Lang::Fi] {
        i18n::set(language);
        ui.settle();
        assert!(ui.has(tr("Strongly avoid")));
        assert!(ui.has(tr("Prefer groups")));
        ui.click(tr("Strongly avoid"));
        assert_eq!(ui.app().project.policy.sitting_alone, SittingAlone::Never);
        assert!(ui.has(tr(
            "Give high priority to having someone beside or opposite each person."
        )));
    }
}

#[test]
fn ignored_entries_have_a_distinct_finnish_action_from_reviewed_notes() {
    let mut ui = Ui::with_example();
    let mut project = crate::model::Project::new();
    project.attendees = crate::example::project()
        .attendees
        .into_iter()
        .take(1)
        .collect();
    project.attendees[0].organization = "Ignored spelling".into();
    project
        .catalog
        .ignore(Kind::Organization, "Ignored spelling", true);
    ui.app_mut().replace_project(project);
    ui.app_mut().page = Page::Categories;
    i18n::set(Lang::Fi);
    ui.settle();
    assert!(ui.has(tr("Unignore entry")));
    assert!(!ui.has(tr("Reopen")));
    ui.click(tr("Unignore entry"));
    assert!(
        !ui.app()
            .project
            .catalog
            .is_ignored(Kind::Organization, "Ignored spelling")
    );
}

#[test]
fn review_instruction_covers_both_notes_marked_by_the_primary_action() {
    let mut ui = Ui::with_example();
    let person = ui.app().project.attendees[0].id;
    ui.app_mut().project.notes = [ImportField::Avec, ImportField::Wish]
        .into_iter()
        .map(|field| ImportNote {
            person,
            field,
            text: "A separate sign-up note".into(),
            reviewed: false,
        })
        .collect();
    ui.app_mut().commit("Saved changes");
    ui.app_mut().page = Page::People;
    i18n::set(Lang::Fi);
    ui.settle();
    ui.click(tr("Review notes"));
    assert!(ui.has(tr("Marks all notes for this person reviewed.")));
    ui.click(tr("Mark reviewed & next"));
    assert!(ui.app().project.notes.iter().all(|note| note.reviewed));
}
