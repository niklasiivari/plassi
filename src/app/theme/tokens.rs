//! Design tokens: the only spacing, type and radius values the UI uses.

/// Spacing scale, in points.
pub mod space {
    pub const XS: f32 = 4.0;
    pub const S: f32 = 8.0;
    pub const M: f32 = 12.0;
    pub const L: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
    pub const XXXL: f32 = 48.0;
}

/// Type scale, in points.
pub mod text {
    /// Hints, legends, uppercase section labels.
    pub const CAPTION: f32 = 12.0;
    /// Secondary lines and dense lists.
    pub const SMALL: f32 = 13.5;
    pub const BODY: f32 = 15.0;
    /// Card titles and emphasised names.
    pub const LEAD: f32 = 17.0;
    /// Panel titles.
    pub const TITLE: f32 = 21.0;
    /// Page titles.
    pub const PAGE: f32 = 28.0;
    /// Large numbers.
    pub const DISPLAY: f32 = 36.0;
}

/// Corner radii: small parts, controls, cards and floating layers, pills.
pub mod radius {
    /// Seat cards, tags, inset blocks.
    pub const SM: u8 = 8;
    /// Buttons, fields, nav items.
    pub const MD: u8 = 10;
    /// Cards, panels, menus, toasts.
    pub const LG: u8 = 16;
    pub const PILL: u8 = 255;
}

/// Height of buttons, fields and other controls.
pub const CONTROL_HEIGHT: f32 = 36.0;
/// Height of chips and tags inside dense rows.
pub const CHIP_HEIGHT: f32 = 28.0;
/// Width of side panels (inspector, filters).
pub const PANEL_WIDTH: f32 = 330.0;
/// Readable width of a single-column page.
pub const CONTENT_WIDTH: f32 = 920.0;
/// Width of pages that switch to two columns on wide screens.
pub const WIDE_WIDTH: f32 = 1480.0;
/// Below this width, two-column layouts stack into one column.
pub const TWO_COLUMN_MIN: f32 = 1100.0;

/// Width below which pages show one task at a time.
pub const MOBILE_MAX: f32 = 600.0;
/// Keep navigation as a rail until there is room for its labels and the workspace.
pub const SIDEBAR_EXPANDED_MIN: f32 = 1200.0;
/// Minimum workspace width for a list and editor side by side.
pub const SPLIT_MIN: f32 = 640.0;
/// Below this workspace width the chart inspector floats over the canvas.
pub const DOCKED_INSPECTOR_MIN: f32 = 900.0;

pub fn mobile(ctx: &eframe::egui::Context) -> bool {
    ctx.content_rect().width() < MOBILE_MAX
}

/// Form layout follows the width of its own panel, independently of navigation.
pub fn narrow(ui: &eframe::egui::Ui) -> bool {
    ui.available_width() < MOBILE_MAX
}
