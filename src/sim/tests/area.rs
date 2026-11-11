//! The area tool's world side: which parts a rectangle touches, deconstructing them in one go,
//! and putting them back.

use super::*;

fn rebuilds() -> u32 {
    REBUILDS.with(|c| c.get())
}

/// Kind, facing and size of every part on a floor, in reading order.
fn layout(w: &World, f: usize) -> Vec<(Kind, Dir, u8)> {
    w.floors[f].tiles.iter().flatten().map(|b| (b.kind, b.dir, b.size)).collect()
}

fn waste_in_crate(w: &mut World, f: usize, x: i32, y: i32, n: usize) {
    fill_crate(w, f, x, y, &vec![Item::Waste; n]);
}

#[test]
fn rect_of_orders_the_corners_all_four_ways() {
    let want = (3, 4, 9, 7);
    for (a, b) in [((3, 4), (9, 7)), ((9, 7), (3, 4)), ((9, 4), (3, 7)), ((3, 7), (9, 4))] {
        assert_eq!(rect_of(a, b), want, "{a:?} {b:?}");
    }
    assert_eq!(rect_size(want), (7, 4));
    assert_eq!(rect_size((5, 5, 5, 5)), (1, 1));
}

#[test]
fn parts_in_lists_anchors_in_reading_order_without_repeats() {
    let mut w = World::blank();
    w.place(0, 20, 12, Kind::Belt, Dir::E).unwrap();
    w.place(0, 10, 10, Kind::Furnace, Dir::E).unwrap(); // 10-12, 10-12
    w.place(0, 14, 10, Kind::Belt, Dir::E).unwrap();
    w.place(0, 11, 20, Kind::Belt, Dir::E).unwrap();
    // The furnace is three tiles wide and nine tiles of it are inside: it still counts once.
    assert_eq!(w.parts_in(0, (9, 9, 15, 13)), vec![(10, 10), (14, 10)]);
    assert_eq!(w.parts_in(0, (0, 0, W - 1, H - 1)), vec![(10, 10), (14, 10), (20, 12), (11, 20)]);
    assert_eq!(w.parts_in(0, (15, 0, 19, 30)), vec![]);
    // A rectangle that runs off the map is clamped, not an error.
    assert_eq!(w.parts_in(0, (-5, -5, 11, 11)), vec![(10, 10)]);
    // Other floors are other lists.
    assert_eq!(w.parts_in(1, (0, 0, W - 1, H - 1)), vec![]);
}

#[test]
fn a_long_machine_with_one_tile_in_the_rectangle_comes_whole() {
    for (kind, dir) in [
        (Kind::Furnace, Dir::E),
        (Kind::Kiln, Dir::E),
        (Kind::Kiln, Dir::S),
        (Kind::LargeKiln, Dir::N),
        (Kind::LargeAdvancedAssembler, Dir::W),
    ] {
        let (w_, h_) = kind.dims(dir);
        // Touch only the machine's bottom-right tile, then only its top-left one.
        for (tx, ty) in [(10 + w_ - 1, 10 + h_ - 1), (10, 10)] {
            let mut w = World::blank();
            w.place(0, 10, 10, kind, dir).unwrap();
            let before = w.econ.money;
            let (rep, undo) = w.remove_area(0, (tx, ty, tx + 4, ty + 4));
            assert_eq!(rep.parts, 1, "{kind:?} {dir:?}");
            assert!(w.tile(0, 10, 10).is_none() && w.tile(0, tx, ty).is_none());
            assert!(w.floors[0].owner.iter().all(|o| o.is_none()), "no tile of {kind:?} is left owned");
            assert_eq!(w.econ.money, before + kind.price(w_));
            assert_eq!(undo.unwrap().len(), 1);
        }
    }
}

#[test]
fn area_deconstruct_refunds_in_full_and_rebuilds_once() {
    let mut w = World::blank();
    for x in 5..45 {
        w.place(0, x, 8, Kind::Belt, Dir::E).unwrap();
    }
    w.place(0, 6, 12, Kind::Solar, Dir::E).unwrap();
    w.place(0, 10, 12, Kind::Pole, Dir::E).unwrap();
    w.place(0, 12, 14, Kind::Furnace, Dir::E).unwrap();
    w.place(0, 30, 30, Kind::Drill, Dir::E).unwrap(); // outside the rectangle
    let money = w.econ.money;
    let want: f64 = [Kind::Solar.price(2), Kind::Pole.price(1), Kind::Furnace.price(3)].iter().sum::<f64>() + 40.0 * Kind::Belt.price(1);
    let before = rebuilds();
    let (rep, undo) = w.remove_area(0, (0, 0, 44, 20));
    assert_eq!(rebuilds() - before, 1, "one rebuild for the whole area");
    assert_eq!((rep.parts, rep.kept, rep.fee), (43, 0, 0.0));
    assert!((rep.refund - want).abs() < 1e-9 && (w.econ.money - money - want).abs() < 1e-9);
    assert_eq!(layout(&w, 0), vec![(Kind::Drill, Dir::E, 0)], "only the part outside is left");
    assert_eq!(undo.unwrap().len(), 43);
    // The grid agrees with taking them away one by one.
    let mut one = World::blank();
    one.place(0, 30, 30, Kind::Drill, Dir::E).unwrap();
    assert_eq!(w.grid.cover, one.grid.cover);
    assert_eq!(w.grid.unpowered, one.grid.unpowered);
    // An empty rectangle does nothing and does not rebuild.
    let before = rebuilds();
    let (rep, undo) = w.remove_area(0, (50, 50, 60, 60));
    assert_eq!((rep.parts, rebuilds() - before), (0, 0));
    assert!(undo.is_none());
}

#[test]
fn waste_storage_pays_its_fee_once_for_the_whole_area() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 8, 5, Kind::Crate, Dir::E).unwrap();
    waste_in_crate(&mut w, 0, 5, 5, 10);
    waste_in_crate(&mut w, 0, 8, 5, 6);
    let each = -Item::Waste.price();
    let info = w.area_info(0, (0, 0, 20, 20));
    assert_eq!((info.parts, info.waste), (2, 16));
    assert!((info.fee - 16.0 * each).abs() < 1e-9);
    let before = w.econ.money;
    let (rep, _) = w.remove_area(0, (0, 0, 20, 20));
    assert_eq!((rep.parts, rep.waste, rep.kept), (2, 16, 0));
    assert!((rep.fee - 16.0 * each).abs() < 1e-9);
    let refund = 2.0 * Kind::Crate.price(1);
    assert!((w.econ.money - (before + refund - 16.0 * each)).abs() < 1e-9, "refund minus one fee");
    assert_eq!(w.econ.sold[Item::Waste.idx()], 16, "counted as waste disposed, like selling it");
}

#[test]
fn a_part_whose_fee_cant_be_paid_is_kept() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 8, 5, Kind::Belt, Dir::E).unwrap();
    waste_in_crate(&mut w, 0, 5, 5, 20);
    w.econ.money = 0.0;
    let (rep, undo) = w.remove_area(0, (0, 0, 20, 20));
    // Reading order looks at the crate first, with no money in hand: it stays. The belt goes.
    assert_eq!((rep.parts, rep.kept), (1, 1));
    assert!(rep.kept_fee > 0.0);
    assert!(w.tile(0, 5, 5).is_some() && w.tile(0, 8, 5).is_none());
    assert_eq!(undo.unwrap().len(), 1);
    // With enough money it goes, and the money never dips below zero on the way.
    w.econ.money = rep.kept_fee;
    let (rep, _) = w.remove_area(0, (0, 0, 20, 20));
    assert_eq!((rep.parts, rep.kept), (1, 0));
    assert!(w.econ.money >= 0.0);
}

#[test]
fn an_elevator_goes_with_every_door_and_its_car() {
    // The rectangle touches one corner tile of a 3x3 elevator, from any floor it has a door on.
    for f in [0usize, 1, 2] {
        let mut w = World::blank();
        w.place_sized(0, 10, 10, Kind::Elevator, Dir::E, 3).unwrap();
        w.place(0, 20, 20, Kind::Belt, Dir::E).unwrap();
        assert_eq!(w.lifts.len(), 1);
        let before = w.econ.money;
        let (rep, _) = w.remove_area(f, (12, 12, 14, 14));
        assert_eq!((rep.parts, rep.elevators), (1, 1), "floor {f}");
        assert!(w.lifts.is_empty());
        for g in 0..w.floors.len() {
            assert!(w.floors[g].tiles[idx(10, 10)].is_none() && w.floors[g].owner[idx(12, 12)].is_none(), "floor {g}");
        }
        assert_eq!(w.econ.money, before + Kind::Elevator.price(3));
        assert!(w.tile(0, 20, 20).is_some());
    }
}

#[test]
fn an_elevator_holding_waste_charges_its_fee_once() {
    let mut w = World::blank();
    w.place_sized(0, 10, 10, Kind::Elevator, Dir::E, 2).unwrap();
    w.lifts[0].load.extend([Item::Waste; 5]);
    if let Some(Building { st: St::Elevator { outbox, .. }, .. }) = w.floors[1].tiles[idx(10, 10)].as_mut() {
        outbox.extend([Item::Waste; 3]);
    }
    let each = -Item::Waste.price();
    let before = w.econ.money;
    let (rep, _) = w.remove_area(0, (0, 0, 30, 30));
    assert_eq!((rep.parts, rep.waste), (1, 8));
    assert!((w.econ.money - (before + Kind::Elevator.price(2) - 8.0 * each)).abs() < 1e-9);
}

#[test]
fn a_part_on_another_floor_is_not_touched() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Belt, Dir::E).unwrap();
    w.place(2, 5, 5, Kind::Belt, Dir::E).unwrap();
    w.place(1, 6, 6, Kind::Belt, Dir::E).unwrap();
    let (rep, _) = w.remove_area(0, (0, 0, 20, 20));
    assert_eq!(rep.parts, 1);
    assert!(w.tile(2, 5, 5).is_some() && w.tile(1, 6, 6).is_some());
}

#[test]
fn restoring_puts_the_area_back_as_it_was() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Drill, Dir::E).unwrap();
    w.place(0, 7, 5, Kind::Belt, Dir::E).unwrap();
    w.place(0, 8, 4, Kind::Furnace, Dir::E).unwrap();
    w.place(0, 12, 4, Kind::Arm, Dir::E).unwrap();
    w.place_sized(0, 20, 10, Kind::Elevator, Dir::E, 2).unwrap();
    w.place(0, 3, 20, Kind::Solar, Dir::E).unwrap();
    w.place(0, 6, 20, Kind::Pole, Dir::E).unwrap();
    w.place(0, 9, 20, Kind::Molder, Dir::E).unwrap();
    w.configure(0, 12, 4); // the arm's filter
    w.configure(0, 20, 10); // the door mode
    run(&mut w, 5);
    let shape = layout(&w, 0);
    let doors = layout(&w, 1);
    let money = w.econ.money;
    let cover = w.grid.cover.clone();
    let setting = w.setting_at(0, 12, 4);
    let (_, undo) = w.remove_area(0, (0, 0, 40, 40));
    let mut undo = undo.unwrap();
    assert_eq!(undo.len(), 8);
    assert!(layout(&w, 0).is_empty() && layout(&w, 1).is_empty() && w.lifts.is_empty());
    assert!((undo.price() - (w.econ.money - money)).abs() < 1e-9);
    let before = rebuilds();
    let rep = w.restore_removed(&mut undo);
    assert_eq!(rebuilds() - before, 1);
    assert_eq!((rep.placed, rep.blocked, rep.short), (8, 0, 0));
    assert!(undo.is_empty());
    assert_eq!(layout(&w, 0), shape);
    assert_eq!(layout(&w, 1), doors);
    assert_eq!(w.lifts.len(), 1);
    assert!((w.econ.money - money).abs() < 1e-9, "remove and restore nets to nothing");
    assert_eq!(w.grid.cover, cover);
    assert_eq!(w.setting_at(0, 12, 4), setting);
    // The door mode on the ground floor and the unload door below came back as they were.
    assert!(matches!(w.tile(0, 20, 10).unwrap().st, St::Elevator { mode: DoorMode::Unload, .. }));
    assert!(matches!(w.tile(1, 20, 10).unwrap().st, St::Elevator { mode: DoorMode::Unload, .. }));
    run(&mut w, 5);
}

#[test]
fn restoring_skips_what_is_in_the_way_and_stops_when_money_runs_out() {
    let mut w = World::blank();
    for x in 5..15 {
        w.place(0, x, 5, Kind::Furnace, Dir::E).unwrap_or(());
    }
    // Furnaces are 3 wide: 5, 8, 11, 14 fit.
    let n = w.parts_in(0, (0, 0, 40, 40)).len();
    assert_eq!(n, 4);
    let money = w.econ.money;
    let (_, undo) = w.remove_area(0, (0, 0, 40, 40));
    let mut undo = undo.unwrap();
    // Something now sits where the second one was.
    w.place(0, 8, 5, Kind::Belt, Dir::E).unwrap();
    let rep = w.restore_removed(&mut undo);
    assert_eq!((rep.placed, rep.blocked, rep.short), (3, 1, 0));
    assert_eq!(undo.len(), 1, "the blocked one waits");
    // Clear the way and press again.
    w.remove(0, 8, 5);
    let rep = w.restore_removed(&mut undo);
    assert_eq!((rep.placed, rep.blocked), (1, 0));
    assert!(undo.is_empty());
    assert!((w.econ.money - money).abs() < 1e-9);
    // Not enough money: a clean cut, nothing goes below zero.
    let (_, undo) = w.remove_area(0, (0, 0, 40, 40));
    let mut undo = undo.unwrap();
    w.econ.money = Kind::Furnace.price(3) * 2.5;
    let rep = w.restore_removed(&mut undo);
    assert_eq!((rep.placed, rep.short), (2, 2));
    assert!(w.econ.money >= 0.0);
    assert_eq!(undo.len(), 2);
}
