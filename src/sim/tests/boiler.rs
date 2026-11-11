//! Boilers burn coal, and waste at 4 to the coal.

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

/// Feeds the boiler by hand: `coal` coal and `waste` waste, each as a belt would.
fn feed(w: &mut World, coal: u32, waste: u32) {
    for _ in 0..coal {
        assert!(w.offer(0, BOILER.0, BOILER.1, Item::Coal));
    }
    for _ in 0..waste {
        assert!(w.offer(0, BOILER.0, BOILER.1, Item::Waste));
    }
}

#[test]
fn four_waste_make_the_fire_of_one_coal() {
    assert_eq!(WASTE_PER_COAL, 4);
    assert_eq!(Item::Waste.burn_ticks().unwrap() * 4, Item::Coal.burn_ticks().unwrap());
    // Waste goes straight into the fire: 4 of them are one coal's 4 seconds.
    let mut w = steam_plant(true);
    feed(&mut w, 0, 4);
    assert_eq!(fire(&w), (Item::Coal.burn_ticks().unwrap(), 0));
    // Burnt under load, 4 waste make steam for as many ticks as 1 coal does, and 3 for three quarters of that.
    let (coal_ticks, waste_ticks, three_ticks) = {
        let mut a = steam_plant(true);
        feed(&mut a, 1, 0);
        let mut b = steam_plant(true);
        feed(&mut b, 0, 4);
        let mut c = steam_plant(true);
        feed(&mut c, 0, 3);
        (burn_ticks(&mut a, 60), burn_ticks(&mut b, 60), burn_ticks(&mut c, 60))
    };
    assert_eq!(coal_ticks, 240);
    assert_eq!(waste_ticks, coal_ticks, "4 waste = 1 coal");
    assert_eq!(three_ticks, coal_ticks * 3 / 4);
    assert_eq!(w.econ.used[Item::Waste.idx()], 4, "burnt waste counts as used");
}

#[test]
fn a_boiler_fed_coal_and_waste_burns_both() {
    let mut w = steam_plant(true);
    // A crate behind the boiler's side pushes a mix up into it: 2 coal and 8 waste.
    w.place(0, 8, 7, Kind::Crate, Dir::N).unwrap();
    let mix =
        [Item::Coal, Item::Waste, Item::Waste, Item::Coal, Item::Waste, Item::Waste, Item::Waste, Item::Waste, Item::Waste, Item::Waste];
    fill_crate(&mut w, 0, 8, 7, &mix);
    let ticks = burn_ticks(&mut w, 90);
    assert!(crate_items(&w, 0, 8, 7).is_empty(), "everything went in");
    assert_eq!(ticks, 2 * 240 + 8 * 60, "2 coal and 8 waste = 4 coal of fire");
    assert_eq!((w.econ.used[Item::Coal.idx()], w.econ.used[Item::Waste.idx()]), (2, 8));
    assert_eq!(fire(&w), (0, 0));
}

#[test]
fn a_boiler_burns_only_what_its_grid_needs() {
    // No load beyond the solar panel: the boiler fills its store with waste and then holds the rest back.
    let mut w = steam_plant(false);
    let cap = BOILER_COAL_CAP * Item::Coal.burn_ticks().unwrap();
    let mut taken = 0;
    while w.offer(0, BOILER.0, BOILER.1, Item::Waste) {
        taken += 1;
    }
    assert_eq!(taken, BOILER_COAL_CAP * WASTE_PER_COAL, "a store of 10 coal's fire, in waste");
    assert_eq!(fire(&w).0, cap);
    // It first fills the engine's steam buffer (20 steam, 2 s of fire), then there is nothing to power.
    let primed = burn_ticks(&mut w, 10);
    assert!(primed > 0 && primed <= 2 * TPS as u32, "{primed}");
    let held = fire(&w).0;
    assert_eq!(held, cap - primed);
    assert_eq!(burn_ticks(&mut w, 30), 0, "nothing to power, nothing burnt");
    assert_eq!(fire(&w).0, held);
    // Add a load and it burns, and takes more waste as it goes.
    w.place(0, 13, 5, Kind::PrecisionAssembler, Dir::E).unwrap();
    assert!(burn_ticks(&mut w, 5) > 0);
    assert!(fire(&w).0 < held);
    assert!(w.offer(0, BOILER.0, BOILER.1, Item::Waste));
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

/// A coke oven's waste can go straight into a boiler against its back.
#[test]
fn a_coke_oven_can_feed_a_boiler_with_its_waste() {
    let mut w = World::blank();
    // The oven faces west, so its back is its east side: a boiler against it takes the waste.
    w.place(0, 12, 8, Kind::CokeOven, Dir::W).unwrap(); // covers 12-13, 8-9
    set_stock(&mut w, 12, 8, Item::Coal, 20);
    w.place(0, 11, 8, Kind::Crate, Dir::W).unwrap(); // the coke, out the front
    w.place(0, 14, 8, Kind::Boiler, Dir::E).unwrap();
    run(&mut w, 30);
    assert_eq!(crate_items(&w, 0, 11, 8), vec![Item::Coke; 10]);
    assert_eq!(w.econ.used[Item::Waste.idx()], 10, "the oven's waste went into the boiler behind it");
}
