//! The searchable, filterable attendee list.

use super::*;
use crate::i18n::{fill, tr};
use std::collections::{HashMap, HashSet};

use crate::model::Attendee;

/// What a list row shows besides the name.
struct RowInfo {
    subtitle: String,
    color_key: String,
    unreviewed: usize,
    selected: bool,
}

impl PlassiApp {
    pub(super) fn people_list(&mut self, ui: &mut egui::Ui, p: Palette) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(tr("People & wishes"))
                    .size(theme::text::TITLE)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(theme::secondary_button(tr("+ Add person")))
                    .on_hover_text(tr("Add a person"))
                    .clicked()
                {
                    self.people.roster = Some(roster::RosterEdit::Add(Default::default()));
                }
            });
        });
        ui.add_space(theme::space::M);
        if self.project.attendees.is_empty() {
            theme::hint(ui, p, tr("Paste a sheet or add people."));
            return;
        }
        ui.add(
            theme::field(&mut self.people.search)
                .hint_text(tr("Search name or organization"))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(theme::space::XS);
        self.filter_chips(ui, p);
        self.bulk_bar(ui, p);
        ui.add_space(theme::space::XS);
        ui.separator();
        let active = self.project.active_people();
        let pinned: HashSet<usize> = self.project.pins().into_keys().collect();
        let unmet = self.unmet_people();
        let seated: Option<HashSet<usize>> = self
            .project
            .candidates
            .get(self.selected_candidate)
            .map(|c| c.assignment.keys().copied().collect());
        let needle = crate::app::search::Needle::new(&self.people.search);
        let mut select = None;
        let mut pick = None;
        let mut shown = Vec::new();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for person in &self.project.attendees {
                    let unreviewed = self.unreviewed_notes(person.id);
                    let keep = match self.people.filter {
                        PeopleFilter::All => true,
                        PeopleFilter::ToReview => unreviewed > 0,
                        PeopleFilter::WithWishes => active.contains(&person.id),
                        PeopleFilter::Unconstrained => !active.contains(&person.id),
                        PeopleFilter::Unmet => unmet.contains(&person.id),
                        PeopleFilter::NotSeated => {
                            seated.as_ref().is_some_and(|s| !s.contains(&person.id))
                        }
                        PeopleFilter::Pinned => pinned.contains(&person.id),
                    };
                    let matches = needle.matches(&person.name)
                        || needle.matches(&self.project.affiliation(person));
                    if !keep || !matches {
                        continue;
                    }
                    let info = RowInfo {
                        subtitle: self.row_subtitle(person, pinned.contains(&person.id)),
                        color_key: self
                            .project
                            .category_key(crate::catalog::Kind::Organization, person),
                        unreviewed,
                        selected: self.selected_person == Some(person.id)
                            || self.people.picked.contains(&person.id),
                    };
                    shown.push(person.id);
                    let row = person_row(ui, p, person, &info);
                    if row.clicked() && ui.input(|i| i.modifiers.command || i.modifiers.shift) {
                        pick = Some(person.id);
                    } else if row.clicked() {
                        select = Some(person.id);
                    }
                }
            });
        if let Some(id) = select {
            self.selected_person = Some(id);
            self.people.wish_draft = None;
            self.people.picked.clear();
        }
        if let Some(id) = pick
            && !self.people.picked.remove(&id)
        {
            self.people.picked.insert(id);
        }
        self.people.shown = shown;
    }

    /// People with a wish or rule not fully met in the selected option.
    fn unmet_people(&mut self) -> HashSet<usize> {
        let Some(evaluation) = self.evaluation() else {
            return HashSet::new();
        };
        let owners: HashMap<u64, usize> = self
            .project
            .rules
            .iter()
            .map(|r| (r.id, r.owner()))
            .collect();
        evaluation
            .results
            .iter()
            .filter(|r| r.outcome() != crate::score::Outcome::Met)
            .filter_map(|r| owners.get(&r.rule_id).copied())
            .collect()
    }

    fn filter_chips(&mut self, ui: &mut egui::Ui, p: Palette) {
        let to_review = self
            .project
            .attendees
            .iter()
            .filter(|a| self.unreviewed_notes(a.id) > 0)
            .count();
        let filters = [
            (
                PeopleFilter::All,
                fill("All {}", &[&self.project.attendees.len()]),
            ),
            (PeopleFilter::ToReview, fill("Unreviewed {}", &[&to_review])),
            (PeopleFilter::WithWishes, tr("Wishes").to_string()),
            (PeopleFilter::Unconstrained, tr("No wishes").to_string()),
            (PeopleFilter::Unmet, tr("Unmet wishes").to_string()),
            (PeopleFilter::NotSeated, tr("Not seated").to_string()),
            (PeopleFilter::Pinned, tr("Pinned").to_string()),
        ];
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().button_padding = egui::vec2(theme::space::M, theme::space::XS);
            ui.spacing_mut().item_spacing =
                egui::vec2(theme::space::XS + 2.0, theme::space::XS + 2.0);
            ui.spacing_mut().interact_size.y = theme::CHIP_HEIGHT;
            for (filter, label) in filters {
                let on = self.people.filter == filter;
                let chip = theme::chip(
                    theme::chip_text(label, p, on).size(theme::text::SMALL),
                    p,
                    on,
                );
                if ui.add(chip).clicked() {
                    self.people.filter = filter;
                }
            }
        });
    }

    fn unreviewed_notes(&self, person: usize) -> usize {
        self.project
            .notes
            .iter()
            .filter(|n| n.person == person && !n.reviewed)
            .count()
    }

    /// Organization, number of wishes and pin state.
    fn row_subtitle(&self, person: &Attendee, pinned: bool) -> String {
        let wishes = self
            .project
            .rules
            .iter()
            .filter(|r| matches!(&r.kind, RuleKind::Proximity { requester, .. } if *requester == person.id))
            .count();
        let mut subtitle = self
            .project
            .category_name(crate::catalog::Kind::Organization, person);
        if wishes > 0 {
            subtitle.push_str(" · ");
            if wishes == 1 {
                subtitle.push_str(tr("1 wish"));
            } else {
                subtitle.push_str(&fill("{} wishes", &[&wishes]));
            }
        }
        if pinned {
            subtitle.push_str(" · ");
            subtitle.push_str(tr("pinned"));
        }
        subtitle
    }
}

/// Avatar, name, subtitle and a badge counting notes to review.
fn person_row(ui: &mut egui::Ui, p: Palette, person: &Attendee, info: &RowInfo) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 48.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            info.selected,
            &person.name,
        )
    });
    let painter = ui.painter();
    let fill = match () {
        _ if info.selected => p.accent_soft,
        _ if response.hovered() => p.surface_hi,
        _ => Color32::TRANSPARENT,
    };
    painter.rect_filled(rect, theme::radius::MD, fill);
    let avatar = rect.left_center() + egui::vec2(theme::space::S + 14.0, 0.0);
    let dark = p.is_dark();
    painter.circle_filled(avatar, 14.0, theme::avatar_color(&info.color_key, dark));
    let initials = theme::initials(&person.name);
    painter.text(
        avatar,
        egui::Align2::CENTER_CENTER,
        initials,
        egui::FontId::proportional(theme::text::CAPTION),
        Color32::WHITE,
    );
    let text_at =
        |dy: f32| rect.left_center() + egui::vec2(theme::space::S + 28.0 + theme::space::M, dy);
    painter.text(
        text_at(-8.0),
        egui::Align2::LEFT_CENTER,
        &person.name,
        egui::FontId::proportional(theme::text::BODY),
        p.text,
    );
    painter.text(
        text_at(9.0),
        egui::Align2::LEFT_CENTER,
        &info.subtitle,
        egui::FontId::proportional(theme::text::CAPTION),
        p.muted,
    );
    if info.unreviewed > 0 {
        let centre = rect.right_center() - egui::vec2(theme::space::L + 4.0, 0.0);
        let badge = egui::Rect::from_center_size(centre, egui::vec2(24.0, 20.0));
        painter.rect_filled(
            badge,
            theme::radius::PILL,
            theme::blend(p.surface, p.warn, 0.16),
        );
        painter.text(
            centre,
            egui::Align2::CENTER_CENTER,
            info.unreviewed.to_string(),
            egui::FontId::proportional(theme::text::CAPTION),
            p.warn,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
