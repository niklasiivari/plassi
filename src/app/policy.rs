use crate::i18n::tr;
use eframe::egui::{self, RichText};

use crate::model::{
    EmptySeatPlacement, OneWish, OrganizationMixing, SeatingPolicy, SittingAlone, TableEnds,
    WithoutWishes,
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
            Some(tr("Rules for the whole event.")),
        );
        let before = self.project.policy.clone();
        let policy = &mut self.project.policy;
        choice(
            ui,
            p,
            tr("Organizations"),
            &mut policy.organization,
            &[
                (
                    OrganizationMixing::SameOrganization,
                    tr("Same organization"),
                    tr("People without wishes sit with their own organization."),
                ),
                (
                    OrganizationMixing::Mixed,
                    tr("Mix organizations"),
                    tr("People without wishes sit with other organizations."),
                ),
                (
                    OrganizationMixing::Indifferent,
                    tr("Doesn't matter"),
                    tr("Organizations do not affect the seating."),
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
                    tr("They are spread between the people with wishes."),
                ),
                (
                    WithoutWishes::ApartFromGroups,
                    tr("Apart from wish groups"),
                    tr("They sit together, away from the wish groups."),
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
                    tr("Never"),
                    tr("Everyone has someone beside or opposite."),
                ),
                (
                    SittingAlone::Avoid,
                    tr("Avoid"),
                    tr("Only wishes can leave someone without a neighbour."),
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
                    tr("Everyone gets one of their wishes close, even at a cost to others."),
                ),
                (
                    OneWish::Preferred,
                    tr("Preferred"),
                    tr("Everyone gets one of their wishes close where it costs little."),
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
                    tr("Groups only"),
                    tr("People without wishes stay off the ends."),
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
                    tr("In opposite pairs at the bottom of tables."),
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
