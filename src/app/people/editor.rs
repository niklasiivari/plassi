//! Editing one person: a readable summary first, controls on demand.

use super::*;
use crate::i18n::tr;

impl PlassiApp {
    /// The whole editor for one attendee; saves once if anything changed.
    pub(super) fn person_editor(&mut self, ui: &mut egui::Ui, p: Palette, id: usize) {
        if self.people.scroll_to_person {
            ui.scroll_to_cursor(Some(egui::Align::Min));
            self.people.scroll_to_person = false;
        }
        self.person_header(ui, p, id);
        let mut changed = self.review_navigation(ui, p, id);
        changed |= self.sign_up_notes(ui, p, id);
        changed |= self.wishes_section(ui, p, id);
        self.wished_for_by(ui, p, id);
        ui.add_space(theme::space::XL);
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            changed |= self.person_details_block(ui, p, id);
        });
        if changed {
            self.commit(tr("Saved changes"));
        }
    }

    /// Name and affiliation, with tr("Show on chart") and a menu for the rest.
    fn person_header(&mut self, ui: &mut egui::Ui, p: Palette, id: usize) {
        let Some(person) = self.project.person(id).cloned() else {
            return;
        };
        theme::responsive_row(ui, |ui| {
            ui.label(RichText::new(&person.name).size(theme::text::PAGE).strong());
            ui.horizontal(|ui| {
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center).with_main_wrap(true),
                    |ui| {
                        egui::containers::menu::MenuButton::new(tr("More"))
                            .config(
                                egui::containers::menu::MenuConfig::new().style(theme::menu_style),
                            )
                            .ui(ui, |ui| {
                                if let Some(next) = self.next_to_review(id)
                                    && ui.button(tr("Next unreviewed person")).clicked()
                                {
                                    self.selected_person = Some(next);
                                    self.people.wish_draft = None;
                                    ui.close();
                                }
                                if ui.button(tr("Replace with someone else…")).clicked() {
                                    self.people.roster =
                                        Some(roster::RosterEdit::Replace(id, Default::default()));
                                    ui.close();
                                }
                                if ui.button(tr("Remove from event")).clicked() {
                                    self.remove_people(&[id]);
                                    ui.close();
                                }
                            });
                        if ui.button(tr("Show on chart")).clicked() {
                            self.page = crate::app::Page::Chart;
                            self.chart.view.focus_person = Some(id);
                        }
                    },
                );
            });
        });
        let affiliation = self.project.affiliation(&person);
        ui.label(RichText::new(affiliation).color(p.muted));
        ui.add_space(theme::space::L);
    }

    /// The person's wishes as readable rows, the new-wish form, and an Add button.
    fn wishes_section(&mut self, ui: &mut egui::Ui, p: Palette, id: usize) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            theme::section_title(ui, p, tr("Their wishes"));
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !self
                        .people
                        .wish_draft
                        .as_ref()
                        .is_some_and(|draft| draft.person == id)
                    {
                        changed |= self.add_rule_menu(ui, id);
                    }
                });
            });
        });
        if self
            .people
            .wish_draft
            .as_ref()
            .is_some_and(|d| d.person == id)
        {
            changed |= self.wish_draft_form(ui, p);
        }
        let results = self
            .evaluation()
            .map(|e| e.results.clone())
            .unwrap_or_default();
        let rule_ids: Vec<u64> = self
            .project
            .rules
            .iter()
            .filter(|r| r.owner() == id)
            .map(|r| r.id)
            .collect();
        if rule_ids.is_empty() && self.people.wish_draft.is_none() {
            theme::hint(ui, p, tr("No wishes. Uses the seating policy."));
        }
        let mut delete = None;
        for rule_id in rule_ids {
            let result = results.iter().find(|r| r.rule_id == rule_id);
            let (edited, deleted) = self.wish_row(ui, p, rule_id, result);
            changed |= edited;
            if deleted {
                delete = Some(rule_id);
            }
        }
        if let Some(rule_id) = delete {
            self.project.rules.retain(|r| r.id != rule_id);
            changed = true;
        }
        ui.add_space(theme::space::XL);
        changed
    }

    /// tr("+ Add") menu: a wish (opens the form) or a ready-made rule.
    fn add_rule_menu(&mut self, ui: &mut egui::Ui, id: usize) -> bool {
        let mut added = false;
        egui::containers::menu::MenuButton::new(tr("+ Add wish"))
            .config(egui::containers::menu::MenuConfig::new().style(theme::menu_style))
            .ui(ui, |ui| {
                if ui.button(tr("Sit near…")).clicked() {
                    self.start_wish(id);
                    ui.close();
                }
                let has_rule =
                    self.project.rules.iter().any(|r| {
                        r.owner() == id && matches!(r.kind, RuleKind::AvoidTableEnd { .. })
                    });
                if !has_rule && ui.button(tr("Not at a table end")).clicked() {
                    let rule = crate::model::Rule {
                        id: self.project.next_rule_id(),
                        label: "Not at a table end".into(),
                        source: String::new(),
                        enabled: true,
                        kind: RuleKind::AvoidTableEnd {
                            person: id,
                            end_penalty: 300,
                            near_end_penalty: 80,
                        },
                    };
                    self.project.rules.push(rule);
                    added = true;
                    ui.close();
                }
            });
        added
    }

    fn wished_for_by(&self, ui: &mut egui::Ui, p: Palette, id: usize) {
        theme::section_title(ui, p, tr("Wished for by"));
        let Some(attendee) = self.project.person(id) else {
            return;
        };
        let organizations = self
            .project
            .category_keys(crate::catalog::Kind::Organization, attendee);
        let mut any = false;
        for rule in self.project.rules.iter().filter(|r| r.enabled) {
            let RuleKind::Proximity {
                requester,
                target,
                wish,
                ..
            } = &rule.kind
            else {
                continue;
            };
            // Test this person's membership instead of expanding the entire
            // organization for every wish on every frame.
            let includes = *requester != id
                && match target {
                    Target::People { ids } => ids.contains(&id),
                    Target::Tag { tag, .. } => attendee.tags.contains(tag),
                    Target::Organization { name } => organizations.contains(
                        &self
                            .project
                            .category_key_of(crate::catalog::Kind::Organization, name),
                    ),
                };
            if !includes {
                continue;
            }
            let Some(person) = self.project.person(*requester) else {
                continue;
            };
            any = true;
            let kind = match wish {
                crate::model::WishKind::Avec => tr("avec"),
                crate::model::WishKind::Seating => tr("wish"),
            };
            ui.horizontal(|ui| {
                ui.label(&person.name);
                theme::hint(ui, p, kind);
            });
        }
        if !any {
            theme::hint(ui, p, tr("Nobody"));
        }
    }

    /// The next person after `current` (wrapping) with unreviewed sign-up text.
    pub(super) fn next_to_review(&self, current: usize) -> Option<usize> {
        if let Some(queue) = &self.people.review
            && let Some(pos) = queue.iter().position(|p| *p == current)
        {
            return queue[pos + 1..]
                .iter()
                .copied()
                .find(|id| self.project.person(*id).is_some());
        }
        let attendees = &self.project.attendees;
        let pos = attendees.iter().position(|a| a.id == current)?;
        attendees[pos + 1..]
            .iter()
            .chain(&attendees[..pos])
            .map(|a| a.id)
            .find(|id| {
                self.project
                    .notes
                    .iter()
                    .any(|n| n.person == *id && !n.reviewed)
            })
    }
}
