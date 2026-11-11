//! Elevator edge cases: choosing doors, door modes, rotation, power, removal, saving.

use super::*;

/// A size-2 elevator with its top-left corner at (10, 10), on the ground floor, in a world with every floor open.
fn lift() -> World {
    let mut w = World::blank();
    w.place_sized(0, 10, 10, Kind::Elevator, Dir::E, 2).unwrap();
    w
}

fn anchor() -> usize {
    idx(10, 10)
}

fn set_door(w: &mut World, f: usize, want: DoorMode) {
    if let Some(Building { st: St::Elevator { mode, .. }, .. }) = w.floors[f].tiles[anchor()].as_mut() {
        *mode = want;
    }
}

fn outbox_len(w: &World, f: usize) -> usize {
    match &w.floors[f].tiles[anchor()] {
        Some(Building { st: St::Elevator { outbox, .. }, .. }) => outbox.len(),
        other => panic!("no door on floor {f}: {other:?}"),
    }
}

fn full_car(w: &mut World) {
    let cap = w.lifts[0].cap();
    w.lifts[0].load.extend(std::iter::repeat_n(Item::Coal, cap));
}

#[test]
fn with_three_floors_the_car_picks_the_nearest_door_then_the_lower_floor() {
    // Floors open so far: -1, ground, +1.
    let (down, up) = (World::blank().floor_of(-1).unwrap(), World::blank().floor_of(1).unwrap());
    // From the ground both unload doors are one floor away: the lower floor index wins.
    let mut w = lift();
    full_car(&mut w);
    w.tick();
    assert_eq!((w.lifts[0].phase, w.lifts[0].target), (LiftPhase::Closing, down));
    // With that door passing, the other one is the target.
    let mut w = lift();
    set_door(&mut w, down, DoorMode::Pass);
    full_car(&mut w);
    w.tick();
    assert_eq!(w.lifts[0].target, up);
    // An empty car at an unload door looks for the nearest load door: the ground (1 away) beats the floor below (2 away).
    let mut w = lift();
    set_door(&mut w, 0, DoorMode::Load);
    set_door(&mut w, down, DoorMode::Load);
    set_door(&mut w, up, DoorMode::Unload);
    w.lifts[0].at = up;
    w.lifts[0].target = up;
    w.tick();
    assert_eq!((w.lifts[0].phase, w.lifts[0].target), (LiftPhase::Closing, 0));
}

#[test]
fn pass_doors_are_skipped_and_take_nothing() {
    let mut w = lift();
    let (down, up) = (w.floor_of(-1).unwrap(), w.floor_of(1).unwrap());
    // Load at the bottom, unload at the top, and the ground floor in between only passes.
    set_door(&mut w, 0, DoorMode::Pass);
    set_door(&mut w, down, DoorMode::Load);
    w.lifts[0].at = down;
    w.lifts[0].target = down;
    assert!(!w.offer(0, 10, 10, Item::Coal), "a pass door doesn't load");
    assert!(w.offer(down, 10, 10, Item::Coal), "a load door does");
    if let Some(Building { st: St::Elevator { inbox, .. }, .. }) = w.floors[down].tiles[anchor()].as_mut() {
        inbox.clear();
    }
    full_car(&mut w);
    run(&mut w, 6);
    assert_eq!(w.lifts[0].at, up, "the car went past the ground floor");
    assert_eq!(outbox_len(&w, 0), 0);
    assert_eq!(outbox_len(&w, up), 40, "all of it unloaded at the top");
}

#[test]
fn turning_an_elevator_keeps_the_shaft_and_changes_the_unload_side() {
    let mut w = lift();
    let down = w.floor_of(-1).unwrap();
    w.place(down, 10, 12, Kind::Crate, Dir::E).unwrap(); // south of the shaft
    w.place(down, 12, 10, Kind::Crate, Dir::E).unwrap(); // east of the shaft
    w.set_dir(down, 10, 10, Dir::S).unwrap();
    assert_eq!(w.anchor_of(down, 11, 11), Some((10, 10)), "same footprint");
    assert_eq!(w.tile(down, 10, 10).unwrap().dir, Dir::S);
    assert_eq!(w.tile(0, 10, 10).unwrap().dir, Dir::E, "each floor's door keeps its own side");
    set_door(&mut w, down, DoorMode::Unload);
    w.lifts[0].at = down;
    w.lifts[0].target = down;
    w.lifts[0].phase = LiftPhase::Unloading;
    w.lifts[0].load.extend([Item::Coal; 6]);
    run(&mut w, 2);
    assert_eq!(crate_items(&w, down, 10, 12), vec![Item::Coal; 6], "out of the south side");
    assert!(crate_items(&w, down, 12, 10).is_empty());
}

#[test]
fn an_elevator_links_the_power_grids_of_its_floors() {
    let mut w = lift();
    let down = w.floor_of(-1).unwrap();
    // A belt on the floor below, next to the shaft, with a panel on the ground floor.
    w.place(down, 12, 10, Kind::Belt, Dir::E).unwrap();
    assert_eq!(w.ratio_at(down, 12, 10), 0.0);
    w.place(0, 8, 12, Kind::Solar, Dir::E).unwrap();
    assert!(w.ratio_at(down, 12, 10) > 0.0, "power comes down the shaft");
    // Remove the elevator and the link goes with it.
    w.remove(0, 10, 10).unwrap();
    assert_eq!(w.ratio_at(down, 12, 10), 0.0);
}

#[test]
fn removing_an_elevator_clears_every_door_and_refunds_once() {
    let mut w = lift();
    let paid = Kind::Elevator.price(2);
    let before = w.econ.money;
    assert_eq!(w.remove(w.floor_of(1).unwrap(), 11, 11), Some(Kind::Elevator));
    assert_eq!(w.econ.money, before + paid, "the price comes back once, not once per floor");
    for (f, fl) in w.floors.iter().enumerate() {
        assert!(fl.tiles[anchor()].is_none() && fl.owner[anchor()].is_none(), "floor {f} still has a door");
        assert!(fl.owner[idx(11, 11)].is_none());
    }
    assert!(w.lifts.is_empty());
    assert_eq!(w.remove(0, 10, 10), None, "nothing left to remove");
    assert_eq!(w.econ.money, before + paid);
}

#[test]
fn a_car_waits_unloaded_while_the_outbox_is_full() {
    let mut w = lift();
    let down = w.floor_of(-1).unwrap();
    let cap = w.lifts[0].cap();
    if let Some(Building { st: St::Elevator { outbox, .. }, .. }) = w.floors[down].tiles[anchor()].as_mut() {
        outbox.extend(std::iter::repeat_n(Item::IronOre, cap));
    }
    w.lifts[0].at = down;
    w.lifts[0].target = down;
    w.lifts[0].phase = LiftPhase::Unloading;
    w.lifts[0].load.extend([Item::Coal; 10]);
    run(&mut w, 5);
    assert_eq!((w.lifts[0].phase, w.lifts[0].load.len()), (LiftPhase::Unloading, 10), "waits with its load");
    assert_eq!(outbox_len(&w, down), cap);
}

#[test]
fn a_car_with_only_one_door_does_nothing() {
    let mut w = lift();
    for f in 1..w.floors.len() {
        w.floors[f].tiles[anchor()] = None;
    }
    assert!(w.offer(0, 10, 10, Item::Coal));
    run(&mut w, 5);
    assert_eq!((w.lifts[0].phase, w.lifts[0].at, w.lifts[0].load.len()), (LiftPhase::Loading, 0, 0), "never picks anything up");
}

#[test]
fn a_car_mid_trip_survives_saving_and_loading() {
    let mut w = lift();
    let down = w.floor_of(-1).unwrap();
    w.place(down, 12, 10, Kind::Crate, Dir::E).unwrap();
    full_car(&mut w);
    // Closing (0.5 s) then partway through the trip.
    for _ in 0..DOOR_TICKS + FLOOR_TICKS / 2 {
        w.tick();
    }
    assert_eq!(w.lifts[0].phase, LiftPhase::Moving);
    let path = std::env::temp_dir().join(format!("factory-test-lift-{}.json", std::process::id()));
    w.save(&path).unwrap();
    let mut loaded = World::load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let (a, b) = (&w.lifts[0], &loaded.lifts[0]);
    assert_eq!((b.phase, b.at, b.target, b.timer, b.load.len()), (a.phase, a.at, a.target, a.timer, a.load.len()));
    run(&mut loaded, 4);
    assert_eq!(crate_items(&loaded, down, 12, 10).len(), 40, "the trip carries on after loading");
}

#[test]
fn a_blocked_tile_on_another_floor_stops_an_elevator_with_its_own_message() {
    let mut w = World::blank();
    let up = w.floor_of(1).unwrap();
    w.place(up, 11, 11, Kind::Crate, Dir::E).unwrap();
    let err = w.check_place_sized(0, 10, 10, Kind::Elevator, Dir::E, 2).unwrap_err();
    assert_eq!(err, "Another floor is blocked at this spot");
    // On the floor itself it's the ordinary message.
    let err = w.check_place_sized(up, 10, 10, Kind::Elevator, Dir::E, 2).unwrap_err();
    assert_eq!(err, "Something is in the way");
}
