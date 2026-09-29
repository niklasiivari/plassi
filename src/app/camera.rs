//! Pan and zoom for the chart and graph canvases.
//!
//! The view is anchored at the canvas centre, so opening or closing panels
//! (which resizes the canvas) keeps what you are looking at in place. Fit,
//! focus and button zooms glide to their target instead of jumping.

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
        let new_zoom = (zoom * factor).clamp(self.min_zoom, self.max_zoom);
        self.goal = Some((new_zoom, pan * (new_zoom / zoom)));
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
