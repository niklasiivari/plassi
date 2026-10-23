//! Regressions for shell content that outgrows its first layout.

use super::harness::Ui;
use crate::app::{PlassiApp, copy_reminder, help, shell, theme};
use crate::i18n::{self, Lang, tr};
use crate::{io, model::Project, platform};
use eframe::egui::{self, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

#[test]
fn growing_project_library_keeps_last_project_and_file_actions_reachable() {
    let mut ui = Ui::open();
    let bytes = io::encode(&Project::new()).unwrap();
    for n in 0..30 {
        ui.app()
            .file
            .library
            .store(&format!("<project> {n:02}"))
            .write(&bytes)
            .unwrap();
    }
    for lang in [Lang::En, Lang::Fi] {
        i18n::set(lang);
        for (width, height) in [(1440.0, 900.0), (320.0, 568.0)] {
            ui.app_mut().switch_project("<project> 00");
            ui.h.set_size(egui::vec2(width, height));
            ui.settle();
            ui.open_project_menu();
            ui.node("<project> 29").scroll_to_me();
            ui.settle();
            let last = ui.node("<project> 29").rect();
            assert!(last.top() >= 0.0 && last.bottom() <= height, "{last:?}");
            ui.click("<project> 29");
            assert_eq!(ui.app().file.name, "<project> 29");
            ui.open_project_menu();
            ui.node(tr("Earlier versions…")).scroll_to_me();
            ui.settle();
            let action = ui.node(tr("Earlier versions…")).rect();
            assert!(
                action.top() >= 0.0 && action.bottom() <= height,
                "{action:?}"
            );
            ui.click(tr("Earlier versions…"));
            assert!(ui.has(tr("No versions yet.")));
            ui.key(Modifiers::NONE, Key::Escape);
            // Windows use an explicit close control; close it before the next viewport.
            ui.app_mut().backups = Default::default();
            ui.settle();
        }
    }
}

#[test]
fn chart_help_outlives_more_menu_and_all_shortcuts_can_scroll_into_view() {
    let mut ui = Ui::open();
    ui.h.set_size(egui::vec2(320.0, 568.0));
    for lang in [Lang::En, Lang::Fi] {
        i18n::set(lang);
        ui.settle();
        ui.click(tr("More"));
        ui.click(tr("Help for this page"));
        egui::Popup::close_all(&ui.h.ctx);
        ui.settle();
        assert!(ui.has_text(tr("Lower scores are better.")));
        ui.node(tr("Undo / redo")).scroll_to_me();
        ui.settle();
        let shortcut = ui.node(tr("Undo / redo")).rect();
        assert!(
            shortcut.top() >= 0.0 && shortcut.bottom() <= 568.0,
            "{shortcut:?}"
        );
        assert!(shortcut.left() >= 0.0 && shortcut.right() <= 320.0);
        ui.key(Modifiers::NONE, Key::Escape);
        assert!(!ui.has_text(tr("Lower scores are better.")));
    }
}

#[test]
fn backup_reminder_reserves_space_above_pinned_controls() {
    let _ui = Ui::open();
    for lang in [Lang::En, Lang::Fi] {
        i18n::set(lang);
        for height in [900.0, 568.0] {
            let mut h = Harness::builder()
                .with_size(egui::vec2(244.0, height))
                .build_ui_state(
                    |ui, dismissed| {
                        theme::install(ui.ctx());
                        let p = theme::palette(ui.ctx());
                        let bounds = ui.max_rect();
                        let mut controls = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(egui::Rect::from_min_max(
                                    bounds.min,
                                    egui::pos2(bounds.left() + 68.0, bounds.bottom()),
                                ))
                                .layout(egui::Layout::bottom_up(egui::Align::Center)),
                        );
                        for text in ["Collapse", "About", "Language", "Theme"] {
                            controls.add_sized(
                                egui::vec2(60.0, theme::CONTROL_HEIGHT),
                                egui::Button::new(text),
                            );
                        }
                        let bottom = controls.min_rect().top() - theme::space::S;
                        shell::sidebar_status(ui, bounds, bottom, 1.0, |ui| {
                            ui.label(tr("Saved in this browser"));
                            if !*dismissed {
                                *dismissed = copy_reminder::reminder_card(ui, p)
                                    == Some(copy_reminder::ReminderAction::Later);
                            }
                            ui.label(crate::i18n::fill("No people yet · {} seats", &[&0]));
                        });
                    },
                    false,
                );
            h.run();
            let download = h.get_by_label(tr("Download")).rect();
            let later = h.get_by_label(tr("Not now")).rect();
            let theme = h.get_by_label("Theme").rect();
            let language = h.get_by_label("Language").rect();
            for notice in [download, later] {
                assert!(
                    notice.top() >= 0.0 && notice.bottom() <= height,
                    "{notice:?}"
                );
                assert!(!notice.intersects(theme), "{notice:?} {theme:?}");
                assert!(!notice.intersects(language), "{notice:?} {language:?}");
            }
            h.get_by_label(tr("Not now")).click();
            h.run();
            assert!(*h.state());
        }
    }
}

#[test]
fn startup_uses_saved_finnish_before_creating_project_names_and_messages() {
    let ui = Ui::open();
    let path = ui.dir.path().join("finnish-startup");
    std::fs::create_dir_all(&path).unwrap();
    let library = platform::Library::at(&path);
    library.set_setting("language", "fi");
    i18n::set(Lang::En);
    let app = PlassiApp::with_library(library);
    assert_eq!(i18n::current(), Lang::Fi);
    assert_eq!(app.file.name, tr("New event"));
    assert_eq!(
        app.status,
        crate::i18n::fill("Opened {}", &[&app.file.name])
    );
    assert!(app.status.starts_with("Avattu:"));
}

#[test]
fn help_command_shortcuts_follow_platform_instead_of_hardcoding_control() {
    let ui = Ui::open();
    let ctx = ui.h.ctx.clone();
    ctx.set_os(egui::os::OperatingSystem::Windows);
    assert_eq!(help::shortcut_keys(&ctx, "Ctrl+K"), "Ctrl+K");
    assert_eq!(
        help::shortcut_keys(&ctx, "Ctrl+Z / Ctrl+Shift+Z"),
        "Ctrl+Z / Ctrl+Shift+Z"
    );
    ctx.set_os(egui::os::OperatingSystem::Mac);
    for keys in ["Ctrl+K", "Ctrl+Z / Ctrl+Shift+Z"] {
        let formatted = help::shortcut_keys(&ctx, keys);
        assert!(!formatted.contains("Ctrl"), "{formatted}");
        assert!(
            formatted.contains('⌘') || formatted.contains("Cmd"),
            "{formatted}"
        );
    }
}
