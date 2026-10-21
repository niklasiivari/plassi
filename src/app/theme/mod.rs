mod controls;
mod icons;
mod tokens;
mod widgets;

pub use controls::{combo, slider};
pub use icons::{Icon, icon_button, icon_button_enabled, paint as paint_icon};
pub use tokens::*;
pub use widgets::*;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    TextStyle, Theme, Visuals,
};

/// Colours used by custom-painted widgets; resolved from the active theme.
///
/// "Ink on linen": neutrals are stone greys with a faint warm bias (paper,
/// table linen), the one accent is a blue-black fountain-pen ink. Status
/// colours differ in lightness as well as hue: pine (met), rust (partly met)
/// and crimson (broken), so they stay apart for colour-blind readers.
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_hi: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    /// Text and icons on an `accent` fill.
    pub on_accent: Color32,
    /// Text on a status fill (badges, the delete button).
    pub on_status: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub bad: Color32,
    pub table: Color32,
    pub canvas: Color32,
    pub grid: Color32,
    /// Tint of shadows: warm on light surfaces, black on dark.
    pub shadow: Color32,
}

impl Palette {
    /// Whether this is the dark theme, for colours that come in a light and a
    /// dark variant.
    pub fn is_dark(&self) -> bool {
        self.bg.r() < 100
    }
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub const DARK: Palette = Palette {
    bg: rgb(0x1A1914),
    surface: rgb(0x23211A),
    surface_hi: rgb(0x2F2C23),
    border: rgb(0x423E34),
    text: rgb(0xE8E7E3),
    muted: rgb(0xAAA69D),
    accent: rgb(0xBFC5AA),
    accent_soft: rgb(0x383D29),
    on_accent: rgb(0x1E2112),
    on_status: rgb(0x1A1914),
    good: rgb(0x88BFA3),
    warn: rgb(0xDBA480),
    bad: rgb(0xDA8B95),
    table: rgb(0x36332B),
    canvas: rgb(0x161410),
    grid: rgb(0x2F2C23),
    shadow: rgb(0x000000),
};

pub const LIGHT: Palette = Palette {
    bg: rgb(0xF2F0EA),
    surface: rgb(0xFCFBF7),
    surface_hi: rgb(0xEDEBE3),
    border: rgb(0xDCD9D0),
    text: rgb(0x28261F),
    muted: rgb(0x6A665D),
    accent: rgb(0x666E4B),
    accent_soft: rgb(0xE2E7D3),
    on_accent: rgb(0xFFFFFF),
    on_status: rgb(0xFFFFFF),
    good: rgb(0x406D57),
    warn: rgb(0x955D37),
    bad: rgb(0x9F414E),
    table: rgb(0xE3DBC8),
    canvas: rgb(0xF9F8F6),
    grid: rgb(0xE8E4D9),
    shadow: rgb(0x3D3829),
};

/// Organization avatar colours: one soft, muted family with evenly spaced
/// hues (moss, sage, teal, slate blue, heather, mauve, rose, straw), holding
/// white initials: (dark theme, light theme).
const AVATARS: [(u32, u32); 8] = [
    (0x8D9867, 0x6E7750),
    (0x679868, 0x507A52),
    (0x67988E, 0x4F7A71),
    (0x677C98, 0x51647E),
    (0x796798, 0x625283),
    (0x986791, 0x7C537A),
    (0x98676B, 0x7E5357),
    (0x988A67, 0x7A6E4F),
];

pub fn palette(ctx: &egui::Context) -> Palette {
    if ctx.global_style().visuals.dark_mode {
        DARK
    } else {
        LIGHT
    }
}

/// Avatar colour for a key (an organization), stable across runs.
pub fn avatar_color(key: &str, dark: bool) -> Color32 {
    let hash = key
        .bytes()
        .fold(2166136261_u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    let (on_dark, on_light) = AVATARS[hash as usize % AVATARS.len()];
    rgb(if dark { on_dark } else { on_light })
}

/// Distinct colours for colouring the chart by category, most common first
/// (on dark, on light).
const CATEGORIES: [(u32, u32); 10] = [
    (0x6F9BD1, 0x3F6FAD),
    (0xE8A15A, 0xC9772A),
    (0x7DBE79, 0x4C8F48),
    (0xD9777A, 0xB8484C),
    (0xA98BCF, 0x7A5AA6),
    (0x6CC0B8, 0x3C8F87),
    (0xD4B04A, 0xA08022),
    (0xD88BB7, 0xB0588A),
    (0xA8907C, 0x7E6450),
    (0x9AA6B2, 0x67727E),
];

/// The colour of the `index`-th most common category; `None` past the palette.
pub fn category_color(index: usize, dark: bool) -> Option<Color32> {
    let (on_dark, on_light) = *CATEGORIES.get(index)?;
    Some(rgb(if dark { on_dark } else { on_light }))
}

/// Consistent row insets for dropdown and context menus.
pub fn menu_style(style: &mut egui::Style) {
    egui::containers::menu::menu_style(style);
    style.spacing.button_padding = egui::vec2(space::M, 6.0);
    style.spacing.interact_size.y = CONTROL_HEIGHT;
    style.spacing.menu_margin = Margin::same(space::M as i8);
}

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.set_visuals_of(Theme::Dark, visuals(DARK, true));
    ctx.set_visuals_of(Theme::Light, visuals(LIGHT, false));
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, FontId::proportional(text::TITLE)),
            (TextStyle::Body, FontId::proportional(text::BODY)),
            (TextStyle::Button, FontId::proportional(text::BODY)),
            (TextStyle::Small, FontId::proportional(text::SMALL)),
            (TextStyle::Monospace, FontId::monospace(text::SMALL)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(space::S, space::S);
        style.spacing.button_padding = egui::vec2(space::M + 2.0, 6.0);
        style.spacing.interact_size.y = CONTROL_HEIGHT;
        style.spacing.window_margin = Margin::same(space::L as i8);
        style.spacing.menu_margin = Margin::same(space::S as i8);
        style.interaction.tooltip_delay = 0.35;
        style.spacing.combo_width = 160.0;
        style.spacing.slider_width = 280.0;
    });
}

fn visuals(p: Palette, dark: bool) -> Visuals {
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let radius = CornerRadius::same(tokens::radius::MD);
    let shadow_lg = widgets::shadow_lg(p);
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    // Text fields: a recessed fill with a clear border; the focus ring uses the accent.
    // Progress tracks and scroll wells: recessed below the surface.
    v.extreme_bg_color = if dark { p.bg } else { p.surface_hi };
    v.text_edit_bg_color = Some(if dark { p.bg } else { p.surface });
    v.faint_bg_color = p.surface_hi;
    // Soft treatment: floating layers carry a tinted shadow instead of a border.
    v.window_stroke = Stroke::NONE;
    v.window_corner_radius = CornerRadius::same(tokens::radius::LG);
    v.menu_corner_radius = CornerRadius::same(tokens::radius::MD);
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.hyperlink_color = p.accent;
    v.override_text_color = Some(p.text);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    // Separators and field outlines: one hairline.
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    // Rails, checkboxes and radio wells: a step darker than button fills.
    w.inactive.bg_fill = blend(p.surface_hi, p.border, 0.7);
    w.inactive.weak_bg_fill = p.surface_hi;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    let hover = blend(p.surface_hi, p.border, 0.6);
    w.hovered.bg_fill = hover;
    w.hovered.weak_bg_fill = hover;
    w.hovered.bg_stroke = Stroke::NONE;
    w.hovered.fg_stroke = Stroke::new(1.0, p.text);
    w.active.bg_fill = p.accent_soft;
    w.active.weak_bg_fill = p.accent_soft;
    w.active.bg_stroke = Stroke::new(1.0, p.accent);
    w.active.fg_stroke = Stroke::new(1.0, p.text);
    w.open.weak_bg_fill = p.surface_hi;
    w.open.bg_fill = p.surface_hi;
    w.open.bg_stroke = Stroke::new(1.0, p.accent);
    w.open.fg_stroke = Stroke::new(1.0, p.text);
    v.slider_trailing_fill = true;
    v.text_cursor.stroke = Stroke::new(2.0, p.accent);
    v.warn_fg_color = p.warn;
    v.error_fg_color = p.bad;
    v.window_shadow = shadow_lg;
    v.popup_shadow = shadow_lg;
    for state in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        state.corner_radius = radius;
        state.expansion = 0.0;
    }
    v
}

/// The interface font: Source Sans 3, the same family as the prints, bundled
/// so the app looks the same on every system and in the browser.
fn install_fonts(ctx: &egui::Context) {
    const REGULAR: &[u8] = include_bytes!("../../../typeset/fonts/SourceSans3-Regular.otf");
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("ui".into(), FontData::from_static(REGULAR).into());
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, "ui".into());
    }
    ctx.set_fonts(fonts);
}

pub fn blend(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}
