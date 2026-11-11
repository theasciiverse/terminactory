//! Products leave a machine's front edge and waste leaves its back edge.

use super::*;

/// A coke oven (2 coal -> 1 coke + 1 waste, 2 s) at (20, 20) facing `dir`, with 40 coal.
fn oven(w: &mut World, dir: Dir) -> (i32, i32) {
    w.place(0, 20, 20, Kind::CokeOven, dir).unwrap();
    set_stock(w, 20, 20, Item::Coal, 40);
    (20, 20)
}

/// The first tile outside a machine's edge on side `edge`.
fn outside(w: &World, x: i32, y: i32, edge: Dir) -> (i32, i32) {
    front_tile(x, y, w.tile(0, x, y).unwrap().dims(), edge, 0)
}

fn made(w: &World, item: Item) -> u64 {
    w.econ.made[item.idx()]
}

#[test]
fn waste_exits_the_back_and_coke_the_front_whichever_way_the_machine_faces() {
    for dir in Dir::ALL {
        let mut w = World::blank();
        let (x, y) = oven(&mut w, dir);
        let (fx, fy) = outside(&w, x, y, dir);
        let (bx, by) = outside(&w, x, y, dir.opposite());
        w.place(0, fx, fy, Kind::Crate, dir).unwrap();
        w.place(0, bx, by, Kind::Crate, dir.opposite()).unwrap();
        run(&mut w, 7); // 2 s a batch: 3 batches
        let (front, back) = (crate_items(&w, 0, fx, fy), crate_items(&w, 0, bx, by));
        assert_eq!(front, vec![Item::Coke; 3], "{dir:?}: coke out the front");
        assert_eq!(back, vec![Item::Waste; 3], "{dir:?}: waste out the back");
    }
}

#[test]
fn waste_goes_nowhere_but_the_back() {
    // A crate at the front and another at the side: the side one gets nothing at all.
    let mut w = World::blank();
    let (x, y) = oven(&mut w, Dir::E);
    w.place(0, 22, 20, Kind::Crate, Dir::E).unwrap();
    w.place(0, 20, 22, Kind::Crate, Dir::E).unwrap();
    run(&mut w, 30);
    assert_eq!(crate_items(&w, 0, 22, 20), vec![Item::Coke; 10], "coke fills the front crate");
    assert!(crate_items(&w, 0, 20, 22).is_empty(), "nothing leaves by the side");
    // The waste has no way out, so the oven fills with it and stops.
    assert_eq!(out_waste(&w, x, y), 10);
}

fn out_waste(w: &World, x: i32, y: i32) -> usize {
    match &w.tile(0, x, y).unwrap().st {
        St::Crafter { out, .. } => out.iter().filter(|i| **i == Item::Waste).count(),
        _ => panic!("not a machine"),
    }
}

#[test]
fn a_machine_with_a_blocked_back_stops_and_says_so() {
    let mut w = World::blank();
    let (x, y) = oven(&mut w, Dir::E);
    w.place(0, 22, 20, Kind::Crate, Dir::E).unwrap(); // the front is clear
    w.place(0, 19, 20, Kind::Crate, Dir::W).unwrap(); // the back is not
    fill_crate(&mut w, 0, 19, 20, &[Item::Stone; CRATE_CAP]);
    assert_eq!(w.idle_reason(0, idx(x, y)), None, "it can work to begin with");
    run(&mut w, 60);
    // Its waste queue is full (10), so it stops after the batch that filled it.
    assert_eq!(made(&w, Item::Waste), 10);
    assert_eq!(made(&w, Item::Coke), 10);
    assert_eq!(w.idle_reason(0, idx(x, y)), Some("BACK"));
    assert_eq!(w.idle_summary(), (1, Some("BACK")));
    // The coke went out the front the whole time: only the waste is stuck.
    assert_eq!(crate_items(&w, 0, 22, 20), vec![Item::Coke; 10]);
    // Make room behind it and it carries on.
    if let Some(Building { st: St::Crate { items }, .. }) = w.floors[0].tiles[idx(19, 20)].as_mut() {
        items.clear();
    }
    run(&mut w, 10);
    assert!(made(&w, Item::Waste) > 10);
    assert!(crate_items(&w, 0, 19, 20).len() >= 10, "the stuck waste went out the back");
}

#[test]
fn nothing_behind_the_machine_also_stops_it_with_back() {
    let mut w = World::blank();
    let (x, y) = oven(&mut w, Dir::E);
    w.place(0, 22, 20, Kind::Crate, Dir::E).unwrap();
    run(&mut w, 60);
    assert_eq!(w.idle_reason(0, idx(x, y)), Some("BACK"));
    // A belt behind it with no power is the real reason, as it is for a blocked front.
    w.place(0, 19, 20, Kind::Belt, Dir::W).unwrap();
    assert_eq!(w.idle_reason(0, idx(x, y)), Some("EC?"));
}

#[test]
fn a_blocked_front_still_says_full_while_the_waste_leaves() {
    let mut w = World::blank();
    let (x, y) = oven(&mut w, Dir::E);
    w.place(0, 19, 20, Kind::Crate, Dir::W).unwrap(); // only the back has somewhere to go
    run(&mut w, 60);
    assert_eq!(w.idle_reason(0, idx(x, y)), Some("FULL"));
    assert_eq!(crate_items(&w, 0, 19, 20), vec![Item::Waste; 10], "the waste kept leaving");
    assert_eq!(out_waste(&w, x, y), 0);
}

#[test]
fn a_blast_furnace_sends_waste_back_and_molten_steel_forward() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::BlastFurnace, Dir::E).unwrap(); // covers 5-6, 5-6
    cycle_to(&mut w, 5, 5, "molten steel (coal)");
    set_stock(&mut w, 5, 5, Item::IronPlate, 2);
    set_stock(&mut w, 5, 5, Item::Coal, 2);
    w.place(0, 7, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 4, 6, Kind::Crate, Dir::W).unwrap();
    run(&mut w, 11);
    assert_eq!((w.fluids.pools[0].fluid, w.fluids.pools[0].amount), (Some(Fluid::MoltenSteel), 1.0), "steel out the front");
    assert_eq!(crate_items(&w, 0, 4, 6), vec![Item::Waste; 3], "3 waste out the back");
}
