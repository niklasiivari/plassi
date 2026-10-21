//! "Arrange by table": the graph takes the room's shape. Tables stand where
//! they do on the seating chart, turned and round ones included, and each
//! person is pulled towards their own seat. Names sit on the side away from
//! the table. Wishes that cross between tables stand out.

use std::collections::HashMap;

use eframe::egui::{Rect, Vec2};

use super::Graph;
use crate::app::turned::TableTop;
use crate::model::{Project, SEAT_PITCH};

/// Graph units per chart unit: seats a little further apart than on the
/// chart, so names have room.
const SCALE: f32 = 64.0 / SEAT_PITCH;
/// Room beside the seats for names, which sit outside the table.
const NAME_ROOM: f32 = 150.0;
/// Margin of the table outline around its seats.
const PADDING: f32 = 48.0;
/// How far a long table reaches past its end seats, in chart units.
const TABLE_END: f32 = 28.0;

pub(super) struct Zone {
    pub(super) label: String,
    /// Around the table, its seats and their names.
    pub(super) rect: Rect,
    /// The table itself.
    pub(super) top: TableTop,
    /// People shown at this table.
    pub(super) members: usize,
    /// Wishes from people here to people at another table.
    pub(super) outgoing: usize,
}

pub(super) struct TableZones {
    pub(super) zones: Vec<Zone>,
    /// Per graph node: its seat position and the direction away from its table.
    pub(super) seat_of: Vec<Option<(Vec2, f32)>>,
}

/// A seat in the graph: where it is, its table's index and the direction
/// away from the table.
struct SeatPoint {
    at: Vec2,
    table: usize,
    out: f32,
}

/// One outline per table, where the chart has it and centred on the origin,
/// plus every shown person's seat position. `None` without a seating option.
pub(super) fn table_zones(
    project: &Project,
    candidate: Option<usize>,
    graph: &Graph,
) -> Option<TableZones> {
    let option = project.candidates.get(candidate?)?;
    let seats = seat_points(project);
    let mut zones = table_outlines(project, &seats);
    let centre = zones
        .iter()
        .fold(Rect::NOTHING, |a, z| a.union(z.rect))
        .center()
        .to_vec2();
    for zone in &mut zones {
        zone.rect = zone.rect.translate(-centre);
        zone.top.top.centre -= centre;
    }
    let seat = |person: &usize| seats.get(option.assignment.get(person)?.as_str());
    let table_of: HashMap<usize, usize> = graph
        .people
        .iter()
        .enumerate()
        .filter_map(|(node, person)| Some((node, seat(person)?.table)))
        .collect();
    let seat_of = graph
        .people
        .iter()
        .map(|person| seat(person).map(|s| (s.at - centre, s.out)))
        .collect();
    for (i, zone) in zones.iter_mut().enumerate() {
        zone.members = table_of.values().filter(|t| **t == i).count();
    }
    for edge in &graph.edges {
        if let (Some(a), Some(b)) = (table_of.get(&edge.from), table_of.get(&edge.to))
            && a != b
        {
            zones[*a].outgoing += 1;
        }
    }
    Some(TableZones { zones, seat_of })
}

/// Every table's top and an outline around it, its seats and their names.
fn table_outlines(project: &Project, seats: &HashMap<String, SeatPoint>) -> Vec<Zone> {
    let frames = project.layout.table_frames();
    project
        .layout
        .rows
        .iter()
        .flat_map(|r| &r.tables)
        .enumerate()
        .map(|(i, table)| {
            let top = TableTop::new(table, frames[&table.id], TABLE_END).scaled(SCALE);
            let rect = seats
                .values()
                .filter(|s| s.table == i)
                .map(|s| Rect::from_center_size(s.at.to_pos2(), Vec2::ZERO))
                .fold(top.bounds(), Rect::union)
                .expand2(Vec2::new(NAME_ROOM, PADDING));
            Zone {
                label: table.label.clone(),
                rect,
                top,
                members: 0,
                outgoing: 0,
            }
        })
        .collect()
}

/// Graph position of every seat, by seat ID.
fn seat_points(project: &Project) -> HashMap<String, SeatPoint> {
    let table_index: HashMap<&str, usize> = project
        .layout
        .rows
        .iter()
        .flat_map(|r| &r.tables)
        .enumerate()
        .map(|(i, t)| (t.id.as_str(), i))
        .collect();
    project
        .layout
        .seats()
        .into_iter()
        .filter_map(|seat| {
            let point = SeatPoint {
                at: Vec2::new(seat.x, seat.y) * SCALE,
                table: *table_index.get(seat.table_id.as_str())?,
                out: seat.out,
            };
            Some((seat.id, point))
        })
        .collect()
}
