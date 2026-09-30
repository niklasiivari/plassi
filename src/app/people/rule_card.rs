//! Editor card for one rule, and the searchable person picker.

use std::collections::BTreeMap;

use super::*;
use crate::i18n::{fill, tr};
use crate::model::{Area, DistanceClass, ProximityCosts, Rule, RuleKind, WeightMode};
use crate::score::RuleResult;

/// Attendee names by ID, for chips and pickers.
type Names = BTreeMap<usize, String>;

/// A searchable "add person" dropdown. Returns the picked attendee.
pub(super) fn person_picker(
    ui: &mut egui::Ui,
    salt: &str,
    names: &Names,
    filter: &mut String,
    exclude: usize,
) -> Option<usize> {
    let mut picked = None;
    theme::combo(salt, tr("+ add person"))
        .height(320.0)
        .show_ui(ui, |ui| {
            ui.add(
                theme::field(filter)
                    .hint_text(tr("Type to filter"))
                    .desired_width(220.0),
            )
            .request_focus();
            let needle = crate::app::search::Needle::new(filter);
            let mut matches: Vec<_> = names
                .iter()
                .filter(|(id, n)| **id != exclude && needle.matches(n))
                .collect();
            matches.sort_by(|a, b| a.1.cmp(b.1));
            for (id, name) in matches {
                if ui.selectable_label(false, name).clicked() {
                    picked = Some(*id);
                }
            }
        });
    if picked.is_some() {
        filter.clear();
    }
    picked
}

impl PlassiApp {
    /// Editor for one rule. Returns (changed, delete requested).
    pub(super) fn rule_card(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        rule_id: u64,
    ) -> (bool, bool) {
        let names = self.person_names();
        let Some(index) = self.project.rules.iter().position(|r| r.id == rule_id) else {
            return (false, false);
        };
        let effective = self.project.effective_weight(&self.project.rules[index]);
        let result = self
            .evaluation()
            .and_then(|e| e.results.iter().find(|r| r.rule_id == rule_id).cloned());
        let rule = &mut self.project.rules[index];
        let picker_filter = &mut self.people.picker_filter;
        let mut changed = false;
        let mut delete = false;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.push_id(rule_id, |ui| {
                let (edited, deleted) = rule_header(ui, p, rule, result.as_ref());
                changed |= edited;
                delete = deleted;
                changed |= match &mut rule.kind {
                    RuleKind::Placement { .. } => placement_editor(ui, &mut rule.kind),
                    RuleKind::Proximity { .. } => {
                        wish_editor(ui, p, rule, &names, picker_filter, effective)
                    }
                    RuleKind::AvoidTableEnd { .. } => table_end_editor(ui, p, &mut rule.kind),
                };
                if !rule.source.is_empty() {
                    ui.label(
                        RichText::new(format!("“{}”", rule.source))
                            .italics()
                            .size(theme::text::SMALL)
                            .color(p.muted),
                    );
                }
            });
        });
        (changed, delete)
    }
}

/// Enabled toggle, label, current result and delete button.
fn rule_header(
    ui: &mut egui::Ui,
    p: Palette,
    rule: &mut Rule,
    result: Option<&RuleResult>,
) -> (bool, bool) {
    ui.horizontal(|ui| {
        let changed = ui
            .checkbox(&mut rule.enabled, "")
            .on_hover_text(tr("Enabled"))
            .changed();
        ui.label(RichText::new(crate::i18n::tr_any(&rule.label)).strong());
        if let Some(result) = result {
            theme::status_pill(ui, p, &result.detail, theme::outcome_color(p, Some(result.outcome())));
        }
        let delete = ui
            .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(theme::ghost_button(tr("Delete"), p)).clicked()
            })
            .inner;
        (changed, delete)
    })
    .inner
}

fn placement_editor(ui: &mut egui::Ui, kind: &mut RuleKind) -> bool {
    let RuleKind::Placement {
        area,
        must_be,
        hard,
        penalty,
        ..
    } = kind
    else {
        return false;
    };
    let area_text = match area {
        Area::Seat(id) => fill("seat {}", &[id]),
        Area::Table(id) => fill("table {}", &[id]),
        Area::Row(id) => fill("row {}", &[id]),
        Area::RowCenter(id) => fill("centre of row {}", &[id]),
        Area::Tag(tag) => fill("seats tagged “{}”", &[tag]),
    };
    ui.label(fill(
        if *must_be { "Must sit at {}" } else { "Must not sit at {}" },
        &[&area_text],
    ));
    ui.horizontal(|ui| {
        let mut changed = ui.checkbox(hard, tr("Hard rule")).changed();
        if !*hard {
            ui.label(tr("Penalty"));
            changed |= ui
                .add(egui::DragValue::new(penalty).range(0..=100_000))
                .changed();
        }
        changed
    })
    .inner
}

/// The two penalties of an "avoid table ends" rule.
fn table_end_editor(ui: &mut egui::Ui, p: Palette, kind: &mut RuleKind) -> bool {
    let RuleKind::AvoidTableEnd {
        end_penalty,
        near_end_penalty,
        ..
    } = kind
    else {
        return false;
    };
    let mut changed = false;
    egui::Grid::new("table-end")
        .num_columns(2)
        .spacing([theme::space::L, theme::space::S])
        .show(ui, |ui| {
            ui.label(tr("At a table end"));
            changed |= ui
                .add(egui::DragValue::new(end_penalty).range(0..=10_000))
                .changed();
            ui.end_row();
            ui.label(tr("Next to a table end"));
            changed |= ui
                .add(egui::DragValue::new(near_end_penalty).range(0..=10_000))
                .changed();
            ui.end_row();
        });
    theme::hint(
        ui,
        p,
        fill(
            "An end seat has nobody beside them on one side. For scale: an ordinary wish at another table costs {}.",
            &[&ProximityCosts::far_penalty(false)],
        ),
    );
    changed
}

/// Kind (wish or avec), who, importance, hard limit and distance penalties.
fn wish_editor(
    ui: &mut egui::Ui,
    p: Palette,
    rule: &mut Rule,
    names: &Names,
    filter: &mut String,
    effective: i64,
) -> bool {
    let was_avec = match &rule.kind {
        RuleKind::Proximity { costs, .. } => costs.is_avec(),
        _ => return false,
    };
    let mut selected_kind = None;
    ui.horizontal(|ui| {
        let mut avec = was_avec;
        ui.selectable_value(&mut avec, false, tr("Seating wish"));
        ui.selectable_value(&mut avec, true, tr("Avec"));
        if avec != was_avec {
            selected_kind = Some(avec);
        }
    });
    if let Some(avec) = selected_kind {
        set_avec_kind(rule, avec);
    }
    let RuleKind::Proximity {
        target,
        costs,
        weight,
        weight_mode,
        max_class,
        requester,
    } = &mut rule.kind
    else {
        return selected_kind.is_some();
    };
    let mut changed = selected_kind.is_some();
    changed |= target_editor(ui, target, *requester, names, filter);
    changed |= importance_editor(ui, p, weight, weight_mode, effective);
    changed |= limit_editor(ui, max_class);
    changed |= costs_editor(ui, p, costs);
    changed
}

/// Selects the complete default profile for one of the two wish kinds.
fn set_avec_kind(rule: &mut Rule, avec: bool) {
    let RuleKind::Proximity {
        costs,
        weight,
        weight_mode,
        ..
    } = &mut rule.kind
    else {
        return;
    };
    *costs = if avec {
        ProximityCosts::AVEC
    } else {
        ProximityCosts::WISH
    };
    *weight = if avec {
        ProximityCosts::AVEC_WEIGHT
    } else {
        ProximityCosts::WISH_WEIGHT
    };
    *weight_mode = if avec {
        WeightMode::Fixed
    } else {
        WeightMode::ShareRequesterBudget
    };
    rule.label = if avec { "Avec" } else { "Seating wish" }.into();
}

fn target_editor(
    ui: &mut egui::Ui,
    target: &mut Target,
    requester: usize,
    names: &Names,
    filter: &mut String,
) -> bool {
    ui.horizontal_wrapped(|ui| {
        ui.label(tr("Near"));
        let mut changed = false;
        match target {
            Target::People { ids } => {
                let mut remove = None;
                for t in ids.iter() {
                    let name = names.get(t).cloned().unwrap_or_default();
                    let chip = theme::tag_button(&name);
                    if ui.add(chip).on_hover_text(tr("Remove")).clicked() {
                        remove = Some(*t);
                    }
                }
                if let Some(t) = remove
                    && ids.len() > 1
                {
                    ids.retain(|x| *x != t);
                    changed = true;
                }
                if let Some(t) = person_picker(ui, "rule-picker", names, filter, requester)
                    && !ids.contains(&t)
                {
                    ids.push(t);
                    changed = true;
                }
            }
            Target::Organization { name, required } => {
                ui.label(tr("people from"));
                changed |= ui.add(theme::field(name).desired_width(140.0)).changed();
                ui.label(tr("needs"));
                changed |= ui
                    .add(egui::DragValue::new(required).range(1..=50))
                    .changed();
            }
            Target::Tag { tag, required } => {
                ui.label(fill("group “{}”, needs", &[tag]));
                changed |= ui
                    .add(egui::DragValue::new(required).range(1..=50))
                    .changed();
            }
        }
        changed
    })
    .inner
}

fn importance_editor(
    ui: &mut egui::Ui,
    p: Palette,
    weight: &mut i64,
    mode: &mut WeightMode,
    effective: i64,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(tr("Importance"));
        let slider = egui::Slider::new(weight, 0..=300)
            .logarithmic(true)
            .clamping(egui::SliderClamping::Never);
        changed |= theme::slider(ui, slider).changed();
        for (label, value) in [("Low", 5), ("Normal", 10), ("High", 40), ("Critical", 150)] {
            if ui.add(theme::small_button(tr(label))).clicked() {
                *weight = value;
                changed = true;
            }
        }
    });
    ui.horizontal(|ui| {
        let mut shared = *mode == WeightMode::ShareRequesterBudget;
        let toggle = ui
            .checkbox(&mut shared, tr("Share importance with their other wishes"))
            .on_hover_text(tr(
                "When on, a person's wishes split one budget, so listing many wishes doesn't outweigh others. \
                 Turn off to give this wish its full importance.",
            ));
        if toggle.changed() {
            *mode = if shared { WeightMode::ShareRequesterBudget } else { WeightMode::Fixed };
            changed = true;
        }
        theme::hint(ui, p, fill("effective ×{}", &[&effective]));
    });
    changed
}

/// Optional hard maximum distance.
fn limit_editor(ui: &mut egui::Ui, max_class: &mut Option<DistanceClass>) -> bool {
    ui.horizontal(|ui| {
        ui.label(tr("Must be"));
        let mut changed = false;
        theme::combo("max", max_class.map(DistanceClass::limit_label).unwrap_or(tr("no hard limit")))
            .show_ui(ui, |ui| {
                changed |= ui
                    .selectable_value(max_class, None, tr("no hard limit"))
                    .changed();
                for class in DistanceClass::ALL.into_iter().filter(|c| *c != DistanceClass::Far) {
                    changed |= ui
                        .selectable_value(max_class, Some(class), class.limit_label())
                        .changed();
                }
            });
        changed
    })
    .inner
}

/// Per-distance penalties, kept non-decreasing with distance.
pub(in crate::app) fn costs_editor(ui: &mut egui::Ui, p: Palette, costs: &mut ProximityCosts) -> bool {
    let mut changed = false;
    egui::CollapsingHeader::new(RichText::new(tr("Distance penalties")).size(theme::text::SMALL).color(p.muted))
        .id_salt("costs")
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for class in DistanceClass::ALL {
                    ui.label(class.label());
                    changed |= ui.add(egui::DragValue::new(&mut costs[class]).range(0..=100_000)).changed();
                }
            });
            theme::hint(ui, p, tr("Penalty = distance cost × importance. A farther distance never costs less than a nearer one."));
        });
    if changed {
        costs.make_ordered();
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_avec_and_wish_applies_the_matching_default_profile() {
        let mut rule = crate::import::wish_rule(1, 0, vec![1], false, "");

        set_avec_kind(&mut rule, true);
        let RuleKind::Proximity {
            costs,
            weight,
            weight_mode,
            ..
        } = &rule.kind
        else {
            panic!("wish rule expected");
        };
        assert_eq!(*costs, ProximityCosts::AVEC);
        assert_eq!(*weight, ProximityCosts::AVEC_WEIGHT);
        assert_eq!(*weight_mode, WeightMode::Fixed);
        assert_eq!(rule.label, "Avec");

        set_avec_kind(&mut rule, false);
        let RuleKind::Proximity {
            costs,
            weight,
            weight_mode,
            ..
        } = &rule.kind
        else {
            panic!("wish rule expected");
        };
        assert_eq!(*costs, ProximityCosts::WISH);
        assert_eq!(*weight, ProximityCosts::WISH_WEIGHT);
        assert_eq!(*weight_mode, WeightMode::ShareRequesterBudget);
        assert_eq!(rule.label, "Seating wish");
    }
}
