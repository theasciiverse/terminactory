//! Simulation core: the world, buildings and the fixed-tick update.
//! Nothing here draws or reads input, so another frontend can reuse it later.
//! There is no randomness during play: the same factory always behaves the same.
//!
//! Buildings can be bigger than one tile (2x2, 3x3). Each building is stored at its
//! top-left "anchor" tile, and every tile it covers points back to that anchor.
//! Buildings take input from any side and send output out of their front only.

use std::collections::VecDeque;
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize};

pub use crate::content::*;

pub const W: i32 = 120;
pub const H: i32 = 80;
pub const TPS: u64 = 60;
pub const SPAWN: (i32, i32) = (60, 40);
pub const START_MONEY: f64 = 1000.0;
/// Floors in the save, by index: ground first so older saves keep their indices.
pub const LEVELS: [i32; 7] = [0, -1, 1, -2, 2, -3, 3];

// Rates (starting points for tuning).
const BELT_SPEED: f64 = 2.0 / TPS as f64; // tiles per tick
const FAST_BELT_SPEED: f64 = 4.0 / TPS as f64; // twice as fast
const BELT_GAP: f64 = 0.25; // 4 items per tile -> 8 items/s
const SPLITTER_TICKS: f64 = 60.0 / 8.0; // 8 items/s
const ARM_TICKS: f64 = 30.0; // a grab or a drop takes 0.5 s
/// Tiles a belt tunnel can pass under (its exit sits at most this many + 1 ahead).
pub const TUNNEL_REACH: i32 = 4;
/// A warehouse holds this many crates' worth.
pub const WAREHOUSE_CRATES: usize = 4;
// Elevators: travel only when full; 0.5 s doors, 1 s per floor; load and unload quickly.
pub const LIFT_PER_TILE: usize = 10;
pub const DOOR_TICKS: u32 = TPS as u32 / 2;
pub const FLOOR_TICKS: u32 = TPS as u32;
const LIFT_RATE: f64 = 30.0 / TPS as f64; // items per tick
pub const MIN_LIFT: i32 = 2;
pub const MAX_LIFT: i32 = 8;
pub const SOLAR_OUTPUT: f64 = 10.0;
/// How far beyond its own edge each power source reaches.
pub const SOLAR_REACH: i32 = 2;
pub const POLE_REACH: i32 = 3;
pub const ELEVATOR_REACH: i32 = 1;

pub const DRILL_COAL_CAP: u32 = 10;
pub const DRILL_OUT_CAP: usize = 4;
pub const CRAFTER_OUT_CAP: usize = 10;
pub const CRATE_CAP: usize = 400;

// Fluids and steam (per second unless noted).
pub const PIPE_CAP: f64 = 20.0; // fluid each pipe tile holds
pub const PUMP_RATE: f64 = 20.0; // water a pump adds
pub const BOILER_RATE: f64 = 10.0; // water in, steam out
pub const ENGINE_POWER: f64 = 40.0; // power at full steam
const STEAM_PER_TICK: f64 = 10.0 / TPS as f64; // steam an engine burns per tick at full power
pub const ENGINE_BUF: f64 = 20.0;
pub const BOILER_COAL_CAP: u32 = 10;
pub const ENGINE_REACH: i32 = 2;
/// Bump when the save format changes: older saves are not loaded.
pub const SAVE_VERSION: u32 = 1;

/// Reads a saved list into a fixed-size array (serde itself stops at 32), padding with defaults.
fn pad<'de, D, T, const N: usize>(d: D) -> Result<[T; N], D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default + Copy,
{
    let v = Vec::<T>::deserialize(d)?;
    let mut a = [T::default(); N];
    for (i, x) in v.into_iter().take(N).enumerate() {
        a[i] = x;
    }
    Ok(a)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Dir {
    N,
    E,
    S,
    W,
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::N, Dir::E, Dir::S, Dir::W];
    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::N => (0, -1),
            Dir::E => (1, 0),
            Dir::S => (0, 1),
            Dir::W => (-1, 0),
        }
    }
    pub fn cw(self) -> Dir {
        match self {
            Dir::N => Dir::E,
            Dir::E => Dir::S,
            Dir::S => Dir::W,
            Dir::W => Dir::N,
        }
    }
    pub fn opposite(self) -> Dir {
        self.cw().cw()
    }
    pub fn idx(self) -> usize {
        self as usize
    }
    pub fn arrow(self) -> char {
        match self {
            Dir::N => '^',
            Dir::E => '>',
            Dir::S => 'v',
            Dir::W => '<',
        }
    }
}

/// A part's setting, copied with a middle-click and given to each copy placed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Setting {
    Filter(Option<Item>),
    Sell(SellMode),
    Recipe(u8),
    Mines(Item),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SellMode {
    Money,
    Split,
    Research,
}

impl SellMode {
    pub fn next(self) -> Self {
        match self {
            SellMode::Money => SellMode::Split,
            SellMode::Split => SellMode::Research,
            SellMode::Research => SellMode::Money,
        }
    }
    /// (share of money, share of research)
    pub fn split(self) -> (f64, f64) {
        match self {
            SellMode::Money => (1.0, 0.0),
            SellMode::Split => (0.4, 0.4),
            SellMode::Research => (0.0, 1.0),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            SellMode::Money => "sell for money",
            SellMode::Split => "split 40/40",
            SellMode::Research => "sell for research",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Deposit {
    None,
    /// Ore patches and lakes are gone (drills and pumps work anywhere). Old saves still
    /// name them; loading clears them.
    Copper,
    Coal,
    Iron,
    Water,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Kind {
    Drill,
    BasicDrill,
    Belt,
    Splitter,
    Arm,
    Furnace,
    Press,
    Crate,
    TradePost,
    Solar,
    Pole,
    Elevator,
    Pipe,
    Pump,
    BlastFurnace,
    Boiler,
    SteamEngine,
    Assembler,
    FastBelt,
    Tunnel,
    LongArm,
    Sorter,
    Warehouse,
}

impl Kind {
    /// Every part (the build menu itself is laid out in app::slots).
    #[cfg_attr(not(test), allow(dead_code))]
    pub const ALL: [Kind; 23] = [
        Kind::Drill,
        Kind::BasicDrill,
        Kind::Belt,
        Kind::FastBelt,
        Kind::Tunnel,
        Kind::Splitter,
        Kind::Sorter,
        Kind::Arm,
        Kind::LongArm,
        Kind::Warehouse,
        Kind::Pipe,
        Kind::Pump,
        Kind::Furnace,
        Kind::Press,
        Kind::BlastFurnace,
        Kind::Assembler,
        Kind::Crate,
        Kind::Solar,
        Kind::Pole,
        Kind::Boiler,
        Kind::SteamEngine,
        Kind::TradePost,
        Kind::Elevator,
    ];
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn category(self) -> Category {
        match self {
            Kind::Drill | Kind::BasicDrill | Kind::Furnace | Kind::BlastFurnace | Kind::Press | Kind::Assembler | Kind::Pump | Kind::Pipe => {
                Category::Create
            }
            Kind::Belt | Kind::FastBelt | Kind::Tunnel | Kind::Splitter | Kind::Sorter | Kind::Arm | Kind::LongArm | Kind::Elevator => {
                Category::Transport
            }
            Kind::Solar | Kind::Pole | Kind::Boiler | Kind::SteamEngine => Category::Power,
            Kind::Crate | Kind::Warehouse | Kind::TradePost => Category::Misc,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Kind::Drill => "Cheap drill",
            Kind::BasicDrill => "Basic drill",
            Kind::Belt => "Basic belt",
            Kind::FastBelt => "Fast belt",
            Kind::Splitter => "Basic splitter",
            Kind::Arm => "Filtering arm",
            Kind::Furnace => "Copper furnace",
            Kind::Press => "Copper press",
            Kind::Assembler => "Assembler",
            Kind::Crate => "Crate",
            Kind::TradePost => "Trade post",
            Kind::Solar => "Cheap solar panel",
            Kind::Pole => "Power pole",
            Kind::Elevator => "Elevator",
            Kind::Pipe => "Pipe",
            Kind::Pump => "Water pump",
            Kind::BlastFurnace => "Blast furnace",
            Kind::Boiler => "Boiler",
            Kind::SteamEngine => "Steam engine",
            Kind::Tunnel => "Belt tunnel",
            Kind::LongArm => "Long arm",
            Kind::Sorter => "Filter splitter",
            Kind::Warehouse => "Warehouse",
        }
    }
    /// Price at the default size (elevators cost more the bigger they are: see `price`).
    pub fn cost(self) -> f64 {
        match self {
            Kind::Drill => 50.0,
            Kind::BasicDrill => 120.0,
            Kind::Belt => 5.0,
            Kind::FastBelt => 12.0,
            Kind::Splitter => 10.0,
            Kind::Arm => 150.0,
            Kind::Furnace => 200.0,
            Kind::Press => 150.0,
            Kind::Assembler => 500.0,
            Kind::Crate => 50.0,
            Kind::TradePost => 0.0,
            Kind::Solar => 100.0,
            Kind::Pole => 10.0,
            Kind::Elevator => 300.0,
            Kind::Pipe => 5.0,
            Kind::Pump => 80.0,
            Kind::BlastFurnace => 300.0,
            Kind::Boiler => 150.0,
            Kind::SteamEngine => 250.0,
            Kind::Tunnel => 25.0,
            Kind::LongArm => 250.0,
            Kind::Sorter => 60.0,
            Kind::Warehouse => 300.0,
        }
    }
    /// Price at a given size: elevators cost $75 per tile they cover.
    pub fn price(self, size: i32) -> f64 {
        if self == Kind::Elevator {
            75.0 * (size * size) as f64
        } else {
            self.cost()
        }
    }
    /// Default side length in tiles: logistics are small, production is big.
    /// Elevators can be built from 2x2 up to 8x8.
    pub fn size(self) -> i32 {
        match self {
            Kind::Drill
            | Kind::BasicDrill
            | Kind::Furnace
            | Kind::Press
            | Kind::BlastFurnace
            | Kind::Solar
            | Kind::Elevator
            | Kind::Boiler
            | Kind::SteamEngine
            | Kind::Warehouse => 2,
            Kind::TradePost | Kind::Assembler => 3,
            Kind::Belt
            | Kind::FastBelt
            | Kind::Tunnel
            | Kind::Splitter
            | Kind::Sorter
            | Kind::Arm
            | Kind::LongArm
            | Kind::Crate
            | Kind::Pole
            | Kind::Pipe
            | Kind::Pump => 1,
        }
    }
    pub fn power_draw(self) -> f64 {
        match self {
            Kind::Belt => 0.5,
            Kind::FastBelt | Kind::Pump => 1.0,
            Kind::Splitter | Kind::Sorter => 2.0,
            Kind::Press => 3.0,
            Kind::Assembler => 5.0,
            Kind::Arm => 10.0,
            Kind::LongArm => 15.0,
            _ => 0.0,
        }
    }
    /// Has a front that can be rotated (belts and arms move that way; buildings output there).
    pub fn directional(self) -> bool {
        !matches!(
            self,
            Kind::Splitter | Kind::TradePost | Kind::Solar | Kind::Pole | Kind::Pipe | Kind::Pump | Kind::SteamEngine
        )
    }
    pub fn is_belt(self) -> bool {
        matches!(self, Kind::Belt | Kind::FastBelt)
    }
    /// Connects to neighbouring pipes.
    pub fn uses_pipes(self) -> bool {
        matches!(self, Kind::Pipe | Kind::Pump | Kind::Boiler | Kind::SteamEngine)
    }
    /// Not part of the current tier; nothing is a preview any more.
    pub fn preview(self) -> bool {
        false
    }
    /// The company that makes it, for the story.
    pub fn company(self) -> Option<Company> {
        match self {
            Kind::Drill => Some(Company::Hollowmere),
            Kind::BasicDrill => Some(Company::Ashgrove),
            Kind::Pipe | Kind::Pump => Some(Company::Wellbourne),
            Kind::BlastFurnace => Some(Company::Cinderhall),
            Kind::Boiler | Kind::SteamEngine => Some(Company::Kettering),
            Kind::Elevator => Some(Company::Underhill),
            Kind::Assembler => Some(Company::Gearwright),
            Kind::FastBelt => Some(Company::Swiftline),
            _ => Some(Company::Brannock),
        }
    }
    /// What must be unlocked before it can be bought (elevators: see `Progress::part_unlocked`).
    pub fn tech(self) -> Option<Tech> {
        match self {
            Kind::Press => Some(Tech::Press),
            Kind::BasicDrill => Some(Tech::BasicDrill),
            Kind::Pipe | Kind::Pump => Some(Tech::Pumps),
            Kind::BlastFurnace => Some(Tech::BlastFurnace),
            Kind::Boiler | Kind::SteamEngine => Some(Tech::Steam),
            Kind::Assembler => Some(Tech::Assembler),
            Kind::FastBelt => Some(Tech::FastBelts),
            Kind::Tunnel => Some(Tech::Tunnels),
            Kind::LongArm => Some(Tech::LongArms),
            Kind::Sorter => Some(Tech::FilterSplitters),
            Kind::Warehouse => Some(Tech::Warehouses),
            _ => None,
        }
    }
    pub fn recipes(self) -> &'static [Recipe] {
        match self {
            Kind::Furnace => FURNACE_RECIPES,
            Kind::Press => PRESS_RECIPES,
            Kind::BlastFurnace => BLAST_RECIPES,
            Kind::Assembler => ASSEMBLER_RECIPES,
            _ => &[],
        }
    }
    fn is_drill(self) -> bool {
        matches!(self, Kind::Drill | Kind::BasicDrill)
    }
    /// Ticks per item mined (on the surface), and items mined per coal burned.
    fn drill_speed(self) -> (u32, u32) {
        match self {
            Kind::BasicDrill => (40, 10),
            _ => (60, 8),
        }
    }
    /// What a drill can be set to mine (f cycles through these). Cheap drills can't cut iron.
    pub fn ores(self) -> &'static [Item] {
        match self {
            Kind::Drill => &[Item::CopperOre, Item::Coal],
            Kind::BasicDrill => &[Item::CopperOre, Item::Coal, Item::IronOre],
            _ => &[],
        }
    }
}

/// Deeper ore is richer: drills mine 1.5x faster on floor -1, 2x on -2, 3x on -3.
pub fn richness(level: i32) -> f64 {
    match level {
        -1 => 1.5,
        -2 => 2.0,
        l if l <= -3 => 3.0,
        _ => 1.0,
    }
}

/// What it costs to dig out (underground) or deck over (upper floors) one tile.
pub fn tile_cost(level: i32) -> f64 {
    match level.abs() {
        0 => 0.0,
        1 => 10.0,
        2 => 25.0,
        _ => 50.0,
    }
}

/// An elevator door's job on its floor.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum DoorMode {
    /// Takes items in; the car leaves when it's full.
    #[default]
    Load,
    /// The car brings its load here and unloads out of the door's front.
    Unload,
    /// The car passes by.
    Pass,
}

impl DoorMode {
    pub fn next(self) -> Self {
        match self {
            DoorMode::Load => DoorMode::Unload,
            DoorMode::Unload => DoorMode::Pass,
            DoorMode::Pass => DoorMode::Load,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            DoorMode::Load => "load",
            DoorMode::Unload => "unload",
            DoorMode::Pass => "pass",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum LiftPhase {
    Loading,
    Closing,
    Moving,
    Opening,
    Unloading,
}

/// An elevator car, shared by the elevator's doors on every floor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Car {
    /// The tile index of the elevator's top-left corner (the same on every floor).
    pub anchor: usize,
    pub size: i32,
    /// The floor the car is at, or leaving from.
    pub at: usize,
    pub target: usize,
    pub phase: LiftPhase,
    pub timer: u32,
    pub load: VecDeque<Item>,
    /// Fractional items owed by the load/unload rate.
    pub carry: f64,
}

impl Car {
    pub fn cap(&self) -> usize {
        LIFT_PER_TILE * (self.size * self.size) as usize
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BeltItem {
    pub item: Item,
    /// 0.0 = just entered the tile, 1.0 = at the exit edge.
    pub pos: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum St {
    Drill { mines: Option<Item>, fuel: u32, coal: u32, progress: u32, out: VecDeque<Item> },
    /// Items sorted front first (highest pos first).
    Belt { items: Vec<BeltItem> },
    Splitter {
        buf: VecDeque<Item>,
        cooldown: f64,
        last_in: Option<Dir>,
        tried: u8,
        tried_prev: u8,
        /// Filter splitters: the item that goes out the front (None = any side, like a splitter).
        #[serde(default)]
        filter: Option<Item>,
    },
    Arm { filter: Option<Item>, hold: Option<Item>, progress: f64 },
    /// Furnaces, presses and blast furnaces: a chosen recipe, a stock of inputs, and finished goods.
    Crafter {
        recipe: u8,
        #[serde(deserialize_with = "pad")]
        stock: [u32; ITEMS],
        fuel: u32,
        progress: f64,
        busy: bool,
        out: VecDeque<Item>,
    },
    Crate { items: VecDeque<Item> },
    /// `stuck`: ticks left to show that waste was just turned away.
    Trade {
        mode: SellMode,
        money: f64,
        research: f64,
        count: u64,
        #[serde(default)]
        stuck: u8,
    },
    Solar,
    Pole,
    Elevator {
        inbox: VecDeque<Item>,
        outbox: VecDeque<Item>,
        #[serde(default)]
        mode: DoorMode,
    },
    Pipe,
    Pump { busy: bool },
    /// Burns coal to turn water (from pipes) into steam (out of its front).
    Boiler { fuel: u32, coal: u32, busy: bool },
    /// Turns steam into power for its grid.
    Engine { steam: f64, output: f64 },
    /// A belt tunnel end. An entrance holds what's passing under, with how far each item
    /// still has to go (in tiles); an exit only marks where they come back up.
    Tunnel { exit: bool, items: VecDeque<(Item, f64)> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Building {
    pub kind: Kind,
    pub dir: Dir,
    /// Round-robin index for which output spot gets the next item.
    pub rr: u8,
    pub st: St,
    /// Side length, for parts that come in sizes (0 = the kind's default).
    #[serde(default)]
    pub size: u8,
}

impl Building {
    pub fn new(kind: Kind, dir: Dir) -> Self {
        let st = match kind {
            // A new drill comes with one coal's worth of fuel so it can start.
            Kind::Drill | Kind::BasicDrill => {
                let (t, per_coal) = kind.drill_speed();
                St::Drill { mines: None, fuel: t * per_coal, coal: 0, progress: 0, out: VecDeque::new() }
            }
            Kind::Belt | Kind::FastBelt => St::Belt { items: Vec::new() },
            Kind::Splitter | Kind::Sorter => {
                St::Splitter { buf: VecDeque::new(), cooldown: 0.0, last_in: None, tried: 0, tried_prev: 0, filter: None }
            }
            Kind::Arm | Kind::LongArm => St::Arm { filter: None, hold: None, progress: 0.0 },
            Kind::Furnace | Kind::Press | Kind::BlastFurnace | Kind::Assembler => {
                St::Crafter { recipe: 0, stock: [0; ITEMS], fuel: 0, progress: 0.0, busy: false, out: VecDeque::new() }
            }
            Kind::Crate | Kind::Warehouse => St::Crate { items: VecDeque::new() },
            Kind::Tunnel => St::Tunnel { exit: false, items: VecDeque::new() },
            Kind::TradePost => St::Trade { mode: SellMode::Money, money: 0.0, research: 0.0, count: 0, stuck: 0 },
            Kind::Solar => St::Solar,
            Kind::Pole => St::Pole,
            Kind::Elevator => St::Elevator { inbox: VecDeque::new(), outbox: VecDeque::new(), mode: DoorMode::Load },
            Kind::Pipe => St::Pipe,
            Kind::Pump => St::Pump { busy: false },
            Kind::Boiler => St::Boiler { fuel: 0, coal: 0, busy: false },
            Kind::SteamEngine => St::Engine { steam: 0.0, output: 0.0 },
        };
        Building { kind, dir, rr: 0, st, size: 0 }
    }

    pub fn size(&self) -> i32 {
        if self.size > 0 {
            self.size as i32
        } else {
            self.kind.size()
        }
    }

    /// The recipe a machine is set to.
    pub fn recipe(&self) -> Option<&'static Recipe> {
        match &self.st {
            St::Crafter { recipe, .. } => self.kind.recipes().get(*recipe as usize),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Floor {
    pub level: i32,
    pub deposits: Vec<Deposit>,
    pub dug: Vec<bool>,
    /// Buildings, stored at their anchor (top-left) tile.
    pub tiles: Vec<Option<Building>>,
    /// For every tile: the anchor index of the building covering it.
    pub owner: Vec<Option<u32>>,
}

impl Floor {
    fn new(level: i32, dug: bool) -> Self {
        let n = (W * H) as usize;
        Floor { level, deposits: vec![Deposit::None; n], dug: vec![dug; n], tiles: vec![None; n], owner: vec![None; n] }
    }
    pub fn anchor_at(&self, x: i32, y: i32) -> Option<usize> {
        if !in_bounds(x, y) {
            return None;
        }
        self.owner[idx(x, y)].map(|a| a as usize)
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Econ {
    pub money: f64,
    pub research: f64,
    /// Units sold that earned research, per item (drives the fading curve).
    #[serde(deserialize_with = "pad")]
    pub research_sales: [u64; ITEMS],
    #[serde(deserialize_with = "pad")]
    pub sold: [u64; ITEMS],
    /// Units ever produced by a machine, per item (drives discoveries).
    #[serde(deserialize_with = "pad")]
    pub made: [u64; ITEMS],
    /// This session only: units used up by machines (inputs and fuel), and what each
    /// item's sales brought in. They drive the production panel.
    #[serde(skip)]
    pub used: [u64; ITEMS],
    #[serde(skip)]
    pub money_from: [f64; ITEMS],
    #[serde(skip)]
    pub research_from: [f64; ITEMS],
    /// Everything ever earned (spending doesn't lower these): drives offline earnings.
    #[serde(default)]
    pub earned_money: f64,
    #[serde(default)]
    pub earned_research: f64,
    /// This session only: (money earned, waste fees paid) as they stood a minute ago.
    #[serde(skip)]
    pub minute_ago: (f64, f64),
}

impl Econ {
    pub fn next_research(&self, item: Item) -> f64 {
        item.curve().value(self.research_sales[item.idx()])
    }
    /// (money earned, waste fees paid) over the last minute. Fees aren't counted as earnings.
    pub fn last_minute(&self) -> (f64, f64) {
        let earned: f64 = Item::ALL.iter().filter(|i| **i != Item::Waste).map(|i| self.money_from[i.idx()]).sum();
        let fees = -self.money_from[Item::Waste.idx()];
        (earned - self.minute_ago.0, fees - self.minute_ago.1)
    }
    /// Why a trade post set to `mode` won't take waste right now (None: it will).
    pub fn waste_refusal(&self, mode: SellMode) -> Option<&'static str> {
        match mode {
            SellMode::Research => return Some("set to research"),
            SellMode::Split => return Some("set to the 40/40 split"),
            SellMode::Money => {}
        }
        if self.money < -Item::Waste.price() {
            return Some("you can't afford the fee");
        }
        let (earned, fees) = self.last_minute();
        (fees > earned).then_some("waste costs more than you're earning")
    }
    /// Pays the waste fee for `n` waste thrown away (the same as selling it).
    fn pay_waste(&mut self, n: usize) {
        let fee = n as f64 * Item::Waste.price();
        self.money += fee;
        self.earned_money += fee;
        self.sold[Item::Waste.idx()] += n as u64;
        self.money_from[Item::Waste.idx()] += fee;
    }
    /// Sells one item. Waste costs money and earns no research.
    fn sell(&mut self, item: Item, mode: SellMode) -> (f64, f64) {
        let (mf, rf) = mode.split();
        let m = item.price() * mf;
        let r = if rf > 0.0 && item != Item::Waste {
            let v = self.next_research(item) * rf;
            self.research_sales[item.idx()] += 1;
            v
        } else {
            0.0
        };
        self.money += m;
        self.research += r;
        self.earned_money += m;
        self.earned_research += r;
        self.sold[item.idx()] += 1;
        self.money_from[item.idx()] += m;
        self.research_from[item.idx()] += r;
        (m, r)
    }
}

/// "$100" or "$0.50".
pub fn money_text(v: f64) -> String {
    if v.fract() == 0.0 { format!("${v:.0}") } else { format!("${v:.2}") }
}

/// Running totals, once a second, for the production panel.
#[derive(Clone, Copy, Debug)]
pub struct FlowSample {
    made: [u64; ITEMS],
    used: [u64; ITEMS],
    sold: [u64; ITEMS],
    money: [f64; ITEMS],
    research: [f64; ITEMS],
}

/// One item's last minute, per minute.
#[derive(Clone, Copy, Debug)]
pub struct Flow {
    pub item: Item,
    pub made: f64,
    pub used: f64,
    pub sold: f64,
    pub money: f64,
    pub research: f64,
}

/// Which companies are open and what has been researched.
#[derive(Debug, Serialize, Deserialize)]
pub struct Progress {
    #[serde(deserialize_with = "pad")]
    pub opened: [bool; COMPANIES],
    #[serde(deserialize_with = "pad")]
    pub researched: [bool; TECHS],
    /// Companies that opened since the player last looked (shown as letters).
    #[serde(skip)]
    pub events: VecDeque<Company>,
    /// Short messages for the player (free unlocks).
    #[serde(skip)]
    pub notes: VecDeque<String>,
}

impl Default for Progress {
    fn default() -> Self {
        let mut opened = [false; COMPANIES];
        for c in Company::ALL {
            opened[c.idx()] = c.key_item().is_none();
        }
        Progress { opened, researched: [false; TECHS], events: VecDeque::new(), notes: VecDeque::new() }
    }
}

impl Progress {
    pub fn is_open(&self, c: Company) -> bool {
        self.opened[c.idx()]
    }
    pub fn has(&self, t: Tech) -> bool {
        self.researched[t.idx()]
    }
    /// Can this part be bought right now?
    pub fn part_unlocked(&self, k: Kind) -> bool {
        if k == Kind::Elevator {
            // Either way into a second floor brings elevators.
            return self.has(Tech::SupportBeams) || self.has(Tech::Excavation);
        }
        k.company().map_or(true, |c| self.is_open(c)) && k.tech().map_or(true, |t| self.has(t))
    }
}

/// One power grid: everything linked by overlapping power ranges.
#[derive(Debug, Clone, Copy, Default)]
pub struct Net {
    /// Steady supply from solar panels.
    pub solar: f64,
    /// What this grid's steam engines could give right now.
    pub engine: f64,
    pub supply: f64,
    pub demand: f64,
    /// 0..1: how much of the demand is met. Electric parts on this grid run at this speed.
    pub ratio: f64,
}

/// Solar panels, poles, engines and elevators power every electric part within their range.
/// Sources whose ranges overlap join into one grid (an elevator links both its floors).
#[derive(Default)]
pub struct Grid {
    /// Per floor, per tile: which grid powers it (-1 = none).
    pub cover: Vec<Vec<i32>>,
    pub nets: Vec<Net>,
    /// Electric parts outside every range.
    pub unpowered: u32,
    /// Steam engines: (floor, anchor, grid).
    pub engines: Vec<(usize, usize, usize)>,
    /// Every power source: (floor, anchor, grid). A source's tile can sit in a
    /// neighbour's range, so this, not the painted reach, says which grid it's on.
    pub sources: Vec<(usize, usize, usize)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fluid {
    Water,
    Steam,
}

impl Fluid {
    pub fn name(self) -> &'static str {
        match self {
            Fluid::Water => "water",
            Fluid::Steam => "steam",
        }
    }
}

/// Everything in one connected run of pipes. A pipe network holds one fluid at a time.
#[derive(Clone, Debug, Default)]
pub struct Pool {
    pub fluid: Option<Fluid>,
    pub amount: f64,
    pub cap: f64,
    pub pipes: u32,
}

impl Pool {
    fn room_for(&self, f: Fluid) -> f64 {
        if self.fluid.is_none() || self.fluid == Some(f) {
            self.cap - self.amount
        } else {
            0.0
        }
    }
    fn add(&mut self, f: Fluid, amount: f64) -> f64 {
        let a = amount.min(self.room_for(f)).max(0.0);
        if a > 0.0 {
            self.fluid = Some(f);
            self.amount += a;
        }
        a
    }
    fn take(&mut self, f: Fluid, amount: f64) -> f64 {
        if self.fluid != Some(f) {
            return 0.0;
        }
        let a = amount.min(self.amount);
        self.amount -= a;
        if self.amount < 1e-9 {
            self.amount = 0.0;
            self.fluid = None;
        }
        a
    }
}

#[derive(Default)]
pub struct Fluids {
    /// Per floor, per tile: which pipe network a pipe belongs to (-1 = not a pipe).
    pub pipe_pool: Vec<Vec<i32>>,
    pub pools: Vec<Pool>,
}

/// Which grid powers a building: the first of its tiles that's in range (-1 = none).
/// A big building counts as powered when any part of it is in range.
fn grid_of(cover: &[i32], x: i32, y: i32, s: i32) -> i32 {
    footprint(x, y, s).filter(|&(tx, ty)| in_bounds(tx, ty)).map(|(tx, ty)| cover[idx(tx, ty)]).find(|&id| id >= 0).unwrap_or(-1)
}

/// The tiles a power source reaches: (x0, y0, x1, y1), inclusive.
pub fn coverage(kind: Kind, s: i32, ax: i32, ay: i32) -> Option<(i32, i32, i32, i32)> {
    let r = match kind {
        Kind::Solar => SOLAR_REACH,
        Kind::Pole => POLE_REACH,
        Kind::Elevator => ELEVATOR_REACH,
        Kind::SteamEngine => ENGINE_REACH,
        _ => return None,
    };
    Some((ax - r, ay - r, ax + s - 1 + r, ay + s - 1 + r))
}

fn overlaps(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0 <= b.2 && b.0 <= a.2 && a.1 <= b.3 && b.1 <= a.3
}

#[derive(Serialize, Deserialize)]
pub struct World {
    /// The save format (0 when missing: those saves are from before versions).
    #[serde(default)]
    pub version: u32,
    pub floors: Vec<Floor>,
    pub econ: Econ,
    pub progress: Progress,
    pub tick: u64,
    /// Elevator cars.
    #[serde(default)]
    pub lifts: Vec<Car>,
    /// When the game was last saved (seconds since 1970), for offline earnings.
    #[serde(default)]
    pub saved_at: u64,
    /// Recent earnings per second (money, research), for offline earnings.
    #[serde(default)]
    pub rate: (f64, f64),
    #[serde(skip)]
    pub samples: VecDeque<(f64, f64)>,
    /// Recent research sales per second, per item: research earned while away fades
    /// item by item, just as it does in play.
    #[serde(default)]
    pub sale_rate: Vec<f64>,
    #[serde(skip)]
    pub sale_samples: VecDeque<[u64; ITEMS]>,
    /// The last minute of production, one sample a second, for the production panel.
    #[serde(skip)]
    pub flow_samples: VecDeque<FlowSample>,
    /// Agent messages waiting to be read.
    #[serde(default)]
    pub mail: crate::mail::Queue,
    /// When the last company delivery came with a message, and how many have come.
    #[serde(default)]
    pub last_gift: u64,
    #[serde(default)]
    pub gifts: u32,
    #[serde(skip)]
    pub grid: Grid,
    #[serde(skip)]
    pub fluids: Fluids,
}

/// Seconds between company deliveries that come with agent messages.
pub const GIFT_GAP: u64 = 180;
/// Most agent messages kept waiting to be read.
pub const MAIL_KEEP: usize = 8;

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

pub fn in_bounds(x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < W && y < H
}

pub fn idx(x: i32, y: i32) -> usize {
    (y * W + x) as usize
}

pub fn pos_of(i: usize) -> (i32, i32) {
    ((i as i32) % W, (i as i32) / W)
}

/// Every tile a building of side `s` covers when its top-left is at (x, y).
pub fn footprint(x: i32, y: i32, s: i32) -> impl Iterator<Item = (i32, i32)> {
    (0..s).flat_map(move |dy| (0..s).map(move |dx| (x + dx, y + dy)))
}

/// The j-th tile just outside a building's front edge.
fn front_tile(ax: i32, ay: i32, s: i32, dir: Dir, j: i32) -> (i32, i32) {
    match dir {
        Dir::E => (ax + s, ay + j),
        Dir::W => (ax - 1, ay + j),
        Dir::S => (ax + j, ay + s),
        Dir::N => (ax + j, ay - 1),
    }
}

impl World {
    /// A new campaign: the starting map, $1000 and the two Copper Works companies.
    pub fn new() -> Self {
        let mut w = World {
            version: SAVE_VERSION,
            floors: gen_floors(),
            econ: Econ { money: START_MONEY, ..Default::default() },
            progress: Progress::default(),
            tick: 0,
            lifts: Vec::new(),
            saved_at: 0,
            rate: (0.0, 0.0),
            samples: VecDeque::new(),
            sale_rate: Vec::new(),
            sale_samples: VecDeque::new(),
            flow_samples: VecDeque::new(),
            mail: VecDeque::new(),
            last_gift: 0,
            gifts: 0,
            grid: Grid::default(),
            fluids: Fluids::default(),
        };
        w.rebuild_grid();
        w
    }

    /// Empty, fully dug world with lots of money and everything unlocked, for tests.
    #[cfg(test)]
    pub fn blank() -> Self {
        let mut w = World {
            version: SAVE_VERSION,
            floors: LEVELS.iter().map(|&l| Floor::new(l, true)).collect(),
            econ: Econ { money: 1e6, ..Default::default() },
            progress: Progress {
                opened: [true; COMPANIES],
                researched: [true; TECHS],
                events: VecDeque::new(),
                notes: VecDeque::new(),
            },
            tick: 0,
            lifts: Vec::new(),
            saved_at: 0,
            rate: (0.0, 0.0),
            samples: VecDeque::new(),
            sale_rate: Vec::new(),
            sale_samples: VecDeque::new(),
            flow_samples: VecDeque::new(),
            mail: VecDeque::new(),
            last_gift: 0,
            gifts: 0,
            grid: Grid::default(),
            fluids: Fluids::default(),
        };
        w.rebuild_grid();
        w
    }

    pub fn save(&mut self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        self.saved_at = now_secs();
        let json = serde_json::to_string(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// The format version a save file says it has (0 if it says nothing), if it's JSON at all.
    pub fn file_version(path: &Path) -> Option<u32> {
        #[derive(Deserialize)]
        struct Header {
            #[serde(default)]
            version: u32,
        }
        let json = std::fs::read_to_string(path).ok()?;
        serde_json::from_str::<Header>(&json).ok().map(|h| h.version)
    }

    /// Loads a save. Saves from another version of the format aren't loaded.
    pub fn load(path: &Path) -> std::io::Result<World> {
        if let Some(v) = World::file_version(path).filter(|v| *v != SAVE_VERSION) {
            let why = if v < SAVE_VERSION { "from an older version" } else { "from a newer version" };
            return Err(std::io::Error::new(std::io::ErrorKind::Unsupported, format!("this save is {why}")));
        }
        let json = std::fs::read_to_string(path)?;
        let mut w: World = serde_json::from_str(&json).map_err(std::io::Error::other)?;
        w.rebuild_grid();
        Ok(w)
    }

    /// The building covering tile (x, y), if any.
    pub fn tile(&self, f: usize, x: i32, y: i32) -> Option<&Building> {
        let a = self.floors[f].anchor_at(x, y)?;
        self.floors[f].tiles[a].as_ref()
    }

    /// Top-left tile of the building covering (x, y).
    pub fn anchor_of(&self, f: usize, x: i32, y: i32) -> Option<(i32, i32)> {
        self.floors[f].anchor_at(x, y).map(pos_of)
    }

    /// The floor index for a level (0 = ground, -1 = first floor down, 1 = first floor up...).
    pub fn floor_of(&self, level: i32) -> Option<usize> {
        self.floors.iter().position(|f| f.level == level)
    }

    /// Can the player build on this floor yet? Stronger beams and deeper digging come in later phases.
    pub fn floor_open(&self, f: usize) -> bool {
        match self.floors[f].level {
            0 => true,
            1 => self.progress.has(Tech::SupportBeams),
            -1 => self.progress.has(Tech::Excavation),
            _ => false,
        }
    }

    /// Why a floor is closed, for messages.
    pub fn floor_hint(&self, level: i32) -> &'static str {
        match level {
            1 => "Building upward needs support beams (make steel girders, then see the tree)",
            -1 => "Digging down needs excavation (make steel girders, then see the tree)",
            _ => "That floor opens in a later phase",
        }
    }

    /// The car of the elevator whose corner is at tile index `anchor`.
    pub fn lift(&self, anchor: usize) -> Option<&Car> {
        self.lifts.iter().find(|c| c.anchor == anchor)
    }

    /// Floors with this elevator's doors: (floor index, level, door mode).
    fn doors(&self, anchor: usize) -> Vec<(usize, i32, DoorMode)> {
        self.floors
            .iter()
            .enumerate()
            .filter_map(|(f, fl)| match &fl.tiles[anchor] {
                Some(Building { st: St::Elevator { mode, .. }, .. }) => Some((f, fl.level, *mode)),
                _ => None,
            })
            .collect()
    }

    /// Spends research points on a tech.
    pub fn research(&mut self, t: Tech) -> Result<(), String> {
        if self.progress.has(t) {
            return Err(format!("{} is already researched", t.name()));
        }
        if !self.progress.is_open(t.company()) {
            let item = t.requires().map_or("its key item", |i| i.name());
            return Err(format!("Make {item} first to unlock {}", t.name()));
        }
        if self.econ.research + 1e-9 < t.cost() {
            return Err(format!("Need {:.0} research (you have {:.1})", t.cost(), self.econ.research));
        }
        self.econ.research -= t.cost();
        self.progress.researched[t.idx()] = true;
        Ok(())
    }

    /// Checks a placement (top-left at x, y) and returns its full cost (part + any digging).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn check_place(&self, f: usize, x: i32, y: i32, kind: Kind) -> Result<f64, String> {
        self.check_place_sized(f, x, y, kind, kind.size())
    }

    pub fn check_place_sized(&self, f: usize, x: i32, y: i32, kind: Kind, s: i32) -> Result<f64, String> {
        if !self.progress.part_unlocked(kind) {
            return Err(format!("{} is locked: see the research tree (t)", kind.name()));
        }
        if !self.floor_open(f) {
            return Err(self.floor_hint(self.floors[f].level).into());
        }
        // Elevators reach every open floor; everything else sits on this one.
        let floors: Vec<usize> = if kind == Kind::Elevator {
            (0..self.floors.len()).filter(|&g| self.floor_open(g)).collect()
        } else {
            vec![f]
        };
        if kind == Kind::Elevator && floors.len() < 2 {
            return Err("Elevators need a second floor to go to".into());
        }
        let mut cost = kind.price(s);
        for &g in &floors {
            let fl = &self.floors[g];
            for (tx, ty) in footprint(x, y, s) {
                if !in_bounds(tx, ty) {
                    return Err("Doesn't fit on the map here".into());
                }
                let i = idx(tx, ty);
                if fl.owner[i].is_some() {
                    return Err(if g == f { "Something is in the way".into() } else { "Another floor is blocked at this spot".into() });
                }
                if !fl.dug[i] {
                    cost += tile_cost(fl.level);
                }
            }
        }
        if (kind.is_drill() || kind == Kind::Pump) && self.floors[f].level > 0 {
            return Err(format!("{}s need ground under them: build them on the surface or below", kind.name()));
        }
        if kind == Kind::Solar && self.floors[f].level < 0 {
            return Err("Solar panels need sunlight".into());
        }
        if self.econ.money + 1e-9 < cost {
            return Err(format!("Not enough money (need ${:.0})", cost));
        }
        Ok(cost)
    }

    fn put(&mut self, f: usize, x: i32, y: i32, b: Building) {
        let a = idx(x, y);
        let fl = &mut self.floors[f];
        for (tx, ty) in footprint(x, y, b.size()) {
            let i = idx(tx, ty);
            fl.dug[i] = true;
            fl.owner[i] = Some(a as u32);
        }
        fl.tiles[a] = Some(b);
    }

    pub fn place(&mut self, f: usize, x: i32, y: i32, kind: Kind, dir: Dir) -> Result<(), String> {
        self.place_sized(f, x, y, kind, dir, kind.size())
    }

    pub fn place_sized(&mut self, f: usize, x: i32, y: i32, kind: Kind, dir: Dir, s: i32) -> Result<(), String> {
        let s = if kind == Kind::Elevator { s.clamp(MIN_LIFT, MAX_LIFT) } else { kind.size() };
        let cost = self.check_place_sized(f, x, y, kind, s)?;
        self.econ.money -= cost;
        let mut b = Building::new(kind, dir);
        if kind == Kind::Elevator {
            b.size = s as u8;
        }
        if let St::Drill { mines, .. } = &mut b.st {
            *mines = kind.ores().first().copied();
        }
        if let St::Tunnel { exit, .. } = &mut b.st {
            // An open entrance behind, facing the same way, makes this its exit.
            *exit = tunnel_behind(&self.floors[f].tiles, x, y, dir) == Some(false);
        }
        if kind == Kind::Elevator {
            // A door on every open floor: this one loads, the others unload.
            for g in 0..self.floors.len() {
                if g == f || !self.floor_open(g) {
                    continue;
                }
                let mut door = b.clone();
                if let St::Elevator { mode, .. } = &mut door.st {
                    *mode = DoorMode::Unload;
                }
                self.put(g, x, y, door);
            }
            self.lifts.push(Car {
                anchor: idx(x, y),
                size: s,
                at: f,
                target: f,
                phase: LiftPhase::Loading,
                timer: 0,
                load: VecDeque::new(),
                carry: 0.0,
            });
        }
        self.put(f, x, y, b);
        self.rebuild_grid();
        Ok(())
    }

    fn take_building(&mut self, f: usize, a: usize) -> Option<Building> {
        let fl = &mut self.floors[f];
        let b = fl.tiles[a].take()?;
        let (x, y) = pos_of(a);
        for (tx, ty) in footprint(x, y, b.size()) {
            fl.owner[idx(tx, ty)] = None;
        }
        Some(b)
    }

    /// How much waste the storage covering (x, y) holds: crates, warehouses, and an
    /// elevator (its car and its doors on every floor).
    pub fn waste_inside(&self, f: usize, x: i32, y: i32) -> usize {
        let Some(a) = self.floors[f].anchor_at(x, y) else { return 0 };
        let count = |it: &mut dyn Iterator<Item = &Item>| it.filter(|i| **i == Item::Waste).count();
        match self.floors[f].tiles[a].as_ref().map(|b| (&b.st, b.kind)) {
            Some((St::Crate { items }, _)) => count(&mut items.iter()),
            Some((St::Elevator { .. }, Kind::Elevator)) => {
                let mut n: usize = self.lifts.iter().filter(|c| c.anchor == a).map(|c| count(&mut c.load.iter())).sum();
                for fl in &self.floors {
                    if let Some(Building { st: St::Elevator { inbox, outbox, .. }, .. }) = fl.tiles[a].as_ref() {
                        n += count(&mut inbox.iter()) + count(&mut outbox.iter());
                    }
                }
                n
            }
            _ => 0,
        }
    }

    /// What it costs to remove the part covering (x, y): the waste it holds, and the fee.
    pub fn removal_fee(&self, f: usize, x: i32, y: i32) -> (usize, f64) {
        let n = self.waste_inside(f, x, y);
        (n, n as f64 * -Item::Waste.price())
    }

    /// Removes the part covering (x, y) and refunds its full price. Items inside are lost,
    /// except waste: that costs the same as selling it, and the removal is refused (Err)
    /// when it can't be paid for. Removing an elevator removes its doors on every floor.
    pub fn try_remove(&mut self, f: usize, x: i32, y: i32) -> Result<Option<Kind>, String> {
        let (n, fee) = self.removal_fee(f, x, y);
        if n > 0 {
            if self.econ.money < fee {
                return Err(format!("Holds {n} waste: removing it costs {}", money_text(fee)));
            }
            self.econ.pay_waste(n);
        }
        Ok(self.remove_part(f, x, y))
    }

    /// Like `try_remove`, but a refused removal is just None.
    #[allow(dead_code)]
    pub fn remove(&mut self, f: usize, x: i32, y: i32) -> Option<Kind> {
        self.try_remove(f, x, y).ok().flatten()
    }

    fn remove_part(&mut self, f: usize, x: i32, y: i32) -> Option<Kind> {
        let a = self.floors[f].anchor_at(x, y)?;
        let b = self.take_building(f, a)?;
        self.econ.money += b.kind.price(b.size());
        if b.kind == Kind::Elevator {
            for g in 0..self.floors.len() {
                if matches!(self.floors[g].tiles[a], Some(Building { kind: Kind::Elevator, .. })) {
                    self.take_building(g, a);
                }
            }
            self.lifts.retain(|c| c.anchor != a);
        }
        self.rebuild_grid();
        Some(b.kind)
    }

    pub fn set_dir(&mut self, f: usize, x: i32, y: i32, dir: Dir) -> bool {
        let Some(a) = self.floors[f].anchor_at(x, y) else { return false };
        match self.floors[f].tiles[a].as_mut() {
            Some(b) if b.kind.directional() => {
                b.dir = dir;
                true
            }
            _ => false,
        }
    }

    /// A tunnel entrance's exit: how many tiles ahead it is.
    pub fn tunnel_exit(&self, f: usize, x: i32, y: i32) -> Option<i32> {
        let b = self.tile(f, x, y)?;
        tunnel_partner(&self.floors[f].tiles, x, y, b.dir)
    }

    /// The setting a middle-click copies: an arm's filter, a trade post's sell mode,
    /// or a machine's recipe.
    pub fn setting_at(&self, f: usize, x: i32, y: i32) -> Option<Setting> {
        match &self.tile(f, x, y)?.st {
            St::Arm { filter, .. } | St::Splitter { filter, .. } => Some(Setting::Filter(*filter)),
            St::Trade { mode, .. } => Some(Setting::Sell(*mode)),
            St::Crafter { recipe, .. } => Some(Setting::Recipe(*recipe)),
            St::Drill { mines: Some(m), .. } => Some(Setting::Mines(*m)),
            _ => None,
        }
    }

    /// Gives a just-placed part a copied setting. Recipes that aren't researched are skipped.
    pub fn apply_setting(&mut self, f: usize, x: i32, y: i32, set: Setting) {
        let Some(a) = self.floors[f].anchor_at(x, y) else { return };
        let progress = &self.progress;
        let Some(b) = self.floors[f].tiles[a].as_mut() else { return };
        let kind = b.kind;
        match (&mut b.st, set) {
            (St::Arm { filter, .. }, Setting::Filter(v)) => *filter = v,
            (St::Splitter { filter, .. }, Setting::Filter(v)) if kind == Kind::Sorter => *filter = v,
            (St::Trade { mode, .. }, Setting::Sell(v)) => *mode = v,
            (St::Drill { mines, progress, .. }, Setting::Mines(m)) if kind.ores().contains(&m) => {
                *mines = Some(m);
                *progress = 0;
            }
            (St::Crafter { recipe, .. }, Setting::Recipe(v)) => {
                if kind.recipes().get(v as usize).is_some_and(|r| r.needs.map_or(true, |t| progress.has(t))) {
                    *recipe = v;
                }
            }
            _ => {}
        }
    }

    /// Cycles an arm's filter, a trade post's sell mode, or a machine's recipe.
    pub fn configure(&mut self, f: usize, x: i32, y: i32) -> Option<String> {
        let a = self.floors[f].anchor_at(x, y)?;
        let progress = &self.progress;
        let b = self.floors[f].tiles[a].as_mut()?;
        let kind = b.kind;
        match &mut b.st {
            St::Arm { filter, .. } => {
                *filter = match *filter {
                    None => Some(Item::ALL[0]),
                    Some(it) => Item::ALL.get(it.idx() + 1).copied(),
                };
                Some(format!("Arm filter: {}", filter.map_or("anything", |i| i.name())))
            }
            St::Splitter { filter, .. } if kind == Kind::Sorter => {
                *filter = match *filter {
                    None => Some(Item::ALL[0]),
                    Some(it) => Item::ALL.get(it.idx() + 1).copied(),
                };
                Some(match filter {
                    Some(i) => format!("Filter splitter: {} out the front, the rest out the sides", i.name()),
                    None => "Filter splitter: no filter, shares out every side".into(),
                })
            }
            St::Drill { mines, progress, .. } => {
                let ores = kind.ores();
                let at = mines.and_then(|m| ores.iter().position(|&o| o == m));
                let next = ores[at.map_or(0, |n| (n + 1) % ores.len())];
                *mines = Some(next);
                *progress = 0;
                let note = if kind == Kind::Drill { " · iron needs a basic drill" } else { "" };
                Some(format!("{} now mines {}{note}", kind.name(), next.name()))
            }
            St::Trade { mode, .. } => {
                *mode = mode.next();
                Some(format!("Trade post: {}", mode.label()))
            }
            St::Crafter { recipe, progress: p, .. } => {
                let list = kind.recipes();
                let usable = |r: &Recipe| r.needs.map_or(true, |t| progress.has(t));
                if list.iter().filter(|r| usable(r)).count() < 2 {
                    return Some(format!("{} has only one recipe so far", kind.name()));
                }
                let mut n = *recipe as usize;
                loop {
                    n = (n + 1) % list.len();
                    if usable(&list[n]) {
                        break;
                    }
                }
                *recipe = n as u8;
                *p = 0.0;
                Some(format!("{} now makes {}", kind.name(), list[n].name))
            }
            St::Elevator { mode, .. } => {
                *mode = mode.next();
                Some(format!("Elevator door on this floor: {}", mode.name()))
            }
            _ => None,
        }
    }

    /// Works out the power grids. Runs whenever something is placed or removed.
    pub fn rebuild_grid(&mut self) {
        // Every power source, with the floor it's on and the tiles it reaches.
        let mut nodes = Vec::new();
        for (f, fl) in self.floors.iter().enumerate() {
            for (i, b) in fl.tiles.iter().enumerate() {
                if let Some(b) = b {
                    let (x, y) = pos_of(i);
                    if let Some(r) = coverage(b.kind, b.size(), x, y) {
                        nodes.push((f, i, r, b.kind));
                    }
                }
            }
        }
        // Link sources whose ranges overlap, and the two ends of each elevator.
        // Solar panels never link to each other directly: poles go between them.
        let n = nodes.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut [usize], mut x: usize) -> usize {
            while p[x] != x {
                p[x] = p[p[x]];
                x = p[x];
            }
            x
        }
        for a in 0..n {
            for b in a + 1..n {
                let (fa, ia, ra, ka) = nodes[a];
                let (fb, ib, rb, kb) = nodes[b];
                let both_solar = ka == Kind::Solar && kb == Kind::Solar;
                let linked = (fa == fb && !both_solar && overlaps(ra, rb)) || (ka == Kind::Elevator && kb == Kind::Elevator && ia == ib);
                if linked {
                    let (x, y) = (find(&mut parent, a), find(&mut parent, b));
                    if x != y {
                        parent[x] = y;
                    }
                }
            }
        }
        let mut net_of_root = vec![usize::MAX; n];
        let mut node_net = vec![0; n];
        let mut nets: Vec<Net> = Vec::new();
        for a in 0..n {
            let r = find(&mut parent, a);
            if net_of_root[r] == usize::MAX {
                net_of_root[r] = nets.len();
                nets.push(Net::default());
            }
            node_net[a] = net_of_root[r];
        }
        // Paint each grid's reach, and add up supply and demand.
        let mut cover = vec![vec![-1i32; (W * H) as usize]; self.floors.len()];
        for (k, &(f, _, (x0, y0, x1, y1), kind)) in nodes.iter().enumerate() {
            let id = node_net[k];
            for y in y0.max(0)..=y1.min(H - 1) {
                for x in x0.max(0)..=x1.min(W - 1) {
                    cover[f][idx(x, y)] = id as i32;
                }
            }
            if kind == Kind::Solar && self.floors[f].level >= 0 {
                nets[id].solar += SOLAR_OUTPUT;
            }
        }
        let engines = nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.3 == Kind::SteamEngine)
            .map(|(k, n)| (n.0, n.1, node_net[k]))
            .collect();
        let mut unpowered = 0;
        for (f, fl) in self.floors.iter().enumerate() {
            for (i, b) in fl.tiles.iter().enumerate() {
                let Some(b) = b else { continue };
                let d = b.kind.power_draw();
                if d > 0.0 {
                    let (x, y) = pos_of(i);
                    match grid_of(&cover[f], x, y, b.size()) {
                        -1 => unpowered += 1,
                        id => nets[id as usize].demand += d,
                    }
                }
            }
        }
        let sources = nodes.iter().enumerate().map(|(k, n)| (n.0, n.1, node_net[k])).collect();
        self.grid = Grid { cover, nets, unpowered, engines, sources };
        self.update_power(false);
        self.rebuild_fluids();
    }

    /// Works out each grid's supply this tick. Engines give as much as their grid needs
    /// beyond solar, up to what their steam allows, and burn steam only for what they give.
    fn update_power(&mut self, burn: bool) {
        let Grid { nets, engines, .. } = &mut self.grid;
        for n in nets.iter_mut() {
            n.engine = 0.0;
        }
        let mut avail = Vec::with_capacity(engines.len());
        let ep = ENGINE_POWER;
        for &(f, a, id) in engines.iter() {
            let p = match &self.floors[f].tiles[a] {
                Some(Building { st: St::Engine { steam, .. }, .. }) => ep * (steam / STEAM_PER_TICK).min(1.0),
                _ => 0.0,
            };
            nets[id].engine += p;
            avail.push(p);
        }
        for n in nets.iter_mut() {
            n.supply = n.solar + n.engine;
            n.ratio = if n.demand <= 0.0 { 1.0 } else { (n.supply / n.demand).min(1.0) };
        }
        for (k, &(f, a, id)) in engines.iter().enumerate() {
            let n = nets[id];
            let load = if n.engine > 0.0 { ((n.demand - n.solar).max(0.0) / n.engine).min(1.0) } else { 0.0 };
            let used = avail[k] * load;
            if let Some(Building { st: St::Engine { steam, output }, .. }) = self.floors[f].tiles[a].as_mut() {
                if burn {
                    *steam = (*steam - used / ep * STEAM_PER_TICK).max(0.0);
                }
                *output = used;
            }
        }
    }

    /// Works out the pipe networks, keeping the fluid that was already in them.
    fn rebuild_fluids(&mut self) {
        let old = std::mem::take(&mut self.fluids);
        let n = (W * H) as usize;
        let mut pipe_pool = vec![vec![-1i32; n]; self.floors.len()];
        let mut pools: Vec<Pool> = Vec::new();
        for (f, fl) in self.floors.iter().enumerate() {
            let is_pipe = |i: usize| matches!(fl.tiles[i], Some(Building { kind: Kind::Pipe, .. }));
            for start in 0..n {
                if !is_pipe(start) || pipe_pool[f][start] >= 0 {
                    continue;
                }
                let id = pools.len();
                let mut pool = Pool::default();
                let mut olds: Vec<usize> = Vec::new();
                let mut stack = vec![start];
                pipe_pool[f][start] = id as i32;
                while let Some(i) = stack.pop() {
                    pool.pipes += 1;
                    if let Some(&o) = old.pipe_pool.get(f).and_then(|v| v.get(i)) {
                        if o >= 0 && !olds.contains(&(o as usize)) {
                            olds.push(o as usize);
                        }
                    }
                    let (x, y) = pos_of(i);
                    for d in Dir::ALL {
                        let (dx, dy) = d.delta();
                        if in_bounds(x + dx, y + dy) {
                            let j = idx(x + dx, y + dy);
                            if is_pipe(j) && pipe_pool[f][j] < 0 {
                                pipe_pool[f][j] = id as i32;
                                stack.push(j);
                            }
                        }
                    }
                }
                pool.cap = pool.pipes as f64 * PIPE_CAP;
                for o in olds {
                    if let Some(op) = old.pools.get(o) {
                        if let Some(fl) = op.fluid {
                            pool.add(fl, op.amount);
                        }
                    }
                }
                pools.push(pool);
            }
        }
        self.fluids = Fluids { pipe_pool, pools };
    }

    /// The grid powering tile (x, y), if any.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn net_at(&self, f: usize, x: i32, y: i32) -> Option<&Net> {
        if !in_bounds(x, y) {
            return None;
        }
        let id = self.grid.cover[f][idx(x, y)];
        (id >= 0).then(|| &self.grid.nets[id as usize])
    }

    /// The grid powering the building whose corner is at tile index `a`.
    pub fn building_net(&self, f: usize, a: usize) -> Option<&Net> {
        let b = self.floors[f].tiles[a].as_ref()?;
        let (x, y) = pos_of(a);
        let id = grid_of(&self.grid.cover[f], x, y, b.size());
        (id >= 0).then(|| &self.grid.nets[id as usize])
    }

    /// How fast the building at tile index `a` runs: 0 = no power, 1 = full power.
    pub fn building_ratio(&self, f: usize, a: usize) -> f64 {
        self.building_net(f, a).map_or(0.0, |n| n.ratio)
    }

    /// How fast an electric part at (x, y) runs: 0 = no power, 1 = full power.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn ratio_at(&self, f: usize, x: i32, y: i32) -> f64 {
        self.net_at(f, x, y).map_or(0.0, |n| n.ratio)
    }

    /// (total supply, total demand, weakest grid's ratio, grid count, unpowered parts)
    /// (supply, demand, worst grid's ratio, grids, unpowered parts, unused supply).
    /// Supply counts only grids that power something; grids with nothing on them
    /// (a lone panel) are the unused supply.
    pub fn power_summary(&self) -> (f64, f64, f64, usize, u32, f64) {
        let nets = &self.grid.nets;
        // (+ 0.0 turns an empty sum's -0 into 0)
        let supply = nets.iter().filter(|n| n.demand > 0.0).map(|n| n.supply).sum::<f64>() + 0.0;
        let unused = nets.iter().filter(|n| n.demand <= 0.0).map(|n| n.supply).sum::<f64>() + 0.0;
        let demand = nets.iter().map(|n| n.demand).sum::<f64>() + 0.0;
        let worst = nets.iter().filter(|n| n.demand > 0.0).map(|n| n.ratio).fold(1.0, f64::min);
        (supply, demand, worst, nets.len(), self.grid.unpowered, unused)
    }

    /// The grid of whatever is at (x, y): a power source's own grid, or the grid
    /// powering an electric part. None for anything else (or empty ground).
    pub fn grid_at(&self, f: usize, x: i32, y: i32) -> Option<usize> {
        let (ax, ay) = self.anchor_of(f, x, y)?;
        let a = idx(ax, ay);
        let b = self.floors[f].tiles[a].as_ref()?;
        if let Some(s) = self.grid.sources.iter().find(|s| s.0 == f && s.1 == a) {
            return Some(s.2);
        }
        let id = grid_of(&self.grid.cover[f], ax, ay, b.size());
        (b.kind.power_draw() > 0.0 && id >= 0).then_some(id as usize)
    }

    /// The grid a power source is on, and how many sources that grid has.
    pub fn source_net(&self, f: usize, a: usize) -> Option<(Net, usize)> {
        let &(_, _, id) = self.grid.sources.iter().find(|s| s.0 == f && s.1 == a)?;
        let count = self.grid.sources.iter().filter(|s| s.2 == id).count();
        Some((self.grid.nets[id], count))
    }

    pub fn tick(&mut self) {
        self.update_power(true);
        for f in 0..self.floors.len() {
            let level_of = self.floors[f].level;
            let Floor { tiles, owner, .. } = &mut self.floors[f];
            let cover = &self.grid.cover[f];
            let nets = &self.grid.nets;
            let pipe_pool = &self.fluids.pipe_pool[f];
            let pools = &mut self.fluids.pools;
            let level = level_of;
            for i in 0..tiles.len() {
                let Some(mut b) = tiles[i].take() else { continue };
                let (x, y) = pos_of(i);
                let ratio = match grid_of(cover, x, y, b.size()) {
                    id if id >= 0 => nets[id as usize].ratio,
                    _ => 0.0,
                };
                let mut ctx = Ctx { tiles: &mut *tiles, owner, econ: &mut self.econ, pipe_pool, pools: &mut *pools, level };
                tick_building(&mut b, x, y, &mut ctx, ratio);
                tiles[i] = Some(b);
            }
        }
        self.tick += 1;
        self.run_lifts();
        self.check_discoveries();
        if self.tick % TPS == 0 {
            self.sample_earnings();
        }
    }

    /// Once a second: remember lifetime earnings, and work out the recent rate.
    fn sample_earnings(&mut self) {
        self.samples.push_back((self.econ.earned_money, self.econ.earned_research));
        while self.samples.len() > 120 {
            self.samples.pop_front();
        }
        if let (Some(a), Some(b)) = (self.samples.front(), self.samples.back()) {
            let n = (self.samples.len() - 1) as f64;
            if n > 0.0 {
                self.rate = ((b.0 - a.0) / n, (b.1 - a.1) / n);
            }
        }
        let e = &self.econ;
        self.flow_samples.push_back(FlowSample { made: e.made, used: e.used, sold: e.sold, money: e.money_from, research: e.research_from });
        while self.flow_samples.len() > 61 {
            self.flow_samples.pop_front();
        }
        if let Some(a) = self.flow_samples.front() {
            let earned = Item::ALL.iter().filter(|i| **i != Item::Waste).map(|i| a.money[i.idx()]).sum();
            self.econ.minute_ago = (earned, -a.money[Item::Waste.idx()]);
        }
        self.sale_samples.push_back(self.econ.research_sales);
        while self.sale_samples.len() > 120 {
            self.sale_samples.pop_front();
        }
        if let (Some(a), Some(b)) = (self.sale_samples.front(), self.sale_samples.back()) {
            let n = (self.sale_samples.len() - 1) as f64;
            if n > 0.0 {
                self.sale_rate = (0..ITEMS).map(|i| (b[i] - a[i]) as f64 / n).collect();
            }
        }
    }

    /// Per item, over the last minute (or as much of it as has passed): made, used and
    /// sold per minute, and the money and research its sales brought per minute.
    /// Only items with something going on. The second value is how many seconds it covers.
    pub fn flow(&self) -> (Vec<Flow>, u64) {
        let (Some(a), Some(b)) = (self.flow_samples.front(), self.flow_samples.back()) else { return (Vec::new(), 0) };
        let secs = (self.flow_samples.len() - 1) as u64;
        if secs == 0 {
            return (Vec::new(), 0);
        }
        let per_min = 60.0 / secs as f64;
        let rows = Item::ALL
            .iter()
            .map(|&item| {
                let i = item.idx();
                Flow {
                    item,
                    made: (b.made[i] - a.made[i]) as f64 * per_min,
                    used: (b.used[i] - a.used[i]) as f64 * per_min,
                    sold: (b.sold[i] - a.sold[i]) as f64 * per_min,
                    money: (b.money[i] - a.money[i]) * per_min,
                    research: (b.research[i] - a.research[i]) * per_min,
                }
            })
            .filter(|f| f.made > 0.0 || f.used > 0.0 || f.sold > 0.0)
            .collect();
        (rows, secs)
    }

    /// Adds what the factory earned while the game was closed (no cap).
    /// Returns (seconds away, money, research) when there's something to report.
    pub fn apply_offline(&mut self, now: u64) -> Option<(u64, f64, f64)> {
        if self.saved_at == 0 || now <= self.saved_at {
            return None;
        }
        let away = now - self.saved_at;
        if away < 60 {
            return None;
        }
        let m = (self.rate.0 * away as f64).max(0.0);
        let r = self.offline_research(away as f64);
        if m <= 0.0 && r <= 0.0 {
            return None;
        }
        self.econ.money += m;
        self.econ.research += r;
        self.econ.earned_money += m;
        self.econ.earned_research += r;
        Some((away, m, r))
    }

    /// Research earned while away: each item keeps selling at its recent pace, and its
    /// research fades the way it does in play. Moves each item along its curve.
    fn offline_research(&mut self, secs: f64) -> f64 {
        if self.sale_rate.len() != ITEMS {
            // Saved before per-item rates were kept: count ten minutes at the last rate.
            return self.rate.1 * secs.min(600.0);
        }
        // How the recent research compares with full-price sales (trade post modes, notebooks).
        let full: f64 = Item::ALL.iter().map(|it| self.sale_rate[it.idx()] * self.econ.next_research(*it)).sum();
        if full <= 0.0 {
            return 0.0;
        }
        let share = (self.rate.1 / full).clamp(0.0, 2.0);
        let mut r = 0.0;
        for it in Item::ALL {
            let n = self.sale_rate[it.idx()] * secs;
            let from = self.econ.research_sales[it.idx()];
            r += it.curve().total(from as f64, n) * share;
            self.econ.research_sales[it.idx()] = from + n.round() as u64;
        }
        r
    }

    /// An agent message arrives. At most once every few minutes a company sends a
    /// delivery with it: about a minute of the factory's earnings (at least $25).
    pub fn receive(&mut self, mut msg: crate::mail::Message, now: u64) {
        if now >= self.last_gift + GIFT_GAP {
            let open: Vec<Company> = Company::ALL.into_iter().filter(|c| self.progress.is_open(*c)).collect();
            let c = open[self.gifts as usize % open.len()];
            let m = (self.rate.0 * 60.0).max(25.0).round();
            self.econ.money += m;
            self.econ.earned_money += m;
            self.last_gift = now;
            self.gifts += 1;
            msg.gift = Some((c, m));
        }
        self.mail.push_back(msg);
        // Only the newest few wait, so a few days away doesn't mean a pile of telegrams.
        while self.mail.len() > MAIL_KEEP {
            self.mail.pop_front();
        }
    }

    /// Making a company's key item for the first time opens that company, and making
    /// enough of a milestone item unlocks its tech for free.
    fn check_discoveries(&mut self) {
        for c in Company::ALL {
            if let Some(key) = c.key_item() {
                if !self.progress.is_open(c) && self.econ.made[key.idx()] > 0 {
                    self.progress.opened[c.idx()] = true;
                    self.progress.events.push_back(c);
                }
            }
        }
        for t in Tech::ALL {
            let (item, n) = t.milestone();
            if !self.progress.has(t) && self.progress.is_open(t.company()) && self.econ.made[item.idx()] >= n {
                self.progress.researched[t.idx()] = true;
                self.progress.notes.push_back(format!("{} unlocked for free: you've made {} {}", t.name(), n, item.name()));
            }
        }
    }

    /// Runs every elevator car: it loads at a load door until full, closes its doors,
    /// travels, opens, unloads at an unload door, then goes back to load.
    fn run_lifts(&mut self) {
        for k in 0..self.lifts.len() {
            let anchor = self.lifts[k].anchor;
            let doors = self.doors(anchor);
            if doors.len() < 2 {
                continue;
            }
            let levels: Vec<i32> = self.floors.iter().map(|fl| fl.level).collect();
            let level = |f: usize| levels[f];
            let nearest = |from: usize, want: DoorMode| {
                doors
                    .iter()
                    .filter(|d| d.2 == want && d.0 != from)
                    .min_by_key(|d| ((d.1 - level(from)).abs(), d.0))
                    .map(|d| d.0)
            };
            let mode_at = |f: usize| doors.iter().find(|d| d.0 == f).map_or(DoorMode::Pass, |d| d.2);
            let mut car = std::mem::replace(
                &mut self.lifts[k],
                Car { anchor, size: 0, at: 0, target: 0, phase: LiftPhase::Loading, timer: 0, load: VecDeque::new(), carry: 0.0 },
            );
            let cap = car.cap();
            match car.phase {
                LiftPhase::Loading => {
                    if mode_at(car.at) != DoorMode::Load {
                        if let Some(t) = nearest(car.at, DoorMode::Load) {
                            car.target = t;
                            car.phase = LiftPhase::Closing;
                            car.timer = DOOR_TICKS;
                        }
                    } else {
                        car.carry = (car.carry + LIFT_RATE).min(2.0);
                        if let Some(Building { st: St::Elevator { inbox, .. }, .. }) = self.floors[car.at].tiles[anchor].as_mut() {
                            while car.carry >= 1.0 && car.load.len() < cap {
                                let Some(it) = inbox.pop_front() else { break };
                                car.load.push_back(it);
                                car.carry -= 1.0;
                            }
                        }
                        if car.load.len() >= cap {
                            if let Some(t) = nearest(car.at, DoorMode::Unload) {
                                car.target = t;
                                car.phase = LiftPhase::Closing;
                                car.timer = DOOR_TICKS;
                            }
                        }
                    }
                }
                LiftPhase::Closing => {
                    car.timer = car.timer.saturating_sub(1);
                    if car.timer == 0 {
                        car.phase = LiftPhase::Moving;
                        car.timer = FLOOR_TICKS * (level(car.at) - level(car.target)).unsigned_abs();
                    }
                }
                LiftPhase::Moving => {
                    car.timer = car.timer.saturating_sub(1);
                    if car.timer == 0 {
                        car.at = car.target;
                        car.phase = LiftPhase::Opening;
                        car.timer = DOOR_TICKS;
                    }
                }
                LiftPhase::Opening => {
                    car.timer = car.timer.saturating_sub(1);
                    if car.timer == 0 {
                        car.phase = if mode_at(car.at) == DoorMode::Unload && !car.load.is_empty() {
                            LiftPhase::Unloading
                        } else {
                            LiftPhase::Loading
                        };
                    }
                }
                LiftPhase::Unloading => {
                    car.carry = (car.carry + LIFT_RATE).min(2.0);
                    if let Some(Building { st: St::Elevator { outbox, .. }, .. }) = self.floors[car.at].tiles[anchor].as_mut() {
                        while car.carry >= 1.0 && outbox.len() < cap {
                            let Some(it) = car.load.pop_front() else { break };
                            outbox.push_back(it);
                            car.carry -= 1.0;
                        }
                    }
                    if car.load.is_empty() {
                        match nearest(car.at, DoorMode::Load) {
                            Some(t) => {
                                car.target = t;
                                car.phase = LiftPhase::Closing;
                                car.timer = DOOR_TICKS;
                            }
                            None => car.phase = LiftPhase::Loading,
                        }
                    }
                }
            }
            self.lifts[k] = car;
        }
    }

    /// A full building says PWR when everything in front of it is a belt with no power,
    /// since that's the real reason it can't get rid of its output.
    fn full_word(&self, f: usize, a: usize) -> &'static str {
        let Some(b) = self.floors[f].tiles[a].as_ref() else { return "FULL" };
        let (ax, ay) = ((a % W as usize) as i32, (a / W as usize) as i32);
        let s = b.size() as i32;
        let mut belts = 0;
        for j in 0..s {
            let (tx, ty) = front_tile(ax, ay, s, b.dir, j);
            if !in_bounds(tx, ty) {
                continue;
            }
            let Some(o) = self.floors[f].owner[idx(tx, ty)] else { continue };
            match self.floors[f].tiles[o as usize].as_ref() {
                Some(nb) if nb.kind.is_belt() => {
                    if self.building_ratio(f, o as usize) > 0.0 {
                        return "FULL";
                    }
                    belts += 1;
                }
                Some(_) => return "FULL",
                None => {}
            }
        }
        if belts > 0 { "PWR" } else { "FULL" }
    }

    /// How many buildings are idle across every floor, and the most common reason.
    pub fn idle_summary(&self) -> (usize, Option<&'static str>) {
        let mut counts: Vec<(&'static str, usize)> = Vec::new();
        for f in 0..self.floors.len() {
            for a in 0..self.floors[f].tiles.len() {
                if self.floors[f].tiles[a].is_none() {
                    continue;
                }
                if let Some(r) = self.idle_reason(f, a) {
                    match counts.iter_mut().find(|c| c.0 == r) {
                        Some(c) => c.1 += 1,
                        None => counts.push((r, 1)),
                    }
                }
            }
        }
        let total = counts.iter().map(|c| c.1).sum();
        (total, counts.iter().max_by_key(|c| c.1).map(|c| c.0))
    }

    /// Why an idle building has stopped, in a word, or None if it's working (or never idles).
    pub fn idle_reason(&self, f: usize, a: usize) -> Option<&'static str> {
        let b = self.floors[f].tiles[a].as_ref()?;
        let powered = self.building_ratio(f, a) > 0.0;
        match &b.st {
            St::Drill { mines, fuel, coal, out, .. } => {
                if mines.is_none() {
                    Some("ORE?")
                } else if out.len() >= DRILL_OUT_CAP {
                    Some(self.full_word(f, a))
                } else if *fuel == 0 && *coal == 0 && *mines != Some(Item::Coal) {
                    Some("COAL")
                } else {
                    None
                }
            }
            St::Crafter { busy: false, stock, fuel, out, .. } => {
                let r = b.recipe()?;
                let made: usize = r.outputs.iter().map(|o| o.1 as usize).sum();
                if out.len() + made > CRAFTER_OUT_CAP {
                    Some(self.full_word(f, a))
                } else if !r.inputs.iter().all(|(it, n)| stock[it.idx()] >= *n) {
                    Some("NEED")
                } else if let Some(fl) = r.fuel.filter(|fl| *fuel == 0 && stock[fl.idx()] == 0) {
                    Some(if fl == Item::Coke { "COKE" } else { "COAL" })
                } else if b.kind.power_draw() > 0.0 && !powered {
                    Some("PWR")
                } else {
                    None
                }
            }
            St::Boiler { busy: false, fuel, coal, .. } => Some(if *fuel == 0 && *coal == 0 { "COAL" } else { "WATR" }),
            St::Engine { steam, .. } if *steam <= 0.0 => Some("STM?"),
            St::Pump { busy: false } => Some(if powered { "FULL" } else { "PWR" }),
            St::Trade { stuck, .. } if *stuck > 0 => Some("WSTE"),
            _ => None,
        }
    }

    /// Offers one item to the building at (x, y), as a belt would. For tests.
    #[cfg(test)]
    pub fn offer(&mut self, f: usize, x: i32, y: i32, item: Item) -> bool {
        let fl = &mut self.floors[f];
        try_insert(&mut fl.tiles, &fl.owner, &mut self.econ, x, y, item, Dir::E)
    }

    /// A small working factory near spawn, for `--demo` and tests.
    pub fn build_demo(&mut self) -> Result<(), String> {
        use Kind::*;
        let f = 0;
        // Copper drill feeds the furnace directly; plates go east to the trade post.
        self.place(f, 51, 37, Drill, Dir::E)?;
        self.place(f, 53, 37, Furnace, Dir::E)?;
        for x in 55..=59 {
            self.place(f, x, 37, Belt, Dir::E)?;
        }
        self.place(f, 60, 36, TradePost, Dir::E)?;
        self.configure(f, 60, 36); // split 40/40
        // Coal drill sends coal west; a splitter shares it between the furnace and the copper drill.
        self.place(f, 65, 41, Drill, Dir::W)?;
        self.apply_setting(f, 65, 41, Setting::Mines(Item::Coal));
        for x in 55..=64 {
            self.place(f, x, 41, Belt, Dir::W)?;
        }
        self.place(f, 54, 41, Splitter, Dir::E)?;
        self.place(f, 54, 40, Belt, Dir::N)?;
        self.place(f, 54, 39, Belt, Dir::N)?;
        self.place(f, 53, 41, Belt, Dir::W)?;
        for y in 39..=41 {
            self.place(f, 52, y, Belt, Dir::N)?;
        }
        // Two solar panels, and poles carrying their power to every belt.
        self.place(f, 56, 33, Solar, Dir::E)?;
        self.place(f, 58, 33, Solar, Dir::E)?;
        self.place(f, 57, 38, Pole, Dir::E)?;
        self.place(f, 63, 39, Pole, Dir::E)?;
        self.place(f, 51, 40, Pole, Dir::E)?;
        Ok(())
    }
}

/// What a building can reach while it ticks.
struct Ctx<'a> {
    tiles: &'a mut [Option<Building>],
    owner: &'a [Option<u32>],
    econ: &'a mut Econ,
    pipe_pool: &'a [i32],
    pools: &'a mut [Pool],
    /// The floor's level (deeper ore mines faster).
    level: i32,
}

/// The pipe networks touching a building's edges, optionally skipping one side.
fn pools_around(pipe_pool: &[i32], ax: i32, ay: i32, s: i32, skip: Option<Dir>) -> Vec<usize> {
    let mut out = Vec::new();
    for d in Dir::ALL {
        if Some(d) == skip {
            continue;
        }
        for j in 0..s {
            let (tx, ty) = front_tile(ax, ay, s, d, j);
            if in_bounds(tx, ty) {
                let p = pipe_pool[idx(tx, ty)];
                if p >= 0 && !out.contains(&(p as usize)) {
                    out.push(p as usize);
                }
            }
        }
    }
    out
}

fn tick_building(b: &mut Building, x: i32, y: i32, ctx: &mut Ctx, ratio: f64) {
    let Ctx { tiles, owner, econ, pipe_pool, pools, level } = ctx;
    let level = *level;
    let (tiles, owner, econ): (&mut [Option<Building>], &[Option<u32>], &mut Econ) = (tiles, owner, econ);
    let dir = b.dir;
    let kind = b.kind;
    let size = b.size();
    let rr = &mut b.rr;
    match &mut b.st {
        St::Belt { items } => {
            let v = if kind == Kind::FastBelt { FAST_BELT_SPEED } else { BELT_SPEED } * ratio;
            if v <= 0.0 {
                return;
            }
            if items.first().is_some_and(|it| it.pos >= 1.0) {
                let (dx, dy) = dir.delta();
                if try_insert(tiles, owner, econ, x + dx, y + dy, items[0].item, dir) {
                    items.remove(0);
                }
            }
            let mut limit = 1.0;
            for it in items.iter_mut() {
                let np = (it.pos + v).min(limit);
                if np > it.pos {
                    it.pos = np;
                }
                limit = it.pos - BELT_GAP;
            }
        }
        St::Drill { mines, fuel, coal, progress, out } => {
            let Some(item) = *mines else { return };
            let (base, per_coal) = kind.drill_speed();
            let ticks = ((base as f64) / richness(level)).round().max(1.0) as u32;
            let per_fuel = ticks * per_coal;
            if *fuel == 0 && *coal > 0 {
                *coal -= 1;
                econ.used[Item::Coal.idx()] += 1;
                *fuel = per_fuel;
            }
            // A drill on coal with nothing to burn fuels itself from what it mines.
            let self_fuel = *fuel == 0 && item == Item::Coal;
            if (*fuel > 0 || self_fuel) && out.len() < DRILL_OUT_CAP {
                if *fuel > 0 {
                    *fuel -= 1;
                }
                *progress += 1;
                if *progress >= ticks {
                    *progress = 0;
                    if self_fuel {
                        *fuel = per_fuel;
                    } else {
                        out.push_back(item);
                        econ.made[item.idx()] += 1;
                    }
                }
            }
            push_front(tiles, owner, econ, x, y, size, dir, out, rr);
        }
        St::Crafter { recipe, stock, fuel, progress, busy, out } => {
            let r = &kind.recipes()[*recipe as usize];
            *busy = false;
            let have = r.inputs.iter().all(|(it, n)| stock[it.idx()] >= *n);
            let made: usize = r.outputs.iter().map(|o| o.1 as usize).sum();
            let room = out.len() + made <= CRAFTER_OUT_CAP;
            if have && room {
                let speed = match r.fuel {
                    Some(f) => {
                        if *fuel == 0 && stock[f.idx()] > 0 {
                            stock[f.idx()] -= 1;
                            econ.used[f.idx()] += 1;
                            *fuel = f.burn_ticks().unwrap_or(0);
                        }
                        if *fuel > 0 {
                            *fuel -= 1;
                            1.0
                        } else {
                            0.0
                        }
                    }
                    None if kind.power_draw() > 0.0 => ratio,
                    None => 1.0,
                };
                if speed > 0.0 {
                    *busy = true;
                    *progress += speed;
                    if *progress >= r.ticks as f64 {
                        *progress -= r.ticks as f64;
                        for (it, n) in r.inputs {
                            stock[it.idx()] -= n;
                            econ.used[it.idx()] += *n as u64;
                        }
                        for (it, n) in r.outputs {
                            for _ in 0..*n {
                                out.push_back(*it);
                                econ.made[it.idx()] += 1;
                            }
                        }
                    }
                }
            }
            push_front(tiles, owner, econ, x, y, size, dir, out, rr);
        }
        St::Pump { busy } => {
            *busy = false;
            if ratio > 0.0 {
                for p in pools_around(pipe_pool, x, y, size, None) {
                    if pools[p].add(Fluid::Water, PUMP_RATE * ratio / TPS as f64) > 0.0 {
                        *busy = true;
                        break;
                    }
                }
            }
        }
        St::Boiler { fuel, coal, busy } => {
            *busy = false;
            let step = BOILER_RATE / TPS as f64;
            if *fuel == 0 && *coal > 0 {
                *coal -= 1;
                econ.used[Item::Coal.idx()] += 1;
                *fuel = Item::Coal.burn_ticks().unwrap_or(0);
            }
            // Water comes from pipes on any side but the front.
            let source = pools_around(pipe_pool, x, y, size, Some(dir)).into_iter().find(|&p| pools[p].fluid == Some(Fluid::Water) && pools[p].amount >= step);
            if *fuel > 0 {
                if let Some(src) = source {
                    // Steam goes out the front: straight into an engine, or into a pipe network.
                    let mut delivered = false;
                    for j in 0..size {
                        let (tx, ty) = front_tile(x, y, size, dir, j);
                        if !in_bounds(tx, ty) {
                            continue;
                        }
                        if let Some(a) = owner[idx(tx, ty)] {
                            if let Some(Building { st: St::Engine { steam, .. }, .. }) = tiles[a as usize].as_mut() {
                                if *steam + step <= ENGINE_BUF {
                                    *steam += step;
                                    delivered = true;
                                    break;
                                }
                            }
                        }
                        let p = pipe_pool[idx(tx, ty)];
                        if p >= 0 && pools[p as usize].add(Fluid::Steam, step) > 0.0 {
                            delivered = true;
                            break;
                        }
                    }
                    if delivered {
                        pools[src].take(Fluid::Water, step);
                        *fuel -= 1;
                        *busy = true;
                    }
                }
            }
        }
        St::Engine { steam, .. } => {
            // Tops up from steam pipes touching it; boilers can also feed it directly.
            for p in pools_around(pipe_pool, x, y, size, None) {
                if *steam < ENGINE_BUF {
                    *steam += pools[p].take(Fluid::Steam, ENGINE_BUF - *steam);
                }
            }
        }
        St::Pipe => {}
        St::Splitter { buf, cooldown, tried, tried_prev, filter, .. } => {
            let only = (kind == Kind::Sorter).then_some((dir, *filter));
            *tried_prev = *tried;
            *tried = 0;
            if ratio <= 0.0 {
                return;
            }
            if *cooldown > 0.0 {
                *cooldown -= ratio;
            }
            if *cooldown <= 0.0 && split_out(tiles, x, y, buf, rr, only) {
                *cooldown += SPLITTER_TICKS;
            }
        }
        St::Arm { filter, hold, progress } => {
            if ratio <= 0.0 {
                return;
            }
            if *progress < 1.0 {
                *progress = (*progress + ratio / ARM_TICKS).min(1.0);
            }
            if *progress < 1.0 {
                return;
            }
            let reach = if kind == Kind::LongArm { 2 } else { 1 };
            let (dx, dy) = dir.delta();
            let (dx, dy) = (dx * reach, dy * reach);
            match *hold {
                None => {
                    let src = if in_bounds(x - dx, y - dy) { owner[idx(x - dx, y - dy)] } else { None };
                    if let Some(src) = src.and_then(|a| tiles[a as usize].as_mut()) {
                        if let Some(it) = take_from(src, *filter) {
                            *hold = Some(it);
                            *progress = 0.0;
                        }
                    }
                }
                Some(it) => {
                    if try_insert(tiles, owner, econ, x + dx, y + dy, it, dir) {
                        *hold = None;
                        *progress = 0.0;
                    }
                }
            }
        }
        St::Crate { items } => {
            push_front(tiles, owner, econ, x, y, size, dir, items, rr);
        }
        St::Tunnel { exit: false, items } => {
            // Items travel under at belt speed, keeping their spacing, and come up out of
            // the exit's front. With no exit ahead they wait.
            let Some(k) = tunnel_partner(tiles, x, y, dir) else { return };
            let mut limit = f64::MIN;
            for it in items.iter_mut() {
                it.1 = (it.1 - BELT_SPEED).max(limit + BELT_GAP).max(0.0);
                limit = it.1;
            }
            if items.front().is_some_and(|it| it.1 <= 0.0) {
                let (dx, dy) = dir.delta();
                let item = items[0].0;
                if try_insert(tiles, owner, econ, x + dx * (k + 1), y + dy * (k + 1), item, dir) {
                    items.pop_front();
                }
            }
        }
        St::Tunnel { exit: true, .. } => {}
        St::Elevator { outbox, .. } => {
            push_front(tiles, owner, econ, x, y, size, dir, outbox, rr);
        }
        St::Trade { stuck, .. } => *stuck = stuck.saturating_sub(1),
        St::Solar | St::Pole => {}
    }
}

fn belt_accept(items: &mut Vec<BeltItem>, item: Item) -> bool {
    if items.last().map_or(true, |l| l.pos >= BELT_GAP) {
        items.push(BeltItem { item, pos: 0.0 });
        true
    } else {
        false
    }
}

/// Buildings send output out of their front edge: onto a belt, or straight into
/// the next building. Takes turns between the spots along the edge.
#[allow(clippy::too_many_arguments)]
fn push_front(
    tiles: &mut [Option<Building>],
    owner: &[Option<u32>],
    econ: &mut Econ,
    ax: i32,
    ay: i32,
    s: i32,
    dir: Dir,
    q: &mut VecDeque<Item>,
    rr: &mut u8,
) -> bool {
    let Some(&item) = q.front() else { return false };
    for k in 0..s {
        let j = (*rr as i32 + k) % s;
        let (tx, ty) = front_tile(ax, ay, s, dir, j);
        if try_insert(tiles, owner, econ, tx, ty, item, dir) {
            q.pop_front();
            *rr = ((j + 1) % s) as u8;
            return true;
        }
    }
    false
}

/// Splitters send items, in turn, onto every neighbouring belt that leads away from them.
/// `only`: a filter splitter's front and filter: the filtered item goes out the front only,
/// everything else out the other sides only.
fn split_out(tiles: &mut [Option<Building>], x: i32, y: i32, q: &mut VecDeque<Item>, rr: &mut u8, only: Option<(Dir, Option<Item>)>) -> bool {
    let Some(&item) = q.front() else { return false };
    for k in 0..4 {
        let d = Dir::ALL[(*rr as usize + k) % 4];
        let allowed = match only {
            Some((front, Some(f))) => (item == f) == (d == front),
            _ => true,
        };
        if !allowed {
            continue;
        }
        let (dx, dy) = d.delta();
        let (nx, ny) = (x + dx, y + dy);
        if !in_bounds(nx, ny) {
            continue;
        }
        if let Some(Building { dir: bd, st: St::Belt { items }, .. }) = tiles[idx(nx, ny)].as_mut() {
            if *bd == d && belt_accept(items, item) {
                q.pop_front();
                *rr = ((*rr as usize + k + 1) % 4) as u8;
                return true;
            }
        }
    }
    false
}

/// Something moving in direction `travel` arrives at tile (tx, ty).
fn try_insert(
    tiles: &mut [Option<Building>],
    owner: &[Option<u32>],
    econ: &mut Econ,
    tx: i32,
    ty: i32,
    item: Item,
    travel: Dir,
) -> bool {
    if !in_bounds(tx, ty) {
        return false;
    }
    let Some(a) = owner[idx(tx, ty)] else { return false };
    // A tunnel entrance only takes items when its exit is in place.
    let tunnel = match tiles[a as usize].as_ref() {
        Some(Building { kind: Kind::Tunnel, dir, .. }) => tunnel_partner(tiles, tx, ty, *dir),
        _ => None,
    };
    let Some(b) = tiles[a as usize].as_mut() else { return false };
    let (bdir, bkind, bsize) = (b.dir, b.kind, b.size());
    match &mut b.st {
        St::Belt { items } => bdir != travel.opposite() && belt_accept(items, item),
        St::Drill { coal, .. } => {
            if item == Item::Coal && *coal < DRILL_COAL_CAP {
                *coal += 1;
                true
            } else {
                false
            }
        }
        St::Crafter { recipe, stock, .. } => {
            // Takes what its current recipe uses, plus coal if it burns fuel.
            let r = &bkind.recipes()[*recipe as usize];
            let cap = match r.inputs.iter().find(|(it, _)| *it == item) {
                Some((_, n)) => (n * 4).max(10),
                None if r.fuel == Some(item) => 10,
                None => return false,
            };
            if stock[item.idx()] < cap {
                stock[item.idx()] += 1;
                true
            } else {
                false
            }
        }
        St::Splitter { buf, last_in, tried, tried_prev, .. } => {
            // Take turns between inputs so merging is fair.
            let bit = 1u8 << travel.idx();
            *tried |= bit;
            if buf.len() >= 2 {
                return false;
            }
            if *last_in == Some(travel) && (*tried_prev & !bit) != 0 {
                return false;
            }
            buf.push_back(item);
            *last_in = Some(travel);
            true
        }
        St::Tunnel { exit: false, items } => {
            let Some(k) = tunnel else { return false };
            let room = items.back().map_or(true, |it| it.1 <= k as f64 - BELT_GAP);
            if travel != bdir.opposite() && room {
                items.push_back((item, k as f64));
                true
            } else {
                false
            }
        }
        St::Tunnel { exit: true, .. } => false,
        St::Crate { items } => {
            let cap = CRATE_CAP * if bkind == Kind::Warehouse { WAREHOUSE_CRATES } else { 1 };
            if items.len() < cap {
                items.push_back(item);
                true
            } else {
                false
            }
        }
        St::Trade { mode, money, research, count, stuck } => {
            if item == Item::Waste && econ.waste_refusal(*mode).is_some() {
                *stuck = 30;
                return false;
            }
            let (m, r) = econ.sell(item, *mode);
            *money += m;
            *research += r;
            *count += 1;
            true
        }
        St::Elevator { inbox, mode, .. } => {
            if *mode == DoorMode::Load && inbox.len() < LIFT_PER_TILE * (bsize * bsize) as usize {
                inbox.push_back(item);
                true
            } else {
                false
            }
        }
        St::Boiler { coal, .. } => {
            if item == Item::Coal && *coal < BOILER_COAL_CAP {
                *coal += 1;
                true
            } else {
                false
            }
        }
        St::Arm { .. } | St::Solar | St::Pole | St::Pipe | St::Pump { .. } | St::Engine { .. } => false,
    }
}

/// How far ahead a tunnel entrance's exit is: the first tunnel ahead facing the same way,
/// if it's an exit, within reach.
fn tunnel_partner(tiles: &[Option<Building>], x: i32, y: i32, dir: Dir) -> Option<i32> {
    let (dx, dy) = dir.delta();
    for k in 1..=TUNNEL_REACH + 1 {
        let (tx, ty) = (x + dx * k, y + dy * k);
        if !in_bounds(tx, ty) {
            return None;
        }
        if let Some(Building { kind: Kind::Tunnel, dir: d, st: St::Tunnel { exit, .. }, .. }) = &tiles[idx(tx, ty)] {
            if *d == dir {
                return exit.then_some(k);
            }
        }
    }
    None
}

/// The nearest tunnel behind (x, y) facing `dir`, within reach: Some(is it an exit).
fn tunnel_behind(tiles: &[Option<Building>], x: i32, y: i32, dir: Dir) -> Option<bool> {
    let (dx, dy) = dir.delta();
    for k in 1..=TUNNEL_REACH + 1 {
        let (tx, ty) = (x - dx * k, y - dy * k);
        if !in_bounds(tx, ty) {
            return None;
        }
        if let Some(Building { kind: Kind::Tunnel, dir: d, st: St::Tunnel { exit, .. }, .. }) = &tiles[idx(tx, ty)] {
            if *d == dir {
                return Some(*exit);
            }
        }
    }
    None
}

/// What an arm can pick up from the tile behind it.
fn take_from(src: &mut Building, filter: Option<Item>) -> Option<Item> {
    let ok = |it: Item| filter.map_or(true, |f| f == it);
    match &mut src.st {
        St::Belt { items } => {
            let p = items.iter().position(|bi| ok(bi.item))?;
            Some(items.remove(p).item)
        }
        St::Drill { out, .. }
        | St::Crafter { out, .. }
        | St::Splitter { buf: out, .. }
        | St::Crate { items: out }
        | St::Elevator { outbox: out, .. } => {
            let p = out.iter().position(|i| ok(*i))?;
            out.remove(p)
        }
        _ => None,
    }
}

// ---------- the map ----------

/// Every floor, in `LEVELS` order. The surface is open ground: no ore patches or lakes,
/// drills and pumps work anywhere. Underground floors are solid rock to dig out (drills
/// there mine faster: see `richness`); floors above the ground start as open air.
fn gen_floors() -> Vec<Floor> {
    LEVELS.iter().map(|&level| Floor::new(level, level == 0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(w: &mut World, secs: u64) {
        for _ in 0..secs * TPS {
            w.tick();
        }
    }

    fn crate_items(w: &World, f: usize, x: i32, y: i32) -> Vec<Item> {
        match &w.tile(f, x, y).unwrap().st {
            St::Crate { items } => items.iter().copied().collect(),
            _ => panic!("not a crate"),
        }
    }

    fn fill_crate(w: &mut World, f: usize, x: i32, y: i32, items: &[Item]) {
        if let Some(Building { st: St::Crate { items: c }, .. }) = w.floors[f].tiles[idx(x, y)].as_mut() {
            c.extend(items.iter().copied());
        }
    }

    #[test]
    fn drill_belt_trade_post() {
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Drill, Dir::E).unwrap(); // covers 5-6, 5-6; front is x=7
        for x in 7..=9 {
            w.place(0, x, 5, Kind::Belt, Dir::E).unwrap();
        }
        w.place(0, 10, 4, Kind::TradePost, Dir::E).unwrap(); // covers 10-12, 4-6
        w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap(); // reaches 5-10, 5-10
        let before = w.econ.money;
        run(&mut w, 30);
        // The starter fuel mines exactly 8 ores, then the drill waits for coal.
        assert_eq!(w.econ.sold[Item::CopperOre.idx()], 8);
        assert!((w.econ.money - before - 8.0).abs() < 1e-9);
    }

    #[test]
    fn drill_feeds_furnace_directly() {
        let mut w = World::blank();
        // Coal drill -> belt down into the furnace's top.
        w.place(0, 5, 10, Kind::Drill, Dir::E).unwrap();
        w.configure(0, 5, 10); // copper -> coal
        w.place(0, 7, 10, Kind::Belt, Dir::E).unwrap();
        for y in 10..=13 {
            w.place(0, 8, y, Kind::Belt, Dir::S).unwrap();
        }
        // Copper drill pushes straight into the furnace next to it, no belt.
        w.place(0, 5, 14, Kind::Drill, Dir::E).unwrap();
        w.place(0, 7, 14, Kind::Furnace, Dir::E).unwrap(); // covers 7-8, 14-15
        w.place(0, 9, 14, Kind::Belt, Dir::E).unwrap();
        w.place(0, 10, 13, Kind::TradePost, Dir::E).unwrap();
        w.place(0, 10, 10, Kind::Solar, Dir::E).unwrap();
        w.place(0, 9, 12, Kind::Pole, Dir::E).unwrap();
        run(&mut w, 120);
        // Copper drill only had its starter fuel: 8 ores -> 8 plates.
        assert_eq!(w.econ.sold[Item::CopperPlate.idx()], 8);
    }

    #[test]
    fn tunnels_carry_items_under_other_things() {
        use Item::*;
        let mut w = World::blank();
        // Crate -> entrance, a belt line crossing above, exit 4 tiles on -> crate.
        w.place(0, 5, 10, Kind::Crate, Dir::E).unwrap();
        fill_crate(&mut w, 0, 5, 10, &[Coal; 10]);
        w.place(0, 6, 10, Kind::Tunnel, Dir::E).unwrap();
        for x in 7..=9 {
            w.place(0, x, 10, Kind::Belt, Dir::N).unwrap();
        }
        assert!(w.tunnel_exit(0, 6, 10).is_none(), "no exit yet");
        w.place(0, 10, 10, Kind::Tunnel, Dir::E).unwrap();
        assert_eq!(w.tunnel_exit(0, 6, 10), Some(4));
        assert!(matches!(w.tile(0, 10, 10).unwrap().st, St::Tunnel { exit: true, .. }));
        w.place(0, 11, 10, Kind::Crate, Dir::E).unwrap();
        run(&mut w, 10);
        assert_eq!(crate_items(&w, 0, 11, 10), vec![Coal; 10]);
        // A tunnel right after an exit starts a new pair; one too far away is just an entrance.
        w.place(0, 20, 20, Kind::Tunnel, Dir::E).unwrap();
        w.place(0, 26, 20, Kind::Tunnel, Dir::E).unwrap();
        assert!(matches!(w.tile(0, 26, 20).unwrap().st, St::Tunnel { exit: false, .. }));
        assert!(w.tunnel_exit(0, 20, 20).is_none());
    }

    #[test]
    fn long_arms_reach_over_a_tile() {
        use Item::*;
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Crate, Dir::N).unwrap();
        fill_crate(&mut w, 0, 5, 5, &[IronOre; 3]);
        w.place(0, 6, 5, Kind::Pipe, Dir::E).unwrap(); // in the way, untouched
        w.place(0, 7, 5, Kind::LongArm, Dir::E).unwrap();
        w.place(0, 9, 5, Kind::Crate, Dir::N).unwrap();
        w.place(0, 7, 7, Kind::Pole, Dir::E).unwrap();
        w.place(0, 5, 8, Kind::Solar, Dir::E).unwrap();
        w.place(0, 9, 8, Kind::Solar, Dir::E).unwrap();
        run(&mut w, 10);
        assert_eq!(crate_items(&w, 0, 9, 5), vec![IronOre; 3]);
    }

    #[test]
    fn filter_splitters_send_one_item_ahead() {
        use Item::*;
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
        fill_crate(&mut w, 0, 5, 5, &[Coal, CopperOre, Coal, CopperOre, CopperOre, Coal]);
        w.place(0, 6, 5, Kind::Sorter, Dir::E).unwrap();
        while !w.configure(0, 6, 5).unwrap().contains("coal out") {}
        w.place(0, 7, 5, Kind::Belt, Dir::E).unwrap();
        w.place(0, 8, 5, Kind::Crate, Dir::E).unwrap();
        w.place(0, 6, 4, Kind::Belt, Dir::N).unwrap();
        w.place(0, 6, 3, Kind::Crate, Dir::N).unwrap();
        w.place(0, 6, 6, Kind::Belt, Dir::S).unwrap();
        w.place(0, 6, 7, Kind::Crate, Dir::S).unwrap();
        w.place(0, 9, 6, Kind::Pole, Dir::E).unwrap();
        w.place(0, 10, 7, Kind::Solar, Dir::E).unwrap();
        run(&mut w, 10);
        assert_eq!(crate_items(&w, 0, 8, 5), vec![Coal; 3]);
        let sides = [crate_items(&w, 0, 6, 3), crate_items(&w, 0, 6, 7)].concat();
        assert_eq!(sides, vec![CopperOre; 3]);
    }

    #[test]
    fn warehouses_hold_four_crates() {
        let mut w = World::blank();
        w.tick();
        w.place(0, 5, 5, Kind::Warehouse, Dir::E).unwrap();
        let Floor { tiles, owner, .. } = &mut w.floors[0];
        let mut n = 0;
        while try_insert(tiles, owner, &mut w.econ, 6, 6, Item::Coal, Dir::E) {
            n += 1;
        }
        assert_eq!(n, CRATE_CAP * WAREHOUSE_CRATES);
    }

    #[test]
    fn production_panel_counts_the_last_minute() {
        let mut w = World::new();
        w.build_demo().unwrap();
        run(&mut w, 70);
        let (rows, secs) = w.flow();
        assert_eq!(secs, 60);
        let get = |it: Item| rows.iter().find(|r| r.item == it).copied().unwrap();
        let plate = get(Item::CopperPlate);
        assert!(plate.made > 10.0 && plate.sold > 10.0 && plate.research > 0.0 && plate.money > 0.0, "{plate:?}");
        let ore = get(Item::CopperOre);
        assert!(ore.made > 10.0 && (ore.used - plate.made).abs() <= 2.0, "furnaces use one ore per plate: {ore:?} {plate:?}");
        assert!(get(Item::Coal).used > 0.0, "fuel counts as used");
    }

    #[test]
    fn demo_factory_runs() {
        let mut w = World::new();
        w.build_demo().unwrap();
        run(&mut w, 90);
        assert!(w.econ.sold[Item::CopperPlate.idx()] > 20, "sold {:?}", w.econ.sold);
        assert!(w.econ.research > 0.0);
    }

    #[test]
    fn arm_filters() {
        let mut w = World::blank();
        w.place(0, 5, 20, Kind::Crate, Dir::E).unwrap();
        fill_crate(&mut w, 0, 5, 20, &[Item::CopperOre, Item::Coal, Item::CopperOre, Item::Coal]);
        w.place(0, 6, 20, Kind::Arm, Dir::E).unwrap();
        w.configure(0, 6, 20); // copper ore
        w.configure(0, 6, 20); // coal
        w.place(0, 7, 20, Kind::Crate, Dir::E).unwrap();
        w.place(0, 6, 22, Kind::Solar, Dir::E).unwrap();
        run(&mut w, 10);
        assert_eq!(crate_items(&w, 0, 7, 20), vec![Item::Coal, Item::Coal]);
    }

    #[test]
    fn elevator_moves_items_between_floors() {
        let mut w = World::blank();
        w.place(0, 8, 10, Kind::Crate, Dir::E).unwrap();
        fill_crate(&mut w, 0, 8, 10, &[Item::CopperOre; 40]);
        w.place(0, 9, 10, Kind::Belt, Dir::E).unwrap();
        w.place(0, 10, 10, Kind::Elevator, Dir::E).unwrap(); // 2x2: holds 40; doors on ground, -1 and +1
        w.place(1, 12, 10, Kind::Belt, Dir::E).unwrap();
        w.place(1, 13, 10, Kind::Crate, Dir::E).unwrap();
        // Power reaches the underground belt down the elevator shaft.
        w.place(0, 8, 12, Kind::Solar, Dir::E).unwrap();
        run(&mut w, 4);
        assert_eq!(w.lifts[0].phase, LiftPhase::Loading, "it waits until it's full");
        run(&mut w, 16);
        assert_eq!(crate_items(&w, 1, 13, 10).len(), 40);
    }

    #[test]
    fn elevator_timing_follows_the_rules() {
        let mut w = World::blank();
        w.place_sized(0, 10, 10, Kind::Elevator, Dir::E, 3).unwrap();
        assert_eq!(w.lifts[0].cap(), 90);
        // Fill the car directly and watch it go: 0.5 s doors, 1 s per floor, 0.5 s doors.
        w.lifts[0].load.extend([Item::Coal; 90]);
        w.tick(); // full: starts closing
        assert_eq!(w.lifts[0].phase, LiftPhase::Closing);
        for _ in 0..DOOR_TICKS {
            w.tick();
        }
        assert_eq!(w.lifts[0].phase, LiftPhase::Moving);
        for _ in 0..FLOOR_TICKS {
            w.tick();
        }
        assert_eq!(w.lifts[0].phase, LiftPhase::Opening);
        for _ in 0..DOOR_TICKS {
            w.tick();
        }
        assert_eq!(w.lifts[0].phase, LiftPhase::Unloading);
        // Unloading is quick but not instant.
        assert!(!w.lifts[0].load.is_empty());
    }

    #[test]
    fn elevators_come_in_sizes_and_cost_by_size() {
        let mut w = World::blank();
        let before = w.econ.money;
        w.place_sized(0, 20, 20, Kind::Elevator, Dir::E, 8).unwrap();
        assert_eq!(w.anchor_of(0, 27, 27), Some((20, 20)));
        assert_eq!(w.anchor_of(1, 27, 27), Some((20, 20)), "the door is on the floor below too");
        assert_eq!(before - w.econ.money, 75.0 * 64.0);
        assert_eq!(w.remove(1, 21, 21), Some(Kind::Elevator));
        assert!(w.tile(0, 20, 20).is_none() && w.lifts.is_empty());
    }

    #[test]
    fn big_buildings_block_their_whole_footprint() {
        let mut w = World::blank();
        w.place(0, 10, 10, Kind::TradePost, Dir::E).unwrap();
        assert!(w.check_place(0, 12, 12, Kind::Belt).is_err());
        assert!(w.check_place(0, 13, 12, Kind::Belt).is_ok());
        assert_eq!(w.anchor_of(0, 12, 11), Some((10, 10)));
        assert_eq!(w.remove(0, 11, 12), Some(Kind::TradePost));
        assert!(w.check_place(0, 12, 12, Kind::Belt).is_ok());
    }

    #[test]
    fn low_power_slows_evenly() {
        let mut w = World::blank();
        w.place(0, 10, 10, Kind::Solar, Dir::E).unwrap();
        // 32 belts fill the panel's reach around it: 16 power wanted, 10 available.
        for (x, y) in footprint(8, 8, 6) {
            if w.check_place(0, x, y, Kind::Belt).is_ok() {
                w.place(0, x, y, Kind::Belt, Dir::E).unwrap();
            }
        }
        assert!((w.ratio_at(0, 8, 8) - 10.0 / 16.0).abs() < 1e-9);
    }

    #[test]
    fn poles_carry_power_to_far_belts() {
        let mut w = World::blank();
        w.place(0, 0, 0, Kind::Solar, Dir::E).unwrap(); // reaches up to 3,3
        w.place(0, 10, 10, Kind::Belt, Dir::E).unwrap();
        assert_eq!(w.ratio_at(0, 10, 10), 0.0);
        assert_eq!(w.grid.unpowered, 1);
        w.place(0, 6, 6, Kind::Pole, Dir::E).unwrap(); // reaches 3-9: touches the panel
        assert_eq!(w.ratio_at(0, 10, 10), 0.0);
        w.place(0, 12, 12, Kind::Pole, Dir::E).unwrap(); // reaches 9-15: touches the first pole
        assert_eq!(w.ratio_at(0, 10, 10), 1.0);
        assert_eq!(w.grid.nets.len(), 1);
    }

    #[test]
    fn separate_grids_share_only_their_own_power() {
        let mut w = World::blank();
        w.place(0, 0, 0, Kind::Solar, Dir::E).unwrap();
        w.place(0, 3, 3, Kind::Belt, Dir::E).unwrap();
        w.place(0, 50, 50, Kind::Pole, Dir::E).unwrap(); // no panel on this grid
        w.place(0, 51, 50, Kind::Belt, Dir::E).unwrap();
        assert_eq!(w.grid.nets.len(), 2);
        assert_eq!(w.ratio_at(0, 3, 3), 1.0);
        assert_eq!(w.ratio_at(0, 51, 50), 0.0);
    }

    #[test]
    fn research_fades_to_a_floor() {
        let c = Item::CopperPlate.curve();
        assert_eq!(c.value(0), 12.0);
        assert!(c.value(100) < 12.0 && c.value(100) > 6.0);
        assert!((c.value(10_000) - 0.36).abs() < 1e-9);
    }

    #[test]
    fn solar_needs_sunlight() {
        let w = World::blank();
        assert!(w.check_place(1, 3, 3, Kind::Solar).is_err());
    }

    #[test]
    fn locked_parts_need_research() {
        let mut w = World::new();
        let spot = (60, 50);
        assert!(w.check_place(0, spot.0, spot.1, Kind::Press).unwrap_err().contains("locked"));
        assert!(w.research(Tech::Press).is_err()); // no research points yet
        w.econ.research = 100.0;
        w.research(Tech::Press).unwrap();
        assert_eq!(w.econ.research, 40.0);
        assert!(w.check_place(0, spot.0, spot.1, Kind::Press).is_ok());
        // Ashgrove isn't open yet, so its drill can't be researched.
        assert!(w.research(Tech::BasicDrill).unwrap_err().contains("Make copper wire first"));
    }

    #[test]
    fn making_wire_opens_ashgrove() {
        let mut w = World::blank();
        w.progress.opened[Company::Ashgrove.idx()] = false;
        w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
        fill_crate(&mut w, 0, 5, 5, &[Item::CopperPlate; 3]);
        w.place(0, 6, 5, Kind::Press, Dir::E).unwrap(); // makes wire by default
        w.place(0, 6, 7, Kind::Solar, Dir::E).unwrap(); // reaches the press
        run(&mut w, 5);
        assert!(w.econ.made[Item::CopperWire.idx()] >= 2);
        assert!(w.progress.is_open(Company::Ashgrove));
        assert_eq!(w.progress.events.pop_front(), Some(Company::Ashgrove));
    }

    #[test]
    fn coke_needs_coking_research() {
        let mut w = World::blank();
        w.progress.researched[Tech::Coking.idx()] = false;
        w.progress.researched[Tech::BlastFurnace.idx()] = false; // its concrete recipe would count too
        w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
        assert!(w.configure(0, 5, 5).unwrap().contains("only one recipe"));
        w.progress.researched[Tech::Coking.idx()] = true;
        assert!(w.configure(0, 5, 5).unwrap().contains("coke"));
        // Coal goes in, coke comes out, no separate fuel needed.
        w.place(0, 3, 5, Kind::Crate, Dir::E).unwrap();
        w.place(0, 4, 5, Kind::Belt, Dir::E).unwrap();
        w.place(0, 4, 7, Kind::Solar, Dir::E).unwrap();
        fill_crate(&mut w, 0, 3, 5, &[Item::Coal; 4]);
        run(&mut w, 10);
        assert_eq!(w.econ.made[Item::Coke.idx()], 2);
    }

    #[test]
    fn drills_mine_what_they_are_set_to_and_only_basic_drills_cut_iron() {
        let mut w = World::blank();
        // A cheap drill goes copper -> coal -> copper: no iron.
        w.place(0, 10, 10, Kind::Drill, Dir::E).unwrap();
        let mines = |w: &World, x, y| match &w.tile(0, x, y).unwrap().st {
            St::Drill { mines, .. } => *mines,
            _ => None,
        };
        assert_eq!(mines(&w, 10, 10), Some(Item::CopperOre));
        assert!(w.configure(0, 10, 10).unwrap().contains("iron needs a basic drill"));
        assert_eq!(mines(&w, 10, 10), Some(Item::Coal));
        w.configure(0, 10, 10);
        assert_eq!(mines(&w, 10, 10), Some(Item::CopperOre));
        w.apply_setting(0, 10, 10, Setting::Mines(Item::IronOre));
        assert_eq!(mines(&w, 10, 10), Some(Item::CopperOre), "a copied iron setting doesn't stick to a cheap drill");
        // A basic drill set to iron, anywhere on the ground.
        w.place(0, 20, 20, Kind::BasicDrill, Dir::E).unwrap();
        w.configure(0, 20, 20);
        w.configure(0, 20, 20);
        assert_eq!(mines(&w, 20, 20), Some(Item::IronOre));
        w.place(0, 22, 20, Kind::Crate, Dir::E).unwrap();
        run(&mut w, 10);
        assert!(crate_items(&w, 0, 22, 20).iter().all(|i| *i == Item::IronOre));
        assert_eq!(crate_items(&w, 0, 22, 20).len(), 10); // its starter coal mines 10 ores
    }

    #[test]
    fn blast_furnace_makes_iron_then_steel_with_waste() {
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
        fill_crate(&mut w, 0, 5, 5, &[Item::IronOre, Item::IronOre, Item::IronOre, Item::Coke]);
        w.place(0, 6, 5, Kind::BlastFurnace, Dir::E).unwrap(); // covers 6-7, 5-6
        w.place(0, 8, 5, Kind::Crate, Dir::E).unwrap();
        run(&mut w, 6);
        assert_eq!(crate_items(&w, 0, 8, 5), vec![Item::IronPlate; 3]);

        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
        fill_crate(&mut w, 0, 5, 5, &[Item::IronPlate, Item::IronPlate, Item::IronPlate, Item::Coke]);
        w.place(0, 6, 5, Kind::BlastFurnace, Dir::E).unwrap();
        assert!(w.configure(0, 6, 5).unwrap().contains("steel"));
        w.place(0, 8, 5, Kind::Crate, Dir::E).unwrap();
        run(&mut w, 6);
        assert_eq!(crate_items(&w, 0, 8, 5), vec![Item::Steel, Item::Waste]);
    }

    fn offer(w: &mut World, x: i32, y: i32, item: Item) -> bool {
        w.offer(0, x, y, item)
    }

    fn set_mode(w: &mut World, x: i32, y: i32, want: SellMode) {
        while !matches!(&w.tile(0, x, y).unwrap().st, St::Trade { mode, .. } if *mode == want) {
            w.configure(0, x, y);
        }
    }

    #[test]
    fn a_money_post_takes_waste_and_charges_for_it() {
        let mut w = World::blank();
        w.place(0, 10, 4, Kind::TradePost, Dir::E).unwrap();
        let before = w.econ.money;
        assert!(offer(&mut w, 10, 4, Item::Waste));
        assert!((w.econ.money - (before - 0.5)).abs() < 1e-9);
        assert_eq!(w.econ.sold[Item::Waste.idx()], 1);
        assert!(w.econ.money_from[Item::Waste.idx()] < 0.0);
    }

    #[test]
    fn waste_never_earns_research() {
        let mut w = World::blank();
        w.place(0, 10, 4, Kind::TradePost, Dir::E).unwrap();
        for _ in 0..20 {
            offer(&mut w, 10, 4, Item::Waste);
        }
        assert_eq!(w.econ.research, 0.0);
        assert_eq!(w.econ.research_sales[Item::Waste.idx()], 0);
        assert_eq!(Item::Waste.curve().value(0), 0.0);
        // Even a research post's refusal leaves research alone.
        set_mode(&mut w, 10, 4, SellMode::Research);
        assert!(!offer(&mut w, 10, 4, Item::Waste));
        assert_eq!(w.econ.research, 0.0);
    }

    #[test]
    fn research_and_split_posts_refuse_waste() {
        let mut w = World::blank();
        w.place(0, 10, 4, Kind::TradePost, Dir::E).unwrap();
        let money = w.econ.money;
        for mode in [SellMode::Research, SellMode::Split] {
            set_mode(&mut w, 10, 4, mode);
            assert!(!offer(&mut w, 10, 4, Item::Waste), "{mode:?}");
            assert!(w.econ.waste_refusal(mode).is_some());
        }
        assert_eq!(w.econ.money, money);
        // It still takes other things, and says WSTE.
        assert!(offer(&mut w, 10, 4, Item::CopperPlate));
        assert_eq!(w.idle_reason(0, idx(10, 4)), Some("WSTE"));
        // A refused waste item stays on the belt in front of the post.
        let mut w = World::blank();
        for x in 7..=9 {
            w.place(0, x, 5, Kind::Belt, Dir::E).unwrap();
        }
        w.place(0, 10, 4, Kind::TradePost, Dir::E).unwrap();
        w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap();
        set_mode(&mut w, 10, 4, SellMode::Research);
        if let Some(Building { st: St::Belt { items }, .. }) = w.floors[0].tiles[idx(9, 5)].as_mut() {
            items.push(BeltItem { item: Item::Waste, pos: 0.9 });
        }
        run(&mut w, 5);
        let Some(Building { st: St::Belt { items }, .. }) = w.floors[0].tiles[idx(9, 5)].as_ref() else { panic!() };
        assert_eq!(items.len(), 1, "the belt backs up");
        assert_eq!(w.econ.sold[Item::Waste.idx()], 0);
    }

    #[test]
    fn a_post_refuses_waste_when_money_would_go_negative() {
        let mut w = World::blank();
        w.place(0, 10, 4, Kind::TradePost, Dir::E).unwrap();
        w.econ.money = 0.75;
        assert!(offer(&mut w, 10, 4, Item::Waste));
        assert!((w.econ.money - 0.25).abs() < 1e-9);
        assert!(!offer(&mut w, 10, 4, Item::Waste), "$0.25 can't pay $0.50");
        assert!((w.econ.money - 0.25).abs() < 1e-9, "money never goes below zero");
        assert_eq!(w.econ.waste_refusal(SellMode::Money), Some("you can't afford the fee"));
    }

    #[test]
    fn waste_stops_selling_when_fees_pass_earnings() {
        let mut w = World::blank();
        w.place(0, 10, 4, Kind::TradePost, Dir::E).unwrap();
        // Earning $10 a minute: waste sells until its fees pass that.
        w.econ.money_from[Item::CopperPlate.idx()] = 10.0;
        let mut taken = 0;
        while offer(&mut w, 10, 4, Item::Waste) {
            taken += 1;
            assert!(taken < 100);
        }
        assert_eq!(taken, 21, "$10.50 of fees is more than the $10 earned");
        assert_eq!(w.econ.waste_refusal(SellMode::Money), Some("waste costs more than you're earning"));
        assert_eq!(w.idle_reason(0, idx(10, 4)), Some("WSTE"));
        // A minute later the old fees and earnings have left the window, and waste sells again.
        run(&mut w, 70);
        assert!(w.econ.waste_refusal(SellMode::Money).is_none());
        assert!(offer(&mut w, 10, 4, Item::Waste));
    }

    #[test]
    fn removing_a_crate_of_waste_costs_money() {
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
        fill_crate(&mut w, 0, 5, 5, &[Item::Waste; 200]);
        fill_crate(&mut w, 0, 5, 5, &[Item::Coal; 10]);
        assert_eq!(w.removal_fee(0, 5, 5), (200, 100.0));
        let price = Kind::Crate.price(1);
        w.econ.money = 99.0;
        let err = w.try_remove(0, 5, 5).unwrap_err();
        assert_eq!(err, "Holds 200 waste: removing it costs $100");
        assert!(w.tile(0, 5, 5).is_some() && w.econ.money == 99.0, "refused: nothing changes");
        w.econ.money = 150.0;
        assert_eq!(w.try_remove(0, 5, 5), Ok(Some(Kind::Crate)));
        assert!((w.econ.money - (50.0 + price)).abs() < 1e-9, "fee paid, refund given");
        assert!(w.tile(0, 5, 5).is_none());
        // An empty crate is free to remove, even with no money.
        w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
        w.econ.money = 0.0;
        assert_eq!(w.try_remove(0, 5, 5), Ok(Some(Kind::Crate)));
    }

    #[test]
    fn removing_an_elevator_with_waste_costs_money() {
        let mut w = World::blank();
        w.place(0, 20, 20, Kind::Elevator, Dir::E).unwrap();
        w.lifts[0].load.extend([Item::Waste, Item::Waste, Item::Coal]);
        assert_eq!(w.removal_fee(0, 20, 20), (2, 1.0));
        w.econ.money = 0.5;
        assert!(w.try_remove(0, 20, 20).unwrap_err().starts_with("Holds 2 waste"));
        w.econ.money = 5.0;
        assert_eq!(w.try_remove(0, 20, 20), Ok(Some(Kind::Elevator)));
    }

    #[test]
    fn old_version_saves_are_refused_and_listed() {
        let data = std::env::temp_dir().join(format!("tfac-old-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        std::fs::create_dir_all(crate::saves::dir(&data)).unwrap();
        // No version at all: from before versions.
        let old = crate::saves::path_of(&data, "save1");
        std::fs::write(&old, r#"{"floors":[],"tick":5}"#).unwrap();
        assert_eq!(World::file_version(&old), Some(0));
        let err = World::load(&old).err().unwrap();
        assert!(err.to_string().contains("older version"), "{err}");
        let msg = crate::saves::version_problem(&old, "save1").unwrap();
        assert!(msg.contains("older version") && msg.contains("tfac --new"), "{msg}");
        assert!(old.exists(), "never deleted");
        // A current save opens fine and says nothing.
        let mut w = World::new();
        let fresh = crate::saves::path_of(&data, "save2");
        w.save(&fresh).unwrap();
        assert_eq!(World::file_version(&fresh), Some(SAVE_VERSION));
        assert!(World::load(&fresh).is_ok() && crate::saves::version_problem(&fresh, "save2").is_none());
        let list = crate::saves::describe(&data);
        let line = |n: &str| list.lines().find(|l| l.trim_start().starts_with(n)).unwrap().to_string();
        assert!(line("save1").contains("(old version)"), "{list}");
        assert!(!line("save2").contains("(old version)") && line("save2").contains("research"), "{list}");
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn solar_panels_need_poles_between_them() {
        let mut w = World::blank();
        w.place(0, 10, 10, Kind::Solar, Dir::E).unwrap();
        w.place(0, 12, 10, Kind::Solar, Dir::E).unwrap();
        assert_eq!(w.grid.nets.len(), 2, "side by side, two panels are still two grids");
        assert_eq!(w.source_net(0, idx(10, 10)).map(|n| n.1), Some(1));
        // Nothing to power yet: their supply counts as unused, not as grid supply.
        let (supply, _, _, _, _, unused) = w.power_summary();
        assert_eq!((supply, unused), (0.0, 2.0 * SOLAR_OUTPUT));
        w.place(0, 12, 13, Kind::Pole, Dir::E).unwrap();
        assert_eq!(w.grid.nets.len(), 1, "a pole between them joins them");
        assert_eq!(w.grid.nets[0].solar, 2.0 * SOLAR_OUTPUT);
        assert_eq!(w.source_net(0, idx(10, 10)).map(|n| n.1), Some(3));
        assert_eq!(w.grid_at(0, 11, 11), Some(0), "any tile of a panel finds its grid");
        assert_eq!(w.grid_at(0, 30, 30), None, "empty ground has none");
    }

    #[test]
    fn steam_engines_power_a_grid_when_solar_is_not_enough() {
        let mut w = World::blank();
        // A pump, pipes to a boiler, boiler straight into an engine.
        w.place(0, 5, 5, Kind::Pump, Dir::E).unwrap();
        w.place(0, 6, 5, Kind::Pipe, Dir::E).unwrap();
        w.place(0, 7, 5, Kind::Pipe, Dir::E).unwrap();
        w.place(0, 8, 5, Kind::Boiler, Dir::E).unwrap(); // covers 8-9, 5-6; front is x=10
        w.place(0, 10, 5, Kind::SteamEngine, Dir::E).unwrap(); // covers 10-11, 5-6
        w.place(0, 8, 7, Kind::Crate, Dir::N).unwrap(); // pushes coal up into the boiler
        fill_crate(&mut w, 0, 8, 7, &[Item::Coal; 5]);
        w.place(0, 5, 7, Kind::Solar, Dir::E).unwrap(); // runs the pump, and links to the engine's grid
        // A pole and 27 belts: far more than one solar panel can run.
        w.place(0, 14, 5, Kind::Pole, Dir::E).unwrap();
        for (x, y) in footprint(14, 2, 4).chain(footprint(14, 6, 3)) {
            if w.check_place(0, x, y, Kind::Belt).is_ok() {
                w.place(0, x, y, Kind::Belt, Dir::E).unwrap();
            }
        }
        w.tick();
        assert!(w.ratio_at(0, 15, 3) < 1.0, "solar alone can't keep up");
        run(&mut w, 10);
        assert_eq!(w.ratio_at(0, 15, 3), 1.0, "the engine makes up the rest");
        // Placing something elsewhere keeps the water already in the pipes.
        let water = w.fluids.pools[0].amount;
        w.place(0, 40, 40, Kind::Belt, Dir::E).unwrap();
        assert_eq!(w.fluids.pools[0].amount, water);
    }

    #[test]
    fn short_lists_are_padded() {
        let e: Econ = serde_json::from_str(r#"{"money":5.0,"research":1.0,"research_sales":[1,2],"sold":[0],"made":[3]}"#).unwrap();
        assert_eq!(e.made[0], 3);
        assert_eq!(e.made.len(), ITEMS);
        assert_eq!(e.made[ITEMS - 1], 0);
    }

    #[test]
    fn no_ore_patches_or_lakes_drills_and_pumps_go_anywhere_on_the_ground() {
        let w = World::new();
        for fl in &w.floors {
            assert!(fl.deposits.iter().all(|d| *d == Deposit::None), "no ore patches or lakes");
        }
        assert!(w.check_place(0, 30, 30, Kind::Drill).is_ok(), "bare ground is fine");
        let w = World::blank();
        assert!(w.check_place(0, 30, 30, Kind::Pump).is_ok(), "pumps draw water from anywhere");
        let up = w.floors.iter().position(|fl| fl.level == 1).unwrap();
        for kind in [Kind::Drill, Kind::Pump] {
            assert!(w.check_place(up, 30, 30, kind).unwrap_err().contains("ground"));
        }
        let down = w.floors.iter().position(|fl| fl.level == -1).unwrap();
        assert!(w.check_place(down, 30, 30, Kind::Pump).is_ok());
        assert_eq!(richness(-1), 1.5);
        assert_eq!(richness(-3), 3.0);
    }

    #[test]
    fn drills_mine_faster_underground() {
        let mut w = World::blank();
        for f in [0, 1] {
            w.place(f, 5, 5, Kind::Drill, Dir::E).unwrap();
            w.place(f, 7, 5, Kind::Crate, Dir::E).unwrap();
        }
        run(&mut w, 4);
        let (top, deep) = (crate_items(&w, 0, 7, 5).len(), crate_items(&w, 1, 7, 5).len());
        assert_eq!(top, 4);
        assert_eq!(deep, 6);
    }

    #[test]
    fn floors_open_with_beams_and_digging() {
        let mut w = World::new();
        let up = w.floor_of(1).unwrap();
        let down = w.floor_of(-1).unwrap();
        assert!(w.check_place(up, 60, 40, Kind::Crate).unwrap_err().contains("support beams"));
        assert!(w.check_place(down, 60, 40, Kind::Crate).unwrap_err().contains("excavation"));
        w.progress.researched[Tech::SupportBeams.idx()] = true;
        // Upper floors cost $10 a tile to deck over; elevators reach every open floor.
        assert_eq!(w.check_place(up, 60, 40, Kind::Crate).unwrap(), 50.0 + 10.0);
        assert!(w.check_place(w.floor_of(2).unwrap(), 60, 40, Kind::Crate).is_err(), "floor +2 comes later");
    }

    #[test]
    fn milestones_unlock_for_free() {
        let mut w = World::new();
        assert!(!w.progress.has(Tech::Press));
        w.econ.made[Item::CopperPlate.idx()] = 49;
        w.tick();
        assert!(!w.progress.has(Tech::Press));
        w.econ.made[Item::CopperPlate.idx()] = 50;
        w.tick();
        assert!(w.progress.has(Tech::Press));
        assert_eq!(w.econ.research, 0.0, "nothing was spent");
        assert!(w.progress.notes.pop_front().unwrap().contains("free"));
    }

    #[test]
    fn fast_belts_move_twice_as_much() {
        let mut w = World::blank();
        for (y, kind) in [(5, Kind::Belt), (8, Kind::FastBelt)] {
            w.place(0, 4, y, Kind::Crate, Dir::E).unwrap();
            fill_crate(&mut w, 0, 4, y, &[Item::Coal; 200]);
            for x in 5..=8 {
                w.place(0, x, y, kind, Dir::E).unwrap();
            }
            w.place(0, 9, y, Kind::Crate, Dir::E).unwrap();
        }
        w.place(0, 6, 6, Kind::Solar, Dir::E).unwrap();
        run(&mut w, 10);
        let (slow, fast) = (crate_items(&w, 0, 9, 5).len(), crate_items(&w, 0, 9, 8).len());
        assert!(fast as f64 > slow as f64 * 1.8, "slow {slow}, fast {fast}");
    }

    #[test]
    fn assembler_makes_the_drilling_rig() {
        let mut w = World::blank();
        w.place(0, 4, 5, Kind::Crate, Dir::E).unwrap();
        fill_crate(&mut w, 0, 4, 5, &[Item::SteelGirder, Item::SteelGirder, Item::SteelGirder, Item::SteelGirder]);
        fill_crate(&mut w, 0, 4, 5, &[Item::Gearbox, Item::Gearbox, Item::Steel, Item::Steel, Item::Steel, Item::Steel]);
        w.place(0, 5, 4, Kind::Assembler, Dir::E).unwrap(); // 3x3
        while !w.configure(0, 5, 4).unwrap().contains("drilling rig") {}
        w.place(0, 8, 5, Kind::Crate, Dir::E).unwrap();
        w.place(0, 5, 8, Kind::Solar, Dir::E).unwrap(); // reaches the assembler's bottom row only
        run(&mut w, 15);
        assert_eq!(crate_items(&w, 0, 8, 5), vec![Item::DrillingRig]);
    }

    #[test]
    fn idle_buildings_say_why() {
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Drill, Dir::E).unwrap(); // nothing at its front
        run(&mut w, 5);
        assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FULL"));
        w.place(0, 10, 10, Kind::Furnace, Dir::E).unwrap();
        w.tick();
        assert_eq!(w.idle_reason(0, idx(10, 10)), Some("NEED"));
    }

    #[test]
    fn drill_behind_unpowered_belt_says_pwr() {
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Drill, Dir::E).unwrap();
        for x in 7..12 {
            w.place(0, x, 5, Kind::Belt, Dir::E).unwrap();
        }
        run(&mut w, 10);
        assert_eq!(w.idle_reason(0, idx(5, 5)), Some("PWR"));
        w.place(0, 8, 7, Kind::Solar, Dir::E).unwrap();
        w.tick();
        assert_ne!(w.idle_reason(0, idx(5, 5)), Some("PWR"));
    }

    #[test]
    fn offline_earnings_have_no_cap() {
        let mut w = World::new();
        // Selling one copper plate a second, all of it for research.
        let plate = Item::CopperPlate;
        w.rate = (2.0, w.econ.next_research(plate));
        w.sale_rate = vec![0.0; ITEMS];
        w.sale_rate[plate.idx()] = 1.0;
        w.saved_at = 1_000;
        let (away, m, r) = w.apply_offline(1_000 + 10 * 3600).unwrap();
        assert_eq!(away, 36_000);
        assert_eq!(m, 72_000.0);
        assert_eq!(w.econ.money, START_MONEY + 72_000.0);
        // Research fades just as it would in play: the same as 36,000 sales one by one.
        let by_hand: f64 = (0..36_000).map(|n| plate.curve().value(n)).sum();
        assert!((r - by_hand).abs() < 1.0, "{r} vs {by_hand}");
        assert!(r < 36_000.0 * 12.0 * 0.1, "fades: {r}");
        assert_eq!(w.econ.research_sales[plate.idx()], 36_000);
    }

    #[test]
    fn agent_mail_brings_spaced_deliveries() {
        let mut w = World::new();
        let money = w.econ.money;
        let msg = || crate::mail::parse("done", 0);
        w.receive(msg(), 1_000);
        w.receive(msg(), 1_010);
        w.receive(msg(), 1_000 + GIFT_GAP);
        assert_eq!(w.mail.len(), 3);
        let gifts: Vec<bool> = w.mail.iter().map(|m| m.gift.is_some()).collect();
        assert_eq!(gifts, [true, false, true]);
        assert_eq!(w.econ.money, money + 50.0);
    }

    #[test]
    fn save_and_load_round_trip() {
        let mut w = World::new();
        w.build_demo().unwrap();
        run(&mut w, 20);
        let path = std::env::temp_dir().join(format!("factory-test-{}.json", std::process::id()));
        w.save(&path).unwrap();
        let mut l = World::load(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(l.tick, w.tick);
        assert_eq!(l.econ.money, w.econ.money);
        assert_eq!(l.tile(0, 54, 38).map(|b| b.kind), Some(Kind::Furnace));
        // Both keep running identically after loading.
        run(&mut w, 10);
        run(&mut l, 10);
        assert_eq!(l.econ.sold, w.econ.sold);
    }
}

