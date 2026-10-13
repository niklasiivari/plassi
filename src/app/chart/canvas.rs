//! The pan/zoom canvas: tables, seat cards, wish lines, drag and drop.

use super::*;
use crate::app::camera::Viewport;

pub(super) const CARD_W: f32 = 176.0;

pub(super) const CARD_H: f32 = 44.0;

/// How far a side seat's card centre sits outward from its seat coordinate,
/// so the card lies just outside the table edge.
pub(super) const CARD_OFFSET: f32 = CARD_W / 2.0 + 6.0;

/// What every card on one frame of the canvas is painted with.
pub(super) struct Paint<'a> {
    pub(super) painter: &'a egui::Painter,
    pub(super) view: &'a Viewport,
    pub(super) p: Palette,
    pub(super) scene: &'a Scene,
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
            .fold(Rect::NOTHING, Rect::union)
            .expand2(Vec2::new(30.0, 50.0));
        let view = &mut self.chart.view;
        if view.fit_pending && rect.width() > 50.0 {
            view.camera.fit(rect, world, 1.4);
            view.fit_pending = false;
            view.fitted = Some(view.camera.target());
        }
        let before = view.camera.viewport(rect);
        self.start_marquee(ui, background, &before);
        let can_pan = self.chart.drag.is_none() && self.chart.marquee.is_none();
        let view = &mut self.chart.view;
        view.camera.handle_input(ui, rect, background, can_pan);
        view.camera.animate(ui.ctx());
        view.camera.viewport(rect)
    }

    fn paint_tables(&self, painter: &egui::Painter, view: &Viewport, p: Palette, scene: &Scene) {
        for table in self.project.layout.rows.iter().flat_map(|r| &r.tables) {
            let Some(body) = scene.table_body(&table.id) else {
                continue;
            };
            let selected = self.chart.selected_table.as_deref() == Some(table.id.as_str());
            painter.rect(
                view.rect(body),
                CornerRadius::same(
                    (theme::radius::SM as f32 * view.zoom).clamp(2.0, theme::radius::SM as f32)
                        as u8,
                ),
                p.table,
                if selected {
                    Stroke::new(2.0, p.accent)
                } else {
                    Stroke::NONE
                },
                StrokeKind::Outside,
            );
            painter.text(
                view.point(Pos2::new(body.center().x, body.min.y - 16.0)),
                Align2::CENTER_CENTER,
                &table.label,
                FontId::proportional((13.0 * view.zoom).max(8.0)),
                if selected { p.accent } else { p.muted },
            );
        }
    }

    /// Lines from the selected person to the people they wished for (bold) and
    /// from people who wished for them (faint), coloured by how well each wish
    /// is met. While someone is dragged, their lines follow the pointer; for
    /// someone in the "Not seated" list they start at the list.
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
        let (selected, from) = match (dragged, self.chart.hovered_person, self.chart.tray_anchor) {
            (Some(person), ..) => (person, ui.input(|i| i.pointer.interact_pos())),
            (None, Some(person), _) => (person, centre_of(person)),
            (None, None, Some((person, at))) => (person, Some(at)),
            (None, None, None) => match self.selected_person {
                Some(person) => (person, centre_of(person)),
                None => return,
            },
        };
        let Some(from) = from else {
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
                .filter(|(_, r)| scene.judged(*requester, r))
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
        let seats = scene.seats.iter().map(|s| {
            (
                card_rect(s),
                Some(s.id.as_str()),
                scene.occupant.get(&s.id).copied(),
            )
        });
        let loose = scene.loose.iter().map(|(person, at)| {
            let rect = Rect::from_center_size(*at, Vec2::new(CARD_W, CARD_H));
            (rect, None, Some(*person))
        });
        for (rect, seat, occupant) in seats.chain(loose) {
            let in_group = occupant.is_some_and(|p| group.contains(&p));
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
            painter.rect(
                view.rect(rect).expand(4.0 * view.zoom.max(0.5)),
                CornerRadius::same((12.0 * view.zoom).clamp(3.0, 14.0) as u8),
                color.gamma_multiply(0.18),
                Stroke::new(1.5, color.gamma_multiply(0.8)),
                StrokeKind::Outside,
            );
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
                let coloring = self.coloring();
                let kind = coloring.map_or(Kind::Organization, |c| c.kind);
                let person = CardPerson {
                    name: &attendee.name,
                    detail: self.project.category_name(kind, attendee),
                    color_key: self.project.category_key(Kind::Organization, attendee),
                    tint: coloring.map(|c| {
                        c.color(&self.project.category_key(kind, attendee), p)
                            .unwrap_or(p.border)
                    }),
                };
                paint_person_card(painter, p, rect, &person, look)
            }
            None => paint_empty_seat(painter, p, rect, look),
        }
    }
}
