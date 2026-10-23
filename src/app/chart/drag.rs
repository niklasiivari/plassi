//! Dragging people: one person or the selected block, from seats, from the
//! chart floor or from the "Not seated" list, onto seats, onto the floor, or
//! back into the list.

use super::*;
use crate::app::camera::Viewport;
use crate::model::{Point, SEAT_PITCH};

/// People being dragged.
pub(in crate::app) struct Drag {
    /// Everyone moving, the person grabbed first; the others keep their
    /// place relative to that person.
    pub(in crate::app) people: Vec<usize>,
    /// From the grabbed card's centre to the pointer, in chart units.
    pub(in crate::app) grip: Vec2,
}

/// Where a drop would put each dragged person.
pub(in crate::app) type Targets = Vec<(usize, Spot)>;

impl PlassiApp {
    /// Starts dragging `person`, with the whole selection when they are in it.
    pub(in crate::app) fn start_drag(&mut self, person: usize, grip: Vec2) {
        let mut people = vec![person];
        if self.chart.multi.contains(&person) {
            people.extend(self.chart.multi.iter().copied().filter(|p| *p != person));
        }
        self.chart.drag = Some(Drag { people, grip });
        self.selected_person = Some(person);
    }

    /// Where each dragged person would land if dropped now; `None` with the
    /// pointer outside the chart and the list.
    pub(super) fn drop_targets(
        &self,
        ui: &egui::Ui,
        view: &Viewport,
        scene: &Scene,
    ) -> Option<Targets> {
        let drag = self.chart.drag.as_ref()?;
        let pointer = ui.input(|i| i.pointer.interact_pos())?;
        if self.chart.tray_rect.is_some_and(|r| r.contains(pointer)) {
            return Some(drag.people.iter().map(|p| (*p, Spot::Unseated)).collect());
        }
        if !view.clip.contains(pointer) {
            return None;
        }
        // Snap around the grabbed card's centre, keeping the original grip.
        let anchor = view.world(pointer) - drag.grip;
        let mut targets = landing(scene, &drag.people, anchor);
        clear_of_obstacles(scene, &mut targets);
        Some(targets)
    }

    /// Cards following the pointer, the seats they would land on outlined,
    /// and what the drop would do.
    pub(super) fn paint_drag(
        &mut self,
        ui: &egui::Ui,
        view: &Viewport,
        p: Palette,
        scene: &Scene,
        targets: Option<&Targets>,
    ) {
        let (Some(drag), Some(pointer)) =
            (&self.chart.drag, ui.input(|i| i.pointer.interact_pos()))
        else {
            return;
        };
        let top = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            ui.id().with("ghost"),
        ));
        if let Some(targets) = targets {
            outline_seats(&top, view, p, scene, targets);
        }
        let size = Vec2::new(CARD_W, CARD_H) * view.zoom;
        let anchor = view.world(pointer) - drag.grip;
        for (person, centre) in follow_pointer(scene, &drag.people, anchor) {
            // Remember the actual release position so the placed card glides
            // from here to its seat or its adjusted floor position.
            self.card_centre_animated(ui.ctx(), person, centre, true);
            let at = view.point(centre);
            let look = CardLook {
                status: scene.status(person),
                selected: true,
                ..Default::default()
            };
            self.paint_card(
                &top,
                p,
                Rect::from_center_size(at, size),
                Some(person),
                look,
            );
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        if let Some(targets) = targets {
            self.paint_drop_preview(
                &top,
                p,
                pointer - Vec2::new(0.0, size.y / 2.0 + 16.0),
                targets,
            );
        }
    }

    /// The score change badge for a drop.
    fn paint_drop_preview(
        &mut self,
        painter: &egui::Painter,
        p: Palette,
        at: Pos2,
        targets: &Targets,
    ) {
        // Where a loose card lies does not change the score, but being on
        // the chart or in the list decides whether there is a move at all.
        let floor = Spot::Loose(Point { x: 0.0, y: 0.0 });
        let key: Targets = targets
            .iter()
            .map(|(person, spot)| match spot {
                Spot::Loose(_) => (*person, floor.clone()),
                _ => (*person, spot.clone()),
            })
            .collect();
        let scorer = self.scorer();
        let (project, candidate) = (&self.project, self.selected_candidate);
        let preview = self
            .chart
            .drop_preview
            .get((self.file.revision, candidate, key), || {
                preview(&scorer, project, candidate, targets)
            });
        if let Some(preview) = preview {
            let pinned = preview.pinned_to.map(|id| self.project.person_name(id));
            let (text, color) = preview_text(preview, pinned, p);
            badge(painter, at, &text, color, p);
        }
    }

    /// Drops the dragged people when the button is released.
    pub(super) fn finish_drag(&mut self, ui: &egui::Ui, targets: Option<Targets>) {
        if self.chart.drag.is_none() || ui.input(|i| i.pointer.primary_down()) {
            return;
        }
        self.chart.drag = None;
        self.chart.drop_preview.clear();
        if let Some(targets) = targets {
            self.rearrange(&targets);
        }
    }
}

/// Where each dragged card is drawn when the first one's centre is at
/// `anchor`: the others keep their offset from it, and people without a
/// card on the chart stack below, one seat apart.
fn follow_pointer(scene: &Scene, people: &[usize], anchor: Pos2) -> Vec<(usize, Pos2)> {
    let origin = scene.card_centre(people[0]);
    let mut row = usize::from(origin.is_some());
    people
        .iter()
        .map(|&p| {
            let offset = match (origin, scene.card_centre(p)) {
                (Some(origin), Some(centre)) => centre - origin,
                _ => {
                    row += 1;
                    Vec2::new(0.0, (row - 1) as f32 * SEAT_PITCH)
                }
            };
            (p, anchor + offset)
        })
        .collect()
}

/// An outline around each seat the dragged people would land on.
fn outline_seats(
    painter: &egui::Painter,
    view: &Viewport,
    p: Palette,
    scene: &Scene,
    targets: &Targets,
) {
    for (_, spot) in targets {
        let Spot::Seat(id) = spot else { continue };
        let Some(seat) = scene.seat(id) else { continue };
        let card = card_turned(seat);
        let centre = view.point(card.centre);
        paint_turned(painter, card.angle, centre, |painter| {
            painter.rect_stroke(
                Rect::from_center_size(centre, card.size * view.zoom)
                    .expand(3.0 * view.zoom.max(0.5)),
                CornerRadius::same((10.0 * view.zoom).clamp(3.0, 12.0) as u8),
                Stroke::new(2.0, p.accent),
                StrokeKind::Outside,
            )
        });
    }
}

/// Each person's spot when the first one's card lands at `anchor`. The
/// others keep their offset from the first; people without a place on the
/// chart stack below, one seat apart, so a stack dropped on a table side
/// fills its seats in a row. The stack skips seats the others land on.
fn landing(scene: &Scene, people: &[usize], anchor: Pos2) -> Targets {
    let origin = scene.card_centre(people[0]);
    let placed: HashMap<usize, Spot> = people
        .iter()
        .filter_map(|&p| {
            let offset = scene.card_centre(p)? - origin?;
            Some((p, spot_at(scene, anchor + offset)))
        })
        .collect();
    let taken: HashSet<&str> = placed
        .values()
        .filter_map(|spot| match spot {
            Spot::Seat(id) => Some(id.as_str()),
            _ => None,
        })
        .collect();
    let mut row = usize::from(origin.is_some());
    let mut stack = || loop {
        let spot = spot_at(scene, anchor + Vec2::new(0.0, row as f32 * SEAT_PITCH));
        row += 1;
        if !matches!(&spot, Spot::Seat(id) if taken.contains(id.as_str())) {
            return spot;
        }
    };
    people
        .iter()
        .map(|&p| (p, placed.get(&p).cloned().unwrap_or_else(&mut stack)))
        .collect()
}

/// Moves the cards dropped on the floor down together, keeping their shape,
/// until none covers a table, its chairs, or a loose card that stays where it is.
fn clear_of_obstacles(scene: &Scene, targets: &mut Targets) {
    if !targets
        .iter()
        .any(|(_, spot)| matches!(spot, Spot::Loose(_)))
    {
        return;
    }
    let size = Vec2::new(CARD_W, CARD_H);
    let mut obstacles: Vec<Rect> = scene
        .loose
        .iter()
        .filter(|(p, _)| !targets.iter().any(|(t, _)| t == *p))
        .map(|(_, at)| Rect::from_center_size(*at, size))
        .collect();
    // Reserve the whole table and chair area, including the gaps between
    // neighbouring seats. Check the full floating card, not just its centre.
    let mut tables: HashMap<&str, Rect> = HashMap::new();
    for seat in &scene.seats {
        let area = tables
            .entry(&seat.table_id)
            .or_insert_with(|| scene.table_body(&seat.table_id).expect("table has a seat"));
        *area = area.union(card_rect(seat));
    }
    obstacles.extend(tables.into_values());
    let dropped: Vec<Pos2> = targets
        .iter()
        .filter_map(|(_, spot)| match spot {
            Spot::Loose(point) => Some(Pos2::new(point.x, point.y)),
            _ => None,
        })
        .collect();
    let mut dy = 0.0;
    // Each step clears one obstacle below, so this ends.
    while let Some(push) = dropped.iter().find_map(|at| {
        let card = Rect::from_center_size(*at + Vec2::new(0.0, dy), size);
        let under = obstacles.iter().find(|r| r.intersects(card))?;
        Some(under.bottom() + 4.0 - card.top())
    }) {
        dy += push;
    }
    for (_, spot) in targets.iter_mut() {
        if let Spot::Loose(point) = spot {
            point.y += dy;
        }
    }
}

/// Snap to the closest seat card within a forgiving margin, including chair
/// gaps. Table centres and open floor remain floating drops.
fn spot_at(scene: &Scene, at: Pos2) -> Spot {
    const SNAP_MARGIN: f32 = 24.0;
    let nearest = scene.seat_at(at).or_else(|| {
        scene
            .seats
            .iter()
            .filter(|seat| card_turned(seat).distance_sq(at) <= SNAP_MARGIN * SNAP_MARGIN)
            .min_by(|a, b| {
                card_turned(a)
                    .centre
                    .distance_sq(at)
                    .total_cmp(&card_turned(b).centre.distance_sq(at))
            })
    });
    match nearest {
        Some(seat) => Spot::Seat(seat.id.clone()),
        None => Spot::Loose(Point { x: at.x, y: at.y }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Layout;

    fn scene() -> Scene {
        let project = Project {
            layout: Layout::simple(2, 3),
            ..Project::new()
        };
        Scene::build(&project, None, None, None)
    }

    fn loose(at: Pos2) -> Spot {
        Spot::Loose(Point { x: at.x, y: at.y })
    }

    fn assert_clear(scene: &Scene, targets: &Targets) {
        for (_, spot) in targets {
            let Spot::Loose(point) = spot else { continue };
            let card =
                Rect::from_center_size(Pos2::new(point.x, point.y), Vec2::new(CARD_W, CARD_H));
            for seat in &scene.seats {
                assert!(!card.intersects(card_rect(seat)));
                assert!(!card.intersects(scene.table_body(&seat.table_id).unwrap()));
            }
            for at in scene.loose.values() {
                assert!(!card.intersects(Rect::from_center_size(*at, Vec2::new(CARD_W, CARD_H))));
            }
        }
    }

    #[test]
    fn floating_cards_clear_table_centres_and_partial_overlaps() {
        let scene = scene();
        let body = scene.table_body("table-1").unwrap();
        let chair = card_rect(&scene.seats[0]);
        for at in [
            body.center(),
            chair.left_center() - Vec2::new(CARD_W / 2.0 - 1.0, 0.0),
        ] {
            let mut targets = landing(&scene, &[0], at);
            assert!(matches!(targets[0].1, Spot::Loose(_)));
            clear_of_obstacles(&scene, &mut targets);
            assert_ne!(targets[0].1, loose(at));
            assert_clear(&scene, &targets);
        }
    }

    #[test]
    fn chair_gaps_and_nearby_drops_snap_to_the_closest_seat() {
        let scene = scene();
        let first = &scene.seats[0];
        let second = &scene.seats[1];
        let card = card_rect(first);
        let gap = card.center() + Vec2::new(0.0, SEAT_PITCH / 2.0);
        assert_eq!(spot_at(&scene, gap - Vec2::Y), Spot::Seat(first.id.clone()));
        assert_eq!(
            spot_at(&scene, gap + Vec2::Y),
            Spot::Seat(second.id.clone())
        );
        assert_eq!(
            spot_at(&scene, card.left_center() - Vec2::new(16.0, 0.0)),
            Spot::Seat(first.id.clone()),
        );
        // Outside the snap margin, the floor remains available.
        let floor = card.left_center() - Vec2::new(30.0, 0.0);
        assert_eq!(spot_at(&scene, floor), loose(floor));
    }

    #[test]
    fn clearing_a_table_and_a_loose_card_keeps_the_block_together() {
        let mut scene = scene();
        let body = scene.table_body("table-1").unwrap();
        let at = body.center();
        scene
            .loose
            .insert(2, Pos2::new(at.x, body.bottom() + CARD_H));
        let mut targets = vec![(0, loose(at)), (1, loose(at + Vec2::new(0.0, SEAT_PITCH)))];
        clear_of_obstacles(&scene, &mut targets);
        assert_clear(&scene, &targets);
        let (Spot::Loose(a), Spot::Loose(b)) = (&targets[0].1, &targets[1].1) else {
            panic!("both cards stay on the floor");
        };
        assert_eq!(a.x, b.x);
        assert_eq!(b.y - a.y, SEAT_PITCH);
    }

    #[test]
    fn clearing_one_table_does_not_leave_the_card_on_the_next_row() {
        let mut scene = scene();
        let first = scene.table_body("table-1").unwrap();
        let second = scene.table_body("table-2").unwrap();
        for seat in &mut scene.seats {
            if seat.table_id == "table-2" {
                seat.x -= second.left() - first.left();
                seat.y += 200.0;
            }
        }
        let mut targets = landing(&scene, &[0], first.center());
        clear_of_obstacles(&scene, &mut targets);
        assert_clear(&scene, &targets);
    }

    #[test]
    fn seat_drops_and_clear_floor_positions_stay_where_requested() {
        let scene = scene();
        let mut targets = landing(&scene, &[0], card_rect(&scene.seats[0]).center());
        let expected = targets.clone();
        clear_of_obstacles(&scene, &mut targets);
        assert_eq!(targets, expected);
        assert_eq!(targets[0].1, Spot::Seat(scene.seats[0].id.clone()));

        let floor = Pos2::new(-400.0, SEAT_PITCH);
        let mut targets = landing(&scene, &[0], floor);
        clear_of_obstacles(&scene, &mut targets);
        assert_eq!(targets, vec![(0, loose(floor))]);
        assert_clear(&scene, &targets);
    }
}
