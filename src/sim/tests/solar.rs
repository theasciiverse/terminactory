//! The basic solar panel (solar mk2): 2x2, 20 EC, reach 4, still linked only through poles.

use super::*;

/// The number of grids (power networks).
fn grids(w: &World) -> usize {
    w.grid.nets.len()
}

#[test]
fn the_basic_solar_panel_has_the_asked_numbers() {
    let k = Kind::BasicSolar;
    assert_eq!((k.name(), k.base_dims(), k.cost()), ("Basic solar panel", (2, 2), 300.0));
    assert_eq!((k.solar_output(), k.reach(), k.category()), (20.0, Some(4), Category::Power));
    assert_eq!((k.company(), k.tech()), (Some(Company::Kettering), Some(Tech::BasicSolar)));
    assert_eq!(Company::Kettering.name(), "Kettering & Vane");
    assert!(k.is_solar() && Kind::Solar.is_solar() && !Kind::Pole.is_solar());
    // The same footprint as the cheap one (so an upgrade planner can swap them), and no facing.
    assert_eq!((Kind::Solar.base_dims(), Kind::Solar.name(), Kind::Solar.cost()), ((2, 2), "Cheap solar panel", 100.0));
    assert_eq!((Kind::Solar.solar_output(), Kind::Solar.reach()), (SOLAR_OUTPUT, Some(SOLAR_REACH)));
    assert!(!k.directional() && !k.electric());
    assert_eq!(k.dims(Dir::N), (2, 2));
    // $15 an EC against the cheap panel's $10: it sells reach, not raw power.
    assert_eq!(k.cost() / k.solar_output(), 15.0);
    assert_eq!(Kind::Solar.cost() / Kind::Solar.solar_output(), 10.0);
    // The tech: 1,800 RP, or make 60 glass.
    assert_eq!((Tech::BasicSolar.cost(), Tech::BasicSolar.milestone()), (1800.0, (Item::Glass, 60)));
    assert_eq!(Tech::BasicSolar.company(), Company::Kettering);
    assert_eq!(Tech::BasicSolar.category(), Category::Power);
}

#[test]
fn basic_solar_has_longer_reach_and_more_output() {
    let mut w = World::blank();
    w.place(0, 20, 20, Kind::BasicSolar, Dir::E).unwrap(); // covers 20-21, reaches 16-25 both ways
    assert_eq!(coverage(Kind::BasicSolar, (2, 2), 20, 20), Some((16, 16, 25, 25)), "10x10");
    assert_eq!(coverage(Kind::Solar, (2, 2), 20, 20), Some((18, 18, 23, 23)), "6x6");
    assert_eq!((grids(&w), w.grid.nets[0].solar), (1, 20.0));
    // A belt 4 tiles from the panel's edge is powered, one 5 away is not (the cheap panel stops at 2).
    w.place(0, 25, 20, Kind::Belt, Dir::E).unwrap();
    w.place(0, 26, 20, Kind::Belt, Dir::E).unwrap();
    assert_eq!(w.ratio_at(0, 25, 20), 1.0);
    assert_eq!(w.ratio_at(0, 26, 20), 0.0);
    let mut cheap = World::blank();
    cheap.place(0, 20, 20, Kind::Solar, Dir::E).unwrap();
    cheap.place(0, 23, 20, Kind::Belt, Dir::E).unwrap();
    cheap.place(0, 24, 20, Kind::Belt, Dir::E).unwrap();
    assert_eq!((cheap.ratio_at(0, 23, 20), cheap.ratio_at(0, 24, 20)), (1.0, 0.0));
}

#[test]
fn a_pole_links_a_basic_panel_from_seven_tiles_a_cheap_one_from_five() {
    // A pole reaches 3 around itself, so it links when its range meets the panel's: 4 + 3 = 7 tiles
    // from the panel's edge for the basic panel, 2 + 3 = 5 for the cheap one.
    for (kind, far) in [(Kind::BasicSolar, 7), (Kind::Solar, 5)] {
        let edge = 21; // the panel covers 20-21
        let mut linked = World::blank();
        linked.place(0, 20, 20, kind, Dir::E).unwrap();
        linked.place(0, edge + far, 20, Kind::Pole, Dir::E).unwrap();
        assert_eq!(grids(&linked), 1, "{kind:?} with a pole {far} tiles from its edge");
        let mut apart = World::blank();
        apart.place(0, 20, 20, kind, Dir::E).unwrap();
        apart.place(0, edge + far + 1, 20, Kind::Pole, Dir::E).unwrap();
        assert_eq!(grids(&apart), 2, "{kind:?} with a pole {} tiles from its edge", far + 1);
    }
}

#[test]
fn solar_panels_never_link_even_mk2() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::BasicSolar, Dir::E).unwrap();
    w.place(0, 12, 10, Kind::BasicSolar, Dir::E).unwrap();
    w.place(0, 14, 10, Kind::Solar, Dir::E).unwrap();
    assert_eq!(grids(&w), 3, "side by side, mk2 beside mk2 beside a cheap one: three grids");
    let mut supplies: Vec<f64> = w.grid.nets.iter().map(|n| n.solar).collect();
    supplies.sort_by(f64::total_cmp);
    assert_eq!(supplies, vec![10.0, 20.0, 20.0]);
    // A lone panel's supply counts as unused.
    let (supply, _, _, _, _, unused) = w.power_summary();
    assert_eq!((supply, unused), (0.0, 50.0));
    // A pole whose range meets all three joins them.
    w.place(0, 13, 14, Kind::Pole, Dir::E).unwrap();
    assert_eq!(grids(&w), 1);
    assert_eq!(w.grid.nets[0].solar, 50.0);
}

#[test]
fn a_basic_panel_powers_a_machine_a_cheap_one_cannot_reach() {
    for (kind, powered) in [(Kind::Solar, 0.0), (Kind::BasicSolar, 1.0)] {
        let mut w = World::blank();
        w.place(0, 20, 20, kind, Dir::E).unwrap();
        w.place(0, 24, 20, Kind::Press, Dir::E).unwrap(); // 3 tiles from the panel's edge
        assert_eq!(w.ratio_at(0, 24, 20), powered, "{kind:?}");
    }
}

#[test]
fn two_basic_panels_on_one_pole_give_forty_ec() {
    // 64 EC of precision assembler on 40 EC of panels runs at 40/64.
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::BasicSolar, Dir::E).unwrap();
    w.place(0, 14, 10, Kind::BasicSolar, Dir::E).unwrap();
    w.place(0, 12, 8, Kind::Pole, Dir::E).unwrap();
    w.place(0, 11, 5, Kind::PrecisionAssembler, Dir::E).unwrap();
    assert_eq!(grids(&w), 1);
    assert_eq!(w.grid.nets[0].solar, 40.0);
    assert!((w.ratio_at(0, 11, 5) - 40.0 / 64.0).abs() < 1e-9, "{}", w.ratio_at(0, 11, 5));
}

#[test]
fn basic_solar_needs_sunlight() {
    let w = World::blank();
    assert!(w.check_place(1, 3, 3, Kind::BasicSolar).unwrap_err().contains("sunlight"));
    assert!(w.check_place(0, 3, 3, Kind::BasicSolar).is_ok());
}

#[test]
fn basic_solar_is_locked_until_researched() {
    let mut w = World::new();
    w.econ.money = 10_000.0;
    assert!(w.check_place(0, 30, 30, Kind::BasicSolar).unwrap_err().contains("locked"));
    assert!(w.research(Tech::BasicSolar).unwrap_err().contains("copper coil"));
    // Kettering & Vane opens on a copper coil; the tech still has to be bought.
    w.econ.made[Item::CopperCoil.idx()] = 1;
    w.tick();
    assert!(w.progress.is_open(Company::Kettering));
    assert!(w.check_place(0, 30, 30, Kind::BasicSolar).unwrap_err().contains("locked"));
    assert!(w.research(Tech::BasicSolar).unwrap_err().contains("1800"));
    w.econ.research = 1800.0;
    w.research(Tech::BasicSolar).unwrap();
    assert!(w.check_place(0, 30, 30, Kind::BasicSolar).is_ok());
    assert_eq!(w.econ.research, 0.0);
}

#[test]
fn sixty_glass_unlock_basic_solar_for_free() {
    let mut w = World::new();
    w.econ.made[Item::CopperCoil.idx()] = 1;
    w.econ.made[Item::Glass.idx()] = 59;
    w.tick();
    assert!(!w.progress.has(Tech::BasicSolar));
    w.econ.made[Item::Glass.idx()] = 60;
    w.tick();
    assert!(w.progress.has(Tech::BasicSolar));
    assert_eq!(w.econ.research, 0.0, "nothing was spent");
    assert!(w.progress.part_unlocked(Kind::BasicSolar));
}

#[test]
fn basic_solar_saves_by_name_and_keeps_its_output() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::BasicSolar, Dir::E).unwrap();
    w.place(0, 14, 10, Kind::Pole, Dir::E).unwrap();
    let path = std::env::temp_dir().join(format!("factory-test-solar2-{}.json", std::process::id()));
    w.save(&path).unwrap();
    let json = std::fs::read_to_string(&path).unwrap();
    assert!(json.contains("\"BasicSolar\""));
    let l = World::load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(l.tile(0, 10, 10).map(|b| b.kind), Some(Kind::BasicSolar));
    assert_eq!(l.grid.nets.iter().map(|n| n.solar).sum::<f64>(), 20.0);
}
