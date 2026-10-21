//! Popups must remain readable when their trigger is just a small arrow.
use super::harness::Ui;
use crate::app::Page;
use eframe::egui::{self, Key, Modifiers};
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn arrow_choice_menus_have_readable_options_and_fit_the_screen() {
    let mut ui = Ui::with_example();
    let person = ui.app().project.attendees[0].id;
    ui.app_mut().project.attendees[0].drink = "Without alcohol".into();
    for lang in [crate::i18n::Lang::En, crate::i18n::Lang::Fi] {
        crate::i18n::set(lang);
        for width in [1440.0, 1024.0, 768.0, 390.0, 320.0] {
            ui.h.set_size(egui::vec2(width, 1000.0));
            ui.app_mut().page = Page::People;
            ui.app_mut().selected_person = Some(person);
            ui.app_mut().people.editing_details = true;
            ui.settle();
            for arrow in [0, 1] {
                {
                    let node =
                        ui.h.query_all_by(|n| {
                            n.role() == egui::accesskit::Role::ComboBox
                                && n.value().as_deref() == Some("")
                        })
                        .nth(arrow)
                        .expect("food/drink arrow");
                    node.scroll_to_me();
                }
                ui.settle();
                ui.h.query_all_by(|n| {
                    n.role() == egui::accesskit::Role::ComboBox && n.value().as_deref() == Some("")
                })
                .nth(arrow)
                .unwrap()
                .click();
                ui.settle();
                let search = ui.node(crate::i18n::tr("Search options…")).rect();
                assert!(search.width() >= 160.0, "width={width}: {search:?}");
                assert!(
                    search.left() >= 0.0 && search.right() <= width,
                    "width={width}: {search:?}"
                );
                let option = ui
                    .node(if arrow == 0 { "Meat" } else { "With alcohol" })
                    .rect();
                assert!(
                    option.height() <= 44.0 && option.width() >= 40.0,
                    "{option:?}"
                );
                assert!(
                    option.left() >= 0.0 && option.right() <= width,
                    "width={width}: {option:?}"
                );
                if arrow == 1 {
                    for key in [Key::ArrowDown, Key::ArrowUp] {
                        ui.key(Modifiers::NONE, key);
                        assert_eq!(
                            ui.node("With alcohol").accesskit_node().toggled(),
                            Some(egui::accesskit::Toggled::False)
                        );
                        assert_eq!(
                            ui.node("Without alcohol").accesskit_node().toggled(),
                            Some(egui::accesskit::Toggled::True)
                        );
                    }
                }
                ui.key(Modifiers::NONE, Key::Escape);
            }
        }
    }
}

#[test]
fn phone_navigation_uses_available_height_for_the_storage_controls() {
    let mut ui = Ui::with_example();
    ui.h.set_size(egui::vec2(390.0, 844.0));
    ui.settle();
    ui.click("Menu");
    let navigation = ui.node("Seating chart").rect().center();
    let storage =
        ui.h.query_all_by(|node| {
            node.value().as_deref() == Some("Saved to file")
                || node.label().as_deref() == Some("Saved to file")
        })
        .next()
        .expect("storage status")
        .rect();
    assert!(storage.bottom() < 844.0);
    assert_eq!(
        ui.h.ctx.layer_id_at(storage.center()),
        ui.h.ctx.layer_id_at(navigation),
        "storage status must be visible in the popup without unnecessary scrolling"
    );
}
