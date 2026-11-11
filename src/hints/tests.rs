//! Hint triggers, hint and aside pacing, and the Hints switch.

use super::*;
use crate::sim::{BeltItem, Dir, Setting};

fn world() -> World {
    let mut w = World::new();
    w.econ.money = 1_000_000.0;
    w
}

/// Opens whatever `kind` needs to be placed.
fn unlock(w: &mut World, kind: Kind) {
    if let Some(t) = kind.tech() {
        w.progress.researched[t.idx()] = true;
        w.progress.opened[t.company().idx()] = true;
    }
}

fn put(w: &mut World, x: i32, y: i32, kind: Kind) {
    unlock(w, kind);
    w.place(0, x, y, kind, Dir::E).unwrap_or_else(|e| panic!("{kind:?} at {x},{y}: {e}"));
}

fn st(w: &mut World, x: i32, y: i32) -> &mut St {
    let a = w.floors[0].anchor_at(x, y).unwrap();
    &mut w.floors[0].tiles[a].as_mut().unwrap().st
}

fn holds(w: &World, h: Hint) -> bool {
    Facts::scan(w).hint_holds(h)
}

fn text_of(w: &World, h: Hint) -> String {
    hint_text(h, &Facts::scan(w), &Keys::of(&Controls::defaults()))
}

/// Every hint at once is far more than a pane shows: only the first ready one comes up.
fn only(w: &World, h: Hint) -> bool {
    Hint::ALL.iter().all(|o| holds(w, *o) == (*o == h))
}

/// A drill with no fuel left, mining copper.
fn dry_drill(w: &mut World, x: i32, y: i32) {
    put(w, x, y, Kind::Drill);
    if let St::Drill { fuel, coal, .. } = st(w, x, y) {
        *fuel = 0;
        *coal = 0;
    }
}

#[test]
fn every_text_fits_a_58_column_line() {
    let keys = Keys::of(&Controls::defaults());
    let f = Facts { coal_machine: Some("Large advanced assembler"), ..Facts::default() };
    for h in Hint::ALL {
        let t = hint_text(h, &f, &keys);
        assert!(t.chars().count() <= 58, "{h:?}: {t} is {}", t.chars().count());
    }
    for a in Aside::ALL {
        assert!(a.text().chars().count() <= 58, "{a:?}: {}", a.text());
        assert!(a.text().starts_with(a.company().name().split_whitespace().next().unwrap()), "{a:?}");
    }
}

#[test]
fn an_empty_world_says_how_to_start() {
    let mut w = world();
    assert!(only(&w, Hint::Empty));
    assert_eq!(text_of(&w, Hint::Empty), "1 opens build · a drill mines ore anywhere (f picks it)");
    put(&mut w, 60, 40, Kind::Pole);
    assert!(!holds(&w, Hint::Empty));
}

#[test]
fn out_of_coal_names_the_drill_or_the_machine() {
    let mut w = world();
    put(&mut w, 50, 40, Kind::Drill);
    // A fresh drill comes with fuel.
    assert!(!holds(&w, Hint::OutOfCoal));
    if let St::Drill { fuel, .. } = st(&mut w, 50, 40) {
        *fuel = 0;
    }
    assert!(holds(&w, Hint::OutOfCoal));
    assert_eq!(text_of(&w, Hint::OutOfCoal), "Your drill is out of coal: put a drill on coal next to it");
    // A drill that mines coal fuels itself.
    w.apply_setting(0, 50, 40, Setting::Mines(Item::Coal));
    assert!(!holds(&w, Hint::OutOfCoal));
    // A boiler with no coal is another machine in the same state.
    let mut w = world();
    put(&mut w, 50, 40, Kind::Boiler);
    assert!(holds(&w, Hint::OutOfCoal));
    assert_eq!(text_of(&w, Hint::OutOfCoal), "Your boiler is out of coal: belt some in");
    if let St::Boiler { coal, .. } = st(&mut w, 50, 40) {
        *coal = 5;
    }
    assert!(!holds(&w, Hint::OutOfCoal));
}

#[test]
fn unpowered_parts_get_the_ec_hint() {
    let mut w = world();
    put(&mut w, 50, 40, Kind::Molder);
    assert!(holds(&w, Hint::NoEc), "{:?}", w.grid.unpowered);
    assert!(text_of(&w, Hint::NoEc).contains("EC") && text_of(&w, Hint::NoEc).contains("v shows range"));
    put(&mut w, 50, 43, Kind::Solar);
    put(&mut w, 52, 43, Kind::Pole);
    assert!(!holds(&w, Hint::NoEc));
}

#[test]
fn no_trade_post_says_items_have_nowhere_to_go() {
    let mut w = world();
    assert!(!holds(&w, Hint::NoTradePost), "nothing built yet: the Empty hint's turn");
    put(&mut w, 50, 40, Kind::Drill);
    assert!(holds(&w, Hint::NoTradePost));
    assert_eq!(text_of(&w, Hint::NoTradePost), "No trade post yet: items have nowhere to go");
    put(&mut w, 60, 40, Kind::TradePost);
    assert!(!holds(&w, Hint::NoTradePost));
    // Only a pole, or a belt, makes nothing that would need a post.
    let mut w = world();
    put(&mut w, 50, 40, Kind::Pole);
    assert!(!holds(&w, Hint::NoTradePost));
}

#[test]
fn full_needs_nothing_at_the_output_edge() {
    let mut w = world();
    put(&mut w, 50, 40, Kind::Drill);
    let (dw, _) = Kind::Drill.dims(Dir::E);
    let cap = Kind::Drill.drill_out_cap();
    let fill = |w: &mut World, n: usize| {
        if let St::Drill { out, .. } = st(w, 50, 40) {
            out.clear();
            out.extend(std::iter::repeat_n(Item::CopperOre, n));
        }
    };
    fill(&mut w, cap - 1);
    assert!(!holds(&w, Hint::Full), "not full yet");
    fill(&mut w, cap);
    assert!(holds(&w, Hint::Full));
    assert_eq!(text_of(&w, Hint::Full), "Output is full: put a belt at the arrow side");
    // With a belt at the arrow side it is just backed up, which is another matter.
    put(&mut w, 50 + dw, 40, Kind::Belt);
    assert!(!holds(&w, Hint::Full));
}

#[test]
fn a_waiting_recipe_machine_points_at_the_book() {
    let mut w = world();
    put(&mut w, 50, 40, Kind::Pole);
    assert!(!holds(&w, Hint::Waiting));
    put(&mut w, 52, 40, Kind::Molder);
    assert!(holds(&w, Hint::Waiting), "a molder with nothing to cast is waiting");
    assert_eq!(text_of(&w, Hint::Waiting), "b opens its recipe to see what it needs");
}

#[test]
fn research_from_selling_needs_sales_a_money_post_and_no_research_yet() {
    let mut w = world();
    put(&mut w, 60, 40, Kind::TradePost);
    assert!(!holds(&w, Hint::ResearchFromSales), "nothing sold yet");
    w.econ.sold[Item::CopperIngot.idx()] = 3;
    assert!(holds(&w, Hint::ResearchFromSales));
    assert_eq!(text_of(&w, Hint::ResearchFromSales), "Research comes from selling: f on a trade post");
    // Set to split (or research): the player knows f already.
    w.apply_setting(0, 60, 40, Setting::Sell(SellMode::Split));
    assert!(!holds(&w, Hint::ResearchFromSales));
    w.apply_setting(0, 60, 40, Setting::Sell(SellMode::Money));
    assert!(holds(&w, Hint::ResearchFromSales));
    // Or once research has been earned.
    w.econ.earned_research = 0.5;
    assert!(!holds(&w, Hint::ResearchFromSales));
    // No post at all: that is another hint's business.
    let mut w = world();
    w.econ.sold[Item::CopperIngot.idx()] = 3;
    assert!(!holds(&w, Hint::ResearchFromSales));
}

#[test]
fn can_research_when_the_points_reach_an_open_techs_cost() {
    let mut w = world();
    w.econ.research = 59.0;
    assert!(!holds(&w, Hint::CanResearch));
    w.econ.research = 60.0; // the press
    assert!(holds(&w, Hint::CanResearch));
    assert_eq!(text_of(&w, Hint::CanResearch), "You can afford a new part: t");
    w.research(Tech::Press).unwrap();
    assert!(!holds(&w, Hint::CanResearch), "nothing else open costs 0");
}

#[test]
fn each_hint_has_a_state_that_holds_only_it() {
    // The states above: one hint, and no other, at a time (the demo-like extras aside).
    let mut w = world();
    put(&mut w, 50, 40, Kind::Pole);
    assert!(Hint::ALL.iter().all(|h| !holds(&w, *h)), "a lone pole asks for nothing");
    let mut w = world();
    dry_drill(&mut w, 50, 40);
    assert!(holds(&w, Hint::OutOfCoal) && holds(&w, Hint::NoTradePost) && !holds(&w, Hint::Empty));
}

// --- Pacing: the clock is passed in, so these run in no time. ---

fn controls() -> Controls {
    Controls::defaults()
}

/// Calls `update` once a second from `from` to `to` (seconds after `t0`).
fn run(h: &mut Hints, w: &World, t0: Instant, from: u64, to: u64) {
    for s in from..=to {
        h.update(w, &controls(), t0 + Duration::from_secs(s), true, true, true);
    }
}

fn shown(h: &Hints) -> Option<String> {
    h.line(true).map(str::to_string)
}

/// A world that wants the "no trade post" hint, and nothing else.
fn needs_a_post() -> World {
    let mut w = world();
    put(&mut w, 50, 40, Kind::Drill);
    w
}

#[test]
fn a_hint_shows_after_its_state_has_held_and_fades() {
    let w = needs_a_post();
    let t0 = Instant::now();
    let mut h = Hints::new(t0);
    run(&mut h, &w, t0, 0, 19);
    assert_eq!(shown(&h), None, "20 seconds first");
    run(&mut h, &w, t0, 20, 20);
    assert_eq!(shown(&h).as_deref(), Some("No trade post yet: items have nowhere to go"));
    run(&mut h, &w, t0, 21, 31);
    assert!(shown(&h).is_some(), "up for 12 seconds");
    run(&mut h, &w, t0, 32, 32);
    assert_eq!(shown(&h), None, "then it fades");
    // The same hint waits 3 minutes before coming back.
    run(&mut h, &w, t0, 33, 20 + 179);
    assert_eq!(shown(&h), None);
    run(&mut h, &w, t0, 20 + 180, 20 + 180);
    assert!(shown(&h).is_some(), "back after 3 minutes");
    // ... and at most three times a session.
    run(&mut h, &w, t0, 20 + 181, 20 + 360 + 1);
    assert!(shown(&h).is_some());
    run(&mut h, &w, t0, 20 + 360 + 2, 20 + 360 + 400);
    assert_eq!(shown(&h), None, "the third was the last");
}

#[test]
fn a_state_that_flickers_never_shows() {
    let mut w = needs_a_post();
    let t0 = Instant::now();
    let mut h = Hints::new(t0);
    for round in 0..4u64 {
        // 15 seconds with the problem, then a post, then the problem again.
        run(&mut h, &w, t0, round * 20, round * 20 + 14);
        put(&mut w, 60 + round as i32 * 4, 40, Kind::TradePost);
        run(&mut h, &w, t0, round * 20 + 15, round * 20 + 19);
        let a = w.floors[0].anchor_at(60 + round as i32 * 4, 40).unwrap();
        w.floors[0].tiles[a] = None;
        w.floors[0].owner.iter_mut().for_each(|o| {
            if *o == Some(a as u32) {
                *o = None;
            }
        });
    }
    assert_eq!(shown(&h), None);
}

#[test]
fn a_hint_goes_at_once_when_the_problem_is_fixed() {
    let mut w = needs_a_post();
    let t0 = Instant::now();
    let mut h = Hints::new(t0);
    run(&mut h, &w, t0, 0, 22);
    assert!(shown(&h).is_some());
    put(&mut w, 60, 40, Kind::TradePost);
    run(&mut h, &w, t0, 23, 24);
    assert_eq!(shown(&h), None);
}

#[test]
fn a_busy_slot_delays_a_hint_without_using_it_up() {
    let w = needs_a_post();
    let t0 = Instant::now();
    let mut h = Hints::new(t0);
    for s in 0..=60 {
        h.update(&w, &controls(), t0 + Duration::from_secs(s), true, true, false);
    }
    assert_eq!(shown(&h), None, "a message or stuck reason holds the slot");
    h.update(&w, &controls(), t0 + Duration::from_secs(61), true, true, true);
    assert!(shown(&h).is_some(), "it shows when the slot frees up, with its 12 seconds intact");
}

#[test]
fn the_earlier_hint_wins_when_two_are_ready() {
    let mut w = world();
    dry_drill(&mut w, 50, 40); // out of coal, and no trade post
    let t0 = Instant::now();
    let mut h = Hints::new(t0);
    run(&mut h, &w, t0, 0, 20);
    assert!(shown(&h).unwrap().contains("out of coal"));
    // After it fades the other one gets its turn, one at a time.
    run(&mut h, &w, t0, 21, 33);
    run(&mut h, &w, t0, 34, 34);
    assert!(shown(&h).unwrap().contains("No trade post"));
}

#[test]
fn hints_off_shows_nothing_and_starts_over_when_back_on() {
    let w = needs_a_post();
    let t0 = Instant::now();
    let mut h = Hints::new(t0);
    for s in 0..=100 {
        h.update(&w, &controls(), t0 + Duration::from_secs(s), false, true, true);
    }
    assert_eq!(shown(&h), None);
    // Switched on at 100 s: the state must hold 20 s again.
    for s in 100..=119 {
        h.update(&w, &controls(), t0 + Duration::from_secs(s), true, true, true);
    }
    assert_eq!(shown(&h), None);
    h.update(&w, &controls(), t0 + Duration::from_secs(120), true, true, true);
    assert!(shown(&h).is_some());
    // And switching off takes the one on screen away.
    h.update(&w, &controls(), t0 + Duration::from_secs(121), false, true, true);
    assert_eq!(shown(&h), None);
}

// --- Asides ---

/// `n` drills set to coal.
fn coal_drills(w: &mut World, n: i32) {
    for i in 0..n {
        let (x, y) = (20 + (i % 10) * 4, 20 + (i / 10) * 4);
        put(w, x, y, Kind::Drill);
        w.apply_setting(0, x, y, Setting::Mines(Item::Coal));
    }
}

/// A healthy world: a post, power, nothing stuck, so no hint is in the way.
fn healthy() -> World {
    let mut w = world();
    put(&mut w, 60, 40, Kind::TradePost);
    put(&mut w, 60, 44, Kind::Solar);
    put(&mut w, 62, 44, Kind::Pole);
    assert!(Hint::ALL.iter().all(|h| !holds(&w, *h)), "a healthy world has no hint ready: {:?}", Facts::scan(&w));
    w.econ.sold[Item::CopperIngot.idx()] = 1;
    w.econ.earned_research = 1.0;
    w
}

fn min(m: u64) -> u64 {
    m * 60
}

#[test]
fn an_aside_waits_eight_minutes_then_shows_for_ten_seconds() {
    let mut w = healthy();
    coal_drills(&mut w, 4);
    let t0 = Instant::now();
    let mut h = Hints::new(t0);
    for s in (0..min(8)).step_by(5) {
        h.update(&w, &controls(), t0 + Duration::from_secs(s), true, true, true);
    }
    assert_eq!(shown(&h), None, "nothing before 8 minutes");
    h.update(&w, &controls(), t0 + Duration::from_secs(min(8)), true, true, true);
    assert_eq!(shown(&h).as_deref(), Some("Hollowmere: Good coal. Dad would have wept."));
    h.update(&w, &controls(), t0 + Duration::from_secs(min(8) + 9), true, true, true);
    assert!(shown(&h).is_some());
    h.update(&w, &controls(), t0 + Duration::from_secs(min(8) + 10), true, true, true);
    assert_eq!(shown(&h), None, "10 seconds");
}

#[test]
fn at_most_one_aside_per_ten_minutes_and_two_of_each() {
    let mut w = healthy();
    coal_drills(&mut w, 4);
    // A second aside whose state turns true later: Pillar, a building on a floor above.
    let t0 = Instant::now();
    let mut h = Hints::new(t0);
    let at = |h: &mut Hints, w: &World, s: u64| h.update(w, &controls(), t0 + Duration::from_secs(s), true, true, true);
    at(&mut h, &w, 0); // the coal state is noticed
    at(&mut h, &w, min(8));
    assert!(shown(&h).unwrap().starts_with("Hollowmere"));
    // Pillar opens and builds up at minute 9: it has to wait for the gap.
    w.progress.opened[Company::Pillar.idx()] = true;
    let up = w.floor_of(1).unwrap();
    w.progress.researched[Tech::SupportBeams.idx()] = true;
    w.place(up, 60, 40, Kind::Pole, Dir::E).unwrap();
    let up_built = w.floors[up].tiles.iter().any(|t| t.is_some());
    assert!(up_built, "a building on the floor above");
    at(&mut h, &w, min(9));
    for s in (min(9) + 1..min(18)).step_by(30) {
        at(&mut h, &w, s);
        assert_eq!(shown(&h), None, "no second aside inside 10 minutes ({s}s)");
    }
    // At minute 18 the more recent state (Pillar) speaks, not the first one again.
    at(&mut h, &w, min(18));
    assert!(shown(&h).unwrap().starts_with("Pillar"), "{:?}", shown(&h));
    // At 28 both states are held and Pillar's is the newer: its second (last) turn.
    at(&mut h, &w, min(28));
    assert!(shown(&h).unwrap().starts_with("Pillar"));
    // At 38 Pillar is used up, so Hollowmere gets its second turn.
    at(&mut h, &w, min(38));
    assert!(shown(&h).unwrap().starts_with("Hollowmere"), "{:?}", shown(&h));
    // Both have spoken twice: no more, however long the game runs.
    for m in [48, 58, 68, 100, 200] {
        at(&mut h, &w, min(m));
        assert_eq!(shown(&h), None, "minute {m}");
    }
}

#[test]
fn the_most_recently_true_aside_is_picked() {
    let mut w = healthy();
    coal_drills(&mut w, 4);
    let t0 = Instant::now();
    let mut h = Hints::new(t0);
    h.update(&w, &controls(), t0, true, true, true);
    // Waste appears at minute 5 and Cinderhall is open: that state is the newer one.
    w.progress.opened[Company::Cinderhall.idx()] = true;
    put(&mut w, 63, 45, Kind::Belt);
    if let St::Belt { items } = st(&mut w, 63, 45) {
        items.push(BeltItem { item: Item::Waste, pos: 0.5 });
    }
    h.update(&w, &controls(), t0 + Duration::from_secs(min(5)), true, true, true);
    h.update(&w, &controls(), t0 + Duration::from_secs(min(8)), true, true, true);
    assert_eq!(shown(&h).as_deref(), Some("Cinderhall: Waste? That's just smoke you can carry."));
}

#[test]
fn only_open_companies_speak() {
    let mut w = healthy();
    put(&mut w, 63, 45, Kind::Belt);
    if let St::Belt { items } = st(&mut w, 63, 45) {
        items.push(BeltItem { item: Item::Waste, pos: 0.5 });
    }
    let t0 = Instant::now();
    let mut h = Hints::new(t0);
    for s in [0, min(8), min(9), min(30)] {
        h.update(&w, &controls(), t0 + Duration::from_secs(s), true, true, true);
        assert_eq!(shown(&h), None, "Cinderhall is not open at {s}s");
    }
}

#[test]
fn asides_never_show_in_the_small_layout_and_wait_for_the_large_one() {
    let mut w = healthy();
    coal_drills(&mut w, 4);
    let t0 = Instant::now();
    let mut h = Hints::new(t0);
    for s in [0, min(8), min(9), min(30)] {
        h.update(&w, &controls(), t0 + Duration::from_secs(s), true, false, true);
        assert_eq!(h.current, None, "small layout at {s}s");
    }
    // Back in the large layout the aside has not been used up.
    h.update(&w, &controls(), t0 + Duration::from_secs(min(31)), true, true, true);
    assert!(shown(&h).unwrap().starts_with("Hollowmere"));
    // And one that is up when the layout shrinks goes at once.
    h.update(&w, &controls(), t0 + Duration::from_secs(min(31) + 1), true, false, true);
    assert_eq!(h.line(true), None);
    // Even a stale aside is not offered to a small layout.
    h.force(What::Aside(Aside::A3), "Hollowmere: x", t0);
    assert_eq!(h.line(false), None);
}

#[test]
fn a_ready_hint_beats_an_aside() {
    let mut w = healthy();
    coal_drills(&mut w, 4); // the aside's state: they have fuel and no problem
    let t0 = Instant::now();
    let mut h = Hints::new(t0);
    let at = |h: &mut Hints, w: &World, s: u64| h.update(w, &controls(), t0 + Duration::from_secs(s), true, true, true);
    for s in (0..min(7) + 40).step_by(10) {
        at(&mut h, &w, s);
    }
    // A stuck drill turns up 20 seconds before the first aside is due.
    dry_drill(&mut w, 90, 60);
    at(&mut h, &w, min(7) + 40);
    for s in min(7) + 41..min(8) {
        at(&mut h, &w, s);
    }
    // Both are ready at minute 8: the hint takes the slot.
    at(&mut h, &w, min(8));
    assert!(shown(&h).unwrap().contains("out of coal"), "{:?}", shown(&h));
    // When it fades the aside, which has waited, comes up (the hint now rests for 3 minutes).
    at(&mut h, &w, min(8) + 12);
    assert!(shown(&h).unwrap().starts_with("Hollowmere"), "{:?}", shown(&h));
}

#[test]
fn the_cut_asides_are_gone() {
    // A1 (slow belts), A7 (no pump) and A14 (power bill) read as nudges: cut.
    let all: Vec<&str> = Aside::ALL.iter().map(|a| a.text()).collect();
    for cut in ["slow belt", "little water", "power bill"] {
        assert!(all.iter().all(|t| !t.contains(cut)), "{cut}");
    }
    // Nothing for companies the game does not have yet.
    for later in ["Halverson", "Meridian", "Sablon", "Voss:"] {
        assert!(all.iter().all(|t| !t.starts_with(later)), "{later}");
    }
}
