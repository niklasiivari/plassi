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

fn organization_chart_project(wishes: usize) -> crate::model::Project {
    use crate::catalog::Category;
    use crate::model::{Attendee, Layout, Project, RuleKind, Target, WishKind};
    let mut project = Project::new();
    let template = crate::example::project().attendees.remove(0);
    project.catalog.organizations = (0..60)
        .map(|i| Category {
            name: format!("ABC {i:03}"),
            aliases: (0..8).map(|j| format!("ABC {i:03}/{j}")).collect(),
            ..Default::default()
        })
        .collect();
    project.attendees = (0..360)
        .map(|id| Attendee {
            id,
            name: format!("Guest {id:03}"),
            organization: format!("ABC {:03}/0", id % 12),
            ..template.clone()
        })
        .collect();
    project.layout = Layout::simple(30, 6);
    for id in 0..wishes {
        let requester = id % 360;
        let mut rule =
            crate::import::wish_rule(id as u64 + 1, requester, vec![], WishKind::Seating, "");
        if let RuleKind::Proximity { target, .. } = &mut rule.kind {
            *target = Target::Organization {
                name: format!("ABC {:03}", (requester + id / 360) % 12),
            };
        }
        project.rules.push(rule);
    }
    let mut option = project.empty_option("Option 1".into());
    option.assignment = project
        .attendees
        .iter()
        .zip(project.layout.seats())
        .map(|(person, seat)| (person.id, seat.id))
        .collect();
    project.candidates.push(option);
    project
}

#[test]
#[ignore = "manual chart timing comparison; run with --ignored --nocapture"]
fn organization_chart_frame_timings() {
    use crate::app::Page;
    for wishes in [0, 60, 360] {
        let mut ui = Ui::with_example();
        ui.app_mut()
            .replace_project(organization_chart_project(wishes));
        ui.app_mut().selected_candidate = 0;
        ui.app_mut().page = Page::Chart;
        ui.app_mut().commit("Saved changes");
        ui.settle();
        for state in ["overview", "hover", "selected"] {
            if state == "hover" {
                let point = ui.card_of(0).center();
                ui.h.hover_at(point);
            } else if state == "selected" {
                ui.h.hover_at(eframe::egui::pos2(0.0, 0.0));
                ui.app_mut().selected_person = Some(0);
            }
            ui.settle();
            let start = std::time::Instant::now();
            ui.h.run_steps(5);
            eprintln!(
                "{wishes} wishes / {state}: {:.2} ms/frame",
                start.elapsed().as_secs_f64() * 200.0
            );
        }
    }
}

#[test]
fn wish_priority_saves_undoes_and_fits_desktop_and_phone() {
    use crate::i18n::{self, Lang, tr};
    use crate::model::WishPriority;
    use eframe::egui::{self, Key, Modifiers};
    for (width, language) in [(1440.0, Lang::En), (390.0, Lang::Fi), (320.0, Lang::En)] {
        let mut ui = Ui::with_example();
        ui.go_to("Seating policy");
        i18n::set(language);
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        let label = tr("Organization wishes first");
        ui.node(label).scroll_to_me();
        ui.settle();
        let rect = ui.node(label).rect();
        assert!(
            rect.left() >= 0.0 && rect.right() <= width,
            "width={width}: {rect:?}"
        );
        ui.click(label);
        assert_eq!(
            ui.app().project.policy.wish_priority,
            WishPriority::Organizations
        );
        ui.key(Modifiers::COMMAND, Key::Z);
        assert_eq!(
            ui.app().project.policy.wish_priority,
            WishPriority::Balanced
        );
        ui.click(tr("Person wishes first"));
        assert_eq!(ui.app().project.policy.wish_priority, WishPriority::People);
        let ui = ui.reopen();
        assert_eq!(ui.app().project.policy.wish_priority, WishPriority::People);
    }
}
