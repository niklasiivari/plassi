//! The "Not seated" list beside the chart: everyone without a seat in the
//! option, as cards to drag onto seats or the chart, and "Seat the rest".

use super::*;

/// Above this many people the list gets a search field.
const SEARCH_FROM: usize = 8;

impl PlassiApp {
    /// The list panel, shown while anyone in the option has no seat.
    pub(super) fn tray(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene) {
        let shown = !scene.unseated.is_empty() && !self.project.candidates.is_empty();
        // The canvas changes width; fit the room again so no table ends up
        // out of view.
        if shown != self.chart.tray_rect.is_some() {
            self.chart.view.fit_pending = true;
        }
        if !shown {
            self.chart.tray_rect = None;
            self.chart.tray_anchor = None;
            return;
        }
        let panel = egui::Panel::left("chart-tray")
            .resizable(false)
            .exact_size(CARD_W + 2.0 * theme::space::XL)
            .frame(theme::floating_panel_frame(p))
            .show(ui, |ui| {
                theme::panel_card(ui, p, theme::space::L, |ui| {
                    self.tray_contents(ui, p, scene)
                });
            });
        self.chart.tray_rect = Some(panel.response.rect);
    }

    fn tray_contents(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene) {
        theme::section_title(ui, p, &fill("Not seated · {}", &[&scene.unseated.len()]));
        theme::hint(
            ui,
            p,
            tr(
                "Drag people onto seats or anywhere on the chart, or select one and click an empty seat.",
            ),
        );
        ui.add_space(theme::space::S);
        if ui
            .add_enabled(
                self.solving.is_none(),
                theme::primary_button(tr("Seat the rest"), p),
            )
            .on_hover_text(tr(
                "Seat everyone without a seat in a new option; people placed by hand stay",
            ))
            .clicked()
        {
            self.start_improve();
        }
        ui.add_space(theme::space::M);
        if scene.unseated.len() > SEARCH_FROM {
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

    /// One draggable card per person without a seat.
    fn tray_cards(&mut self, ui: &mut egui::Ui, p: Palette, scene: &Scene) {
        let needle = crate::app::search::Needle::new(&self.chart.tray_search);
        let (mut hovered, mut selected) = (None, None);
        for &person in &scene.unseated {
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
                loose: scene.loose.contains_key(&person),
                ..Default::default()
            };
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &attendee.name)
            });
            self.paint_card(ui.painter(), p, rect, Some(person), look);
            if self.chart.drag.is_none() {
                let response = response.clone();
                response.on_hover_ui(|ui| self.hover_card(ui, p, scene, person, None));
            }
            let anchor = Some((person, rect.right_center()));
            if response.hovered() {
                hovered = anchor;
            } else if self.selected_person == Some(person) && ui.is_rect_visible(rect) {
                selected = anchor;
            }
            self.tray_card_input(ui, scene, &response, person);
            ui.add_space(theme::space::XS);
        }
        self.chart.tray_anchor = hovered.or(selected);
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
        } else if response.clicked() && ui.input(|i| i.modifiers.command || i.modifiers.shift) {
            self.toggle_pick(person);
        } else if response.clicked() {
            self.selected_person = Some(person);
            self.focus_person(person, scene);
        }
    }
}
