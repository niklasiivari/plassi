//! Keeping options, pins and rules when the room changes.

use std::collections::HashSet;

use super::{Area, Project, RuleKind};

/// What a room change took away.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RoomFit {
    /// People who lost a seat, in the option that lost the most.
    pub unseated: usize,
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
                Area::Tag(_) => true,
            },
            _ => true,
        });
        let layout = &mut self.layout;
        layout.seat_tags.retain(|seat, _| seats.contains(seat));
        layout
            .relations
            .retain(|r| seats.contains(&r.a) && seats.contains(&r.b));
        let mut unseated = 0;
        for option in &mut self.candidates {
            let seated = option.assignment.len();
            option.assignment.retain(|_, seat| seats.contains(seat));
            unseated = unseated.max(seated - option.assignment.len());
        }
        RoomFit {
            unseated,
            rules_removed: before - self.rules.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{Area, DistanceClass, Pin, Rule, RuleKind, SeatRelation, Spot};

    #[test]
    fn people_keep_seats_that_still_exist() {
        let mut project = crate::tests::two_tables();
        let mut option = project.empty_option("Option 1".into());
        for (person, seat) in project.layout.seats().iter().enumerate() {
            option.place(person, Spot::Seat(seat.id.clone()));
        }
        project.candidates.push(option.clone());
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
                unseated: 5,
                rules_removed: 2
            }
        );

        let option = &project.candidates[0];
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
    fn rules_tags_and_distances_of_removed_places_go() {
        let mut project = crate::tests::two_tables();
        project
            .layout
            .seat_tags
            .insert("t2:left:0".into(), vec!["aisle".into()]);
        project.layout.relations.push(SeatRelation {
            a: "t1:left:0".into(),
            b: "t2:left:0".into(),
            class: DistanceClass::Near,
        });
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
        assert!(project.layout.seat_tags.is_empty());
        assert!(project.layout.relations.is_empty());
        project.validate().unwrap();
    }
}
