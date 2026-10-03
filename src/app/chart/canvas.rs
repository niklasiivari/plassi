//! The pan/zoom canvas: tables, seat cards, wish lines, drag and drop.

use super::*;
use crate::app::camera::Viewport;

pub(super) const CARD_W: f32 = 176.0;

pub(super) const CARD_H: f32 = 44.0;

/// How far a side seat's card centre sits outward from its seat coordinate,
/// so the card lies just outside the table edge.
pub(super) const CARD_OFFSET: f32 = CARD_W / 2.0 + 6.0;

/// What the user did with seat cards this frame.
#[derive(Default)]
pub(super) struct SeatActions {
    /// `Some(None)` clicks an empty seat, which clears the selection.
    clicked: Option<Option<usize>>,
    drag_start: Option<usize>,
    drop_target: Option<String>,
    hovered_card: bool,
    hovered_person: Option<usize>,
    /// Ctrl/Shift-click: add to or remove from the multi-selection.
    pick: Option<usize>,
    /// Pin a person to a seat, or unpin them (`None`).
    pin: Option<(usize, Option<String>)>,
    start_swap: Option<usize>,
    swap_with: Option<String>,
    /// Open a roster change for a person on the People page.
    roster: Option<(usize, crate::app::people::RosterEdit)>,
    remove: Option<usize>,
}

/// Dotted backdrop that pans and zooms with the chart.
fn paint_grid(painter: &egui::Painter, view: &Viewport, p: Palette) {
    let spacing = 40.0 * view.zoom;
    if spacing <= 8.0 {
        return;
    }
    let rect = view.clip;
    let mut y = rect.min.y + (view.origin.y - rect.min.y).rem_euclid(spacing);
    while y < rect.max.y {
        let mut x = rect.min.x + (view.origin.x - rect.min.x).rem_euclid(spacing);
        while x < rect.max.x {
            painter.circle_filled(Pos2::new(x, y), 1.1, p.grid);
            x += spacing;
        }
        y += spacing;
    }
}

impl PlassiApp {
    /// Draws the chart and handles all interaction on it.
    pub(super) fn canvas(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene) {
        let rect = ui.max_rect();
        let background = ui.interact(rect, ui.id().with("canvas"), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        if scene.seats.is_empty() {
            self.setup_card(ui, p, rect);
            return;
        }
        let view = self.update_viewport(ui, rect, &background, scene);
        paint_grid(&painter, &view, p);
        self.paint_tables(&painter, &view, p, scene);
        self.paint_wish_lines(&painter, &view, p, scene);
        self.paint_group_highlights(&painter, &view, p, scene);
        let actions = self.seat_cards(ui, &painter, &view, p, scene);
        self.paint_drag_ghost(ui, &view, p, scene, actions.drop_target.as_deref());
        self.apply_seat_actions(ui, &background, actions);
        self.chart_shortcuts(ui);
        self.paint_status_badge(ui, &painter, rect, p);
        self.preference_bar(ui, p, rect);
        if self.project.candidates.is_empty() && self.solving.is_none() {
            self.setup_card(ui, p, rect);
        }
    }

    /// Fits, focuses, pans and zooms the view; returns the resulting transform.
    fn update_viewport(
        &mut self,
        ui: &egui::Ui,
        rect: Rect,
        background: &egui::Response,
        scene: &Scene,
    ) -> Viewport {
        let world = scene
            .seats
            .iter()
            .fold(Rect::NOTHING, |r, seat| r.union(card_rect(seat)))
            .expand2(Vec2::new(30.0, 50.0));
        let can_pan = self.chart.dragged_person.is_none();
        let view = &mut self.chart.view;
        if view.fit_pending && rect.width() > 50.0 {
            view.camera.fit(rect, world, 1.4);
            view.fit_pending = false;
        }
        view.camera.handle_input(ui, rect, background, can_pan);
        view.camera.animate(ui.ctx());
        view.camera.viewport(rect)
    }

    fn paint_tables(&self, painter: &egui::Painter, view: &Viewport, p: Palette, scene: &Scene) {
        for table in self.project.layout.rows.iter().flat_map(|r| &r.tables) {
            let seats: Vec<_> = scene
                .seats
                .iter()
                .filter(|s| s.table_id == table.id)
                .collect();
            if seats.is_empty() {
                continue;
            }
            let min_x = seats.iter().map(|s| s.x).fold(f32::INFINITY, f32::min);
            let min_y = seats.iter().map(|s| s.y).fold(f32::INFINITY, f32::min);
            let max_y = seats.iter().map(|s| s.y).fold(f32::NEG_INFINITY, f32::max);
            let body = Rect::from_min_max(
                Pos2::new(min_x, min_y - CARD_H / 2.0 - 6.0),
                Pos2::new(min_x + TABLE_SPAN, max_y + CARD_H / 2.0 + 6.0),
            );
            painter.rect(
                view.rect(body),
                CornerRadius::same(
                    (theme::radius::SM as f32 * view.zoom).clamp(2.0, theme::radius::SM as f32)
                        as u8,
                ),
                p.table,
                Stroke::NONE,
                StrokeKind::Inside,
            );
            painter.text(
                view.point(Pos2::new(body.center().x, body.min.y - 16.0)),
                Align2::CENTER_CENTER,
                &table.label,
                FontId::proportional((13.0 * view.zoom).max(8.0)),
                p.muted,
            );
        }
    }

    /// Lines from the selected person to the people they wished for (bold) and
    /// from people who wished for them (faint), coloured by how well each wish is met.
    fn paint_wish_lines(
        &self,
        painter: &egui::Painter,
        view: &Viewport,
        p: Palette,
        scene: &Scene,
    ) {
        let Some(selected) = self.chart.hovered_person.or(self.selected_person) else {
            return;
        };
        let centre_of = |person: usize| {
            scene
                .seat_of
                .get(&person)
                .and_then(|id| scene.seats.iter().find(|s| &s.id == id))
                .map(|s| view.point(card_rect(s).center()))
        };
        let Some(from) = centre_of(selected) else {
            return;
        };
        for rule in self.project.rules.iter().filter(|r| r.enabled) {
            let RuleKind::Proximity {
                requester, target, ..
            } = &rule.kind
            else {
                continue;
            };
            let outgoing = *requester == selected;
            let targets = self.project.target_ids(*requester, target);
            if !outgoing && !targets.contains(&selected) {
                continue;
            }
            let result = scene
                .own
                .get(requester)
                .and_then(|rs| rs.iter().find(|(id, _)| *id == rule.id))
                .map(|(_, r)| r.outcome());
            let color = theme::outcome_color(p, result);
            let stroke = if outgoing {
                Stroke::new(2.5, color)
            } else {
                Stroke::new(1.5, color.gamma_multiply(0.55))
            };
            let others = if outgoing { targets } else { vec![*requester] };
            for to in others.into_iter().filter_map(centre_of) {
                painter.line_segment([from, to], stroke);
                painter.circle_filled(to, 3.5, stroke.color);
            }
        }
    }

    /// Outlines the selected person's wish group, and the seats of a group
    /// move the pointer is over in the inspector.
    fn paint_group_highlights(
        &self,
        painter: &egui::Painter,
        view: &Viewport,
        p: Palette,
        scene: &Scene,
    ) {
        let group: HashSet<usize> = self
            .selected_person
            .and_then(|id| crate::groups::group_of(&self.project, id))
            .map(|g| g.into_iter().collect())
            .unwrap_or_default();
        let preview: HashSet<usize> = self
            .chart
            .preference_preview
            .and_then(|id| self.project.preferences.iter().find(|p| p.id == id))
            .map(|p| p.kind.people().iter().copied().collect())
            .unwrap_or_default();
        for seat in &scene.seats {
            let in_group = scene
                .occupant
                .get(&seat.id)
                .is_some_and(|p| group.contains(p));
            let occupant = scene.occupant.get(&seat.id);
            let previewed = self
                .chart
                .group_preview
                .as_ref()
                .is_some_and(|s| s.contains(&seat.id))
                || occupant.is_some_and(|p| preview.contains(p));
            let picked = occupant.is_some_and(|p| self.chart.multi.contains(p));
            if !in_group && !previewed && !picked {
                continue;
            }
            let color = if picked {
                p.accent
            } else if previewed {
                p.good
            } else {
                p.accent.gamma_multiply(0.6)
            };
            painter.rect(
                view.rect(card_rect(seat)).expand(4.0 * view.zoom.max(0.5)),
                CornerRadius::same((12.0 * view.zoom).clamp(3.0, 14.0) as u8),
                color.gamma_multiply(0.18),
                Stroke::new(1.5, color.gamma_multiply(0.8)),
                StrokeKind::Outside,
            );
        }
    }

    /// Paints every seat card and collects what the user did with them.
    fn seat_cards(
        &self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        view: &Viewport,
        p: Palette,
        scene: &Scene,
    ) -> SeatActions {
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let needle = crate::app::search::Needle::new(&self.chart.search);
        let mut actions = SeatActions::default();
        for seat in &scene.seats {
            let screen = view.rect(card_rect(seat));
            if !screen.intersects(view.clip) {
                continue;
            }
            let response = ui
                .interact(
                    screen,
                    ui.id().with(("seat", &seat.id)),
                    Sense::click_and_drag(),
                )
                .on_hover_cursor(egui::CursorIcon::Grab);
            actions.hovered_card |= response.hovered();
            if response.hovered() {
                actions.hovered_person = scene.occupant.get(&seat.id).copied();
            }
            let person = scene.occupant.get(&seat.id).copied();
            if let Some(attendee) = person.and_then(|id| self.project.person(id)) {
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &attendee.name)
                });
            }
            let is_drop = self.chart.dragged_person.is_some()
                && pointer.is_some_and(|pos| screen.contains(pos));
            if is_drop {
                actions.drop_target = Some(seat.id.clone());
            }
            let search_miss = !needle.is_empty()
                && !person
                    .and_then(|id| self.project.person(id))
                    .is_some_and(|a| needle.matches(&a.name));
            self.paint_card(
                painter,
                p,
                screen,
                person,
                CardLook {
                    status: person.and_then(|id| scene.status(id)),
                    selected: person.is_some() && person == self.selected_person,
                    hovered: response.hovered() || is_drop,
                    dimmed: search_miss
                        || (person.is_some() && person == self.chart.dragged_person),
                    pinned: person.is_some_and(|id| scene.pinned.contains(&id)),
                    moved: person.is_some_and(|id| scene.moved.contains(&id)),
                    gap: scene.gaps.contains(&seat.id),
                },
            );
            let adding = ui.input(|i| i.modifiers.command || i.modifiers.shift);
            if response.clicked() && adding {
                actions.pick = person;
            } else if response.clicked() {
                if self.chart.swap_from.is_some() {
                    actions.swap_with = Some(seat.id.clone());
                } else {
                    actions.clicked = Some(person);
                }
            }
            if response.drag_started() && person.is_some() {
                actions.drag_start = person;
            }
            let Some(id) = person else { continue };
            let pinned = scene.pinned.contains(&id);
            egui::Popup::context_menu(&response)
                .style(theme::menu_style)
                .show(|ui| {
                    if ui
                        .button(if pinned {
                            tr("Unpin (P)")
                        } else {
                            tr("Pin to this seat (P)")
                        })
                        .clicked()
                    {
                        actions.pin = Some((id, (!pinned).then(|| seat.id.clone())));
                    }
                    if ui.button(tr("Swap with…")).clicked() {
                        actions.start_swap = Some(id);
                    }
                    let picked = self.chart.multi.contains(&id);
                    let pick = if picked {
                        tr("Remove from selection")
                    } else {
                        tr("Add to selection")
                    };
                    if ui
                        .button(pick)
                        .on_hover_text(tr(
                            "Or Ctrl-click. Select two or more to group them or keep them apart.",
                        ))
                        .clicked()
                    {
                        actions.pick = Some(id);
                    }
                    if ui.button(tr("Show details")).clicked() {
                        actions.clicked = Some(Some(id));
                    }
                    ui.separator();
                    use crate::app::people::RosterEdit;
                    if ui.button(tr("Replace with someone else…")).clicked() {
                        actions.roster = Some((id, RosterEdit::Replace(id, Default::default())));
                    }
                    if ui.button(tr("Remove from event")).clicked() {
                        actions.remove = Some(id);
                    }
                });
            if self.chart.dragged_person.is_none() {
                response.on_hover_ui(|ui| self.hover_card(ui, p, scene, id, &seat.id));
            }
        }
        actions
    }

    /// The card following the pointer while dragging, with the score change
    /// the drop would cause.
    fn paint_drag_ghost(
        &mut self,
        ui: &egui::Ui,
        view: &Viewport,
        p: Palette,
        scene: &Scene,
        target: Option<&str>,
    ) {
        let (Some(person), Some(pos)) = (
            self.chart.dragged_person,
            ui.input(|i| i.pointer.interact_pos()),
        ) else {
            return;
        };
        let zoom = view.zoom.max(0.7);
        let top = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            ui.id().with("ghost"),
        ));
        let look = CardLook {
            status: scene.status(person),
            selected: true,
            hovered: false,
            dimmed: false,
            pinned: false,
            moved: false,
            gap: false,
        };
        let ghost = Rect::from_center_size(pos, Vec2::new(CARD_W, CARD_H) * zoom);
        self.paint_card(&top, p, ghost, Some(person), look);
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        let Some(target) = target else { return };
        let key = (
            self.file.revision,
            self.selected_candidate,
            person,
            target.to_string(),
        );
        let (project, candidate) = (&self.project, self.selected_candidate);
        if let Some(preview) = self
            .chart
            .drop_preview
            .get(key, || preview_move(project, candidate, person, target))
        {
            let (text, color) = preview_text(preview, p);
            badge(
                &top,
                pos - Vec2::new(0.0, CARD_H * zoom / 2.0 + 16.0),
                &text,
                color,
                p,
            );
        }
    }

    fn apply_seat_actions(
        &mut self,
        ui: &egui::Ui,
        background: &egui::Response,
        actions: SeatActions,
    ) {
        if let Some(person) = actions.remove {
            self.remove_people(&[person]);
        }
        if let Some((person, edit)) = actions.roster {
            self.selected_person = Some(person);
            self.people.roster = Some(edit);
            self.page = crate::app::Page::People;
        }
        if let Some(person) = actions.pick {
            let seed = self
                .chart
                .multi
                .is_empty()
                .then_some(self.selected_person)
                .flatten()
                .filter(|id| self.project.person(*id).is_some());
            if let Some(selected) = seed {
                self.chart.multi.insert(selected);
            }
            if Some(person) != seed && !self.chart.multi.remove(&person) {
                self.chart.multi.insert(person);
            }
        }
        if actions.hovered_person != self.chart.hovered_person {
            self.chart.hovered_person = actions.hovered_person;
            ui.ctx().request_repaint();
        }
        if let Some(person) = actions.start_swap {
            self.chart.swap_from = Some(person);
            self.selected_person = Some(person);
        }
        if let Some(target) = actions.swap_with
            && let Some(from) = self.chart.swap_from.take()
        {
            self.move_or_swap(from, &target);
        }
        let typing = ui.ctx().memory(|m| m.focused().is_some());
        if !typing
            && ui.input(|i| i.key_pressed(egui::Key::Escape))
            && self.chart.swap_from.take().is_none()
            && std::mem::take(&mut self.chart.multi).is_empty()
            && self.chart.dragged_person.take().is_none()
        {
            self.selected_person = None;
        }
        if let Some((person, seat)) = actions.pin {
            self.set_pin(person, seat.as_deref());
        }
        if let Some(person) = actions.drag_start {
            self.chart.dragged_person = Some(person);
            self.selected_person = Some(person);
        }
        if let Some(person) = actions.clicked {
            self.selected_person = person;
        } else if background.clicked() && !actions.hovered_card {
            self.selected_person = None;
        }
        if self.chart.dragged_person.is_some() && ui.input(|i| !i.pointer.primary_down()) {
            let person = self.chart.dragged_person.take().unwrap();
            if let Some(seat) = actions.drop_target {
                self.move_or_swap(person, &seat);
            }
            self.chart.drop_preview.clear();
        }
    }

    /// Swap-mode instructions, or the score change of the last edit.
    fn paint_status_badge(&self, ui: &egui::Ui, painter: &egui::Painter, rect: Rect, p: Palette) {
        let at = Pos2::new(rect.center().x, rect.min.y + 28.0);
        if let Some(from) = self.chart.swap_from {
            let name = self.project.person_name(from);
            badge(
                painter,
                at,
                &fill("Click someone to swap with {}  ·  Esc to cancel", &[&name]),
                p.accent,
                p,
            );
        } else if let Some((delta, when)) = self.chart.last_change
            && when.elapsed().as_secs_f32() < 4.0
        {
            let color = match delta {
                d if d < 0 => p.good,
                d if d > 0 => p.warn,
                _ => p.muted,
            };
            badge(painter, at, &fill("Score {}", &[&signed(delta)]), color, p);
            ui.ctx().request_repaint();
        }
    }

    /// Paints one seat card (a person, or an empty seat) filling `rect`.
    pub(super) fn paint_card(
        &self,
        painter: &egui::Painter,
        p: Palette,
        rect: Rect,
        person: Option<usize>,
        look: CardLook,
    ) {
        match person.and_then(|id| self.project.person(id)) {
            Some(attendee) => {
                let person = CardPerson {
                    name: &attendee.name,
                    organization: self.project.category_name(Kind::Organization, attendee),
                    color_key: self.project.category_key(Kind::Organization, attendee),
                };
                paint_person_card(painter, p, rect, &person, look)
            }
            None => paint_empty_seat(painter, p, rect, look),
        }
    }
}
