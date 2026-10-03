//! The steps from an empty project to a first chart, shown on the chart
//! until there is one.

use super::*;
use crate::app::Page;

struct Step {
    label: String,
    done: bool,
    /// Optional steps don't block solving.
    optional: bool,
    page: Page,
}

impl PlassiApp {
    fn setup_steps(&self) -> Vec<Step> {
        let people = self.project.attendees.len();
        let seats = self.project.layout.seat_count();
        let pending = self.project.notes.iter().filter(|n| !n.reviewed).count();
        let seats_label = match seats.cmp(&people) {
            _ if seats == 0 => tr("Set up tables").to_string(),
            std::cmp::Ordering::Less => {
                fill("Add seats: {} seats for {} people", &[&seats, &people])
            }
            _ => fill("{} seats for {} people", &[&seats, &people]),
        };
        let mut steps = vec![Step {
            label: if people == 0 {
                tr("Add people").into()
            } else {
                fill("{} people", &[&people])
            },
            done: people > 0,
            optional: false,
            page: Page::People,
        }];
        if people > 0 || !self.project.notes.is_empty() {
            steps.push(Step {
                label: match pending {
                    _ if self.project.notes.is_empty() => tr("No notes").into(),
                    0 => tr("Notes reviewed").into(),
                    n => fill("Review notes ({})", &[&n]),
                },
                done: pending == 0,
                optional: true,
                page: Page::People,
            });
        }
        steps.push(Step {
            label: seats_label,
            done: seats >= people && seats > 0,
            optional: false,
            page: Page::Room,
        });
        steps
    }

    /// A card in the middle of the canvas listing what is left before solving.
    pub(super) fn setup_card(&mut self, ui: &mut egui::Ui, p: Palette, rect: Rect) {
        let steps = self.setup_steps();
        let ready = steps.iter().all(|s| s.done || s.optional);
        let area = Rect::from_center_size(
            rect.center(),
            Vec2::new(380.0_f32.min(rect.width() - theme::space::XXL), 260.0),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                theme::section_title(ui, p, tr("Get started"));
                for step in &steps {
                    ui.horizontal(|ui| {
                        let color = match (step.done, step.optional) {
                            (true, _) => p.good,
                            (false, true) => p.warn,
                            (false, false) => p.muted,
                        };
                        theme::status_dot(ui, color);
                        let text = RichText::new(&step.label).color(if step.done {
                            p.muted
                        } else {
                            p.text
                        });
                        if ui
                            .add(egui::Label::new(text).sense(Sense::click()))
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            self.page = step.page;
                        }
                    });
                }
                ui.add_space(theme::space::M);
                ui.horizontal(|ui| {
                    let solve = ui.add_enabled(ready, theme::primary_button(tr("Solve"), p));
                    if solve
                        .on_disabled_hover_text(tr("Add people and enough seats first"))
                        .clicked()
                    {
                        self.start_solve(false);
                    }
                    if self.project.attendees.is_empty() && ui.button(tr("Try example")).clicked() {
                        self.load_example();
                    }
                });
            });
        });
    }
}
