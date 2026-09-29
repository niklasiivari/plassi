//! Seat geometry: where seats are and how far apart.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::*;

/// Vertical distance between neighbouring seats on one side of a table.
pub const SEAT_PITCH: f32 = 56.0;
/// Horizontal distance between the left and right seat columns of a table.
pub const TABLE_SPAN: f32 = 96.0;
/// Horizontal space between neighbouring tables' seat columns; leaves room for
/// name cards on both sides plus an aisle.
const TABLE_GAP: f32 = 440.0;
const ROW_GAP: f32 = 170.0;

impl Layout {
    /// One row of `tables` tables with `per_side` seats on each side.
    pub fn simple(tables: usize, per_side: usize) -> Self {
        let tables = (1..=tables)
            .map(|i| TableSpec { id: format!("table-{i}"), label: crate::i18n::fill("Table {}", &[&i]), left: per_side, right: per_side })
            .collect();
        Self { rows: vec![RowSpec { id: "row-1".into(), label: crate::i18n::fill("Row {}", &[&1]), tables }], ..Default::default() }
    }

    /// A room for `people`: tables of up to 10 seats per side, as few as fit.
    pub fn suggested(people: usize) -> (usize, usize) {
        let people = people.max(2);
        let tables = people.div_ceil(20);
        (tables, people.div_ceil(tables * 2))
    }

    pub fn seats(&self) -> Vec<Seat> {
        let mut result = Vec::new();
        let mut row_y = 0.0_f32;
        for row in &self.rows {
            let mut table_x = 0.0_f32;
            let mut row_height = 0.0_f32;
            for table in &row.tables {
                let mut push = |side: Side, index: usize, suffix: &str, x: f32, y: f32| {
                    let id = format!("{}:{suffix}:{index}", table.id);
                    let tags = self.seat_tags.get(&id).cloned().unwrap_or_default();
                    result.push(Seat {
                        id,
                        row_id: row.id.clone(),
                        table_id: table.id.clone(),
                        x,
                        y,
                        side,
                        index,
                        tags,
                    });
                };
                for index in 0..table.left {
                    push(
                        Side::Left,
                        index,
                        "left",
                        table_x,
                        row_y + index as f32 * SEAT_PITCH,
                    );
                }
                for index in 0..table.right {
                    let x = table_x + TABLE_SPAN;
                    push(
                        Side::Right,
                        index,
                        "right",
                        x,
                        row_y + index as f32 * SEAT_PITCH,
                    );
                }
                let length = table.left.max(table.right).max(1) as f32;
                row_height = row_height.max((length - 1.0) * SEAT_PITCH);
                table_x += TABLE_SPAN + TABLE_GAP;
            }
            row_y += row_height + ROW_GAP;
        }
        result
    }

    pub fn seat_count(&self) -> usize {
        self.rows.iter().flat_map(|r| &r.tables).map(|t| t.left + t.right).sum()
    }
}

pub fn area_contains(area: &Area, seat: &Seat, seats: &[Seat]) -> bool {
    match area {
        Area::Seat(id) => seat.id == *id,
        Area::Table(id) => seat.table_id == *id,
        Area::Row(id) => seat.row_id == *id,
        Area::Tag(tag) => seat.tags.iter().any(|t| t == tag),
        Area::RowCenter(id) => {
            if seat.row_id != *id {
                return false;
            }
            let row_seats: Vec<_> = seats.iter().filter(|s| s.row_id == *id).collect();
            if row_seats.is_empty() {
                return false;
            }
            let min = row_seats.iter().map(|s| s.x).fold(f32::INFINITY, f32::min);
            let max = row_seats
                .iter()
                .map(|s| s.x)
                .fold(f32::NEG_INFINITY, f32::max);
            (seat.x - (min + max) / 2.0).abs() <= (TABLE_SPAN + TABLE_GAP) / 2.0
        }
    }
}

/// For each seat, the seats directly beside it on the same side.
pub fn beside_lists(layout: &Layout, seats: &[Seat]) -> Vec<Vec<usize>> {
    seats
        .iter()
        .map(|a| {
            (0..seats.len())
                .filter(|&j| {
                    seats[j].id != a.id
                        && distance_class(layout, a, &seats[j]) == DistanceClass::Beside
                })
                .collect()
        })
        .collect()
}

/// How close a seat is to the end of its occupied row: 2 = at an end (nobody
/// beside on one side), 1 = next to an end, 0 = inside.
pub fn end_level(seat: usize, beside: &[Vec<usize>], occupied: &dyn Fn(usize) -> bool) -> u8 {
    if beside[seat].is_empty() {
        return 0;
    }
    let neighbours = |s: usize| beside[s].iter().filter(|&&n| occupied(n)).count();
    if neighbours(seat) < 2 {
        2
    } else if beside[seat].iter().any(|&n| neighbours(n) < 2) {
        1
    } else {
        0
    }
}

/// Empty seats with someone seated further down the same side. Spare seats
/// belong at table bottoms, so each of these is a gap.
pub fn gaps<'a>(seats: &'a [Seat], assignment: &BTreeMap<usize, String>) -> Vec<&'a Seat> {
    let taken: HashSet<&str> = assignment.values().map(String::as_str).collect();
    let mut deepest: HashMap<(&str, Side), usize> = HashMap::new();
    for seat in seats.iter().filter(|s| taken.contains(s.id.as_str())) {
        let depth = deepest.entry((seat.table_id.as_str(), seat.side)).or_default();
        *depth = (*depth).max(seat.index + 1);
    }
    seats
        .iter()
        .filter(|s| !taken.contains(s.id.as_str()))
        .filter(|s| deepest.get(&(s.table_id.as_str(), s.side)).is_some_and(|d| s.index + 1 < *d))
        .collect()
}

pub fn distance_class(layout: &Layout, a: &Seat, b: &Seat) -> DistanceClass {
    if let Some(relation) = layout
        .relations
        .iter()
        .find(|r| (r.a == a.id && r.b == b.id) || (r.a == b.id && r.b == a.id))
    {
        return relation.class;
    }
    table_distance(a, b)
}

/// Distance from the table geometry alone, ignoring hand-set relations.
pub fn table_distance(a: &Seat, b: &Seat) -> DistanceClass {
    if a.table_id != b.table_id {
        return DistanceClass::Far;
    }
    match (a.side, b.side) {
        (Side::Left, Side::Left) | (Side::Right, Side::Right) if a.index.abs_diff(b.index) == 1 => {
            DistanceClass::Beside
        }
        (Side::Left, Side::Right) | (Side::Right, Side::Left) => match a.index.abs_diff(b.index) {
            0 => DistanceClass::Opposite,
            1 => DistanceClass::Diagonal,
            gap if gap <= NEAR_ROWS => DistanceClass::Near,
            _ => DistanceClass::SameTable,
        },
        _ if a.index.abs_diff(b.index) <= NEAR_ROWS => DistanceClass::Near,
        _ => DistanceClass::SameTable,
    }
}
