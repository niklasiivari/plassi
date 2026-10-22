//! The categories themselves: name, how many people, and the spellings that map to them.

use super::*;
use crate::i18n::tr;

impl PlassiApp {
    pub(super) fn category_entries(&mut self, ui: &mut egui::Ui, p: Palette, kind: Kind) -> bool {
        theme::section_title(ui, p, kind.plural());
        if self.project.catalog.list(kind).is_empty() {
            theme::hint(ui, p, tr("No categories yet"));
            return false;
        }
        let lookup = self.project.catalog.lookup(kind);
        let spellings = self.project.spellings(kind);
        let mut counts = vec![0; self.project.catalog.list(kind).len()];
        for (spelling, people) in &spellings {
            for &i in lookup.find_all(spelling) {
                counts[i] += people;
            }
        }
        // Each spelling with the categories it is in.
        let spellings: Vec<(String, Vec<usize>)> = spellings
            .into_iter()
            .map(|(s, _)| {
                let found = lookup.find_all(&s).to_vec();
                (s, found)
            })
            .collect();
        let mut changed = false;
        let mut delete = None;
        let mut show_people = None;
        for (index, category) in self.project.catalog.list_mut(kind).iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                theme::card(p).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        changed |= ui
                            .add(
                                theme::field(&mut category.name)
                                    .desired_width(ui.available_width().min(260.0)),
                            )
                            .lost_focus();
                        if ui
                            .link(super::unmapped::people_count(counts[index]))
                            .on_hover_text(tr("Show people with this entry"))
                            .clicked()
                        {
                            show_people = Some(index);
                        }
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
                        changed |= theme::price_input(ui, tr("Price"), &mut category.price_cents);
                    }
                    if matches!(kind, Kind::Food | Kind::Drink) {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(tr("Print mark"));
                            changed |= ui
                                .add(theme::field(&mut category.mark).desired_width(60.0))
                                .on_hover_text(tr("On the kitchen and bar prints, e.g. VG"))
                                .lost_focus();
                        });
                    }
                    changed |= alias_chips(ui, p, &mut category.aliases);
                    changed |= add_spelling(ui, category, index, &spellings);
                });
            });
            ui.add_space(theme::space::S);
        }
        if let Some(index) = show_people {
            let ids = self
                .project
                .attendees
                .iter()
                .filter(|a| lookup.find_all(kind.raw(a)).contains(&index))
                .map(|a| a.id)
                .collect();
            self.categories.entry_people =
                Some((self.project.catalog.list(kind)[index].name.clone(), ids));
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
        theme::hint(ui, p, tr("Other spellings"));
        for (i, alias) in aliases.iter().enumerate() {
            let chip = theme::tag_button(alias);
            if ui
                .add(chip)
                .on_hover_text(tr("Remove from category"))
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
    index: usize,
    spellings: &[(String, Vec<usize>)],
) -> bool {
    let options: Vec<&String> = spellings
        .iter()
        .filter(|(_, found)| !found.contains(&index))
        .map(|(s, _)| s)
        .collect();
    if options.is_empty() {
        return false;
    }
    let mut picked = None;
    theme::combo("add-spelling", tr("+ Add spelling")).show_ui(ui, |ui| {
        for spelling in options {
            if ui.action(false, spelling) {
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
