//! egui's built-in controls (dropdowns, sliders) restyled for the Soft
//! treatment. They take their look from widget visuals, so each helper
//! scopes the visuals around the one control.

use std::fmt::Debug;
use std::hash::Hash;

use eframe::egui::{self, Stroke};

use super::icons::{Icon, paint};

/// Field look: the text-field fill and a hairline outline that darkens a
/// little on hover. Shared by text fields and dropdowns.
pub(super) fn field_visuals(ui: &mut egui::Ui) {
    let visuals = ui.visuals_mut();
    let line = visuals.widgets.noninteractive.bg_stroke;
    let fill = visuals.text_edit_bg_color();
    let w = &mut visuals.widgets;
    let hover = Stroke::new(
        1.0,
        super::blend(line.color, w.hovered.fg_stroke.color, 0.25),
    );
    w.inactive.bg_stroke = line;
    w.hovered.bg_stroke = hover;
    for state in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        state.weak_bg_fill = fill;
    }
}

/// A dropdown that looks like a [`super::field`]: same height, corners and
/// outline, with the app's chevron.
pub struct Combo {
    widget: egui::ComboBox,
    width: Option<f32>,
}

pub fn combo(id_salt: impl Hash + Debug, selected: impl Into<egui::WidgetText>) -> Combo {
    Combo {
        widget: egui::ComboBox::from_id_salt(id_salt)
            .selected_text(selected)
            .truncate()
            .icon(|ui, rect, visuals, _open| {
                let color = super::blend(visuals.text_color(), ui.visuals().panel_fill, 0.35);
                paint(ui.painter(), Icon::ChevronDown, rect.center(), color);
            }),
        width: None,
    }
}

impl Combo {
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Maximum height of the open list.
    pub fn height(mut self, height: f32) -> Self {
        self.widget = self.widget.height(height);
        self
    }

    pub fn show_ui<R>(
        self,
        ui: &mut egui::Ui,
        add: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        ui.scope(|ui| {
            field_visuals(ui);
            // egui grows the frame to the control height at the bottom only;
            // padding it evenly keeps the text centred like a field's.
            let text = ui.text_style_height(&egui::TextStyle::Button);
            let padding = egui::vec2(
                super::space::M,
                ((super::CONTROL_HEIGHT - text) / 2.0).max(0.0),
            );
            ui.spacing_mut().button_padding = padding;
            let width = self
                .width
                .unwrap_or(ui.spacing().combo_width)
                .min(ui.available_width());
            self.widget.width(width).show_ui(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                add(ui)
            })
        })
        .inner
    }
}

/// A slider with a soft handle: a solid accent disc with no outline, over
/// a quiet rail.
pub fn slider(ui: &mut egui::Ui, slider: egui::Slider<'_>) -> egui::Response {
    ui.scope(|ui| {
        // The handle is sized from the row height (radius = height / 2.5).
        ui.spacing_mut().interact_size.y = HANDLE * 2.5;
        ui.spacing_mut().slider_rail_height = 6.0;
        let visuals = ui.visuals_mut();
        let accent = visuals.selection.stroke.color;
        let w = &mut visuals.widgets;
        // The rail and a resting handle share `inactive.bg_fill`, so the
        // handle is painted by an inside stroke as wide as the handle.
        for state in [&mut w.inactive, &mut w.hovered, &mut w.active] {
            state.fg_stroke = Stroke::new(HANDLE, accent);
        }
        ui.add(slider.handle_shape(egui::style::HandleShape::Rect { aspect_ratio: 1.0 }))
    })
    .inner
}

/// Radius of a slider handle, in points.
const HANDLE: f32 = 8.0;
