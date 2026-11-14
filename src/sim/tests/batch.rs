//! Batch placing and removing: the no-rebuild variants, `place_cost` and `PlaceErr`.

use super::*;

fn rebuilds() -> u32 {
    REBUILDS.with(|c| c.get())
}

/// About a hundred parts: belts, poles, panels, a furnace, drills, a crate, a tunnel pair and an elevator.
fn layout() -> Vec<(usize, i32, i32, Kind, Dir, i32)> {
    let mut v = Vec::new();
    for x in 2..70 {
        v.push((0, x, 2, if x % 7 == 0 { Kind::Pole } else { Kind::Belt }, Dir::E, 0));
    }
    for k in 0..8 {
        v.push((0, 3 + k * 5, 10, Kind::Solar, Dir::E, 0));
        v.push((0, 3 + k * 5, 14, Kind::BasicSolar, Dir::E, 0));
        v.push((0, 3 + k * 5, 18, Kind::Furnace, Dir::S, 0));
        v.push((0, 3 + k * 5, 24, Kind::Drill, Dir::E, 0));
    }
    v.push((0, 50, 30, Kind::Tunnel, Dir::E, 0));
    v.push((0, 53, 30, Kind::Tunnel, Dir::E, 0));
    v.push((0, 40, 40, Kind::Elevator, Dir::E, 3));
    v.push((2, 3, 3, Kind::Belt, Dir::N, 0));
    v.push((1, 3, 3, Kind::Crate, Dir::N, 0));
    v
}

/// What two worlds must agree on: money, who is powered by what, and every part's kind and facing.
fn same(a: &World, b: &World) {
    assert_eq!(a.econ.money, b.econ.money);
    assert_eq!(a.grid.cover, b.grid.cover);
    assert_eq!(a.grid.unpowered, b.grid.unpowered);
    assert_eq!(a.grid.nets.len(), b.grid.nets.len());
    for (x, y) in a.grid.nets.iter().zip(&b.grid.nets) {
        assert_eq!((x.supply, x.demand), (y.supply, y.demand));
    }
    assert_eq!(a.lifts.len(), b.lifts.len());
    for (fa, fb) in a.floors.iter().zip(&b.floors) {
        assert_eq!(fa.owner, fb.owner);
        assert_eq!(fa.dug, fb.dug);
        let kinds = |f: &Floor| f.tiles.iter().map(|t| t.as_ref().map(|b| (b.kind, b.dir, b.size))).collect::<Vec<_>>();
        assert_eq!(kinds(fa), kinds(fb));
    }
}

#[test]
fn batch_place_matches_one_by_one() {
    let parts = layout();
    assert!(parts.len() >= 100);
    let mut one = World::blank();
    let before = rebuilds();
    let mut placed = 0;
    for &(f, x, y, k, d, s) in &parts {
        if one.place_sized(f, x, y, k, d, s).is_ok() {
            placed += 1;
        }
    }
    assert_eq!(rebuilds() - before, placed, "one rebuild per placement");
    assert!(placed >= 100);

    let mut batch = World::blank();
    let before = rebuilds();
    let mut ok = 0;
    for &(f, x, y, k, d, s) in &parts {
        if batch.place_no_rebuild(f, x, y, k, d, s).is_ok() {
            ok += 1;
        }
    }
    assert_eq!(rebuilds() - before, 0, "no rebuild inside the batch");
    batch.rebuild_grid();
    assert_eq!(rebuilds() - before, 1, "one rebuild after it");
    assert_eq!(ok, placed);
    same(&one, &batch);
    // The tunnel pair was recognised in order, as it is by hand.
    assert_eq!(batch.tunnel_exit(0, 50, 30), Some(3));
}

#[test]
fn batch_remove_matches_one_by_one_and_rebuilds_once() {
    let parts = layout();
    let build = || {
        let mut w = World::blank();
        for &(f, x, y, k, d, s) in &parts {
            let _ = w.place_sized(f, x, y, k, d, s);
        }
        w
    };
    let mut one = build();
    let mut batch = build();
    same(&one, &batch);
    let before = rebuilds();
    for &(f, x, y, ..) in &parts {
        one.remove(f, x, y);
    }
    assert!(rebuilds() - before >= 100);
    let before = rebuilds();
    for &(f, x, y, ..) in &parts {
        batch.remove_no_rebuild(f, x, y);
    }
    assert_eq!(rebuilds() - before, 0);
    batch.rebuild_grid();
    assert_eq!(rebuilds() - before, 1);
    same(&one, &batch);
    assert!(batch.lifts.is_empty());
    assert_eq!(batch.econ.money, 1e6, "full refund, nothing dug that wasn't already");
}

#[test]
fn place_cost_ignores_money_and_place_errors_read_as_before() {
    let mut w = World::blank();
    w.econ.money = 0.0;
    // Money is the only thing the cost check leaves out.
    assert_eq!(w.place_cost(0, 5, 5, Kind::Furnace, Dir::E, 0), Ok(Kind::Furnace.price(3)));
    assert_eq!(w.place_cost_money(0, 5, 5, Kind::Furnace, Dir::E, 0), Err(PlaceErr::Money(Kind::Furnace.price(3))));
    assert_eq!(w.check_place(0, 5, 5, Kind::Furnace), Err(format!("Not enough money (need ${:.0})", Kind::Furnace.price(3))));
    w.econ.money = 1e6;
    w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
    assert_eq!(w.place_cost(0, 6, 6, Kind::Belt, Dir::E, 0), Err(PlaceErr::Blocked));
    assert_eq!(w.place_cost(0, EDGE - 1, 3, Kind::Furnace, Dir::E, 0), Err(PlaceErr::OffMap));
    assert_eq!(w.place_cost(2, 3, 3, Kind::Drill, Dir::E, 0), Err(PlaceErr::NotAboveGround(Kind::Drill)));
    assert_eq!(w.place_cost(1, 3, 3, Kind::Solar, Dir::E, 0), Err(PlaceErr::NeedsSun));
    assert_eq!(w.place_cost(0, 5, 5, Kind::Elevator, Dir::E, 3), Err(PlaceErr::Blocked));
    w.place_sized(2, 20, 20, Kind::Belt, Dir::E, 0).unwrap();
    assert_eq!(w.place_cost(0, 20, 20, Kind::Elevator, Dir::E, 2), Err(PlaceErr::OtherFloorBlocked));
    // The text is what the player has always read.
    assert_eq!(PlaceErr::Blocked.to_string(), "Something is in the way");
    assert_eq!(PlaceErr::OffMap.to_string(), "Doesn't fit on the map here");
    assert_eq!(PlaceErr::NeedsSun.to_string(), "Solar panels need sunlight");
    assert_eq!(PlaceErr::OtherFloorBlocked.to_string(), "Another floor is blocked at this spot");
    assert_eq!(PlaceErr::Locked(Kind::Kiln).to_string(), "Kiln is locked: see the research tree (t)");
    assert_eq!(w.place(2, 3, 3, Kind::Drill, Dir::E).unwrap_err(), PlaceErr::NotAboveGround(Kind::Drill).to_string());
    // A fresh world still keeps the upper floor shut.
    let w = World::new();
    assert_eq!(w.place_cost(2, 3, 3, Kind::Belt, Dir::E, 0), Err(PlaceErr::FloorClosed(1)));
    assert_eq!(PlaceErr::FloorClosed(1).to_string(), w.floor_hint(1));
}
