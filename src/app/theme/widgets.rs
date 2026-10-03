//! Shared building blocks, so every page looks and spaces things the same way.

use eframe::egui::{self, Color32, Margin, RichText, Stroke};

use super::Palette;
use super::tokens::{radius, space, text};

/// A page title with an optional one-line description below it, and
/// `trailing` controls (e.g. a help button) at the right end of the title row.
pub fn page_header(
    ui: &mut egui::Ui,
    p: Palette,
    title: &str,
    subtitle: Option<&str>,
    trailing: impl FnOnce(&mut egui::Ui),
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(text::PAGE).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), trailing);
    });
    if let Some(subtitle) = subtitle {
        ui.add_space(space::XS);
        ui.label(RichText::new(subtitle).size(text::BODY).color(p.muted));
    }
    ui.add_space(space::XL);
}

/// A small uppercase label above a group of content.
pub fn section_title(ui: &mut egui::Ui, p: Palette, label: &str) {
    ui.label(
        RichText::new(label.to_uppercase())
            .size(text::CAPTION)
            .color(p.muted)
            .strong(),
    );
    ui.add_space(space::S);
}

/// Resting shadow of cards: a close contact shadow plus a wide soft one,
/// tinted with the palette's shadow colour.
pub fn shadow_sm(p: Palette) -> egui::Shadow {
    egui::Shadow {
        offset: [0, 2],
        blur: 12,
        spread: 0,
        color: shadow_color(p, 18),
    }
}

/// Shadow of floating layers: menus, popups, toasts.
pub fn shadow_lg(p: Palette) -> egui::Shadow {
    egui::Shadow {
        offset: [0, 10],
        blur: 32,
        spread: 0,
        color: shadow_color(p, 40),
    }
}

/// Soft shadows cast into `rect` by the panels around it (edges that are not
/// the window's), so a canvas sits visibly below the chrome without borders.
pub fn panel_edge_shadows(ui: &egui::Ui, p: Palette, rect: egui::Rect) {
    let screen = ui.ctx().content_rect();
    let shadow = egui::Shadow {
        offset: [0, 0],
        blur: 14,
        spread: 0,
        color: shadow_color(p, 26),
    };
    let depth = 100.0;
    let outside = [
        (
            rect.top() > screen.top() + 1.0,
            egui::Rect::from_min_max(
                rect.left_top() - egui::vec2(depth, depth),
                rect.right_top() + egui::vec2(depth, 0.0),
            ),
        ),
        (
            rect.left() > screen.left() + 1.0,
            egui::Rect::from_min_max(
                rect.left_top() - egui::vec2(depth, depth),
                rect.left_bottom() + egui::vec2(0.0, depth),
            ),
        ),
        (
            rect.right() < screen.right() - 1.0,
            egui::Rect::from_min_max(
                rect.right_top() - egui::vec2(0.0, depth),
                rect.right_bottom() + egui::vec2(depth, depth),
            ),
        ),
    ];
    let painter = ui.painter_at(rect);
    for (shown, source) in outside {
        if shown {
            painter.add(shadow.as_shape(source, 0));
        }
    }
}

/// Shadow under a chart seat card, scaled with the zoom.
pub fn seat_shadow(p: Palette, zoom: f32, alpha: f32) -> egui::Shadow {
    let base = shadow_sm(p);
    let scale = zoom.clamp(0.3, 1.5);
    egui::Shadow {
        offset: [0, (scale * 1.5).round() as i8],
        blur: (base.blur as f32 * 0.7 * scale).round() as u8,
        spread: 0,
        color: base.color.gamma_multiply(alpha),
    }
}

fn shadow_color(p: Palette, alpha: u8) -> Color32 {
    let dark = p.bg.r() < 100;
    let alpha = if dark { alpha.saturating_mul(3) } else { alpha };
    Color32::from_rgba_unmultiplied(p.shadow.r(), p.shadow.g(), p.shadow.b(), alpha)
}

/// A card: surface colour, no border, soft shadow, large corners.
pub fn card(p: Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.surface)
        .corner_radius(radius::LG)
        .shadow(shadow_sm(p))
        .inner_margin(Margin::same(space::L as i8 + 2))
}

/// A floating layer anchored to something: search results, pickers.
pub fn floating(p: Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.surface)
        .corner_radius(radius::LG)
        .shadow(shadow_lg(p))
        .inner_margin(Margin::same(space::XS as i8 + 2))
}

/// A status toast that follows the active theme, floating above the canvas.
pub fn toast_frame(p: Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.surface_hi)
        .corner_radius(radius::LG)
        .shadow(shadow_lg(p))
        .inner_margin(Margin::symmetric(space::L as i8 - 2, space::M as i8))
}

/// A card lifted above its neighbours: the one being edited.
pub fn raised_card(p: Palette) -> egui::Frame {
    card(p).shadow(shadow_lg(p))
}

/// A card tinted with a status colour, for something that needs attention.
pub fn tinted_card(p: Palette, color: Color32) -> egui::Frame {
    card(p).fill(super::blend(p.surface, color, 0.08))
}

/// Background of a side panel or bar: surface, set apart from the page by tone.
pub fn panel_frame(p: Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.surface)
        .inner_margin(Margin::same(space::L as i8))
}

/// Background of a side panel: the light surface, edge to edge. Panels are
/// set apart from their neighbours by a hairline (egui's separator line).
pub fn floating_panel_frame(p: Palette) -> egui::Frame {
    egui::Frame::new().fill(p.surface)
}

/// A side panel's content area; `padding` is the space around the content.
pub fn panel_card<R>(
    ui: &mut egui::Ui,
    _p: Palette,
    padding: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Frame::new()
        .inner_margin(Margin::same(padding as i8))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            add(ui)
        })
        .inner
}

/// A recessed block inside a card or panel.
pub fn inset(p: Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.surface_hi)
        .corner_radius(radius::SM)
        .inner_margin(Margin::same(space::M as i8))
}

/// Controls inside an [`inset`] sit on the surface colour, so they stay
/// visible against the recessed fill.
pub fn inset_controls(ui: &mut egui::Ui, p: Palette) {
    let w = &mut ui.visuals_mut().widgets;
    w.inactive.weak_bg_fill = p.surface;
    w.inactive.bg_fill = p.surface;
}

/// Rows that are lines of text (legends, key–value lists), not controls:
/// they take the text's height instead of a control's.
pub fn text_rows<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = space::L;
        ui.spacing_mut().item_spacing.y = space::S - 2.0;
        add(ui)
    })
    .inner
}

/// Small rounded label, e.g. a count or status.
pub fn pill(ui: &mut egui::Ui, label: impl Into<String>, fg: Color32, bg: Color32) {
    egui::Frame::new()
        .fill(bg)
        .corner_radius(radius::PILL)
        .inner_margin(Margin::symmetric(10, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(label.into()).size(text::SMALL).color(fg));
        });
}

/// A pill tinted with a status colour.
pub fn status_pill(ui: &mut egui::Ui, p: Palette, label: impl Into<String>, color: Color32) {
    pill(ui, label, color, super::blend(p.surface, color, 0.16));
}

pub fn primary_button(label: &str, p: Palette) -> egui::Button<'_> {
    egui::Button::new(RichText::new(label).color(p.on_accent).strong())
        .fill(p.accent)
        .stroke(Stroke::NONE)
        .corner_radius(radius::MD)
}

/// A secondary action: soft surface fill, no outline (egui's default button look).
pub fn secondary_button(label: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(label).strong()).corner_radius(radius::MD)
}

/// A quiet action: muted text, no fill until hovered.
pub fn ghost_button(label: &str, p: Palette) -> egui::Button<'_> {
    egui::Button::new(RichText::new(label).color(p.muted).strong())
        .frame_when_inactive(false)
        .corner_radius(radius::MD)
}

/// A destructive action: status fill.
pub fn danger_button(label: &str, p: Palette) -> egui::Button<'_> {
    egui::Button::new(RichText::new(label).color(p.on_status).strong())
        .fill(p.bad)
        .stroke(Stroke::NONE)
        .corner_radius(radius::MD)
}

/// A round toggle chip, e.g. a filter or tab: accent tint when on.
pub fn chip(label: impl Into<egui::WidgetText>, p: Palette, on: bool) -> egui::Button<'static> {
    egui::Button::new(label)
        .fill(if on { p.accent_soft } else { p.surface_hi })
        .stroke(Stroke::NONE)
        .corner_radius(radius::PILL)
}

/// Text for a [`chip`]: accent and a little bolder when on.
pub fn chip_text(label: impl Into<String>, p: Palette, on: bool) -> RichText {
    let text = RichText::new(label.into());
    if on {
        text.color(p.accent).strong()
    } else {
        text
    }
}

/// A small removable tag: "label  ×" in a pill.
pub fn tag_button(label: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(format!("{label}  ×")).size(text::SMALL))
        .corner_radius(radius::PILL)
        .min_size(egui::vec2(0.0, super::CHIP_HEIGHT))
}

/// A compact secondary action inside a dense row or card.
pub fn small_button(label: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    egui::Button::new(label)
        .corner_radius(radius::MD)
        .min_size(egui::vec2(0.0, super::CHIP_HEIGHT))
}

/// A small square button for a single glyph, e.g. "+", "−" or "×".
pub fn glyph_button(glyph: &str) -> egui::Button<'_> {
    egui::Button::new(glyph)
        .corner_radius(radius::SM)
        .min_size(egui::vec2(super::CHIP_HEIGHT, super::CHIP_HEIGHT))
}

/// Muted secondary text.
pub fn hint(ui: &mut egui::Ui, p: Palette, label: impl Into<String>) {
    ui.label(RichText::new(label.into()).size(text::SMALL).color(p.muted));
}

/// A keyboard shortcut and its action, laid out as a compact, reusable row.
pub fn key_hint(ui: &mut egui::Ui, p: Palette, keys: &str, action: &str) {
    ui.horizontal_wrapped(|ui| {
        for (index, key) in keys.split(" / ").enumerate() {
            if index > 0 {
                hint(ui, p, "/");
            }
            egui::Frame::new()
                .fill(p.surface_hi)
                .corner_radius(radius::SM)
                .inner_margin(Margin::symmetric(5, 2))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(key)
                            .monospace()
                            .size(text::CAPTION)
                            .color(p.text),
                    );
                });
        }
        hint(ui, p, action);
    });
}

/// A text field with a hairline outline (the one place outlines stay).
pub struct Field<'t>(egui::TextEdit<'t>);

impl<'t> Field<'t> {
    pub fn id(self, id: egui::Id) -> Self {
        Self(self.0.id(id))
    }

    pub fn hint_text(self, hint: impl Into<egui::WidgetText>) -> Self {
        Self(self.0.hint_text(hint))
    }

    pub fn desired_width(self, width: f32) -> Self {
        Self(self.0.desired_width(width))
    }

    pub fn desired_rows(self, rows: usize) -> Self {
        Self(self.0.desired_rows(rows))
    }

    pub fn horizontal_align(self, align: egui::Align) -> Self {
        Self(self.0.horizontal_align(align))
    }

    pub fn font(self, font: impl Into<egui::FontSelection>) -> Self {
        Self(self.0.font(font))
    }
}

impl egui::Widget for Field<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.scope(|ui| {
            super::controls::field_visuals(ui);
            ui.add(self.0)
        })
        .inner
    }
}

/// A single-line text field sized and padded like the buttons next to it.
pub fn field(value: &mut String) -> Field<'_> {
    Field(
        egui::TextEdit::singleline(value)
            .margin(Margin::symmetric(space::M as i8, 6))
            .min_size(egui::vec2(0.0, super::CONTROL_HEIGHT))
            .vertical_align(egui::Align::Center),
    )
}

/// A multi-line text area with the same padding as [`field`].
pub fn text_area(value: &mut String) -> Field<'_> {
    Field(
        egui::TextEdit::multiline(value).margin(Margin::symmetric(space::M as i8, space::S as i8)),
    )
}

/// A small coloured dot, e.g. before a status line.
pub fn status_dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 16.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
}

/// Colour of an outcome; muted when there is nothing to judge.
pub fn outcome_color(p: Palette, outcome: Option<crate::score::Outcome>) -> Color32 {
    use crate::score::Outcome;
    match outcome {
        Some(Outcome::Met) => p.good,
        Some(Outcome::Partial) => p.warn,
        Some(Outcome::Broken) => p.bad,
        None => p.muted,
    }
}

/// Two columns side by side when there is room, otherwise stacked.
/// `add` is called with column 0, then column 1.
pub fn two_columns(ui: &mut egui::Ui, mut add: impl FnMut(&mut egui::Ui, usize)) {
    if ui.available_width() < super::TWO_COLUMN_MIN {
        add(ui, 0);
        ui.add_space(space::L);
        add(ui, 1);
        return;
    }
    ui.horizontal_top(|ui| {
        let gap = space::XL;
        let width = (ui.available_width() - gap) / 2.0;
        for column in 0..2 {
            if column == 1 {
                ui.add_space(gap);
            }
            ui.allocate_ui_with_layout(
                egui::vec2(width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(width);
                    add(ui, column);
                },
            );
        }
    });
}

/// Avatar initials: first letters of the first and last name. A trailing
/// number stands for itself ("Guest 07" → "07").
pub fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    match words.as_slice() {
        [] => String::new(),
        [.., last] if last.chars().all(|c| c.is_ascii_digit()) => last
            .chars()
            .rev()
            .take(2)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect(),
        [only] => only.chars().take(1).collect(),
        [first, .., last] => first.chars().take(1).chain(last.chars().take(1)).collect(),
    }
}

/// Optional euro amount stored as exact cents, shared by categories and overrides.
pub fn price_input(ui: &mut egui::Ui, label: &str, price: &mut Option<u32>) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let mut enabled = price.is_some();
        if ui.checkbox(&mut enabled, label).changed() {
            *price = enabled.then_some(0);
            changed = true;
        }
        if let Some(cents) = price {
            let mut euros = f64::from(*cents) / 100.0;
            if ui
                .add(
                    egui::DragValue::new(&mut euros)
                        .speed(0.1)
                        .range(0.0..=1_000_000.0)
                        .fixed_decimals(2)
                        .suffix(" €"),
                )
                .changed()
            {
                *cents = (euros * 100.0).round() as u32;
                changed = true;
            }
        }
    });
    changed
}
