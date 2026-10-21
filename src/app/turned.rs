//! Turned tables and cards: painting something upright and then turning
//! it, and hit tests in a turned shape's own frame.

use eframe::egui::{self, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, emath::Rot2};

use super::camera::Viewport;
use crate::model::{Seat, TablePosition, TableShape, TableSpec};

/// A seat card on the chart, in world units.
pub(super) const CARD_W: f32 = 176.0;
pub(super) const CARD_H: f32 = 44.0;
/// How far a seat's card centre sits outward from its seat coordinate, so
/// the card lies just outside the table edge.
pub(super) const CARD_OFFSET: f32 = CARD_W / 2.0 + 6.0;

/// The card drawn for a seat: just outside the table edge, turned to point
/// away from the table, its text never upside down.
pub(super) fn seat_card(seat: &Seat) -> Turned {
    let out = Vec2::angled(seat.out);
    Turned {
        centre: Pos2::new(seat.x, seat.y) + out * CARD_OFFSET,
        size: Vec2::new(CARD_W, CARD_H),
        angle: readable(seat.out),
    }
}

/// An angle turned by half a circle if that keeps text from reading upside
/// down: within -90°..90°, and 90° rather than -90° so both sides of a
/// table turned sideways read the same way.
fn readable(angle: f32) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    const EPSILON: f32 = 1e-3;
    let angle = angle.rem_euclid(PI);
    if (angle - FRAC_PI_2).abs() < EPSILON {
        FRAC_PI_2
    } else if angle < EPSILON || PI - angle < EPSILON {
        0.0
    } else if angle > FRAC_PI_2 {
        angle - PI
    } else {
        angle
    }
}

/// Paints with `paint`, then turns everything it painted by `angle` radians
/// clockwise around `pivot`. `paint` gets a painter that clips nothing yet:
/// the clip of `painter` applies after turning.
pub(super) fn paint_turned<R>(
    painter: &egui::Painter,
    angle: f32,
    pivot: Pos2,
    paint: impl FnOnce(&egui::Painter) -> R,
) -> R {
    if angle == 0.0 {
        return paint(painter);
    }
    let outer = painter.clip_rect();
    let mut unclipped = painter.clone();
    unclipped.set_clip_rect(Rect::EVERYTHING);
    let layer = painter.layer_id();
    let ctx = painter.ctx();
    let start = ctx.graphics_mut(|g| g.entry(layer).next_idx().0);
    let result = paint(&unclipped);
    let rot = Rot2::from_angle(angle);
    ctx.graphics_mut(|g| {
        let list = g.entry(layer);
        for i in start..list.next_idx().0 {
            list.mutate_shape(egui::layers::ShapeIdx(i), |s| {
                turn(&mut s.shape, rot, pivot);
                // A clip inside the painter's (around a name) turns with it.
                let inner = if s.clip_rect == Rect::EVERYTHING {
                    outer
                } else {
                    let clip = Turned {
                        centre: pivot + rot * (s.clip_rect.center() - pivot),
                        size: s.clip_rect.size(),
                        angle,
                    };
                    clip.bounds()
                };
                s.clip_rect = inner.intersect(outer);
            });
        }
    });
    result
}

fn turn(shape: &mut Shape, rot: Rot2, pivot: Pos2) {
    let at = |p: Pos2| pivot + rot * (p - pivot);
    match shape {
        Shape::Vec(shapes) => shapes.iter_mut().for_each(|s| turn(s, rot, pivot)),
        Shape::Circle(circle) => circle.center = at(circle.center),
        Shape::Ellipse(ellipse) => {
            ellipse.center = at(ellipse.center);
            ellipse.angle += rot.angle();
        }
        Shape::LineSegment { points, .. } => points.iter_mut().for_each(|p| *p = at(*p)),
        Shape::Path(path) => path.points.iter_mut().for_each(|p| *p = at(*p)),
        Shape::Rect(rect) => {
            rect.rect = Rect::from_center_size(at(rect.rect.center()), rect.rect.size());
            rect.angle += rot.angle();
        }
        Shape::Text(text) => {
            text.pos = at(text.pos);
            text.angle += rot.angle();
        }
        Shape::Mesh(mesh) => std::sync::Arc::make_mut(mesh).rotate(rot, pivot),
        _ => {}
    }
}

/// A rectangle of `size` centred on `centre` and turned by `angle` radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Turned {
    pub(super) centre: Pos2,
    pub(super) size: Vec2,
    pub(super) angle: f32,
}

impl Turned {
    /// The rectangle before turning.
    pub(super) fn upright(&self) -> Rect {
        Rect::from_center_size(self.centre, self.size)
    }

    /// The smallest upright rectangle around the turned one.
    pub(super) fn bounds(&self) -> Rect {
        let (sin, cos) = (self.angle.sin().abs(), self.angle.cos().abs());
        let size = Vec2::new(
            self.size.x * cos + self.size.y * sin,
            self.size.x * sin + self.size.y * cos,
        );
        Rect::from_center_size(self.centre, size)
    }

    /// A point relative to the centre, in the rectangle's own frame.
    fn local(&self, at: Pos2) -> Vec2 {
        Rot2::from_angle(-self.angle) * (at - self.centre)
    }

    pub(super) fn contains(&self, at: Pos2) -> bool {
        let local = self.local(at);
        local.x.abs() <= self.size.x / 2.0 && local.y.abs() <= self.size.y / 2.0
    }

    pub(super) fn distance_sq(&self, at: Pos2) -> f32 {
        Rect::from_center_size(Pos2::ZERO, self.size).distance_sq_to_pos(self.local(at).to_pos2())
    }

    fn corners(&self) -> [Pos2; 4] {
        let rot = Rot2::from_angle(self.angle);
        let (w, h) = (self.size.x / 2.0, self.size.y / 2.0);
        [(-w, -h), (w, -h), (w, h), (-w, h)].map(|(x, y)| self.centre + rot * Vec2::new(x, y))
    }

    /// Whether two turned rectangles share any area: no edge direction of
    /// either separates them.
    pub(super) fn overlaps(&self, other: &Turned) -> bool {
        if !self.bounds().intersects(other.bounds()) {
            return false;
        }
        let (a, b) = (self.corners(), other.corners());
        let axes = [self.angle, other.angle]
            .into_iter()
            .flat_map(|angle| [Vec2::angled(angle), Vec2::angled(angle).rot90()]);
        let span = |corners: &[Pos2; 4], axis: Vec2| {
            let along = corners.map(|c| c.to_vec2().dot(axis));
            let min = along.iter().copied().fold(f32::INFINITY, f32::min);
            let max = along.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            (min, max)
        };
        axes.into_iter().all(|axis| {
            let ((a0, a1), (b0, b1)) = (span(&a, axis), span(&b, axis));
            // Touching edges are not an overlap.
            a0 < b1 - 0.01 && b0 < a1 - 0.01
        })
    }

    /// Turned by a multiple of a quarter, so `bounds` is exact.
    pub(super) fn square(&self) -> bool {
        let quarter = std::f32::consts::FRAC_PI_2;
        let rest = self.angle.rem_euclid(quarter);
        rest < 1e-3 || quarter - rest < 1e-3
    }

    /// Where to listen for the pointer. Cards on a slanted table overlap in
    /// their bounds, so only the one under the pointer gets its bounds and the
    /// others a square that fits inside them.
    pub(super) fn hit_rect(&self, pointer: Option<Pos2>) -> Rect {
        if self.square() || pointer.is_some_and(|at| self.contains(at)) {
            return self.bounds();
        }
        let (sin, cos) = (self.angle.sin().abs(), self.angle.cos().abs());
        let side = self.size.min_elem() / (sin + cos);
        Rect::from_center_size(self.centre, Vec2::splat(side))
    }
}

/// A table's top, before or after turning, in world units.
#[derive(Clone, Copy, Debug)]
pub(super) struct TableTop {
    pub(super) top: Turned,
    pub(super) round: bool,
}

impl TableTop {
    /// The top reaching `margin` past the outermost seats along a long
    /// table, or out to the seats of a round one.
    pub(super) fn new(table: &TableSpec, frame: TablePosition, margin: f32) -> Self {
        let (across, along) = table.half_size();
        let round = table.shape == TableShape::Round;
        let size = if round {
            Vec2::splat(2.0 * across)
        } else {
            Vec2::new(2.0 * across, 2.0 * (along + margin))
        };
        Self {
            top: Turned {
                centre: Pos2::new(frame.x, frame.y),
                size,
                angle: frame.radians(),
            },
            round,
        }
    }

    /// The same top with every length multiplied by `factor`.
    pub(super) fn scaled(self, factor: f32) -> Self {
        let top = Turned {
            centre: (self.top.centre.to_vec2() * factor).to_pos2(),
            size: self.top.size * factor,
            ..self.top
        };
        Self { top, ..self }
    }

    pub(super) fn contains(&self, at: Pos2) -> bool {
        if self.round {
            at.distance(self.top.centre) <= self.top.size.x / 2.0
        } else {
            self.top.contains(at)
        }
    }

    pub(super) fn bounds(&self) -> Rect {
        if self.round {
            self.top.upright()
        } else {
            self.top.bounds()
        }
    }

    pub(super) fn paint(
        &self,
        painter: &egui::Painter,
        view: &Viewport,
        fill: egui::Color32,
        stroke: Stroke,
        radius: u8,
    ) {
        let centre = view.point(self.top.centre);
        if self.round {
            painter.circle(centre, self.top.size.x / 2.0 * view.zoom, fill, stroke);
            return;
        }
        let rect = Rect::from_center_size(centre, self.top.size * view.zoom);
        painter.add(
            egui::epaint::RectShape::new(rect, radius, fill, stroke, StrokeKind::Outside)
                .with_angle(self.top.angle),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turned_card_is_hit_in_its_own_frame() {
        let card = Turned {
            centre: Pos2::new(100.0, 100.0),
            size: Vec2::new(176.0, 44.0),
            angle: std::f32::consts::FRAC_PI_2,
        };
        assert!(card.square());
        assert!(card.contains(Pos2::new(100.0, 180.0)));
        assert!(!card.contains(Pos2::new(180.0, 100.0)));
        let bounds = card.bounds();
        assert!((bounds.width() - 44.0).abs() < 0.01 && (bounds.height() - 176.0).abs() < 0.01);
        let slanted = Turned {
            angle: std::f32::consts::FRAC_PI_4,
            ..card
        };
        assert!(!slanted.square());
        let corner = bounds.min;
        assert!(!slanted.contains(corner));
        assert!(slanted.hit_rect(None).width() < 44.0);
        assert_eq!(slanted.hit_rect(Some(slanted.centre)), slanted.bounds());
        let beside = Turned {
            centre: card.centre + Vec2::new(44.0, 0.0),
            ..card
        };
        assert!(!card.overlaps(&beside));
        let nudged = Turned {
            centre: card.centre + Vec2::new(40.0, 0.0),
            ..card
        };
        assert!(card.overlaps(&nudged));
        // Slanted cards one seat apart along their table clear each other,
        // though their bounds overlap.
        let step = Vec2::angled(slanted.angle).rot90() * 56.0;
        let next = Turned {
            centre: slanted.centre + step,
            ..slanted
        };
        assert!(slanted.bounds().intersects(next.bounds()));
        assert!(!slanted.overlaps(&next));
    }
}
