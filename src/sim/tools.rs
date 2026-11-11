//! Area tools: which parts a rectangle touches, removing them all at once, and the "last
//! removed" snapshot that puts them back. Copy and upgrade will join this file.
//!
//! A batch checks and pays part by part, then rebuilds the power grids and pipe networks once.

#![allow(dead_code)] // until the area tool in app.rs uses it

use super::*;

/// A rectangle of tiles with both corners inclusive: (x0, y0, x1, y1), x0 <= x1 and y0 <= y1.
pub type TileRect = (i32, i32, i32, i32);

/// The rectangle between two corner tiles, whichever way round they are.
pub fn rect_of(a: (i32, i32), b: (i32, i32)) -> TileRect {
    (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1))
}

/// Width and height in tiles.
pub fn rect_size((x0, y0, x1, y1): TileRect) -> (i32, i32) {
    (x1 - x0 + 1, y1 - y0 + 1)
}

/// What a rectangle holds, for the sidebar and the confirmation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AreaInfo {
    pub parts: usize,
    /// Of those, elevators (each goes with all its doors and its car).
    pub elevators: usize,
    /// What removing everything pays back: every part's full price.
    pub refund: f64,
    /// Waste units held in crates, warehouses and elevators, and the fee to remove them.
    pub waste: usize,
    pub fee: f64,
}

/// What `remove_area` did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoveReport {
    pub parts: usize,
    pub elevators: usize,
    pub refund: f64,
    /// Waste fee paid, and the units it covered.
    pub fee: f64,
    pub waste: usize,
    /// Parts kept because their waste fee couldn't be paid, and what those fees come to.
    pub kept: usize,
    pub kept_fee: f64,
}

/// One part taken out by `remove_area`, whole: with its state, and for an elevator its doors on
/// the other floors and its car.
#[derive(Clone, Debug)]
pub struct RemovedPart {
    x: i32,
    y: i32,
    main: Building,
    doors: Vec<(usize, Building)>,
    car: Option<Car>,
}

/// Everything the last area deconstruct took, so one action can put it back. Memory only: it is
/// not saved. (When blueprints arrive, this becomes the "Last removed" blueprint.)
#[derive(Clone, Debug)]
pub struct Removed {
    /// The floor they were on.
    pub floor: usize,
    parts: Vec<RemovedPart>,
}

impl Removed {
    pub fn len(&self) -> usize {
        self.parts.len()
    }
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }
    /// What putting everything back costs: the full price of each part.
    pub fn price(&self) -> f64 {
        self.parts.iter().map(|p| p.main.kind.price(p.main.dims().0)).sum()
    }
}

/// What `restore_removed` did. The parts that weren't put back stay in the `Removed`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RestoreReport {
    pub placed: usize,
    /// Something is in the way now.
    pub blocked: usize,
    /// Left for lack of money (it stops at the first part it can't pay for).
    pub short: usize,
    pub price: f64,
}

impl World {
    /// Anchors (top-left tiles) of the parts whose footprint overlaps `r` on floor `f`, in reading
    /// order. A long machine with one tile in the rectangle counts whole, once.
    pub fn parts_in(&self, f: usize, r: TileRect) -> Vec<(i32, i32)> {
        let fl = &self.floors[f];
        let mut found: Vec<u32> = Vec::new();
        for y in r.1.max(0)..=r.3.min(H - 1) {
            for x in r.0.max(0)..=r.2.min(W - 1) {
                if let Some(a) = fl.owner[idx(x, y)] {
                    found.push(a);
                }
            }
        }
        found.sort_unstable();
        found.dedup();
        found.into_iter().map(|a| pos_of(a as usize)).collect()
    }

    /// Counts for the parts `parts_in` finds.
    pub fn area_info(&self, f: usize, r: TileRect) -> AreaInfo {
        let mut info = AreaInfo::default();
        for (x, y) in self.parts_in(f, r) {
            let Some(b) = self.tile(f, x, y) else { continue };
            info.parts += 1;
            info.elevators += (b.kind == Kind::Elevator) as usize;
            info.refund += b.kind.price(b.dims().0);
            let (n, fee) = self.removal_fee(f, x, y);
            info.waste += n;
            info.fee += fee;
        }
        info
    }

    /// Removes every part that overlaps `r` on floor `f`, refunding each one's full price. A
    /// part holding waste pays its fee first (as one-by-one removal does) and is kept when the
    /// fee can't be paid at that moment. Elevators go with all their doors and their car. The
    /// power grids and pipe networks are rebuilt once at the end. Returns what was done and a
    /// snapshot that `restore_removed` can put back.
    pub fn remove_area(&mut self, f: usize, r: TileRect) -> (RemoveReport, Option<Removed>) {
        let mut report = RemoveReport::default();
        let mut taken = Vec::new();
        for (x, y) in self.parts_in(f, r) {
            let (n, fee) = self.removal_fee(f, x, y);
            if n > 0 && self.econ.money < fee {
                report.kept += 1;
                report.kept_fee += fee;
                continue;
            }
            let Some(snap) = self.snapshot_part(f, x, y) else { continue };
            if n > 0 {
                self.econ.pay_waste(n);
                report.fee += fee;
                report.waste += n;
            }
            report.refund += snap.main.kind.price(snap.main.dims().0);
            report.elevators += (snap.main.kind == Kind::Elevator) as usize;
            self.remove_no_rebuild(f, x, y);
            taken.push(snap);
        }
        report.parts = taken.len();
        if taken.is_empty() {
            return (report, None);
        }
        self.rebuild_grid();
        (report, Some(Removed { floor: f, parts: taken }))
    }

    fn snapshot_part(&self, f: usize, x: i32, y: i32) -> Option<RemovedPart> {
        let a = self.floors[f].anchor_at(x, y)?;
        let main = self.floors[f].tiles[a].clone()?;
        let (mut doors, mut car) = (Vec::new(), None);
        if main.kind == Kind::Elevator {
            for (g, fl) in self.floors.iter().enumerate() {
                match &fl.tiles[a] {
                    Some(d) if g != f && d.kind == Kind::Elevator => doors.push((g, d.clone())),
                    _ => {}
                }
            }
            car = self.lifts.iter().find(|c| c.anchor == a).cloned();
        }
        let (x, y) = pos_of(a);
        Some(RemovedPart { x, y, main, doors, car })
    }

    /// Puts back what an area deconstruct took, in reading order, paying each part's full price
    /// (the refund it gave). Parts keep their settings and what they held. A part that something
    /// now sits on is skipped; it stops for good at the first part the money doesn't cover. Parts
    /// not put back stay in `undo`, so pressing again after clearing the way finishes the job.
    pub fn restore_removed(&mut self, undo: &mut Removed) -> RestoreReport {
        let mut report = RestoreReport::default();
        let f = undo.floor;
        let mut left = Vec::new();
        for p in std::mem::take(&mut undo.parts) {
            if report.short > 0 {
                report.short += 1;
                left.push(p);
                continue;
            }
            let b = &p.main;
            let cost = match self.place_cost(f, p.x, p.y, b.kind, b.dir, b.size as i32) {
                Ok(cost) => cost,
                Err(_) => {
                    report.blocked += 1;
                    left.push(p);
                    continue;
                }
            };
            if self.econ.money + 1e-9 < cost {
                report.short += 1;
                left.push(p);
                continue;
            }
            self.econ.money -= cost;
            report.price += cost;
            report.placed += 1;
            if p.main.kind == Kind::Elevator {
                // A door on every open floor: the saved ones, and a free unload door on any floor
                // that has opened since.
                for g in 0..self.floors.len() {
                    if g == f || !self.floor_open(g) {
                        continue;
                    }
                    let door = match p.doors.iter().find(|(h, _)| *h == g) {
                        Some((_, d)) => d.clone(),
                        None => {
                            let mut d = p.main.clone();
                            if let St::Elevator { mode, inbox, outbox } = &mut d.st {
                                *mode = DoorMode::Unload;
                                inbox.clear();
                                outbox.clear();
                            }
                            d
                        }
                    };
                    self.put(g, p.x, p.y, door);
                }
                if let Some(car) = p.car {
                    self.lifts.push(car);
                }
            }
            self.put(f, p.x, p.y, p.main);
        }
        undo.parts = left;
        if report.placed > 0 {
            self.rebuild_grid();
        }
        report
    }
}
