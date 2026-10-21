use crate::i18n::{fill, tr};
use eframe::egui::{self, RichText};

use crate::model::{EmptySeatPlacement, OrganizationMixing, SeatingPolicy};

use super::PlassiApp;
use super::theme::{self, Palette};

impl PlassiApp {
    pub(super) fn policy_page(&mut self, ui: &mut egui::Ui, p: Palette) {
        crate::app::help::page_header(
            ui,
            p,
            crate::app::help::HelpPage::Policy,
            tr("Seating policy"),
            Some(tr("Rules for the whole event.")),
        );
        let before = self.project.policy.clone();
        let policy = &mut self.project.policy;
        let mut settled = true;

        section(
            ui,
            p,
            tr("Organizations"),
            tr("Neighbours for people without wishes."),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (value, label) in [
                        (
                            OrganizationMixing::SameOrganization,
                            tr("Same organization"),
                        ),
                        (OrganizationMixing::Mixed, tr("Mix organizations")),
                        (OrganizationMixing::Indifferent, tr("No preference")),
                    ] {
                        ui.selectable_value(&mut policy.organization, value, label);
                    }
                });
                if policy.organization != OrganizationMixing::Indifferent {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(tr("Strength"));
                        let r = theme::slider(
                            ui,
                            egui::Slider::new(&mut policy.organization_weight, 0..=100),
                        );
                        settled &= !r.dragged();
                    });
                }
            },
        );

        section(
            ui,
            p,
            tr("People without wishes"),
            tr("Seat them among wish groups or separately."),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    theme::hint(ui, p, tr("Keep separate"));
                    let r = theme::slider(
                        ui,
                        egui::Slider::new(&mut policy.sprinkle, -50..=50).show_value(false),
                    );
                    settled &= !r.dragged();
                    theme::hint(ui, p, tr("Mix in"));
                });
                let text = match policy.sprinkle {
                    0 => tr("Neutral").to_string(),
                    s if s > 0 => fill("Adjacent people without wishes: {} points", &[&s]),
                    s => fill("Beside a wish group: {} points", &[&-s]),
                };
                theme::hint(ui, p, text);
            },
        );

        section(
            ui,
            p,
            tr("Avoid sitting alone"),
            tr("Penalty for having nobody beside or opposite."),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    let r = theme::slider(
                        ui,
                        egui::Slider::new(&mut policy.isolation_penalty, 0..=2000),
                    );
                    settled &= !r.dragged();
                });
            },
        );

        section(
            ui,
            p,
            tr("At least one wish"),
            tr("Penalty if no wish is beside, opposite or diagonal."),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    let r = theme::slider(
                        ui,
                        egui::Slider::new(&mut policy.left_out_penalty, 0..=2000),
                    );
                    settled &= !r.dragged();
                });
            },
        );

        section(
            ui,
            p,
            tr("Table ends"),
            tr("Penalty for someone without wishes at a table end; groups take the ends instead."),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    let r = theme::slider(
                        ui,
                        egui::Slider::new(&mut policy.lone_end_penalty, 0..=2000),
                    );
                    settled &= !r.dragged();
                });
            },
        );

        section(
            ui,
            p,
            tr("Empty seats"),
            tr("Where spare seats go."),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(
                        &mut policy.empty_seats,
                        EmptySeatPlacement::TableBottoms,
                        tr("At table bottoms, in opposite pairs"),
                    );
                    ui.selectable_value(
                        &mut policy.empty_seats,
                        EmptySeatPlacement::Anywhere,
                        tr("Anywhere"),
                    );
                });
            },
        );

        ui.add_space(theme::space::S);
        theme::responsive_row(ui, |ui| {
            if ui.button(tr("Reset to defaults")).clicked() {
                *policy = SeatingPolicy::default();
            }
            let scale = fill(
                "Another table: wish {} points · avec {} points",
                &[
                    &crate::model::WishKind::Seating.far_penalty(),
                    &crate::model::WishKind::Avec.far_penalty(),
                ],
            );
            ui.label(RichText::new(scale).size(theme::text::SMALL).color(p.muted));
        });
        // Keep slider drags local until release, then save once.
        let changed = self.project.policy != before;
        if !settled {
            self.policy_dirty |= changed;
        } else if changed || std::mem::take(&mut self.policy_dirty) {
            self.policy_dirty = false;
            self.commit(tr("Policy saved; solve again to apply"));
        }
    }
}

fn section(
    ui: &mut egui::Ui,
    p: Palette,
    title: &str,
    help: &str,
    add: impl FnOnce(&mut egui::Ui),
) {
    theme::card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).strong().size(theme::text::LEAD));
        theme::hint(ui, p, help);
        ui.add_space(theme::space::S);
        add(ui);
    });
    ui.add_space(theme::space::S);
}
