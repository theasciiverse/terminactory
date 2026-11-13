//! Pipes, molten metal and machines with two kinds of output.

use super::*;

fn pool_total(w: &World) -> f64 {
    w.fluids.pools.iter().map(|p| p.amount).sum()
}

fn out_len(w: &World, x: i32, y: i32) -> usize {
    match &w.tile(0, x, y).unwrap().st {
        St::Crafter { out, .. } => out.len(),
        _ => panic!("not a machine"),
    }
}

#[test]
fn joining_two_networks_with_different_fluids_keeps_one_of_them() {
    // Pinned: the network holding the first pipe in map order (top row, then left) keeps its fluid;
    // the other's is lost. Nothing is ever mixed.
    let mut w = World::blank();
    for x in [10, 11, 13, 14] {
        w.place(0, x, 10, Kind::Pipe, Dir::E).unwrap();
    }
    assert_eq!(w.fluids.pools.len(), 2);
    w.fluids.pools[0].add(Fluid::MoltenCopper, 5.0);
    w.fluids.pools[1].add(Fluid::Water, 7.0);
    w.place(0, 12, 10, Kind::Pipe, Dir::E).unwrap();
    assert_eq!(w.fluids.pools.len(), 1);
    let p = &w.fluids.pools[0];
    assert_eq!((p.fluid, p.amount, p.pipes), (Some(Fluid::MoltenCopper), 5.0, 5));
}

#[test]
fn joining_two_networks_of_the_same_fluid_adds_them_up() {
    let mut w = World::blank();
    for x in [10, 11, 13, 14] {
        w.place(0, x, 10, Kind::Pipe, Dir::E).unwrap();
    }
    w.fluids.pools[0].add(Fluid::Water, 5.0);
    w.fluids.pools[1].add(Fluid::Water, 7.0);
    w.place(0, 12, 10, Kind::Pipe, Dir::E).unwrap();
    assert_eq!((w.fluids.pools[0].fluid, w.fluids.pools[0].amount), (Some(Fluid::Water), 12.0));
}

#[test]
fn a_networks_capacity_grows_with_its_pipes() {
    let mut w = World::blank();
    for x in 10..13 {
        w.place(0, x, 10, Kind::Pipe, Dir::E).unwrap();
    }
    assert_eq!((w.fluids.pools[0].pipes, w.fluids.pools[0].cap), (3, 3.0 * PIPE_CAP));
    w.place(0, 13, 10, Kind::Pipe, Dir::E).unwrap();
    assert_eq!((w.fluids.pools[0].pipes, w.fluids.pools[0].cap), (4, 4.0 * PIPE_CAP));
    // A pipe on its own is its own network.
    w.place(0, 30, 30, Kind::Pipe, Dir::E).unwrap();
    assert_eq!(w.fluids.pools.len(), 2);
    assert_eq!(w.fluids.pools[1].cap, PIPE_CAP);
}

#[test]
fn a_machine_takes_fluid_from_its_back_and_sides_but_not_its_front() {
    let molten = |pipe_at: (i32, i32)| {
        let mut w = World::blank();
        w.place(0, 10, 10, Kind::Molder, Dir::E).unwrap(); // covers 10-11, 10-11; front is x=12
        w.place(0, pipe_at.0, pipe_at.1, Kind::Pipe, Dir::E).unwrap();
        w.fluids.pools[0].add(Fluid::MoltenCopper, 10.0);
        run(&mut w, 1);
        (crafter_fluid(&w, 10, 10).0, w.fluids.pools[0].amount)
    };
    assert_eq!(molten((12, 10)), (0, 10.0), "a pipe at the front is for output only");
    assert!(molten((9, 10)).0 > 0, "back");
    assert!(molten((10, 9)).0 > 0, "side");
    assert!(molten((11, 12)).0 > 0, "other side");
}

#[test]
fn changing_a_recipe_lets_go_of_held_fluid() {
    // Pinned: the fluid isn't returned to the pipes; it is dropped.
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Molder, Dir::E).unwrap();
    if let Some(Building { st: St::Crafter { fluid, fluid_out, .. }, .. }) = w.floors[0].tiles[idx(10, 10)].as_mut() {
        *fluid = 5;
        fluid_out[0] = 3;
        fluid_out[1] = 4;
    }
    w.configure(0, 10, 10).unwrap();
    assert_eq!(crafter_fluid(&w, 10, 10), (0, 0));
    assert!(matches!(&w.tile(0, 10, 10).unwrap().st, St::Crafter { fluid_out, .. } if *fluid_out == [0, 0]));
}

#[test]
fn a_pump_next_to_another_fluid_adds_nothing() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Pump, Dir::E).unwrap(); // covers 5, 5-6
    w.place(0, 6, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 5, 7, Kind::Solar, Dir::E).unwrap();
    assert!(w.ratio_at(0, 5, 5) > 0.0);
    w.fluids.pools[0].add(Fluid::MoltenCopper, 5.0);
    run(&mut w, 3);
    assert_eq!((w.fluids.pools[0].fluid, w.fluids.pools[0].amount), (Some(Fluid::MoltenCopper), 5.0));
    // Once the metal is gone the same pump fills it with water.
    w.fluids.pools[0].take(Fluid::MoltenCopper, 5.0);
    run(&mut w, 3);
    assert_eq!(w.fluids.pools[0].fluid, Some(Fluid::Water));
    assert!(w.fluids.pools[0].amount > 0.0);
}

#[test]
fn pipe_contents_survive_saving_and_loading() {
    let mut w = World::blank();
    for x in 10..15 {
        w.place(0, x, 10, Kind::Pipe, Dir::E).unwrap();
    }
    w.fluids.pools[0].add(Fluid::MoltenCopper, 60.0);
    let path = std::env::temp_dir().join(format!("factory-test-pipes-{}.json", std::process::id()));
    w.save(&path).unwrap();
    let loaded = World::load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(pool_total(&loaded), 60.0);
    assert_eq!(loaded.fluids.pools[0].fluid, Some(Fluid::MoltenCopper));
}

/// Coal in, one coke and one unit of molten copper out (the game has no recipe like it yet).
static TWO_OUT: [Recipe; 1] = [Recipe {
    name: "test",
    inputs: &[(Item::Coal, 1)],
    outputs: &[(Item::Coke, 1)],
    fluid_in: None,
    fluid_out: Some((Fluid::MoltenCopper, 1)),
    fluid_out2: None,
    ticks: 60,
    fuel: None,
    needs: None,
    tag: 'T',
}];

static TWO_FLUIDS: [Recipe; 1] = [Recipe {
    name: "two fluids",
    inputs: &[(Item::Coal, 1)],
    outputs: &[],
    fluid_in: None,
    fluid_out: Some((Fluid::MoltenCopper, 1)),
    fluid_out2: Some((Fluid::CrudeOil, 1)),
    ticks: 60,
    fuel: None,
    needs: None,
    tag: 'F',
}];

#[test]
fn two_fluid_outputs_use_separate_front_and_back_pipes_and_are_counted() {
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &TWO_FLUIDS))));
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Molder, Dir::E).unwrap();
    w.place(0, 7, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 4, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 5, 7, Kind::Solar, Dir::E).unwrap();
    set_stock(&mut w, 5, 5, Item::Coal, 5);
    run(&mut w, 3);
    assert_eq!(w.fluids.pools.len(), 2);
    assert_eq!(w.fluids.pools[0].fluid, Some(Fluid::CrudeOil));
    assert_eq!(w.fluids.pools[1].fluid, Some(Fluid::MoltenCopper));
    assert_eq!(w.fluids.pools.iter().map(|p| p.amount).sum::<f64>(), 6.0);
    assert_eq!(w.econ.made_fluid[Fluid::MoltenCopper], 3);
    assert_eq!(w.econ.made_fluid[Fluid::CrudeOil], 3);
    let path = std::env::temp_dir().join(format!("factory-test-fluid-made-{}.json", std::process::id()));
    w.save(&path).unwrap();
    let loaded = World::load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(loaded.econ.made_fluid, w.econ.made_fluid);
    RECIPE_OVERRIDE.with(|o| o.set(None));
}

#[test]
fn full_back_fluid_output_stops_then_resumes_when_drained() {
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &TWO_FLUIDS))));
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Molder, Dir::E).unwrap();
    w.place(0, 7, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 4, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 5, 7, Kind::Solar, Dir::E).unwrap();
    w.fluids.pools[0].add(Fluid::Water, 20.0);
    set_stock(&mut w, 5, 5, Item::Coal, 30);
    run(&mut w, 15);
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FULL"));
    let used = w.econ.used[Item::Coal.idx()];
    w.fluids.pools[0].take(Fluid::Water, 20.0);
    run(&mut w, 3);
    assert!(w.econ.used[Item::Coal.idx()] > used);
    RECIPE_OVERRIDE.with(|o| o.set(None));
}

#[test]
fn two_outputs_refuse_wrong_fluids_and_back_pipe_is_not_an_input() {
    static INPUT_AND_TWO_OUT: [Recipe; 1] = [Recipe {
        name: "input and two fluids",
        inputs: &[(Item::Coal, 1)],
        outputs: &[],
        fluid_in: Some((Fluid::Water, 1)),
        fluid_out: Some((Fluid::MoltenCopper, 1)),
        fluid_out2: Some((Fluid::CrudeOil, 1)),
        ticks: 60,
        fuel: None,
        needs: None,
        tag: 'I',
    }];
    // Both output pipes hold a different fluid, so neither output can leave. The machine
    // fills its own output buffers, then stops full.
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &TWO_FLUIDS))));
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Molder, Dir::E).unwrap();
    w.place(0, 7, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 4, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 5, 7, Kind::Solar, Dir::E).unwrap();
    w.fluids.pools[0].add(Fluid::Water, 20.0);
    w.fluids.pools[1].add(Fluid::Steam, 20.0);
    set_stock(&mut w, 5, 5, Item::Coal, 30);
    run(&mut w, 15);
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FULL"));
    assert_eq!(w.econ.used[Item::Coal.idx()], 10, "both mismatched output pools fill their machine buffers");
    assert!(
        matches!(&w.tile(0, 5, 5).unwrap().st, St::Crafter { fluid_out, .. } if *fluid_out == [CRAFTER_FLUID_OUT_CAP; 2]),
        "each output sits at its buffer cap"
    );
    assert_eq!((w.fluids.pools[0].fluid, w.fluids.pools[1].fluid), (Some(Fluid::Water), Some(Fluid::Steam)));
    assert_eq!(pool_total(&w), 40.0, "neither pipe took a drop");

    // A back pipe is ignored for fluid input when the recipe has a second output.
    let mut w = World::blank();
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &INPUT_AND_TWO_OUT))));
    w.place(0, 5, 5, Kind::Molder, Dir::E).unwrap();
    w.place(0, 7, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 4, 5, Kind::Pipe, Dir::E).unwrap();
    w.fluids.pools[0].add(Fluid::Water, 10.0);
    set_stock(&mut w, 5, 5, Item::Coal, 10);
    run(&mut w, 2);
    assert_eq!(crafter_fluid(&w, 5, 5).0, 0, "the back pipe is reserved for the second output");
    assert_eq!(w.econ.used[Item::Coal.idx()], 0);
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FLUD"));
    RECIPE_OVERRIDE.with(|o| o.set(None));
}

#[test]
fn reactor_sized_fluid_batch_fits_the_buffer_and_fires() {
    static LARGE_FLUID: [Recipe; 1] = [Recipe {
        name: "large fluid batch",
        inputs: &[(Item::Coal, 1)],
        outputs: &[],
        fluid_in: Some((Fluid::Ethylene, 16)),
        fluid_out: Some((Fluid::MoltenSilicon, 16)),
        fluid_out2: None,
        ticks: 60,
        fuel: None,
        needs: None,
        tag: 'L',
    }];
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &LARGE_FLUID))));
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Molder, Dir::E).unwrap();
    w.place(0, 7, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 5, 4, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 5, 7, Kind::Solar, Dir::E).unwrap();
    w.fluids.pools[0].add(Fluid::Ethylene, 16.0);
    set_stock(&mut w, 5, 5, Item::Coal, 1);
    run(&mut w, 2);
    assert_eq!(w.econ.used[Item::Coal.idx()], 1);
    assert_eq!(w.econ.made_fluid[Fluid::MoltenSilicon], 16);
    RECIPE_OVERRIDE.with(|o| o.set(None));
}

#[test]
fn a_machine_stops_when_the_items_are_full_even_if_the_pipe_has_room() {
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &TWO_OUT))));
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Molder, Dir::E).unwrap(); // front is x=7, y=5 and 6
    set_stock(&mut w, 5, 5, Item::Coal, 100);
    w.place(0, 7, 5, Kind::Pipe, Dir::E).unwrap(); // the metal can go here...
                                                   // ...but nothing takes the coke at (7, 6): it piles up in the machine.
    w.place(0, 5, 7, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 60);
    assert_eq!(out_len(&w, 5, 5), Kind::Molder.out_cap());
    let metal = w.fluids.pools[0].amount;
    assert_eq!(metal, Kind::Molder.out_cap() as f64, "one unit per coke, then both stop");
    assert!(metal < w.fluids.pools[0].cap, "the pipe still has room");
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FULL"));
    run(&mut w, 10);
    assert_eq!(w.fluids.pools[0].amount, metal);
    RECIPE_OVERRIDE.with(|o| o.set(None));
}

/// Stone in, a brick and a piece of waste out, fired eight at a time.
static KILN_TWO_OUT: [Recipe; 1] = [Recipe {
    name: "test",
    inputs: &[(Item::Stone, 2)],
    outputs: &[(Item::Brick, 1), (Item::Waste, 1)],
    fluid_in: None,
    fluid_out: None,
    fluid_out2: None,
    ticks: 300,
    fuel: Some(Item::Coal),
    needs: None,
    tag: 'T',
}];

#[test]
fn a_kiln_batch_multiplies_every_output_by_eight() {
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Kiln, &KILN_TWO_OUT))));
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Kiln, Dir::E).unwrap(); // covers 5-7, 5-9; front is x=8
    set_stock(&mut w, 5, 5, Item::Stone, 16);
    set_stock(&mut w, 5, 5, Item::Coal, 5);
    w.place(0, 5, 10, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 8);
    assert_eq!((w.econ.made[Item::Brick.idx()], w.econ.made[Item::Waste.idx()]), (8, 8));
    assert_eq!(out_len(&w, 5, 5), 16);
    // The batch needs room for all eight bricks at once: with 73 waiting, 73 + 8 is over the cap of 80.
    if let Some(Building { st: St::Crafter { out, .. }, .. }) = w.floors[0].tiles[idx(5, 5)].as_mut() {
        out.extend(std::iter::repeat_n(Item::Brick, 65));
    }
    set_stock(&mut w, 5, 5, Item::Stone, 16);
    run(&mut w, 8);
    assert_eq!(w.econ.made[Item::Brick.idx()], 8, "didn't fire");
    assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FULL"));
    RECIPE_OVERRIDE.with(|o| o.set(None));
}

#[test]
fn outputs_on_a_wide_front_take_turns_between_the_tiles() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Kiln, Dir::E).unwrap(); // front is x=8, five tiles tall
    cycle_to(&mut w, 5, 5, "brick");
    set_stock(&mut w, 5, 5, Item::Stone, 16);
    set_stock(&mut w, 5, 5, Item::Coal, 5);
    w.place(0, 8, 6, Kind::Crate, Dir::E).unwrap();
    w.place(0, 8, 8, Kind::Crate, Dir::E).unwrap();
    w.place(0, 5, 10, Kind::Solar, Dir::E).unwrap();
    run(&mut w, 26);
    assert_eq!(crate_items(&w, 0, 8, 6).len(), 4);
    assert_eq!(crate_items(&w, 0, 8, 8).len(), 4);
}
