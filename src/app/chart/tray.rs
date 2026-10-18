//! The "Not seated" list beside the chart: everyone without a seat who is not
//! on the chart either, as cards to drag onto seats or the chart, and "Seat
//! the rest". People left on the chart show there only, never in both places.

use super::tray_menu::TrayAction;
use super::*;

/// Above this many people the list gets a search field.
const SEARCH_FROM: usize = 8;

impl PlassiApp {
    /// The list panel, shown while anyone in the option has no seat.
    pub(super) fn tray(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene) {
        let mut shown = !scene.unseated.is_empty() && !self.project.candidates.is_empty();
        if !shown {
            self.chart.tray_hover = None;
            self.chart.tray_reveal = None;
        }
        let panel = egui::Panel::left("chart-tray")
            .resizable(false)
            .exact_size(CARD_W + 2.0 * theme::space::XL)
            .frame(theme::floating_panel_frame(p))
            .show_collapsible(ui, &mut shown, |ui| {
                if !scene.unseated.is_empty() {
                    theme::panel_card(ui, p, theme::space::L, |ui| {
                        self.tray_contents(ui, p, scene)
                    });
                }
            });
        self.chart.tray_rect = panel.filter(|_| shown).map(|panel| panel.response.rect);
    }

    fn tray_contents(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene) {
        theme::section_title(ui, p, &fill("Not seated · {}", &[&scene.unseated.len()]));
        let on_chart = scene.unseated.len() - scene.listed.len();
        if on_chart > 0 {
            theme::hint(ui, p, fill("{} of them on the chart", &[&on_chart]));
        }
        theme::hint(
            ui,
            p,
            tr(
                "Drag people onto seats or anywhere on the chart, or select one and click an empty seat.",
            ),
        );
        ui.add_space(theme::space::S);
        let enough = self.project.layout.seat_count() >= self.project.attendees.len();
        if ui
            .add_enabled(
                self.solving.is_none() && enough && !self.file.external_invalid,
                theme::primary_button(tr("Seat the rest"), p),
            )
            .on_hover_text(fill(
                "Seat everyone without a seat in a new option; {} people placed by hand or pinned stay",
                &[&self.kept_by_improve()],
            ))
            .on_disabled_hover_text(match () {
                _ if !enough => tr("Add enough seats first"),
                _ if self.solving.is_some() => tr("Wait for the search to finish"),
                _ => tr("Invalid project file"),
            })
            .clicked()
        {
            self.start_improve();
        }
        ui.add_space(theme::space::M);
        if scene.listed.len() > SEARCH_FROM {
            ui.add(
                theme::field(&mut self.chart.tray_search)
                    .hint_text(tr("Search"))
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(theme::space::S);
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| self.tray_cards(ui, p, scene));
    }

    /// One draggable card per person in the list.
    fn tray_cards(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene) {
        let reveal = self.chart.tray_reveal.take();
        if reveal.is_some_and(|person| !self.matches_tray_search(person)) {
            self.chart.tray_search.clear();
        }
        let needle = crate::app::search::Needle::new(&self.chart.tray_search);
        let mut hovered = None;
        let mut action = None;
        for &person in &scene.listed {
            let Some(attendee) = self.project.person(person) else {
                continue;
            };
            if !needle.matches(&attendee.name) {
                continue;
            }
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), CARD_H),
                Sense::click_and_drag(),
            );
            let dragged = self
                .chart
                .drag
                .as_ref()
                .is_some_and(|d| d.people.contains(&person));
            let look = CardLook {
                status: scene.status(person),
                selected: self.selected_person == Some(person)
                    || self.chart.multi.contains(&person),
                hovered: response.hovered(),
                dimmed: dragged,
                pin: scene.pin_mark(person),
                ..Default::default()
            };
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &attendee.name)
            });
            self.paint_card(ui.painter(), p, rect, Some(person), look);
            if self.chart.drag.is_none() {
                let response = response.clone();
                response.on_hover_ui(|ui| {
                    let footer = tr("Drag onto a seat or the chart · right-click for actions");
                    self.hover_card(ui, p, scene, person, None, footer)
                });
            }
            if reveal == Some(person) {
                ui.scroll_to_rect(rect, None);
            }
            if response.hovered() {
                hovered = Some(person);
            }
            egui::Popup::context_menu(&response)
                .style(theme::menu_style)
                .show(|ui| action = self.tray_menu(ui, scene, person).or(action.take()));
            self.tray_card_input(ui, scene, &response, person);
            ui.add_space(theme::space::XS);
        }
        self.chart.tray_hover = hovered;
        if let Some(action) = action {
            self.apply_tray_action(action, scene);
        }
    }

    fn matches_tray_search(&self, person: usize) -> bool {
        let needle = crate::app::search::Needle::new(&self.chart.tray_search);
        self.project
            .person(person)
            .is_some_and(|a| needle.matches(&a.name))
    }

    fn tray_card_input(
        &mut self,
        ui: &egui::Ui,
        scene: &Scene,
        response: &egui::Response,
        person: usize,
    ) {
        if response.drag_started_by(egui::PointerButton::Primary) {
            self.start_drag(person, Vec2::ZERO);
        } else if response.double_clicked() {
            self.apply_tray_action(TrayAction::SeatBest(vec![person]), scene);
        } else if response.clicked() && ui.input(|i| i.modifiers.command || i.modifiers.shift) {
            self.toggle_pick(person);
        } else if response.clicked() {
            self.selected_person = Some(person);
            self.focus_person(person, scene);
        }
    }
}
