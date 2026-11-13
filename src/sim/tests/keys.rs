//! Companies that open on a list of keys: two keys, a fluid key, and the letter that comes with them.
//! No real company has two keys yet, so these tests give one other keys (`KEYS_OVERRIDE`).

use super::saving::temp;
use super::*;
use crate::content::KEYS_OVERRIDE;

const WIRE: Made = Made::Item(Item::CopperWire);
const COKE: Made = Made::Item(Item::Coke);
const METAL: Made = Made::Fluid(Fluid::MoltenCopper);

static WIRE_OR_COKE: [Made; 2] = [WIRE, COKE];
static WIRE_OR_METAL: [Made; 2] = [WIRE, METAL];
static METAL_ONLY: [Made; 1] = [METAL];

fn give_keys(c: Company, keys: &'static [Made]) {
    KEYS_OVERRIDE.with(|o| o.set(Some((c, keys))));
}

fn clear_keys() {
    KEYS_OVERRIDE.with(|o| o.set(None));
}

/// A blank world (every part unlocked) whose companies are open only if they have no keys.
/// Call it after `give_keys`.
fn closed_world() -> World {
    let mut w = World::blank();
    w.progress.opened = Company::ALL.map(|c| c.key_items().is_empty());
    w
}

/// How many letters from `c` the world has logged.
fn letters_from(w: &World, c: Company) -> usize {
    w.log.iter().filter(|e| e.msg.event == "letter" && e.msg.from == c.name()).count()
}

#[test]
fn the_first_key_made_opens_a_company() {
    let have = |made: &'static [Made]| move |m: Made| u64::from(made.contains(&m));
    assert_eq!(first_key_made(&[WIRE, COKE], have(&[])), None);
    assert_eq!(first_key_made(&[WIRE, COKE], have(&[WIRE])), Some(WIRE));
    assert_eq!(first_key_made(&[WIRE, COKE], have(&[COKE])), Some(COKE));
    assert_eq!(first_key_made(&[METAL], have(&[METAL])), Some(METAL));
    assert_eq!(first_key_made(&[], have(&[WIRE])), None, "no keys: nothing opens it (it was open already)");
}

#[test]
fn a_company_with_two_keys_opens_on_either() {
    let c = Company::Ashgrove;
    for (key, made) in [(WIRE, Item::CopperWire), (COKE, Item::Coke)] {
        give_keys(c, &WIRE_OR_COKE);
        let mut w = closed_world();
        assert!(!w.progress.is_open(c));
        run(&mut w, 2);
        assert!(!w.progress.is_open(c), "nothing made yet");
        w.econ.made[made] = 1;
        run(&mut w, 2);
        assert!(w.progress.is_open(c), "{key:?} opens {c:?}");
        assert_eq!(letters_from(&w, c), 1);
        clear_keys();
    }
}

#[test]
fn a_fluid_key_opens_a_company() {
    // A molder that makes only molten copper (no machine does yet): the metal alone opens the company.
    static METAL_ONLY_RECIPE: [Recipe; 1] = [Recipe {
        name: "test",
        inputs: &[(Item::Coal, 1)],
        outputs: &[],
        fluid_in: None,
        fluid_out: Some((Fluid::MoltenCopper, 2)),
        fluid_out2: None,
        ticks: 60,
        fuel: None,
        needs: None,
        tag: 'T',
    }];
    let c = Company::Wellbourne;
    give_keys(c, &METAL_ONLY);
    RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &METAL_ONLY_RECIPE))));
    let mut w = World::blank(); // everything unlocked, so the pipe can be placed...
    w.place(0, 5, 5, Kind::Molder, Dir::E).unwrap();
    set_stock(&mut w, 5, 5, Item::Coal, 10);
    w.place(0, 7, 5, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 5, 7, Kind::Solar, Dir::E).unwrap();
    w.progress.opened = Company::ALL.map(|c| c.key_items().is_empty()); // ...then close the company
    assert!(!w.progress.is_open(c));
    run(&mut w, 3);
    let made = w.econ.made_fluid[Fluid::MoltenCopper];
    assert!(made >= 2 && made % 2 == 0, "each run makes two units, got {made}");
    assert!(w.progress.is_open(c), "making the metal opens the company");
    assert_eq!(letters_from(&w, c), 1);
    // Making more does not send it again.
    run(&mut w, 20);
    assert!(w.econ.made_fluid[Fluid::MoltenCopper] > made);
    assert_eq!(letters_from(&w, c), 1);
    RECIPE_OVERRIDE.with(|o| o.set(None));
    clear_keys();
}

#[test]
fn the_letter_is_delivered_once_whichever_key_comes_second() {
    let c = Company::Ashgrove;
    give_keys(c, &WIRE_OR_METAL);
    let mut w = closed_world();
    w.econ.made[Item::CopperWire] = 1;
    run(&mut w, 2);
    assert_eq!(letters_from(&w, c), 1);
    // The other key arrives later: the company is open already, so no second letter.
    w.econ.made_fluid[Fluid::MoltenCopper] = 5;
    run(&mut w, 5);
    assert_eq!(letters_from(&w, c), 1);
    let m = w.log.iter().find(|e| e.msg.from == c.name()).unwrap();
    assert!(m.msg.md.contains(c.letter(WIRE)), "{}", m.msg.md);
    // Both keys made at once still make one letter.
    clear_keys();
    give_keys(Company::Cinderhall, &WIRE_OR_METAL);
    let mut w = closed_world();
    w.econ.made[Item::CopperWire] = 1;
    w.econ.made_fluid[Fluid::MoltenCopper] = 1;
    run(&mut w, 5);
    assert_eq!(letters_from(&w, Company::Cinderhall), 1);
    clear_keys();
}

#[test]
fn the_letter_hook_takes_the_key_that_opened_the_company() {
    // Today every company writes one letter whichever key opened it; the key is there for two.
    for c in Company::ALL {
        for key in [WIRE, METAL] {
            assert_eq!(c.letter(key), c.letter(c.key_items().first().copied().unwrap_or(key)));
            let m = crate::mail::letter(c, key, 7);
            assert_eq!((m.from.as_str(), m.event.as_str()), (c.name(), "letter"));
        }
    }
}

#[test]
fn fluid_made_counts_survive_a_save() {
    let data = temp("made-fluid");
    let mut w = closed_world();
    w.econ.made_fluid[Fluid::MoltenIron] = 42;
    w.econ.made_fluid[Fluid::Steam] = 1;
    let path = crate::saves::path_of(&data, "save1");
    w.save(&path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    // Saved by name, with the zero ones left out.
    assert_eq!(json["econ"]["made_fluid"], serde_json::json!({"MoltenIron": 42, "Steam": 1}));
    let l = World::load(&path).unwrap();
    assert_eq!((l.econ.made_fluid[Fluid::MoltenIron], l.econ.made_fluid[Fluid::Steam], l.econ.made_fluid[Fluid::Water]), (42, 1, 0));
    // A fluid this game does not know is refused, not dropped.
    let mut bad = json.clone();
    bad["econ"]["made_fluid"]["Oil"] = serde_json::json!(3);
    std::fs::write(&path, bad.to_string()).unwrap();
    let err = World::load(&path).err().expect("refused").to_string();
    assert!(err.contains("unknown fluid 'Oil'"), "{err}");
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn an_opened_two_key_company_stays_open_and_quiet_after_a_load() {
    let data = temp("two-key-load");
    let c = Company::Ashgrove;
    give_keys(c, &WIRE_OR_COKE);
    let mut w = closed_world();
    w.econ.made[Item::Coke] = 3;
    run(&mut w, 2);
    assert_eq!(letters_from(&w, c), 1);
    let path = crate::saves::path_of(&data, "save1");
    w.save(&path).unwrap();
    let mut l = World::load(&path).unwrap();
    assert!(l.progress.is_open(c));
    l.econ.made[Item::CopperWire] = 1;
    run(&mut l, 3);
    assert_eq!(letters_from(&l, c), 1, "the saved letter is not sent again");
    clear_keys();
    let _ = std::fs::remove_dir_all(&data);
}
