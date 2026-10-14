//! Drawing and interaction for the Connections graph.

use crate::i18n::{fill, tr};
use eframe::egui::{self, Align2, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use super::Graph;
use super::layout::ForceLayout;
use crate::app::PlassiApp;
use crate::app::camera::Viewport;
use crate::app::theme::{self, Palette};
use crate::model::{Project, Side};

const NODE: f32 = 7.0;
/// Opacity of nodes and lines unrelated to the hovered person, fully faded.
const DIM_NODES: f32 = 0.35;
const DIM_EDGES: f32 = 0.3;
/// Seconds for the highlight to fade in or out.
const FADE: f32 = 0.2;

/// Which node is highlighted, and how far the fade has progressed (0..1).
#[derive(Clone, Copy)]
struct Spotlight {
    node: Option<usize>,
    amount: f32,
}

impl Spotlight {
    /// Opacity for something that is (or isn't) linked to the highlighted node.
    fn opacity(self, linked: bool, dim: f32) -> f32 {
        if linked {
            1.0
        } else {
            1.0 - self.amount * (1.0 - dim)
        }
    }
}

/// On-screen radius of a node.
fn node_radius(view: &Viewport, graph: &Graph, node: usize) -> f32 {
    (NODE + (graph.degree[node] as f32).sqrt() * 2.2) * view.zoom.clamp(0.6, 1.6)
}

impl PlassiApp {
    pub(super) fn graph_canvas(&mut self, ui: &mut egui::Ui, p: Palette) {
        let rect = ui.max_rect();
        let background = ui.interact(rect, ui.id().with("graph"), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let state = &mut self.graph;
        let (Some(graph), Some(layout)) = (&state.graph, &mut state.layout) else {
            return;
        };
        if graph.people.is_empty() {
            let text = tr("Add wishes on People & wishes");
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                text,
                FontId::proportional(theme::text::LEAD),
                p.muted,
            );
            return;
        }
        if !layout.settled() {
            for _ in 0..2 {
                layout.step();
            }
            ui.ctx().request_repaint();
        }
        let camera = &mut state.camera;
        if state.fit_pending {
            let mut bounds = node_bounds(&layout.positions);
            for zone in state.zones.iter().flat_map(|z| &z.zones) {
                bounds = bounds.union(zone.rect.expand(60.0));
            }
            camera.fit(rect, bounds, 1.6);
            state.fit_pending = !layout.settled();
        }
        if camera.handle_input(ui, rect, &background, layout.held.is_none()) {
            state.fit_pending = false;
        }
        camera.animate(ui.ctx());
        let view = camera.viewport(rect);
        let pointer = ui.input(|i| i.pointer.hover_pos());
        // While dragging, the held node stays highlighted even if the pointer
        // runs ahead of it.
        state.hovered = layout
            .held
            .or_else(|| pointer.and_then(|pos| node_at(graph, layout, &view, pos)));
        if state.hovered.is_some() {
            state.spotlight = state.hovered;
        }
        let amount = ui.ctx().animate_bool_with_time(
            ui.id().with("spotlight"),
            state.hovered.is_some(),
            FADE,
        );
        if amount == 0.0 {
            state.spotlight = None;
        }
        let spotlight = Spotlight {
            node: state.spotlight,
            amount,
        };
        let selected = self
            .selected_person
            .and_then(|id| graph.index.get(&id).copied());
        if let Some(zones) = &state.zones {
            paint_zones(&painter, &view, p, zones);
        }
        paint_edges(&painter, &view, p, graph, &layout.positions, spotlight);
        let focus = NodeFocus {
            spotlight,
            selected,
            labels: state.labels,
            clip: rect,
            sides: state
                .zones
                .as_ref()
                .map(|z| z.seat_of.iter().map(|s| s.map(|(_, side)| side)).collect()),
        };
        paint_nodes(&painter, &view, p, &self.project, graph, layout, focus);
        if let Some(clicked) = drag_nodes(
            ui,
            &background,
            &view,
            graph,
            layout,
            state.hovered,
            pointer,
        ) {
            self.selected_person = clicked.map(|i| graph.people[i]);
        }
        self.graph_controls(ui, p, rect);
    }

    /// Floating buttons in the canvas corner: fit to view and re-arrange.
    fn graph_controls(&mut self, ui: &mut egui::Ui, p: Palette, rect: Rect) {
        let size = egui::vec2(
            2.0 * theme::CONTROL_HEIGHT + theme::space::XS,
            theme::CONTROL_HEIGHT,
        );
        let area = Rect::from_min_size(
            rect.right_top() + egui::vec2(-size.x - theme::space::L, theme::space::L),
            size,
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(area)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = theme::space::XS;
                if theme::icon_button(ui, p, theme::Icon::Fit, tr("Fit everyone into view"), false)
                    .clicked()
                {
                    self.graph.fit_pending = true;
                }
                if theme::icon_button(ui, p, theme::Icon::Shuffle, tr("Rearrange"), false).clicked()
                    && let Some(layout) = &mut self.graph.layout
                {
                    layout.shake();
                    self.graph.fit_pending = true;
                }
            },
        );
    }
}

/// Tables drawn like on the chart: an outline around the seats, the table
/// itself down the middle, and the name with how many wishes reach other tables.
fn paint_zones(
    painter: &egui::Painter,
    view: &Viewport,
    p: Palette,
    zones: &super::tables::TableZones,
) {
    for zone in &zones.zones {
        let rect = Rect::from_min_max(view.point(zone.rect.min), view.point(zone.rect.max));
        let rounding = (14.0 * view.zoom).clamp(4.0, 18.0) as u8;
        painter.rect(
            rect,
            rounding,
            p.surface.gamma_multiply(0.6),
            Stroke::new(1.0, p.border),
            egui::StrokeKind::Inside,
        );
        let half = super::tables::TABLE_HALF_WIDTH * view.zoom;
        let body = Rect::from_min_max(
            egui::pos2(rect.center().x - half, rect.min.y + 24.0 * view.zoom),
            egui::pos2(rect.center().x + half, rect.max.y - 24.0 * view.zoom),
        );
        painter.rect_filled(body, (8.0 * view.zoom).clamp(2.0, 10.0) as u8, p.table);
        let top = rect.center_top() - Vec2::new(0.0, theme::space::S);
        let title = top - Vec2::new(0.0, theme::text::CAPTION + theme::space::XS);
        painter.text(
            title,
            Align2::CENTER_BOTTOM,
            &zone.label,
            FontId::proportional(theme::text::LEAD),
            p.text,
        );
        let mut detail = if zone.members == 1 {
            tr("1 person").to_string()
        } else {
            fill("{} people", &[&zone.members])
        };
        if zone.outgoing > 0 {
            let wishes = if zone.outgoing == 1 {
                tr("1 wish to other tables").to_string()
            } else {
                fill("{} wishes to other tables", &[&zone.outgoing])
            };
            detail.push_str(&format!(" · {wishes}"));
        }
        let color = if zone.outgoing > 0 { p.warn } else { p.muted };
        painter.text(
            top,
            Align2::CENTER_BOTTOM,
            detail,
            FontId::proportional(theme::text::CAPTION),
            color,
        );
    }
}

/// World rectangle around every node, with a margin for labels.
fn node_bounds(positions: &[Vec2]) -> Rect {
    positions
        .iter()
        .fold(Rect::NOTHING, |r, p| {
            r.union(Rect::from_center_size(p.to_pos2(), Vec2::splat(1.0)))
        })
        .expand(70.0)
}

/// The node under the pointer, if any.
fn node_at(graph: &Graph, layout: &ForceLayout, view: &Viewport, pos: Pos2) -> Option<usize> {
    let distance = |i: usize| (view.point(layout.positions[i].to_pos2()) - pos).length();
    (0..graph.people.len())
        .filter(|&i| distance(i) <= node_radius(view, graph, i) + 3.0)
        .min_by(|&a, &b| distance(a).total_cmp(&distance(b)))
}

/// Dragging moves a node (the layout flows around it) and selects it; a click
/// returns the node clicked, or `Some(None)` for the background.
fn drag_nodes(
    ui: &egui::Ui,
    background: &egui::Response,
    view: &Viewport,
    graph: &Graph,
    layout: &mut ForceLayout,
    hovered: Option<usize>,
    pointer: Option<Pos2>,
) -> Option<Option<usize>> {
    let mut selection = None;
    if background.drag_started() {
        // A fast drag has left the node by the time it registers, so grab
        // whatever was under the pointer when the button went down.
        let pressed_at = ui.input(|i| i.pointer.press_origin());
        layout.held = pressed_at.and_then(|pos| node_at(graph, layout, view, pos));
        if layout.held.is_some() {
            selection = Some(layout.held);
        }
    }
    match (layout.held, pointer) {
        (Some(held), Some(pos)) if background.dragged() => {
            layout.positions[held] = view.world(pos).to_vec2();
            layout.reheat(0.3);
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        _ if hovered.is_some() => ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand),
        _ => {}
    }
    if background.drag_stopped() {
        layout.held = None;
    }
    if background.clicked() {
        selection = Some(hovered);
    }
    selection
}

/// What to emphasise when drawing nodes.
struct NodeFocus {
    spotlight: Spotlight,
    selected: Option<usize>,
    labels: bool,
    clip: Rect,
    /// Seat side per node when arranging by table; names go on the outer side.
    sides: Option<Vec<Option<Side>>>,
}

/// Dots coloured by organization; hovering fades everyone not linked to the hovered person.
fn paint_nodes(
    painter: &egui::Painter,
    view: &Viewport,
    p: Palette,
    project: &Project,
    graph: &Graph,
    layout: &ForceLayout,
    focus: NodeFocus,
) {
    let linked = |i: usize| {
        focus.spotlight.node.is_none_or(|h| {
            h == i
                || graph
                    .edges
                    .iter()
                    .any(|e| (e.from == h && e.to == i) || (e.to == h && e.from == i))
        })
    };
    let dark = p.is_dark();
    for (i, &person) in graph.people.iter().enumerate() {
        let Some(attendee) = project.person(person) else {
            continue;
        };
        let centre = view.point(layout.positions[i].to_pos2());
        if !focus.clip.expand(60.0).contains(centre) {
            continue;
        }
        let radius = node_radius(view, graph, i);
        let alpha = focus.spotlight.opacity(linked(i), DIM_NODES);
        let selected = focus.selected == Some(i);
        painter.circle_filled(
            centre,
            radius,
            theme::avatar_color(
                &project.category_key(crate::catalog::Kind::Organization, attendee),
                dark,
            )
            .gamma_multiply(alpha),
        );
        let ring = if selected {
            Stroke::new(2.5, p.accent)
        } else {
            Stroke::new(1.0, p.bg.gamma_multiply(alpha))
        };
        painter.circle_stroke(centre, radius, ring);
        let emphasised =
            selected || focus.spotlight.node.is_some() && focus.spotlight.amount > 0.5 && linked(i);
        if focus.labels && (view.zoom > 0.4 || emphasised) {
            let size = (11.5 * view.zoom.sqrt()).clamp(9.0, 14.0);
            let color = if emphasised {
                p.text
            } else {
                p.text.gamma_multiply(0.85 * alpha)
            };
            let side = focus.sides.as_ref().and_then(|s| s[i]);
            let (at, align) = match side {
                Some(Side::Left) => (centre - Vec2::new(radius + 6.0, 0.0), Align2::RIGHT_CENTER),
                Some(Side::Right) => (centre + Vec2::new(radius + 6.0, 0.0), Align2::LEFT_CENTER),
                None => (centre + Vec2::new(0.0, radius + 3.0), Align2::CENTER_TOP),
            };
            label(painter, p, at, align, &attendee.name, size, color);
        }
    }
}

/// Text on a soft backdrop so lines behind it don't hurt legibility.
fn label(
    painter: &egui::Painter,
    p: Palette,
    at: Pos2,
    align: Align2,
    text: &str,
    size: f32,
    color: egui::Color32,
) {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(size), color);
    let rect = align.anchor_size(at, galley.size() + Vec2::new(6.0, 2.0));
    painter.rect_filled(rect, 3.0, p.canvas.gamma_multiply(0.8));
    painter.galley(rect.center() - galley.size() / 2.0, galley, color);
}

/// Wish lines with arrowheads at the wished-for person; mutual wishes have none.
fn paint_edges(
    painter: &egui::Painter,
    view: &Viewport,
    p: Palette,
    graph: &Graph,
    positions: &[Vec2],
    spotlight: Spotlight,
) {
    for edge in &graph.edges {
        let involved = spotlight
            .node
            .is_none_or(|h| edge.from == h || edge.to == h);
        let color = theme::outcome_color(p, edge.outcome)
            .gamma_multiply(0.9 * spotlight.opacity(involved, DIM_EDGES));
        let width = if edge.avec {
            3.5
        } else if edge.mutual {
            2.4
        } else {
            1.6
        } * view.zoom.clamp(0.7, 1.3);
        let (a, b) = (
            view.point(positions[edge.from].to_pos2()),
            view.point(positions[edge.to].to_pos2()),
        );
        let direction = (b - a).normalized();
        let start = a + direction * (node_radius(view, graph, edge.from) + 2.0);
        let end = b - direction * (node_radius(view, graph, edge.to) + 2.0);
        painter.line_segment([start, end], Stroke::new(width, color));
        if !edge.mutual {
            let side = Vec2::new(-direction.y, direction.x);
            let size = 7.0 * view.zoom.clamp(0.7, 1.3);
            let head = vec![
                end,
                end - direction * size + side * size * 0.5,
                end - direction * size - side * size * 0.5,
            ];
            painter.add(egui::Shape::convex_polygon(head, color, Stroke::NONE));
        }
    }
}
