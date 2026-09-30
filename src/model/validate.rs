//! Consistency checks run on load and before solving.

use std::collections::HashSet;

use anyhow::{Result, anyhow, bail};

use super::*;

/// Largest wish importance and distance cost; keeps scores far from overflow.
const MAX_WEIGHT: i64 = 10_000;
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
        if self
            .attendees
            .iter()
            .any(|p| p.id == usize::MAX || p.name.trim().is_empty())
        {
            bail!("attendees need a name and a usable ID");
        }
        let known = self.layout.validate()?;
        let mut rule_ids = HashSet::new();
        for rule in &self.rules {
            if rule.id == u64::MAX {
                bail!("rule ID is out of range");
            }
            if !rule_ids.insert(rule.id) {
                bail!("duplicate rule ID {}", rule.id);
            }
            rule.validate(&people, &known)?;
        }
        if self.notes.iter().any(|note| !people.contains(&note.person)) {
            return Err(anyhow!("import note refers to unknown person"));
        }
        if !(0..=100).contains(&self.policy.organization_weight)
            || !(-50..=50).contains(&self.policy.sprinkle)
            || !(0..=2_000).contains(&self.policy.isolation_penalty)
            || !(0..=2_000).contains(&self.policy.left_out_penalty)
        {
            bail!("seating policy values are outside their supported ranges");
        }
        for candidate in &self.candidates {
            candidate.validate(&people)?;
        }
        self.validate_preferences(&people)?;
        let (soft, hard) = self.score_bounds();
        let encoded = hard
            .saturating_mul(soft.saturating_add(1))
            .saturating_add(soft);
        if encoded > i128::from(i64::MAX / 4) {
            bail!("project scores exceed the supported range");
        }
        Ok(())
    }

    /// Conservative bounds for soft penalties and possible hard violations.
    /// The solver uses the soft bound plus one as its score radix.
    pub(crate) fn score_bounds(&self) -> (i128, i128) {
        let mut soft = 0_i128;
        let mut hard = 0_i128;
        let people = self.attendees.len() as i128;

        for rule in self.rules.iter().filter(|rule| rule.enabled) {
            match &rule.kind {
                RuleKind::Placement {
                    penalty,
                    hard: is_hard,
                    ..
                } => {
                    if *is_hard {
                        hard += 1;
                    } else {
                        soft = soft.saturating_add(i128::from(*penalty));
                    }
                }
                RuleKind::AvoidTableEnd {
                    end_penalty,
                    near_end_penalty,
                    ..
                } => {
                    soft = soft.saturating_add(i128::from((*end_penalty).max(*near_end_penalty)));
                }
                RuleKind::Proximity {
                    costs,
                    weight,
                    max_class,
                    ..
                } => {
                    soft = soft.saturating_add(
                        i128::from(costs.0[CLASSES - 1]).saturating_mul(i128::from(*weight)),
                    );
                    if max_class.is_some() {
                        hard += 1;
                    }
                }
            }
        }

        for preference in self.active_preferences() {
            let count = preference.kind.people().len() as i128;
            let pairs = count.saturating_mul(count.saturating_sub(1)) / 2;
            let pair_cost = match &preference.kind {
                crate::preferences::PreferenceKind::Together { costs, .. } => {
                    i128::from(costs.0[CLASSES - 1])
                        .saturating_mul(i128::from(preference.strength.weight()) * 2)
                }
                crate::preferences::PreferenceKind::Apart { .. } => {
                    i128::from(ProximityCosts::WISH[DistanceClass::Far])
                        .saturating_mul(i128::from(preference.strength.weight()))
                }
            };
            soft = soft.saturating_add(pairs.saturating_mul(pair_cost));
            if preference.strength.hard() {
                hard = hard.saturating_add(pairs);
            }
        }

        let pairs = people.saturating_mul(people.saturating_sub(1)) / 2;
        let policy_pair =
            i128::from(self.policy.organization_weight) + i128::from(self.policy.sprinkle).abs();
        soft = soft
            .saturating_add(pairs.saturating_mul(policy_pair))
            .saturating_add(people.saturating_mul(i128::from(self.policy.isolation_penalty)))
            .saturating_add(people.saturating_mul(i128::from(self.policy.left_out_penalty)))
            .saturating_add(people.saturating_mul(5_000));
        (soft, hard)
    }

    pub(crate) fn score_radix(&self) -> i64 {
        self.score_bounds()
            .0
            .saturating_add(1)
            .min(i128::from(i64::MAX / 4)) as i64
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
                if !(0..=MAX_DISTANCE_COST).contains(penalty) {
                    bail!("rule {} has invalid placement penalty", self.id);
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
                if !(0..=10_000).contains(end_penalty) || !(0..=10_000).contains(near_end_penalty) {
                    bail!("rule {} has an invalid table-end penalty", self.id);
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
                    || costs.0[CLASSES - 1] > MAX_DISTANCE_COST
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
