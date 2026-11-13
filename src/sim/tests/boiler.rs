//! Boilers burn coal, and never waste.

use super::*;

const BOILER: (i32, i32) = (8, 5);

/// A pump, pipes, a boiler (8-9, 5-6) with its front on an engine (10-11, 5-6), and a solar panel
/// for the pump. With `load`, a precision assembler (64 EC) draws far more than the panel gives,
/// so the engine, and so the boiler, has work to do.
fn steam_plant(load: bool) -> World {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Pump, Dir::E).unwrap();
    w.place(0, 6, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 7, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, BOILER.0, BOILER.1, Kind::Boiler, Dir::E).unwrap();
    w.place(0, 10, 5, Kind::SteamEngine, Dir::E).unwrap();
    w.place(0, 5, 7, Kind::Solar, Dir::E).unwrap();
    if load {
        w.place(0, 13, 5, Kind::PrecisionAssembler, Dir::E).unwrap();
    }
    w
}

/// (fire in ticks, coal waiting) in the boiler.
fn fire(w: &World) -> (u32, u32) {
    match &w.tile(0, BOILER.0, BOILER.1).unwrap().st {
        St::Boiler { fuel, coal, .. } => (*fuel, *coal),
        _ => panic!("not a boiler"),
    }
}

fn busy(w: &World) -> bool {
    matches!(w.tile(0, BOILER.0, BOILER.1).unwrap().st, St::Boiler { busy: true, .. })
}

/// Ticks a boiler spends making steam (each one burns a tick of fire) over `secs`.
fn burn_ticks(w: &mut World, secs: u64) -> u32 {
    let mut n = 0;
    for _ in 0..secs * TPS {
        w.tick();
        n += busy(w) as u32;
    }
    n
}

/// Feeds the boiler by hand, as a belt would.
fn feed(w: &mut World, coal: u32) {
    for _ in 0..coal {
        assert!(w.offer(0, BOILER.0, BOILER.1, Item::Coal));
    }
}

#[test]
fn a_boiler_refuses_waste() {
    assert_eq!(Item::Waste.burn_ticks(), None);
    let mut w = steam_plant(true);
    assert!(!w.offer(0, BOILER.0, BOILER.1, Item::Waste));
    assert_eq!(fire(&w), (0, 0));
    // A crate pushing coal then waste keeps its waste: only the coal goes in.
    w.place(0, 8, 7, Kind::Crate, Dir::N).unwrap();
    fill_crate(&mut w, 0, 8, 7, &[Item::Coal, Item::Coal, Item::Waste]);
    run(&mut w, 5);
    assert_eq!(crate_items(&w, 0, 8, 7), vec![Item::Waste]);
    assert_eq!(w.econ.used[Item::Waste.idx()], 0, "no waste burnt");
}

#[test]
fn one_coal_makes_four_seconds_of_steam() {
    let mut w = steam_plant(true);
    feed(&mut w, 1);
    assert_eq!(burn_ticks(&mut w, 60), 240);
    assert_eq!(w.econ.used[Item::Coal.idx()], 1);
}

#[test]
fn a_boiler_burns_only_what_its_grid_needs() {
    // No load beyond the solar panel: the boiler fills its coal store and then holds it.
    let mut w = steam_plant(false);
    let mut taken = 0;
    while w.offer(0, BOILER.0, BOILER.1, Item::Coal) {
        taken += 1;
    }
    assert_eq!(taken, BOILER_COAL_CAP);
    // It first fills the engine's steam buffer (20 steam, 2 s of fire), then there is nothing to power.
    let primed = burn_ticks(&mut w, 10);
    assert!(primed > 0 && primed <= 2 * TPS as u32, "{primed}");
    let held = fire(&w);
    assert_eq!(burn_ticks(&mut w, 30), 0, "nothing to power, nothing burnt");
    assert_eq!(fire(&w), held);
    // Add a load and it burns again.
    w.place(0, 13, 5, Kind::PrecisionAssembler, Dir::E).unwrap();
    assert!(burn_ticks(&mut w, 5) > 0);
}

#[test]
fn other_machines_refuse_waste_as_fuel() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
    w.place(0, 20, 5, Kind::Kiln, Dir::E).unwrap();
    w.place(0, 30, 5, Kind::BlastFurnace, Dir::E).unwrap();
    for x in [5, 20, 30] {
        assert!(!w.offer(0, x, 5, Item::Waste), "{x}");
    }
}

/// A boiler against a coke oven's back doesn't take its waste.
#[test]
fn a_boiler_behind_a_coke_oven_takes_no_waste() {
    let mut w = World::blank();
    w.place(0, 12, 8, Kind::CokeOven, Dir::W).unwrap(); // covers 12-13, 8-9; back is east
    set_stock(&mut w, 12, 8, Item::Coal, 20);
    w.place(0, 11, 8, Kind::Crate, Dir::W).unwrap(); // the coke, out the front
    w.place(0, 14, 8, Kind::Boiler, Dir::E).unwrap();
    run(&mut w, 30);
    assert_eq!(w.econ.used[Item::Waste.idx()], 0, "the boiler took no waste");
}
