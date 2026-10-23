//! The searchable, filterable attendee list.

use super::*;
use crate::i18n::{fill, tr};
use std::collections::{HashMap, HashSet};

use crate::model::{Attendee, Project};

/// What a list row shows besides the name.
struct RowInfo {
    subtitle: String,
    color_key: String,
    unreviewed: usize,
    pinned: bool,
    name_lowercase: String,
    affiliation_lowercase: String,
}

/// Project-derived list data shared by every frame until the next edit.
pub(super) struct ListInfo {
    pub(super) active: HashSet<usize>,
    rows: HashMap<usize, RowInfo>,
    to_review: usize,
}

impl ListInfo {
    fn new(project: &Project) -> Self {
        let mut unreviewed = HashMap::<usize, usize>::new();
        for note in project.notes.iter().filter(|n| !n.reviewed) {
            *unreviewed.entry(note.person).or_default() += 1;
        }
        let mut wishes = HashMap::<usize, usize>::new();
        for rule in &project.rules {
            if let RuleKind::Proximity { requester, .. } = rule.kind {
                *wishes.entry(requester).or_default() += 1;
            }
        }
        let pinned = project.pins();
        Self {
            active: project.active_people(),
            to_review: unreviewed.len(),
            rows: project
                .attendees
                .iter()
                .map(|person| {
                    let pinned = pinned.contains_key(&person.id);
                    let affiliation = project.affiliation(person);
                    (
                        person.id,
                        RowInfo {
                            subtitle: row_subtitle(
                                project,
                                person,
                                pinned,
                                wishes.get(&person.id).copied().unwrap_or(0),
                            ),
                            color_key: project
                                .category_key(crate::catalog::Kind::Organization, person),
                            affiliation_lowercase: affiliation.to_lowercase(),
                            name_lowercase: person.name.to_lowercase(),
                            unreviewed: unreviewed.get(&person.id).copied().unwrap_or(0),
                            pinned,
                        },
                    )
                })
                .collect(),
        }
    }
}

impl PlassiApp {
    pub(super) fn people_list_info(&mut self) -> std::rc::Rc<ListInfo> {
        let project = &self.project;
        self.people
            .list_cache
            .get(self.file.revision, || {
                std::rc::Rc::new(ListInfo::new(project))
            })
            .clone()
    }

    pub(super) fn people_list(&mut self, ui: &mut egui::Ui, p: Palette, scroll_rows: bool) {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(tr("People & wishes"))
                    .size(theme::text::TITLE)
                    .strong(),
            );
            if ui
                .add(theme::secondary_button(tr("+ Add person")))
                .clicked()
            {
                self.flush_people_details();
                self.people.import_open = false;
                self.people.roster = Some(roster::RosterEdit::Add(Default::default()));
            }
        });
        if ui
            .add(theme::secondary_button(tr("Import people")))
            .clicked()
        {
            self.flush_people_details();
            self.people.import_open = true;
            self.people.roster = None;
        }
        ui.add_space(theme::space::M);
        if self.project.attendees.is_empty() {
            return;
        }
        if scroll_rows
            && ui
                .add(theme::chip(
                    tr("Overview"),
                    p,
                    self.selected_person.is_none()
                        && !self.people.import_open
                        && self.people.roster.is_none(),
                ))
                .clicked()
        {
            self.flush_people_details();
            self.selected_person = None;
            self.people.end_review();
            self.people.import_open = false;
            self.people.roster = None;
            self.people.wish_draft = None;
            self.people.filter = PeopleFilter::All;
            self.people.search.clear();
            self.people.picked.clear();
            self.people.selecting = false;
        }
        // The phone has one pane, so its review action belongs with the list.
        if !scroll_rows
            && self.project.notes.iter().any(|note| !note.reviewed)
            && ui
                .add(theme::primary_button(tr("Review notes"), p))
                .clicked()
        {
            self.start_review();
        }
        ui.add_space(theme::space::S);
        let search_field = ui.add(
            theme::field(&mut self.people.search)
                .hint_text(tr("Search name or organization"))
                .desired_width(f32::INFINITY),
        );
        search_field.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::TextEdit,
                true,
                tr("Search name or organization"),
            )
        });
        ui.add_space(theme::space::XS);
        let info = self.people_list_info();
        let to_review = info.to_review;
        if !self.people.picked.is_empty() && ui.button(tr("Clear selection")).clicked() {
            self.people.picked.clear();
            self.people.selecting = false;
        }
        self.filter_chips(ui, p, to_review);
        self.people_list_indices(&info);
        self.bulk_bar(ui);
        ui.add_space(theme::space::XS);
        ui.separator();
        // Bulk actions above can change review counts, memberships or pins.
        let info = self.people_list_info();
        let indices = self.people_list_indices(&info);
        let mut select = None;
        let mut pick = None;
        let mut rows = |ui: &mut egui::Ui, range: std::ops::Range<usize>| {
            for &index in &indices[range] {
                let person = &self.project.attendees[index];
                let selected = self.selected_person == Some(person.id)
                    || self.people.picked.contains(&person.id);
                let row = ui
                    .push_id(person.id, |ui| {
                        person_row(ui, p, person, &info.rows[&person.id], selected)
                    })
                    .inner;
                if row.clicked()
                    && (self.people.selecting
                        || ui.input(|i| i.modifiers.command || i.modifiers.shift))
                {
                    pick = Some(person.id);
                } else if row.clicked() {
                    self.people.import_open = false;
                    select = Some(person.id);
                }
            }
        };
        if scroll_rows {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show_viewport(ui, |ui, viewport| {
                    let spacing = ui.spacing().item_spacing.y;
                    let pitch = 48.0 + spacing;
                    ui.set_height((pitch * indices.len() as f32 - spacing).max(0.0));
                    let mut first = (viewport.min.y / pitch).floor() as usize;
                    let mut last = (viewport.max.y / pitch).ceil() as usize + 1;
                    if last > indices.len() {
                        let count = last.saturating_sub(first);
                        last = indices.len();
                        first = last.saturating_sub(count);
                    }
                    // Retain a page on either side so nearby rows remain
                    // reachable by focus/accessibility scroll actions when
                    // controls above the list change height.
                    let buffer = (viewport.height() / pitch).ceil() as usize;
                    first = first.saturating_sub(buffer);
                    last = last.saturating_add(buffer).min(indices.len());
                    let top = ui.max_rect().top();
                    let rect = egui::Rect::from_x_y_ranges(
                        ui.max_rect().x_range(),
                        top + first as f32 * pitch..=top + last as f32 * pitch,
                    );
                    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                        ui.skip_ahead_auto_ids(first);
                        rows(ui, first..last);
                    });
                });
        } else {
            rows(ui, 0..indices.len());
        }
        if let Some(id) = select {
            self.flush_people_details();
            self.people.end_review();
            self.selected_person = Some(id);
            self.people.wish_draft = None;
            self.people.picked.clear();
        }
        if let Some(id) = pick
            && !self.people.picked.remove(&id)
        {
            self.people.picked.insert(id);
        }
    }

    /// Filter once per edit/search and retain every match for bulk selection.
    fn people_list_indices(&mut self, info: &ListInfo) -> std::rc::Rc<Vec<usize>> {
        let key = (
            self.file.revision,
            self.selected_candidate,
            self.people.filter,
            self.people.search.clone(),
        );
        if let Some(indices) = self.people.list_matches.peek(&key) {
            return std::rc::Rc::clone(indices);
        }
        let unmet = if self.people.filter == PeopleFilter::Unmet {
            self.unmet_people()
        } else {
            HashSet::new()
        };
        let assignment = self
            .project
            .candidates
            .get(self.selected_candidate)
            .map(|c| &c.assignment);
        let needle = crate::app::search::Needle::new(&self.people.search);
        let indices: Vec<_> = self
            .project
            .attendees
            .iter()
            .enumerate()
            .filter_map(|(index, person)| {
                let row = &info.rows[&person.id];
                let keep = match self.people.filter {
                    PeopleFilter::All => true,
                    PeopleFilter::ToReview => row.unreviewed > 0,
                    PeopleFilter::WithWishes => info.active.contains(&person.id),
                    PeopleFilter::Unconstrained => !info.active.contains(&person.id),
                    PeopleFilter::Unmet => unmet.contains(&person.id),
                    PeopleFilter::NotSeated => {
                        assignment.is_some_and(|a| !a.contains_key(&person.id))
                    }
                    PeopleFilter::Pinned => row.pinned,
                };
                let matches = needle.matches_lowercase(&row.name_lowercase)
                    || needle.matches_lowercase(&row.affiliation_lowercase);
                (keep && matches).then_some(index)
            })
            .collect();
        self.people.shown = indices
            .iter()
            .map(|&i| self.project.attendees[i].id)
            .collect();
        let indices = std::rc::Rc::new(indices);
        self.people
            .list_matches
            .set(key, std::rc::Rc::clone(&indices));
        indices
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

    fn filter_chips(&mut self, ui: &mut egui::Ui, p: Palette, to_review: usize) {
        let filters = [
            (
                PeopleFilter::All,
                fill("All {}", &[&self.project.attendees.len()]),
            ),
            (PeopleFilter::ToReview, fill("Unreviewed {}", &[&to_review])),
            (
                PeopleFilter::WithWishes,
                tr("In wishes or rules").to_string(),
            ),
            (
                PeopleFilter::Unconstrained,
                tr("No wishes or rules").to_string(),
            ),
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
}

/// Organization, number of wishes and pin state.
fn row_subtitle(project: &Project, person: &Attendee, pinned: bool, wishes: usize) -> String {
    let mut subtitle = project.category_name(crate::catalog::Kind::Organization, person);
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

/// Avatar, name, subtitle and a badge counting notes to review.
fn person_row(
    ui: &mut egui::Ui,
    p: Palette,
    person: &Attendee,
    info: &RowInfo,
    selected: bool,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 48.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            selected,
            &person.name,
        )
    });
    if !ui.is_rect_visible(rect) {
        return response.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    let painter = ui.painter();
    let fill = match () {
        _ if selected => p.accent_soft,
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
    let right_padding = if info.unreviewed > 0 {
        theme::space::L + 4.0 + 12.0 + theme::space::S
    } else {
        theme::space::S
    };
    let text_width = (rect.right() - right_padding - text_at(0.0).x).max(0.0);
    let mut elided = false;
    for (text, dy, size, color) in [
        (person.name.as_str(), -8.0, theme::text::BODY, p.text),
        (info.subtitle.as_str(), 9.0, theme::text::CAPTION, p.muted),
    ] {
        let galley = egui::WidgetText::from(RichText::new(text).size(size)).into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            text_width,
            egui::TextStyle::Body,
        );
        elided |= galley.elided;
        let pos = text_at(dy) - egui::vec2(0.0, galley.size().y / 2.0);
        painter.galley(pos, galley, color);
    }
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
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if elided {
        response.on_hover_text(format!("{}\n{}", person.name, info.subtitle))
    } else {
        response
    }
}
