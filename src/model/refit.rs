//! Keeping options, pins and rules when the room changes.

use std::collections::HashSet;

use super::{Area, Project, RuleKind, SolverStatus};

/// What a room change took away.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RoomFit {
    /// How many people lost their seat, per option.
    pub unseated: Vec<usize>,
    /// Rules about removed seats, tables or rows, pins included.
    pub rules_removed: usize,
}

impl Project {
    /// Fits options, rules and seat settings to a changed room. Everyone whose
    /// seat still exists keeps it; people on removed seats lose their seat and
    /// are listed as not seated. Rules about removed seats, tables or rows
    /// (pins included) are deleted, and so are tags and distances of removed
    /// seats.
    pub fn fit_to_room(&mut self) -> RoomFit {
        let seats: HashSet<String> = self.layout.seats().into_iter().map(|s| s.id).collect();
        let tables: HashSet<String> = self
            .layout
            .rows
            .iter()
            .flat_map(|r| &r.tables)
            .map(|t| t.id.clone())
            .collect();
        let rows: HashSet<String> = self.layout.rows.iter().map(|r| r.id.clone()).collect();
        let before = self.rules.len();
        self.rules.retain(|rule| match &rule.kind {
            RuleKind::Placement { area, .. } => match area {
                Area::Seat(id) => seats.contains(id),
                Area::Table(id) => tables.contains(id),
                Area::Row(id) | Area::RowCenter(id) => rows.contains(id),
            },
            _ => true,
        });
        let mut unseated = Vec::new();
        for option in &mut self.candidates {
            let lost: Vec<usize> = option
                .assignment
                .iter()
                .filter(|(_, seat)| !seats.contains(*seat))
                .map(|(person, _)| *person)
                .collect();
            for person in &lost {
                // No longer placed anywhere, so Improve has nothing to keep.
                option.assignment.remove(person);
                option.hand_placed.remove(person);
            }
            if !lost.is_empty() {
                // A solver's status no longer describes this option.
                option.solver_status = SolverStatus::RoomChanged;
            }
            unseated.push(lost.len());
        }
        RoomFit {
            unseated,
            rules_removed: before - self.rules.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{Area, Pin, Rule, RuleKind, SolverStatus, Spot};

    #[test]
    fn people_keep_seats_that_still_exist() {
        let mut project = crate::tests::two_tables();
        let mut option = project.empty_option("Option 1".into());
        for (person, seat) in project.layout.seats().iter().enumerate() {
            option.place(person, Spot::Seat(seat.id.clone()));
        }
        project.candidates.push(option.clone());
        // A second option with only the first table's top seat taken loses nobody.
        let mut small = project.empty_option("Option 2".into());
        small.place(0, Spot::Seat("t1:left:0".into()));
        project.candidates.push(small);
        let on = |seat: &str| {
            *option
                .assignment
                .iter()
                .find(|(_, s)| *s == seat)
                .unwrap()
                .0
        };
        let (kept, cut, moved_table) = (on("t1:left:0"), on("t1:left:1"), on("t2:right:0"));
        project.pin(kept, Pin::Seat("t1:left:0".into()));
        project.pin(cut, Pin::Seat("t1:left:1".into()));
        project.pin(moved_table, Pin::Table("t2".into()));

        let table = &mut project.layout.rows[0].tables;
        table[0].left = 1;
        table[0].right = 3;
        table.remove(1);
        assert_eq!(
            project.fit_to_room(),
            super::RoomFit {
                unseated: vec![5, 0],
                rules_removed: 2
            }
        );

        assert_eq!(
            project.candidates[1].spot_of(0),
            Spot::Seat("t1:left:0".into())
        );
        let option = &project.candidates[0];
        assert_eq!(option.solver_status, SolverStatus::RoomChanged);
        assert_eq!(
            project.candidates[1].solver_status,
            SolverStatus::StartedEmpty
        );
        assert_eq!(option.spot_of(kept), Spot::Seat("t1:left:0".into()));
        assert_eq!(option.spot_of(cut), Spot::Unseated);
        assert_eq!(option.spot_of(moved_table), Spot::Unseated);
        assert_eq!(option.assignment.len(), 3);
        let pins = project.pins();
        assert_eq!(pins.get(&kept), Some(&Pin::Seat("t1:left:0".into())));
        assert!(!pins.contains_key(&cut) && !pins.contains_key(&moved_table));
        project.validate().unwrap();
    }

    #[test]
    fn rules_about_removed_places_go() {
        let mut project = crate::tests::two_tables();
        let id = project.next_rule_id();
        project.rules.push(Rule {
            id,
            label: "Row".into(),
            source: "test".into(),
            enabled: true,
            kind: RuleKind::Placement {
                person: 0,
                area: Area::Table("t2".into()),
                must_be: false,
                hard: false,
                penalty: 10,
            },
        });
        project.layout.rows[0].tables.remove(1);
        assert_eq!(project.fit_to_room().rules_removed, 1);
        project.validate().unwrap();
    }
}
