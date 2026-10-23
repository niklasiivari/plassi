//! The pan/zoom canvas: tables, seat cards, wish lines, drag and drop.

use super::*;
use crate::app::camera::Viewport;

pub(super) use crate::app::turned::{CARD_H, CARD_W};

fn animate_position(ctx: &egui::Context, id: egui::Id, target: Pos2, duration: f32) -> Pos2 {
    Pos2::new(
        ctx.animate_value_with_time(id.with("x"), target.x, duration),
        ctx.animate_value_with_time(id.with("y"), target.y, duration),
    )
}

/// What every card on one frame of the canvas is painted with.
pub(super) struct Paint<'a> {
    pub(super) painter: &'a egui::Painter,
    pub(super) view: &'a Viewport,
    pub(super) p: Palette,
    pub(super) scene: &'a Scene,
}

/// An outline around a card, given in world space.
fn outline_card(painter: &egui::Painter, view: &Viewport, card: Turned, stroke: Stroke) {
    let centre = view.point(card.centre);
    let rect = Rect::from_center_size(centre, card.size * view.zoom);
    paint_turned(painter, card.angle, centre, |painter| {
        painter.rect_stroke(
            rect.expand(3.0 * view.zoom.max(0.5)),
            CornerRadius::same((10.0 * view.zoom).clamp(3.0, 12.0) as u8),
            stroke,
            StrokeKind::Outside,
        )
    });
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
    /// Briefly settle hand edits and released cards, while a grabbed card
    /// tracks the pointer immediately. World coordinates keep zooming smooth.
    pub(super) fn card_centre_animated(
        &self,
        ctx: &egui::Context,
        person: usize,
        target: Pos2,
        immediate: bool,
    ) -> Pos2 {
        let id = egui::Id::new((
            "chart-card-motion",
            &self.file.name,
            self.selected_candidate,
            person,
        ));
        let duration = if immediate { 0.0 } else { 0.18 };
        animate_position(ctx, id, target, duration)
    }

    /// Draws the chart and handles all interaction on it.
    pub(super) fn canvas(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene) {
        let rect = ui.max_rect();
        if ui.input(|i| i.multi_touch().is_some()) {
            // A second finger changes a card drag into a camera gesture.
            self.chart.drag = None;
            self.chart.marquee = None;
        }
        let background = ui.interact(rect, ui.id().with("canvas"), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        if scene.seats.is_empty() {
            self.setup_card(ui, p, rect);
            return;
        }
        let view = self.update_viewport(ui, rect, &background, scene);
        paint_grid(&painter, &view, p);
        self.paint_walls(&painter, &view, p);
        if self.project.layout.free_layout {
            for label in &self.project.layout.labels {
                crate::app::room_labels::paint(&painter, &view, label, p.muted);
            }
        }
        self.paint_tables(ui, &painter, &view, p, scene);
        self.paint_wish_lines(ui, &painter, &view, p, scene);
        self.paint_group_highlights(&painter, &view, p, scene);
        let targets = self.drop_targets(ui, &view, scene);
        let paint = Paint {
            painter: &painter,
            view: &view,
            p,
            scene,
        };
        let actions = self.cards(ui, &paint, targets.as_ref());
        self.paint_drag(ui, &view, p, scene, targets.as_ref());
        self.marquee(ui, &painter, &view, p, scene);
        let table = background
            .interact_pointer_pos()
            .and_then(|at| scene.table_at(&self.project, view.world(at)));
        self.apply_card_actions(ui, &background, table, actions);
        self.finish_drag(ui, targets);
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
        let card = Vec2::new(CARD_W, CARD_H);
        let world = scene
            .loose
            .values()
            .map(|at| Rect::from_center_size(*at, card))
            .chain(scene.seats.iter().map(card_rect))
            .chain(
                self.project
                    .layout
                    .labels
                    .iter()
                    .filter(|_| self.project.layout.free_layout)
                    .map(crate::app::room_labels::bounds),
            )
            .fold(Rect::NOTHING, Rect::union)
            .expand2(Vec2::new(30.0, 50.0));
        let view = &mut self.chart.view;
        let screen_width = ui.ctx().content_rect().width();
        if view
            .screen_width
            .is_some_and(|previous| (previous - screen_width).abs() > 1.0)
        {
            view.fit_pending = true;
        }
        view.screen_width = Some(screen_width);
        if let Some(previous) = view.canvas_rect {
            view.camera.resize(previous, rect);
        }
        view.canvas_rect = Some(rect);
        if view.fit_pending && rect.width() > 50.0 {
            view.camera.fit(rect, world, 1.4);
            view.fit_pending = false;
        }
        let before = view.camera.viewport(rect);
        self.start_marquee(ui, background, &before);
        let can_pan = self.chart.drag.is_none() && self.chart.marquee.is_none();
        let view = &mut self.chart.view;
        view.camera.handle_input(ui, rect, background, can_pan);
        view.camera.animate(ui.ctx());
        view.camera.viewport(rect)
    }

    /// The room's walls from the free layout.
    fn paint_walls(&self, painter: &egui::Painter, view: &Viewport, p: Palette) {
        let layout = &self.project.layout;
        let Some(room) = layout.room.filter(|_| layout.free_layout) else {
            return;
        };
        let walls = Rect::from_min_size(
            Pos2::new(room.x, room.y),
            Vec2::new(room.width, room.height),
        );
        painter.rect_stroke(
            view.rect(walls),
            2.0,
            Stroke::new(2.0, p.border),
            StrokeKind::Inside,
        );
    }

    fn paint_tables(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        view: &Viewport,
        p: Palette,
        scene: &Scene,
    ) {
        for table in self.project.layout.rows.iter().flat_map(|r| &r.tables) {
            let Some(top) = scene.tops.get(&table.id) else {
                continue;
            };
            let selected = self.chart.selected_table.as_deref() == Some(table.id.as_str());
            let radius =
                (theme::radius::SM as f32 * view.zoom).clamp(2.0, theme::radius::SM as f32);
            let stroke = if selected {
                Stroke::new(2.0, p.accent)
            } else {
                Stroke::NONE
            };
            top.paint(painter, view, p.table, stroke, radius as u8);
            // Cards surround a turned or round table, so its name goes on top.
            let body = top.bounds();
            let at = if top.round || top.top.angle != 0.0 {
                body.center()
            } else {
                Pos2::new(body.center().x, body.min.y - 16.0)
            };
            let color = if selected { p.accent } else { p.muted };
            let mut job = egui::text::LayoutJob::simple_singleline(
                table.label.clone(),
                FontId::proportional((13.0 * view.zoom).max(8.0)),
                color,
            );
            job.wrap.max_width = view.rect(body).width().min(view.clip.width()).max(0.0);
            job.wrap.max_rows = 1;
            job.wrap.break_anywhere = true;
            let label = painter.layout_job(job);
            let rect = Rect::from_center_size(view.point(at), label.size());
            painter.galley(rect.min, label, color);
            ui.interact(
                rect,
                ui.id().with(("table-label", &table.id)),
                Sense::hover(),
            )
            .on_hover_text(&table.label);
        }
    }

    /// Lines from the selected person to the people they wished for (bold) and
    /// from people who wished for them (faint), coloured by how well each wish
    /// is met. While someone is dragged, their lines follow the pointer. For
    /// someone in the "Not seated" list, with no card on the chart, the cards
    /// at the other ends are outlined instead.
    fn paint_wish_lines(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        view: &Viewport,
        p: Palette,
        scene: &Scene,
    ) {
        let centre_of = |person: usize| scene.card_centre(person).map(|c| view.point(c));
        let dragged = self.chart.drag.as_ref().map(|d| d.people[0]);
        let shown = (dragged.or(self.chart.hovered_person))
            .or(self.chart.tray_hover)
            .or(self.selected_person);
        let Some(selected) = shown else {
            return;
        };
        let from = match dragged {
            Some(_) => ui.input(|i| i.pointer.interact_pos()),
            None => centre_of(selected),
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
                .filter(|(_, r)| scene.judged(*requester, r))
                .map(|(_, r)| r.outcome());
            let color = theme::outcome_color(p, result);
            let stroke = if outgoing {
                Stroke::new(2.5, color)
            } else {
                Stroke::new(1.5, color.gamma_multiply(0.55))
            };
            let others = if outgoing { targets } else { vec![*requester] };
            for card in others.into_iter().filter_map(|p| scene.card_of(p)) {
                let to = view.point(card.centre);
                match from {
                    Some(from) => {
                        painter.line_segment([from, to], stroke);
                        painter.circle_filled(to, 3.5, stroke.color);
                    }
                    None => outline_card(painter, view, card, stroke),
                }
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
        let groups = self.chart.groups.peek(&self.file.revision);
        let group = self
            .selected_person
            .and_then(|id| groups?.membership.get(&id).copied());
        let fallback: HashSet<_> = if groups.is_none() {
            self.selected_person
                .and_then(|id| self.wish_group(id))
                .map(|g| g.iter().copied().collect())
                .unwrap_or_default()
        } else {
            HashSet::new()
        };
        let preview: HashSet<usize> = self
            .chart
            .preference_preview
            .and_then(|id| self.project.preferences.iter().find(|p| p.id == id))
            .map(|p| p.kind.people().iter().copied().collect())
            .unwrap_or_default();
        let seats = scene.seats.iter().map(|s| {
            (
                Some(card_turned(s)),
                Some(s.id.as_str()),
                scene.occupant.get(&s.id).copied(),
            )
        });
        let loose = scene
            .loose
            .keys()
            .map(|&person| (scene.card_of(person), None, Some(person)));
        for (card, seat, occupant) in seats.chain(loose) {
            let Some(card) = card else { continue };
            let in_group = occupant.is_some_and(|p| {
                group.is_some_and(|g| {
                    groups.is_some_and(|groups| groups.membership.get(&p) == Some(&g))
                }) || fallback.contains(&p)
            });
            let previewed = seat.is_some_and(|id| {
                self.chart
                    .group_preview
                    .as_ref()
                    .is_some_and(|s| s.iter().any(|s| s == id))
            }) || occupant.is_some_and(|p| preview.contains(&p));
            let picked = occupant.is_some_and(|p| self.chart.multi.contains(&p));
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
            let centre = view.point(card.centre);
            paint_turned(painter, card.angle, centre, |painter| {
                painter.rect(
                    Rect::from_center_size(centre, card.size * view.zoom)
                        .expand(4.0 * view.zoom.max(0.5)),
                    CornerRadius::same((12.0 * view.zoom).clamp(3.0, 14.0) as u8),
                    color.gamma_multiply(0.18),
                    Stroke::new(1.5, color.gamma_multiply(0.8)),
                    StrokeKind::Outside,
                )
            });
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
                &fill("Choose who swaps with {} · Esc to cancel", &[&name]),
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
            let text = match delta {
                0 => tr("Score unchanged").to_string(),
                _ => fill("Score {}", &[&signed(delta)]),
            };
            badge(painter, at, &text, color, p);
            ui.ctx().request_repaint_after(
                web_time::Duration::from_secs(4).saturating_sub(when.elapsed()),
            );
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
        match person.and_then(|id| self.card_metadata(id)) {
            Some(metadata) => {
                let attendee = &self.project.attendees[metadata.attendee];
                let coloring = self.coloring();
                let kind = coloring.map_or(Kind::Organization, |c| c.kind);
                let person = CardPerson {
                    name: &attendee.name,
                    detail: &metadata.categories[kind as usize].name,
                    color_key: metadata.categories[Kind::Organization as usize]
                        .keys
                        .first()
                        .map_or("", String::as_str),
                    tint: coloring.map(|c| {
                        c.tint(
                            &metadata.categories[kind as usize].keys,
                            self.chart.color_hover.as_deref(),
                            p,
                        )
                    }),
                };
                paint_person_card(painter, p, rect, &person, look)
            }
            None => {
                // A control can edit the project after this frame's scene was
                // built. Show fresh text until the cache refreshes next frame.
                if let Some(attendee) = person.and_then(|id| self.project.person(id)) {
                    let coloring = self.coloring();
                    let kind = coloring.map_or(Kind::Organization, |c| c.kind);
                    let detail = self.project.category_name(kind, attendee);
                    let color_key = self.project.category_key(Kind::Organization, attendee);
                    let tint = coloring.map(|c| {
                        c.tint(
                            &self.project.category_keys(kind, attendee),
                            self.chart.color_hover.as_deref(),
                            p,
                        )
                    });
                    paint_person_card(
                        painter,
                        p,
                        rect,
                        &CardPerson {
                            name: &attendee.name,
                            detail: &detail,
                            color_key: &color_key,
                            tint,
                        },
                        look,
                    );
                } else {
                    paint_empty_seat(painter, p, rect, look);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_settle_between_positions_but_dragging_has_no_lag() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("card-motion-test");
        let frame = |time, target, duration| {
            let mut at = Pos2::ZERO;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    ..Default::default()
                },
                |ui| {
                    at = animate_position(ui.ctx(), id, target, duration);
                },
            );
            output.textures_delta.clear();
            at
        };
        let start = Pos2::new(10.0, 20.0);
        let end = Pos2::new(110.0, 220.0);
        assert_eq!(frame(0.0, start, 0.18), start);
        frame(0.01, end, 0.18);
        let between = frame(0.1, end, 0.18);
        assert!(start.x < between.x && between.x < end.x);
        assert!(start.y < between.y && between.y < end.y);
        assert_eq!(frame(0.3, end, 0.18), end);
        assert_eq!(frame(0.31, start, 0.0), start);
    }
}
