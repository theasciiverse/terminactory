//! Area tools: which parts a rectangle touches, removing them all at once, and the "last
//! removed" snapshot that puts them back, and the upgrade planner (swap a part for its next
//! version, paying the difference).
//!
//! A batch checks and pays part by part, then rebuilds the power grids and pipe networks once.

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

/// Every upgrade the planner knows: a placed part and the version that takes its place. The pairs
/// are a swap of one machine for another, never a change of stats in place, so each one must keep
/// the footprint (`base_dims`), cost more, and agree with its partner on having a facing (a test
/// enforces all three, so a later pair cannot break a layout). Left out on purpose: arm to long
/// arm (reach changes what it grabs), and pump to advanced pump, crate to warehouse and the other
/// pairs whose footprint grows.
pub const UPGRADES: &[(Kind, Kind)] = &[
    (Kind::Belt, Kind::FastBelt),
    (Kind::Drill, Kind::BasicDrill),
    (Kind::Solar, Kind::BasicSolar),
    (Kind::AdvancedAssembler, Kind::PrecisionAssembler),
    (Kind::LargeBasicAssembler, Kind::LargeAdvancedAssembler),
    (Kind::LargeBasicDrill, Kind::LargeAdvancedDrill),
];

impl Kind {
    /// The version the upgrade planner swaps this part for, if it has one.
    pub fn upgrade(self) -> Option<Kind> {
        UPGRADES.iter().find(|(from, _)| *from == self).map(|(_, to)| *to)
    }

    /// What the swap to `to` costs: the difference in price (an undo pays it back).
    pub fn upgrade_cost(self, to: Kind) -> f64 {
        to.cost() - self.cost()
    }
}

/// What the planner will do with one part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpState {
    /// It gets upgraded, and the money covers it.
    Fits,
    /// The money stops here: this part and every upgradable part after it in reading order.
    Short,
    /// The new version isn't unlocked yet.
    Locked,
    /// A machine whose recipe the new version can't make (a precision assembler makes none of
    /// the crude or basic ones): it is left as it is.
    NoRecipe,
}

/// One part the planner found, with what it would become.
#[derive(Clone, Debug, PartialEq)]
pub struct UpgradePart {
    pub x: i32,
    pub y: i32,
    pub from: Kind,
    pub to: Kind,
    /// The price difference.
    pub cost: f64,
    pub state: UpState,
}

/// What `plan_upgrade` found in a rectangle, in reading order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpgradePlan {
    pub parts: Vec<UpgradePart>,
}

impl UpgradePlan {
    pub fn count(&self, state: UpState) -> usize {
        self.parts.iter().filter(|p| p.state == state).count()
    }
    /// What the parts in `state` cost together.
    pub fn cost(&self, state: UpState) -> f64 {
        self.parts.iter().filter(|p| p.state == state).map(|p| p.cost).sum()
    }
    /// The first part whose new version is locked, for the message.
    pub fn locked_kind(&self) -> Option<Kind> {
        self.parts.iter().find(|p| p.state == UpState::Locked).map(|p| p.to)
    }
    /// How much more money the parts left out would need (given what is in the bank now).
    pub fn short_by(&self, money: f64) -> f64 {
        (self.cost(UpState::Short) - (money - self.cost(UpState::Fits)).max(0.0)).max(0.0)
    }
}

/// What `upgrade_area` did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpgradeReport {
    pub upgraded: usize,
    pub cost: f64,
    /// Left for lack of money, and how much more it would have taken.
    pub short: usize,
    pub short_by: f64,
    pub locked: usize,
    pub locked_kind: Option<Kind>,
    pub no_recipe: usize,
    /// Of the upgraded, how many of each new version.
    pub made: Vec<(Kind, usize)>,
}

/// What the last upgrade did, so `undo_upgrade` can swap the old versions back. Memory only.
#[derive(Clone, Debug)]
pub struct Upgraded {
    pub floor: usize,
    parts: Vec<(i32, i32, Kind, Kind)>,
}

impl Upgraded {
    pub fn len(&self) -> usize {
        self.parts.len()
    }
}

/// What `undo_upgrade` did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UndoUpgradeReport {
    pub reverted: usize,
    /// The difference paid back.
    pub refund: f64,
    /// Not swapped back: the part is gone or is something else now, or it has been set to a
    /// recipe the old version can't make.
    pub kept: usize,
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

    /// What the upgrade planner would do in `r` on floor `f`: every part with a next version, in
    /// reading order. It follows the paste rule: money is counted down the list, and the first
    /// part it can't pay for is `Short`, and so is every upgradable part after it. Locked parts
    /// and machines that can't keep their recipe don't use up any money.
    pub fn plan_upgrade(&self, f: usize, r: TileRect) -> UpgradePlan {
        let mut left = self.econ.money;
        let mut cut = false;
        let mut parts = Vec::new();
        for (x, y) in self.parts_in(f, r) {
            let Some(b) = self.tile(f, x, y) else { continue };
            let Some(to) = b.kind.upgrade() else { continue };
            let cost = b.kind.upgrade_cost(to);
            let state = if !self.progress.part_unlocked(to) {
                UpState::Locked
            } else if !self.keeps_recipe(b, to) {
                UpState::NoRecipe
            } else if cut || left + 1e-9 < cost {
                cut = true;
                UpState::Short
            } else {
                left -= cost;
                UpState::Fits
            };
            parts.push(UpgradePart { x, y, from: b.kind, to, cost, state });
        }
        UpgradePlan { parts }
    }

    /// Can `b` become `to` with its recipe? (Only crafters have one: it is kept by name.)
    fn keeps_recipe(&self, b: &Building, to: Kind) -> bool {
        match (&b.st, b.recipe()) {
            (St::Crafter { .. }, Some(r)) => to.recipes().iter().any(|n| n.name == r.name && n.needs.is_none_or(|t| self.progress.has(t))),
            (St::Crafter { .. }, None) => false,
            _ => true,
        }
    }

    /// `b` as a `to`: the same facing, size and settings, with what it holds where the new part
    /// can hold it. A drill keeps its ore, its output and its fuel; coal beyond what the new drill
    /// can hold is dropped, and a drill that runs on EC has none. A crafter keeps its stock, its
    /// output and its fuel, and its recipe by name (its progress starts over). Belts keep every
    /// item. None when the recipe can't be kept.
    fn converted(&self, b: &Building, to: Kind) -> Option<Building> {
        let mut n = Building { kind: to, ..b.clone() };
        match &mut n.st {
            St::Drill { fuel, coal, .. } => {
                if !to.burns_coal() {
                    *fuel = 0;
                    *coal = 0;
                } else {
                    *coal = (*coal).min(to.drill_coal_cap());
                    if !b.kind.burns_coal() {
                        // From an EC drill to a coal burner: it starts with a coal's worth, as a new one does.
                        if let St::Drill { fuel: start, .. } = Building::new(to, b.dir).st {
                            *fuel = start;
                        }
                    }
                }
            }
            St::Crafter { recipe, progress, busy, .. } => {
                let name = b.recipe()?.name;
                let i = to.recipes().iter().position(|r| r.name == name && r.needs.is_none_or(|t| self.progress.has(t)))?;
                *recipe = i as u8;
                *progress = 0.0;
                *busy = false;
            }
            _ => {}
        }
        Some(n)
    }

    /// Swaps the part anchored at `a` for a `to` in the same footprint.
    fn swap_in_place(&mut self, f: usize, a: usize, to: Kind) -> bool {
        let Some(old) = self.floors[f].tiles[a].as_ref() else { return false };
        let Some(new) = self.converted(old, to) else { return false };
        self.floors[f].tiles[a] = Some(new);
        true
    }

    /// Upgrades every part in `r` that the plan says fits: each swapped for its next version in
    /// place (same footprint, facing, settings and contents as far as the new one can hold them),
    /// paying the price difference, in reading order, stopping when the money runs out. The power
    /// grids and pipe networks are rebuilt once at the end. Also returns what `undo_upgrade`
    /// needs to swap them back.
    pub fn upgrade_area(&mut self, f: usize, r: TileRect) -> (UpgradeReport, Option<Upgraded>) {
        let plan = self.plan_upgrade(f, r);
        let mut report = UpgradeReport {
            short: plan.count(UpState::Short),
            short_by: plan.short_by(self.econ.money),
            locked: plan.count(UpState::Locked),
            locked_kind: plan.locked_kind(),
            no_recipe: plan.count(UpState::NoRecipe),
            ..Default::default()
        };
        let mut done = Vec::new();
        for p in plan.parts.iter().filter(|p| p.state == UpState::Fits) {
            if !self.swap_in_place(f, idx(p.x, p.y), p.to) {
                continue;
            }
            self.econ.money -= p.cost;
            report.cost += p.cost;
            report.upgraded += 1;
            match report.made.iter_mut().find(|(k, _)| *k == p.to) {
                Some((_, n)) => *n += 1,
                None => report.made.push((p.to, 1)),
            }
            done.push((p.x, p.y, p.from, p.to));
        }
        if done.is_empty() {
            return (report, None);
        }
        self.rebuild_grid();
        (report, Some(Upgraded { floor: f, parts: done }))
    }

    /// Swaps the parts of the last upgrade back to their old versions and pays the difference back.
    /// Contents move across as they do going up. A part that has been removed or replaced since is
    /// left alone, and so is one set to a recipe the old version can't make.
    pub fn undo_upgrade(&mut self, undo: &Upgraded) -> UndoUpgradeReport {
        let mut report = UndoUpgradeReport::default();
        let f = undo.floor;
        for &(x, y, from, to) in &undo.parts {
            let a = idx(x, y);
            let here = matches!(&self.floors[f].tiles[a], Some(b) if b.kind == to);
            if !here || !self.swap_in_place(f, a, from) {
                report.kept += 1;
                continue;
            }
            let back = from.upgrade_cost(to);
            self.econ.money += back;
            report.refund += back;
            report.reverted += 1;
        }
        if report.reverted > 0 {
            self.rebuild_grid();
        }
        report
    }
}
