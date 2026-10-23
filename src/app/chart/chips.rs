//! The option chips in the toolbar: one per option with its result, and
//! their right-click menu.

use super::*;

/// Something chosen from an option chip's context menu.
enum OptionAction {
    Duplicate(usize),
    Compare(usize),
    Delete(usize),
    Rename(usize),
    Release(usize),
}

/// What an option chip shows.
#[derive(Clone, Copy, Default)]
pub(in crate::app) struct OptionSummary {
    score: i64,
    hard: usize,
    /// Wishes fully met, of all wishes.
    met: usize,
    wishes: usize,
    /// People without a seat; an option with any is not finished.
    unseated: usize,
}

impl OptionSummary {
    /// Orders options: finished first, then fewest broken hard rules, then score.
    fn rank(&self) -> (bool, usize, i64) {
        (self.unseated > 0, self.hard, self.score)
    }

    /// The short result after the option's name.
    fn chip_text(&self) -> String {
        match self.unseated {
            0 => fill("  {}/{} met", &[&self.met, &self.wishes]),
            n => fill("  {} not seated", &[&n]),
        }
    }
}

impl PlassiApp {
    /// Score and hard-violation count of every option, recomputed when the project changes.
    fn option_scores(&mut self) -> Vec<OptionSummary> {
        let scorer = self.scorer();
        let project = &self.project;
        self.chart
            .score_cache
            .get(self.file.revision, || {
                let wishes: HashSet<_> = project
                    .rules
                    .iter()
                    .filter(|r| matches!(r.kind, RuleKind::Proximity { .. }))
                    .map(|r| r.id)
                    .collect();
                project
                    .candidates
                    .iter()
                    .map(|c| {
                        let e = score::evaluate_with(&scorer, project, &c.assignment);
                        let wishes: Vec<_> = e
                            .results
                            .iter()
                            .filter(|r| wishes.contains(&r.rule_id))
                            .collect();
                        OptionSummary {
                            score: e.penalty(),
                            hard: e.hard_violations,
                            met: wishes
                                .iter()
                                .filter(|r| r.outcome() == Outcome::Met)
                                .count(),
                            wishes: wishes.len(),
                            unseated: c.unseated(project).len(),
                        }
                    })
                    .collect()
            })
            .clone()
    }

    /// One chip per option with its score; the best is green, broken hard rules red.
    pub(super) fn option_chips(&mut self, ui: &mut egui::Ui, p: Palette, max_width: f32) {
        let scores = self.option_scores();
        let best = scores.iter().map(OptionSummary::rank).min();
        let mut selected = self.selected_candidate;
        let mut action = None;
        let chips = egui::ScrollArea::horizontal()
            .id_salt("option-chips")
            .max_width(max_width.max(80.0))
            .show(ui, |ui| {
                for (index, candidate) in self.project.candidates.iter().enumerate() {
                    let active = index == self.selected_candidate;
                    let summary = scores.get(index).copied().unwrap_or_default();
                    let (score, hard) = (summary.score, summary.hard);
                    let is_best = best == Some(summary.rank());
                    let score_color = match () {
                        _ if hard > 0 => p.bad,
                        _ if summary.unseated > 0 => p.warn,
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
                        &summary.chip_text(),
                        0.0,
                        egui::TextFormat::simple(
                            FontId::proportional(theme::text::SMALL),
                            score_color,
                        ),
                    );
                    let mut hover = format!(
                        "{}\n{}",
                        candidate.solver_status.describe(),
                        fill(
                            "{} of {} wishes met · score {} (lower is better)",
                            &[&summary.met, &summary.wishes, &score]
                        ),
                    );
                    if hard > 0 {
                        hover.push_str(&fill(" · {} hard rules broken", &[&hard]));
                    }
                    if summary.unseated > 0 {
                        hover.push_str(&fill(" · {} not seated", &[&summary.unseated]));
                    }
                    if is_best {
                        hover.push_str(tr(" · best"));
                    }
                    hover.push_str(&format!("\n{}", tr("Right-click for actions")));
                    let response = ui.add(theme::chip(label, p, active)).on_hover_text(hover);
                    if response.clicked() {
                        selected = index;
                    }
                    if response.secondary_clicked() {
                        self.chart.rename = candidate.name.clone();
                    }
                    egui::Popup::context_menu(&response)
                        .style(theme::menu_style)
                        .show(|ui| {
                            ui.horizontal(|ui| {
                                let field = ui
                                    .add(theme::field(&mut self.chart.rename).desired_width(140.0));
                                let enter = field.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                if ui.button(tr("Rename")).clicked() || enter {
                                    action = Some(OptionAction::Rename(index));
                                    ui.close();
                                }
                            });
                            if ui.button(tr("Duplicate")).clicked() {
                                action = Some(OptionAction::Duplicate(index));
                            }
                            if !active && ui.button(tr("Compare with current")).clicked() {
                                action = Some(OptionAction::Compare(index));
                            }
                            if !candidate.hand_placed.is_empty()
                                && ui
                                    .button(tr("Let Improve move everyone"))
                                    .on_hover_text(tr("Forget who was placed by hand; pins stay"))
                                    .clicked()
                            {
                                action = Some(OptionAction::Release(index));
                            }
                            if ui.button(tr("Delete")).clicked() {
                                action = Some(OptionAction::Delete(index));
                            }
                        });
                }
            });
        self.tour.anchors.options =
            (!self.project.candidates.is_empty()).then_some(chips.inner_rect);
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
            Some(OptionAction::Release(i)) => self.release_hand_placed(i),
            Some(OptionAction::Rename(i)) => {
                let name = self.chart.rename.trim().to_string();
                if let Some(candidate) = self
                    .project
                    .candidates
                    .get_mut(i)
                    .filter(|_| !name.is_empty())
                {
                    let old = std::mem::replace(&mut candidate.name, name);
                    self.commit(&fill(
                        "Renamed {} to {}",
                        &[&old, &self.project.candidates[i].name],
                    ));
                }
            }
            None => {}
        }
    }
}
