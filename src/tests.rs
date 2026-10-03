use std::collections::BTreeMap;

use crate::model::*;

fn fixture() -> Project {
    let mut project = Project::new();
    project.attendees = (0..4)
        .map(|id| Attendee {
            id,
            name: format!("Person {id}"),
            organization: String::new(),
            participant_group: String::new(),
            food: String::new(),
            drink: String::new(),
            allergies: String::new(),
            price_override_cents: None,
            extra: Default::default(),
            tags: if id == 1 || id == 2 {
                vec!["friends".into()]
            } else {
                Vec::new()
            },
        })
        .collect();
    project.layout.rows.push(RowSpec {
        id: "row-a".into(),
        label: "A".into(),
        tables: vec![TableSpec {
            id: "table-a".into(),
            label: "A".into(),
            left: 2,
            right: 2,
        }],
    });
    project
}

#[test]
fn project_validation_rejects_out_of_range_scoring_values() {
    let mut project = fixture();
    project.policy.sprinkle = i64::MIN;
    assert!(project.validate().is_err());

    project.policy = SeatingPolicy::default();
    project.policy.left_out_penalty = -1;
    assert!(project.validate().is_err());

    project.policy = SeatingPolicy::default();
    project.policy.organization_weight = 101;
    assert!(project.validate().is_err());

    project.policy = SeatingPolicy::default();
    project.policy.isolation_penalty = 2_001;
    assert!(project.validate().is_err());

    project.policy = SeatingPolicy::default();
    project.preferences.push(crate::preferences::Preference {
        id: 1,
        kind: crate::preferences::PreferenceKind::Together {
            people: vec![0, 1],
            costs: ProximityCosts([-1, 0, 0, 0, 0, 0]),
        },
        strength: crate::preferences::Strength::Important,
        enabled: true,
    });
    assert!(project.validate().is_err());
}

#[test]
fn project_validation_rejects_reserved_maximum_ids() {
    let mut project = fixture();
    project.attendees[0].id = usize::MAX;
    assert!(project.validate().is_err());

    let mut project = fixture();
    project.rules.push(Rule {
        id: u64::MAX,
        label: "reserved".into(),
        source: String::new(),
        enabled: true,
        kind: RuleKind::Placement {
            person: 0,
            area: Area::Seat("table-a:left:0".into()),
            must_be: true,
            hard: false,
            penalty: 1,
        },
    });
    assert!(project.validate().is_err());

    let mut project = fixture();
    project.preferences.push(crate::preferences::Preference {
        id: u64::MAX,
        kind: crate::preferences::PreferenceKind::together(vec![0, 1]),
        strength: crate::preferences::Strength::Important,
        enabled: true,
    });
    assert!(project.validate().is_err());
}

#[test]
fn progress_keeps_large_soft_scores_separate_from_hard_violations() {
    let mut project = fixture();
    project.policy.organization_weight = 0;
    project.policy.sprinkle = 0;
    project.policy.isolation_penalty = 0;
    project.policy.left_out_penalty = 0;
    project.rules.push(Rule {
        id: 1,
        label: "expensive wish".into(),
        source: "test".into(),
        enabled: true,
        kind: RuleKind::Proximity {
            requester: 0,
            target: Target::People { ids: vec![1] },
            costs: ProximityCosts([100_000; CLASSES]),
            weight: 11,
            weight_mode: WeightMode::Fixed,
            max_class: None,
        },
    });
    let search = crate::seating::Search::new(
        &project,
        crate::seating::Options {
            start: None,
            fixed: Default::default(),
            differ_from: None,
            budget: web_time::Duration::ZERO,
            runs: 1,
            seed: Some(1),
        },
    )
    .expect("search");

    assert_eq!(search.progress().best(), Some((1_100_000, 0)));
    let outcome = search.finish();
    let evaluation = crate::score::evaluate(&project, &outcome.assignment);
    assert_eq!(outcome.cost, 1_100_000);
    assert_eq!(outcome.hard_violations, 0);
    assert_eq!(evaluation.penalty(), outcome.cost);
    assert_eq!(evaluation.hard_violations, outcome.hard_violations);
}

#[test]
fn vertical_table_neighbours() {
    let project = fixture();
    let seats = project.layout.seats();
    let seat = |id: &str| seats.iter().find(|s| s.id == id).unwrap();
    let class = |a, b| distance_class(&project.layout, seat(a), seat(b));
    assert_eq!(
        class("table-a:left:0", "table-a:left:1"),
        DistanceClass::Beside
    );
    assert_eq!(
        class("table-a:left:0", "table-a:right:0"),
        DistanceClass::Opposite
    );
    assert_eq!(
        class("table-a:left:0", "table-a:right:1"),
        DistanceClass::Diagonal
    );
    assert!(seat("table-a:left:1").y > seat("table-a:left:0").y);
}

#[test]
fn unconstrained_attendees_fill_all_seats() {
    let project = fixture();
    let candidate = crate::seating::solve(&project, None, 0, web_time::Duration::from_millis(400))
        .expect("free seating");
    assert_eq!(candidate.assignment.len(), 4);
    assert_eq!(
        candidate
            .assignment
            .values()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
    assert_eq!(
        crate::score::evaluate(&project, &candidate.assignment).wish_penalty,
        0
    );
}

#[test]
fn group_and_direction_are_preserved() {
    let mut project = fixture();
    project.rules.push(Rule {
        id: 1,
        label: "two friends".into(),
        source: "friends".into(),
        enabled: true,
        kind: RuleKind::Proximity {
            requester: 0,
            target: Target::Tag {
                tag: "friends".into(),
                required: 2,
            },
            costs: ProximityCosts::WISH,
            weight: 10,
            weight_mode: WeightMode::Fixed,
            max_class: None,
        },
    });
    let assignment = BTreeMap::from([
        (0, "table-a:left:0".into()),
        (1, "table-a:left:1".into()),
        (2, "table-a:right:0".into()),
        (3, "table-a:right:1".into()),
    ]);
    let evaluation = crate::score::evaluate(&project, &assignment);
    assert_eq!(evaluation.wish_penalty, 0);
    assert_eq!(evaluation.results.len(), 1);
    assert!(evaluation.results[0].detail.contains("2 nearest"));
}

#[test]
fn multi_person_wish_counts_every_named_person() {
    let mut project = fixture();
    project.rules.push(Rule {
        id: 1,
        label: "friends".into(),
        source: String::new(),
        enabled: true,
        kind: RuleKind::Proximity {
            requester: 0,
            target: Target::People { ids: vec![1, 2] },
            costs: ProximityCosts::WISH,
            weight: 10,
            weight_mode: WeightMode::Fixed,
            max_class: None,
        },
    });
    let assignment = BTreeMap::from([
        (0, "table-a:left:0".into()),
        (1, "table-a:left:1".into()),
        (2, "table-a:right:1".into()),
        (3, "table-a:right:0".into()),
    ]);
    let result = crate::score::evaluate(&project, &assignment);
    assert_eq!(
        result.results[0].detail,
        "all of Person 1, Person 2: diagonal"
    );
    assert!(
        result.results[0].penalty > 0,
        "both named people count toward the wish"
    );
}

#[test]
fn requester_budget_is_shared_across_wishes() {
    let mut project = fixture();
    for (id, target) in [(1, 1), (2, 2)] {
        project.rules.push(Rule {
            id,
            label: "wish".into(),
            source: String::new(),
            enabled: true,
            kind: RuleKind::Proximity {
                requester: 0,
                target: Target::People { ids: vec![target] },
                costs: ProximityCosts::WISH,
                weight: 10,
                weight_mode: WeightMode::ShareRequesterBudget,
                max_class: None,
            },
        });
    }
    assert_eq!(project.effective_weight(&project.rules[0]), 5);
    project.rules[1].enabled = false;
    assert_eq!(project.effective_weight(&project.rules[0]), 10);
}

#[test]
fn solver_obeys_hard_seat_and_finds_beside() {
    let mut project = fixture();
    project.rules.push(Rule {
        id: 1,
        label: "seat".into(),
        source: String::new(),
        enabled: true,
        kind: RuleKind::Placement {
            person: 0,
            area: Area::Seat("table-a:left:0".into()),
            must_be: true,
            hard: true,
            penalty: 100,
        },
    });
    project.rules.push(Rule {
        id: 2,
        label: "avec".into(),
        source: String::new(),
        enabled: true,
        kind: RuleKind::Proximity {
            requester: 0,
            target: Target::People { ids: vec![1] },
            costs: ProximityCosts::AVEC,
            weight: 100,
            weight_mode: WeightMode::Fixed,
            max_class: None,
        },
    });
    assert_eq!(project.active_people().len(), 2);
    let first = crate::seating::solve(&project, None, 0, web_time::Duration::from_millis(400))
        .expect("first solution");
    assert_eq!(first.assignment.len(), 4);
    assert_eq!(
        first
            .assignment
            .values()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
    assert_eq!(first.assignment[&0], "table-a:left:0");
    assert_eq!(first.assignment[&1], "table-a:left:1");
    assert_eq!(
        crate::score::evaluate(&project, &first.assignment).wish_penalty,
        0
    );
    let second = crate::seating::solve(
        &project,
        Some(&first),
        1,
        web_time::Duration::from_millis(400),
    )
    .expect("distinct solution");
    assert!(
        first
            .assignment
            .iter()
            .any(|(person, seat)| second.assignment[person] != *seat)
    );
}

#[test]
fn solver_matches_exhaustive_score_for_group_and_avoidance() {
    let mut project = fixture();
    project.rules.push(Rule {
        id: 1,
        label: "avoid end".into(),
        source: String::new(),
        enabled: true,
        kind: RuleKind::Placement {
            person: 0,
            area: Area::Seat("table-a:left:0".into()),
            must_be: false,
            hard: true,
            penalty: 10,
        },
    });
    project.rules.push(Rule {
        id: 2,
        label: "two friends".into(),
        source: String::new(),
        enabled: true,
        kind: RuleKind::Proximity {
            requester: 0,
            target: Target::Tag {
                tag: "friends".into(),
                required: 2,
            },
            costs: ProximityCosts::AVEC,
            weight: 3,
            weight_mode: WeightMode::Fixed,
            max_class: None,
        },
    });
    project.rules.push(Rule {
        id: 3,
        label: "one-way wish".into(),
        source: String::new(),
        enabled: true,
        kind: RuleKind::Proximity {
            requester: 3,
            target: Target::People { ids: vec![0] },
            costs: ProximityCosts::WISH,
            weight: 5,
            weight_mode: WeightMode::ShareRequesterBudget,
            max_class: None,
        },
    });
    let seats = project.layout.seats();
    let mut best = i64::MAX;
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let indexes = [a, b, c, d];
                    if indexes
                        .iter()
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        != 4
                    {
                        continue;
                    }
                    let assignment = (0..4)
                        .map(|person| (person, seats[indexes[person]].id.clone()))
                        .collect();
                    let evaluation = crate::score::evaluate(&project, &assignment);
                    if evaluation.hard_violations == 0 {
                        best = best.min(evaluation.wish_penalty);
                    }
                }
            }
        }
    }
    let solution = crate::seating::solve(&project, None, 0, web_time::Duration::from_millis(400))
        .expect("solution");
    let evaluation = crate::score::evaluate(&project, &solution.assignment);
    assert_eq!(evaluation.hard_violations, 0);
    assert_eq!(evaluation.wish_penalty, best);
}

#[test]
fn spare_seats_come_from_table_bottoms_in_pairs() {
    let mut project = fixture();
    project.layout.rows[0].tables[0].left = 4;
    project.layout.rows[0].tables[0].right = 4;
    let seats = project.layout.seats();
    let usable =
        crate::seating::spare_seats::usable_seats(&project, &seats, &[], &Default::default());
    let empty: Vec<_> = seats
        .iter()
        .zip(&usable)
        .filter(|(_, u)| !**u)
        .map(|(s, _)| s.id.as_str())
        .collect();
    assert_eq!(empty.len(), 4);
    for id in [
        "table-a:left:3",
        "table-a:right:3",
        "table-a:left:2",
        "table-a:right:2",
    ] {
        assert!(empty.contains(&id), "{id} should stay empty: {empty:?}");
    }
    project.attendees.push(Attendee {
        id: 4,
        name: "Person 4".into(),
        organization: String::new(),
        participant_group: String::new(),
        tags: Vec::new(),
        food: String::new(),
        drink: String::new(),
        allergies: String::new(),
        price_override_cents: None,
        extra: Default::default(),
    });
    let usable =
        crate::seating::spare_seats::usable_seats(&project, &seats, &[], &Default::default());
    let right_bottom = seats
        .iter()
        .position(|s| s.id == "table-a:right:2")
        .unwrap();
    assert!(
        !usable[right_bottom],
        "odd spare seat is the right-hand bottom one"
    );
}

#[test]
fn pinned_person_stays_on_seat() {
    let mut project = fixture();
    project.pin(3, "table-a:right:1");
    let candidate = crate::seating::solve(&project, None, 0, web_time::Duration::from_millis(400))
        .expect("pinned seating");
    assert_eq!(candidate.assignment[&3], "table-a:right:1");
    project.unpin(3);
    assert!(project.pinned_people().is_empty());
}

#[test]
fn solved_chart_has_no_gaps_mid_table() {
    let mut project = fixture();
    project.layout.rows[0].tables[0].left = 3;
    project.layout.rows[0].tables[0].right = 3;
    let candidate = crate::seating::solve(&project, None, 0, web_time::Duration::from_millis(400))
        .expect("seating");
    let seats = project.layout.seats();
    let taken: std::collections::HashSet<_> = candidate.assignment.values().collect();
    for empty in seats.iter().filter(|s| !taken.contains(&s.id)) {
        assert!(
            !seats
                .iter()
                .any(|s| s.side == empty.side && s.index > empty.index && taken.contains(&s.id)),
            "{} is empty with someone below it",
            empty.id
        );
    }
}

#[test]
fn suggests_names_hidden_in_notes() {
    let mut project = fixture();
    project.attendees[1].name = "Xxxx Yyyy".into();
    project.attendees[2].name = "Zzzz Wwww".into();
    project.attendees[3].organization = "XYZ".into();
    project.notes.push(ImportNote {
        person: 0,
        field: ImportField::Wish,
        text: "Xxxx Yyyy, and Zzzz Wwwx or other XYZ members".into(),
        reviewed: false,
    });
    let found = crate::suggest::for_note(&project, 0);
    assert!(found.iter().all(|s| {
        !matches!(&s.target, crate::suggest::SuggestedTarget::People(ids) if ids.len() > 1)
    }));
    let people: Vec<_> = found
        .iter()
        .filter_map(|s| match &s.target {
            crate::suggest::SuggestedTarget::People(ids) if ids.len() == 1 => {
                Some((ids[0], s.confidence))
            }
            _ => None,
        })
        .collect();
    assert!(
        people.contains(&(1, crate::suggest::Confidence::High)),
        "{found:?}"
    );
    assert!(
        people.contains(&(2, crate::suggest::Confidence::Medium)),
        "misspelled name: {found:?}"
    );
    assert!(
        found
            .iter()
            .any(|s| s.target == crate::suggest::SuggestedTarget::Organization("XYZ".into()))
    );
    crate::suggest::accept(&mut project, &found[0]);
    project.validate().expect("accepted suggestion is valid");
}

#[test]
fn merge_import_keeps_rules_and_adds_food() {
    let mut project = Project::new();
    let sheet = crate::import::parse_sheet(
        "Name\tOrganization\tSeating wish\nXxxx Yyyy\tABC\tZzzz Wwww\nZzzz Wwww\tXYZ\t\n",
    )
    .expect("first sheet");
    crate::import::merge(&mut project, &sheet).expect("first import");
    let rules = project.rules.len();
    assert_eq!(rules, 1, "the exact name in the wish becomes a rule");
    let first = project.attendees[0].name.clone();
    let sheet = crate::import::parse_sheet(&format!(
        "Nimi\tRuoka\tJuoma\tAllergiat\tNotes\n{first}\tVegetarian\tNon-alcoholic\tNuts\tx\n"
    ))
    .expect("sheet");
    let summary = crate::import::merge(&mut project, &sheet).expect("merge");
    assert_eq!((summary.added, summary.updated), (0, 1));
    assert_eq!(project.rules.len(), rules);
    assert_eq!(project.attendees[0].food, "Vegetarian");
    assert_eq!(project.attendees[0].drink, "Non-alcoholic");
    assert_eq!(project.attendees[0].allergies, "Nuts");
    assert_eq!(project.attendees[0].extra["Notes"], "x");
}

#[test]
fn group_moves_keep_group_together() {
    let mut project = fixture();
    project.layout.rows[0].tables[0].left = 3;
    project.layout.rows[0].tables[0].right = 3;
    project
        .rules
        .push(crate::import::wish_rule(1, 0, vec![3], true, ""));
    let groups = crate::groups::wish_groups(&project);
    assert_eq!(groups, vec![vec![0, 3]]);
    let assignment: BTreeMap<usize, String> = [
        (0, "table-a:left:0"),
        (1, "table-a:right:0"),
        (2, "table-a:left:1"),
        (3, "table-a:right:1"),
    ]
    .into_iter()
    .map(|(p, s)| (p, s.to_string()))
    .collect();
    let moves = crate::groups::suggest_moves(&project, &assignment, &groups[0], |a| {
        (crate::score::evaluate(&project, a).wish_penalty, 0)
    });
    assert!(!moves.is_empty());
    let best = &moves[0];
    assert!(
        best.delta < 0,
        "best move should bring the pair together: {best:?}"
    );
    let seats = project.layout.seats();
    let seat = |p: usize| seats.iter().find(|s| s.id == best.assignment[&p]).unwrap();
    assert!(distance_class(&project.layout, seat(0), seat(3)) <= DistanceClass::Opposite);
}

#[test]
fn every_print_renders() {
    let mut project = fixture();
    project.event.title = "Test".into();
    project.attendees[0].food = "Vegetarian".into();
    project.attendees[1].allergies = "Nuts".into();
    project.attendees[0].participant_group = "Member".into();
    project.attendees[0].drink.clear();
    for (kind, name, cents) in [
        (crate::catalog::Kind::Ticket, "Member", 2000),
        (crate::catalog::Kind::Food, "Vegetarian", 550),
    ] {
        let index = project.catalog.create(kind, name);
        project.catalog.list_mut(kind)[index].price_cents = Some(cents);
    }
    let candidate = crate::seating::solve(&project, None, 0, web_time::Duration::from_millis(400))
        .expect("seating");
    project.candidates.push(candidate);
    for kind in crate::print::PrintKind::ALL {
        let pdf =
            crate::print::render(&project, 0, kind).unwrap_or_else(|e| panic!("{kind:?}: {e:#}"));
        assert!(pdf.starts_with(b"%PDF"), "{kind:?}");
    }
}

#[test]
fn incremental_deltas_match_full_rescoring() {
    let mut project = synthetic_project(88, 45);
    for person in [3, 17, 40] {
        project.rules.push(Rule {
            id: 1000 + person as u64,
            label: "Not at a table end".into(),
            source: String::new(),
            enabled: true,
            kind: RuleKind::AvoidTableEnd {
                person,
                end_penalty: 300,
                near_end_penalty: 80,
            },
        });
    }
    project.layout.rows.push(RowSpec {
        id: "row".into(),
        label: "Row 1".into(),
        tables: (1..=3)
            .map(|i| TableSpec {
                id: format!("t{i}"),
                label: format!("Table {i}"),
                left: 15,
                right: 15,
            })
            .collect(),
    });
    project.policy.organization = OrganizationMixing::Mixed;
    crate::seating::check_deltas(&project);
}

#[test]
fn roster_changes_keep_the_project_consistent() {
    let mut project = fixture();
    project.layout.rows[0].tables[0].left = 3;
    project.layout.rows[0].tables[0].right = 3;
    project
        .rules
        .push(crate::import::wish_rule(1, 0, vec![3], true, ""));
    project
        .rules
        .push(crate::import::wish_rule(2, 3, vec![0, 1], false, ""));
    project.candidates.push(
        crate::seating::solve(&project, None, 0, web_time::Duration::from_millis(300))
            .expect("seating"),
    );

    // A cancellation removes their rules and them from others' wishes.
    crate::roster::remove(&mut project, 0);
    project.validate().expect("valid after removal");
    assert!(project.rules.iter().all(|r| r.id != 1));
    assert!(
        matches!(&project.rules[0].kind, RuleKind::Proximity { target: Target::People { ids, .. }, .. } if ids == &vec![1])
    );
    assert!(!project.candidates[0].assignment.contains_key(&0));

    // A walk-in gets a free seat without leaving a gap.
    let walk_in = crate::roster::NewAttendee {
        name: "X".into(),
        ..Default::default()
    };
    let id = crate::roster::add(&mut project, walk_in).expect("add");
    let seat = project.candidates[0].assignment[&id].clone();
    let seats = project.layout.seats();
    let taken: std::collections::HashSet<_> = project.candidates[0].assignment.values().collect();
    let placed = seats.iter().find(|s| s.id == seat).unwrap();
    assert!(
        !seats
            .iter()
            .any(|s| s.side == placed.side && s.index < placed.index && !taken.contains(&s.id))
    );

    // A replacement takes the departing person's seat.
    let before = project.candidates[0].assignment[&3].clone();
    let new = crate::roster::replace(
        &mut project,
        3,
        crate::roster::NewAttendee {
            name: "Y".into(),
            ..Default::default()
        },
    )
    .expect("replace");
    assert_eq!(project.candidates[0].assignment[&new], before);
    assert!(project.person(3).is_none());
    project.validate().expect("valid after replacement");
    assert!(
        crate::roster::add(
            &mut project,
            crate::roster::NewAttendee {
                name: "X".into(),
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn catalog_merges_spellings_everywhere() {
    use crate::catalog::Kind;
    let mut project = fixture();
    project.attendees[0].organization = "ABC".into();
    project.attendees[1].organization = "Abc".into();
    project.attendees[2].organization = "Aa-Bb Cc".into();
    project.attendees[3].organization = "XYZ".into();
    let abc = project.catalog.create(Kind::Organization, "ABC");
    project.attendees[3].organization = "ABC/XYZ".into();
    project.attendees.push(Attendee {
        id: 9,
        name: "Z".into(),
        organization: "XYZ".into(),
        ..project.attendees[0].clone()
    });
    let suggestions = crate::catalog::merge_suggestions(&project, Kind::Organization);
    let suggested: std::collections::HashMap<_, _> = suggestions
        .into_iter()
        .map(|m| (m.spelling, m.targets.join("+")))
        .collect();
    assert_eq!(
        suggested.get("ABC/XYZ").map(String::as_str),
        Some("ABC+XYZ")
    );
    // Case and spacing differences map automatically; no suggestion needed.
    assert_eq!(project.catalog.find(Kind::Organization, "Abc"), Some(abc));
    assert!(!suggested.contains_key("Abc"));
    assert_eq!(suggested.get("Aa-Bb Cc").map(String::as_str), Some("ABC"));
    assert!(!suggested.contains_key("XYZ"));

    project.catalog.assign(Kind::Organization, "Abc", abc);
    project.catalog.assign(Kind::Organization, "Aa-Bb Cc", abc);
    let same = |a: usize, b: usize| {
        let key = |i: usize| project.category_key(Kind::Organization, &project.attendees[i]);
        key(a) == key(b)
    };
    assert!(same(0, 1) && same(0, 2) && !same(0, 3));
    // An organization wish written with any spelling reaches everyone in the category.
    let wish = Target::Organization {
        name: "aa-bb cc".into(),
        required: 1,
    };
    let mut targets = project.target_ids(3, &wish);
    targets.sort();
    assert_eq!(targets, vec![0, 1, 2]);

    // "ABC/XYZ" split into both categories counts as both.
    let xyz = project.catalog.create(Kind::Organization, "XYZ");
    project.catalog.assign(Kind::Organization, "ABC/XYZ", abc);
    project.catalog.assign(Kind::Organization, "ABC/XYZ", xyz);
    assert_eq!(
        project.category_name(Kind::Organization, &project.attendees[3]),
        "ABC / XYZ"
    );
    let xyz_wish = Target::Organization {
        name: "XYZ".into(),
        required: 1,
    };
    assert!(project.target_ids(0, &xyz_wish).contains(&3));
    assert!(project.target_ids(3, &wish).contains(&0));
}

/// A made-up project: `people` attendees in three organizations, and
/// `wishes` random seating wishes (some avecs), for solver-level tests.
fn synthetic_project(people: usize, wishes: usize) -> Project {
    use rand::{Rng, SeedableRng};
    let mut rng = rand::rngs::StdRng::seed_from_u64(7);
    let mut project = Project::new();
    project.attendees = (0..people)
        .map(|id| Attendee {
            id,
            name: format!("Person {id}"),
            organization: ["ABC", "XYZ", "Other"][id % 3].into(),
            participant_group: String::new(),
            tags: Vec::new(),
            food: String::new(),
            drink: String::new(),
            allergies: String::new(),
            price_override_cents: None,
            extra: Default::default(),
        })
        .collect();
    for rule in 0..wishes {
        let from = rng.random_range(0..people);
        let to = (from + rng.random_range(1..people)) % people;
        project.rules.push(crate::import::wish_rule(
            rule as u64 + 1,
            from,
            vec![to],
            rule % 5 == 0,
            "",
        ));
    }
    project
}

#[test]
fn avoid_table_end_keeps_people_off_the_ends() {
    let mut project = fixture();
    project.layout.rows[0].tables[0].left = 3;
    project.layout.rows[0].tables[0].right = 3;
    project.attendees.truncate(4);
    project.rules.push(Rule {
        id: 1,
        label: "Not at a table end".into(),
        source: String::new(),
        enabled: true,
        kind: RuleKind::AvoidTableEnd {
            person: 0,
            end_penalty: 300,
            near_end_penalty: 80,
        },
    });
    // 6 seats, 4 people: the spare pair is at the bottom, so rows 1 and 2 are
    // occupied on both sides; row 1 is the top end, row 2 the bottom end.
    // Either way the person sits at an end, but never next to nobody.
    let candidate = crate::seating::solve(&project, None, 0, web_time::Duration::from_millis(300))
        .expect("seating");
    let result = crate::score::evaluate(&project, &candidate.assignment);
    let end = result.results.iter().find(|r| r.rule_id == 1).unwrap();
    assert_eq!(end.detail, "at a table end");

    // With a full 3-row side, the middle seat is best.
    let template = project.attendees[0].clone();
    project.attendees.extend((4..6).map(|id| Attendee {
        id,
        name: format!("Person {id}"),
        ..template.clone()
    }));
    let candidate = crate::seating::solve(&project, None, 0, web_time::Duration::from_millis(300))
        .expect("seating");
    let result = crate::score::evaluate(&project, &candidate.assignment);
    let end = result.results.iter().find(|r| r.rule_id == 1).unwrap();
    assert_eq!(end.penalty, 80, "{}", end.detail);
}

/// Two tables of four, eight people, no wishes.
fn two_tables() -> Project {
    let mut project = synthetic_project(8, 0);
    project.layout.rows.push(RowSpec {
        id: "row".into(),
        label: "Row 1".into(),
        tables: (1..=2)
            .map(|i| TableSpec {
                id: format!("t{i}"),
                label: format!("Table {i}"),
                left: 2,
                right: 2,
            })
            .collect(),
    });
    project
}

#[test]
fn preferences_group_together_and_keep_apart() {
    use crate::preferences::{Closeness, PreferenceKind, Strength};
    let mut project = two_tables();
    // Spread over the organizations' natural order, so the solver must move them.
    project.add_preference(
        PreferenceKind::together(vec![0, 3, 5, 6]),
        Strength::Important,
    );
    project.add_preference(
        PreferenceKind::Apart {
            people: vec![1, 2],
            closeness: Closeness::Neighbours,
        },
        Strength::Must,
    );
    project.validate().expect("valid");
    crate::seating::check_deltas(&project);
    let candidate = crate::seating::solve(&project, None, 0, web_time::Duration::from_millis(300))
        .expect("seating");
    let result = crate::score::evaluate(&project, &candidate.assignment);
    assert_eq!(result.hard_violations, 0);
    let details: Vec<&str> = result
        .preferences
        .iter()
        .map(|p| p.detail.as_str())
        .collect();
    assert_eq!(details[1], "kept apart");
    assert!(
        details[0] == "all at one table" || details[0] == "everyone close",
        "{details:?}"
    );

    // Removing people shrinks preferences; one left alone is dropped.
    crate::roster::remove(&mut project, 1);
    crate::roster::remove(&mut project, 0);
    assert_eq!(project.preferences.len(), 1);
    assert_eq!(project.preferences[0].kind.people(), &[3, 5, 6]);
    project.validate().expect("valid after removal");
}

#[test]
fn report_score_matches_solver_score() {
    use crate::preferences::{Closeness, PreferenceKind, Strength};
    let mut project = synthetic_project(40, 30);
    project.layout.rows.push(RowSpec {
        id: "row".into(),
        label: "Row 1".into(),
        tables: (1..=3)
            .map(|i| TableSpec {
                id: format!("t{i}"),
                label: format!("Table {i}"),
                left: 7,
                right: 7,
            })
            .collect(),
    });
    project.policy.organization = OrganizationMixing::Mixed;
    project.rules.push(Rule {
        id: 999,
        label: "Not at a table end".into(),
        source: String::new(),
        enabled: true,
        kind: RuleKind::AvoidTableEnd {
            person: 5,
            end_penalty: 300,
            near_end_penalty: 80,
        },
    });
    project.add_preference(PreferenceKind::together(vec![1, 2, 3]), Strength::Important);
    project.add_preference(
        PreferenceKind::Apart {
            people: vec![4, 6],
            closeness: Closeness::Table,
        },
        Strength::NiceToHave,
    );
    let outcome = crate::seating::search(
        &project,
        crate::seating::Options {
            start: None,
            fixed: Default::default(),
            differ_from: None,
            budget: web_time::Duration::from_millis(200),
            runs: 1,
            seed: Some(1),
        },
    )
    .expect("seating");
    let evaluation = crate::score::evaluate(&project, &outcome.assignment);
    assert_eq!(evaluation.hard_violations, 0);
    assert_eq!(evaluation.penalty(), outcome.cost);
    assert_eq!(evaluation.hard_violations, outcome.hard_violations);
}

#[test]
fn improve_moves_a_pinned_person_to_their_pin() {
    let mut project = two_tables();
    let seating = crate::seating::solve(&project, None, 0, web_time::Duration::from_millis(200))
        .expect("seating");
    let other = seating
        .assignment
        .iter()
        .find(|(p, _)| **p != 0)
        .map(|(_, s)| s.clone())
        .unwrap();
    project.pin(0, &other);
    let improved = crate::seating::improve(
        &project,
        &seating.assignment,
        &Default::default(),
        web_time::Duration::from_millis(200),
    );
    assert_eq!(improved[&0], other);
    assert_eq!(
        crate::score::evaluate(&project, &improved).hard_violations,
        0
    );
}

#[test]
fn spare_seats_leave_room_for_hard_table_rules() {
    let mut project = two_tables();
    project.attendees.truncate(7);
    // Four people must sit at table 2, which has exactly four seats.
    for person in 0..4 {
        project.rules.push(Rule {
            id: 100 + person as u64,
            label: "Table 2".into(),
            source: String::new(),
            enabled: true,
            kind: RuleKind::Placement {
                person,
                area: Area::Table("t2".into()),
                must_be: true,
                hard: true,
                penalty: 0,
            },
        });
    }
    let seating = crate::seating::solve(&project, None, 0, web_time::Duration::from_millis(200))
        .expect("seating");
    assert_eq!(
        crate::score::evaluate(&project, &seating.assignment).hard_violations,
        0
    );
}

#[test]
fn suggests_names_written_surname_first() {
    let mut project = fixture();
    project.attendees[1].name = "Xxxx Yyyy".into();
    project.notes.push(ImportNote {
        person: 0,
        field: ImportField::Wish,
        text: "Yyyy Xxxx please".into(),
        reviewed: false,
    });
    let found = crate::suggest::for_note(&project, 0);
    assert!(
        found.iter().any(
            |s| s.target == crate::suggest::SuggestedTarget::People(vec![1])
                && s.confidence == crate::suggest::Confidence::High
        ),
        "{found:?}"
    );
}

#[test]
fn import_keeps_same_named_rows_apart() {
    let sheet = crate::import::parse_sheet("Name\tAvec\nX Y\tZ W\nX Y\t\n").expect("sheet");
    let mut project = Project::new();
    let summary = crate::import::merge(&mut project, &sheet).expect("merge");
    assert_eq!(project.attendees.len(), 2);
    assert_eq!(summary.duplicate_names, vec!["X Y".to_string()]);
    assert!(project.attendees.iter().all(|a| a.name == "X Y"));
}

#[test]
fn example_event_is_valid_and_solvable() {
    let project = crate::example::project();
    project.validate().expect("valid example");
    let seating = crate::seating::solve(&project, None, 0, web_time::Duration::from_millis(300))
        .expect("seating");
    assert_eq!(
        crate::score::evaluate(&project, &seating.assignment).hard_violations,
        0
    );
}

#[test]
fn finnish_sample_header_is_recognised() {
    let sheet = crate::import::parse_sheet("Nimi\tJärjestö\tAvec\tPöytäseuratoive\tRuoka\tJuoma\tAllergiat\nX Y\tABC\t\t\tKala\tVesi\t\n").expect("sheet");
    assert!(
        !sheet.columns.contains(&crate::import::Column::Other),
        "{:?}",
        sheet.headers
    );
}

#[test]
fn avatar_initials() {
    use crate::app::theme_initials as initials;
    assert_eq!(initials("Guest 07"), "07");
    assert_eq!(initials("Xxxx Yyyy Zzzz"), "XZ");
    assert_eq!(initials("Xxxx"), "X");
}

#[test]
fn solver_status_is_data_rendered_at_display_time() {
    use crate::model::SolverStatus;
    crate::i18n::set(crate::i18n::Lang::En);
    let status = SolverStatus::BestFound {
        seconds: 30,
        runs_at_best: 2,
        runs: 8,
    };
    assert_eq!(status.describe(), "Searched 30 s · best in 2/8 runs");
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(json["kind"], "best_found");
    assert_eq!(
        serde_json::from_value::<SolverStatus>(json).unwrap(),
        status
    );
    let improved = SolverStatus::Improved {
        from: "Option 1".into(),
        hand_placed: 3,
    };
    assert_eq!(
        improved.describe(),
        "Improved Option 1 · 3 manual seats kept"
    );
    let continued = SolverStatus::Continued {
        from: "Option 2".into(),
        seconds: 30,
    };
    assert_eq!(continued.describe(), "Searched 30 s more · Option 2");
    assert_eq!(SolverStatus::EditedByHand.describe(), "Edited by hand");
}

#[test]
fn print_language_resolves_explicit_choices() {
    use crate::model::PrintLanguage;
    assert_eq!(PrintLanguage::Finnish.code(), "fi");
    assert_eq!(PrintLanguage::English.code(), "en");
    assert_eq!(PrintLanguage::default(), PrintLanguage::SameAsApp);
}

fn quick_search(project: &Project, budget_ms: u64) -> crate::seating::Search {
    crate::seating::Search::new(
        project,
        crate::seating::Options {
            start: None,
            fixed: Default::default(),
            differ_from: None,
            budget: web_time::Duration::from_millis(budget_ms),
            runs: 2,
            seed: Some(3),
        },
    )
    .expect("search")
}

#[test]
fn progress_reports_the_best_cost_found() {
    let project = two_tables();
    let mut search = quick_search(&project, 200);
    let progress = search.progress();
    assert!(
        progress.best().is_some(),
        "the starting seatings count as progress"
    );
    search.run_to_end();
    let outcome = search.finish();
    let (score, hard) = progress.best().unwrap();
    assert_eq!(score, outcome.cost);
    assert_eq!(hard as usize, outcome.hard_violations);
}

#[test]
fn stopped_search_ends_early_with_a_seating() {
    let project = two_tables();
    let mut search = quick_search(&project, 60_000);
    search.progress().stop();
    let started = web_time::Instant::now();
    search.run_to_end();
    assert!(started.elapsed() < web_time::Duration::from_secs(5));
    assert!(!search.finish().assignment.is_empty());
}

#[test]
fn template_headers_are_recognised_exactly_in_both_languages() {
    use crate::import::{Column, Match, parse_sheet};
    for (en, fi) in Column::TEMPLATE.map(Column::headers) {
        for header in [en, fi, &en.to_uppercase()] {
            let sheet = parse_sheet(&format!("{header}\tNimi\nx\tX Y\n")).expect("sheet");
            assert_eq!(sheet.matches[0], Match::Exact, "{header}");
        }
    }
    // Anything else is left for the person to decide: no guessing.
    let sheet = parse_sheet("Koko nimi\tLempiväri\nX Y\tsininen\n").expect("sheet");
    assert!(sheet.columns.iter().all(|c| *c == Column::Other));
    assert!(sheet.matches.iter().all(|m| *m == Match::Unknown));
}

#[test]
fn ignored_columns_are_left_out_and_a_name_is_required() {
    use crate::import::{Column, merge, parse_sheet};
    let mut sheet = parse_sheet("Name\tNotes\nX Y\tsecret\n").expect("sheet");
    sheet.columns[1] = Column::Ignore;
    let mut project = Project::new();
    merge(&mut project, &sheet).expect("merge");
    assert!(project.attendees[0].extra.is_empty());
    sheet.columns[0] = Column::Other;
    assert!(merge(&mut Project::new(), &sheet).is_err());
}

#[test]
fn exported_people_import_back_unchanged() {
    let project = crate::example::project();
    let csv = crate::import::export_csv(&project);
    let sheet = crate::import::parse_sheet(&csv).expect("sheet");
    assert!(
        sheet
            .matches
            .iter()
            .all(|m| *m == crate::import::Match::Exact)
    );
    let mut again = project.clone();
    let summary = crate::import::merge(&mut again, &sheet).expect("merge");
    assert_eq!(
        (summary.added, summary.updated, summary.new_notes),
        (0, project.attendees.len(), 0)
    );
    assert_eq!(again.attendees.len(), project.attendees.len());
    let template = crate::import::parse_sheet(&crate::import::template_csv()).expect("template");
    assert!(template.has_name() && template.rows.len() == 1);
}

#[test]
fn column_choices_are_remembered_for_the_next_import() {
    use crate::import::{Column, Match, merge, parse_sheet};
    let text = "Koko nimi\tPöytätoive\tLempiväri\nX Y\tZ W\tsininen\n";
    let mut project = Project::new();
    let mut sheet = parse_sheet(text).expect("sheet");
    sheet.columns = vec![Column::Name, Column::Wish, Column::Ignore];
    merge(&mut project, &sheet).expect("merge");
    let mut again = parse_sheet(text).expect("sheet");
    again.remember(&project.import_columns);
    assert_eq!(again.columns, [Column::Name, Column::Wish, Column::Ignore]);
    assert!(again.matches.iter().all(|m| *m == Match::Remembered));
}

#[test]
fn attendee_prices_sum_mapped_ticket_food_and_drink_without_guessing() {
    use crate::catalog::Kind;
    let mut project = fixture();
    let person = &mut project.attendees[0];
    person.participant_group = "Member".into();
    person.food = "Vegetarian".into();
    person.drink = "Alcohol free".into();
    for (kind, name, cents) in [
        (Kind::Ticket, "Member", 2000),
        (Kind::Food, "Vegetarian", 550),
        (Kind::Drink, "Alcohol free", 0),
    ] {
        let index = project.catalog.create(kind, name);
        project.catalog.list_mut(kind)[index].price_cents = Some(cents);
    }
    assert_eq!(
        project.attendee_price_cents(&project.attendees[0]),
        Some(2550)
    );
    project.catalog.assign(Kind::Ticket, "member alias", 0);
    project.attendees[0].participant_group = "member alias".into();
    assert_eq!(
        project.attendee_price_cents(&project.attendees[0]),
        Some(2550)
    );
    let extra = project.catalog.create(Kind::Drink, "Extra drink");
    project.catalog.assign(Kind::Drink, "Alcohol free", extra);
    project.catalog.list_mut(Kind::Drink)[extra].price_cents = Some(125);
    assert_eq!(
        project.attendee_price_cents(&project.attendees[0]),
        Some(2675)
    );
    project.catalog.list_mut(Kind::Drink)[extra].price_cents = None;
    assert_eq!(project.attendee_price_cents(&project.attendees[0]), None);
    project.attendees[0].drink = "Unmapped choice".into();
    assert_eq!(project.attendee_price_cents(&project.attendees[0]), None);
    project.attendees[0].participant_group.clear();
    project.attendees[0].food.clear();
    project.attendees[0].drink.clear();
    assert_eq!(project.attendee_price_cents(&project.attendees[0]), None);
    project.attendees[0].price_override_cents = Some(1234);
    assert_eq!(
        project.attendee_price_cents(&project.attendees[0]),
        Some(1234)
    );
    project.attendees[0].price_override_cents = Some(0);
    assert_eq!(project.attendee_price_cents(&project.attendees[0]), Some(0));
    let decoded: crate::model::Project =
        serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
    assert_eq!(decoded.catalog, project.catalog);
    assert_eq!(decoded.attendees[0].price_override_cents, Some(0));
    project.attendees[0].price_override_cents = None;
    assert_eq!(project.attendee_price_cents(&project.attendees[0]), None);
}
