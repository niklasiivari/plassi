//! Consistency checks run on load and before solving.

use std::collections::HashSet;

use anyhow::{Result, anyhow, bail};

use super::*;

/// Largest wish importance and distance cost; keeps scores far from overflow.
const MAX_WEIGHT: i64 = 10_000;
const MAX_COST: i64 = 100_000;

/// Seats on one side of a table, at most.
const MAX_SIDE_SEATS: usize = 500;

impl Project {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            bail!("unsupported project version {}", self.version);
        }
        let people: HashSet<_> = self.attendees.iter().map(|p| p.id).collect();
        if people.len() != self.attendees.len() {
            bail!("duplicate attendee ID");
        }
        let known = self.layout.validate()?;
        let mut rule_ids = HashSet::new();
        for rule in &self.rules {
            if !rule_ids.insert(rule.id) {
                bail!("duplicate rule ID {}", rule.id);
            }
            rule.validate(&people, &known)?;
        }
        if self.notes.iter().any(|note| !people.contains(&note.person)) {
            return Err(anyhow!("import note refers to unknown person"));
        }
        for candidate in &self.candidates {
            candidate.validate(&people)?;
        }
        self.validate_preferences(&people)
    }
}

impl Candidate {
    /// Known people, at most one per seat. Seats that no longer exist are
    /// allowed (the room may have changed); they show up as assignment errors.
    fn validate(&self, people: &HashSet<usize>) -> Result<()> {
        if let Some(person) = self.assignment.keys().find(|p| !people.contains(p)) {
            bail!("{} seats unknown person {person}", self.name);
        }
        let mut taken = HashSet::new();
        if let Some(seat) = self.assignment.values().find(|s| !taken.insert(*s)) {
            bail!("{} puts two people on seat {seat}", self.name);
        }
        Ok(())
    }
}

/// Row, table and seat IDs of a validated layout, for checking rule references.
struct LayoutIds<'a> {
    rows: HashSet<&'a String>,
    tables: HashSet<&'a String>,
    seats: HashSet<String>,
}

impl Layout {
    /// Checks ID uniqueness, custom seat geometry, tags and relations.
    fn validate(&self) -> Result<LayoutIds<'_>> {
        let mut rows = HashSet::new();
        let mut tables = HashSet::new();
        for row in &self.rows {
            if row.id.is_empty() || !rows.insert(&row.id) {
                bail!("empty or duplicate row ID: {}", row.id);
            }
            for table in &row.tables {
                if table.id.is_empty() || !tables.insert(&table.id) {
                    bail!("empty or duplicate table ID: {}", table.id);
                }
                // Seat IDs are "table:side:index".
                if table.id.contains(':') {
                    bail!("table ID {} may not contain ':'", table.id);
                }
                if table.left.max(table.right) > MAX_SIDE_SEATS {
                    bail!(
                        "table {} has more than {MAX_SIDE_SEATS} seats on a side",
                        table.id
                    );
                }
            }
        }
        let all_seats = self.seats();
        let seats: HashSet<String> = all_seats.iter().map(|s| s.id.clone()).collect();
        if seats.len() != all_seats.len() {
            bail!("duplicate seat ID");
        }
        if let Some(id) = self.seat_tags.keys().find(|id| !seats.contains(*id)) {
            bail!("tags refer to unknown seat {id}");
        }
        self.validate_relations(&seats)?;
        Ok(LayoutIds {
            rows,
            tables,
            seats,
        })
    }

    fn validate_relations(&self, seats: &HashSet<String>) -> Result<()> {
        let mut pairs = HashSet::new();
        for relation in &self.relations {
            if relation.a == relation.b
                || !seats.contains(&relation.a)
                || !seats.contains(&relation.b)
            {
                bail!(
                    "relation refers to missing or identical seats: {} / {}",
                    relation.a,
                    relation.b
                );
            }
            let mut pair = [relation.a.as_str(), relation.b.as_str()];
            pair.sort();
            if !pairs.insert(pair) {
                bail!("duplicate seat relation {} / {}", relation.a, relation.b);
            }
        }
        Ok(())
    }
}

impl Rule {
    /// Checks references to people and layout IDs, and that costs are sane.
    fn validate(&self, people: &HashSet<usize>, layout: &LayoutIds) -> Result<()> {
        match &self.kind {
            RuleKind::Placement {
                person,
                area,
                penalty,
                ..
            } => {
                if !people.contains(person) {
                    bail!("rule {} refers to unknown person", self.id);
                }
                if *penalty < 0 {
                    bail!("rule {} has negative penalty", self.id);
                }
                match area {
                    Area::Seat(id) if !layout.seats.contains(id) => {
                        bail!("rule {} refers to unknown seat {}", self.id, id)
                    }
                    Area::Table(id) if !layout.tables.contains(id) => {
                        bail!("rule {} refers to unknown table {}", self.id, id)
                    }
                    Area::Row(id) | Area::RowCenter(id) if !layout.rows.contains(id) => {
                        bail!("rule {} refers to unknown row {}", self.id, id)
                    }
                    _ => Ok(()),
                }
            }
            RuleKind::AvoidTableEnd {
                person,
                end_penalty,
                near_end_penalty,
            } => {
                if !people.contains(person) {
                    bail!("rule {} refers to unknown person", self.id);
                }
                if *end_penalty < 0 || *near_end_penalty < 0 {
                    bail!("rule {} has a negative penalty", self.id);
                }
                Ok(())
            }
            RuleKind::Proximity {
                requester,
                target,
                costs,
                weight,
                ..
            } => {
                if !people.contains(requester) {
                    bail!("rule {} refers to unknown requester", self.id);
                }
                if !(0..=MAX_WEIGHT).contains(weight)
                    || !costs.is_ordered()
                    || costs.0[0] < 0
                    || costs.0[CLASSES - 1] > MAX_COST
                {
                    bail!("rule {} has invalid proximity costs", self.id);
                }
                self.validate_target(target, *requester, people)
            }
        }
    }

    fn validate_target(
        &self,
        target: &Target,
        requester: usize,
        people: &HashSet<usize>,
    ) -> Result<()> {
        match target {
            Target::People { ids } => {
                let distinct = ids.iter().collect::<HashSet<_>>().len() == ids.len();
                if ids.is_empty()
                    || !distinct
                    || ids
                        .iter()
                        .any(|id| !people.contains(id) || *id == requester)
                {
                    bail!("rule {} has invalid targets", self.id);
                }
            }
            Target::Tag {
                required,
                tag: name,
            }
            | Target::Organization { required, name } => {
                if *required == 0 || name.trim().is_empty() {
                    bail!("rule {} has invalid tag target", self.id);
                }
            }
        }
        Ok(())
    }
}
