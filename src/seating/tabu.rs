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

use crate::model::Project;

use super::problem::{NONE, Problem, Scratch};

/// Mutable search state with scratch buffers for incremental evaluation.
#[derive(Clone)]
pub(super) struct State {
    pub(super) seat_of: Vec<usize>,
    pub(super) occupant: Vec<usize>,
    pub(super) cost: i64,
    changed: usize,
}

struct Evaluator {
    stamp: Vec<u32>,
    epoch: u32,
    terms: Vec<usize>,
    seats: Vec<usize>,
    scratch: Scratch,
}

impl Evaluator {
    fn new(problem: &Problem) -> Self {
        Self {
            stamp: vec![0; problem.terms.len().max(problem.n_seats)],
            epoch: 0,
            terms: Vec::new(),
            seats: Vec::new(),
            scratch: Scratch::default(),
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

    /// Cost of everything that swapping the contents of seats x and y can change.
    fn local(&mut self, pr: &Problem, st: &State, x: usize, y: usize) -> i64 {
        let (a, b) = (st.occupant[x], st.occupant[y]);
        let mut total = 0;
        for (p, s) in [(a, x), (b, y)] {
            if p != NONE {
                total += pr.placement_cost(p, s);
            }
        }
        // Wish terms touching either person.
        self.next_round();
        self.terms.clear();
        for p in [a, b].into_iter().filter(|p| *p != NONE) {
            for &t in &pr.terms_of[p] {
                if self.fresh(t) {
                    self.terms.push(t);
                }
            }
        }
        for i in 0..self.terms.len() {
            total += pr.term_cost(self.terms[i], &st.seat_of, &mut self.scratch);
        }
        // "Nobody loses every wish" for the requesters of those terms.
        if !pr.fairness.groups.is_empty() {
            self.next_round();
            for i in 0..self.terms.len() {
                if let Some(g) = pr.fairness.of_term[self.terms[i]]
                    && self.fresh(g)
                {
                    total += pr.left_out_cost(g, &st.seat_of, &mut self.scratch);
                }
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

fn delta(pr: &Problem, st: &mut State, ev: &mut Evaluator, x: usize, y: usize) -> i64 {
    let before = ev.local(pr, st, x, y) + pr.diversity_cost(st.changed);
    swap(pr, st, x, y);
    let after = ev.local(pr, st, x, y) + pr.diversity_cost(st.changed);
    swap(pr, st, x, y);
    after - before
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
    State {
        seat_of,
        occupant,
        cost,
        changed,
    }
}

/// One tabu search run with breakout perturbations.
pub(super) struct Run {
    state: State,
    pub(super) best: State,
    tabu: Vec<u64>,
    pub(super) iteration: u64,
    stall: u64,
    breakouts: usize,
    evaluator: Evaluator,
    moves: Vec<usize>,
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
            state,
        }
    }

    pub(super) fn step(&mut self, pr: &Problem, rng: &mut impl Rng) {
        self.iteration += 1;
        let it = self.iteration;
        let n_seats = pr.n_seats;
        let mut best_move: Option<(i64, usize, usize)> = None;
        let mut ties = 0u32;
        for i in 0..self.moves.len() {
            for j in (i + 1)..self.moves.len() {
                let (x, y) = (self.moves[i], self.moves[j]);
                if !valid_swap(pr, &self.state, x, y) {
                    continue;
                }
                let d = delta(pr, &mut self.state, &mut self.evaluator, x, y);
                let (a, b) = (self.state.occupant[x], self.state.occupant[y]);
                let tabu = (a != NONE && self.tabu[a * n_seats + y] > it)
                    || (b != NONE && self.tabu[b * n_seats + x] > it);
                if tabu && self.state.cost + d >= self.best.cost {
                    continue;
                }
                match best_move {
                    Some((bd, _, _)) if d > bd => {}
                    Some((bd, _, _)) if d == bd => {
                        ties += 1;
                        if rng.random_range(0..=ties) == 0 {
                            best_move = Some((d, x, y));
                        }
                    }
                    _ => {
                        ties = 0;
                        best_move = Some((d, x, y));
                    }
                }
            }
        }
        let block = self.best_block_move(pr, rng);
        let swaps: Vec<(usize, usize)> = match (best_move, block) {
            (Some((d, _, _)), Some((bd, pairs))) if bd < d => pairs.to_vec(),
            (None, Some((_, pairs))) => pairs.to_vec(),
            (Some((_, x, y)), _) => vec![(x, y)],
            (None, None) => {
                self.breakout(pr, rng);
                return;
            }
        };
        let tenure = 5 + rng.random_range(0..=(pr.people.len() as u64 / 8).max(3));
        for (x, y) in swaps {
            let (a, b) = (self.state.occupant[x], self.state.occupant[y]);
            if a != NONE {
                self.tabu[a * n_seats + x] = it + tenure;
            }
            if b != NONE {
                self.tabu[b * n_seats + y] = it + tenure;
            }
            let d = delta(pr, &mut self.state, &mut self.evaluator, x, y);
            swap(pr, &mut self.state, x, y);
            self.state.cost += d;
        }
        if self.state.cost < self.best.cost {
            self.best = self.state.clone();
            self.stall = 0;
            self.breakouts = 0;
        } else {
            self.stall += 1;
        }
        if self.stall > (pr.people.len() as u64 / 2).max(60) {
            self.breakout(pr, rng);
        }
    }

    /// The best of a sample of block moves: two neighbouring seats swap
    /// contents with two other neighbouring seats, so pairs (couples, small
    /// groups) move together instead of being torn apart on the way.
    fn best_block_move(
        &mut self,
        pr: &Problem,
        rng: &mut impl Rng,
    ) -> Option<(i64, [(usize, usize); 2])> {
        let mut best: Option<(i64, [(usize, usize); 2])> = None;
        for _ in 0..self.moves.len() {
            let x1 = self.moves[rng.random_range(0..self.moves.len())];
            let y1 = self.moves[rng.random_range(0..self.moves.len())];
            let (Some(&x2), Some(&y2)) =
                (pr.neighbours[x1].choose(rng), pr.neighbours[y1].choose(rng))
            else {
                continue;
            };
            let distinct = x1 != y1 && x1 != y2 && x2 != y1 && x2 != y2 && x1 != x2 && y1 != y2;
            if !distinct || !valid_swap(pr, &self.state, x1, y1) {
                continue;
            }
            let d1 = delta(pr, &mut self.state, &mut self.evaluator, x1, y1);
            swap(pr, &mut self.state, x1, y1);
            let total = valid_swap(pr, &self.state, x2, y2)
                .then(|| d1 + delta(pr, &mut self.state, &mut self.evaluator, x2, y2));
            swap(pr, &mut self.state, x1, y1);
            if let Some(total) = total
                && best.is_none_or(|(b, _)| total < b)
            {
                best = Some((total, [(x1, y1), (x2, y2)]));
            }
        }
        best
    }

    /// Restarts from the best solution with a growing number of random swaps.
    fn breakout(&mut self, pr: &Problem, rng: &mut impl Rng) {
        self.breakouts += 1;
        self.state = self.best.clone();
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
    let pr = Problem::new(project, &project.pinned_people(), None, Some(assignment));
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
    };
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
        swap(&pr, &mut st, x, y);
        st.cost += d;
        assert_eq!(
            st.cost,
            pr.cost(&st.seat_of, &st.occupant),
            "incremental delta drifted"
        );
    }
}
