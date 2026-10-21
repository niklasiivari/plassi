//! The side panel for one person: seat, status, pins, wishes and who wished for them.

use super::inspector::result_row;
use super::*;

impl PlassiApp {
    /// Everything about the selected person on this chart.
    pub(super) fn person_details(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        scene: &Scene,
        id: usize,
    ) {
        self.person_heading(ui, p, scene, id);
        if self.chart.single_pane && ui.button(tr("Edit wishes")).clicked() {
            self.page = crate::app::Page::People;
        }
        ui.add_space(theme::space::L);
        if self.tags_editor(ui, p, &[id]) {
            self.commit(tr("Saved changes"));
        }
        ui.add_space(theme::space::L);
        self.pin_buttons(ui, p, scene, id);
        self.suggestions_ui(ui, p, id);
        self.group_ui(ui, p, id);
        ui.add_space(theme::space::XL);
        let mut opened_wish = false;
        ui.horizontal_wrapped(|ui| {
            theme::section_title(ui, p, tr("Their wishes"));
            if !self
                .people
                .wish_draft
                .as_ref()
                .is_some_and(|draft| draft.person == id)
                && ui.add(theme::secondary_button(tr("+ Add wish"))).clicked()
            {
                self.start_wish(id);
                opened_wish = true;
            }
        });
        if self
            .people
            .wish_draft
            .as_ref()
            .is_some_and(|draft| draft.person == id)
            && self.wish_draft_form(ui, p)
        {
            self.commit(tr("Saved changes"));
        }
        if opened_wish {
            ui.scroll_to_cursor(Some(egui::Align::Max));
        }
        match scene.own.get(&id) {
            Some(results) => results
                .iter()
                .for_each(|(_, result)| result_row(ui, p, result, scene.judged(id, result))),
            None => {
                ui.label(RichText::new(tr("None")).color(p.muted));
            }
        }
        self.person_preferences(ui, p, scene, id);
        ui.add_space(theme::space::XL);
        self.wished_by_list(ui, p, scene, id);
        self.sign_up_text(ui, p, id);
    }

    /// Name, organization, seat and wish status, with close and focus buttons.
    fn person_heading(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene, id: usize) {
        let Some(attendee) = self.project.person(id).cloned() else {
            return;
        };
        ui.horizontal(|ui| {
            theme::section_title(ui, p, tr("Person"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::icon_button(ui, p, theme::Icon::Close, tr("Deselect"), false).clicked() {
                    self.selected_person = None;
                }
                if theme::icon_button(ui, p, theme::Icon::Focus, tr("Show on chart"), false)
                    .clicked()
                {
                    self.focus_person(id, scene);
                    self.chart.mobile_view = MobileChart::Chart;
                }
            });
        });
        ui.label(
            RichText::new(&attendee.name)
                .size(theme::text::TITLE)
                .strong(),
        );
        ui.label(RichText::new(self.project.affiliation(&attendee)).color(p.muted));
        ui.horizontal_wrapped(|ui| {
            let place = match scene.seat_of.get(&id) {
                Some(seat) => self.project.seat_label(seat),
                None => tr("Not seated").into(),
            };
            theme::pill(ui, place, p.text, p.surface_hi);
            // Without a seat, "Not seated" says it all.
            if !scene.seat_of.contains_key(&id) {
                return;
            }
            let mut status = scene.status(id);
            if scene.waits(id) && status.is_none_or(|s| s == Outcome::Met) {
                status = None;
            }
            let text = match status {
                None if scene.waits(id) => tr("Waiting for people not seated"),
                Some(Outcome::Met) => tr("All wishes met"),
                Some(Outcome::Partial) => tr("Partly met"),
                Some(Outcome::Broken) => tr("Hard rule broken"),
                None => tr("No wishes"),
            };
            theme::status_pill(ui, p, text, theme::outcome_color(p, status));
        });
    }

    /// Pin or unpin; a seated person can also be kept at their table, and
    /// someone without a seat at a table chosen for them.
    fn pin_buttons(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene, id: usize) {
        if let Some(Pin::Table(table)) = scene.pins.get(&id) {
            theme::hint(
                ui,
                p,
                fill("Kept at {}", &[&self.project.table_label(table)]),
            );
        }
        let seat = scene.seat_of.get(&id).cloned();
        ui.horizontal_wrapped(|ui| match (scene.pins.get(&id), seat) {
            (Some(_), _) => {
                if ui.add(theme::secondary_button(tr("Unpin"))).clicked() {
                    self.set_pin(id, None);
                }
            }
            (None, Some(seat)) => {
                if ui
                    .add(theme::secondary_button(tr("Pin to this seat")))
                    .on_hover_text(tr("Keep this seat when solving"))
                    .clicked()
                {
                    self.set_pin(id, Some(Pin::Seat(seat.clone())));
                }
                if ui
                    .add(theme::secondary_button(tr("Keep at this table")))
                    .on_hover_text(tr("The solver may change their seats at this table"))
                    .clicked()
                {
                    self.keep_at_table(&[id], crate::model::seat_table(&seat));
                }
            }
            (None, None) => {
                if let Some(table) = self.keep_at_table_menu(ui, &[id]) {
                    self.keep_at_table(&[id], &table);
                }
            }
        });
    }

    /// People who wished to sit near this person, coloured by how well that wish is met.
    fn wished_by_list(&self, ui: &mut egui::Ui, p: Palette, scene: &Scene, id: usize) {
        theme::section_title(ui, p, tr("Wished for by"));
        let mut any = false;
        for rule in self.project.rules.iter().filter(|r| r.enabled) {
            let RuleKind::Proximity {
                requester, target, ..
            } = &rule.kind
            else {
                continue;
            };
            if !self.project.target_ids(*requester, target).contains(&id) {
                continue;
            }
            any = true;
            let name = self.project.person_name(*requester);
            let result = scene
                .own
                .get(requester)
                .and_then(|rs| rs.iter().find(|(r, _)| *r == rule.id))
                .filter(|(_, r)| scene.judged(*requester, r));
            let color = theme::outcome_color(p, result.map(|(_, r)| r.outcome()));
            ui.horizontal(|ui| {
                theme::status_dot(ui, color);
                ui.label(name);
            });
        }
        if !any {
            ui.label(RichText::new(tr("Nobody")).color(p.muted));
        }
    }

    fn sign_up_text(&self, ui: &mut egui::Ui, p: Palette, id: usize) {
        let notes: Vec<_> = self
            .project
            .notes
            .iter()
            .filter(|n| n.person == id)
            .collect();
        if notes.is_empty() {
            return;
        }
        ui.add_space(theme::space::XL);
        theme::section_title(ui, p, tr("Original sign-up notes"));
        for note in notes {
            theme::inset(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(format!("{:?}", note.field))
                        .size(theme::text::CAPTION)
                        .color(p.muted),
                );
                ui.label(&note.text);
                if !note.reviewed {
                    ui.label(
                        RichText::new(tr("Unreviewed"))
                            .size(theme::text::CAPTION)
                            .color(p.warn),
                    );
                }
            });
        }
    }

    pub(super) fn hover_card(
        &self,
        ui: &mut egui::Ui,
        p: Palette,
        scene: &Scene,
        person: usize,
        seat: Option<&str>,
        footer: &str,
    ) {
        let Some(attendee) = self.project.person(person) else {
            return;
        };
        ui.set_max_width(300.0);
        ui.label(
            RichText::new(&attendee.name)
                .strong()
                .size(theme::text::LEAD),
        );
        ui.label(
            RichText::new(self.project.affiliation(attendee))
                .color(p.muted)
                .size(theme::text::SMALL),
        );
        let place = match seat {
            Some(seat) => self.project.seat_label(seat),
            None => tr("Not seated").into(),
        };
        theme::hint(ui, p, place);
        if let Some(results) = scene.own.get(&person) {
            ui.separator();
            for (_, result) in results {
                result_row(ui, p, result, scene.judged(person, result));
            }
        } else {
            theme::hint(ui, p, tr("No wishes"));
        }
        ui.label(
            RichText::new(footer)
                .size(theme::text::CAPTION)
                .color(p.muted),
        );
    }
}
