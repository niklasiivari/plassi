//! Pan and zoom for the chart and graph canvases.
//!
//! Fit, focus and button zooms glide to their target. Resizing the chart's
//! canvas preserves screen positions so panels never move the room.

use eframe::egui::{self, Pos2, Rect, Vec2};

/// How quickly animated moves settle (per second).
const EASE: f32 = 12.0;

pub(super) struct Camera {
    pub(super) zoom: f32,
    /// Screen offset of the world origin from the canvas centre, at `zoom`.
    pub(super) pan: Vec2,
    /// Where an animated move is heading: (zoom, pan).
    goal: Option<(f32, Vec2)>,
    min_zoom: f32,
    max_zoom: f32,
    /// No view has been set yet; the first fit snaps instead of gliding.
    fresh: bool,
    touch: super::input::TouchNavigation,
}

/// A frame's world-to-screen transform, taken from a `Camera`.
pub(super) struct Viewport {
    /// Screen area of the canvas.
    pub(super) clip: Rect,
    /// Screen position of the world origin.
    pub(super) origin: Pos2,
    pub(super) zoom: f32,
}

impl Viewport {
    pub(super) fn point(&self, world: Pos2) -> Pos2 {
        self.origin + world.to_vec2() * self.zoom
    }

    pub(super) fn rect(&self, world: Rect) -> Rect {
        Rect::from_min_max(self.point(world.min), self.point(world.max))
    }

    pub(super) fn world(&self, screen: Pos2) -> Pos2 {
        Pos2::ZERO + (screen - self.origin) / self.zoom
    }
}

impl Camera {
    /// Keep world points at their screen positions when panels resize the canvas,
    /// including the destination of a move that is still animating.
    pub(super) fn resize(&mut self, previous: Rect, current: Rect) {
        let offset = previous.center() - current.center();
        self.pan += offset;
        if let Some((_, pan)) = &mut self.goal {
            *pan += offset;
        }
    }

    /// The transform for a canvas occupying `rect`.
    pub(super) fn viewport(&self, rect: Rect) -> Viewport {
        Viewport {
            clip: rect,
            origin: self.to_screen(rect, Pos2::ZERO),
            zoom: self.zoom,
        }
    }

    pub(super) fn new(min_zoom: f32, max_zoom: f32) -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            goal: None,
            min_zoom,
            max_zoom,
            fresh: true,
            touch: Default::default(),
        }
    }

    pub(super) fn to_screen(&self, rect: Rect, world: Pos2) -> Pos2 {
        rect.center() + self.pan + world.to_vec2() * self.zoom
    }

    fn head_to(&mut self, zoom: f32, pan: Vec2) {
        if self.fresh {
            (self.zoom, self.pan, self.fresh) = (zoom, pan, false);
        } else {
            self.goal = Some((zoom, pan));
        }
    }

    /// Glide so `world` fits inside `rect`, zooming no further in than `max_fit`.
    pub(super) fn fit(&mut self, rect: Rect, world: Rect, max_fit: f32) {
        if rect.width() < 50.0 || world.width() <= 0.0 {
            return;
        }
        let zoom = (rect.width() / world.width())
            .min(rect.height() / world.height())
            .clamp(self.min_zoom, max_fit);
        self.head_to(zoom, -world.center().to_vec2() * zoom);
    }

    /// Glide so the world point is in the middle, keeping the zoom.
    pub(super) fn focus(&mut self, world: Pos2) {
        let zoom = self.goal.map_or(self.zoom, |(z, _)| z);
        self.head_to(zoom, -world.to_vec2() * zoom);
    }

    /// Glide to a zoom level around the canvas centre (for zoom buttons).
    pub(super) fn zoom_by(&mut self, factor: f32) {
        let (zoom, pan) = self.goal.unwrap_or((self.zoom, self.pan));
        self.zoom_to_with_pan(zoom * factor, zoom, pan);
    }

    /// The zoom and pan the view is moving toward, or showing.
    #[cfg(test)]
    pub(super) fn target(&self) -> (f32, Vec2) {
        self.goal.unwrap_or((self.zoom, self.pan))
    }

    /// The zoom level the view is currently moving toward, or displaying.
    pub(super) fn zoom_target(&self) -> f32 {
        self.goal.map_or(self.zoom, |(zoom, _)| zoom)
    }

    /// Glide to an explicit zoom level, preserving the view around the canvas centre.
    pub(super) fn zoom_to(&mut self, zoom: f32) {
        let (current, pan) = self.goal.unwrap_or((self.zoom, self.pan));
        self.zoom_to_with_pan(zoom, current, pan);
    }

    fn zoom_to_with_pan(&mut self, zoom: f32, current: f32, pan: Vec2) {
        let new_zoom = zoom.clamp(self.min_zoom, self.max_zoom);
        self.goal = Some((new_zoom, pan * (new_zoom / current)));
    }

    /// Dragging `background` pans (when `can_pan`); the scroll wheel zooms around
    /// the pointer. Returns true if the user moved the view.
    pub(super) fn handle_input(
        &mut self,
        ui: &egui::Ui,
        rect: Rect,
        background: &egui::Response,
        can_pan: bool,
    ) -> bool {
        let mut moved = false;
        if let Some(touch) = self.touch.update(ui, rect, can_pan && background.dragged()) {
            if !touch.navigating {
                return false;
            }
            let new_zoom = (self.zoom * touch.zoom).clamp(self.min_zoom, self.max_zoom);
            let anchor = touch.previous_center - rect.center() - self.pan;
            self.pan +=
                touch.center - touch.previous_center - anchor * (new_zoom / self.zoom - 1.0);
            self.zoom = new_zoom;
            self.goal = None;
            self.fresh = false;
            return true;
        }
        if can_pan && background.dragged() {
            self.pan += background.drag_delta();
            moved = true;
        }
        if ui.rect_contains_pointer(rect) {
            let factor = super::input::zoom_factor(ui);
            if factor != 1.0
                && let Some(pointer) = ui.input(|i| i.pointer.hover_pos())
            {
                let new_zoom = (self.zoom * factor).clamp(self.min_zoom, self.max_zoom);
                let anchor = pointer - rect.center() - self.pan;
                self.pan -= anchor * (new_zoom / self.zoom - 1.0);
                self.zoom = new_zoom;
                moved = true;
            }
        }
        if moved {
            self.goal = None;
            self.fresh = false;
        }
        moved
    }

    /// A navigation gesture owns its synthetic pointer events through the
    /// last finger's release, so lifting one cannot click a seat or node.
    pub(super) fn is_touch_navigation(&self) -> bool {
        self.touch.suppresses_pointer()
    }

    /// Advances an animated move; keeps repainting until it arrives.
    pub(super) fn animate(&mut self, ctx: &egui::Context) {
        let Some((zoom, pan)) = self.goal else { return };
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        let t = 1.0 - (-EASE * dt).exp();
        self.zoom += (zoom - self.zoom) * t;
        self.pan += (pan - self.pan) * t;
        if (zoom - self.zoom).abs() < 0.001 && (pan - self.pan).length() < 0.5 {
            (self.zoom, self.pan) = (zoom, pan);
            self.goal = None;
        }
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resizing_preserves_screen_positions_and_pending_animation() {
        let mut camera = Camera::new(0.2, 3.0);
        let full = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0));
        let smaller = Rect::from_min_max(Pos2::new(220.0, 0.0), full.max);
        camera.fit(full, full, 1.4);
        camera.zoom_by(1.5);
        let point = Pos2::new(60.0, 90.0);
        let before = camera.to_screen(full, point);
        let (zoom, pan) = camera.target();
        camera.resize(full, smaller);
        assert_eq!(camera.to_screen(smaller, point), before);
        assert_eq!(
            camera.target(),
            (zoom, pan + (full.center() - smaller.center()))
        );
        camera.resize(smaller, full);
        assert_eq!(camera.to_screen(full, point), before);
        assert_eq!(camera.target(), (zoom, pan));
    }
}
