use super::*;
use crate::model::Point;

/// Two tables of 2 + 2 seats, person `i` on the `i`-th seat.
fn seated() -> (Project, Candidate) {
    let mut project = crate::tests::two_tables();
    let mut candidate = project.empty_option("Option 1".into());
    for (person, seat) in project.layout.seats().iter().enumerate() {
        candidate.place(person, Spot::Seat(seat.id.clone()));
    }
    project.candidates.push(candidate.clone());
    (project, candidate)
}

fn seat(id: &str) -> Spot {
    Spot::Seat(id.into())
}

fn after(project: &Project, candidate: &Candidate, wanted: &[(usize, Spot)]) -> Candidate {
    plan(project, candidate, wanted)
        .expect("allowed")
        .result(candidate)
}

#[test]
fn moving_onto_someone_swaps_the_two() {
    let (project, candidate) = seated();
    let next = after(&project, &candidate, &[(0, seat("t2:left:0"))]);
    assert_eq!(next.spot_of(0), seat("t2:left:0"));
    assert_eq!(next.spot_of(4), candidate.spot_of(0));
    assert!(next.hand_placed.contains(&0) && next.hand_placed.contains(&4));
}

#[test]
fn a_block_dropped_on_a_block_swaps_them() {
    let (project, candidate) = seated();
    let wanted = [(0, seat("t2:left:0")), (1, seat("t2:left:1"))];
    let next = after(&project, &candidate, &wanted);
    assert_eq!(next.spot_of(4), candidate.spot_of(0));
    assert_eq!(next.spot_of(5), candidate.spot_of(1));
}

#[test]
fn a_shifted_block_sends_the_person_it_lands_on_to_where_it_started() {
    let (project, candidate) = seated();
    let start = candidate.spot_of(0);
    let wanted = [(0, candidate.spot_of(1)), (1, candidate.spot_of(4))];
    let next = after(&project, &candidate, &wanted);
    assert_eq!(next.spot_of(4), start);
}

#[test]
fn people_can_leave_their_seat_and_come_back() {
    let (project, candidate) = seated();
    let point = Point { x: -500.0, y: 20.0 };
    let off = after(&project, &candidate, &[(0, Spot::Loose(point))]);
    assert_eq!(off.spot_of(0), Spot::Loose(point));
    assert!(!off.hand_placed.contains(&0));
    assert_eq!(off.unseated(&project), vec![0]);
    // Back onto a taken seat: its occupant takes the loose spot.
    let back = after(&project, &off, &[(0, seat("t2:left:0"))]);
    assert_eq!(back.spot_of(4), Spot::Loose(point));
}

#[test]
fn someone_displaced_from_the_list_goes_back_to_the_list() {
    let (project, candidate) = seated();
    let off = after(&project, &candidate, &[(0, Spot::Unseated)]);
    let next = after(&project, &off, &[(0, seat("t2:left:0"))]);
    assert_eq!(next.spot_of(4), Spot::Unseated);
}

#[test]
fn two_people_cannot_land_on_one_seat() {
    let (project, candidate) = seated();
    let wanted = [(0, seat("t2:left:0")), (1, seat("t2:left:0"))];
    assert!(plan(&project, &candidate, &wanted).is_err());
}

#[test]
fn people_pinned_to_a_seat_are_not_pushed_aside() {
    let (mut project, candidate) = seated();
    project.pin(4, Pin::Seat("t2:left:0".into()));
    assert!(plan(&project, &candidate, &[(0, seat("t2:left:0"))]).is_err());
}

#[test]
fn people_kept_at_a_table_are_only_pushed_within_it() {
    let (mut project, candidate) = seated();
    project.pin(5, Pin::Table("t2".into()));
    // 5 would go to table 1.
    assert!(plan(&project, &candidate, &[(0, seat("t2:left:1"))]).is_err());
    // 5 moves to the seat 4 left at the same table.
    let next = after(&project, &candidate, &[(4, seat("t2:left:1"))]);
    assert_eq!(next.spot_of(5), seat("t2:left:0"));
}

#[test]
fn pins_follow_the_person_moved() {
    let (mut project, candidate) = seated();
    project.pin(0, Pin::Seat("t1:left:0".into()));
    project.pin(1, Pin::Table("t1".into()));
    let wanted = [(0, seat("t2:left:0")), (1, seat("t2:left:1"))];
    let edit = plan(&project, &candidate, &wanted).unwrap();
    edit.apply(&mut project, 0);
    let pins = project.pins();
    assert_eq!(pins[&0], Pin::Seat("t2:left:0".into()));
    assert_eq!(pins[&1], Pin::Table("t2".into()));
    // Off the seats, a seat pin goes and a table pin stays.
    let candidate = project.candidates[0].clone();
    let wanted = [(0, Spot::Unseated), (1, Spot::Unseated)];
    plan(&project, &candidate, &wanted)
        .unwrap()
        .apply(&mut project, 0);
    let pins = project.pins();
    assert!(!pins.contains_key(&0));
    assert_eq!(pins[&1], Pin::Table("t2".into()));
}

#[test]
fn an_empty_option_seats_only_people_pinned_to_a_seat() {
    let mut project = crate::tests::two_tables();
    project.pin(2, Pin::Seat("t1:right:0".into()));
    project.pin(3, Pin::Table("t2".into()));
    let empty = project.empty_option("Option 1".into());
    assert_eq!(empty.spot_of(2), seat("t1:right:0"));
    assert_eq!(empty.unseated(&project).len(), 7);
}

#[test]
fn seating_the_rest_keeps_hand_placed_people_and_table_pins() {
    let mut project = crate::tests::two_tables();
    project.pin(1, Pin::Table("t2".into()));
    project
        .candidates
        .push(project.empty_option("Option 1".into()));
    let candidate = project.candidates[0].clone();
    let wanted = [
        (0, seat("t2:right:1")),
        (5, Spot::Loose(Point { x: 0.0, y: 0.0 })),
    ];
    plan(&project, &candidate, &wanted)
        .unwrap()
        .apply(&mut project, 0);
    let option = &project.candidates[0];
    let mut search = crate::seating::improve_search(
        &project,
        &option.assignment,
        &option.hand_placed.iter().copied().collect(),
        web_time::Duration::from_millis(200),
    )
    .expect("search");
    search.run_to_end();
    let seated = search.finish().assignment;
    assert_eq!(seated.len(), project.attendees.len());
    assert_eq!(seated[&0], "t2:right:1");
    assert_eq!(seat_table(&seated[&1]), "t2");
}

#[test]
fn a_new_option_never_repeats_a_name() {
    let (mut project, _) = seated();
    project
        .candidates
        .push(project.empty_option("Option 2".into()));
    // Option 1 deleted: "Option 2" is taken, so the next is "Option 3".
    project.candidates.remove(0);
    assert_eq!(project.new_option_name(), "Option 3");
}

#[test]
fn someone_aimed_at_the_seat_of_a_mover_staying_put_is_refused() {
    let (project, mut candidate) = seated();
    candidate.place(2, Spot::Unseated);
    let wanted = [
        (0, candidate.spot_of(0)),
        (1, candidate.spot_of(1)),
        (2, candidate.spot_of(1)),
    ];
    assert!(plan(&project, &candidate, &wanted).is_err());
}

#[test]
fn a_seat_holds_one_pin_so_an_empty_option_stays_valid() {
    let (mut project, _) = seated();
    project.pin(0, Pin::Seat("t1:left:0".into()));
    // Pinned in another option where someone else sits there.
    assert_eq!(project.pin(1, Pin::Seat("t1:left:0".into())), Some(0));
    assert!(!project.pins().contains_key(&0));
    let empty = project.empty_option("Option 2".into());
    project.candidates.push(empty);
    assert!(project.validate().is_ok());
}

#[test]
fn two_pinned_people_swapping_keep_both_pins() {
    let (mut project, candidate) = seated();
    project.pin(0, Pin::Seat("t1:left:0".into()));
    project.pin(4, Pin::Seat("t2:left:0".into()));
    let wanted = [(0, seat("t2:left:0")), (4, seat("t1:left:0"))];
    plan(&project, &candidate, &wanted)
        .unwrap()
        .apply(&mut project, 0);
    let pins = project.pins();
    assert_eq!(pins[&0], Pin::Seat("t2:left:0".into()));
    assert_eq!(pins[&4], Pin::Seat("t1:left:0".into()));
}
