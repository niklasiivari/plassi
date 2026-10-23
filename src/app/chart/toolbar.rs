//! The toolbar above the chart: solving, options, undo/redo, search and zoom.

use super::*;

impl PlassiApp {
    pub(super) fn chart_toolbar(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene) {
        if self.chart.single_pane {
            ui.horizontal_wrapped(|ui| {
                self.solve_buttons(ui, p);
                let more = ui.button(tr("More"));
                egui::Popup::menu(&more)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .style(theme::menu_style)
                    .show(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            if theme::icon_button(
                                ui,
                                p,
                                theme::Icon::Fit,
                                tr("Fit the room to the window (F)"),
                                false,
                            )
                            .clicked()
                            {
                                self.chart.view.fit_pending = true;
                            }
                            if theme::icon_button(
                                ui,
                                p,
                                theme::Icon::ZoomOut,
                                tr("Zoom out (scroll wheel)"),
                                false,
                            )
                            .clicked()
                            {
                                self.chart.view.camera.zoom_by(0.8);
                            }
                            if theme::icon_button(
                                ui,
                                p,
                                theme::Icon::ZoomIn,
                                tr("Zoom in (scroll wheel)"),
                                false,
                            )
                            .clicked()
                            {
                                self.chart.view.camera.zoom_by(1.25);
                            }
                        });
                        self.comparison_chip(ui, p);
                        crate::app::help::toggle_button(ui, p, crate::app::help::HelpPage::Chart);
                    });
            });
            if crate::app::help::show_panel(ui.ctx(), p, crate::app::help::HelpPage::Chart) {
                self.tour.start();
            }
            ui.horizontal(|ui| {
                self.person_search(ui, p, scene);
                self.history_buttons(ui, p);
            });
            if !self.project.candidates.is_empty() {
                ui.horizontal(|ui| self.option_chips(ui, p, ui.available_width()));
            }
            return;
        }
        if ui.available_width() < 1100.0 {
            ui.horizontal(|ui| {
                self.solve_buttons(ui, p);
                self.option_chips(ui, p, ui.available_width() - 52.0);
                if crate::app::help::help_button(ui, p, crate::app::help::HelpPage::Chart) {
                    self.tour.start();
                }
            });
            ui.horizontal_wrapped(|ui| {
                self.person_search(ui, p, scene);
                ui.horizontal(|ui| self.history_buttons(ui, p));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.view_controls(ui, p)
                });
            });
            return;
        }
        ui.horizontal(|ui| {
            self.solve_buttons(ui, p);
            ui.add_space(theme::space::L);
            self.option_chips(ui, p, ui.available_width() - 600.0);
            ui.add_space(theme::space::S);
            if crate::app::help::help_button(ui, p, crate::app::help::HelpPage::Chart) {
                self.tour.start();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.view_controls(ui, p);
                ui.add_space(theme::space::M);
                self.history_buttons(ui, p);
                ui.add_space(theme::space::M);
                self.person_search(ui, p, scene);
                self.comparison_chip(ui, p);
            });
        });
    }

    fn solve_buttons(&mut self, ui: &mut egui::Ui, p: Palette) {
        let seats = self.project.layout.seat_count();
        let people = self.project.attendees.len();
        let can_solve =
            self.solving.is_none() && !self.file.external_invalid && people > 0 && seats >= people;
        let has_option = !self.project.candidates.is_empty();
        let disabled_reason = if self.solving.is_some() {
            tr("Stop the current search before starting another")
        } else if self.file.external_invalid {
            tr("Restore or open a valid project before solving")
        } else if people == 0 {
            tr("Add people before solving")
        } else {
            tr("Add enough seats for everyone")
        };
        if ui
            .add_enabled(can_solve, theme::primary_button(tr("Solve"), p))
            .on_hover_text(tr("Find a seating arrangement"))
            .on_disabled_hover_text(disabled_reason)
            .clicked()
        {
            self.start_solve(false);
        }
        if ui
            .add_enabled(
                can_solve && has_option,
                theme::secondary_button(tr("Improve")),
            )
            .on_hover_text(fill(
                "Keep manual seats and improve the rest ({} people stay)",
                &[&self.kept_by_improve()],
            ))
            .clicked()
        {
            self.start_improve();
        }
        // Less frequent searches and the empty option, under the add-option action.
        let button = theme::icon_button_enabled(
            ui,
            p,
            theme::Icon::Plus,
            tr("Create another seating option"),
            false,
            !self.file.external_invalid && people > 0,
        );
        egui::Popup::menu(&button)
            .style(theme::menu_style)
            .show(|ui| {
                if ui
                    .button(tr("Empty option"))
                    .on_hover_text(tr("Start with nobody seated except pinned people"))
                    .clicked()
                {
                    self.add_empty_option();
                    ui.close();
                }
                let can_search = can_solve && has_option;
                if ui
                    .add_enabled(can_search, egui::Button::new(tr("Different option")))
                    .on_hover_text(tr("Find a different seating arrangement"))
                    .clicked()
                {
                    self.start_solve(true);
                    ui.close();
                }
                if ui
                    .add_enabled(can_search, egui::Button::new(tr("Continue search")))
                    .on_hover_text(tr("Keep pins; save only a better result"))
                    .clicked()
                {
                    self.start_continue();
                    ui.close();
                }
            });
        if self.solving.is_some() {
            ui.spinner();
        }
    }

    /// Panel toggle and zoom (laid out right to left).
    fn view_controls(&mut self, ui: &mut egui::Ui, p: Palette) {
        let open = if self.chart.inspector_floats {
            self.chart.floating_inspector
        } else {
            !self.chart.hide_inspector
        };
        let tip = if open {
            tr("Hide side panel")
        } else {
            tr("Show side panel")
        };
        if theme::icon_button(ui, p, theme::Icon::SidePanel, tip, open).clicked() {
            if self.chart.inspector_floats {
                self.chart.floating_inspector = !open;
            } else {
                self.chart.hide_inspector = open;
            }
        }
        if theme::icon_button(
            ui,
            p,
            theme::Icon::Fit,
            tr("Fit the room to the window (F)"),
            false,
        )
        .clicked()
        {
            self.chart.view.fit_pending = true;
        }
        if theme::icon_button(
            ui,
            p,
            theme::Icon::ZoomIn,
            tr("Zoom in (scroll wheel)"),
            false,
        )
        .clicked()
        {
            self.chart.view.camera.zoom_by(1.25);
        }
        // This toolbar lays out from right to left, so the suffix comes first.
        ui.label(RichText::new("%").size(theme::text::SMALL).color(p.muted));
        let zoom = ui
            .add(
                theme::field(&mut self.chart.zoom_entry)
                    .desired_width(52.0)
                    .horizontal_align(egui::Align::Center),
            )
            .on_hover_text(tr("Zoom percentage (20–300)"));
        if zoom.has_focus() || zoom.lost_focus() {
            self.chart.text_input_frame = Some(ui.ctx().cumulative_frame_nr());
        }
        if zoom.lost_focus()
            && let Ok(percent) = self.chart.zoom_entry.trim().parse::<f32>()
            && percent.is_finite()
            && percent > 0.0
            && format!("{:.0}", self.chart.view.camera.zoom_target() * 100.0)
                != self.chart.zoom_entry.trim()
        {
            self.chart.view.camera.zoom_to(percent / 100.0);
        }
        if !zoom.has_focus() {
            self.chart.zoom_entry = format!("{:.0}", self.chart.view.camera.zoom_target() * 100.0);
        }
        if theme::icon_button(
            ui,
            p,
            theme::Icon::ZoomOut,
            tr("Zoom out (scroll wheel)"),
            false,
        )
        .clicked()
        {
            self.chart.view.camera.zoom_by(0.8);
        }
    }

    fn history_buttons(&mut self, ui: &mut egui::Ui, p: Palette) {
        let undo_key = ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::Z,
        ));
        let redo_key = ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        ));
        let undo_tip = self.file.undo_stack.last().map_or_else(
            || fill("Undo ({})", &[&undo_key]),
            |s| fill("Undo: {} ({})", &[&s.change, &undo_key]),
        );
        let redo_tip = self.file.redo_stack.last().map_or_else(
            || fill("Redo ({})", &[&redo_key]),
            |s| fill("Redo: {} ({})", &[&s.change, &redo_key]),
        );
        let mut actions = [
            (
                theme::Icon::Undo,
                undo_tip,
                false,
                !self.file.undo_stack.is_empty(),
            ),
            (
                theme::Icon::Redo,
                redo_tip,
                true,
                !self.file.redo_stack.is_empty(),
            ),
        ];
        if ui.layout().main_dir() == egui::Direction::RightToLeft {
            actions.reverse();
        }
        for (icon, tip, redo, enabled) in actions {
            if theme::icon_button_enabled(ui, p, icon, &tip, false, enabled).clicked() {
                self.undo(redo);
            }
        }
    }

    /// A search field with a list of matching people (name or organization).
    /// Arrow keys pick, Enter selects and centres; Ctrl+K focuses it.
    fn person_search(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene) {
        let search_width = if self.chart.single_pane {
            (ui.available_width() - 112.0).max(80.0)
        } else {
            190.0
        };
        let shortcut = ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::K,
        ));
        let search = ui.add(
            theme::field(&mut self.chart.search)
                .hint_text(fill("Find person ({})", &[&shortcut]))
                .desired_width(search_width),
        );
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::K)) {
            search.request_focus();
            self.chart.text_input_frame = Some(ui.ctx().cumulative_frame_nr());
        }
        if search.has_focus() || search.lost_focus() || search.changed() {
            self.chart.text_input_frame = Some(ui.ctx().cumulative_frame_nr());
        }
        if search.changed() {
            self.chart.search_pick = 0;
        }
        let needle = crate::app::search::Needle::new(&self.chart.search);
        // Keep the list while the pointer is on it, so a click can land.
        let over_list = self
            .chart
            .search_list
            .zip(ui.input(|i| i.pointer.hover_pos()))
            .is_some_and(|(r, pos)| r.contains(pos));
        if needle.is_empty() || !(search.has_focus() || search.lost_focus() || over_list) {
            self.chart.search_list = None;
            return;
        }
        let matches: Vec<usize> = self
            .project
            .attendees
            .iter()
            .filter(|a| self.chart_search_matches(a.id, &needle))
            .map(|a| a.id)
            .take(8)
            .collect();
        let (down, up, enter, escape) = ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            )
        });
        let last = matches.len().saturating_sub(1);
        self.chart.search_pick = match (down, up) {
            (true, _) => (self.chart.search_pick + 1).min(last),
            (_, true) => self.chart.search_pick.saturating_sub(1),
            _ => self.chart.search_pick.min(last),
        };
        if escape {
            self.chart.search.clear();
            self.chart.search_list = None;
            search.surrender_focus();
            return;
        }
        let mut chosen = (enter && search.lost_focus())
            .then(|| matches.get(self.chart.search_pick).copied())
            .flatten();
        let screen = ui.ctx().content_rect().shrink(8.0);
        let below = (screen.bottom() - search.rect.bottom() - 4.0).max(0.0);
        let above = (search.rect.top() - screen.top() - 4.0).max(0.0);
        let open_above = below < 120.0 && above > below;
        let height = if open_above { above } else { below };
        let frame = theme::floating(p);
        let margin = frame.total_margin().sum();
        let width = 280.0_f32.min((screen.width() - margin.x).max(0.0));
        let x = search.rect.left().clamp(
            screen.left(),
            (screen.right() - width - margin.x).max(screen.left()),
        );
        let y = if open_above {
            search.rect.top() - 4.0
        } else {
            search.rect.bottom() + 4.0
        };
        let list = egui::Area::new(ui.id().with("search-matches"))
            .fixed_pos(Pos2::new(x, y))
            .pivot(if open_above {
                Align2::LEFT_BOTTOM
            } else {
                Align2::LEFT_TOP
            })
            .constrain_to(screen)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                frame.show(ui, |ui| {
                    ui.set_width(width);
                    egui::ScrollArea::vertical()
                        .id_salt("search-results")
                        .max_height((height - margin.y).max(0.0))
                        .min_scrolled_height(0.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            if matches.is_empty() {
                                theme::hint(ui, p, tr("Nobody matches"));
                            }
                            for (i, &id) in matches.iter().enumerate() {
                                let seat = scene
                                    .seat_of
                                    .get(&id)
                                    .map(|s| self.project.seat_label(s))
                                    .unwrap_or_else(|| tr("no seat").into());
                                let person = self.project.person(id).expect("matched person");
                                let text = format!(
                                    "{} · {} · {seat}",
                                    person.name,
                                    self.project.affiliation(person)
                                );
                                let row = ui.add(
                                    egui::Button::selectable(i == self.chart.search_pick, text)
                                        .wrap(),
                                );
                                if i == self.chart.search_pick && (down || up || search.changed()) {
                                    row.scroll_to_me(None);
                                }
                                if row.clicked() {
                                    chosen = Some(id);
                                }
                            }
                        });
                });
            });
        self.chart.search_list = Some(list.response.rect);
        if let Some(id) = chosen {
            self.chart.search_list = None;
            self.selected_person = Some(id);
            self.focus_person(id, scene);
            self.chart.search.clear();
            self.chart.mobile_view = MobileChart::Chart;
        }
    }

    fn comparison_chip(&mut self, ui: &mut egui::Ui, p: Palette) {
        let Some(name) = self
            .chart
            .compare_with
            .and_then(|i| self.project.candidates.get(i))
            .map(|c| c.name.clone())
        else {
            return;
        };
        let chip = theme::chip(
            RichText::new(fill("Comparing with {}  ×", &[&name])).color(p.warn),
            p,
            false,
        )
        .fill(theme::blend(p.surface, p.warn, 0.16));
        if ui.add(chip).on_hover_text(tr("Stop comparing")).clicked() {
            self.chart.compare_with = None;
        }
    }
}
