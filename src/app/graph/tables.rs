//! "Arrange by table": the graph takes the room's shape. Each table is drawn
//! as a tall rounded table, tables stand side by side in their rows like on
//! the chart, and each person is pulled towards their own seat — left or right
//! side, top to bottom. Wishes that cross between tables stand out.

use std::collections::HashMap;

use eframe::egui::{Pos2, Rect, Vec2};

use super::Graph;
use crate::model::{Project, Side};

/// Horizontal distance between a table's left and right seats.
const SIDE_GAP: f32 = 150.0;
/// Room beside the seats for names, which sit outside the table.
const NAME_ROOM: f32 = 150.0;
/// Vertical distance between seats.
const SEAT_GAP: f32 = 64.0;
/// Space between neighbouring tables, and between rows of tables.
const TABLE_GAP: f32 = 170.0;
/// Margin of the table outline around its seats.
const PADDING: f32 = 48.0;

pub(super) struct Zone {
    pub(super) label: String,
    pub(super) rect: Rect,
    /// People shown at this table.
    pub(super) members: usize,
    /// Wishes from people here to people at another table.
    pub(super) outgoing: usize,
}

pub(super) struct TableZones {
    pub(super) zones: Vec<Zone>,
    /// Per graph node: its seat position and which side of the table it is on.
    pub(super) seat_of: Vec<Option<(Vec2, Side)>>,
}

/// One outline per table, in the room's arrangement and centred on the origin,
/// plus every shown person's seat position. `None` without a seating option.
pub(super) fn table_zones(
    project: &Project,
    candidate: Option<usize>,
    graph: &Graph,
) -> Option<TableZones> {
    let option = project.candidates.get(candidate?)?;
    let (mut rects, labels) = table_outlines(project);
    let seat_point = seat_points(project, &rects);
    let table_of: HashMap<usize, usize> = graph
        .people
        .iter()
        .enumerate()
        .filter_map(|(node, person)| {
            let (_, table, _) = seat_point.get(option.assignment.get(person)?.as_str())?;
            Some((node, *table))
        })
        .collect();
    let centre = rects
        .iter()
        .fold(Rect::NOTHING, |a, r| a.union(*r))
        .center()
        .to_vec2();
    for rect in &mut rects {
        *rect = rect.translate(-centre);
    }
    let seat_of = graph
        .people
        .iter()
        .map(|person| {
            let (point, _, side) = seat_point.get(option.assignment.get(person)?.as_str())?;
            Some((*point - centre, *side))
        })
        .collect();
    let mut zones: Vec<Zone> = rects
        .into_iter()
        .zip(labels)
        .enumerate()
        .map(|(i, (rect, label))| Zone {
            label,
            rect,
            members: table_of.values().filter(|t| **t == i).count(),
            outgoing: 0,
        })
        .collect();
    for edge in &graph.edges {
        if let (Some(a), Some(b)) = (table_of.get(&edge.from), table_of.get(&edge.to))
            && a != b
        {
            zones[*a].outgoing += 1;
        }
    }
    Some(TableZones { zones, seat_of })
}

/// Outline and label of every table, laid out row by row like the room.
fn table_outlines(project: &Project) -> (Vec<Rect>, Vec<String>) {
    let mut rects = Vec::new();
    let mut labels = Vec::new();
    let mut y = 0.0;
    for row in &project.layout.rows {
        let mut x = 0.0;
        let mut row_height: f32 = 0.0;
        for table in &row.tables {
            let length = table.left.max(table.right).max(1) as f32;
            let size = Vec2::new(
                SIDE_GAP + 2.0 * NAME_ROOM,
                (length - 1.0) * SEAT_GAP + 2.0 * PADDING,
            );
            rects.push(Rect::from_min_size(Pos2::new(x, y), size));
            labels.push(table.label.clone());
            x += size.x + TABLE_GAP;
            row_height = row_height.max(size.y);
        }
        y += row_height + TABLE_GAP;
    }
    (rects, labels)
}

/// Graph position of every seat, with its table index and side.
fn seat_points(project: &Project, rects: &[Rect]) -> HashMap<String, (Vec2, usize, Side)> {
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
            let table = *table_index.get(seat.table_id.as_str())?;
            let origin = rects[table].min + Vec2::new(NAME_ROOM, PADDING);
            let x = if seat.side == Side::Left {
                0.0
            } else {
                SIDE_GAP
            };
            let point = origin + Vec2::new(x, seat.index as f32 * SEAT_GAP);
            Some((seat.id, (point.to_vec2(), table, seat.side)))
        })
        .collect()
}

/// Half the width of the table strip drawn between the seat columns.
pub(super) const TABLE_HALF_WIDTH: f32 = SIDE_GAP / 2.0 - 30.0;
