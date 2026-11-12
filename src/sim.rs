//! Simulation core: the world, buildings and the fixed-tick update.
//! Nothing here draws or reads input, so another frontend can reuse it later.
//! There is no randomness during play: the same factory always behaves the same.
//!
//! Buildings can be bigger than one tile (2x2, 3x3). Each building is stored at its
//! top-left "anchor" tile, and every tile it covers points back to that anchor.
//! Buildings take input from any side and send output out of their front only.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

mod blueprint;
mod serde_util;
mod tools;
pub use blueprint::{Blueprint, PasteReport, PlanState, BP_FORMAT};
use serde_util::{de_set, ser_set};
pub use serde_util::{PerFluid, PerItem};
pub use tools::{rect_of, rect_size, Removed, TileRect, UpState, UpgradeReport, Upgraded};

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
/// The basic solar panel (solar mk2): twice the EC, twice the reach.
pub const SOLAR2_OUTPUT: f64 = 20.0;
/// How far beyond its own edge each power source reaches.
pub const SOLAR_REACH: i32 = 2;
pub const SOLAR2_REACH: i32 = 4;
pub const POLE_REACH: i32 = 3;
pub const ELEVATOR_REACH: i32 = 1;

pub const DRILL_COAL_CAP: u32 = 10;
pub const DRILL_OUT_CAP: usize = 4;
pub const CRAFTER_OUT_CAP: usize = 10;
/// Units of fluid a machine holds waiting to leave its front.
pub const CRAFTER_FLUID_OUT_CAP: u32 = 10;
pub const CRATE_CAP: usize = 400;

// Fluids and steam (per second unless noted).
pub const PIPE_CAP: f64 = 20.0; // fluid each pipe tile holds
pub const PUMP_RATE: f64 = 20.0; // water a pump adds
pub const BOILER_RATE: f64 = 10.0; // water in, steam out
pub const ENGINE_POWER: f64 = 100.0; // power at full steam
const STEAM_PER_TICK: f64 = 10.0 / TPS as f64; // steam an engine burns per tick at full power
pub const ENGINE_BUF: f64 = 20.0;
pub const BOILER_COAL_CAP: u32 = 10;
/// How much waste makes the fire of one coal in a boiler. A boiler holds the fire of
/// `BOILER_COAL_CAP` coal, whatever mix of coal and waste made it.
pub const WASTE_PER_COAL: u32 = 4;
pub const ENGINE_REACH: i32 = 2;
/// Bump when the save format changes: older saves are not loaded.
pub const SAVE_VERSION: u32 = 6;
/// The game version, saved in every file.
pub const GAME_VERSION: &str = env!("CARGO_PKG_VERSION");

/// An array of defaults (the std Default stops at 32 elements).
fn zeros<T: Default + Copy, const N: usize>() -> [T; N] {
    [T::default(); N]
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
    CrudeAssembler,
    FastBelt,
    Tunnel,
    LongArm,
    Sorter,
    Warehouse,
    Molder,
    CokeOven,
    Crusher,
    WireDrawer,
    RollingMill,
    Lathe,
    Winder,
    BasicAssembler,
    AdvancedAssembler,
    PrecisionAssembler,
    LargeBasicAssembler,
    LargeAdvancedAssembler,
    Welder,
    Kiln,
    Mixer,
    AdvancedDrill,
    LargeBasicDrill,
    LargeAdvancedDrill,
    AdvancedPump,
    LargeKiln,
    BasicSolar,
}

// What each assembler above the crude one can make: its tiers, each at a speed
// (a tier runs at 1x on the lowest assembler that can make it, faster on the ones above).
static BASIC_ASSEMBLER: LazyLock<Vec<Recipe>> = LazyLock::new(|| assembler_list(&[(CRUDE_RECIPES, 2), (BASIC_RECIPES, 1)]));
static ADVANCED_ASSEMBLER: LazyLock<Vec<Recipe>> = LazyLock::new(|| {
    let mut list = assembler_list(&[(CRUDE_RECIPES, 4), (BASIC_RECIPES, 2), (ADVANCED_RECIPES, 1)]);
    // It has the proper control board recipe: no need for the hand-wired one.
    list.retain(|r| r.name != HAND_WIRED);
    list
});
// The precision assembler only makes the top tiers: no crude or basic recipes.
static PRECISION_ASSEMBLER: LazyLock<Vec<Recipe>> = LazyLock::new(|| assembler_list(&[(ADVANCED_RECIPES, 2), (PRECISION_RECIPES, 1)]));
// The large kiln is four kilns in one: four times the batch (32 against 8) in the same 20 s firing,
// burning four times the coal (see `Kind::fires`).
static LARGE_KILN: LazyLock<Vec<Recipe>> = LazyLock::new(|| batch_list(KILN_RECIPES, 1));
static LARGE_ADVANCED_ASSEMBLER: LazyLock<Vec<Recipe>> =
    LazyLock::new(|| assembler_list(&[(LARGE_BASIC_RECIPES, 2), (LARGE_ADVANCED_RECIPES, 1)]));

#[cfg(test)]
thread_local! {
    static RECIPE_OVERRIDE: std::cell::Cell<Option<(Kind, &'static [Recipe])>> = const { std::cell::Cell::new(None) };
}

impl Kind {
    /// Every part (the build menu itself is laid out in app::slots).
    pub const ALL: [Kind; 44] = [
        Kind::Drill,
        Kind::BasicDrill,
        Kind::Crusher,
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
        Kind::Molder,
        Kind::Press,
        Kind::BlastFurnace,
        Kind::CokeOven,
        Kind::WireDrawer,
        Kind::Winder,
        Kind::RollingMill,
        Kind::Lathe,
        Kind::CrudeAssembler,
        Kind::Crate,
        Kind::Solar,
        Kind::Pole,
        Kind::Boiler,
        Kind::SteamEngine,
        Kind::TradePost,
        Kind::Elevator,
        Kind::BasicAssembler,
        Kind::AdvancedAssembler,
        Kind::PrecisionAssembler,
        Kind::LargeBasicAssembler,
        Kind::LargeAdvancedAssembler,
        Kind::Welder,
        Kind::Kiln,
        Kind::Mixer,
        Kind::AdvancedDrill,
        Kind::LargeBasicDrill,
        Kind::LargeAdvancedDrill,
        Kind::AdvancedPump,
        Kind::LargeKiln,
        Kind::BasicSolar,
    ];
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn category(self) -> Category {
        match self {
            Kind::Drill
            | Kind::BasicDrill
            | Kind::Furnace
            | Kind::Molder
            | Kind::BlastFurnace
            | Kind::CokeOven
            | Kind::Crusher
            | Kind::WireDrawer
            | Kind::RollingMill
            | Kind::Lathe
            | Kind::Winder
            | Kind::Press
            | Kind::CrudeAssembler
            | Kind::BasicAssembler
            | Kind::AdvancedAssembler
            | Kind::PrecisionAssembler
            | Kind::LargeBasicAssembler
            | Kind::LargeAdvancedAssembler
            | Kind::Welder
            | Kind::Kiln
            | Kind::LargeKiln
            | Kind::Mixer
            | Kind::AdvancedDrill
            | Kind::LargeBasicDrill
            | Kind::LargeAdvancedDrill
            | Kind::AdvancedPump
            | Kind::Pump => Category::Create,
            Kind::Belt
            | Kind::FastBelt
            | Kind::Tunnel
            | Kind::Splitter
            | Kind::Sorter
            | Kind::Arm
            | Kind::LongArm
            | Kind::Elevator
            | Kind::Pipe => Category::Transport,
            Kind::Solar | Kind::BasicSolar | Kind::Pole | Kind::Boiler | Kind::SteamEngine => Category::Power,
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
            Kind::Furnace => "Furnace",
            Kind::Molder => "Molder",
            Kind::Press => "Press",
            Kind::CrudeAssembler => "Crude assembler",
            Kind::BasicAssembler => "Basic assembler",
            Kind::AdvancedAssembler => "Advanced assembler",
            Kind::PrecisionAssembler => "Precision assembler",
            Kind::LargeBasicAssembler => "Large basic assembler",
            Kind::LargeAdvancedAssembler => "Large advanced assembler",
            Kind::Welder => "Welder",
            Kind::Kiln => "Kiln",
            Kind::LargeKiln => "Large kiln",
            Kind::Mixer => "Mixer",
            Kind::AdvancedDrill => "Advanced drill",
            Kind::LargeBasicDrill => "Large basic drill",
            Kind::LargeAdvancedDrill => "Large advanced drill",
            Kind::AdvancedPump => "Advanced pump",
            Kind::Crate => "Crate",
            Kind::TradePost => "Trade post",
            Kind::Solar => "Cheap solar panel",
            Kind::BasicSolar => "Basic solar panel",
            Kind::Pole => "EC pole",
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
            Kind::CokeOven => "Coke oven",
            Kind::Crusher => "Crusher",
            Kind::WireDrawer => "Wire drawer",
            Kind::RollingMill => "Rolling mill",
            Kind::Lathe => "Lathe",
            Kind::Winder => "Winder",
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
            Kind::Molder => 100.0,
            Kind::Press => 150.0,
            Kind::CrudeAssembler => 300.0,
            Kind::BasicAssembler => 600.0,
            Kind::AdvancedAssembler => 9000.0,
            Kind::PrecisionAssembler => 20_000.0,
            Kind::LargeBasicAssembler => 12_000.0,
            Kind::LargeAdvancedAssembler => 30_000.0,
            Kind::Welder => 800.0,
            Kind::Kiln => 200.0,
            // About four minutes of the ~$2,000 a minute a tidy player earns when its tech (10,000 RP) arrives.
            Kind::LargeKiln => 7500.0,
            Kind::Mixer => 200.0,
            Kind::AdvancedDrill => 240.0,
            Kind::LargeBasicDrill => 1000.0,
            Kind::LargeAdvancedDrill => 2000.0,
            Kind::AdvancedPump => 240.0,
            Kind::Crate => 50.0,
            Kind::TradePost => 0.0,
            Kind::Solar => 100.0,
            // $15 an EC against the cheap panel's $10: you pay for reach (fewer poles), not for raw power.
            Kind::BasicSolar => 300.0,
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
            Kind::CokeOven => 150.0,
            Kind::Crusher => 250.0,
            Kind::WireDrawer => 250.0,
            Kind::RollingMill => 400.0,
            Kind::Lathe => 200.0,
            Kind::Winder => 200.0,
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
    /// Footprint in tiles as (width, height) when facing east. Facing north or south
    /// the two swap; facing west is the same as east. Most parts are square.
    /// Elevators can be built from 2x2 up to 8x8 (this is their smallest).
    pub fn base_dims(self) -> (i32, i32) {
        match self {
            Kind::Furnace | Kind::TradePost | Kind::Lathe | Kind::AdvancedAssembler | Kind::PrecisionAssembler | Kind::Welder => (3, 3),
            Kind::Kiln => (3, 5),
            Kind::LargeKiln => (5, 7),
            Kind::CrudeAssembler | Kind::AdvancedDrill => (4, 4),
            Kind::LargeBasicAssembler | Kind::LargeAdvancedAssembler => (5, 5),
            Kind::LargeBasicDrill | Kind::LargeAdvancedDrill => (8, 8),
            Kind::Press | Kind::WireDrawer | Kind::Winder => (2, 1),
            Kind::Pump => (1, 2),
            Kind::BasicAssembler
            | Kind::Mixer
            | Kind::AdvancedPump
            | Kind::Drill
            | Kind::BasicDrill
            | Kind::Molder
            | Kind::BlastFurnace
            | Kind::CokeOven
            | Kind::Crusher
            | Kind::RollingMill
            | Kind::Solar
            | Kind::BasicSolar
            | Kind::Elevator
            | Kind::Boiler
            | Kind::SteamEngine
            | Kind::Warehouse => (2, 2),
            Kind::Belt
            | Kind::FastBelt
            | Kind::Tunnel
            | Kind::Splitter
            | Kind::Sorter
            | Kind::Arm
            | Kind::LongArm
            | Kind::Crate
            | Kind::Pole
            | Kind::Pipe => (1, 1),
        }
    }
    /// Footprint (width, height) of a part facing `dir`.
    pub fn dims(self, dir: Dir) -> (i32, i32) {
        let (w, h) = self.base_dims();
        if self.directional() && matches!(dir, Dir::N | Dir::S) {
            (h, w)
        } else {
            (w, h)
        }
    }
    pub fn power_draw(self) -> f64 {
        match self {
            Kind::Belt => 0.5,
            Kind::FastBelt | Kind::Pump => 1.0,
            Kind::Splitter | Kind::Sorter | Kind::Molder => 2.0,
            Kind::Press => 2.0,
            Kind::Mixer | Kind::AdvancedDrill | Kind::AdvancedPump => 4.0,
            Kind::BasicAssembler => 6.0,
            Kind::LargeBasicAssembler => 12.0,
            Kind::Crusher => 6.0,
            Kind::WireDrawer | Kind::Lathe => 4.0,
            Kind::RollingMill => 8.0,
            Kind::Winder => 2.0,
            Kind::Arm => 2.0,
            Kind::LongArm => 4.0,
            Kind::AdvancedAssembler => 16.0,
            Kind::LargeAdvancedAssembler => 48.0,
            Kind::PrecisionAssembler => 64.0,
            Kind::LargeAdvancedDrill => 40.0,
            _ => 0.0,
        }
    }
    /// EC drawn only while the machine is working (none when idle). The grid's demand
    /// follows it tick by tick, so a grid sized for the average browns out while it works.
    pub fn burst_draw(self) -> f64 {
        match self {
            Kind::Welder => 30.0,
            _ => 0.0,
        }
    }
    /// Runs on EC (always-on draw or bursts).
    pub fn electric(self) -> bool {
        self.power_draw() > 0.0 || self.burst_draw() > 0.0
    }
    /// Runs this many recipe runs at once: it loads inputs for all of them, then fires them
    /// together (one recipe time). Only kilns do.
    pub fn batch(self) -> u32 {
        match self {
            Kind::Kiln => 8,
            Kind::LargeKiln => 32,
            _ => 1,
        }
    }
    /// Finished goods a machine holds before it stops.
    pub fn out_cap(self) -> usize {
        CRAFTER_OUT_CAP * self.batch() as usize
    }
    /// Has a front that can be rotated (belts and arms move that way; buildings output there).
    pub fn directional(self) -> bool {
        !matches!(self, Kind::Splitter | Kind::TradePost | Kind::Solar | Kind::BasicSolar | Kind::Pole | Kind::Pipe | Kind::SteamEngine)
    }
    pub fn is_belt(self) -> bool {
        matches!(self, Kind::Belt | Kind::FastBelt)
    }
    /// Connects to neighbouring pipes.
    pub fn uses_pipes(self) -> bool {
        matches!(
            self,
            Kind::Pipe
                | Kind::Pump
                | Kind::AdvancedPump
                | Kind::Boiler
                | Kind::SteamEngine
                | Kind::Furnace
                | Kind::BlastFurnace
                | Kind::Molder
                | Kind::Mixer
        )
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
            Kind::Pipe | Kind::Pump | Kind::AdvancedPump | Kind::Mixer => Some(Company::Wellbourne),
            Kind::BlastFurnace | Kind::CokeOven => Some(Company::Cinderhall),
            Kind::Boiler | Kind::SteamEngine | Kind::WireDrawer | Kind::Winder | Kind::BasicSolar => Some(Company::Kettering),
            Kind::Crusher => Some(Company::Ashgrove),
            Kind::Lathe | Kind::CrudeAssembler => Some(Company::Gearwright),
            Kind::RollingMill | Kind::Welder => Some(Company::Pillar),
            Kind::Elevator | Kind::Kiln | Kind::LargeKiln => Some(Company::Underhill),
            Kind::FastBelt | Kind::BasicAssembler => Some(Company::Swiftline),
            Kind::AdvancedDrill | Kind::LargeBasicAssembler | Kind::LargeBasicDrill => Some(Company::Ampleworth),
            Kind::AdvancedAssembler | Kind::PrecisionAssembler | Kind::LargeAdvancedAssembler | Kind::LargeAdvancedDrill => {
                Some(Company::LatticeFinch)
            }
            _ => Some(Company::Brannock),
        }
    }
    /// What must be unlocked before it can be bought (elevators: see `Progress::part_unlocked`).
    pub fn tech(self) -> Option<Tech> {
        match self {
            Kind::Press => Some(Tech::Press),
            Kind::BasicDrill => Some(Tech::BasicDrill),
            Kind::Pipe | Kind::Pump => Some(Tech::Pumps),
            Kind::AdvancedPump => Some(Tech::AdvancedPump),
            Kind::Mixer => Some(Tech::Mixer),
            Kind::Welder => Some(Tech::Welder),
            Kind::Kiln => Some(Tech::Kiln),
            Kind::LargeKiln => Some(Tech::LargeKiln),
            Kind::BasicSolar => Some(Tech::BasicSolar),
            Kind::BasicAssembler => Some(Tech::BasicAssembler),
            Kind::AdvancedAssembler => Some(Tech::AdvancedAssembler),
            Kind::PrecisionAssembler => Some(Tech::PrecisionAssembler),
            Kind::LargeBasicAssembler => Some(Tech::LargeBasicAssembler),
            Kind::LargeAdvancedAssembler => Some(Tech::LargeAdvancedAssembler),
            Kind::AdvancedDrill => Some(Tech::AdvancedDrill),
            Kind::LargeBasicDrill => Some(Tech::LargeBasicDrill),
            Kind::LargeAdvancedDrill => Some(Tech::LargeAdvancedDrill),
            Kind::BlastFurnace => Some(Tech::BlastFurnace),
            Kind::Boiler | Kind::SteamEngine => Some(Tech::Steam),
            Kind::CrudeAssembler => Some(Tech::Assembler),
            Kind::Crusher => Some(Tech::Crusher),
            Kind::WireDrawer => Some(Tech::WireDrawer),
            Kind::Winder => Some(Tech::Winder),
            Kind::CokeOven => Some(Tech::CokeOven),
            Kind::Lathe => Some(Tech::Lathe),
            Kind::RollingMill => Some(Tech::RollingMill),
            Kind::FastBelt => Some(Tech::FastBelts),
            Kind::Tunnel => Some(Tech::Tunnels),
            Kind::LongArm => Some(Tech::LongArms),
            Kind::Sorter => Some(Tech::FilterSplitters),
            Kind::Warehouse => Some(Tech::Warehouses),
            _ => None,
        }
    }
    pub fn recipes(self) -> &'static [Recipe] {
        // Tests can swap a machine's recipes (this thread only) to try recipes no part has yet.
        #[cfg(test)]
        if let Some(list) = RECIPE_OVERRIDE.with(|o| o.get()).filter(|(k, _)| *k == self).map(|(_, l)| l) {
            return list;
        }
        match self {
            Kind::Furnace => FURNACE_RECIPES,
            Kind::Molder => MOLDER_RECIPES,
            Kind::Press => PRESS_RECIPES,
            Kind::BlastFurnace => BLAST_RECIPES,
            Kind::CrudeAssembler => CRUDE_RECIPES,
            Kind::BasicAssembler => &BASIC_ASSEMBLER,
            Kind::AdvancedAssembler => &ADVANCED_ASSEMBLER,
            Kind::PrecisionAssembler => &PRECISION_ASSEMBLER,
            Kind::LargeBasicAssembler => LARGE_BASIC_RECIPES,
            Kind::LargeAdvancedAssembler => &LARGE_ADVANCED_ASSEMBLER,
            Kind::Welder => WELDER_RECIPES,
            Kind::Kiln => KILN_RECIPES,
            Kind::LargeKiln => &LARGE_KILN,
            Kind::Mixer => MIXER_RECIPES,
            Kind::CokeOven => COKE_OVEN_RECIPES,
            Kind::Crusher => CRUSHER_RECIPES,
            Kind::WireDrawer => WIRE_DRAWER_RECIPES,
            Kind::RollingMill => ROLLING_MILL_RECIPES,
            Kind::Lathe => LATHE_RECIPES,
            Kind::Winder => WINDER_RECIPES,
            _ => &[],
        }
    }
    pub fn is_drill(self) -> bool {
        matches!(self, Kind::Drill | Kind::BasicDrill | Kind::AdvancedDrill | Kind::LargeBasicDrill | Kind::LargeAdvancedDrill)
    }
    /// The rules of a part that crafts nothing, as the short lines its recipe-book page prints
    /// (at most 4 lines of 38 columns). A part with rules gets a page even with no recipes.
    pub fn rules(self) -> Option<&'static [&'static str]> {
        match self {
            Kind::Pipe => Some(&[
                "A joined run of pipes is one pool.",
                "Each tile holds 20 units of one fluid.",
                "No flow, no pressure, no direction.",
                "Pumps and machines beside it join in.",
            ]),
            _ => None,
        }
    }
    /// Has a page in the recipe book: it crafts, mines, pumps, or has rules to read.
    pub fn has_book_page(self) -> bool {
        !self.recipes().is_empty() || self.is_drill() || self.is_pump() || self.rules().is_some()
    }
    /// A solar panel (never joins another panel directly: poles go between them).
    pub fn is_solar(self) -> bool {
        matches!(self, Kind::Solar | Kind::BasicSolar)
    }
    /// EC a solar panel gives (zero for anything else).
    pub fn solar_output(self) -> f64 {
        match self {
            Kind::Solar => SOLAR_OUTPUT,
            Kind::BasicSolar => SOLAR2_OUTPUT,
            _ => 0.0,
        }
    }
    /// How far beyond its own edge a power source reaches.
    pub fn reach(self) -> Option<i32> {
        match self {
            Kind::Solar => Some(SOLAR_REACH),
            Kind::BasicSolar => Some(SOLAR2_REACH),
            Kind::Pole => Some(POLE_REACH),
            Kind::Elevator => Some(ELEVATOR_REACH),
            Kind::SteamEngine => Some(ENGINE_REACH),
            _ => None,
        }
    }
    pub fn is_pump(self) -> bool {
        matches!(self, Kind::Pump | Kind::AdvancedPump)
    }
    /// Ticks per item mined by a drill doing one unit of work a tick (on the surface), and
    /// items mined per coal burned. A cheap drill cuts iron at half the speed of a basic one.
    fn drill_speed(self, ore: Item) -> (u32, u32) {
        match (self, ore) {
            (Kind::BasicDrill | Kind::AdvancedDrill | Kind::LargeBasicDrill | Kind::LargeAdvancedDrill, _) => (40, 10),
            (_, Item::IronOre) => (80, 8),
            _ => (60, 8),
        }
    }
    /// (work per tick, fuel burned per tick) of a drill, in units of a basic drill's.
    /// An advanced drill is 2x a basic one and runs on EC. A large basic drill mines 5x
    /// (7.5 ore a second: one belt) and burns 10x the coal; a large advanced drill is 5x an
    /// advanced one (15 a second: one fast belt).
    pub fn drill_rates(self) -> (u32, u32) {
        match self {
            Kind::AdvancedDrill => (2, 0),
            Kind::LargeBasicDrill => (5, 10),
            Kind::LargeAdvancedDrill => (10, 0),
            _ => (1, 1),
        }
    }
    /// Burns coal (the others run on EC).
    /// How many fires this machine keeps going at once: one coal lasts this many times less.
    /// The large kiln is four kilns in one, so it burns like four.
    pub fn fires(self) -> u32 {
        match self {
            Kind::LargeKiln => 4,
            _ => 1,
        }
    }

    pub fn burns_coal(self) -> bool {
        self.drill_rates().1 > 0
    }
    /// How many items a drill holds waiting to leave (the fast ones hold more).
    pub fn drill_out_cap(self) -> usize {
        DRILL_OUT_CAP.max(self.drill_rates().0 as usize)
    }
    /// How much coal a drill keeps on hand (the thirsty ones keep more).
    pub fn drill_coal_cap(self) -> u32 {
        DRILL_COAL_CAP * self.drill_rates().1.max(1)
    }
    /// What a drill can be set to mine (f cycles through these).
    pub fn ores(self) -> &'static [Item] {
        if self.is_drill() {
            &[Item::CopperOre, Item::Coal, Item::IronOre, Item::Stone]
        } else {
            &[]
        }
    }
    /// How much water a pump adds per second.
    pub fn pump_rate(self) -> f64 {
        if self == Kind::AdvancedPump {
            4.0 * PUMP_RATE
        } else {
            PUMP_RATE
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
    Drill {
        mines: Option<Item>,
        fuel: u32,
        coal: u32,
        progress: f64,
        out: VecDeque<Item>,
    },
    /// Items sorted front first (highest pos first).
    Belt {
        items: Vec<BeltItem>,
    },
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
    Arm {
        filter: Option<Item>,
        hold: Option<Item>,
        progress: f64,
    },
    /// Furnaces, molders, presses and blast furnaces: a chosen recipe, a stock of inputs, and finished goods.
    Crafter {
        /// Index into `kind.recipes()`. Saved by name instead (`recipe_name`): indices shift when a recipe is added.
        #[serde(skip)]
        recipe: u8,
        /// The recipe's name. Only filled in while saving (`World::save`) and read back on load
        /// (`World::load`); during play `recipe` is the truth.
        #[serde(default)]
        recipe_name: String,
        stock: PerItem<u32>,
        fuel: u32,
        progress: f64,
        busy: bool,
        out: VecDeque<Item>,
        /// Whole units of the recipe's input fluid taken in, and of its output fluid waiting to leave.
        #[serde(default)]
        fluid: u32,
        #[serde(default)]
        fluid_out: u32,
    },
    Crate {
        items: VecDeque<Item>,
    },
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
    Pump {
        busy: bool,
    },
    /// Burns coal to turn water (from pipes) into steam (out of its front).
    Boiler {
        fuel: u32,
        coal: u32,
        busy: bool,
    },
    /// Turns steam into power for its grid.
    Engine {
        steam: f64,
        output: f64,
    },
    /// A belt tunnel end. An entrance holds what's passing under, with how far each item
    /// still has to go (in tiles); an exit only marks where they come back up.
    Tunnel {
        exit: bool,
        items: VecDeque<(Item, f64)>,
    },
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
            Kind::Drill | Kind::BasicDrill | Kind::AdvancedDrill | Kind::LargeBasicDrill | Kind::LargeAdvancedDrill => {
                let (t, per_coal) = kind.drill_speed(Item::CopperOre);
                let fuel = if kind.burns_coal() { t * per_coal } else { 0 };
                St::Drill { mines: None, fuel, coal: 0, progress: 0.0, out: VecDeque::new() }
            }
            Kind::Belt | Kind::FastBelt => St::Belt { items: Vec::new() },
            Kind::Splitter | Kind::Sorter => {
                St::Splitter { buf: VecDeque::new(), cooldown: 0.0, last_in: None, tried: 0, tried_prev: 0, filter: None }
            }
            Kind::Arm | Kind::LongArm => St::Arm { filter: None, hold: None, progress: 0.0 },
            Kind::Furnace
            | Kind::Molder
            | Kind::Press
            | Kind::BlastFurnace
            | Kind::CrudeAssembler
            | Kind::BasicAssembler
            | Kind::AdvancedAssembler
            | Kind::PrecisionAssembler
            | Kind::LargeBasicAssembler
            | Kind::LargeAdvancedAssembler
            | Kind::Welder
            | Kind::Kiln
            | Kind::LargeKiln
            | Kind::Mixer
            | Kind::CokeOven
            | Kind::Crusher
            | Kind::WireDrawer
            | Kind::RollingMill
            | Kind::Lathe
            | Kind::Winder => St::Crafter {
                recipe: 0,
                recipe_name: String::new(),
                stock: PerItem::default(),
                fuel: 0,
                progress: 0.0,
                busy: false,
                out: VecDeque::new(),
                fluid: 0,
                fluid_out: 0,
            },
            Kind::Crate | Kind::Warehouse => St::Crate { items: VecDeque::new() },
            Kind::Tunnel => St::Tunnel { exit: false, items: VecDeque::new() },
            Kind::TradePost => St::Trade { mode: SellMode::Money, money: 0.0, research: 0.0, count: 0, stuck: 0 },
            Kind::Solar | Kind::BasicSolar => St::Solar,
            Kind::Pole => St::Pole,
            Kind::Elevator => St::Elevator { inbox: VecDeque::new(), outbox: VecDeque::new(), mode: DoorMode::Load },
            Kind::Pipe => St::Pipe,
            Kind::Pump | Kind::AdvancedPump => St::Pump { busy: false },
            Kind::Boiler => St::Boiler { fuel: 0, coal: 0, busy: false },
            Kind::SteamEngine => St::Engine { steam: 0.0, output: 0.0 },
        };
        Building { kind, dir, rr: 0, st, size: 0 }
    }

    /// Footprint (width, height) in tiles, turned to face `dir`.
    pub fn dims(&self) -> (i32, i32) {
        if self.size > 0 {
            (self.size as i32, self.size as i32)
        } else {
            self.kind.dims(self.dir)
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
    pub dug: Vec<bool>,
    /// Buildings, stored at their anchor (top-left) tile.
    pub tiles: Vec<Option<Building>>,
    /// For every tile: the anchor index of the building covering it.
    pub owner: Vec<Option<u32>>,
}

impl Floor {
    fn new(level: i32, dug: bool) -> Self {
        let n = (W * H) as usize;
        Floor { level, dug: vec![dug; n], tiles: vec![None; n], owner: vec![None; n] }
    }
    pub fn anchor_at(&self, x: i32, y: i32) -> Option<usize> {
        if !in_bounds(x, y) {
            return None;
        }
        self.owner[idx(x, y)].map(|a| a as usize)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Econ {
    pub money: f64,
    pub research: f64,
    /// Units sold that earned research, per item (drives the fading curve).
    pub research_sales: PerItem<u64>,
    pub sold: PerItem<u64>,
    /// Units ever produced by a machine, per item (drives discoveries).
    pub made: PerItem<u64>,
    /// Units of each fluid ever produced by a machine (a fluid can be a company's key).
    #[serde(default)]
    pub made_fluid: PerFluid<u64>,
    /// This session only: units used up by machines (inputs and fuel), and what each
    /// item's sales brought in. They drive the production panel.
    #[serde(skip, default = "zeros")]
    pub used: [u64; ITEMS],
    #[serde(skip, default = "zeros")]
    pub money_from: [f64; ITEMS],
    #[serde(skip, default = "zeros")]
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
    /// How much of a key has been made.
    pub fn made_of(&self, m: Made) -> u64 {
        match m {
            Made::Item(i) => self.made[i],
            Made::Fluid(f) => self.made_fluid[f],
        }
    }
}

impl Default for Econ {
    fn default() -> Self {
        Econ {
            money: 0.0,
            research: 0.0,
            research_sales: PerItem::default(),
            sold: PerItem::default(),
            made: PerItem::default(),
            made_fluid: PerFluid::default(),
            used: [0; ITEMS],
            money_from: [0.0; ITEMS],
            research_from: [0.0; ITEMS],
            earned_money: 0.0,
            earned_research: 0.0,
            minute_ago: (0.0, 0.0),
        }
    }
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
    if v.fract() == 0.0 {
        format!("${v:.0}")
    } else {
        format!("${v:.2}")
    }
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

fn ser_companies<S: serde::Serializer>(f: &[bool; COMPANIES], s: S) -> Result<S::Ok, S::Error> {
    ser_set(f, &Company::ALL, Company::idx, s)
}

fn de_companies<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[bool; COMPANIES], D::Error> {
    de_set(d, &Company::ALL, Company::idx, "company")
}

fn ser_techs<S: serde::Serializer>(f: &[bool; TECHS], s: S) -> Result<S::Ok, S::Error> {
    ser_set(f, &Tech::ALL, Tech::idx, s)
}

fn de_techs<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[bool; TECHS], D::Error> {
    de_set(d, &Tech::ALL, Tech::idx, "tech")
}

/// Which companies are open and what has been researched.
#[derive(Debug, Serialize, Deserialize)]
pub struct Progress {
    /// Saved as lists of names.
    #[serde(serialize_with = "ser_companies", deserialize_with = "de_companies")]
    pub opened: [bool; COMPANIES],
    #[serde(serialize_with = "ser_techs", deserialize_with = "de_techs")]
    pub researched: [bool; TECHS],
    /// Short messages for the player (free unlocks).
    #[serde(skip)]
    pub notes: VecDeque<String>,
}

impl Default for Progress {
    fn default() -> Self {
        let mut opened = [false; COMPANIES];
        for c in Company::ALL {
            opened[c.idx()] = c.key_items().is_empty();
        }
        Progress { opened, researched: [false; TECHS], notes: VecDeque::new() }
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
        k.company().is_none_or(|c| self.is_open(c)) && k.tech().is_none_or(|t| self.has(t))
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
    /// Everything the grid's parts want right now: `steady` plus the bursts of busy machines.
    pub demand: f64,
    /// What always-on parts want.
    pub steady: f64,
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
    /// Machines that draw only while working: (floor, anchor, grid).
    pub bursts: Vec<(usize, usize, usize)>,
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

/// One pipe network's fluid in a save. The network is named by one of its pipes (networks have no ids
/// that last), and the floor by its level.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PipeFill {
    pub level: i32,
    pub x: i32,
    pub y: i32,
    pub fluid: Fluid,
    pub amount: f64,
}

#[derive(Default)]
pub struct Fluids {
    /// Per floor, per tile: which pipe network a pipe belongs to (-1 = not a pipe).
    pub pipe_pool: Vec<Vec<i32>>,
    pub pools: Vec<Pool>,
}

/// Which grid powers a building: the first of its tiles that's in range (-1 = none).
/// A big building counts as powered when any part of it is in range.
fn grid_of(cover: &[i32], x: i32, y: i32, (w, h): (i32, i32)) -> i32 {
    footprint(x, y, w, h).filter(|&(tx, ty)| in_bounds(tx, ty)).map(|(tx, ty)| cover[idx(tx, ty)]).find(|&id| id >= 0).unwrap_or(-1)
}

/// Why a part can't be placed. `Display` gives the text the player reads.
#[derive(Clone, Debug, PartialEq)]
pub enum PlaceErr {
    Locked(Kind),
    /// The floor (by level) isn't open yet.
    FloorClosed(i32),
    NeedsSecondFloor,
    OffMap,
    Blocked,
    OtherFloorBlocked,
    /// Drills and pumps need ground under them.
    NotAboveGround(Kind),
    NeedsSun,
    /// Not enough money: the full cost.
    Money(f64),
}

impl std::fmt::Display for PlaceErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaceErr::Locked(k) => write!(f, "{} is locked: see the research tree (t)", k.name()),
            PlaceErr::FloorClosed(level) => f.write_str(floor_hint_text(*level)),
            PlaceErr::NeedsSecondFloor => f.write_str("Elevators need a second floor to go to"),
            PlaceErr::OffMap => f.write_str("Doesn't fit on the map here"),
            PlaceErr::Blocked => f.write_str("Something is in the way"),
            PlaceErr::OtherFloorBlocked => f.write_str("Another floor is blocked at this spot"),
            PlaceErr::NotAboveGround(k) => write!(f, "{}s need ground under them: build them on the surface or below", k.name()),
            PlaceErr::NeedsSun => f.write_str("Solar panels need sunlight"),
            PlaceErr::Money(cost) => write!(f, "Not enough money (need ${:.0})", cost),
        }
    }
}

/// Why a floor is closed, for messages.
pub fn floor_hint_text(level: i32) -> &'static str {
    match level {
        1 => "Building upward needs support beams (make steel girders, then see the tree)",
        -1 => "Digging down needs excavation (make concrete, then see the tree)",
        _ => "That floor opens in a later phase",
    }
}

/// The tiles a power source reaches: (x0, y0, x1, y1), inclusive.
pub fn coverage(kind: Kind, (w, h): (i32, i32), ax: i32, ay: i32) -> Option<(i32, i32, i32, i32)> {
    let r = kind.reach()?;
    Some((ax - r, ay - r, ax + w - 1 + r, ay + h - 1 + r))
}

fn overlaps(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0 <= b.2 && b.0 <= a.2 && a.1 <= b.3 && b.1 <= a.3
}

#[derive(Serialize, Deserialize)]
pub struct World {
    /// The save format (0 when missing: those saves are from before versions).
    #[serde(default)]
    pub version: u32,
    /// The game version that wrote the save (`Cargo.toml`'s version).
    #[serde(default)]
    pub game: String,
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
    /// A slow average of the same rate (about ten minutes), so one jam just before
    /// quitting doesn't zero a whole night away.
    #[serde(default)]
    pub rate_long: (f64, f64),
    #[serde(skip)]
    pub samples: VecDeque<(f64, f64)>,
    /// Money from company deliveries this session; left out of the rate, since
    /// deliveries don't keep coming while the game is closed.
    #[serde(skip)]
    pub gifted: f64,
    /// Recent research sales per second, per item: research earned while away fades
    /// item by item, just as it does in play.
    #[serde(default)]
    pub sale_rate: PerItem<f64>,
    #[serde(skip)]
    pub sale_samples: VecDeque<[u64; ITEMS]>,
    /// The last minute of production, one sample a second, for the production panel.
    #[serde(skip)]
    pub flow_samples: VecDeque<FlowSample>,
    /// Money and research earned per minute, hour and day, for the History tab.
    #[serde(default)]
    pub history: crate::history::History,
    /// Agent messages waiting to be read.
    #[serde(default)]
    pub mail: crate::mail::Queue,
    /// Every telegram and letter received, oldest first, with whether it was read.
    /// Kept in the save, so a telegram skipped by accident can be opened again.
    #[serde(default)]
    pub log: VecDeque<crate::mail::LogEntry>,
    /// How many messages have been delivered (the next one's id).
    #[serde(default)]
    pub mail_seq: u64,
    /// When the last company delivery came with a message, and how many have come.
    #[serde(default)]
    pub last_gift: u64,
    #[serde(default)]
    pub gifts: u32,
    #[serde(skip)]
    pub grid: Grid,
    #[serde(skip)]
    pub fluids: Fluids,
    /// The fluid in each pipe network, as saved. Only filled in while saving and applied on load;
    /// during play `fluids` is the truth.
    #[serde(default)]
    pub pipe_fluid: Vec<PipeFill>,
    #[serde(skip)]
    pub active: Vec<Vec<u32>>,
}

/// Samples (one a second) needed before the earning rate is trusted.
const RATE_MIN_SAMPLES: usize = 30;
/// Time constant, in seconds, of the slow earning average used for offline pay.
const RATE_LONG_SECS: f64 = 600.0;

/// Seconds between company deliveries that come with agent messages.
pub const GIFT_GAP: u64 = 180;
/// Most agent messages kept waiting to be read.
pub const MAIL_KEEP: usize = 8;
/// Most telegrams and letters kept in the log (read ones go first when it is full).
pub const LOG_KEEP: usize = 50;

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

/// Every tile a building `w` wide and `h` tall covers when its top-left is at (x, y).
pub fn footprint(x: i32, y: i32, w: i32, h: i32) -> impl Iterator<Item = (i32, i32)> {
    (0..h).flat_map(move |dy| (0..w).map(move |dx| (x + dx, y + dy)))
}

/// Footprint of a part facing `dir`; `s` is the side of an elevator (ignored for other parts).
pub fn dims_of(kind: Kind, dir: Dir, s: i32) -> (i32, i32) {
    if kind == Kind::Elevator {
        (s, s)
    } else {
        kind.dims(dir)
    }
}

/// How many tiles long a building's edge is on side `d`.
pub fn edge_len((w, h): (i32, i32), d: Dir) -> i32 {
    match d {
        Dir::E | Dir::W => h,
        Dir::N | Dir::S => w,
    }
}

/// The j-th tile just outside a building's edge on side `dir`.
fn front_tile(ax: i32, ay: i32, (w, h): (i32, i32), dir: Dir, j: i32) -> (i32, i32) {
    match dir {
        Dir::E => (ax + w, ay + j),
        Dir::W => (ax - 1, ay + j),
        Dir::S => (ax + j, ay + h),
        Dir::N => (ax + j, ay - 1),
    }
}

impl World {
    /// A new campaign: the starting map, $1000 and the two Copper Works companies.
    pub fn new() -> Self {
        let mut w = World {
            version: SAVE_VERSION,
            game: GAME_VERSION.to_string(),
            floors: gen_floors(),
            econ: Econ { money: START_MONEY, ..Default::default() },
            progress: Progress::default(),
            tick: 0,
            lifts: Vec::new(),
            saved_at: 0,
            rate: (0.0, 0.0),
            rate_long: (0.0, 0.0),
            gifted: 0.0,
            samples: VecDeque::new(),
            sale_rate: PerItem::default(),
            sale_samples: VecDeque::new(),
            flow_samples: VecDeque::new(),
            history: Default::default(),
            mail: VecDeque::new(),
            log: VecDeque::new(),
            mail_seq: 0,
            last_gift: 0,
            gifts: 0,
            grid: Grid::default(),
            fluids: Fluids::default(),
            pipe_fluid: Vec::new(),
            active: Vec::new(),
        };
        w.rebuild_grid();
        w
    }

    /// Empty, fully dug world with lots of money and everything unlocked, for tests.
    #[cfg(test)]
    pub fn blank() -> Self {
        let mut w = World {
            version: SAVE_VERSION,
            game: GAME_VERSION.to_string(),
            floors: LEVELS.iter().map(|&l| Floor::new(l, true)).collect(),
            econ: Econ { money: 1e6, ..Default::default() },
            progress: Progress { opened: [true; COMPANIES], researched: [true; TECHS], notes: VecDeque::new() },
            tick: 0,
            lifts: Vec::new(),
            saved_at: 0,
            rate: (0.0, 0.0),
            rate_long: (0.0, 0.0),
            gifted: 0.0,
            samples: VecDeque::new(),
            sale_rate: PerItem::default(),
            sale_samples: VecDeque::new(),
            flow_samples: VecDeque::new(),
            history: Default::default(),
            mail: VecDeque::new(),
            log: VecDeque::new(),
            mail_seq: 0,
            last_gift: 0,
            gifts: 0,
            grid: Grid::default(),
            fluids: Fluids::default(),
            pipe_fluid: Vec::new(),
            active: Vec::new(),
        };
        w.rebuild_grid();
        w
    }

    pub fn save(&mut self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        self.saved_at = now_secs();
        self.game = GAME_VERSION.to_string();
        self.stamp_recipe_names();
        self.pipe_fluid = self.pipe_fills();
        let json = serde_json::to_string(self);
        self.pipe_fluid = Vec::new();
        let json = json.map_err(std::io::Error::other)?;
        // Write a temp file next to the save, flush it to disk, then swap it in, so a crash
        // mid-write never leaves a half-written save behind.
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        {
            use std::io::Write;
            let mut file = std::fs::File::create(&tmp)?;
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
        }
        std::fs::rename(&tmp, path)
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
        w.resolve_recipes();
        w.rebuild_grid();
        w.restore_pipe_fluid();
        // A company that opens from the start is open, whatever the save says (it may be new).
        for c in Company::ALL {
            if c.key_items().is_empty() {
                w.progress.opened[c.idx()] = true;
            }
        }
        Ok(w)
    }

    /// Writes each machine's recipe name beside its index, for the save.
    fn stamp_recipe_names(&mut self) {
        for b in self.floors.iter_mut().flat_map(|fl| fl.tiles.iter_mut().flatten()) {
            let list = b.kind.recipes();
            if let St::Crafter { recipe, recipe_name, .. } = &mut b.st {
                *recipe_name = list.get(*recipe as usize).map_or_else(String::new, |r| r.name.to_string());
            }
        }
    }

    /// Turns each saved recipe name back into an index. A name the machine no longer has (a recipe
    /// that was renamed or removed) falls back to its first recipe, with a note for the player.
    fn resolve_recipes(&mut self) {
        let mut lost = Vec::new();
        for (f, fl) in self.floors.iter_mut().enumerate() {
            for (i, b) in fl.tiles.iter_mut().enumerate() {
                let Some(b) = b else { continue };
                let list = b.kind.recipes();
                if let St::Crafter { recipe, recipe_name, .. } = &mut b.st {
                    match list.iter().position(|r| r.name == recipe_name.as_str()) {
                        Some(n) => *recipe = n as u8,
                        None => {
                            *recipe = 0;
                            let (x, y) = pos_of(i);
                            lost.push(format!("The {} at {x},{y} on floor {} lost its recipe '{recipe_name}'.", b.kind.name(), f));
                        }
                    }
                    recipe_name.clear();
                }
            }
        }
        self.progress.notes.extend(lost);
    }

    /// The fluid in each pipe network, one entry per network that holds any.
    fn pipe_fills(&self) -> Vec<PipeFill> {
        let mut out = Vec::new();
        for (f, fl) in self.floors.iter().enumerate() {
            let Some(ids) = self.fluids.pipe_pool.get(f) else { continue };
            let mut seen = vec![false; self.fluids.pools.len()];
            for (i, &id) in ids.iter().enumerate() {
                if id < 0 || std::mem::replace(&mut seen[id as usize], true) {
                    continue;
                }
                let pool = &self.fluids.pools[id as usize];
                if let Some(fluid) = pool.fluid.filter(|_| pool.amount > 0.0) {
                    let (x, y) = pos_of(i);
                    out.push(PipeFill { level: fl.level, x, y, fluid, amount: pool.amount });
                }
            }
        }
        out
    }

    /// Puts the saved fluid back into the pipe networks (after they are worked out).
    fn restore_pipe_fluid(&mut self) {
        for p in std::mem::take(&mut self.pipe_fluid) {
            let Some(f) = self.floor_of(p.level) else { continue };
            if !in_bounds(p.x, p.y) {
                continue;
            }
            let id = self.fluids.pipe_pool[f][idx(p.x, p.y)];
            if id >= 0 {
                self.fluids.pools[id as usize].add(p.fluid, p.amount);
            }
        }
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
        floor_hint_text(level)
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
            let item = keys_text(t.requires());
            return Err(format!("Make {item} first to unlock {}", t.name()));
        }
        if self.econ.research + 1e-9 < t.cost() {
            return Err(format!("Need {:.0} research (you have {:.1})", t.cost(), self.econ.research));
        }
        self.econ.research -= t.cost();
        self.progress.researched[t.idx()] = true;
        self.add_new_floor_doors();
        Ok(())
    }

    /// When a floor opens, every elevator gets a free unload door there, so the player
    /// never has to rebuild one. A door whose spot is taken is skipped, with a note.
    fn add_new_floor_doors(&mut self) {
        let mut added = false;
        for k in 0..self.lifts.len() {
            let anchor = self.lifts[k].anchor;
            let Some(door) = self.floors.iter().find_map(|fl| fl.tiles[anchor].clone()) else {
                continue;
            };
            let (x, y) = pos_of(anchor);
            let (w, h) = door.dims();
            for g in 0..self.floors.len() {
                if !self.floor_open(g) || self.floors[g].tiles[anchor].is_some() {
                    continue;
                }
                if footprint(x, y, w, h).any(|(tx, ty)| self.floors[g].owner[idx(tx, ty)].is_some()) {
                    let level = self.floors[g].level;
                    self.progress.notes.push_back(format!(
                        "An elevator's shaft is built over on floor {level:+}: clear it and rebuild the elevator for a door there"
                    ));
                    continue;
                }
                let mut d = door.clone();
                if let St::Elevator { mode, inbox, outbox } = &mut d.st {
                    *mode = DoorMode::Unload;
                    inbox.clear();
                    outbox.clear();
                }
                self.put(g, x, y, d);
                added = true;
            }
        }
        if added {
            self.rebuild_grid();
        }
    }

    /// Checks a placement facing east (top-left at x, y) and returns its full cost (part + any digging).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn check_place(&self, f: usize, x: i32, y: i32, kind: Kind) -> Result<f64, String> {
        self.check_place_sized(f, x, y, kind, Dir::E, 0)
    }

    /// Like `check_place`, facing `dir`; `s` is the side of an elevator (ignored for other parts).
    pub fn check_place_sized(&self, f: usize, x: i32, y: i32, kind: Kind, dir: Dir, s: i32) -> Result<f64, String> {
        self.place_cost_money(f, x, y, kind, dir, s).map_err(|e| e.to_string())
    }

    /// `place_cost` and then the money test: the whole check for one placement.
    pub(crate) fn place_cost_money(&self, f: usize, x: i32, y: i32, kind: Kind, dir: Dir, s: i32) -> Result<f64, PlaceErr> {
        let cost = self.place_cost(f, x, y, kind, dir, s)?;
        if self.econ.money + 1e-9 < cost {
            return Err(PlaceErr::Money(cost));
        }
        Ok(cost)
    }

    /// Everything `check_place_sized` checks except the player's money: the part is unlocked,
    /// the floor is open, the footprint is on the map and free on every floor it touches, and the
    /// ground suits it. Returns the full cost (part plus digging or decking), which a batch adds
    /// up against the money it has left.
    pub(crate) fn place_cost(&self, f: usize, x: i32, y: i32, kind: Kind, dir: Dir, s: i32) -> Result<f64, PlaceErr> {
        if !self.progress.part_unlocked(kind) {
            return Err(PlaceErr::Locked(kind));
        }
        if !self.floor_open(f) {
            return Err(PlaceErr::FloorClosed(self.floors[f].level));
        }
        // Elevators reach every open floor; everything else sits on this one.
        let floors: Vec<usize> =
            if kind == Kind::Elevator { (0..self.floors.len()).filter(|&g| self.floor_open(g)).collect() } else { vec![f] };
        if kind == Kind::Elevator && floors.len() < 2 {
            return Err(PlaceErr::NeedsSecondFloor);
        }
        let (w, h) = dims_of(kind, dir, s);
        let mut cost = kind.price(w);
        for &g in &floors {
            let fl = &self.floors[g];
            for (tx, ty) in footprint(x, y, w, h) {
                if !in_bounds(tx, ty) {
                    return Err(PlaceErr::OffMap);
                }
                let i = idx(tx, ty);
                if fl.owner[i].is_some() {
                    return Err(if g == f { PlaceErr::Blocked } else { PlaceErr::OtherFloorBlocked });
                }
                if !fl.dug[i] {
                    cost += tile_cost(fl.level);
                }
            }
        }
        if (kind.is_drill() || kind.is_pump()) && self.floors[f].level > 0 {
            return Err(PlaceErr::NotAboveGround(kind));
        }
        if kind.is_solar() && self.floors[f].level < 0 {
            return Err(PlaceErr::NeedsSun);
        }
        Ok(cost)
    }

    fn put(&mut self, f: usize, x: i32, y: i32, b: Building) {
        let a = idx(x, y);
        let fl = &mut self.floors[f];
        let (w, h) = b.dims();
        for (tx, ty) in footprint(x, y, w, h) {
            let i = idx(tx, ty);
            fl.dug[i] = true;
            fl.owner[i] = Some(a as u32);
        }
        fl.tiles[a] = Some(b);
    }

    pub fn place(&mut self, f: usize, x: i32, y: i32, kind: Kind, dir: Dir) -> Result<(), String> {
        self.place_sized(f, x, y, kind, dir, 0)
    }

    pub fn place_sized(&mut self, f: usize, x: i32, y: i32, kind: Kind, dir: Dir, s: i32) -> Result<(), String> {
        self.place_no_rebuild(f, x, y, kind, dir, s).map_err(|e| e.to_string())?;
        self.rebuild_grid();
        Ok(())
    }

    /// Places a part and pays for it, but leaves the power grids and pipe networks as they were:
    /// a batch places many parts this way and then calls `rebuild_grid` once.
    pub(crate) fn place_no_rebuild(&mut self, f: usize, x: i32, y: i32, kind: Kind, dir: Dir, s: i32) -> Result<(), PlaceErr> {
        let s = s.clamp(MIN_LIFT, MAX_LIFT);
        let cost = self.place_cost_money(f, x, y, kind, dir, s)?;
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
        Ok(())
    }

    fn take_building(&mut self, f: usize, a: usize) -> Option<Building> {
        let fl = &mut self.floors[f];
        let b = fl.tiles[a].take()?;
        let (x, y) = pos_of(a);
        let (w, h) = b.dims();
        for (tx, ty) in footprint(x, y, w, h) {
            fl.owner[idx(tx, ty)] = None;
        }
        Some(b)
    }

    /// How much waste the part covering (x, y) holds: crates, warehouses, belts, tunnels,
    /// splitters, arms, drills and machines, and an elevator (its car and its doors on every floor).
    pub fn waste_inside(&self, f: usize, x: i32, y: i32) -> usize {
        let Some(a) = self.floors[f].anchor_at(x, y) else {
            return 0;
        };
        let count = |it: &mut dyn Iterator<Item = &Item>| it.filter(|i| **i == Item::Waste).count();
        match self.floors[f].tiles[a].as_ref().map(|b| (&b.st, b.kind)) {
            Some((St::Crate { items }, _)) => count(&mut items.iter()),
            Some((St::Belt { items }, _)) => count(&mut items.iter().map(|b| &b.item)),
            Some((St::Tunnel { items, .. }, _)) => count(&mut items.iter().map(|(i, _)| i)),
            Some((St::Splitter { buf, .. }, _)) => count(&mut buf.iter()),
            Some((St::Arm { hold, .. }, _)) => count(&mut hold.iter()),
            Some((St::Drill { out, .. }, _)) => count(&mut out.iter()),
            Some((St::Crafter { stock, out, .. }, _)) => stock[Item::Waste] as usize + count(&mut out.iter()),
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
        let kind = self.remove_no_rebuild(f, x, y)?;
        self.rebuild_grid();
        Some(kind)
    }

    /// Takes the part covering (x, y) away and refunds its full price, leaving the power grids
    /// and pipe networks as they were (a batch rebuilds once at the end). An elevator takes its
    /// doors on every floor and its car with it. Waste fees are the caller's business.
    pub(crate) fn remove_no_rebuild(&mut self, f: usize, x: i32, y: i32) -> Option<Kind> {
        let a = self.floors[f].anchor_at(x, y)?;
        let b = self.take_building(f, a)?;
        self.econ.money += b.kind.price(b.dims().0);
        if b.kind == Kind::Elevator {
            for g in 0..self.floors.len() {
                if matches!(self.floors[g].tiles[a], Some(Building { kind: Kind::Elevator, .. })) {
                    self.take_building(g, a);
                }
            }
            self.lifts.retain(|c| c.anchor != a);
        }
        Some(b.kind)
    }

    /// Turns the part covering (x, y). A long machine keeps its top-left corner and takes
    /// the turned rectangle, which must be free (new ground is dug, and paid for).
    pub fn set_dir(&mut self, f: usize, x: i32, y: i32, dir: Dir) -> Result<(), String> {
        let a = self.floors[f].anchor_at(x, y).ok_or("Nothing here to turn")?;
        let (kind, old) = match self.floors[f].tiles[a].as_ref() {
            Some(b) if b.kind.directional() => (b.kind, b.dims()),
            _ => return Err("This part has no output side to rotate".into()),
        };
        let (ax, ay) = pos_of(a);
        let (w, h) = if kind == Kind::Elevator { old } else { kind.dims(dir) };
        if (w, h) != old {
            let fl = &self.floors[f];
            let mut cost = 0.0;
            for (tx, ty) in footprint(ax, ay, w, h) {
                if !in_bounds(tx, ty) {
                    return Err("Doesn't fit on the map here".into());
                }
                let i = idx(tx, ty);
                if fl.owner[i].is_some_and(|o| o as usize != a) {
                    return Err("Something is in the way".into());
                }
                if !fl.dug[i] {
                    cost += tile_cost(fl.level);
                }
            }
            if self.econ.money + 1e-9 < cost {
                return Err(format!("Not enough money (need ${:.0})", cost));
            }
            self.econ.money -= cost;
            let fl = &mut self.floors[f];
            for (tx, ty) in footprint(ax, ay, old.0, old.1) {
                fl.owner[idx(tx, ty)] = None;
            }
            for (tx, ty) in footprint(ax, ay, w, h) {
                fl.dug[idx(tx, ty)] = true;
                fl.owner[idx(tx, ty)] = Some(a as u32);
            }
        }
        if let Some(b) = self.floors[f].tiles[a].as_mut() {
            b.dir = dir;
        }
        if (w, h) != old {
            self.rebuild_grid();
        }
        Ok(())
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
        let Some(a) = self.floors[f].anchor_at(x, y) else {
            return;
        };
        let progress = &self.progress;
        let Some(b) = self.floors[f].tiles[a].as_mut() else {
            return;
        };
        let kind = b.kind;
        match (&mut b.st, set) {
            (St::Arm { filter, .. }, Setting::Filter(v)) => *filter = v,
            (St::Splitter { filter, .. }, Setting::Filter(v)) if kind == Kind::Sorter => *filter = v,
            (St::Trade { mode, .. }, Setting::Sell(v)) => *mode = v,
            (St::Drill { mines, progress, .. }, Setting::Mines(m)) if kind.ores().contains(&m) => {
                *mines = Some(m);
                *progress = 0.0;
            }
            (St::Crafter { recipe, fluid, fluid_out, .. }, Setting::Recipe(v))
                if kind.recipes().get(v as usize).is_some_and(|r| r.needs.is_none_or(|t| progress.has(t))) =>
            {
                *recipe = v;
                *fluid = 0;
                *fluid_out = 0;
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
                *progress = 0.0;
                let note = if kind == Kind::Drill && next == Item::IronOre { " · half the speed of a basic drill" } else { "" };
                Some(format!("{} now mines {}{note}", kind.name(), next.name()))
            }
            St::Trade { mode, .. } => {
                *mode = mode.next();
                Some(format!("Trade post: {}", mode.label()))
            }
            St::Crafter { recipe, progress: p, fluid, fluid_out, .. } => {
                let list = kind.recipes();
                let usable = |r: &Recipe| r.needs.is_none_or(|t| progress.has(t));
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
                // Fluid held for the old recipe is let go.
                *fluid = 0;
                *fluid_out = 0;
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
        #[cfg(test)]
        REBUILDS.with(|c| c.set(c.get() + 1));
        // Every power source, with the floor it's on and the tiles it reaches.
        let mut nodes = Vec::new();
        for (f, fl) in self.floors.iter().enumerate() {
            for (i, b) in fl.tiles.iter().enumerate() {
                if let Some(b) = b {
                    let (x, y) = pos_of(i);
                    if let Some(r) = coverage(b.kind, b.dims(), x, y) {
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
                let both_solar = ka.is_solar() && kb.is_solar();
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
        for (a, net) in node_net.iter_mut().enumerate() {
            let r = find(&mut parent, a);
            if net_of_root[r] == usize::MAX {
                net_of_root[r] = nets.len();
                nets.push(Net::default());
            }
            *net = net_of_root[r];
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
            if kind.is_solar() && self.floors[f].level >= 0 {
                nets[id].solar += kind.solar_output();
            }
        }
        let engines = nodes.iter().enumerate().filter(|(_, n)| n.3 == Kind::SteamEngine).map(|(k, n)| (n.0, n.1, node_net[k])).collect();
        let mut unpowered = 0;
        let mut bursts = Vec::new();
        for (f, fl) in self.floors.iter().enumerate() {
            for (i, b) in fl.tiles.iter().enumerate() {
                let Some(b) = b else { continue };
                if b.kind.electric() {
                    let (x, y) = pos_of(i);
                    match grid_of(&cover[f], x, y, b.dims()) {
                        -1 => unpowered += 1,
                        id => {
                            nets[id as usize].steady += b.kind.power_draw();
                            if b.kind.burst_draw() > 0.0 {
                                bursts.push((f, i, id as usize));
                            }
                        }
                    }
                }
            }
        }
        let sources = nodes.iter().enumerate().map(|(k, n)| (n.0, n.1, node_net[k])).collect();
        self.grid = Grid { cover, nets, unpowered, engines, sources, bursts };
        self.active = self
            .floors
            .iter()
            .map(|fl| fl.tiles.iter().enumerate().filter(|(_, b)| b.is_some()).map(|(i, _)| i as u32).collect())
            .collect();
        self.update_power(false);
        self.rebuild_fluids();
    }

    /// Works out each grid's supply this tick. Engines give as much as their grid needs
    /// beyond solar, up to what their steam allows, and burn steam only for what they give.
    fn update_power(&mut self, burn: bool) {
        let Grid { nets, engines, bursts, .. } = &mut self.grid;
        for n in nets.iter_mut() {
            n.engine = 0.0;
            n.demand = n.steady;
        }
        // Machines that draw only while working add their draw while they are.
        for &(f, a, id) in bursts.iter() {
            if let Some(Building { kind, st: St::Crafter { busy: true, .. }, .. }) = &self.floors[f].tiles[a] {
                nets[id].demand += kind.burst_draw();
            }
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
            n.ratio = if n.demand > 0.0 {
                (n.supply / n.demand).min(1.0)
            } else if n.supply > 0.0 {
                1.0
            } else {
                // Nothing to give: a machine that draws only while working can't start.
                0.0
            };
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
                // Old networks this one takes pipes from, and how many of their pipes.
                let mut olds: Vec<(usize, u32)> = Vec::new();
                let mut stack = vec![start];
                pipe_pool[f][start] = id as i32;
                while let Some(i) = stack.pop() {
                    pool.pipes += 1;
                    if let Some(&o) = old.pipe_pool.get(f).and_then(|v| v.get(i)) {
                        if o >= 0 {
                            match olds.iter_mut().find(|(k, _)| *k == o as usize) {
                                Some((_, c)) => *c += 1,
                                None => olds.push((o as usize, 1)),
                            }
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
                // A network cut in two splits its fluid by pipe count, so cutting never makes fluid.
                for (o, c) in olds {
                    if let Some(op) = old.pools.get(o) {
                        if let Some(fl) = op.fluid {
                            let share = c as f64 / op.pipes.max(1) as f64;
                            pool.add(fl, op.amount * share.min(1.0));
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
        let id = grid_of(&self.grid.cover[f], x, y, b.dims());
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
        let id = grid_of(&self.grid.cover[f], ax, ay, b.dims());
        (b.kind.electric() && id >= 0).then_some(id as usize)
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
            for &i in &self.active[f] {
                let i = i as usize;
                let Some(mut b) = tiles[i].take() else {
                    continue;
                };
                let (x, y) = pos_of(i);
                let ratio = match grid_of(cover, x, y, b.dims()) {
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
        if self.tick.is_multiple_of(TPS) {
            self.sample_earnings();
            let now = now_secs();
            self.sample_history(now, crate::history::local_offset(now));
        }
    }

    /// Once a second: adds what was earned since the last sample to the history. Company
    /// deliveries aren't earnings, so they're left out (the offline rate leaves them out too).
    pub fn sample_history(&mut self, now: u64, off: i64) {
        let total = (self.econ.earned_money - self.gifted, self.econ.earned_research);
        self.history.record(now, off, total);
    }

    /// Once a second: remember lifetime earnings, and work out the recent rate.
    fn sample_earnings(&mut self) {
        self.samples.push_back((self.econ.earned_money - self.gifted, self.econ.earned_research));
        while self.samples.len() > 120 {
            self.samples.pop_front();
        }
        // Wait for half a minute of samples, so a short session can't replace a good saved rate.
        if self.samples.len() > RATE_MIN_SAMPLES {
            if let (Some(a), Some(b)) = (self.samples.front(), self.samples.back()) {
                let n = (self.samples.len() - 1) as f64;
                self.rate = ((b.0 - a.0) / n, (b.1 - a.1) / n);
                let k = 1.0 / RATE_LONG_SECS;
                let l = &mut self.rate_long;
                if *l == (0.0, 0.0) {
                    *l = self.rate;
                } else {
                    *l = (l.0 + (self.rate.0 - l.0) * k, l.1 + (self.rate.1 - l.1) * k);
                }
            }
        }
        let e = &self.econ;
        self.flow_samples.push_back(FlowSample {
            made: e.made.to_array(),
            used: e.used,
            sold: e.sold.to_array(),
            money: e.money_from,
            research: e.research_from,
        });
        while self.flow_samples.len() > 61 {
            self.flow_samples.pop_front();
        }
        if let Some(a) = self.flow_samples.front() {
            let earned = Item::ALL.iter().filter(|i| **i != Item::Waste).map(|i| a.money[i.idx()]).sum();
            self.econ.minute_ago = (earned, -a.money[Item::Waste.idx()]);
        }
        self.sale_samples.push_back(self.econ.research_sales.to_array());
        while self.sale_samples.len() > 120 {
            self.sale_samples.pop_front();
        }
        if let (Some(a), Some(b)) = (self.sale_samples.front(), self.sale_samples.back()) {
            let n = (self.sale_samples.len() - 1) as f64;
            if n > 0.0 {
                for i in 0..ITEMS {
                    self.sale_rate[i] = (b[i] - a[i]) as f64 / n;
                }
            }
        }
    }

    /// Per item, over the last minute (or as much of it as has passed): made, used and
    /// sold per minute, and the money and research its sales brought per minute.
    /// Only items with something going on. The second value is how many seconds it covers.
    pub fn flow(&self) -> (Vec<Flow>, u64) {
        let (Some(a), Some(b)) = (self.flow_samples.front(), self.flow_samples.back()) else {
            return (Vec::new(), 0);
        };
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
        let m = (self.rate.0.max(self.rate_long.0) * away as f64).max(0.0);
        let r = self.offline_research(away as f64);
        if m <= 0.0 && r <= 0.0 {
            return None;
        }
        self.econ.money += m;
        self.econ.research += r;
        self.econ.earned_money += m;
        self.econ.earned_research += r;
        self.history.record_offline(now, crate::history::local_offset(now), m, r);
        Some((away, m, r))
    }

    /// Research earned while away: each item keeps selling at its recent pace, and its
    /// research fades the way it does in play. Moves each item along its curve.
    fn offline_research(&mut self, secs: f64) -> f64 {
        if self.sale_rate.iter().all(|r| *r == 0.0) {
            // No per-item rates (a new game, or too soon to tell): count ten minutes at the last rate.
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
            if !open.is_empty() {
                let c = open[self.gifts as usize % open.len()];
                let m = (self.rate.0 * 60.0).max(25.0).round();
                self.econ.money += m;
                self.econ.earned_money += m;
                self.gifted += m;
                self.last_gift = now;
                self.gifts += 1;
                msg.gift = Some((c, m));
            }
        }
        self.deliver(msg);
    }

    /// The telegram a brand-new world starts with, from the two starting companies. It
    /// states how things work and gives no orders; there is no delivery with it.
    pub fn welcome(&mut self, now: u64) {
        let md = "Hollowmere here. The ground is yours, so dig anywhere. Drills and furnaces burn coal.\n\n\
                  Brannock here, all three of us. Molders and presses want EC: a solar panel and a pole will do. \
                  A trade post sells whatever reaches it, and **f** picks money or research.\n\n\
                  **1** builds, **t** is research, **b** is recipes, **?** is help. No rush.";
        self.deliver(crate::mail::Message {
            from: "Hollowmere & Brannock".into(),
            event: String::new(),
            md: md.into(),
            at: now,
            gift: None,
            id: 0,
        });
    }

    /// Puts a message in the waiting queue and in the log (unread).
    pub fn deliver(&mut self, mut msg: crate::mail::Message) {
        self.mail_seq += 1;
        msg.id = self.mail_seq;
        self.log.push_back(crate::mail::LogEntry { msg: msg.clone(), read: false });
        while self.log.len() > LOG_KEEP {
            let at = self.log.iter().position(|e| e.read).unwrap_or(0);
            self.log.remove(at);
        }
        self.mail.push_back(msg);
        // Only the newest few wait, so a few days away doesn't mean a pile of telegrams.
        // (The log keeps the rest.)
        while self.mail.len() > MAIL_KEEP {
            self.mail.pop_front();
        }
    }

    /// Marks a logged message as read.
    pub fn mark_read(&mut self, id: u64) {
        if let Some(e) = self.log.iter_mut().find(|e| e.msg.id == id) {
            e.read = true;
        }
    }

    /// How many logged messages are still unread.
    pub fn unread(&self) -> usize {
        self.log.iter().filter(|e| !e.read).count()
    }

    /// Making a company's key item for the first time opens that company, and making
    /// enough of a milestone item unlocks its tech for free.
    fn check_discoveries(&mut self) {
        for c in Company::ALL {
            if self.progress.is_open(c) {
                continue;
            }
            if let Some(key) = first_key_made(c.key_items(), |m| self.econ.made_of(m)) {
                self.progress.opened[c.idx()] = true;
                self.deliver(crate::mail::letter(c, key, now_secs()));
            }
        }
        for t in Tech::ALL {
            let (item, n) = t.milestone();
            if !self.progress.has(t) && self.progress.is_open(t.company()) && self.econ.made[item.idx()] >= n {
                self.progress.researched[t.idx()] = true;
                self.progress.notes.push_back(format!("{} unlocked for free: you've made {} {}", t.name(), n, item.name()));
                if matches!(t, Tech::SupportBeams | Tech::Excavation) {
                    self.add_new_floor_doors();
                }
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
                doors.iter().filter(|d| d.2 == want && d.0 != from).min_by_key(|d| ((d.1 - level(from)).abs(), d.0)).map(|d| d.0)
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
                            let Some(it) = car.load.pop_front() else {
                                break;
                            };
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

    /// A full building says `word` (FULL for the front, BACK for waste), or EC? when everything
    /// along that port's edge is a belt with no power, since that's the real reason it can't
    /// get rid of its output.
    fn full_word(&self, f: usize, a: usize, port: Port, word: &'static str) -> &'static str {
        let Some(b) = self.floors[f].tiles[a].as_ref() else {
            return word;
        };
        let (ax, ay) = ((a % W as usize) as i32, (a / W as usize) as i32);
        let dims = b.dims();
        let edge = port.edge(b.dir);
        let mut belts = 0;
        for j in 0..edge_len(dims, edge) {
            let (tx, ty) = front_tile(ax, ay, dims, edge, j);
            if !in_bounds(tx, ty) {
                continue;
            }
            let Some(o) = self.floors[f].owner[idx(tx, ty)] else {
                continue;
            };
            match self.floors[f].tiles[o as usize].as_ref() {
                Some(nb) if nb.kind.is_belt() => {
                    if self.building_ratio(f, o as usize) > 0.0 {
                        return word;
                    }
                    belts += 1;
                }
                Some(_) => return word,
                None => {}
            }
        }
        if belts > 0 {
            "EC?"
        } else {
            word
        }
    }

    /// True when nothing at all stands along the building's output edge: no belt and no
    /// machine to take what it makes. (A full building with something there is just backed up.)
    pub fn front_open(&self, f: usize, a: usize) -> bool {
        let Some(b) = self.floors[f].tiles[a].as_ref() else {
            return false;
        };
        let (ax, ay) = ((a % W as usize) as i32, (a / W as usize) as i32);
        let dims = b.dims();
        let edge = Port::Front.edge(b.dir);
        (0..edge_len(dims, edge)).all(|j| {
            let (tx, ty) = front_tile(ax, ay, dims, edge, j);
            !in_bounds(tx, ty) || self.floors[f].owner[idx(tx, ty)].is_none()
        })
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
                } else if out.len() >= b.kind.drill_out_cap() {
                    Some(self.full_word(f, a, Port::Front, "FULL"))
                } else if b.kind.burns_coal() && *fuel == 0 && *coal == 0 && *mines != Some(Item::Coal) {
                    Some("COAL")
                } else if !b.kind.burns_coal() && !powered {
                    Some("EC?")
                } else {
                    None
                }
            }
            St::Crafter { busy: false, stock, fuel, out, fluid, fluid_out, .. } => {
                let r = b.recipe()?;
                let batch = b.kind.batch();
                let fluid_full = r.fluid_out.is_some_and(|(_, n)| *fluid_out + n * batch > fluid_out_cap(n * batch));
                let (products_fit, waste_fits) = outputs_fit(r, batch, out, b.kind.out_cap());
                if !products_fit || fluid_full {
                    Some(self.full_word(f, a, Port::Front, "FULL"))
                } else if !waste_fits {
                    // Waste leaves by the back: nothing there takes it.
                    Some(self.full_word(f, a, Port::Back, "BACK"))
                } else if !r.inputs.iter().all(|(it, n)| stock[it.idx()] >= *n * batch) {
                    // A kiln is still loading its batch.
                    Some(if batch > 1 { "LOAD" } else { "NEED" })
                } else if r.fluid_in.is_some_and(|(_, n)| *fluid < n * batch) {
                    Some("FLUD")
                } else if let Some(fl) = r.fuel.filter(|fl| *fuel == 0 && stock[fl.idx()] == 0) {
                    Some(if fl == Item::Coke { "COKE" } else { "COAL" })
                } else if b.kind.electric() && !powered {
                    Some("EC?")
                } else {
                    None
                }
            }
            St::Boiler { busy: false, fuel, coal, .. } => Some(if *fuel == 0 && *coal == 0 { "COAL" } else { "WATR" }),
            St::Engine { steam, .. } if *steam <= 0.0 => Some("STM?"),
            St::Pump { busy: false } => Some(if powered { "FULL" } else { "EC?" }),
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
        // The press comes free with the demo.
        self.progress.researched[Tech::Press.idx()] = true;
        // Copper drill -> furnace -> molder -> press, each pushing straight into the next
        // (ore, molten copper, ingots); plates ride a belt east to the trade post.
        self.place(f, 51, 37, Drill, Dir::E)?;
        self.place(f, 53, 36, Furnace, Dir::E)?; // covers 53-55, 36-38
        self.place(f, 56, 37, Molder, Dir::E)?;
        self.place(f, 58, 37, Press, Dir::E)?; // pushes plates straight into the trade post
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
        // Two solar panels, and poles carrying their power to the molder, press and every belt.
        self.place(f, 56, 33, Solar, Dir::E)?;
        self.place(f, 58, 33, Solar, Dir::E)?;
        self.place(f, 57, 36, Pole, Dir::E)?;
        self.place(f, 57, 40, Pole, Dir::E)?;
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
fn pools_around(pipe_pool: &[i32], ax: i32, ay: i32, dims: (i32, i32), skip: Option<Dir>) -> Vec<usize> {
    let mut out = Vec::new();
    for d in Dir::ALL {
        if Some(d) == skip {
            continue;
        }
        for j in 0..edge_len(dims, d) {
            let (tx, ty) = front_tile(ax, ay, dims, d, j);
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
    let dims = b.dims();
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
            let (base, per_coal) = kind.drill_speed(item);
            // Work done per tick, and fuel burned per tick (see `drill_rates`).
            let (mult, burn) = kind.drill_rates();
            let ticks = ((base as f64) / richness(level)).round().max(1.0);
            let per_fuel = ticks as u32 * per_coal;
            let cap = kind.drill_out_cap();
            // Refuel before the fire runs short (any leftover carries over, so big drills burn evenly).
            if *fuel < burn && *coal > 0 {
                *coal -= 1;
                econ.used[Item::Coal.idx()] += 1;
                *fuel += per_fuel;
            }
            // A drill on coal with nothing to burn fuels itself from what it mines.
            let mut self_fuel = burn > 0 && *fuel == 0 && item == Item::Coal;
            // Electric drills slow down with the grid.
            let speed = if burn == 0 {
                ratio
            } else if *fuel > 0 || self_fuel {
                1.0
            } else {
                0.0
            };
            if speed > 0.0 && out.len() < cap {
                *fuel = fuel.saturating_sub(burn);
                *progress += mult as f64 * speed;
                // A fast drill can finish several items in one tick.
                while *progress >= ticks && out.len() < cap {
                    *progress -= ticks;
                    if self_fuel {
                        *fuel = per_fuel;
                        self_fuel = false;
                    } else {
                        out.push_back(item);
                        econ.made[item.idx()] += 1;
                    }
                }
            }
            // Big drills push several items a tick, round-robin along their front edge.
            for _ in 0..(mult / 4).max(1) {
                if !push_front(tiles, owner, econ, x, y, dims, dir, out, rr) {
                    break;
                }
            }
        }
        St::Crafter { recipe, stock, fuel, progress, busy, out, fluid, fluid_out, .. } => {
            let r = &kind.recipes()[*recipe as usize];
            let batch = kind.batch();
            *busy = false;
            // A fluid input comes from pipes on any side but the front.
            if let Some((fl, n)) = r.fluid_in {
                for p in pools_around(pipe_pool, x, y, dims, Some(dir)) {
                    while *fluid < fluid_cap(n * batch) && pools[p].fluid == Some(fl) && pools[p].amount >= 1.0 {
                        pools[p].take(fl, 1.0);
                        *fluid += 1;
                    }
                }
            }
            // A batch machine (the kiln) waits until it holds the inputs for every run, then fires them all.
            let have = r.inputs.iter().all(|(it, n)| stock[it.idx()] >= *n * batch) && r.fluid_in.is_none_or(|(_, n)| *fluid >= n * batch);
            // Every output needs room: products (out the front), waste (out the back) and fluid.
            let (products_fit, waste_fits) = outputs_fit(r, batch, out, kind.out_cap());
            let room = products_fit && waste_fits && r.fluid_out.is_none_or(|(_, n)| *fluid_out + n * batch <= fluid_out_cap(n * batch));
            if have && room {
                let speed = match r.fuel {
                    Some(f) => {
                        if *fuel == 0 && stock[f.idx()] > 0 {
                            stock[f.idx()] -= 1;
                            econ.used[f.idx()] += 1;
                            *fuel = f.burn_ticks().unwrap_or(0) / kind.fires();
                        }
                        if *fuel > 0 {
                            *fuel -= 1;
                            1.0
                        } else {
                            0.0
                        }
                    }
                    None if kind.electric() => ratio,
                    None => 1.0,
                };
                if speed > 0.0 {
                    *busy = true;
                    *progress += speed;
                    if *progress >= r.ticks as f64 {
                        *progress -= r.ticks as f64;
                        for (it, n) in r.inputs {
                            stock[it.idx()] -= n * batch;
                            econ.used[it.idx()] += (*n * batch) as u64;
                        }
                        if let Some((_, n)) = r.fluid_in {
                            *fluid -= n * batch;
                        }
                        if let Some((fl, n)) = r.fluid_out {
                            *fluid_out += n * batch;
                            econ.made_fluid[fl] += (n * batch) as u64;
                        }
                        for (it, n) in r.outputs {
                            for _ in 0..*n * batch {
                                out.push_back(*it);
                                econ.made[it.idx()] += 1;
                            }
                        }
                    }
                }
            }
            if let (Some((fl, _)), true) = (r.fluid_out, *fluid_out > 0) {
                *fluid_out -= push_fluid(tiles, owner, pipe_pool, pools, x, y, dims, dir, fl, *fluid_out);
            }
            push_port(tiles, owner, econ, x, y, dims, dir, Port::Front, out, rr);
            push_port(tiles, owner, econ, x, y, dims, dir, Port::Back, out, rr);
        }
        St::Pump { busy } => {
            *busy = false;
            if ratio > 0.0 {
                for p in pools_around(pipe_pool, x, y, dims, None) {
                    if pools[p].add(Fluid::Water, kind.pump_rate() * ratio / TPS as f64) > 0.0 {
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
            let source = pools_around(pipe_pool, x, y, dims, Some(dir))
                .into_iter()
                .find(|&p| pools[p].fluid == Some(Fluid::Water) && pools[p].amount >= step);
            if *fuel > 0 {
                if let Some(src) = source {
                    // Steam goes out the front: straight into an engine, or into a pipe network.
                    let mut delivered = false;
                    for j in 0..edge_len(dims, dir) {
                        let (tx, ty) = front_tile(x, y, dims, dir, j);
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
            for p in pools_around(pipe_pool, x, y, dims, None) {
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
            push_front(tiles, owner, econ, x, y, dims, dir, items, rr);
        }
        St::Tunnel { exit: false, items } => {
            // Items travel under at belt speed, keeping their spacing, and come up out of
            // the exit's front. With no exit ahead they wait.
            let Some(k) = tunnel_partner(tiles, x, y, dir) else {
                return;
            };
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
            push_front(tiles, owner, econ, x, y, dims, dir, outbox, rr);
        }
        St::Trade { stuck, .. } => *stuck = stuck.saturating_sub(1),
        St::Solar | St::Pole => {}
    }
}

fn belt_accept(items: &mut Vec<BeltItem>, item: Item) -> bool {
    if items.last().is_none_or(|l| l.pos >= BELT_GAP) {
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
    dims: (i32, i32),
    dir: Dir,
    q: &mut VecDeque<Item>,
    rr: &mut u8,
) -> bool {
    push_edge(tiles, owner, econ, ax, ay, dims, dir, q, rr, |_| true)
}

/// Which edge of a machine an output leaves by. Every machine pushes its products out the
/// front and its waste out the back, the edge facing away from the arrow. A second fluid
/// output would take `Port::Back` the same way: `push_fluid` already takes the edge to use,
/// so pass it `Port::Back.edge(dir)`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Port {
    Front,
    Back,
}

impl Port {
    /// The side of a machine facing `dir` that this port is on.
    fn edge(self, dir: Dir) -> Dir {
        match self {
            Port::Front => dir,
            Port::Back => dir.opposite(),
        }
    }
    /// Whether an item leaves by this port.
    fn takes(self, item: Item) -> bool {
        (item == Item::Waste) == (self == Port::Back)
    }
    /// Where this port keeps its round-robin spot in a building's `rr` (a nibble each).
    fn shift(self) -> u8 {
        match self {
            Port::Front => 0,
            Port::Back => 4,
        }
    }
}

/// Pushes the first item in `q` that leaves by `port` out of that edge, taking turns between
/// the spots along it. A machine's queue holds products and waste together; each port only
/// ever sends its own, so a stuck waste item never holds up the products.
#[allow(clippy::too_many_arguments)]
fn push_port(
    tiles: &mut [Option<Building>],
    owner: &[Option<u32>],
    econ: &mut Econ,
    ax: i32,
    ay: i32,
    dims: (i32, i32),
    dir: Dir,
    port: Port,
    q: &mut VecDeque<Item>,
    rr: &mut u8,
) -> bool {
    let shift = port.shift();
    let mut spot = (*rr >> shift) & 0xF;
    let sent = push_edge(tiles, owner, econ, ax, ay, dims, port.edge(dir), q, &mut spot, |it| port.takes(it));
    *rr = (*rr & !(0xF << shift)) | (spot << shift);
    sent
}

/// Sends the first item of `q` that `pick` accepts out of `edge`, onto a belt or into the
/// building there. Takes turns between the spots along the edge.
#[allow(clippy::too_many_arguments)]
fn push_edge(
    tiles: &mut [Option<Building>],
    owner: &[Option<u32>],
    econ: &mut Econ,
    ax: i32,
    ay: i32,
    dims: (i32, i32),
    edge: Dir,
    q: &mut VecDeque<Item>,
    rr: &mut u8,
    pick: impl Fn(Item) -> bool,
) -> bool {
    let Some(at) = q.iter().position(|it| pick(*it)) else { return false };
    let item = q[at];
    let n = edge_len(dims, edge);
    for k in 0..n {
        let j = (*rr as i32 + k) % n;
        let (tx, ty) = front_tile(ax, ay, dims, edge, j);
        if try_insert(tiles, owner, econ, tx, ty, item, edge) {
            q.remove(at);
            *rr = ((j + 1) % n) as u8;
            return true;
        }
    }
    false
}

/// Whether a machine's queue has room for one more run (or batch) of the recipe's items,
/// as (products fit, waste fits). Products and waste each get the machine's whole `cap`,
/// so a blocked back stops it only once the waste fills up, and a blocked front only
/// once the products do.
fn outputs_fit(r: &Recipe, batch: u32, out: &VecDeque<Item>, cap: usize) -> (bool, bool) {
    let (held_waste, held_products) = split_waste(out.iter().copied());
    let (made_waste, made_products) = split_waste(r.outputs.iter().flat_map(|(it, n)| std::iter::repeat_n(*it, (*n * batch) as usize)));
    (held_products + made_products <= cap, held_waste + made_waste <= cap)
}

/// How many of these items are waste, and how many are not.
fn split_waste(items: impl Iterator<Item = Item>) -> (usize, usize) {
    items.fold((0, 0), |(w, p), it| if it == Item::Waste { (w + 1, p) } else { (w, p + 1) })
}

/// How much of its input fluid a machine keeps on hand (whole units).
fn fluid_cap(per_craft: u32) -> u32 {
    (per_craft * 4).max(10)
}

/// How much output fluid a machine holds waiting to leave: at least one firing's worth.
fn fluid_out_cap(per_fire: u32) -> u32 {
    CRAFTER_FLUID_OUT_CAP.max(per_fire)
}

/// How much of one input a machine takes in: a few runs' worth, or a whole batch.
fn input_cap(per_run: u32, batch: u32) -> u32 {
    if batch > 1 {
        per_run * batch
    } else {
        (per_run * 4).max(10)
    }
}

/// Pushes up to `n` whole units of a fluid out of a building's front edge: into a pipe
/// network with room for it (a network holds one fluid), or straight into a machine
/// there that takes it. Returns how many units went.
#[allow(clippy::too_many_arguments)]
fn push_fluid(
    tiles: &mut [Option<Building>],
    owner: &[Option<u32>],
    pipe_pool: &[i32],
    pools: &mut [Pool],
    ax: i32,
    ay: i32,
    dims: (i32, i32),
    dir: Dir,
    fl: Fluid,
    n: u32,
) -> u32 {
    let mut sent = 0;
    'units: while sent < n {
        for j in 0..edge_len(dims, dir) {
            let (tx, ty) = front_tile(ax, ay, dims, dir, j);
            if !in_bounds(tx, ty) {
                continue;
            }
            let p = pipe_pool[idx(tx, ty)];
            if p >= 0 {
                let pool = &mut pools[p as usize];
                if pool.room_for(fl) >= 1.0 {
                    pool.add(fl, 1.0);
                    sent += 1;
                    continue 'units;
                }
                continue;
            }
            let Some(a) = owner[idx(tx, ty)] else {
                continue;
            };
            if let Some(Building { kind, st: St::Crafter { recipe, fluid, .. }, .. }) = tiles[a as usize].as_mut() {
                if let Some((f2, per)) = kind.recipes()[*recipe as usize].fluid_in {
                    if f2 == fl && *fluid < fluid_cap(per * kind.batch()) {
                        *fluid += 1;
                        sent += 1;
                        continue 'units;
                    }
                }
            }
        }
        break;
    }
    sent
}

/// Splitters send items, in turn, onto every neighbouring belt that leads away from them.
/// `only`: a filter splitter's front and filter: the filtered item goes out the front only,
/// everything else out the other sides only.
fn split_out(
    tiles: &mut [Option<Building>],
    x: i32,
    y: i32,
    q: &mut VecDeque<Item>,
    rr: &mut u8,
    only: Option<(Dir, Option<Item>)>,
) -> bool {
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
fn try_insert(tiles: &mut [Option<Building>], owner: &[Option<u32>], econ: &mut Econ, tx: i32, ty: i32, item: Item, travel: Dir) -> bool {
    if !in_bounds(tx, ty) {
        return false;
    }
    let Some(a) = owner[idx(tx, ty)] else {
        return false;
    };
    // A tunnel entrance only takes items when its exit is in place.
    let tunnel = match tiles[a as usize].as_ref() {
        Some(Building { kind: Kind::Tunnel, dir, .. }) => tunnel_partner(tiles, tx, ty, *dir),
        _ => None,
    };
    let Some(b) = tiles[a as usize].as_mut() else {
        return false;
    };
    let (bdir, bkind, bside) = (b.dir, b.kind, b.dims().0);
    match &mut b.st {
        St::Belt { items } => bdir != travel.opposite() && belt_accept(items, item),
        St::Drill { coal, .. } => {
            if item == Item::Coal && bkind.burns_coal() && *coal < bkind.drill_coal_cap() {
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
                Some((_, n)) => input_cap(*n, bkind.batch()),
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
            let room = items.back().is_none_or(|it| it.1 <= k as f64 - BELT_GAP);
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
            if *mode == DoorMode::Load && inbox.len() < LIFT_PER_TILE * (bside * bside) as usize {
                inbox.push_back(item);
                true
            } else {
                false
            }
        }
        St::Boiler { coal, fuel, .. } => match item {
            Item::Coal if *coal < BOILER_COAL_CAP => {
                *coal += 1;
                true
            }
            // Waste goes straight into the fire, a quarter of a coal's worth each, up to a full
            // store. The fire only burns down while steam is wanted, so a boiler burns what its
            // grid needs and no more; the rest waits on the belt.
            Item::Waste if *fuel + Item::Waste.burn_ticks().unwrap_or(0) <= BOILER_COAL_CAP * Item::Coal.burn_ticks().unwrap_or(0) => {
                *fuel += Item::Waste.burn_ticks().unwrap_or(0);
                econ.used[Item::Waste.idx()] += 1;
                true
            }
            _ => false,
        },
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
    let ok = |it: Item| filter.is_none_or(|f| f == it);
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
thread_local! {
    /// How many times `rebuild_grid` ran on this thread, so tests can check a batch rebuilds once.
    pub(crate) static REBUILDS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests;
