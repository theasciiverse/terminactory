//! Belt tunnels: spacing, which sides take items, reach, a missing exit, a blocked exit, turning, saving.

use super::*;

/// A source crate holding `n` coal, an entrance at x=6, and an exit `gap` tiles after it, all on row 10.
/// Returns the exit's x.
fn pair(w: &mut World, n: usize, gap: i32) -> i32 {
    w.place(0, 5, 10, Kind::Crate, Dir::E).unwrap();
    fill_crate(w, 0, 5, 10, &vec![Item::Coal; n]);
    w.place(0, 6, 10, Kind::Tunnel, Dir::E).unwrap();
    w.place(0, 6 + gap, 10, Kind::Tunnel, Dir::E).unwrap();
    6 + gap
}

fn inside(w: &World, x: i32) -> Vec<(Item, f64)> {
    match &w.tile(0, x, 10).unwrap().st {
        St::Tunnel { items, .. } => items.iter().copied().collect(),
        _ => panic!("not a tunnel"),
    }
}

#[test]
fn a_tunnel_keeps_a_belts_spacing_and_throughput_and_needs_no_power() {
    // Through a tunnel: nothing powered at all.
    let mut tunnel = World::blank();
    let exit = pair(&mut tunnel, 200, 4);
    tunnel.place(0, exit + 1, 10, Kind::Crate, Dir::E).unwrap();
    assert!(tunnel.grid.nets.is_empty());
    // Along four belts: a panel is needed there.
    let mut belt = World::blank();
    belt.place(0, 5, 10, Kind::Crate, Dir::E).unwrap();
    fill_crate(&mut belt, 0, 5, 10, &[Item::Coal; 200]);
    for x in 6..=9 {
        belt.place(0, x, 10, Kind::Belt, Dir::E).unwrap();
    }
    belt.place(0, 10, 10, Kind::Crate, Dir::E).unwrap();
    belt.place(0, 7, 12, Kind::Solar, Dir::E).unwrap();
    let count = |w: &World, x| crate_items(w, 0, x, 10).len();
    run(&mut tunnel, 6);
    run(&mut belt, 6);
    // Spacing in transit is never closer than a belt's four to a tile.
    let pos: Vec<f64> = inside(&tunnel, 6).iter().map(|i| i.1).collect();
    assert!(pos.len() >= 8, "a steady stream: {pos:?}");
    assert!(pos.windows(2).all(|p| p[1] - p[0] >= BELT_GAP - 1e-9), "spacing: {pos:?}");
    // Delivered over the next five seconds: the same as a belt (about 8 a second, limited by the crate; give or take one).
    let (t0, b0) = (count(&tunnel, exit + 1), count(&belt, 10));
    run(&mut tunnel, 5);
    run(&mut belt, 5);
    let (t, b) = (count(&tunnel, exit + 1) - t0, count(&belt, 10) - b0);
    assert!(t.abs_diff(b) <= 1 && (36..=41).contains(&t), "tunnel {t}, belt {b} in 5 s");
}

#[test]
fn an_entrance_takes_from_behind_and_the_sides_but_not_its_front() {
    let mut w = World::blank();
    w.place(0, 6, 10, Kind::Tunnel, Dir::E).unwrap();
    w.place(0, 10, 10, Kind::Tunnel, Dir::E).unwrap();
    w.place(0, 11, 10, Kind::Crate, Dir::E).unwrap();
    // Behind (pushing east), above (pushing south), below (pushing north), and in front (pushing west).
    let sources = [(5, 10, Dir::E), (6, 9, Dir::S), (6, 11, Dir::N), (7, 10, Dir::W)];
    for (x, y, dir) in sources {
        w.place(0, x, y, Kind::Crate, dir).unwrap();
        fill_crate(&mut w, 0, x, y, &[Item::Coal; 3]);
    }
    run(&mut w, 8);
    for (x, y, dir) in sources {
        let left = crate_items(&w, 0, x, y).len();
        if dir == Dir::W {
            assert_eq!(left, 3, "the front side takes nothing");
        } else {
            assert_eq!(left, 0, "{dir:?} source should empty");
        }
    }
    assert_eq!(crate_items(&w, 0, 11, 10).len(), 9);
}

#[test]
fn a_gap_of_four_tiles_works_and_five_does_not() {
    let mut w = World::blank();
    let exit = pair(&mut w, 5, 5); // four tiles between the two
    assert_eq!(w.tunnel_exit(0, 6, 10), Some(5));
    w.place(0, exit + 1, 10, Kind::Crate, Dir::E).unwrap();
    run(&mut w, 8);
    assert_eq!(crate_items(&w, 0, exit + 1, 10).len(), 5);
    let mut w = World::blank();
    pair(&mut w, 5, 6); // five tiles between: the second is just another entrance
    assert!(w.tunnel_exit(0, 6, 10).is_none());
    assert!(matches!(w.tile(0, 12, 10).unwrap().st, St::Tunnel { exit: false, .. }));
    run(&mut w, 3);
    assert_eq!(crate_items(&w, 0, 5, 10).len(), 5, "nothing leaves the source");
}

#[test]
fn items_wait_when_the_exit_is_removed_and_come_out_when_it_returns() {
    let mut w = World::blank();
    let exit = pair(&mut w, 10, 4);
    w.place(0, exit + 1, 10, Kind::Crate, Dir::E).unwrap();
    run(&mut w, 1);
    let source_left = crate_items(&w, 0, 5, 10).len();
    assert!(source_left < 10 && !inside(&w, 6).is_empty());
    assert_eq!(w.remove(0, exit, 10), Some(Kind::Tunnel));
    let before = inside(&w, 6).len();
    run(&mut w, 6);
    assert_eq!(inside(&w, 6).len(), before, "items wait where they are");
    assert_eq!(crate_items(&w, 0, 5, 10).len(), source_left, "and nothing new goes in");
    assert!(crate_items(&w, 0, exit + 1, 10).is_empty());
    w.place(0, exit, 10, Kind::Tunnel, Dir::E).unwrap();
    assert!(matches!(w.tile(0, exit, 10).unwrap().st, St::Tunnel { exit: true, .. }));
    run(&mut w, 10);
    assert_eq!(crate_items(&w, 0, exit + 1, 10), vec![Item::Coal; 10], "all of them come out");
}

#[test]
fn items_wait_in_the_tunnel_while_the_exit_front_is_blocked() {
    let mut w = World::blank();
    let exit = pair(&mut w, 30, 4);
    w.place(0, exit + 1, 10, Kind::Pipe, Dir::E).unwrap(); // takes nothing
    run(&mut w, 10);
    let queued = inside(&w, 6);
    assert!(!queued.is_empty());
    assert!(queued.windows(2).all(|p| p[1].1 - p[0].1 >= BELT_GAP - 1e-9), "queue keeps its spacing: {queued:?}");
    assert!(queued[0].1 <= 0.0 + 1e-9, "the first waits at the exit");
    let left = crate_items(&w, 0, 5, 10).len();
    assert!(left > 0, "the source isn't drained: the tunnel is full");
    run(&mut w, 5);
    assert_eq!((inside(&w, 6).len(), crate_items(&w, 0, 5, 10).len()), (queued.len(), left), "nothing moves");
    // Clear the way: everything arrives.
    w.remove(0, exit + 1, 10).unwrap();
    w.place(0, exit + 1, 10, Kind::Crate, Dir::E).unwrap();
    run(&mut w, 30);
    assert_eq!(crate_items(&w, 0, exit + 1, 10).len(), 30);
}

#[test]
fn turning_a_tunnel_keeps_its_role() {
    let mut w = World::blank();
    let exit = pair(&mut w, 3, 4);
    w.place(0, exit + 1, 10, Kind::Crate, Dir::E).unwrap();
    assert_eq!(w.tunnel_exit(0, 6, 10), Some(4));
    w.set_dir(0, exit, 10, Dir::N).unwrap();
    assert!(matches!(w.tile(0, exit, 10).unwrap().st, St::Tunnel { exit: true, .. }), "still an exit");
    assert!(w.tunnel_exit(0, 6, 10).is_none(), "but it no longer faces the entrance's way");
    w.set_dir(0, exit, 10, Dir::E).unwrap();
    assert_eq!(w.tunnel_exit(0, 6, 10), Some(4));
    w.set_dir(0, 6, 10, Dir::N).unwrap();
    assert!(matches!(w.tile(0, 6, 10).unwrap().st, St::Tunnel { exit: false, .. }), "an entrance stays an entrance");
}

#[test]
fn items_in_transit_survive_saving_and_loading() {
    let mut w = World::blank();
    let exit = pair(&mut w, 10, 4);
    w.place(0, exit + 1, 10, Kind::Crate, Dir::E).unwrap();
    run(&mut w, 1);
    let (queued, source) = (inside(&w, 6), crate_items(&w, 0, 5, 10).len());
    assert!(!queued.is_empty() && crate_items(&w, 0, exit + 1, 10).is_empty());
    let path = std::env::temp_dir().join(format!("factory-test-tunnel-{}.json", std::process::id()));
    w.save(&path).unwrap();
    let mut loaded = World::load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(inside(&loaded, 6), queued);
    assert_eq!(crate_items(&loaded, 0, 5, 10).len(), source);
    run(&mut loaded, 10);
    assert_eq!(crate_items(&loaded, 0, exit + 1, 10).len(), 10);
}
