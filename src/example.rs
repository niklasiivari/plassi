//! A made-up example event for trying the app: placeholder guests, wishes,
//! sign-up notes and a room. No real names or organizations.

use crate::import::wish_rule;
use crate::model::{Attendee, ImportField, ImportNote, Layout, Project};

const GUESTS: usize = 48;
const ORGANIZATIONS: [&str; 4] = ["Org A", "Org B", "Org C", "Org D"];
const FOOD: [&str; 3] = ["Meat", "Fish", "Vegetarian"];

fn guest(id: usize) -> String {
    format!("Guest {:02}", id + 1)
}

pub fn project() -> Project {
    let mut project = Project::new();
    project.event.title = "Example dinner".into();
    project.attendees = (0..GUESTS)
        .map(|id| Attendee {
            id,
            name: guest(id),
            organization: ORGANIZATIONS[id * 7 % ORGANIZATIONS.len()].into(),
            participant_group: if id % 5 == 0 { "Guest of honour" } else { "Member" }.into(),
            tags: Vec::new(),
            food: FOOD[id % FOOD.len()].into(),
            drink: if id % 4 == 0 { "Without alcohol" } else { "With alcohol" }.into(),
            allergies: if id % 11 == 0 { "Nuts".into() } else { String::new() },
            extra: Default::default(),
        })
        .collect();
    // Couples (avecs), and a few friends who asked for each other.
    let mut rule = 0;
    let mut add = |project: &mut Project, from: usize, to: Vec<usize>, avec: bool| {
        rule += 1;
        project.rules.push(wish_rule(rule, from, to, avec, ""));
    };
    for pair in (0..12).step_by(2) {
        add(&mut project, pair, vec![pair + 1], true);
        add(&mut project, pair + 1, vec![pair], true);
    }
    for (from, to) in [(20, vec![21, 22]), (21, vec![20]), (30, vec![31]), (40, vec![12])] {
        add(&mut project, from, to, false);
    }
    // Sign-up text still to review, as it would come from a form.
    for (person, wish) in [(14, 15), (25, 26), (33, 20)] {
        project.notes.push(ImportNote {
            person,
            field: ImportField::Wish,
            text: format!("Would like to sit with {}", guest(wish)),
            reviewed: false,
        });
    }
    let (tables, per_side) = Layout::suggested(GUESTS);
    project.layout = Layout::simple(tables, per_side);
    project
}
