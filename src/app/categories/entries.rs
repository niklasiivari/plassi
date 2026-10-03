//! The categories themselves: name, how many people, and the spellings that map to them.

use super::*;
use crate::i18n::tr;

impl PlassiApp {
    pub(super) fn category_entries(&mut self, ui: &mut egui::Ui, p: Palette, kind: Kind) -> bool {
        theme::section_title(ui, p, kind.plural());
        if self.project.catalog.list(kind).is_empty() {
            theme::hint(
                ui,
                p,
                tr("No categories yet. Create them from the spellings that aren't mapped."),
            );
            return false;
        }
        let counts: Vec<usize> = (0..self.project.catalog.list(kind).len())
            .map(|i| {
                self.project
                    .attendees
                    .iter()
                    .filter(|a| self.project.catalog.find(kind, kind.raw(a)) == Some(i))
                    .count()
            })
            .collect();
        let spellings: Vec<String> = self
            .project
            .spellings(kind)
            .into_iter()
            .map(|(s, _)| s)
            .collect();
        let mut changed = false;
        let mut delete = None;
        for (index, category) in self.project.catalog.list_mut(kind).iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                theme::card(p).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        changed |= ui
                            .add(theme::field(&mut category.name).desired_width(260.0))
                            .lost_focus();
                        theme::hint(ui, p, super::unmapped::people_count(counts[index]));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(theme::ghost_button(tr("Delete"), p))
                                .on_hover_text(tr("Its spellings become unmapped"))
                                .clicked()
                            {
                                delete = Some(index);
                            }
                        });
                    });
                    if kind != Kind::Organization {
                        ui.horizontal(|ui| {
                            let mut priced = category.price_cents.is_some();
                            if ui.checkbox(&mut priced, tr("Price contribution")).changed() {
                                category.price_cents = priced.then_some(0);
                                changed = true;
                            }
                            if let Some(cents) = &mut category.price_cents {
                                let mut euros = f64::from(*cents) / 100.0;
                                if ui.add(egui::DragValue::new(&mut euros)
                                    .speed(0.1).range(0.0..=1_000_000.0)
                                    .fixed_decimals(2).suffix(" €")).changed() {
                                    *cents = (euros * 100.0).round() as u32;
                                    changed = true;
                                }
                            }
                        });
                        theme::hint(ui, p, tr("Added to the ticket, food and drink total. Set 0 for included choices."));
                    }
                    changed |= alias_chips(ui, p, &mut category.aliases);
                    changed |= add_spelling(ui, category, &spellings);
                });
            });
            ui.add_space(theme::space::S);
        }
        if let Some(index) = delete {
            self.project.catalog.remove(kind, index);
            changed = true;
        }
        changed
    }
}

/// The spellings mapped to a category; × unmaps one.
fn alias_chips(ui: &mut egui::Ui, p: Palette, aliases: &mut Vec<String>) -> bool {
    if aliases.is_empty() {
        return false;
    }
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        theme::hint(ui, p, tr("Also written as"));
        for (i, alias) in aliases.iter().enumerate() {
            let chip = theme::tag_button(alias);
            if ui
                .add(chip)
                .on_hover_text(tr("Unmap this spelling"))
                .clicked()
            {
                remove = Some(i);
            }
        }
    });
    if let Some(i) = remove {
        aliases.remove(i);
    }
    remove.is_some()
}

/// Adds a spelling to this category, even one that already belongs to another
/// (so "ABC/XYZ" can count as both).
fn add_spelling(
    ui: &mut egui::Ui,
    category: &mut crate::catalog::Category,
    spellings: &[String],
) -> bool {
    let options: Vec<&String> = spellings.iter().filter(|s| !category.matches(s)).collect();
    if options.is_empty() {
        return false;
    }
    let mut picked = None;
    theme::combo("add-spelling", tr("+ Add spelling")).show_ui(ui, |ui| {
        for spelling in options {
            if ui.selectable_label(false, spelling).clicked() {
                picked = Some(spelling.clone());
            }
        }
    });
    if let Some(spelling) = picked {
        category.aliases.push(spelling);
        return true;
    }
    false
}
