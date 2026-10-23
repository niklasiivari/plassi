//! Tabu search with incremental move evaluation, a group-aware starting
//! construction and breakout perturbations.
//!
//! Follows the seating-plan and quadratic-assignment literature (Lewis &
//! Carroll 2016; Taillard's robust tabu search; breakout local search): every
//! iteration evaluates all seat swaps, takes the best one that is not tabu,
//! forbids moving people straight back for a while, and shakes the best
//! solution up when progress stalls.

use std::collections::BTreeMap;

use rand::Rng;
use rand::seq::{IndexedRandom, SliceRandom};

use crate::model::{DistanceClass, Project};

use super::problem::{NONE, Problem, Scratch};

/// Mutable search state with scratch buffers for incremental evaluation.
#[derive(Clone)]
pub(super) struct State {
    pub(super) seat_of: Vec<usize>,
    pub(super) occupant: Vec<usize>,
    pub(super) cost: i64,
    changed: usize,
    /// Outcomes for the current seating, retained across candidate probes.
    outcomes: Vec<(DistanceClass, i64)>,
    /// Number of nearby wishes per fairness group.
    near: Vec<usize>,
}

impl State {
    fn refresh(&mut self, pr: &Problem) {
        let mut scratch = Scratch::default();
        self.outcomes = (0..pr.terms.len())
            .map(|t| pr.term_outcome(t, &self.seat_of, &mut scratch))
            .collect();
        self.near = pr
            .fairness
            .groups
            .iter()
            .map(|terms| {
                terms
                    .iter()
                    .filter(|&&t| self.outcomes[t].0 <= DistanceClass::Diagonal)
                    .count()
            })
            .collect();
    }
}

struct Evaluator {
    stamp: Vec<u32>,
    epoch: u32,
    terms: Vec<usize>,
    seats: Vec<usize>,
    scratch: Scratch,
    outcomes: Vec<(DistanceClass, i64)>,
    groups: Vec<usize>,
    near_delta: Vec<isize>,
}

impl Evaluator {
    fn new(problem: &Problem) -> Self {
        Self {
            stamp: vec![0; problem.terms.len().max(problem.n_seats)],
            epoch: 0,
            terms: Vec::new(),
            seats: Vec::new(),
            scratch: Scratch::default(),
            outcomes: Vec::new(),
            groups: Vec::new(),
            near_delta: vec![0; problem.fairness.groups.len()],
        }
    }

    /// Starts a new round of `fresh` checks.
    fn next_round(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        if self.epoch == 0 {
            self.stamp.fill(0);
            self.epoch = 1;
        }
    }

    /// Whether `i` (a term or seat) is seen for the first time this round.
    fn fresh(&mut self, i: usize) -> bool {
        let first = self.stamp[i] != self.epoch;
        self.stamp[i] = self.epoch;
        first
    }

    /// Terms touching either occupant; this set is the same before and after a swap.
    fn affected(&mut self, pr: &Problem, st: &State, x: usize, y: usize) {
        self.next_round();
        self.terms.clear();
        for p in [st.occupant[x], st.occupant[y]]
            .into_iter()
            .filter(|p| *p != NONE)
        {
            for &t in &pr.terms_of[p] {
                if self.fresh(t) {
                    self.terms.push(t);
                }
            }
        }
    }

    /// Local costs other than wishes, whose current outcomes are cached in the state.
    fn local_policy(&mut self, pr: &Problem, st: &State, x: usize, y: usize) -> i64 {
        let mut total = 0;
        for (p, s) in [(st.occupant[x], x), (st.occupant[y], y)] {
            if p != NONE {
                total += pr.placement_cost(p, s);
            }
        }
        // Neighbour pairs at x and y (the x–y pair once).
        for s in [x, y] {
            let p = st.occupant[s];
            if p == NONE {
                continue;
            }
            for &n in &pr.neighbours[s] {
                let q = st.occupant[n];
                if q != NONE && !(s == y && n == x) {
                    total += pr.pair_cost(p, q);
                }
            }
        }
        // Isolation around both seats.
        if !pr.may_isolate {
            return total + self.ends_cost(pr, st, x, y);
        }
        self.next_round();
        self.seats.clear();
        for s in [x, y]
            .into_iter()
            .chain(pr.neighbours[x].iter().copied())
            .chain(pr.neighbours[y].iter().copied())
        {
            if self.fresh(s) {
                self.seats.push(s);
            }
        }
        for i in 0..self.seats.len() {
            total += pr.isolated_cost(self.seats[i], &st.occupant);
        }
        total + self.ends_cost(pr, st, x, y)
    }

    /// Table-end costs of everyone within reach of seats x and y.
    fn ends_cost(&mut self, pr: &Problem, st: &State, x: usize, y: usize) -> i64 {
        if pr.ends.is_empty() {
            return 0;
        }
        self.next_round();
        let mut total = 0;
        for &s in pr.ends.reach[x].iter().chain(&pr.ends.reach[y]) {
            if self.fresh(s) {
                total += pr.ends.cost_at(s, &st.occupant);
            }
        }
        total
    }
}

fn swap(pr: &Problem, st: &mut State, x: usize, y: usize) {
    let (a, b) = (st.occupant[x], st.occupant[y]);
    let changed_before = |p: usize, s: usize| match &pr.prior {
        Some((before, _)) if p != NONE && before[p] != NONE => usize::from(before[p] != s),
        _ => 0,
    };
    st.changed -= changed_before(a, x) + changed_before(b, y);
    st.occupant[x] = b;
    st.occupant[y] = a;
    if a != NONE {
        st.seat_of[a] = y;
    }
    if b != NONE {
        st.seat_of[b] = x;
    }
    st.changed += changed_before(a, y) + changed_before(b, x);
}

/// Evaluates a swap against cached current outcomes. With `keep`, updates
/// those caches and leaves the swap in place; probes restore the seating.
fn evaluate_swap(
    pr: &Problem,
    st: &mut State,
    ev: &mut Evaluator,
    x: usize,
    y: usize,
    keep: bool,
) -> i64 {
    ev.affected(pr, st, x, y);
    let before = ev.local_policy(pr, st, x, y) + pr.diversity_cost(st.changed);
    swap(pr, st, x, y);
    let after = ev.local_policy(pr, st, x, y) + pr.diversity_cost(st.changed);
    let mut change = after - before;
    ev.outcomes.clear();
    ev.groups.clear();
    ev.next_round();
    for i in 0..ev.terms.len() {
        let t = ev.terms[i];
        let outcome = pr.term_outcome(t, &st.seat_of, &mut ev.scratch);
        change += pr.cost_from_outcome(t, outcome) - pr.cost_from_outcome(t, st.outcomes[t]);
        ev.outcomes.push(outcome);
        if let Some(g) = pr.fairness.of_term[t] {
            if ev.fresh(g) {
                ev.groups.push(g);
                ev.near_delta[g] = 0;
            }
            ev.near_delta[g] += isize::from(outcome.0 <= DistanceClass::Diagonal)
                - isize::from(st.outcomes[t].0 <= DistanceClass::Diagonal);
        }
    }
    for &g in &ev.groups {
        let after = st.near[g]
            .checked_add_signed(ev.near_delta[g])
            .expect("nearby wish count");
        change += (i64::from(after == 0) - i64::from(st.near[g] == 0)) * pr.fairness.penalty;
        if keep {
            st.near[g] = after;
        }
    }
    if keep {
        for (&t, &outcome) in ev.terms.iter().zip(&ev.outcomes) {
            st.outcomes[t] = outcome;
        }
    } else {
        swap(pr, st, x, y);
    }
    change
}

fn delta(pr: &Problem, st: &mut State, ev: &mut Evaluator, x: usize, y: usize) -> i64 {
    evaluate_swap(pr, st, ev, x, y, false)
}

/// Two row blocks of the same height: (table, first row) each, and the height.
type RowBlocks = ((usize, usize), (usize, usize), usize);

/// Most rows a block swap moves at once.
const MAX_BLOCK_ROWS: usize = 4;

/// Row blocks depend only on table geometry. Keeping their row starts lets
/// a paused scan resume without rebuilding or walking earlier candidates.
struct RowBlockPairs {
    starts: [Vec<(usize, usize)>; MAX_BLOCK_ROWS],
    height: usize,
    a: usize,
    b: usize,
    remaining: usize,
}

impl RowBlockPairs {
    fn first_partner(starts: &[(usize, usize)], a: usize, height: usize) -> usize {
        let (table, row) = starts[a];
        starts.partition_point(|&b| b < (table, row + height))
    }

    fn next_start(&mut self) {
        self.a += 1;
        while self.height < MAX_BLOCK_ROWS && self.a >= self.starts[self.height].len() {
            self.height += 1;
            self.a = 0;
        }
        if self.height < MAX_BLOCK_ROWS {
            self.b = Self::first_partner(&self.starts[self.height], self.a, self.height + 1);
        }
    }
}

impl Iterator for RowBlockPairs {
    type Item = RowBlocks;

    fn next(&mut self) -> Option<Self::Item> {
        self.nth(0)
    }

    fn nth(&mut self, mut n: usize) -> Option<Self::Item> {
        if n >= self.remaining {
            self.remaining = 0;
            return None;
        }
        loop {
            let starts = &self.starts[self.height];
            let available = starts.len() - self.b;
            if n < available {
                let pair = (starts[self.a], starts[self.b + n], self.height + 1);
                self.b += n + 1;
                self.remaining -= n + 1;
                return Some(pair);
            }
            n -= available;
            self.remaining -= available;
            self.next_start();
        }
    }

    fn count(self) -> usize {
        self.remaining
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for RowBlockPairs {}

/// Every pair of non-overlapping row blocks, in the original height/table/row order.
fn row_block_pairs(pr: &Problem) -> RowBlockPairs {
    let starts: [Vec<_>; MAX_BLOCK_ROWS] = std::array::from_fn(|k| {
        pr.table_rows
            .iter()
            .enumerate()
            .flat_map(|(t, rows)| (0..rows.len().saturating_sub(k)).map(move |r| (t, r)))
            .collect()
    });
    let remaining = starts
        .iter()
        .enumerate()
        .map(|(k, starts)| {
            (0..starts.len())
                .map(|a| starts.len() - RowBlockPairs::first_partner(starts, a, k + 1))
                .sum::<usize>()
        })
        .sum();
    let b = if starts[0].is_empty() {
        0
    } else {
        RowBlockPairs::first_partner(&starts[0], 0, 1)
    };
    RowBlockPairs {
        starts,
        height: 0,
        a: 0,
        b,
        remaining,
    }
}

/// Seats that trade places in a row block swap: same row offset, same side.
fn seat_pairs(
    pr: &Problem,
    ((ta, ra), (tb, rb), k): RowBlocks,
) -> impl Iterator<Item = (usize, usize)> + '_ {
    (0..k).flat_map(move |i| {
        let (a, b) = (pr.table_rows[ta][ra + i], pr.table_rows[tb][rb + i]);
        [(a[0], b[0]), (a[1], b[1])]
    })
}

/// Who sits in a seat; nobody for a missing seat.
fn seat_occupant(st: &State, seat: usize) -> usize {
    if seat == NONE {
        NONE
    } else {
        st.occupant[seat]
    }
}

fn valid_swap(pr: &Problem, st: &State, x: usize, y: usize) -> bool {
    let (a, b) = (st.occupant[x], st.occupant[y]);
    if x == y || (a == NONE && b == NONE) {
        return false;
    }
    // Swapping two interchangeable people changes nothing.
    if a != NONE && b != NONE && pr.swap_key[a].is_some() && pr.swap_key[a] == pr.swap_key[b] {
        return false;
    }
    let ok = |p: usize, to: usize| {
        p == NONE || (!pr.fixed[p] && pr.placement[p][to].is_some() && pr.usable[to])
    };
    ok(a, y) && ok(b, x)
}

/// Group-aware randomised construction: pinned and restricted people first,
/// then wish groups in contiguous blocks, then everyone else.
pub(super) fn construct(pr: &Problem, rng: &mut impl Rng, start: Option<&[usize]>) -> State {
    let n_people = pr.people.len();
    let mut seat_of = vec![NONE; n_people];
    let mut occupant = vec![NONE; pr.n_seats];
    let place = |p: usize, s: usize, seat_of: &mut Vec<usize>, occupant: &mut Vec<usize>| {
        seat_of[p] = s;
        occupant[s] = p;
    };
    // Pinned people (exactly one allowed seat) take it before anyone else.
    for p in 0..n_people {
        let mut allowed = (0..pr.n_seats).filter(|&s| pr.placement[p][s].is_some());
        if let (Some(s), None) = (allowed.next(), allowed.next())
            && occupant[s] == NONE
        {
            place(p, s, &mut seat_of, &mut occupant);
        }
    }
    // Then the starting seating, where it is still allowed.
    if let Some(start) = start {
        for (p, &s) in start.iter().enumerate() {
            if seat_of[p] == NONE
                && s != NONE
                && occupant[s] == NONE
                && pr.placement[p][s].is_some()
            {
                place(p, s, &mut seat_of, &mut occupant);
            }
        }
    }
    // People with the fewest allowed seats first (pins have exactly one).
    let mut restricted: Vec<(usize, usize)> = (0..n_people)
        .filter(|&p| seat_of[p] == NONE)
        .map(|p| (pr.placement[p].iter().filter(|c| c.is_some()).count(), p))
        .filter(|(count, _)| *count < pr.n_seats)
        .collect();
    restricted.sort();
    for (_, p) in restricted {
        let mut options: Vec<usize> = (0..pr.n_seats)
            .filter(|&s| {
                occupant[s] == NONE && pr.placement[p][s].is_some() && (pr.usable[s] || pr.fixed[p])
            })
            .collect();
        options.shuffle(rng);
        if let Some(&s) = options.first() {
            place(p, s, &mut seat_of, &mut occupant);
        }
    }
    // Groups in blocks, starting at a random point of the seat order.
    let mut groups = pr.groups.clone();
    groups.shuffle(rng);
    groups.sort_by_key(|g| std::cmp::Reverse(g.len()));
    let offset = rng.random_range(0..pr.seat_order.len().max(1));
    for group in groups {
        let mut members: Vec<usize> = group.into_iter().filter(|&p| seat_of[p] == NONE).collect();
        members.shuffle(rng);
        let free: Vec<usize> = (0..pr.seat_order.len())
            .map(|i| pr.seat_order[(i + offset) % pr.seat_order.len()])
            .filter(|&s| occupant[s] == NONE && pr.usable[s])
            .collect();
        for (p, s) in members.into_iter().zip(free) {
            if pr.placement[p][s].is_some() {
                place(p, s, &mut seat_of, &mut occupant);
            }
        }
    }
    let mut rest: Vec<usize> = (0..n_people).filter(|&p| seat_of[p] == NONE).collect();
    rest.shuffle(rng);
    for p in rest {
        let mut free: Vec<usize> = (0..pr.n_seats)
            .filter(|&s| occupant[s] == NONE && pr.usable[s] && pr.placement[p][s].is_some())
            .collect();
        if free.is_empty() {
            free = (0..pr.n_seats).filter(|&s| occupant[s] == NONE).collect();
        }
        if let Some(&s) = free.choose(rng) {
            place(p, s, &mut seat_of, &mut occupant);
        }
    }
    let changed = pr.changed(&seat_of);
    let cost = pr.cost(&seat_of, &occupant);
    let mut state = State {
        seat_of,
        occupant,
        cost,
        changed,
        outcomes: Vec::new(),
        near: Vec::new(),
    };
    state.refresh(pr);
    state
}

type SingleMove = (i64, usize, usize);
type BlockMove = (i64, [(usize, usize); 2]);

struct SwapScan {
    i: usize,
    j: usize,
    best: Option<SingleMove>,
    ties: u32,
}

enum Pending {
    Swaps(SwapScan),
    Blocks {
        sample: usize,
        single: Option<SingleMove>,
        best: Option<BlockMove>,
    },
    Rows {
        pairs: RowBlockPairs,
        best: Option<(i64, RowBlocks)>,
    },
}

/// One tabu search run with breakout perturbations. A pending neighborhood
/// retains its exact candidate order and tie-breaking RNG state across yields.
pub(super) struct Run {
    state: State,
    pub(super) best: State,
    tabu: Vec<u64>,
    pub(super) iteration: u64,
    stall: u64,
    breakouts: usize,
    evaluator: Evaluator,
    moves: Vec<usize>,
    /// Swaps made while scoring a row block swap, to undo them.
    swapped: Vec<(usize, usize)>,
    pending: Option<Pending>,
}

impl Run {
    pub(super) fn new(pr: &Problem, state: State) -> Self {
        let moves: Vec<usize> = (0..pr.n_seats)
            .filter(|&s| pr.usable[s] || state.occupant[s] != NONE)
            .collect();
        Self {
            best: state.clone(),
            tabu: vec![0; pr.people.len() * pr.n_seats],
            iteration: 0,
            stall: 0,
            breakouts: 0,
            evaluator: Evaluator::new(pr),
            moves,
            swapped: Vec::new(),
            pending: None,
            state,
        }
    }

    #[cfg(test)]
    pub(super) fn step(&mut self, pr: &Problem, rng: &mut impl Rng) {
        assert!(self.advance(pr, rng, || true));
    }

    /// Evaluates complete neighborhoods, yielding between small batches of
    /// candidates. A yield never selects a move from an incomplete sample.
    pub(super) fn advance(
        &mut self,
        pr: &Problem,
        rng: &mut impl Rng,
        mut keep_going: impl FnMut() -> bool,
    ) -> bool {
        const BATCH: usize = 32;
        if !keep_going() {
            return false;
        }
        let mut pending = self.pending.take().unwrap_or_else(|| {
            self.iteration += 1;
            Pending::Swaps(SwapScan {
                i: 0,
                j: 1,
                best: None,
                ties: 0,
            })
        });
        let mut work = 0;
        loop {
            if work == BATCH {
                if !keep_going() {
                    self.pending = Some(pending);
                    return false;
                }
                work = 0;
            }
            work += 1;
            pending = match pending {
                Pending::Swaps(mut scan) => {
                    if scan.i + 1 >= self.moves.len() {
                        Pending::Blocks {
                            sample: 0,
                            single: scan.best,
                            best: None,
                        }
                    } else {
                        let (x, y) = (self.moves[scan.i], self.moves[scan.j]);
                        scan.j += 1;
                        if scan.j == self.moves.len() {
                            scan.i += 1;
                            scan.j = scan.i + 1;
                        }
                        self.consider_swap(pr, rng, &mut scan, x, y);
                        Pending::Swaps(scan)
                    }
                }
                Pending::Blocks {
                    sample,
                    single,
                    mut best,
                } => {
                    if sample < self.moves.len() {
                        if let Some(candidate) = self.sample_block_move(pr, rng)
                            && best.is_none_or(|(cost, _)| candidate.0 < cost)
                        {
                            best = Some(candidate);
                        }
                        Pending::Blocks {
                            sample: sample + 1,
                            single,
                            best,
                        }
                    } else if self.apply_best_move(pr, rng, single, best) {
                        self.state = self.best.clone();
                        Pending::Rows {
                            pairs: row_block_pairs(pr),
                            best: None,
                        }
                    } else {
                        return true;
                    }
                }
                Pending::Rows {
                    mut pairs,
                    mut best,
                } => {
                    if let Some(blocks) = pairs.next() {
                        if let Some(d) = self.row_block_delta(pr, blocks, false)
                            && best.is_none_or(|(cost, _)| d < cost)
                        {
                            best = Some((d, blocks));
                        }
                        Pending::Rows { pairs, best }
                    } else {
                        if let Some((d, blocks)) = best.filter(|(d, _)| *d < 0) {
                            self.row_block_delta(pr, blocks, true);
                            self.state.cost += d;
                            self.best = self.state.clone();
                            self.stall = 0;
                        } else {
                            self.breakout(pr, rng);
                        }
                        return true;
                    }
                }
            };
        }
    }

    fn consider_swap(
        &mut self,
        pr: &Problem,
        rng: &mut impl Rng,
        scan: &mut SwapScan,
        x: usize,
        y: usize,
    ) {
        if !valid_swap(pr, &self.state, x, y) {
            return;
        }
        let d = delta(pr, &mut self.state, &mut self.evaluator, x, y);
        let (a, b) = (self.state.occupant[x], self.state.occupant[y]);
        let tabu = (a != NONE && self.tabu[a * pr.n_seats + y] > self.iteration)
            || (b != NONE && self.tabu[b * pr.n_seats + x] > self.iteration);
        if tabu && self.state.cost + d >= self.best.cost {
            return;
        }
        match scan.best {
            Some((bd, _, _)) if d > bd => {}
            Some((bd, _, _)) if d == bd => {
                scan.ties += 1;
                if rng.random_range(0..=scan.ties) == 0 {
                    scan.best = Some((d, x, y));
                }
            }
            _ => {
                scan.ties = 0;
                scan.best = Some((d, x, y));
            }
        }
    }

    /// Applies the selected single or pair move. Returns whether stalled
    /// progress needs an exhaustive row-block scan from the best seating.
    fn apply_best_move(
        &mut self,
        pr: &Problem,
        rng: &mut impl Rng,
        single: Option<SingleMove>,
        block: Option<BlockMove>,
    ) -> bool {
        let (swaps, count) = match (single, block) {
            (Some((d, _, _)), Some((bd, pairs))) if bd < d => (pairs, 2),
            (None, Some((_, pairs))) => (pairs, 2),
            (Some((_, x, y)), _) => ([(x, y), (0, 0)], 1),
            (None, None) => {
                self.breakout(pr, rng);
                return false;
            }
        };
        let tenure = 5 + rng.random_range(0..=(pr.people.len() as u64 / 8).max(3));
        for &(x, y) in &swaps[..count] {
            let (a, b) = (self.state.occupant[x], self.state.occupant[y]);
            if a != NONE {
                self.tabu[a * pr.n_seats + x] = self.iteration + tenure;
            }
            if b != NONE {
                self.tabu[b * pr.n_seats + y] = self.iteration + tenure;
            }
            let d = evaluate_swap(pr, &mut self.state, &mut self.evaluator, x, y, true);
            self.state.cost += d;
        }
        if self.state.cost < self.best.cost {
            self.best = self.state.clone();
            self.stall = 0;
            self.breakouts = 0;
        } else {
            self.stall += 1;
        }
        self.stall > (pr.people.len() as u64 / 2).max(60)
    }

    /// Score change of swapping two row blocks, or `None` if someone in them
    /// may not sit in the other block or a seat has no counterpart. People
    /// who may not move (pinned) stay where they are, and so does whoever
    /// sits in the matching seat of the other block. With `keep` the swap
    /// stays made; otherwise it is undone.
    fn row_block_delta(&mut self, pr: &Problem, blocks: RowBlocks, keep: bool) -> Option<i64> {
        let mut total = Some(0);
        self.swapped.clear();
        for (x, y) in seat_pairs(pr, blocks) {
            let (a, b) = (seat_occupant(&self.state, x), seat_occupant(&self.state, y));
            let stays = |p: usize| p != NONE && pr.fixed[p];
            if (a == NONE && b == NONE) || stays(a) || stays(b) {
                continue;
            }
            if x == NONE || y == NONE || !valid_swap(pr, &self.state, x, y) {
                total = None;
                break;
            }
            let d = evaluate_swap(pr, &mut self.state, &mut self.evaluator, x, y, true);
            self.swapped.push((x, y));
            total = total.map(|t| t + d);
        }
        if total.is_none() || !keep {
            for &(x, y) in self.swapped.iter().rev() {
                evaluate_swap(pr, &mut self.state, &mut self.evaluator, x, y, true);
            }
        }
        total
    }

    /// One sample of a neighbouring-pair move, in the same random draw order
    /// whether the surrounding neighborhood scan is paused or uninterrupted.
    fn sample_block_move(&mut self, pr: &Problem, rng: &mut impl Rng) -> Option<BlockMove> {
        let x1 = self.moves[rng.random_range(0..self.moves.len())];
        let y1 = self.moves[rng.random_range(0..self.moves.len())];
        let (Some(&x2), Some(&y2)) = (pr.neighbours[x1].choose(rng), pr.neighbours[y1].choose(rng))
        else {
            return None;
        };
        let distinct = x1 != y1 && x1 != y2 && x2 != y1 && x2 != y2 && x1 != x2 && y1 != y2;
        if !distinct || !valid_swap(pr, &self.state, x1, y1) {
            return None;
        }
        let d1 = evaluate_swap(pr, &mut self.state, &mut self.evaluator, x1, y1, true);
        let total = valid_swap(pr, &self.state, x2, y2)
            .then(|| d1 + delta(pr, &mut self.state, &mut self.evaluator, x2, y2));
        evaluate_swap(pr, &mut self.state, &mut self.evaluator, x1, y1, true);
        total.map(|cost| (cost, [(x1, y1), (x2, y2)]))
    }

    /// Restarts from the best solution with a growing number of random swaps,
    /// every other time after swapping a random pair of row blocks, which
    /// moves groups without breaking them up.
    fn breakout(&mut self, pr: &Problem, rng: &mut impl Rng) {
        self.breakouts += 1;
        self.state = self.best.clone();
        if rng.random_bool(0.5) {
            let count = row_block_pairs(pr).count();
            if count > 0
                && let Some(blocks) = row_block_pairs(pr).nth(rng.random_range(0..count))
            {
                self.row_block_delta(pr, blocks, true);
            }
        }
        let strength = (2 + self.breakouts).min(pr.people.len() / 3 + 2);
        let mut done = 0;
        let mut attempts = 0;
        while done < strength && attempts < strength * 50 && self.moves.len() > 1 {
            attempts += 1;
            let x = self.moves[rng.random_range(0..self.moves.len())];
            let y = self.moves[rng.random_range(0..self.moves.len())];
            if valid_swap(pr, &self.state, x, y) {
                swap(pr, &mut self.state, x, y);
                done += 1;
            }
        }
        self.state.cost = pr.cost(&self.state.seat_of, &self.state.occupant);
        self.state.refresh(pr);
        self.stall = 0;
    }
}

/// Solver-score change of moving `person` to each other seat (swapping with
/// whoever sits there), best first: one incremental delta per seat.
pub fn move_deltas(
    project: &Project,
    assignment: &BTreeMap<usize, String>,
    person: usize,
) -> Vec<(String, i64)> {
    let pr = Problem::new(
        project,
        &project.seat_pinned_people(),
        None,
        Some(assignment),
    );
    let (seat_of, occupant) = pr.state_of(assignment);
    let Some(from) = pr
        .people
        .iter()
        .position(|id| *id == person)
        .map(|p| seat_of[p])
    else {
        return Vec::new();
    };
    if from == NONE {
        return Vec::new();
    }
    let mut st = State {
        seat_of,
        occupant,
        cost: 0,
        changed: 0,
        outcomes: Vec::new(),
        near: Vec::new(),
    };
    st.refresh(&pr);
    let mut ev = Evaluator::new(&pr);
    let mut deltas: Vec<_> = (0..pr.n_seats)
        .filter_map(|s| {
            let valid = valid_swap(&pr, &st, from, s);
            valid.then(|| {
                (
                    pr.seat_ids[s].clone(),
                    delta(&pr, &mut st, &mut ev, from, s),
                )
            })
        })
        .collect();
    deltas.sort_by_key(|d| d.1);
    deltas
}

#[cfg(test)]
pub(crate) fn check_deltas(project: &Project) {
    let pr = Problem::new(project, &Default::default(), None, None);
    let mut rng = rand::rng();
    let mut st = construct(&pr, &mut rng, None);
    let mut ev = Evaluator::new(&pr);
    for _ in 0..300 {
        let x = rng.random_range(0..pr.n_seats);
        let y = rng.random_range(0..pr.n_seats);
        if !valid_swap(&pr, &st, x, y) {
            continue;
        }
        let d = delta(&pr, &mut st, &mut ev, x, y);
        let committed = evaluate_swap(&pr, &mut st, &mut ev, x, y, true);
        assert_eq!(d, committed);
        st.cost += d;
        assert_eq!(
            st.cost,
            pr.cost(&st.seat_of, &st.occupant),
            "incremental delta drifted"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Importance, Layout, Rule, RuleKind, Target, WishKind};
    use crate::preferences::{Preference, PreferenceKind, Strength};
    use rand::{SeedableRng, rngs::StdRng};

    fn project() -> Project {
        let mut project = crate::tests::synthetic_project(24, 36);
        project.layout = Layout::simple(2, 8);
        project.preferences.push(Preference {
            id: 1,
            kind: PreferenceKind::Together {
                people: vec![0, 1, 2, 3, 4, 5],
            },
            strength: Strength::Important,
            enabled: true,
        });
        project.rules.push(Rule {
            id: 100,
            label: String::new(),
            source: String::new(),
            enabled: true,
            kind: RuleKind::Proximity {
                requester: 0,
                target: Target::Organization { name: "ABC".into() },
                wish: WishKind::Seating,
                importance: Importance::Normal,
                max_class: Some(DistanceClass::SameTable),
            },
        });
        project.rules.push(Rule {
            id: 101,
            label: String::new(),
            source: String::new(),
            enabled: true,
            kind: RuleKind::AvoidTableEnd {
                person: 1,
                end_penalty: 40,
                near_end_penalty: 10,
            },
        });
        project
    }

    fn assert_cache(pr: &Problem, state: &State) {
        let mut scratch = Scratch::default();
        for (t, &cached) in state.outcomes.iter().enumerate() {
            assert_eq!(cached, pr.term_outcome(t, &state.seat_of, &mut scratch));
        }
        for (g, terms) in pr.fairness.groups.iter().enumerate() {
            assert_eq!(
                state.near[g],
                terms
                    .iter()
                    .filter(|&&t| state.outcomes[t].0 <= DistanceClass::Diagonal)
                    .count()
            );
        }
    }

    #[test]
    fn cached_deltas_match_full_scores_and_preserve_probed_state() {
        let project = project();
        let mut rng = StdRng::seed_from_u64(5);
        let first = Problem::new(&project, &Default::default(), None, None);
        let start = construct(&first, &mut rng, None);
        let assignment: BTreeMap<_, _> = first
            .people
            .iter()
            .zip(&start.seat_of)
            .map(|(&p, &s)| (p, first.seat_ids[s].clone()))
            .collect();
        let pr = Problem::new(
            &project,
            &Default::default(),
            Some((&assignment, 5)),
            Some(&assignment),
        );
        let mut state = construct(&pr, &mut rng, None);
        let mut evaluator = Evaluator::new(&pr);
        for _ in 0..500 {
            let (x, y) = (
                rng.random_range(0..pr.n_seats),
                rng.random_range(0..pr.n_seats),
            );
            if !valid_swap(&pr, &state, x, y) {
                continue;
            }
            let before = state.clone();
            let d = delta(&pr, &mut state, &mut evaluator, x, y);
            assert_eq!(state.seat_of, before.seat_of);
            assert_eq!(state.occupant, before.occupant);
            assert_eq!(state.outcomes, before.outcomes);
            assert_eq!(state.near, before.near);
            assert_eq!(state.changed, before.changed);
            let old = pr.cost(&state.seat_of, &state.occupant);
            swap(&pr, &mut state, x, y);
            let new = pr.cost(&state.seat_of, &state.occupant);
            swap(&pr, &mut state, x, y);
            assert_eq!(d, new - old);
            let committed = evaluate_swap(&pr, &mut state, &mut evaluator, x, y, true);
            assert_eq!(d, committed);
            state.cost += committed;
            assert_eq!(state.cost, new);
            assert_cache(&pr, &state);
        }
    }

    #[test]
    fn row_block_probes_restore_all_caches() {
        let mut project = project();
        project.layout.rows[0].tables[1].right = 5;
        let pr = Problem::new(&project, &std::collections::HashSet::from([0]), None, None);
        let mut rng = StdRng::seed_from_u64(19);
        let state = construct(&pr, &mut rng, None);
        let mut run = Run::new(&pr, state);
        for blocks in row_block_pairs(&pr).take(200) {
            let before = run.state.clone();
            let delta = run.row_block_delta(&pr, blocks, false);
            assert_eq!(run.state.seat_of, before.seat_of);
            assert_eq!(run.state.outcomes, before.outcomes);
            assert_eq!(run.state.near, before.near);
            assert_cache(&pr, &run.state);
            if let Some(delta) = delta {
                run.row_block_delta(&pr, blocks, true);
                run.state.cost += delta;
                assert_eq!(
                    run.state.cost,
                    pr.cost(&run.state.seat_of, &run.state.occupant)
                );
                assert_cache(&pr, &run.state);
            }
        }
    }

    #[test]
    fn sliced_scans_preserve_moves_and_random_draws() {
        let project = project();
        let pr = Problem::new(&project, &Default::default(), None, None);
        let mut rng = StdRng::seed_from_u64(41);
        let state = construct(&pr, &mut rng, None);
        let mut whole = Run::new(&pr, state.clone());
        let mut sliced = Run::new(&pr, state);
        let mut sliced_rng = rng.clone();
        let mut yields = 0;
        for _ in 0..160 {
            whole.step(&pr, &mut rng);
            loop {
                let mut checks = 0;
                if sliced.advance(&pr, &mut sliced_rng, || {
                    checks += 1;
                    checks == 1
                }) {
                    break;
                }
                yields += 1;
            }
            assert_eq!(whole.state.seat_of, sliced.state.seat_of);
            assert_eq!(whole.state.cost, sliced.state.cost);
            assert_eq!(whole.best.seat_of, sliced.best.seat_of);
            assert_eq!(whole.tabu, sliced.tabu);
            assert_eq!(whole.stall, sliced.stall);
            assert_eq!(whole.breakouts, sliced.breakouts);
            assert_cache(&pr, &sliced.state);
        }
        assert!(yields > 160);
        assert_eq!(rng.random::<u64>(), sliced_rng.random::<u64>());
    }

    #[test]
    fn indexed_row_block_enumeration_preserves_order_and_sampling() {
        let project = project();
        let pr = Problem::new(&project, &Default::default(), None, None);
        let mut expected = Vec::new();
        for k in 1..=MAX_BLOCK_ROWS {
            let starts: Vec<_> = pr
                .table_rows
                .iter()
                .enumerate()
                .flat_map(|(t, rows)| (0..(rows.len() + 1).saturating_sub(k)).map(move |r| (t, r)))
                .collect();
            for &a in &starts {
                for &b in &starts {
                    if b > a && (b.0 != a.0 || b.1 >= a.1 + k) {
                        expected.push((a, b, k));
                    }
                }
            }
        }
        assert_eq!(row_block_pairs(&pr).collect::<Vec<_>>(), expected);
        assert_eq!(row_block_pairs(&pr).count(), expected.len());
        for (n, &pair) in expected.iter().enumerate() {
            let mut pairs = row_block_pairs(&pr);
            assert_eq!(pairs.nth(n), Some(pair));
            assert_eq!(pairs.len(), expected.len() - n - 1);
            assert_eq!(pairs.collect::<Vec<_>>(), expected[n + 1..]);
        }
        assert!(row_block_pairs(&pr).nth(expected.len()).is_none());
    }
}
