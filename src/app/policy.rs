use crate::i18n::tr;
use eframe::egui::{self, RichText};

use crate::model::{
    EmptySeatPlacement, OneWish, OrganizationMixing, SeatingPolicy, SittingAlone, TableEnds,
    WishPriority, WithoutWishes,
};

use super::PlassiApp;
use super::theme::{self, Palette};

impl PlassiApp {
    pub(super) fn policy_page(&mut self, ui: &mut egui::Ui, p: Palette) {
        crate::app::help::page_header(
            ui,
            p,
            crate::app::help::HelpPage::Policy,
            tr("Seating policy"),
            None,
        );
        let before = self.project.policy.clone();
        let policy = &mut self.project.policy;
        choice(
            ui,
            p,
            tr("Wish priority"),
            &mut policy.wish_priority,
            &[
                (
                    WishPriority::Balanced,
                    tr("Balanced"),
                    tr("Keep the usual wish weights."),
                ),
                (
                    WishPriority::People,
                    tr("Person wishes first"),
                    tr("Give person and Avec wishes three times their usual weight."),
                ),
                (
                    WishPriority::Organizations,
                    tr("Organization wishes first"),
                    tr("Give organization wishes three times their usual weight."),
                ),
            ],
        );
        choice(
            ui,
            p,
            tr("Organizations"),
            &mut policy.organization,
            &[
                (
                    OrganizationMixing::SameOrganization,
                    tr("Same organization"),
                    tr("Prefer seating people without wishes with their own organization."),
                ),
                (
                    OrganizationMixing::Mixed,
                    tr("Mix organizations"),
                    tr("Prefer seating people without wishes with other organizations."),
                ),
                (
                    OrganizationMixing::Indifferent,
                    tr("Doesn't matter"),
                    tr("No organization preference for people without wishes."),
                ),
            ],
        );
        choice(
            ui,
            p,
            tr("People without wishes"),
            &mut policy.without_wishes,
            &[
                (
                    WithoutWishes::AmongGroups,
                    tr("Among wish groups"),
                    tr("Prefer spreading them among people with wishes."),
                ),
                (
                    WithoutWishes::ApartFromGroups,
                    tr("Apart from wish groups"),
                    tr("Prefer seating them together, away from wish groups."),
                ),
                (
                    WithoutWishes::Indifferent,
                    tr("Doesn't matter"),
                    tr("They fill the seats that are left."),
                ),
            ],
        );
        choice(
            ui,
            p,
            tr("Sitting alone"),
            &mut policy.sitting_alone,
            &[
                (
                    SittingAlone::Never,
                    tr("Strongly avoid"),
                    tr("Give high priority to having someone beside or opposite each person."),
                ),
                (
                    SittingAlone::Avoid,
                    tr("Avoid"),
                    tr("Prefer having someone beside or opposite each person."),
                ),
                (
                    SittingAlone::Indifferent,
                    tr("Doesn't matter"),
                    tr("Empty seats may leave someone without a neighbour."),
                ),
            ],
        );
        choice(
            ui,
            p,
            tr("At least one wish nearby"),
            &mut policy.one_wish,
            &[
                (
                    OneWish::Important,
                    tr("Important"),
                    tr(
                        "Give high priority to one nearby wish per person, even at a cost to others.",
                    ),
                ),
                (
                    OneWish::Preferred,
                    tr("Preferred"),
                    tr("Prefer one nearby wish per person when it costs little elsewhere."),
                ),
                (
                    OneWish::Indifferent,
                    tr("Doesn't matter"),
                    tr("Only the wishes themselves count."),
                ),
            ],
        );
        choice(
            ui,
            p,
            tr("Table ends"),
            &mut policy.table_ends,
            &[
                (
                    TableEnds::GroupsOnly,
                    tr("Prefer groups"),
                    tr("Prefer keeping people without wishes away from table ends."),
                ),
                (
                    TableEnds::Indifferent,
                    tr("Doesn't matter"),
                    tr("Anyone may sit at an end."),
                ),
            ],
        );
        choice(
            ui,
            p,
            tr("Empty seats"),
            &mut policy.empty_seats,
            &[
                (
                    EmptySeatPlacement::TableBottoms,
                    tr("At table bottoms"),
                    tr("Prefer opposite pairs at the bottom of tables."),
                ),
                (
                    EmptySeatPlacement::Anywhere,
                    tr("Anywhere"),
                    tr("Any seat may stay empty."),
                ),
            ],
        );
        ui.add_space(theme::space::S);
        if ui.button(tr("Reset to defaults")).clicked() {
            *policy = SeatingPolicy::default();
        }
        if self.project.policy != before {
            self.commit(tr("Policy saved; solve again to apply"));
        }
    }
}

/// A setting as a card: its title, one button per choice, and what the
/// chosen one does.
fn choice<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    p: Palette,
    title: &str,
    value: &mut T,
    options: &[(T, &str, &str)],
) {
    theme::card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).strong().size(theme::text::LEAD));
        ui.add_space(theme::space::S);
        ui.horizontal_wrapped(|ui| {
            for (option, label, _) in options {
                ui.selectable_value(value, *option, *label);
            }
        });
        if let Some((_, _, meaning)) = options.iter().find(|(option, _, _)| option == value) {
            theme::hint(ui, p, *meaning);
        }
    });
    ui.add_space(theme::space::S);
}
