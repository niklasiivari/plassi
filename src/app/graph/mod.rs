//! The Connections page: an interactive graph of who wished to sit with whom,
//! coloured by how well each wish is met in the current option.

mod data;
mod layout;
mod panel;
mod tables;
mod view;

use std::collections::HashMap;

use eframe::egui::{self, Vec2};

use super::PlassiApp;
use super::theme::{self, Palette};
use layout::ForceLayout;


/// One wish from one person to another.
struct Edge {
    from: usize,
    to: usize,
    avec: bool,
    mutual: bool,
    /// How well the wish is met; `None` without an option to judge.
    outcome: Option<crate::score::Outcome>,
}

/// The graph for the current project and settings.
struct Graph {
    /// Attendee ID per node.
    people: Vec<usize>,
    index: HashMap<usize, usize>,
    edges: Vec<Edge>,
    degree: Vec<usize>,
    /// Wish groups (connected components) with two or more people.
    groups: usize,
    largest_group: usize,
}

/// State of the Connections page.
pub(in crate::app) struct GraphState {
    graph: Option<Graph>,
    layout: Option<ForceLayout>,
    /// Revision and settings the graph was built for.
    built_for: Option<(u64, usize, bool, bool)>,
    /// Remembered positions by attendee, so rebuilds keep the picture stable.
    remembered: HashMap<usize, Vec2>,
    camera: crate::app::camera::Camera,
    /// Table zones when arranging by table.
    zones: Option<tables::TableZones>,
    /// Keep the whole graph in view until it settles or the user moves the view.
    fit_pending: bool,
    hovered: Option<usize>,
    /// The last hovered node, kept while its highlight fades out.
    spotlight: Option<usize>,
    show_everyone: bool,
    by_table: bool,
    labels: bool,
}

impl Default for GraphState {
    fn default() -> Self {
        Self {
            graph: None,
            layout: None,
            built_for: None,
            remembered: HashMap::new(),
            camera: crate::app::camera::Camera::new(0.2, 4.0),
            zones: None,
            fit_pending: true,
            hovered: None,
            spotlight: None,
            show_everyone: false,
            by_table: false,
            labels: true,
        }
    }
}

impl GraphState {
#[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn set_by_table(&mut self, on: bool) {
        self.by_table = on;
    }

    /// Keeps each person's position so a rebuilt graph starts where it was.
    fn remember_positions(&mut self) {
        if let (Some(graph), Some(layout)) = (&self.graph, &self.layout) {
            for (i, person) in graph.people.iter().enumerate() {
                self.remembered.insert(*person, layout.positions[i]);
            }
        }
    }
}

/// A layout for `graph` that starts from remembered positions. People seen
/// before stay put; newcomers appear next to someone they are linked to.
/// Returns the layout and whether every person was known.
fn build_layout(
    graph: &Graph,
    remembered: &HashMap<usize, Vec2>,
    zones: Option<&tables::TableZones>,
) -> (ForceLayout, bool) {
    let mut layout = ForceLayout::new(graph.people.len());
    let known: Vec<bool> = graph
        .people
        .iter()
        .map(|p| remembered.contains_key(p))
        .collect();
    for (i, person) in graph.people.iter().enumerate() {
        if let Some(pos) = remembered.get(person) {
            layout.positions[i] = *pos;
        }
    }
    for (i, _) in graph.people.iter().enumerate().filter(|(i, _)| !known[*i]) {
        let neighbour = graph
            .edges
            .iter()
            .find_map(|e| match (e.from == i, e.to == i) {
                (true, _) if known[e.to] => Some(e.to),
                (_, true) if known[e.from] => Some(e.from),
                _ => None,
            });
        if let Some(n) = neighbour {
            let angle = i as f32 * 2.399_963;
            layout.positions[i] = layout.positions[n] + Vec2::new(angle.cos(), angle.sin()) * 40.0;
        }
    }
    layout.links = graph
        .edges
        .iter()
        .map(|e| (e.from, e.to, if e.avec || e.mutual { 1.4 } else { 1.0 }))
        .collect();
    if let Some(zones) = zones {
        layout.anchors = Some(
            zones
                .seat_of
                .iter()
                .map(|s| s.map(|(point, _)| point))
                .collect(),
        );
    }
    let all_known = known.iter().all(|k| *k);
    if all_known {
        layout.calm(0.35);
    }
    (layout, all_known)
}

impl PlassiApp {
    pub(in crate::app) fn graph_page(&mut self, ui: &mut egui::Ui, p: Palette) {
        self.refresh_graph();
        egui::Panel::right("graph-panel")
            .resizable(false)
            .exact_size(theme::PANEL_WIDTH)
            .show_separator_line(false)
            .frame(theme::panel_frame(p))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.graph_panel(ui, p));
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.canvas))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                self.graph_canvas(ui, p);
                theme::panel_edge_shadows(ui, p, rect);
            });
    }

    /// Rebuilds the graph when the project, option or settings change.
    fn refresh_graph(&mut self) {
        let candidate = (!self.project.candidates.is_empty()).then_some(self.selected_candidate);
        let state = &mut self.graph;
        let key = (
            self.file.revision,
            candidate.unwrap_or(usize::MAX),
            state.show_everyone,
            state.by_table,
        );
        if state.built_for == Some(key) {
            return;
        }
        state.remember_positions();
        let graph = Graph::build(&self.project, candidate, state.show_everyone);
        state.zones = if state.by_table {
            tables::table_zones(&self.project, candidate, &graph)
        } else {
            None
        };
        let (mut layout, all_known) = build_layout(&graph, &state.remembered, state.zones.as_ref());
        if state.zones.is_some() {
            layout.reheat(0.8);
        }
        if !all_known {
            state.fit_pending = true;
        }
        state.graph = Some(graph);
        state.layout = Some(layout);
        state.built_for = Some(key);
    }
}
