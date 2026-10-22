//! Large synthetic wish-review sessions.

use super::harness::Ui;
use crate::app::{Page, people::PeopleFilter};
use crate::catalog::{Category, Kind};
use crate::model::{ImportField, ImportNote, Project, RuleKind, Target, WishKind};
use eframe::egui::{self, Key, Modifiers};
use egui_kittest::kittest::Queryable;

fn review_project() -> Project {
    let mut project = Project::new();
    let template = crate::example::project().attendees.remove(0);
    project.catalog.organizations = (0..60)
        .map(|i| Category {
            name: format!("ABC {i:03}"),
            aliases: (0..4).map(|j| format!("ABC {i:03}/{j}")).collect(),
            ..Default::default()
        })
        .collect();
    project.attendees = (0..360)
        .map(|id| crate::model::Attendee {
            id,
            name: format!("Guest {id:03}"),
            organization: format!("ABC {:03}/0", id % 60),
            ..template.clone()
        })
        .collect();
    project.notes = (0..360)
        .map(|person| ImportNote {
            person,
            field: ImportField::Wish,
            text: format!(
                "Guest {:03} and ABC {:03}",
                (person + 1) % 360,
                (person + 1) % 60
            ),
            reviewed: false,
        })
        .collect();
    project
}

fn add_organization_wish(project: &mut Project, requester: usize) {
    let mut rule = crate::import::wish_rule(
        project.next_rule_id(),
        requester,
        vec![],
        WishKind::Seating,
        "",
    );
    if let RuleKind::Proximity { target, .. } = &mut rule.kind {
        *target = Target::Organization {
            name: format!("ABC {:03}", (requester + 1) % 60),
        };
    }
    project.rules.push(rule);
    project.notes[requester].reviewed = true;
}

#[test]
#[ignore = "manual timing comparison; run with --ignored --nocapture"]
fn wish_review_frame_timings() {
    let mut ui = Ui::with_example();
    ui.app_mut().replace_project(review_project());
    ui.app_mut().page = Page::People;
    for reviewed in [0, 30, 60] {
        while ui.app().project.rules.len() < reviewed {
            let id = ui.app().project.rules.len();
            add_organization_wish(&mut ui.app_mut().project, id);
        }
        ui.app_mut().selected_person = Some(reviewed);
        ui.app_mut().commit("Saved changes");
        ui.settle();
        let start = std::time::Instant::now();
        ui.h.run_steps(10);
        eprintln!(
            "{reviewed} reviewed: {:.2} ms/frame",
            start.elapsed().as_secs_f64() * 100.0
        );
        let project = &ui.app().project;
        let start = std::time::Instant::now();
        std::hint::black_box(project.active_people());
        eprintln!("active people: {:?}", start.elapsed());
        let start = std::time::Instant::now();
        std::hint::black_box(crate::suggest::for_note(project, reviewed));
        eprintln!("note suggestions: {:?}", start.elapsed());
        assert!(
            !project
                .category_names(Kind::Organization, &project.attendees[0])
                .is_empty()
        );
    }
}

#[test]
fn many_wishes_can_be_accepted_and_reviewed_on_desktop_and_phone() {
    for width in [1440.0, 390.0] {
        let mut ui = Ui::with_example();
        let mut project = review_project();
        for id in 0..30 {
            add_organization_wish(&mut project, id);
        }
        project.validate().expect("synthetic project");
        ui.app_mut().replace_project(project);
        ui.app_mut().page = Page::People;
        ui.app_mut().people.filter = PeopleFilter::ToReview;
        ui.app_mut().selected_person = Some(30);
        ui.app_mut().commit("Saved changes");
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        for id in 30..65 {
            assert_eq!(ui.app().selected_person, Some(id));
            // The organization suggestion follows the individual name match.
            ui.h.query_all_by_label("+ Add")
                .nth(1)
                .expect("organization suggestion")
                .scroll_to_me();
            ui.settle();
            ui.h.query_all_by_label("+ Add").nth(1).unwrap().click();
            ui.settle();
            assert_eq!(
                ui.app().project.rules.len(),
                id + 1,
                "width={width}, id={id}"
            );
            assert!(matches!(&ui.app().project.rules.last().unwrap().kind,
                RuleKind::Proximity { requester, target: Target::Organization { .. }, .. } if *requester == id));
            ui.click("Review and next");
            assert!(ui.app().project.notes[id].reviewed);
        }
        assert_eq!(ui.app().selected_person, Some(65));
        ui.key(Modifiers::COMMAND, Key::Z);
        assert!(!ui.app().project.notes[64].reviewed);
        ui.app_mut().selected_person = Some(64);
        ui.settle();
        assert!(ui.has("Review and next"));
        ui.key(Modifiers::COMMAND, Key::Z);
        assert_eq!(ui.app().project.rules.len(), 64);
        assert_eq!(ui.count("+ Add"), 3);
        ui.key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        assert_eq!(ui.count("+ Add"), 2);
        // The list must refresh after review and after undo, including on phone.
        ui.app_mut().selected_person = None;
        ui.settle();
        assert_eq!(ui.app().people.shown.len(), 360 - 64);
        assert!(!ui.app().people.shown.contains(&63));
        assert!(ui.app().people.shown.contains(&64));
    }
}

#[test]
fn review_navigation_can_skip_backtrack_and_keep_reviewed_people_in_the_session() {
    let mut ui = Ui::with_example();
    ui.go_to("People & wishes");
    ui.click("Review notes");
    let first = ui.app().selected_person.unwrap();
    let first_notes: Vec<_> = ui
        .app()
        .project
        .notes
        .iter()
        .filter(|n| n.person == first)
        .map(|n| n.reviewed)
        .collect();
    ui.click("Next person");
    let second = ui.app().selected_person.unwrap();
    assert_ne!(first, second);
    assert_eq!(
        ui.app()
            .project
            .notes
            .iter()
            .filter(|n| n.person == first)
            .map(|n| n.reviewed)
            .collect::<Vec<_>>(),
        first_notes
    );
    ui.click("Previous person");
    assert_eq!(ui.app().selected_person, Some(first));
    ui.click("Review and next");
    assert_eq!(ui.app().selected_person, Some(second));
    ui.click("Previous person");
    assert_eq!(ui.app().selected_person, Some(first));
    assert!(ui.has("Reopen"));
    ui.click("Reopen");
    assert!(
        ui.app()
            .project
            .notes
            .iter()
            .any(|n| n.person == first && !n.reviewed)
    );
    ui.click("Next person");
    ui.click("Finish review");
    assert_eq!(ui.app().selected_person, None);
}

#[test]
fn select_all_can_be_cleared_from_the_top_of_the_people_list() {
    let mut ui = Ui::with_example();
    ui.go_to("People & wishes");
    ui.click("Select all shown");
    assert_eq!(ui.app().people.picked.len(), 48);
    ui.click("Clear selection");
    assert!(ui.app().people.picked.is_empty());
    ui.click("Select all shown");
    ui.key(Modifiers::NONE, Key::Escape);
    assert!(ui.app().people.picked.is_empty());
}
