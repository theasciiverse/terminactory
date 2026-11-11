//! Saves by name: item tables, flags and recipes are found by what they are called, never by position.

use super::saving::{everything, temp};
use super::*;
use serde_json::{json, Value};

fn save_json(w: &mut World, data: &std::path::Path, name: &str) -> (std::path::PathBuf, Value) {
    let path = crate::saves::path_of(data, name);
    w.save(&path).unwrap();
    let json = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    (path, json)
}

fn load_json(path: &std::path::Path, json: &Value) -> std::io::Result<World> {
    std::fs::write(path, json.to_string()).unwrap();
    World::load(path)
}

fn crafter_at(w: &World, x: i32, y: i32) -> (usize, PerItem<u32>) {
    match &w.tile(0, x, y).unwrap().st {
        St::Crafter { recipe, stock, .. } => (*recipe as usize, stock.clone()),
        _ => panic!("not a machine"),
    }
}

fn set_recipe(w: &mut World, x: i32, y: i32, n: usize) {
    if let Some(Building { st: St::Crafter { recipe, .. }, .. }) = w.floors[0].tiles[idx(x, y)].as_mut() {
        *recipe = n as u8;
    }
}

fn pool_total(w: &World) -> f64 {
    w.fluids.pools.iter().map(|p| p.amount).sum()
}

fn recipe_name_at(w: &World, x: i32, y: i32) -> &'static str {
    w.tile(0, x, y).unwrap().recipe().unwrap().name
}

/// Saves name items, companies and techs, so their order may change; what must hold is that
/// every name stands for exactly one entry and every entry has its own slot in the run-time tables.
#[test]
fn item_company_and_tech_names_are_unique_and_have_their_own_slot() {
    fn check<T: std::fmt::Debug + Copy>(what: &str, all: &[T], idx: fn(T) -> usize) {
        let mut names: Vec<String> = all.iter().map(|v| format!("{v:?}")).collect();
        let mut slots: Vec<usize> = all.iter().map(|v| idx(*v)).collect();
        names.sort();
        names.dedup();
        slots.sort();
        slots.dedup();
        assert_eq!(names.len(), all.len(), "two {what} share a name");
        assert_eq!(slots, (0..all.len()).collect::<Vec<_>>(), "{what} slots are not 0..n, each once");
    }
    check("items", &Item::ALL, Item::idx);
    check("companies", &Company::ALL, Company::idx);
    check("techs", &Tech::ALL, Tech::idx);
}

#[test]
fn per_item_tables_are_written_and_read_by_item_name() {
    let mut w = World::blank();
    w.econ.made[Item::IronOre] = 5;
    w.econ.made[Item::SteamTractor.idx()] = 9;
    let json = serde_json::to_value(&w.econ).unwrap();
    assert_eq!(json["made"], json!({"IronOre": 5, "SteamTractor": 9}), "names, with the zeros left out");
    // Written by hand in any order, with items left out: each lands on its own item.
    let e: Econ =
        serde_json::from_str(r#"{"money":1.0,"research":0.0,"made":{"SteamTractor":9,"IronOre":5},"sold":{"Coal":2},"research_sales":{}}"#)
            .unwrap();
    assert_eq!((e.made[Item::IronOre], e.made[Item::SteamTractor], e.made[Item::Coal]), (5, 9, 0));
    assert_eq!((e.sold[Item::Coal], e.sold[Item::IronOre]), (2, 0));
    assert!(e.research_sales.iter().all(|n| *n == 0));
}

/// The flag lists are names too, so the order of the companies or techs can't matter. A list in
/// the reverse order of `ALL` (what a reorder would look like to the code) reads the same.
#[test]
fn flag_lists_do_not_depend_on_the_order_of_the_lists() {
    let mut p = Progress::default();
    p.researched[Tech::Press.idx()] = true;
    p.researched[Tech::ALL[TECHS - 1].idx()] = true;
    let json = serde_json::to_value(&p).unwrap();
    let techs: Vec<&str> = json["researched"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(techs.len(), 2);
    assert!(techs.contains(&"Press") && techs.contains(&format!("{:?}", Tech::ALL[TECHS - 1]).as_str()));
    // The helpers take the list and each entry's slot, and give the same flags for a reversed list.
    let mut reversed = Tech::ALL;
    reversed.reverse();
    let names = json["researched"].clone();
    let flags = serde_util::de_set(names.clone(), &reversed, Tech::idx, "tech").unwrap();
    assert_eq!(flags, p.researched);
    let back = serde_util::ser_set(&flags, &reversed, Tech::idx, serde_json::value::Serializer).unwrap();
    let mut a = back.as_array().unwrap().clone();
    let mut b = names.as_array().unwrap().clone();
    a.sort_by_key(|v| v.to_string());
    b.sort_by_key(|v| v.to_string());
    assert_eq!(a, b);
}

#[test]
fn an_unknown_item_company_or_tech_name_refuses_the_save_and_leaves_the_file() {
    let data = temp("unknown-names");
    let mut w = World::blank();
    let (path, good) = save_json(&mut w, &data, "save1");
    assert!(World::load(&path).is_ok());
    type Edit = Box<dyn Fn(&mut Value)>;
    let cases: Vec<(&str, &str, Edit)> = vec![
        ("item", "Unobtainium", Box::new(|j| j["econ"]["made"]["Unobtainium"] = json!(3))),
        ("item", "Moonrock", Box::new(|j| j["sale_rate"]["Moonrock"] = json!(1.5))),
        ("company", "Nowhere Ltd", Box::new(|j| j["progress"]["opened"] = json!(["Nowhere Ltd"]))),
        ("tech", "Teleport", Box::new(|j| j["progress"]["researched"] = json!(["Press", "Teleport"]))),
    ];
    for (what, name, edit) in cases {
        let mut bad = good.clone();
        edit(&mut bad);
        let text = bad.to_string();
        std::fs::write(&path, &text).unwrap();
        let err = World::load(&path).err().unwrap_or_else(|| panic!("an unknown {what} loaded"));
        assert!(err.to_string().contains(name), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "{what}: the file is left as it was");
    }
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn a_company_that_is_open_from_the_start_is_open_even_if_the_save_forgot_it() {
    let data = temp("keyless");
    let mut w = World::new();
    let (path, mut json) = save_json(&mut w, &data, "save1");
    json["progress"]["opened"] = json!([]);
    let l = load_json(&path, &json).unwrap();
    for c in Company::ALL {
        assert_eq!(l.progress.is_open(c), c.key_item().is_none(), "{c:?}");
    }
    let _ = std::fs::remove_dir_all(&data);
}

/// A few made-up recipes for a molder, to reorder and extend.
const fn test_recipe(name: &'static str, tag: char) -> Recipe {
    Recipe {
        name,
        inputs: &[(Item::Coal, 1)],
        outputs: &[(Item::Coke, 1)],
        fluid_in: None,
        fluid_out: None,
        ticks: 60,
        fuel: None,
        needs: None,
        tag,
    }
}
static LIST_ABC: [Recipe; 3] = [test_recipe("alpha", 'A'), test_recipe("beta", 'B'), test_recipe("gamma", 'C')];
/// The same machine after an update: a recipe added in front and the others shuffled.
static LIST_NEW: [Recipe; 4] = [test_recipe("zeta", 'Z'), test_recipe("gamma", 'C'), test_recipe("alpha", 'A'), test_recipe("beta", 'B')];
static LIST_SHORT: [Recipe; 1] = [test_recipe("alpha", 'A')];

#[test]
fn a_recipe_is_saved_by_name_so_a_reordered_or_extended_list_changes_nothing() {
    let data = temp("recipe-reorder");
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &LIST_ABC))));
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Molder, Dir::E).unwrap();
    w.place(0, 10, 5, Kind::Molder, Dir::E).unwrap();
    set_recipe(&mut w, 5, 5, 2);
    set_recipe(&mut w, 10, 5, 1);
    let (path, json) = save_json(&mut w, &data, "save1");
    let machine = &json["floors"][0]["tiles"][idx(5, 5)]["st"]["Crafter"];
    assert_eq!(machine["recipe_name"], "gamma");
    assert!(machine.get("recipe").is_none(), "the index itself is not saved");
    // The game is updated: the list now starts with a new recipe and the old ones moved.
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &LIST_NEW))));
    let l = World::load(&path).unwrap();
    assert_eq!(recipe_name_at(&l, 5, 5), "gamma");
    assert_eq!(recipe_name_at(&l, 10, 5), "beta");
    assert_eq!(crafter_at(&l, 5, 5).0, 1, "gamma is the second recipe now");
    assert!(l.progress.notes.is_empty(), "nothing was lost");
    RECIPE_OVERRIDE.with(|o| o.set(None));
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn a_recipe_that_is_gone_falls_back_to_the_first_with_a_note_and_does_not_panic() {
    let data = temp("recipe-gone");
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &LIST_ABC))));
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Molder, Dir::E).unwrap();
    w.place(0, 10, 5, Kind::Molder, Dir::E).unwrap();
    set_recipe(&mut w, 5, 5, 2); // gamma
    set_recipe(&mut w, 10, 5, 0); // alpha
    let (path, json) = save_json(&mut w, &data, "save1");
    // The update removed gamma.
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &LIST_SHORT))));
    let mut l = World::load(&path).unwrap();
    assert_eq!(recipe_name_at(&l, 5, 5), "alpha", "the first recipe");
    assert_eq!(recipe_name_at(&l, 10, 5), "alpha");
    assert_eq!(l.progress.notes.len(), 1, "{:?}", l.progress.notes);
    assert!(l.progress.notes[0].contains("gamma") && l.progress.notes[0].contains("5,5"), "{:?}", l.progress.notes);
    run(&mut l, 2);
    // A name that never existed, and a machine whose name is missing from the file.
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &LIST_ABC))));
    let mut bad = json.clone();
    bad["floors"][0]["tiles"][idx(5, 5)]["st"]["Crafter"]["recipe_name"] = json!("no such recipe");
    bad["floors"][0]["tiles"][idx(10, 5)]["st"]["Crafter"].as_object_mut().unwrap().remove("recipe_name");
    let mut l = load_json(&path, &bad).unwrap();
    assert_eq!((recipe_name_at(&l, 5, 5), recipe_name_at(&l, 10, 5)), ("alpha", "alpha"));
    assert_eq!(l.progress.notes.len(), 2);
    run(&mut l, 2);
    RECIPE_OVERRIDE.with(|o| o.set(None));
    let _ = std::fs::remove_dir_all(&data);
}

/// Saved recipes are found by name, so every machine's names have to be different (the merged
/// assembler lists included) and not empty (an empty name is what "none saved" looks like).
#[test]
fn recipe_names_are_unique_per_kind() {
    for kind in Kind::ALL {
        let mut names: Vec<&str> = kind.recipes().iter().map(|r| r.name).collect();
        assert!(names.iter().all(|n| !n.is_empty()), "{kind:?} has a recipe with no name");
        let n = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), n, "{kind:?} has two recipes with one name");
    }
}

#[test]
fn every_real_recipe_survives_a_save_and_load() {
    let data = temp("every-recipe");
    let machines: Vec<Kind> = Kind::ALL.into_iter().filter(|k| !k.recipes().is_empty()).collect();
    let mut w = World::blank();
    let mut at = Vec::new();
    for (n, kind) in machines.iter().enumerate() {
        let (x, y) = (4 + 9 * (n as i32 % 10), 4 + 9 * (n as i32 / 10));
        w.place(0, x, y, *kind, Dir::E).unwrap();
        for r in 0..kind.recipes().len() {
            set_recipe(&mut w, x, y, r);
            let (path, _) = save_json(&mut w, &data, "save1");
            let l = World::load(&path).unwrap();
            assert_eq!(recipe_name_at(&l, x, y), kind.recipes()[r].name, "{kind:?} recipe {r}");
            assert_eq!(crafter_at(&l, x, y).0, r);
        }
        set_recipe(&mut w, x, y, kind.recipes().len() - 1);
        at.push((x, y, *kind));
    }
    // All of them at once, on their last recipe.
    let (path, _) = save_json(&mut w, &data, "save2");
    let l = World::load(&path).unwrap();
    for (x, y, kind) in at {
        assert_eq!(recipe_name_at(&l, x, y), kind.recipes().last().unwrap().name, "{kind:?}");
    }
    assert!(l.progress.notes.is_empty());
    let _ = std::fs::remove_dir_all(&data);
}

/// Saves a world, loads it, saves the load, and compares: every part, item table, flag, recipe,
/// pipe and mail entry has to come back exactly as it went in.
#[test]
fn a_world_with_every_part_round_trips_exactly() {
    let data = temp("everything");
    let mut w = everything();
    // Machines on their last recipe, with something in stock; the flags and tables at their ends.
    for (n, b) in w.floors[0].tiles.iter_mut().flatten().enumerate() {
        let list = b.kind.recipes();
        if let St::Crafter { recipe, stock, fluid, .. } = &mut b.st {
            *recipe = (list.len() - 1) as u8;
            stock[Item::ALL[n % ITEMS]] = n as u32 + 1;
            *fluid = 2;
        }
    }
    let last = Item::ALL[ITEMS - 1];
    w.econ.made[last] = 7;
    w.econ.sold[last] = 5;
    w.econ.research_sales[last] = 3;
    w.sale_rate[last] = 0.25;
    let up = w.floor_of(1).unwrap();
    w.place(up, 20, 20, Kind::Pipe, Dir::E).unwrap();
    w.place(up, 21, 20, Kind::Pipe, Dir::E).unwrap();
    let pool = w.fluids.pipe_pool[up][idx(20, 20)] as usize;
    w.fluids.pools[pool].add(Fluid::MoltenSteel, 12.5);
    w.progress.researched = [false; TECHS];
    w.progress.researched[Tech::ALL[TECHS - 1].idx()] = true;
    // (Companies that are open from the start are always open after a load.)
    w.progress.opened = Company::ALL.map(|c| c.key_item().is_none());
    let closed = Company::ALL.into_iter().find(|c| c.key_item().is_some() && *c != Company::ALL[COMPANIES - 1]).unwrap();
    w.progress.opened[Company::ALL[COMPANIES - 1].idx()] = true;
    let (path, mut first) = save_json(&mut w, &data, "save1");
    let mut l = World::load(&path).unwrap();
    let (_, mut second) = save_json(&mut l, &data, "save2");
    // The time of saving is the only thing that may differ.
    first["saved_at"] = json!(0);
    second["saved_at"] = json!(0);
    for (key, v) in first.as_object().unwrap() {
        assert!(v == &second[key], "{key} changed in a save, load, save round trip");
    }
    assert_eq!(first.as_object().unwrap().len(), second.as_object().unwrap().len());
    // And the loaded world really holds it, not just its file.
    assert_eq!((l.econ.made[last], l.econ.sold[last], l.econ.research_sales[last], l.sale_rate[last]), (7, 5, 3, 0.25));
    assert!(l.progress.has(Tech::ALL[TECHS - 1]) && !l.progress.has(Tech::Press));
    assert!(l.progress.is_open(Company::ALL[COMPANIES - 1]) && !l.progress.is_open(closed));
    let pool = l.fluids.pipe_pool[up][idx(21, 20)] as usize;
    assert_eq!((l.fluids.pools[pool].fluid, l.fluids.pools[pool].amount), (Some(Fluid::MoltenSteel), 12.5));
    for (a, b) in w.floors[0].tiles.iter().zip(&l.floors[0].tiles) {
        let (Some(a), Some(b)) = (a, b) else {
            assert!(a.is_none() && b.is_none());
            continue;
        };
        assert_eq!(a.kind, b.kind);
        if let (St::Crafter { recipe: ra, stock: sa, fluid: fa, .. }, St::Crafter { recipe: rb, stock: sb, fluid: fb, .. }) = (&a.st, &b.st)
        {
            assert_eq!((ra, sa, fa), (rb, sb, fb), "{:?}", a.kind);
        }
    }
    assert_eq!(l.game, env!("CARGO_PKG_VERSION"));
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn saving_leaves_the_live_world_alone() {
    let data = temp("live");
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Molder, Dir::E).unwrap();
    w.place(0, 10, 10, Kind::Pipe, Dir::E).unwrap();
    w.fluids.pools[0].add(Fluid::Water, 4.0);
    save_json(&mut w, &data, "save1");
    assert!(w.pipe_fluid.is_empty(), "the stamp for the file is cleared");
    assert_eq!(w.fluids.pools[0].amount, 4.0);
    assert_eq!(w.fluids.pools[0].fluid, Some(Fluid::Water));
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn separate_pipe_networks_keep_their_own_fluid_across_floors() {
    let data = temp("pipes");
    let mut w = World::blank();
    let down = w.floor_of(-1).unwrap();
    let runs = [(0usize, 10), (0, 30), (down, 10)];
    for (f, y) in runs {
        for x in 10..14 {
            w.place(f, x, y, Kind::Pipe, Dir::E).unwrap();
        }
    }
    w.place(0, 10, 50, Kind::Pipe, Dir::E).unwrap(); // an empty network is not saved
    let fluids = [Fluid::Water, Fluid::MoltenIron, Fluid::Steam];
    for ((f, y), fluid) in runs.iter().zip(fluids) {
        let p = w.fluids.pipe_pool[*f][idx(10, *y)] as usize;
        w.fluids.pools[p].add(fluid, 33.0);
    }
    let (path, json) = save_json(&mut w, &data, "save1");
    assert_eq!(json["pipe_fluid"].as_array().unwrap().len(), 3);
    let l = World::load(&path).unwrap();
    for ((f, y), fluid) in runs.iter().zip(fluids) {
        let p = l.fluids.pipe_pool[*f][idx(12, *y)] as usize;
        assert_eq!((l.fluids.pools[p].fluid, l.fluids.pools[p].amount), (Some(fluid), 33.0), "floor {f} row {y}");
    }
    let empty = l.fluids.pipe_pool[0][idx(10, 50)] as usize;
    assert_eq!((l.fluids.pools[empty].fluid, l.fluids.pools[empty].amount), (None, 0.0));
    assert_eq!(pool_total(&l), 99.0);
    // Fluid saved for a pipe that is off the map is skipped, and too much is held to the pipes' room.
    let mut bad = json.clone();
    bad["pipe_fluid"][0]["x"] = json!(9999);
    bad["pipe_fluid"][1]["amount"] = json!(1e9);
    let l = load_json(&path, &bad).unwrap();
    assert!(pool_total(&l) <= 33.0 + 4.0 * PIPE_CAP);
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn the_game_version_is_saved_and_refreshed() {
    let data = temp("game");
    let mut w = World::blank();
    assert_eq!(w.game, env!("CARGO_PKG_VERSION"));
    w.game = "0.0.1".into(); // loaded from a save by an older build, say
    let (path, json) = save_json(&mut w, &data, "save1");
    assert_eq!(json["game"], env!("CARGO_PKG_VERSION"));
    assert!(World::load(&path).is_ok());
    let _ = std::fs::remove_dir_all(&data);
}

/// What version 4 wrote: item arrays by position, a recipe number, floor deposits.
const V4_SAVE: &str = r#"{"version":4,"floors":[{"level":0,"deposits":["None"],"dug":[true],"tiles":[null],"owner":[null]}],
"econ":{"money":5.0,"research":0.0,"research_sales":[0,1],"sold":[0],"made":[3,2,1]},
"progress":{"opened":[true,false],"researched":[false,true]},"tick":9}"#;

#[test]
fn a_version_4_save_is_refused_listed_as_old_and_never_touched() {
    let data = temp("v4");
    let path = crate::saves::path_of(&data, "save1");
    std::fs::write(&path, V4_SAVE).unwrap();
    assert_eq!(World::file_version(&path), Some(4));
    let err = World::load(&path).err().unwrap();
    assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
    assert!(err.to_string().contains("older version"), "{err}");
    let msg = crate::saves::version_problem(&path, "save1").unwrap();
    assert!(msg.contains("older version") && msg.contains("tfac --new"), "{msg}");
    assert!(crate::saves::describe(&data).contains("(old version)"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), V4_SAVE, "never rewritten or deleted");
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn a_building_stays_small() {
    // The crafter's item table is boxed, so the biggest part is no bigger than the others by much.
    // (Every tile of every floor holds an Option<Building>.)
    let size = std::mem::size_of::<Building>();
    assert!(size <= 128, "Building grew to {size} bytes: box the large state");
    assert_eq!(std::mem::size_of::<Option<Building>>(), size);
}
