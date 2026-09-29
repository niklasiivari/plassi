//! Wish groups (people connected by person-to-person wishes) and suggestions
//! for moving a whole group to another part of the room.

use std::collections::{BTreeMap, HashMap, HashSet};
use web_time::Duration;

use crate::i18n::{fill, tr};
use crate::model::{Project, RuleKind, Seat, Side, Target};

/// Connected components of the person-to-person wish graph, largest first.
/// Only groups of two or more people are returned.
pub fn wish_groups(project: &Project) -> Vec<Vec<usize>> {
    let ids: Vec<usize> = project.attendees.iter().map(|a| a.id).collect();
    let mut parent: HashMap<usize, usize> = ids.iter().map(|&i| (i, i)).collect();
    fn find(parent: &mut HashMap<usize, usize>, x: usize) -> usize {
        let p = parent[&x];
        if p == x {
            return x;
        }
        let root = find(parent, p);
        parent.insert(x, root);
        root
    }
    for rule in project.rules.iter().filter(|r| r.enabled) {
        if let RuleKind::Proximity {
            requester,
            target: Target::People { ids: targets, .. },
            ..
        } = &rule.kind
        {
            for t in targets {
                if parent.contains_key(t) && parent.contains_key(requester) {
                    let (a, b) = (find(&mut parent, *requester), find(&mut parent, *t));
                    parent.insert(a, b);
                }
            }
        }
    }
    // A "together" preference links its members like mutual wishes.
    for preference in project.active_preferences() {
        if let crate::preferences::PreferenceKind::Together { people, .. } = &preference.kind {
            for pair in people.windows(2) {
                if !pair.iter().all(|x| parent.contains_key(x)) {
                    continue;
                }
                let (a, b) = (find(&mut parent, pair[0]), find(&mut parent, pair[1]));
                parent.insert(a, b);
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &id in &ids {
        let root = find(&mut parent, id);
        groups.entry(root).or_default().push(id);
    }
    let mut result: Vec<Vec<usize>> = groups.into_values().filter(|g| g.len() > 1).collect();
    result.sort_by_key(|g| std::cmp::Reverse(g.len()));
    result
}

pub fn group_of(project: &Project, person: usize) -> Option<Vec<usize>> {
    wish_groups(project)
        .into_iter()
        .find(|g| g.contains(&person))
}

#[derive(Clone, Debug)]
pub struct GroupMove {
    /// Seats the group would take.
    pub seats: Vec<String>,
    /// People who would move out of those seats into the group's old seats.
    pub displaced: Vec<usize>,
    pub assignment: BTreeMap<usize, String>,
    pub delta: i64,
    pub description: String,
}

/// Seats ordered top to bottom, left before right, per table.
fn table_order(seats: &[Seat]) -> Vec<Vec<&Seat>> {
    let mut tables: BTreeMap<&str, Vec<&Seat>> = BTreeMap::new();
    for seat in seats {
        tables.entry(seat.table_id.as_str()).or_default().push(seat);
    }
    let mut result: Vec<Vec<&Seat>> = tables.into_values().collect();
    for table in &mut result {
        table.sort_by_key(|s| (s.index, s.side == Side::Right));
    }
    result
}

/// Ranks places a group could move to: blocks of consecutive rows on one
/// table. Whoever sits there swaps into the group's old seats.
pub fn suggest_moves(
    project: &Project,
    assignment: &BTreeMap<usize, String>,
    group: &[usize],
    score: impl Fn(&BTreeMap<usize, String>) -> (i64, usize),
) -> Vec<GroupMove> {
    let pinned = project.pinned_people();
    if group.is_empty() || group.iter().any(|p| pinned.contains(p)) {
        return Vec::new();
    }
    let seats = project.layout.seats();
    let group_set: HashSet<usize> = group.iter().copied().collect();
    let occupant: HashMap<&str, usize> = assignment.iter().map(|(p, s)| (s.as_str(), *p)).collect();
    let (before, hard_before) = score(assignment);
    let gaps_before = crate::model::gaps(&seats, assignment).len();
    let members = in_seating_order(&seats, assignment, group);

    let mut moves = Vec::new();
    let mut seen: HashSet<Vec<String>> = HashSet::new();
    for table in table_order(&seats) {
        for block in table.windows(group.len()) {
            let block_ids: Vec<String> = block.iter().map(|s| s.id.clone()).collect();
            if !seen.insert(block_ids.clone()) {
                continue;
            }
            let displaced: Vec<usize> = block
                .iter()
                .filter_map(|s| occupant.get(s.id.as_str()).copied())
                .filter(|p| !group_set.contains(p))
                .collect();
            // Everyone displaced needs a seat the group leaves.
            let vacating = members.iter().filter(|p| assignment.get(*p).is_some_and(|s| !block_ids.contains(s))).count();
            if displaced.iter().any(|p| pinned.contains(p)) || displaced.len() > vacating {
                continue;
            }
            let next = swapped(assignment, &members, &block_ids, &displaced);
            if next == *assignment || crate::model::gaps(&seats, &next).len() > gaps_before {
                continue;
            }
            let (after, hard_after) = score(&next);
            if hard_after > hard_before {
                continue;
            }
            moves.push(GroupMove {
                description: describe_move(project, block, &displaced),
                seats: block_ids,
                displaced,
                assignment: next,
                delta: after - before,
            });
        }
    }
    moves.sort_by_key(|m| m.delta);
    moves.truncate(6);
    moves
}

/// Group members in their current seating order; unseated members last.
fn in_seating_order(
    seats: &[Seat],
    assignment: &BTreeMap<usize, String>,
    group: &[usize],
) -> Vec<usize> {
    let order: HashMap<&str, usize> = table_order(seats)
        .iter()
        .flatten()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    let mut members: Vec<usize> = group.to_vec();
    members.sort_by_key(|p| {
        assignment
            .get(p)
            .and_then(|s| order.get(s.as_str()))
            .copied()
            .unwrap_or(usize::MAX)
    });
    members
}

/// `assignment` with `members` moved into `block` and the `displaced`
/// occupants moved into the seats the members left.
fn swapped(
    assignment: &BTreeMap<usize, String>,
    members: &[usize],
    block: &[String],
    displaced: &[usize],
) -> BTreeMap<usize, String> {
    let vacated: Vec<String> = members
        .iter()
        .filter_map(|p| assignment.get(p).cloned())
        .filter(|s| !block.contains(s))
        .collect();
    let mut next = assignment.clone();
    for (person, seat) in members.iter().zip(block) {
        next.insert(*person, seat.clone());
    }
    for (person, seat) in displaced.iter().zip(&vacated) {
        next.insert(*person, seat.clone());
    }
    next
}

/// E.g. "Table 2, rows 3–4, swapping with X and Y".
fn describe_move(project: &Project, block: &[&Seat], displaced: &[usize]) -> String {
    let rows: Vec<usize> = block.iter().map(|s| s.index + 1).collect();
    let (lo, hi) = (rows.iter().min().unwrap(), rows.iter().max().unwrap());
    let table_label = project.seat_place(&block[0].id).map_or("", |(table, _, _)| table);
    let who = match displaced.len() {
        0 => tr("into free seats").to_string(),
        n => {
            let names: Vec<String> = displaced
                .iter()
                .take(2)
                .filter_map(|p| project.person(*p).map(|a| a.name.clone()))
                .collect();
            if n > 2 {
                fill("swapping with {} and {} more", &[&names.join(", "), &(n - 2)])
            } else {
                match names.as_slice() {
                    [a, b] => fill("swapping with {} and {}", &[a, b]),
                    _ => fill("swapping with {}", &[&names.join(", ")]),
                }
            }
        }
    };
    fill("{}, rows {}–{}, {}", &[&table_label, lo, hi, &who])
}

/// After applying a group move, tidies seating among the people who moved.
pub fn polish(project: &Project, mv: &GroupMove, group: &[usize]) -> BTreeMap<usize, String> {
    let moving: HashSet<usize> = group.iter().chain(&mv.displaced).copied().collect();
    let fixed = project
        .attendees
        .iter()
        .map(|a| a.id)
        .filter(|p| !moving.contains(p))
        .chain(project.pinned_people())
        .collect();
    crate::seating::improve(project, &mv.assignment, &fixed, Duration::from_millis(300))
}
