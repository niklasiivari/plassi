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
            Some(tr(
                "Rules for everyone, mostly for people without wishes of their own. They are weighed against wishes when you solve.",
            )),
        );
        let before = self.project.policy.clone();
        let policy = &mut self.project.policy;
        let mut settled = true;

        section(
            ui,
            p,
            tr("Organizations"),
            tr("Who unconstrained people sit next to (beside or opposite)."),
            |ui| {
                ui.horizontal(|ui| {
                    for (value, label) in [
                        (
                            OrganizationMixing::SameOrganization,
                            tr("With their own organization"),
                        ),
                        (OrganizationMixing::Mixed, tr("Mix organizations")),
                        (OrganizationMixing::Indifferent, tr("Don't care")),
                    ] {
                        ui.selectable_value(&mut policy.organization, value, label);
                    }
                });
                if policy.organization != OrganizationMixing::Indifferent {
                    ui.horizontal(|ui| {
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
            tr("Wish groups and unconstrained people"),
            tr(
                "Should people without wishes be sprinkled among the wish groups, or kept to themselves?",
            ),
            |ui| {
                ui.horizontal(|ui| {
                    theme::hint(ui, p, tr("Keep separate"));
                    let r = theme::slider(
                        ui,
                        egui::Slider::new(&mut policy.sprinkle, -50..=50).show_value(false),
                    );
                    settled &= !r.dragged();
                    theme::hint(ui, p, tr("Sprinkle in"));
                });
                let text = match policy.sprinkle {
                    0 => tr("Neutral").to_string(),
                    s if s > 0 => fill(
                        "Sprinkle in: each pair of adjacent unconstrained people costs {}",
                        &[&s],
                    ),
                    s => fill(
                        "Keep separate: each unconstrained person next to a wish group costs {}",
                        &[&-s],
                    ),
                };
                theme::hint(ui, p, text);
            },
        );

        section(
            ui,
            p,
            tr("Nobody sits alone"),
            tr("Cost for anyone with no one beside or opposite them."),
            |ui| {
                ui.horizontal(|ui| {
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
            tr("Nobody loses every wish"),
            tr(
                "Extra cost for anyone none of whose wishes is near them (same table, two rows or closer).",
            ),
            |ui| {
                ui.horizontal(|ui| {
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
            tr("Empty seats"),
            tr("Where spare seats go when there are more seats than people."),
            |ui| {
                ui.horizontal(|ui| {
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
        ui.horizontal(|ui| {
            if ui.button(tr("Reset to defaults")).clicked() {
                *policy = SeatingPolicy::default();
            }
            let scale = fill(
                "For reference: an ordinary wish seated at another table costs {}; an avec costs {}.",
                &[
                    &crate::model::ProximityCosts::far_penalty(false),
                    &crate::model::ProximityCosts::far_penalty(true),
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
            self.commit(tr("Changed the seating policy; solve again to apply it"));
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
