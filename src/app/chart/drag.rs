//! Dragging people: one person or the selected block, from seats, from the
//! chart floor or from the "Not seated" list, onto seats, onto the floor, or
//! back into the list.

use super::*;
use crate::app::camera::Viewport;
use crate::model::Point;

/// People being dragged.
pub(in crate::app) struct Drag {
    /// Everyone moving, the person grabbed first; the others keep their
    /// place relative to that person.
    pub(in crate::app) people: Vec<usize>,
    /// From the grabbed card's centre to the pointer, in chart units.
    pub(in crate::app) grip: Vec2,
}

/// Space between cards stacked when people without a place are dropped together.
const STACK_GAP: f32 = 8.0;

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
        let world = view.world(pointer);
        let anchor = match scene.seat_at(world) {
            Some(seat) => card_rect(seat).center(),
            None => world - drag.grip,
        };
        Some(landing(scene, &drag.people, anchor))
    }

    /// Ghost cards where the people would land, and what the drop would do.
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
        let size = Vec2::new(CARD_W, CARD_H) * view.zoom.max(0.7);
        for (i, &person) in drag.people.iter().enumerate() {
            let target = targets.and_then(|t| t.iter().find(|(p, _)| *p == person));
            let at = match target.map(|(_, spot)| spot) {
                Some(Spot::Seat(id)) => scene.seat(id).map(|s| view.point(card_rect(s).center())),
                Some(Spot::Loose(point)) => Some(view.point(Pos2::new(point.x, point.y))),
                _ => None,
            };
            let at = at.unwrap_or(pointer + Vec2::new(0.0, i as f32 * (size.y + STACK_GAP)));
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
        // Where a loose card lies does not change the score.
        let key: Targets = targets
            .iter()
            .map(|(person, spot)| match spot {
                Spot::Loose(_) => (*person, Spot::Unseated),
                _ => (*person, spot.clone()),
            })
            .collect();
        let (project, candidate) = (&self.project, self.selected_candidate);
        let preview = self
            .chart
            .drop_preview
            .get((self.file.revision, candidate, key), || {
                preview(project, candidate, targets)
            });
        if let Some(preview) = preview {
            let (text, color) = preview_text(preview, p);
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

/// Each person's spot when the first one's card lands at `anchor`. The
/// others keep their offset from the first; people without a place on the
/// chart stack below.
fn landing(scene: &Scene, people: &[usize], anchor: Pos2) -> Targets {
    let origin = scene.card_centre(people[0]);
    let mut stacked = usize::from(origin.is_some());
    people
        .iter()
        .map(|&person| {
            let at = match (origin, scene.card_centre(person)) {
                (Some(origin), Some(centre)) => anchor + (centre - origin),
                _ => {
                    stacked += 1;
                    anchor + Vec2::new(0.0, (stacked - 1) as f32 * (CARD_H + STACK_GAP))
                }
            };
            (person, spot_at(scene, at))
        })
        .collect()
}

/// The seat whose card covers a point, or that point on the floor.
fn spot_at(scene: &Scene, at: Pos2) -> Spot {
    match scene.seat_at(at) {
        Some(seat) => Spot::Seat(seat.id.clone()),
        None => Spot::Loose(Point { x: at.x, y: at.y }),
    }
}
