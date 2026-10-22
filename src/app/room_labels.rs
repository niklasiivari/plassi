//! Landmarks shared by the room designer and seating chart.
use super::camera::Viewport;
use crate::model::RoomLabel;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Vec2};

pub(super) fn bounds(label: &RoomLabel) -> Rect {
    Rect::from_center_size(
        Pos2::new(label.x, label.y),
        Vec2::new(
            280.0,
            (label
                .text
                .lines()
                .map(|line| line.chars().count().max(1).div_ceil(18))
                .sum::<usize>()
                .max(1) as f32
                * 28.0)
                .max(40.0),
        ),
    )
}

pub(super) fn paint(
    painter: &egui::Painter,
    view: &Viewport,
    label: &RoomLabel,
    color: Color32,
) -> Rect {
    let galley = painter.layout(
        label.text.clone(),
        FontId::proportional(22.0 * view.zoom),
        color,
        280.0 * view.zoom,
    );
    let centre = view.point(Pos2::new(label.x, label.y));
    let rect = Rect::from_center_size(centre, galley.size());
    painter.galley(rect.min, galley, color);
    rect.expand(6.0)
}
