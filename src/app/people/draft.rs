//! The form for adding a wish by hand: near people, near everyone with a
//! tag, or near an organization.

use super::*;
use crate::catalog::Kind;
use crate::i18n::tr;

impl PlassiApp {
    /// The new-wish form. Returns true when a rule was created.
    pub(super) fn wish_draft_form(&mut self, ui: &mut egui::Ui, p: Palette) -> bool {
        let names = self.person_names();
        let tags = self.all_tags();
        let organizations = self.organization_names();
        let Some(draft) = self.people.wish_draft.as_mut() else {
            return false;
        };
        let mut created = false;
        let mut cancel = false;
        theme::raised_card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(tr("New wish"))
                    .size(theme::text::LEAD)
                    .strong(),
            );
            ui.add_space(theme::space::S);
            draft_kind(ui, draft);
            ui.add_space(theme::space::S);
            match &mut draft.near {
                DraftTarget::People => {
                    people_targets(ui, p, draft, &names, &mut self.people.picker_filter)
                }
                DraftTarget::Tag(tag) => choice(
                    ui,
                    tr("Tag"),
                    tag,
                    &tags,
                    tr("Add tags under Details first."),
                ),
                DraftTarget::Organization(org) => choice(
                    ui,
                    tr("Organization"),
                    org,
                    &organizations,
                    tr("No organizations"),
                ),
            }
            ui.add(
                theme::field(&mut draft.source)
                    .hint_text(tr("Original note (optional)"))
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(theme::space::S);
            ui.horizontal(|ui| {
                created = ui
                    .add_enabled(draft_ready(draft), theme::primary_button(tr("Add wish"), p))
                    .clicked();
                cancel = ui.button(tr("Cancel")).clicked();
            });
        });
        if cancel {
            self.people.wish_draft = None;
        }
        if created && let Some(draft) = self.people.wish_draft.take() {
            self.create_wish(draft);
        }
        created
    }

    fn create_wish(&mut self, draft: WishDraft) {
        let id = self.project.next_rule_id();
        let mut rule =
            crate::import::wish_rule(id, draft.person, draft.targets, draft.avec, &draft.source);
        if let RuleKind::Proximity { target, .. } = &mut rule.kind {
            match draft.near {
                DraftTarget::People => {}
                DraftTarget::Tag(tag) => *target = Target::Tag { tag, required: 1 },
                DraftTarget::Organization(name) => {
                    *target = Target::Organization { name, required: 1 }
                }
            }
        }
        self.project.rules.push(rule);
        if let Some(note) = draft.note.and_then(|n| self.project.notes.get_mut(n)) {
            note.reviewed = true;
        }
    }

    fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .project
            .attendees
            .iter()
            .flat_map(|a| a.tags.iter().cloned())
            .collect();
        tags.sort();
        tags.dedup();
        tags
    }

    /// Category names, plus spellings not mapped to a category yet.
    fn organization_names(&self) -> Vec<String> {
        let kind = Kind::Organization;
        let mut names: Vec<String> = self
            .project
            .catalog
            .list(kind)
            .iter()
            .map(|c| c.name.clone())
            .collect();
        for (spelling, _) in self.project.spellings(kind) {
            if self.project.catalog.find(kind, &spelling).is_none() {
                names.push(spelling);
            }
        }
        names
    }
}

/// Avec or wish, and whether it is about people, a tag or an organization.
fn draft_kind(ui: &mut egui::Ui, draft: &mut WishDraft) {
    ui.horizontal(|ui| {
        ui.selectable_value(&mut draft.avec, false, tr("Seating wish"));
        ui.selectable_value(&mut draft.avec, true, tr("Avec"));
    });
    if draft.avec {
        draft.near = DraftTarget::People;
        return;
    }
    ui.horizontal(|ui| {
        ui.label(tr("Near"));
        let is = |d: &DraftTarget, n: u8| {
            matches!(
                (d, n),
                (DraftTarget::People, 0)
                    | (DraftTarget::Tag(_), 1)
                    | (DraftTarget::Organization(_), 2)
            )
        };
        if ui
            .selectable_label(is(&draft.near, 0), tr("People"))
            .clicked()
        {
            draft.near = DraftTarget::People;
        }
        if ui.selectable_label(is(&draft.near, 1), tr("Tag")).clicked() && !is(&draft.near, 1) {
            draft.near = DraftTarget::Tag(String::new());
        }
        if ui
            .selectable_label(is(&draft.near, 2), tr("Organization"))
            .clicked()
            && !is(&draft.near, 2)
        {
            draft.near = DraftTarget::Organization(String::new());
        }
    });
}

fn people_targets(
    ui: &mut egui::Ui,
    p: Palette,
    draft: &mut WishDraft,
    names: &std::collections::BTreeMap<usize, String>,
    filter: &mut String,
) {
    ui.horizontal_wrapped(|ui| {
        let mut remove = None;
        for t in &draft.targets {
            let name = names.get(t).cloned().unwrap_or_default();
            if ui.add(theme::tag_button(&name)).clicked() {
                remove = Some(*t);
            }
        }
        draft.targets.retain(|x| Some(*x) != remove);
        if let Some(t) = person_picker(ui, "draft-picker", names, filter, draft.person)
            && !draft.targets.contains(&t)
        {
            draft.targets.push(t);
        }
    });
    if draft.targets.len() > 1 {
        theme::hint(ui, p, tr("Sit near all selected people."));
    }
}

/// A dropdown of known values.
fn choice(ui: &mut egui::Ui, label: &str, value: &mut String, options: &[String], empty: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        if options.is_empty() {
            ui.label(empty);
            return;
        }
        let shown = if value.is_empty() {
            tr("Choose…")
        } else {
            value.as_str()
        };
        theme::combo(("draft-choice", label), shown).show_ui(ui, |ui| {
            for option in options {
                ui.selectable_value(value, option.clone(), option);
            }
        });
    });
}

fn draft_ready(draft: &WishDraft) -> bool {
    match &draft.near {
        DraftTarget::People => !draft.targets.is_empty(),
        DraftTarget::Tag(value) | DraftTarget::Organization(value) => !value.trim().is_empty(),
    }
}
