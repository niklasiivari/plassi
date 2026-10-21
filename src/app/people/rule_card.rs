//! Editor card for one rule, and the searchable person picker.

use std::collections::BTreeMap;

use super::*;
use crate::i18n::{fill, tr};
use crate::model::{Area, DistanceClass, Importance, Rule, RuleKind, WishKind};
use crate::score::RuleResult;

/// Attendee names by ID, for chips and pickers.
type Names = BTreeMap<usize, String>;

/// Selected chips and a searchable checklist, shared by new and existing wishes.
pub(super) fn person_picker(
    ui: &mut egui::Ui,
    salt: &str,
    names: &Names,
    filter: &mut String,
    requester: usize,
    selected: &mut Vec<usize>,
    allow_empty: bool,
) -> bool {
    let mut changed = false;
    ui.push_id(salt, |ui| {
        if !selected.is_empty() {
            ui.horizontal_wrapped(|ui| {
                let mut remove = None;
                for id in selected.iter() {
                    let name = names.get(id).map_or("", String::as_str);
                    if ui
                        .add_enabled(allow_empty || selected.len() > 1, theme::tag_button(name))
                        .on_hover_text(tr("Remove"))
                        .clicked()
                    {
                        remove = Some(*id);
                    }
                }
                if let Some(id) = remove {
                    selected.retain(|t| *t != id);
                    changed = true;
                }
            });
        }
        ui.add(
            theme::field(filter)
                .id(ui.make_persistent_id("search"))
                .hint_text(tr("Search people"))
                .desired_width(f32::INFINITY),
        )
        .widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, tr("Search people"))
        });
        let p = theme::palette(ui.ctx());
        theme::inset(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let needle = crate::app::search::Needle::new(filter);
            let mut matches: Vec<_> = names
                .iter()
                .filter(|(id, name)| **id != requester && needle.matches(name))
                .collect();
            matches.sort_by(|a, b| a.1.cmp(b.1));
            egui::ScrollArea::vertical()
                .id_salt("matches")
                .max_height(180.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if matches.is_empty() {
                        theme::hint(ui, p, tr("No matching people"));
                    }
                    for (id, name) in matches {
                        let mut checked = selected.contains(id);
                        let enabled = !checked || allow_empty || selected.len() > 1;
                        ui.push_id(id, |ui| {
                            if ui
                                .add_enabled_ui(enabled, |ui| {
                                    ui.add_sized(
                                        [ui.available_width(), theme::CONTROL_HEIGHT],
                                        egui::Checkbox::new(&mut checked, name),
                                    )
                                })
                                .inner
                                .changed()
                            {
                                if checked {
                                    selected.push(*id);
                                } else {
                                    selected.retain(|t| t != id);
                                }
                                changed = true;
                            }
                        });
                    }
                });
        });
    });
    changed
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
                    RuleKind::Proximity { .. } => wish_editor(ui, rule, &names, picker_filter),
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
    ui.horizontal_wrapped(|ui| {
        let changed = ui
            .checkbox(&mut rule.enabled, "")
            .on_hover_text(tr("Enabled"))
            .changed();
        ui.label(RichText::new(crate::i18n::tr_any(&rule.label)).strong());
        if let Some(result) = result {
            theme::status_pill(
                ui,
                p,
                &result.detail,
                theme::outcome_color(p, Some(result.outcome())),
            );
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
        match (*must_be, *hard) {
            (true, true) => "Must sit at {}",
            (false, true) => "Must not sit at {}",
            (true, false) => "Prefer {}",
            (false, false) => "Avoid {}",
        },
        &[&area_text],
    ));
    ui.horizontal_wrapped(|ui| {
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
            "No neighbour on one side. A wish at another table costs {}.",
            &[&WishKind::Seating.far_penalty()],
        ),
    );
    changed
}

/// Kind (wish or avec), who, importance and hard limit.
fn wish_editor(ui: &mut egui::Ui, rule: &mut Rule, names: &Names, filter: &mut String) -> bool {
    let RuleKind::Proximity {
        target,
        wish,
        importance,
        max_class,
        requester,
    } = &mut rule.kind
    else {
        return false;
    };
    let kind_changed = ui
        .horizontal_wrapped(|ui| {
            ui.selectable_value(wish, WishKind::Seating, tr("Seating wish"))
                .changed()
                | ui.selectable_value(wish, WishKind::Avec, tr("Avec"))
                    .changed()
        })
        .inner;
    let wish = *wish;
    let mut changed = kind_changed;
    changed |= target_editor(ui, target, *requester, names, filter);
    changed |= importance_editor(ui, importance);
    changed |= limit_editor(ui, max_class);
    if kind_changed {
        rule.label = wish.label().into();
    }
    changed
}

fn target_editor(
    ui: &mut egui::Ui,
    target: &mut Target,
    requester: usize,
    names: &Names,
    filter: &mut String,
) -> bool {
    if let Target::People { ids } = target {
        ui.label(tr("Near"));
        return person_picker(ui, "rule-picker", names, filter, requester, ids, false);
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(tr("Near"));
        let mut changed = false;
        match target {
            Target::People { .. } => unreachable!("people use the checklist above"),
            Target::Organization { name, .. } => {
                ui.label(tr("people from"));
                changed |= ui.add(theme::field(name).desired_width(140.0)).changed();
                theme::hint(
                    ui,
                    theme::palette(ui.ctx()),
                    tr("Everyone in this organization"),
                );
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

fn importance_editor(ui: &mut egui::Ui, importance: &mut Importance) -> bool {
    ui.horizontal_wrapped(|ui| {
        ui.label(tr("Importance"));
        let mut changed = false;
        for value in Importance::ALL {
            changed |= ui
                .selectable_value(importance, value, value.label())
                .changed();
        }
        changed
    })
    .inner
}

/// Optional hard maximum distance.
fn limit_editor(ui: &mut egui::Ui, max_class: &mut Option<DistanceClass>) -> bool {
    theme::responsive_row(ui, |ui| {
        ui.label(tr("Maximum distance"));
        let mut changed = false;
        theme::combo(
            "max",
            max_class
                .map(DistanceClass::limit_label)
                .unwrap_or(tr("No limit")),
        )
        .show_ui(ui, |ui| {
            changed |= ui.value(max_class, None, tr("No limit"));
            for class in DistanceClass::ALL
                .into_iter()
                .filter(|c| *c != DistanceClass::Far)
            {
                changed |= ui.value(max_class, Some(class), class.limit_label());
            }
        });
        changed
    })
    .inner
}
