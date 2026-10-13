//! Painting one seat card: empty seat, person card, avatar, badges.

use super::*;

/// World-space rectangle of the card drawn for a seat.
pub(super) fn card_rect(seat: &Seat) -> Rect {
    let dx = match seat.side {
        Side::Left => -CARD_OFFSET,
        Side::Right => CARD_OFFSET,
    };
    Rect::from_center_size(Pos2::new(seat.x + dx, seat.y), Vec2::new(CARD_W, CARD_H))
}

pub(super) fn initials(name: &str) -> String {
    theme::initials(name)
}

#[derive(Clone, Copy, Default)]
pub(super) struct CardLook {
    /// How well this person's wishes are met; `None` for an empty seat.
    pub(super) status: Option<Outcome>,
    pub(super) selected: bool,
    pub(super) hovered: bool,
    pub(super) dimmed: bool,
    pub(super) pin: Option<PinMark>,
    pub(super) moved: bool,
    pub(super) gap: bool,
    /// Off the seats, left on the chart.
    pub(super) loose: bool,
}

/// How a person is pinned: a filled lock for a seat, an outlined one for a table.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PinMark {
    Seat,
    Table,
}

/// What a seat card shows about its person.
pub(super) struct CardPerson<'a> {
    pub(super) name: &'a str,
    /// Organization as its category name.
    pub(super) organization: String,
    /// Keeps avatar colours equal for all spellings of one organization.
    pub(super) color_key: String,
}

fn card_radius(zoom: f32) -> u8 {
    (theme::radius::SM as f32 * zoom).clamp(2.0, theme::radius::SM as f32) as u8
}

/// A dashed outline along the edges of `rect`.
fn dashed_outline(painter: &egui::Painter, rect: Rect, zoom: f32, color: Color32) {
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    let dash = (4.0 * zoom).max(2.0);
    painter.extend(egui::Shape::dashed_line(
        &corners,
        Stroke::new(1.0, color),
        dash,
        dash * 0.75,
    ));
}

/// A spare seat (outlined), or a gap mid-table (red) that should be closed.
pub(super) fn paint_empty_seat(painter: &egui::Painter, p: Palette, rect: Rect, look: CardLook) {
    let zoom = rect.height() / CARD_H;
    let (fill, stroke, text, text_color) = match () {
        _ if look.hovered => (p.accent_soft, p.accent, tr("spare seat"), p.muted),
        _ if look.gap => (
            theme::blend(p.canvas, p.bad, 0.12),
            p.bad,
            tr("gap — close it"),
            p.bad,
        ),
        _ => (
            Color32::TRANSPARENT,
            theme::blend(p.border, p.muted, 0.35),
            tr("spare seat"),
            p.muted,
        ),
    };
    painter.rect_filled(rect, card_radius(zoom), fill);
    dashed_outline(painter, rect.shrink(0.5), zoom, stroke);
    if zoom > 0.45 {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(11.0 * zoom),
            text_color,
        );
    }
}

/// Frame, status stripe, organization colour, name and markers.
pub(super) fn paint_person_card(
    painter: &egui::Painter,
    p: Palette,
    rect: Rect,
    person: &CardPerson,
    look: CardLook,
) {
    let zoom = rect.height() / CARD_H;
    let alpha = if look.dimmed { 0.3 } else { 1.0 };
    let base = if look.hovered {
        p.surface_hi
    } else {
        p.surface
    };
    let fill = if look.selected { p.accent_soft } else { base }.gamma_multiply(alpha);
    let stroke = match () {
        _ if look.selected => Stroke::new(1.0, p.accent),
        _ if look.hovered => Stroke::new(1.0, theme::blend(p.surface, p.accent, 0.5)),
        _ => Stroke::NONE,
    };
    let radius = card_radius(zoom);
    painter.add(theme::seat_shadow(p, zoom, alpha).as_shape(rect, radius));
    // The status stripe follows the card's rounded left edge: the card is
    // painted in the status colour, then covered except for a thin strip.
    let stripe = look.status.map_or(0.0, |_| 3.0 * zoom.max(0.67));
    if let Some(status) = look.status {
        painter.rect_filled(
            rect,
            radius,
            theme::outcome_color(p, Some(status)).gamma_multiply(alpha),
        );
    }
    let body = Rect::from_min_max(Pos2::new(rect.min.x + stripe, rect.min.y), rect.max);
    let body_radius = if stripe > 0.0 {
        CornerRadius {
            nw: 0,
            sw: 0,
            ne: radius,
            se: radius,
        }
    } else {
        CornerRadius::same(radius)
    };
    painter.rect_filled(body, body_radius, fill);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(stroke.width, stroke.color.gamma_multiply(alpha)),
        StrokeKind::Inside,
    );
    let text_x = paint_avatar(painter, p, rect, person, alpha);
    if rect.height() > 14.0 {
        let marker_width = match (look.pin.is_some(), look.moved) {
            (true, true) => 30.0 * zoom,
            (true, false) | (false, true) => 18.0 * zoom,
            (false, false) => 0.0,
        };
        let text_rect = Rect::from_min_max(
            Pos2::new(text_x, rect.min.y),
            Pos2::new(rect.max.x - marker_width - 6.0 * zoom, rect.max.y),
        );
        paint_card_text(painter, p, rect, text_rect, person, alpha);
    }
    let corner = Pos2::new(rect.max.x - 11.0 * zoom, rect.min.y + 11.0 * zoom);
    if let Some(pin) = look.pin {
        paint_lock(
            painter,
            corner,
            9.0 * zoom,
            p.accent.gamma_multiply(alpha),
            pin,
        );
    }
    if look.loose {
        dashed_outline(
            painter,
            rect.expand(2.0 * zoom),
            zoom,
            p.muted.gamma_multiply(alpha),
        );
    }
    if look.moved {
        let at = if look.pin.is_some() {
            corner - Vec2::new(14.0 * zoom, 0.0)
        } else {
            corner
        };
        painter.circle_filled(at, 4.0 * zoom, p.warn.gamma_multiply(alpha));
    }
}

/// Organization colour, with initials when there is room. Returns the text start.
fn paint_avatar(
    painter: &egui::Painter,
    p: Palette,
    rect: Rect,
    person: &CardPerson,
    alpha: f32,
) -> f32 {
    let zoom = rect.height() / CARD_H;
    let compact = rect.height() < 34.0;
    let radius = if compact { 2.5 } else { 14.0 * zoom };
    let centre = Pos2::new(
        rect.min.x + (12.0 * zoom).max(5.0) + radius,
        rect.center().y,
    );
    let dark = p.bg.r() < 100;
    painter.circle_filled(
        centre,
        radius,
        theme::avatar_color(&person.color_key, dark).gamma_multiply(alpha),
    );
    if !compact {
        painter.text(
            centre,
            Align2::CENTER_CENTER,
            initials(person.name),
            FontId::proportional(11.0 * zoom),
            Color32::WHITE.gamma_multiply(alpha),
        );
    }
    centre.x + radius + (7.0 * zoom).max(4.0)
}

/// A readable, elided name; below it the organization unless the card is small.
fn paint_card_text(
    painter: &egui::Painter,
    p: Palette,
    rect: Rect,
    text_rect: Rect,
    person: &CardPerson,
    alpha: f32,
) {
    let zoom = rect.height() / CARD_H;
    let compact = rect.height() < 34.0;
    let size = if compact {
        (rect.height() * 0.46).clamp(11.0, 12.5)
    } else {
        (13.0 * zoom).max(11.0)
    };
    let mut job = egui::text::LayoutJob::simple_singleline(
        person.name.to_string(),
        FontId::proportional(size),
        p.text.gamma_multiply(alpha),
    );
    job.wrap.max_width = text_rect.width().max(0.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let name = painter.layout_job(job);
    let clipped = painter.with_clip_rect(text_rect.intersect(painter.clip_rect()));
    let name_y = if compact {
        rect.center().y - name.size().y / 2.0
    } else {
        rect.center().y - 15.0 * zoom
    };
    clipped.galley(Pos2::new(text_rect.min.x, name_y), name, p.text);
    if !compact {
        clipped.text(
            Pos2::new(text_rect.min.x, rect.center().y + 2.0 * zoom),
            Align2::LEFT_TOP,
            &person.organization,
            FontId::proportional(10.5 * zoom),
            p.muted.gamma_multiply(alpha),
        );
    }
}

/// A small padlock drawn with shapes (no font glyph needed).
pub(super) fn paint_lock(
    painter: &egui::Painter,
    centre: Pos2,
    size: f32,
    color: Color32,
    mark: PinMark,
) {
    let body = Rect::from_center_size(
        centre + Vec2::new(0.0, size * 0.25),
        Vec2::new(size, size * 0.7),
    );
    painter.circle_stroke(
        centre - Vec2::new(0.0, size * 0.15),
        size * 0.32,
        Stroke::new(size * 0.14, color),
    );
    match mark {
        PinMark::Seat => painter.rect_filled(body, size * 0.15, color),
        PinMark::Table => painter.rect_stroke(
            body,
            size * 0.15,
            Stroke::new(size * 0.14, color),
            StrokeKind::Inside,
        ),
    };
}

/// A pill-shaped floating label centred on `at`.
pub(super) fn badge(painter: &egui::Painter, at: Pos2, text: &str, color: Color32, p: Palette) {
    let galley = painter.layout_no_wrap(
        text.to_string(),
        FontId::proportional(theme::text::SMALL),
        color,
    );
    let rect = Rect::from_center_size(
        at,
        galley.size() + Vec2::new(theme::space::XL, theme::space::M),
    );
    let radius = theme::radius::PILL;
    painter.add(theme::shadow_lg(p).as_shape(rect, radius));
    painter.rect_filled(rect, radius, theme::blend(p.surface, color, 0.12));
    painter.galley(rect.center() - galley.size() / 2.0, galley, color);
}

pub(super) fn preview_text(preview: &DropPreview, p: Palette) -> (String, Color32) {
    if let Some(reason) = &preview.blocked {
        return (reason.clone(), p.bad);
    }
    let wishes = match preview.met_delta {
        0 => String::new(),
        1 => tr("1 more wish met · ").into(),
        -1 => tr("1 wish fewer met · ").into(),
        n if n > 0 => fill("{} more wishes met · ", &[&n]),
        n => fill("{} wishes fewer met · ", &[&-n]),
    };
    let mut text = format!("{wishes}{}", fill("score {}", &[&signed(preview.delta)]));
    if preview.hard_delta > 0 {
        text.push_str(tr(" · breaks a hard rule"));
        return (text, p.bad);
    }
    if preview.hard_delta < 0 {
        text.push_str(tr(" · fixes a hard rule"));
    }
    let color = match preview.delta {
        d if d < 0 => p.good,
        0 => p.muted,
        _ => p.warn,
    };
    (text, color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_names_are_elided_without_shrinking_or_overlapping_markers() {
        let ctx = egui::Context::default();
        for name in [
            "A very long guest name that cannot fit",
            "Ääkkönen 李明 Åström",
        ] {
            for (pinned, moved) in [(false, false), (true, false), (true, true)] {
                let rect = Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::new(76.0, 19.0));
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    let ctx = ui.ctx();
                    paint_person_card(
                        &ctx.layer_painter(egui::LayerId::background()),
                        theme::palette(ctx),
                        rect,
                        &CardPerson {
                            name,
                            organization: "Organization".into(),
                            color_key: "Organization".into(),
                        },
                        CardLook {
                            status: Some(Outcome::Met),
                            pin: pinned.then_some(PinMark::Seat),
                            moved,
                            ..Default::default()
                        },
                    );
                });
                output.textures_delta.clear();
                let text = output
                    .shapes
                    .iter()
                    .find_map(|shape| {
                        if let egui::Shape::Text(text) = &shape.shape {
                            Some((shape.clip_rect, text))
                        } else {
                            None
                        }
                    })
                    .expect("guest name is visible");
                let (clip, text) = text;
                assert!(text.galley.elided);
                assert_eq!(text.galley.rows.len(), 1);
                assert_eq!(text.galley.job.text, name);
                assert!(
                    text.galley
                        .job
                        .sections
                        .iter()
                        .all(|s| s.format.font_id.size >= 11.0)
                );
                assert!(text.galley.size().x <= clip.width() + 0.5);
                let zoom = rect.height() / CARD_H;
                let marker_width = if pinned && moved {
                    30.0 * zoom
                } else if pinned {
                    18.0 * zoom
                } else {
                    0.0
                };
                assert!(clip.right() <= rect.right() - marker_width);
            }
        }
    }
}
