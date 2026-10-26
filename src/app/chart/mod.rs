//! The seating chart page: a pan/zoom canvas of tables and people, a toolbar and an inspector panel.

mod canvas;
mod card;
mod chips;
mod colors;
mod connections;
mod drag;
mod edits;
mod inspector;
mod interact;
mod marquee;
mod moves;
mod nudge;
mod options;
mod person;
mod pins;
mod preferences;
mod presentation;
mod scene;
mod setup;
mod shortcuts;
mod table;
pub(in crate::app) mod toolbar;
mod tray;
mod tray_menu;

use canvas::*;
use card::*;
pub(in crate::app) use colors::COLOR_SETTING;
use colors::*;
use edits::*;
use presentation::*;
use scene::*;

use std::collections::{BTreeMap, HashMap, HashSet};

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
    Vec2,
};

use crate::catalog::Kind;
use crate::i18n::{fill, tr};
use crate::model::{Pin, Project, RuleKind, Seat, Side, Spot};
use crate::score::{self, Evaluation, Outcome, RuleResult};
use crate::seating::PolicyReport;

use super::PlassiApp;
use super::turned::{TableTop, Turned, paint_turned};

/// Pan/zoom state of the chart canvas.
pub(in crate::app) struct ChartView {
    pub(in crate::app) camera: crate::app::camera::Camera,
    /// Fit the whole room into view on the next frame.
    pub(in crate::app) fit_pending: bool,
    /// Previous canvas bounds, used to keep cards stationary when panels resize it.
    canvas_rect: Option<Rect>,
    screen_width: Option<f32>,
    /// Person to centre on when the chart is next shown.
    pub(in crate::app) focus_person: Option<usize>,
}

impl Default for ChartView {
    fn default() -> Self {
        Self {
            camera: crate::app::camera::Camera::new(0.2, 3.0),
            fit_pending: true,
            canvas_rect: None,
            screen_width: None,
            focus_person: None,
        }
    }
}

/// One full-width chart workspace on a phone.
#[derive(Default, PartialEq, Eq)]
pub(in crate::app) enum MobileChart {
    #[default]
    Chart,
    Details,
    Unseated,
}

/// State of the chart page.
#[derive(Default)]
pub(in crate::app) struct ChartState {
    pub(in crate::app) mobile_view: MobileChart,
    pub(in crate::app) floating_inspector: bool,
    pub(in crate::app) inspector_floats: bool,
    pub(in crate::app) single_pane: bool,
    pub(in crate::app) view: ChartView,
    pub(in crate::app) search: String,
    /// Text currently being edited in the zoom control.
    pub(in crate::app) zoom_entry: String,
    /// Highlighted row in the search matches.
    pub(in crate::app) search_pick: usize,
    /// Where the search matches were drawn last frame.
    pub(in crate::app) search_list: Option<Rect>,
    /// Keep text editing keys out of canvas actions, including its submit frame.
    text_input_frame: Option<u64>,
    pub(in crate::app) hide_inspector: bool,
    /// Option whose differences are marked on the chart.
    pub(in crate::app) compare_with: Option<usize>,
    pub(in crate::app) drag: Option<drag::Drag>,
    /// Where a box selection started, in chart units.
    pub(in crate::app) marquee: Option<Pos2>,
    /// Where the "Not seated" list was drawn last frame; dropping there unseats.
    pub(in crate::app) tray_rect: Option<Rect>,
    /// The person under the pointer in the "Not seated" list; the people in
    /// their wishes are outlined on the chart.
    pub(in crate::app) tray_hover: Option<usize>,
    /// Person to scroll the "Not seated" list to on the next frame.
    pub(in crate::app) tray_reveal: Option<usize>,
    /// Search in the "Not seated" list.
    pub(in crate::app) tray_search: String,
    /// Person under the pointer last frame; their wish lines are drawn.
    pub(in crate::app) hovered_person: Option<usize>,
    /// Person chosen with "Swap with…", waiting for a second click.
    pub(in crate::app) swap_from: Option<usize>,
    /// Cached score preview for the current drop, keyed by (revision, option, moves).
    pub(in crate::app) drop_preview: Memo<(u64, usize, drag::Targets), Option<DropPreview>>,
    /// Score change from the last hand edit, shown briefly on the chart.
    pub(in crate::app) last_change: Option<(i64, web_time::Instant)>,
    /// Per-option (score, hard violations), keyed by project revision.
    pub(in crate::app) score_cache: Memo<u64, Vec<chips::OptionSummary>>,
    /// New name being typed for an option.
    pub(in crate::app) rename: String,
    /// The chart scene, keyed by (revision, option, compared option).
    scene: Memo<(u64, Option<usize>, Option<usize>), std::rc::Rc<Scene>>,
    /// Best swaps for the selected person, keyed by (revision, person, option).
    pub(in crate::app) suggestions: Memo<(u64, usize, usize), Vec<Suggestion>>,
    /// Group-move suggestions keyed by (revision, option, person).
    pub(in crate::app) group_moves:
        Memo<(u64, usize, usize), std::rc::Rc<Vec<crate::groups::GroupMove>>>,
    /// Attendee text and catalog memberships, shared by options of this revision.
    presentation: Memo<u64, Presentation>,
    /// Wish components and membership, independent of the selected option.
    groups: Memo<u64, WishGroups>,
    /// People picked with Ctrl/Shift-click, for a new preference.
    pub(in crate::app) multi: std::collections::BTreeSet<usize>,
    /// Preference under the pointer in the side panel; its members are outlined.
    pub(in crate::app) preference_preview: Option<u64>,
    /// Seats of the group move or suggested seat under the pointer,
    /// highlighted on the chart.
    pub(in crate::app) group_preview: Option<Vec<String>>,
    /// Table whose summary the side panel shows.
    pub(in crate::app) selected_table: Option<String>,
    /// The category cards are coloured by; `None` keeps the usual look.
    pub(in crate::app) color_by: Option<Kind>,
    /// Categories of `color_by`, keyed by (revision, kind).
    coloring: Memo<(u64, Kind), Coloring>,
    /// The category key under the pointer in the colour legend; other cards dim.
    pub(in crate::app) color_hover: Option<String>,
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
        if self
            .drag
            .as_ref()
            .is_some_and(|d| !d.people.iter().all(exists))
        {
            self.drag = None;
        }
        self.hovered_person = None;
        self.drop_preview.clear();
    }
}

impl PlassiApp {
    pub(in crate::app) fn chart_page(&mut self, ui: &mut egui::Ui, p: Palette) {
        if ui.ctx().memory(|m| m.focused().is_some()) {
            self.chart.text_input_frame = Some(ui.ctx().cumulative_frame_nr());
        }
        if !self.project.candidates.is_empty() {
            self.selected_candidate = self
                .selected_candidate
                .min(self.project.candidates.len() - 1);
        }
        let candidate = (!self.project.candidates.is_empty()).then_some(self.selected_candidate);
        let project = &self.project;
        self.chart
            .presentation
            .get(self.file.revision, || Presentation::new(project));
        self.chart
            .groups
            .get(self.file.revision, || WishGroups::new(project));
        self.chart.group_preview = None;
        // However someone was selected (the list, search, the issue list,
        // another page), they replace the table, which must not come back
        // when they are deselected.
        if self.selected_person.is_some() {
            self.chart.selected_table = None;
        }
        let key = (self.file.revision, candidate, self.chart.compare_with);
        let scorer = self.scorer();
        let evaluation = self.evaluation();
        let scene = std::rc::Rc::clone(self.chart.scene.get(key, || {
            std::rc::Rc::new(Scene::build(
                &self.project,
                candidate,
                self.chart.compare_with,
                evaluation,
                &scorer,
            ))
        }));
        self.chart.color_hover = None;
        if let Some(kind) = self.chart.color_by {
            let project = &self.project;
            self.chart
                .coloring
                .get((self.file.revision, kind), || Coloring::new(project, kind));
        }
        if let Some(id) = self.chart.view.focus_person.take() {
            self.selected_person = Some(id);
            self.chart.mobile_view = MobileChart::Chart;
            self.focus_person(id, &scene);
        }

        // Where the tour points; recorded while drawing this frame.
        self.tour.anchors = Default::default();
        let workspace_width = ui.available_width();
        self.chart.inspector_floats = workspace_width < theme::DOCKED_INSPECTOR_MIN;
        self.chart.single_pane = workspace_width < theme::SPLIT_MIN;
        egui::Panel::top("chart-toolbar")
            .frame(theme::panel_frame(p).inner_margin(egui::Margin::symmetric(
                theme::space::L as i8,
                theme::space::M as i8,
            )))
            .show(ui, |ui| self.chart_toolbar(ui, p, &scene));
        if self.chart.single_pane {
            self.mobile_chart(ui, p, &scene);
            return;
        }
        if !self.chart.inspector_floats {
            let mut open = !self.chart.hide_inspector;
            let inspector = egui::Panel::right("chart-inspector")
                .resizable(true)
                .default_size(theme::PANEL_WIDTH + 2.0 * theme::space::M)
                .size_range(
                    300.0
                        ..=(workspace_width
                            - if scene.unseated.is_empty() {
                                320.0
                            } else {
                                560.0
                            })
                        .clamp(300.0, 540.0),
                )
                .show_separator_line(true)
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
        }
        self.tray(ui, p, &scene);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.canvas))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                self.tour.anchors.canvas = Some(rect);
                self.canvas(ui, p, &scene);
                theme::panel_edge_shadows(ui, p, rect);
            });
        if self.chart.inspector_floats && self.chart.floating_inspector {
            let mut open = true;
            egui::Window::new(tr("Details"))
                .id(egui::Id::new("floating-chart-inspector"))
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .default_width(320.0)
                .show(ui.ctx(), |ui| {
                    egui::ScrollArea::vertical()
                        .max_height((ui.ctx().content_rect().height() - 140.0).max(100.0))
                        .show(ui, |ui| self.inspector(ui, p, &scene));
                });
            self.chart.floating_inspector = open;
        }
        self.chart_tour(ui.ctx(), p);
    }

    fn mobile_chart(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene) {
        egui::Panel::top("chart-mobile-tabs")
            .frame(theme::panel_frame(p).inner_margin(8))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(
                        &mut self.chart.mobile_view,
                        MobileChart::Chart,
                        tr("Chart"),
                    );
                    ui.selectable_value(
                        &mut self.chart.mobile_view,
                        MobileChart::Details,
                        tr("Details"),
                    );
                    ui.selectable_value(
                        &mut self.chart.mobile_view,
                        MobileChart::Unseated,
                        fill("Not seated · {}", &[&scene.unseated.len()]),
                    );
                });
            });
        self.chart.tray_rect = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.canvas))
            .show(ui, |ui| match self.chart.mobile_view {
                MobileChart::Chart => {
                    let rect = ui.max_rect();
                    self.tour.anchors.canvas = Some(rect);
                    self.canvas(ui, p, scene);
                }
                MobileChart::Details => {
                    crate::app::shell::page_scroll(ui, theme::CONTENT_WIDTH, |ui| {
                        self.inspector(ui, p, scene)
                    });
                }
                MobileChart::Unseated => {
                    egui::Frame::new().inner_margin(16).show(ui, |ui| {
                        self.chart.tray_rect = Some(ui.max_rect());
                        self.tray_contents(ui, p, scene);
                    });
                }
            });
    }

    /// The chart's colouring by category, if one is chosen.
    fn coloring(&self) -> Option<&Coloring> {
        let kind = self.chart.color_by?;
        self.chart.coloring.peek(&(self.file.revision, kind))
    }

    fn card_metadata(&self, person: usize) -> Option<&CardMetadata> {
        self.chart
            .presentation
            .peek(&self.file.revision)?
            .people
            .get(&person)
    }

    /// Search results and canvas emphasis use the same name/affiliation match.
    fn chart_search_matches(&self, person: usize, needle: &crate::app::search::Needle) -> bool {
        match self.card_metadata(person) {
            Some(a) => {
                needle.matches_lowercase(&a.search_name)
                    || needle.matches_lowercase(&a.search_affiliation)
            }
            None => self.project.person(person).is_some_and(|a| {
                needle.matches(&a.name) || needle.matches(&self.project.affiliation(a))
            }),
        }
    }

    fn wish_group(&self, person: usize) -> Option<std::rc::Rc<Vec<usize>>> {
        let Some(groups) = self.chart.groups.peek(&self.file.revision) else {
            return crate::groups::group_of(&self.project, person).map(std::rc::Rc::new);
        };
        Some(std::rc::Rc::clone(
            &groups.groups[*groups.membership.get(&person)?],
        ))
    }

    /// Pans the canvas so the person's card is centred on the next frame,
    /// or scrolls the "Not seated" list to them.
    fn focus_person(&mut self, person: usize, scene: &Scene) {
        match scene.card_centre(person) {
            Some(centre) => self.chart.view.camera.focus(centre),
            None => self.chart.tray_reveal = Some(person),
        }
    }
}

#[cfg(test)]
impl PlassiApp {
    /// Where a person's card (on a seat or loose) was on screen in the last
    /// frame, for UI tests.
    pub(in crate::app) fn card_on_screen(&self, person: usize) -> Option<Rect> {
        let option = self.project.candidates.get(self.selected_candidate)?;
        match option.spot_of(person) {
            Spot::Seat(id) => self.seat_card_on_screen(&id),
            Spot::Loose(at) => {
                let card = Rect::from_center_size(Pos2::new(at.x, at.y), Vec2::new(CARD_W, CARD_H));
                self.world_on_screen(card)
            }
            Spot::Unseated => None,
        }
    }

    /// Where a seat's card was on screen in the last frame.
    pub(in crate::app) fn seat_card_on_screen(&self, seat_id: &str) -> Option<Rect> {
        let seats = self.project.layout.seats();
        self.world_on_screen(card_rect(seats.iter().find(|s| s.id == seat_id)?))
    }

    /// A point on a table's body, between its two middle seats, in the last frame.
    pub(in crate::app) fn table_on_screen(&self, table_id: &str) -> Option<Pos2> {
        let tables = self.project.layout.rows.iter().flat_map(|r| &r.tables);
        let table = tables.into_iter().find(|t| t.id == table_id)?;
        let row = table.left.min(table.right).saturating_sub(1) / 2;
        let left = self.seat_card_on_screen(&format!("{table_id}:left:{row}"))?;
        let right = self.seat_card_on_screen(&format!("{table_id}:right:{row}"))?;
        Some(left.center().lerp(right.center(), 0.5))
    }

    fn world_on_screen(&self, world: Rect) -> Option<Rect> {
        let canvas = self.tour.anchors.canvas?;
        Some(self.chart.view.camera.viewport(canvas).rect(world))
    }

    /// The chart canvas on screen in the last frame.
    pub(in crate::app) fn canvas_on_screen(&self) -> Option<Rect> {
        self.tour.anchors.canvas
    }
}
