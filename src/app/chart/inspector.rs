//! The right-hand panel: overview of the option, or details of the selected person.

use super::*;

pub(super) fn result_row(ui: &mut egui::Ui, p: Palette, result: &RuleResult) {
    outcome_row(ui, p, &result.label, &result.detail, result.penalty, result.hard_violation);
}

/// Status dot, label, detail and penalty on one line.
pub(super) fn outcome_row(ui: &mut egui::Ui, p: Palette, label: &str, detail: &str, penalty: i64, hard_violation: bool) {
    let color = theme::outcome_color(p, Some(crate::score::Outcome::of(hard_violation, penalty)));
    ui.horizontal_wrapped(|ui| {
        theme::status_dot(ui, color);
        ui.label(RichText::new(label).size(theme::text::SMALL));
        theme::hint(ui, p, detail);
        if penalty > 0 {
            ui.label(
                RichText::new(format!("−{penalty}"))
                    .size(theme::text::SMALL)
                    .color(color),
            );
        }
    });
}

pub(super) fn stat_tile(ui: &mut egui::Ui, p: Palette, label: &str, value: String, color: Color32) {
    theme::inset(p)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
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
}

fn stat_grid(ui: &mut egui::Ui, p: Palette, scene: &Scene, evaluation: &Evaluation) {
    let wishers = scene.own.len();
    let met = scene
        .own
        .keys()
        .filter(|id| scene.status(**id) == Some(Outcome::Met))
        .count();
    let alone = scene.policy().map_or(0, |p| p.isolated.len());
    let good_if = |ok: bool| if ok { p.good } else { p.bad };
    ui.columns(2, |cols| {
        let met_color = if met == wishers { p.good } else { p.text };
        stat_tile(
            &mut cols[0],
            p,
            tr("Wishes fully met"),
            format!("{met}/{wishers}"),
            met_color,
        );
        let hard = evaluation.hard_violations;
        stat_tile(
            &mut cols[1],
            p,
            tr("Hard rules broken"),
            hard.to_string(),
            good_if(hard == 0),
        );
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
        Rect::from_min_size(rect.min + Vec2::new(0.0, theme::space::S), Vec2::new(3.0, 26.0)),
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

fn legend(ui: &mut egui::Ui, p: Palette) {
    theme::section_title(ui, p, tr("Legend"));
    theme::text_rows(ui, |ui| {
        for (status, text) in [
            (Outcome::Met, tr("All their wishes met")),
            (Outcome::Partial, tr("Some wishes not met")),
            (Outcome::Broken, tr("Hard rule broken")),
        ] {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::new(3.0, 14.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, theme::outcome_color(p, Some(status)));
                theme::hint(ui, p, text);
            });
        }
    });
    ui.add_space(theme::space::S);
    theme::hint(ui, p, tr("Drag a card to move or swap. Ctrl-click several people to group them (G) or keep them apart. Right-click for more. Keys: F fits the room, P pins the selected person, Ctrl+F finds someone."));
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
        let Some(evaluation) = &scene.evaluation else {
            theme::section_title(ui, p, tr("Overview"));
            ui.label(
                RichText::new(tr("Solve to create a first chart. You can then drag people between seats; scores update instantly."))
                    .color(p.muted),
            );
            return;
        };
        self.score_card(ui, p, scene, evaluation);
        ui.add_space(theme::space::L);
        stat_grid(ui, p, scene, evaluation);
        self.warnings(ui, p, scene, evaluation);
        ui.add_space(theme::space::XL);
        self.issue_list(ui, p, scene, evaluation);
        self.preferences_section(ui, p, &evaluation.preferences);
        ui.add_space(theme::space::XL);
        legend(ui, p);
    }

    fn score_card(&self, ui: &mut egui::Ui, p: Palette, scene: &Scene, evaluation: &Evaluation) {
        let status = self
            .project
            .candidates
            .get(self.selected_candidate)
            .map(|c| c.solver_status.clone())
            .unwrap_or_default();
        let policy_cost = evaluation.policy_penalty();
        theme::card(p)
            .fill(p.accent_soft)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                theme::text_rows(ui, |ui| ui.horizontal(|ui| {
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
                }));
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
                        &[&evaluation.wish_penalty, &evaluation.preference_penalty, &policy_cost],
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
                ui.label(RichText::new(tr("Empty seats in the middle of a table")).strong().color(p.bad));
                theme::hint(ui, p, tr("Spare seats belong at table bottoms. Close the gaps by sliding people up (pinned people stay put)."));
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
        for a in evaluation.left_out.iter().filter_map(|id| self.project.person(*id)) {
            ui.label(
                RichText::new(fill("None of {}'s wishes is near them", &[&a.name]))
                    .size(theme::text::SMALL)
                    .color(p.warn),
            );
        }
        if !scene.moved.is_empty() {
            ui.horizontal(|ui| {
                theme::status_dot(ui, p.warn);
                ui.label(
                    RichText::new(fill(
                        "{} people sit differently in the compared option",
                        &[&scene.moved.len()],
                    ))
                    .size(theme::text::SMALL),
                );
            });
        }
        let broken = scene
            .own
            .keys()
            .filter(|id| scene.status(**id) == Some(Outcome::Broken))
            .count();
        if broken > 0 {
            ui.label(
                RichText::new(fill("{} people have a broken hard rule", &[&broken])).color(p.bad),
            );
        }
        for error in &evaluation.assignment_errors {
            ui.label(RichText::new(error).color(p.bad).size(theme::text::SMALL));
        }
    }

    /// Unmet wishes, worst first; clicking one selects and centres its owner.
    fn issue_list(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        scene: &Scene,
        evaluation: &Evaluation,
    ) {
        theme::section_title(ui, p, tr("Needs attention"));
        let mut issues: Vec<_> = evaluation
            .results
            .iter()
            .filter(|r| r.penalty > 0 || r.hard_violation)
            .collect();
        issues.sort_by_key(|r| (!r.hard_violation, -r.penalty));
        if issues.is_empty() {
            ui.label(RichText::new(tr("Every wish is met")).color(p.good));
        }
        let mut select = None;
        for result in issues {
            let owner = self
                .project
                .rules
                .iter()
                .find(|r| r.id == result.rule_id)
                .map(crate::model::Rule::owner);
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
