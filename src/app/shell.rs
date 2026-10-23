//! The window chrome: sidebar navigation, status toast and page container.

use super::*;
use crate::i18n::{fill, tr};

/// Interface language: a small button showing the selected language's code.
pub(super) fn language_toggle(app: &mut PlassiApp, ui: &mut egui::Ui) {
    use crate::i18n::{self, Lang};
    let current = i18n::current();
    let next = match current {
        Lang::En => Lang::Fi,
        Lang::Fi => Lang::En,
    };
    let button = egui::Button::new(
        RichText::new(current.code().to_uppercase())
            .size(theme::text::SMALL)
            .strong(),
    )
    .min_size(egui::vec2(theme::CONTROL_HEIGHT, theme::CONTROL_HEIGHT));
    if ui
        .add(button)
        .on_hover_text(fill("Switch language to {}", &[&next.name()]))
        .clicked()
    {
        i18n::set(next);
        app.file.library.set_setting("language", next.code());
        app.file.revision += 1;
    }
}

/// Light/dark switch as a sun or moon icon.
pub(super) fn theme_toggle(ui: &mut egui::Ui, p: Palette) {
    let dark = ui.ctx().global_style().visuals.dark_mode;
    let (icon, tip) = if dark {
        (theme::Icon::Sun, tr("Light mode"))
    } else {
        (theme::Icon::Moon, tr("Dark mode"))
    };
    if theme::icon_button(ui, p, icon, tip, false).clicked() {
        ui.ctx().set_theme(if dark {
            egui::Theme::Light
        } else {
            egui::Theme::Dark
        });
    }
}

fn nav_item(
    ui: &mut egui::Ui,
    p: Palette,
    selected: bool,
    icon: theme::Icon,
    label: Option<&str>,
    badge: Option<(usize, Color32)>,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(
            ui.available_width(),
            if theme::mobile(ui.ctx()) {
                44.0
            } else {
                theme::CONTROL_HEIGHT
            },
        ),
        egui::Sense::click(),
    );
    let fill = if selected {
        p.accent_soft
    } else if response.hovered() {
        p.surface_hi
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, theme::radius::MD, fill);
    let fg = if selected { p.text } else { p.muted };
    // The rail's icon column stays fixed while its labels are revealed.
    let icon_at = rect.left_center() + egui::vec2(34.0, 0.0);
    theme::paint_icon(
        ui.painter(),
        icon,
        icon_at,
        if selected { p.accent } else { p.muted },
    );
    if let Some(label) = label {
        let opacity = ((rect.width() - 68.0) / 160.0).clamp(0.0, 1.0);
        let fg = fg.gamma_multiply(opacity);
        let label = RichText::new(label).size(theme::text::BODY).color(fg);
        let label = if selected { label.strong() } else { label };
        let galley = egui::WidgetText::from(label).into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            (rect.width() - 68.0 - if badge.is_some() { 36.0 } else { 0.0 }).max(0.0),
            egui::TextStyle::Body,
        );
        let at = rect.left_center() + egui::vec2(60.0, -galley.size().y / 2.0);
        ui.painter().galley(at, galley, fg);
    }
    if let Some((count, color)) = badge {
        if label.is_none() {
            ui.painter()
                .circle_filled(rect.right_top() + egui::vec2(-8.0, 8.0), 4.0, color);
            return response.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
        let center = rect.right_center() - egui::vec2(theme::space::M + 8.0, 0.0);
        let badge = egui::Rect::from_center_size(center, egui::vec2(26.0, 20.0));
        ui.painter().rect_filled(
            badge,
            theme::radius::PILL,
            theme::blend(p.surface, color, 0.16),
        );
        ui.painter().text(
            center,
            egui::Align2::CENTER_CENTER,
            count.to_string(),
            egui::FontId::proportional(theme::text::CAPTION),
            color,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A scrolling page whose content column is centred and at most `width` wide.
pub(super) fn page_scroll(ui: &mut egui::Ui, width: f32, add: impl FnOnce(&mut egui::Ui)) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let available = ui.available_width();
            let padding = if theme::narrow(ui) {
                theme::space::L
            } else {
                theme::space::XXL
            };
            let content = width.min((available - 2.0 * padding).max(0.0));
            let side = ((available - content) / 2.0).max(padding);
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: side as i8,
                    right: side as i8,
                    top: theme::space::XXL as i8,
                    bottom: theme::space::XXXL as i8,
                })
                .show(ui, |ui| {
                    ui.set_width(content);
                    add(ui);
                });
        });
}

/// The save status above the sidebar's controls, in points. It takes the
/// place of the save dot, which only the narrow sidebar shows.
const STATUS_HEIGHT: f32 = 60.0;

/// Measure the status content and reserve its height above the pinned controls.
/// The previous measurement positions the immediate-mode UI; clipping keeps a
/// newly appearing reminder away from the controls until the next frame.
pub(super) fn sidebar_status(
    ui: &mut egui::Ui,
    bounds: egui::Rect,
    bottom: f32,
    openness: f32,
    add: impl FnOnce(&mut egui::Ui),
) -> f32 {
    let id = ui.id().with("sidebar-status-height");
    let max_height = (bottom - bounds.top() - 64.0).max(0.0);
    let height = ui
        .ctx()
        .data(|d| d.get_temp::<f32>(id))
        .unwrap_or(STATUS_HEIGHT)
        .min(max_height);
    let rect = egui::Rect::from_min_max(
        egui::pos2(bounds.left() + theme::space::L, bottom - height),
        egui::pos2(bounds.right() - theme::space::S, bottom),
    );
    let mut status = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("sidebar-project")
            .max_rect(rect),
    );
    status.set_opacity(openness);
    status.set_clip_rect(status.clip_rect().intersect(rect));
    let content = egui::ScrollArea::vertical()
        .id_salt("sidebar-status-content")
        .max_height(height)
        .auto_shrink([false, false])
        .show(&mut status, add);
    let measured = content.content_size.y.min(max_height);
    if (measured - height).abs() > 0.5 {
        ui.ctx().data_mut(|d| d.insert_temp(id, measured));
        ui.ctx().request_repaint();
    }
    rect.top()
}

impl PlassiApp {
    /// Compact navigation retains project and file actions without taking canvas width.
    pub(super) fn mobile_header(&mut self, ui: &mut egui::Ui, p: Palette) {
        egui::Panel::top("mobile-header")
            .frame(theme::panel_frame(p).inner_margin(egui::Margin::same(8)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let button = ui.button(tr("Menu"));
                    egui::Popup::menu(&button)
                        .style(theme::menu_style)
                        .show(|ui| {
                            self.navigation_menu_contents(ui, p);
                        });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        self.project_switcher(ui, p);
                    });
                });
            });
    }

    fn navigation_menu_contents(&mut self, ui: &mut egui::Ui, p: Palette) {
        ui.set_width((ui.ctx().content_rect().width() - 48.0).min(320.0));
        let height = (ui.ctx().content_rect().height() - 100.0).max(100.0);
        // Popup areas start with a smaller default height. Let navigation use
        // the space below the header before introducing a scrollbar.
        ui.set_max_height(height);
        egui::ScrollArea::vertical()
            .max_height(height)
            .show(ui, |ui| {
                let before = self.page;
                self.navigation(ui, p, true);
                if self.page != before {
                    ui.close();
                }
                ui.separator();
                ui.horizontal(|ui| {
                    theme_toggle(ui, p);
                    language_toggle(self, ui);
                    super::about::button(ui, p);
                });
                self.file_controls(ui, p);
            });
    }

    /// Navigation scrolls independently of the controls pinned to the bottom.
    pub(super) fn sidebar(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        expanded: bool,
        openness: f32,
    ) -> bool {
        let bounds = ui.max_rect();
        let mut header = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("sidebar-project-selector")
                .max_rect(egui::Rect::from_min_size(
                    bounds.min,
                    egui::vec2(bounds.width(), 56.0),
                )),
        );
        self.sidebar_project_switcher(&mut header, p);
        let controls_rect = egui::Rect::from_min_max(
            bounds.min,
            egui::pos2(bounds.left() + 68.0, bounds.bottom()),
        );
        let mut controls = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("sidebar-controls")
                .max_rect(controls_rect)
                .layout(egui::Layout::bottom_up(egui::Align::Center)),
        );
        // This order and column are identical in both widths.
        let toggle = self.collapse_toggle(&mut controls, p, expanded);
        super::about::button(&mut controls, p);
        language_toggle(self, &mut controls);
        theme_toggle(&mut controls, p);
        let status_bottom = controls.min_rect().top() - theme::space::S;
        // Cross-fade the rail's dot into the status row; do not show two
        // separate indicators for the same saved state in the expanded sidebar.
        controls.scope(|ui| {
            ui.set_opacity(1.0 - openness);
            self.sync_dot(ui, p).on_hover_text(self.sync_text());
        });

        let footer_top = if openness > 0.0 {
            sidebar_status(ui, bounds, status_bottom, openness, |ui| {
                self.file_controls(ui, p);
            })
        } else {
            controls.min_rect().top()
        };
        let navigation_rect = egui::Rect::from_min_max(
            bounds.min + egui::vec2(0.0, 64.0),
            egui::pos2(
                bounds.right(),
                (footer_top - theme::space::S).max(bounds.top() + 64.0),
            ),
        );
        let mut navigation = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("sidebar-navigation")
                .max_rect(navigation_rect),
        );
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(&mut navigation, |ui| self.navigation(ui, p, openness > 0.0));
        toggle
    }

    fn collapse_toggle(&mut self, ui: &mut egui::Ui, p: Palette, expanded: bool) -> bool {
        let tip = if expanded {
            tr("Collapse sidebar")
        } else {
            tr("Expand sidebar")
        };
        theme::icon_button(ui, p, theme::Icon::Sidebar, tip, false).clicked()
    }

    /// One item per page; badges show hard-rule violations and notes to review.
    fn navigation(&mut self, ui: &mut egui::Ui, p: Palette, expanded: bool) {
        let pending = self.project.notes.iter().filter(|n| !n.reviewed).count();
        let hard = self.evaluation().map_or(0, |e| e.hard_violations);
        let unmapped: usize = crate::catalog::Kind::ALL
            .iter()
            .map(|kind| {
                self.project
                    .spellings(*kind)
                    .iter()
                    .filter(|(s, _)| self.project.catalog.find(*kind, s).is_none())
                    .count()
            })
            .sum();
        let unmapped_badge = (unmapped > 0).then_some((unmapped, p.warn));
        // Seats missing for everyone to sit.
        let short = self
            .project
            .attendees
            .len()
            .saturating_sub(self.project.layout.seat_count());
        ui.spacing_mut().item_spacing.y = theme::space::XS;
        let pages = [
            (
                Page::Chart,
                theme::Icon::Chart,
                tr("Seating chart"),
                (hard > 0).then_some((hard, p.bad)),
            ),
            (Page::Graph, theme::Icon::Graph, tr("Connections"), None),
            (
                Page::People,
                theme::Icon::People,
                tr("People & wishes"),
                (pending > 0).then_some((pending, p.warn)),
            ),
            (
                Page::Categories,
                theme::Icon::Categories,
                tr("Categories"),
                unmapped_badge,
            ),
            (
                Page::Policy,
                theme::Icon::Policy,
                tr("Seating policy"),
                None,
            ),
            (
                Page::Room,
                theme::Icon::Room,
                tr("Room layout"),
                (short > 0).then_some((short, p.bad)),
            ),
            (Page::Print, theme::Icon::Print, tr("Print"), None),
        ];
        for (page, icon, label, badge) in pages {
            let shown = expanded.then_some(label);
            let item = nav_item(ui, p, self.page == page, icon, shown, badge);
            // Named for screen readers (and UI tests), also when only the icon shows.
            item.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    true,
                    self.page == page,
                    label,
                )
            });
            if item.on_hover_text(label).clicked() {
                if self.page != page {
                    self.flush_pending_details();
                }
                self.page = page;
            }
        }
    }

    /// Open-file field, sync status and project size (laid out bottom up).
    fn file_controls(&mut self, ui: &mut egui::Ui, p: Palette) {
        // Status text should wrap instead of losing the part explaining
        // where the project was saved.
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        let sync = ui
            .horizontal_top(|ui| {
                self.sync_dot(ui, p);
                theme::hint(ui, p, self.sync_text());
            })
            .response;
        #[cfg(target_arch = "wasm32")]
        {
            let note = match platform::storage_persisted() {
                Some(true) => tr("Clearing site data deletes your projects."),
                _ => tr("Browser data can be cleared or removed. Download a backup."),
            };
            sync.on_hover_text(note);
        }
        #[cfg(not(target_arch = "wasm32"))]
        sync.on_hover_text(self.file.store.location());
        #[cfg(target_arch = "wasm32")]
        self.browser_storage(ui, p);
        let (people, seats) = (
            self.project.attendees.len(),
            self.project.layout.seat_count(),
        );
        let size = match seats.cmp(&people) {
            _ if people == 0 => fill("No people yet · {} seats", &[&seats]),
            std::cmp::Ordering::Less => {
                fill("{} people · {} seats short", &[&people, &(people - seats)])
            }
            std::cmp::Ordering::Equal => fill("{} people · no spare seats", &[&people]),
            std::cmp::Ordering::Greater => {
                fill("{} people · {} free seats", &[&people, &(seats - people)])
            }
        };
        let color = if seats < people { p.bad } else { p.muted };
        ui.label(RichText::new(size).size(theme::text::SMALL).color(color));
    }

    fn sync_text(&self) -> &'static str {
        if self.file.external_invalid {
            tr("Invalid file · last saved version shown")
        } else if self.printing.details_dirty || self.people.details_dirty {
            tr("Unsaved changes")
        } else if cfg!(target_arch = "wasm32") {
            tr("Saved in this browser")
        } else {
            tr("Saved to file")
        }
    }

    fn sync_dot(&self, ui: &mut egui::Ui, p: Palette) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
        let color = if self.file.external_invalid
            || self.printing.details_dirty
            || self.people.details_dirty
        {
            p.warn
        } else {
            p.good
        };
        ui.painter().circle_filled(rect.center(), 4.0, color);
        response
    }

    /// Transient status message in the bottom-right corner.
    pub(super) fn toast(&mut self, ctx: &egui::Context, p: Palette) {
        let age = self.status_at.elapsed().as_secs_f32();
        let sticky = self.file.external_invalid || self.solving.is_some();
        if self.status.is_empty() || (!sticky && age > 6.0) {
            return;
        }
        if !sticky {
            if age < 5.0 {
                ctx.request_repaint_after(Duration::from_secs_f32(5.0 - age));
            } else {
                ctx.request_repaint();
            }
        }
        let alpha = if sticky {
            1.0
        } else {
            (6.0 - age).clamp(0.0, 1.0)
        };
        let undoable = self
            .file
            .undo_stack
            .last()
            .is_some_and(|s| s.change == self.status);
        egui::Area::new(egui::Id::new("toast"))
            .anchor(
                egui::Align2::RIGHT_BOTTOM,
                egui::vec2(-theme::space::XL, -theme::space::XL),
            )
            .movable(false)
            .interactable(self.solving.is_some() || undoable)
            .show(ctx, |ui| {
                ui.set_opacity(alpha);
                theme::toast_frame(p).show(ui, |ui| {
                    ui.set_max_width(420.0_f32.min(ui.ctx().content_rect().width() - 72.0));
                    let color = p.text;
                    ui.visuals_mut().override_text_color = Some(color);
                    let mut stop = false;
                    if let Some(job) = &self.solving {
                        let (started, budget) = (job.started, job.budget);
                        if job.ready {
                            let done =
                                started.elapsed().as_secs_f32() / budget.as_secs_f32().max(1.0);
                            ui.label(fill(
                                "Searching… {} / {} s",
                                &[
                                    &started.elapsed().as_secs().min(budget.as_secs()),
                                    &budget.as_secs(),
                                ],
                            ));
                            ui.add(egui::ProgressBar::new(done.min(1.0)).desired_height(6.0));
                        } else {
                            ui.label(&self.status);
                        }
                        ui.horizontal_wrapped(|ui| {
                            if let Some((score, hard)) = job.runner.progress() {
                                let best = if hard > 0 {
                                    fill("Best: score {} · {} hard rules broken", &[&score, &hard])
                                } else {
                                    fill("Best score: {}", &[&score])
                                };
                                ui.label(best);
                            }
                            let button = theme::danger_button(tr("Stop"), p)
                                .min_size(egui::vec2(0.0, theme::CHIP_HEIGHT));
                            stop = ui
                                .add(button)
                                .on_hover_text(tr("Keep the best result"))
                                .clicked();
                        });
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            if self.file.external_invalid {
                                theme::status_dot(ui, p.warn);
                            }
                            ui.add(
                                egui::Label::new(RichText::new(&self.status).color(color)).wrap(),
                            );
                            let undo =
                                egui::Button::new(RichText::new(tr("Undo")).color(color).strong())
                                    .frame(false);
                            if undoable && ui.add(undo).clicked() {
                                self.undo(false);
                            }
                        });
                    }
                    if stop {
                        self.stop_search();
                    }
                });
            });
    }
}
