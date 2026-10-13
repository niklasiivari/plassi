//! Better seats for one person and places for their whole group.

use super::*;

impl PlassiApp {
    /// Lists the swaps for `person` that improve the score the most.
    pub(super) fn suggestions_ui(&mut self, ui: &mut egui::Ui, p: Palette, person: usize) {
        let key = (self.file.revision, person, self.selected_candidate);
        let project = &self.project;
        let moves = self
            .chart
            .suggestions
            .get(key, || best_moves(project, key.2, person));
        if moves.is_empty() {
            return;
        }
        ui.add_space(theme::space::XL);
        theme::section_title(ui, p, tr("Better seats"));
        let mut apply = None;
        for (seat, other, delta) in moves.iter().take(4) {
            ui.horizontal(|ui| {
                let who = other
                    .and_then(|o| self.project.person(o))
                    .map(|a| fill("Swap with {}", &[&a.name]))
                    .unwrap_or_else(|| tr("Move to spare seat").into());
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(RichText::new(who).size(theme::text::SMALL));
                    ui.label(
                        RichText::new(self.project.seat_label(seat))
                            .size(theme::text::CAPTION)
                            .color(p.muted),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(theme::small_button(tr("Apply"))).clicked() {
                        apply = Some(seat.clone());
                    }
                    ui.label(RichText::new(format!("{delta}")).color(p.good).strong());
                });
            });
        }
        if let Some(seat) = apply {
            self.move_to(person, Spot::Seat(seat));
        }
    }

    /// The selected person's wish group and places the whole group could move to.
    pub(super) fn group_ui(&mut self, ui: &mut egui::Ui, p: Palette, person: usize) {
        let Some(group) = crate::groups::group_of(&self.project, person) else {
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
                let project = &self.project;
                let scorer = crate::seating::Scorer::new(project);
                let moves = project
                    .candidates
                    .get(self.selected_candidate)
                    .map(|c| {
                        crate::groups::suggest_moves(project, &c.assignment, &group, |a| {
                            total_score(&scorer, project, a)
                        })
                    })
                    .unwrap_or_default();
                self.chart.group_moves.set(key, moves);
            }
            return;
        };
        if moves.is_empty() {
            theme::hint(ui, p, tr("No valid seats for this group"));
        }
        let mut apply = None;
        self.chart.group_preview = None;
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
            let scorer = crate::seating::Scorer::new(&self.project);
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
