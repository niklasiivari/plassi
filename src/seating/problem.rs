//! The seating problem compiled to indices, and its score.
//!
//! One definition of the score serves the search, the incremental move
//! evaluation and the policy breakdown shown in the app.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::model::{
    DistanceClass, OrganizationMixing, Project, Rule, RuleKind, Seat, Side, area_contains,
    table_distance,
};

/// Seating-policy cost of a chart, by part.
#[derive(Clone, Debug, Default)]
pub struct PolicyReport {
    pub organization: i64,
    pub sprinkle: i64,
    /// Attendee IDs of people with nobody beside or opposite them.
    pub isolated: Vec<usize>,
    pub isolation: i64,
}

impl PolicyReport {
    pub fn total(&self) -> i64 {
        self.organization + self.sprinkle + self.isolation
    }
}

pub(super) const HARD: i64 = 1_000_000;
pub(super) const NONE: usize = usize::MAX;
/// Organization members beyond this many nearest weigh nothing.
const DECAY_RANKS: usize = 10;

/// Reused buffers for scoring terms without allocating.
#[derive(Default)]
pub(super) struct Scratch {
    classes: Vec<DistanceClass>,
    /// Per group member not yet joined: seat and distance to the nearest joined one.
    link: Vec<(usize, DistanceClass)>,
    /// Group members' (table, row).
    spots: Vec<(usize, usize)>,
}

/// How a group sits: its widest break, what its breaks cost, how many
/// pieces it is in and how many rows longer than needed it is.
pub(super) struct BlockShape {
    pub(super) widest: DistanceClass,
    pub(super) gap_cost: i64,
    pub(super) pieces: usize,
    pub(super) extra_rows: usize,
}

/// Where a part of the score comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Rule(u64),
    Preference(u64),
}

/// How a term turns distances into a cost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Shape {
    /// The mean cost of the `required` nearest targets.
    Nearest,
    /// Every target, each half as much as the next nearer one (an
    /// organization); the nearest decides hard limits and "met".
    Halving,
    /// The requester and targets as one compact block: `costs` per break
    /// between pieces, by its width, and `stretch` per row longer than needed.
    Block { stretch: i64 },
}

/// One person's wish to be near others, or a group that sits together.
pub(super) struct Term {
    pub(super) requester: usize,
    pub(super) targets: Vec<usize>,
    pub(super) required: usize,
    /// Penalty per distance class.
    pub(super) costs: [i64; crate::model::CLASSES],
    /// Distance classes that break a hard rule.
    pub(super) forbidden: [bool; crate::model::CLASSES],
    pub(super) shape: Shape,
    pub(super) source: Source,
}

/// Each requester's wishes, for the "nobody loses every wish" cost.
pub(super) struct Fairness {
    /// Per requester with wishes: their wish terms.
    pub(super) groups: Vec<Vec<usize>>,
    pub(super) requester: Vec<usize>,
    /// Group of each term, if it is a wish.
    pub(super) of_term: Vec<Option<usize>>,
    pub(super) penalty: i64,
}

impl Fairness {
    fn new(terms: &[Term], n_people: usize, penalty: i64) -> Self {
        let mut group_of_person = vec![None; n_people];
        let mut fairness = Self {
            groups: Vec::new(),
            requester: Vec::new(),
            of_term: vec![None; terms.len()],
            penalty,
        };
        if penalty == 0 {
            return fairness;
        }
        for (t, term) in terms
            .iter()
            .enumerate()
            .filter(|(_, t)| matches!(t.source, Source::Rule(_)))
        {
            let g = *group_of_person[term.requester].get_or_insert_with(|| {
                fairness.groups.push(Vec::new());
                fairness.requester.push(term.requester);
                fairness.groups.len() - 1
            });
            fairness.groups[g].push(t);
            fairness.of_term[t] = Some(g);
        }
        fairness
    }
}

/// A placement rule's cost per seat: `None` where a hard rule forbids it.
pub(super) struct PlacementRule {
    pub(super) rule_id: u64,
    pub(super) person: usize,
    pub(super) cost: Vec<Option<i64>>,
}

/// A project compiled to indices, with the full score and its parts.
pub(super) struct Problem {
    /// Radix separating hard violations from the total soft penalty.
    pub(super) hard_cost: i64,
    pub(super) people: Vec<usize>,
    pub(super) seat_ids: Vec<String>,
    /// Per seat: its table (as an index) and row at that table.
    seat_spot: Vec<(usize, usize)>,
    pub(super) n_seats: usize,
    pub(super) class: Vec<DistanceClass>,
    pub(super) neighbours: Vec<Vec<usize>>,
    /// Per person, per seat: soft placement cost, or `None` if a hard rule forbids it.
    pub(super) placement: Vec<Vec<Option<i64>>>,
    pub(super) placement_rules: Vec<PlacementRule>,
    pub(super) terms: Vec<Term>,
    pub(super) terms_of: Vec<Vec<usize>>,
    pub(super) fairness: Fairness,
    pub(super) constrained: Vec<bool>,
    /// People with the same key are interchangeable: swapping them changes
    /// nothing. `None` for anyone the score treats individually.
    pub(super) swap_key: Vec<Option<usize>>,
    /// Each person's organizations as sorted IDs; two people share an
    /// organization if these overlap.
    pub(super) org: Vec<Vec<u32>>,
    pub(super) org_mode: OrganizationMixing,
    pub(super) org_weight: i64,
    pub(super) sprinkle: i64,
    pub(super) isolation: i64,
    /// Someone can end up with nobody beside or opposite: there are empty seats
    /// and isolation costs something.
    pub(super) may_isolate: bool,
    /// Policy cost of each pair of people sitting side by side, row-major.
    pair: Vec<i64>,
    pub(super) usable: Vec<bool>,
    pub(super) fixed: Vec<bool>,
    /// Seat order used to place groups in contiguous blocks.
    pub(super) seat_order: Vec<usize>,
    /// Per table, per row: the left and right seat (`NONE` where a side is shorter).
    pub(super) table_rows: Vec<Vec<[usize; 2]>>,
    pub(super) groups: Vec<Vec<usize>>,
    /// For "different option": each constrained person's previous seat.
    pub(super) prior: Option<(Vec<usize>, usize)>,
    pub(super) ends: super::ends::Ends,
}

impl Problem {
    pub(super) fn new(
        project: &Project,
        fixed_people: &HashSet<usize>,
        prior: Option<(&BTreeMap<usize, String>, usize)>,
        start: Option<&BTreeMap<usize, String>>,
    ) -> Self {
        let seats = project.layout.seats();
        let n = seats.len();
        let class = distance_matrix(project, &seats);
        let neighbours = neighbours(&class, n);
        let people: Vec<usize> = project.attendees.iter().map(|a| a.id).collect();
        let index: HashMap<usize, usize> =
            people.iter().enumerate().map(|(i, p)| (*p, i)).collect();
        let placement_rules = placement_rules(project, &seats, &index);
        let placement = combine_placement(&placement_rules, people.len(), n);
        let terms = compile_terms(project, &index);
        let terms_of = terms_of(&terms, people.len());
        let fairness = Fairness::new(&terms, people.len(), project.policy.left_out_penalty);
        let active = project.active_people();
        let groups = crate::groups::wish_groups(project)
            .into_iter()
            .map(|g| g.iter().filter_map(|p| index.get(p).copied()).collect())
            .collect();
        let prior = prior.map(|(assignment, min_changed)| {
            (
                prior_seats(&people, &active, &seats, assignment),
                min_changed,
            )
        });
        let ends = super::ends::Ends::new(project, &seats, &index);
        let usable = usable(project, &seats, &placement, &people, fixed_people, start);
        let fixed: Vec<bool> = people.iter().map(|p| fixed_people.contains(p)).collect();
        let constrained: Vec<bool> = people.iter().map(|p| active.contains(p)).collect();
        let org = organization_sets(project, &people);
        let swap_key = (0..people.len())
            .map(|p| (!constrained[p] && !fixed[p]).then(|| set_id(&org, p)))
            .collect();
        let mut problem = Self {
            hard_cost: project.score_radix(),
            ends,
            fixed,
            constrained,
            swap_key,
            placement_rules,
            seat_ids: seats.iter().map(|s| s.id.clone()).collect(),
            seat_spot: seat_spots(&seats),
            table_rows: table_rows(&seats),
            n_seats: n,
            class,
            neighbours,
            placement,
            terms,
            terms_of,
            fairness,
            org,
            org_mode: project.policy.organization,
            org_weight: project.policy.organization_weight,
            sprinkle: project.policy.sprinkle,
            isolation: project.policy.isolation_penalty,
            may_isolate: project.policy.isolation_penalty != 0 && n > people.len(),
            pair: Vec::new(),
            usable,
            seat_order: seat_order(&seats),
            groups,
            prior,
            people,
        };
        problem.fill_pair_costs();
        problem
    }

    /// Distance between two people's seats; `Far` if either has none.
    pub(super) fn distance(&self, seat_of: &[usize], a: usize, b: usize) -> DistanceClass {
        self.seat_distance(seat_of[a], seat_of[b])
    }

    /// Distance between two seats; `Far` if either is `NONE`.
    fn seat_distance(&self, x: usize, y: usize) -> DistanceClass {
        match (x, y) {
            (NONE, _) | (_, NONE) => DistanceClass::Far,
            (x, y) => self.class[x * self.n_seats + y],
        }
    }

    /// How a term fares: the distance that decides hard limits and the
    /// penalty. For a wish that is the `required`-th nearest target and the
    /// mean cost of the `required` nearest, so each target that comes closer
    /// counts. For an organization the nearest member decides instead, since
    /// a big one always has someone far away. For a group it is the widest
    /// break between its pieces.
    pub(super) fn term_outcome(
        &self,
        t: usize,
        seat_of: &[usize],
        scratch: &mut Scratch,
    ) -> (DistanceClass, i64) {
        let term = &self.terms[t];
        if let Shape::Block { stretch } = term.shape {
            let block = self.block_shape(t, seat_of, scratch);
            return (
                block.widest,
                block.gap_cost + block.extra_rows as i64 * stretch,
            );
        }
        let classes = &mut scratch.classes;
        classes.clear();
        classes.extend(
            term.targets
                .iter()
                .map(|&x| self.distance(seat_of, term.requester, x)),
        );
        let k = term.required;
        if k == 1 {
            let class = classes.iter().copied().min().unwrap_or(DistanceClass::Far);
            return (class, term.costs[class as usize]);
        }
        classes.sort_unstable();
        classes.resize(classes.len().max(k), DistanceClass::Far);
        if term.shape == Shape::Halving {
            // Halving weights: the nearest few decide, the rest add next to nothing.
            let mut weighted = 0;
            let mut total_weight = 0;
            for (rank, class) in classes[..k].iter().take(DECAY_RANKS).enumerate() {
                let weight = 1 << (DECAY_RANKS - 1 - rank);
                weighted += term.costs[*class as usize] * weight;
                total_weight += weight;
            }
            (classes[0], weighted / total_weight)
        } else {
            let sum: i64 = classes[..k].iter().map(|c| term.costs[*c as usize]).sum();
            (classes[k - 1], sum / k as i64)
        }
    }

    /// How a group sits. The cheapest links joining everyone (a minimum
    /// spanning tree) cost nothing between neighbours, so only the breaks
    /// between pieces count, by how wide they are.
    pub(super) fn block_shape(
        &self,
        t: usize,
        seat_of: &[usize],
        scratch: &mut Scratch,
    ) -> BlockShape {
        let term = &self.terms[t];
        let first = seat_of[term.requester];
        let mut block = BlockShape {
            widest: DistanceClass::Beside,
            gap_cost: 0,
            pieces: 1,
            extra_rows: 0,
        };
        // Per member not yet joined: their seat and distance to the nearest joined one.
        let link = &mut scratch.link;
        link.clear();
        link.extend(term.targets.iter().map(|&p| {
            let seat = seat_of[p];
            (seat, self.seat_distance(first, seat))
        }));
        while let Some(next) = (0..link.len()).min_by_key(|&i| link[i].1) {
            let (seat, class) = link.swap_remove(next);
            block.widest = block.widest.max(class);
            block.gap_cost += term.costs[class as usize];
            if class > DistanceClass::Diagonal {
                block.pieces += 1;
            }
            for (other, c) in link.iter_mut() {
                *c = (*c).min(self.seat_distance(seat, *other));
            }
        }
        block.extra_rows = self.extra_rows(
            std::iter::once(term.requester).chain(term.targets.iter().copied()),
            seat_of,
            &mut scratch.spots,
        );
        block
    }

    /// Rows a group takes beyond what it needs, per table: two sides fit two
    /// people a row, and a pair or a four may take two rows.
    fn extra_rows(
        &self,
        members: impl Iterator<Item = usize>,
        seat_of: &[usize],
        spots: &mut Vec<(usize, usize)>,
    ) -> usize {
        spots.clear();
        spots.extend(
            members
                .filter(|&p| seat_of[p] != NONE)
                .map(|p| self.seat_spot[seat_of[p]]),
        );
        spots.sort_unstable();
        spots
            .chunk_by(|a, b| a.0 == b.0)
            .map(|table| {
                let span = table[table.len() - 1].1 - table[0].1 + 1;
                span.saturating_sub(table.len().div_ceil(2).max(2))
            })
            .sum()
    }

    /// Extra cost if none of a requester's wishes is beside, opposite or diagonal.
    pub(super) fn left_out_cost(&self, g: usize, seat_of: &[usize], scratch: &mut Scratch) -> i64 {
        let near = self.fairness.groups[g]
            .iter()
            .any(|&t| self.term_outcome(t, seat_of, scratch).0 <= DistanceClass::Diagonal);
        if near { 0 } else { self.fairness.penalty }
    }

    pub(super) fn term_cost(&self, t: usize, seat_of: &[usize], scratch: &mut Scratch) -> i64 {
        let (class, penalty) = self.term_outcome(t, seat_of, scratch);
        penalty
            + if self.terms[t].forbidden[class as usize] {
                self.hard_cost
            } else {
                0
            }
    }

    /// Placement cost of `p` on seat `s`; nothing for someone without a seat.
    pub(super) fn placement_cost(&self, p: usize, s: usize) -> i64 {
        if s == NONE {
            0
        } else {
            self.placement[p][s].unwrap_or(self.hard_cost)
        }
    }

    /// Seating-policy cost of two people sitting beside or opposite each other.
    pub(super) fn pair_cost(&self, a: usize, b: usize) -> i64 {
        self.pair[a * self.people.len() + b]
    }

    fn fill_pair_costs(&mut self) {
        let n = self.people.len();
        self.pair = (0..n * n)
            .map(|i| self.organization_cost(i / n, i % n) + self.sprinkle_cost(i / n, i % n))
            .collect();
    }

    fn organization_cost(&self, a: usize, b: usize) -> i64 {
        if self.constrained[a] && self.constrained[b] {
            return 0;
        }
        let same = shares(&self.org[a], &self.org[b]);
        match self.org_mode {
            OrganizationMixing::SameOrganization if !same => self.org_weight,
            OrganizationMixing::Mixed if same => self.org_weight,
            _ => 0,
        }
    }

    fn sprinkle_cost(&self, a: usize, b: usize) -> i64 {
        match (!self.constrained[a], !self.constrained[b]) {
            (true, true) if self.sprinkle > 0 => self.sprinkle,
            (true, false) | (false, true) if self.sprinkle < 0 => -self.sprinkle,
            _ => 0,
        }
    }

    /// The seating-policy part of a state's score, broken down for display.
    pub(super) fn policy_report(&self, occupant: &[usize]) -> PolicyReport {
        let mut report = PolicyReport::default();
        for s in 0..self.n_seats {
            let a = occupant[s];
            if a == NONE {
                continue;
            }
            for &n in &self.neighbours[s] {
                let b = occupant[n];
                if b != NONE && n > s {
                    report.organization += self.organization_cost(a, b);
                    report.sprinkle += self.sprinkle_cost(a, b);
                }
            }
            if self.isolated_cost(s, occupant) > 0 {
                report.isolated.push(self.people[a]);
                report.isolation += self.isolation;
            }
        }
        report
    }

    /// Converts an assignment by attendee ID to per-person seat indices.
    pub(super) fn state_of(
        &self,
        assignment: &BTreeMap<usize, String>,
    ) -> (Vec<usize>, Vec<usize>) {
        let seat_index: HashMap<&str, usize> = self
            .seat_ids
            .iter()
            .enumerate()
            .map(|(i, s)| (s.as_str(), i))
            .collect();
        let seat_of: Vec<usize> = self
            .people
            .iter()
            .map(|p| {
                assignment
                    .get(p)
                    .and_then(|s| seat_index.get(s.as_str()).copied())
                    .unwrap_or(NONE)
            })
            .collect();
        let mut occupant = vec![NONE; self.n_seats];
        for (p, &s) in seat_of.iter().enumerate() {
            if s != NONE {
                occupant[s] = p;
            }
        }
        (seat_of, occupant)
    }

    pub(super) fn isolated_cost(&self, seat: usize, occupant: &[usize]) -> i64 {
        if !self.may_isolate
            || occupant[seat] == NONE
            || self.neighbours[seat].iter().any(|&n| occupant[n] != NONE)
        {
            0
        } else {
            self.isolation
        }
    }

    pub(super) fn diversity_cost(&self, changed: usize) -> i64 {
        match &self.prior {
            Some((_, min_changed)) if changed < *min_changed => {
                (*min_changed - changed) as i64 * 5_000
            }
            _ => 0,
        }
    }

    pub(super) fn changed(&self, seat_of: &[usize]) -> usize {
        match &self.prior {
            Some((before, _)) => before
                .iter()
                .zip(seat_of)
                .filter(|(b, s)| **b != NONE && *b != *s)
                .count(),
            None => 0,
        }
    }

    /// Full cost of a state (used to initialise and to verify deltas).
    pub(super) fn cost(&self, seat_of: &[usize], occupant: &[usize]) -> i64 {
        let mut scratch = Scratch::default();
        let mut total = 0;
        for (p, &s) in seat_of.iter().enumerate() {
            total += self.placement_cost(p, s);
        }
        for t in 0..self.terms.len() {
            total += self.term_cost(t, seat_of, &mut scratch);
        }
        for g in 0..self.fairness.groups.len() {
            total += self.left_out_cost(g, seat_of, &mut scratch);
        }
        total
            + self.policy_report(occupant).total()
            + self.ends.total(occupant)
            + self.diversity_cost(self.changed(seat_of))
    }

    /// The best any seating could score: every wish at its cheapest distance.
    pub(super) fn lower_bound(&self) -> i64 {
        let wishes: i64 = self
            .terms
            .iter()
            .map(|t| t.costs.iter().copied().min().unwrap_or(0))
            .sum();
        let placement: i64 = self
            .placement
            .iter()
            .map(|row| {
                row.iter()
                    .flatten()
                    .copied()
                    .min()
                    .unwrap_or(self.hard_cost)
            })
            .sum();
        wishes + placement
    }
}

/// Seats that may be used: spare seats sit at table bottoms, never where
/// people restricted by hard rules would run out of room or where someone
/// who may not move already sits.
fn usable(
    project: &Project,
    seats: &[Seat],
    placement: &[Vec<Option<i64>>],
    people: &[usize],
    fixed: &HashSet<usize>,
    start: Option<&BTreeMap<usize, String>>,
) -> Vec<bool> {
    let allowed: Vec<Vec<bool>> = placement
        .iter()
        .filter(|row| row.iter().any(Option::is_none))
        .map(|row| row.iter().map(Option::is_some).collect())
        .collect();
    let index: HashMap<&str, usize> = seats
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    let held = people
        .iter()
        .filter(|p| fixed.contains(p))
        .filter_map(|p| index.get(start?.get(p)?.as_str()).copied())
        .collect();
    super::spare_seats::usable_seats(project, seats, &allowed, &held)
}

/// Row-major `n × n` distance classes between seats; a seat is `Beside` itself.
fn distance_matrix(project: &Project, seats: &[Seat]) -> Vec<DistanceClass> {
    let n = seats.len();
    let mut class = Vec::with_capacity(n * n);
    for (i, a) in seats.iter().enumerate() {
        for (j, b) in seats.iter().enumerate() {
            class.push(if i == j {
                DistanceClass::Beside
            } else {
                table_distance(a, b)
            });
        }
    }
    let index: HashMap<&str, usize> = seats
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    for relation in &project.layout.relations {
        if let (Some(&a), Some(&b)) = (
            index.get(relation.a.as_str()),
            index.get(relation.b.as_str()),
        ) {
            class[a * n + b] = relation.class;
            class[b * n + a] = relation.class;
        }
    }
    class
}

/// Per seat, the other seats beside or opposite it.
fn neighbours(class: &[DistanceClass], n: usize) -> Vec<Vec<usize>> {
    (0..n)
        .map(|i| {
            (0..n)
                .filter(|&j| j != i && class[i * n + j] <= DistanceClass::Opposite)
                .collect()
        })
        .collect()
}

/// Each enabled placement rule's cost per seat.
fn placement_rules(
    project: &Project,
    seats: &[Seat],
    index: &HashMap<usize, usize>,
) -> Vec<PlacementRule> {
    project
        .rules
        .iter()
        .filter(|r| r.enabled)
        .filter_map(|rule| {
            let RuleKind::Placement {
                person,
                area,
                must_be,
                hard,
                penalty,
            } = &rule.kind
            else {
                return None;
            };
            let cost = seats
                .iter()
                .map(|seat| match area_contains(area, seat, seats) == *must_be {
                    true => Some(0),
                    false if *hard => None,
                    false => Some(*penalty),
                })
                .collect();
            Some(PlacementRule {
                rule_id: rule.id,
                person: *index.get(person)?,
                cost,
            })
        })
        .collect()
}

/// Per person, per seat: the sum of their placement rules.
fn combine_placement(
    rules: &[PlacementRule],
    n_people: usize,
    n_seats: usize,
) -> Vec<Vec<Option<i64>>> {
    let mut placement = vec![vec![Some(0); n_seats]; n_people];
    for rule in rules {
        for (total, cost) in placement[rule.person].iter_mut().zip(&rule.cost) {
            *total = total.zip(*cost).map(|(a, b)| a + b);
        }
    }
    placement
}

/// Per seat: its table's index and its row at that table.
fn seat_spots(seats: &[Seat]) -> Vec<(usize, usize)> {
    let mut tables: HashMap<&str, usize> = HashMap::new();
    seats
        .iter()
        .map(|s| {
            let next = tables.len();
            (*tables.entry(&s.table_id).or_insert(next), s.index)
        })
        .collect()
}

/// Per table, per row: its left and right seat, `NONE` where a side has no seat.
fn table_rows(seats: &[Seat]) -> Vec<Vec<[usize; 2]>> {
    let spots = seat_spots(seats);
    let mut tables: Vec<Vec<[usize; 2]>> = Vec::new();
    for (s, (seat, &(table, row))) in seats.iter().zip(&spots).enumerate() {
        if tables.len() <= table {
            tables.resize(table + 1, Vec::new());
        }
        let rows = &mut tables[table];
        if rows.len() <= row {
            rows.resize(row + 1, [NONE; 2]);
        }
        rows[row][usize::from(seat.side == Side::Right)] = s;
    }
    tables
}

/// Wishes and the planner's preferences as terms.
fn compile_terms(project: &Project, index: &HashMap<usize, usize>) -> Vec<Term> {
    let wishes = project
        .rules
        .iter()
        .filter(|r| r.enabled)
        .filter_map(|rule| wish_term(project, rule, index));
    let groups = project
        .active_preferences()
        .filter_map(|preference| group_term(preference, index));
    let pairs = project.active_preferences().flat_map(|preference| {
        preference.pair_terms().filter_map(move |t| {
            Some(Term {
                requester: *index.get(&t.a)?,
                targets: vec![*index.get(&t.b)?],
                required: 1,
                costs: t.costs,
                forbidden: t.forbidden,
                shape: Shape::Nearest,
                source: Source::Preference(preference.id),
            })
        })
    });
    wishes.chain(groups).chain(pairs).collect()
}

/// A "together" preference as one block term over its members.
fn group_term(
    preference: &crate::preferences::Preference,
    index: &HashMap<usize, usize>,
) -> Option<Term> {
    let costs = preference.group_costs()?;
    let mut members = preference
        .kind
        .people()
        .iter()
        .filter_map(|p| index.get(p).copied());
    let requester = members.next()?;
    let targets: Vec<usize> = members.collect();
    Some(Term {
        requester,
        required: targets.len(),
        targets,
        costs: costs.gap,
        forbidden: costs.forbidden,
        shape: Shape::Block {
            stretch: costs.stretch,
        },
        source: Source::Preference(preference.id),
    })
}

/// The weighted term of a proximity rule, or `None` if it is not one.
fn wish_term(project: &Project, rule: &Rule, index: &HashMap<usize, usize>) -> Option<Term> {
    let RuleKind::Proximity {
        requester,
        target,
        max_class,
        ..
    } = &rule.kind
    else {
        return None;
    };
    let targets = project
        .target_ids(*requester, target)
        .iter()
        .filter_map(|t| index.get(t).copied())
        .collect::<Vec<_>>();
    let shape = match target {
        crate::model::Target::Organization { .. } => Shape::Halving,
        _ => Shape::Nearest,
    };
    // An organization wish means being near the organization group: every
    // matching member counts.
    let required = target.required().min(targets.len());
    Some(Term {
        requester: *index.get(requester)?,
        targets,
        // Never more than there are: a removed person can't make it unmeetable.
        required: required.max(1),
        costs: project.wish_costs(rule)?,
        forbidden: DistanceClass::ALL.map(|c| max_class.is_some_and(|max| c > max)),
        shape,
        source: Source::Rule(rule.id),
    })
}

/// Per person, the terms they request or are a target of.
fn terms_of(terms: &[Term], n_people: usize) -> Vec<Vec<usize>> {
    let mut terms_of = vec![Vec::new(); n_people];
    for (t, term) in terms.iter().enumerate() {
        terms_of[term.requester].push(t);
        for &target in &term.targets {
            if !terms_of[target].contains(&t) {
                terms_of[target].push(t);
            }
        }
    }
    terms_of
}

/// Per person, the sorted IDs of their organizations (someone can belong to several).
fn organization_sets(project: &Project, people: &[usize]) -> Vec<Vec<u32>> {
    let mut ids: HashMap<String, u32> = HashMap::new();
    people
        .iter()
        .map(|p| {
            let keys = project
                .person(*p)
                .map(|a| project.category_keys(crate::catalog::Kind::Organization, a))
                .unwrap_or_default();
            let mut set: Vec<u32> = keys
                .into_iter()
                .map(|key| {
                    let next = ids.len() as u32;
                    *ids.entry(key).or_insert(next)
                })
                .collect();
            set.sort_unstable();
            set.dedup();
            set
        })
        .collect()
}

/// Whether two sorted ID lists have an element in common.
fn shares(a: &[u32], b: &[u32]) -> bool {
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Equal => return true,
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
        }
    }
    false
}

/// The first person with the same organizations as `p`, as an ID for that set.
fn set_id(org: &[Vec<u32>], p: usize) -> usize {
    org.iter().position(|o| *o == org[p]).unwrap_or(p)
}

/// Seats ordered by table, then position, so groups fill contiguous blocks.
fn seat_order(seats: &[Seat]) -> Vec<usize> {
    fn key(s: &Seat) -> (&str, usize, bool) {
        (&s.table_id, s.index, s.side == Side::Right)
    }
    let mut order: Vec<usize> = (0..seats.len()).collect();
    order.sort_by(|&a, &b| key(&seats[a]).cmp(&key(&seats[b])));
    order
}

/// Each constrained person's seat index in a previous assignment, else `NONE`.
fn prior_seats(
    people: &[usize],
    active: &HashSet<usize>,
    seats: &[Seat],
    assignment: &BTreeMap<usize, String>,
) -> Vec<usize> {
    let seat_index: HashMap<&str, usize> = seats
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    people
        .iter()
        .map(|p| {
            if active.contains(p) {
                assignment
                    .get(p)
                    .and_then(|s| seat_index.get(s.as_str()).copied())
                    .unwrap_or(NONE)
            } else {
                NONE
            }
        })
        .collect()
}
