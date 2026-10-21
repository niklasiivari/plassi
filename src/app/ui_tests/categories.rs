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
