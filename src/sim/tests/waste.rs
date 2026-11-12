//! What it costs to remove storage that holds waste (and what it doesn't).

use super::*;

const WASTE_EACH: f64 = 0.5;

fn elevator_door_items(w: &mut World, f: usize, inbox_waste: usize, outbox_waste: usize) {
    let a = idx(20, 20);
    if let Some(Building { st: St::Elevator { inbox, outbox, .. }, .. }) = w.floors[f].tiles[a].as_mut() {
        inbox.extend(std::iter::repeat_n(Item::Waste, inbox_waste));
        outbox.extend(std::iter::repeat_n(Item::Waste, outbox_waste));
    }
}

#[test]
fn a_warehouse_of_waste_costs_by_every_crate_it_holds() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Warehouse, Dir::E).unwrap();
    // Four crates' worth: a warehouse holds WAREHOUSE_CRATES times a crate.
    fill_crate(&mut w, 0, 5, 5, &vec![Item::Waste; CRATE_CAP * WAREHOUSE_CRATES]);
    let n = CRATE_CAP * WAREHOUSE_CRATES;
    assert_eq!(w.removal_fee(0, 5, 5), (n, n as f64 * WASTE_EACH));
    w.econ.money = n as f64 * WASTE_EACH - 1.0;
    assert!(w.try_remove(0, 5, 5).is_err());
    assert!(w.tile(0, 5, 5).is_some());
    w.econ.money = n as f64 * WASTE_EACH + 10.0;
    assert_eq!(w.try_remove(0, 5, 5), Ok(Some(Kind::Warehouse)));
}

#[test]
fn waste_in_an_elevators_inbox_and_outbox_counts_as_well_as_the_car() {
    let mut w = World::blank();
    w.place(0, 20, 20, Kind::Elevator, Dir::E).unwrap();
    let down = w.floor_of(-1).unwrap();
    elevator_door_items(&mut w, 0, 3, 0);
    elevator_door_items(&mut w, down, 0, 5);
    w.lifts[0].load.extend([Item::Waste, Item::Coal]);
    // Counted whichever floor you ask from.
    assert_eq!(w.waste_inside(0, 20, 20), 9);
    assert_eq!(w.waste_inside(down, 20, 20), 9);
    assert_eq!(w.removal_fee(down, 21, 21), (9, 4.5));
    w.econ.money = 4.0;
    assert!(w.try_remove(down, 20, 20).unwrap_err().starts_with("Holds 9 waste"));
    assert_eq!(w.floors.iter().filter(|fl| fl.tiles[idx(20, 20)].is_some()).count(), 3, "refused: every door stays");
}

#[test]
fn having_exactly_the_fee_is_enough_and_money_never_goes_negative() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
    fill_crate(&mut w, 0, 5, 5, &[Item::Waste; 40]); // $20
    let refund = Kind::Crate.price(1);
    w.econ.money = 20.0;
    assert_eq!(w.try_remove(0, 5, 5), Ok(Some(Kind::Crate)));
    assert_eq!(w.econ.money, refund, "the fee takes the money to zero and the refund comes back");
    assert!(w.econ.money >= 0.0);
    // A cent short: refused, and nothing changes.
    w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
    fill_crate(&mut w, 0, 5, 5, &[Item::Waste; 40]);
    w.econ.money = 19.99;
    assert!(w.try_remove(0, 5, 5).is_err());
    assert_eq!(w.econ.money, 19.99);
    assert_eq!(crate_items(&w, 0, 5, 5).len(), 40);
}

#[test]
fn a_paid_removal_counts_as_waste_sold() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
    fill_crate(&mut w, 0, 5, 5, &[Item::Waste; 10]);
    let before = w.econ.earned_money;
    w.try_remove(0, 5, 5).unwrap();
    assert_eq!(w.econ.sold[Item::Waste.idx()], 10);
    assert_eq!(w.econ.money_from[Item::Waste.idx()], -5.0);
    assert_eq!(w.econ.earned_money, before - 5.0);
    assert_eq!(w.econ.research_from[Item::Waste.idx()], 0.0, "no research either way");
}

#[test]
fn waste_in_machines_belts_and_tunnels_pays_the_fee_too() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
    if let Some(Building { st: St::Crafter { out, .. }, .. }) = w.floors[0].tiles[idx(5, 5)].as_mut() {
        out.extend([Item::Waste; 4]);
    }
    w.place(0, 20, 5, Kind::Belt, Dir::E).unwrap();
    if let Some(Building { st: St::Belt { items }, .. }) = w.floors[0].tiles[idx(20, 5)].as_mut() {
        items.push(BeltItem { item: Item::Waste, pos: 0.5 });
    }
    w.place(0, 30, 5, Kind::Tunnel, Dir::E).unwrap();
    if let Some(Building { st: St::Tunnel { items, .. }, .. }) = w.floors[0].tiles[idx(30, 5)].as_mut() {
        items.push_back((Item::Waste, 3.0));
    }
    w.place(0, 40, 5, Kind::Drill, Dir::E).unwrap();
    if let Some(Building { st: St::Drill { out, .. }, .. }) = w.floors[0].tiles[idx(40, 5)].as_mut() {
        out.push_back(Item::Waste);
    }
    if let Some(Building { st: St::Crafter { stock, .. }, .. }) = w.floors[0].tiles[idx(5, 5)].as_mut() {
        stock[Item::Waste] = 2;
    }
    for (x, kind, n) in [(5, Kind::Furnace, 6), (20, Kind::Belt, 1), (30, Kind::Tunnel, 1), (40, Kind::Drill, 1)] {
        let fee = n as f64 * WASTE_EACH;
        assert_eq!(w.removal_fee(0, x, 5), (n, fee), "{kind:?}");
        w.econ.money = 0.0;
        assert!(w.try_remove(0, x, 5).is_err(), "{kind:?} needs the fee");
        w.econ.money = fee;
        assert_eq!(w.try_remove(0, x, 5), Ok(Some(kind)));
        assert_eq!(w.econ.money, kind.price(kind.dims(Dir::E).0), "the refund, less the fee");
    }
    assert_eq!(w.econ.sold[Item::Waste.idx()], 9);
}

#[test]
fn remove_swallows_a_refusal_and_returns_none() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
    fill_crate(&mut w, 0, 5, 5, &[Item::Waste; 100]);
    w.econ.money = 1.0;
    assert_eq!(w.remove(0, 5, 5), None);
    assert!(w.tile(0, 5, 5).is_some(), "still there");
    assert_eq!(w.remove(0, 6, 6), None, "and an empty tile is None too");
    w.econ.money = 100.0;
    assert_eq!(w.remove(0, 5, 5), Some(Kind::Crate));
}
