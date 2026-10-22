//! Selection, magnetic guides and arranging tables while preserving their IDs.
use super::*;
use crate::app::{camera::Viewport, turned::TableTop};
use crate::model::{Layout, TablePosition};
use egui::{Pos2, Rect, Stroke, StrokeKind, Vec2};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
enum Align {
    Left,
    Right,
    Top,
    Bottom,
    CentreX,
    CentreY,
    Horizontal,
    Vertical,
}

/// Closest matching edge or centre line in each screen axis.
pub(super) fn magnetic(
    moving: Rect,
    others: &[Rect],
    tolerance: f32,
) -> (Vec2, (Option<f32>, Option<f32>)) {
    let axis = |values: [f32; 3], other: Vec<f32>| {
        values
            .into_iter()
            .flat_map(|value| other.iter().map(move |target| (*target - value, *target)))
            .filter(|(delta, _)| delta.abs() <= tolerance)
            .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
    };
    let x = axis(
        [moving.left(), moving.center().x, moving.right()],
        others
            .iter()
            .flat_map(|r| [r.left(), r.center().x, r.right()])
            .collect(),
    );
    let y = axis(
        [moving.top(), moving.center().y, moving.bottom()],
        others
            .iter()
            .flat_map(|r| [r.top(), r.center().y, r.bottom()])
            .collect(),
    );
    (
        Vec2::new(x.map_or(0.0, |v| v.0), y.map_or(0.0, |v| v.0)),
        (x.map(|v| v.1), y.map(|v| v.1)),
    )
}

fn arrange(layout: &Layout, ids: &[String], action: Align) -> BTreeMap<String, TablePosition> {
    let frames = layout.table_frames();
    let mut tables: Vec<_> = ids
        .iter()
        .filter_map(|id| {
            let table = layout.table(id)?;
            let at = frames[id];
            Some((id.clone(), at, TableTop::new(table, at, 28.0).bounds()))
        })
        .collect();
    if tables.len() < 2 {
        return BTreeMap::new();
    }
    let bounds = tables
        .iter()
        .fold(Rect::NOTHING, |b, (_, _, r)| b.union(*r));
    let horizontal = matches!(action, Align::Horizontal);
    if matches!(action, Align::Horizontal | Align::Vertical) {
        if tables.len() < 3 {
            return BTreeMap::new();
        }
        tables.sort_by(|a, b| {
            if horizontal {
                a.2.left().total_cmp(&b.2.left())
            } else {
                a.2.top().total_cmp(&b.2.top())
            }
        });
        let start = if horizontal {
            tables[0].2.left()
        } else {
            tables[0].2.top()
        };
        let end = if horizontal {
            tables.last().unwrap().2.right()
        } else {
            tables.last().unwrap().2.bottom()
        };
        let total: f32 = tables
            .iter()
            .map(|(_, _, r)| if horizontal { r.width() } else { r.height() })
            .sum();
        let gap = (end - start - total) / (tables.len() - 1) as f32;
        let mut cursor = start;
        for (_, at, r) in &mut tables {
            if horizontal {
                at.x += cursor - r.left();
                cursor += r.width() + gap;
            } else {
                at.y += cursor - r.top();
                cursor += r.height() + gap;
            }
        }
    } else {
        for (_, at, r) in &mut tables {
            match action {
                Align::Left => at.x += bounds.left() - r.left(),
                Align::Right => at.x += bounds.right() - r.right(),
                Align::Top => at.y += bounds.top() - r.top(),
                Align::Bottom => at.y += bounds.bottom() - r.bottom(),
                Align::CentreX => at.x += bounds.center().x - r.center().x,
                Align::CentreY => at.y += bounds.center().y - r.center().y,
                _ => {}
            }
        }
    }
    tables.into_iter().map(|(id, at, _)| (id, at)).collect()
}

impl PlassiApp {
    pub(super) fn table_arrangement(&mut self, ui: &mut egui::Ui, p: Palette) -> RoomEdit {
        let ids: Vec<_> = self.room.canvas.selected_tables.iter().cloned().collect();
        theme::section_title(ui, p, &fill("{} tables selected", &[&ids.len()]));
        theme::hint(
            ui,
            p,
            tr(
                "Drag any selected table to move them together. Alignments use the visible table edges.",
            ),
        );
        let mut chosen = None;
        ui.horizontal_wrapped(|ui| {
            for (label, action) in [
                ("Align left edges", Align::Left),
                ("Align right edges", Align::Right),
                ("Align top ends", Align::Top),
                ("Align bottom ends", Align::Bottom),
                ("Align horizontal centres", Align::CentreY),
                ("Align vertical centres", Align::CentreX),
                ("Distribute horizontal gaps", Align::Horizontal),
                ("Distribute vertical gaps", Align::Vertical),
            ] {
                let enabled =
                    !matches!(action, Align::Horizontal | Align::Vertical) || ids.len() >= 3;
                if ui
                    .add_enabled(enabled, egui::Button::new(tr(label)).wrap())
                    .clicked()
                {
                    chosen = Some(action);
                }
            }
        });
        if ui.button(tr("Clear table selection")).clicked() {
            self.room.canvas.selected_tables.clear();
            self.room.canvas.selected = None;
        }
        if let Some(action) = chosen {
            let positions = arrange(&self.project.layout, &ids, action);
            self.project.layout.table_positions.extend(positions);
            self.chart.view.fit_pending = true;
            return RoomEdit {
                labels: true,
                geometry: false,
            };
        }
        RoomEdit::default()
    }

    pub(super) fn room_selection_box(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        view: &Viewport,
        background: &egui::Response,
        tables: &[(&TableSpec, TableTop)],
        p: Palette,
    ) {
        let state = &mut self.room.canvas;
        if ui.input(|i| i.multi_touch().is_some() || i.key_pressed(egui::Key::Escape)) {
            state.marquee = None;
            return;
        }
        if (state.selection_mode || ui.input(|i| i.modifiers.shift))
            && background.drag_started()
            && let Some(at) = ui.input(|i| i.pointer.press_origin())
        {
            state.marquee = Some((view.world(at), view.world(at)));
        }
        if let Some((start, end)) = &mut state.marquee {
            if let Some(at) = ui.input(|i| i.pointer.interact_pos()) {
                *end = view.world(at);
            }
            let area = Rect::from_two_pos(*start, *end);
            painter.rect_filled(view.rect(area), 0.0, p.accent.linear_multiply(0.12));
            painter.rect_stroke(
                view.rect(area),
                0.0,
                Stroke::new(1.0, p.accent),
                StrokeKind::Inside,
            );
            if ui.input(|i| i.pointer.any_released()) {
                state.selected_tables = tables
                    .iter()
                    .filter(|(_, top)| area.intersects(top.bounds()))
                    .map(|(t, _)| t.id.clone())
                    .collect();
                state.selected = state.selected_tables.first().cloned();
                state.selected_label = None;
                state.marquee = None;
            }
        }
    }

    pub(super) fn paint_alignment_guides(
        &self,
        painter: &egui::Painter,
        view: &Viewport,
        layout: &Layout,
        moving: Option<&(String, TablePosition)>,
        p: Palette,
    ) {
        if !self.room.canvas.snap || self.room.canvas.drag.is_none() {
            return;
        }
        let Some((id, at)) = moving else {
            return;
        };
        let Some(table) = layout.table(id) else {
            return;
        };
        let others = layout
            .table_frames()
            .into_iter()
            .filter(|(id, _)| !self.room.canvas.selected_tables.contains(id))
            .filter_map(|(id, at)| {
                layout
                    .table(&id)
                    .map(|t| TableTop::new(t, at, 28.0).bounds())
            })
            .collect::<Vec<_>>();
        let (_, (x, y)) = magnetic(TableTop::new(table, *at, 28.0).bounds(), &others, 0.2);
        if let Some(x) = x {
            let x = view.point(Pos2::new(x, 0.0)).x;
            painter.line_segment(
                [
                    Pos2::new(x, view.clip.top()),
                    Pos2::new(x, view.clip.bottom()),
                ],
                Stroke::new(1.0, p.accent),
            );
        }
        if let Some(y) = y {
            let y = view.point(Pos2::new(0.0, y)).y;
            painter.line_segment(
                [
                    Pos2::new(view.clip.left(), y),
                    Pos2::new(view.clip.right(), y),
                ],
                Stroke::new(1.0, p.accent),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn layout() -> Layout {
        let mut layout = Layout::simple(3, 4);
        layout.set_free_layout(true);
        layout.rows[0].tables[1].left = 7;
        layout.rows[0].tables[1].right = 7;
        for (id, at) in layout.table_positions.iter_mut() {
            if id == "table-2" {
                at.rotation = 45.0;
                at.x = 340.0;
                at.y = -100.0;
            }
        }
        layout
    }
    fn bounds(layout: &Layout) -> Vec<Rect> {
        layout
            .table_frames()
            .iter()
            .map(|(id, at)| TableTop::new(layout.table(id).unwrap(), *at, 28.0).bounds())
            .collect()
    }
    #[test]
    fn aligning_and_distributing_unequal_rotated_tables_preserves_ids_and_rotations() {
        let mut layout = layout();
        let ids: Vec<_> = layout.table_frames().into_keys().collect();
        let old_seats: Vec<_> = layout.seats().iter().map(|s| s.id.clone()).collect();
        let positions = arrange(&layout, &ids, Align::Bottom);
        layout.table_positions.extend(positions);
        let boxes = bounds(&layout);
        assert!(
            boxes
                .windows(2)
                .all(|b| (b[0].bottom() - b[1].bottom()).abs() < 0.01)
        );
        assert_eq!(layout.table_positions["table-2"].rotation, 45.0);
        assert_eq!(
            layout
                .seats()
                .iter()
                .map(|s| s.id.clone())
                .collect::<Vec<_>>(),
            old_seats
        );
        let outer = boxes.iter().fold(Rect::NOTHING, |a, b| a.union(*b));
        let positions = arrange(&layout, &ids, Align::Horizontal);
        layout.table_positions.extend(positions);
        let mut boxes = bounds(&layout);
        boxes.sort_by(|a, b| a.left().total_cmp(&b.left()));
        assert!(
            ((boxes[1].left() - boxes[0].right()) - (boxes[2].left() - boxes[1].right())).abs()
                < 0.01
        );
        assert!((boxes[0].left() - outer.left()).abs() < 0.01);
        assert!((boxes[2].right() - outer.right()).abs() < 0.01);
    }
    #[test]
    fn magnetic_guides_pick_nearby_edges_and_ignore_distant_tables() {
        let moving = Rect::from_min_size(Pos2::new(3.0, 4.0), Vec2::new(40.0, 80.0));
        let other = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(40.0, 80.0));
        let (offset, guides) = magnetic(moving, &[other], 5.0);
        assert_eq!(offset, Vec2::new(-3.0, -4.0));
        assert!(guides.0.is_some() && guides.1.is_some());
        assert_eq!(
            magnetic(moving, &[other.translate(Vec2::splat(1000.0))], 5.0).1,
            (None, None)
        );
    }
}
