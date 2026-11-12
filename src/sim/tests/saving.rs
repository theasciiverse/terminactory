//! Save files: versions, damaged files, and a golden shape of the format.

use super::*;
use serde_json::{json, Value};

pub(super) fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tfac-saving-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(crate::saves::dir(&dir)).unwrap();
    dir
}

#[test]
fn a_newer_save_is_refused_with_its_own_message_and_left_alone() {
    let data = temp("newer");
    let path = crate::saves::path_of(&data, "save1");
    let text = format!(r#"{{"version":{},"floors":[]}}"#, SAVE_VERSION + 1);
    std::fs::write(&path, &text).unwrap();
    assert_eq!(World::file_version(&path), Some(SAVE_VERSION + 1));
    let err = World::load(&path).err().unwrap();
    assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
    assert!(err.to_string().contains("newer version") && !err.to_string().contains("older"), "{err}");
    let msg = crate::saves::version_problem(&path, "save1").unwrap();
    assert!(msg.contains("newer version") && msg.contains("Update the game") && msg.contains("tfac --new"), "{msg}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "never rewritten");
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn the_save_list_says_newer_version_for_a_newer_save() {
    let data = temp("list-newer");
    std::fs::write(crate::saves::path_of(&data, "save1"), format!(r#"{{"version":{}}}"#, SAVE_VERSION + 1)).unwrap();
    let list = crate::saves::describe(&data);
    assert!(list.contains("(newer version)"), "{list}");
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn damaged_files_are_errors_not_panics() {
    let data = temp("damaged");
    let mut w = World::new();
    let good = crate::saves::path_of(&data, "save1");
    w.save(&good).unwrap();
    let json = std::fs::read_to_string(&good).unwrap();
    let cases = [
        ("empty", String::new()),
        ("text", "this is not a save".to_string()),
        ("truncated", json[..json.len() / 2].to_string()),
        ("wrong type", format!(r#"{{"version":{SAVE_VERSION},"floors":"none"}}"#)),
        ("array", "[1,2,3]".to_string()),
    ];
    let mut paths = Vec::new();
    for (n, (name, text)) in cases.iter().enumerate() {
        let path = crate::saves::path_of(&data, &format!("save{}", n + 2));
        std::fs::write(&path, text).unwrap();
        assert!(World::load(&path).is_err(), "{name} should not load");
        assert_eq!(&std::fs::read_to_string(&path).unwrap(), text, "{name} is left as it was");
        paths.push((name, path));
    }
    // Not JSON at all, or cut off: no readable version, so no version message either.
    for (name, path) in &paths {
        let readable = *name == &"wrong type";
        assert_eq!(World::file_version(path).is_some(), readable, "{name}");
        assert!(crate::saves::version_problem(path, "x").is_none(), "{name}");
    }
    assert!(World::load(&data.join("saves").join("missing.json")).is_err());
    // The list shows the good one normally and the rest as unreadable, without panicking.
    let list = crate::saves::describe(&data);
    assert!(list.lines().any(|l| l.trim_start().starts_with("save1") && l.contains("research")), "{list}");
    assert_eq!(list.matches("(can't be read)").count(), cases.len(), "{list}");
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn a_saved_recipe_past_the_end_of_the_list_falls_back_to_the_first() {
    let mut w = World::blank();
    w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
    if let Some(Building { st: St::Crafter { recipe, .. }, .. }) = w.floors[0].tiles[idx(5, 5)].as_mut() {
        *recipe = 200;
    }
    let path = std::env::temp_dir().join(format!("factory-test-recipe-{}.json", std::process::id()));
    w.save(&path).unwrap();
    let mut loaded = World::load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(matches!(&loaded.tile(0, 5, 5).unwrap().st, St::Crafter { recipe: 0, .. }));
    run(&mut loaded, 1); // and it doesn't panic when it ticks
}

/// The structure of a JSON value with the data left out: object keys, what kind of value each is,
/// and for arrays their length and the distinct shapes of their elements. Strings keep their text,
/// since that is how enum variants (kinds, directions, items) are saved.
fn shape(v: &Value) -> Value {
    match v {
        Value::Null => json!("null"),
        Value::Bool(_) => json!("bool"),
        Value::Number(n) => json!(if n.is_f64() { "float" } else { "int" }),
        Value::String(s) => json!(format!("\"{s}\"")),
        Value::Array(a) => {
            let mut of: Vec<String> = a.iter().map(|e| shape(e).to_string()).collect();
            of.sort();
            of.dedup();
            let of: Vec<Value> = of.iter().map(|s| serde_json::from_str(s).unwrap()).collect();
            json!({ "len": a.len(), "of": of })
        }
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), shape(v))).collect()),
    }
}

/// New parts and techs only add names to the lists a save already writes, so a save made before the
/// large kiln and the basic solar panel (which names neither) opens in a build that has them.
#[test]
fn a_save_that_names_none_of_the_newest_parts_still_opens() {
    let mut w = World::new();
    w.place(0, 60, 50, Kind::Solar, Dir::E).unwrap();
    w.progress.researched[Tech::Kiln.idx()] = true;
    let data = temp("older-content");
    let path = crate::saves::path_of(&data, "save1");
    w.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        !text.contains("LargeKiln") && !text.contains("BasicSolar") && !text.contains("Blueprints"),
        "an older save has none of the names"
    );
    assert!(text.contains(&format!("\"version\": {SAVE_VERSION}")) || text.contains(&format!("\"version\":{SAVE_VERSION}")));
    let l = World::load(&path).unwrap();
    assert!(l.progress.has(Tech::Kiln) && !l.progress.has(Tech::LargeKiln) && !l.progress.has(Tech::BasicSolar));
    assert!(!l.progress.has(Tech::Blueprints));
    // Nor the tube: no Sablon, no evacuator, no tube items, and the file is still format 6.
    assert!(!text.contains("Sablon") && !text.contains("Evacuator") && !text.contains("\"Tube\""));
    assert!(!l.progress.is_open(Company::Sablon) && !l.progress.has(Tech::Evacuator));
    // Nor silicon: no Halverson, no silicon items or molten silicon, and still format 6.
    assert!(!text.contains("Halverson") && !text.contains("MoltenSilicon") && !text.contains("Silicon") && !text.contains("Wafer"));
    assert!(!l.progress.is_open(Company::Halverson));
    // Nor chips: no Meridian, no etcher, no etchant, masks, dies or chips, and still format 6.
    assert!(!text.contains("Meridian") && !text.contains("Etcher") && !text.contains("Etchant") && !text.contains("Microprocessor"));
    assert!(!text.contains("\"Mask\"") && !text.contains("\"Die\"") && !text.contains("RamChip"));
    assert!(!l.progress.is_open(Company::Meridian) && !l.progress.has(Tech::Etcher));
    // Nor the better silicon machines: no arc furnace, crystal grower, wafer saw or silicon crystal.
    assert!(
        !text.contains("ArcFurnace") && !text.contains("CrystalGrower") && !text.contains("WaferSaw") && !text.contains("SiliconCrystal")
    );
    assert!(!l.progress.has(Tech::ArcFurnace) && !l.progress.has(Tech::CrystalGrower) && !l.progress.has(Tech::WaferSaw));
    // Nor fine etching: no fine etcher, fine mask, fine die or x4x2, and still format 6.
    assert!(
        !text.contains("FineEtcher") && !text.contains("FineMask") && !text.contains("FineDie") && !text.contains("FineMicroprocessor")
    );
    assert!(!l.progress.has(Tech::FineEtcher));
    assert_eq!(l.tile(0, 60, 50).map(|b| b.kind), Some(Kind::Solar));
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn molten_silicon_in_a_pipe_survives_a_save_by_name() {
    let mut w = World::blank();
    w.place(0, 20, 20, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 21, 20, Kind::Pipe, Dir::E).unwrap();
    let pool = w.fluids.pipe_pool[0][idx(20, 20)] as usize;
    w.fluids.pools[pool].add(Fluid::MoltenSilicon, 3.0);
    let data = temp("silicon-pipe");
    let path = crate::saves::path_of(&data, "save1");
    w.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("MoltenSilicon"), "the fluid is saved by name");
    assert!(text.contains(&format!("\"version\":{SAVE_VERSION}")) || text.contains(&format!("\"version\": {SAVE_VERSION}")));
    let l = World::load(&path).unwrap();
    let pool = l.fluids.pipe_pool[0][idx(21, 20)] as usize;
    assert_eq!((l.fluids.pools[pool].fluid, l.fluids.pools[pool].amount), (Some(Fluid::MoltenSilicon), 3.0));
    let _ = std::fs::remove_dir_all(&data);
}

/// A world with one of every part, an elevator car, mail and some goods in transit.
pub(super) fn everything() -> World {
    let mut w = World::blank();
    for (n, kind) in Kind::ALL.into_iter().enumerate() {
        let (x, y) = (4 + 9 * (n as i32 % 10), 4 + 9 * (n as i32 / 10));
        w.place_sized(0, x, y, kind, Dir::E, 2).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        if matches!(kind, Kind::Arm | Kind::LongArm | Kind::Sorter) {
            w.configure(0, x, y);
        }
    }
    w.place(0, 100, 5, Kind::Crate, Dir::E).unwrap();
    fill_crate(&mut w, 0, 100, 5, &[Item::Coal, Item::Waste]);
    if let Some(Building { st: St::Belt { items }, .. }) = w.floors[0].tiles[idx(4 + 9 * 3, 4)].as_mut() {
        items.push(BeltItem { item: Item::Stone, pos: 0.5 });
    }
    w.lifts[0].load.push_back(Item::Coal);
    w.fluids.pools[0].add(Fluid::Water, 5.0);
    w.receive(crate::mail::parse("from: tests\nevent: finished\nhello", 0), 1_000);
    w.saved_at = 1_000;
    w.sample_history(1_000, 0);
    w.econ.earned_money += 5.0;
    w.sample_history(1_001, 0);
    // Every per-item table has a count (the tables leave out zeros, so an empty one shows no shape).
    w.econ.made[Item::Coal] = 3;
    w.econ.sold[Item::Coal] = 2;
    w.econ.research_sales[Item::Coal] = 1;
    w.sale_rate[Item::Coal] = 0.5;
    for b in w.floors.iter_mut().flat_map(|fl| fl.tiles.iter_mut().flatten()) {
        if let St::Crafter { stock, .. } = &mut b.st {
            stock[Item::Coal] = 4;
        }
    }
    w
}

/// Changing any saved struct changes this shape. If it fails on purpose, bump `SAVE_VERSION`
/// (old saves can't be read by the new layout) and regenerate the file with `UPDATE_GOLDEN=1 cargo test save_format`.
#[test]
fn save_format_matches_the_golden_shape() {
    let mut w = everything();
    let path = std::env::temp_dir().join(format!("factory-test-shape-{}.json", std::process::id()));
    w.save(&path).unwrap();
    let mut saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let _ = std::fs::remove_file(&path);
    // The game version changes with every release; only the fact that it is there matters.
    assert_eq!(saved["game"], env!("CARGO_PKG_VERSION"));
    saved["game"] = json!("<game version>");
    let now = serde_json::to_string_pretty(&shape(&saved)).unwrap() + "\n";
    let golden = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/save-shape.json");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&golden, &now).unwrap();
    }
    let old = std::fs::read_to_string(&golden).expect("tests/fixtures/save-shape.json (UPDATE_GOLDEN=1 writes it)");
    let version = saved["version"].as_u64().unwrap();
    assert!(
        old == now,
        "the save format changed while SAVE_VERSION is {version}. If that is on purpose, bump SAVE_VERSION and run: UPDATE_GOLDEN=1 cargo test save_format"
    );
}

const DAY0: u64 = 1_794_398_400; // 2026-11-11 12:00 UTC

fn history_days(w: &World, now: u64) -> f64 {
    w.history.series(crate::history::Range::Days, now, 0).iter().map(|p| p.money).sum()
}

#[test]
fn company_deliveries_are_left_out_of_the_history() {
    let mut w = World::blank();
    w.sample_history(DAY0, 0);
    w.receive(crate::mail::parse("from: tests\nevent: finished\nhello", 0), DAY0);
    assert!(w.gifted > 0.0, "a delivery came with it");
    w.sample_history(DAY0 + 1, 0);
    assert_eq!(history_days(&w, DAY0 + 1), 0.0);
    // Real earnings still count, whatever the delivery did to the lifetime total.
    w.econ.earned_money += 12.0;
    w.sample_history(DAY0 + 2, 0);
    assert_eq!(history_days(&w, DAY0 + 2), 12.0);
    // A delivery in the same second as a sale doesn't hide it, or add to it.
    w.econ.earned_money += 3.0;
    w.gifted += 40.0;
    w.econ.earned_money += 40.0;
    w.sample_history(DAY0 + 3, 0);
    assert_eq!(history_days(&w, DAY0 + 3), 15.0);
}

#[test]
fn selling_shows_up_in_the_history() {
    let mut w = World::blank();
    w.sample_history(DAY0, 0);
    w.econ.sell(Item::CopperPlate, SellMode::Money);
    w.econ.sell(Item::CopperPlate, SellMode::Research);
    w.sample_history(DAY0 + 1, 0);
    let today = *w.history.series(crate::history::Range::Days, DAY0 + 1, 0).last().unwrap();
    assert_eq!(today.money, Item::CopperPlate.price());
    assert!(today.research > 0.0);
}

#[test]
fn offline_earnings_go_in_the_history_when_applied() {
    let mut w = World::new();
    w.rate = (2.0, 0.0);
    w.rate_long = (2.0, 0.0);
    w.saved_at = DAY0 - 3600;
    let (_, m, _) = w.apply_offline(DAY0).unwrap();
    assert_eq!(m, 7200.0);
    // (It is applied on the local clock, whatever zone the machine is in.)
    let off = crate::history::local_offset(DAY0);
    let at = |r| w.history.series(r, DAY0, off);
    assert_eq!(at(crate::history::Range::Days).iter().map(|p| p.money).sum::<f64>(), 7200.0);
    assert_eq!(at(crate::history::Range::Hours)[47].money, 7200.0);
    assert_eq!(at(crate::history::Range::Minutes)[59].money, 0.0);
}

#[test]
fn history_survives_a_save_and_load() {
    let data = temp("history");
    let path = crate::saves::path_of(&data, "save1");
    let mut w = everything();
    w.save(&path).unwrap();
    let back = World::load(&path).unwrap();
    assert_eq!((&back.history.minutes, &back.history.hours, &back.history.days), (&w.history.minutes, &w.history.hours, &w.history.days));
    assert_eq!(history_days(&back, 1_001), 5.0);
    assert!(!back.history.is_empty());
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn a_current_save_without_history_still_loads() {
    let data = temp("no-history");
    let path = crate::saves::path_of(&data, "save1");
    let mut w = everything();
    w.save(&path).unwrap();
    let mut json: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(json.as_object_mut().unwrap().remove("history").is_some(), "the save has the field");
    assert_eq!(json["version"], SAVE_VERSION);
    std::fs::write(&path, json.to_string()).unwrap();
    let back = World::load(&path).unwrap();
    assert!(back.history.is_empty());
    assert_eq!(back.econ.earned_money, w.econ.earned_money);
    let _ = std::fs::remove_dir_all(&data);
}
