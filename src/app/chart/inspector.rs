//! The right-hand panel: overview of the option, or details of the selected person.

use super::*;

/// A rule's row; one that can't be judged yet (someone involved has no
/// seat) is muted and shows no penalty.
pub(super) fn result_row(ui: &mut egui::Ui, p: Palette, result: &RuleResult, judged: bool) {
    let (penalty, outcome) = match judged {
        true => (result.penalty, Some(result.outcome())),
        false => (0, None),
    };
    outcome_row(ui, p, &result.label, &result.detail, penalty, outcome);
}

/// Status dot, label, detail and penalty on one line.
pub(super) fn outcome_row(
    ui: &mut egui::Ui,
    p: Palette,
    label: &str,
    detail: &str,
    penalty: i64,
    outcome: Option<crate::score::Outcome>,
) {
    let color = theme::outcome_color(p, outcome);
    ui.horizontal_wrapped(|ui| {
        theme::status_dot(ui, color);
        ui.label(RichText::new(label).size(theme::text::SMALL));
        theme::hint(ui, p, detail);
        if penalty > 0 {
            ui.label(
                RichText::new(format!("+{penalty}"))
                    .size(theme::text::SMALL)
                    .color(color),
            );
        }
    });
}

pub(super) fn stat_tile(
    ui: &mut egui::Ui,
    p: Palette,
    label: &str,
    value: String,
    color: Color32,
) -> egui::Response {
    theme::inset(p)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.label(
                    RichText::new(label)
                        .size(theme::text::CAPTION)
                        .color(p.muted),
                );
                ui.label(
                    RichText::new(value)
                        .size(theme::text::PAGE)
                        .strong()
                        .color(color),
                );
            });
        })
        .response
}

/// Wishes met, as on the option chips, and the other headline counts.
fn stat_grid(ui: &mut egui::Ui, p: Palette, scene: &Scene, evaluation: &Evaluation) {
    let (met, total) = scene.wishes;
    let alone = scene.policy().map_or(0, |p| p.isolated.len());
    let good_if = |ok: bool| if ok { p.good } else { p.bad };
    ui.columns(2, |cols| {
        let met_color = if met == total { p.good } else { p.text };
        stat_tile(
            &mut cols[0],
            p,
            tr("Wishes met"),
            format!("{met}/{total}"),
            met_color,
        );
        let hard = evaluation.hard_violations;
        stat_tile(
            &mut cols[1],
            p,
            tr("Hard rules broken"),
            hard.to_string(),
            good_if(hard == 0),
        )
        .on_hover_text(tr("Broken hard rules"));
    });
    ui.columns(2, |cols| {
        stat_tile(
            &mut cols[0],
            p,
            tr("Sitting alone"),
            alone.to_string(),
            good_if(alone == 0),
        );
        let gaps = scene.gaps.len();
        stat_tile(
            &mut cols[1],
            p,
            tr("Gaps mid-table"),
            gaps.to_string(),
            good_if(gaps == 0),
        );
    });
}

/// One clickable row: owner and rule, what happened, and the penalty.
fn issue_row(ui: &mut egui::Ui, p: Palette, name: &str, result: &RuleResult) -> egui::Response {
    let color = if result.hard_violation { p.bad } else { p.warn };
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 42.0), Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, theme::radius::SM, p.surface_hi);
    }
    painter.rect_filled(
        Rect::from_min_size(
            rect.min + Vec2::new(0.0, theme::space::S),
            Vec2::new(3.0, 26.0),
        ),
        2.0,
        color,
    );
    let penalty = painter.text(
        rect.right_center() - Vec2::new(theme::space::S, 0.0),
        Align2::RIGHT_CENTER,
        result.penalty.to_string(),
        FontId::proportional(theme::text::SMALL),
        color,
    );
    let text = painter.with_clip_rect(Rect::from_min_max(
        rect.min,
        Pos2::new(penalty.min.x - theme::space::S, rect.max.y),
    ));
    text.text(
        rect.left_top() + Vec2::new(theme::space::M, 5.0),
        Align2::LEFT_TOP,
        format!("{name} · {}", result.label),
        FontId::proportional(theme::text::SMALL),
        p.text,
    );
    text.text(
        rect.left_top() + Vec2::new(theme::space::M, 23.0),
        Align2::LEFT_TOP,
        &result.detail,
        FontId::proportional(theme::text::CAPTION),
        p.muted,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Stands in for the score while people have no seat: their wishes and the
/// seating policy would only add noise.
fn unseated_card(ui: &mut egui::Ui, p: Palette, scene: &Scene) {
    theme::card(p).fill(p.accent_soft).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(fill("{} not seated", &[&scene.unseated.len()]))
                .size(theme::text::PAGE)
                .strong()
                .color(p.warn),
        );
    });
}

fn legend(ui: &mut egui::Ui, p: Palette) {
    theme::section_title(ui, p, tr("Legend"));
    theme::text_rows(ui, |ui| {
        for (status, text) in [
            (Outcome::Met, tr("All wishes met")),
            (Outcome::Partial, tr("Some wishes not met")),
            (Outcome::Broken, tr("Hard rule broken")),
        ] {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::new(3.0, 14.0), Sense::hover());
                ui.painter()
                    .rect_filled(r, 2.0, theme::outcome_color(p, Some(status)));
                theme::hint(ui, p, text);
            });
        }
    });
    ui.add_space(theme::space::S);
    ui.add_space(theme::space::M);
    theme::section_title(ui, p, tr("Keys"));
    theme::text_rows(ui, |ui| {
        for (keys, action) in crate::app::help::HelpPage::Chart.shortcuts() {
            theme::key_hint(
                ui,
                p,
                &crate::app::help::shortcut_keys(ui.ctx(), keys),
                tr(action),
            );
        }
    });
}

impl PlassiApp {
    /// The selected person's details, or an overview of the option.
    pub(super) fn inspector(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene) {
        if let Some(id) = self.selected_person
            && self.project.person(id).is_some()
        {
            self.person_details(ui, p, scene, id);
            return;
        }
        if let Some(table) = self.chart.selected_table.clone()
            && scene.table_body(&table).is_some()
        {
            self.table_summary(ui, p, scene, &table);
            return;
        }
        let Some(evaluation) = &scene.evaluation else {
            theme::section_title(ui, p, tr("Overview"));
            ui.label(RichText::new(tr("No seating yet")).color(p.muted));
            return;
        };
        if scene.unseated.is_empty() {
            self.score_card(ui, p, scene, evaluation);
        } else {
            unseated_card(ui, p, scene);
        }
        ui.add_space(theme::space::L);
        stat_grid(ui, p, scene, evaluation);
        self.warnings(ui, p, scene, evaluation);
        ui.add_space(theme::space::XL);
        self.issue_list(ui, p, scene, evaluation);
        self.preferences_section(ui, p, &evaluation.preferences);
        ui.add_space(theme::space::XL);
        self.color_section(ui, p);
        ui.add_space(theme::space::XL);
        legend(ui, p);
    }

    fn score_card(&self, ui: &mut egui::Ui, p: Palette, scene: &Scene, evaluation: &Evaluation) {
        let status = self
            .project
            .candidates
            .get(self.selected_candidate)
            .map(|c| c.solver_status.describe())
            .unwrap_or_default();
        let policy_cost = evaluation.policy_penalty();
        theme::card(p).fill(p.accent_soft).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::text_rows(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(tr("SCORE"))
                            .size(theme::text::CAPTION)
                            .strong()
                            .color(p.muted),
                    );
                    ui.label(
                        RichText::new(tr("lower is better"))
                            .size(theme::text::CAPTION)
                            .color(p.muted),
                    );
                })
            });
            ui.label(
                RichText::new(scene.score.to_string())
                    .size(theme::text::DISPLAY)
                    .strong(),
            );
            theme::hint(
                ui,
                p,
                fill(
                    "wishes {} · your preferences {} · seating policy {}",
                    &[
                        &evaluation.wish_penalty,
                        &evaluation.preference_penalty,
                        &policy_cost,
                    ],
                ),
            );
            theme::hint(ui, p, status);
        });
    }

    /// Gaps with a fix, people sitting alone, comparison and hard-rule notes.
    fn warnings(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene, evaluation: &Evaluation) {
        if !scene.gaps.is_empty() {
            ui.add_space(theme::space::L);
            theme::tinted_card(p, p.bad).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(tr("Gaps in tables")).strong().color(p.bad));
                theme::hint(ui, p, tr("Moves people up; keeps pinned seats."));
                if ui.add(theme::primary_button(tr("Close gaps"), p)).clicked() {
                    self.close_gaps();
                }
            });
        }
        let isolated = scene
            .policy()
            .map(|p| p.isolated.as_slice())
            .unwrap_or_default();
        for a in isolated.iter().filter_map(|id| self.project.person(*id)) {
            ui.label(
                RichText::new(fill("{} has nobody beside or opposite", &[&a.name]))
                    .size(theme::text::SMALL)
                    .color(p.bad),
            );
        }
        for a in evaluation
            .left_out
            .iter()
            .filter(|id| scene.seat_of.contains_key(id) && !scene.waits(**id))
            .filter_map(|id| self.project.person(*id))
        {
            ui.label(
                RichText::new(fill("No wished-for people near {}", &[&a.name]))
                    .size(theme::text::SMALL)
                    .color(p.warn),
            );
        }
        if !scene.moved.is_empty() {
            let text = match scene.moved.len() {
                1 => tr("1 person in a different seat").to_string(),
                n => fill("{} people in different seats", &[&n]),
            };
            ui.horizontal(|ui| {
                theme::status_dot(ui, p.warn);
                ui.label(RichText::new(text).size(theme::text::SMALL));
            });
        }
        let broken = scene
            .own
            .keys()
            .filter(|id| scene.seat_of.contains_key(id))
            .filter(|id| scene.status(**id) == Some(Outcome::Broken))
            .count();
        let broken = match broken {
            0 => None,
            1 => Some(tr("1 person with a broken hard rule").to_string()),
            n => Some(fill("{} people with broken hard rules", &[&n])),
        };
        if let Some(text) = broken {
            ui.label(RichText::new(text).color(p.bad));
        }
        for error in &evaluation.assignment_errors {
            ui.label(RichText::new(error).color(p.bad).size(theme::text::SMALL));
        }
    }

    /// Unmet wishes of seated people, worst first; clicking one selects and
    /// centres its owner.
    fn issue_list(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        scene: &Scene,
        evaluation: &Evaluation,
    ) {
        theme::section_title(ui, p, tr("Needs attention"));
        let issues = &scene.issues;
        if issues.is_empty() && !scene.unseated.is_empty() {
            theme::hint(ui, p, tr("Nothing for the people seated so far"));
        } else if issues.is_empty() {
            ui.label(RichText::new(tr("All wishes met")).color(p.good));
        }
        let mut select = None;
        for &(owner, index) in issues {
            let result = &evaluation.results[index];
            let name = owner
                .and_then(|id| self.project.person(id))
                .map_or("?", |a| a.name.as_str());
            if issue_row(ui, p, name, result).clicked() {
                select = owner;
            }
        }
        if let Some(id) = select {
            self.selected_person = Some(id);
            self.focus_person(id, scene);
        }
    }
}
