//! Pointer input shared by the zoomable canvases.

use eframe::egui;

/// How much the scroll wheel zooms per point of scroll.
const WHEEL_ZOOM: f32 = 0.0015;

/// This frame's zoom factor: the scroll wheel (and pinch / Ctrl+scroll) zoom
/// instead of panning. 1.0 means no zoom.
pub(super) fn zoom_factor(ui: &egui::Ui) -> f32 {
    ui.input(|i| i.zoom_delta() * (i.smooth_scroll_delta.y * WHEEL_ZOOM).exp())
}
