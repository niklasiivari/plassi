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
        self.project_switcher_ui(ui, p, false);
    }

    pub(super) fn sidebar_project_switcher(&mut self, ui: &mut egui::Ui, p: Palette) {
        self.project_switcher_ui(ui, p, true);
    }

    fn project_switcher_ui(&mut self, ui: &mut egui::Ui, p: Palette, sidebar: bool) {
        let current = self.file.name.clone();
        if self.projects.editing_name && (!sidebar || ui.available_width() >= 210.0) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme::space::XS;
                let control_width = if theme::mobile(ui.ctx()) {
                    44.0
                } else {
                    theme::CONTROL_HEIGHT
                };
                let field_width =
                    (ui.available_width() - 2.0 * control_width - 2.0 * theme::space::XS).max(40.0);
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
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .style(theme::menu_style);
                let button = if sidebar {
                    project_row(ui, p, &current)
                } else {
                    ui.scope(|ui| {
                        ui.spacing_mut().button_padding.x += theme::space::S;
                        ui.add(egui::Button::new(label))
                    })
                    .inner
                };
                if !sidebar {
                    theme::paint_icon(
                        ui.painter(),
                        theme::Icon::ChevronDown,
                        button.rect.right_center() - egui::vec2(theme::space::M, 0.0),
                        p.muted,
                    );
                }
                egui::Popup::menu(&button)
                    .close_behavior(config.close_behavior)
                    .style(theme::menu_style)
                    .show(|ui| {
                        let width = (ui.ctx().content_rect().width() - 48.0).max(0.0);
                        ui.set_max_width(width);
                        ui.set_min_width(240.0_f32.min(width));
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
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
                            if sidebar {
                                self.sidebar_compact = false;
                                self.tablet_sidebar_expanded = true;
                            }
                            self.projects.editing_name = true;
                            self.projects.focus_name = true;
                            ui.close();
                        }
                        if ui.button(tr("Duplicate project")).clicked() {
                            let copy = self.project.clone();
                            self.create_project(&fill("{} (copy)", &[&current]), copy);
                            ui.close();
                        }
                        self.file_actions(ui);
                        self.backup_menu_items(ui);
                        ui.separator();
                        if self.projects.confirm_delete {
                            ui.label(
                                RichText::new(fill(
                                    "Delete {}? Restore it from Deleted projects.",
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
                                if ui.button(tr("Cancel")).clicked() {
                                    self.projects.confirm_delete = false;
                                }
                            });
                        } else if ui.button(tr("Delete project…")).clicked() {
                            self.projects.confirm_delete = true;
                        }
                    });
                if button.clicked() {
                    self.projects.rename = current.clone();
                }
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
        if ui.button(tr("Open project file…")).clicked() {
            self.file.upload = platform::FilePick::open(".json");
            ui.close();
        }
        if ui.button(tr("Download project")).clicked() {
            self.download_project();
            ui.close();
        }
    }
}

/// A quiet project row, with the same emblem position in both sidebar widths.
fn project_row(ui: &mut egui::Ui, p: Palette, name: &str) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 56.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, theme::radius::MD, p.surface_hi);
    }
    let center = rect.left_center() + egui::vec2(34.0, 0.0);
    ui.painter().rect_filled(
        egui::Rect::from_center_size(center, egui::vec2(32.0, 32.0)),
        theme::radius::SM,
        p.accent_soft,
    );
    let initial = name
        .trim()
        .chars()
        .next()
        .unwrap_or('P')
        .to_uppercase()
        .to_string();
    ui.painter().text(
        center,
        egui::Align2::CENTER_CENTER,
        initial,
        egui::FontId::proportional(theme::text::LEAD),
        p.accent,
    );
    let opacity = ((rect.width() - 68.0) / 160.0).clamp(0.0, 1.0);
    if opacity > 0.0 {
        let left = rect.left() + 60.0;
        ui.painter().text(
            egui::pos2(left, rect.center().y - 9.0),
            egui::Align2::LEFT_CENTER,
            tr("Projects"),
            egui::FontId::proportional(theme::text::CAPTION),
            p.muted.gamma_multiply(opacity),
        );
        let galley = egui::WidgetText::from(RichText::new(name).strong().size(theme::text::BODY))
            .into_galley(
                ui,
                Some(egui::TextWrapMode::Truncate),
                (rect.width() - 88.0).max(0.0),
                egui::TextStyle::Body,
            );
        ui.painter().galley(
            egui::pos2(left, rect.center().y + 3.0),
            galley,
            p.text.gamma_multiply(opacity),
        );
        theme::paint_icon(
            ui.painter(),
            theme::Icon::ChevronDown,
            rect.right_center() - egui::vec2(16.0, 0.0),
            p.muted.gamma_multiply(opacity),
        );
    }
    response
        .on_hover_text(name)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}
