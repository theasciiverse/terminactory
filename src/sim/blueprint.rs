//! Blueprints: a layout copied off the map, turned in the hand and pasted somewhere else.
//!
//! A blueprint is a layout, not a factory: it holds each part's kind, facing and settings (by
//! name, never by index), and never what the parts hold. Its price is not stored: it is always
//! worked out from the parts' current prices.
//!
//! Pasting places every part that fits and that the money covers, in reading order. Parts that
//! are already there cost nothing, so pasting again at the same spot fills in the rest.

use std::collections::HashSet;

use super::*;

/// The version of the blueprint file format (each file carries its own, apart from the save's).
pub const BP_FORMAT: u32 = 1;

/// The most parts a blueprint file may hold (a whole map is far fewer).
pub const BP_MAX_PARTS: usize = (W * H) as usize;

fn is_zero(n: &u8) -> bool {
    *n == 0
}

/// A part's setting as a blueprint keeps it: by name, so a retuned recipe list can't mix them up.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PartSetting {
    /// An arm's or filter splitter's item (None = anything).
    Filter(Option<Item>),
    Sell(SellMode),
    /// A machine's recipe, by `Recipe::name` in the machine's own list.
    Recipe(String),
    Mines(Item),
    /// An elevator door's job on the floor it was copied from.
    Door(DoorMode),
}

/// One part of a blueprint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BpPart {
    /// Top-left of the part's footprint, relative to the blueprint's top-left corner.
    pub dx: i32,
    pub dy: i32,
    pub kind: Kind,
    pub dir: Dir,
    /// Side length of an elevator (0 for every other part).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub size: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<PartSetting>,
    /// A tunnel end: true for an exit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<bool>,
}

impl BpPart {
    /// Footprint (width, height) as the part faces now.
    pub fn dims(&self) -> (i32, i32) {
        dims_of(self.kind, self.dir, (self.size as i32).clamp(MIN_LIFT, MAX_LIFT))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blueprint {
    /// `BP_FORMAT` when it was written.
    pub format: u32,
    pub name: String,
    /// The floor level it was copied on (information only: a blueprint pastes on any floor).
    #[serde(default)]
    pub level: i32,
    /// The tight box around the parts' footprints.
    pub w: i32,
    pub h: i32,
    /// In reading order: top row first, left to right.
    pub parts: Vec<BpPart>,
}

impl Blueprint {
    /// A blueprint of these parts, whose offsets are relative to anything: the box is fitted
    /// around them and they are put in reading order.
    pub fn from_parts(name: &str, level: i32, mut parts: Vec<BpPart>) -> Blueprint {
        let x0 = parts.iter().map(|p| p.dx).min().unwrap_or(0);
        let y0 = parts.iter().map(|p| p.dy).min().unwrap_or(0);
        let (mut w, mut h) = (0, 0);
        for p in &mut parts {
            p.dx -= x0;
            p.dy -= y0;
            let (pw, ph) = p.dims();
            w = w.max(p.dx + pw);
            h = h.max(p.dy + ph);
        }
        parts.sort_by_key(|p| (p.dy, p.dx));
        Blueprint { format: BP_FORMAT, name: name.to_string(), level, w, h, parts }
    }

    pub fn part_count(&self) -> usize {
        self.parts.len()
    }

    /// What every part costs at today's prices (digging and decking not counted: that depends
    /// on where it is pasted).
    pub fn cost(&self) -> f64 {
        self.parts.iter().map(|p| p.kind.price(p.dims().0)).sum()
    }

    /// Each kind used once, in the order they first appear.
    pub fn kinds(&self) -> Vec<Kind> {
        let mut out: Vec<Kind> = Vec::new();
        for p in &self.parts {
            if !out.contains(&p.kind) {
                out.push(p.kind);
            }
        }
        out
    }

    /// Turned `turns` quarter turns clockwise. A part with footprint (pw, ph) at (dx, dy) moves to
    /// (h - dy - ph, dx) in the turned box, which is h wide and w tall; facing parts turn with it
    /// (a long machine's footprint swaps width and height exactly as `Kind::dims` does when its
    /// facing goes from east to south). Outputs follow because they leave from the facing edge.
    pub fn rotated(&self, turns: u8) -> Blueprint {
        let mut bp = self.clone();
        for _ in 0..turns % 4 {
            bp = bp.quarter_turn();
        }
        bp
    }

    /// Flipped left to right (or top to bottom with `vertical`): every part keeps its footprint and
    /// moves to the other side of the box, and facing parts face the other way, so outputs and
    /// tunnel pairs still line up.
    pub fn mirrored(&self, vertical: bool) -> Blueprint {
        let mut parts: Vec<BpPart> = self
            .parts
            .iter()
            .map(|p| {
                let (pw, ph) = p.dims();
                let dir = match (p.kind.directional(), vertical) {
                    (false, _) => p.dir,
                    (true, false) => p.dir.flip_h(),
                    (true, true) => p.dir.flip_v(),
                };
                let (dx, dy) = if vertical { (p.dx, self.h - p.dy - ph) } else { (self.w - p.dx - pw, p.dy) };
                BpPart { dx, dy, dir, ..p.clone() }
            })
            .collect();
        parts.sort_by_key(|p| (p.dy, p.dx));
        Blueprint { parts, ..self.clone() }
    }

    fn quarter_turn(&self) -> Blueprint {
        let mut parts: Vec<BpPart> = self
            .parts
            .iter()
            .map(|p| {
                let (_, ph) = p.dims();
                BpPart { dx: self.h - p.dy - ph, dy: p.dx, dir: if p.kind.directional() { p.dir.cw() } else { p.dir }, ..p.clone() }
            })
            .collect();
        parts.sort_by_key(|p| (p.dy, p.dx));
        Blueprint { format: self.format, name: self.name.clone(), level: self.level, w: self.h, h: self.w, parts }
    }

    /// Whether a file's contents make sense: a sane box, parts inside it, none on top of another.
    /// A hand-edited file can break all of these.
    pub fn check(&self) -> Result<(), String> {
        if self.w < 1 || self.h < 1 || self.w > W || self.h > H {
            return Err(format!("its box is {}x{}", self.w, self.h));
        }
        if self.parts.is_empty() {
            return Err("it has no parts".into());
        }
        if self.parts.len() > BP_MAX_PARTS {
            return Err("it has far too many parts".into());
        }
        let mut taken = HashSet::new();
        for p in &self.parts {
            let (pw, ph) = p.dims();
            if p.dx < 0 || p.dy < 0 || p.dx + pw > self.w || p.dy + ph > self.h {
                return Err(format!("a {} sticks out of its box", p.kind.name()));
            }
            for t in footprint(p.dx, p.dy, pw, ph) {
                if !taken.insert(t) {
                    return Err(format!("two parts overlap at {},{}", t.0, t.1));
                }
            }
        }
        Ok(())
    }

    /// The first kind that isn't unlocked yet, if any (the library greys such a blueprint).
    pub fn first_locked(&self, progress: &Progress) -> Option<Kind> {
        self.kinds().into_iter().find(|k| !progress.part_unlocked(*k))
    }
}

/// What copying an area made.
#[derive(Clone, Debug)]
pub struct Capture {
    pub bp: Blueprint,
    /// Tunnel ends whose partner is outside the copy.
    pub orphan_tunnels: usize,
}

/// Where a part would go, and why not if it can't.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanState {
    /// The identical part is already there: free, skipped.
    Present,
    Fits,
    /// Fits the ground but not the money left (this part and everything after it in reading order).
    Short,
    /// Something is in the way, or it is off the map.
    Blocked,
    /// Not unlocked in this save.
    Locked,
    /// Doesn't suit this floor: a drill or pump above ground, a panel underground, an elevator
    /// with nowhere to go or blocked on another floor.
    WrongFloor,
}

#[derive(Clone, Debug)]
pub struct PlanPart {
    /// Where its top-left tile would be.
    pub x: i32,
    pub y: i32,
    pub state: PlanState,
    /// The full cost (part plus any digging or decking); 0 when it can't be placed.
    pub cost: f64,
}

/// What pasting a blueprint at a spot would do. The ghost and the paste both read this.
#[derive(Clone, Debug, Default)]
pub struct PastePlan {
    /// One per blueprint part, in the same order.
    pub parts: Vec<PlanPart>,
    /// The whole floor is closed to building.
    pub refused: Option<PlaceErr>,
    pub fits: usize,
    pub present: usize,
    pub short: usize,
    pub blocked: usize,
    pub locked: usize,
    pub wrong_floor: usize,
    /// The first locked kind, to name it.
    pub locked_kind: Option<Kind>,
    /// What the parts that fit cost.
    pub cost: f64,
    /// What everything still to place costs (those that fit and those that don't fit the money).
    pub total: f64,
    /// How much more money placing it all would take.
    pub short_by: f64,
}

/// What a paste did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PasteReport {
    pub total: usize,
    pub placed: usize,
    pub present: usize,
    pub short: usize,
    pub blocked: usize,
    pub locked: usize,
    pub wrong_floor: usize,
    pub locked_kind: Option<Kind>,
    pub refused: Option<PlaceErr>,
    /// Money spent.
    pub cost: f64,
    /// How much more money the rest would take.
    pub short_by: f64,
    pub elevators: usize,
    /// Recipes left at the default: not researched, or not on that machine.
    pub recipes_skipped: usize,
}

/// The nearest tunnel behind (x, y) facing `dir`, within reach: how far back, and whether it is an exit.
fn tunnel_behind_at(tiles: &[Option<Building>], x: i32, y: i32, dir: Dir) -> Option<(i32, bool)> {
    let (dx, dy) = dir.delta();
    for k in 1..=TUNNEL_REACH + 1 {
        let (tx, ty) = (x - dx * k, y - dy * k);
        if !in_bounds(tx, ty) {
            return None;
        }
        if let Some(Building { kind: Kind::Tunnel, dir: d, st: St::Tunnel { exit, .. }, .. }) = &tiles[idx(tx, ty)] {
            if *d == dir {
                return Some((k, *exit));
            }
        }
    }
    None
}

/// The setting a blueprint keeps for a building.
fn part_setting(b: &Building) -> Option<PartSetting> {
    match (&b.st, b.kind) {
        (St::Arm { filter, .. }, _) => filter.map(|i| PartSetting::Filter(Some(i))),
        (St::Splitter { filter, .. }, Kind::Sorter) => filter.map(|i| PartSetting::Filter(Some(i))),
        (St::Trade { mode, .. }, _) => Some(PartSetting::Sell(*mode)),
        (St::Crafter { recipe, .. }, kind) => kind.recipes().get(*recipe as usize).map(|r| PartSetting::Recipe(r.name.to_string())),
        (St::Drill { mines, .. }, _) => mines.map(PartSetting::Mines),
        (St::Elevator { mode, .. }, _) => Some(PartSetting::Door(*mode)),
        _ => None,
    }
}

impl World {
    /// Copies every part whose footprint overlaps `r` on floor `f` into a blueprint: the box is the
    /// tight union of their footprints, so it can be bigger than `r`.
    pub fn capture(&self, f: usize, r: TileRect) -> Capture {
        let mut parts = Vec::new();
        let mut tunnels = Vec::new();
        for (x, y) in self.parts_in(f, r) {
            let Some(b) = self.tile(f, x, y) else { continue };
            let exit = match &b.st {
                St::Tunnel { exit, .. } => Some(*exit),
                _ => None,
            };
            if let Some(e) = exit {
                tunnels.push((x, y, b.dir, e));
            }
            parts.push(BpPart {
                dx: x,
                dy: y,
                kind: b.kind,
                dir: b.dir,
                size: if b.kind == Kind::Elevator { b.size } else { 0 },
                set: part_setting(b),
                exit,
            });
        }
        // The ends of a tunnel pair that the copy cuts in two.
        let inside: HashSet<(i32, i32)> = tunnels.iter().map(|t| (t.0, t.1)).collect();
        let tiles = &self.floors[f].tiles;
        let orphan_tunnels = tunnels
            .iter()
            .filter(|&&(x, y, dir, exit)| {
                let (dx, dy) = dir.delta();
                if exit {
                    tunnel_behind_at(tiles, x, y, dir)
                        .is_some_and(|(k, partner_exit)| !partner_exit && !inside.contains(&(x - dx * k, y - dy * k)))
                } else {
                    tunnel_partner(tiles, x, y, dir).is_some_and(|k| !inside.contains(&(x + dx * k, y + dy * k)))
                }
            })
            .count();
        let bp = Blueprint::from_parts("Blueprint", self.floors[f].level, parts);
        Capture { bp, orphan_tunnels }
    }

    /// Whether the very part `p` is already standing with its top-left at (x, y).
    fn is_present(&self, f: usize, x: i32, y: i32, p: &BpPart) -> bool {
        if !in_bounds(x, y) || self.floors[f].anchor_at(x, y) != Some(idx(x, y)) {
            return false;
        }
        self.tile(f, x, y).is_some_and(|b| b.kind == p.kind && b.dir == p.dir && (p.kind != Kind::Elevator || b.size == p.size))
    }

    /// Works out what pasting `bp` with its top-left corner at (ax, ay) on floor `f` would do:
    /// each part is `Present` (already there), `Fits`, `Short` (cut off by the money: that part and
    /// every one after it), or can't go (`Blocked`, `Locked`, `WrongFloor`). Reading order.
    pub fn plan_paste(&self, f: usize, ax: i32, ay: i32, bp: &Blueprint) -> PastePlan {
        let mut plan = PastePlan { parts: Vec::with_capacity(bp.parts.len()), ..Default::default() };
        if !self.floor_open(f) {
            plan.refused = Some(PlaceErr::FloorClosed(self.floors[f].level));
            plan.blocked = bp.parts.len();
            plan.parts = bp.parts.iter().map(|p| PlanPart { x: ax + p.dx, y: ay + p.dy, state: PlanState::Blocked, cost: 0.0 }).collect();
            return plan;
        }
        let mut left = self.econ.money;
        let mut cut = false;
        for p in &bp.parts {
            let (x, y) = (ax + p.dx, ay + p.dy);
            let size = (p.size as i32).clamp(MIN_LIFT, MAX_LIFT);
            let (state, cost) = if self.is_present(f, x, y, p) {
                (PlanState::Present, 0.0)
            } else {
                match self.place_cost(f, x, y, p.kind, p.dir, size) {
                    Err(PlaceErr::Locked(_)) => (PlanState::Locked, 0.0),
                    Err(PlaceErr::NeedsSecondFloor | PlaceErr::OtherFloorBlocked | PlaceErr::NotAboveGround(_) | PlaceErr::NeedsSun) => {
                        (PlanState::WrongFloor, 0.0)
                    }
                    Err(_) => (PlanState::Blocked, 0.0),
                    Ok(cost) if cut || left + 1e-9 < cost => {
                        cut = true;
                        (PlanState::Short, cost)
                    }
                    Ok(cost) => {
                        left -= cost;
                        (PlanState::Fits, cost)
                    }
                }
            };
            match state {
                PlanState::Present => plan.present += 1,
                PlanState::Fits => {
                    plan.fits += 1;
                    plan.cost += cost;
                    plan.total += cost;
                }
                PlanState::Short => {
                    plan.short += 1;
                    plan.total += cost;
                }
                PlanState::Blocked => plan.blocked += 1,
                PlanState::Locked => {
                    plan.locked += 1;
                    plan.locked_kind.get_or_insert(p.kind);
                }
                PlanState::WrongFloor => plan.wrong_floor += 1,
            }
            plan.parts.push(PlanPart { x, y, state, cost });
        }
        plan.short_by = (plan.total - self.econ.money).max(0.0);
        plan
    }

    /// Pastes `bp` with its top-left corner at (ax, ay) on floor `f`: every part that fits, in
    /// reading order, then one rebuild of the grids and pipes. If the money does not cover all of
    /// them, nothing is placed. Doing it again at the same spot places what is left (parts that
    /// were blocked before) and charges nothing twice.
    pub fn paste(&mut self, f: usize, ax: i32, ay: i32, bp: &Blueprint) -> PasteReport {
        let plan = self.plan_paste(f, ax, ay, bp);
        let mut rep = PasteReport {
            total: bp.parts.len(),
            present: plan.present,
            short: plan.short,
            blocked: plan.blocked,
            locked: plan.locked,
            wrong_floor: plan.wrong_floor,
            locked_kind: plan.locked_kind,
            refused: plan.refused.clone(),
            short_by: plan.short_by,
            ..Default::default()
        };
        if plan.refused.is_some() {
            return rep;
        }
        // All or nothing for the money: a blueprint that costs more than you have is not pasted at
        // all, so a half-built layout never happens.
        if plan.short > 0 {
            return rep;
        }
        for (p, pp) in bp.parts.iter().zip(&plan.parts) {
            if pp.state != PlanState::Fits {
                continue;
            }
            let size = (p.size as i32).clamp(MIN_LIFT, MAX_LIFT);
            if self.place_no_rebuild(f, pp.x, pp.y, p.kind, p.dir, size).is_err() {
                rep.blocked += 1;
                continue;
            }
            rep.placed += 1;
            rep.cost += pp.cost;
            rep.elevators += (p.kind == Kind::Elevator) as usize;
            if let Some(set) = &p.set {
                if !self.apply_part_setting(f, pp.x, pp.y, set) {
                    rep.recipes_skipped += 1;
                }
            }
            if let Some(e) = p.exit {
                if let Some(Building { st: St::Tunnel { exit, .. }, .. }) = self.floors[f].tiles[idx(pp.x, pp.y)].as_mut() {
                    *exit = e;
                }
            }
        }
        if rep.placed > 0 {
            self.rebuild_grid();
        }
        rep
    }

    /// Gives the just-placed part at (x, y) a blueprint's setting. False when it had to be left
    /// at the default: a recipe that isn't researched or that this machine doesn't have.
    pub fn apply_part_setting(&mut self, f: usize, x: i32, y: i32, set: &PartSetting) -> bool {
        let Some(a) = self.floors[f].anchor_at(x, y) else { return true };
        let Some(kind) = self.floors[f].tiles[a].as_ref().map(|b| b.kind) else { return true };
        match set {
            PartSetting::Filter(v) => self.apply_setting(f, x, y, Setting::Filter(*v)),
            PartSetting::Sell(m) => self.apply_setting(f, x, y, Setting::Sell(*m)),
            PartSetting::Mines(m) => self.apply_setting(f, x, y, Setting::Mines(*m)),
            PartSetting::Recipe(name) => {
                let Some(i) = kind.recipes().iter().position(|r| r.name == name) else { return false };
                let usable = kind.recipes()[i].needs.is_none_or(|t| self.progress.has(t));
                if !usable {
                    return false;
                }
                self.apply_setting(f, x, y, Setting::Recipe(i as u8));
            }
            PartSetting::Door(m) => {
                if let Some(Building { st: St::Elevator { mode, .. }, .. }) = self.floors[f].tiles[a].as_mut() {
                    *mode = *m;
                }
            }
        }
        true
    }
}
