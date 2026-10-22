//! Create organizations absent from the sheet and map combined memberships.

use super::harness::Ui;
use crate::catalog::Kind;
use eframe::egui::{self, Key, Modifiers};
use egui_kittest::kittest::{NodeT, Queryable};

fn create_organization(ui: &mut Ui, name: &str) {
    ui.click("Organization name");
    ui.type_text(name);
    ui.choose("Create organization");
}

#[test]
fn compact_category_tasks_keep_creation_reachable_and_preserve_mappings() {
    for width in [320.0, 768.0, 1024.0] {
        let mut ui = Ui::with_example();
        let mut project = crate::model::Project::new();
        project.attendees = crate::example::project()
            .attendees
            .into_iter()
            .take(1)
            .collect();
        project.attendees[0].organization = "ABC / XYZ".into();
        ui.app_mut().replace_project(project);
        ui.app_mut().commit("Saved changes");
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        if !ui.has("Categories") {
            ui.click("Menu");
        }
        ui.go_to("Categories");
        assert!(ui.has("Imported entries"));
        assert!(!ui.has("Organization name"));
        ui.click("Manage categories");
        create_organization(&mut ui, "XYZ");
        assert!(!ui.has("Find imported entry"));
        ui.click("Review entries");
        ui.open_combo("Choose organizations…");
        ui.choose("XYZ");
        ui.key(Modifiers::NONE, Key::Escape);
        assert_eq!(
            ui.app()
                .project
                .catalog
                .find_all(Kind::Organization, "ABC / XYZ"),
            [0]
        );
        ui.click("Manage categories");
        assert_eq!(ui.app().project.catalog.organizations.len(), 1);
        assert!(ui.has_text("1 person"));
    }
}

#[test]
fn organization_dropdown_search_preserves_memberships_and_has_an_independent_query() {
    let mut ui = Ui::open();
    ui.go_to("People & wishes");
    ui.app_mut().load_sheet_file(
        "guests.tsv",
        b"Name\tOrganization\nExample Guest\tOrg A + Org B",
    );
    ui.settle();
    ui.click("Import");
    ui.go_to("Categories");
    create_organization(&mut ui, "Org A");
    create_organization(&mut ui, "Org B");
    ui.open_combo("Choose organizations…");
    ui.type_text("ORG B");
    ui.key(Modifiers::NONE, Key::Enter);
    assert_eq!(
        ui.app()
            .project
            .catalog
            .find_all(Kind::Organization, "Org A + Org B"),
        [1]
    );
    ui.key(Modifiers::COMMAND, Key::A);
    ui.type_text("Org A");
    ui.key(Modifiers::NONE, Key::Enter);
    assert_eq!(
        ui.app()
            .project
            .catalog
            .find_all(Kind::Organization, "Org A + Org B"),
        [0, 1]
    );
    ui.key(Modifiers::COMMAND, Key::A);
    ui.type_text("missing organization");
    assert!(ui.has_text("No matching options"));
    ui.key(Modifiers::NONE, Key::Escape);
    assert_eq!(
        ui.app()
            .project
            .catalog
            .find_all(Kind::Organization, "Org A + Org B"),
        [0, 1]
    );
    ui.open_combo("Choose organizations…");
    assert!(!ui.has_text("No matching options"));
    assert!(ui.has("Org A") && ui.has("Org B"));
}

#[test]
fn organizations_can_be_created_before_import_and_combined_entries_map_to_both() {
    let mut ui = Ui::open();
    ui.go_to("Categories");
    create_organization(&mut ui, "Org A");
    assert!(ui.app().project.attendees.is_empty());
    assert_eq!(ui.app().project.catalog.organizations.len(), 1);
    ui.go_to("People & wishes");
    ui.app_mut().load_sheet_file(
        "guests.tsv",
        b"Name\tOrganization\nExample guest\tOrg A + Org B",
    );
    ui.settle();
    ui.click("Import");
    ui.go_to("Categories");
    create_organization(&mut ui, "Org B");
    ui.open_combo("Choose organizations…");
    ui.choose("Org A");
    ui.choose("Org B");
    ui.key(Modifiers::NONE, Key::Escape);
    let guest = &ui.app().project.attendees[0];
    assert_eq!(
        ui.app().project.category_names(Kind::Organization, guest),
        ["Org A", "Org B"]
    );
    assert_eq!(guest.organization, "Org A + Org B");
    assert!(
        !ui.has_text("0 people"),
        "both organizations count this guest"
    );
    // The entry remains available after its first assignment, including after reopening.
    ui.go_to("People & wishes");
    ui.go_to("Categories");
    ui.open_combo("Choose organizations…");
    ui.choose("Org A");
    ui.key(Modifiers::NONE, Key::Escape);
    assert_eq!(
        ui.app()
            .project
            .category_names(Kind::Organization, &ui.app().project.attendees[0]),
        ["Org B"]
    );
    ui.key(Modifiers::COMMAND, Key::Z);
    assert_eq!(
        ui.app()
            .project
            .category_names(Kind::Organization, &ui.app().project.attendees[0]),
        ["Org A", "Org B"]
    );
}

#[test]
fn category_creation_and_mapping_fit_phone_and_tablet_widths() {
    let mut ui = Ui::open();
    ui.go_to("Categories");
    for name in ["Org A", "Org B"] {
        create_organization(&mut ui, name);
    }
    ui.app_mut().project.attendees = crate::example::project().attendees;
    for width in [320.0, 390.0, 600.0, 820.0, 1024.0, 1440.0] {
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        ui.h.run_steps(4);
        for node in ui.h.query_all_by(|n| {
            matches!(
                n.role(),
                egui::accesskit::Role::TextInput
                    | egui::accesskit::Role::ComboBox
                    | egui::accesskit::Role::Button
            )
        }) {
            let rect = node.rect();
            if rect.bottom() < 0.0 || rect.top() > 900.0 {
                continue;
            }
            assert!(
                rect.left() >= -1.0 && rect.right() <= width + 1.0,
                "width={width}, rect={rect:?}, label={:?}",
                node.accesskit_node().label()
            );
        }
    }
}

#[test]
fn imported_entries_show_memberships_link_people_and_support_ignore_reopen_and_filters() {
    let mut ui = Ui::with_example();
    let mut project = crate::model::Project::new();
    project.attendees = crate::example::project()
        .attendees
        .into_iter()
        .take(3)
        .collect();
    for (person, spelling) in project.attendees.iter_mut().zip(["ABC / XYZ", "-", "?"]) {
        person.organization = spelling.into();
    }
    project.catalog.create_named(Kind::Organization, "XYZ");
    ui.app_mut().replace_project(project);
    ui.app_mut().commit("Saved changes");
    ui.go_to("Categories");
    assert!(ui.has("XYZ · Existing"));
    assert!(ui.has("ABC · New"));
    ui.click("Merge");
    assert_eq!(
        ui.app()
            .project
            .category_names(Kind::Organization, &ui.app().project.attendees[0]),
        ["XYZ", "ABC"]
    );
    ui.click("Unresolved");
    // Only the two unresolved spellings remain in the imported-entry section.
    assert_eq!(ui.count("Choose organizations…"), 2);
    ui.open_combo("Choose organizations…");
    let order: Vec<_> =
        ui.h.query_all_by(|n| n.role() == egui::accesskit::Role::CheckBox)
            .map(|n| n.accesskit_node().label().unwrap_or_default())
            .collect();
    assert_eq!(order, ["ABC", "XYZ"]);
    ui.choose("ABC");
    ui.key(Modifiers::NONE, Key::Escape);
    assert_eq!(
        ui.app().project.catalog.find_all(Kind::Organization, "-"),
        [1]
    );
    ui.click("Ignore entry");
    assert!(ui.app().project.catalog.is_ignored(Kind::Organization, "?"));
    assert_eq!(ui.count("Choose organizations…"), 0);
    ui.click("Resolved");
    assert!(ui.has("Ignored"));
    ui.click("Reopen");
    assert!(!ui.app().project.catalog.is_ignored(Kind::Organization, "?"));
    ui.click("Unresolved");
    ui.click("1 person");
    assert!(ui.has("People with this entry"));
    ui.click("Guest 03");
    assert_eq!(ui.app().selected_person, Some(2));
    assert!(ui.app().page == crate::app::Page::People);
}

#[test]
fn suggestion_targets_wrap_as_separate_pills_on_phones_and_tablets() {
    let mut ui = Ui::with_example();
    let mut project = crate::model::Project::new();
    project.attendees = crate::example::project()
        .attendees
        .into_iter()
        .take(1)
        .collect();
    project.attendees[0].organization =
        "ABC with a long category label / XYZ with another long category label".into();
    project
        .catalog
        .create_named(Kind::Organization, "XYZ with another long category label");
    ui.app_mut().replace_project(project);
    ui.app_mut().commit("Saved changes");
    ui.go_to("Categories");
    for width in [320.0, 390.0, 768.0, 1024.0, 1440.0] {
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        ui.h.run_steps(4);
        assert!(ui.has_text("ABC with a long category label"));
        assert!(ui.has_text("XYZ with another long category label"));
        for node in ui.h.query_all_by(|n| {
            n.value()
                .is_some_and(|v| v.contains("· New") || v.contains("· Existing"))
        }) {
            let rect = node.rect();
            assert!(
                rect.left() >= -1.0 && rect.right() <= width + 1.0,
                "width={width}: {rect:?}"
            );
        }
    }
}

#[test]
fn apply_all_skips_ambiguous_matches_until_a_person_chooses_one() {
    let mut ui = Ui::with_example();
    let mut project = crate::model::Project::new();
    project.attendees = crate::example::project()
        .attendees
        .into_iter()
        .take(2)
        .collect();
    project.attendees[0].organization = "AX".into();
    project.attendees[1].organization = "ABC / XYZ".into();
    project.catalog.create_named(Kind::Organization, "ABC XYZ");
    project.catalog.create_named(Kind::Organization, "ABC-XYZ");
    ui.app_mut().replace_project(project);
    ui.app_mut().commit("Saved changes");
    ui.go_to("Categories");
    assert!(ui.has("Abbreviation match"));
    for width in [1440.0, 390.0] {
        ui.h.set_size(egui::vec2(width, 900.0));
        ui.settle();
        let chooser =
            ui.h.query_all_by(|n| {
                n.role() == egui::accesskit::Role::ComboBox
                    && n.value().as_deref() == Some("Choose a match…")
            })
            .next()
            .unwrap()
            .rect();
        for merge in ui.h.query_all_by_label("Merge") {
            assert!(!chooser.intersects(merge.rect()), "width={width}");
        }
    }
    ui.click("Apply all");
    assert!(
        ui.app()
            .project
            .catalog
            .find(Kind::Organization, "AX")
            .is_none()
    );
    assert_eq!(
        ui.app()
            .project
            .catalog
            .find_all(Kind::Organization, "ABC / XYZ")
            .len(),
        2
    );
    ui.h.set_size(egui::vec2(390.0, 844.0));
    ui.settle();
    ui.open_combo("Choose a match…");
    ui.choose("ABC-XYZ");
    ui.click("Merge");
    assert_eq!(
        ui.app().project.catalog.find_all(Kind::Organization, "AX"),
        [1]
    );
}
