//! The desktop app: state shared by all pages, the frame loop and page dispatch.

// Pages.
mod categories;
mod chart;
mod graph;
mod people;
mod policy;
mod printing;
mod room;

// App infrastructure.
mod about;
mod backups;
mod camera;
mod copy_reminder;
mod help;
mod input;
mod memo;
mod project_file;
mod projects;
mod room_labels;
mod scoring_cache;
mod search;
mod shell;
mod solving;
mod theme;
mod tour;
mod turned;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod ui_tests;
#[cfg(test)]
pub(crate) use theme::initials as theme_initials;

use shell::*;

use web_time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText};

use theme::Palette;

use crate::model::Project;
use crate::{io, platform, score, seating};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Page {
    Chart,
    People,
    Graph,
    Categories,
    Policy,
    Room,
    Print,
}

impl Page {
    /// A page by its short name, e.g. "people".
    fn named(name: &str) -> Option<Self> {
        Some(match name {
            "chart" => Page::Chart,
            "people" => Page::People,
            "graph" => Page::Graph,
            "categories" => Page::Categories,
            "policy" => Page::Policy,
            "room" => Page::Room,
            "print" => Page::Print,
            _ => return None,
        })
    }
}

/// Everything the desktop app keeps between frames.
pub(super) struct PlassiApp {
    project: Project,
    file: project_file::ProjectFile,
    page: Page,
    status: String,
    status_at: Instant,
    sidebar_compact: bool,
    tablet_sidebar_expanded: bool,
    /// The option shown on the chart and printed.
    selected_candidate: usize,
    /// Shared between the chart and the People page.
    selected_person: Option<usize>,
    /// The search in progress, if any.
    solving: Option<solving::SolveJob>,
    /// Search time for a solve or a longer search, and for an improvement.
    solve_time: Duration,
    improve_time: Duration,
    chart: chart::ChartState,
    graph: graph::GraphState,
    categories: categories::CategoriesState,
    people: people::PeopleState,
    room: room::RoomState,
    printing: printing::PrintState,
    projects: projects::ProjectsState,
    tour: tour::TourState,
    backups: backups::BackupsState,
    #[cfg(target_arch = "wasm32")]
    copy_reminder: copy_reminder::CopyReminder,
    scoring: scoring_cache::ScoringCache,
}

impl PlassiApp {
    fn scorer(&mut self) -> std::rc::Rc<seating::Scorer> {
        self.scoring.scorer(&self.project, self.file.revision)
    }

    /// Each option retains its evaluation until scoring inputs or its assignment change.
    fn evaluation(&mut self) -> Option<std::rc::Rc<score::Evaluation>> {
        self.scoring
            .evaluation(&self.project, self.file.revision, self.selected_candidate)
    }
}

impl eframe::App for PlassiApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        platform::set_repaint(ui.ctx());
        let previous_status = self.status.clone();
        // Edits in this frame are undone back to the option shown now.
        self.file.shown_option = (self.selected_candidate < self.project.candidates.len())
            .then_some(self.selected_candidate);
        self.sync_external();
        self.poll_solver();
        self.poll_print();
        self.remember_option();
        self.poll_upload();
        let p = theme::palette(ui.ctx());
        let typing = ui.ctx().memory(|m| m.focused().is_some());
        if !typing {
            let (undo, redo) = ui.ctx().input_mut(|i| {
                let redo = i.consume_shortcut(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::Z,
                )) || i.consume_shortcut(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Y,
                ));
                let undo = i.consume_shortcut(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Z,
                ));
                (undo, redo)
            });
            if undo {
                self.undo(false);
            }
            if redo {
                self.undo(true);
            }
        }
        let mobile = theme::mobile(ui.ctx());
        if mobile {
            ui.spacing_mut().interact_size.y = 44.0;
            ui.spacing_mut().button_padding.y = 10.0;
            ui.spacing_mut().slider_width = (ui.available_width() - 130.0).clamp(100.0, 280.0);
            self.mobile_header(ui, p);
        } else {
            let frame = theme::floating_panel_frame(p);
            let auto_compact = ui.ctx().content_rect().width() < theme::SIDEBAR_EXPANDED_MIN;
            let expanded = if auto_compact {
                self.tablet_sidebar_expanded
            } else {
                !self.sidebar_compact
            };
            let mut toggle_sidebar = false;
            let openness = ui.ctx().animate_bool_with_time_and_easing(
                egui::Id::new("sidebar-width"),
                expanded,
                0.24,
                egui::emath::easing::cubic_in_out,
            );
            egui::Panel::left("sidebar")
                .resizable(false)
                .exact_size(egui::lerp(84.0..=244.0, openness))
                .show_separator_line(true)
                .frame(frame)
                .show(ui, |ui| {
                    theme::panel_card(ui, p, theme::space::S, |ui| {
                        toggle_sidebar = self.sidebar(ui, p, expanded, openness);
                    });
                });
            if toggle_sidebar {
                if auto_compact {
                    self.tablet_sidebar_expanded = !expanded;
                } else {
                    self.sidebar_compact = expanded;
                }
            }
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.bg))
            .show(ui, |ui| match self.page {
                Page::Chart => self.chart_page(ui, p),
                Page::People => self.people_page(ui, p),
                Page::Policy => page_scroll(ui, theme::CONTENT_WIDTH, |ui| self.policy_page(ui, p)),
                Page::Room if self.project.layout.free_layout && !self.file.external_invalid => {
                    self.free_room_page(ui, p)
                }
                Page::Room => page_scroll(ui, theme::WIDE_WIDTH, |ui| self.room_page(ui, p)),
                Page::Print => page_scroll(ui, theme::CONTENT_WIDTH, |ui| self.print_page(ui, p)),
                Page::Graph => self.graph_page(ui, p),
                Page::Categories => {
                    page_scroll(ui, theme::WIDE_WIDTH, |ui| self.categories_page(ui, p))
                }
            });
        if self.status != previous_status {
            self.status_at = Instant::now();
        }
        about::show(ui.ctx(), p);
        self.backups_window(ui.ctx(), p);
        self.toast(ui.ctx(), p);
        self.compress_history(ui.ctx());
        if self.solving.is_some() || self.printing.job.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        } else if !platform::external_notifications_available()
            || !self.file.store.notifications_available()
        {
            ui.ctx().request_repaint_after(Duration::from_millis(350));
        }
    }
}

/// Runs the desktop app.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn run() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 560.0])
            .with_app_id("plassi"),
        ..Default::default()
    };
    eframe::run_native(
        "Plassi",
        options,
        Box::new(|cc| {
            theme::install(&cc.egui_ctx);
            let mut app = PlassiApp::new();
            // Optional start page and selection, e.g. for screenshots.
            if let Some(page) = std::env::var("PLASSI_PAGE")
                .ok()
                .and_then(|p| Page::named(&p))
            {
                app.page = page;
            }
            app.graph
                .set_by_table(std::env::var("PLASSI_GRAPH").as_deref() == Ok("tables"));
            if std::env::var("PLASSI_THEME").as_deref() == Ok("light") {
                cc.egui_ctx.set_theme(egui::Theme::Light);
            }
            app.selected_person = std::env::var("PLASSI_SELECT")
                .ok()
                .and_then(|v| v.parse().ok());
            Ok(Box::new(app))
        }),
    )
    .map_err(|error| anyhow::anyhow!("desktop UI failed: {error}"))
}

/// Starts the app in the page's `<canvas id="plassi">`.
#[cfg(target_arch = "wasm32")]
pub fn start_web() {
    use wasm_bindgen::JsCast;
    eframe::WebLogger::init(log::LevelFilter::Warn).ok();
    wasm_bindgen_futures::spawn_local(async {
        let canvas = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("plassi"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("a <canvas id=\"plassi\"> on the page");
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| {
                    theme::install(&cc.egui_ctx);
                    // Browsers choose their own chrome theme; eframe's web
                    // backend cannot apply egui's SetTheme viewport command.
                    cc.egui_ctx
                        .options_mut(|options| options.sync_window_theme = false);
                    platform::request_persistence();
                    let mut app = PlassiApp::new();
                    // A start page from the address, e.g. ".../#people".
                    let hash = web_sys::window()
                        .and_then(|w| w.location().hash().ok())
                        .unwrap_or_default();
                    if let Some(page) = Page::named(hash.trim_start_matches('#')) {
                        app.page = page;
                    }
                    Ok(Box::new(app))
                }),
            )
            .await;
        let document = web_sys::window().and_then(|w| w.document());
        match started {
            // The page's loading screen gives way to the app.
            Ok(()) => {
                if let Some(loading) = document.and_then(|d| d.get_element_by_id("loading")) {
                    loading.remove();
                }
            }
            Err(error) => {
                web_sys::console::error_1(&format!("Plassi failed to start: {error:?}").into());
                if let Some(body) = document.and_then(|d| d.body()) {
                    let _ = body.class_list().add_1("failed");
                }
            }
        }
    });
}

impl PlassiApp {
    pub(super) fn new() -> Self {
        Self::with_library(platform::Library::open())
    }

    /// The app on the projects in `library`.
    pub(super) fn with_library(library: platform::Library) -> Self {
        // Startup project names and status messages use the selected language too.
        let saved = library.setting("language");
        let lang = saved
            .or_else(platform::system_locale)
            .and_then(|l| crate::i18n::Lang::from_locale(&l));
        crate::i18n::set(lang.unwrap_or(crate::i18n::Lang::En));
        let (file, project, status) = project_file::ProjectFile::open_default(library);
        let option = project_file::saved_option(&project);
        let mut chart = chart::ChartState::default();
        chart.color_by = file
            .library
            .setting(chart::COLOR_SETTING)
            .and_then(|key| crate::catalog::Kind::from_key(&key));
        Self {
            project,
            file,
            page: Page::Chart,
            status,
            status_at: Instant::now(),
            sidebar_compact: false,
            tablet_sidebar_expanded: false,
            selected_candidate: option,
            selected_person: None,
            solving: None,
            solve_time: seating::SOLVE_TIME,
            improve_time: seating::IMPROVE_TIME,
            chart,
            graph: Default::default(),
            categories: Default::default(),
            people: Default::default(),
            room: Default::default(),
            printing: Default::default(),
            scoring: Default::default(),
            projects: Default::default(),
            tour: Default::default(),
            backups: Default::default(),
            #[cfg(target_arch = "wasm32")]
            copy_reminder: Default::default(),
        }
    }
}
