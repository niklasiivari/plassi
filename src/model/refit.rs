//! Keeping options and pins when the room changes.

use std::collections::HashSet;

use super::{Pin, Project};

impl Project {
    /// Fits options and pins to a changed room. Everyone whose seat still
    /// exists keeps it; people on removed seats lose their seat and are listed
    /// as not seated. Pins on removed seats or tables are dropped. Returns how
    /// many people lost a seat in the option that lost the most.
    pub fn fit_to_room(&mut self) -> usize {
        let seats: HashSet<String> = self.layout.seats().into_iter().map(|s| s.id).collect();
        let tables: HashSet<&String> = self
            .layout
            .rows
            .iter()
            .flat_map(|r| &r.tables)
            .map(|t| &t.id)
            .collect();
        let gone: HashSet<u64> = self
            .rules
            .iter()
            .filter(|r| match self.pin_of(r) {
                Some((_, Pin::Seat(seat))) => !seats.contains(&seat),
                Some((_, Pin::Table(table))) => !tables.contains(&table),
                None => false,
            })
            .map(|r| r.id)
            .collect();
        self.rules.retain(|r| !gone.contains(&r.id));
        let mut most = 0;
        for option in &mut self.candidates {
            let before = option.assignment.len();
            option.assignment.retain(|_, seat| seats.contains(seat));
            most = most.max(before - option.assignment.len());
        }
        most
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{Pin, Spot};

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
        assert_eq!(project.fit_to_room(), 5);

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
}
