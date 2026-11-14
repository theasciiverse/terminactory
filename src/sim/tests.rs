use super::*;

mod area;
mod batch;
mod blueprints;
mod boiler;
mod elevators;
mod keys;
mod molten;
mod names;
mod offline;
mod others;
mod perf;
mod ports;
mod saving;
mod solar;
mod tunnels;
mod upgrade;
mod waste;

fn run(w: &mut World, secs: u64) {
    for _ in 0..secs * TPS {
        w.tick();
    }
}

fn crate_items(w: &World, f: usize, x: i32, y: i32) -> Vec<Item> {
    match &w.tile(f, x, y).unwrap().st {
        St::Crate { items } => items.iter().copied().collect(),
        _ => panic!("not a crate"),
    }
}

fn fill_crate(w: &mut World, f: usize, x: i32, y: i32, items: &[Item]) {
    if let Some(Building { st: St::Crate { items: c }, .. }) = w.floors[f].tiles[idx(x, y)].as_mut() {
        c.extend(items.iter().copied());
    }
}

#[test]
fn drill_belt_trade_post() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Drill, Dir::E).unwrap(); // covers 5-6, 5-6; front is x=7
    for x in 7..=9 {
        w.place(0, x, 5, Kind::Belt, Dir::E).unwrap();
    }
    w.place(0, 10, 4, Kind::TradePost, Dir::E).unwrap(); // covers 10-12, 4-6
    w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap(); // reaches 5-10, 5-10
    let before = w.econ.money;
    run(&mut w, 30);
    // The starter fuel mines exactly 8 ores ($0.50 each), then the drill waits for coal.
    assert_eq!(w.econ.sold[Item::CopperOre.idx()], 8);
    assert!((w.econ.money - before - 4.0).abs() < 1e-9);
}

#[test]
fn copper_chain_drill_furnace_molder_press_trade_post() {
    let mut w = World::blank();
    // Copper drill -> furnace -> molder -> press, each pushing straight into the next.
    w.place(0, 5, 14, Kind::Drill, Dir::E).unwrap();
    w.place(0, 7, 14, Kind::Furnace, Dir::E).unwrap(); // covers 7-9, 14-16
    w.place(0, 7, 13, Kind::Crate, Dir::S).unwrap(); // coal, pushed down into the furnace
    fill_crate(&mut w, 0, 7, 13, &[Item::Coal; 12]);
    w.place(0, 10, 14, Kind::Molder, Dir::E).unwrap();
    w.place(0, 12, 14, Kind::Press, Dir::E).unwrap(); // makes copper plates first
    w.place(0, 14, 14, Kind::Belt, Dir::E).unwrap();
    w.place(0, 15, 13, Kind::TradePost, Dir::E).unwrap();
    w.place(0, 9, 11, Kind::Solar, Dir::E).unwrap();
    w.place(0, 13, 12, Kind::Pole, Dir::E).unwrap();
    run(&mut w, 100);
    // The drill only had its starter fuel: 8 ores -> 8 molten copper -> 8 ingots -> 8 plates.
    assert_eq!(w.econ.made[Item::CopperIngot.idx()], 8);
    assert_eq!(w.econ.sold[Item::CopperPlate.idx()], 8);
    assert_eq!(w.econ.sold[Item::CopperIngot.idx()], 0, "the press ate them all");
}

#[test]
fn a_fresh_game_can_start_the_copper_chain_and_reach_the_press() {
    let mut w = World::new();
    // Drill -> furnace -> molder -> trade post, all on the starting $1000 (and no research).
    w.place(0, 10, 10, Kind::Drill, Dir::E).unwrap();
    w.place(0, 12, 10, Kind::Furnace, Dir::E).unwrap();
    w.place(0, 12, 9, Kind::Crate, Dir::S).unwrap(); // coal for the furnace
    fill_crate(&mut w, 0, 12, 9, &[Item::Coal; 12]);
    w.place(0, 15, 10, Kind::Molder, Dir::E).unwrap();
    w.place(0, 17, 9, Kind::TradePost, Dir::E).unwrap();
    w.place(0, 15, 13, Kind::Solar, Dir::E).unwrap();
    assert!(w.check_place(0, 30, 30, Kind::Press).unwrap_err().contains("locked"));
    let before = w.econ.money;
    run(&mut w, 100);
    assert_eq!(w.econ.sold[Item::CopperIngot.idx()], 8);
    assert!((w.econ.money - before - 16.0).abs() < 1e-9, "ingots sell for $2");
    // 50 ingots open the press for free.
    w.econ.made[Item::CopperIngot.idx()] = 50;
    w.tick();
    assert!(w.check_place(0, 30, 30, Kind::Press).is_ok());
}

/// Sets what a machine has in stock (tests only).
fn set_stock(w: &mut World, x: i32, y: i32, item: Item, n: u32) {
    let a = w.floors[0].anchor_at(x, y).unwrap();
    if let Some(Building { st: St::Crafter { stock, .. }, .. }) = w.floors[0].tiles[a].as_mut() {
        stock[item.idx()] = n;
    }
}

/// (fluid taken in, fluid waiting to leave) of a machine.
fn crafter_fluid(w: &World, x: i32, y: i32) -> (u32, u32) {
    match &w.tile(0, x, y).unwrap().st {
        St::Crafter { fluid, fluid_out, .. } => (*fluid, fluid_out[0]),
        _ => panic!("not a machine"),
    }
}

#[test]
fn furnace_feeds_a_molder_directly() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap(); // covers 5-7, 5-7
    set_stock(&mut w, 5, 5, Item::CopperOre, 6);
    set_stock(&mut w, 5, 5, Item::Coal, 10);
    w.place(0, 8, 5, Kind::Molder, Dir::E).unwrap(); // takes the molten copper, no pipe
    w.place(0, 10, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 8, 7, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 40);
    assert_eq!(crate_items(&w, 0, 10, 5), vec![Item::CopperIngot; 6]);
}

#[test]
fn furnace_pipe_molder() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
    set_stock(&mut w, 5, 5, Item::CopperOre, 4);
    set_stock(&mut w, 5, 5, Item::Coal, 10);
    w.place(0, 8, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 9, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 10, 4, Kind::Molder, Dir::E).unwrap(); // the pipe touches its back side
    w.place(0, 12, 4, Kind::Crate, Dir::E).unwrap();
    // Set to iron, the molder leaves the copper in the pipe: it's carrying molten copper.
    w.configure(0, 10, 4);
    run(&mut w, 8);
    assert_eq!((w.fluids.pools[0].fluid, w.fluids.pools[0].amount), (Some(Fluid::MoltenCopper), 1.0));
    assert_eq!(crafter_fluid(&w, 10, 4).0, 0);
    // Back to copper (iron -> steel -> silicon -> copper) and with EC, it casts them all.
    w.configure(0, 10, 4);
    w.configure(0, 10, 4);
    w.configure(0, 10, 4);
    w.place(0, 10, 6, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 40);
    assert_eq!(crate_items(&w, 0, 12, 4), vec![Item::CopperIngot; 4]);
    assert_eq!(w.fluids.pools[0].amount, 0.0);
    assert_eq!(w.fluids.pools[0].fluid, None, "an empty network holds nothing");
}

#[test]
fn a_pipe_network_holds_one_fluid() {
    let mut p = Pool { fluid: None, amount: 0.0, cap: 20.0, pipes: 1 };
    assert_eq!(p.add(Fluid::MoltenCopper, 5.0), 5.0);
    assert_eq!(p.add(Fluid::Water, 5.0), 0.0, "no second fluid while it holds one");
    assert_eq!(p.take(Fluid::Water, 5.0), 0.0);
    assert_eq!(p.take(Fluid::MoltenCopper, 5.0), 5.0);
    assert_eq!(p.add(Fluid::Water, 5.0), 5.0, "once empty it takes anything");
    // In the world: a furnace can't push copper into a pipe full of water.
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
    set_stock(&mut w, 5, 5, Item::CopperOre, 20);
    set_stock(&mut w, 5, 5, Item::Coal, 30);
    w.place(0, 8, 5, Kind::Pipe, Dir::E).unwrap();
    w.fluids.pools[0].add(Fluid::Water, 3.0);
    run(&mut w, 80);
    assert_eq!(w.fluids.pools[0].fluid, Some(Fluid::Water));
    assert_eq!(w.fluids.pools[0].amount, 3.0);
    assert_eq!(crafter_fluid(&w, 5, 5).1, CRAFTER_FLUID_OUT_CAP, "the molten copper has nowhere to go");
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FULL"));
}

#[test]
fn a_full_fluid_output_stops_the_machine() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
    set_stock(&mut w, 5, 5, Item::CopperOre, 100);
    set_stock(&mut w, 5, 5, Item::Coal, 100);
    w.place(0, 8, 5, Kind::Pipe, Dir::E).unwrap(); // one pipe holds 20 units, and nothing drains it
    run(&mut w, 250);
    assert_eq!(w.fluids.pools[0].amount, PIPE_CAP);
    assert_eq!(crafter_fluid(&w, 5, 5), (0, CRAFTER_FLUID_OUT_CAP));
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FULL"));
    let made = w.econ.used[Item::CopperOre.idx()];
    run(&mut w, 30);
    assert_eq!(w.econ.used[Item::CopperOre.idx()], made, "it stops using ore too");
    // Draining the pipe lets it carry on.
    w.fluids.pools[0].take(Fluid::MoltenCopper, 20.0);
    run(&mut w, 30);
    assert!(w.econ.used[Item::CopperOre.idx()] > made);
}

#[test]
fn a_machine_with_two_outputs_needs_room_for_both() {
    use Item::*;
    static TWO_OUT: [Recipe; 1] = [Recipe {
        name: "test",
        inputs: &[(Coal, 1)],
        outputs: &[(Coke, 1)],
        fluid_in: None,
        fluid_out: Some((Fluid::MoltenCopper, 1)),
        fluid_out2: None,
        ticks: 60,
        fuel: None,
        needs: None,
        tag: 'T',
    }];
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &TWO_OUT))));
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Molder, Dir::E).unwrap(); // front is x=7, y=5 and 6
    set_stock(&mut w, 5, 5, Coal, 30);
    w.place(0, 7, 5, Kind::Pipe, Dir::E).unwrap(); // fluid out here...
    w.place(0, 7, 6, Kind::Crate, Dir::E).unwrap(); // ...and items out here
    w.place(0, 5, 7, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 5);
    assert_eq!(crate_items(&w, 0, 7, 6), vec![Coke; 5]);
    assert_eq!(w.fluids.pools[0].amount, 5.0);
    // The pipe fills up with something else: the metal can't leave, so the items stop too.
    w.fluids.pools[0].take(Fluid::MoltenCopper, 5.0);
    w.fluids.pools[0].add(Fluid::Water, 1.0);
    run(&mut w, 40);
    assert_eq!(crate_items(&w, 0, 7, 6).len(), 5 + CRAFTER_FLUID_OUT_CAP as usize);
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FULL"));
    RECIPE_OVERRIDE.with(|o| o.set(None));
}

#[test]
fn molders_follow_their_recipe_and_idle_without_metal() {
    let mut w = World::blank();
    w.place(0, 12, 5, Kind::Molder, Dir::E).unwrap();
    w.tick();
    assert_eq!(w.idle_reason(0, idx(12, 5)), Some("FLUD"));
    // A copper furnace can't pour into a molder set to iron.
    w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
    w.place(0, 8, 5, Kind::Molder, Dir::E).unwrap();
    assert!(w.configure(0, 8, 5).unwrap().contains("iron ingot"));
    set_stock(&mut w, 5, 5, Item::CopperOre, 20);
    set_stock(&mut w, 5, 5, Item::Coal, 30);
    run(&mut w, 80);
    assert_eq!(crafter_fluid(&w, 8, 5), (0, 0));
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FULL"), "and the furnace waits");
    assert!(w.configure(0, 8, 5).unwrap().contains("steel"));
    assert!(w.configure(0, 8, 5).unwrap().contains("silicon ingot"));
    assert!(w.configure(0, 8, 5).unwrap().contains("copper ingot"));
}

#[test]
fn presses_make_plates_from_ingots() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Press, Dir::E).unwrap();
    set_stock(&mut w, 5, 5, Item::CopperIngot, 2);
    w.place(0, 7, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 5, 7, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 8);
    assert_eq!(crate_items(&w, 0, 7, 5), vec![Item::CopperPlate; 2]);
    assert!(w.configure(0, 5, 5).unwrap().contains("iron plate"));
    set_stock(&mut w, 5, 5, Item::IronIngot, 1);
    run(&mut w, 4);
    assert_eq!(crate_items(&w, 0, 7, 5).last(), Some(&Item::IronPlate));
}

#[test]
fn tunnels_carry_items_under_other_things() {
    use Item::*;
    let mut w = World::blank();
    // Crate -> entrance, a belt line crossing above, exit 4 tiles on -> crate.
    w.place(0, 5, 10, Kind::Crate, Dir::E).unwrap();
    fill_crate(&mut w, 0, 5, 10, &[Coal; 10]);
    w.place(0, 6, 10, Kind::Tunnel, Dir::E).unwrap();
    for x in 7..=9 {
        w.place(0, x, 10, Kind::Belt, Dir::N).unwrap();
    }
    assert!(w.tunnel_exit(0, 6, 10).is_none(), "no exit yet");
    w.place(0, 10, 10, Kind::Tunnel, Dir::E).unwrap();
    assert_eq!(w.tunnel_exit(0, 6, 10), Some(4));
    assert!(matches!(w.tile(0, 10, 10).unwrap().st, St::Tunnel { exit: true, .. }));
    w.place(0, 11, 10, Kind::Crate, Dir::E).unwrap();
    run(&mut w, 10);
    assert_eq!(crate_items(&w, 0, 11, 10), vec![Coal; 10]);
    // A tunnel right after an exit starts a new pair; one too far away is just an entrance.
    w.place(0, 20, 20, Kind::Tunnel, Dir::E).unwrap();
    w.place(0, 26, 20, Kind::Tunnel, Dir::E).unwrap();
    assert!(matches!(w.tile(0, 26, 20).unwrap().st, St::Tunnel { exit: false, .. }));
    assert!(w.tunnel_exit(0, 20, 20).is_none());
}

#[test]
fn long_arms_reach_over_a_tile() {
    use Item::*;
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Crate, Dir::N).unwrap();
    fill_crate(&mut w, 0, 5, 5, &[IronOre; 3]);
    w.place(0, 6, 5, Kind::Pipe, Dir::E).unwrap(); // in the way, untouched
    w.place(0, 7, 5, Kind::LongArm, Dir::E).unwrap();
    w.place(0, 9, 5, Kind::Crate, Dir::N).unwrap();
    w.place(0, 7, 7, Kind::Pole, Dir::E).unwrap();
    w.place(0, 5, 8, Kind::Solar, Dir::E).unwrap();
    w.place(0, 9, 8, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 10);
    assert_eq!(crate_items(&w, 0, 9, 5), vec![IronOre; 3]);
}

#[test]
fn filter_splitters_send_one_item_ahead() {
    use Item::*;
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
    fill_crate(&mut w, 0, 5, 5, &[Coal, CopperOre, Coal, CopperOre, CopperOre, Coal]);
    w.place(0, 6, 5, Kind::Sorter, Dir::E).unwrap();
    while !w.configure(0, 6, 5).unwrap().contains("coal out") {}
    w.place(0, 7, 5, Kind::Belt, Dir::E).unwrap();
    w.place(0, 8, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 6, 4, Kind::Belt, Dir::N).unwrap();
    w.place(0, 6, 3, Kind::Crate, Dir::N).unwrap();
    w.place(0, 6, 6, Kind::Belt, Dir::S).unwrap();
    w.place(0, 6, 7, Kind::Crate, Dir::S).unwrap();
    w.place(0, 9, 6, Kind::Pole, Dir::E).unwrap();
    w.place(0, 10, 7, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 10);
    assert_eq!(crate_items(&w, 0, 8, 5), vec![Coal; 3]);
    let sides = [crate_items(&w, 0, 6, 3), crate_items(&w, 0, 6, 7)].concat();
    assert_eq!(sides, vec![CopperOre; 3]);
}

#[test]
fn warehouses_hold_four_crates() {
    let mut w = World::blank();
    w.tick();
    w.place(0, 5, 5, Kind::Warehouse, Dir::E).unwrap();
    let Floor { tiles, owner, .. } = &mut w.floors[0];
    let mut n = 0;
    while try_insert(tiles, owner, &mut w.econ, 6, 6, Item::Coal, Dir::E) {
        n += 1;
    }
    assert_eq!(n, CRATE_CAP * WAREHOUSE_CRATES);
}

#[test]
fn production_panel_counts_the_last_minute() {
    let mut w = World::new();
    w.build_demo().unwrap();
    run(&mut w, 100);
    let (rows, secs) = w.flow();
    assert_eq!(secs, 60);
    let get = |it: Item| rows.iter().find(|r| r.item == it).copied().unwrap();
    // The furnace melts one ore per 5 s: about 12 a minute all the way down the chain.
    let plate = get(Item::CopperPlate);
    assert!(plate.made > 8.0 && plate.sold > 8.0 && plate.research > 0.0 && plate.money > 0.0, "{plate:?}");
    let ingot = get(Item::CopperIngot);
    assert!((ingot.made - plate.made).abs() <= 2.0 && (ingot.used - plate.made).abs() <= 2.0, "{ingot:?} {plate:?}");
    let ore = get(Item::CopperOre);
    assert!(ore.made > 8.0 && (ore.used - ingot.made).abs() <= 2.0, "furnaces use one ore per ingot: {ore:?} {ingot:?}");
    assert!(get(Item::Coal).used > 0.0, "fuel counts as used");
}

#[test]
fn demo_factory_runs() {
    let mut w = World::new();
    w.build_demo().unwrap();
    run(&mut w, 200);
    assert!(w.econ.sold[Item::CopperPlate.idx()] > 20, "sold {:?}", w.econ.sold);
    assert!(w.econ.research > 0.0);
    // Everything in the chain has full EC.
    assert_eq!(w.grid.unpowered, 0);
    assert_eq!(w.building_ratio(0, idx(56, 37)), 1.0, "molder");
    assert_eq!(w.building_ratio(0, idx(58, 37)), 1.0, "press");
}

#[test]
fn arm_filters() {
    let mut w = World::blank();
    w.place(0, 5, 20, Kind::Crate, Dir::E).unwrap();
    fill_crate(&mut w, 0, 5, 20, &[Item::CopperOre, Item::Coal, Item::CopperOre, Item::Coal]);
    w.place(0, 6, 20, Kind::Arm, Dir::E).unwrap();
    w.configure(0, 6, 20); // copper ore
    w.configure(0, 6, 20); // coal
    w.place(0, 7, 20, Kind::Crate, Dir::E).unwrap();
    w.place(0, 6, 22, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 10);
    assert_eq!(crate_items(&w, 0, 7, 20), vec![Item::Coal, Item::Coal]);
}

#[test]
fn elevator_moves_items_between_floors() {
    let mut w = World::blank();
    w.place(0, 8, 10, Kind::Crate, Dir::E).unwrap();
    fill_crate(&mut w, 0, 8, 10, &[Item::CopperOre; 40]);
    w.place(0, 9, 10, Kind::Belt, Dir::E).unwrap();
    w.place(0, 10, 10, Kind::Elevator, Dir::E).unwrap(); // 2x2: holds 40; doors on ground, -1 and +1
    w.place(1, 12, 10, Kind::Belt, Dir::E).unwrap();
    w.place(1, 13, 10, Kind::Crate, Dir::E).unwrap();
    // Power reaches the underground belt down the elevator shaft.
    w.place(0, 8, 12, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 4);
    assert_eq!(w.lifts[0].phase, LiftPhase::Loading, "it waits until it's full");
    run(&mut w, 16);
    assert_eq!(crate_items(&w, 1, 13, 10).len(), 40);
}

#[test]
fn elevator_timing_follows_the_rules() {
    let mut w = World::blank();
    w.place_sized(0, 10, 10, Kind::Elevator, Dir::E, 3).unwrap();
    assert_eq!(w.lifts[0].cap(), 90);
    // Fill the car directly and watch it go: 0.5 s doors, 1 s per floor, 0.5 s doors.
    w.lifts[0].load.extend([Item::Coal; 90]);
    w.tick(); // full: starts closing
    assert_eq!(w.lifts[0].phase, LiftPhase::Closing);
    for _ in 0..DOOR_TICKS {
        w.tick();
    }
    assert_eq!(w.lifts[0].phase, LiftPhase::Moving);
    for _ in 0..FLOOR_TICKS {
        w.tick();
    }
    assert_eq!(w.lifts[0].phase, LiftPhase::Opening);
    for _ in 0..DOOR_TICKS {
        w.tick();
    }
    assert_eq!(w.lifts[0].phase, LiftPhase::Unloading);
    // Unloading is quick but not instant.
    assert!(!w.lifts[0].load.is_empty());
}

#[test]
fn elevators_come_in_sizes_and_cost_by_size() {
    let mut w = World::blank();
    let before = w.econ.money;
    w.place_sized(0, 20, 20, Kind::Elevator, Dir::E, 8).unwrap();
    assert_eq!(w.anchor_of(0, 27, 27), Some((20, 20)));
    assert_eq!(w.anchor_of(1, 27, 27), Some((20, 20)), "the door is on the floor below too");
    assert_eq!(before - w.econ.money, 75.0 * 64.0);
    assert_eq!(w.remove(1, 21, 21), Some(Kind::Elevator));
    assert!(w.tile(0, 20, 20).is_none() && w.lifts.is_empty());
}

#[test]
fn big_buildings_block_their_whole_footprint() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::TradePost, Dir::E).unwrap();
    assert!(w.check_place(0, 12, 12, Kind::Belt).is_err());
    assert!(w.check_place(0, 13, 12, Kind::Belt).is_ok());
    assert_eq!(w.anchor_of(0, 12, 11), Some((10, 10)));
    assert_eq!(w.remove(0, 11, 12), Some(Kind::TradePost));
    assert!(w.check_place(0, 12, 12, Kind::Belt).is_ok());
}

#[test]
fn rectangles_turn_with_the_building() {
    assert_eq!(Kind::Press.dims(Dir::E), (2, 1));
    assert_eq!(Kind::Press.dims(Dir::W), (2, 1));
    assert_eq!(Kind::Press.dims(Dir::N), (1, 2));
    assert_eq!(Kind::Press.dims(Dir::S), (1, 2));
    assert_eq!(Kind::Pump.dims(Dir::E), (1, 2));
    assert_eq!(Kind::Pump.dims(Dir::N), (2, 1));
    assert_eq!(Kind::Furnace.dims(Dir::N), (3, 3));
    assert_eq!(Kind::Belt.dims(Dir::N), (1, 1));
    assert_eq!(dims_of(Kind::Elevator, Dir::N, 5), (5, 5));
}

#[test]
fn press_blocks_two_tiles_and_turns_to_one_by_two() {
    let mut w = World::blank();
    let money = w.econ.money;
    w.place(0, 10, 10, Kind::Press, Dir::E).unwrap();
    for (x, y) in [(10, 10), (11, 10)] {
        assert_eq!(w.anchor_of(0, x, y), Some((10, 10)));
        assert!(w.check_place(0, x, y, Kind::Belt).is_err());
    }
    for (x, y) in [(12, 10), (10, 11), (11, 11), (10, 9)] {
        assert!(w.check_place(0, x, y, Kind::Belt).is_ok(), "{x},{y} should be free");
    }
    // Removing it from its far tile frees both and refunds in full.
    assert_eq!(w.remove(0, 11, 10), Some(Kind::Press));
    assert_eq!(w.econ.money, money);
    assert!(w.check_place(0, 10, 10, Kind::Belt).is_ok() && w.check_place(0, 11, 10, Kind::Belt).is_ok());

    // Facing north it is one wide and two tall.
    w.place(0, 10, 10, Kind::Press, Dir::N).unwrap();
    for (x, y) in [(10, 10), (10, 11)] {
        assert_eq!(w.anchor_of(0, x, y), Some((10, 10)));
    }
    for (x, y) in [(11, 10), (11, 11), (10, 12)] {
        assert!(w.check_place(0, x, y, Kind::Belt).is_ok(), "{x},{y} should be free");
    }
    // The check follows the direction: a belt on (10, 11) blocks a north press, not an east one.
    let mut w = World::blank();
    w.place(0, 10, 11, Kind::Belt, Dir::E).unwrap();
    assert_eq!(w.check_place_sized(0, 10, 10, Kind::Press, Dir::N, 0), Err("Something is in the way".into()));
    assert!(w.check_place_sized(0, 10, 10, Kind::Press, Dir::E, 0).is_ok());
}

#[test]
fn placing_off_the_map_is_refused_for_long_machines() {
    let w = World::blank();
    let fit = |x, y, k, d| w.check_place_sized(0, x, y, k, d, 0);
    assert!(fit(EDGE - 2, 5, Kind::Press, Dir::E).is_ok());
    assert_eq!(fit(EDGE - 1, 5, Kind::Press, Dir::E), Err("Doesn't fit on the map here".into()));
    assert!(fit(EDGE - 1, 5, Kind::Press, Dir::N).is_ok());
    assert_eq!(fit(5, EDGE - 1, Kind::Press, Dir::N), Err("Doesn't fit on the map here".into()));
    assert!(fit(5, EDGE - 1, Kind::Press, Dir::E).is_ok());
    assert_eq!(fit(EDGE - 2, 5, Kind::Furnace, Dir::E), Err("Doesn't fit on the map here".into()));
}

#[test]
fn furnace_is_three_by_three() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Furnace, Dir::E).unwrap();
    for (x, y) in footprint(10, 10, 3, 3) {
        assert_eq!(w.anchor_of(0, x, y), Some((10, 10)));
        assert!(w.check_place(0, x, y, Kind::Belt).is_err());
    }
    assert!(w.check_place(0, 13, 12, Kind::Belt).is_ok());
    assert!(w.check_place(0, 12, 13, Kind::Belt).is_ok());
    assert_eq!(w.remove(0, 12, 12), Some(Kind::Furnace));
    assert!(footprint(10, 10, 3, 3).all(|(x, y)| w.check_place(0, x, y, Kind::Belt).is_ok()));
}

#[test]
fn pump_is_one_by_two() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Pump, Dir::E).unwrap();
    assert_eq!(w.anchor_of(0, 5, 6), Some((5, 5)));
    assert!(w.check_place(0, 6, 5, Kind::Pipe).is_ok());
    assert!(w.check_place(0, 5, 7, Kind::Pipe).is_ok());
    // A pipe beside its second tile is on its network too.
    w.place(0, 6, 6, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 7, 5, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 3);
    assert!(w.fluids.pools[0].amount > 0.0);
    // Turned north it lies down: two wide, one tall.
    assert_eq!(w.remove(0, 5, 6), Some(Kind::Pump));
    w.place(0, 5, 5, Kind::Pump, Dir::N).unwrap();
    assert_eq!(w.anchor_of(0, 6, 5), Some((5, 5)));
    assert!(w.check_place(0, 5, 6, Kind::Pipe).is_ok());
}

#[test]
fn the_oil_extractor_pumps_crude_oil_into_a_pipe_at_its_front() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::OilExtractor, Dir::E).unwrap(); // 1 wide, 5 tall: front is x=6
    w.place(0, 6, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 2);
    assert_eq!(w.econ.made_fluid[Fluid::CrudeOil], 1);
    assert_eq!((w.fluids.pools[0].fluid, w.fluids.pools[0].amount), (Some(Fluid::CrudeOil), 1.0));
    // Not every tick: the next unit takes another two seconds.
    run(&mut w, 1);
    assert_eq!(w.econ.made_fluid[Fluid::CrudeOil], 1);
    run(&mut w, 1);
    assert_eq!(w.econ.made_fluid[Fluid::CrudeOil], 2);
    // No EC, no oil.
    assert_eq!(w.remove(0, 7, 7), Some(Kind::Solar));
    let made = w.econ.made_fluid[Fluid::CrudeOil];
    run(&mut w, 4);
    assert_eq!(w.econ.made_fluid[Fluid::CrudeOil], made);
}

#[test]
fn the_oil_extractor_stops_when_the_pipe_in_front_is_full() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::OilExtractor, Dir::E).unwrap();
    w.place(0, 6, 5, Kind::Pipe, Dir::E).unwrap(); // one pipe holds 20 units
    w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 60); // room for 30
    assert_eq!(w.econ.made_fluid[Fluid::CrudeOil], PIPE_CAP as u64);
    assert_eq!(w.fluids.pools[0].amount, PIPE_CAP);
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FLUD"));
}

#[test]
fn deeper_floors_give_crude_oil_faster() {
    // 80 ticks is not enough for a unit on the surface.
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::OilExtractor, Dir::E).unwrap();
    w.place(0, 6, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap();
    for _ in 0..80 {
        w.tick();
    }
    assert_eq!(w.econ.made_fluid[Fluid::CrudeOil], 0, "the surface is slow");

    // Underground the ground is richer (x1.5), so the same time gives one unit. An elevator
    // carries the surface panel's power down to it.
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Elevator, Dir::E).unwrap();
    w.place(0, 12, 9, Kind::Solar, Dir::E).unwrap();
    let f = w.floor_of(-1).unwrap();
    w.place(f, 9, 9, Kind::OilExtractor, Dir::E).unwrap();
    w.place(f, 10, 9, Kind::Pipe, Dir::E).unwrap();
    assert!(w.building_ratio(f, idx(9, 9)) > 0.0, "the elevator links the floors' grids");
    for _ in 0..80 {
        w.tick();
    }
    assert_eq!(w.econ.made_fluid[Fluid::CrudeOil], 1, "1.5x underground");
}

#[test]
fn the_oil_extractor_needs_open_ground() {
    let w = World::blank();
    assert!(w.check_place(0, 5, 5, Kind::OilExtractor).is_ok());
    // It cannot sit on an upper floor, like the pumps and drills.
    let up = w.floor_of(1).unwrap();
    assert!(w.check_place(up, 5, 5, Kind::OilExtractor).is_err());
}

#[test]
fn the_liquid_trader_sells_the_fluid_in_the_pipe_it_touches() {
    let mut w = World::blank();
    for x in 5..=6 {
        w.place(0, x, 5, Kind::Pipe, Dir::E).unwrap();
    }
    w.place(0, 7, 5, Kind::LiquidTrader, Dir::E).unwrap(); // covers 7-8, 5-6
    w.place(0, 7, 8, Kind::Solar, Dir::E).unwrap();
    let pool = w.fluids.pipe_pool[0][idx(6, 5)] as usize;
    w.fluids.pools[pool].add(Fluid::CrudeOil, 40.0);
    let money = w.econ.money;
    run(&mut w, 1);
    // Four units a second at crude oil's price.
    assert!((w.econ.money - money - 8.0).abs() < 1e-6, "made {}", w.econ.money - money);
    assert!((w.fluids.pools[pool].amount - 36.0).abs() < 1e-6);
    // Water cannot be sold, and an empty pipe gives it nothing to do.
    w.fluids.pools[pool].take(Fluid::CrudeOil, 36.0);
    w.fluids.pools[pool].add(Fluid::Water, 10.0);
    let money = w.econ.money;
    run(&mut w, 2);
    assert_eq!(w.econ.money, money);
    assert_eq!(w.idle_reason(0, idx(7, 5)), Some("FLUD"));
}

#[test]
fn the_liquid_trader_earns_research_that_fades() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 6, 5, Kind::LiquidTrader, Dir::E).unwrap();
    w.place(0, 6, 8, Kind::Solar, Dir::E).unwrap();
    let pool = w.fluids.pipe_pool[0][idx(5, 5)] as usize;
    w.fluids.pools[pool].add(Fluid::Ethylene, 60.0);
    // Money -> split -> research.
    w.configure(0, 6, 5);
    w.configure(0, 6, 5);
    let money = w.econ.money;
    run(&mut w, 1);
    assert!(w.econ.research > 0.0 && w.econ.research_sales_fluid[Fluid::Ethylene] > 0.0);
    let first = w.econ.next_fluid_research(Fluid::Ethylene);
    assert!(first < Fluid::Ethylene.curve().unwrap().worth, "the next unit pays less");
    assert_eq!(w.econ.money, money, "research mode earns no money");
}

#[test]
fn the_still_splits_crude_oil_into_naphtha_and_waste() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Still, Dir::E).unwrap(); // 2 wide, 3 tall: front is x=12, back is x=9
    cycle_to(&mut w, 10, 10, "naphtha");
    // Crude oil comes from a pipe on the side; the waste leaves by the back, into a crate.
    w.place(0, 10, 9, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 9, 11, Kind::Crate, Dir::W).unwrap();
    for y in 10..=12 {
        w.place(0, 12, y, Kind::Pipe, Dir::E).unwrap();
    }
    let pool = w.fluids.pipe_pool[0][idx(10, 9)] as usize;
    w.fluids.pools[pool].add(Fluid::CrudeOil, 30.0);
    run(&mut w, 9);
    assert_eq!(w.econ.made_fluid[Fluid::Naphtha], 0, "a run takes 10 s");
    run(&mut w, 2);
    assert_eq!(w.econ.made_fluid[Fluid::Naphtha], 2);
    assert_eq!(crate_items(&w, 0, 9, 11), vec![Item::Waste]);
}

#[test]
fn the_still_makes_ethylene_from_naphtha_and_carbon_black_waits_for_fuel_oil() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Still, Dir::E).unwrap();
    cycle_to(&mut w, 10, 10, "ethylene");
    w.place(0, 10, 9, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 9, 11, Kind::Crate, Dir::W).unwrap();
    w.place(0, 12, 11, Kind::Pipe, Dir::E).unwrap();
    let pool = w.fluids.pipe_pool[0][idx(10, 9)] as usize;
    w.fluids.pools[pool].add(Fluid::Naphtha, 12.0);
    run(&mut w, 11);
    assert_eq!(w.econ.made_fluid[Fluid::Ethylene], 2);
    // Carbon black: the recipe is there, but nothing makes fuel oil yet.
    let black = STILL_RECIPES.iter().find(|r| r.name == "carbon black").unwrap();
    assert_eq!(black.inputs, &[(Item::Coal, 4)]);
    assert_eq!((black.fluid_in, black.outputs), (Some((Fluid::FuelOil, 1)), &[(Item::CarbonBlack, 2), (Item::Waste, 1)][..]));
    assert!(Kind::ALL.iter().all(|k| k.recipes().iter().all(|r| r.fluid_out != Some((Fluid::FuelOil, 1)))));
}

#[test]
fn press_output_leaves_from_the_front_tile() {
    // Facing east: crate -> press -> belt in front of its right-hand tile -> crate.
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
    fill_crate(&mut w, 0, 5, 5, &[Item::CopperIngot; 3]);
    w.place(0, 6, 5, Kind::Press, Dir::E).unwrap(); // covers 6-7, 5
    w.place(0, 8, 5, Kind::Belt, Dir::E).unwrap();
    w.place(0, 9, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 6, 7, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 6);
    assert!(!crate_items(&w, 0, 9, 5).is_empty());
    assert!(crate_items(&w, 0, 9, 5).iter().all(|&i| i == Item::CopperPlate));

    // Facing north: it takes ingots from the side and sends plates out of its top tile.
    let mut w = World::blank();
    w.place(0, 9, 11, Kind::Crate, Dir::E).unwrap(); // beside the lower tile
    fill_crate(&mut w, 0, 9, 11, &[Item::CopperIngot; 3]);
    w.place(0, 10, 10, Kind::Press, Dir::N).unwrap(); // covers 10, 10-11
    w.place(0, 10, 9, Kind::Belt, Dir::N).unwrap();
    w.place(0, 10, 8, Kind::Crate, Dir::E).unwrap();
    w.place(0, 12, 10, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 6);
    assert!(!crate_items(&w, 0, 10, 8).is_empty());
    // Nothing comes out of the sides or the back.
    w.place(0, 11, 11, Kind::Crate, Dir::E).unwrap();
    w.place(0, 10, 12, Kind::Crate, Dir::E).unwrap();
    run(&mut w, 3);
    assert!(crate_items(&w, 0, 11, 11).is_empty() && crate_items(&w, 0, 10, 12).is_empty());
}

#[test]
fn turning_a_placed_press_needs_room() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Press, Dir::E).unwrap(); // covers 10-11, 10
    w.place(0, 10, 11, Kind::Belt, Dir::E).unwrap();
    // North would need (10, 11), which is taken: nothing changes.
    assert_eq!(w.set_dir(0, 10, 10, Dir::N), Err("Something is in the way".into()));
    assert_eq!(w.tile(0, 10, 10).unwrap().dir, Dir::E);
    assert_eq!(w.anchor_of(0, 11, 10), Some((10, 10)));
    w.remove(0, 10, 11);
    w.set_dir(0, 10, 10, Dir::N).unwrap();
    assert_eq!(w.anchor_of(0, 10, 11), Some((10, 10)));
    assert_eq!(w.anchor_of(0, 11, 10), None, "the old tile is free again");
    // Turning back and forth between east and west changes nothing.
    w.set_dir(0, 10, 10, Dir::W).unwrap();
    assert_eq!(w.anchor_of(0, 11, 10), Some((10, 10)));
    assert_eq!(w.anchor_of(0, 10, 11), None);
    // A belt just turns.
    w.place(0, 20, 20, Kind::Belt, Dir::E).unwrap();
    w.set_dir(0, 20, 20, Dir::S).unwrap();
    assert!(w.set_dir(0, 30, 30, Dir::S).is_err());
}

#[test]
fn low_power_slows_evenly() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Solar, Dir::E).unwrap();
    // 32 belts fill the panel's reach around it: 16 power wanted, 10 available.
    for (x, y) in footprint(8, 8, 6, 6) {
        if w.check_place(0, x, y, Kind::Belt).is_ok() {
            w.place(0, x, y, Kind::Belt, Dir::E).unwrap();
        }
    }
    assert!((w.ratio_at(0, 8, 8) - 10.0 / 16.0).abs() < 1e-9);
}

#[test]
fn poles_carry_power_to_far_belts() {
    let mut w = World::blank();
    w.place(0, 0, 0, Kind::Solar, Dir::E).unwrap(); // reaches up to 3,3
    w.place(0, 10, 10, Kind::Belt, Dir::E).unwrap();
    assert_eq!(w.ratio_at(0, 10, 10), 0.0);
    assert_eq!(w.grid.unpowered, 1);
    w.place(0, 6, 6, Kind::Pole, Dir::E).unwrap(); // reaches 3-9: touches the panel
    assert_eq!(w.ratio_at(0, 10, 10), 0.0);
    w.place(0, 12, 12, Kind::Pole, Dir::E).unwrap(); // reaches 9-15: touches the first pole
    assert_eq!(w.ratio_at(0, 10, 10), 1.0);
    assert_eq!(w.grid.nets.len(), 1);
}

#[test]
fn separate_grids_share_only_their_own_power() {
    let mut w = World::blank();
    w.place(0, 0, 0, Kind::Solar, Dir::E).unwrap();
    w.place(0, 3, 3, Kind::Belt, Dir::E).unwrap();
    w.place(0, 50, 50, Kind::Pole, Dir::E).unwrap(); // no panel on this grid
    w.place(0, 51, 50, Kind::Belt, Dir::E).unwrap();
    assert_eq!(w.grid.nets.len(), 2);
    assert_eq!(w.ratio_at(0, 3, 3), 1.0);
    assert_eq!(w.ratio_at(0, 51, 50), 0.0);
}

#[test]
fn research_fades_to_a_floor() {
    let c = Item::CopperPlate.curve();
    assert_eq!(c.value(0), 12.0);
    assert!(c.value(100) < 12.0 && c.value(100) > 6.0);
    assert!((c.value(10_000) - 0.36).abs() < 1e-9);
}

#[test]
fn solar_needs_sunlight() {
    let w = World::blank();
    assert!(w.check_place(1, 3, 3, Kind::Solar).is_err());
}

#[test]
fn locked_parts_need_research() {
    let mut w = World::new();
    let spot = (60, 50);
    assert!(w.check_place(0, spot.0, spot.1, Kind::Press).unwrap_err().contains("locked"));
    assert!(w.research(Tech::Press).is_err()); // no research points yet
    w.econ.research = 100.0;
    w.research(Tech::Press).unwrap();
    assert_eq!(w.econ.research, 40.0);
    assert!(w.check_place(0, spot.0, spot.1, Kind::Press).is_ok());
    // Ashgrove isn't open yet, so its drill can't be researched.
    assert!(w.research(Tech::BasicDrill).unwrap_err().contains("Make copper wire first"));
}

#[test]
fn making_wire_opens_ashgrove() {
    let mut w = World::blank();
    w.progress.opened[Company::Ashgrove.idx()] = false;
    w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
    fill_crate(&mut w, 0, 5, 5, &[Item::CopperPlate; 3]);
    w.place(0, 6, 5, Kind::Press, Dir::E).unwrap();
    while !w.configure(0, 6, 5).unwrap().contains("copper wire") {}
    w.place(0, 6, 7, Kind::Solar, Dir::E).unwrap(); // reaches the press
    run(&mut w, 5);
    assert!(w.econ.made[Item::CopperWire.idx()] >= 2);
    assert!(w.progress.is_open(Company::Ashgrove));
    // Its letter arrives as a telegram, with what it unlocks.
    let m = w.mail.pop_front().expect("a letter");
    assert_eq!((m.from.as_str(), m.event.as_str()), ("Ashgrove Ironstone", "letter"));
    assert!(m.md.contains("ironstone") && m.md.contains("Basic drill"), "{}", m.md);
    assert!(m.gift.is_none() && w.unread() == 1);
}

#[test]
fn coke_needs_coking_research() {
    let mut w = World::blank();
    w.progress.researched[Tech::Coking.idx()] = false;
    w.progress.researched[Tech::BlastFurnace.idx()] = false; // its concrete recipe would count too
    w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
    // Metals only: it goes round without ever reaching coke.
    for _ in 0..8 {
        assert!(w.configure(0, 5, 5).unwrap().contains("molten"));
    }
    w.progress.researched[Tech::Coking.idx()] = true;
    while !w.configure(0, 5, 5).unwrap().ends_with("makes coke") {}
    // Three coal go in, coke comes out in 5 s, no separate fuel needed.
    w.place(0, 3, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 4, 5, Kind::Belt, Dir::E).unwrap();
    w.place(0, 3, 7, Kind::Solar, Dir::E).unwrap();
    fill_crate(&mut w, 0, 3, 5, &[Item::Coal; 6]);
    run(&mut w, 14);
    assert_eq!(w.econ.made[Item::Coke.idx()], 2);
}

#[test]
fn drills_mine_copper_coal_iron_and_stone() {
    let mut w = World::blank();
    // A cheap drill goes copper -> coal -> iron -> stone -> copper.
    w.place(0, 10, 10, Kind::Drill, Dir::E).unwrap();
    let mines = |w: &World, x, y| match &w.tile(0, x, y).unwrap().st {
        St::Drill { mines, .. } => *mines,
        _ => None,
    };
    assert_eq!(mines(&w, 10, 10), Some(Item::CopperOre));
    w.configure(0, 10, 10);
    assert_eq!(mines(&w, 10, 10), Some(Item::Coal));
    assert!(w.configure(0, 10, 10).unwrap().contains("half the speed"));
    assert_eq!(mines(&w, 10, 10), Some(Item::IronOre));
    w.configure(0, 10, 10);
    assert_eq!(mines(&w, 10, 10), Some(Item::Stone));
    w.configure(0, 10, 10);
    assert_eq!(mines(&w, 10, 10), Some(Item::CopperOre));
    w.apply_setting(0, 10, 10, Setting::Mines(Item::IronOre));
    assert_eq!(mines(&w, 10, 10), Some(Item::IronOre), "a copied iron setting sticks to a cheap drill now");
    // Stone, anywhere on the ground; the starter fuel mines 8.
    w.place(0, 30, 30, Kind::Drill, Dir::E).unwrap();
    w.apply_setting(0, 30, 30, Setting::Mines(Item::Stone));
    w.place(0, 32, 30, Kind::Crate, Dir::E).unwrap();
    run(&mut w, 10);
    assert_eq!(crate_items(&w, 0, 32, 30), vec![Item::Stone; 8]);
    assert_eq!(Item::Stone.price(), 0.3);
}

#[test]
fn a_cheap_drill_cuts_iron_at_half_the_basic_drills_speed() {
    let mut w = World::blank();
    for (kind, y) in [(Kind::BasicDrill, 20), (Kind::Drill, 25)] {
        w.place(0, 20, y, kind, Dir::E).unwrap();
        w.apply_setting(0, 20, y, Setting::Mines(Item::IronOre));
        w.place(0, 22, y, Kind::Crate, Dir::E).unwrap();
    }
    run(&mut w, 4);
    let (basic, cheap) = (crate_items(&w, 0, 22, 20), crate_items(&w, 0, 22, 25));
    assert!(basic.iter().chain(&cheap).all(|i| *i == Item::IronOre));
    assert_eq!((basic.len(), cheap.len()), (6, 3));
    // Copper is unchanged: a cheap drill is at 60 ticks an ore.
    w.place(0, 40, 20, Kind::Drill, Dir::E).unwrap();
    w.place(0, 42, 20, Kind::Crate, Dir::E).unwrap();
    run(&mut w, 3);
    assert_eq!(crate_items(&w, 0, 42, 20).len(), 3);
}

#[test]
fn blast_furnace_melts_iron_then_makes_steel_with_waste() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
    fill_crate(&mut w, 0, 5, 5, &[Item::IronOre, Item::IronOre, Item::IronOre, Item::Coke, Item::Coke]);
    w.place(0, 6, 5, Kind::BlastFurnace, Dir::E).unwrap(); // covers 6-7, 5-6
    w.place(0, 8, 5, Kind::Pipe, Dir::E).unwrap();
    run(&mut w, 20);
    // 5 s an ore, one molten iron each, into the pipe.
    assert_eq!(w.fluids.pools[0].fluid, Some(Fluid::MoltenIron));
    assert_eq!(w.fluids.pools[0].amount, 3.0);
    // The iron furnace is slower and gives 2 molten iron for 3 ore.
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
    assert!(w.configure(0, 5, 5).unwrap().contains("molten iron"));
    set_stock(&mut w, 5, 5, Item::IronOre, 3);
    set_stock(&mut w, 5, 5, Item::Coal, 5);
    w.place(0, 8, 5, Kind::Pipe, Dir::E).unwrap();
    run(&mut w, 9);
    assert_eq!(w.fluids.pools[0].amount, 0.0, "10 s a batch");
    run(&mut w, 3);
    assert_eq!((w.fluids.pools[0].fluid, w.fluids.pools[0].amount), (Some(Fluid::MoltenIron), 2.0));
}

fn offer(w: &mut World, x: i32, y: i32, item: Item) -> bool {
    w.offer(0, x, y, item)
}

fn set_mode(w: &mut World, x: i32, y: i32, want: SellMode) {
    while !matches!(&w.tile(0, x, y).unwrap().st, St::Trade { mode, .. } if *mode == want) {
        w.configure(0, x, y);
    }
}

#[test]
fn a_money_post_takes_waste_and_charges_for_it() {
    let mut w = World::blank();
    w.place(0, 10, 4, Kind::TradePost, Dir::E).unwrap();
    let before = w.econ.money;
    assert!(offer(&mut w, 10, 4, Item::Waste));
    assert!((w.econ.money - (before - 0.5)).abs() < 1e-9);
    assert_eq!(w.econ.sold[Item::Waste.idx()], 1);
    assert!(w.econ.money_from[Item::Waste.idx()] < 0.0);
}

#[test]
fn waste_never_earns_research() {
    let mut w = World::blank();
    w.place(0, 10, 4, Kind::TradePost, Dir::E).unwrap();
    for _ in 0..20 {
        offer(&mut w, 10, 4, Item::Waste);
    }
    assert_eq!(w.econ.research, 0.0);
    assert_eq!(w.econ.research_sales[Item::Waste.idx()], 0);
    assert_eq!(Item::Waste.curve().value(0), 0.0);
    // Even a research post's refusal leaves research alone.
    set_mode(&mut w, 10, 4, SellMode::Research);
    assert!(!offer(&mut w, 10, 4, Item::Waste));
    assert_eq!(w.econ.research, 0.0);
}

#[test]
fn research_and_split_posts_refuse_waste() {
    let mut w = World::blank();
    w.place(0, 10, 4, Kind::TradePost, Dir::E).unwrap();
    let money = w.econ.money;
    for mode in [SellMode::Research, SellMode::Split] {
        set_mode(&mut w, 10, 4, mode);
        assert!(!offer(&mut w, 10, 4, Item::Waste), "{mode:?}");
        assert!(w.econ.waste_refusal(mode).is_some());
    }
    assert_eq!(w.econ.money, money);
    // It still takes other things, and says WSTE.
    assert!(offer(&mut w, 10, 4, Item::CopperPlate));
    assert_eq!(w.idle_reason(0, idx(10, 4)), Some("WSTE"));
    // A refused waste item stays on the belt in front of the post.
    let mut w = World::blank();
    for x in 7..=9 {
        w.place(0, x, 5, Kind::Belt, Dir::E).unwrap();
    }
    w.place(0, 10, 4, Kind::TradePost, Dir::E).unwrap();
    w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap();
    set_mode(&mut w, 10, 4, SellMode::Research);
    if let Some(Building { st: St::Belt { items }, .. }) = w.floors[0].tiles[idx(9, 5)].as_mut() {
        items.push(BeltItem { item: Item::Waste, pos: 0.9 });
    }
    run(&mut w, 5);
    let Some(Building { st: St::Belt { items }, .. }) = w.floors[0].tiles[idx(9, 5)].as_ref() else { panic!() };
    assert_eq!(items.len(), 1, "the belt backs up");
    assert_eq!(w.econ.sold[Item::Waste.idx()], 0);
}

#[test]
fn a_post_refuses_waste_when_money_would_go_negative() {
    let mut w = World::blank();
    w.place(0, 10, 4, Kind::TradePost, Dir::E).unwrap();
    w.econ.money = 0.75;
    assert!(offer(&mut w, 10, 4, Item::Waste));
    assert!((w.econ.money - 0.25).abs() < 1e-9);
    assert!(!offer(&mut w, 10, 4, Item::Waste), "$0.25 can't pay $0.50");
    assert!((w.econ.money - 0.25).abs() < 1e-9, "money never goes below zero");
    assert_eq!(w.econ.waste_refusal(SellMode::Money), Some("you can't afford the fee"));
}

#[test]
fn waste_stops_selling_when_fees_pass_earnings() {
    let mut w = World::blank();
    w.place(0, 10, 4, Kind::TradePost, Dir::E).unwrap();
    // Earning $10 a minute: waste sells until its fees pass that.
    w.econ.money_from[Item::CopperPlate.idx()] = 10.0;
    let mut taken = 0;
    while offer(&mut w, 10, 4, Item::Waste) {
        taken += 1;
        assert!(taken < 100);
    }
    assert_eq!(taken, 21, "$10.50 of fees is more than the $10 earned");
    assert_eq!(w.econ.waste_refusal(SellMode::Money), Some("waste costs more than you're earning"));
    assert_eq!(w.idle_reason(0, idx(10, 4)), Some("WSTE"));
    // A minute later the old fees and earnings have left the window, and waste sells again.
    run(&mut w, 70);
    assert!(w.econ.waste_refusal(SellMode::Money).is_none());
    assert!(offer(&mut w, 10, 4, Item::Waste));
}

#[test]
fn cutting_a_pipe_splits_its_fluid_instead_of_making_more() {
    let mut w = World::blank();
    for x in 10..20 {
        w.place(0, x, 10, Kind::Pipe, Dir::E).unwrap();
    }
    assert_eq!(w.fluids.pools.len(), 1);
    w.fluids.pools[0].fluid = Some(Fluid::Water);
    w.fluids.pools[0].amount = 100.0; // half full: 10 pipes hold 200
    w.try_remove(0, 14, 10).unwrap();
    assert_eq!(w.fluids.pools.len(), 2);
    let total: f64 = w.fluids.pools.iter().map(|p| p.amount).sum();
    assert!((total - 90.0).abs() < 1e-9, "the cut pipe's share leaves with it: {total}");
    let amounts: Vec<f64> = w.fluids.pools.iter().map(|p| p.amount).collect();
    assert!(amounts.contains(&40.0) && amounts.contains(&50.0), "{amounts:?}");
}

#[test]
fn opening_a_floor_gives_elevators_a_door_there() {
    let mut w = World::blank();
    // Only the floor above is open: the elevator gets a door on the surface and up.
    w.progress.researched[Tech::Excavation.idx()] = false;
    w.progress.researched[Tech::SupportBeams.idx()] = true;
    let down = w.floors.iter().position(|fl| fl.level == -1).unwrap();
    assert!(!w.floor_open(down));
    w.place_sized(0, 10, 10, Kind::Elevator, Dir::E, 2).unwrap();
    let a = w.floors[0].anchor_at(10, 10).unwrap();
    if let Some(Building { st: St::Elevator { inbox, .. }, .. }) = w.floors[0].tiles[a].as_mut() {
        inbox.push_back(Item::Coal);
    }
    assert!(w.floors[down].tiles[a].is_none());
    // Excavation opens floor -1 (unlocked through research here).
    w.progress.opened[Tech::Excavation.company().idx()] = true;
    w.econ.research = Tech::Excavation.cost();
    w.research(Tech::Excavation).unwrap();
    match &w.floors[down].tiles[a] {
        Some(Building { st: St::Elevator { mode, inbox, outbox }, .. }) => {
            assert_eq!(*mode, DoorMode::Unload);
            assert!(inbox.is_empty() && outbox.is_empty(), "no items copied");
        }
        other => panic!("no door on the new floor: {other:?}"),
    }
    assert!(w.check_place(down, 10, 10, Kind::Crate).is_err(), "the shaft is taken");
}

#[test]
fn removing_a_crate_of_waste_costs_money() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
    fill_crate(&mut w, 0, 5, 5, &[Item::Waste; 200]);
    fill_crate(&mut w, 0, 5, 5, &[Item::Coal; 10]);
    assert_eq!(w.removal_fee(0, 5, 5), (200, 100.0));
    let price = Kind::Crate.price(1);
    w.econ.money = 99.0;
    let err = w.try_remove(0, 5, 5).unwrap_err();
    assert_eq!(err, "Holds 200 waste: removing it costs $100");
    assert!(w.tile(0, 5, 5).is_some() && w.econ.money == 99.0, "refused: nothing changes");
    w.econ.money = 150.0;
    assert_eq!(w.try_remove(0, 5, 5), Ok(Some(Kind::Crate)));
    assert!((w.econ.money - (50.0 + price)).abs() < 1e-9, "fee paid, refund given");
    assert!(w.tile(0, 5, 5).is_none());
    // An empty crate is free to remove, even with no money.
    w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
    w.econ.money = 0.0;
    assert_eq!(w.try_remove(0, 5, 5), Ok(Some(Kind::Crate)));
}

#[test]
fn removing_an_elevator_with_waste_costs_money() {
    let mut w = World::blank();
    w.place(0, 20, 20, Kind::Elevator, Dir::E).unwrap();
    w.lifts[0].load.extend([Item::Waste, Item::Waste, Item::Coal]);
    assert_eq!(w.removal_fee(0, 20, 20), (2, 1.0));
    w.econ.money = 0.5;
    assert!(w.try_remove(0, 20, 20).unwrap_err().starts_with("Holds 2 waste"));
    w.econ.money = 5.0;
    assert_eq!(w.try_remove(0, 20, 20), Ok(Some(Kind::Elevator)));
}

#[test]
fn old_version_saves_are_refused_and_listed() {
    let data = std::env::temp_dir().join(format!("tfac-old-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    std::fs::create_dir_all(crate::saves::dir(&data)).unwrap();
    // No version at all: from before versions.
    let old = crate::saves::path_of(&data, "save1");
    std::fs::write(&old, r#"{"floors":[],"tick":5}"#).unwrap();
    assert_eq!(World::file_version(&old), Some(0));
    let err = World::load(&old).err().unwrap();
    assert!(err.to_string().contains("older version"), "{err}");
    let msg = crate::saves::version_problem(&old, "save1").unwrap();
    assert!(msg.contains("older version") && msg.contains("tfac --new"), "{msg}");
    assert!(old.exists(), "never deleted");
    // A current save opens fine and says nothing.
    let mut w = World::new();
    let fresh = crate::saves::path_of(&data, "save2");
    w.save(&fresh).unwrap();
    assert_eq!(World::file_version(&fresh), Some(SAVE_VERSION));
    assert!(World::load(&fresh).is_ok() && crate::saves::version_problem(&fresh, "save2").is_none());
    let list = crate::saves::describe(&data);
    let line = |n: &str| list.lines().find(|l| l.trim_start().starts_with(n)).unwrap().to_string();
    assert!(line("save1").contains("(old version)"), "{list}");
    assert!(!line("save2").contains("(old version)") && line("save2").contains("research"), "{list}");
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn solar_panels_need_poles_between_them() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Solar, Dir::E).unwrap();
    w.place(0, 12, 10, Kind::Solar, Dir::E).unwrap();
    assert_eq!(w.grid.nets.len(), 2, "side by side, two panels are still two grids");
    assert_eq!(w.source_net(0, idx(10, 10)).map(|n| n.1), Some(1));
    // Nothing to power yet: their supply counts as unused, not as grid supply.
    let (supply, _, _, _, _, unused) = w.power_summary();
    assert_eq!((supply, unused), (0.0, 2.0 * SOLAR_OUTPUT));
    w.place(0, 12, 13, Kind::Pole, Dir::E).unwrap();
    assert_eq!(w.grid.nets.len(), 1, "a pole between them joins them");
    assert_eq!(w.grid.nets[0].solar, 2.0 * SOLAR_OUTPUT);
    assert_eq!(w.source_net(0, idx(10, 10)).map(|n| n.1), Some(3));
    assert_eq!(w.grid_at(0, 11, 11), Some(0), "any tile of a panel finds its grid");
    assert_eq!(w.grid_at(0, 30, 30), None, "empty ground has none");
}

#[test]
fn steam_engines_power_a_grid_when_solar_is_not_enough() {
    let mut w = World::blank();
    // A pump, pipes to a boiler, boiler straight into an engine.
    w.place(0, 5, 5, Kind::Pump, Dir::E).unwrap();
    w.place(0, 6, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 7, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 8, 5, Kind::Boiler, Dir::E).unwrap(); // covers 8-9, 5-6; front is x=10
    w.place(0, 10, 5, Kind::SteamEngine, Dir::E).unwrap(); // covers 10-11, 5-6
    w.place(0, 8, 7, Kind::Crate, Dir::N).unwrap(); // pushes coal up into the boiler
    fill_crate(&mut w, 0, 8, 7, &[Item::Coal; 5]);
    w.place(0, 5, 7, Kind::Solar, Dir::E).unwrap(); // runs the pump, and links to the engine's grid
                                                    // A pole and 27 belts: far more than one solar panel can run.
    w.place(0, 14, 5, Kind::Pole, Dir::E).unwrap();
    for (x, y) in footprint(14, 2, 4, 4).chain(footprint(14, 6, 3, 3)) {
        if w.check_place(0, x, y, Kind::Belt).is_ok() {
            w.place(0, x, y, Kind::Belt, Dir::E).unwrap();
        }
    }
    w.tick();
    assert!(w.ratio_at(0, 15, 3) < 1.0, "solar alone can't keep up");
    run(&mut w, 10);
    assert_eq!(w.ratio_at(0, 15, 3), 1.0, "the engine makes up the rest");
    // Placing something elsewhere keeps the water already in the pipes.
    let water = w.fluids.pools[0].amount;
    w.place(0, 40, 40, Kind::Belt, Dir::E).unwrap();
    assert_eq!(w.fluids.pools[0].amount, water);
}

#[test]
fn no_ore_patches_or_lakes_drills_and_pumps_go_anywhere_on_the_ground() {
    let w = World::new();
    assert!(w.check_place(0, 30, 30, Kind::Drill).is_ok(), "bare ground is fine");
    let w = World::blank();
    assert!(w.check_place(0, 30, 30, Kind::Pump).is_ok(), "pumps draw water from anywhere");
    let up = w.floors.iter().position(|fl| fl.level == 1).unwrap();
    for kind in [Kind::Drill, Kind::Pump] {
        assert!(w.check_place(up, 30, 30, kind).unwrap_err().contains("ground"));
    }
    let down = w.floors.iter().position(|fl| fl.level == -1).unwrap();
    assert!(w.check_place(down, 30, 30, Kind::Pump).is_ok());
    assert_eq!(richness(-1), 1.5);
    assert_eq!(richness(-3), 3.0);
}

#[test]
fn drills_mine_faster_underground() {
    let mut w = World::blank();
    for f in [0, 1] {
        w.place(f, 5, 5, Kind::Drill, Dir::E).unwrap();
        w.place(f, 7, 5, Kind::Crate, Dir::E).unwrap();
    }
    run(&mut w, 4);
    let (top, deep) = (crate_items(&w, 0, 7, 5).len(), crate_items(&w, 1, 7, 5).len());
    assert_eq!(top, 4);
    assert_eq!(deep, 6);
}

#[test]
fn floors_open_with_beams_and_digging() {
    let mut w = World::new();
    let up = w.floor_of(1).unwrap();
    let down = w.floor_of(-1).unwrap();
    assert!(w.check_place(up, 60, 40, Kind::Crate).unwrap_err().contains("support beams"));
    assert!(w.check_place(down, 60, 40, Kind::Crate).unwrap_err().contains("excavation"));
    w.progress.researched[Tech::SupportBeams.idx()] = true;
    // Upper floors cost $10 a tile to deck over; elevators reach every open floor.
    assert_eq!(w.check_place(up, 60, 40, Kind::Crate).unwrap(), 50.0 + 10.0);
    assert!(w.check_place(w.floor_of(2).unwrap(), 60, 40, Kind::Crate).is_err(), "floor +2 comes later");
}

#[test]
fn milestones_unlock_for_free() {
    let mut w = World::new();
    assert!(!w.progress.has(Tech::Press));
    w.econ.made[Item::CopperIngot.idx()] = 49;
    w.tick();
    assert!(!w.progress.has(Tech::Press));
    w.econ.made[Item::CopperIngot.idx()] = 50;
    w.tick();
    assert!(w.progress.has(Tech::Press));
    assert_eq!(w.econ.research, 0.0, "nothing was spent");
    assert!(w.progress.notes.pop_front().unwrap().contains("free"));
}

#[test]
fn fast_belts_move_twice_as_much() {
    let mut w = World::blank();
    for (y, kind) in [(5, Kind::Belt), (8, Kind::FastBelt)] {
        w.place(0, 4, y, Kind::Crate, Dir::E).unwrap();
        fill_crate(&mut w, 0, 4, y, &[Item::Coal; 200]);
        for x in 5..=8 {
            w.place(0, x, y, kind, Dir::E).unwrap();
        }
        w.place(0, 9, y, Kind::Crate, Dir::E).unwrap();
    }
    w.place(0, 6, 6, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 10);
    let (slow, fast) = (crate_items(&w, 0, 9, 5).len(), crate_items(&w, 0, 9, 8).len());
    assert!(fast as f64 > slow as f64 * 1.8, "slow {slow}, fast {fast}");
}

// ---------- the later machines ----------

fn recipe_names(kind: Kind) -> Vec<&'static str> {
    kind.recipes().iter().map(|r| r.name).collect()
}

/// Points a machine at a recipe by name.
fn pick_recipe(w: &mut World, x: i32, y: i32, name: &str) {
    let a = w.floors[0].anchor_at(x, y).unwrap();
    let b = w.floors[0].tiles[a].as_mut().unwrap();
    let n = b.kind.recipes().iter().position(|r| r.name == name).unwrap();
    if let St::Crafter { recipe, .. } = &mut b.st {
        *recipe = n as u8;
    }
}

/// Ticks a recipe takes in a machine, from its list.
fn recipe_ticks(kind: Kind, name: &str) -> u32 {
    kind.recipes().iter().find(|r| r.name == name).unwrap().ticks
}

fn scaled(t: u32, by: u32) -> u32 {
    ((t as f64 / by as f64).round() as u32).max(1)
}

#[test]
fn the_assembler_ladder_lists_what_each_can_make() {
    let tier = |t: &[Recipe]| t.iter().map(|r| r.name).collect::<Vec<_>>();
    let (crude, basic, adv, prec) = (tier(CRUDE_RECIPES), tier(BASIC_RECIPES), tier(ADVANCED_RECIPES), tier(PRECISION_RECIPES));
    let (large_basic, large_adv, large_prec) = (tier(LARGE_BASIC_RECIPES), tier(LARGE_ADVANCED_RECIPES), tier(LARGE_PRECISION_RECIPES));
    let joined = |parts: &[&Vec<&'static str>]| parts.iter().flat_map(|p| p.iter().copied()).collect::<Vec<_>>();
    assert_eq!(recipe_names(Kind::CrudeAssembler), crude);
    assert_eq!(recipe_names(Kind::BasicAssembler), joined(&[&crude, &basic]));
    // The advanced assembler hides the hand-wired board: it has the proper recipe.
    let crude_without_hand_wiring: Vec<_> = crude.iter().copied().filter(|n| *n != HAND_WIRED).collect();
    assert_eq!(recipe_names(Kind::AdvancedAssembler), joined(&[&crude_without_hand_wiring, &basic, &adv]));
    assert_eq!(recipe_names(Kind::PrecisionAssembler), joined(&[&adv, &prec]), "precision makes only the top tiers");
    assert_eq!(recipe_names(Kind::LargeBasicAssembler), large_basic);
    assert_eq!(recipe_names(Kind::LargeAdvancedAssembler), joined(&[&large_basic, &large_adv]));
    assert_eq!(recipe_names(Kind::LargePrecisionAssembler), joined(&[&large_adv, &large_prec]), "large precision makes only the top tiers");
    assert_eq!(
        crude,
        ["valve", "gauge", "lamp", "relay", "gearbox", "transformer", "keyboard", "lamp matrix", "control board (hand-wired)"]
    );
    assert_eq!(basic, ["electric motor", "piston", "terminal", "disk drive"]);
    assert_eq!(adv, ["control board", "tube board", "circuit board", "transistor", "transistor board", "mask"]);
    assert_eq!(prec, ["microprocessor (x4x1)", "RAM chip", "fine mask", "microprocessor (x4x2)", "mainboard", "mainboard (two coarse)"]);
    assert_eq!(large_basic, ["pump unit", "engine block"]);
    assert_eq!(large_adv, ["generator", "control panel", "drilling rig", "steam tractor"]);
    assert_eq!(large_prec, ["computer"]);
    // Large and small assemblers share nothing.
    for small in [Kind::CrudeAssembler, Kind::BasicAssembler, Kind::AdvancedAssembler, Kind::PrecisionAssembler] {
        for big in [Kind::LargeBasicAssembler, Kind::LargeAdvancedAssembler, Kind::LargePrecisionAssembler] {
            assert!(recipe_names(small).iter().all(|n| !recipe_names(big).contains(n)), "{small:?} and {big:?}");
        }
    }
    // Each tier's time is the lowest assembler's; the ones above run it faster.
    let t = recipe_ticks;
    assert_eq!(t(Kind::CrudeAssembler, "gearbox"), 600);
    assert_eq!(t(Kind::BasicAssembler, "gearbox"), 300);
    assert_eq!(t(Kind::AdvancedAssembler, "gearbox"), 150);
    assert_eq!(t(Kind::CrudeAssembler, HAND_WIRED), 2400);
    assert_eq!(t(Kind::BasicAssembler, HAND_WIRED), 1200);
    assert_eq!(t(Kind::BasicAssembler, "electric motor"), 600);
    assert_eq!(t(Kind::AdvancedAssembler, "electric motor"), 300);
    assert_eq!(t(Kind::AdvancedAssembler, "control board"), 600);
    assert_eq!(t(Kind::PrecisionAssembler, "control board"), 300);
    assert_eq!(t(Kind::LargeBasicAssembler, "pump unit"), 1200);
    assert_eq!(t(Kind::LargeAdvancedAssembler, "pump unit"), 600);
    assert_eq!(t(Kind::LargeAdvancedAssembler, "generator"), 1200);
    assert_eq!(t(Kind::LargeAdvancedAssembler, "drilling rig"), 2400);
    assert_eq!(t(Kind::LargeAdvancedAssembler, "steam tractor"), 3600);
    assert_eq!(t(Kind::LargePrecisionAssembler, "generator"), 600);
    assert_eq!(t(Kind::LargePrecisionAssembler, "steam tractor"), 1800);
    assert_eq!(t(Kind::LargePrecisionAssembler, "computer"), 3600);
    // Odd times round to a whole tick.
    assert_eq!(scaled(75, 2), 38);
    // Sizes, power and prices.
    let specs = [
        (Kind::CrudeAssembler, (4, 4), 0.0, 300.0),
        (Kind::BasicAssembler, (2, 2), 6.0, 600.0),
        (Kind::AdvancedAssembler, (3, 3), 16.0, 9000.0),
        (Kind::PrecisionAssembler, (3, 3), 64.0, 20_000.0),
        (Kind::LargeBasicAssembler, (5, 5), 12.0, 12_000.0),
        (Kind::LargeAdvancedAssembler, (5, 5), 48.0, 30_000.0),
        (Kind::LargePrecisionAssembler, (5, 5), 160.0, 100_000.0),
    ];
    for (k, dims, ec, cost) in specs {
        assert_eq!((k.base_dims(), k.power_draw(), k.cost()), (dims, ec, cost), "{k:?}");
    }
}

#[test]
fn higher_assemblers_run_crude_recipes_faster() {
    let mut w = World::blank();
    let rows = [(Kind::CrudeAssembler, 4), (Kind::BasicAssembler, 14), (Kind::AdvancedAssembler, 24)];
    for (k, y) in rows {
        w.place(0, 10, y, k, Dir::E).unwrap();
        w.place(0, 10 + k.base_dims().0, y, Kind::Crate, Dir::E).unwrap();
        // Two panels joined by a pole, so even the advanced one (16 EC) has enough.
        w.place(0, 8, y - 2, Kind::Solar, Dir::E).unwrap();
        w.place(0, 12, y - 2, Kind::Solar, Dir::E).unwrap();
        w.place(0, 10, y - 3, Kind::Pole, Dir::E).unwrap();
        pick_recipe(&mut w, 10, y, "gearbox");
    }
    for _ in 0..1200 {
        for (_, y) in rows {
            set_stock(&mut w, 10, y, Item::Steel, 10);
            set_stock(&mut w, 10, y, Item::IronGear, 30);
        }
        w.tick();
    }
    let boxes = |w: &World, k: Kind, y: i32| crate_items(w, 0, 10 + k.base_dims().0, y).len();
    assert_eq!(boxes(&w, Kind::CrudeAssembler, 4), 2, "10 s each");
    assert_eq!(boxes(&w, Kind::BasicAssembler, 14), 4, "twice as fast");
    assert_eq!(boxes(&w, Kind::AdvancedAssembler, 24), 8, "four times as fast");
}

#[test]
fn a_precision_assembler_refuses_crude_and_basic_recipes() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::PrecisionAssembler, Dir::E).unwrap();
    w.place(0, 10, 8, Kind::Solar, Dir::E).unwrap();
    let names = recipe_names(Kind::PrecisionAssembler);
    for crude in CRUDE_RECIPES.iter().chain(BASIC_RECIPES) {
        assert!(!names.contains(&crude.name), "{}", crude.name);
    }
    assert_eq!(
        names,
        [
            "control board",
            "tube board",
            "circuit board",
            "transistor",
            "transistor board",
            "mask",
            "microprocessor (x4x1)",
            "RAM chip",
            "fine mask",
            "microprocessor (x4x2)",
            "mainboard",
            "mainboard (two coarse)"
        ]
    );
    // It cycles between the processors and boards it knows.
    assert!(!w.configure(0, 10, 10).unwrap().contains("only one recipe"));
    w.apply_setting(0, 10, 10, Setting::Recipe(0));
    // It takes what a control board uses, and turns away what only a crude or basic recipe uses.
    assert!(w.offer(0, 10, 10, Item::Relay));
    assert!(!w.offer(0, 10, 10, Item::IronGear), "a crude recipe's input");
    assert!(!w.offer(0, 10, 10, Item::CopperCoil), "a basic recipe's input");
    assert!(!w.offer(0, 10, 10, Item::SteelRod), "a basic recipe's input");
    // A copied recipe number past its list is ignored.
    w.apply_setting(0, 10, 10, Setting::Recipe(40));
    assert!(w.tile(0, 10, 10).unwrap().recipe().is_some());
}

#[test]
fn a_control_board_can_be_wired_by_hand_before_steel() {
    let stock = [(Item::Relay, 1), (Item::FineWire, 4), (Item::IronSheet, 1)];
    // 40 s in a crude assembler, 20 s in a basic one.
    let crude = |secs| run_powered(Kind::CrudeAssembler, HAND_WIRED, &stock, secs);
    let basic = |secs| run_powered(Kind::BasicAssembler, HAND_WIRED, &stock, secs);
    assert!(crude(39).is_empty() && crude(41) == vec![Item::ControlBoard]);
    assert!(basic(19).is_empty() && basic(21) == vec![Item::ControlBoard]);
    // The advanced assembler and up have the proper recipe and hide this one.
    for k in [Kind::AdvancedAssembler, Kind::PrecisionAssembler] {
        assert!(!recipe_names(k).contains(&HAND_WIRED), "{k:?}");
        assert!(recipe_names(k).contains(&"control board"), "{k:?}");
    }
    // Nothing in it needs steel, a gearbox or a motor: only relay, fine wire and iron sheet.
    let made_from_steel =
        [Item::Steel, Item::SteelRod, Item::SteelSheet, Item::SteelGirder, Item::Gearbox, Item::ElectricMotor, Item::Bearing];
    let hand = CRUDE_RECIPES.iter().find(|r| r.name == HAND_WIRED).unwrap();
    assert!(hand.inputs.iter().all(|(i, _)| !made_from_steel.contains(i)));
}

#[test]
fn the_press_makes_crude_sheets_at_three_plates_each() {
    let out = run_machine(Kind::Press, "iron sheet (pressed)", &[(Item::IronPlate, 3)], 6);
    assert_eq!(out, vec![Item::IronSheet]);
    assert!(run_machine(Kind::Press, "iron sheet (pressed)", &[(Item::IronPlate, 2)], 12).is_empty());
    let out = run_machine(Kind::Press, "copper sheet (pressed)", &[(Item::CopperPlate, 3)], 6);
    assert_eq!(out, vec![Item::CopperSheet]);
    // The rolling mill stays the better road: two plates a sheet.
    let mill = ROLLING_MILL_RECIPES.iter().find(|r| r.name == "iron sheet").unwrap();
    assert_eq!(mill.inputs, &[(Item::IronPlate, 2)]);
}

#[test]
fn control_boards_take_10_s_in_an_advanced_assembler_and_5_s_in_a_precision_one() {
    let stock = [(Item::Relay, 1), (Item::FineWire, 4), (Item::IronSheet, 1)];
    let adv = |secs| run_powered(Kind::AdvancedAssembler, "control board", &stock, secs);
    let prec = |secs| run_powered(Kind::PrecisionAssembler, "control board", &stock, secs);
    assert!(adv(9).is_empty() && adv(11) == vec![Item::ControlBoard]);
    assert!(prec(4).is_empty() && prec(6) == vec![Item::ControlBoard]);
    // The other tiers fit the same recipe at their own pace.
    assert_eq!(recipe_ticks(Kind::PrecisionAssembler, "control board") * 2, recipe_ticks(Kind::AdvancedAssembler, "control board"));
}

#[test]
fn the_welder_draws_ec_only_while_it_works() {
    let mut w = World::blank();
    // Two panels (20 EC) joined by a pole, reaching a welder and a belt.
    w.place(0, 12, 10, Kind::Welder, Dir::E).unwrap(); // covers 12-14, 10-12
    w.place(0, 8, 9, Kind::Solar, Dir::E).unwrap();
    w.place(0, 8, 13, Kind::Solar, Dir::E).unwrap();
    w.place(0, 9, 12, Kind::Pole, Dir::E).unwrap();
    w.place(0, 15, 11, Kind::Crate, Dir::E).unwrap();
    w.place(0, 7, 12, Kind::Belt, Dir::E).unwrap();
    assert_eq!(Kind::Welder.burst_draw(), 30.0);
    assert_eq!(Kind::Welder.power_draw(), 0.0, "nothing while idle");
    assert_eq!(w.grid.nets.len(), 1);
    let demand = |w: &World| w.net_at(0, 12, 10).unwrap().demand;
    // Idle: only the belt (0.5) is on the grid.
    w.tick();
    assert_eq!(demand(&w), 0.5);
    assert_eq!(w.ratio_at(0, 12, 10), 1.0);
    // Weld: it draws 30, and the 20 EC grid browns out, slowing the belt too.
    set_stock(&mut w, 12, 10, Item::SteelGirder, 4);
    set_stock(&mut w, 12, 10, Item::Screw, 16);
    pick_recipe(&mut w, 12, 10, "frame");
    w.tick(); // it starts working
    w.tick(); // and the grid feels it
    assert_eq!(demand(&w), 30.5);
    let ratio = 20.0 / 30.5;
    assert!((w.ratio_at(0, 12, 10) - ratio).abs() < 1e-9);
    assert!((w.ratio_at(0, 7, 12) - ratio).abs() < 1e-9, "everything on the grid slows evenly");
    // At that speed a 600 tick weld takes 915 ticks; two welds use up the stock.
    let made = |w: &World| w.econ.made[Item::Frame.idx()];
    for _ in 0..900 {
        w.tick();
    }
    assert_eq!(made(&w), 0, "slowed by the shortfall");
    run(&mut w, 1);
    assert_eq!(made(&w), 1);
    run(&mut w, 15);
    assert_eq!(made(&w), 2, "the second weld");
    // Out of material: the draw switches off and the grid recovers.
    assert_eq!(demand(&w), 0.5);
    assert_eq!(w.ratio_at(0, 7, 12), 1.0);
    assert_eq!(w.idle_reason(0, idx(12, 10)), Some("NEED"));
}

#[test]
fn a_welder_with_no_power_waits() {
    let mut w = World::blank();
    w.place(0, 12, 10, Kind::Welder, Dir::E).unwrap();
    pick_recipe(&mut w, 12, 10, "frame");
    set_stock(&mut w, 12, 10, Item::SteelGirder, 2);
    set_stock(&mut w, 12, 10, Item::Screw, 8);
    run(&mut w, 15);
    assert_eq!(w.econ.made[Item::Frame.idx()], 0);
    assert_eq!(w.idle_reason(0, idx(12, 10)), Some("EC?"));
    // A grid with nothing to give can't start it for free either.
    w.place(0, 12, 13, Kind::Pole, Dir::E).unwrap();
    run(&mut w, 15);
    assert_eq!(w.econ.made[Item::Frame.idx()], 0);
}

#[test]
fn the_kiln_waits_for_a_full_load_then_fires_eight_at_once() {
    let mut w = World::blank();
    // 3 wide, 5 tall facing east; 5 wide, 3 tall facing north.
    assert_eq!(Kind::Kiln.dims(Dir::E), (3, 5));
    assert_eq!(Kind::Kiln.dims(Dir::N), (5, 3));
    w.place(0, 5, 5, Kind::Kiln, Dir::E).unwrap(); // covers 5-7, 5-9: front is x=8
    assert!(w.place(0, 6, 9, Kind::Crate, Dir::E).is_err(), "5 tall");
    w.place(0, 8, 9, Kind::Crate, Dir::E).unwrap(); // beside its bottom row
    set_stock(&mut w, 5, 5, Item::Coal, 5);
    let stones = |w: &mut World, n: u32| (0..n).for_each(|_| assert!(w.offer(0, 5, 5, Item::Stone)));
    let made = |w: &World| w.econ.made[Item::Brick.idx()];
    stones(&mut w, 15);
    run(&mut w, 20);
    assert_eq!(made(&w), 0, "7 runs of 8 is not enough");
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("LOAD"));
    match &w.tile(0, 5, 5).unwrap().st {
        St::Crafter { busy, .. } => assert!(!busy),
        _ => unreachable!(),
    }
    // The last stone starts the fire. All 8 come out together after one recipe time.
    stones(&mut w, 1);
    assert!(!w.offer(0, 5, 5, Item::Stone), "it holds exactly one batch");
    let mut fired_at = None;
    for t in 0..1400 {
        w.tick();
        if made(&w) > 0 {
            assert_eq!(made(&w), 8, "all at once");
            fired_at = Some(t);
            break;
        }
    }
    let t = fired_at.expect("it fired");
    assert!((1199..=1201).contains(&t), "one firing, 20 s (1200 ticks): {t}");
    run(&mut w, 5);
    assert_eq!(crate_items(&w, 0, 8, 9), vec![Item::Brick; 8]);
    assert_eq!(w.econ.used[Item::Stone.idx()], 16);
    // And it is empty again: the next batch needs 16 more.
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("LOAD"));
}

#[test]
fn a_batch_machine_fires_for_one_runs_time_times_batch_over_two() {
    // The rule for the kiln: 5 s a run x 8 runs / 2 = 20 s, so 24 bricks a minute. The large kiln is
    // four kilns in one (decided after review 14), so it keeps the kiln's 20 s firing.
    // The baked tube is the exception: it takes 10 s a run, so a batch of eight fires for 40 s (12 a minute).
    for r in Kind::Kiln.recipes() {
        // The baked tube takes 10 s a run; doped wafer and ceramic 8 s a run (32 s a batch).
        let run_secs = match r.name {
            "tube (baked)" => 10,
            "doped wafer" | "ceramic" => 8,
            _ => 5,
        };
        assert_eq!(r.ticks as u64, run_secs * TPS * Kind::Kiln.batch() as u64 / 2, "{}", r.name);
    }
    let kiln = |k: Kind, name: &str| k.recipes().iter().find(|r| r.name == name).unwrap().ticks;
    assert_eq!((kiln(Kind::Kiln, "brick"), kiln(Kind::LargeKiln, "brick")), (20 * 60, 20 * 60));
    assert_eq!((kiln(Kind::Kiln, "tube (baked)"), kiln(Kind::LargeKiln, "tube (baked)")), (40 * 60, 40 * 60));
}

#[test]
fn the_large_kiln_is_four_kilns_in_one() {
    let (small, big) = (Kind::Kiln.recipes(), Kind::LargeKiln.recipes());
    assert_eq!((Kind::Kiln.batch(), Kind::LargeKiln.batch()), (8, 32));
    assert_eq!(small.len(), big.len());
    for (a, b) in small.iter().zip(big) {
        // Same name, tag, inputs, outputs and fuel: only the time differs.
        assert_eq!((a.name, a.tag, a.inputs, a.outputs, a.fuel), (b.name, b.tag, b.inputs, b.outputs, b.fuel));
        assert_eq!((a.fluid_in, a.fluid_out, a.needs), (b.fluid_in, b.fluid_out, b.needs));
        // Four kilns' worth a minute, for four kilns' coal.
        let rate = |k: Kind, r: &Recipe| r.outputs[0].1 * k.batch() * 60 * 60 / r.ticks;
        assert_eq!(rate(Kind::LargeKiln, b), 4 * rate(Kind::Kiln, a), "{}", a.name);
        // Bricks and glass fire for 20 s; the baked tube takes 40 s; doped wafer and ceramic 32 s.
        let per_minute = match a.name {
            "tube (baked)" => 48,
            "doped wafer" => 60,
            "ceramic" => 120,
            _ => 96,
        };
        assert_eq!(rate(Kind::LargeKiln, b), per_minute, "{}", a.name);
        assert_eq!(Kind::LargeKiln.fires(), 4);
    }
    // Four times the lump: it holds 32 runs of input and 320 finished goods.
    assert_eq!((Kind::Kiln.out_cap(), Kind::LargeKiln.out_cap()), (80, 320));
}

#[test]
fn the_large_kiln_is_5x7_burns_coal_and_costs_a_few_minutes_of_income() {
    assert_eq!((Kind::LargeKiln.dims(Dir::E), Kind::LargeKiln.dims(Dir::N)), ((5, 7), (7, 5)));
    assert!(!Kind::LargeKiln.electric());
    assert_eq!(Kind::LargeKiln.category(), Category::Create);
    assert_eq!(Kind::LargeKiln.name(), "Large kiln");
    // Review 14 change 11: 3 to 5 minutes of income when it unlocks. A tidy player earns about
    // $2,000 a minute when 10,000 RP of motor-stage research arrives (plan 01).
    assert_eq!((Kind::LargeKiln.cost(), Tech::LargeKiln.cost()), (7500.0, 10_000.0));
    let minutes = Kind::LargeKiln.cost() / 2000.0;
    assert!((3.0..=5.0).contains(&minutes), "{minutes} minutes");
    assert!(Kind::LargeKiln.cost() > 10.0 * Kind::Kiln.cost(), "it is something to save for");
    // Sold by whoever sells the kiln, from a tech that is free after 1,000 bricks.
    assert_eq!((Kind::LargeKiln.company(), Tech::LargeKiln.company()), (Kind::Kiln.company(), Kind::Kiln.company().unwrap()));
    assert_eq!(Tech::LargeKiln.milestone(), (Made::Item(Item::Brick), 1000));
    assert_eq!(Kind::LargeKiln.tech(), Some(Tech::LargeKiln));
}

#[test]
fn the_large_kiln_waits_for_a_full_load_then_fires_32_at_once_after_20_seconds() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::LargeKiln, Dir::E).unwrap(); // covers 5-9, 5-11: front is x=10
    assert!(w.place(0, 6, 11, Kind::Crate, Dir::E).is_err(), "7 tall");
    w.place(0, 10, 11, Kind::Crate, Dir::E).unwrap(); // beside its bottom row
    set_stock(&mut w, 5, 5, Item::Coal, 25);
    let stones = |w: &mut World, n: u32| (0..n).for_each(|_| assert!(w.offer(0, 5, 5, Item::Stone)));
    let made = |w: &World| w.econ.made[Item::Brick.idx()];
    stones(&mut w, 63);
    run(&mut w, 100);
    assert_eq!(made(&w), 0, "31 runs of 32 is not enough");
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("LOAD"));
    stones(&mut w, 1);
    assert!(!w.offer(0, 5, 5, Item::Stone), "it holds exactly one batch");
    let mut fired_at = None;
    for t in 0..5000 {
        w.tick();
        if made(&w) > 0 {
            assert_eq!(made(&w), 32, "all at once");
            fired_at = Some(t);
            break;
        }
    }
    let t = fired_at.expect("it fired");
    assert!((1199..=1201).contains(&t), "one firing, 20 s (1200 ticks): {t}");
    assert_eq!(w.econ.used[Item::Stone.idx()], 64);
    // 20 coal for the 20 s: four fires, a coal lasting 1 s each (4 s in a kiln).
    assert_eq!(w.econ.used[Item::Coal.idx()], 20);
    // The goods leave by the front, into the crate beside its edge.
    run(&mut w, 10);
    assert_eq!(crate_items(&w, 0, 10, 11), vec![Item::Brick; 32]);
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("LOAD"));
}

#[test]
fn the_large_kiln_makes_bricks_and_glass() {
    let out = run_machine(Kind::LargeKiln, "brick", &[(Item::Stone, 64), (Item::Coal, 25)], 24);
    assert_eq!(out, vec![Item::Brick; 32]);
    let out = run_machine(Kind::LargeKiln, "glass", &[(Item::Sand, 96), (Item::Coal, 25)], 24);
    assert_eq!(out, vec![Item::Glass; 32]);
    // Still waiting at 15 s.
    assert!(run_machine(Kind::LargeKiln, "brick", &[(Item::Stone, 64), (Item::Coal, 25)], 15).is_empty());
}

#[test]
fn the_kiln_makes_bricks_and_glass_in_batches_of_eight() {
    let out = run_machine(Kind::Kiln, "brick", &[(Item::Stone, 16), (Item::Coal, 5)], 24);
    assert_eq!(out, vec![Item::Brick; 8]);
    let out = run_machine(Kind::Kiln, "glass", &[(Item::Sand, 24), (Item::Coal, 5)], 24);
    assert_eq!(out, vec![Item::Glass; 8]);
    // A firing takes 20 s, not 5: still waiting at 15 s.
    assert!(run_machine(Kind::Kiln, "brick", &[(Item::Stone, 16), (Item::Coal, 5)], 15).is_empty());
    // Not a full batch: nothing fires.
    assert!(run_machine(Kind::Kiln, "glass", &[(Item::Sand, 21), (Item::Coal, 5)], 40).is_empty());
    // No coal, no fire.
    assert!(run_machine(Kind::Kiln, "brick", &[(Item::Stone, 16)], 40).is_empty());
}

#[test]
fn a_pump_pipe_mixer_chain_makes_concrete() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Pump, Dir::E).unwrap(); // covers 5, 5-6
    for x in 6..=8 {
        w.place(0, x, 5, Kind::Pipe, Dir::E).unwrap();
    }
    w.place(0, 9, 5, Kind::Mixer, Dir::E).unwrap(); // covers 9-10, 5-6; front is x=11
    w.place(0, 11, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap(); // reaches the pump and the mixer
    set_stock(&mut w, 9, 5, Item::Sand, 10);
    set_stock(&mut w, 9, 5, Item::Stone, 5);
    w.tick();
    assert_eq!(w.idle_reason(0, idx(9, 5)), Some("FLUD"), "no water yet");
    run(&mut w, 30);
    // 300 ticks a run, 2 sand + 1 stone + 1 water -> 1 concrete: 5 runs.
    assert_eq!(crate_items(&w, 0, 11, 5), vec![Item::Concrete; 5]);
    assert_eq!(w.econ.used[Item::Sand.idx()], 10);
    assert_eq!(w.econ.used[Item::Stone.idx()], 5);
}

#[test]
fn a_mixer_makes_reinforced_concrete_from_concrete_rod_and_water() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Pump, Dir::E).unwrap();
    for x in 6..=8 {
        w.place(0, x, 5, Kind::Pipe, Dir::E).unwrap();
    }
    w.place(0, 9, 5, Kind::Mixer, Dir::E).unwrap();
    w.place(0, 11, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap();
    pick_recipe(&mut w, 9, 5, "reinforced concrete");
    set_stock(&mut w, 9, 5, Item::Concrete, 5);
    set_stock(&mut w, 9, 5, Item::IronRod, 3);
    run(&mut w, 15);
    // 2 concrete + 1 rod + 1 water each: two made, and one concrete is left over.
    assert_eq!(crate_items(&w, 0, 11, 5), vec![Item::ReinforcedConcrete; 2]);
    assert_eq!(w.econ.used[Item::Concrete.idx()], 4);
    assert_eq!(w.econ.used[Item::IronRod.idx()], 2);
}

#[test]
fn a_mixer_without_water_waits() {
    let mut w = World::blank();
    w.place(0, 9, 5, Kind::Mixer, Dir::E).unwrap();
    w.place(0, 11, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap();
    set_stock(&mut w, 9, 5, Item::Sand, 10);
    set_stock(&mut w, 9, 5, Item::Stone, 5);
    run(&mut w, 10);
    assert!(crate_items(&w, 0, 11, 5).is_empty());
    assert_eq!(w.idle_reason(0, idx(9, 5)), Some("FLUD"));
}

#[test]
fn an_evacuator_makes_two_tubes_a_run_from_water_and_its_inputs() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Pump, Dir::E).unwrap();
    for x in 6..=8 {
        w.place(0, x, 5, Kind::Pipe, Dir::E).unwrap();
    }
    w.place(0, 9, 5, Kind::Evacuator, Dir::E).unwrap();
    w.place(0, 11, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap();
    assert_eq!(recipe_names(Kind::Evacuator), ["tube"]);
    set_stock(&mut w, 9, 5, Item::Lamp, 3);
    set_stock(&mut w, 9, 5, Item::CopperSheet, 3);
    set_stock(&mut w, 9, 5, Item::FineWire, 6);
    run(&mut w, 30);
    // 8 s a run: three runs need 24 s, and each makes two tubes with no waste.
    assert_eq!(crate_items(&w, 0, 11, 5), vec![Item::Tube; 6]);
    assert_eq!(w.econ.used[Item::Lamp.idx()], 3);
    assert_eq!(w.econ.used[Item::FineWire.idx()], 6);
    assert_eq!(w.econ.made[Item::Waste.idx()], 0);
}

#[test]
fn an_evacuator_without_water_waits() {
    let mut w = World::blank();
    w.place(0, 9, 5, Kind::Evacuator, Dir::E).unwrap();
    w.place(0, 11, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap();
    set_stock(&mut w, 9, 5, Item::Lamp, 1);
    set_stock(&mut w, 9, 5, Item::CopperSheet, 1);
    set_stock(&mut w, 9, 5, Item::FineWire, 2);
    run(&mut w, 20);
    assert!(crate_items(&w, 0, 11, 5).is_empty());
    assert_eq!(w.idle_reason(0, idx(9, 5)), Some("FLUD"));
}

#[test]
fn the_evacuator_is_a_two_by_two_water_machine_that_sablon_sells() {
    assert_eq!((Kind::Evacuator.cost(), Kind::Evacuator.power_draw(), Kind::Evacuator.base_dims()), (3500.0, 6.0, (2, 2)));
    assert!(Kind::Evacuator.uses_pipes());
    assert_eq!((Kind::Evacuator.company(), Kind::Evacuator.tech()), (Some(Company::Sablon), Some(Tech::Evacuator)));
    assert_eq!(Kind::Evacuator.category(), Category::Create);
    let r = &EVACUATOR_RECIPES[0];
    assert_eq!((r.fluid_in, r.outputs, r.ticks, r.fuel), (Some((Fluid::Water, 1)), &[(Item::Tube, 2)][..], 8 * 60, None));
    assert_eq!(r.inputs, &[(Item::Lamp, 1), (Item::CopperSheet, 1), (Item::FineWire, 2)]);
    // Locked until researched, like the other machines.
    let mut p = Progress::default();
    assert!(!p.part_unlocked(Kind::Evacuator));
    p.opened[Company::Sablon.idx()] = true;
    p.researched[Tech::Evacuator.idx()] = true;
    assert!(p.part_unlocked(Kind::Evacuator));
}

#[test]
fn a_kiln_bakes_tubes_in_forty_seconds_and_sends_waste_out_the_back() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Kiln, Dir::E).unwrap(); // covers 5-7, 5-9: front is x=8, back is x=4
    w.place(0, 8, 9, Kind::Crate, Dir::E).unwrap(); // the tubes
    w.place(0, 4, 5, Kind::Crate, Dir::W).unwrap(); // the waste
    pick_recipe(&mut w, 5, 5, "tube (baked)");
    set_stock(&mut w, 5, 5, Item::Coal, 12); // a coal keeps the fire going for 4 s
    set_stock(&mut w, 5, 5, Item::Lamp, 8);
    set_stock(&mut w, 5, 5, Item::CopperSheet, 8);
    set_stock(&mut w, 5, 5, Item::FineWire, 16);
    run(&mut w, 38);
    assert_eq!(w.econ.made[Item::Tube.idx()], 0, "a full batch fires for 40 s");
    run(&mut w, 5);
    assert_eq!(crate_items(&w, 0, 8, 9), vec![Item::Tube; 8]);
    assert_eq!(crate_items(&w, 0, 4, 5), vec![Item::Waste; 8]);
    // The large kiln picks the recipe up the same way: 32 at once, still 40 s.
    let big = Kind::LargeKiln.recipes().iter().find(|r| r.name == "tube (baked)").expect("the large kiln bakes tubes too");
    assert_eq!((big.ticks, big.outputs), (40 * 60, &[(Item::Tube, 1), (Item::Waste, 1)][..]));
}

#[test]
fn sablons_key_item_can_be_made_without_anything_sablon_sells() {
    // Everything but Sablon (and Lattice & Finch, whose control board the tube board comes after).
    let mut open = [true; COMPANIES];
    open[Company::Sablon.idx()] = false;
    open[Company::LatticeFinch.idx()] = false;
    let (have, _) = makeable(&open);
    assert_eq!(Company::Sablon.key_items(), &[Made::Item(Item::Tube)]);
    assert!(have[Item::Tube.idx()], "baked in the kiln");
    assert!(!have[Item::TubeBoard.idx()], "the tube board needs the advanced assembler");
    // The evacuator is not the way in: it is locked behind Sablon itself.
    assert_eq!(Tech::Evacuator.company(), Company::Sablon);
    assert_eq!(Tech::Evacuator.requires(), &[Made::Item(Item::Tube)]);
    assert_eq!((Tech::Evacuator.cost(), Tech::Evacuator.milestone()), (30_000.0, (Made::Item(Item::Tube), 40)));
    // The baked recipe asks for no tech at all.
    let baked = KILN_RECIPES.iter().find(|r| r.name == "tube (baked)").unwrap();
    assert!(baked.needs.is_none());
    // And the tree puts the tube under the control board, the evacuator under the tube.
    let at = |n: Node| TREE.iter().position(|(m, _)| *m == n).unwrap();
    let board = at(Node::Key(Made::Item(Item::ControlBoard)));
    let tube = at(Node::Key(Made::Item(Item::Tube)));
    let evac = at(Node::Tech(Tech::Evacuator));
    assert_eq!((TREE[tube].1, TREE[evac].1), (Some(board), Some(tube)));
    assert!(UNLOCK_GROUPS.iter().any(|g| g.1.contains(&Tech::Evacuator)));
}

#[test]
fn a_tube_board_is_built_from_tubes_a_control_board_and_iron_sheet() {
    let r = ADVANCED_RECIPES.iter().find(|r| r.name == "tube board").unwrap();
    assert_eq!(r.inputs, &[(Item::Tube, 6), (Item::ControlBoard, 1), (Item::IronSheet, 1)]);
    assert!(r.inputs.iter().all(|(i, _)| *i != Item::SteelSheet), "iron sheet, not steel");
    let stock = [(Item::Tube, 6), (Item::ControlBoard, 1), (Item::IronSheet, 1)];
    // 15 s in an advanced assembler, 7.5 s in a precision one; the crude and basic ones don't know it.
    assert!(run_powered(Kind::AdvancedAssembler, "tube board", &stock, 14).is_empty());
    assert_eq!(run_powered(Kind::AdvancedAssembler, "tube board", &stock, 16), vec![Item::TubeBoard]);
    assert_eq!(run_powered(Kind::PrecisionAssembler, "tube board", &stock, 9), vec![Item::TubeBoard]);
    for k in [Kind::CrudeAssembler, Kind::BasicAssembler, Kind::LargeBasicAssembler, Kind::LargeAdvancedAssembler] {
        assert!(!recipe_names(k).contains(&"tube board"), "{k:?}");
    }
}

#[test]
fn the_transformer_lamp_matrix_and_terminal_come_from_the_assembler_tiers() {
    let coil_sheets = [(Item::CopperCoil, 2), (Item::IronSheet, 4)];
    assert!(run_powered(Kind::CrudeAssembler, "transformer", &coil_sheets, 9).is_empty());
    assert_eq!(run_powered(Kind::CrudeAssembler, "transformer", &coil_sheets, 11), vec![Item::Transformer]);
    let matrix = [(Item::Lamp, 8), (Item::CopperSheet, 1), (Item::FineWire, 4)];
    assert_eq!(run_powered(Kind::CrudeAssembler, "lamp matrix", &matrix, 11), vec![Item::LampMatrix]);
    // The basic assembler runs the crude tier twice as fast.
    assert_eq!(run_powered(Kind::BasicAssembler, "lamp matrix", &matrix, 6), vec![Item::LampMatrix]);
    let parts = [(Item::LampMatrix, 1), (Item::Keyboard, 1), (Item::TubeBoard, 1)];
    assert!(!recipe_names(Kind::CrudeAssembler).contains(&"terminal"));
    assert!(run_powered(Kind::BasicAssembler, "terminal", &parts, 9).is_empty());
    assert_eq!(run_powered(Kind::BasicAssembler, "terminal", &parts, 11), vec![Item::Terminal]);
    // Higher assemblers know the lower tiers' recipes, faster.
    assert_eq!(run_powered(Kind::AdvancedAssembler, "terminal", &parts, 6), vec![Item::Terminal]);
    // The keyboard is part of it: without one, nothing is made.
    let no_keyboard = [(Item::LampMatrix, 1), (Item::TubeBoard, 1)];
    assert!(run_powered(Kind::BasicAssembler, "terminal", &no_keyboard, 30).is_empty());
}

#[test]
fn sablons_letter_comes_with_its_one_tech() {
    let m = crate::mail::letter(Company::Sablon, Made::Item(Item::Tube), 0);
    assert!(m.md.contains("Nothing is as patient as a tube."), "{}", m.md);
    assert!(m.md.contains("- Evacuator (Create)"), "{}", m.md);
    assert!(!crate::mail::gift_note(Company::Sablon).is_empty());
}

// ---------- silicon on the old machines (computers phase 2a) ----------

/// A blast furnace on `recipe`, with its waste crate behind it and a molder casting its molten
/// silicon into a crate. Returns (silicon ingots, waste) after `secs`.
fn silicon_route(stock: &[(Item, u32)], secs: u64) -> (usize, usize) {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::BlastFurnace, Dir::E).unwrap(); // covers 5-6, 5-6: front is x=7
    pick_recipe(&mut w, 5, 5, "molten silicon");
    for (item, n) in stock {
        set_stock(&mut w, 5, 5, *item, *n);
    }
    w.place(0, 4, 5, Kind::Crate, Dir::W).unwrap(); // the waste, out the back
    w.place(0, 7, 6, Kind::Molder, Dir::E).unwrap();
    pick_recipe(&mut w, 7, 6, "silicon ingot");
    w.place(0, 9, 6, Kind::Crate, Dir::E).unwrap(); // the ingots
    w.place(0, 7, 8, Kind::Solar, Dir::E).unwrap();
    run(&mut w, secs);
    (
        crate_items(&w, 0, 9, 6).iter().filter(|i| **i == Item::SiliconIngot).count(),
        crate_items(&w, 0, 4, 5).iter().filter(|i| **i == Item::Waste).count(),
    )
}

#[test]
fn a_blast_furnace_makes_molten_silicon_and_two_waste_from_sand_and_coke() {
    let r = Kind::BlastFurnace.recipes().iter().find(|r| r.name == "molten silicon").unwrap();
    assert_eq!((r.inputs, r.outputs, r.ticks), (&[(Item::Sand, 3), (Item::Coke, 1)][..], &[(Item::Waste, 2)][..], 10 * 60));
    assert_eq!((r.fluid_out, r.fluid_in, r.fuel, r.needs), (Some((Fluid::MoltenSilicon, 1)), None, None, None));
    let stock = [(Item::Sand, 3), (Item::Coke, 1)];
    assert_eq!(silicon_route(&stock, 9), (0, 0), "still melting");
    assert_eq!(silicon_route(&stock, 11), (0, 2), "the waste first, the silicon still casting");
    assert_eq!(silicon_route(&stock, 14), (1, 2), "10 s melting, 2.5 s casting");
    // Without the coke, it waits.
    assert_eq!(silicon_route(&[(Item::Sand, 3)], 30), (0, 0));
    // Molten silicon is not sold: it has no price, and no company opens on it.
    assert!(Company::ALL.iter().all(|c| !c.key_items().contains(&Made::Fluid(Fluid::MoltenSilicon))));
}

#[test]
fn silicon_is_wafers_and_scrap_in_the_lathe_and_doped_wafers_in_the_kiln() {
    // The lathe: 2 ingots give 2 wafers and 2 scrap, all out of the front (scrap is not waste), in 40 s.
    let r = LATHE_RECIPES.iter().find(|r| r.name == "wafer (rough)").unwrap();
    assert_eq!(r.outputs, &[(Item::Wafer, 2), (Item::SiliconScrap, 2)]);
    assert_eq!(r.ticks, 40 * 60);
    assert!(run_powered(Kind::Lathe, "wafer (rough)", &[(Item::SiliconIngot, 2)], 39).is_empty());
    let out = run_powered(Kind::Lathe, "wafer (rough)", &[(Item::SiliconIngot, 2)], 41);
    assert_eq!((count(&out, Item::Wafer), count(&out, Item::SiliconScrap), out.len()), (2, 2, 4));
    assert!(Item::SiliconScrap.price() > 0.0, "scrap sells");
    // The kiln: a full batch of 8 fires for 32 s and gives 8 doped wafers from 8 wafers and 16 dopant.
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Kiln, Dir::E).unwrap(); // front is x=8
    w.place(0, 8, 9, Kind::Crate, Dir::E).unwrap();
    pick_recipe(&mut w, 5, 5, "doped wafer");
    set_stock(&mut w, 5, 5, Item::Coal, 12);
    set_stock(&mut w, 5, 5, Item::Wafer, 8);
    set_stock(&mut w, 5, 5, Item::Dopant, 16);
    run(&mut w, 30);
    assert_eq!(w.econ.made[Item::DopedWafer.idx()], 0, "a full batch fires for 32 s");
    run(&mut w, 5);
    assert_eq!(crate_items(&w, 0, 8, 9), vec![Item::DopedWafer; 8]);
    // Dopant and ceramic, and the large kiln, pick the new recipes up with the rest.
    let find = |k: Kind, n: &str| *k.recipes().iter().find(|r| r.name == n).unwrap_or_else(|| panic!("{k:?} has no {n}"));
    for k in [Kind::Kiln, Kind::LargeKiln] {
        let d = find(k, "dopant");
        assert_eq!((d.inputs, d.outputs, d.fuel), (&[(Item::Stone, 4)][..], &[(Item::Dopant, 1)][..], Some(Item::Coal)));
        let c = find(k, "ceramic");
        assert_eq!((c.inputs, c.outputs, c.fuel), (&[(Item::Brick, 2), (Item::Sand, 1)][..], &[(Item::Ceramic, 2)][..], Some(Item::Coal)));
        assert!(find(k, "doped wafer").needs.is_none());
    }
    assert_eq!(
        (find(Kind::Kiln, "dopant").ticks, find(Kind::Kiln, "doped wafer").ticks, find(Kind::Kiln, "ceramic").ticks),
        (1200, 1920, 1920)
    );
}

#[test]
fn the_advanced_assemblers_build_circuit_boards_transistors_and_transistor_boards() {
    let find = |n: &str| ADVANCED_RECIPES.iter().find(|r| r.name == n).unwrap();
    assert_eq!(find("circuit board").inputs, &[(Item::Ceramic, 1), (Item::CopperSheet, 1), (Item::FineWire, 4)]);
    assert_eq!(find("transistor").inputs, &[(Item::DopedWafer, 1), (Item::FineWire, 6), (Item::IronSheet, 1)]);
    assert_eq!(find("transistor board").inputs, &[(Item::Transistor, 6), (Item::CircuitBoard, 1), (Item::FineWire, 4)]);
    assert!(ADVANCED_RECIPES.iter().all(|r| r.name == "tube board" || r.name == "control board" || r.ticks == 10 * 60));
    // One doped wafer gives three transistors, in 10 s.
    let stock = [(Item::DopedWafer, 1), (Item::FineWire, 6), (Item::IronSheet, 1)];
    assert!(run_powered(Kind::AdvancedAssembler, "transistor", &stock, 9).is_empty());
    assert_eq!(run_powered(Kind::AdvancedAssembler, "transistor", &stock, 11), vec![Item::Transistor; 3]);
    // The precision assembler knows them too, twice as fast.
    assert_eq!(run_powered(Kind::PrecisionAssembler, "transistor", &stock, 6), vec![Item::Transistor; 3]);
    let board = [(Item::Transistor, 6), (Item::CircuitBoard, 1), (Item::FineWire, 4)];
    assert_eq!(run_powered(Kind::AdvancedAssembler, "transistor board", &board, 11), vec![Item::TransistorBoard]);
    let circuit = [(Item::Ceramic, 1), (Item::CopperSheet, 1), (Item::FineWire, 4)];
    assert_eq!(run_powered(Kind::AdvancedAssembler, "circuit board", &circuit, 11), vec![Item::CircuitBoard]);
    for k in [Kind::CrudeAssembler, Kind::BasicAssembler, Kind::LargeBasicAssembler, Kind::LargeAdvancedAssembler] {
        assert!(!recipe_names(k).contains(&"transistor"), "{k:?}");
    }
}

#[test]
fn halversons_key_is_silicon_made_before_it_sells_anything() {
    assert_eq!(Company::Halverson.key_items(), &[Made::Item(Item::SiliconIngot), Made::Fluid(Fluid::CrudeOil)]);
    // It sells exactly three techs and their machines: the arc furnace, crystal grower and wafer saw.
    let techs: Vec<Tech> = Tech::ALL.iter().copied().filter(|t| t.company() == Company::Halverson).collect();
    assert_eq!(techs, [Tech::ArcFurnace, Tech::CrystalGrower, Tech::WaferSaw, Tech::LiquidTrader, Tech::Still]);
    let kinds: Vec<Kind> = Kind::ALL.iter().copied().filter(|k| k.company() == Some(Company::Halverson)).collect();
    assert_eq!(kinds, [Kind::ArcFurnace, Kind::CrystalGrower, Kind::WaferSaw, Kind::LiquidTrader, Kind::Still]);
    // Sand and coke melt in a blast furnace (Cinderhall), a crusher grinds the sand (Ashgrove),
    // and a molder casts it: with only the companies whose techs those need, the ingot is makeable.
    let mut open = [false; COMPANIES];
    for c in Company::ALL {
        open[c.idx()] = c.key_items().is_empty();
    }
    for c in [Company::Cinderhall, Company::Ashgrove] {
        open[c.idx()] = true;
    }
    let (have, _) = makeable(&open);
    assert!(have[Item::SiliconIngot.idx()], "silicon ingot with only the early companies");
    assert!(!open[Company::Halverson.idx()] && !open[Company::Sablon.idx()]);
    // In the tree it hangs under the kiln, beside its dopant and ceramic.
    let at = |n: Node| TREE.iter().position(|(m, _)| *m == n).unwrap();
    assert_eq!(TREE[at(Node::Key(Made::Item(Item::SiliconIngot)))].1, Some(at(Node::Tech(Tech::Kiln))));
    // A letter that lists the three techs, and a gift note.
    let m = crate::mail::letter(Company::Halverson, Made::Item(Item::SiliconIngot), 0);
    assert!(m.md.contains("A lovely byproduct of sand."), "{}", m.md);
    assert!(
        m.md.contains("Unlocked for research") && m.md.contains("- Arc furnace (Create)") && m.md.contains("- Wafer saw (Create)"),
        "{}",
        m.md
    );
    assert_eq!(m.from, "Halverson Smelting");
    assert!(!crate::mail::gift_note(Company::Halverson).is_empty());
}

#[test]
fn a_mixer_makes_two_etchant_and_a_waste_with_the_waste_out_the_back() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Pump, Dir::E).unwrap();
    for x in 6..=8 {
        w.place(0, x, 5, Kind::Pipe, Dir::E).unwrap();
    }
    w.place(0, 9, 5, Kind::Mixer, Dir::E).unwrap(); // covers 9-10, 5-6; front is x=11, back is x=8
    w.place(0, 11, 5, Kind::Crate, Dir::E).unwrap(); // the etchant
    w.place(0, 8, 6, Kind::Crate, Dir::W).unwrap(); // the waste
    w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap();
    pick_recipe(&mut w, 9, 5, "etchant");
    let r = MIXER_RECIPES.iter().find(|r| r.name == "etchant").unwrap();
    assert_eq!((r.inputs, r.fluid_in, r.ticks), (&[(Item::Coal, 1), (Item::Sand, 1)][..], Some((Fluid::Water, 1)), 5 * 60));
    assert_eq!(r.outputs, &[(Item::Etchant, 2), (Item::Waste, 1)]);
    set_stock(&mut w, 9, 5, Item::Coal, 2);
    set_stock(&mut w, 9, 5, Item::Sand, 2);
    run(&mut w, 4);
    assert_eq!(w.econ.made[Item::Etchant.idx()], 0, "a run takes 5 s");
    run(&mut w, 8);
    assert_eq!(crate_items(&w, 0, 11, 5), vec![Item::Etchant; 4]);
    assert_eq!(crate_items(&w, 0, 8, 6), vec![Item::Waste; 2], "one waste a run, out the back");
}

#[test]
fn a_mixer_with_nowhere_for_waste_stops_when_it_fills_up() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Pump, Dir::E).unwrap();
    for x in 6..=8 {
        w.place(0, x, 5, Kind::Pipe, Dir::E).unwrap();
    }
    w.place(0, 9, 5, Kind::Mixer, Dir::E).unwrap();
    w.place(0, 11, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap();
    pick_recipe(&mut w, 9, 5, "etchant");
    set_stock(&mut w, 9, 5, Item::Coal, 30);
    set_stock(&mut w, 9, 5, Item::Sand, 30);
    run(&mut w, 120);
    // Nothing takes the waste: it piles up to the machine's cap and the machine stops.
    assert_eq!(w.econ.made[Item::Waste.idx()] as usize, CRAFTER_OUT_CAP);
    assert_eq!(w.idle_reason(0, idx(9, 5)), Some("BACK"));
}

#[test]
fn an_etcher_makes_two_dies_and_two_scrap_from_a_wafer_a_mask_and_etchant() {
    assert_eq!(recipe_names(Kind::Etcher), ["die"]);
    let r = &ETCHER_RECIPES[0];
    assert_eq!(r.inputs, &[(Item::DopedWafer, 1), (Item::Mask, 1), (Item::Etchant, 1)]);
    assert_eq!(r.outputs, &[(Item::Die, 2), (Item::SiliconScrap, 2)]);
    assert_eq!((r.fluid_in, r.fluid_out, r.fuel, r.needs, r.ticks), (None, None, None, None, 15 * 60));
    let stock = [(Item::DopedWafer, 1), (Item::Mask, 1), (Item::Etchant, 1)];
    assert!(run_powered(Kind::Etcher, "die", &stock, 14).is_empty());
    let out = run_powered(Kind::Etcher, "die", &stock, 16);
    // The scrap is a product, out the front with the dies; nothing is waste.
    assert_eq!((count(&out, Item::Die), count(&out, Item::SiliconScrap), out.len()), (2, 2, 4));
    // Without the etchant, it waits.
    let out = run_powered(Kind::Etcher, "die", &[(Item::DopedWafer, 1), (Item::Mask, 1)], 30);
    assert!(out.is_empty());
}

#[test]
fn the_etcher_is_a_three_by_three_ec_machine_that_lattice_and_finch_sell() {
    assert_eq!((Kind::Etcher.cost(), Kind::Etcher.power_draw(), Kind::Etcher.base_dims()), (25_000.0, 24.0, (3, 3)));
    assert!(Kind::Etcher.electric() && !Kind::Etcher.uses_pipes());
    assert_eq!((Kind::Etcher.company(), Kind::Etcher.tech()), (Some(Company::LatticeFinch), Some(Tech::Etcher)));
    assert_eq!(Kind::Etcher.category(), Category::Create);
    assert_eq!(
        (Tech::Etcher.company(), Tech::Etcher.cost(), Tech::Etcher.milestone()),
        (Company::LatticeFinch, 80_000.0, (Made::Item(Item::TransistorBoard), 60))
    );
    assert_eq!(Tech::Etcher.requires(), &[Made::Item(Item::ControlBoard)]);
    // The sixth slot of Create, first in it (the grower and the saw follow).
    assert_eq!(crate::app::slots(Category::Create)[5][0], Kind::Etcher);
    // In the tree beside the precision assembler, and in the same research group.
    let at = |n: Node| TREE.iter().position(|(m, _)| *m == n).unwrap();
    let board = at(Node::Key(Made::Item(Item::ControlBoard)));
    assert_eq!(TREE[at(Node::Tech(Tech::Etcher))].1, Some(board));
    assert_eq!(TREE[at(Node::Tech(Tech::PrecisionAssembler))].1, Some(board));
    let group = |t: Tech| UNLOCK_GROUPS.iter().position(|g| g.1.contains(&t));
    assert!(group(Tech::Etcher).is_some() && group(Tech::Etcher) == group(Tech::PrecisionAssembler));
    // Locked until researched, like the other machines.
    let mut p = Progress::default();
    assert!(!p.part_unlocked(Kind::Etcher));
    p.opened[Company::LatticeFinch.idx()] = true;
    p.researched[Tech::Etcher.idx()] = true;
    assert!(p.part_unlocked(Kind::Etcher));
}

#[test]
fn a_mask_is_glass_a_copper_sheet_and_fine_wire_in_an_advanced_assembler() {
    let r = ADVANCED_RECIPES.iter().find(|r| r.name == "mask").unwrap();
    assert_eq!(r.inputs, &[(Item::Glass, 1), (Item::CopperSheet, 1), (Item::FineWire, 2)]);
    assert_eq!((r.outputs, r.ticks), (&[(Item::Mask, 1)][..], 600));
    let stock = [(Item::Glass, 1), (Item::CopperSheet, 1), (Item::FineWire, 2)];
    assert!(run_powered(Kind::AdvancedAssembler, "mask", &stock, 9).is_empty());
    assert_eq!(run_powered(Kind::AdvancedAssembler, "mask", &stock, 11), vec![Item::Mask]);
    assert_eq!(run_powered(Kind::PrecisionAssembler, "mask", &stock, 6), vec![Item::Mask], "twice as fast");
    for k in [Kind::CrudeAssembler, Kind::BasicAssembler, Kind::LargeBasicAssembler, Kind::LargeAdvancedAssembler] {
        assert!(!recipe_names(k).contains(&"mask"), "{k:?}");
    }
}

#[test]
fn only_the_precision_assembler_builds_the_x4x1_microprocessor_and_ram_chip() {
    let find = |n: &str| PRECISION_RECIPES.iter().find(|r| r.name == n).unwrap();
    assert_eq!(find("microprocessor (x4x1)").inputs, &[(Item::Die, 1), (Item::Ceramic, 1), (Item::FineWire, 4)]);
    assert_eq!(find("RAM chip").inputs, &[(Item::Die, 1), (Item::Ceramic, 1), (Item::FineWire, 2)]);
    // 10 s each, but the coarse mainboard (two x4x1 for one x4x2) takes twice as long.
    assert!(PRECISION_RECIPES.iter().all(|r| r.ticks == if r.name == "mainboard (two coarse)" { 20 * 60 } else { 10 * 60 }));
    let cpu = [(Item::Die, 1), (Item::Ceramic, 1), (Item::FineWire, 4)];
    let ram = [(Item::Die, 1), (Item::Ceramic, 1), (Item::FineWire, 2)];
    assert!(run_powered(Kind::PrecisionAssembler, "microprocessor (x4x1)", &cpu, 9).is_empty());
    assert_eq!(run_powered(Kind::PrecisionAssembler, "microprocessor (x4x1)", &cpu, 11), vec![Item::Microprocessor]);
    assert_eq!(run_powered(Kind::PrecisionAssembler, "RAM chip", &ram, 11), vec![Item::RamChip]);
    // No other machine has them, the advanced assemblers included.
    for k in Kind::ALL.into_iter().filter(|k| *k != Kind::PrecisionAssembler) {
        let names = recipe_names(k);
        assert!(!names.contains(&"microprocessor (x4x1)") && !names.contains(&"RAM chip"), "{k:?}");
        assert!(!names.contains(&"fine mask") && !names.contains(&"microprocessor (x4x2)"), "{k:?}");
    }
    // Each product has its own letter in the precision assembler.
    assert_eq!((find("microprocessor (x4x1)").tag, find("RAM chip").tag), ('M', 'R'));
    assert_eq!((find("fine mask").tag, find("microprocessor (x4x2)").tag), ('F', 'X'));
    assert_eq!((find("mainboard").tag, find("mainboard (two coarse)").tag), ('B', 'B'));
}

#[test]
fn meridians_key_is_the_x4x1_made_before_it_sells_anything() {
    assert_eq!(Company::Meridian.key_items(), &[Made::Item(Item::Microprocessor)]);
    assert_eq!(Company::Meridian.name(), "Meridian Instruments");
    // It sells exactly the fine etcher and the large precision assembler.
    let techs: Vec<Tech> = Tech::ALL.iter().copied().filter(|t| t.company() == Company::Meridian).collect();
    assert_eq!(techs, [Tech::FineEtcher, Tech::LargePrecisionAssembler]);
    let kinds: Vec<Kind> = Kind::ALL.iter().copied().filter(|k| k.company() == Some(Company::Meridian)).collect();
    assert_eq!(kinds, [Kind::FineEtcher, Kind::LargePrecisionAssembler]);
    // The etcher comes from Lattice & Finch, the mask from the advanced assembler, the etchant
    // from Wellbourne's mixer: with Meridian, Halverson, Sablon and Ampleworth shut, the chip is makeable.
    let mut open = [true; COMPANIES];
    for c in [Company::Meridian, Company::Halverson, Company::Sablon, Company::Ampleworth] {
        open[c.idx()] = false;
    }
    let (have, _) = makeable(&open);
    assert!(have[Item::Microprocessor.idx()] && have[Item::RamChip.idx()] && have[Item::Die.idx()]);
    // Without Lattice & Finch there is no etcher, so no die and no chip.
    open[Company::LatticeFinch.idx()] = false;
    let (have, _) = makeable(&open);
    assert!(!have[Item::Die.idx()] && !have[Item::Microprocessor.idx()]);
    // In the tree it hangs under the etcher.
    let at = |n: Node| TREE.iter().position(|(m, _)| *m == n).unwrap();
    assert_eq!(TREE[at(Node::Key(Made::Item(Item::Microprocessor)))].1, Some(at(Node::Tech(Tech::Etcher))));
    // A letter whose research list is the fine etcher and the large precision assembler, and a gift note.
    let m = crate::mail::letter(Company::Meridian, Made::Item(Item::Microprocessor), 0);
    assert!(m.md.contains("Your very first chip! We are all friends here."), "{}", m.md);
    assert!(m.md.contains("Unlocked for research") && m.md.contains("Fine etcher") && m.md.contains("Large prec. assembler"), "{}", m.md);
    assert_eq!(m.from, "Meridian Instruments");
    assert!(!crate::mail::gift_note(Company::Meridian).is_empty());
}

// ---------- computers (phase 5): the computer and the large precision assembler ----------

/// The five things a computer is made of.
const COMPUTER_PARTS: [(Item, u32); 5] =
    [(Item::Mainboard, 1), (Item::Transformer, 1), (Item::Terminal, 1), (Item::DiskDrive, 1), (Item::Frame, 1)];

#[test]
fn the_computers_new_items_have_their_recipes_prices_and_curves() {
    let find = |kind: Kind, name: &str| kind.recipes().iter().find(|r| r.name == name).unwrap();
    let shape = |r: &Recipe| (r.inputs.to_vec(), r.outputs.to_vec(), r.ticks);
    // Crude tier: the keyboard, 10 s.
    let r = find(Kind::CrudeAssembler, "keyboard");
    assert_eq!(shape(r), (vec![(Item::Spring, 4), (Item::CircuitBoard, 1), (Item::IronSheet, 1)], vec![(Item::Keyboard, 1)], 600));
    // Basic tier: the terminal gains the keyboard, and the disk drive.
    let r = find(Kind::BasicAssembler, "terminal");
    assert_eq!(r.inputs, &[(Item::LampMatrix, 1), (Item::Keyboard, 1), (Item::TubeBoard, 1)]);
    let r = find(Kind::BasicAssembler, "disk drive");
    assert_eq!(
        shape(r),
        (vec![(Item::ElectricMotor, 1), (Item::SteelSheet, 1), (Item::TransistorBoard, 1)], vec![(Item::DiskDrive, 1)], 600)
    );
    // Precision tier: the mainboard, with the x4x2 or with two x4x1.
    let r = find(Kind::PrecisionAssembler, "mainboard");
    assert_eq!(
        shape(r),
        (vec![(Item::CircuitBoard, 1), (Item::FineMicroprocessor, 1), (Item::RamChip, 4)], vec![(Item::Mainboard, 1)], 600)
    );
    let r = find(Kind::PrecisionAssembler, "mainboard (two coarse)");
    assert_eq!(shape(r), (vec![(Item::CircuitBoard, 1), (Item::Microprocessor, 2), (Item::RamChip, 4)], vec![(Item::Mainboard, 1)], 1200));
    // The computer, 60 s.
    let r = find(Kind::LargePrecisionAssembler, "computer");
    assert_eq!(shape(r), (COMPUTER_PARTS.to_vec(), vec![(Item::Computer, 1)], 3600));
    // Prices, and about twice the price in research (the computer is the crown: worth 22,000).
    for (item, price, worth) in [
        (Item::Keyboard, 260.0, 520.0),
        (Item::DiskDrive, 1100.0, 2200.0),
        (Item::Mainboard, 1800.0, 3600.0),
        (Item::Computer, 11_000.0, 22_000.0),
    ] {
        assert_eq!((item.price(), item.curve().worth), (price, worth), "{item:?}");
    }
    let c = Item::Computer.curve();
    assert_eq!((c.decay, c.floor), (0.99, 0.3));
    assert_eq!(ITEMS, 79);
    // Carbon black sells, and waits for its later machines.
    assert_eq!((Item::CarbonBlack.price(), Item::CarbonBlack.curve().worth), (3.0, 10.0));
}

#[test]
fn a_terminal_needs_the_keyboard() {
    let without = [(Item::LampMatrix, 1), (Item::TubeBoard, 1)];
    assert!(run_powered(Kind::BasicAssembler, "terminal", &without, 40).is_empty(), "no keyboard, no terminal");
    let with = [(Item::LampMatrix, 1), (Item::Keyboard, 1), (Item::TubeBoard, 1)];
    assert_eq!(run_powered(Kind::BasicAssembler, "terminal", &with, 11), vec![Item::Terminal]);
    // The keyboard itself is a crude part: 4 springs, a circuit board and an iron sheet.
    let keyboard = [(Item::Spring, 4), (Item::CircuitBoard, 1), (Item::IronSheet, 1)];
    assert!(run_powered(Kind::CrudeAssembler, "keyboard", &keyboard, 9).is_empty());
    assert_eq!(run_powered(Kind::CrudeAssembler, "keyboard", &keyboard, 11), vec![Item::Keyboard]);
}

#[test]
fn a_precision_assembler_makes_the_mainboard_both_ways() {
    let fine = [(Item::CircuitBoard, 1), (Item::FineMicroprocessor, 1), (Item::RamChip, 4)];
    assert!(run_powered(Kind::PrecisionAssembler, "mainboard", &fine, 9).is_empty());
    assert_eq!(run_powered(Kind::PrecisionAssembler, "mainboard", &fine, 11), vec![Item::Mainboard], "10 s with the x4x2");
    let coarse = [(Item::CircuitBoard, 1), (Item::Microprocessor, 2), (Item::RamChip, 4)];
    assert!(run_powered(Kind::PrecisionAssembler, "mainboard (two coarse)", &coarse, 19).is_empty());
    assert_eq!(run_powered(Kind::PrecisionAssembler, "mainboard (two coarse)", &coarse, 21), vec![Item::Mainboard], "20 s with two x4x1");
    // Only the precision assembler has it.
    for k in Kind::ALL.into_iter().filter(|k| *k != Kind::PrecisionAssembler) {
        assert!(!recipe_names(k).iter().any(|n| n.starts_with("mainboard")), "{k:?}");
    }
}

#[test]
fn a_fully_powered_large_precision_assembler_makes_a_computer_in_a_minute() {
    assert!(run_powered(Kind::LargePrecisionAssembler, "computer", &COMPUTER_PARTS, 59).is_empty(), "nothing before 60 s");
    assert_eq!(run_powered(Kind::LargePrecisionAssembler, "computer", &COMPUTER_PARTS, 61), vec![Item::Computer]);
    // Short of one part, nothing is made at all.
    assert!(run_powered(Kind::LargePrecisionAssembler, "computer", &COMPUTER_PARTS[1..], 90).is_empty());
    // It is hungry: 100 EC is not enough for its 160 (run_powered's grid grows to feed it).
    let mut w = World::blank();
    big_grid(&mut w, 10, 0, 10);
    w.place(0, 12, 12, Kind::LargePrecisionAssembler, Dir::E).unwrap();
    assert!(w.ratio_at(0, 12, 12) < 1.0);
    let mut w = World::blank();
    big_grid(&mut w, 10, 0, 16);
    w.place(0, 12, 12, Kind::LargePrecisionAssembler, Dir::E).unwrap();
    assert_eq!(w.ratio_at(0, 12, 12), 1.0);
}

#[test]
fn only_the_large_precision_assembler_builds_the_computer() {
    // A large advanced assembler has no such recipe, and no machine but the large precision one does.
    assert!(!recipe_names(Kind::LargeAdvancedAssembler).contains(&"computer"));
    for k in Kind::ALL.into_iter().filter(|k| *k != Kind::LargePrecisionAssembler) {
        assert!(!recipe_names(k).contains(&"computer"), "{k:?}");
    }
    // A large advanced assembler left holding the parts makes nothing.
    let mut w = World::blank();
    big_grid(&mut w, 10, 0, 10);
    w.place(0, 12, 12, Kind::LargeAdvancedAssembler, Dir::E).unwrap();
    for (item, n) in COMPUTER_PARTS {
        set_stock(&mut w, 12, 12, item, n);
    }
    w.place(0, 17, 12, Kind::Crate, Dir::E).unwrap();
    run(&mut w, 120);
    assert!(crate_items(&w, 0, 17, 12).is_empty());
}

#[test]
fn the_large_precision_assembler_runs_the_large_advanced_recipes_in_half_the_time() {
    let generator = [(Item::Frame, 1), (Item::ElectricMotor, 2), (Item::Gauge, 1), (Item::ControlBoard, 1)];
    // 20 s in a large advanced assembler, 10 s here.
    assert!(run_powered(Kind::LargePrecisionAssembler, "generator", &generator, 9).is_empty());
    assert_eq!(run_powered(Kind::LargePrecisionAssembler, "generator", &generator, 11), vec![Item::Generator]);
    assert!(run_powered(Kind::LargeAdvancedAssembler, "generator", &generator, 11).is_empty());
    let tractor = [(Item::EngineBlock, 1), (Item::Frame, 1), (Item::HardenedGear, 4), (Item::ControlPanel, 1), (Item::Generator, 1)];
    assert!(run_powered(Kind::LargePrecisionAssembler, "steam tractor", &tractor, 29).is_empty());
    assert_eq!(run_powered(Kind::LargePrecisionAssembler, "steam tractor", &tractor, 31), vec![Item::SteamTractor]);
    // Nothing below: no large basic goods (the pump unit, the engine block).
    let names = recipe_names(Kind::LargePrecisionAssembler);
    assert_eq!(names, ["generator", "control panel", "drilling rig", "steam tractor", "computer"]);
    // Its letters: one per product.
    let tag = |n: &str| Kind::LargePrecisionAssembler.recipes().iter().find(|r| r.name == n).unwrap().tag;
    assert_eq!(
        (tag("generator"), tag("control panel"), tag("drilling rig"), tag("steam tractor"), tag("computer")),
        ('G', 'P', 'D', 'T', 'C')
    );
}

#[test]
fn meridian_sells_the_fine_etcher_and_the_large_precision_assembler() {
    let sold: Vec<Tech> = Tech::ALL.iter().copied().filter(|t| t.company() == Company::Meridian).collect();
    assert_eq!(sold, [Tech::FineEtcher, Tech::LargePrecisionAssembler]);
    let t = Tech::LargePrecisionAssembler;
    assert_eq!((t.cost(), t.milestone()), (300_000.0, (Made::Item(Item::Mainboard), 40)));
    assert_eq!(Kind::LargePrecisionAssembler.tech(), Some(t));
    assert_eq!(Kind::LargePrecisionAssembler.company(), Some(Company::Meridian));
    assert_eq!(Kind::LargePrecisionAssembler.cost(), 100_000.0);
    assert_eq!(Kind::LargePrecisionAssembler.base_dims(), (5, 5));
    // It sits in the research group that holds the large advanced assembler, and in the tree under the x4x1, beside the fine etcher.
    let group = UNLOCK_GROUPS.iter().find(|(_, techs)| techs.contains(&Tech::LargeAdvancedAssembler)).unwrap();
    assert!(group.1.contains(&t));
    let at = |n: Node| TREE.iter().position(|(m, _)| *m == n).unwrap();
    assert_eq!(TREE[at(Node::Tech(t))].1, TREE[at(Node::Tech(Tech::FineEtcher))].1);
    assert_eq!(TREE[at(Node::Tech(t))].1, Some(at(Node::Key(Made::Item(Item::Microprocessor)))));
}

/// A save made before this phase has a terminal half-built (its lamp matrix and tube board are in stock; inputs
/// leave at the end of a run). It opens in this build and the terminal keeps its name (recipes are saved by name,
/// and the keyboard joined the list before it). The machine now waits for a keyboard, keeping what it holds.
#[test]
fn a_terminal_mid_recipe_in_an_older_save_waits_for_a_keyboard_and_finishes() {
    let mut w = World::blank();
    big_grid(&mut w, 10, 0, 10);
    w.place(0, 12, 12, Kind::BasicAssembler, Dir::E).unwrap();
    pick_recipe(&mut w, 12, 12, "terminal");
    set_stock(&mut w, 12, 12, Item::LampMatrix, 1);
    set_stock(&mut w, 12, 12, Item::TubeBoard, 1);
    if let Some(Building { st: St::Crafter { progress, busy, .. }, .. }) = w.floors[0].tiles[idx(12, 12)].as_mut() {
        *progress = 300.0;
        *busy = true;
    }
    w.place(0, 14, 12, Kind::Crate, Dir::E).unwrap();
    let data = saving::temp("terminal-mid-run");
    let path = crate::saves::path_of(&data, "save1");
    w.save(&path).unwrap();
    let mut l = World::load(&path).unwrap();
    assert_eq!(l.tile(0, 12, 12).unwrap().recipe().unwrap().name, "terminal");
    run(&mut l, 20);
    assert!(crate_items(&l, 0, 14, 12).is_empty(), "no keyboard, so it waits");
    set_stock(&mut l, 12, 12, Item::Keyboard, 1);
    run(&mut l, 6);
    assert_eq!(crate_items(&l, 0, 14, 12), vec![Item::Terminal], "the half-done run carries on from where it was");
    let _ = std::fs::remove_dir_all(&data);
}

/// `n` panels (10 EC each) in a column at x - 2, joined by poles at x, from row y down.
fn big_grid(w: &mut World, x: i32, y: i32, n: i32) {
    for i in 0..n {
        w.place(0, x - 2, y + i * 2, Kind::Solar, Dir::E).unwrap();
    }
    for i in 0..=(n * 2) / 6 {
        w.place(0, x, y + i * 6 + 1, Kind::Pole, Dir::E).unwrap();
    }
}

fn drill_crates(w: &mut World, x: i32, y: i32, n: i32) {
    for i in 0..n {
        w.place(0, x, y + i, Kind::Crate, Dir::E).unwrap();
    }
}

fn crates_total(w: &World, x: i32, y: i32, n: i32) -> usize {
    (0..n).map(|i| crate_items(w, 0, x, y + i).len()).sum()
}

#[test]
fn an_advanced_drill_is_electric_and_twice_as_fast_as_a_basic_drill() {
    let mut w = World::blank();
    assert_eq!((Kind::AdvancedDrill.base_dims(), Kind::AdvancedDrill.cost(), Kind::AdvancedDrill.power_draw()), ((4, 4), 240.0, 4.0));
    // A basic drill with coal on hand and an advanced one with none, both on iron.
    w.place(0, 5, 5, Kind::BasicDrill, Dir::E).unwrap();
    w.apply_setting(0, 5, 5, Setting::Mines(Item::IronOre));
    w.place(0, 7, 5, Kind::Crate, Dir::E).unwrap();
    if let Some(Building { st: St::Drill { coal, .. }, .. }) = w.floors[0].tiles[idx(5, 5)].as_mut() {
        *coal = 10;
    }
    w.place(0, 20, 5, Kind::AdvancedDrill, Dir::E).unwrap(); // covers 20-23, 5-8; front is x=24
    w.apply_setting(0, 20, 5, Setting::Mines(Item::IronOre));
    w.place(0, 24, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 20, 9, Kind::Solar, Dir::E).unwrap(); // reaches its bottom rows
    w.tick();
    assert_eq!(w.idle_reason(0, idx(20, 5)), None, "no coal needed");
    run(&mut w, 10);
    let (basic, adv) = (crate_items(&w, 0, 7, 5), crate_items(&w, 0, 24, 5));
    assert!(adv.iter().all(|i| *i == Item::IronOre));
    assert_eq!((basic.len(), adv.len()), (15, 30), "iron at 40 ticks an ore, and at 20");
    assert!(w.econ.used[Item::Coal.idx()] <= 2, "only the basic drill burned coal");
}

#[test]
fn an_advanced_drill_without_power_waits_and_slows_with_the_grid() {
    let mut w = World::blank();
    w.place(0, 20, 5, Kind::AdvancedDrill, Dir::E).unwrap();
    w.place(0, 24, 5, Kind::Crate, Dir::E).unwrap();
    run(&mut w, 5);
    assert!(crate_items(&w, 0, 24, 5).is_empty());
    assert_eq!(w.idle_reason(0, idx(20, 5)), Some("EC?"));
    // On a grid that's too small, it slows with everything else.
    w.place(0, 20, 9, Kind::Solar, Dir::E).unwrap();
    for (x, y) in footprint(18, 9, 6, 6) {
        if w.check_place(0, x, y, Kind::Belt).is_ok() {
            w.place(0, x, y, Kind::Belt, Dir::E).unwrap();
        }
    }
    let ratio = w.ratio_at(0, 20, 7);
    assert!(ratio < 1.0 && ratio > 0.0, "{ratio}");
    run(&mut w, 10);
    let n = crate_items(&w, 0, 24, 5).len() as f64;
    assert!((n - 30.0 * ratio).abs() <= 1.5, "{n} items at {ratio}");
}

#[test]
fn a_large_basic_drill_mines_5x_and_burns_10x_the_coal() {
    let mut w = World::blank();
    assert_eq!((Kind::LargeBasicDrill.base_dims(), Kind::LargeBasicDrill.cost()), ((8, 8), 1000.0));
    // A basic drill and a large one, each fed all the coal they can hold.
    w.place(0, 5, 5, Kind::BasicDrill, Dir::E).unwrap();
    w.place(0, 7, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 5, 20, Kind::LargeBasicDrill, Dir::E).unwrap(); // covers 5-12, 20-27; front is x=13
    drill_crates(&mut w, 13, 20, 8);
    for b in w.floors[0].tiles.iter_mut().flatten() {
        if let St::Drill { coal, .. } = &mut b.st {
            *coal = b.kind.drill_coal_cap();
        }
    }
    assert_eq!(Kind::LargeBasicDrill.drill_coal_cap(), 100);
    run(&mut w, 20);
    let basic = crate_items(&w, 0, 7, 5).len();
    let large = crates_total(&w, 13, 20, 8);
    assert_eq!(basic, 30, "1200 ticks at 40");
    assert_eq!(large, 5 * basic, "5 times the ore: 7.5 a second, one belt");
    // 10 times the coal: the basic drill burns 1 unit a tick (3 coal in 1200 ticks, one of
    // them the starter), the big one 10 a tick (30).
    let used = w.econ.used[Item::Coal.idx()] as f64;
    assert!((used - 33.0).abs() <= 3.0, "coal used {used}");
    // It pushes into everything along its front edge, in turn.
    for i in 0..8 {
        assert!(crate_items(&w, 0, 13, 20 + i).len() >= 15, "crate {i}");
    }
}

#[test]
fn a_large_basic_drill_runs_dry_without_coal() {
    let mut w = World::blank();
    w.place(0, 5, 20, Kind::LargeBasicDrill, Dir::E).unwrap();
    drill_crates(&mut w, 13, 20, 8);
    run(&mut w, 2);
    // Only the starter fuel: 400 units is a bit over 12 ticks of work.
    let n = crates_total(&w, 13, 20, 8);
    assert!(n > 0 && n <= 6, "{n}");
    assert_eq!(w.idle_reason(0, idx(5, 20)), Some("COAL"));
}

#[test]
fn a_large_advanced_drill_mines_5x_an_advanced_drill() {
    let mut w = World::blank();
    assert_eq!(
        (Kind::LargeAdvancedDrill.base_dims(), Kind::LargeAdvancedDrill.cost(), Kind::LargeAdvancedDrill.power_draw()),
        ((8, 8), 2000.0, 40.0)
    );
    // 130 EC of panels for the big one, 10 for the advanced one.
    big_grid(&mut w, 32, 0, 13);
    w.place(0, 34, 6, Kind::LargeAdvancedDrill, Dir::E).unwrap(); // covers 34-41, 6-13; front is x=42
    drill_crates(&mut w, 42, 6, 8);
    w.place(0, 5, 40, Kind::AdvancedDrill, Dir::E).unwrap();
    w.place(0, 9, 40, Kind::Crate, Dir::E).unwrap();
    w.place(0, 5, 44, Kind::Solar, Dir::E).unwrap();
    assert_eq!(w.ratio_at(0, 34, 6), 1.0);
    run(&mut w, 10);
    let adv = crate_items(&w, 0, 9, 40).len();
    let large = crates_total(&w, 42, 6, 8);
    assert_eq!(adv, 30);
    assert_eq!(large, 5 * adv, "5 times an advanced drill: 15 a second, one fast belt");
    assert_eq!(w.econ.used[Item::Coal.idx()], 0, "no coal anywhere");
}

#[test]
fn an_advanced_pump_gives_four_times_the_water() {
    let mut w = World::blank();
    assert_eq!((Kind::AdvancedPump.base_dims(), Kind::AdvancedPump.cost(), Kind::AdvancedPump.power_draw()), ((2, 2), 240.0, 4.0));
    w.place(0, 5, 5, Kind::Pump, Dir::E).unwrap();
    w.place(0, 5, 20, Kind::AdvancedPump, Dir::E).unwrap();
    // Long pipes so neither network fills up.
    for x in 6..=45 {
        w.place(0, x, 5, Kind::Pipe, Dir::E).unwrap();
    }
    for x in 7..=46 {
        w.place(0, x, 20, Kind::Pipe, Dir::E).unwrap();
    }
    w.place(0, 5, 8, Kind::Solar, Dir::E).unwrap();
    w.place(0, 5, 23, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 5);
    let water = |w: &World, x: i32, y: i32| {
        let p = w.fluids.pipe_pool[0][idx(x, y)];
        w.fluids.pools[p as usize].amount
    };
    let (small, big) = (water(&w, 6, 5), water(&w, 7, 20));
    assert!((small - 100.0).abs() < 1e-6, "{small}");
    assert!((big - 400.0).abs() < 1e-6, "{big}");
}

#[test]
fn new_machines_say_whether_they_run_on_ec() {
    for (k, electric) in
        [(Kind::Welder, true), (Kind::Kiln, false), (Kind::Mixer, true), (Kind::LargeBasicDrill, false), (Kind::LargeAdvancedDrill, true)]
    {
        assert_eq!(k.electric(), electric, "{k:?}");
    }
    assert_eq!(Kind::Kiln.cost(), 200.0);
    assert_eq!((Kind::Mixer.cost(), Kind::Mixer.power_draw(), Kind::Mixer.base_dims()), (200.0, 4.0, (2, 2)));
    assert_eq!((Kind::Welder.cost(), Kind::Welder.base_dims()), (800.0, (3, 3)));
}

#[test]
fn idle_buildings_say_why() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Drill, Dir::E).unwrap(); // nothing at its front
    run(&mut w, 5);
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FULL"));
    w.place(0, 10, 10, Kind::Furnace, Dir::E).unwrap();
    w.tick();
    assert_eq!(w.idle_reason(0, idx(10, 10)), Some("NEED"));
}

#[test]
fn drill_behind_unpowered_belt_says_pwr() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Drill, Dir::E).unwrap();
    for x in 7..12 {
        w.place(0, x, 5, Kind::Belt, Dir::E).unwrap();
    }
    run(&mut w, 10);
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("EC?"));
    w.place(0, 8, 7, Kind::Solar, Dir::E).unwrap();
    w.tick();
    assert_ne!(w.idle_reason(0, idx(5, 5)), Some("EC?"));
}

#[test]
fn offline_earnings_have_no_cap() {
    let mut w = World::new();
    // Selling one copper plate a second, all of it for research.
    let plate = Item::CopperPlate;
    w.rate = (2.0, w.econ.next_research(plate));
    w.sale_rate = PerItem::default();
    w.sale_rate[plate.idx()] = 1.0;
    w.saved_at = 1_000;
    let (away, m, r) = w.apply_offline(1_000 + 10 * 3600).unwrap();
    assert_eq!(away, 36_000);
    assert_eq!(m, 72_000.0);
    assert_eq!(w.econ.money, START_MONEY + 72_000.0);
    // Research fades just as it would in play: the same as 36,000 sales one by one.
    let by_hand: f64 = (0..36_000).map(|n| plate.curve().value(n)).sum();
    assert!((r - by_hand).abs() < 1.0, "{r} vs {by_hand}");
    assert!(r < 36_000.0 * 12.0 * 0.1, "fades: {r}");
    assert_eq!(w.econ.research_sales[plate.idx()], 36_000);
}

#[test]
fn deliveries_and_short_sessions_leave_the_offline_rate_alone() {
    // Earning $1 a second for two minutes.
    let mut w = World::new();
    for _ in 0..120 {
        w.econ.earned_money += 1.0;
        w.sample_earnings();
    }
    assert!((w.rate.0 - 1.0).abs() < 1e-9, "{:?}", w.rate);
    // A company delivery is not earnings the factory keeps making while closed.
    w.receive(crate::mail::parse("done", 0), 1_000);
    assert!(w.gifted > 0.0);
    w.econ.earned_money += 1.0;
    w.sample_earnings();
    assert!((w.rate.0 - 1.0).abs() < 1e-9, "{:?}", w.rate);

    // A fresh session (samples aren't saved) of a few fast seconds keeps the saved rate.
    let mut w2 = World::new();
    w2.rate = (1.0, 0.0);
    for _ in 0..5 {
        w2.econ.earned_money += 5.0;
        w2.sample_earnings();
    }
    assert_eq!(w2.rate.0, 1.0);
}

#[test]
fn offline_pay_uses_the_better_of_recent_and_long_rates() {
    let mut w = World::new();
    w.rate = (0.0, 0.0); // a jam right before quitting
    w.rate_long = (2.0, 0.0);
    w.saved_at = 1_000;
    let (_, m, _) = w.apply_offline(1_000 + 3600).unwrap();
    assert_eq!(m, 7_200.0);
}

#[test]
fn agent_mail_brings_spaced_deliveries() {
    let mut w = World::new();
    let money = w.econ.money;
    let msg = || crate::mail::parse("done", 0);
    w.receive(msg(), 1_000);
    w.receive(msg(), 1_010);
    w.receive(msg(), 1_000 + GIFT_GAP);
    assert_eq!(w.mail.len(), 3);
    let gifts: Vec<bool> = w.mail.iter().map(|m| m.gift.is_some()).collect();
    assert_eq!(gifts, [true, false, true]);
    assert_eq!(w.econ.money, money + 50.0);
}

#[test]
fn a_new_world_welcomes_the_player_with_one_unread_telegram() {
    let mut w = World::new();
    assert!(w.mail.is_empty() && w.log.is_empty(), "World::new alone sends nothing (the demo and tests use it)");
    w.welcome(1_000);
    assert_eq!((w.mail.len(), w.unread()), (1, 1));
    let m = &w.mail[0];
    assert_eq!(m.from, "Hollowmere & Brannock");
    assert!(m.gift.is_none() && m.md.contains("**1** builds") && m.md.contains("**f** picks money or research"), "{}", m.md);
    // Not an agent telegram: no delivery, and the next agent telegram can still bring one.
    assert_eq!(w.econ.money, START_MONEY);
    assert_eq!(w.last_gift, 0);
}

#[test]
fn every_telegram_is_logged_until_read() {
    let mut w = World::new();
    for i in 0..3 {
        w.receive(crate::mail::parse(&format!("from: a\nmsg {i}"), 0), 1_000);
    }
    assert_eq!(w.unread(), 3);
    let ids: Vec<u64> = w.log.iter().map(|e| e.msg.id).collect();
    assert_eq!(ids, [1, 2, 3]);
    w.mark_read(2);
    assert_eq!(w.unread(), 2);
    assert!(w.log[1].read && !w.log[0].read);
    // Taking telegrams off the waiting queue doesn't touch the log.
    w.mail.clear();
    assert_eq!(w.log.len(), 3);
}

#[test]
fn the_log_keeps_the_last_fifty_and_drops_read_ones_first() {
    let mut w = World::new();
    for _ in 0..LOG_KEEP {
        w.receive(crate::mail::parse("hi", 0), 1_000);
    }
    assert_eq!((w.log.len(), w.mail.len()), (LOG_KEEP, MAIL_KEEP));
    // Read the 10th; when a new one arrives it goes, not the oldest unread.
    w.mark_read(10);
    w.receive(crate::mail::parse("new", 0), 1_000);
    assert_eq!(w.log.len(), LOG_KEEP);
    assert!(w.log.iter().all(|e| e.msg.id != 10) && w.log.front().unwrap().msg.id == 1);
    // With none read, the oldest goes.
    w.receive(crate::mail::parse("newer", 0), 1_000);
    assert_eq!(w.log.front().unwrap().msg.id, 2);
    assert_eq!(w.log.back().unwrap().msg.md, "newer");
}

#[test]
fn the_log_survives_save_and_load() {
    let mut w = World::new();
    w.receive(crate::mail::parse("from: a\nfirst", 0), 1_000);
    w.receive(crate::mail::parse("from: b\nsecond", 0), 1_000);
    w.mark_read(1);
    let path = std::env::temp_dir().join(format!("factory-test-log-{}.json", std::process::id()));
    w.save(&path).unwrap();
    let loaded = World::load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let log: Vec<(u64, bool, &str)> = loaded.log.iter().map(|e| (e.msg.id, e.read, e.msg.md.as_str())).collect();
    assert_eq!(log, [(1, true, "first"), (2, false, "second")]);
    // New messages carry on from the saved count.
    let mut loaded = loaded;
    loaded.receive(crate::mail::parse("third", 0), 1_000);
    assert_eq!(loaded.log.back().unwrap().msg.id, 3);
}

#[test]
fn save_and_load_round_trip() {
    let mut w = World::new();
    w.build_demo().unwrap();
    run(&mut w, 20);
    let path = std::env::temp_dir().join(format!("factory-test-{}.json", std::process::id()));
    w.save(&path).unwrap();
    let mut l = World::load(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert_eq!(l.tick, w.tick);
    assert_eq!(l.econ.money, w.econ.money);
    assert_eq!(l.tile(0, 54, 38).map(|b| b.kind), Some(Kind::Furnace));
    // Both keep running identically after loading.
    run(&mut w, 10);
    run(&mut l, 10);
    assert_eq!(l.econ.sold, w.econ.sold);
}

#[test]
fn saves_round_trip_with_the_full_item_list() {
    let mut w = World::blank();
    let last = Item::ALL[ITEMS - 1];
    assert_eq!(last, Item::CarbonBlack);
    w.econ.made[last.idx()] = 7;
    w.econ.sold[last.idx()] = 5;
    w.econ.research_sales[last.idx()] = 3;
    w.econ.made[Item::Gauge.idx()] = 11;
    w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 8, 5, Kind::LargeAdvancedAssembler, Dir::E).unwrap();
    set_stock(&mut w, 8, 5, last, 4);
    set_stock(&mut w, 8, 5, Item::ControlPanel, 2);
    fill_crate(&mut w, 0, 5, 5, &[last, Item::PumpUnit]);
    w.progress.researched[Tech::ALL[TECHS - 1].idx()] = true;
    w.progress.opened[Company::ALL[COMPANIES - 1].idx()] = true;
    let path = std::env::temp_dir().join(format!("factory-test-items-{}.json", std::process::id()));
    w.save(&path).unwrap();
    // The file names each item that has a count, and leaves the zeros out.
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(json["econ"]["made"], serde_json::json!({"CarbonBlack": 7, "Gauge": 11}));
    assert_eq!(json["version"], SAVE_VERSION);
    let l = World::load(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert_eq!((l.econ.made[last.idx()], l.econ.sold[last.idx()], l.econ.research_sales[last.idx()]), (7, 5, 3));
    assert_eq!(l.econ.made[Item::Gauge.idx()], 11);
    match &l.tile(0, 8, 5).unwrap().st {
        St::Crafter { stock, .. } => {
            assert_eq!((stock[last.idx()], stock[Item::ControlPanel.idx()]), (4, 2))
        }
        _ => panic!("not a machine"),
    }
    assert_eq!(crate_items(&l, 0, 5, 5), vec![last, Item::PumpUnit]);
    assert!(l.progress.has(Tech::ALL[TECHS - 1]) && l.progress.is_open(Company::ALL[COMPANIES - 1]));
}

// ---------- the iron and coal step ----------

/// Picks a machine's recipe by name (cycling with f).
fn cycle_to(w: &mut World, x: i32, y: i32, name: &str) {
    for _ in 0..30 {
        if w.tile(0, x, y).and_then(|b| b.recipe()).is_some_and(|r| r.name == name) {
            return;
        }
        w.configure(0, x, y);
    }
    panic!("no recipe called {name}");
}

/// A machine facing east with a recipe, its inputs in stock, a panel beside it, a crate at
/// its front (products) and one at its back (waste). Runs it and returns what landed in the
/// two crates, front first.
fn run_machine(kind: Kind, recipe: &str, stock: &[(Item, u32)], secs: u64) -> Vec<Item> {
    let mut w = World::blank();
    let (x, y) = (10, 10);
    w.place(0, x, y, kind, Dir::E).unwrap();
    let (mw, mh) = kind.dims(Dir::E);
    cycle_to(&mut w, x, y, recipe);
    for (item, n) in stock {
        set_stock(&mut w, x, y, *item, *n);
    }
    w.place(0, x + mw, y, Kind::Crate, Dir::E).unwrap();
    w.place(0, x - 1, y, Kind::Crate, Dir::W).unwrap();
    w.place(0, x, y + mh, Kind::Solar, Dir::E).unwrap();
    run(&mut w, secs);
    let mut out = crate_items(&w, 0, x + mw, y);
    out.extend(crate_items(&w, 0, x - 1, y));
    out
}

fn count(items: &[Item], it: Item) -> usize {
    items.iter().filter(|i| **i == it).count()
}

#[test]
fn new_items_have_the_asked_prices_and_curves() {
    let prices = [
        (Item::CopperGrit, 0.6),
        (Item::IronGrit, 0.6),
        (Item::Sand, 0.25),
        (Item::FineWire, 2.5),
        (Item::CopperSheet, 14.0),
        (Item::IronRod, 5.0),
        (Item::Screw, 2.0),
        (Item::Spring, 15.0),
        (Item::IronSheet, 18.0),
        (Item::CopperWire, 3.0),
        (Item::CopperPipe, 12.0),
        (Item::CopperCoil, 18.0),
        (Item::IronGear, 18.0),
        (Item::Coke, 2.5),
    ];
    for (item, price) in prices {
        assert_eq!(item.price(), price, "{item:?}");
    }
    for item in &Item::ALL[19..] {
        let worth = item.curve().worth;
        assert!(worth >= 10.0 && worth >= 2.0 * item.price(), "{item:?} worth {worth}");
        assert!(worth <= 2.0 * item.price().max(5.0), "{item:?} worth {worth} is too generous");
    }
}

#[test]
fn arms_are_cheap_to_power_toys() {
    // An arm used to draw a whole solar panel (10 EC). Layout toys are cheap to play with.
    assert_eq!((Kind::Arm.power_draw(), Kind::LongArm.power_draw()), (2.0, 4.0));
    assert!(Kind::Arm.power_draw() < Kind::Press.power_draw() + 1.0);
}

#[test]
fn new_machines_have_the_asked_size_power_and_cost() {
    let table = [
        (Kind::CokeOven, (2, 2), 0.0, 150.0),
        (Kind::Crusher, (2, 2), 6.0, 250.0),
        (Kind::WireDrawer, (2, 1), 4.0, 250.0),
        (Kind::RollingMill, (2, 2), 8.0, 400.0),
        (Kind::Lathe, (3, 3), 4.0, 200.0),
        (Kind::Winder, (2, 1), 2.0, 200.0),
    ];
    for (k, dims, power, cost) in table {
        assert_eq!((k.base_dims(), k.power_draw(), k.cost()), (dims, power, cost), "{k:?}");
        assert_eq!(k.category(), Category::Create);
    }
    // A long one turns with its facing.
    assert_eq!(Kind::WireDrawer.dims(Dir::N), (1, 2));
    assert_eq!(Kind::Lathe.dims(Dir::N), (3, 3));
}

#[test]
fn crusher_grinds_ore_and_stone() {
    let out = run_machine(Kind::Crusher, "copper grit", &[(Item::CopperOre, 3)], 13);
    assert_eq!(out, vec![Item::CopperGrit; 6]);
    let out = run_machine(Kind::Crusher, "iron grit", &[(Item::IronOre, 2)], 9);
    assert_eq!(out, vec![Item::IronGrit; 4]);
    let out = run_machine(Kind::Crusher, "sand", &[(Item::Stone, 3)], 8);
    assert_eq!(out, vec![Item::Sand; 6]);
    // No EC, no grit.
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Crusher, Dir::E).unwrap();
    set_stock(&mut w, 10, 10, Item::CopperOre, 3);
    w.place(0, 12, 10, Kind::Crate, Dir::E).unwrap();
    run(&mut w, 10);
    assert!(crate_items(&w, 0, 12, 10).is_empty());
}

/// Ore in, ingots out: (crusher ->) furnace -> molder -> crate, with coal and a panel.
fn ingots_from(ore: u32, crush: bool) -> usize {
    let mut w = World::blank();
    let mut x = 10;
    if crush {
        w.place(0, x, 10, Kind::Crusher, Dir::E).unwrap(); // covers 10-11, 10-11
        cycle_to(&mut w, x, 10, "copper grit");
        set_stock(&mut w, x, 10, Item::CopperOre, ore);
        x += 2;
    }
    w.place(0, x, 10, Kind::Furnace, Dir::E).unwrap(); // covers x..x+2, 10-12
    cycle_to(&mut w, x, 10, if crush { "molten copper (grit)" } else { "molten copper" });
    if !crush {
        set_stock(&mut w, x, 10, Item::CopperOre, ore);
    }
    set_stock(&mut w, x, 10, Item::Coal, 20);
    w.place(0, x + 3, 10, Kind::Molder, Dir::E).unwrap();
    w.place(0, x + 5, 10, Kind::Crate, Dir::E).unwrap();
    w.place(0, x, 13, Kind::Solar, Dir::E).unwrap(); // reaches the crusher and the molder
    run(&mut w, 90);
    count(&crate_items(&w, 0, x + 5, 10), Item::CopperIngot)
}

#[test]
fn crushed_copper_gives_a_third_more_ingots() {
    assert_eq!(ingots_from(6, false), 6);
    assert_eq!(ingots_from(6, true), 8);
}

#[test]
fn furnace_and_blast_furnace_melt_grit() {
    // 2 iron grit make 1 molten iron in the furnace.
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
    cycle_to(&mut w, 5, 5, "molten iron (grit)");
    set_stock(&mut w, 5, 5, Item::IronGrit, 6);
    set_stock(&mut w, 5, 5, Item::Coal, 10);
    w.place(0, 8, 5, Kind::Molder, Dir::E).unwrap();
    cycle_to(&mut w, 8, 5, "iron ingot");
    w.place(0, 10, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 8, 7, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 40);
    assert_eq!(crate_items(&w, 0, 10, 5), vec![Item::IronIngot; 3]);
    // The blast furnace gets 2 molten iron from 3 grit.
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::BlastFurnace, Dir::E).unwrap(); // covers 5-6, 5-6
    cycle_to(&mut w, 5, 5, "molten iron (grit)");
    set_stock(&mut w, 5, 5, Item::IronGrit, 6);
    set_stock(&mut w, 5, 5, Item::Coke, 4);
    w.place(0, 7, 5, Kind::Molder, Dir::E).unwrap();
    cycle_to(&mut w, 7, 5, "iron ingot");
    w.place(0, 9, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 30);
    assert_eq!(crate_items(&w, 0, 9, 5), vec![Item::IronIngot; 4]);
}

#[test]
fn coke_oven_makes_coke_and_waste_with_no_fuel_or_ec() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::CokeOven, Dir::E).unwrap();
    set_stock(&mut w, 10, 10, Item::Coal, 6);
    w.place(0, 12, 10, Kind::Crate, Dir::E).unwrap(); // the coke, out the front
    w.place(0, 9, 10, Kind::Crate, Dir::W).unwrap(); // the waste, out the back
    run(&mut w, 7); // 2 s a batch
    let out = crate_items(&w, 0, 12, 10);
    let waste = crate_items(&w, 0, 9, 10);
    assert_eq!((count(&out, Item::Coke), count(&out, Item::Waste)), (3, 0));
    assert_eq!((count(&waste, Item::Coke), count(&waste, Item::Waste)), (0, 3));
    assert_eq!(w.econ.used[Item::Coal.idx()], 6);
    assert_eq!(Kind::CokeOven.recipes().len(), 1);
}

#[test]
fn the_furnace_cooks_coke_one_slow_clean_way() {
    // The fast and dirty recipe is gone: it lost to a second furnace, or to a coke oven.
    assert!(!recipe_names(Kind::Furnace).contains(&"coke (fast)"));
    // 3 coal, 1 coke, no waste.
    let out = run_machine(Kind::Furnace, "coke", &[(Item::Coal, 6)], 11);
    assert_eq!((count(&out, Item::Coke), count(&out, Item::Waste)), (2, 0));
}

#[test]
fn wire_drawer_draws_three_wire_per_plate_and_fine_wire() {
    let out = run_machine(Kind::WireDrawer, "copper wire", &[(Item::CopperPlate, 2)], 9);
    assert_eq!(out, vec![Item::CopperWire; 6]);
    let out = run_machine(Kind::WireDrawer, "fine wire", &[(Item::CopperWire, 2)], 9);
    assert_eq!(out, vec![Item::FineWire; 4]);
}

#[test]
fn press_wire_pipe_coil_and_gear_times() {
    let out = run_machine(Kind::Press, "copper wire", &[(Item::CopperPlate, 1)], 6);
    assert_eq!(out, vec![Item::CopperWire; 2]);
    assert!(run_machine(Kind::Press, "copper wire", &[(Item::CopperPlate, 1)], 4).is_empty(), "5 s a batch");
    assert_eq!(run_machine(Kind::Press, "copper pipe", &[(Item::CopperPlate, 4)], 6), vec![Item::CopperPipe; 2]);
    assert_eq!(run_machine(Kind::Press, "iron gear", &[(Item::IronPlate, 4)], 6), vec![Item::IronGear; 2]);
    assert_eq!(run_machine(Kind::Press, "copper coil", &[(Item::CopperWire, 4)], 6), vec![Item::CopperCoil]);
    // The fine-wire coil moved to the winder.
    assert!(!recipe_names(Kind::Press).contains(&"copper coil (fine wire)"));
}

#[test]
fn rolling_mill_rolls_sheets() {
    assert_eq!(run_machine(Kind::RollingMill, "copper sheet", &[(Item::CopperPlate, 4)], 11), vec![Item::CopperSheet; 2]);
    assert_eq!(run_machine(Kind::RollingMill, "iron sheet", &[(Item::IronPlate, 4)], 6), vec![Item::IronSheet; 2]);
    // Steel: a sheet in 5 s, and 3 steel into 2 girders in 4 s.
    assert_eq!(run_machine(Kind::RollingMill, "steel sheet", &[(Item::Steel, 2)], 11), vec![Item::SteelSheet; 2]);
    assert!(run_machine(Kind::RollingMill, "steel sheet", &[(Item::Steel, 1)], 4).is_empty());
    assert_eq!(run_machine(Kind::RollingMill, "steel girder", &[(Item::Steel, 3)], 5), vec![Item::SteelGirder; 2]);
    assert!(run_machine(Kind::RollingMill, "steel girder", &[(Item::Steel, 3)], 3).is_empty());
}

#[test]
fn the_press_makes_girders_and_hardened_gears_and_no_longer_gearboxes() {
    assert_eq!(run_machine(Kind::Press, "steel girder", &[(Item::Steel, 4)], 11), vec![Item::SteelGirder; 2]);
    assert!(run_machine(Kind::Press, "steel girder", &[(Item::Steel, 2)], 4).is_empty(), "5 s a girder");
    assert_eq!(run_machine(Kind::Press, "hardened gear", &[(Item::HardenedIronPlate, 2)], 6), vec![Item::HardenedGear]);
    assert!(!recipe_names(Kind::Press).contains(&"gearbox"), "the gearbox moved to the crude assembler");
    assert!(recipe_names(Kind::CrudeAssembler).contains(&"gearbox"));
}

#[test]
fn lathe_turns_rods_then_screws() {
    let rods = run_machine(Kind::Lathe, "iron rod", &[(Item::IronPlate, 2)], 11);
    assert_eq!(rods, vec![Item::IronRod; 4]);
    let screws = run_machine(Kind::Lathe, "screw", &[(Item::IronRod, 2)], 11);
    assert_eq!(screws, vec![Item::Screw; 8]);
}

#[test]
fn lathe_turns_steel_into_rods_then_bearings() {
    // 5 s for two steel rods; 2.5 s for two bearings from one rod.
    assert_eq!(run_machine(Kind::Lathe, "steel rod", &[(Item::Steel, 1)], 6), vec![Item::SteelRod; 2]);
    assert!(run_machine(Kind::Lathe, "steel rod", &[(Item::Steel, 1)], 4).is_empty());
    assert_eq!(run_machine(Kind::Lathe, "bearing", &[(Item::SteelRod, 2)], 6), vec![Item::Bearing; 4]);
    assert_eq!(run_machine(Kind::Lathe, "bearing", &[(Item::SteelRod, 1)], 3), vec![Item::Bearing; 2]);
}

#[test]
fn winder_winds_coils_and_springs() {
    assert_eq!(run_machine(Kind::Winder, "copper coil", &[(Item::CopperWire, 6)], 9), vec![Item::CopperCoil; 2]);
    assert_eq!(run_machine(Kind::Winder, "spring", &[(Item::IronRod, 4)], 11), vec![Item::Spring; 2]);
    // Fine wire goes further: 6 fine wire make 2 coils in 4 s, 3 fine wire a coil against 3 plain wire.
    let fine = "copper coil (fine wire)";
    assert_eq!(run_machine(Kind::Winder, fine, &[(Item::FineWire, 6)], 5), vec![Item::CopperCoil; 2]);
    assert!(run_machine(Kind::Winder, fine, &[(Item::FineWire, 6)], 3).is_empty());
    assert!(run_machine(Kind::Winder, fine, &[(Item::FineWire, 5)], 9).is_empty(), "needs all 6 fine wire");
}

#[test]
fn new_machines_are_locked_until_their_company_opens_and_they_are_researched() {
    let table = [
        (Kind::Crusher, Company::Ashgrove, Tech::Crusher, Item::CopperWire),
        (Kind::CokeOven, Company::Cinderhall, Tech::CokeOven, Item::Coke),
        (Kind::WireDrawer, Company::Kettering, Tech::WireDrawer, Item::CopperCoil),
        (Kind::BasicSolar, Company::Kettering, Tech::BasicSolar, Item::CopperCoil),
        (Kind::Winder, Company::Kettering, Tech::Winder, Item::CopperCoil),
        (Kind::Lathe, Company::Gearwright, Tech::Lathe, Item::IronGear),
        (Kind::RollingMill, Company::Pillar, Tech::RollingMill, Item::SteelGirder),
        (Kind::Welder, Company::Pillar, Tech::Welder, Item::SteelGirder),
        (Kind::Mixer, Company::Wellbourne, Tech::Mixer, Item::CopperPipe),
        (Kind::AdvancedPump, Company::Wellbourne, Tech::AdvancedPump, Item::CopperPipe),
        (Kind::Kiln, Company::Underhill, Tech::Kiln, Item::Concrete),
        (Kind::LargeKiln, Company::Underhill, Tech::LargeKiln, Item::Concrete),
        (Kind::BasicAssembler, Company::Swiftline, Tech::BasicAssembler, Item::Gearbox),
        (Kind::AdvancedDrill, Company::Ampleworth, Tech::AdvancedDrill, Item::ElectricMotor),
        (Kind::AdvancedAssembler, Company::LatticeFinch, Tech::AdvancedAssembler, Item::ControlBoard),
        (Kind::LargeBasicAssembler, Company::Ampleworth, Tech::LargeBasicAssembler, Item::ElectricMotor),
        (Kind::LargeBasicDrill, Company::Ampleworth, Tech::LargeBasicDrill, Item::ElectricMotor),
        (Kind::PrecisionAssembler, Company::LatticeFinch, Tech::PrecisionAssembler, Item::ControlBoard),
        (Kind::Etcher, Company::LatticeFinch, Tech::Etcher, Item::ControlBoard),
        (Kind::LargeAdvancedAssembler, Company::LatticeFinch, Tech::LargeAdvancedAssembler, Item::ControlBoard),
        (Kind::LargeAdvancedDrill, Company::LatticeFinch, Tech::LargeAdvancedDrill, Item::ControlBoard),
    ];
    for (kind, company, tech, key) in table {
        assert_eq!(
            (kind.company(), kind.tech(), tech.company(), company.key_items()),
            (Some(company), Some(tech), company, &[Made::Item(key)][..])
        );
        let mut w = World::new();
        assert!(!w.progress.part_unlocked(kind), "{kind:?} locked at the start");
        assert!(w.research(tech).unwrap_err().contains(key.name()), "{tech:?} needs {key:?} first");
        // Making the key item opens the company; the part still needs its tech.
        w.econ.made[key.idx()] = 1;
        w.tick();
        assert!(w.progress.is_open(company));
        assert!(!w.progress.part_unlocked(kind), "{kind:?} still needs research");
        assert!(w.check_place(0, 30, 30, kind).unwrap_err().contains("locked"));
        w.econ.research = tech.cost();
        w.research(tech).unwrap();
        assert!(w.progress.part_unlocked(kind));
        w.econ.money = 1e6;
        assert!(w.check_place(0, 30, 30, kind).is_ok(), "{kind:?}");
    }
    // Or the milestone does it for free.
    let mut w = World::new();
    let (_key, n) = Tech::Crusher.milestone();
    w.econ.made[Item::CopperWire.idx()] = 1;
    w.tick();
    assert!(!w.progress.has(Tech::Crusher));
    w.econ.made[Item::CopperOre.idx()] = n;
    w.tick();
    assert!(w.progress.has(Tech::Crusher));
}

#[test]
fn every_machine_can_be_built_once_unlocked() {
    // Nothing is hidden: every machine is owned by a company and a tech (or is a starter).
    let mut w = World::new();
    w.econ.money = 1e6;
    w.progress.opened = [true; COMPANIES];
    w.progress.researched = [true; TECHS];
    for k in Kind::ALL {
        assert!(w.progress.part_unlocked(k), "{k:?}");
        assert!(w.check_place(0, 30, 30, k).is_ok(), "{k:?}");
    }
    // A new game can build none of the twelve late machines.
    let fresh = World::new();
    for k in [
        Kind::BasicAssembler,
        Kind::AdvancedAssembler,
        Kind::PrecisionAssembler,
        Kind::Etcher,
        Kind::LargeBasicAssembler,
        Kind::LargeAdvancedAssembler,
        Kind::Welder,
        Kind::Kiln,
        Kind::LargeKiln,
        Kind::Mixer,
        Kind::AdvancedDrill,
        Kind::LargeBasicDrill,
        Kind::LargeAdvancedDrill,
        Kind::AdvancedPump,
    ] {
        assert!(!fresh.progress.part_unlocked(k), "{k:?}");
        assert!(k.tech().is_some() && k.company().is_some());
    }
    assert_eq!(Kind::CrudeAssembler.tech(), Some(Tech::Assembler));
}

#[test]
fn new_techs_are_priced_by_depth() {
    let copper = [(Tech::Crusher, 150.0), (Tech::Winder, 200.0), (Tech::WireDrawer, 250.0)];
    for (t, cost) in copper {
        assert_eq!(t.cost(), cost);
        assert!((50.0..=300.0).contains(&t.cost()), "{t:?}");
    }
    for t in [Tech::CokeOven, Tech::Lathe, Tech::RollingMill] {
        assert!((300.0..=1500.0).contains(&t.cost()), "{t:?}");
    }
    // Steel and stone machines, then motor machines, then processor machines.
    for t in [Tech::Mixer, Tech::Kiln, Tech::AdvancedPump, Tech::Welder, Tech::BasicAssembler] {
        assert!((1500.0..=6000.0).contains(&t.cost()), "{t:?}");
    }
    // The large kiln sits at the start of the motor stage, beside the kiln's own company.
    assert!((6000.0..=25_000.0).contains(&Tech::LargeKiln.cost()) && Tech::LargeKiln.cost() > Tech::Kiln.cost());
    for t in [Tech::AdvancedDrill, Tech::LargeBasicDrill, Tech::LargeBasicAssembler] {
        assert!((6000.0..=25_000.0).contains(&t.cost()), "{t:?}");
        assert_eq!(t.company(), Company::Ampleworth);
    }
    // The advanced assembler is the first thing Lattice & Finch sells: boards, not motors, open it.
    assert!((6000.0..=25_000.0).contains(&Tech::AdvancedAssembler.cost()));
    assert_eq!(
        (Tech::AdvancedAssembler.company(), Tech::AdvancedAssembler.milestone().0),
        (Company::LatticeFinch, Made::Item(Item::ControlBoard))
    );
    for t in [Tech::PrecisionAssembler, Tech::Etcher, Tech::LargeAdvancedDrill, Tech::LargeAdvancedAssembler] {
        assert!((25_000.0..=100_000.0).contains(&t.cost()), "{t:?}");
        assert_eq!(t.company(), Company::LatticeFinch);
    }
    // Every tech sits in the research list and in the tree once, and has a free milestone.
    for t in Tech::ALL {
        let n: usize = UNLOCK_GROUPS.iter().map(|g| g.1.iter().filter(|x| **x == t).count()).sum();
        assert_eq!(n, 1, "{t:?}");
        assert!(t.milestone().1 > 0 && !t.about().is_empty());
    }
    for t in Tech::ALL.into_iter().filter(|t| {
        !matches!(t, Tech::Tunnels | Tech::LongArms | Tech::FilterSplitters | Tech::Warehouses | Tech::Blueprints | Tech::UpgradePlanner)
    }) {
        assert!(TREE.iter().any(|(node, _)| *node == Node::Tech(t)), "{t:?} is missing from the tree");
    }
    // The tree's links point backwards, and every company's key item is a node.
    for (i, (_, parent)) in TREE.iter().enumerate() {
        assert!(parent.map_or(i == 0, |p| p < i), "node {i}");
    }
    for c in Company::ALL {
        for key in c.key_items() {
            assert!(TREE.iter().any(|(n, _)| *n == Node::Key(*key)), "{c:?}'s {key:?} is not in the tree");
        }
    }
}

/// Which items can be made once these companies are open, taking every tech of an open
/// company as researchable (techs can always be bought or earned). Pumps give water.
fn makeable(open: &[bool; COMPANIES]) -> ([bool; ITEMS], Vec<Fluid>) {
    let tech_ok = |t: Tech| open[t.company().idx()];
    let kind_ok = |k: Kind| k.company().is_none_or(|c| open[c.idx()]) && k.tech().is_none_or(tech_ok);
    let mut have = [false; ITEMS];
    let mut fluids: Vec<Fluid> = Vec::new();
    loop {
        let before = (have, fluids.len());
        for k in Kind::ALL.into_iter().filter(|k| kind_ok(*k)) {
            for ore in k.ores() {
                have[ore.idx()] = true;
            }
            if k.is_pump() && !fluids.contains(&Fluid::Water) {
                fluids.push(Fluid::Water);
            }
            if k.is_extractor() && !fluids.contains(&Fluid::CrudeOil) {
                fluids.push(Fluid::CrudeOil);
            }
            for r in k.recipes().iter().filter(|r| r.needs.is_none_or(tech_ok)) {
                let ok = r.inputs.iter().all(|(i, _)| have[i.idx()])
                    && r.fuel.is_none_or(|f| have[f.idx()])
                    && r.fluid_in.is_none_or(|(f, _)| fluids.contains(&f));
                if ok {
                    for (i, _) in r.outputs {
                        have[i.idx()] = true;
                    }
                    if let Some((f, _)) = r.fluid_out.filter(|(f, _)| !fluids.contains(f)) {
                        fluids.push(f);
                    }
                }
            }
        }
        if (have, fluids.len()) == before {
            return (have, fluids);
        }
    }
}

/// Walks the key items: each company opens once its key item can be made with what the
/// companies opened so far offer. No chain may lock, and the order must make sense.
#[test]
fn every_key_item_can_be_made_before_its_company_opens() {
    let mut open = [false; COMPANIES];
    for c in Company::ALL {
        open[c.idx()] = c.key_items().is_empty();
    }
    let mut order: Vec<Company> = Vec::new();
    loop {
        let (have, fluids) = makeable(&open);
        let can = |k: &Made| match k {
            Made::Item(i) => have[i.idx()],
            Made::Fluid(f) => fluids.contains(f),
        };
        let next: Vec<Company> = Company::ALL.into_iter().filter(|c| !open[c.idx()] && c.key_items().iter().any(can)).collect();
        if next.is_empty() {
            break;
        }
        for c in next {
            open[c.idx()] = true;
            order.push(c);
        }
    }
    println!("companies open in this order: {order:?}");
    let stuck: Vec<Company> = Company::ALL.into_iter().filter(|c| !open[c.idx()]).collect();
    assert!(stuck.is_empty(), "these companies can never open: {stuck:?} (opened in order {order:?})");
    let at = |c: Company| order.iter().position(|x| *x == c).unwrap();
    // The press makes wire, pipe, coil and now the iron gear, and the furnace makes coke: all of
    // them with the two starter companies, no blast furnace.
    let mut starters = [false; COMPANIES];
    for c in Company::ALL {
        starters[c.idx()] = c.key_items().is_empty();
    }
    let (early, _) = makeable(&starters);
    for item in [Item::CopperWire, Item::CopperPipe, Item::CopperCoil, Item::Coke, Item::IronGear] {
        assert!(early[item.idx()], "{item:?} with only the starter companies");
    }
    // Girders and steel need the blast furnace's tech, so Cinderhall first.
    for c in [Company::Pillar, Company::Swiftline] {
        assert!(at(Company::Cinderhall) < at(c), "Cinderhall before {c:?}");
    }
    // Concrete needs the mixer (Wellbourne) and sand from the crusher (Ashgrove).
    assert!(at(Company::Wellbourne) < at(Company::Underhill) && at(Company::Ashgrove) < at(Company::Underhill));
    // A gearbox is built in the crude assembler (Gearwright), a motor in the basic one
    // (Swiftline). A control board is wired by hand in the crude assembler too, so Lattice & Finch
    // opens off the same road as Swiftline, not after the motors.
    assert!(at(Company::Gearwright) < at(Company::Swiftline));
    assert!(at(Company::Swiftline) < at(Company::Ampleworth));
    assert!(at(Company::Gearwright) < at(Company::LatticeFinch) && at(Company::Kettering) < at(Company::LatticeFinch));
    assert_eq!(Company::Gearwright.key_items(), &[Made::Item(Item::IronGear)]);
    assert_eq!(Company::Swiftline.key_items(), &[Made::Item(Item::Gearbox)]);
    assert_eq!(Company::Underhill.key_items(), &[Made::Item(Item::Concrete)]);
    assert_eq!(Company::Ampleworth.key_items(), &[Made::Item(Item::ElectricMotor)]);
    assert_eq!(Company::LatticeFinch.key_items(), &[Made::Item(Item::ControlBoard)]);
    // With everything open, every item has a way to be made.
    let (have, fluids) = makeable(&open);
    for it in Item::ALL {
        // Carbon black needs fuel oil, which only the refinery of a later phase makes.
        if it == Item::CarbonBlack {
            continue;
        }
        assert!(have[it.idx()], "{it:?} can't be made");
    }
    assert!(fluids.contains(&Fluid::MoltenIron) && fluids.contains(&Fluid::MoltenSteel) && fluids.contains(&Fluid::Water));
}

#[test]
fn a_company_never_gates_its_own_key_item() {
    // With every other company open, a key item is still makeable without its own company.
    for c in Company::ALL {
        let mut open = [true; COMPANIES];
        open[c.idx()] = false;
        let (have, fluids) = makeable(&open);
        for key in c.key_items() {
            let ok = match key {
                Made::Item(i) => have[i.idx()],
                Made::Fluid(f) => fluids.contains(f),
            };
            assert!(ok, "{key:?} needs {c:?}'s machines");
        }
    }
}

// ---------- the content step: steel, stone, parts and big goods ----------

/// Like `run_machine`, on a big grid (100 EC) so even the thirsty machines run at full speed.
fn run_powered(kind: Kind, recipe: &str, stock: &[(Item, u32)], secs: u64) -> Vec<Item> {
    let mut w = World::blank();
    // 10 EC a panel, and at least the usual 100 EC: the large precision assembler draws 160.
    big_grid(&mut w, 10, 0, ((kind.power_draw() / 10.0).ceil() as i32).max(10));
    let (x, y) = (12, 12);
    w.place(0, x, y, kind, Dir::E).unwrap();
    pick_recipe(&mut w, x, y, recipe);
    for (item, n) in stock {
        set_stock(&mut w, x, y, *item, *n);
    }
    w.place(0, x + kind.dims(Dir::E).0, y, Kind::Crate, Dir::E).unwrap();
    assert_eq!(w.ratio_at(0, x, y), 1.0, "{kind:?} is on the grid");
    run(&mut w, secs);
    crate_items(&w, 0, x + kind.dims(Dir::E).0, y)
}

/// A blast furnace set to `recipe`, with its stock; molten steel runs into a molder
/// (waste into a crate behind it). Returns (steel, waste) after `secs`.
fn steel_route(recipe: &str, stock: &[(Item, u32)], secs: u64) -> (usize, usize) {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::BlastFurnace, Dir::E).unwrap(); // covers 5-6, 5-6: front is x=7
    pick_recipe(&mut w, 5, 5, recipe);
    for (item, n) in stock {
        set_stock(&mut w, 5, 5, *item, *n);
    }
    w.place(0, 4, 5, Kind::Crate, Dir::W).unwrap(); // the waste, out the back
    w.place(0, 7, 6, Kind::Molder, Dir::E).unwrap(); // covers 7-8, 6-7, on the furnace's other front tile
    pick_recipe(&mut w, 7, 6, "steel");
    w.place(0, 9, 6, Kind::Crate, Dir::E).unwrap(); // the steel
    w.place(0, 7, 8, Kind::Solar, Dir::E).unwrap();
    run(&mut w, secs);
    (
        crate_items(&w, 0, 9, 6).iter().filter(|i| **i == Item::Steel).count(),
        crate_items(&w, 0, 4, 5).iter().filter(|i| **i == Item::Waste).count(),
    )
}

#[test]
fn the_blast_furnace_makes_steel_from_plate_three_ways_each_with_its_waste() {
    let plates = (Item::IronPlate, 2);
    // 2 iron plate + 1 coke: 10 s of melting, 2.5 s of casting, 1 waste.
    let route = [
        ("molten steel", vec![plates, (Item::Coke, 1)], 10, 1),
        ("molten steel (coal)", vec![plates, (Item::Coal, 2)], 10, 3),
        ("molten steel (fast)", vec![plates, (Item::Coke, 2)], 5, 2),
    ];
    for (name, stock, melt, waste) in route {
        let r = Kind::BlastFurnace.recipes().iter().find(|r| r.name == name).unwrap();
        assert_eq!(r.ticks as u64, melt * TPS, "{name}");
        assert_eq!(r.fluid_out, Some((Fluid::MoltenSteel, 1)), "{name}");
        assert_eq!(r.outputs, &[(Item::Waste, waste as u32)], "{name}");
        // Still melting just before the time is up, all done (cast, with its waste) a bit after.
        assert_eq!(steel_route(name, &stock, melt - 1), (0, 0), "{name} early");
        assert_eq!(steel_route(name, &stock, melt + 1), (0, waste), "{name} waste first, steel still casting");
        assert_eq!(steel_route(name, &stock, melt + 4), (1, waste), "{name}");
        // Two batches' worth of stock makes two.
        let double: Vec<(Item, u32)> = stock.iter().map(|(i, n)| (*i, n * 2)).collect();
        assert_eq!(steel_route(name, &double, 2 * melt + 4), (2, waste * 2), "{name} twice");
    }
    // Without the coke or coal, it waits.
    assert_eq!(steel_route("molten steel", &[plates], 30), (0, 0));
    assert_eq!(steel_route("molten steel (fast)", &[plates, (Item::Coke, 1)], 30), (0, 0), "the fast way needs 2 coke");
    // The fast coal route is gone: it lost to one more furnace, or to the oven.
    assert!(!recipe_names(Kind::BlastFurnace).contains(&"molten steel (fast, coal)"));
    // The old steel recipe is gone.
    assert!(!recipe_names(Kind::BlastFurnace).contains(&"steel"));
    // A molder casts 1 molten steel into 1 steel in 2.5 s.
    assert_eq!(recipe_ticks(Kind::Molder, "steel"), 150);
    assert_eq!(Item::Steel.price(), 24.0);
}

/// Direct steel takes molten iron in and puts molten steel out (the first recipe to do both): an
/// iron blast furnace right against a steel one feeds it with no pipe between them.
#[test]
fn direct_steel_turns_molten_iron_and_coke_into_molten_steel_with_no_pipe() {
    let r = Kind::BlastFurnace.recipes().iter().find(|r| r.name == "molten steel (direct)").unwrap();
    assert_eq!((r.fluid_in, r.fluid_out), (Some((Fluid::MoltenIron, 3)), Some((Fluid::MoltenSteel, 1))));
    assert_eq!((r.inputs, r.outputs, r.ticks as u64), (&[(Item::Coke, 1)][..], &[(Item::Waste, 2)][..], 10 * TPS));
    let mut w = World::blank();
    // The iron furnace (5-6, 5-6) faces east into the steel furnace (7-8, 5-6), which faces east into a molder.
    w.place(0, 5, 5, Kind::BlastFurnace, Dir::E).unwrap();
    pick_recipe(&mut w, 5, 5, "molten iron");
    w.place(0, 7, 5, Kind::BlastFurnace, Dir::E).unwrap();
    pick_recipe(&mut w, 7, 5, "molten steel (direct)");
    w.place(0, 9, 5, Kind::Molder, Dir::E).unwrap();
    pick_recipe(&mut w, 9, 5, "steel");
    w.place(0, 11, 5, Kind::Crate, Dir::E).unwrap();
    w.place(0, 9, 8, Kind::Solar, Dir::E).unwrap();
    set_stock(&mut w, 5, 5, Item::IronOre, 6);
    set_stock(&mut w, 5, 5, Item::Coke, 6);
    set_stock(&mut w, 7, 5, Item::Coke, 2);
    run(&mut w, 40);
    let steel = crate_items(&w, 0, 11, 5).iter().filter(|i| **i == Item::Steel).count();
    assert_eq!(steel, 2, "3 molten iron each, no pipe, no cast and re-melt");
    assert_eq!(w.econ.made[Item::Waste.idx()], 4, "2 waste a steel");
}

/// Molten metal a minute from a recipe.
fn molten_per_min(list: &[Recipe], name: &str) -> f64 {
    let r = list.iter().find(|r| r.name == name).unwrap();
    r.fluid_out.unwrap().1 as f64 * 60.0 * TPS as f64 / r.ticks as f64
}

#[test]
fn the_blast_furnace_melts_iron_twice_as_fast_as_a_furnace() {
    let (furnace, blast) = (molten_per_min(FURNACE_RECIPES, "molten iron"), molten_per_min(BLAST_RECIPES, "molten iron"));
    assert_eq!((furnace, blast), (12.0, 24.0));
    // Grit still melts at 48 a minute in the blast furnace, and better than ore in the furnace.
    assert_eq!(molten_per_min(BLAST_RECIPES, "molten iron (grit)"), 48.0);
    assert!(molten_per_min(FURNACE_RECIPES, "molten iron (grit)") < 48.0);
}

#[test]
fn the_blast_furnace_hardens_plate_with_coke_or_coal() {
    let out = run_machine(Kind::BlastFurnace, "hardened iron plate", &[(Item::IronPlate, 2), (Item::Coke, 2)], 11);
    assert_eq!(out, vec![Item::HardenedIronPlate; 2], "5 s each, no waste");
    assert!(run_machine(Kind::BlastFurnace, "hardened iron plate", &[(Item::IronPlate, 1), (Item::Coke, 1)], 4).is_empty());
    let out = run_machine(Kind::BlastFurnace, "hardened iron plate (coal)", &[(Item::IronPlate, 2), (Item::Coal, 4)], 11);
    assert_eq!((count(&out, Item::HardenedIronPlate), count(&out, Item::Waste)), (2, 4));
    let out = run_machine(Kind::BlastFurnace, "hardened iron plate (coal)", &[(Item::IronPlate, 1), (Item::Coal, 1)], 11);
    assert!(out.is_empty(), "2 coal each");
    assert_eq!(Item::HardenedIronPlate.price(), 14.0);
}

/// Every recipe that takes no fluid runs in its machine: a batch of inputs in, its outputs
/// out in the recipe's time (assemblers at every tier's speed), with the inputs used up.
/// Recipes that give fluid hold it at the front.
#[test]
fn every_item_recipe_runs_in_its_machine() {
    let mut checked = 0;
    for kind in Kind::ALL {
        for r in kind.recipes().iter().filter(|r| r.fluid_in.is_none()) {
            let batch = kind.batch();
            let mut w = World::blank();
            big_grid(&mut w, 10, 0, ((kind.power_draw() / 10.0).ceil() as i32).max(10));
            let (x, y) = (12, 12);
            w.place(0, x, y, kind, Dir::E).unwrap();
            pick_recipe(&mut w, x, y, r.name);
            for (it, n) in r.inputs {
                set_stock(&mut w, x, y, *it, n * batch);
            }
            if let Some(f) = r.fuel {
                // Enough to burn for the whole recipe time (a long firing outlasts the 10 a feed keeps in stock).
                set_stock(&mut w, x, y, f, (r.ticks * kind.fires() / f.burn_ticks().unwrap_or(1) + 2).max(10));
            }
            let front = x + kind.dims(Dir::E).0;
            w.place(0, front, y, Kind::Crate, Dir::E).unwrap();
            w.place(0, x - 1, y, Kind::Crate, Dir::W).unwrap(); // waste leaves by the back
                                                                // Just short of the recipe's time nothing has come out; shortly after, it all has.
            for _ in 0..r.ticks - 3 {
                w.tick();
            }
            assert!(crate_items(&w, 0, front, y).is_empty(), "{kind:?} {} early", r.name);
            assert!(crate_items(&w, 0, x - 1, y).is_empty(), "{kind:?} {} early", r.name);
            // A batch leaves one item a tick through the one crate beside its front edge.
            let per_run: u32 = r.outputs.iter().map(|o| o.1).sum();
            for _ in 0..13 + batch * per_run {
                w.tick();
            }
            let mut out = crate_items(&w, 0, front, y);
            out.extend(crate_items(&w, 0, x - 1, y));
            for (it, n) in r.outputs {
                assert_eq!(count(&out, *it) as u32, n * batch, "{kind:?} {}: {out:?}", r.name);
            }
            if let Some((_, n)) = r.fluid_out {
                assert_eq!(crafter_fluid(&w, x, y).1, n * batch, "{kind:?} {} holds its fluid", r.name);
            }
            for (it, n) in r.inputs {
                assert_eq!(w.econ.used[it.idx()] as u32, n * batch, "{kind:?} {} used {it:?}", r.name);
            }
            checked += 1;
        }
    }
    assert!(checked > 60, "{checked} recipes checked");
}

#[test]
fn welder_parts() {
    type Case<'a> = (&'a str, &'a [(Item, u32)], Item);
    let recipes: [Case; 3] = [
        ("boiler shell", &[(Item::SteelSheet, 1), (Item::Brick, 4)], Item::BoilerShell),
        ("silo shell", &[(Item::SteelSheet, 3), (Item::Valve, 1)], Item::SiloShell),
        ("frame", &[(Item::SteelGirder, 2), (Item::Screw, 8)], Item::Frame),
    ];
    for (name, stock, out) in recipes {
        assert_eq!(run_powered(Kind::Welder, name, stock, 11), vec![out], "{name}");
        assert!(run_powered(Kind::Welder, name, stock, 9).is_empty(), "{name} takes 10 s");
    }
}

#[test]
fn assemblers_build_their_parts_and_the_big_goods() {
    let t = |k: Kind, name: &str, stock: &[(Item, u32)], out: Item, secs: u64| {
        assert_eq!(run_powered(k, name, stock, secs), vec![out], "{k:?} {name}");
    };
    // Crude: 10 s each.
    t(Kind::CrudeAssembler, "valve", &[(Item::CopperPipe, 1), (Item::IronGear, 1)], Item::Valve, 11);
    t(Kind::CrudeAssembler, "gauge", &[(Item::Glass, 1), (Item::Spring, 1), (Item::CopperSheet, 1)], Item::Gauge, 11);
    t(Kind::CrudeAssembler, "lamp", &[(Item::Glass, 1), (Item::FineWire, 2)], Item::Lamp, 11);
    t(Kind::CrudeAssembler, "relay", &[(Item::CopperCoil, 1), (Item::Spring, 1), (Item::IronSheet, 1)], Item::Relay, 11);
    t(Kind::CrudeAssembler, "gearbox", &[(Item::IronGear, 3), (Item::Steel, 1)], Item::Gearbox, 11);
    // Basic: motors and pistons, 10 s; the crude parts at 5 s.
    t(Kind::BasicAssembler, "electric motor", &[(Item::CopperCoil, 2), (Item::SteelRod, 1), (Item::Bearing, 2)], Item::ElectricMotor, 11);
    t(Kind::BasicAssembler, "piston", &[(Item::SteelRod, 1), (Item::HardenedIronPlate, 1), (Item::Screw, 2)], Item::Piston, 11);
    t(Kind::BasicAssembler, "valve", &[(Item::CopperPipe, 1), (Item::IronGear, 1)], Item::Valve, 6);
    // Advanced: control boards, 10 s; motors at 5 s, crude parts at 2.5 s.
    t(Kind::AdvancedAssembler, "control board", &[(Item::Relay, 1), (Item::FineWire, 4), (Item::IronSheet, 1)], Item::ControlBoard, 11);
    t(Kind::AdvancedAssembler, "electric motor", &[(Item::CopperCoil, 2), (Item::SteelRod, 1), (Item::Bearing, 2)], Item::ElectricMotor, 6);
    t(Kind::AdvancedAssembler, "gearbox", &[(Item::IronGear, 3), (Item::Steel, 1)], Item::Gearbox, 3);
    // Large basic: 20 s.
    t(
        Kind::LargeBasicAssembler,
        "pump unit",
        &[(Item::ElectricMotor, 1), (Item::Valve, 2), (Item::SiloShell, 1), (Item::Gauge, 1)],
        Item::PumpUnit,
        21,
    );
    t(
        Kind::LargeBasicAssembler,
        "engine block",
        &[(Item::Frame, 1), (Item::Piston, 4), (Item::Gearbox, 1), (Item::BoilerShell, 1)],
        Item::EngineBlock,
        21,
    );
    // Large advanced: the same two at 10 s, then its own: 20 s, 20 s, 40 s and 60 s.
    t(
        Kind::LargeAdvancedAssembler,
        "engine block",
        &[(Item::Frame, 1), (Item::Piston, 4), (Item::Gearbox, 1), (Item::BoilerShell, 1)],
        Item::EngineBlock,
        11,
    );
    t(
        Kind::LargeAdvancedAssembler,
        "generator",
        &[(Item::Frame, 1), (Item::ElectricMotor, 2), (Item::Gauge, 1), (Item::ControlBoard, 1)],
        Item::Generator,
        21,
    );
    t(
        Kind::LargeAdvancedAssembler,
        "control panel",
        &[(Item::ControlBoard, 2), (Item::Lamp, 4), (Item::Gauge, 2), (Item::SteelSheet, 1)],
        Item::ControlPanel,
        21,
    );
    t(
        Kind::LargeAdvancedAssembler,
        "drilling rig",
        &[(Item::SteelGirder, 4), (Item::Gearbox, 2), (Item::ReinforcedConcrete, 4), (Item::ElectricMotor, 1), (Item::PumpUnit, 1)],
        Item::DrillingRig,
        41,
    );
    t(
        Kind::LargeAdvancedAssembler,
        "steam tractor",
        &[(Item::EngineBlock, 1), (Item::Frame, 1), (Item::HardenedGear, 4), (Item::ControlPanel, 1), (Item::Generator, 1)],
        Item::SteamTractor,
        61,
    );
    // Not before their time.
    assert!(run_powered(
        Kind::LargeAdvancedAssembler,
        "steam tractor",
        &[(Item::EngineBlock, 1), (Item::Frame, 1), (Item::HardenedGear, 4), (Item::ControlPanel, 1), (Item::Generator, 1)],
        59
    )
    .is_empty());
}

#[test]
fn every_item_but_waste_and_the_crown_items_is_used_in_a_recipe() {
    // Carbon black waits for the plastic machines of a later oil phase; it sells until then.
    let crown = [Item::DrillingRig, Item::SteamTractor, Item::Computer, Item::CarbonBlack];
    let mut used = [false; ITEMS];
    for k in Kind::ALL {
        for r in k.recipes() {
            for (i, _) in r.inputs {
                used[i.idx()] = true;
            }
        }
    }
    for it in Item::ALL {
        if it == Item::Waste || crown.contains(&it) {
            continue;
        }
        assert!(used[it.idx()], "{it:?} is an input to nothing");
    }
    for c in crown {
        assert!(!used[c.idx()], "{c:?} has no recipe use yet: only for selling and research");
    }
    assert!(!used[Item::Waste.idx()]);
}

#[test]
fn recipes_have_at_most_five_kinds_of_input_and_small_ones_three() {
    for k in Kind::ALL {
        for r in k.recipes() {
            let kinds = r.inputs.len() + r.fluid_in.is_some() as usize;
            assert!(kinds <= 5, "{k:?} {} has {kinds} kinds of input", r.name);
        }
    }
    for r in CRUDE_RECIPES.iter().chain(BASIC_RECIPES) {
        assert!(r.inputs.len() <= 3, "{} has {} kinds of input", r.name, r.inputs.len());
    }
}

/// Prints each made item's price against its inputs, and checks that every recipe's
/// outputs sell for more than what goes in (waste counts against them).
#[test]
fn each_step_sells_for_more_than_its_inputs() {
    let total = |list: &[(Item, u32)]| -> f64 { list.iter().map(|(i, n)| i.price() * *n as f64).sum() };
    for k in Kind::ALL {
        for r in k.recipes() {
            // Molten metal has no price; grit is about yield, not price.
            if r.outputs.iter().all(|o| o.0 == Item::Waste) || k == Kind::Crusher {
                continue;
            }
            let (put, got) = (total(r.inputs), total(r.outputs));
            println!("{:<18} {:<26} in ${put:>8.2}  out ${got:>8.2}", k.name(), r.name);
            assert!(got > put, "{k:?} {}: ${put} in, ${got} out", r.name);
        }
    }
    // The steel routes: the steel is worth more than the plate, coke and coal behind it.
    for r in BLAST_RECIPES.iter().filter(|r| r.fluid_out.is_some_and(|f| f.0 == Fluid::MoltenSteel)) {
        assert!(Item::Steel.price() > total(r.inputs), "{}", r.name);
    }
}

#[test]
fn crown_items_keep_a_high_research_floor() {
    let crown = [Item::DrillingRig, Item::SteamTractor, Item::Computer];
    let top = Item::ALL.iter().filter(|i| !crown.contains(i)).map(|i| i.curve().floor).fold(0.0, f64::max);
    for c in crown {
        let curve = c.curve();
        assert!(curve.floor > 2.0 * top, "{c:?} floor {} vs the rest's {top}", curve.floor);
        // However many are sold, each still pays a good share of its worth.
        assert!(curve.value(1_000_000) >= curve.worth * curve.floor);
        assert!(curve.value(1_000_000) > 1000.0, "{c:?}");
        assert!(curve.worth >= 2.0 * c.price(), "{c:?} worth about 2x the price");
    }
    assert_eq!((Item::DrillingRig.price(), Item::SteamTractor.price(), Item::Computer.price()), (3500.0, 7500.0, 11_000.0));
    // Every new item is worth at least 10 research, and about twice its price.
    for it in &Item::ALL[28..] {
        let worth = it.curve().worth;
        assert!(worth >= 10.0 && (worth == 10.0 || worth == 2.0 * it.price()), "{it:?} worth {worth}");
    }
}

#[test]
fn new_items_have_prices_and_research_curves() {
    let prices = [
        (Item::Brick, 1.0),
        (Item::Glass, 2.0),
        (Item::SteelRod, 16.0),
        (Item::Bearing, 12.0),
        (Item::SteelSheet, 36.0),
        (Item::HardenedIronPlate, 14.0),
        (Item::HardenedGear, 50.0),
        (Item::ReinforcedConcrete, 16.0),
        (Item::Valve, 50.0),
        (Item::Gauge, 35.0),
        (Item::Lamp, 15.0),
        (Item::Relay, 80.0),
        (Item::ElectricMotor, 160.0),
        (Item::Piston, 60.0),
        (Item::ControlBoard, 170.0),
        (Item::BoilerShell, 80.0),
        (Item::SiloShell, 260.0),
        (Item::Frame, 280.0),
        (Item::PumpUnit, 900.0),
        (Item::Generator, 1100.0),
        (Item::EngineBlock, 1150.0),
        (Item::ControlPanel, 900.0),
        (Item::SteamTractor, 7500.0),
        (Item::DrillingRig, 3500.0),
        (Item::Steel, 24.0),
        (Item::SteelGirder, 75.0),
        (Item::Gearbox, 120.0),
        (Item::Concrete, 2.0),
    ];
    for (item, price) in prices {
        assert_eq!(item.price(), price, "{item:?}");
    }
    assert_eq!(ITEMS, Item::ALL.len());
    assert_eq!(Item::ALL[28], Item::Brick, "new items go on the end");
}

/// Ore in, motors out: copper (furnace -> molder -> press -> wire drawer -> winder), iron
/// (furnace -> molder -> press) into steel (blast furnace -> molder), steel rods and bearings
/// (two lathes), and a basic assembler putting it together.
#[test]
fn a_small_factory_makes_electric_motors_from_ore() {
    let mut w = World::blank();
    let crate_with = |w: &mut World, x: i32, y: i32, d: Dir, items: &[Item]| {
        w.place(0, x, y, Kind::Crate, d).unwrap();
        fill_crate(w, 0, x, y, items);
    };
    let mix = |ore: Item, n: usize, coal: usize| -> Vec<Item> {
        let mut v = Vec::new();
        for i in 0..n.max(coal) {
            if i < coal {
                v.push(Item::Coal);
            }
            if i < n {
                v.push(ore);
            }
        }
        v
    };
    // Lays belts along the points (straight runs); the last tile points the way of its run.
    let lay = |w: &mut World, pts: &[(i32, i32)]| {
        let last = pts.len() - 2;
        for (n, pair) in pts.windows(2).enumerate() {
            let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
            let d = if x1 > x0 {
                Dir::E
            } else if x1 < x0 {
                Dir::W
            } else if y1 > y0 {
                Dir::S
            } else {
                Dir::N
            };
            let (dx, dy) = d.delta();
            let steps = (x1 - x0).abs() + (y1 - y0).abs() + (n == last) as i32;
            for k in 0..steps {
                w.place(0, x0 + dx * k, y0 + dy * k, Kind::Belt, d).unwrap();
            }
        }
    };
    // Copper line along y = 10.
    crate_with(&mut w, 5, 10, Dir::E, &mix(Item::CopperOre, 6, 8));
    w.place(0, 6, 9, Kind::Furnace, Dir::E).unwrap(); // molten copper
    w.place(0, 9, 10, Kind::Molder, Dir::E).unwrap(); // copper ingots
    w.place(0, 11, 10, Kind::Press, Dir::E).unwrap(); // copper plate
    w.place(0, 13, 10, Kind::WireDrawer, Dir::E).unwrap(); // 3 wire a plate
    w.place(0, 15, 10, Kind::Winder, Dir::E).unwrap(); // coils
                                                       // Iron line along y = 20.
    crate_with(&mut w, 5, 20, Dir::E, &mix(Item::IronOre, 9, 8));
    w.place(0, 6, 19, Kind::Furnace, Dir::E).unwrap();
    pick_recipe(&mut w, 6, 19, "molten iron");
    w.place(0, 9, 20, Kind::Molder, Dir::E).unwrap();
    pick_recipe(&mut w, 9, 20, "iron ingot");
    w.place(0, 11, 20, Kind::Press, Dir::E).unwrap();
    pick_recipe(&mut w, 11, 20, "iron plate");
    w.place(0, 13, 20, Kind::BlastFurnace, Dir::E).unwrap(); // covers 13-14, 20-21
    pick_recipe(&mut w, 13, 20, "molten steel (coal)");
    crate_with(&mut w, 13, 19, Dir::S, &[Item::Coal; 6]);
    crate_with(&mut w, 15, 20, Dir::E, &[]); // the waste
    w.place(0, 15, 21, Kind::Molder, Dir::E).unwrap(); // covers 15-16, 21-22
    pick_recipe(&mut w, 15, 21, "steel");
    w.place(0, 17, 21, Kind::Lathe, Dir::E).unwrap(); // steel rods; covers 17-19, 21-23
    pick_recipe(&mut w, 17, 21, "steel rod");
    w.place(0, 20, 22, Kind::Lathe, Dir::E).unwrap(); // bearings; covers 20-22, 22-24
    pick_recipe(&mut w, 20, 22, "bearing");
    // The assembler at 30,12 and its three feeds: coils from the north, rods from the
    // west, bearings from the south. Rods leave the first lathe in turn onto the belt
    // (20,21) and into the bearing lathe.
    w.place(0, 30, 12, Kind::BasicAssembler, Dir::E).unwrap();
    pick_recipe(&mut w, 30, 12, "electric motor");
    w.place(0, 32, 12, Kind::Crate, Dir::E).unwrap();
    lay(&mut w, &[(17, 10), (30, 10), (30, 11)]);
    lay(&mut w, &[(20, 21), (27, 21), (27, 12), (29, 12)]);
    lay(&mut w, &[(23, 23), (30, 23), (30, 14)]);
    // Power: poles in a loose grid over everything, and panels beside some of them.
    let mut poles = Vec::new();
    for y in (6..=27).step_by(3) {
        for x in (6..=36).step_by(3) {
            if w.check_place(0, x, y, Kind::Pole).is_ok() {
                w.place(0, x, y, Kind::Pole, Dir::E).unwrap();
                poles.push((x, y));
            }
        }
    }
    let mut panels = 0;
    'panels: for (px, py) in poles {
        for (dx, dy) in [(1, 1), (-2, 1), (1, -2), (-2, -2)] {
            if panels < 10 && w.check_place(0, px + dx, py + dy, Kind::Solar).is_ok() {
                w.place(0, px + dx, py + dy, Kind::Solar, Dir::E).unwrap();
                panels += 1;
                continue 'panels;
            }
        }
    }
    assert!(panels >= 8, "{panels} panels");
    // Up to five minutes, but stop as soon as two motors are out.
    for _ in 0..300 {
        run(&mut w, 1);
        if crate_items(&w, 0, 32, 12).len() >= 2 {
            break;
        }
    }
    let motors = crate_items(&w, 0, 32, 12);
    let at_asm = match &w.tile(0, 30, 12).unwrap().st {
        St::Crafter { stock, .. } => [Item::CopperCoil, Item::SteelRod, Item::Bearing].map(|i| stock[i.idx()]),
        _ => unreachable!(),
    };
    let made = [Item::CopperCoil, Item::Steel, Item::SteelRod, Item::Bearing].map(|i| w.econ.made[i.idx()]);
    assert!(
        motors.len() >= 2,
        "motors out: {} (made coil/steel/rod/bearing {made:?}; assembler holds {at_asm:?}, {:?}, EC {})",
        motors.len(),
        w.idle_reason(0, idx(30, 12)),
        w.ratio_at(0, 30, 12)
    );
    assert!(motors.iter().all(|i| *i == Item::ElectricMotor));
    assert!(w.econ.made[Item::Waste.idx()] > 0, "steel left waste behind");
}

#[test]
fn no_machine_shows_two_recipes_with_one_letter() {
    // Two routes to one product may share a letter; different products may not.
    let made = |r: &Recipe| r.fluid_out.map(|f| f.0.name()).or(r.outputs.iter().find(|o| o.0 != Item::Waste).map(|o| o.0.name()));
    for kind in Kind::ALL {
        let mut seen: Vec<(char, Option<&str>, &str)> = Vec::new();
        for r in kind.recipes() {
            if let Some((_, _, other)) = seen.iter().find(|(t, m, _)| *t == r.tag && *m != made(r)) {
                panic!("{kind:?}: {} and {other} share the letter {}", r.name, r.tag);
            }
            seen.push((r.tag, made(r), r.name));
        }
    }
}

// ---------- the better silicon machines (computers phase 2b) ----------

/// An arc furnace on `recipe` at (12, 12), on power only, with a waste crate behind it and a
/// molder casting its silicon into a crate. Returns (silicon ingots, waste) after `secs`.
fn arc_route(recipe: &str, stock: &[(Item, u32)], secs: u64) -> (usize, usize) {
    let mut w = World::blank();
    big_grid(&mut w, 10, 0, 20);
    w.place(0, 12, 12, Kind::ArcFurnace, Dir::E).unwrap(); // covers 12-14: front is x=15
    assert_eq!(w.ratio_at(0, 12, 12), 1.0, "the arc furnace is fully powered");
    pick_recipe(&mut w, 12, 12, recipe);
    for (item, n) in stock {
        set_stock(&mut w, 12, 12, *item, *n);
    }
    w.place(0, 11, 12, Kind::Crate, Dir::W).unwrap(); // the waste, out the back
    w.place(0, 15, 13, Kind::Molder, Dir::E).unwrap();
    pick_recipe(&mut w, 15, 13, "silicon ingot");
    w.place(0, 15, 15, Kind::Solar, Dir::E).unwrap(); // the molder's own power
    w.place(0, 17, 13, Kind::Crate, Dir::E).unwrap(); // the ingots
    run(&mut w, secs);
    (
        crate_items(&w, 0, 17, 13).iter().filter(|i| **i == Item::SiliconIngot).count(),
        crate_items(&w, 0, 11, 12).iter().filter(|i| **i == Item::Waste).count(),
    )
}

#[test]
fn an_arc_furnace_melts_silicon_on_power_alone_and_remelts_scrap() {
    for r in Kind::ArcFurnace.recipes() {
        assert_eq!(r.fuel, None, "{} burns nothing", r.name);
    }
    // On coke: 2 silicon and 1 waste, where the blast furnace gives 1 and 2.
    assert_eq!(arc_route("molten silicon", &[(Item::Sand, 3), (Item::Coke, 1)], 20), (2, 1));
    // On coal: 1 silicon and 2 waste.
    assert_eq!(arc_route("molten silicon (coal)", &[(Item::Sand, 3), (Item::Coal, 2)], 20), (1, 2));
    // Scrap back to silicon: 3 scrap give 2, with no waste.
    assert_eq!(arc_route("molten silicon (remelt)", &[(Item::SiliconScrap, 3)], 15), (2, 0));
    // The old routes stay.
    assert!(Kind::BlastFurnace.recipes().iter().any(|r| r.name == "molten silicon"));
    assert!(Kind::Lathe.recipes().iter().any(|r| r.name == "wafer (rough)"));
}

#[test]
fn a_grower_and_saw_make_six_wafers_from_three_ingots() {
    assert!(run_powered(Kind::CrystalGrower, "silicon crystal", &[(Item::SiliconIngot, 3)], 19).is_empty());
    assert_eq!(run_powered(Kind::CrystalGrower, "silicon crystal", &[(Item::SiliconIngot, 3)], 21), [Item::SiliconCrystal]);
    let out = run_powered(Kind::WaferSaw, "wafer", &[(Item::SiliconCrystal, 1), (Item::FineWire, 1)], 11);
    assert_eq!((count(&out, Item::Wafer), out.len()), (6, 6), "no scrap");
}

// ---------- fine etching (computers phase 4) ----------

#[test]
fn a_fine_etcher_makes_a_fine_die_and_a_spare_die_from_each_wafer() {
    assert_eq!(recipe_names(Kind::FineEtcher), ["fine die"]);
    let stock = [(Item::DopedWafer, 1), (Item::FineMask, 1), (Item::Etchant, 1)];
    // 96 EC: a bigger grid than the usual 90, still fully powered.
    let mut w = World::blank();
    big_grid(&mut w, 10, 0, 10);
    w.place(0, 12, 12, Kind::FineEtcher, Dir::E).unwrap(); // covers 12-14 across, 12-16 down: front is x=15
    assert_eq!(w.ratio_at(0, 12, 12), 1.0, "the fine etcher is fully powered");
    pick_recipe(&mut w, 12, 12, "fine die");
    for (item, n) in stock {
        set_stock(&mut w, 12, 12, item, n);
    }
    w.place(0, 15, 12, Kind::Crate, Dir::E).unwrap();
    run(&mut w, 19);
    assert!(crate_items(&w, 0, 15, 12).is_empty(), "nothing before 20 s");
    run(&mut w, 3);
    let out = crate_items(&w, 0, 15, 12);
    assert_eq!((count(&out, Item::FineDie), count(&out, Item::Die), out.len()), (1, 1, 2), "{out:?}");
    assert_eq!(w.econ.used[Item::FineMask.idx()], 1);
    // Short of any one input, it does not start.
    for missing in 0..stock.len() {
        let mut part = stock.to_vec();
        part.remove(missing);
        assert!(run_powered(Kind::FineEtcher, "fine die", &part, 30).is_empty(), "missing {:?}", stock[missing].0);
    }
}

#[test]
fn the_fine_etcher_is_meridians_machine_and_a_better_route_not_the_only_one() {
    assert_eq!((Kind::FineEtcher.cost(), Kind::FineEtcher.power_draw(), Kind::FineEtcher.base_dims()), (60_000.0, 96.0, (3, 5)));
    assert!(Kind::FineEtcher.electric() && !Kind::FineEtcher.uses_pipes());
    assert!(Kind::FineEtcher.recipes().iter().all(|r| r.fuel.is_none() && r.fluid_in.is_none() && r.fluid_out.is_none()));
    assert_eq!((Kind::FineEtcher.company(), Kind::FineEtcher.tech()), (Some(Company::Meridian), Some(Tech::FineEtcher)));
    assert_eq!(Kind::FineEtcher.category(), Category::Create);
    assert_eq!(
        (Tech::FineEtcher.company(), Tech::FineEtcher.cost(), Tech::FineEtcher.milestone()),
        (Company::Meridian, 150_000.0, (Made::Item(Item::Microprocessor), 100))
    );
    assert_eq!(Tech::FineEtcher.requires(), &[Made::Item(Item::Microprocessor)]);
    // Create slot 6: the etcher is still the first pick.
    assert_eq!(crate::app::slots(Category::Create)[5], &[Kind::Etcher, Kind::CrystalGrower, Kind::WaferSaw, Kind::FineEtcher]);
    // Under the x4x1 in the tree, in the etcher's research group.
    let at = |n: Node| TREE.iter().position(|(m, _)| *m == n).unwrap();
    assert_eq!(TREE[at(Node::Tech(Tech::FineEtcher))].1, Some(at(Node::Key(Made::Item(Item::Microprocessor)))));
    let group = |t: Tech| UNLOCK_GROUPS.iter().find(|(_, ts)| ts.contains(&t)).map(|(n, _)| *n);
    assert!(group(Tech::FineEtcher).is_some() && group(Tech::FineEtcher) == group(Tech::Etcher));
    // Locked until researched (or the 100 x4x1 milestone).
    let mut p = Progress::default();
    assert!(!p.part_unlocked(Kind::FineEtcher));
    p.opened[Company::Meridian.idx()] = true;
    p.researched[Tech::FineEtcher.idx()] = true;
    assert!(p.part_unlocked(Kind::FineEtcher));
    // The old road stays: the etcher's die and the x4x1 do not need Meridian's machine.
    assert_eq!(recipe_names(Kind::Etcher), ["die"]);
    let mut open = [true; COMPANIES];
    open[Company::Meridian.idx()] = false;
    let (have, _) = makeable(&open);
    assert!(have[Item::Microprocessor.idx()] && have[Item::Die.idx()] && !have[Item::FineDie.idx()]);
    let (have, _) = makeable(&[true; COMPANIES]);
    assert!(have[Item::FineDie.idx()] && have[Item::FineMicroprocessor.idx()]);
}

#[test]
fn only_a_precision_assembler_makes_the_fine_mask_and_the_x4x2() {
    let out = run_powered(Kind::PrecisionAssembler, "fine mask", &[(Item::Mask, 1), (Item::Glass, 1), (Item::FineWire, 2)], 11);
    assert_eq!(out, [Item::FineMask]);
    let cpu = [(Item::FineDie, 1), (Item::Ceramic, 1), (Item::FineWire, 4)];
    assert!(run_powered(Kind::PrecisionAssembler, "microprocessor (x4x2)", &cpu, 9).is_empty());
    assert_eq!(run_powered(Kind::PrecisionAssembler, "microprocessor (x4x2)", &cpu, 11), [Item::FineMicroprocessor]);
    for k in [Kind::CrudeAssembler, Kind::BasicAssembler, Kind::AdvancedAssembler] {
        assert!(!recipe_names(k).contains(&"fine mask") && !recipe_names(k).contains(&"microprocessor (x4x2)"), "{k:?}");
    }
}

#[test]
fn a_splitter_feeds_a_belt_lying_beside_it_but_not_one_facing_it() {
    let mut w = World::new();
    w.place(0, 12, 10, Kind::Splitter, Dir::E).unwrap();
    // East of it a belt runs south; west of it a belt faces the splitter head-on.
    w.place(0, 13, 10, Kind::Belt, Dir::S).unwrap();
    w.place(0, 11, 10, Kind::Belt, Dir::E).unwrap();
    // Power for the belts and splitter.
    w.place(0, 14, 8, Kind::Solar, Dir::E).unwrap();
    w.place(0, 12, 8, Kind::Pole, Dir::E).unwrap();
    if let Some(St::Splitter { buf, .. }) = w.floors[0].anchor_at(12, 10).and_then(|a| w.floors[0].tiles[a].as_mut()).map(|b| &mut b.st) {
        buf.push_back(Item::Coal);
    }
    for _ in 0..TPS * 3 {
        w.tick();
    }
    let items = |x, y| match &w.tile(0, x, y).unwrap().st {
        St::Belt { items } => items.len(),
        _ => 0,
    };
    let beyond = (0..14).map(|y| if w.tile(0, 13, y).is_some() { items(13, y) } else { 0 }).sum::<usize>();
    assert!(beyond > 0, "the coal went onto the belt beside the splitter");
    assert_eq!(items(11, 10), 0, "nothing went back into the belt facing it");
}

#[test]
fn a_mirrored_blueprint_swaps_sides_and_facings_and_mirroring_twice_is_the_identity() {
    use crate::sim::blueprint::{Blueprint, BpPart};
    let part = |dx, dy, kind, dir| BpPart { dx, dy, kind, dir, size: 0, set: None, exit: None };
    let bp = Blueprint::from_parts(
        "line",
        0,
        vec![
            part(0, 0, Kind::Drill, Dir::E),
            part(2, 0, Kind::Belt, Dir::E),
            part(3, 0, Kind::Belt, Dir::E),
            part(2, 1, Kind::Belt, Dir::S),
        ],
    );
    let m = bp.mirrored(false);
    assert_eq!((m.w, m.h), (bp.w, bp.h));
    let at = |b: &Blueprint, dx, dy| b.parts.iter().find(|p| p.dx == dx && p.dy == dy).map(|p| (p.kind, p.dir));
    // The 2x2 drill moves to the right edge and faces west; the belts swap ends and face west.
    assert_eq!(at(&m, 2, 0), Some((Kind::Drill, Dir::W)));
    assert_eq!(at(&m, 1, 0), Some((Kind::Belt, Dir::W)));
    assert_eq!(at(&m, 0, 0), Some((Kind::Belt, Dir::W)));
    assert_eq!(at(&m, 1, 1), Some((Kind::Belt, Dir::S)), "north and south facings stay in a left-right mirror");
    assert_eq!(m.mirrored(false), bp);
    // Top to bottom: south and north swap, east and west stay.
    let v = bp.mirrored(true);
    assert_eq!(at(&v, 2, 0), Some((Kind::Belt, Dir::N)));
    assert_eq!(v.mirrored(true), bp);
    // A mirrored copy still pastes cleanly.
    assert!(m.check().is_ok() && v.check().is_ok());
}

#[test]
fn the_map_has_no_edge_to_meet_a_factory_can_stand_far_out_and_below_zero() {
    for (ox, oy) in [(-300, -200), (5000, 4000), (-20_000, 12_000)] {
        let mut w = World::blank();
        w.econ.money = 1e6;
        // A drill straight into a trade post, nowhere near the start.
        w.place(0, ox, oy, Kind::Drill, Dir::E).unwrap();
        w.place(0, ox + 2, oy, Kind::TradePost, Dir::E).unwrap();
        let before = w.econ.money;
        for _ in 0..TPS * 60 {
            w.tick();
        }
        assert!(w.econ.money > before, "it sells at {ox},{oy}");
        // The area tool, a save and a load all find it again.
        assert_eq!(w.parts_in(0, (ox - 5, oy - 5, ox + 5, oy + 5)), vec![(ox, oy), (ox + 2, oy)]);
        assert_eq!(w.parts_in(0, (-30_000, -30_000, 30_000, 30_000)).len(), 2, "a huge rectangle is cheap");
        let path = std::env::temp_dir().join(format!("factory-far-{}-{ox}.json", std::process::id()));
        w.save(&path).unwrap();
        let l = World::load(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(l.tile(0, ox, oy).is_some_and(|b| b.kind == Kind::Drill) && l.tile(0, ox + 2, oy).is_some());
    }
    // The only edge is far past anything a factory needs.
    let mut w = World::blank();
    assert_eq!(w.place(0, EDGE, 0, Kind::Belt, Dir::E), Err("Doesn't fit on the map here".to_string()));
}
