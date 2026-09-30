//! One wish as a readable sentence with its result; the full editor opens on demand.

use super::*;
use crate::i18n::{fill, tr};
use crate::model::{Area, Project, ProximityCosts, Rule};
use crate::score::{Outcome, RuleResult};

/// "Avec with X", "Sit near Y and Z (high importance)", …
pub(super) fn describe_rule(project: &Project, rule: &Rule) -> (String, Vec<String>) {
    let name = |id: &usize| project.person_name(*id).to_string();
    let mut notes = Vec::new();
    if !rule.enabled {
        notes.push(tr("turned off").to_string());
    }
    let title = match &rule.kind {
        RuleKind::AvoidTableEnd { .. } => tr("Not at a table end").to_string(),
        RuleKind::Placement {
            area: Area::Seat(seat),
            must_be: true,
            hard: true,
            ..
        } => fill("Pinned to {}", &[&project.seat_label(seat)]),
        RuleKind::Placement {
            area,
            must_be,
            hard,
            ..
        } => {
            let place = match area {
                Area::Seat(id) => project.seat_label(id),
                Area::Table(id) | Area::Row(id) | Area::RowCenter(id) => id.clone(),
                Area::Tag(tag) => fill("seats tagged “{}”", &[tag]),
            };
            if !hard {
                notes.push(tr("preference").into());
            }
            fill(if *must_be { "Sits at {}" } else { "Not at {}" }, &[&place])
        }
        RuleKind::Proximity {
            target,
            costs,
            weight,
            max_class,
            ..
        } => {
            let who = match target {
                Target::People { ids, .. } if ids.len() > 1 => fill(
                    "All of {}",
                    &[&ids.iter().map(name).collect::<Vec<_>>().join(", ")],
                ),
                Target::People { ids, .. } => ids.iter().map(name).collect::<Vec<_>>().join(", "),
                Target::Organization { name, .. } => fill("people from {}", &[name]),
                Target::Tag { tag, .. } => fill("group “{}”", &[tag]),
            };
            let avec = costs.is_avec();
            let normal = if avec {
                ProximityCosts::AVEC_WEIGHT
            } else {
                ProximityCosts::WISH_WEIGHT
            };
            match *weight {
                w if w < normal / 2 => notes.push(tr("low importance").into()),
                w if w > normal * 3 => notes.push(tr("critical").into()),
                w if w > normal => notes.push(tr("high importance").into()),
                _ => {}
            }
            if let Some(class) = max_class {
                notes.push(fill("must be {}", &[&class.limit_label()]));
            }
            fill(if avec { "Avec with {}" } else { "Sit near {}" }, &[&who])
        }
    };
    (title, notes)
}

impl PlassiApp {
    /// A one-line summary of a rule. Returns (changed, delete requested).
    pub(super) fn wish_row(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        rule_id: u64,
        result: Option<&RuleResult>,
    ) -> (bool, bool) {
        if self.people.editing_rule == Some(rule_id) {
            return self.wish_row_editor(ui, p, rule_id);
        }
        let Some(rule) = self.project.rules.iter().find(|r| r.id == rule_id) else {
            return (false, false);
        };
        let (title, notes) = describe_rule(&self.project, rule);
        let (color, outcome) = match result {
            _ if !rule.enabled => (p.muted, String::new()),
            Some(r) => {
                let outcome = match r.outcome() {
                    Outcome::Broken => tr("Broken"),
                    Outcome::Partial => tr("Not met"),
                    Outcome::Met => tr("Met"),
                };
                (
                    theme::outcome_color(p, Some(r.outcome())),
                    format!("{outcome} · {}", r.detail),
                )
            }
            None => (p.muted, String::new()),
        };
        let mut delete = false;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                theme::status_dot(ui, color);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(RichText::new(title).size(theme::text::BODY).strong());
                    let mut line = notes;
                    if !outcome.is_empty() {
                        line.insert(0, outcome);
                    }
                    if !line.is_empty() {
                        theme::hint(ui, p, line.join(" · "));
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    delete = ui.add(theme::ghost_button(tr("Delete"), p)).clicked();
                    if ui.add(theme::small_button(tr("Edit"))).clicked() {
                        self.people.editing_rule = Some(rule_id);
                    }
                });
            });
        });
        (false, delete)
    }

    fn wish_row_editor(&mut self, ui: &mut egui::Ui, p: Palette, rule_id: u64) -> (bool, bool) {
        let result = self.rule_card(ui, p, rule_id);
        if ui.button(tr("Done")).clicked() {
            self.people.editing_rule = None;
        }
        ui.add_space(theme::space::S);
        result
    }
}
