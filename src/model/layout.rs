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
            .map(|i| TableSpec {
                id: format!("table-{i}"),
                label: crate::i18n::fill("Table {}", &[&i]),
                left: per_side,
                right: per_side,
            })
            .collect();
        Self {
            rows: vec![RowSpec {
                id: "row-1".into(),
                label: crate::i18n::fill("Row {}", &[&1]),
                tables,
            }],
            ..Default::default()
        }
    }

    /// Generate rows without changing the stable table and seat IDs.
    pub fn grid(tables: usize, per_side: usize, columns: usize) -> Self {
        let mut layout = Self::simple(tables, per_side);
        let tables = std::mem::take(&mut layout.rows[0].tables);
        layout.rows = tables
            .chunks(columns.max(1))
            .enumerate()
            .map(|(i, tables)| RowSpec {
                id: format!("row-{}", i + 1),
                label: crate::i18n::fill("Row {}", &[&(i + 1)]),
                tables: tables.to_vec(),
            })
            .collect();
        layout
    }

    /// Start free placement from the current geometry, including tables without seats.
    pub fn set_free_layout(&mut self, free: bool) {
        if free && !self.free_layout {
            for (id, at) in self.row_positions() {
                self.table_positions.entry(id).or_insert(at);
            }
        }
        self.free_layout = free;
    }

    fn row_positions(&self) -> BTreeMap<String, TablePosition> {
        let mut positions = BTreeMap::new();
        let mut y = 0.0;
        for row in &self.rows {
            let mut height = 0.0_f32;
            for (i, table) in row.tables.iter().enumerate() {
                positions.insert(
                    table.id.clone(),
                    TablePosition {
                        x: i as f32 * (TABLE_SPAN + TABLE_GAP),
                        y,
                    },
                );
                height = height.max((table.left.max(table.right).max(1) - 1) as f32 * SEAT_PITCH);
            }
            y += height + ROW_GAP;
        }
        positions
    }

    pub fn table_origins(&self) -> BTreeMap<String, TablePosition> {
        let mut positions = self.row_positions();
        if self.free_layout {
            for (id, position) in &mut positions {
                if let Some(custom) = self.table_positions.get(id) {
                    *position = *custom;
                }
            }
        }
        positions
    }

    /// Move the table intact; assignments, tags and table rules use stable IDs.
    pub fn move_table(&mut self, table_id: &str, row_id: &str) -> bool {
        if !self.rows.iter().any(|r| r.id == row_id) {
            return false;
        }
        let Some(source) = self
            .rows
            .iter()
            .position(|r| r.tables.iter().any(|t| t.id == table_id))
        else {
            return false;
        };
        if self.rows[source].id == row_id {
            return false;
        }
        let index = self.rows[source]
            .tables
            .iter()
            .position(|t| t.id == table_id)
            .unwrap();
        let table = self.rows[source].tables.remove(index);
        self.rows
            .iter_mut()
            .find(|r| r.id == row_id)
            .unwrap()
            .tables
            .push(table);
        true
    }

    /// A room for `people`: tables of up to 10 seats per side, as few as fit.
    pub fn suggested(people: usize) -> (usize, usize) {
        let people = people.max(2);
        let tables = people.div_ceil(20);
        (tables, people.div_ceil(tables * 2))
    }

    pub fn seats(&self) -> Vec<Seat> {
        let mut result = Vec::new();
        let origins = self.table_origins();
        for row in &self.rows {
            for table in &row.tables {
                let TablePosition {
                    x: table_x,
                    y: row_y,
                } = origins[&table.id];
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
            }
        }
        result
    }

    pub fn seat_count(&self) -> usize {
        self.rows
            .iter()
            .flat_map(|r| &r.tables)
            .map(|t| t.left + t.right)
            .sum()
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

/// The table a seat belongs to; seat IDs are "table:side:index".
pub fn seat_table(seat_id: &str) -> &str {
    seat_id.split(':').next().unwrap_or(seat_id)
}

/// Empty seats with someone seated further down the same side. Spare seats
/// belong at table bottoms, so each of these is a gap.
pub fn gaps<'a>(seats: &'a [Seat], assignment: &BTreeMap<usize, String>) -> Vec<&'a Seat> {
    let taken: HashSet<&str> = assignment.values().map(String::as_str).collect();
    let mut deepest: HashMap<(&str, Side), usize> = HashMap::new();
    for seat in seats.iter().filter(|s| taken.contains(s.id.as_str())) {
        let depth = deepest
            .entry((seat.table_id.as_str(), seat.side))
            .or_default();
        *depth = (*depth).max(seat.index + 1);
    }
    seats
        .iter()
        .filter(|s| !taken.contains(s.id.as_str()))
        .filter(|s| {
            deepest
                .get(&(s.table_id.as_str(), s.side))
                .is_some_and(|d| s.index + 1 < *d)
        })
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

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn generated_rows_keep_every_table_and_seat_id() {
        let flat = Layout::simple(17, 4);
        let grid = Layout::grid(17, 4, 4);
        assert_eq!(
            grid.rows.iter().map(|r| r.tables.len()).collect::<Vec<_>>(),
            [4, 4, 4, 4, 1]
        );
        assert_eq!(
            flat.seats().iter().map(|s| &s.id).collect::<Vec<_>>(),
            grid.seats().iter().map(|s| &s.id).collect::<Vec<_>>()
        );
        assert!(
            grid.seats()
                .iter()
                .find(|s| s.table_id == "table-5")
                .unwrap()
                .y
                > 0.0
        );
    }

    #[test]
    fn moving_between_rows_preserves_seats_and_free_positions_survive_serialization() {
        let mut layout = Layout::grid(3, 2, 2);
        let ids: Vec<_> = layout.seats().iter().map(|s| s.id.clone()).collect();
        assert!(layout.move_table("table-1", "row-2"));
        assert!(!layout.move_table("table-2", "missing"));
        assert_eq!(layout.seat_count(), 12);
        let mut after: Vec<_> = layout.seats().iter().map(|s| s.id.clone()).collect();
        let mut before = ids;
        before.sort();
        after.sort();
        assert_eq!(before, after);
        assert!(
            layout
                .seats()
                .iter()
                .filter(|s| s.table_id == "table-1")
                .all(|s| s.row_id == "row-2")
        );
        let row_seats = layout.seats();
        layout.set_free_layout(true);
        assert_eq!(
            layout
                .seats()
                .iter()
                .map(|s| (s.x, s.y))
                .collect::<Vec<_>>(),
            row_seats.iter().map(|s| (s.x, s.y)).collect::<Vec<_>>()
        );
        layout.table_positions.insert(
            "table-1".into(),
            TablePosition {
                x: -240.0,
                y: 120.0,
            },
        );
        let layout: Layout =
            serde_json::from_str(&serde_json::to_string(&layout).unwrap()).unwrap();
        let seat = layout
            .seats()
            .into_iter()
            .find(|s| s.id == "table-1:left:0")
            .unwrap();
        assert_eq!((seat.x, seat.y), (-240.0, 120.0));
        let mut layout = layout;
        layout.set_free_layout(false);
        layout.set_free_layout(true);
        assert_eq!(
            layout.table_positions["table-1"],
            TablePosition {
                x: -240.0,
                y: 120.0
            }
        );
    }
}
