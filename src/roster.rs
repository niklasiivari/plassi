//! Late changes to who attends: adding a walk-in, removing a cancellation,
//! and replacing one person with another in the same seat.

use std::collections::BTreeMap;

use anyhow::{Result, bail};

use crate::i18n::{fill, tr};
use crate::model::{Attendee, Project, RuleKind, Spot, Target};

/// Details for a new attendee.
#[derive(Clone, Default)]
pub struct NewAttendee {
    pub name: String,
    pub organization: String,
    pub participant_group: String,
    pub food: String,
    pub drink: String,
    pub allergies: String,
}

impl NewAttendee {
    fn into_attendee(self, id: usize) -> Attendee {
        let tags = [&self.organization, &self.participant_group]
            .into_iter()
            .filter(|t| !t.trim().is_empty())
            .cloned()
            .collect();
        Attendee {
            id,
            name: self.name.trim().to_string(),
            organization: self.organization.trim().to_string(),
            participant_group: self.participant_group.trim().to_string(),
            tags,
            food: self.food,
            drink: self.drink,
            allergies: self.allergies,
            price_override_cents: None,
            extra: BTreeMap::new(),
        }
    }
}

/// Adds an attendee and seats them in the best free seat of every option.
/// Fails if the name is empty or taken, or an option has no free seat.
pub fn add(project: &mut Project, person: NewAttendee) -> Result<usize> {
    let name = person.name.trim();
    if name.is_empty() {
        bail!(tr("Enter a name"));
    }
    if project
        .attendees
        .iter()
        .any(|a| a.name.eq_ignore_ascii_case(name))
    {
        bail!(fill("{} is already on the list", &[&name]));
    }
    let seat_count = project.layout.seat_count();
    if project.attendees.len() >= seat_count {
        bail!(fill(
            "All {} seats are full. Add seats in Room layout.",
            &[&seat_count]
        ));
    }
    let id = project
        .attendees
        .iter()
        .map(|a| a.id + 1)
        .max()
        .unwrap_or(0);
    project.attendees.push(person.into_attendee(id));
    for index in 0..project.candidates.len() {
        if let Some(seat) = best_free_seat(project, index, id) {
            let candidate = &mut project.candidates[index];
            candidate.assignment.insert(id, seat);
            candidate.hand_placed.insert(id);
        }
    }
    Ok(id)
}

/// Removes an attendee with their notes, their own rules, and themselves from
/// other people's wishes. Their seats become free.
pub fn remove(project: &mut Project, id: usize) {
    project.attendees.retain(|a| a.id != id);
    project.notes.retain(|n| n.person != id);
    project.remove_from_preferences(id);
    project.rules.retain_mut(|rule| match &mut rule.kind {
        RuleKind::Placement { person, .. } | RuleKind::AvoidTableEnd { person, .. } => {
            *person != id
        }
        RuleKind::Proximity { requester, .. } if *requester == id => false,
        RuleKind::Proximity {
            target: Target::People { ids },
            ..
        } => {
            ids.retain(|t| *t != id);
            !ids.is_empty()
        }
        RuleKind::Proximity { .. } => true,
    });
    for candidate in &mut project.candidates {
        candidate.forget(id);
    }
}

/// Replaces `old` with a new attendee who takes their place in every option.
/// Wishes by or about the departing person are removed.
pub fn replace(project: &mut Project, old: usize, person: NewAttendee) -> Result<usize> {
    let name = person.name.trim();
    if name.is_empty() {
        bail!(tr("Enter a name"));
    }
    if project
        .attendees
        .iter()
        .any(|a| a.id != old && a.name.eq_ignore_ascii_case(name))
    {
        bail!(fill("{} is already on the list", &[&name]));
    }
    let id = project
        .attendees
        .iter()
        .map(|a| a.id + 1)
        .max()
        .unwrap_or(0);
    let spots: Vec<Spot> = project.candidates.iter().map(|c| c.spot_of(old)).collect();
    remove(project, old);
    project.attendees.push(person.into_attendee(id));
    for (candidate, spot) in project.candidates.iter_mut().zip(spots) {
        if matches!(spot, Spot::Seat(_)) {
            candidate.hand_placed.insert(id);
        }
        candidate.place(id, spot);
    }
    Ok(id)
}

/// The free seat in option `index` where `person` scores best.
fn best_free_seat(project: &Project, index: usize, person: usize) -> Option<String> {
    let candidate = &project.candidates[index];
    let (seat, _) = crate::arrange::free_seats(project, candidate, person)
        .into_iter()
        .next()?;
    Some(seat)
}
