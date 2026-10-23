//! Better seats for one person and places for their whole group.

use super::*;

impl PlassiApp {
    /// Lists the swaps for `person` that improve the score the most, or the
    /// best free seats when they have no seat.
    pub(super) fn suggestions_ui(&mut self, ui: &mut egui::Ui, p: Palette, person: usize) {
        let seated = self.seat_in_option(person).is_some();
        let key = (self.file.revision, person, self.selected_candidate);
        let scorer = self.scorer();
        let project = &self.project;
        let moves = self
            .chart
            .suggestions
            .get(key, || best_moves(&scorer, project, key.2, person));
        if moves.is_empty() {
            return;
        }
        ui.add_space(theme::space::XL);
        let title = if seated {
            tr("Better seats")
        } else {
            tr("Free seats")
        };
        theme::section_title(ui, p, title);
        let mut apply = None;
        for (seat, other, delta) in moves.iter().take(4) {
            let row = ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), ui.spacing().interact_size.y),
                egui::Layout::right_to_left(egui::Align::Min),
                |ui| {
                    // Someone without a seat gets just the seat and "Seat here".
                    let who = match other.and_then(|o| self.project.person(o)) {
                        Some(a) => Some(fill("Swap with {}", &[&a.name])),
                        None if seated => Some(tr("Move to spare seat").into()),
                        None => None,
                    };
                    let place = self.project.seat_label(seat);
                    let button = if seated { tr("Apply") } else { tr("Seat here") };
                    if ui.add(theme::small_button(button)).clicked() {
                        apply = Some(seat.clone());
                    }
                    let color = if *delta < 0 { p.good } else { p.muted };
                    let text = match delta {
                        0 => "0".to_string(),
                        d => format!("{d:+}"),
                    };
                    ui.label(RichText::new(text).color(color).strong());
                    let text_width = ui.available_width().max(0.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(text_width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_width(text_width);
                            ui.spacing_mut().item_spacing.y = 0.0;
                            match &who {
                                Some(who) => {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(who).size(theme::text::SMALL),
                                        )
                                        .wrap(),
                                    );
                                    let place = RichText::new(place).size(theme::text::CAPTION);
                                    ui.add(egui::Label::new(place.color(p.muted)).wrap());
                                }
                                None => {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&place).size(theme::text::SMALL),
                                        )
                                        .wrap(),
                                    );
                                }
                            }
                        },
                    );
                },
            );
            // Pointing at a row marks its seat on the chart.
            if row.response.contains_pointer() {
                self.chart.group_preview = Some(vec![seat.clone()]);
            }
        }
        if let Some(seat) = apply {
            self.move_to(person, Spot::Seat(seat));
        }
    }

    /// The selected person's wish group and places the whole group could move to.
    pub(super) fn group_ui(&mut self, ui: &mut egui::Ui, p: Palette, person: usize) {
        let Some(group) = self.wish_group(person) else {
            return;
        };
        ui.add_space(theme::space::XL);
        theme::section_title(ui, p, &fill("Wish group · {} people", &[&group.len()]));
        let names: Vec<String> = group
            .iter()
            .filter_map(|id| self.project.person(*id).map(|a| a.name.clone()))
            .collect();
        theme::hint(ui, p, names.join(", "));
        let key = (self.file.revision, self.selected_candidate, person);
        let Some(moves) = self.chart.group_moves.peek(&key).cloned() else {
            if ui
                .button(tr("Find places for this group"))
                .on_hover_text(tr("Swap the group with people in nearby seats"))
                .clicked()
            {
                let scorer = self.scorer();
                let project = &self.project;
                let moves = project
                    .candidates
                    .get(self.selected_candidate)
                    .map(|c| {
                        crate::groups::suggest_moves(project, &c.assignment, &group, |a| {
                            total_score(&scorer, project, a)
                        })
                    })
                    .unwrap_or_default();
                self.chart.group_moves.set(key, std::rc::Rc::new(moves));
            }
            return;
        };
        if moves.is_empty() {
            theme::hint(ui, p, tr("No valid seats for this group"));
        }
        let mut apply = None;
        for (index, mv) in moves.iter().enumerate() {
            let response = ui
                .horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_max_width(ui.available_width() - 110.0);
                        ui.label(RichText::new(&mv.description).size(theme::text::SMALL));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(theme::small_button(tr("Move"))).clicked() {
                            apply = Some(index);
                        }
                        let color = if mv.delta < 0 {
                            p.good
                        } else if mv.delta > 0 {
                            p.warn
                        } else {
                            p.muted
                        };
                        ui.label(
                            RichText::new(signed(mv.delta))
                                .size(theme::text::SMALL)
                                .color(color),
                        );
                    });
                })
                .response;
            if response.contains_pointer() {
                self.chart.group_preview = Some(mv.seats.clone());
            }
        }
        if let Some(index) = apply {
            let mv = moves[index].clone();
            let scorer = self.scorer();
            let polished = crate::groups::polish(&mv, |a| total_score(&scorer, &self.project, a));
            let before = self
                .project
                .candidates
                .get(self.selected_candidate)
                .map(|c| total_score(&scorer, &self.project, &c.assignment).0)
                .unwrap_or(0);
            let after = total_score(&scorer, &self.project, &polished).0;
            let candidate = &mut self.project.candidates[self.selected_candidate];
            candidate.reseat(polished);
            candidate.solver_status = crate::model::SolverStatus::EditedByHand;
            candidate.hand_placed.extend(group.iter().copied());
            self.chart.last_change = Some((after - before, web_time::Instant::now()));
            self.chart.group_moves.clear();
            self.commit(&fill(
                "Moved group of {} · score {} → {} ({})",
                &[&group.len(), &before, &after, &signed(after - before)],
            ));
        }
    }
}
