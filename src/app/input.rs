//! Pointer input shared by the zoomable canvases.

use eframe::egui;
use std::collections::BTreeMap;

type Touches = BTreeMap<(egui::TouchDeviceId, egui::TouchId), egui::Pos2>;

/// Follow physical contacts: the browser emits a pointer press/release for
/// every finger added/removed, including during an existing canvas drag.
#[derive(Default)]
pub(super) struct TouchNavigation {
    points: Touches,
    owns_gesture: bool,
    navigating: bool,
    suppress_pointer: bool,
    last_frame: Option<u64>,
}

pub(super) struct TouchMotion {
    pub previous_center: egui::Pos2,
    pub center: egui::Pos2,
    pub zoom: f32,
    pub navigating: bool,
}

impl TouchNavigation {
    pub(super) fn suppresses_pointer(&self) -> bool {
        self.suppress_pointer
    }

    /// `Some` also consumes idle or unowned touch frames, so their synthetic
    /// pointer events cannot also pan or zoom the camera.
    pub(super) fn update(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        pan_started: bool,
    ) -> Option<TouchMotion> {
        let frame = ui.ctx().cumulative_frame_nr();
        if self.last_frame.is_some_and(|last| frame > last + 1) {
            // The canvas was hidden; do not resume a stale contact or gesture.
            *self = Self::default();
        }
        self.last_frame = Some(frame);
        let previous = self.points.clone();
        self.suppress_pointer = self.navigating;
        let mut touched = !previous.is_empty();
        let over_canvas = ui.rect_contains_pointer(rect);
        ui.input(|input| {
            for event in &input.raw.events {
                let egui::Event::Touch {
                    device_id,
                    id,
                    phase,
                    pos,
                    ..
                } = event
                else {
                    continue;
                };
                touched = true;
                let key = (*device_id, *id);
                match phase {
                    egui::TouchPhase::Start => {
                        if self.points.is_empty() {
                            self.owns_gesture = rect.contains(*pos) && over_canvas;
                            self.navigating = false;
                        }
                        self.points.insert(key, *pos);
                    }
                    egui::TouchPhase::Move => {
                        if let Some(point) = self.points.get_mut(&key) {
                            *point = *pos;
                        }
                    }
                    egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                        self.points.remove(&key);
                    }
                }
            }
            if !input.any_touches() {
                self.points.clear();
            }
            touched |= input.any_touches();
        });
        if self.points.is_empty() {
            self.owns_gesture = false;
            self.navigating = false;
        } else if self.owns_gesture && (self.points.len() >= 2 || pan_started) {
            self.navigating = true;
        }
        self.suppress_pointer |= self.navigating;
        if !touched {
            return None;
        }
        let (center, radius) = geometry(&self.points);
        let mut motion = TouchMotion {
            previous_center: center,
            center,
            zoom: 1.0,
            navigating: self.navigating,
        };
        // A changed set of fingers establishes a new baseline without moving
        // the view, including replacement of a finger within the same frame.
        if self.navigating && previous.keys().eq(self.points.keys()) {
            let (previous_center, previous_radius) = geometry(&previous);
            motion.previous_center = previous_center;
            if self.points.len() >= 2 && previous_radius > 0.0 && radius > 0.0 {
                motion.zoom = radius / previous_radius;
            }
        }
        Some(motion)
    }
}

fn geometry(points: &Touches) -> (egui::Pos2, f32) {
    if points.is_empty() {
        return (egui::Pos2::ZERO, 0.0);
    }
    let count = points.len() as f32;
    let center = egui::Pos2::ZERO
        + points
            .values()
            .fold(egui::Vec2::ZERO, |sum, point| sum + point.to_vec2())
            / count;
    let radius = points.values().map(|p| p.distance(center)).sum::<f32>() / count;
    (center, radius)
}

/// How much the scroll wheel zooms per point of scroll.
const WHEEL_ZOOM: f32 = 0.0015;

/// This frame's zoom factor: the scroll wheel (and pinch / Ctrl+scroll) zoom
/// instead of panning. 1.0 means no zoom.
pub(super) fn zoom_factor(ui: &egui::Ui) -> f32 {
    ui.input(|i| i.zoom_delta() * (i.smooth_scroll_delta.y * WHEEL_ZOOM).exp())
}
