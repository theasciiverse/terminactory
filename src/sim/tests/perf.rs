//! A simple benchmark harness, run by hand (no new dependencies):
//!
//!     cargo test --release perf -- --ignored --nocapture
//!
//! Budgets: 16.7 ms per tick at 60 TPS, 33 ms per frame at 30 FPS.

use super::*;
use std::time::Instant;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn rich() -> World {
    let mut w = World::new();
    w.econ.money = 1e12;
    w.progress.opened = [true; COMPANIES];
    w.progress.researched = [true; TECHS];
    w
}

fn belt_items(w: &World) -> usize {
    w.floors[0].tiles.iter().flatten().map(|b| if let St::Belt { items } = &b.st { items.len() } else { 0 }).sum()
}

/// A closed loop of belts 98 tiles wide and 4 tall, three items on every tile, with panels and poles.
fn ring(w: &mut World, y: i32, load: usize) {
    let (x0, x1, y0, y1) = (2, 100, y, y + 4);
    for x in x0..x1 {
        w.place(0, x, y0, Kind::Belt, Dir::E).unwrap();
    }
    for yy in y0..y1 {
        w.place(0, x1, yy, Kind::Belt, Dir::S).unwrap();
    }
    for x in (x0 + 1..=x1).rev() {
        w.place(0, x, y1, Kind::Belt, Dir::W).unwrap();
    }
    for yy in (y0 + 1..=y1).rev() {
        w.place(0, x0, yy, Kind::Belt, Dir::N).unwrap();
    }
    for k in 0..12 {
        w.place(0, 6 + k * 8, y + 1, Kind::Solar, Dir::E).unwrap();
    }
    for k in 0..16 {
        w.place(0, 5 + k * 6, y + 3, Kind::Pole, Dir::E).unwrap();
    }
    for yy in y0..=y1 {
        for x in x0..=x1 {
            if let Some(Building { st: St::Belt { items }, .. }) = w.floors[0].tiles[idx(x, yy)].as_mut() {
                for k in 0..load {
                    items.push(BeltItem { item: Item::CopperOre, pos: 0.75 - 0.25 * k as f64 });
                }
            }
        }
    }
}

fn ms_per_tick(w: &mut World, n: u32) -> f64 {
    let t = Instant::now();
    for _ in 0..n {
        w.tick();
    }
    ms(t) / n as f64
}

#[test]
#[ignore = "benchmark: cargo test --release perf -- --ignored --nocapture"]
fn perf_tick_rings() {
    let mut last = 0.0;
    for rings in [1, 4, 8, 12] {
        let mut w = rich();
        for r in 0..rings {
            ring(&mut w, 2 + r * 6, 3);
        }
        let buildings: usize = w.floors.iter().map(|f| f.tiles.iter().flatten().count()).sum();
        let items = belt_items(&w);
        ms_per_tick(&mut w, 300); // warm up
        last = ms_per_tick(&mut w, 600);
        println!("rings {rings:>2}: {buildings} buildings, {items} belt items: {last:.3} ms/tick (budget 16.7)");
        let t = Instant::now();
        let _ = w.idle_summary();
        println!("         idle_summary {:.3} ms", ms(t));
        let t = Instant::now();
        w.place(0, 1, 79, Kind::Pole, Dir::E).ok();
        println!("         one place() (rebuild_grid + rebuild_fluids) {:.3} ms", ms(t));
    }
    // A loose guard for release builds only: 12 rings was 0.42 ms before the active-tile list.
    if !cfg!(debug_assertions) {
        assert!(last < 2.0, "12 rings took {last:.3} ms per tick");
    }
}

#[test]
#[ignore = "benchmark: cargo test --release perf -- --ignored --nocapture"]
fn perf_rebuild_grid() {
    for n in [100usize, 500, 1500, 2400] {
        let mut w = rich();
        let mut placed = 0;
        'fill: for y in (0..H).step_by(2) {
            for x in (0..W).step_by(2) {
                if placed >= n {
                    break 'fill;
                }
                if w.place(0, x, y, Kind::Pole, Dir::E).is_ok() {
                    placed += 1;
                }
            }
        }
        let t = Instant::now();
        for _ in 0..5 {
            w.rebuild_grid();
        }
        println!("{placed:>5} poles: rebuild_grid {:.2} ms", ms(t) / 5.0);
    }
}

#[test]
#[ignore = "benchmark: cargo test --release perf -- --ignored --nocapture"]
fn perf_draw() {
    use ratatui::{backend::TestBackend, Terminal};
    let mut w = rich();
    // A dense area: 96 large assemblers with belts between and below.
    for r in 0..8 {
        for c in 0..12 {
            w.place(0, 2 + c * 9, 2 + r * 9, Kind::LargeBasicAssembler, Dir::E).unwrap();
            w.place(0, 8 + c * 9, 2 + r * 9, Kind::Belt, Dir::E).ok();
            for k in 0..4 {
                w.place(0, 2 + c * 9 + k, 8 + r * 9, Kind::Belt, Dir::E).unwrap();
            }
        }
    }
    let mut app = crate::app::App::new(w);
    app.cursor = (50, 40);
    for (cols, rows) in [(120u16, 40u16), (240, 70), (400, 120)] {
        let mut term = Terminal::new(TestBackend::new(cols, rows)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        let t = Instant::now();
        for _ in 0..50 {
            term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        }
        println!("draw {cols}x{rows}: {:.3} ms/frame (budget 33)", ms(t) / 50.0);
    }
}

#[test]
#[ignore = "benchmark: cargo test --release perf -- --ignored --nocapture"]
fn perf_save_load() {
    let mut w = rich();
    for r in 0..4 {
        ring(&mut w, 2 + r * 6, 3);
    }
    ms_per_tick(&mut w, 300);
    let path = std::env::temp_dir().join(format!("factory-perf-{}.json", std::process::id()));
    let t = Instant::now();
    w.save(&path).unwrap();
    let save = ms(t);
    let kb = std::fs::metadata(&path).unwrap().len() / 1024;
    let t = Instant::now();
    let loaded = World::load(&path).unwrap();
    let load = ms(t);
    let _ = std::fs::remove_file(&path);
    println!("save {save:.1} ms, load {load:.1} ms, {kb} KB");
    assert_eq!(belt_items(&loaded), belt_items(&w));
}

/// Every tile holds an `Option<Building>`, and a machine's stock is an array with a slot per item,
/// so each new item makes every tile bigger. This catches the growth early.
#[test]
#[ignore = "benchmark: cargo test --release perf -- --ignored --nocapture"]
fn perf_building_size() {
    let size = std::mem::size_of::<Building>();
    println!(
        "Building {size} bytes, St {} bytes, Option<Building> {} bytes, ITEMS {ITEMS}",
        std::mem::size_of::<St>(),
        std::mem::size_of::<Option<Building>>()
    );
    assert!(size <= 400, "Building grew to {size} bytes: box the crafter state (see plan 05, risk 2)");
}
