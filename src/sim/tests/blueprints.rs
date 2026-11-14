//! Blueprints in the world: capture, turning, pasting (whole and in part), settings by name,
//! tunnel pairs, elevators and floors.

use super::*;
use crate::sim::blueprint::{BpPart, PartSetting};
use std::collections::HashSet;

fn rebuilds() -> u32 {
    REBUILDS.with(|c| c.get())
}

const ALL: TileRect = (0, 0, W - 1, H - 1);

/// A bit of everything on the ground floor: facing parts, long machines, settings, a tunnel pair
/// and an elevator. It fills (10, 10) to (30, 21).
fn mixed() -> World {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Belt, Dir::E).unwrap();
    w.place(0, 11, 10, Kind::Belt, Dir::S).unwrap();
    w.place(0, 12, 10, Kind::Belt, Dir::W).unwrap();
    w.place(0, 14, 10, Kind::Pole, Dir::E).unwrap();
    w.place(0, 16, 10, Kind::Solar, Dir::E).unwrap();
    w.place(0, 10, 12, Kind::Furnace, Dir::S).unwrap();
    w.place(0, 14, 12, Kind::Press, Dir::N).unwrap();
    w.place(0, 18, 12, Kind::Drill, Dir::W).unwrap();
    w.apply_setting(0, 18, 12, Setting::Mines(Item::Coal));
    w.place(0, 20, 12, Kind::TradePost, Dir::E).unwrap();
    w.apply_setting(0, 20, 12, Setting::Sell(SellMode::Split));
    w.place(0, 10, 17, Kind::Arm, Dir::E).unwrap();
    w.apply_setting(0, 10, 17, Setting::Filter(Some(Item::Coal)));
    w.place(0, 12, 17, Kind::Sorter, Dir::N).unwrap();
    w.apply_setting(0, 12, 17, Setting::Filter(Some(Item::IronOre)));
    w.place(0, 14, 17, Kind::BasicAssembler, Dir::E).unwrap();
    w.apply_setting(0, 14, 17, Setting::Recipe(1));
    w.place(0, 18, 17, Kind::Tunnel, Dir::E).unwrap();
    w.place(0, 21, 17, Kind::Tunnel, Dir::E).unwrap();
    w.place_sized(0, 24, 10, Kind::Elevator, Dir::E, 3).unwrap();
    set_door(&mut w, 0, 24, 10, DoorMode::Pass);
    w.place(0, 30, 10, Kind::Crate, Dir::E).unwrap();
    w.place(0, 24, 17, Kind::Kiln, Dir::W).unwrap();
    w
}

fn set_door(w: &mut World, f: usize, x: i32, y: i32, m: DoorMode) {
    if let Some(Building { st: St::Elevator { mode, .. }, .. }) = w.floors[f].tiles[idx(x, y)].as_mut() {
        *mode = m;
    }
}

fn door(w: &World, f: usize, x: i32, y: i32) -> Option<DoorMode> {
    match w.floors[f].tiles[idx(x, y)].as_ref() {
        Some(Building { st: St::Elevator { mode, .. }, .. }) => Some(*mode),
        _ => None,
    }
}

/// A blueprint of single belts and the like, from (kind, dx, dy) with everything facing east.
fn bp_of(parts: &[(Kind, i32, i32)]) -> Blueprint {
    let parts = parts
        .iter()
        .map(|&(kind, dx, dy)| BpPart { dx, dy, kind, dir: Dir::E, size: 0, set: None, exit: (kind == Kind::Tunnel).then_some(false) })
        .collect();
    Blueprint::from_parts("test", 0, parts)
}

/// One part of the given facing, nothing else.
fn one(kind: Kind, dir: Dir, size: u8) -> Blueprint {
    Blueprint::from_parts("one", 0, vec![BpPart { dx: 0, dy: 0, kind, dir, size, set: None, exit: None }])
}

#[test]
fn capture_then_paste_round_trips() {
    let mut w = mixed();
    let cap = w.capture(0, ALL);
    let bp = cap.bp.clone();
    assert_eq!(bp.part_count(), 17);
    assert_eq!((bp.w, bp.h), (21, 12));
    assert_eq!(bp.level, 0);
    assert_eq!(cap.orphan_tunnels, 0);
    assert!(bp.check().is_ok());
    // Reading order.
    assert!(bp.parts.windows(2).all(|p| (p[0].dy, p[0].dx) < (p[1].dy, p[1].dx)));
    // Recipes by name, tunnel flags, door mode and elevator size are in it.
    let part = |kind| bp.parts.iter().find(|p| p.kind == kind).unwrap();
    assert_eq!(part(Kind::BasicAssembler).set, Some(PartSetting::Recipe(Kind::BasicAssembler.recipes()[1].name.to_string())));
    assert_eq!(part(Kind::Elevator).set, Some(PartSetting::Door(DoorMode::Pass)));
    assert_eq!(part(Kind::Elevator).size, 3);
    assert_eq!(bp.parts.iter().filter_map(|p| p.exit).collect::<Vec<_>>(), vec![false, true]);
    assert_eq!(part(Kind::Drill).set, Some(PartSetting::Mines(Item::Coal)));
    assert_eq!(part(Kind::TradePost).set, Some(PartSetting::Sell(SellMode::Split)));
    assert_eq!(part(Kind::Sorter).set, Some(PartSetting::Filter(Some(Item::IronOre))));
    assert_eq!(part(Kind::Arm).set, Some(PartSetting::Filter(Some(Item::Coal))));
    assert_eq!(part(Kind::Kiln).dir, Dir::W);
    // Pasted somewhere else: money drops by exactly the blueprint's price, with one rebuild.
    let (before, rb) = (w.econ.money, rebuilds());
    let rep = w.paste(0, 40, 40, &bp);
    assert_eq!((rep.placed, rep.total, rep.blocked, rep.short, rep.locked, rep.wrong_floor), (17, 17, 0, 0, 0, 0), "{rep:?}");
    assert_eq!(rebuilds() - rb, 1);
    assert!((before - w.econ.money - bp.cost()).abs() < 1e-6);
    assert!((rep.cost - bp.cost()).abs() < 1e-6);
    // Copying the copy gives the same blueprint.
    let again = w.capture(0, (40, 40, 40 + bp.w - 1, 40 + bp.h - 1)).bp;
    assert_eq!(again.parts, bp.parts);
    assert_eq!((again.w, again.h), (bp.w, bp.h));
    // Its elevator has a door on every open floor, and a car of its own.
    assert_eq!(w.lifts.len(), 2);
    assert_eq!(door(&w, 0, 54, 40), Some(DoorMode::Pass));
    for g in 1..w.floors.len() {
        if w.floor_open(g) {
            assert_eq!(door(&w, g, 54, 40), Some(DoorMode::Unload), "floor {g}");
        }
    }
}

#[test]
fn four_turns_is_identity_and_each_turn_is_a_quarter() {
    let bp = mixed().capture(0, ALL).bp;
    assert_eq!(bp.rotated(0), bp);
    assert_eq!(bp.rotated(4), bp);
    assert_eq!(bp.rotated(1).rotated(3), bp);
    assert_eq!(bp.rotated(2).rotated(2), bp);
    assert_eq!(bp.rotated(5), bp.rotated(1));
    let r = bp.rotated(1);
    assert_eq!((r.w, r.h), (bp.h, bp.w));
    assert_eq!(bp.rotated(2).w, bp.w);
    // Same parts, same cost, whatever the turn.
    for t in 0..4 {
        let r = bp.rotated(t);
        assert_eq!(r.part_count(), bp.part_count());
        assert!((r.cost() - bp.cost()).abs() < 1e-9);
        assert!(r.check().is_ok(), "turn {t}: {:?}", r.check());
        assert!(r.parts.windows(2).all(|p| (p[0].dy, p[0].dx) < (p[1].dy, p[1].dx)), "turn {t} is in reading order");
    }
    // By hand: a press (2x1) facing east with a belt two tiles on. Turned clockwise the press
    // stands up facing south and the belt is below it.
    let mut line = bp_of(&[(Kind::Press, 0, 0), (Kind::Belt, 2, 0)]);
    assert_eq!((line.w, line.h), (3, 1));
    line = line.rotated(1);
    assert_eq!((line.w, line.h), (1, 3));
    assert_eq!((line.parts[0].kind, line.parts[0].dir, line.parts[0].dx, line.parts[0].dy), (Kind::Press, Dir::S, 0, 0));
    assert_eq!((line.parts[1].kind, line.parts[1].dir, line.parts[1].dx, line.parts[1].dy), (Kind::Belt, Dir::S, 0, 2));
    assert_eq!(line.parts[0].dims(), (1, 2));
    // Another turn: west, lying down, with the belt on the left.
    line = line.rotated(1);
    assert_eq!((line.w, line.h), (3, 1));
    assert_eq!((line.parts[0].kind, line.parts[0].dir, line.parts[0].dx), (Kind::Belt, Dir::W, 0));
    assert_eq!((line.parts[1].kind, line.parts[1].dir, line.parts[1].dx, line.parts[1].dims()), (Kind::Press, Dir::W, 1, (2, 1)));
}

#[test]
fn rotated_footprints_match_kind_dims() {
    for kind in Kind::ALL {
        let sizes: Vec<u8> = if kind == Kind::Elevator { (MIN_LIFT as u8..=MAX_LIFT as u8).collect() } else { vec![0] };
        for size in sizes {
            for dir in Dir::ALL {
                let bp = one(kind, dir, size);
                let (mut want_dir, mut want_dims) = (dir, bp.parts[0].dims());
                assert_eq!((bp.w, bp.h), want_dims, "{kind:?} {dir:?}");
                for turn in 1..=4u8 {
                    let r = bp.rotated(turn);
                    if kind.directional() {
                        want_dir = want_dir.cw();
                    }
                    want_dims = dims_of(kind, want_dir, size as i32);
                    let p = &r.parts[0];
                    assert_eq!((p.dir, p.dims()), (want_dir, want_dims), "{kind:?} {dir:?} turn {turn}");
                    assert_eq!((r.w, r.h), want_dims, "{kind:?} {dir:?} turn {turn}: the box is the part");
                    assert_eq!((p.dx, p.dy), (0, 0));
                }
            }
        }
    }
}

#[test]
fn no_two_rotated_parts_overlap_and_the_box_holds_them() {
    let bp = mixed().capture(0, ALL).bp;
    for t in 0..4 {
        let r = bp.rotated(t);
        let mut seen = HashSet::new();
        for p in &r.parts {
            let (pw, ph) = p.dims();
            assert!(p.dx >= 0 && p.dy >= 0 && p.dx + pw <= r.w && p.dy + ph <= r.h, "turn {t}: {p:?}");
            for tile in footprint(p.dx, p.dy, pw, ph) {
                assert!(seen.insert(tile), "turn {t}: {tile:?} twice");
            }
        }
    }
}

#[test]
fn a_turned_blueprint_pastes_as_the_turned_layout() {
    let bp = mixed().capture(0, ALL).bp;
    for turns in 0..4u8 {
        let turned = bp.rotated(turns);
        let mut w = World::blank();
        let rep = w.paste(0, 5, 5, &turned);
        assert_eq!(rep.placed, bp.part_count(), "turn {turns}: {rep:?}");
        let back = w.capture(0, (5, 5, 5 + turned.w - 1, 5 + turned.h - 1)).bp;
        assert_eq!(back.parts, turned.parts, "turn {turns}");
        assert_eq!((back.w, back.h), (turned.w, turned.h), "turn {turns}");
    }
}

#[test]
fn a_rotated_copy_works() {
    // The demo factory (drill, furnace, molder, press, trade post, solar panels, poles, belts and a
    // splitter), copied and pasted in all four turns, far apart: every copy sells the same.
    let mut w = World::new();
    w.build_demo().unwrap();
    w.econ.money = 1e6;
    let bp = w.capture(0, (40, 25, 80, 50)).bp;
    assert!(bp.part_count() > 25, "{}", bp.part_count());
    let spots = [(2, 2), (2, 50), (90, 2), (90, 50)];
    for (turns, (ax, ay)) in spots.iter().enumerate() {
        let rep = w.paste(0, *ax, *ay, &bp.rotated(turns as u8));
        assert_eq!(rep.placed, bp.part_count(), "turn {turns}: {rep:?}");
    }
    run(&mut w, 240);
    let mut sold: Vec<u64> = Vec::new();
    for (x, y) in w.parts_in(0, ALL) {
        if let Some(Building { st: St::Trade { count, .. }, .. }) = w.tile(0, x, y) {
            sold.push(*count);
        }
    }
    assert_eq!(sold.len(), 5, "{sold:?}");
    let (lo, hi) = (*sold.iter().min().unwrap(), *sold.iter().max().unwrap());
    assert!(lo >= 20, "every copy sells: {sold:?}");
    assert!(hi - lo <= hi / 10 + 2, "the copies earn the same, turned or not: {sold:?}");
}

#[test]
fn partial_paste_in_reading_order_and_the_rest_fills_in_later() {
    // Furnace and press on the top row, two belts and a pole under them.
    let bp = bp_of(&[(Kind::Furnace, 0, 0), (Kind::Press, 4, 0), (Kind::Belt, 0, 3), (Kind::Belt, 1, 3), (Kind::Pole, 2, 3)]);
    let (furnace, press, belt, pole) = (Kind::Furnace.cost(), Kind::Press.cost(), Kind::Belt.cost(), Kind::Pole.cost());
    assert_eq!(bp.cost(), furnace + press + 2.0 * belt + pole);
    let mut w = World::blank();
    // Enough for the furnace, the press and one belt, and a few dollars over: not for the second belt.
    money_set(&mut w, furnace + press + belt + 3.0);
    let plan = w.plan_paste(0, 20, 20, &bp);
    let states: Vec<PlanState> = plan.parts.iter().map(|p| p.state).collect();
    assert_eq!(states, vec![PlanState::Fits, PlanState::Fits, PlanState::Fits, PlanState::Short, PlanState::Short]);
    assert_eq!((plan.fits, plan.short), (3, 2));
    // The pole is cheap enough for the $3 left but comes after the cut: a clean cut, not a scatter.
    assert!(pole > 3.0 || plan.parts[4].state == PlanState::Short);
    assert!((plan.short_by - (belt + pole - 3.0)).abs() < 1e-9, "{}", plan.short_by);
    let rep = w.paste(0, 20, 20, &bp);
    assert_eq!((rep.placed, rep.short), (3, 2));
    assert!(w.tile(0, 20, 20).is_some() && w.tile(0, 24, 20).is_some() && w.tile(0, 20, 23).is_some());
    assert!(w.tile(0, 21, 23).is_none() && w.tile(0, 22, 23).is_none());
    assert!((w.econ.money - 3.0).abs() < 1e-9, "money never goes below zero: {}", w.econ.money);
    // Pasting again with the same money changes nothing.
    let again = w.paste(0, 20, 20, &bp);
    assert_eq!((again.placed, again.present, again.short), (0, 3, 2));
    assert!((w.econ.money - 3.0).abs() < 1e-9);
    // More money, same spot: the rest, and the total over both pastes is the blueprint's price.
    money_set(&mut w, 1000.0);
    let last = w.paste(0, 20, 20, &bp);
    assert_eq!((last.placed, last.present, last.short), (2, 3, 0));
    assert!((last.cost - (belt + pole)).abs() < 1e-9);
    assert!((w.econ.money - (1000.0 - belt - pole)).abs() < 1e-9);
    // And once it is all there, a further paste does nothing at all.
    let done = w.paste(0, 20, 20, &bp);
    assert_eq!((done.placed, done.present), (0, 5));
    assert!((w.econ.money - (1000.0 - belt - pole)).abs() < 1e-9);
}

fn money_set(w: &mut World, m: f64) {
    w.econ.money = m;
}

#[test]
fn nothing_is_charged_twice_across_pastes() {
    let bp = mixed().capture(0, ALL).bp;
    let mut w = World::blank();
    let start = bp.cost() * 0.6;
    money_set(&mut w, start);
    let (mut earned, mut pastes) = (0.0, 0);
    loop {
        let rep = w.paste(0, 30, 30, &bp);
        pastes += 1;
        assert!(w.econ.money >= -1e-9, "money never goes below zero");
        if rep.short == 0 {
            break;
        }
        // The factory earns a little between pastes.
        w.econ.money += bp.cost() * 0.1;
        earned += bp.cost() * 0.1;
        assert!(pastes < 20);
    }
    assert!(pastes > 1, "it took more than one paste");
    assert!(((start + earned) - w.econ.money - bp.cost()).abs() < 1e-6, "in all the blueprint's price was paid, once");
    assert_eq!(w.capture(0, (30, 30, 30 + bp.w - 1, 30 + bp.h - 1)).bp.parts, bp.parts);
}

#[test]
fn blocked_parts_are_skipped_and_pasting_again_fills_the_gap() {
    let bp = bp_of(&[(Kind::Belt, 0, 0), (Kind::Belt, 1, 0), (Kind::Belt, 2, 0), (Kind::Belt, 3, 0)]);
    let mut w = World::blank();
    w.place(0, 12, 20, Kind::Crate, Dir::E).unwrap();
    let before = w.econ.money;
    let rep = w.paste(0, 10, 20, &bp);
    assert_eq!((rep.placed, rep.blocked, rep.short), (3, 1, 0), "{rep:?}");
    assert!((before - w.econ.money - 3.0 * Kind::Belt.cost()).abs() < 1e-9);
    // Off the map counts as blocked too.
    let edge = w.paste(0, W - 2, 3, &bp);
    assert_eq!((edge.placed, edge.blocked), (2, 2));
    w.remove(0, 12, 20);
    let money = w.econ.money;
    let fill = w.paste(0, 10, 20, &bp);
    assert_eq!((fill.placed, fill.present, fill.blocked), (1, 3, 0));
    assert!((money - w.econ.money - Kind::Belt.cost()).abs() < 1e-9, "only the missing belt is charged");
}

#[test]
fn locked_kinds_are_skipped_with_a_note() {
    let mut w = World::new();
    let bp = bp_of(&[(Kind::Belt, 0, 0), (Kind::Kiln, 2, 0), (Kind::Belt, 6, 0)]);
    assert!(!w.progress.part_unlocked(Kind::Kiln));
    let rep = w.paste(0, 60, 20, &bp);
    assert_eq!((rep.placed, rep.locked, rep.locked_kind), (2, 1, Some(Kind::Kiln)), "{rep:?}");
    assert_eq!(bp.first_locked(&w.progress), Some(Kind::Kiln));
    assert!(w.tile(0, 62, 20).is_none());
    w.progress.researched[Tech::Kiln.idx()] = true;
    w.progress.opened[Company::Underhill.idx()] = true;
    assert_eq!(bp.first_locked(&w.progress), None);
}

#[test]
fn floors_charge_digging_and_refuse_the_wrong_parts() {
    let mut w = World::new();
    w.econ.money = 1e6;
    // Closed floors refuse the whole paste.
    let up = w.floor_of(1).unwrap();
    let down = w.floor_of(-1).unwrap();
    let closed = bp_of(&[(Kind::Belt, 0, 0), (Kind::Belt, 1, 0)]);
    let plan = w.plan_paste(up, 30, 30, &closed);
    assert_eq!(plan.refused, Some(PlaceErr::FloorClosed(1)));
    assert_eq!(w.paste(up, 30, 30, &closed).placed, 0);
    w.progress.researched[Tech::SupportBeams.idx()] = true;
    w.progress.researched[Tech::Excavation.idx()] = true;
    // Open now: the press pays its price and 2 tiles of decking, the belt one tile.
    let before = w.econ.money;
    let bp = bp_of(&[(Kind::Press, 0, 0), (Kind::Belt, 4, 0)]);
    w.progress.researched[Tech::Press.idx()] = true;
    let plan = w.plan_paste(up, 30, 30, &bp);
    assert_eq!(plan.fits, 2);
    let deck = tile_cost(1);
    assert!((plan.parts[0].cost - (Kind::Press.cost() + 2.0 * deck)).abs() < 1e-9, "{:?}", plan.parts[0]);
    assert!((plan.parts[1].cost - (Kind::Belt.cost() + deck)).abs() < 1e-9);
    let rep = w.paste(up, 30, 30, &bp);
    assert!((rep.cost - plan.total).abs() < 1e-9 && rep.placed == 2);
    assert!((before - w.econ.money - plan.total).abs() < 1e-9);
    // Digging down costs per undug tile; solar panels can't go underground, nor drills up.
    let mixed_parts = bp_of(&[(Kind::Solar, 0, 0), (Kind::Drill, 3, 0), (Kind::Belt, 6, 0)]);
    let plan = w.plan_paste(down, 30, 30, &mixed_parts);
    let states: Vec<PlanState> = plan.parts.iter().map(|p| p.state).collect();
    assert_eq!(states, vec![PlanState::WrongFloor, PlanState::Fits, PlanState::Fits]);
    let plan = w.plan_paste(up, 30, 40, &mixed_parts);
    let states: Vec<PlanState> = plan.parts.iter().map(|p| p.state).collect();
    assert_eq!(states, vec![PlanState::Fits, PlanState::WrongFloor, PlanState::Fits]);
    assert_eq!(plan.wrong_floor, 1);
    let rep = w.paste(up, 30, 40, &mixed_parts);
    assert_eq!((rep.placed, rep.wrong_floor), (2, 1));
}

#[test]
fn elevators_paste_with_doors_a_car_and_the_copied_door_mode() {
    let mut w = World::blank();
    let bp = Blueprint::from_parts(
        "lift",
        0,
        vec![BpPart { dx: 0, dy: 0, kind: Kind::Elevator, dir: Dir::S, size: 4, set: Some(PartSetting::Door(DoorMode::Pass)), exit: None }],
    );
    assert_eq!((bp.w, bp.h), (4, 4));
    assert!((bp.cost() - 75.0 * 16.0).abs() < 1e-9);
    let rep = w.paste(0, 20, 20, &bp);
    assert_eq!((rep.placed, rep.elevators), (1, 1));
    assert_eq!(w.lifts.len(), 1);
    assert_eq!((w.lifts[0].size, w.lifts[0].anchor), (4, idx(20, 20)));
    assert_eq!(door(&w, 0, 20, 20), Some(DoorMode::Pass));
    let others: Vec<usize> = (1..w.floors.len()).filter(|&g| w.floor_open(g)).collect();
    assert_eq!(others.len(), 2);
    for g in others {
        assert_eq!(door(&w, g, 20, 20), Some(DoorMode::Unload), "floor {g}: other floors' doors reset to unload");
    }
    // The same spot again: it is there already.
    let again = w.paste(0, 20, 20, &bp);
    assert_eq!((again.placed, again.present), (0, 1));
    assert_eq!(w.lifts.len(), 1);
    // Another floor in the way: this spot can't take it, and nothing is placed or charged.
    let down = w.floor_of(-1).unwrap();
    w.place(down, 31, 31, Kind::Crate, Dir::E).unwrap();
    let money = w.econ.money;
    let plan = w.plan_paste(0, 30, 30, &bp);
    assert_eq!(plan.parts[0].state, PlanState::WrongFloor);
    let blocked = w.paste(0, 30, 30, &bp);
    assert_eq!((blocked.placed, blocked.wrong_floor), (0, 1));
    assert_eq!((w.econ.money, w.lifts.len()), (money, 1));
    // A different size on the same spot is not "present": it is in the way.
    let small = Blueprint { parts: vec![BpPart { size: 2, ..bp.parts[0].clone() }], w: 2, h: 2, ..bp.clone() };
    assert_eq!(w.plan_paste(0, 20, 20, &small).parts[0].state, PlanState::Blocked);
    // Before any floor opens, elevators are locked.
    let fresh = World::new();
    assert_eq!(fresh.plan_paste(0, 20, 20, &bp).parts[0].state, PlanState::Locked);
}

#[test]
fn an_elevator_is_copied_with_its_size_and_door_whole_even_when_the_area_touches_one_tile() {
    let mut w = World::blank();
    w.place_sized(0, 20, 20, Kind::Elevator, Dir::E, 5).unwrap();
    let cap = w.capture(0, (24, 24, 30, 30));
    assert_eq!((cap.bp.part_count(), cap.bp.w, cap.bp.h), (1, 5, 5));
    assert_eq!(cap.bp.parts[0].size, 5);
}

#[test]
fn tunnel_pairs_survive_paste_and_every_turn() {
    // A pair on row 10: the entrance, two tiles of gap, the exit.
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Tunnel, Dir::E).unwrap();
    w.place(0, 13, 10, Kind::Tunnel, Dir::E).unwrap();
    let cap = w.capture(0, (10, 10, 13, 10));
    assert_eq!(cap.orphan_tunnels, 0);
    let bp = cap.bp;
    assert_eq!(bp.parts.iter().map(|p| p.exit).collect::<Vec<_>>(), vec![Some(false), Some(true)]);
    for turns in 0..4u8 {
        let turned = bp.rotated(turns);
        let mut w = World::blank();
        let (ax, ay) = (30, 30);
        assert_eq!(w.paste(0, ax, ay, &turned).placed, 2);
        let entrance = turned.parts.iter().find(|p| p.exit == Some(false)).unwrap();
        let exit = turned.parts.iter().find(|p| p.exit == Some(true)).unwrap();
        let (ex, ey) = (ax + entrance.dx, ay + entrance.dy);
        let (xx, xy) = (ax + exit.dx, ay + exit.dy);
        assert_eq!(w.tunnel_exit(0, ex, ey), Some(3), "turn {turns}: the entrance finds its exit");
        // Coal in a crate behind the entrance goes under and out into a crate past the exit.
        let (dx, dy) = entrance.dir.delta();
        w.place(0, ex - dx, ey - dy, Kind::Crate, entrance.dir).unwrap();
        fill_crate(&mut w, 0, ex - dx, ey - dy, &[Item::Coal; 6]);
        w.place(0, xx + dx, xy + dy, Kind::Crate, entrance.dir).unwrap();
        run(&mut w, 8);
        assert_eq!(crate_items(&w, 0, xx + dx, xy + dy).len(), 6, "turn {turns}: the coal came out the other end");
    }
}

#[test]
fn a_copy_that_cuts_a_pair_counts_the_ends_without_a_partner_and_they_paste_as_they_were() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Tunnel, Dir::E).unwrap();
    w.place(0, 13, 10, Kind::Tunnel, Dir::E).unwrap();
    // Only the exit.
    let exit_only = w.capture(0, (13, 10, 13, 10));
    assert_eq!(exit_only.orphan_tunnels, 1);
    // Only the entrance.
    let entrance_only = w.capture(0, (10, 10, 10, 10));
    assert_eq!(entrance_only.orphan_tunnels, 1);
    // A lone exit pastes as an exit, with nothing behind it; a lone entrance pastes as an
    // entrance even right behind someone else's entrance (placing by hand would have made it an exit).
    let mut v = World::blank();
    v.paste(0, 30, 30, &exit_only.bp);
    assert!(matches!(v.tile(0, 30, 30).map(|b| &b.st), Some(St::Tunnel { exit: true, .. })));
    v.place(0, 40, 40, Kind::Tunnel, Dir::E).unwrap();
    v.paste(0, 42, 40, &entrance_only.bp);
    assert!(matches!(v.tile(0, 42, 40).map(|b| &b.st), Some(St::Tunnel { exit: false, .. })));
    // The ends are gone from the count once the whole pair is inside.
    assert_eq!(w.capture(0, (10, 10, 13, 10)).orphan_tunnels, 0);
}

#[test]
fn a_long_machine_poking_into_the_rectangle_comes_whole_and_the_box_is_the_union() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Furnace, Dir::E).unwrap(); // 10-12 x 10-12
    w.place(0, 20, 10, Kind::Belt, Dir::E).unwrap();
    // The rectangle touches only the furnace's bottom-right tile.
    let cap = w.capture(0, (12, 12, 16, 16));
    assert_eq!(cap.bp.part_count(), 1);
    assert_eq!((cap.bp.w, cap.bp.h), (3, 3));
    // With the belt too, the box is the union of both footprints, bigger than the rectangle.
    let both = w.capture(0, (12, 10, 20, 12));
    assert_eq!(both.bp.part_count(), 2);
    assert_eq!((both.bp.w, both.bp.h), (11, 3));
    let kiln = {
        let mut w = World::blank();
        w.place(0, 10, 10, Kind::Kiln, Dir::S).unwrap(); // 5 wide, 3 tall facing south
        w.capture(0, (14, 10, 14, 10)).bp
    };
    assert_eq!((kiln.w, kiln.h), (5, 3));
    assert_eq!((kiln.parts[0].dir, kiln.parts[0].dims()), (Dir::S, (5, 3)));
}

#[test]
fn settings_travel_by_name() {
    let mut w = World::blank();
    // Furnace "coke" needs the Coking tech; the basic assembler's recipe list differs from the advanced one's.
    let coke = FURNACE_RECIPES.iter().position(|r| r.name == "coke").unwrap();
    w.place(0, 10, 10, Kind::Furnace, Dir::E).unwrap();
    w.apply_setting(0, 10, 10, Setting::Recipe(coke as u8));
    w.place(0, 20, 10, Kind::AdvancedAssembler, Dir::E).unwrap();
    let last_adv = Kind::AdvancedAssembler.recipes().len() as u8 - 1;
    w.apply_setting(0, 20, 10, Setting::Recipe(last_adv));
    w.place(0, 30, 10, Kind::TradePost, Dir::E).unwrap();
    w.apply_setting(0, 30, 10, Setting::Sell(SellMode::Research));
    w.place(0, 34, 10, Kind::Drill, Dir::E).unwrap();
    w.apply_setting(0, 34, 10, Setting::Mines(Item::IronOre));
    let bp = w.capture(0, ALL).bp;
    // A world that has not researched the coke recipe: the furnace pastes at its default and says so.
    let mut v = World::blank();
    v.progress.researched[Tech::Coking.idx()] = false;
    let rep = v.paste(0, 5, 40, &bp);
    assert_eq!((rep.placed, rep.recipes_skipped), (4, 1), "{rep:?}");
    let recipe = |v: &World, x, y| match &v.tile(0, x, y).unwrap().st {
        St::Crafter { recipe, .. } => *recipe,
        _ => panic!("not a crafter"),
    };
    assert_eq!(recipe(&v, 5, 40), 0, "default recipe when the tech is missing");
    assert_eq!(recipe(&v, 15, 40) as usize, last_adv as usize, "the advanced assembler's recipe, by name");
    assert_eq!(v.setting_at(0, 25, 40), Some(Setting::Sell(SellMode::Research)));
    assert_eq!(v.setting_at(0, 29, 40), Some(Setting::Mines(Item::IronOre)));
    // With the tech, the furnace gets coke.
    let mut v = World::blank();
    v.paste(0, 5, 40, &bp);
    assert_eq!(recipe(&v, 5, 40) as usize, coke);
    // A recipe the machine doesn't have is left at the default as well.
    let odd = Blueprint::from_parts(
        "odd",
        0,
        vec![BpPart {
            dx: 0,
            dy: 0,
            kind: Kind::Press,
            dir: Dir::E,
            size: 0,
            set: Some(PartSetting::Recipe("no such recipe".into())),
            exit: None,
        }],
    );
    let mut v = World::blank();
    assert_eq!(v.paste(0, 5, 5, &odd).recipes_skipped, 1);
}

#[test]
fn a_recipe_travels_by_name_even_when_the_list_is_reordered() {
    static BEFORE: [Recipe; 2] = [recipe("alpha", 'A'), recipe("beta", 'B')];
    static AFTER: [Recipe; 3] = [recipe("new", 'N'), recipe("beta", 'B'), recipe("alpha", 'A')];
    const fn recipe(name: &'static str, tag: char) -> Recipe {
        Recipe {
            name,
            inputs: &[(Item::Coal, 1)],
            outputs: &[(Item::Coke, 1)],
            fluid_in: None,
            fluid_out: None,
            fluid_out2: None,
            ticks: 60,
            fuel: None,
            needs: None,
            tag,
        }
    }
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &BEFORE))));
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Molder, Dir::E).unwrap();
    w.apply_setting(0, 10, 10, Setting::Recipe(1));
    let bp = w.capture(0, ALL).bp;
    assert_eq!(bp.parts[0].set, Some(PartSetting::Recipe("beta".into())));
    let text = serde_json::to_string(&bp).unwrap();
    assert!(text.contains("beta") && !text.contains("\"Recipe\":1"), "{text}");
    // Later the list has changed: "beta" is second, "alpha" third.
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &AFTER))));
    let mut v = World::blank();
    v.paste(0, 20, 20, &serde_json::from_str(&text).unwrap());
    let got = match &v.tile(0, 20, 20).unwrap().st {
        St::Crafter { recipe, .. } => *recipe,
        _ => panic!(),
    };
    assert_eq!(Kind::Molder.recipes()[got as usize].name, "beta");
    assert_eq!(got, 1);
    RECIPE_OVERRIDE.with(|o| o.set(None));
}

#[test]
fn a_batch_paste_equals_placing_one_by_one() {
    let mut src = World::blank();
    for x in 2..80 {
        src.place(0, x, 2, if x % 9 == 0 { Kind::Pole } else { Kind::Belt }, Dir::E).unwrap();
    }
    for k in 0..6 {
        src.place(0, 3 + k * 6, 6, Kind::Solar, Dir::E).unwrap();
        src.place(0, 3 + k * 6, 10, Kind::Furnace, Dir::S).unwrap();
        src.place(0, 3 + k * 6, 16, Kind::Drill, Dir::E).unwrap();
    }
    let bp = src.capture(0, ALL).bp;
    assert!(bp.part_count() >= 80);
    let mut batch = World::blank();
    let rb = rebuilds();
    let rep = batch.paste(0, 20, 30, &bp);
    assert_eq!(rep.placed, bp.part_count());
    assert_eq!(rebuilds() - rb, 1, "one rebuild for the whole paste");
    let mut one = World::blank();
    for p in &bp.parts {
        one.place_sized(0, 20 + p.dx, 30 + p.dy, p.kind, p.dir, p.size as i32).unwrap();
    }
    assert_eq!(batch.econ.money, one.econ.money);
    assert_eq!(batch.grid.cover, one.grid.cover);
    assert_eq!(batch.grid.unpowered, one.grid.unpowered);
    let kinds = |w: &World| w.floors[0].tiles.iter().map(|t| t.as_ref().map(|b| (b.kind, b.dir))).collect::<Vec<_>>();
    assert_eq!(kinds(&batch), kinds(&one));
}

#[test]
fn removing_an_area_and_pasting_its_blueprint_restores_the_layout_for_nothing() {
    let mut w = mixed();
    let cap = w.capture(0, ALL);
    let money = w.econ.money;
    let (_, undo) = w.remove_area(0, ALL);
    assert!(undo.is_some());
    assert!(w.econ.money > money, "refunded");
    let rep = w.paste(0, 10, 10, &cap.bp);
    assert_eq!(rep.placed, cap.bp.part_count());
    assert!((w.econ.money - money).abs() < 1e-6, "net money is zero");
    assert_eq!(w.capture(0, ALL).bp.parts, cap.bp.parts);
}

#[test]
fn the_file_shape_is_plain_json_and_ignores_what_it_does_not_know() {
    let bp = mixed().capture(0, ALL).bp;
    let text = serde_json::to_string_pretty(&bp).unwrap();
    assert!(text.contains("\"format\": 1") && text.contains("\"kind\": \"Furnace\"") && text.contains("\"dir\": \"S\""), "{text}");
    assert!(!text.contains("\"size\": 0"), "a size is written for elevators only");
    let back: Blueprint = serde_json::from_str(&text).unwrap();
    assert_eq!(back, bp);
    // Unknown fields (a newer game's) are ignored; missing optional ones default.
    let extra = r#"{"format":1,"name":"x","w":1,"h":1,"colour":"red","parts":[{"dx":0,"dy":0,"kind":"Belt","dir":"E","note":"hi"}]}"#;
    let b: Blueprint = serde_json::from_str(extra).unwrap();
    assert_eq!((b.level, b.parts[0].size, b.parts[0].set.clone(), b.parts[0].exit), (0, 0, None, None));
    // An unknown kind is an error.
    assert!(serde_json::from_str::<Blueprint>(&extra.replace("Belt", "Teleporter")).is_err());
    // Nonsense that parses is still caught by check().
    let mut bad = bp.clone();
    bad.w = 3;
    assert!(bad.check().is_err());
    let mut twice = bp_of(&[(Kind::Belt, 0, 0), (Kind::Belt, 1, 0)]);
    twice.parts[1].dx = 0;
    assert!(twice.check().is_err());
    assert!(Blueprint { parts: vec![], ..bp.clone() }.check().is_err());
    assert!(Blueprint { w: 0, ..bp }.check().is_err());
}

#[test]
fn blueprints_are_a_brannock_tech_that_unlocks_after_a_pile_of_copper_plate() {
    assert_eq!(Tech::Blueprints.company(), Company::Brannock);
    assert_eq!(Tech::Blueprints.cost(), 800.0);
    assert_eq!(Tech::Blueprints.milestone(), (Made::Item(Item::CopperPlate), 1500));
    let mut w = World::new();
    assert!(!w.progress.has(Tech::Blueprints));
    w.econ.research = 799.0;
    assert!(w.research(Tech::Blueprints).is_err());
    w.econ.research = 800.0;
    assert!(w.research(Tech::Blueprints).is_ok() && w.progress.has(Tech::Blueprints));
    // Or free by making 1,500 plates.
    let mut w = World::new();
    w.econ.made[Item::CopperPlate.idx()] = 1499;
    w.check_discoveries();
    assert!(!w.progress.has(Tech::Blueprints));
    w.econ.made[Item::CopperPlate.idx()] = 1500;
    w.check_discoveries();
    assert!(w.progress.has(Tech::Blueprints));
    // The area tool never needed a tech.
    assert!(World::new().progress.researched.iter().all(|r| !r));
}
