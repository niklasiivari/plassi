//! Room geometry retained across frames, including exact collision pairs.

use std::collections::{BTreeMap, BTreeSet};

use eframe::egui::{Pos2, Rect, Vec2};

use super::footprint::Footprint;
use crate::app::turned::TableTop;
use crate::model::{Layout, Seat, TablePosition, TableSpec};

pub(super) const TABLE_END: f32 = 28.0;
const CARD_MARGIN: f32 = 190.0;

pub(super) struct TableGeometry {
    pub(super) spec: TableSpec,
    pub(super) base: TablePosition,
    pub(super) frame: TablePosition,
    pub(super) base_bounds: Rect,
    pub(super) top: TableTop,
    pub(super) footprint: Footprint,
    pub(super) dots: Vec<Pos2>,
    seats: Vec<Seat>,
}

impl TableGeometry {
    fn set_frame(&mut self, frame: TablePosition) {
        self.frame = frame;
        self.top = TableTop::new(&self.spec, frame, TABLE_END);
        for seat in &mut self.seats {
            let (x, y, out) = self.spec.local_seat(seat.side, seat.index);
            (seat.x, seat.y) = frame.place(x, y);
            seat.out = out + frame.radians();
        }
        self.dots.clear();
        self.dots.extend(self.seats.iter().map(seat_dot));
        self.footprint.update(self.top, &self.seats);
    }
}

fn seat_dots(seats: &[Seat]) -> Vec<Pos2> {
    seats.iter().map(seat_dot).collect()
}

fn seat_dot(seat: &Seat) -> Pos2 {
    Pos2::new(seat.x, seat.y) + 12.0 * Vec2::angled(seat.out)
}

pub(super) struct RoomGeometry {
    /// Project order controls painting; sorted IDs control snap tie breaking.
    pub(super) tables: Vec<TableGeometry>,
    pub(super) by_id: BTreeMap<String, usize>,
    overlaps: BTreeSet<(usize, usize)>,
    pub(super) crowded: BTreeSet<String>,
    pub(super) bounds: Rect,
}

impl RoomGeometry {
    pub(super) fn new(layout: &Layout) -> Self {
        let frames = layout.table_frames();
        let mut seats = BTreeMap::<String, Vec<Seat>>::new();
        for seat in layout.seats() {
            seats.entry(seat.table_id.clone()).or_default().push(seat);
        }
        let tables: Vec<_> = layout
            .rows
            .iter()
            .flat_map(|row| &row.tables)
            .map(|spec| {
                let frame = frames[&spec.id];
                let top = TableTop::new(spec, frame, TABLE_END);
                let seats = seats.remove(&spec.id).unwrap_or_default();
                TableGeometry {
                    spec: spec.clone(),
                    base: frame,
                    frame,
                    base_bounds: top.bounds(),
                    top,
                    footprint: Footprint::new(top, &seats),
                    dots: seat_dots(&seats),
                    seats,
                }
            })
            .collect();
        let by_id = tables
            .iter()
            .enumerate()
            .map(|(i, table)| (table.spec.id.clone(), i))
            .collect();
        let mut geometry = Self {
            tables,
            by_id,
            overlaps: BTreeSet::new(),
            crowded: BTreeSet::new(),
            bounds: Rect::NOTHING,
        };
        let changed: Vec<_> = (0..geometry.tables.len()).collect();
        geometry.update_overlaps(&changed);
        geometry
    }

    pub(super) fn table(&self, id: &str) -> Option<&TableGeometry> {
        self.by_id.get(id).map(|&i| &self.tables[i])
    }

    /// Apply this frame's gesture overlay, restoring old previews that ended.
    /// Unmoved tables retain their seat geometry and collision results.
    pub(super) fn preview(&mut self, positions: &BTreeMap<String, TablePosition>) {
        let mut changed = Vec::new();
        for (i, table) in self.tables.iter_mut().enumerate() {
            let frame = positions.get(&table.spec.id).copied().unwrap_or(table.base);
            if table.frame != frame {
                table.set_frame(frame);
                changed.push(i);
            }
        }
        self.update_overlaps(&changed);
    }

    fn update_overlaps(&mut self, changed: &[usize]) {
        if changed.is_empty() {
            return;
        }
        self.bounds = self
            .tables
            .iter()
            .map(|table| table.top.bounds().expand2(Vec2::new(CARD_MARGIN, 42.0)))
            .fold(Rect::NOTHING, Rect::union);
        let changed: BTreeSet<_> = changed.iter().copied().collect();
        self.overlaps
            .retain(|(a, b)| !changed.contains(a) && !changed.contains(b));
        if changed.len() == self.tables.len() {
            // A full rebuild uses an exact broad phase. Distant tables need
            // no rectangle tests, and the overlap predicate stays unchanged.
            let mut order: Vec<_> = (0..self.tables.len()).collect();
            order.sort_by(|&a, &b| {
                self.tables[a]
                    .footprint
                    .bounds()
                    .min
                    .x
                    .total_cmp(&self.tables[b].footprint.bounds().min.x)
            });
            for (offset, &i) in order.iter().enumerate() {
                let right = self.tables[i].footprint.bounds().max.x;
                for &j in &order[offset + 1..] {
                    if self.tables[j].footprint.bounds().min.x > right {
                        break;
                    }
                    self.check_overlap(i, j);
                }
            }
        } else {
            for &i in &changed {
                for j in 0..self.tables.len() {
                    // Each changed pair is checked once, with stable argument order.
                    if i == j || j < i && changed.contains(&j) {
                        continue;
                    }
                    self.check_overlap(i, j);
                }
            }
        }
        self.crowded.clear();
        for &(a, b) in &self.overlaps {
            self.crowded.insert(self.tables[a].spec.id.clone());
            self.crowded.insert(self.tables[b].spec.id.clone());
        }
    }

    fn check_overlap(&mut self, i: usize, j: usize) {
        let (a, b) = (i.min(j), i.max(j));
        if self.tables[a].footprint.overlaps(&self.tables[b].footprint) {
            self.overlaps.insert((a, b));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TableShape;

    fn layout() -> Layout {
        let mut layout = Layout::simple(4, 4);
        layout.set_free_layout(true);
        let tables = &mut layout.rows[0].tables;
        tables[1].left = 7;
        tables[1].right = 3;
        tables[2].shape = TableShape::Round;
        tables[2].left = 5;
        tables[2].right = 4;
        tables[3].left = 0;
        tables[3].right = 0;
        for (i, position) in layout.table_positions.values_mut().enumerate() {
            position.x = i as f32 * 175.0;
            position.y = i as f32 * 85.0;
            position.rotation = [0.0, 45.0, 115.0, 270.0][i];
        }
        layout
    }

    fn assert_matches(geometry: &RoomGeometry, layout: &Layout) {
        let seats = layout.seats();
        for table in &geometry.tables {
            let expected: Vec<_> = seats
                .iter()
                .filter(|s| s.table_id == table.spec.id)
                .collect();
            assert_eq!(expected.len(), table.seats.len());
            for (a, b) in expected.iter().zip(&table.seats) {
                assert_eq!(a.id, b.id);
                assert_eq!(a.row_id, b.row_id);
                assert_eq!(a.table_id, b.table_id);
                assert_eq!((a.x, a.y, a.out), (b.x, b.y, b.out));
                assert_eq!((a.side, a.index, a.ring), (b.side, b.index, b.ring));
            }
        }
        let frames = layout.table_frames();
        let tops: Vec<_> = layout
            .rows
            .iter()
            .flat_map(|r| &r.tables)
            .map(|t| (t, TableTop::new(t, frames[&t.id], TABLE_END)))
            .collect();
        let prints = super::super::footprint::footprints(layout, &tops);
        assert_eq!(geometry.crowded, super::super::footprint::crowded(&prints));
    }

    #[test]
    fn previews_and_cancellation_match_full_geometry_for_unequal_rotated_and_round_tables() {
        let original = layout();
        let mut geometry = RoomGeometry::new(&original);
        assert_matches(&geometry, &original);
        // Move one table, then a group, turn, return to previous positions,
        // and cancel. Old moved-table collisions must disappear each time.
        for step in 0..32 {
            let mut layout = original.clone();
            let mut preview = BTreeMap::new();
            for i in 0..4 {
                if (step + i) % 3 == 0 {
                    let id = format!("table-{}", i + 1);
                    let mut frame = original.table_positions[&id];
                    frame.x += step as f32 * 43.0 - 400.0;
                    frame.y -= i as f32 * 157.0;
                    frame.rotation += step as f32 * 15.0;
                    layout.table_positions.insert(id.clone(), frame);
                    preview.insert(id, frame);
                }
            }
            geometry.preview(&preview);
            assert_matches(&geometry, &layout);
            let collisions = geometry.overlaps.clone();
            geometry.preview(&preview);
            assert_eq!(geometry.overlaps, collisions);
        }
        geometry.preview(&BTreeMap::new());
        assert_matches(&geometry, &original);
    }

    #[test]
    fn full_sweep_matches_all_pairs_for_dense_sparse_and_reordered_bounds() {
        let mut layout = Layout::simple(25, 3);
        for (i, table) in layout.rows[0].tables.iter_mut().enumerate() {
            table.left = i % 9;
            table.right = (i * 3) % 7;
            if i % 3 == 0 {
                table.shape = TableShape::Round;
            }
        }
        layout.set_free_layout(true);
        let mut geometry = RoomGeometry::new(&layout);
        assert_matches(&geometry, &layout);
        for spread in [75.0, 180.0, 1000.0] {
            let mut preview = BTreeMap::new();
            for (i, (id, frame)) in layout.table_positions.iter_mut().enumerate() {
                frame.x = ((i * 13) % 11) as f32 * spread - 500.0;
                frame.y = ((i * 7) % 9) as f32 * spread - 200.0;
                frame.rotation = (i * 37) as f32;
                preview.insert(id.clone(), *frame);
            }
            geometry.preview(&preview);
            assert_matches(&geometry, &layout);
        }
    }
}
