//! Direct tag editing shared by the guest inspector and bulk selection.

use super::*;

impl PlassiApp {
    /// Applies each action to all selected guests. The caller saves one undo step.
    pub(in crate::app) fn tags_editor(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        people: &[usize],
    ) -> bool {
        let mut changed = false;
        ui.push_id(("guest-tags", people), |ui| {
            theme::section_title(ui, p, tr("Tags"));
            let tags = self.all_tags();
            let selected: Vec<_> = self
                .project
                .attendees
                .iter()
                .filter(|a| people.contains(&a.id))
                .collect();
            let mut remove = None;
            ui.horizontal_wrapped(|ui| {
                for tag in &tags {
                    let count = selected
                        .iter()
                        .filter(|a| {
                            a.tags.contains(tag)
                                && **tag != a.organization
                                && **tag != a.participant_group
                        })
                        .count();
                    if count == 0 {
                        continue;
                    }
                    let label = if people.len() > 1 {
                        format!("{tag} ({count}/{})", selected.len())
                    } else {
                        tag.clone()
                    };
                    if ui
                        .add(theme::tag_button(&label).wrap())
                        .on_hover_text(tr("Remove tag from selected guests"))
                        .clicked()
                    {
                        remove = Some(tag.clone());
                    }
                }
            });
            let input_id = ui.id().with("input");
            let mut input = ui
                .ctx()
                .data_mut(|d| d.get_temp::<String>(input_id).unwrap_or_default());
            let mut add = None;
            ui.horizontal(|ui| {
                let button_width = ui
                    .painter()
                    .layout_no_wrap(
                        tr("Add tag").into(),
                        egui::TextStyle::Button.resolve(ui.style()),
                        p.text,
                    )
                    .size()
                    .x
                    + 2.0 * ui.spacing().button_padding.x
                    + 4.0;
                let width =
                    (ui.available_width() - button_width - ui.spacing().item_spacing.x).max(40.0);
                let edit = ui.add(
                    theme::field(&mut input)
                        .id(input_id)
                        .hint_text(tr("Tag name"))
                        .desired_width(width),
                );
                edit.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, tr("Tag name"))
                });
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui
                    .add_enabled(!input.trim().is_empty(), theme::small_button(tr("Add tag")))
                    .clicked()
                    || enter && !input.trim().is_empty()
                {
                    add = Some(input.trim().to_string());
                    input.clear();
                    edit.request_focus();
                }
            });
            ui.ctx().data_mut(|d| d.insert_temp(input_id, input));
            let choices: Vec<_> = tags
                .iter()
                .filter(|tag| selected.iter().any(|a| !a.tags.contains(tag)))
                .collect();
            if !choices.is_empty() {
                theme::combo("existing-tag", tr("Use existing tag…"))
                    .width(ui.available_width().min(240.0))
                    .show_ui(ui, |ui| {
                        for tag in choices {
                            if ui.action(false, tag) {
                                add = Some(tag.clone());
                            }
                        }
                    });
            }
            // Reuse existing spelling: tag wishes match exact strings.
            let add = add.map(|name| {
                tags.iter()
                    .find(|tag| tag.eq_ignore_ascii_case(&name))
                    .cloned()
                    .unwrap_or(name)
            });
            for person in self
                .project
                .attendees
                .iter_mut()
                .filter(|a| people.contains(&a.id))
            {
                if let Some(tag) = &remove
                    && *tag != person.organization
                    && *tag != person.participant_group
                {
                    let before = person.tags.len();
                    person.tags.retain(|t| t != tag);
                    changed |= before != person.tags.len();
                }
                if let Some(tag) = &add
                    && !person.tags.contains(tag)
                {
                    person.tags.push(tag.clone());
                    changed = true;
                }
            }
        });
        changed
    }
}
