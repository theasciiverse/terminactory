//! The upgrade planner's world side: the pairs table, swapping parts in place (settings,
//! recipes and contents kept where the new part can hold them), paying the difference, the
//! reading-order cut when money runs short, locked targets, and swapping back.

use super::*;
use crate::sim::tools::UPGRADES;

fn rebuilds() -> u32 {
    REBUILDS.with(|c| c.get())
}

const ALL: TileRect = (0, 0, W - 1, H - 1);

fn kinds_on(w: &World, f: usize) -> Vec<Kind> {
    w.floors[f].tiles.iter().flatten().map(|b| b.kind).collect()
}

fn set_recipe(w: &mut World, x: i32, y: i32, name: &str) {
    cycle_to(w, x, y, name);
}

fn crafter(w: &World, x: i32, y: i32) -> (u8, f64, bool, u32, usize, u32) {
    match &w.tile(0, x, y).unwrap().st {
        St::Crafter { recipe, progress, busy, stock, out, fuel, .. } => (*recipe, *progress, *busy, stock.iter().sum(), out.len(), *fuel),
        _ => panic!("not a machine"),
    }
}

fn drill(w: &World, x: i32, y: i32) -> (Option<Item>, u32, u32, usize) {
    match &w.tile(0, x, y).unwrap().st {
        St::Drill { mines, fuel, coal, out, .. } => (*mines, *fuel, *coal, out.len()),
        _ => panic!("not a drill"),
    }
}

#[test]
fn every_pair_keeps_the_footprint_costs_more_and_agrees_on_facing() {
    assert!(!UPGRADES.is_empty());
    for &(from, to) in UPGRADES {
        assert_eq!(from.base_dims(), to.base_dims(), "{from:?} to {to:?} must keep the footprint");
        assert!(to.cost() > from.cost(), "{from:?} to {to:?} must cost more");
        assert_eq!(from.directional(), to.directional(), "{from:?} to {to:?} must agree on having a facing");
        for dir in Dir::ALL {
            assert_eq!(from.dims(dir), to.dims(dir), "{from:?} to {to:?} facing {dir:?}");
        }
        // The same ground rules, so a swap never lands a part somewhere it can't stand.
        assert_eq!(from.is_drill(), to.is_drill(), "{from:?} to {to:?}");
        assert_eq!(from.is_solar(), to.is_solar(), "{from:?} to {to:?}");
        assert_eq!(from.category(), to.category(), "{from:?} to {to:?}");
        assert_eq!(from.upgrade(), Some(to));
    }
    // A part is the start of one pair at most. A target is a start only for the large advanced
    // assembler (large basic to large advanced to large precision): one step per press.
    for (i, (from, to)) in UPGRADES.iter().enumerate() {
        assert!(UPGRADES.iter().skip(i + 1).all(|(f, _)| f != from), "{from:?} twice");
        assert!(to.upgrade().is_none() || *to == Kind::LargeAdvancedAssembler, "{to:?} is a target and a start");
    }
    // The ones left out on purpose.
    for kind in [
        Kind::Arm,
        Kind::Pump,
        Kind::Crate,
        Kind::AdvancedDrill,
        Kind::Kiln,
        Kind::PrecisionAssembler,
        Kind::LargePrecisionAssembler,
        Kind::FastBelt,
    ] {
        assert_eq!(kind.upgrade(), None, "{kind:?}");
    }
}

#[test]
fn the_table_is_what_the_plan_names() {
    assert_eq!(
        UPGRADES,
        &[
            (Kind::Belt, Kind::FastBelt),
            (Kind::Drill, Kind::BasicDrill),
            (Kind::Solar, Kind::BasicSolar),
            (Kind::AdvancedAssembler, Kind::PrecisionAssembler),
            (Kind::LargeBasicAssembler, Kind::LargeAdvancedAssembler),
            (Kind::LargeAdvancedAssembler, Kind::LargePrecisionAssembler),
            (Kind::LargeBasicDrill, Kind::LargeAdvancedDrill),
        ]
    );
}

#[test]
fn each_pair_swaps_in_place_facing_every_way_and_pays_the_difference() {
    for &(from, to) in UPGRADES {
        for dir in Dir::ALL {
            let mut w = World::blank();
            w.place(0, 20, 20, from, dir).unwrap();
            if let Some(name) = from.recipes().iter().map(|r| r.name).find(|n| to.recipes().iter().any(|t| t.name == *n)) {
                set_recipe(&mut w, 20, 20, name);
            }
            let (fw, fh) = from.dims(dir);
            let before = w.econ.money;
            let (rep, undo) = w.upgrade_area(0, ALL);
            let b = w.tile(0, 20, 20).unwrap();
            assert_eq!((b.kind, b.dir), (to, dir), "{from:?} facing {dir:?}");
            assert_eq!(b.dims(), (fw, fh));
            assert_eq!(rep.upgraded, 1, "{from:?}");
            assert_eq!(rep.made, vec![(to, 1)]);
            assert!((before - w.econ.money - (to.cost() - from.cost())).abs() < 1e-9, "{from:?}: only the difference is charged");
            assert!((rep.cost - (to.cost() - from.cost())).abs() < 1e-9);
            assert_eq!(undo.unwrap().len(), 1);
            // Every tile of the footprint still belongs to the one part, and nothing else moved in.
            let a = idx(20, 20) as u32;
            for (tx, ty) in footprint(20, 20, fw, fh) {
                assert_eq!(w.floors[0].owner[idx(tx, ty)], Some(a), "{from:?} {tx},{ty}");
            }
            assert_eq!(kinds_on(&w, 0), vec![to]);
        }
    }
}

#[test]
fn a_swap_is_one_rebuild_however_many_parts() {
    let mut w = World::blank();
    for i in 0..10 {
        w.place(0, 5 + i, 5, Kind::Belt, Dir::E).unwrap();
        w.place(0, 5 + i * 2, 10, Kind::Solar, Dir::E).unwrap();
    }
    let n = rebuilds();
    let (rep, _) = w.upgrade_area(0, ALL);
    assert_eq!(rep.upgraded, 20);
    assert_eq!(rebuilds() - n, 1);
    // Nothing to do, nothing rebuilt.
    let n = rebuilds();
    let (rep, undo) = w.upgrade_area(0, ALL);
    assert_eq!((rep.upgraded, rebuilds() - n), (0, 0));
    assert!(undo.is_none());
}

#[test]
fn only_parts_in_the_rectangle_change_and_a_long_machine_counts_whole() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Belt, Dir::E).unwrap();
    w.place(0, 12, 10, Kind::Belt, Dir::E).unwrap();
    w.place(0, 10, 20, Kind::LargeBasicAssembler, Dir::E).unwrap(); // 10-14, 20-24
                                                                    // One tile of the assembler and one belt are inside.
    let (rep, _) = w.upgrade_area(0, (12, 10, 14, 20));
    assert_eq!(rep.upgraded, 2);
    assert_eq!(w.tile(0, 10, 10).unwrap().kind, Kind::Belt);
    assert_eq!(w.tile(0, 12, 10).unwrap().kind, Kind::FastBelt);
    assert_eq!(w.tile(0, 10, 20).unwrap().kind, Kind::LargeAdvancedAssembler);
}

#[test]
fn belts_keep_every_item_where_it_is() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Belt, Dir::S).unwrap();
    w.place(0, 10, 11, Kind::Belt, Dir::S).unwrap();
    w.place(0, 12, 10, Kind::Solar, Dir::E).unwrap(); // belts run on EC
    if let Some(Building { st: St::Belt { items }, .. }) = w.floors[0].tiles[idx(10, 10)].as_mut() {
        items.push(BeltItem { item: Item::CopperOre, pos: 0.9 });
        items.push(BeltItem { item: Item::Coal, pos: 0.4 });
    }
    w.upgrade_area(0, ALL);
    let b = w.tile(0, 10, 10).unwrap();
    assert_eq!((b.kind, b.dir), (Kind::FastBelt, Dir::S));
    match &b.st {
        St::Belt { items } => {
            assert_eq!(items.len(), 2);
            assert_eq!((items[0].item, items[0].pos), (Item::CopperOre, 0.9));
            assert_eq!((items[1].item, items[1].pos), (Item::Coal, 0.4));
        }
        _ => panic!("not a belt"),
    }
    // The items keep moving on the new belt.
    run(&mut w, 5);
    assert!(w.tile(0, 10, 11).is_some_and(|b| matches!(&b.st, St::Belt { items } if !items.is_empty())));
}

#[test]
fn a_drill_keeps_its_ore_its_fuel_and_its_output() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Drill, Dir::W).unwrap();
    w.apply_setting(0, 10, 10, Setting::Mines(Item::IronOre));
    if let Some(Building { st: St::Drill { fuel, coal, out, .. }, .. }) = w.floors[0].tiles[idx(10, 10)].as_mut() {
        *fuel = 321;
        *coal = 3;
        out.push_back(Item::IronOre);
    }
    w.upgrade_area(0, ALL);
    assert_eq!(w.tile(0, 10, 10).unwrap().kind, Kind::BasicDrill);
    assert_eq!(w.tile(0, 10, 10).unwrap().dir, Dir::W);
    assert_eq!(drill(&w, 10, 10), (Some(Item::IronOre), 321, 3, 1));
}

#[test]
fn a_coal_drill_that_becomes_an_ec_drill_has_no_coal_left() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::LargeBasicDrill, Dir::E).unwrap();
    let before = w.econ.money;
    w.apply_setting(0, 10, 10, Setting::Mines(Item::Coal));
    if let Some(Building { st: St::Drill { fuel, coal, out, .. }, .. }) = w.floors[0].tiles[idx(10, 10)].as_mut() {
        *fuel = 99;
        *coal = 40;
        out.push_back(Item::Coal);
        out.push_back(Item::Coal);
    }
    assert!(Kind::LargeBasicDrill.burns_coal() && !Kind::LargeAdvancedDrill.burns_coal());
    w.upgrade_area(0, ALL);
    assert_eq!(w.tile(0, 10, 10).unwrap().kind, Kind::LargeAdvancedDrill);
    // It still mines coal, still holds what it had mined, but carries no fuel.
    assert_eq!(drill(&w, 10, 10), (Some(Item::Coal), 0, 0, 2));
    assert_eq!(w.econ.money, before - Kind::LargeBasicDrill.upgrade_cost(Kind::LargeAdvancedDrill));
}

#[test]
fn swapping_back_to_a_coal_drill_gives_it_a_coals_worth_of_fuel() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::LargeBasicDrill, Dir::E).unwrap();
    let (_, undo) = w.upgrade_area(0, ALL);
    let back = w.undo_upgrade(&undo.unwrap());
    assert_eq!(back.reverted, 1);
    let (_, fuel, coal, _) = drill(&w, 10, 10);
    assert_eq!(coal, 0);
    assert!(fuel > 0, "it can start again");
}

#[test]
fn coal_beyond_the_new_drills_tank_is_dropped() {
    // The tank only ever grows going up, so this is the clamp for pairs that may shrink it.
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Drill, Dir::E).unwrap();
    if let Some(Building { st: St::Drill { coal, .. }, .. }) = w.floors[0].tiles[idx(10, 10)].as_mut() {
        *coal = 9999;
    }
    w.upgrade_area(0, ALL);
    assert_eq!(drill(&w, 10, 10).2, Kind::BasicDrill.drill_coal_cap());
}

#[test]
fn an_assembler_keeps_its_recipe_by_name_and_what_it_holds() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::AdvancedAssembler, Dir::S).unwrap();
    let name = PRECISION_NAME;
    assert!(Kind::AdvancedAssembler.recipes().iter().any(|r| r.name == name), "{name} is in both lists");
    set_recipe(&mut w, 10, 10, name);
    let old_index = crafter(&w, 10, 10).0;
    set_stock(&mut w, 10, 10, Item::IronGear, 7);
    if let Some(Building { st: St::Crafter { progress, busy, out, fuel, .. }, .. }) = w.floors[0].tiles[idx(10, 10)].as_mut() {
        *progress = 5.0;
        *busy = true;
        *fuel = 12;
        out.push_back(Item::IronGear);
    }
    w.upgrade_area(0, ALL);
    let b = w.tile(0, 10, 10).unwrap();
    assert_eq!((b.kind, b.dir), (Kind::PrecisionAssembler, Dir::S));
    assert_eq!(b.recipe().unwrap().name, name);
    let (index, progress, busy, stock, out, fuel) = crafter(&w, 10, 10);
    // The two lists order their recipes differently, so the index moved but the name did not.
    assert_ne!(index, old_index);
    assert_eq!((progress, busy, stock, out, fuel), (0.0, false, 7, 1, 12));
}

/// A recipe both the advanced and the precision assembler have.
const PRECISION_NAME: &str = "control board";

#[test]
fn a_large_assembler_keeps_its_recipe_too() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::LargeBasicAssembler, Dir::E).unwrap();
    let name = Kind::LargeBasicAssembler.recipes().last().unwrap().name;
    set_recipe(&mut w, 10, 10, name);
    set_stock(&mut w, 10, 10, Item::IronGear, 3);
    w.upgrade_area(0, ALL);
    assert_eq!(w.tile(0, 10, 10).unwrap().kind, Kind::LargeAdvancedAssembler);
    assert_eq!(w.tile(0, 10, 10).unwrap().recipe().unwrap().name, name);
    assert_eq!(crafter(&w, 10, 10).3, 3);
}

#[test]
fn a_machine_whose_recipe_the_new_one_cannot_make_is_left_alone_and_counted() {
    let mut w = World::blank();
    // A new advanced assembler makes its first recipe, a crude one that the precision assembler lacks.
    w.place(0, 10, 10, Kind::AdvancedAssembler, Dir::E).unwrap();
    w.place(0, 20, 10, Kind::AdvancedAssembler, Dir::E).unwrap();
    set_recipe(&mut w, 20, 10, PRECISION_NAME);
    let first = w.tile(0, 10, 10).unwrap().recipe().unwrap().name;
    assert!(!Kind::PrecisionAssembler.recipes().iter().any(|r| r.name == first), "{first}");
    let before = w.econ.money;
    let plan = w.plan_upgrade(0, ALL);
    assert_eq!((plan.count(UpState::NoRecipe), plan.count(UpState::Fits)), (1, 1));
    let (rep, _) = w.upgrade_area(0, ALL);
    assert_eq!((rep.upgraded, rep.no_recipe), (1, 1));
    assert_eq!(w.tile(0, 10, 10).unwrap().kind, Kind::AdvancedAssembler);
    assert_eq!(w.tile(0, 10, 10).unwrap().recipe().unwrap().name, first);
    assert_eq!(w.tile(0, 20, 10).unwrap().kind, Kind::PrecisionAssembler);
    assert!((before - w.econ.money - Kind::AdvancedAssembler.upgrade_cost(Kind::PrecisionAssembler)).abs() < 1e-9, "only one is paid for");
}

#[test]
fn a_recipe_not_researched_in_the_new_version_blocks_the_swap() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::AdvancedAssembler, Dir::E).unwrap();
    set_recipe(&mut w, 10, 10, PRECISION_NAME);
    let needs = Kind::PrecisionAssembler.recipes().iter().find(|r| r.name == PRECISION_NAME).and_then(|r| r.needs);
    if let Some(t) = needs {
        w.progress.researched[t.idx()] = false;
        assert_eq!(w.plan_upgrade(0, ALL).count(UpState::NoRecipe), 1);
    }
}

#[test]
fn short_money_upgrades_a_prefix_in_reading_order() {
    let mut w = World::blank();
    // Belts cost $7 more each; two rows of five.
    for y in [10, 11] {
        for x in 10..15 {
            w.place(0, x, y, Kind::Belt, Dir::E).unwrap();
        }
    }
    let each = Kind::Belt.upgrade_cost(Kind::FastBelt);
    w.econ.money = each * 6.5;
    let plan = w.plan_upgrade(0, ALL);
    assert_eq!((plan.count(UpState::Fits), plan.count(UpState::Short)), (6, 4));
    assert!((plan.short_by(w.econ.money) - each * 3.5).abs() < 1e-9);
    let (rep, undo) = w.upgrade_area(0, ALL);
    assert_eq!((rep.upgraded, rep.short), (6, 4));
    assert!((rep.short_by - each * 3.5).abs() < 1e-9);
    // The whole first row and the first belt of the second.
    for x in 10..15 {
        assert_eq!(w.tile(0, x, 10).unwrap().kind, Kind::FastBelt, "{x}");
    }
    assert_eq!(w.tile(0, 10, 11).unwrap().kind, Kind::FastBelt);
    assert_eq!(w.tile(0, 11, 11).unwrap().kind, Kind::Belt);
    assert!((w.econ.money - each * 0.5).abs() < 1e-9);
    assert_eq!(undo.unwrap().len(), 6);
    // Earn more and press again: the rest is done, nothing is charged twice.
    w.econ.money = 1000.0;
    let (rep, _) = w.upgrade_area(0, ALL);
    assert_eq!((rep.upgraded, rep.short), (4, 0));
    assert!((w.econ.money - (1000.0 - each * 4.0)).abs() < 1e-9);
}

#[test]
fn the_cut_is_clean_a_cheaper_part_later_does_not_sneak_in() {
    let mut w = World::blank();
    // Reading order: an assembler (costly) first, belts after.
    w.place(0, 10, 10, Kind::LargeBasicAssembler, Dir::E).unwrap();
    w.place(0, 20, 10, Kind::Belt, Dir::E).unwrap();
    set_recipe(&mut w, 10, 10, Kind::LargeBasicAssembler.recipes().last().unwrap().name);
    w.econ.money = 100.0; // the assembler costs $18,000 more; a belt costs $7
    let (rep, _) = w.upgrade_area(0, ALL);
    assert_eq!((rep.upgraded, rep.short), (0, 2));
    assert_eq!(w.econ.money, 100.0);
}

#[test]
fn nothing_is_charged_for_a_part_that_cannot_change() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Belt, Dir::E).unwrap();
    w.place(0, 12, 10, Kind::FastBelt, Dir::E).unwrap();
    w.place(0, 14, 10, Kind::Crate, Dir::E).unwrap();
    w.place(0, 16, 10, Kind::Arm, Dir::E).unwrap();
    let before = w.econ.money;
    let (rep, _) = w.upgrade_area(0, (12, 0, 20, 20));
    assert_eq!((rep.upgraded, rep.short, rep.locked, rep.no_recipe), (0, 0, 0, 0));
    assert_eq!(w.econ.money, before);
    assert_eq!(w.plan_upgrade(0, ALL).parts.len(), 1, "only the basic belt has a next version");
}

#[test]
fn locked_targets_stay_as_they_are_and_cost_nothing() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Belt, Dir::E).unwrap();
    w.place(0, 12, 10, Kind::Drill, Dir::E).unwrap();
    w.place(0, 16, 10, Kind::Solar, Dir::E).unwrap();
    // Fast belts are not researched; the rest is.
    w.progress.researched[Tech::FastBelts.idx()] = false;
    let before = w.econ.money;
    let plan = w.plan_upgrade(0, ALL);
    assert_eq!((plan.count(UpState::Locked), plan.count(UpState::Fits)), (1, 2));
    assert_eq!(plan.locked_kind(), Some(Kind::FastBelt));
    let (rep, _) = w.upgrade_area(0, ALL);
    assert_eq!((rep.upgraded, rep.locked, rep.locked_kind), (2, 1, Some(Kind::FastBelt)));
    assert_eq!(w.tile(0, 10, 10).unwrap().kind, Kind::Belt);
    assert_eq!(w.tile(0, 12, 10).unwrap().kind, Kind::BasicDrill);
    assert_eq!(w.tile(0, 16, 10).unwrap().kind, Kind::BasicSolar);
    let paid = Kind::Drill.upgrade_cost(Kind::BasicDrill) + Kind::Solar.upgrade_cost(Kind::BasicSolar);
    assert!((before - w.econ.money - paid).abs() < 1e-9);

    // A company that has not opened locks its parts too.
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Belt, Dir::E).unwrap();
    w.progress.opened[Company::Swiftline.idx()] = false;
    assert_eq!(w.plan_upgrade(0, ALL).count(UpState::Locked), 1);
}

#[test]
fn a_locked_part_in_the_middle_does_not_stop_the_ones_after_it() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Drill, Dir::E).unwrap();
    w.place(0, 14, 10, Kind::Belt, Dir::E).unwrap();
    w.place(0, 16, 10, Kind::Solar, Dir::E).unwrap();
    w.progress.researched[Tech::FastBelts.idx()] = false;
    let (rep, _) = w.upgrade_area(0, ALL);
    assert_eq!((rep.upgraded, rep.locked), (2, 1));
}

#[test]
fn a_solar_panel_reaches_further_once_swapped() {
    let mut w = World::blank();
    w.place(0, 20, 20, Kind::Solar, Dir::E).unwrap();
    w.place(0, 20, 28, Kind::Pole, Dir::E).unwrap(); // beyond the cheap panel's reach
    let supply = |w: &World| w.grid.nets.iter().map(|n| n.supply).fold(0.0, f64::max);
    w.upgrade_area(0, ALL);
    assert_eq!(w.tile(0, 20, 20).unwrap().kind, Kind::BasicSolar);
    assert!(supply(&w) >= Kind::BasicSolar.solar_output(), "the grid was rebuilt with the new panel");
}

#[test]
fn undo_swaps_the_old_parts_back_and_pays_the_difference_back() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Belt, Dir::N).unwrap();
    w.place(0, 12, 10, Kind::Drill, Dir::W).unwrap();
    w.apply_setting(0, 12, 10, Setting::Mines(Item::Stone));
    w.place(0, 20, 10, Kind::AdvancedAssembler, Dir::E).unwrap();
    set_recipe(&mut w, 20, 10, PRECISION_NAME);
    let before = w.econ.money;
    let (rep, undo) = w.upgrade_area(0, ALL);
    assert_eq!(rep.upgraded, 3);
    let undo = undo.unwrap();
    // Something happens in between: the belt carries an item now.
    if let Some(Building { st: St::Belt { items }, .. }) = w.floors[0].tiles[idx(10, 10)].as_mut() {
        items.push(BeltItem { item: Item::Stone, pos: 0.3 });
    }
    let back = w.undo_upgrade(&undo);
    assert_eq!((back.reverted, back.kept), (3, 0));
    assert!((back.refund - rep.cost).abs() < 1e-9);
    assert!((w.econ.money - before).abs() < 1e-9, "whole again");
    assert_eq!(kinds_on(&w, 0), vec![Kind::Belt, Kind::Drill, Kind::AdvancedAssembler]);
    assert_eq!(w.tile(0, 10, 10).unwrap().dir, Dir::N);
    assert!(matches!(&w.tile(0, 10, 10).unwrap().st, St::Belt { items } if items.len() == 1), "what the belt carries stays");
    assert_eq!(drill(&w, 12, 10).0, Some(Item::Stone));
    assert_eq!(w.tile(0, 20, 10).unwrap().recipe().unwrap().name, PRECISION_NAME);
}

#[test]
fn undo_leaves_a_part_that_was_removed_or_replaced() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Belt, Dir::E).unwrap();
    w.place(0, 12, 10, Kind::Belt, Dir::E).unwrap();
    w.place(0, 14, 10, Kind::Belt, Dir::E).unwrap();
    let (_, undo) = w.upgrade_area(0, ALL);
    let undo = undo.unwrap();
    w.remove(0, 10, 10).unwrap();
    w.remove(0, 12, 10).unwrap();
    w.place(0, 12, 10, Kind::Crate, Dir::E).unwrap();
    let money = w.econ.money;
    let back = w.undo_upgrade(&undo);
    assert_eq!((back.reverted, back.kept), (1, 2));
    assert_eq!(w.tile(0, 14, 10).unwrap().kind, Kind::Belt);
    assert_eq!(w.tile(0, 12, 10).unwrap().kind, Kind::Crate);
    assert!((w.econ.money - money - Kind::Belt.upgrade_cost(Kind::FastBelt)).abs() < 1e-9);
}

#[test]
fn undo_keeps_a_precision_assembler_that_was_set_to_a_recipe_only_it_makes() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::AdvancedAssembler, Dir::E).unwrap();
    set_recipe(&mut w, 10, 10, PRECISION_NAME);
    let (_, undo) = w.upgrade_area(0, ALL);
    let only_precision = Kind::PrecisionAssembler
        .recipes()
        .iter()
        .find(|r| !Kind::AdvancedAssembler.recipes().iter().any(|a| a.name == r.name))
        .map(|r| r.name);
    let Some(only) = only_precision else { return };
    set_recipe(&mut w, 10, 10, only);
    let back = w.undo_upgrade(&undo.unwrap());
    assert_eq!((back.reverted, back.kept), (0, 1));
    assert_eq!(w.tile(0, 10, 10).unwrap().kind, Kind::PrecisionAssembler);
}

#[test]
fn upgrading_works_on_other_floors_and_only_there() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Belt, Dir::E).unwrap();
    w.place(1, 10, 10, Kind::Belt, Dir::E).unwrap();
    let (rep, undo) = w.upgrade_area(1, ALL);
    assert_eq!(rep.upgraded, 1);
    assert_eq!(undo.as_ref().map(|u| u.floor), Some(1));
    assert_eq!(w.tile(0, 10, 10).unwrap().kind, Kind::Belt);
    assert_eq!(w.tile(1, 10, 10).unwrap().kind, Kind::FastBelt);
}

#[test]
fn the_planner_tech_is_priced_as_the_plan_says() {
    let t = Tech::UpgradePlanner;
    assert_eq!((t.cost(), t.milestone()), (3000.0, (Made::Item(Item::Gearbox), 60)));
    assert_eq!((t.company(), t.name()), (Company::Swiftline, "Upgrade planner"));
    assert!(UNLOCK_GROUPS.iter().any(|g| g.0 == "Tools" && g.1.contains(&t)));
}

#[test]
fn a_large_advanced_assembler_becomes_a_large_precision_one_and_pays_the_difference() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::LargeAdvancedAssembler, Dir::E).unwrap();
    set_recipe(&mut w, 10, 10, "steam tractor");
    set_stock(&mut w, 10, 10, Item::Frame, 3);
    let before = w.econ.money;
    let (rep, undo) = w.upgrade_area(0, ALL);
    assert_eq!((rep.upgraded, rep.made.clone()), (1, vec![(Kind::LargePrecisionAssembler, 1)]));
    assert!((before - w.econ.money - 70_000.0).abs() < 1e-9, "$100,000 less the $30,000 it was");
    let b = w.tile(0, 10, 10).unwrap();
    assert_eq!((b.kind, b.recipe().unwrap().name), (Kind::LargePrecisionAssembler, "steam tractor"));
    assert_eq!(crafter(&w, 10, 10).3, 3, "what it holds comes along");
    // Nothing further to upgrade, and the swap undoes for the same money.
    assert_eq!(w.plan_upgrade(0, ALL).parts.len(), 0);
    w.undo_upgrade(&undo.unwrap());
    assert!((w.econ.money - before).abs() < 1e-9);
    assert_eq!(w.tile(0, 10, 10).unwrap().kind, Kind::LargeAdvancedAssembler);
}

#[test]
fn a_large_basic_assembler_climbs_one_step_a_press_and_a_large_basic_recipe_stops_at_the_second() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::LargeBasicAssembler, Dir::E).unwrap();
    set_recipe(&mut w, 10, 10, "engine block");
    w.upgrade_area(0, ALL);
    assert_eq!(w.tile(0, 10, 10).unwrap().kind, Kind::LargeAdvancedAssembler, "one step");
    // The large precision assembler does not make engine blocks: it is left as it is, and counted.
    let (rep, undo) = w.upgrade_area(0, ALL);
    assert_eq!((rep.upgraded, rep.no_recipe), (0, 1));
    assert!(undo.is_none());
    assert_eq!(w.tile(0, 10, 10).unwrap().kind, Kind::LargeAdvancedAssembler);
    // Set to something both make, it goes up.
    set_recipe(&mut w, 10, 10, "generator");
    let (rep, _) = w.upgrade_area(0, ALL);
    assert_eq!(rep.upgraded, 1);
    assert_eq!(w.tile(0, 10, 10).unwrap().kind, Kind::LargePrecisionAssembler);
}
