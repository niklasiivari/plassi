//! Table shapes: where seats are around a table's centre, how a turned table
//! places them in the room, and how far apart seats at a round table are.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use super::*;

/// A seat's place around a round table, clockwise from the top.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ring {
    pub position: usize,
    pub seats: usize,
}

/// The radius of a round table's seat circle: a seat pitch apart along the edge.
pub fn round_radius(seats: usize) -> f32 {
    (seats as f32 * SEAT_PITCH / TAU).max(TABLE_SPAN / 2.0)
}

impl TableSpec {
    /// Half the table's size before it is turned: across and along a long
    /// table between its outermost seats, the radius twice for a round one.
    pub fn half_size(&self) -> (f32, f32) {
        match self.shape {
            TableShape::Long => (
                TABLE_SPAN / 2.0,
                self.left.max(self.right).saturating_sub(1) as f32 * SEAT_PITCH / 2.0,
            ),
            TableShape::Round => {
                let radius = round_radius(self.left + self.right);
                (radius, radius)
            }
        }
    }

    /// Place around a round table, clockwise from the top: the right side
    /// runs down, the left side back up, so both sides' last seats meet at
    /// the bottom.
    pub fn ring_position(&self, side: Side, index: usize) -> usize {
        match side {
            Side::Right => index,
            Side::Left => self.left + self.right - 1 - index,
        }
    }

    /// A seat relative to the table centre before turning, and the direction
    /// away from the table.
    pub(crate) fn local_seat(&self, side: Side, index: usize) -> (f32, f32, f32) {
        match self.shape {
            TableShape::Long => {
                let (across, along) = self.half_size();
                let y = index as f32 * SEAT_PITCH - along;
                match side {
                    Side::Left => (-across, y, PI),
                    Side::Right => (across, y, 0.0),
                }
            }
            TableShape::Round => {
                let seats = self.left + self.right;
                let radius = round_radius(seats);
                let angle = (self.ring_position(side, index) as f32 + 0.5) / seats as f32 * TAU;
                (
                    radius * angle.sin(),
                    -radius * angle.cos(),
                    angle - FRAC_PI_2,
                )
            }
        }
    }

    pub(super) fn ring(&self, side: Side, index: usize) -> Option<Ring> {
        (self.shape == TableShape::Round).then(|| Ring {
            position: self.ring_position(side, index),
            seats: self.left + self.right,
        })
    }
}

impl TablePosition {
    pub fn radians(&self) -> f32 {
        self.rotation.to_radians()
    }

    /// A point given relative to the table centre, turned with the table.
    pub fn place(&self, x: f32, y: f32) -> (f32, f32) {
        let (sin, cos) = self.radians().sin_cos();
        (self.x + x * cos - y * sin, self.y + x * sin + y * cos)
    }
}

/// Rotation in degrees, kept within 0..360.
pub fn normal_degrees(degrees: f32) -> f32 {
    degrees.rem_euclid(360.0)
}

/// Seats at one round table: the next seat around is beside, and the whole
/// table counts as together, like across a long table.
pub(super) fn ring_distance(a: Ring, b: Ring) -> DistanceClass {
    let apart = a.position.abs_diff(b.position);
    if apart.min(a.seats - apart) == 1 {
        DistanceClass::Beside
    } else {
        DistanceClass::Opposite
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(seats: usize) -> Layout {
        let mut layout = Layout::simple(1, 0);
        let table = &mut layout.rows[0].tables[0];
        table.left = seats.div_ceil(2);
        table.right = seats / 2;
        table.shape = TableShape::Round;
        layout
    }

    #[test]
    fn round_table_neighbours_wrap_around_and_the_rest_count_as_together() {
        let layout = round(8);
        let seats = layout.seats();
        let seat = |id: &str| seats.iter().find(|s| s.id == id).unwrap();
        let class = |a, b| table_distance(seat(a), seat(b));
        assert_eq!(
            class("table-1:right:0", "table-1:left:0"),
            DistanceClass::Beside
        );
        assert_eq!(
            class("table-1:right:3", "table-1:left:3"),
            DistanceClass::Beside
        );
        assert_eq!(
            class("table-1:right:0", "table-1:right:1"),
            DistanceClass::Beside
        );
        assert_eq!(
            class("table-1:right:0", "table-1:left:2"),
            DistanceClass::Opposite
        );
        let beside = beside_lists(&seats);
        assert!(beside.iter().all(|b| b.len() == 2));
    }

    #[test]
    fn round_seats_lie_on_a_circle_with_the_right_side_first_clockwise() {
        let layout = round(6);
        let radius = round_radius(6);
        let centre = layout.table_frames()["table-1"];
        for seat in layout.seats() {
            let (x, y) = (seat.x - centre.x, seat.y - centre.y);
            assert!((x.hypot(y) - radius).abs() < 0.01);
            let (dx, dy) = (seat.out.cos(), seat.out.sin());
            assert!((dx * radius - x).abs() < 0.01 && (dy * radius - y).abs() < 0.01);
        }
        let right = layout
            .seats()
            .into_iter()
            .find(|s| s.id == "table-1:right:0");
        assert!(right.unwrap().x > centre.x);
    }

    #[test]
    fn turning_a_table_turns_its_seats_around_the_centre() {
        let mut layout = Layout::simple(1, 2);
        layout.set_free_layout(true);
        let before = layout.seats();
        let at = layout.table_positions.get_mut("table-1").unwrap();
        at.rotation = 90.0;
        let centre = (at.x, at.y);
        for (a, b) in before.iter().zip(layout.seats()) {
            // A quarter turn clockwise: (x, y) → (-y, x) around the centre.
            let (x, y) = (a.x - centre.0, a.y - centre.1);
            assert!((b.x - centre.0 + y).abs() < 0.01 && (b.y - centre.1 - x).abs() < 0.01);
            assert!((b.out - a.out - FRAC_PI_2).abs() < 0.01);
        }
    }
}
