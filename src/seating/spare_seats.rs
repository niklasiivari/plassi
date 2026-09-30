//! Which seats stay empty when there are more seats than people.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::model::{Area, EmptySeatPlacement, Project, RuleKind, Seat, Side};

/// Which seats may be used, leaving spare seats at table bottoms as opposite pairs.
///
/// `allowed` lists, for each person limited by hard placement rules, the seats
/// they may take; seats are only left empty while each such group of people
/// still fits. Seats in `held` (by people who may not move) stay usable.
pub fn usable_seats(
    project: &Project,
    seats: &[Seat],
    allowed: &[Vec<bool>],
    held: &HashSet<usize>,
) -> Vec<bool> {
    let mut usable = vec![true; seats.len()];
    let mut spare = seats.len().saturating_sub(project.attendees.len());
    if spare == 0 {
        return usable;
    }
    let fits = Restrictions::new(allowed);
    let protected: HashSet<&str> = project
        .rules
        .iter()
        .filter(|r| r.enabled)
        .filter_map(|r| match &r.kind {
            RuleKind::Placement {
                area: Area::Seat(id),
                must_be: true,
                ..
            } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    if project.policy.empty_seats == EmptySeatPlacement::TableBottoms {
        // Remaining side rows per table: row index -> seat indices.
        let mut tables: Vec<BTreeMap<usize, Vec<usize>>> = Vec::new();
        let mut table_of: HashMap<&str, usize> = HashMap::new();
        for (i, seat) in seats.iter().enumerate() {
            let t = *table_of.entry(seat.table_id.as_str()).or_insert_with(|| {
                tables.push(BTreeMap::new());
                tables.len() - 1
            });
            tables[t].entry(seat.index).or_default().push(i);
        }
        let removable = |row: &Vec<usize>, usable: &[bool]| {
            row.iter()
                .all(|&i| !protected.contains(seats[i].id.as_str()) && !held.contains(&i))
                && fits.after_removing(usable, row)
        };
        while spare > 0 {
            // Prefer a full opposite pair while two or more seats remain spare; take
            // it from the table with the most rows so tables stay balanced.
            let want = if spare >= 2 { 2 } else { 1 };
            let pick = tables
                .iter()
                .enumerate()
                .filter_map(|(t, rows)| {
                    rows.iter().next_back().map(|(_, row)| (t, rows.len(), row))
                })
                .filter(|(_, _, row)| removable(row, &usable))
                .max_by_key(|(_, len, row)| (row.len() == want, *len));
            let Some((t, _, row)) = pick else { break };
            let row = row.clone();
            if row.len() <= spare {
                for &i in &row {
                    usable[i] = false;
                }
                spare -= row.len();
                tables[t].pop_last();
            } else {
                // One spare seat but a full pair at the bottom: drop the right-hand one.
                let &i = row
                    .iter()
                    .max_by_key(|&&i| seats[i].side == Side::Right)
                    .unwrap();
                usable[i] = false;
                tables[t]
                    .last_entry()
                    .unwrap()
                    .get_mut()
                    .retain(|&j| j != i);
                spare -= 1;
            }
        }
    }
    for i in (0..seats.len()).rev() {
        if spare == 0 {
            break;
        }
        if usable[i]
            && !protected.contains(seats[i].id.as_str())
            && !held.contains(&i)
            && fits.after_removing(&usable, &[i])
        {
            usable[i] = false;
            spare -= 1;
        }
    }
    usable
}

/// People limited by hard placement rules, grouped by their allowed seats.
struct Restrictions<'a> {
    /// Allowed seats and how many people share exactly that set.
    groups: Vec<(&'a [bool], usize)>,
}

impl<'a> Restrictions<'a> {
    fn new(allowed: &'a [Vec<bool>]) -> Self {
        let mut groups: Vec<(&[bool], usize)> = Vec::new();
        for seats in allowed {
            match groups.iter_mut().find(|(s, _)| *s == seats.as_slice()) {
                Some((_, count)) => *count += 1,
                None => groups.push((seats, 1)),
            }
        }
        Self { groups }
    }

    /// Whether every group still has a usable seat per person once `removed` go.
    fn after_removing(&self, usable: &[bool], removed: &[usize]) -> bool {
        self.groups.iter().all(|(allowed, count)| {
            let left = (0..usable.len())
                .filter(|&s| usable[s] && allowed[s] && !removed.contains(&s))
                .count();
            left >= *count
        })
    }
}
