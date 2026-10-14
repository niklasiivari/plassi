//! A table's summary in the side panel: who sits there, their wishes, the
//! mix of categories and the allergies.

use super::*;
use crate::model::Target;

impl Scene {
    /// The table's body in world space, from its top seat to its bottom seat.
    pub(super) fn table_body(&self, table_id: &str) -> Option<Rect> {
        let seats = self.seats.iter().filter(|s| s.table_id == table_id);
        let (min_x, min_y, max_y) = seats.fold(None, |acc: Option<(f32, f32, f32)>, s| {
            Some(acc.map_or((s.x, s.y, s.y), |(x, lo, hi)| {
                (x.min(s.x), lo.min(s.y), hi.max(s.y))
            }))
        })?;
        Some(Rect::from_min_max(
            Pos2::new(min_x, min_y - CARD_H / 2.0 - 6.0),
            Pos2::new(min_x + TABLE_SPAN, max_y + CARD_H / 2.0 + 6.0),
        ))
    }

    /// The table whose body or name covers a world point.
    pub(super) fn table_at(&self, project: &Project, world: Pos2) -> Option<String> {
        let tables = project.layout.rows.iter().flat_map(|r| &r.tables);
        tables
            .filter(|t| {
                self.table_body(&t.id).is_some_and(|body| {
                    let with_name = Rect::from_min_max(body.min - Vec2::new(0.0, 28.0), body.max);
                    with_name.contains(world)
                })
            })
            .map(|t| t.id.clone())
            .next()
    }

    /// People seated at a table, in seat order.
    fn people_at(&self, table_id: &str) -> Vec<usize> {
        self.seats
            .iter()
            .filter(|s| s.table_id == table_id)
            .filter_map(|s| self.occupant.get(&s.id).copied())
            .collect()
    }
}

impl PlassiApp {
    /// The table's name as the chart shows it, or "Table 2" by its place in the room.
    pub(super) fn table_title(&self, table_id: &str) -> String {
        let label = self.project.table_label(table_id).trim();
        if !label.is_empty() && label != table_id {
            return label.to_string();
        }
        let tables = self.project.layout.rows.iter().flat_map(|r| &r.tables);
        let number = tables.take_while(|t| t.id != table_id).count() + 1;
        fill("Table {}", &[&number])
    }

    pub(super) fn table_summary(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        scene: &Scene,
        table_id: &str,
    ) {
        let people = scene.people_at(table_id);
        let seats = scene
            .seats
            .iter()
            .filter(|s| s.table_id == table_id)
            .count();
        ui.horizontal(|ui| {
            theme::section_title(ui, p, tr("Table"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::icon_button(ui, p, theme::Icon::Close, tr("Deselect"), false).clicked() {
                    self.chart.selected_table = None;
                }
            });
        });
        ui.label(
            RichText::new(self.table_title(table_id))
                .size(theme::text::TITLE)
                .strong(),
        );
        theme::hint(
            ui,
            p,
            fill("{} of {} seats taken", &[&people.len(), &seats]),
        );
        ui.add_space(theme::space::L);
        table_stats(ui, p, scene, &people);
        ui.add_space(theme::space::XL);
        self.table_people(ui, p, scene, &people);
        self.wishes_away(ui, p, scene, table_id, &people);
        for kind in Kind::ALL {
            self.table_mix(ui, p, kind, &people);
        }
        self.table_allergies(ui, p, &people);
    }

    /// Everyone at the table with how their wishes fare; a click selects them.
    fn table_people(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene, people: &[usize]) {
        theme::section_title(ui, p, tr("People"));
        if people.is_empty() {
            theme::hint(ui, p, tr("Nobody sits here yet"));
            return;
        }
        theme::text_rows(ui, |ui| {
            for &id in people {
                let Some(name) = self.project.person(id).map(|a| a.name.clone()) else {
                    continue;
                };
                ui.horizontal(|ui| {
                    theme::status_dot(ui, theme::outcome_color(p, scene.status(id)));
                    let label = egui::Label::new(RichText::new(name).size(theme::text::SMALL))
                        .sense(Sense::click());
                    if ui
                        .add(label)
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.chart.selected_table = None;
                        self.selected_person = Some(id);
                    }
                });
            }
        });
    }

    /// Wishes from people here for people seated at another table.
    fn wishes_away(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        scene: &Scene,
        table_id: &str,
        people: &[usize],
    ) {
        let table_of = |person: usize| {
            let seat = scene.seat_of.get(&person)?;
            scene.seat(seat).map(|s| s.table_id.clone())
        };
        let mut away: Vec<(usize, usize, String)> = Vec::new();
        for rule in self.project.rules.iter().filter(|r| r.enabled) {
            let RuleKind::Proximity {
                requester,
                target: Target::People { ids },
                ..
            } = &rule.kind
            else {
                continue;
            };
            if !people.contains(requester) {
                continue;
            }
            for &id in ids {
                if let Some(other) = table_of(id).filter(|t| t != table_id) {
                    away.push((*requester, id, other));
                }
            }
        }
        if away.is_empty() {
            return;
        }
        ui.add_space(theme::space::L);
        theme::section_title(ui, p, tr("Wishes to other tables"));
        theme::text_rows(ui, |ui| {
            for (from, to, table) in away {
                let name = |id| self.project.person(id).map_or("", |a| a.name.as_str());
                let text = fill(
                    "{} → {} at {}",
                    &[&name(from), &name(to), &self.table_title(&table)],
                );
                let label =
                    egui::Label::new(RichText::new(text).size(theme::text::SMALL).color(p.warn))
                        .sense(Sense::click());
                if ui
                    .add(label)
                    .on_hover_text(tr("Show who they wished for"))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    self.chart.selected_table = None;
                    self.selected_person = Some(to);
                    self.chart.view.focus_person = Some(to);
                }
            }
        });
    }

    /// How many at the table belong to each category of `kind`, most first.
    fn table_mix(&self, ui: &mut egui::Ui, p: Palette, kind: Kind, people: &[usize]) {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for attendee in people.iter().filter_map(|id| self.project.person(*id)) {
            // A spelling such as "ABC/XYZ" counts in every category it means.
            let mut names = self.project.category_names(kind, attendee);
            if names.is_empty() {
                names.push(kind.raw(attendee).trim().to_string());
            }
            for name in names.into_iter().filter(|n| !n.is_empty()) {
                match counts.iter_mut().find(|(n, _)| *n == name) {
                    Some((_, count)) => *count += 1,
                    None => counts.push((name, 1)),
                }
            }
        }
        if counts.is_empty() {
            return;
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        ui.add_space(theme::space::L);
        theme::section_title(ui, p, kind.plural());
        let text = counts
            .iter()
            .map(|(name, count)| format!("{name} {count}"))
            .collect::<Vec<_>>()
            .join(" · ");
        ui.label(RichText::new(text).size(theme::text::SMALL));
    }

    /// Allergies of the people at the table, for the kitchen.
    fn table_allergies(&self, ui: &mut egui::Ui, p: Palette, people: &[usize]) {
        let allergies: Vec<(&str, &str)> = people
            .iter()
            .filter_map(|id| self.project.person(*id))
            .map(|a| (a.name.as_str(), crate::print::allergy_text(&a.allergies)))
            .filter(|(_, text)| !text.is_empty())
            .collect();
        if allergies.is_empty() {
            return;
        }
        ui.add_space(theme::space::L);
        theme::section_title(ui, p, tr("Allergies"));
        theme::text_rows(ui, |ui| {
            for (name, text) in allergies {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(name).size(theme::text::SMALL));
                    ui.label(RichText::new(text).size(theme::text::SMALL).color(p.bad));
                });
            }
        });
    }
}

/// Wishes met and hard rules broken among the people at the table.
fn table_stats(ui: &mut egui::Ui, p: Palette, scene: &Scene, people: &[usize]) {
    let statuses: Vec<Outcome> = people.iter().filter_map(|id| scene.status(*id)).collect();
    let met = statuses.iter().filter(|s| **s == Outcome::Met).count();
    let broken = statuses.iter().filter(|s| **s == Outcome::Broken).count();
    ui.columns(2, |cols| {
        let met_color = if !statuses.is_empty() && met == statuses.len() {
            p.good
        } else {
            p.text
        };
        inspector::stat_tile(
            &mut cols[0],
            p,
            tr("Wishes met"),
            format!("{met}/{}", statuses.len()),
            met_color,
        )
        .on_hover_text(tr("People here whose wishes are all met"));
        inspector::stat_tile(
            &mut cols[1],
            p,
            tr("Hard rules broken"),
            broken.to_string(),
            if broken == 0 { p.good } else { p.bad },
        )
        .on_hover_text(tr("People here with a hard rule broken"));
    });
}
