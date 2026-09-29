//! Vector icons drawn with shapes, so they look the same everywhere and need no icon font.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use super::{CONTROL_HEIGHT, Palette, radius};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    // Pages.
    Chart,
    People,
    Graph,
    Categories,
    Policy,
    Room,
    Print,
    // Controls.
    SidePanel,
    Sidebar,
    Fit,
    ZoomIn,
    ZoomOut,
    Undo,
    Redo,
    Close,
    Focus,
    Sun,
    Moon,
    Shuffle,
    ChevronDown,
}

/// Draws `icon` centred on `c`, about 16 points across.
pub fn paint(painter: &egui::Painter, icon: Icon, c: Pos2, color: Color32) {
    let stroke = Stroke::new(1.6, color);
    let line = |a: Vec2, b: Vec2| {
        painter.line_segment([c + a, c + b], stroke);
    };
    let frame = |size: Vec2| {
        let r = Rect::from_center_size(c, size);
        painter.rect_stroke(r, 2.5, stroke, StrokeKind::Inside);
        r
    };
    match icon {
        Icon::Chart => {
            for dx in [-4.0, 4.0] {
                for dy in [-5.0, 0.0, 5.0] {
                    painter.rect_filled(
                        Rect::from_center_size(c + vec2(dx, dy), vec2(5.0, 3.0)),
                        1.0,
                        color,
                    );
                }
            }
        }
        Icon::People => {
            painter.circle_stroke(c + vec2(0.0, -3.5), 3.0, stroke);
            painter.add(egui::Shape::line(
                arc(
                    c + vec2(0.0, 7.5),
                    6.0,
                    std::f32::consts::PI,
                    std::f32::consts::TAU,
                ),
                stroke,
            ));
        }
        Icon::Graph => {
            let nodes = [c + vec2(-5.5, 4.0), c + vec2(5.5, 4.0), c + vec2(0.0, -5.5)];
            for (i, a) in nodes.iter().enumerate() {
                painter.line_segment([*a, nodes[(i + 1) % 3]], stroke);
            }
            for n in nodes {
                painter.circle_filled(n, 2.6, color);
            }
        }
        Icon::Categories => {
            for dy in [-5.0_f32, 0.0, 5.0] {
                painter.circle_filled(c + vec2(-6.0, dy), 1.6, color);
                line(vec2(-2.5, dy), vec2(7.0, dy));
            }
        }
        Icon::Policy => {
            for (dy, knob) in [(-5.0_f32, -3.0), (0.0, 3.5), (5.0, -0.5)] {
                line(vec2(-7.0, dy), vec2(7.0, dy));
                painter.circle_filled(c + vec2(knob, dy), 2.4, color);
            }
        }
        Icon::Room => {
            let r = frame(vec2(15.0, 13.0));
            painter.line_segment([r.center_top(), r.center_bottom()], stroke);
            painter.line_segment([r.left_center(), r.right_center()], stroke);
        }
        Icon::Print => {
            painter.rect_stroke(
                Rect::from_center_size(c + vec2(0.0, 1.0), vec2(15.0, 7.0)),
                2.0,
                stroke,
                StrokeKind::Inside,
            );
            painter.rect_stroke(
                Rect::from_min_max(c + vec2(-4.5, -7.0), c + vec2(4.5, -2.5)),
                0.5,
                stroke,
                StrokeKind::Inside,
            );
            painter.rect_filled(
                Rect::from_min_max(c + vec2(-4.5, 2.5), c + vec2(4.5, 7.5)),
                0.5,
                color,
            );
        }
        Icon::SidePanel | Icon::Sidebar => {
            let r = frame(vec2(16.0, 13.0));
            let x = if icon == Icon::SidePanel {
                r.max.x - 5.0
            } else {
                r.min.x + 5.0
            };
            painter.line_segment([pos2(x, r.min.y), pos2(x, r.max.y)], stroke);
        }
        Icon::Fit => {
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let corner = vec2(6.5 * sx, 6.5 * sy);
                line(corner, corner - vec2(4.0 * sx, 0.0));
                line(corner, corner - vec2(0.0, 4.0 * sy));
            }
        }
        Icon::ZoomIn | Icon::ZoomOut => {
            line(vec2(-6.0, 0.0), vec2(6.0, 0.0));
            if icon == Icon::ZoomIn {
                line(vec2(0.0, -6.0), vec2(0.0, 6.0));
            }
        }
        Icon::Undo | Icon::Redo => {
            // An arrow pointing back (left for undo), whose tail curls down
            // and returns underneath: ←⌒ shape.
            let flip = if icon == Icon::Undo { 1.0 } else { -1.0 };
            let at = |x: f32, y: f32| c + vec2(x * flip, y);
            let mut path = vec![at(-6.0, -3.0), at(2.0, -3.0)];
            path.extend((1..=12).map(|i| {
                let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 12.0;
                at(2.0 + 4.5 * a.cos(), 1.5 + 4.5 * a.sin())
            }));
            path.push(at(-2.0, 6.0));
            painter.add(egui::Shape::line(path, stroke));
            painter.line_segment([at(-6.0, -3.0), at(-2.5, -6.5)], stroke);
            painter.line_segment([at(-6.0, -3.0), at(-2.5, 0.5)], stroke);
        }
        Icon::Close => {
            line(vec2(-5.0, -5.0), vec2(5.0, 5.0));
            line(vec2(-5.0, 5.0), vec2(5.0, -5.0));
        }
        Icon::Focus => {
            painter.circle_stroke(c, 4.5, stroke);
            for d in [
                vec2(0.0, -1.0),
                vec2(0.0, 1.0),
                vec2(-1.0, 0.0),
                vec2(1.0, 0.0),
            ] {
                line(d * 5.5, d * 8.0);
            }
        }
        Icon::Sun => {
            painter.circle_stroke(c, 3.5, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let d = vec2(a.cos(), a.sin());
                line(d * 6.0, d * 8.0);
            }
        }
        Icon::Moon => {
            painter.add(egui::Shape::line(arc(c, 6.5, 0.9, 5.4), stroke));
            painter.add(egui::Shape::line(
                arc(c + vec2(2.6, -1.6), 4.8, 1.6, 4.6),
                stroke,
            ));
        }
        Icon::ChevronDown => {
            line(vec2(-4.0, -2.0), vec2(0.0, 2.0));
            line(vec2(0.0, 2.0), vec2(4.0, -2.0));
        }
        Icon::Shuffle => {
            line(vec2(-7.0, -4.0), vec2(-3.0, -4.0));
            line(vec2(-3.0, -4.0), vec2(3.0, 4.0));
            line(vec2(3.0, 4.0), vec2(7.0, 4.0));
            line(vec2(-7.0, 4.0), vec2(-3.0, 4.0));
            line(vec2(-3.0, 4.0), vec2(3.0, -4.0));
            line(vec2(3.0, -4.0), vec2(7.0, -4.0));
            line(vec2(7.0, 4.0), vec2(5.0, 2.0));
            line(vec2(7.0, -4.0), vec2(5.0, -6.0));
        }
    }
}

fn arc(centre: Pos2, r: f32, from: f32, to: f32) -> Vec<Pos2> {
    (0..=16)
        .map(|i| {
            let a = from + (to - from) * i as f32 / 16.0;
            centre + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

/// A square icon button the size of other controls. `on` shows it pressed.
pub fn icon_button(
    ui: &mut egui::Ui,
    p: Palette,
    icon: Icon,
    tooltip: &str,
    on: bool,
) -> egui::Response {
    icon_button_enabled(ui, p, icon, tooltip, on, true)
}

pub fn icon_button_enabled(
    ui: &mut egui::Ui,
    p: Palette,
    icon: Icon,
    tooltip: &str,
    on: bool,
    enabled: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::splat(CONTROL_HEIGHT),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let fill = match () {
        _ if on => p.accent_soft,
        _ if enabled && response.hovered() => p.surface_hi,
        _ => Color32::TRANSPARENT,
    };
    ui.painter().rect_filled(rect, radius::MD, fill);
    let color = match () {
        _ if !enabled => p.muted.gamma_multiply(0.45),
        _ if on => p.accent,
        _ if response.hovered() => p.text,
        _ => p.muted,
    };
    paint(ui.painter(), icon, rect.center(), color);
    let response = response.on_hover_text(tooltip);
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}
