//! The project switcher at the top of the sidebar: every event is a project
//! of its own in the app's library.

use super::*;
use crate::i18n::{fill, tr};

/// Text typed in the switcher, and a pending delete.
#[derive(Default)]
pub(super) struct ProjectsState {
    rename: String,
    editing_name: bool,
    focus_name: bool,
    confirm_delete: bool,
}

impl PlassiApp {
    pub(super) fn project_switcher(&mut self, ui: &mut egui::Ui, p: Palette) {
        let current = self.file.name.clone();
        if self.projects.editing_name {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme::space::XS;
                let field_width =
                    (ui.available_width() - 2.0 * theme::CONTROL_HEIGHT - theme::space::XS)
                        .max(80.0);
                let field =
                    ui.add(theme::field(&mut self.projects.rename).desired_width(field_width));
                if self.projects.focus_name {
                    field.request_focus();
                    self.projects.focus_name = false;
                }
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if theme::icon_button(ui, p, theme::Icon::Check, tr("Save project name"), false)
                    .clicked()
                    || enter
                {
                    let name = std::mem::take(&mut self.projects.rename);
                    self.rename_project(&name);
                    self.projects.editing_name = false;
                }
                if theme::icon_button(ui, p, theme::Icon::Close, tr("Cancel rename"), false)
                    .clicked()
                    || (field.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)))
                {
                    self.projects.editing_name = false;
                    self.projects.rename.clear();
                }
            });
        } else {
            ui.horizontal(|ui| {
                let label = RichText::new(&current).strong().size(theme::text::BODY);
                // Clicks inside keep the menu open (the delete confirmation); every action closes it itself.
                let config = egui::containers::menu::MenuConfig::new()
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
                let (button, _) = ui
                    .scope(|ui| {
                        ui.spacing_mut().button_padding.x += theme::space::M;
                        egui::containers::menu::MenuButton::new(label)
                            .config(config)
                            .ui(ui, |ui| {
                                ui.set_min_width(240.0);
                                theme::section_title(ui, p, tr("Projects"));
                                for name in self.file.library.projects() {
                                    if ui.selectable_label(name == current, &name).clicked()
                                        && name != current
                                    {
                                        self.switch_project(&name);
                                        ui.close();
                                    }
                                }
                                ui.separator();
                                if ui.button(tr("New project")).clicked() {
                                    self.create_project(tr("New event"), Project::new());
                                    ui.close();
                                }
                                if ui.button(tr("Rename project")).clicked() {
                                    self.projects.rename = current.clone();
                                    self.projects.editing_name = true;
                                    self.projects.focus_name = true;
                                    ui.close();
                                }
                                if ui.button(tr("Duplicate this project")).clicked() {
                                    let copy = self.project.clone();
                                    self.create_project(&format!("{current} copy"), copy);
                                    ui.close();
                                }
                                self.file_actions(ui);
                                self.backup_menu_items(ui);
                                ui.separator();
                                if self.projects.confirm_delete {
                                    ui.label(
                                        RichText::new(fill(
                                            "Delete {}? A backup stays under Deleted projects.",
                                            &[&current],
                                        ))
                                        .color(p.bad),
                                    );
                                    ui.horizontal(|ui| {
                                        if ui.add(theme::danger_button(tr("Delete"), p)).clicked() {
                                            self.projects.confirm_delete = false;
                                            self.delete_project();
                                            ui.close();
                                        }
                                        if ui.button(tr("Keep it")).clicked() {
                                            self.projects.confirm_delete = false;
                                        }
                                    });
                                } else if ui.button(tr("Delete this project…")).clicked() {
                                    self.projects.confirm_delete = true;
                                }
                            })
                    })
                    .inner;
                let chevron = button.rect.right_center() - egui::vec2(theme::space::S, 0.0);
                theme::paint_icon(ui.painter(), theme::Icon::ChevronDown, chevron, p.muted);
                if button.clicked() {
                    self.projects.rename = current.clone();
                }
                button.on_hover_text(tr("Switch between events, or start a new one"));
            });
        }
    }

    /// Where the project files are (desktop), or moving projects in and out (browser).
    #[cfg(not(target_arch = "wasm32"))]
    fn file_actions(&mut self, ui: &mut egui::Ui) {
        if ui
            .button(tr("Open projects folder"))
            .on_hover_text(self.file.library.folder().display().to_string())
            .clicked()
        {
            if let Err(error) = platform::open_location(self.file.library.folder()) {
                self.status = format!("{error:#}");
            }
            ui.close();
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn file_actions(&mut self, ui: &mut egui::Ui) {
        if ui.button(tr("Load a project file…")).clicked() {
            self.file.upload = platform::FilePick::open(".json");
            ui.close();
        }
        if ui.button(tr("Download this project")).clicked() {
            self.download_project();
            ui.close();
        }
    }
}
