//! Single tests for gaps that don't belong to a bigger topic.

use super::*;

#[test]
fn belts_carry_the_same_in_all_four_directions() {
    // Tiles tick in index order, so east and south belts update before west and north ones.
    // The order must not change how much a belt carries.
    let mut w = World::blank();
    // (crate that feeds, belt tiles, crate that collects), with a panel beside each line.
    let lines = [((20, 20), Dir::E, (26, 20)), ((46, 20), Dir::W, (40, 20)), ((60, 20), Dir::S, (60, 26)), ((80, 26), Dir::N, (80, 20))];
    for ((sx, sy), dir, (ex, ey)) in lines {
        w.place(0, sx, sy, Kind::Crate, dir).unwrap();
        fill_crate(&mut w, 0, sx, sy, &[Item::Coal; 300]);
        let (dx, dy) = dir.delta();
        for k in 1..=5 {
            w.place(0, sx + dx * k, sy + dy * k, Kind::Belt, dir).unwrap();
        }
        w.place(0, ex, ey, Kind::Crate, dir).unwrap();
        let (px, py) = if dx != 0 { (sx + dx * 3, sy + 2) } else { (sx + 2, sy + dy * 3) };
        w.place(0, px, py, Kind::Solar, Dir::E).unwrap();
    }
    for ((sx, sy), dir, _) in lines {
        let (dx, dy) = dir.delta();
        assert!(w.ratio_at(0, sx + dx * 3, sy + dy * 3) > 0.0, "{dir:?} belts are powered");
    }
    run(&mut w, 20);
    let got: Vec<usize> = lines.iter().map(|(_, _, (ex, ey))| crate_items(&w, 0, *ex, *ey).len()).collect();
    assert!(got.iter().all(|n| n.abs_diff(got[0]) <= 1), "E, W, S, N delivered {got:?}");
    assert!(got[0] > 100, "and it is a real flow: {got:?}");
}

#[test]
fn a_splitter_takes_turns_between_two_inputs_when_its_output_is_the_limit() {
    let mut w = World::blank();
    w.place(0, 10, 10, Kind::Splitter, Dir::E).unwrap();
    // Two crates push into it from behind and from above, each with its own item.
    w.place(0, 9, 10, Kind::Crate, Dir::E).unwrap();
    fill_crate(&mut w, 0, 9, 10, &[Item::Coal; 200]);
    w.place(0, 10, 9, Kind::Crate, Dir::S).unwrap();
    fill_crate(&mut w, 0, 10, 9, &[Item::CopperOre; 200]);
    // One output, so the splitter is what limits the flow (8 items a second for both together).
    w.place(0, 11, 10, Kind::Belt, Dir::E).unwrap();
    w.place(0, 12, 10, Kind::Crate, Dir::E).unwrap();
    w.place(0, 11, 12, Kind::Solar, Dir::E).unwrap();
    assert!(w.ratio_at(0, 10, 10) > 0.0);
    run(&mut w, 10);
    let out = crate_items(&w, 0, 12, 10);
    let (coal, ore) = (count(&out, Item::Coal), count(&out, Item::CopperOre));
    assert!(coal + ore >= 60, "{coal} + {ore}");
    assert!(coal.abs_diff(ore) <= 2, "fair: {coal} coal, {ore} ore");
}

#[test]
fn drills_and_pumps_need_ground_and_panels_need_sunlight() {
    let w = World::blank();
    let (up, down) = (w.floor_of(1).unwrap(), w.floor_of(-1).unwrap());
    for kind in [Kind::Drill, Kind::BasicDrill, Kind::Pump] {
        let err = w.check_place(up, 10, 10, kind).unwrap_err();
        assert!(err.contains("need ground"), "{kind:?}: {err}");
        assert!(w.check_place(0, 10, 10, kind).is_ok(), "{kind:?} on the ground");
        assert!(w.check_place(down, 10, 10, kind).is_ok(), "{kind:?} below");
    }
    assert!(w.check_place(down, 10, 10, Kind::Solar).unwrap_err().contains("sunlight"));
    assert!(w.check_place(up, 10, 10, Kind::Solar).is_ok());
}

#[test]
fn merging_three_pipe_networks_keeps_every_drop() {
    let mut w = World::blank();
    // Three separate runs of different length with different amounts of one fluid, joined by two pipes.
    for (x0, n) in [(10, 3), (20, 5), (30, 2)] {
        for x in x0..x0 + n {
            w.place(0, x, 10, Kind::Pipe, Dir::E).unwrap();
        }
    }
    let amounts = [11.5, 40.25, 3.0];
    for (i, a) in amounts.into_iter().enumerate() {
        let p = w.fluids.pipe_pool[0][idx([10, 20, 30][i], 10)] as usize;
        w.fluids.pools[p].add(Fluid::Water, a);
    }
    for x in (13..20).chain(25..30) {
        w.place(0, x, 10, Kind::Pipe, Dir::E).unwrap();
    }
    assert_eq!(w.fluids.pools.len(), 1);
    let total: f64 = amounts.iter().sum();
    assert!((w.fluids.pools[0].amount - total).abs() < 1e-9, "{} vs {total}", w.fluids.pools[0].amount);
}
