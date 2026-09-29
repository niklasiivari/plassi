//! The seating chart page: a pan/zoom canvas of tables and people, a toolbar and an inspector panel.

mod canvas;
mod card;
mod edits;
mod inspector;
mod moves;
mod person;
mod preferences;
mod scene;
mod setup;
pub(in crate::app) mod toolbar;

use canvas::*;
use card::*;
use edits::*;
use scene::*;

use std::collections::{BTreeMap, HashMap, HashSet};

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
    Vec2,
};

use crate::catalog::Kind;
use crate::i18n::{fill, tr};
use crate::model::{Project, RuleKind, Seat, Side, TABLE_SPAN};
use crate::score::{self, Evaluation, Outcome, RuleResult};
use crate::seating::PolicyReport;

use super::PlassiApp;

/// Pan/zoom state of the chart canvas.
pub(in crate::app) struct ChartView {
    pub(in crate::app) camera: crate::app::camera::Camera,
    /// Fit the whole room into view on the next frame.
    pub(in crate::app) fit_pending: bool,
    /// Person to centre on when the chart is next shown.
    pub(in crate::app) focus_person: Option<usize>,
}

impl Default for ChartView {
    fn default() -> Self {
        Self {
            camera: crate::app::camera::Camera::new(0.2, 3.0),
            fit_pending: true,
            focus_person: None,
        }
    }
}

/// State of the chart page.
#[derive(Default)]
pub(in crate::app) struct ChartState {
    pub(in crate::app) view: ChartView,
    pub(in crate::app) search: String,
    /// Highlighted row in the search matches.
    pub(in crate::app) search_pick: usize,
    /// Where the search matches were drawn last frame.
    pub(in crate::app) search_list: Option<Rect>,
    pub(in crate::app) hide_inspector: bool,
    /// Option whose differences are marked on the chart.
    pub(in crate::app) compare_with: Option<usize>,
    pub(in crate::app) dragged_person: Option<usize>,
    /// Person under the pointer last frame; their wish lines are drawn.
    pub(in crate::app) hovered_person: Option<usize>,
    /// Person chosen with "Swap with…", waiting for a second click.
    pub(in crate::app) swap_from: Option<usize>,
    /// Cached score preview for the current drag target.
    pub(in crate::app) drop_preview: Memo<(u64, usize, usize, String), Option<DropPreview>>,
    /// Score change from the last hand edit, shown briefly on the chart.
    pub(in crate::app) last_change: Option<(i64, web_time::Instant)>,
    /// Per-option (score, hard violations), keyed by project revision.
    pub(in crate::app) score_cache: Memo<u64, Vec<toolbar::OptionSummary>>,
    /// New name being typed for an option.
    pub(in crate::app) rename: String,
    /// The chart scene, keyed by (revision, option, compared option).
    scene: Memo<(u64, Option<usize>, Option<usize>), std::rc::Rc<Scene>>,
    /// Best swaps for the selected person, keyed by (revision, person, option).
    pub(in crate::app) suggestions: Memo<(u64, usize, usize), Vec<Suggestion>>,
    /// Group-move suggestions keyed by (revision, option, person).
    pub(in crate::app) group_moves: Memo<(u64, usize, usize), Vec<crate::groups::GroupMove>>,
    /// People picked with Ctrl/Shift-click, for a new preference.
    pub(in crate::app) multi: std::collections::BTreeSet<usize>,
    /// Preference under the pointer in the side panel; its members are outlined.
    pub(in crate::app) preference_preview: Option<u64>,
    /// Seats of the group move under the pointer, highlighted on the chart.
    pub(in crate::app) group_preview: Option<Vec<String>>,
}

use super::memo::Memo;
use super::theme::{self, Palette};

impl ChartState {
    /// Forgets people that no longer exist.
    pub(in crate::app) fn forget_people(&mut self, exists: &impl Fn(&usize) -> bool) {
        self.multi.retain(exists);
        if !self.swap_from.as_ref().is_none_or(exists) {
            self.swap_from = None;
        }
        if !self.dragged_person.as_ref().is_none_or(exists) {
            self.dragged_person = None;
        }
        self.hovered_person = None;
        self.drop_preview.clear();
    }
}

impl PlassiApp {
    pub(in crate::app) fn chart_page(&mut self, ui: &mut egui::Ui, p: Palette) {
        if !self.project.candidates.is_empty() {
            self.selected_candidate = self
                .selected_candidate
                .min(self.project.candidates.len() - 1);
        }
        let candidate = (!self.project.candidates.is_empty()).then_some(self.selected_candidate);
        self.chart.group_preview = None;
        let key = (self.file.revision, candidate, self.chart.compare_with);
        let evaluation = self.evaluation();
        let scene = std::rc::Rc::clone(self.chart.scene.get(key, || {
            std::rc::Rc::new(Scene::build(&self.project, candidate, self.chart.compare_with, evaluation))
        }));
        if let Some(id) = self.chart.view.focus_person.take() {
            self.selected_person = Some(id);
            self.focus_person(id, &scene);
        }

        // Where the tour points; recorded while drawing this frame.
        self.tour.anchors = Default::default();
        egui::Panel::top("chart-toolbar")
            .frame(
                theme::panel_frame(p).inner_margin(egui::Margin::symmetric(
                        theme::space::L as i8,
                        theme::space::M as i8,
                    )),
            )
            .show(ui, |ui| self.chart_toolbar(ui, p, &scene));
        let mut open = !self.chart.hide_inspector;
        let inspector = egui::Panel::right("chart-inspector")
            .resizable(true)
            .default_size(theme::PANEL_WIDTH + 2.0 * theme::space::M)
            .size_range(300.0..=540.0)
            .show_separator_line(false)
            .frame(theme::floating_panel_frame(p))
            .show_collapsible(ui, &mut open, |ui| {
                theme::panel_card(ui, p, theme::space::L, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.inspector(ui, p, &scene));
                });
            });
        self.tour.anchors.panel = inspector.map(|r| r.response.rect);
        self.chart.hide_inspector = !open;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.canvas))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                self.tour.anchors.canvas = Some(rect);
                self.canvas(ui, p, &scene);
                theme::panel_edge_shadows(ui, p, rect);
            });
        self.chart_tour(ui.ctx(), p);
    }

    /// Pans the canvas so the person's seat is centred on the next frame.
    fn focus_person(&mut self, person: usize, scene: &Scene) {
        if let Some(seat) = scene
            .seat_of
            .get(&person)
            .and_then(|id| scene.seats.iter().find(|s| &s.id == id))
        {
            self.chart.view.camera.focus(card_rect(seat).center());
        }
    }
}
