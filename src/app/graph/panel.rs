//! The side panel of the Connections page: settings, structure, legend and selection.

use crate::i18n::{fill, tr};
use eframe::egui::{self, RichText, Stroke, Vec2};

use crate::app::theme::{self, Palette};
use crate::app::{Page, PlassiApp};
use crate::score::Outcome;

impl PlassiApp {
    pub(super) fn graph_panel(&mut self, ui: &mut egui::Ui, p: Palette) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(tr("Connections"))
                    .size(theme::text::TITLE)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                crate::app::help::help_button(ui, p, crate::app::help::HelpPage::Graph);
            });
        });
        ui.label(
            RichText::new(tr(
                "Who wished to sit with whom. Drag people around; click to select.",
            ))
            .size(theme::text::SMALL)
            .color(p.muted),
        );
        ui.add_space(theme::space::L);
        theme::text_rows(ui, |ui| self.graph_settings(ui));
        ui.add_space(theme::space::XL);
        self.graph_structure(ui, p);
        legend(ui, p);
        self.graph_selection(ui, p);
    }

    fn graph_settings(&mut self, ui: &mut egui::Ui) {
        let state = &mut self.graph;
        ui.checkbox(
            &mut state.show_everyone,
            tr("Include people without wishes"),
        );
        ui.checkbox(&mut state.by_table, tr("Arrange by table")).on_hover_text(
            tr("Pull everyone towards the table they sit at in the current option, to see which groups were split."),
        );
        ui.checkbox(&mut state.labels, tr("Show names"));
    }

    /// Counts of people, wishes and groups, and how many wishes are met.
    fn graph_structure(&self, ui: &mut egui::Ui, p: Palette) {
        let Some(graph) = &self.graph.graph else {
            return;
        };
        theme::section_title(ui, p, tr("Structure"));
        let mut lines = vec![
            fill(
                "{} wishes between {} people",
                &[&graph.edges.len(), &graph.people.len()],
            ),
            fill(
                "{} wish groups, the largest has {}",
                &[&graph.groups, &graph.largest_group],
            ),
        ];
        if !self.graph.show_everyone {
            let hidden = self
                .project
                .attendees
                .len()
                .saturating_sub(graph.people.len());
            lines.push(fill("{} people without wishes hidden", &[&hidden]));
        }
        if graph.edges.iter().any(|e| e.outcome.is_some()) {
            let met = graph
                .edges
                .iter()
                .filter(|e| e.outcome == Some(Outcome::Met))
                .count();
            lines.push(fill(
                "{} of {} wishes met in this option",
                &[&met, &graph.edges.len()],
            ));
        }
        for line in lines {
            ui.label(RichText::new(line).size(theme::text::SMALL));
        }
        ui.add_space(theme::space::XL);
    }

    fn graph_selection(&mut self, ui: &mut egui::Ui, p: Palette) {
        let Some(id) = self.selected_person else {
            return;
        };
        let Some(person) = self.project.person(id) else {
            return;
        };
        ui.add_space(theme::space::XL);
        theme::section_title(ui, p, tr("Selected"));
        ui.label(RichText::new(&person.name).strong().size(theme::text::LEAD));
        ui.label(RichText::new(self.project.affiliation(person)).color(p.muted));
        ui.horizontal(|ui| {
            if ui
                .add(theme::secondary_button(tr("Show on chart")))
                .clicked()
            {
                self.page = Page::Chart;
                self.chart.view.focus_person = Some(id);
            }
            if ui.add(theme::secondary_button(tr("Edit wishes"))).clicked() {
                self.page = Page::People;
            }
        });
    }
}

fn legend(ui: &mut egui::Ui, p: Palette) {
    theme::section_title(ui, p, tr("Legend"));
    theme::text_rows(ui, |ui| {
        for (color, text, width) in [
            (
                theme::outcome_color(p, Some(Outcome::Met)),
                tr("Wish met"),
                2.0,
            ),
            (
                theme::outcome_color(p, Some(Outcome::Partial)),
                tr("Wish partly met"),
                2.0,
            ),
            (
                theme::outcome_color(p, Some(Outcome::Broken)),
                tr("Hard rule broken"),
                2.0,
            ),
            (
                theme::outcome_color(p, None),
                tr("Not judged (no option yet)"),
                2.0,
            ),
            (p.text, tr("Avec (thicker)"), 3.5),
        ] {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(26.0, 12.0), egui::Sense::hover());
                ui.painter().line_segment(
                    [rect.left_center(), rect.right_center()],
                    Stroke::new(width, color),
                );
                theme::hint(ui, p, text);
            });
        }
    });
    ui.label(
        RichText::new(tr("Arrows point from the person who wished. A line without arrows is mutual. Dot colour is the organization; larger dots have more links."))
            .size(theme::text::CAPTION)
            .color(p.muted),
    );
}
