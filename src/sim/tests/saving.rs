//! Save files: versions, damaged files, and a golden shape of the format.

use super::*;
use serde_json::{json, Value};

fn temp(name: &str) -> std::path::PathBuf {
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

/// Every array in a save is by position, so the existing items must keep their numbers.
/// New items go on the end.
#[test]
fn item_order_is_pinned() {
    const PINNED: &str = "CopperOre Coal CopperPlate CopperWire CopperPipe CopperCoil Coke IronOre IronPlate Steel Waste SteelGirder \
        Concrete IronGear Gearbox DrillingRig CopperIngot IronIngot Stone CopperGrit IronGrit Sand FineWire CopperSheet IronRod Screw \
        Spring IronSheet Brick Glass SteelRod Bearing SteelSheet HardenedIronPlate HardenedGear ReinforcedConcrete Valve Gauge Lamp \
        Relay ElectricMotor Piston ControlBoard BoilerShell SiloShell Frame PumpUnit Generator EngineBlock ControlPanel SteamTractor";
    let now: Vec<String> = Item::ALL.iter().map(|i| format!("{i:?}")).collect();
    let pinned: Vec<&str> = PINNED.split_whitespace().collect();
    assert!(now.len() >= pinned.len(), "items were removed");
    assert_eq!(&now[..pinned.len()], &pinned[..], "existing items moved: old saves would read the wrong item's numbers");
    for (i, item) in Item::ALL.iter().enumerate() {
        assert_eq!(item.idx(), i);
    }
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

/// A world with one of every part, an elevator car, mail and some goods in transit.
fn everything() -> World {
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
    w.receive(crate::mail::parse("from: tests\nevent: finished\nhello", 0), 1_000);
    w.saved_at = 1_000;
    w
}

/// Changing any saved struct changes this shape. If it fails on purpose, bump `SAVE_VERSION`
/// (old saves can't be read by the new layout) and regenerate the file with `UPDATE_GOLDEN=1 cargo test save_format`.
#[test]
fn save_format_matches_the_golden_shape() {
    let mut w = everything();
    let path = std::env::temp_dir().join(format!("factory-test-shape-{}.json", std::process::id()));
    w.save(&path).unwrap();
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let _ = std::fs::remove_file(&path);
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
