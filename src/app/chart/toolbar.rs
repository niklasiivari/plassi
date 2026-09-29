//! The toolbar above the chart: solving, options, undo/redo, search and zoom.

use super::*;

/// Something chosen from an option chip's context menu.
enum OptionAction {
    Duplicate(usize),
    Compare(usize),
    Delete(usize),
    Rename(usize),
}

/// What an option chip shows.
#[derive(Clone, Copy, Default)]
pub(in crate::app) struct OptionSummary {
    score: i64,
    hard: usize,
    /// Wishes fully met, of all wishes.
    met: usize,
    wishes: usize,
}

impl PlassiApp {
    pub(super) fn chart_toolbar(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene) {
        ui.horizontal(|ui| {
            self.solve_buttons(ui, p);
            ui.add_space(theme::space::L);
            self.option_chips(ui, p);
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
        let can_solve = self.solving.is_none() && !self.file.external_invalid && people > 0 && seats >= people;
        let has_option = !self.project.candidates.is_empty();
        if ui
            .add_enabled(can_solve, theme::primary_button(tr("Solve"), p))
            .on_hover_text(tr("Find the best chart for the current wishes and policy"))
            .on_disabled_hover_text(tr("Add people and enough seats first"))
            .clicked()
        {
            self.start_solve(false);
        }
        let hand_placed = self
            .project
            .candidates
            .get(self.selected_candidate)
            .map_or(0, |c| c.hand_placed.len());
        if ui
            .add_enabled(can_solve && has_option, theme::secondary_button(tr("Improve")))
            .on_hover_text(fill(
                "Re-optimize this option as a new one, keeping pinned people and the {} people you moved by hand",
                &[&hand_placed],
            ))
            .clicked()
        {
            self.start_improve();
        }
        // Less frequent searches, in a menu to keep the toolbar short.
        ui.add_enabled_ui(can_solve && has_option, |ui| {
            ui.menu_button(tr("More"), |ui| {
                if ui.button(tr("Different option")).on_hover_text(tr("Solve again, moving about 10% of the people in wishes")).clicked() {
                    self.start_solve(true);
                    ui.close();
                }
                let more = fill(
                    "Search {} s more from this option, keeping only pinned people; saves a new option only if it is better",
                    &[&crate::seating::SOLVE_TIME.as_secs()],
                );
                if ui.button(tr("Keep searching")).on_hover_text(more).clicked() {
                    self.start_continue();
                    ui.close();
                }
            });
        });
        if self.solving.is_some() {
            ui.spinner();
        }
    }

    /// Score and hard-violation count of every option, recomputed when the project changes.
    fn option_scores(&mut self) -> Vec<OptionSummary> {
        let project = &self.project;
        self.chart
            .score_cache
            .get(self.file.revision, || {
                let scorer = crate::seating::Scorer::new(project);
                project
                    .candidates
                    .iter()
                    .map(|c| {
                        let e = score::evaluate_with(&scorer, project, &c.assignment);
                        let wishes: Vec<_> = e.results.iter().filter(|r| project.is_wish(r.rule_id)).collect();
                        OptionSummary {
                            score: e.penalty(),
                            hard: e.hard_violations,
                            met: wishes.iter().filter(|r| r.outcome() == Outcome::Met).count(),
                            wishes: wishes.len(),
                        }
                    })
                    .collect()
            })
            .clone()
    }

    /// One chip per option with its score; the best is green, broken hard rules red.
    fn option_chips(&mut self, ui: &mut egui::Ui, p: Palette) {
        let scores = self.option_scores();
        let best = scores.iter().map(|s| (s.hard, s.score)).min();
        let mut selected = self.selected_candidate;
        let mut action = None;
        let chips = egui::ScrollArea::horizontal()
            .id_salt("option-chips")
            .max_width((ui.available_width() - 600.0).max(120.0))
            .show(ui, |ui| {
                for (index, candidate) in self.project.candidates.iter().enumerate() {
                    let active = index == self.selected_candidate;
                    let summary = scores.get(index).copied().unwrap_or_default();
                    let (score, hard) = (summary.score, summary.hard);
                    let is_best = best == Some((hard, score));
                    let score_color = match () {
                        _ if hard > 0 => p.bad,
                        _ if is_best => p.good,
                        _ => p.muted,
                    };
                    let mut label = egui::text::LayoutJob::default();
                    label.append(
                        &candidate.name,
                        0.0,
                        egui::TextFormat::simple(
                            FontId::proportional(theme::text::SMALL),
                            if active { p.accent } else { p.text },
                        ),
                    );
                    label.append(
                        &fill("  {}/{} met", &[&summary.met, &summary.wishes]),
                        0.0,
                        egui::TextFormat::simple(
                            FontId::proportional(theme::text::SMALL),
                            score_color,
                        ),
                    );
                    let mut hover = format!(
                        "{}\n{}",
                        candidate.solver_status.describe(),
                        fill("{} of {} wishes fully met · score {} (lower is better)", &[&summary.met, &summary.wishes, &score]),
                    );
                    if hard > 0 {
                        hover.push_str(&fill(" · {} hard rules broken", &[&hard]));
                    }
                    if is_best {
                        hover.push_str(tr(" · best"));
                    }
                    hover.push_str(&format!("\n{}", tr("Right-click for more")));
                    let response = ui
                        .add(
                            theme::chip(label, p, active),
                        )
                        .on_hover_text(hover);
                    if response.clicked() {
                        selected = index;
                    }
                    if response.secondary_clicked() {
                        self.chart.rename = candidate.name.clone();
                    }
                    response.context_menu(|ui| {
                        ui.horizontal(|ui| {
                            let field = ui.add(theme::field(&mut self.chart.rename).desired_width(140.0));
                            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            if ui.button(tr("Rename")).clicked() || enter {
                                action = Some(OptionAction::Rename(index));
                                ui.close();
                            }
                        });
                        if ui.button(tr("Duplicate (edit a copy)")).clicked() {
                            action = Some(OptionAction::Duplicate(index));
                        }
                        if !active && ui.button(tr("Compare with current")).clicked() {
                            action = Some(OptionAction::Compare(index));
                        }
                        if ui.button(tr("Delete")).clicked() {
                            action = Some(OptionAction::Delete(index));
                        }
                    });
                }
            });
        self.tour.anchors.options = (!self.project.candidates.is_empty()).then_some(chips.inner_rect);
        if selected != self.selected_candidate {
            self.selected_candidate = selected;
            if self.chart.compare_with == Some(selected) {
                self.chart.compare_with = None;
            }
        }
        match action {
            Some(OptionAction::Duplicate(i)) => self.duplicate_option(i),
            Some(OptionAction::Compare(i)) => self.chart.compare_with = Some(i),
            Some(OptionAction::Delete(i)) => self.delete_option(i),
            Some(OptionAction::Rename(i)) => {
                let name = self.chart.rename.trim().to_string();
                if let Some(candidate) = self.project.candidates.get_mut(i).filter(|_| !name.is_empty()) {
                    let old = std::mem::replace(&mut candidate.name, name);
                    self.commit(&fill("Renamed {} to {}", &[&old, &self.project.candidates[i].name]));
                }
            }
            None => {}
        }
    }

    /// Panel toggle and zoom (laid out right to left).
    fn view_controls(&mut self, ui: &mut egui::Ui, p: Palette) {
        let open = !self.chart.hide_inspector;
        let tip = if open {
            tr("Hide side panel")
        } else {
            tr("Show side panel")
        };
        if theme::icon_button(ui, p, theme::Icon::SidePanel, tip, open).clicked() {
            self.chart.hide_inspector = open;
        }
        let view = &mut self.chart.view;
        if theme::icon_button(ui, p, theme::Icon::Fit, tr("Fit the room to the window (F)"), false)
            .clicked()
        {
            view.fit_pending = true;
        }
        if theme::icon_button(ui, p, theme::Icon::ZoomIn, tr("Zoom in (scroll wheel)"), false).clicked()
        {
            view.camera.zoom_by(1.25);
        }
        let percent = RichText::new(format!("{:.0}%", view.camera.zoom * 100.0))
            .size(theme::text::SMALL)
            .color(p.muted);
        ui.add_sized([44.0, theme::CONTROL_HEIGHT], egui::Label::new(percent));
        if theme::icon_button(
            ui,
            p,
            theme::Icon::ZoomOut,
            tr("Zoom out (scroll wheel)"),
            false,
        )
        .clicked()
        {
            view.camera.zoom_by(0.8);
        }
    }

    fn history_buttons(&mut self, ui: &mut egui::Ui, p: Palette) {
        let redo = !self.file.redo_stack.is_empty();
        let tip = self.file.redo_stack.last().map_or(tr("Redo (Ctrl+Shift+Z)").into(), |s| fill("Redo: {} (Ctrl+Shift+Z)", &[&s.change]));
        if theme::icon_button_enabled(ui, p, theme::Icon::Redo, &tip, false, redo)
            .clicked()
        {
            self.undo(true);
        }
        let undo = !self.file.undo_stack.is_empty();
        let tip = self.file.undo_stack.last().map_or(tr("Undo (Ctrl+Z)").into(), |s| fill("Undo: {} (Ctrl+Z)", &[&s.change]));
        if theme::icon_button_enabled(ui, p, theme::Icon::Undo, &tip, false, undo)
            .clicked()
        {
            self.undo(false);
        }
    }

    /// A search field with a list of matching people (name or organization).
    /// Arrow keys pick, Enter selects and centres; Ctrl+F focuses it.
    fn person_search(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene) {
        let search = ui.add(theme::field(&mut self.chart.search).hint_text(tr("Find person (Ctrl+F)")).desired_width(190.0));
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
            search.request_focus();
        }
        if search.changed() {
            self.chart.search_pick = 0;
        }
        let needle = crate::app::search::Needle::new(&self.chart.search);
        // Keep the list while the pointer is on it, so a click can land.
        let over_list = self.chart.search_list.zip(ui.input(|i| i.pointer.hover_pos())).is_some_and(|(r, pos)| r.contains(pos));
        if needle.is_empty() || !(search.has_focus() || search.lost_focus() || over_list) {
            self.chart.search_list = None;
            return;
        }
        let matches: Vec<usize> = self
            .project
            .attendees
            .iter()
            .filter(|a| needle.matches(&a.name) || needle.matches(&self.project.affiliation(a)))
            .map(|a| a.id)
            .take(8)
            .collect();
        let (down, up, enter, escape) = ui.input(|i| {
            (i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape))
        });
        let last = matches.len().saturating_sub(1);
        self.chart.search_pick = match (down, up) {
            (true, _) => (self.chart.search_pick + 1).min(last),
            (_, true) => self.chart.search_pick.saturating_sub(1),
            _ => self.chart.search_pick.min(last),
        };
        if escape {
            self.chart.search.clear();
            return;
        }
        let mut chosen = (enter && search.lost_focus()).then(|| matches.get(self.chart.search_pick).copied()).flatten();
        let list = egui::Area::new(ui.id().with("search-matches"))
            .fixed_pos(search.rect.left_bottom() + Vec2::new(0.0, 4.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                theme::floating(p).show(ui, |ui| {
                    ui.set_width(280.0);
                    if matches.is_empty() {
                        theme::hint(ui, p, tr("Nobody matches"));
                    }
                    for (i, &id) in matches.iter().enumerate() {
                        let seat = scene.seat_of.get(&id).map(|s| self.project.seat_label(s)).unwrap_or_else(|| tr("no seat").into());
                        let person = self.project.person(id).expect("matched person");
                        let text = format!("{} · {} · {seat}", person.name, self.project.affiliation(person));
                        if ui.selectable_label(i == self.chart.search_pick, text).clicked() {
                            chosen = Some(id);
                        }
                    }
                });
            });
        self.chart.search_list = Some(list.response.rect);
        if let Some(id) = chosen {
            self.chart.search_list = None;
            self.selected_person = Some(id);
            self.focus_person(id, scene);
            self.chart.search.clear();
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
        let chip =
            theme::chip(RichText::new(fill("Comparing with {}  ×", &[&name])).color(p.warn), p, false)
                .fill(theme::blend(p.surface, p.warn, 0.16));
        if ui.add(chip).on_hover_text(tr("Stop comparing")).clicked() {
            self.chart.compare_with = None;
        }
    }
}
