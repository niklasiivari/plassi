//! Per-frame view of the selected option: who sits where, rule results, score and gaps.

use super::*;

/// Everything the chart needs for one frame, derived from the project.
pub(super) struct Scene {
    pub(super) seats: Vec<Seat>,
    pub(super) occupant: HashMap<String, usize>,
    pub(super) seat_of: HashMap<usize, String>,
    pub(super) evaluation: Option<std::rc::Rc<Evaluation>>,
    /// Rule results owned by each requester (or placed person).
    pub(super) own: HashMap<usize, Vec<(u64, RuleResult)>>,
    pub(super) pins: HashMap<usize, Pin>,
    /// People off the seats but on the chart: their card's centre.
    pub(super) loose: BTreeMap<usize, Pos2>,
    /// Everyone without a seat, loose or not, in attendee order.
    pub(super) unseated: Vec<usize>,
    /// People seated differently in the option being compared against.
    pub(super) moved: HashSet<usize>,
    /// Empty seats with someone seated below them on the same side.
    pub(super) gaps: HashSet<String>,
    /// Wish penalty plus policy cost; lower is better.
    pub(super) score: i64,
}

impl Scene {
    pub(super) fn build(
        project: &Project,
        candidate: Option<usize>,
        compare: Option<usize>,
        evaluation: Option<std::rc::Rc<Evaluation>>,
    ) -> Self {
        let seats = project.layout.seats();
        let option = candidate.and_then(|i| project.candidates.get(i));
        let assignment = option.map(|c| c.assignment.clone()).unwrap_or_default();
        let loose = option
            .map(|c| {
                c.loose
                    .iter()
                    .map(|(p, at)| (*p, Pos2::new(at.x, at.y)))
                    .collect()
            })
            .unwrap_or_default();
        let unseated = option.map(|c| c.unseated(project)).unwrap_or_default();
        let occupant = assignment.iter().map(|(p, s)| (s.clone(), *p)).collect();
        let mut own: HashMap<usize, Vec<(u64, RuleResult)>> = HashMap::new();
        if let Some(evaluation) = &evaluation {
            let owners: HashMap<u64, usize> =
                project.rules.iter().map(|r| (r.id, r.owner())).collect();
            for result in &evaluation.results {
                if let Some(&owner) = owners.get(&result.rule_id) {
                    own.entry(owner)
                        .or_default()
                        .push((result.rule_id, result.clone()));
                }
            }
        }
        let moved = compare
            .filter(|c| Some(*c) != candidate)
            .and_then(|c| project.candidates.get(c))
            .map(|other| {
                assignment
                    .iter()
                    .filter(|(p, s)| other.assignment.get(p) != Some(s))
                    .map(|(p, _)| *p)
                    .collect()
            })
            .unwrap_or_default();
        let gaps = crate::model::gaps(&seats, &assignment)
            .into_iter()
            .map(|s| s.id.clone())
            .collect();
        let score = evaluation.as_ref().map_or(0, |e| e.penalty());
        Self {
            seats,
            occupant,
            seat_of: assignment.into_iter().collect(),
            evaluation,
            own,
            pins: project.pins(),
            loose,
            unseated,
            moved,
            score,
            gaps,
        }
    }

    /// World centre of a person's card: on their seat, or where they were left.
    pub(super) fn card_centre(&self, person: usize) -> Option<Pos2> {
        match self.seat_of.get(&person) {
            Some(id) => self.seat(id).map(|s| card_rect(s).center()),
            None => self.loose.get(&person).copied(),
        }
    }

    /// Where a person is: on a seat, loose at their card's centre, or neither.
    pub(super) fn spot_of(&self, person: usize) -> Spot {
        match (self.seat_of.get(&person), self.loose.get(&person)) {
            (Some(seat), _) => Spot::Seat(seat.clone()),
            (None, Some(at)) => Spot::Loose(crate::model::Point { x: at.x, y: at.y }),
            (None, None) => Spot::Unseated,
        }
    }

    pub(super) fn seat(&self, id: &str) -> Option<&Seat> {
        self.seats.iter().find(|s| s.id == id)
    }

    /// The seat whose card covers a world point.
    pub(super) fn seat_at(&self, world: Pos2) -> Option<&Seat> {
        self.seats.iter().find(|s| card_rect(s).contains(world))
    }

    /// The pin mark shown on a person's card.
    pub(super) fn pin_mark(&self, person: usize) -> Option<PinMark> {
        self.pins.get(&person).map(|pin| match pin {
            Pin::Seat(_) => PinMark::Seat,
            Pin::Table(_) => PinMark::Table,
        })
    }

    pub(super) fn policy(&self) -> Option<&PolicyReport> {
        Some(&self.evaluation.as_ref()?.policy)
    }

    /// The worst outcome of this person's rules; `None` without any. Wishes
    /// waiting for someone to be seated don't count yet.
    pub(super) fn status(&self, person: usize) -> Option<Outcome> {
        self.own
            .get(&person)?
            .iter()
            .filter(|(_, r)| !self.pending(r))
            .map(|(_, r)| r.outcome())
            .max()
    }

    /// A wish for someone not seated yet can't be judged; a broken hard
    /// rule still shows.
    pub(super) fn pending(&self, result: &RuleResult) -> bool {
        result.waiting && !result.hard_violation
    }

    /// Whether any of this person's wishes is waiting for someone to be seated.
    pub(super) fn waits(&self, person: usize) -> bool {
        self.own
            .get(&person)
            .is_some_and(|rs| rs.iter().any(|(_, r)| self.pending(r)))
    }
}

/// Score and hard-violation count for an assignment (wishes plus policy).
pub(in crate::app) fn total_score(
    scorer: &crate::seating::Scorer,
    project: &Project,
    assignment: &BTreeMap<usize, String>,
) -> (i64, usize) {
    let evaluation = score::evaluate_with(scorer, project, assignment);
    (evaluation.penalty(), evaluation.hard_violations)
}
