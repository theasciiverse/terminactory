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

use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
/// Units of fluid a machine holds waiting to leave its front.
pub const CRAFTER_FLUID_OUT_CAP: u32 = 10;
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
pub const SAVE_VERSION: u32 = 3;

/// Writes a fixed-size array as a list (serde itself stops at 32 elements).
fn ser_arr<S, T, const N: usize>(a: &[T; N], s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    s.collect_seq(a.iter())
}

/// An array of defaults (the std Default stops at 32 elements).
fn zeros<T: Default + Copy, const N: usize>() -> [T; N] {
    [T::default(); N]
}

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
}

// What each assembler above the crude one can make: its tiers, each at a speed
// (a tier runs at 1x on the lowest assembler that can make it, faster on the ones above).
static BASIC_ASSEMBLER: LazyLock<Vec<Recipe>> = LazyLock::new(|| assembler_list(&[(CRUDE_RECIPES, 2), (BASIC_RECIPES, 1)]));
static ADVANCED_ASSEMBLER: LazyLock<Vec<Recipe>> =
    LazyLock::new(|| assembler_list(&[(CRUDE_RECIPES, 4), (BASIC_RECIPES, 2), (ADVANCED_RECIPES, 1)]));
// The precision assembler only makes the top tiers: no crude or basic recipes.
static PRECISION_ASSEMBLER: LazyLock<Vec<Recipe>> = LazyLock::new(|| assembler_list(&[(ADVANCED_RECIPES, 2), (PRECISION_RECIPES, 1)]));
static LARGE_ADVANCED_ASSEMBLER: LazyLock<Vec<Recipe>> =
    LazyLock::new(|| assembler_list(&[(LARGE_BASIC_RECIPES, 2), (LARGE_ADVANCED_RECIPES, 1)]));

#[cfg(test)]
thread_local! {
    static RECIPE_OVERRIDE: std::cell::Cell<Option<(Kind, &'static [Recipe])>> = const { std::cell::Cell::new(None) };
}

impl Kind {
    /// Every part (the build menu itself is laid out in app::slots).
    #[cfg_attr(not(test), allow(dead_code))]
    pub const ALL: [Kind; 42] = [
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
            | Kind::Mixer
            | Kind::AdvancedDrill
            | Kind::LargeBasicDrill
            | Kind::LargeAdvancedDrill
            | Kind::AdvancedPump
            | Kind::Pump
            | Kind::Pipe => {
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
            Kind::Mixer => "Mixer",
            Kind::AdvancedDrill => "Advanced drill",
            Kind::LargeBasicDrill => "Large basic drill",
            Kind::LargeAdvancedDrill => "Large advanced drill",
            Kind::AdvancedPump => "Advanced pump",
            Kind::Crate => "Crate",
            Kind::TradePost => "Trade post",
            Kind::Solar => "Cheap solar panel",
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
            Kind::AdvancedAssembler => 1500.0,
            Kind::PrecisionAssembler => 4000.0,
            Kind::LargeBasicAssembler => 2500.0,
            Kind::LargeAdvancedAssembler => 6000.0,
            Kind::Welder => 800.0,
            Kind::Kiln => 200.0,
            Kind::Mixer => 200.0,
            Kind::AdvancedDrill => 240.0,
            Kind::LargeBasicDrill => 1000.0,
            Kind::LargeAdvancedDrill => 2000.0,
            Kind::AdvancedPump => 240.0,
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
            Kind::Furnace
            | Kind::TradePost
            | Kind::Lathe
            | Kind::AdvancedAssembler
            | Kind::PrecisionAssembler
            | Kind::Welder => (3, 3),
            Kind::Kiln => (3, 5),
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
        if self.directional() && matches!(dir, Dir::N | Dir::S) { (h, w) } else { (w, h) }
    }
    pub fn power_draw(self) -> f64 {
        match self {
            Kind::Belt => 0.5,
            Kind::FastBelt | Kind::Pump => 1.0,
            Kind::Splitter | Kind::Sorter | Kind::Molder => 2.0,
            Kind::Press => 3.0,
            Kind::CrudeAssembler | Kind::Mixer | Kind::AdvancedDrill | Kind::AdvancedPump => 4.0,
            Kind::BasicAssembler => 6.0,
            Kind::LargeBasicAssembler => 12.0,
            Kind::Crusher => 6.0,
            Kind::WireDrawer | Kind::Lathe => 4.0,
            Kind::RollingMill => 8.0,
            Kind::Winder => 2.0,
            Kind::Arm => 10.0,
            Kind::LongArm => 15.0,
            Kind::AdvancedAssembler => 16.0,
            Kind::LargeAdvancedAssembler => 48.0,
            Kind::PrecisionAssembler => 64.0,
            Kind::LargeAdvancedDrill => 128.0,
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
        if self == Kind::Kiln { 8 } else { 1 }
    }
    /// Finished goods a machine holds before it stops.
    pub fn out_cap(self) -> usize {
        CRAFTER_OUT_CAP * self.batch() as usize
    }
    /// Has a front that can be rotated (belts and arms move that way; buildings output there).
    pub fn directional(self) -> bool {
        !matches!(
            self,
            Kind::Splitter | Kind::TradePost | Kind::Solar | Kind::Pole | Kind::Pipe | Kind::SteamEngine
        )
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
            Kind::Boiler | Kind::SteamEngine | Kind::WireDrawer | Kind::Winder => Some(Company::Kettering),
            Kind::Crusher => Some(Company::Ashgrove),
            Kind::Lathe | Kind::CrudeAssembler => Some(Company::Gearwright),
            Kind::RollingMill | Kind::Welder => Some(Company::Pillar),
            Kind::Elevator | Kind::Kiln => Some(Company::Underhill),
            Kind::FastBelt | Kind::BasicAssembler => Some(Company::Swiftline),
            Kind::AdvancedDrill | Kind::AdvancedAssembler | Kind::LargeBasicAssembler | Kind::LargeBasicDrill => Some(Company::Ampleworth),
            Kind::PrecisionAssembler | Kind::LargeAdvancedAssembler | Kind::LargeAdvancedDrill => Some(Company::LatticeFinch),
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
        matches!(
            self,
            Kind::Drill | Kind::BasicDrill | Kind::AdvancedDrill | Kind::LargeBasicDrill | Kind::LargeAdvancedDrill
        )
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
    /// An advanced drill is 2x a basic one and runs on EC. A large basic drill mines 16x
    /// and burns 32x the coal; a large advanced drill is 16x an advanced one.
    pub fn drill_rates(self) -> (u32, u32) {
        match self {
            Kind::AdvancedDrill => (2, 0),
            Kind::LargeBasicDrill => (16, 32),
            Kind::LargeAdvancedDrill => (32, 0),
            _ => (1, 1),
        }
    }
    /// Burns coal (the others run on EC).
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
        if self.is_drill() { &[Item::CopperOre, Item::Coal, Item::IronOre, Item::Stone] } else { &[] }
    }
    /// How much water a pump adds per second.
    pub fn pump_rate(self) -> f64 {
        if self == Kind::AdvancedPump { 4.0 * PUMP_RATE } else { PUMP_RATE }
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
    Drill { mines: Option<Item>, fuel: u32, coal: u32, progress: f64, out: VecDeque<Item> },
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
    /// Furnaces, molders, presses and blast furnaces: a chosen recipe, a stock of inputs, and finished goods.
    Crafter {
        recipe: u8,
        #[serde(serialize_with = "ser_arr", deserialize_with = "pad")]
        stock: [u32; ITEMS],
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
            | Kind::Mixer
            | Kind::CokeOven
            | Kind::Crusher
            | Kind::WireDrawer
            | Kind::RollingMill
            | Kind::Lathe
            | Kind::Winder => {
                St::Crafter { recipe: 0, stock: [0; ITEMS], fuel: 0, progress: 0.0, busy: false, out: VecDeque::new(), fluid: 0, fluid_out: 0 }
            }
            Kind::Crate | Kind::Warehouse => St::Crate { items: VecDeque::new() },
            Kind::Tunnel => St::Tunnel { exit: false, items: VecDeque::new() },
            Kind::TradePost => St::Trade { mode: SellMode::Money, money: 0.0, research: 0.0, count: 0, stuck: 0 },
            Kind::Solar => St::Solar,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Econ {
    pub money: f64,
    pub research: f64,
    /// Units sold that earned research, per item (drives the fading curve).
    #[serde(serialize_with = "ser_arr", deserialize_with = "pad")]
    pub research_sales: [u64; ITEMS],
    #[serde(serialize_with = "ser_arr", deserialize_with = "pad")]
    pub sold: [u64; ITEMS],
    /// Units ever produced by a machine, per item (drives discoveries).
    #[serde(serialize_with = "ser_arr", deserialize_with = "pad")]
    pub made: [u64; ITEMS],
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

impl Default for Econ {
    fn default() -> Self {
        Econ {
            money: 0.0,
            research: 0.0,
            research_sales: [0; ITEMS],
            sold: [0; ITEMS],
            made: [0; ITEMS],
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
    #[serde(serialize_with = "ser_arr", deserialize_with = "pad")]
    pub opened: [bool; COMPANIES],
    #[serde(serialize_with = "ser_arr", deserialize_with = "pad")]
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

/// The tiles a power source reaches: (x0, y0, x1, y1), inclusive.
pub fn coverage(kind: Kind, (w, h): (i32, i32), ax: i32, ay: i32) -> Option<(i32, i32, i32, i32)> {
    let r = match kind {
        Kind::Solar => SOLAR_REACH,
        Kind::Pole => POLE_REACH,
        Kind::Elevator => ELEVATOR_REACH,
        Kind::SteamEngine => ENGINE_REACH,
        _ => return None,
    };
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
    if kind == Kind::Elevator { (s, s) } else { kind.dims(dir) }
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
            sale_rate: Vec::new(),
            sale_samples: VecDeque::new(),
            flow_samples: VecDeque::new(),
            mail: VecDeque::new(),
            last_gift: 0,
            gifts: 0,
            grid: Grid::default(),
            fluids: Fluids::default(),
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
            rate_long: (0.0, 0.0),
            gifted: 0.0,
            samples: VecDeque::new(),
            sale_rate: Vec::new(),
            sale_samples: VecDeque::new(),
            flow_samples: VecDeque::new(),
            mail: VecDeque::new(),
            last_gift: 0,
            gifts: 0,
            grid: Grid::default(),
            fluids: Fluids::default(),
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
        let json = serde_json::to_string(self).map_err(std::io::Error::other)?;
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
        w.clamp_recipes();
        w.rebuild_grid();
        Ok(w)
    }

    /// A saved recipe index can point past the end if a recipe list shrank; fall back to the first recipe.
    fn clamp_recipes(&mut self) {
        for fl in &mut self.floors {
            for b in fl.tiles.iter_mut().flatten() {
                let n = b.kind.recipes().len();
                if let St::Crafter { recipe, .. } = &mut b.st {
                    if *recipe as usize >= n {
                        *recipe = 0;
                    }
                }
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
        match level {
            1 => "Building upward needs support beams (make steel girders, then see the tree)",
            -1 => "Digging down needs excavation (make concrete, then see the tree)",
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

    /// Checks a placement facing east (top-left at x, y) and returns its full cost (part + any digging).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn check_place(&self, f: usize, x: i32, y: i32, kind: Kind) -> Result<f64, String> {
        self.check_place_sized(f, x, y, kind, Dir::E, 0)
    }

    /// Like `check_place`, facing `dir`; `s` is the side of an elevator (ignored for other parts).
    pub fn check_place_sized(&self, f: usize, x: i32, y: i32, kind: Kind, dir: Dir, s: i32) -> Result<f64, String> {
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
        let (w, h) = dims_of(kind, dir, s);
        let mut cost = kind.price(w);
        for &g in &floors {
            let fl = &self.floors[g];
            for (tx, ty) in footprint(x, y, w, h) {
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
        if (kind.is_drill() || kind.is_pump()) && self.floors[f].level > 0 {
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
        let s = s.clamp(MIN_LIFT, MAX_LIFT);
        let cost = self.check_place_sized(f, x, y, kind, dir, s)?;
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
        let (w, h) = b.dims();
        for (tx, ty) in footprint(x, y, w, h) {
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
        self.econ.money += b.kind.price(b.dims().0);
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
                *progress = 0.0;
            }
            (St::Crafter { recipe, fluid, fluid_out, .. }, Setting::Recipe(v)) => {
                if kind.recipes().get(v as usize).is_some_and(|r| r.needs.map_or(true, |t| progress.has(t))) {
                    *recipe = v;
                    *fluid = 0;
                    *fluid_out = 0;
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
        self.active = self.floors.iter().map(|fl| fl.tiles.iter().enumerate().filter(|(_, b)| b.is_some()).map(|(i, _)| i as u32).collect()).collect();
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
                let Some(mut b) = tiles[i].take() else { continue };
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
        if self.tick % TPS == 0 {
            self.sample_earnings();
        }
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
        let m = (self.rate.0.max(self.rate_long.0) * away as f64).max(0.0);
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

    /// A full building says EC? when everything in front of it is a belt with no power,
    /// since that's the real reason it can't get rid of its output.
    fn full_word(&self, f: usize, a: usize) -> &'static str {
        let Some(b) = self.floors[f].tiles[a].as_ref() else { return "FULL" };
        let (ax, ay) = ((a % W as usize) as i32, (a / W as usize) as i32);
        let dims = b.dims();
        let mut belts = 0;
        for j in 0..edge_len(dims, b.dir) {
            let (tx, ty) = front_tile(ax, ay, dims, b.dir, j);
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
        if belts > 0 { "EC?" } else { "FULL" }
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
                    Some(self.full_word(f, a))
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
                let made: usize = r.outputs.iter().map(|o| (o.1 * batch) as usize).sum();
                let fluid_full = r.fluid_out.is_some_and(|(_, n)| *fluid_out + n * batch > fluid_out_cap(n * batch));
                if out.len() + made > b.kind.out_cap() || fluid_full {
                    Some(self.full_word(f, a))
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
            let speed = if burn == 0 { ratio } else if *fuel > 0 || self_fuel { 1.0 } else { 0.0 };
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
        St::Crafter { recipe, stock, fuel, progress, busy, out, fluid, fluid_out } => {
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
            let have = r.inputs.iter().all(|(it, n)| stock[it.idx()] >= *n * batch) && r.fluid_in.map_or(true, |(_, n)| *fluid >= n * batch);
            let made: usize = r.outputs.iter().map(|o| (o.1 * batch) as usize).sum();
            // Both outputs need room: items and fluid.
            let room = out.len() + made <= kind.out_cap()
                && r.fluid_out.map_or(true, |(_, n)| *fluid_out + n * batch <= fluid_out_cap(n * batch));
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
                        if let Some((_, n)) = r.fluid_out {
                            *fluid_out += n * batch;
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
            push_front(tiles, owner, econ, x, y, dims, dir, out, rr);
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
            let source = pools_around(pipe_pool, x, y, dims, Some(dir)).into_iter().find(|&p| pools[p].fluid == Some(Fluid::Water) && pools[p].amount >= step);
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
            push_front(tiles, owner, econ, x, y, dims, dir, outbox, rr);
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
    dims: (i32, i32),
    dir: Dir,
    q: &mut VecDeque<Item>,
    rr: &mut u8,
) -> bool {
    let Some(&item) = q.front() else { return false };
    let n = edge_len(dims, dir);
    for k in 0..n {
        let j = (*rr as i32 + k) % n;
        let (tx, ty) = front_tile(ax, ay, dims, dir, j);
        if try_insert(tiles, owner, econ, tx, ty, item, dir) {
            q.pop_front();
            *rr = ((j + 1) % n) as u8;
            return true;
        }
    }
    false
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
    if batch > 1 { per_run * batch } else { (per_run * 4).max(10) }
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
            let Some(a) = owner[idx(tx, ty)] else { continue };
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
            if *mode == DoorMode::Load && inbox.len() < LIFT_PER_TILE * (bside * bside) as usize {
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
        // The starter fuel mines exactly 8 ores ($0.50 each), then the drill waits for coal.
        assert_eq!(w.econ.sold[Item::CopperOre.idx()], 8);
        assert!((w.econ.money - before - 4.0).abs() < 1e-9);
    }

    #[test]
    fn copper_chain_drill_furnace_molder_press_trade_post() {
        let mut w = World::blank();
        // Copper drill -> furnace -> molder -> press, each pushing straight into the next.
        w.place(0, 5, 14, Kind::Drill, Dir::E).unwrap();
        w.place(0, 7, 14, Kind::Furnace, Dir::E).unwrap(); // covers 7-9, 14-16
        w.place(0, 7, 13, Kind::Crate, Dir::S).unwrap(); // coal, pushed down into the furnace
        fill_crate(&mut w, 0, 7, 13, &[Item::Coal; 12]);
        w.place(0, 10, 14, Kind::Molder, Dir::E).unwrap();
        w.place(0, 12, 14, Kind::Press, Dir::E).unwrap(); // makes copper plates first
        w.place(0, 14, 14, Kind::Belt, Dir::E).unwrap();
        w.place(0, 15, 13, Kind::TradePost, Dir::E).unwrap();
        w.place(0, 9, 11, Kind::Solar, Dir::E).unwrap();
        w.place(0, 13, 12, Kind::Pole, Dir::E).unwrap();
        run(&mut w, 100);
        // The drill only had its starter fuel: 8 ores -> 8 molten copper -> 8 ingots -> 8 plates.
        assert_eq!(w.econ.made[Item::CopperIngot.idx()], 8);
        assert_eq!(w.econ.sold[Item::CopperPlate.idx()], 8);
        assert_eq!(w.econ.sold[Item::CopperIngot.idx()], 0, "the press ate them all");
    }

    #[test]
    fn a_fresh_game_can_start_the_copper_chain_and_reach_the_press() {
        let mut w = World::new();
        // Drill -> furnace -> molder -> trade post, all on the starting $1000 (and no research).
        w.place(0, 10, 10, Kind::Drill, Dir::E).unwrap();
        w.place(0, 12, 10, Kind::Furnace, Dir::E).unwrap();
        w.place(0, 12, 9, Kind::Crate, Dir::S).unwrap(); // coal for the furnace
        fill_crate(&mut w, 0, 12, 9, &[Item::Coal; 12]);
        w.place(0, 15, 10, Kind::Molder, Dir::E).unwrap();
        w.place(0, 17, 9, Kind::TradePost, Dir::E).unwrap();
        w.place(0, 15, 13, Kind::Solar, Dir::E).unwrap();
        assert!(w.check_place(0, 30, 30, Kind::Press).unwrap_err().contains("locked"));
        let before = w.econ.money;
        run(&mut w, 100);
        assert_eq!(w.econ.sold[Item::CopperIngot.idx()], 8);
        assert!((w.econ.money - before - 16.0).abs() < 1e-9, "ingots sell for $2");
        // 50 ingots open the press for free.
        w.econ.made[Item::CopperIngot.idx()] = 50;
        w.tick();
        assert!(w.check_place(0, 30, 30, Kind::Press).is_ok());
    }

    /// Sets what a machine has in stock (tests only).
    fn set_stock(w: &mut World, x: i32, y: i32, item: Item, n: u32) {
        let a = w.floors[0].anchor_at(x, y).unwrap();
        if let Some(Building { st: St::Crafter { stock, .. }, .. }) = w.floors[0].tiles[a].as_mut() {
            stock[item.idx()] = n;
        }
    }

    /// (fluid taken in, fluid waiting to leave) of a machine.
    fn crafter_fluid(w: &World, x: i32, y: i32) -> (u32, u32) {
        match &w.tile(0, x, y).unwrap().st {
            St::Crafter { fluid, fluid_out, .. } => (*fluid, *fluid_out),
            _ => panic!("not a machine"),
        }
    }

    #[test]
    fn furnace_feeds_a_molder_directly() {
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap(); // covers 5-7, 5-7
        set_stock(&mut w, 5, 5, Item::CopperOre, 6);
        set_stock(&mut w, 5, 5, Item::Coal, 10);
        w.place(0, 8, 5, Kind::Molder, Dir::E).unwrap(); // takes the molten copper, no pipe
        w.place(0, 10, 5, Kind::Crate, Dir::E).unwrap();
        w.place(0, 8, 7, Kind::Solar, Dir::E).unwrap();
        run(&mut w, 40);
        assert_eq!(crate_items(&w, 0, 10, 5), vec![Item::CopperIngot; 6]);
    }

    #[test]
    fn furnace_pipe_molder() {
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
        set_stock(&mut w, 5, 5, Item::CopperOre, 4);
        set_stock(&mut w, 5, 5, Item::Coal, 10);
        w.place(0, 8, 5, Kind::Pipe, Dir::E).unwrap();
        w.place(0, 9, 5, Kind::Pipe, Dir::E).unwrap();
        w.place(0, 10, 4, Kind::Molder, Dir::E).unwrap(); // the pipe touches its back side
        w.place(0, 12, 4, Kind::Crate, Dir::E).unwrap();
        // Set to iron, the molder leaves the copper in the pipe: it's carrying molten copper.
        w.configure(0, 10, 4);
        run(&mut w, 8);
        assert_eq!((w.fluids.pools[0].fluid, w.fluids.pools[0].amount), (Some(Fluid::MoltenCopper), 1.0));
        assert_eq!(crafter_fluid(&w, 10, 4).0, 0);
        // Back to copper (iron -> steel -> copper) and with EC, it casts them all.
        w.configure(0, 10, 4);
        w.configure(0, 10, 4);
        w.place(0, 10, 6, Kind::Solar, Dir::E).unwrap();
        run(&mut w, 40);
        assert_eq!(crate_items(&w, 0, 12, 4), vec![Item::CopperIngot; 4]);
        assert_eq!(w.fluids.pools[0].amount, 0.0);
        assert_eq!(w.fluids.pools[0].fluid, None, "an empty network holds nothing");
    }

    #[test]
    fn a_pipe_network_holds_one_fluid() {
        let mut p = Pool { fluid: None, amount: 0.0, cap: 20.0, pipes: 1 };
        assert_eq!(p.add(Fluid::MoltenCopper, 5.0), 5.0);
        assert_eq!(p.add(Fluid::Water, 5.0), 0.0, "no second fluid while it holds one");
        assert_eq!(p.take(Fluid::Water, 5.0), 0.0);
        assert_eq!(p.take(Fluid::MoltenCopper, 5.0), 5.0);
        assert_eq!(p.add(Fluid::Water, 5.0), 5.0, "once empty it takes anything");
        // In the world: a furnace can't push copper into a pipe full of water.
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
        set_stock(&mut w, 5, 5, Item::CopperOre, 20);
        set_stock(&mut w, 5, 5, Item::Coal, 30);
        w.place(0, 8, 5, Kind::Pipe, Dir::E).unwrap();
        w.fluids.pools[0].add(Fluid::Water, 3.0);
        run(&mut w, 80);
        assert_eq!(w.fluids.pools[0].fluid, Some(Fluid::Water));
        assert_eq!(w.fluids.pools[0].amount, 3.0);
        assert_eq!(crafter_fluid(&w, 5, 5).1, CRAFTER_FLUID_OUT_CAP, "the molten copper has nowhere to go");
        assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FULL"));
    }

    #[test]
    fn a_full_fluid_output_stops_the_machine() {
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
        set_stock(&mut w, 5, 5, Item::CopperOre, 100);
        set_stock(&mut w, 5, 5, Item::Coal, 100);
        w.place(0, 8, 5, Kind::Pipe, Dir::E).unwrap(); // one pipe holds 20 units, and nothing drains it
        run(&mut w, 250);
        assert_eq!(w.fluids.pools[0].amount, PIPE_CAP);
        assert_eq!(crafter_fluid(&w, 5, 5), (0, CRAFTER_FLUID_OUT_CAP));
        assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FULL"));
        let made = w.econ.used[Item::CopperOre.idx()];
        run(&mut w, 30);
        assert_eq!(w.econ.used[Item::CopperOre.idx()], made, "it stops using ore too");
        // Draining the pipe lets it carry on.
        w.fluids.pools[0].take(Fluid::MoltenCopper, 20.0);
        run(&mut w, 30);
        assert!(w.econ.used[Item::CopperOre.idx()] > made);
    }

    #[test]
    fn a_machine_with_two_outputs_needs_room_for_both() {
        use Item::*;
        static TWO_OUT: [Recipe; 1] = [Recipe {
            name: "test",
            inputs: &[(Coal, 1)],
            outputs: &[(Coke, 1)],
            fluid_in: None,
            fluid_out: Some((Fluid::MoltenCopper, 1)),
            ticks: 60,
            fuel: None,
            needs: None,
            tag: 'T',
        }];
        RECIPE_OVERRIDE.with(|o| o.set(Some((Kind::Molder, &TWO_OUT))));
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Molder, Dir::E).unwrap(); // front is x=7, y=5 and 6
        set_stock(&mut w, 5, 5, Coal, 30);
        w.place(0, 7, 5, Kind::Pipe, Dir::E).unwrap(); // fluid out here...
        w.place(0, 7, 6, Kind::Crate, Dir::E).unwrap(); // ...and items out here
        w.place(0, 5, 7, Kind::Solar, Dir::E).unwrap();
        run(&mut w, 5);
        assert_eq!(crate_items(&w, 0, 7, 6), vec![Coke; 5]);
        assert_eq!(w.fluids.pools[0].amount, 5.0);
        // The pipe fills up with something else: the metal can't leave, so the items stop too.
        w.fluids.pools[0].take(Fluid::MoltenCopper, 5.0);
        w.fluids.pools[0].add(Fluid::Water, 1.0);
        run(&mut w, 40);
        assert_eq!(crate_items(&w, 0, 7, 6).len(), 5 + CRAFTER_FLUID_OUT_CAP as usize);
        assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FULL"));
        RECIPE_OVERRIDE.with(|o| o.set(None));
    }

    #[test]
    fn molders_follow_their_recipe_and_idle_without_metal() {
        let mut w = World::blank();
        w.place(0, 12, 5, Kind::Molder, Dir::E).unwrap();
        w.tick();
        assert_eq!(w.idle_reason(0, idx(12, 5)), Some("FLUD"));
        // A copper furnace can't pour into a molder set to iron.
        w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
        w.place(0, 8, 5, Kind::Molder, Dir::E).unwrap();
        assert!(w.configure(0, 8, 5).unwrap().contains("iron ingot"));
        set_stock(&mut w, 5, 5, Item::CopperOre, 20);
        set_stock(&mut w, 5, 5, Item::Coal, 30);
        run(&mut w, 80);
        assert_eq!(crafter_fluid(&w, 8, 5), (0, 0));
        assert_eq!(w.idle_reason(0, idx(5, 5)), Some("FULL"), "and the furnace waits");
        assert!(w.configure(0, 8, 5).unwrap().contains("steel"));
        assert!(w.configure(0, 8, 5).unwrap().contains("copper ingot"));
    }

    #[test]
    fn presses_make_plates_from_ingots() {
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Press, Dir::E).unwrap();
        set_stock(&mut w, 5, 5, Item::CopperIngot, 2);
        w.place(0, 7, 5, Kind::Crate, Dir::E).unwrap();
        w.place(0, 5, 7, Kind::Solar, Dir::E).unwrap();
        run(&mut w, 8);
        assert_eq!(crate_items(&w, 0, 7, 5), vec![Item::CopperPlate; 2]);
        assert!(w.configure(0, 5, 5).unwrap().contains("iron plate"));
        set_stock(&mut w, 5, 5, Item::IronIngot, 1);
        run(&mut w, 4);
        assert_eq!(crate_items(&w, 0, 7, 5).last(), Some(&Item::IronPlate));
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
        run(&mut w, 100);
        let (rows, secs) = w.flow();
        assert_eq!(secs, 60);
        let get = |it: Item| rows.iter().find(|r| r.item == it).copied().unwrap();
        // The furnace melts one ore per 5 s: about 12 a minute all the way down the chain.
        let plate = get(Item::CopperPlate);
        assert!(plate.made > 8.0 && plate.sold > 8.0 && plate.research > 0.0 && plate.money > 0.0, "{plate:?}");
        let ingot = get(Item::CopperIngot);
        assert!((ingot.made - plate.made).abs() <= 2.0 && (ingot.used - plate.made).abs() <= 2.0, "{ingot:?} {plate:?}");
        let ore = get(Item::CopperOre);
        assert!(ore.made > 8.0 && (ore.used - ingot.made).abs() <= 2.0, "furnaces use one ore per ingot: {ore:?} {ingot:?}");
        assert!(get(Item::Coal).used > 0.0, "fuel counts as used");
    }

    #[test]
    fn demo_factory_runs() {
        let mut w = World::new();
        w.build_demo().unwrap();
        run(&mut w, 200);
        assert!(w.econ.sold[Item::CopperPlate.idx()] > 20, "sold {:?}", w.econ.sold);
        assert!(w.econ.research > 0.0);
        // Everything in the chain has full EC.
        assert_eq!(w.grid.unpowered, 0);
        assert_eq!(w.building_ratio(0, idx(56, 37)), 1.0, "molder");
        assert_eq!(w.building_ratio(0, idx(58, 37)), 1.0, "press");
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
    fn rectangles_turn_with_the_building() {
        assert_eq!(Kind::Press.dims(Dir::E), (2, 1));
        assert_eq!(Kind::Press.dims(Dir::W), (2, 1));
        assert_eq!(Kind::Press.dims(Dir::N), (1, 2));
        assert_eq!(Kind::Press.dims(Dir::S), (1, 2));
        assert_eq!(Kind::Pump.dims(Dir::E), (1, 2));
        assert_eq!(Kind::Pump.dims(Dir::N), (2, 1));
        assert_eq!(Kind::Furnace.dims(Dir::N), (3, 3));
        assert_eq!(Kind::Belt.dims(Dir::N), (1, 1));
        assert_eq!(dims_of(Kind::Elevator, Dir::N, 5), (5, 5));
    }

    #[test]
    fn press_blocks_two_tiles_and_turns_to_one_by_two() {
        let mut w = World::blank();
        let money = w.econ.money;
        w.place(0, 10, 10, Kind::Press, Dir::E).unwrap();
        for (x, y) in [(10, 10), (11, 10)] {
            assert_eq!(w.anchor_of(0, x, y), Some((10, 10)));
            assert!(w.check_place(0, x, y, Kind::Belt).is_err());
        }
        for (x, y) in [(12, 10), (10, 11), (11, 11), (10, 9)] {
            assert!(w.check_place(0, x, y, Kind::Belt).is_ok(), "{x},{y} should be free");
        }
        // Removing it from its far tile frees both and refunds in full.
        assert_eq!(w.remove(0, 11, 10), Some(Kind::Press));
        assert_eq!(w.econ.money, money);
        assert!(w.check_place(0, 10, 10, Kind::Belt).is_ok() && w.check_place(0, 11, 10, Kind::Belt).is_ok());

        // Facing north it is one wide and two tall.
        w.place(0, 10, 10, Kind::Press, Dir::N).unwrap();
        for (x, y) in [(10, 10), (10, 11)] {
            assert_eq!(w.anchor_of(0, x, y), Some((10, 10)));
        }
        for (x, y) in [(11, 10), (11, 11), (10, 12)] {
            assert!(w.check_place(0, x, y, Kind::Belt).is_ok(), "{x},{y} should be free");
        }
        // The check follows the direction: a belt on (10, 11) blocks a north press, not an east one.
        let mut w = World::blank();
        w.place(0, 10, 11, Kind::Belt, Dir::E).unwrap();
        assert_eq!(w.check_place_sized(0, 10, 10, Kind::Press, Dir::N, 0), Err("Something is in the way".into()));
        assert!(w.check_place_sized(0, 10, 10, Kind::Press, Dir::E, 0).is_ok());
    }

    #[test]
    fn placing_off_the_map_is_refused_for_long_machines() {
        let w = World::blank();
        let fit = |x, y, k, d| w.check_place_sized(0, x, y, k, d, 0);
        assert!(fit(W - 2, 5, Kind::Press, Dir::E).is_ok());
        assert_eq!(fit(W - 1, 5, Kind::Press, Dir::E), Err("Doesn't fit on the map here".into()));
        assert!(fit(W - 1, 5, Kind::Press, Dir::N).is_ok());
        assert_eq!(fit(5, H - 1, Kind::Press, Dir::N), Err("Doesn't fit on the map here".into()));
        assert!(fit(5, H - 1, Kind::Press, Dir::E).is_ok());
        assert_eq!(fit(W - 2, 5, Kind::Furnace, Dir::E), Err("Doesn't fit on the map here".into()));
    }

    #[test]
    fn furnace_is_three_by_three() {
        let mut w = World::blank();
        w.place(0, 10, 10, Kind::Furnace, Dir::E).unwrap();
        for (x, y) in footprint(10, 10, 3, 3) {
            assert_eq!(w.anchor_of(0, x, y), Some((10, 10)));
            assert!(w.check_place(0, x, y, Kind::Belt).is_err());
        }
        assert!(w.check_place(0, 13, 12, Kind::Belt).is_ok());
        assert!(w.check_place(0, 12, 13, Kind::Belt).is_ok());
        assert_eq!(w.remove(0, 12, 12), Some(Kind::Furnace));
        assert!(footprint(10, 10, 3, 3).all(|(x, y)| w.check_place(0, x, y, Kind::Belt).is_ok()));
    }

    #[test]
    fn pump_is_one_by_two() {
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Pump, Dir::E).unwrap();
        assert_eq!(w.anchor_of(0, 5, 6), Some((5, 5)));
        assert!(w.check_place(0, 6, 5, Kind::Pipe).is_ok());
        assert!(w.check_place(0, 5, 7, Kind::Pipe).is_ok());
        // A pipe beside its second tile is on its network too.
        w.place(0, 6, 6, Kind::Pipe, Dir::E).unwrap();
        w.place(0, 7, 5, Kind::Solar, Dir::E).unwrap();
        run(&mut w, 3);
        assert!(w.fluids.pools[0].amount > 0.0);
        // Turned north it lies down: two wide, one tall.
        assert_eq!(w.remove(0, 5, 6), Some(Kind::Pump));
        w.place(0, 5, 5, Kind::Pump, Dir::N).unwrap();
        assert_eq!(w.anchor_of(0, 6, 5), Some((5, 5)));
        assert!(w.check_place(0, 5, 6, Kind::Pipe).is_ok());
    }

    #[test]
    fn press_output_leaves_from_the_front_tile() {
        // Facing east: crate -> press -> belt in front of its right-hand tile -> crate.
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
        fill_crate(&mut w, 0, 5, 5, &[Item::CopperIngot; 3]);
        w.place(0, 6, 5, Kind::Press, Dir::E).unwrap(); // covers 6-7, 5
        w.place(0, 8, 5, Kind::Belt, Dir::E).unwrap();
        w.place(0, 9, 5, Kind::Crate, Dir::E).unwrap();
        w.place(0, 6, 7, Kind::Solar, Dir::E).unwrap();
        run(&mut w, 6);
        assert!(!crate_items(&w, 0, 9, 5).is_empty());
        assert!(crate_items(&w, 0, 9, 5).iter().all(|&i| i == Item::CopperPlate));

        // Facing north: it takes ingots from the side and sends plates out of its top tile.
        let mut w = World::blank();
        w.place(0, 9, 11, Kind::Crate, Dir::E).unwrap(); // beside the lower tile
        fill_crate(&mut w, 0, 9, 11, &[Item::CopperIngot; 3]);
        w.place(0, 10, 10, Kind::Press, Dir::N).unwrap(); // covers 10, 10-11
        w.place(0, 10, 9, Kind::Belt, Dir::N).unwrap();
        w.place(0, 10, 8, Kind::Crate, Dir::E).unwrap();
        w.place(0, 12, 10, Kind::Solar, Dir::E).unwrap();
        run(&mut w, 6);
        assert!(!crate_items(&w, 0, 10, 8).is_empty());
        // Nothing comes out of the sides or the back.
        w.place(0, 11, 11, Kind::Crate, Dir::E).unwrap();
        w.place(0, 10, 12, Kind::Crate, Dir::E).unwrap();
        run(&mut w, 3);
        assert!(crate_items(&w, 0, 11, 11).is_empty() && crate_items(&w, 0, 10, 12).is_empty());
    }

    #[test]
    fn turning_a_placed_press_needs_room() {
        let mut w = World::blank();
        w.place(0, 10, 10, Kind::Press, Dir::E).unwrap(); // covers 10-11, 10
        w.place(0, 10, 11, Kind::Belt, Dir::E).unwrap();
        // North would need (10, 11), which is taken: nothing changes.
        assert_eq!(w.set_dir(0, 10, 10, Dir::N), Err("Something is in the way".into()));
        assert_eq!(w.tile(0, 10, 10).unwrap().dir, Dir::E);
        assert_eq!(w.anchor_of(0, 11, 10), Some((10, 10)));
        w.remove(0, 10, 11);
        w.set_dir(0, 10, 10, Dir::N).unwrap();
        assert_eq!(w.anchor_of(0, 10, 11), Some((10, 10)));
        assert_eq!(w.anchor_of(0, 11, 10), None, "the old tile is free again");
        // Turning back and forth between east and west changes nothing.
        w.set_dir(0, 10, 10, Dir::W).unwrap();
        assert_eq!(w.anchor_of(0, 11, 10), Some((10, 10)));
        assert_eq!(w.anchor_of(0, 10, 11), None);
        // A belt just turns.
        w.place(0, 20, 20, Kind::Belt, Dir::E).unwrap();
        w.set_dir(0, 20, 20, Dir::S).unwrap();
        assert!(w.set_dir(0, 30, 30, Dir::S).is_err());
    }

    #[test]
    fn low_power_slows_evenly() {
        let mut w = World::blank();
        w.place(0, 10, 10, Kind::Solar, Dir::E).unwrap();
        // 32 belts fill the panel's reach around it: 16 power wanted, 10 available.
        for (x, y) in footprint(8, 8, 6, 6) {
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
        w.place(0, 6, 5, Kind::Press, Dir::E).unwrap();
        while !w.configure(0, 6, 5).unwrap().contains("copper wire") {}
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
        // Metals only: it goes round without ever reaching coke.
        for _ in 0..8 {
            assert!(w.configure(0, 5, 5).unwrap().contains("molten"));
        }
        w.progress.researched[Tech::Coking.idx()] = true;
        while !w.configure(0, 5, 5).unwrap().ends_with("makes coke") {}
        // Three coal go in, coke comes out in 5 s, no separate fuel needed.
        w.place(0, 3, 5, Kind::Crate, Dir::E).unwrap();
        w.place(0, 4, 5, Kind::Belt, Dir::E).unwrap();
        w.place(0, 3, 7, Kind::Solar, Dir::E).unwrap();
        fill_crate(&mut w, 0, 3, 5, &[Item::Coal; 6]);
        run(&mut w, 14);
        assert_eq!(w.econ.made[Item::Coke.idx()], 2);
    }

    #[test]
    fn drills_mine_copper_coal_iron_and_stone() {
        let mut w = World::blank();
        // A cheap drill goes copper -> coal -> iron -> stone -> copper.
        w.place(0, 10, 10, Kind::Drill, Dir::E).unwrap();
        let mines = |w: &World, x, y| match &w.tile(0, x, y).unwrap().st {
            St::Drill { mines, .. } => *mines,
            _ => None,
        };
        assert_eq!(mines(&w, 10, 10), Some(Item::CopperOre));
        w.configure(0, 10, 10);
        assert_eq!(mines(&w, 10, 10), Some(Item::Coal));
        assert!(w.configure(0, 10, 10).unwrap().contains("half the speed"));
        assert_eq!(mines(&w, 10, 10), Some(Item::IronOre));
        w.configure(0, 10, 10);
        assert_eq!(mines(&w, 10, 10), Some(Item::Stone));
        w.configure(0, 10, 10);
        assert_eq!(mines(&w, 10, 10), Some(Item::CopperOre));
        w.apply_setting(0, 10, 10, Setting::Mines(Item::IronOre));
        assert_eq!(mines(&w, 10, 10), Some(Item::IronOre), "a copied iron setting sticks to a cheap drill now");
        // Stone, anywhere on the ground; the starter fuel mines 8.
        w.place(0, 30, 30, Kind::Drill, Dir::E).unwrap();
        w.apply_setting(0, 30, 30, Setting::Mines(Item::Stone));
        w.place(0, 32, 30, Kind::Crate, Dir::E).unwrap();
        run(&mut w, 10);
        assert_eq!(crate_items(&w, 0, 32, 30), vec![Item::Stone; 8]);
        assert_eq!(Item::Stone.price(), 0.3);
    }

    #[test]
    fn a_cheap_drill_cuts_iron_at_half_the_basic_drills_speed() {
        let mut w = World::blank();
        for (kind, y) in [(Kind::BasicDrill, 20), (Kind::Drill, 25)] {
            w.place(0, 20, y, kind, Dir::E).unwrap();
            w.apply_setting(0, 20, y, Setting::Mines(Item::IronOre));
            w.place(0, 22, y, Kind::Crate, Dir::E).unwrap();
        }
        run(&mut w, 4);
        let (basic, cheap) = (crate_items(&w, 0, 22, 20), crate_items(&w, 0, 22, 25));
        assert!(basic.iter().chain(&cheap).all(|i| *i == Item::IronOre));
        assert_eq!((basic.len(), cheap.len()), (6, 3));
        // Copper is unchanged: a cheap drill is at 60 ticks an ore.
        w.place(0, 40, 20, Kind::Drill, Dir::E).unwrap();
        w.place(0, 42, 20, Kind::Crate, Dir::E).unwrap();
        run(&mut w, 3);
        assert_eq!(crate_items(&w, 0, 42, 20).len(), 3);
    }

    #[test]
    fn blast_furnace_melts_iron_then_makes_steel_with_waste() {
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
        fill_crate(&mut w, 0, 5, 5, &[Item::IronOre, Item::IronOre, Item::IronOre, Item::Coke, Item::Coke]);
        w.place(0, 6, 5, Kind::BlastFurnace, Dir::E).unwrap(); // covers 6-7, 5-6
        w.place(0, 8, 5, Kind::Pipe, Dir::E).unwrap();
        run(&mut w, 20);
        // 5 s an ore, one molten iron each, into the pipe.
        assert_eq!(w.fluids.pools[0].fluid, Some(Fluid::MoltenIron));
        assert_eq!(w.fluids.pools[0].amount, 3.0);
        // The iron furnace is slower and gives 2 molten iron for 3 ore.
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
        assert!(w.configure(0, 5, 5).unwrap().contains("molten iron"));
        set_stock(&mut w, 5, 5, Item::IronOre, 3);
        set_stock(&mut w, 5, 5, Item::Coal, 5);
        w.place(0, 8, 5, Kind::Pipe, Dir::E).unwrap();
        run(&mut w, 9);
        assert_eq!(w.fluids.pools[0].amount, 0.0, "10 s a batch");
        run(&mut w, 3);
        assert_eq!((w.fluids.pools[0].fluid, w.fluids.pools[0].amount), (Some(Fluid::MoltenIron), 2.0));
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
        for (x, y) in footprint(14, 2, 4, 4).chain(footprint(14, 6, 3, 3)) {
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
        w.econ.made[Item::CopperIngot.idx()] = 49;
        w.tick();
        assert!(!w.progress.has(Tech::Press));
        w.econ.made[Item::CopperIngot.idx()] = 50;
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

    // ---------- the later machines ----------

    fn recipe_names(kind: Kind) -> Vec<&'static str> {
        kind.recipes().iter().map(|r| r.name).collect()
    }

    /// Points a machine at a recipe by name.
    fn pick_recipe(w: &mut World, x: i32, y: i32, name: &str) {
        let a = w.floors[0].anchor_at(x, y).unwrap();
        let b = w.floors[0].tiles[a].as_mut().unwrap();
        let n = b.kind.recipes().iter().position(|r| r.name == name).unwrap();
        if let St::Crafter { recipe, .. } = &mut b.st {
            *recipe = n as u8;
        }
    }

    /// Ticks a recipe takes in a machine, from its list.
    fn recipe_ticks(kind: Kind, name: &str) -> u32 {
        kind.recipes().iter().find(|r| r.name == name).unwrap().ticks
    }

    fn scaled(t: u32, by: u32) -> u32 {
        ((t as f64 / by as f64).round() as u32).max(1)
    }

    #[test]
    fn the_assembler_ladder_lists_what_each_can_make() {
        let tier = |t: &[Recipe]| t.iter().map(|r| r.name).collect::<Vec<_>>();
        let (crude, basic, adv, prec) = (tier(CRUDE_RECIPES), tier(BASIC_RECIPES), tier(ADVANCED_RECIPES), tier(PRECISION_RECIPES));
        let (large_basic, large_adv) = (tier(LARGE_BASIC_RECIPES), tier(LARGE_ADVANCED_RECIPES));
        let joined = |parts: &[&Vec<&'static str>]| parts.iter().flat_map(|p| p.iter().copied()).collect::<Vec<_>>();
        assert_eq!(recipe_names(Kind::CrudeAssembler), crude);
        assert_eq!(recipe_names(Kind::BasicAssembler), joined(&[&crude, &basic]));
        assert_eq!(recipe_names(Kind::AdvancedAssembler), joined(&[&crude, &basic, &adv]));
        assert_eq!(recipe_names(Kind::PrecisionAssembler), joined(&[&adv, &prec]), "precision makes only the top tiers");
        assert_eq!(recipe_names(Kind::LargeBasicAssembler), large_basic);
        assert_eq!(recipe_names(Kind::LargeAdvancedAssembler), joined(&[&large_basic, &large_adv]));
        assert_eq!(crude, ["valve", "gauge", "lamp", "relay", "gearbox"]);
        assert_eq!(basic, ["electric motor", "piston"]);
        assert_eq!(adv, ["control board"]);
        assert!(prec.is_empty(), "later processors go here");
        assert_eq!(large_basic, ["pump unit", "engine block"]);
        assert_eq!(large_adv, ["generator", "control panel", "drilling rig", "steam tractor"]);
        // Large and small assemblers share nothing.
        for small in [Kind::CrudeAssembler, Kind::BasicAssembler, Kind::AdvancedAssembler, Kind::PrecisionAssembler] {
            for big in [Kind::LargeBasicAssembler, Kind::LargeAdvancedAssembler] {
                assert!(recipe_names(small).iter().all(|n| !recipe_names(big).contains(n)), "{small:?} and {big:?}");
            }
        }
        // Each tier's time is the lowest assembler's; the ones above run it faster.
        let t = recipe_ticks;
        assert_eq!(t(Kind::CrudeAssembler, "gearbox"), 600);
        assert_eq!(t(Kind::BasicAssembler, "gearbox"), 300);
        assert_eq!(t(Kind::AdvancedAssembler, "gearbox"), 150);
        assert_eq!(t(Kind::BasicAssembler, "electric motor"), 600);
        assert_eq!(t(Kind::AdvancedAssembler, "electric motor"), 300);
        assert_eq!(t(Kind::AdvancedAssembler, "control board"), 600);
        assert_eq!(t(Kind::PrecisionAssembler, "control board"), 300);
        assert_eq!(t(Kind::LargeBasicAssembler, "pump unit"), 1200);
        assert_eq!(t(Kind::LargeAdvancedAssembler, "pump unit"), 600);
        assert_eq!(t(Kind::LargeAdvancedAssembler, "generator"), 1200);
        assert_eq!(t(Kind::LargeAdvancedAssembler, "drilling rig"), 2400);
        assert_eq!(t(Kind::LargeAdvancedAssembler, "steam tractor"), 3600);
        // Odd times round to a whole tick.
        assert_eq!(scaled(75, 2), 38);
        // Sizes, power and prices.
        let specs = [
            (Kind::CrudeAssembler, (4, 4), 4.0, 300.0),
            (Kind::BasicAssembler, (2, 2), 6.0, 600.0),
            (Kind::AdvancedAssembler, (3, 3), 16.0, 1500.0),
            (Kind::PrecisionAssembler, (3, 3), 64.0, 4000.0),
            (Kind::LargeBasicAssembler, (5, 5), 12.0, 2500.0),
            (Kind::LargeAdvancedAssembler, (5, 5), 48.0, 6000.0),
        ];
        for (k, dims, ec, cost) in specs {
            assert_eq!((k.base_dims(), k.power_draw(), k.cost()), (dims, ec, cost), "{k:?}");
        }
    }

    #[test]
    fn higher_assemblers_run_crude_recipes_faster() {
        let mut w = World::blank();
        let rows = [(Kind::CrudeAssembler, 4), (Kind::BasicAssembler, 14), (Kind::AdvancedAssembler, 24)];
        for (k, y) in rows {
            w.place(0, 10, y, k, Dir::E).unwrap();
            w.place(0, 10 + k.base_dims().0, y, Kind::Crate, Dir::E).unwrap();
            // Two panels joined by a pole, so even the advanced one (16 EC) has enough.
            w.place(0, 8, y - 2, Kind::Solar, Dir::E).unwrap();
            w.place(0, 12, y - 2, Kind::Solar, Dir::E).unwrap();
            w.place(0, 10, y - 3, Kind::Pole, Dir::E).unwrap();
            pick_recipe(&mut w, 10, y, "gearbox");
        }
        for _ in 0..1200 {
            for (_, y) in rows {
                set_stock(&mut w, 10, y, Item::Steel, 10);
                set_stock(&mut w, 10, y, Item::IronGear, 30);
            }
            w.tick();
        }
        let boxes = |w: &World, k: Kind, y: i32| crate_items(w, 0, 10 + k.base_dims().0, y).len();
        assert_eq!(boxes(&w, Kind::CrudeAssembler, 4), 2, "10 s each");
        assert_eq!(boxes(&w, Kind::BasicAssembler, 14), 4, "twice as fast");
        assert_eq!(boxes(&w, Kind::AdvancedAssembler, 24), 8, "four times as fast");
    }

    #[test]
    fn a_precision_assembler_refuses_crude_and_basic_recipes() {
        let mut w = World::blank();
        w.place(0, 10, 10, Kind::PrecisionAssembler, Dir::E).unwrap();
        w.place(0, 10, 8, Kind::Solar, Dir::E).unwrap();
        let names = recipe_names(Kind::PrecisionAssembler);
        for crude in CRUDE_RECIPES.iter().chain(BASIC_RECIPES) {
            assert!(!names.contains(&crude.name), "{}", crude.name);
        }
        assert_eq!(names, ["control board"]);
        // Nothing to cycle to: the later processors are not here yet.
        assert!(w.configure(0, 10, 10).unwrap().contains("only one recipe"));
        // It takes what a control board uses, and turns away what only a crude or basic recipe uses.
        assert!(w.offer(0, 10, 10, Item::Relay));
        assert!(!w.offer(0, 10, 10, Item::IronGear), "a crude recipe's input");
        assert!(!w.offer(0, 10, 10, Item::CopperCoil), "a basic recipe's input");
        assert!(!w.offer(0, 10, 10, Item::SteelRod), "a basic recipe's input");
        // A copied recipe number past its list is ignored.
        w.apply_setting(0, 10, 10, Setting::Recipe(40));
        assert!(w.tile(0, 10, 10).unwrap().recipe().is_some());
    }

    #[test]
    fn control_boards_take_10_s_in_an_advanced_assembler_and_5_s_in_a_precision_one() {
        let stock = [(Item::Relay, 1), (Item::FineWire, 4), (Item::IronSheet, 1)];
        let adv = |secs| run_powered(Kind::AdvancedAssembler, "control board", &stock, secs);
        let prec = |secs| run_powered(Kind::PrecisionAssembler, "control board", &stock, secs);
        assert!(adv(9).is_empty() && adv(11) == vec![Item::ControlBoard]);
        assert!(prec(4).is_empty() && prec(6) == vec![Item::ControlBoard]);
        // The other tiers fit the same recipe at their own pace.
        assert_eq!(recipe_ticks(Kind::PrecisionAssembler, "control board") * 2, recipe_ticks(Kind::AdvancedAssembler, "control board"));
    }

    #[test]
    fn the_welder_draws_ec_only_while_it_works() {
        let mut w = World::blank();
        // Two panels (20 EC) joined by a pole, reaching a welder and a belt.
        w.place(0, 12, 10, Kind::Welder, Dir::E).unwrap(); // covers 12-14, 10-12
        w.place(0, 8, 9, Kind::Solar, Dir::E).unwrap();
        w.place(0, 8, 13, Kind::Solar, Dir::E).unwrap();
        w.place(0, 9, 12, Kind::Pole, Dir::E).unwrap();
        w.place(0, 15, 11, Kind::Crate, Dir::E).unwrap();
        w.place(0, 7, 12, Kind::Belt, Dir::E).unwrap();
        assert_eq!(Kind::Welder.burst_draw(), 30.0);
        assert_eq!(Kind::Welder.power_draw(), 0.0, "nothing while idle");
        assert_eq!(w.grid.nets.len(), 1);
        let demand = |w: &World| w.net_at(0, 12, 10).unwrap().demand;
        // Idle: only the belt (0.5) is on the grid.
        w.tick();
        assert_eq!(demand(&w), 0.5);
        assert_eq!(w.ratio_at(0, 12, 10), 1.0);
        // Weld: it draws 30, and the 20 EC grid browns out, slowing the belt too.
        set_stock(&mut w, 12, 10, Item::SteelGirder, 4);
        set_stock(&mut w, 12, 10, Item::Screw, 16);
        pick_recipe(&mut w, 12, 10, "frame");
        w.tick(); // it starts working
        w.tick(); // and the grid feels it
        assert_eq!(demand(&w), 30.5);
        let ratio = 20.0 / 30.5;
        assert!((w.ratio_at(0, 12, 10) - ratio).abs() < 1e-9);
        assert!((w.ratio_at(0, 7, 12) - ratio).abs() < 1e-9, "everything on the grid slows evenly");
        // At that speed a 600 tick weld takes 915 ticks; two welds use up the stock.
        let made = |w: &World| w.econ.made[Item::Frame.idx()];
        for _ in 0..900 {
            w.tick();
        }
        assert_eq!(made(&w), 0, "slowed by the shortfall");
        run(&mut w, 1);
        assert_eq!(made(&w), 1);
        run(&mut w, 15);
        assert_eq!(made(&w), 2, "the second weld");
        // Out of material: the draw switches off and the grid recovers.
        assert_eq!(demand(&w), 0.5);
        assert_eq!(w.ratio_at(0, 7, 12), 1.0);
        assert_eq!(w.idle_reason(0, idx(12, 10)), Some("NEED"));
    }

    #[test]
    fn a_welder_with_no_power_waits() {
        let mut w = World::blank();
        w.place(0, 12, 10, Kind::Welder, Dir::E).unwrap();
        pick_recipe(&mut w, 12, 10, "frame");
        set_stock(&mut w, 12, 10, Item::SteelGirder, 2);
        set_stock(&mut w, 12, 10, Item::Screw, 8);
        run(&mut w, 15);
        assert_eq!(w.econ.made[Item::Frame.idx()], 0);
        assert_eq!(w.idle_reason(0, idx(12, 10)), Some("EC?"));
        // A grid with nothing to give can't start it for free either.
        w.place(0, 12, 13, Kind::Pole, Dir::E).unwrap();
        run(&mut w, 15);
        assert_eq!(w.econ.made[Item::Frame.idx()], 0);
    }

    #[test]
    fn the_kiln_waits_for_a_full_load_then_fires_eight_at_once() {
        let mut w = World::blank();
        // 3 wide, 5 tall facing east; 5 wide, 3 tall facing north.
        assert_eq!(Kind::Kiln.dims(Dir::E), (3, 5));
        assert_eq!(Kind::Kiln.dims(Dir::N), (5, 3));
        w.place(0, 5, 5, Kind::Kiln, Dir::E).unwrap(); // covers 5-7, 5-9: front is x=8
        assert!(w.place(0, 6, 9, Kind::Crate, Dir::E).is_err(), "5 tall");
        w.place(0, 8, 9, Kind::Crate, Dir::E).unwrap(); // beside its bottom row
        set_stock(&mut w, 5, 5, Item::Coal, 5);
        let stones = |w: &mut World, n: u32| (0..n).for_each(|_| assert!(w.offer(0, 5, 5, Item::Stone)));
        let made = |w: &World| w.econ.made[Item::Brick.idx()];
        stones(&mut w, 15);
        run(&mut w, 20);
        assert_eq!(made(&w), 0, "7 runs of 8 is not enough");
        assert_eq!(w.idle_reason(0, idx(5, 5)), Some("LOAD"));
        match &w.tile(0, 5, 5).unwrap().st {
            St::Crafter { busy, .. } => assert!(!busy),
            _ => unreachable!(),
        }
        // The last stone starts the fire. All 8 come out together after one recipe time.
        stones(&mut w, 1);
        assert!(!w.offer(0, 5, 5, Item::Stone), "it holds exactly one batch");
        let mut fired_at = None;
        for t in 0..400 {
            w.tick();
            if made(&w) > 0 {
                assert_eq!(made(&w), 8, "all at once");
                fired_at = Some(t);
                break;
            }
        }
        let t = fired_at.expect("it fired");
        assert!((299..=301).contains(&t), "one recipe time (300 ticks): {t}");
        run(&mut w, 5);
        assert_eq!(crate_items(&w, 0, 8, 9), vec![Item::Brick; 8]);
        assert_eq!(w.econ.used[Item::Stone.idx()], 16);
        // And it is empty again: the next batch needs 16 more.
        assert_eq!(w.idle_reason(0, idx(5, 5)), Some("LOAD"));
    }

    #[test]
    fn the_kiln_makes_bricks_and_glass_in_batches_of_eight() {
        let out = run_machine(Kind::Kiln, "brick", &[(Item::Stone, 16), (Item::Coal, 5)], 8);
        assert_eq!(out, vec![Item::Brick; 8]);
        let out = run_machine(Kind::Kiln, "glass", &[(Item::Sand, 24), (Item::Coal, 5)], 8);
        assert_eq!(out, vec![Item::Glass; 8]);
        // Not a full batch: nothing fires.
        assert!(run_machine(Kind::Kiln, "glass", &[(Item::Sand, 21), (Item::Coal, 5)], 12).is_empty());
        // No coal, no fire.
        assert!(run_machine(Kind::Kiln, "brick", &[(Item::Stone, 16)], 12).is_empty());
    }

    #[test]
    fn a_pump_pipe_mixer_chain_makes_concrete() {
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Pump, Dir::E).unwrap(); // covers 5, 5-6
        for x in 6..=8 {
            w.place(0, x, 5, Kind::Pipe, Dir::E).unwrap();
        }
        w.place(0, 9, 5, Kind::Mixer, Dir::E).unwrap(); // covers 9-10, 5-6; front is x=11
        w.place(0, 11, 5, Kind::Crate, Dir::E).unwrap();
        w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap(); // reaches the pump and the mixer
        set_stock(&mut w, 9, 5, Item::Sand, 10);
        set_stock(&mut w, 9, 5, Item::Stone, 5);
        w.tick();
        assert_eq!(w.idle_reason(0, idx(9, 5)), Some("FLUD"), "no water yet");
        run(&mut w, 30);
        // 300 ticks a run, 2 sand + 1 stone + 1 water -> 1 concrete: 5 runs.
        assert_eq!(crate_items(&w, 0, 11, 5), vec![Item::Concrete; 5]);
        assert_eq!(w.econ.used[Item::Sand.idx()], 10);
        assert_eq!(w.econ.used[Item::Stone.idx()], 5);
    }

    #[test]
    fn a_mixer_makes_reinforced_concrete_from_concrete_rod_and_water() {
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Pump, Dir::E).unwrap();
        for x in 6..=8 {
            w.place(0, x, 5, Kind::Pipe, Dir::E).unwrap();
        }
        w.place(0, 9, 5, Kind::Mixer, Dir::E).unwrap();
        w.place(0, 11, 5, Kind::Crate, Dir::E).unwrap();
        w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap();
        pick_recipe(&mut w, 9, 5, "reinforced concrete");
        set_stock(&mut w, 9, 5, Item::Concrete, 5);
        set_stock(&mut w, 9, 5, Item::IronRod, 3);
        run(&mut w, 15);
        // 2 concrete + 1 rod + 1 water each: two made, and one concrete is left over.
        assert_eq!(crate_items(&w, 0, 11, 5), vec![Item::ReinforcedConcrete; 2]);
        assert_eq!(w.econ.used[Item::Concrete.idx()], 4);
        assert_eq!(w.econ.used[Item::IronRod.idx()], 2);
    }

    #[test]
    fn a_mixer_without_water_waits() {
        let mut w = World::blank();
        w.place(0, 9, 5, Kind::Mixer, Dir::E).unwrap();
        w.place(0, 11, 5, Kind::Crate, Dir::E).unwrap();
        w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap();
        set_stock(&mut w, 9, 5, Item::Sand, 10);
        set_stock(&mut w, 9, 5, Item::Stone, 5);
        run(&mut w, 10);
        assert!(crate_items(&w, 0, 11, 5).is_empty());
        assert_eq!(w.idle_reason(0, idx(9, 5)), Some("FLUD"));
    }

    /// `n` panels (10 EC each) in a column at x - 2, joined by poles at x, from row y down.
    fn big_grid(w: &mut World, x: i32, y: i32, n: i32) {
        for i in 0..n {
            w.place(0, x - 2, y + i * 2, Kind::Solar, Dir::E).unwrap();
        }
        for i in 0..=(n * 2) / 6 {
            w.place(0, x, y + i * 6 + 1, Kind::Pole, Dir::E).unwrap();
        }
    }

    fn drill_crates(w: &mut World, x: i32, y: i32, n: i32) {
        for i in 0..n {
            w.place(0, x, y + i, Kind::Crate, Dir::E).unwrap();
        }
    }

    fn crates_total(w: &World, x: i32, y: i32, n: i32) -> usize {
        (0..n).map(|i| crate_items(w, 0, x, y + i).len()).sum()
    }

    #[test]
    fn an_advanced_drill_is_electric_and_twice_as_fast_as_a_basic_drill() {
        let mut w = World::blank();
        assert_eq!((Kind::AdvancedDrill.base_dims(), Kind::AdvancedDrill.cost(), Kind::AdvancedDrill.power_draw()), ((4, 4), 240.0, 4.0));
        // A basic drill with coal on hand and an advanced one with none, both on iron.
        w.place(0, 5, 5, Kind::BasicDrill, Dir::E).unwrap();
        w.apply_setting(0, 5, 5, Setting::Mines(Item::IronOre));
        w.place(0, 7, 5, Kind::Crate, Dir::E).unwrap();
        if let Some(Building { st: St::Drill { coal, .. }, .. }) = w.floors[0].tiles[idx(5, 5)].as_mut() {
            *coal = 10;
        }
        w.place(0, 20, 5, Kind::AdvancedDrill, Dir::E).unwrap(); // covers 20-23, 5-8; front is x=24
        w.apply_setting(0, 20, 5, Setting::Mines(Item::IronOre));
        w.place(0, 24, 5, Kind::Crate, Dir::E).unwrap();
        w.place(0, 20, 9, Kind::Solar, Dir::E).unwrap(); // reaches its bottom rows
        w.tick();
        assert_eq!(w.idle_reason(0, idx(20, 5)), None, "no coal needed");
        run(&mut w, 10);
        let (basic, adv) = (crate_items(&w, 0, 7, 5), crate_items(&w, 0, 24, 5));
        assert!(adv.iter().all(|i| *i == Item::IronOre));
        assert_eq!((basic.len(), adv.len()), (15, 30), "iron at 40 ticks an ore, and at 20");
        assert!(w.econ.used[Item::Coal.idx()] <= 2, "only the basic drill burned coal");
    }

    #[test]
    fn an_advanced_drill_without_power_waits_and_slows_with_the_grid() {
        let mut w = World::blank();
        w.place(0, 20, 5, Kind::AdvancedDrill, Dir::E).unwrap();
        w.place(0, 24, 5, Kind::Crate, Dir::E).unwrap();
        run(&mut w, 5);
        assert!(crate_items(&w, 0, 24, 5).is_empty());
        assert_eq!(w.idle_reason(0, idx(20, 5)), Some("EC?"));
        // On a grid that's too small, it slows with everything else.
        w.place(0, 20, 9, Kind::Solar, Dir::E).unwrap();
        for (x, y) in footprint(18, 9, 6, 6) {
            if w.check_place(0, x, y, Kind::Belt).is_ok() {
                w.place(0, x, y, Kind::Belt, Dir::E).unwrap();
            }
        }
        let ratio = w.ratio_at(0, 20, 7);
        assert!(ratio < 1.0 && ratio > 0.0, "{ratio}");
        run(&mut w, 10);
        let n = crate_items(&w, 0, 24, 5).len() as f64;
        assert!((n - 30.0 * ratio).abs() <= 1.5, "{n} items at {ratio}");
    }

    #[test]
    fn a_large_basic_drill_mines_16x_and_burns_32x_the_coal() {
        let mut w = World::blank();
        assert_eq!((Kind::LargeBasicDrill.base_dims(), Kind::LargeBasicDrill.cost()), ((8, 8), 1000.0));
        // A basic drill and a large one, each fed all the coal they can hold.
        w.place(0, 5, 5, Kind::BasicDrill, Dir::E).unwrap();
        w.place(0, 7, 5, Kind::Crate, Dir::E).unwrap();
        w.place(0, 5, 20, Kind::LargeBasicDrill, Dir::E).unwrap(); // covers 5-12, 20-27; front is x=13
        drill_crates(&mut w, 13, 20, 8);
        for b in w.floors[0].tiles.iter_mut().flatten() {
            if let St::Drill { coal, .. } = &mut b.st {
                *coal = b.kind.drill_coal_cap();
            }
        }
        assert_eq!(Kind::LargeBasicDrill.drill_coal_cap(), 320);
        run(&mut w, 20);
        let basic = crate_items(&w, 0, 7, 5).len();
        let large = crates_total(&w, 13, 20, 8);
        assert_eq!(basic, 30, "1200 ticks at 40");
        assert_eq!(large, 16 * basic, "16 times the ore");
        // 32 times the coal: the basic drill burns 1 unit a tick (3 coal in 1200 ticks, one of
        // them the starter), the big one 32 a tick (96).
        let used = w.econ.used[Item::Coal.idx()] as f64;
        assert!((used - 98.0).abs() <= 2.0, "coal used {used}");
        // It pushes into everything along its front edge, in turn.
        for i in 0..8 {
            assert!(crate_items(&w, 0, 13, 20 + i).len() >= 50, "crate {i}");
        }
    }

    #[test]
    fn a_large_basic_drill_runs_dry_without_coal() {
        let mut w = World::blank();
        w.place(0, 5, 20, Kind::LargeBasicDrill, Dir::E).unwrap();
        drill_crates(&mut w, 13, 20, 8);
        run(&mut w, 2);
        // Only the starter fuel: 400 units is a bit over 12 ticks of work.
        let n = crates_total(&w, 13, 20, 8);
        assert!(n > 0 && n <= 6, "{n}");
        assert_eq!(w.idle_reason(0, idx(5, 20)), Some("COAL"));
    }

    #[test]
    fn a_large_advanced_drill_mines_16x_an_advanced_drill() {
        let mut w = World::blank();
        assert_eq!(
            (Kind::LargeAdvancedDrill.base_dims(), Kind::LargeAdvancedDrill.cost(), Kind::LargeAdvancedDrill.power_draw()),
            ((8, 8), 2000.0, 128.0)
        );
        // 130 EC of panels for the big one, 10 for the advanced one.
        big_grid(&mut w, 32, 0, 13);
        w.place(0, 34, 6, Kind::LargeAdvancedDrill, Dir::E).unwrap(); // covers 34-41, 6-13; front is x=42
        drill_crates(&mut w, 42, 6, 8);
        w.place(0, 5, 40, Kind::AdvancedDrill, Dir::E).unwrap();
        w.place(0, 9, 40, Kind::Crate, Dir::E).unwrap();
        w.place(0, 5, 44, Kind::Solar, Dir::E).unwrap();
        assert_eq!(w.ratio_at(0, 34, 6), 1.0);
        run(&mut w, 10);
        let adv = crate_items(&w, 0, 9, 40).len();
        let large = crates_total(&w, 42, 6, 8);
        assert_eq!(adv, 30);
        assert_eq!(large, 16 * adv, "16 times an advanced drill");
        assert_eq!(w.econ.used[Item::Coal.idx()], 0, "no coal anywhere");
    }

    #[test]
    fn an_advanced_pump_gives_four_times_the_water() {
        let mut w = World::blank();
        assert_eq!((Kind::AdvancedPump.base_dims(), Kind::AdvancedPump.cost(), Kind::AdvancedPump.power_draw()), ((2, 2), 240.0, 4.0));
        w.place(0, 5, 5, Kind::Pump, Dir::E).unwrap();
        w.place(0, 5, 20, Kind::AdvancedPump, Dir::E).unwrap();
        // Long pipes so neither network fills up.
        for x in 6..=45 {
            w.place(0, x, 5, Kind::Pipe, Dir::E).unwrap();
        }
        for x in 7..=46 {
            w.place(0, x, 20, Kind::Pipe, Dir::E).unwrap();
        }
        w.place(0, 5, 8, Kind::Solar, Dir::E).unwrap();
        w.place(0, 5, 23, Kind::Solar, Dir::E).unwrap();
        run(&mut w, 5);
        let water = |w: &World, x: i32, y: i32| {
            let p = w.fluids.pipe_pool[0][idx(x, y)];
            w.fluids.pools[p as usize].amount
        };
        let (small, big) = (water(&w, 6, 5), water(&w, 7, 20));
        assert!((small - 100.0).abs() < 1e-6, "{small}");
        assert!((big - 400.0).abs() < 1e-6, "{big}");
    }

    #[test]
    fn new_machines_say_whether_they_run_on_ec() {
        for (k, electric) in [
            (Kind::Welder, true),
            (Kind::Kiln, false),
            (Kind::Mixer, true),
            (Kind::LargeBasicDrill, false),
            (Kind::LargeAdvancedDrill, true),
        ] {
            assert_eq!(k.electric(), electric, "{k:?}");
        }
        assert_eq!(Kind::Kiln.cost(), 200.0);
        assert_eq!((Kind::Mixer.cost(), Kind::Mixer.power_draw(), Kind::Mixer.base_dims()), (200.0, 4.0, (2, 2)));
        assert_eq!((Kind::Welder.cost(), Kind::Welder.base_dims()), (800.0, (3, 3)));
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
        assert_eq!(w.idle_reason(0, idx(5, 5)), Some("EC?"));
        w.place(0, 8, 7, Kind::Solar, Dir::E).unwrap();
        w.tick();
        assert_ne!(w.idle_reason(0, idx(5, 5)), Some("EC?"));
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
    fn deliveries_and_short_sessions_leave_the_offline_rate_alone() {
        // Earning $1 a second for two minutes.
        let mut w = World::new();
        for _ in 0..120 {
            w.econ.earned_money += 1.0;
            w.sample_earnings();
        }
        assert!((w.rate.0 - 1.0).abs() < 1e-9, "{:?}", w.rate);
        // A company delivery is not earnings the factory keeps making while closed.
        w.receive(crate::mail::parse("done", 0), 1_000);
        assert!(w.gifted > 0.0);
        w.econ.earned_money += 1.0;
        w.sample_earnings();
        assert!((w.rate.0 - 1.0).abs() < 1e-9, "{:?}", w.rate);

        // A fresh session (samples aren't saved) of a few fast seconds keeps the saved rate.
        let mut w2 = World::new();
        w2.rate = (1.0, 0.0);
        for _ in 0..5 {
            w2.econ.earned_money += 5.0;
            w2.sample_earnings();
        }
        assert_eq!(w2.rate.0, 1.0);
    }

    #[test]
    fn offline_pay_uses_the_better_of_recent_and_long_rates() {
        let mut w = World::new();
        w.rate = (0.0, 0.0); // a jam right before quitting
        w.rate_long = (2.0, 0.0);
        w.saved_at = 1_000;
        let (_, m, _) = w.apply_offline(1_000 + 3600).unwrap();
        assert_eq!(m, 7_200.0);
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

    #[test]
    fn saves_round_trip_with_the_full_item_list() {
        let mut w = World::blank();
        let last = Item::ALL[ITEMS - 1];
        assert_eq!(last, Item::SteamTractor);
        assert!(ITEMS > 32, "longer than serde's own array limit");
        w.econ.made[last.idx()] = 7;
        w.econ.sold[last.idx()] = 5;
        w.econ.research_sales[last.idx()] = 3;
        w.econ.made[Item::Gauge.idx()] = 11;
        w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
        w.place(0, 8, 5, Kind::LargeAdvancedAssembler, Dir::E).unwrap();
        set_stock(&mut w, 8, 5, last, 4);
        set_stock(&mut w, 8, 5, Item::ControlPanel, 2);
        fill_crate(&mut w, 0, 5, 5, &[last, Item::PumpUnit]);
        w.progress.researched[Tech::ALL[TECHS - 1].idx()] = true;
        w.progress.opened[Company::ALL[COMPANIES - 1].idx()] = true;
        let path = std::env::temp_dir().join(format!("factory-test-items-{}.json", std::process::id()));
        w.save(&path).unwrap();
        // The file holds a whole list per item array.
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(json["econ"]["made"].as_array().unwrap().len(), ITEMS);
        assert_eq!(json["version"], SAVE_VERSION);
        let l = World::load(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!((l.econ.made[last.idx()], l.econ.sold[last.idx()], l.econ.research_sales[last.idx()]), (7, 5, 3));
        assert_eq!(l.econ.made[Item::Gauge.idx()], 11);
        match &l.tile(0, 8, 5).unwrap().st {
            St::Crafter { stock, .. } => assert_eq!((stock[last.idx()], stock[Item::ControlPanel.idx()]), (4, 2)),
            _ => panic!("not a machine"),
        }
        assert_eq!(crate_items(&l, 0, 5, 5), vec![last, Item::PumpUnit]);
        assert!(l.progress.has(Tech::ALL[TECHS - 1]) && l.progress.is_open(Company::ALL[COMPANIES - 1]));
    }

    // ---------- the iron and coal step ----------

    /// Picks a machine's recipe by name (cycling with f).
    fn cycle_to(w: &mut World, x: i32, y: i32, name: &str) {
        for _ in 0..12 {
            if w.tile(0, x, y).and_then(|b| b.recipe()).is_some_and(|r| r.name == name) {
                return;
            }
            w.configure(0, x, y);
        }
        panic!("no recipe called {name}");
    }

    /// A machine facing east with a recipe, its inputs in stock, a panel beside it and a
    /// crate at its front. Runs it and returns what landed in the crate.
    fn run_machine(kind: Kind, recipe: &str, stock: &[(Item, u32)], secs: u64) -> Vec<Item> {
        let mut w = World::blank();
        let (x, y) = (10, 10);
        w.place(0, x, y, kind, Dir::E).unwrap();
        let (mw, mh) = kind.dims(Dir::E);
        cycle_to(&mut w, x, y, recipe);
        for (item, n) in stock {
            set_stock(&mut w, x, y, *item, *n);
        }
        w.place(0, x + mw, y, Kind::Crate, Dir::E).unwrap();
        w.place(0, x, y + mh, Kind::Solar, Dir::E).unwrap();
        run(&mut w, secs);
        crate_items(&w, 0, x + mw, y)
    }

    fn count(items: &[Item], it: Item) -> usize {
        items.iter().filter(|i| **i == it).count()
    }

    #[test]
    fn new_items_have_the_asked_prices_and_curves() {
        let prices = [
            (Item::CopperGrit, 0.8),
            (Item::IronGrit, 1.2),
            (Item::Sand, 0.25),
            (Item::FineWire, 2.5),
            (Item::CopperSheet, 12.0),
            (Item::IronRod, 5.0),
            (Item::Screw, 4.0),
            (Item::Spring, 15.0),
            (Item::IronSheet, 18.0),
            (Item::CopperWire, 3.0),
            (Item::CopperPipe, 12.0),
            (Item::CopperCoil, 18.0),
            (Item::IronGear, 18.0),
            (Item::Coke, 2.5),
        ];
        for (item, price) in prices {
            assert_eq!(item.price(), price, "{item:?}");
        }
        for item in &Item::ALL[19..] {
            let worth = item.curve().worth;
            assert!(worth >= 10.0 && worth >= 2.0 * item.price(), "{item:?} worth {worth}");
            assert!(worth <= 2.0 * item.price().max(5.0), "{item:?} worth {worth} is too generous");
        }
    }

    #[test]
    fn new_machines_have_the_asked_size_power_and_cost() {
        let table = [
            (Kind::CokeOven, (2, 2), 0.0, 150.0),
            (Kind::Crusher, (2, 2), 6.0, 250.0),
            (Kind::WireDrawer, (2, 1), 4.0, 250.0),
            (Kind::RollingMill, (2, 2), 8.0, 400.0),
            (Kind::Lathe, (3, 3), 4.0, 200.0),
            (Kind::Winder, (2, 1), 2.0, 200.0),
        ];
        for (k, dims, power, cost) in table {
            assert_eq!((k.base_dims(), k.power_draw(), k.cost()), (dims, power, cost), "{k:?}");
            assert_eq!(k.category(), Category::Create);
        }
        // A long one turns with its facing.
        assert_eq!(Kind::WireDrawer.dims(Dir::N), (1, 2));
        assert_eq!(Kind::Lathe.dims(Dir::N), (3, 3));
    }

    #[test]
    fn crusher_grinds_ore_and_stone() {
        let out = run_machine(Kind::Crusher, "copper grit", &[(Item::CopperOre, 3)], 13);
        assert_eq!(out, vec![Item::CopperGrit; 6]);
        let out = run_machine(Kind::Crusher, "iron grit", &[(Item::IronOre, 2)], 9);
        assert_eq!(out, vec![Item::IronGrit; 4]);
        let out = run_machine(Kind::Crusher, "sand", &[(Item::Stone, 3)], 8);
        assert_eq!(out, vec![Item::Sand; 6]);
        // No EC, no grit.
        let mut w = World::blank();
        w.place(0, 10, 10, Kind::Crusher, Dir::E).unwrap();
        set_stock(&mut w, 10, 10, Item::CopperOre, 3);
        w.place(0, 12, 10, Kind::Crate, Dir::E).unwrap();
        run(&mut w, 10);
        assert!(crate_items(&w, 0, 12, 10).is_empty());
    }

    /// Ore in, ingots out: (crusher ->) furnace -> molder -> crate, with coal and a panel.
    fn ingots_from(ore: u32, crush: bool) -> usize {
        let mut w = World::blank();
        let mut x = 10;
        if crush {
            w.place(0, x, 10, Kind::Crusher, Dir::E).unwrap(); // covers 10-11, 10-11
            cycle_to(&mut w, x, 10, "copper grit");
            set_stock(&mut w, x, 10, Item::CopperOre, ore);
            x += 2;
        }
        w.place(0, x, 10, Kind::Furnace, Dir::E).unwrap(); // covers x..x+2, 10-12
        cycle_to(&mut w, x, 10, if crush { "molten copper (grit)" } else { "molten copper" });
        if !crush {
            set_stock(&mut w, x, 10, Item::CopperOre, ore);
        }
        set_stock(&mut w, x, 10, Item::Coal, 20);
        w.place(0, x + 3, 10, Kind::Molder, Dir::E).unwrap();
        w.place(0, x + 5, 10, Kind::Crate, Dir::E).unwrap();
        w.place(0, x, 13, Kind::Solar, Dir::E).unwrap(); // reaches the crusher and the molder
        run(&mut w, 90);
        count(&crate_items(&w, 0, x + 5, 10), Item::CopperIngot)
    }

    #[test]
    fn crushed_copper_gives_a_third_more_ingots() {
        assert_eq!(ingots_from(6, false), 6);
        assert_eq!(ingots_from(6, true), 8);
    }

    #[test]
    fn furnace_and_blast_furnace_melt_grit() {
        // 2 iron grit make 1 molten iron in the furnace.
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Furnace, Dir::E).unwrap();
        cycle_to(&mut w, 5, 5, "molten iron (grit)");
        set_stock(&mut w, 5, 5, Item::IronGrit, 6);
        set_stock(&mut w, 5, 5, Item::Coal, 10);
        w.place(0, 8, 5, Kind::Molder, Dir::E).unwrap();
        cycle_to(&mut w, 8, 5, "iron ingot");
        w.place(0, 10, 5, Kind::Crate, Dir::E).unwrap();
        w.place(0, 8, 7, Kind::Solar, Dir::E).unwrap();
        run(&mut w, 40);
        assert_eq!(crate_items(&w, 0, 10, 5), vec![Item::IronIngot; 3]);
        // The blast furnace gets 2 molten iron from 3 grit.
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::BlastFurnace, Dir::E).unwrap(); // covers 5-6, 5-6
        cycle_to(&mut w, 5, 5, "molten iron (grit)");
        set_stock(&mut w, 5, 5, Item::IronGrit, 6);
        set_stock(&mut w, 5, 5, Item::Coke, 4);
        w.place(0, 7, 5, Kind::Molder, Dir::E).unwrap();
        cycle_to(&mut w, 7, 5, "iron ingot");
        w.place(0, 9, 5, Kind::Crate, Dir::E).unwrap();
        w.place(0, 7, 7, Kind::Solar, Dir::E).unwrap();
        run(&mut w, 30);
        assert_eq!(crate_items(&w, 0, 9, 5), vec![Item::IronIngot; 4]);
    }

    #[test]
    fn coke_oven_makes_coke_and_waste_with_no_fuel_or_ec() {
        let mut w = World::blank();
        w.place(0, 10, 10, Kind::CokeOven, Dir::E).unwrap();
        set_stock(&mut w, 10, 10, Item::Coal, 6);
        w.place(0, 12, 10, Kind::Crate, Dir::E).unwrap();
        run(&mut w, 7); // 2 s a batch
        let out = crate_items(&w, 0, 12, 10);
        assert_eq!((count(&out, Item::Coke), count(&out, Item::Waste)), (3, 3));
        assert_eq!(w.econ.used[Item::Coal.idx()], 6);
        assert_eq!(Kind::CokeOven.recipes().len(), 1);
    }

    #[test]
    fn furnace_can_cook_fast_and_dirty_coke() {
        let out = run_machine(Kind::Furnace, "coke (fast)", &[(Item::Coal, 14)], 6);
        assert_eq!((count(&out, Item::Coke), count(&out, Item::Waste)), (4, 6));
        // The slow recipe stays: 3 coal, 1 coke, no waste.
        let out = run_machine(Kind::Furnace, "coke", &[(Item::Coal, 6)], 11);
        assert_eq!((count(&out, Item::Coke), count(&out, Item::Waste)), (2, 0));
    }

    #[test]
    fn wire_drawer_draws_three_wire_per_plate_and_fine_wire() {
        let out = run_machine(Kind::WireDrawer, "copper wire", &[(Item::CopperPlate, 2)], 9);
        assert_eq!(out, vec![Item::CopperWire; 6]);
        let out = run_machine(Kind::WireDrawer, "fine wire", &[(Item::CopperWire, 2)], 9);
        assert_eq!(out, vec![Item::FineWire; 4]);
    }

    #[test]
    fn press_wire_pipe_coil_and_gear_times() {
        let out = run_machine(Kind::Press, "copper wire", &[(Item::CopperPlate, 1)], 6);
        assert_eq!(out, vec![Item::CopperWire; 2]);
        assert!(run_machine(Kind::Press, "copper wire", &[(Item::CopperPlate, 1)], 4).is_empty(), "5 s a batch");
        assert_eq!(run_machine(Kind::Press, "copper pipe", &[(Item::CopperPlate, 4)], 6), vec![Item::CopperPipe; 2]);
        assert_eq!(run_machine(Kind::Press, "iron gear", &[(Item::IronPlate, 4)], 6), vec![Item::IronGear; 2]);
        assert_eq!(run_machine(Kind::Press, "copper coil", &[(Item::CopperWire, 4)], 6), vec![Item::CopperCoil]);
        // Five fine wire wind into a coil in half the time.
        let fine = "copper coil (fine wire)";
        assert_eq!(run_machine(Kind::Press, fine, &[(Item::FineWire, 5)], 3), vec![Item::CopperCoil]);
        assert!(run_machine(Kind::Press, fine, &[(Item::FineWire, 5)], 2).is_empty());
    }

    #[test]
    fn rolling_mill_rolls_sheets() {
        assert_eq!(run_machine(Kind::RollingMill, "copper sheet", &[(Item::CopperPlate, 4)], 11), vec![Item::CopperSheet; 2]);
        assert_eq!(run_machine(Kind::RollingMill, "iron sheet", &[(Item::IronPlate, 4)], 6), vec![Item::IronSheet; 2]);
        // Steel: a sheet in 5 s, and 3 steel into 2 girders in 4 s.
        assert_eq!(run_machine(Kind::RollingMill, "steel sheet", &[(Item::Steel, 2)], 11), vec![Item::SteelSheet; 2]);
        assert!(run_machine(Kind::RollingMill, "steel sheet", &[(Item::Steel, 1)], 4).is_empty());
        assert_eq!(run_machine(Kind::RollingMill, "steel girder", &[(Item::Steel, 3)], 5), vec![Item::SteelGirder; 2]);
        assert!(run_machine(Kind::RollingMill, "steel girder", &[(Item::Steel, 3)], 3).is_empty());
    }

    #[test]
    fn the_press_makes_girders_and_hardened_gears_and_no_longer_gearboxes() {
        assert_eq!(run_machine(Kind::Press, "steel girder", &[(Item::Steel, 4)], 11), vec![Item::SteelGirder; 2]);
        assert!(run_machine(Kind::Press, "steel girder", &[(Item::Steel, 2)], 4).is_empty(), "5 s a girder");
        assert_eq!(run_machine(Kind::Press, "hardened gear", &[(Item::HardenedIronPlate, 2)], 6), vec![Item::HardenedGear]);
        assert!(!recipe_names(Kind::Press).contains(&"gearbox"), "the gearbox moved to the crude assembler");
        assert!(recipe_names(Kind::CrudeAssembler).contains(&"gearbox"));
    }

    #[test]
    fn lathe_turns_rods_then_screws() {
        let rods = run_machine(Kind::Lathe, "iron rod", &[(Item::IronPlate, 2)], 11);
        assert_eq!(rods, vec![Item::IronRod; 4]);
        let screws = run_machine(Kind::Lathe, "screw", &[(Item::IronRod, 2)], 11);
        assert_eq!(screws, vec![Item::Screw; 8]);
    }

    #[test]
    fn lathe_turns_steel_into_rods_then_bearings() {
        // 5 s for two steel rods; 2.5 s for two bearings from one rod.
        assert_eq!(run_machine(Kind::Lathe, "steel rod", &[(Item::Steel, 1)], 6), vec![Item::SteelRod; 2]);
        assert!(run_machine(Kind::Lathe, "steel rod", &[(Item::Steel, 1)], 4).is_empty());
        assert_eq!(run_machine(Kind::Lathe, "bearing", &[(Item::SteelRod, 2)], 6), vec![Item::Bearing; 4]);
        assert_eq!(run_machine(Kind::Lathe, "bearing", &[(Item::SteelRod, 1)], 3), vec![Item::Bearing; 2]);
    }

    #[test]
    fn winder_winds_coils_and_springs() {
        assert_eq!(run_machine(Kind::Winder, "copper coil", &[(Item::CopperWire, 6)], 9), vec![Item::CopperCoil; 2]);
        assert_eq!(run_machine(Kind::Winder, "spring", &[(Item::IronRod, 4)], 11), vec![Item::Spring; 2]);
    }

    #[test]
    fn new_machines_are_locked_until_their_company_opens_and_they_are_researched() {
        let table = [
            (Kind::Crusher, Company::Ashgrove, Tech::Crusher, Item::CopperWire),
            (Kind::CokeOven, Company::Cinderhall, Tech::CokeOven, Item::Coke),
            (Kind::WireDrawer, Company::Kettering, Tech::WireDrawer, Item::CopperCoil),
            (Kind::Winder, Company::Kettering, Tech::Winder, Item::CopperCoil),
            (Kind::Lathe, Company::Gearwright, Tech::Lathe, Item::IronGear),
            (Kind::RollingMill, Company::Pillar, Tech::RollingMill, Item::SteelGirder),
            (Kind::Welder, Company::Pillar, Tech::Welder, Item::SteelGirder),
            (Kind::Mixer, Company::Wellbourne, Tech::Mixer, Item::CopperPipe),
            (Kind::AdvancedPump, Company::Wellbourne, Tech::AdvancedPump, Item::CopperPipe),
            (Kind::Kiln, Company::Underhill, Tech::Kiln, Item::Concrete),
            (Kind::BasicAssembler, Company::Swiftline, Tech::BasicAssembler, Item::Gearbox),
            (Kind::AdvancedDrill, Company::Ampleworth, Tech::AdvancedDrill, Item::ElectricMotor),
            (Kind::AdvancedAssembler, Company::Ampleworth, Tech::AdvancedAssembler, Item::ElectricMotor),
            (Kind::LargeBasicAssembler, Company::Ampleworth, Tech::LargeBasicAssembler, Item::ElectricMotor),
            (Kind::LargeBasicDrill, Company::Ampleworth, Tech::LargeBasicDrill, Item::ElectricMotor),
            (Kind::PrecisionAssembler, Company::LatticeFinch, Tech::PrecisionAssembler, Item::ControlBoard),
            (Kind::LargeAdvancedAssembler, Company::LatticeFinch, Tech::LargeAdvancedAssembler, Item::ControlBoard),
            (Kind::LargeAdvancedDrill, Company::LatticeFinch, Tech::LargeAdvancedDrill, Item::ControlBoard),
        ];
        for (kind, company, tech, key) in table {
            assert_eq!((kind.company(), kind.tech(), tech.company(), company.key_item()), (Some(company), Some(tech), company, Some(key)));
            let mut w = World::new();
            assert!(!w.progress.part_unlocked(kind), "{kind:?} locked at the start");
            assert!(w.research(tech).unwrap_err().contains(key.name()), "{tech:?} needs {key:?} first");
            // Making the key item opens the company; the part still needs its tech.
            w.econ.made[key.idx()] = 1;
            w.tick();
            assert!(w.progress.is_open(company));
            assert!(!w.progress.part_unlocked(kind), "{kind:?} still needs research");
            assert!(w.check_place(0, 30, 30, kind).unwrap_err().contains("locked"));
            w.econ.research = tech.cost();
            w.research(tech).unwrap();
            assert!(w.progress.part_unlocked(kind));
            w.econ.money = 1e6;
            assert!(w.check_place(0, 30, 30, kind).is_ok(), "{kind:?}");
        }
        // Or the milestone does it for free.
        let mut w = World::new();
        let (item, n) = Tech::Crusher.milestone();
        w.econ.made[Item::CopperWire.idx()] = 1;
        w.tick();
        assert!(!w.progress.has(Tech::Crusher));
        w.econ.made[item.idx()] = n;
        w.tick();
        assert!(w.progress.has(Tech::Crusher));
    }

    #[test]
    fn every_machine_can_be_built_once_unlocked() {
        // Nothing is hidden: every machine is owned by a company and a tech (or is a starter).
        let mut w = World::new();
        w.econ.money = 1e6;
        w.progress.opened = [true; COMPANIES];
        w.progress.researched = [true; TECHS];
        for k in Kind::ALL {
            assert!(w.progress.part_unlocked(k), "{k:?}");
            assert!(w.check_place(0, 30, 30, k).is_ok(), "{k:?}");
        }
        // A new game can build none of the twelve late machines.
        let fresh = World::new();
        for k in [
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
        ] {
            assert!(!fresh.progress.part_unlocked(k), "{k:?}");
            assert!(k.tech().is_some() && k.company().is_some());
        }
        assert_eq!(Kind::CrudeAssembler.tech(), Some(Tech::Assembler));
    }

    #[test]
    fn new_techs_are_priced_by_depth() {
        let copper = [(Tech::Crusher, 150.0), (Tech::Winder, 200.0), (Tech::WireDrawer, 250.0)];
        for (t, cost) in copper {
            assert_eq!(t.cost(), cost);
            assert!((50.0..=300.0).contains(&t.cost()), "{t:?}");
        }
        for t in [Tech::CokeOven, Tech::Lathe, Tech::RollingMill] {
            assert!((300.0..=1500.0).contains(&t.cost()), "{t:?}");
        }
        // Steel and stone machines, then motor machines, then processor machines.
        for t in [Tech::Mixer, Tech::Kiln, Tech::AdvancedPump, Tech::Welder, Tech::BasicAssembler] {
            assert!((1500.0..=6000.0).contains(&t.cost()), "{t:?}");
        }
        for t in [Tech::AdvancedDrill, Tech::AdvancedAssembler, Tech::LargeBasicDrill, Tech::LargeBasicAssembler] {
            assert!((6000.0..=25_000.0).contains(&t.cost()), "{t:?}");
            assert_eq!(t.company(), Company::Ampleworth);
        }
        for t in [Tech::PrecisionAssembler, Tech::LargeAdvancedDrill, Tech::LargeAdvancedAssembler] {
            assert!((25_000.0..=100_000.0).contains(&t.cost()), "{t:?}");
            assert_eq!(t.company(), Company::LatticeFinch);
        }
        // Every tech sits in the research list and in the tree once, and has a free milestone.
        for t in Tech::ALL {
            let n: usize = UNLOCK_GROUPS.iter().map(|g| g.1.iter().filter(|x| **x == t).count()).sum();
            assert_eq!(n, 1, "{t:?}");
            assert!(t.milestone().1 > 0 && !t.about().is_empty());
        }
        for t in Tech::ALL.into_iter().filter(|t| !matches!(t, Tech::Tunnels | Tech::LongArms | Tech::FilterSplitters | Tech::Warehouses)) {
            assert!(TREE.iter().any(|(node, _)| *node == Node::Tech(t)), "{t:?} is missing from the tree");
        }
        // The tree's links point backwards, and every company's key item is a node.
        for (i, (_, parent)) in TREE.iter().enumerate() {
            assert!(parent.map_or(i == 0, |p| p < i), "node {i}");
        }
        for c in Company::ALL {
            if let Some(key) = c.key_item() {
                assert!(TREE.iter().any(|(n, _)| *n == Node::Key(key)), "{c:?}'s {key:?} is not in the tree");
            }
        }
    }

    /// Which items can be made once these companies are open, taking every tech of an open
    /// company as researchable (techs can always be bought or earned). Pumps give water.
    fn makeable(open: &[bool; COMPANIES]) -> ([bool; ITEMS], Vec<Fluid>) {
        let tech_ok = |t: Tech| open[t.company().idx()];
        let kind_ok = |k: Kind| k.company().map_or(true, |c| open[c.idx()]) && k.tech().map_or(true, tech_ok);
        let mut have = [false; ITEMS];
        let mut fluids: Vec<Fluid> = Vec::new();
        loop {
            let before = (have, fluids.len());
            for k in Kind::ALL.into_iter().filter(|k| kind_ok(*k)) {
                for ore in k.ores() {
                    have[ore.idx()] = true;
                }
                if k.is_pump() && !fluids.contains(&Fluid::Water) {
                    fluids.push(Fluid::Water);
                }
                for r in k.recipes().iter().filter(|r| r.needs.map_or(true, tech_ok)) {
                    let ok = r.inputs.iter().all(|(i, _)| have[i.idx()])
                        && r.fuel.map_or(true, |f| have[f.idx()])
                        && r.fluid_in.map_or(true, |(f, _)| fluids.contains(&f));
                    if ok {
                        for (i, _) in r.outputs {
                            have[i.idx()] = true;
                        }
                        if let Some((f, _)) = r.fluid_out.filter(|(f, _)| !fluids.contains(f)) {
                            fluids.push(f);
                        }
                    }
                }
            }
            if (have, fluids.len()) == before {
                return (have, fluids);
            }
        }
    }

    /// Walks the key items: each company opens once its key item can be made with what the
    /// companies opened so far offer. No chain may lock, and the order must make sense.
    #[test]
    fn every_key_item_can_be_made_before_its_company_opens() {
        let mut open = [false; COMPANIES];
        for c in Company::ALL {
            open[c.idx()] = c.key_item().is_none();
        }
        let mut order: Vec<Company> = Vec::new();
        loop {
            let (have, _) = makeable(&open);
            let next: Vec<Company> =
                Company::ALL.into_iter().filter(|c| !open[c.idx()] && c.key_item().is_some_and(|k| have[k.idx()])).collect();
            if next.is_empty() {
                break;
            }
            for c in next {
                open[c.idx()] = true;
                order.push(c);
            }
        }
        println!("companies open in this order: {order:?}");
        let stuck: Vec<Company> = Company::ALL.into_iter().filter(|c| !open[c.idx()]).collect();
        assert!(stuck.is_empty(), "these companies can never open: {stuck:?} (opened in order {order:?})");
        let at = |c: Company| order.iter().position(|x| *x == c).unwrap();
        // Copper first: the press makes wire, pipe and coil, the furnace makes coke.
        for c in [Company::Ashgrove, Company::Wellbourne, Company::Kettering, Company::Cinderhall] {
            assert!(at(c) < at(Company::Gearwright), "{c:?} before Gearwright");
        }
        // Gears, girders and steel all need the blast furnace's tech, so Cinderhall first.
        for c in [Company::Gearwright, Company::Pillar, Company::Swiftline] {
            assert!(at(Company::Cinderhall) < at(c), "Cinderhall before {c:?}");
        }
        // Concrete needs the mixer (Wellbourne) and sand from the crusher (Ashgrove).
        assert!(at(Company::Wellbourne) < at(Company::Underhill) && at(Company::Ashgrove) < at(Company::Underhill));
        // A gearbox is built in the crude assembler (Gearwright), a motor in the basic one
        // (Swiftline), a control board in the advanced one (Ampleworth).
        assert!(at(Company::Gearwright) < at(Company::Swiftline));
        assert!(at(Company::Swiftline) < at(Company::Ampleworth));
        assert!(at(Company::Ampleworth) < at(Company::LatticeFinch));
        assert_eq!(Company::Gearwright.key_item(), Some(Item::IronGear));
        assert_eq!(Company::Swiftline.key_item(), Some(Item::Gearbox));
        assert_eq!(Company::Underhill.key_item(), Some(Item::Concrete));
        assert_eq!(Company::Ampleworth.key_item(), Some(Item::ElectricMotor));
        assert_eq!(Company::LatticeFinch.key_item(), Some(Item::ControlBoard));
        // With everything open, every item has a way to be made.
        let (have, fluids) = makeable(&open);
        for it in Item::ALL {
            assert!(have[it.idx()], "{it:?} can't be made");
        }
        assert!(fluids.contains(&Fluid::MoltenIron) && fluids.contains(&Fluid::MoltenSteel) && fluids.contains(&Fluid::Water));
    }


    #[test]
    fn a_company_never_gates_its_own_key_item() {
        // With every other company open, a key item is still makeable without its own company.
        for c in Company::ALL {
            let Some(key) = c.key_item() else { continue };
            let mut open = [true; COMPANIES];
            open[c.idx()] = false;
            let (have, _) = makeable(&open);
            assert!(have[key.idx()], "{key:?} needs {c:?}'s machines");
        }
    }

    // ---------- the content step: steel, stone, parts and big goods ----------

    /// Like `run_machine`, on a big grid (90 EC) so even the thirsty machines run at full speed.
    fn run_powered(kind: Kind, recipe: &str, stock: &[(Item, u32)], secs: u64) -> Vec<Item> {
        let mut w = World::blank();
        big_grid(&mut w, 10, 0, 9);
        let (x, y) = (12, 12);
        w.place(0, x, y, kind, Dir::E).unwrap();
        pick_recipe(&mut w, x, y, recipe);
        for (item, n) in stock {
            set_stock(&mut w, x, y, *item, *n);
        }
        w.place(0, x + kind.dims(Dir::E).0, y, Kind::Crate, Dir::E).unwrap();
        assert_eq!(w.ratio_at(0, x, y), 1.0, "{kind:?} is on the grid");
        run(&mut w, secs);
        crate_items(&w, 0, x + kind.dims(Dir::E).0, y)
    }

    /// A blast furnace set to `recipe`, with its stock; molten steel runs into a molder
    /// (waste into a crate beside it). Returns (steel, waste) after `secs`.
    fn steel_route(recipe: &str, stock: &[(Item, u32)], secs: u64) -> (usize, usize) {
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::BlastFurnace, Dir::E).unwrap(); // covers 5-6, 5-6: front is x=7
        pick_recipe(&mut w, 5, 5, recipe);
        for (item, n) in stock {
            set_stock(&mut w, 5, 5, *item, *n);
        }
        w.place(0, 7, 5, Kind::Crate, Dir::E).unwrap(); // the waste
        w.place(0, 7, 6, Kind::Molder, Dir::E).unwrap(); // covers 7-8, 6-7, on the furnace's other front tile
        pick_recipe(&mut w, 7, 6, "steel");
        w.place(0, 9, 6, Kind::Crate, Dir::E).unwrap(); // the steel
        w.place(0, 7, 8, Kind::Solar, Dir::E).unwrap();
        run(&mut w, secs);
        (crate_items(&w, 0, 9, 6).iter().filter(|i| **i == Item::Steel).count(), crate_items(&w, 0, 7, 5).iter().filter(|i| **i == Item::Waste).count())
    }

    #[test]
    fn the_blast_furnace_makes_steel_four_ways_each_with_its_waste() {
        let plates = (Item::IronPlate, 2);
        // 2 iron plate + 1 coke: 10 s of melting, 2.5 s of casting, 1 waste.
        let route = [
            ("molten steel", vec![plates, (Item::Coke, 1)], 10, 1),
            ("molten steel (coal)", vec![plates, (Item::Coal, 2)], 10, 2),
            ("molten steel (fast)", vec![plates, (Item::Coke, 2)], 5, 2),
            ("molten steel (fast, coal)", vec![plates, (Item::Coal, 4)], 5, 4),
        ];
        for (name, stock, melt, waste) in route {
            let r = Kind::BlastFurnace.recipes().iter().find(|r| r.name == name).unwrap();
            assert_eq!(r.ticks as u64, melt * TPS, "{name}");
            assert_eq!(r.fluid_out, Some((Fluid::MoltenSteel, 1)), "{name}");
            assert_eq!(r.outputs, &[(Item::Waste, waste as u32)], "{name}");
            // Still melting just before the time is up, all done (cast, with its waste) a bit after.
            assert_eq!(steel_route(name, &stock, melt - 1), (0, 0), "{name} early");
            assert_eq!(steel_route(name, &stock, melt + 1), (0, waste), "{name} waste first, steel still casting");
            assert_eq!(steel_route(name, &stock, melt + 4), (1, waste), "{name}");
            // Two batches' worth of stock makes two.
            let double: Vec<(Item, u32)> = stock.iter().map(|(i, n)| (*i, n * 2)).collect();
            assert_eq!(steel_route(name, &double, 2 * melt + 4), (2, waste * 2), "{name} twice");
        }
        // Without the coke or coal, it waits.
        assert_eq!(steel_route("molten steel", &[plates], 30), (0, 0));
        assert_eq!(steel_route("molten steel (fast)", &[plates, (Item::Coke, 1)], 30), (0, 0), "the fast way needs 2 coke");
        // The old steel recipe is gone.
        assert!(!recipe_names(Kind::BlastFurnace).contains(&"steel"));
        // A molder casts 1 molten steel into 1 steel in 2.5 s.
        assert_eq!(recipe_ticks(Kind::Molder, "steel"), 150);
        assert_eq!(Item::Steel.price(), 24.0);
    }

    #[test]
    fn the_blast_furnace_hardens_plate_with_coke_or_coal() {
        let out = run_machine(Kind::BlastFurnace, "hardened iron plate", &[(Item::IronPlate, 2), (Item::Coke, 2)], 11);
        assert_eq!(out, vec![Item::HardenedIronPlate; 2], "5 s each, no waste");
        assert!(run_machine(Kind::BlastFurnace, "hardened iron plate", &[(Item::IronPlate, 1), (Item::Coke, 1)], 4).is_empty());
        let out = run_machine(Kind::BlastFurnace, "hardened iron plate (coal)", &[(Item::IronPlate, 2), (Item::Coal, 4)], 11);
        assert_eq!((count(&out, Item::HardenedIronPlate), count(&out, Item::Waste)), (2, 2));
        let out = run_machine(Kind::BlastFurnace, "hardened iron plate (coal)", &[(Item::IronPlate, 1), (Item::Coal, 1)], 11);
        assert!(out.is_empty(), "2 coal each");
        assert_eq!(Item::HardenedIronPlate.price(), 20.0);
    }

    /// Every recipe that takes no fluid runs in its machine: a batch of inputs in, its outputs
    /// out in the recipe's time (assemblers at every tier's speed), with the inputs used up.
    /// Recipes that give fluid hold it at the front.
    #[test]
    fn every_item_recipe_runs_in_its_machine() {
        let mut checked = 0;
        for kind in Kind::ALL {
            for r in kind.recipes().iter().filter(|r| r.fluid_in.is_none()) {
                let batch = kind.batch();
                let mut w = World::blank();
                big_grid(&mut w, 10, 0, 9);
                let (x, y) = (12, 12);
                w.place(0, x, y, kind, Dir::E).unwrap();
                pick_recipe(&mut w, x, y, r.name);
                for (it, n) in r.inputs {
                    set_stock(&mut w, x, y, *it, n * batch);
                }
                if let Some(f) = r.fuel {
                    set_stock(&mut w, x, y, f, 10);
                }
                let front = x + kind.dims(Dir::E).0;
                w.place(0, front, y, Kind::Crate, Dir::E).unwrap();
                // Just short of the recipe's time nothing has come out; shortly after, it all has.
                for _ in 0..r.ticks - 3 {
                    w.tick();
                }
                assert!(crate_items(&w, 0, front, y).is_empty(), "{kind:?} {} early", r.name);
                for _ in 0..13 {
                    w.tick();
                }
                let out = crate_items(&w, 0, front, y);
                for (it, n) in r.outputs {
                    assert_eq!(count(&out, *it) as u32, n * batch, "{kind:?} {}: {out:?}", r.name);
                }
                if let Some((_, n)) = r.fluid_out {
                    assert_eq!(crafter_fluid(&w, x, y).1, n * batch, "{kind:?} {} holds its fluid", r.name);
                }
                for (it, n) in r.inputs {
                    assert_eq!(w.econ.used[it.idx()] as u32, n * batch, "{kind:?} {} used {it:?}", r.name);
                }
                checked += 1;
            }
        }
        assert!(checked > 60, "{checked} recipes checked");
    }

    #[test]
    fn welder_parts() {
        let recipes: [(&str, &[(Item, u32)], Item); 3] = [
            ("boiler shell", &[(Item::SteelSheet, 1), (Item::Brick, 4)], Item::BoilerShell),
            ("silo shell", &[(Item::SteelSheet, 3), (Item::Valve, 1)], Item::SiloShell),
            ("frame", &[(Item::SteelGirder, 2), (Item::Screw, 8)], Item::Frame),
        ];
        for (name, stock, out) in recipes {
            assert_eq!(run_powered(Kind::Welder, name, stock, 11), vec![out], "{name}");
            assert!(run_powered(Kind::Welder, name, stock, 9).is_empty(), "{name} takes 10 s");
        }
    }

    #[test]
    fn assemblers_build_their_parts_and_the_big_goods() {
        let t = |k: Kind, name: &str, stock: &[(Item, u32)], out: Item, secs: u64| {
            assert_eq!(run_powered(k, name, stock, secs), vec![out], "{k:?} {name}");
        };
        // Crude: 10 s each.
        t(Kind::CrudeAssembler, "valve", &[(Item::CopperPipe, 1), (Item::IronGear, 1)], Item::Valve, 11);
        t(Kind::CrudeAssembler, "gauge", &[(Item::Glass, 1), (Item::Spring, 1), (Item::CopperSheet, 1)], Item::Gauge, 11);
        t(Kind::CrudeAssembler, "lamp", &[(Item::Glass, 1), (Item::FineWire, 2)], Item::Lamp, 11);
        t(Kind::CrudeAssembler, "relay", &[(Item::CopperCoil, 1), (Item::Spring, 1), (Item::IronSheet, 1)], Item::Relay, 11);
        t(Kind::CrudeAssembler, "gearbox", &[(Item::IronGear, 3), (Item::Steel, 1)], Item::Gearbox, 11);
        // Basic: motors and pistons, 10 s; the crude parts at 5 s.
        t(Kind::BasicAssembler, "electric motor", &[(Item::CopperCoil, 2), (Item::SteelRod, 1), (Item::Bearing, 2)], Item::ElectricMotor, 11);
        t(Kind::BasicAssembler, "piston", &[(Item::SteelRod, 1), (Item::HardenedIronPlate, 1), (Item::Screw, 2)], Item::Piston, 11);
        t(Kind::BasicAssembler, "valve", &[(Item::CopperPipe, 1), (Item::IronGear, 1)], Item::Valve, 6);
        // Advanced: control boards, 10 s; motors at 5 s, crude parts at 2.5 s.
        t(Kind::AdvancedAssembler, "control board", &[(Item::Relay, 1), (Item::FineWire, 4), (Item::IronSheet, 1)], Item::ControlBoard, 11);
        t(Kind::AdvancedAssembler, "electric motor", &[(Item::CopperCoil, 2), (Item::SteelRod, 1), (Item::Bearing, 2)], Item::ElectricMotor, 6);
        t(Kind::AdvancedAssembler, "gearbox", &[(Item::IronGear, 3), (Item::Steel, 1)], Item::Gearbox, 3);
        // Large basic: 20 s.
        t(
            Kind::LargeBasicAssembler,
            "pump unit",
            &[(Item::ElectricMotor, 1), (Item::Valve, 2), (Item::SiloShell, 1), (Item::Gauge, 1)],
            Item::PumpUnit,
            21,
        );
        t(
            Kind::LargeBasicAssembler,
            "engine block",
            &[(Item::Frame, 1), (Item::Piston, 4), (Item::Gearbox, 1), (Item::BoilerShell, 1)],
            Item::EngineBlock,
            21,
        );
        // Large advanced: the same two at 10 s, then its own: 20 s, 20 s, 40 s and 60 s.
        t(
            Kind::LargeAdvancedAssembler,
            "engine block",
            &[(Item::Frame, 1), (Item::Piston, 4), (Item::Gearbox, 1), (Item::BoilerShell, 1)],
            Item::EngineBlock,
            11,
        );
        t(
            Kind::LargeAdvancedAssembler,
            "generator",
            &[(Item::Frame, 1), (Item::ElectricMotor, 2), (Item::Gauge, 1), (Item::ControlBoard, 1)],
            Item::Generator,
            21,
        );
        t(
            Kind::LargeAdvancedAssembler,
            "control panel",
            &[(Item::ControlBoard, 2), (Item::Lamp, 4), (Item::Gauge, 2), (Item::SteelSheet, 1)],
            Item::ControlPanel,
            21,
        );
        t(
            Kind::LargeAdvancedAssembler,
            "drilling rig",
            &[(Item::SteelGirder, 4), (Item::Gearbox, 2), (Item::ReinforcedConcrete, 4), (Item::ElectricMotor, 1), (Item::PumpUnit, 1)],
            Item::DrillingRig,
            41,
        );
        t(
            Kind::LargeAdvancedAssembler,
            "steam tractor",
            &[(Item::EngineBlock, 1), (Item::Frame, 1), (Item::HardenedGear, 4), (Item::ControlPanel, 1), (Item::Generator, 1)],
            Item::SteamTractor,
            61,
        );
        // Not before their time.
        assert!(run_powered(Kind::LargeAdvancedAssembler, "steam tractor", &[(Item::EngineBlock, 1), (Item::Frame, 1), (Item::HardenedGear, 4), (Item::ControlPanel, 1), (Item::Generator, 1)], 59).is_empty());
    }

    #[test]
    fn every_item_but_waste_and_the_crown_items_is_used_in_a_recipe() {
        let crown = [Item::DrillingRig, Item::SteamTractor];
        let mut used = [false; ITEMS];
        for k in Kind::ALL {
            for r in k.recipes() {
                for (i, _) in r.inputs {
                    used[i.idx()] = true;
                }
            }
        }
        for it in Item::ALL {
            if it == Item::Waste || crown.contains(&it) {
                continue;
            }
            assert!(used[it.idx()], "{it:?} is an input to nothing");
        }
        for c in crown {
            assert!(!used[c.idx()], "{c:?} is a crown item: only for selling and research");
        }
        assert!(!used[Item::Waste.idx()]);
    }

    #[test]
    fn recipes_have_at_most_five_kinds_of_input_and_small_ones_three() {
        for k in Kind::ALL {
            for r in k.recipes() {
                let kinds = r.inputs.len() + r.fluid_in.is_some() as usize;
                assert!(kinds <= 5, "{k:?} {} has {kinds} kinds of input", r.name);
            }
        }
        for r in CRUDE_RECIPES.iter().chain(BASIC_RECIPES) {
            assert!(r.inputs.len() <= 3, "{} has {} kinds of input", r.name, r.inputs.len());
        }
    }

    /// Prints each made item's price against its inputs, and checks that every recipe's
    /// outputs sell for more than what goes in (waste counts against them).
    #[test]
    fn each_step_sells_for_more_than_its_inputs() {
        let total = |list: &[(Item, u32)]| -> f64 { list.iter().map(|(i, n)| i.price() * *n as f64).sum() };
        for k in Kind::ALL {
            for r in k.recipes() {
                // Molten metal has no price; grit is about yield, not price.
                if r.outputs.iter().all(|o| o.0 == Item::Waste) || k == Kind::Crusher {
                    continue;
                }
                let (put, got) = (total(r.inputs), total(r.outputs));
                println!("{:<18} {:<26} in ${put:>8.2}  out ${got:>8.2}", k.name(), r.name);
                assert!(got > put, "{k:?} {}: ${put} in, ${got} out", r.name);
            }
        }
        // The steel routes: the steel is worth more than the plate, coke and coal behind it.
        for r in BLAST_RECIPES.iter().filter(|r| r.fluid_out.is_some_and(|f| f.0 == Fluid::MoltenSteel)) {
            assert!(Item::Steel.price() > total(r.inputs), "{}", r.name);
        }
    }

    #[test]
    fn crown_items_keep_a_high_research_floor() {
        let crown = [Item::DrillingRig, Item::SteamTractor];
        let top = Item::ALL.iter().filter(|i| !crown.contains(i)).map(|i| i.curve().floor).fold(0.0, f64::max);
        for c in crown {
            let curve = c.curve();
            assert!(curve.floor > 2.0 * top, "{c:?} floor {} vs the rest's {top}", curve.floor);
            // However many are sold, each still pays a good share of its worth.
            assert!(curve.value(1_000_000) >= curve.worth * curve.floor);
            assert!(curve.value(1_000_000) > 1000.0, "{c:?}");
            assert!(curve.worth >= 2.0 * c.price(), "{c:?} worth about 2x the price");
        }
        assert_eq!((Item::DrillingRig.price(), Item::SteamTractor.price()), (3500.0, 7500.0));
        // Every new item is worth at least 10 research, and about twice its price.
        for it in &Item::ALL[28..] {
            let worth = it.curve().worth;
            assert!(worth >= 10.0 && (worth == 10.0 || worth == 2.0 * it.price()), "{it:?} worth {worth}");
        }
    }

    #[test]
    fn new_items_have_prices_and_research_curves() {
        let prices = [
            (Item::Brick, 1.5),
            (Item::Glass, 2.0),
            (Item::SteelRod, 16.0),
            (Item::Bearing, 12.0),
            (Item::SteelSheet, 36.0),
            (Item::HardenedIronPlate, 20.0),
            (Item::HardenedGear, 65.0),
            (Item::ReinforcedConcrete, 16.0),
            (Item::Valve, 50.0),
            (Item::Gauge, 35.0),
            (Item::Lamp, 15.0),
            (Item::Relay, 80.0),
            (Item::ElectricMotor, 160.0),
            (Item::Piston, 60.0),
            (Item::ControlBoard, 170.0),
            (Item::BoilerShell, 80.0),
            (Item::SiloShell, 260.0),
            (Item::Frame, 280.0),
            (Item::PumpUnit, 900.0),
            (Item::Generator, 1100.0),
            (Item::EngineBlock, 1150.0),
            (Item::ControlPanel, 900.0),
            (Item::SteamTractor, 7500.0),
            (Item::DrillingRig, 3500.0),
            (Item::Steel, 24.0),
            (Item::SteelGirder, 75.0),
            (Item::Gearbox, 120.0),
            (Item::Concrete, 2.0),
        ];
        for (item, price) in prices {
            assert_eq!(item.price(), price, "{item:?}");
        }
        assert_eq!(ITEMS, Item::ALL.len());
        assert_eq!(Item::ALL[28], Item::Brick, "new items go on the end");
    }

    /// Ore in, motors out: copper (furnace -> molder -> press -> wire drawer -> winder), iron
    /// (furnace -> molder -> press) into steel (blast furnace -> molder), steel rods and bearings
    /// (two lathes), and a basic assembler putting it together.
    #[test]
    fn a_small_factory_makes_electric_motors_from_ore() {
        let mut w = World::blank();
        let crate_with = |w: &mut World, x: i32, y: i32, d: Dir, items: &[Item]| {
            w.place(0, x, y, Kind::Crate, d).unwrap();
            fill_crate(w, 0, x, y, items);
        };
        let mix = |ore: Item, n: usize, coal: usize| -> Vec<Item> {
            let mut v = Vec::new();
            for i in 0..n.max(coal) {
                if i < coal {
                    v.push(Item::Coal);
                }
                if i < n {
                    v.push(ore);
                }
            }
            v
        };
        // Lays belts along the points (straight runs); the last tile points the way of its run.
        let lay = |w: &mut World, pts: &[(i32, i32)]| {
            let last = pts.len() - 2;
            for (n, pair) in pts.windows(2).enumerate() {
                let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
                let d = if x1 > x0 { Dir::E } else if x1 < x0 { Dir::W } else if y1 > y0 { Dir::S } else { Dir::N };
                let (dx, dy) = d.delta();
                let steps = (x1 - x0).abs() + (y1 - y0).abs() + (n == last) as i32;
                for k in 0..steps {
                    w.place(0, x0 + dx * k, y0 + dy * k, Kind::Belt, d).unwrap();
                }
            }
        };
        // Copper line along y = 10.
        crate_with(&mut w, 5, 10, Dir::E, &mix(Item::CopperOre, 6, 8));
        w.place(0, 6, 9, Kind::Furnace, Dir::E).unwrap(); // molten copper
        w.place(0, 9, 10, Kind::Molder, Dir::E).unwrap(); // copper ingots
        w.place(0, 11, 10, Kind::Press, Dir::E).unwrap(); // copper plate
        w.place(0, 13, 10, Kind::WireDrawer, Dir::E).unwrap(); // 3 wire a plate
        w.place(0, 15, 10, Kind::Winder, Dir::E).unwrap(); // coils
        // Iron line along y = 20.
        crate_with(&mut w, 5, 20, Dir::E, &mix(Item::IronOre, 9, 8));
        w.place(0, 6, 19, Kind::Furnace, Dir::E).unwrap();
        pick_recipe(&mut w, 6, 19, "molten iron");
        w.place(0, 9, 20, Kind::Molder, Dir::E).unwrap();
        pick_recipe(&mut w, 9, 20, "iron ingot");
        w.place(0, 11, 20, Kind::Press, Dir::E).unwrap();
        pick_recipe(&mut w, 11, 20, "iron plate");
        w.place(0, 13, 20, Kind::BlastFurnace, Dir::E).unwrap(); // covers 13-14, 20-21
        pick_recipe(&mut w, 13, 20, "molten steel (coal)");
        crate_with(&mut w, 13, 19, Dir::S, &[Item::Coal; 6]);
        crate_with(&mut w, 15, 20, Dir::E, &[]); // the waste
        w.place(0, 15, 21, Kind::Molder, Dir::E).unwrap(); // covers 15-16, 21-22
        pick_recipe(&mut w, 15, 21, "steel");
        w.place(0, 17, 21, Kind::Lathe, Dir::E).unwrap(); // steel rods; covers 17-19, 21-23
        pick_recipe(&mut w, 17, 21, "steel rod");
        w.place(0, 20, 22, Kind::Lathe, Dir::E).unwrap(); // bearings; covers 20-22, 22-24
        pick_recipe(&mut w, 20, 22, "bearing");
        // The assembler at 30,12 and its three feeds: coils from the north, rods from the
        // west, bearings from the south. Rods leave the first lathe in turn onto the belt
        // (20,21) and into the bearing lathe.
        w.place(0, 30, 12, Kind::BasicAssembler, Dir::E).unwrap();
        pick_recipe(&mut w, 30, 12, "electric motor");
        w.place(0, 32, 12, Kind::Crate, Dir::E).unwrap();
        lay(&mut w, &[(17, 10), (30, 10), (30, 11)]);
        lay(&mut w, &[(20, 21), (27, 21), (27, 12), (29, 12)]);
        lay(&mut w, &[(23, 23), (30, 23), (30, 14)]);
        // Power: poles in a loose grid over everything, and panels beside some of them.
        let mut poles = Vec::new();
        for y in (6..=27).step_by(3) {
            for x in (6..=36).step_by(3) {
                if w.check_place(0, x, y, Kind::Pole).is_ok() {
                    w.place(0, x, y, Kind::Pole, Dir::E).unwrap();
                    poles.push((x, y));
                }
            }
        }
        let mut panels = 0;
        'panels: for (px, py) in poles {
            for (dx, dy) in [(1, 1), (-2, 1), (1, -2), (-2, -2)] {
                if panels < 10 && w.check_place(0, px + dx, py + dy, Kind::Solar).is_ok() {
                    w.place(0, px + dx, py + dy, Kind::Solar, Dir::E).unwrap();
                    panels += 1;
                    continue 'panels;
                }
            }
        }
        assert!(panels >= 8, "{panels} panels");
        // Up to five minutes, but stop as soon as two motors are out.
        for _ in 0..300 {
            run(&mut w, 1);
            if crate_items(&w, 0, 32, 12).len() >= 2 {
                break;
            }
        }
        let motors = crate_items(&w, 0, 32, 12);
        let at_asm = match &w.tile(0, 30, 12).unwrap().st {
            St::Crafter { stock, .. } => [Item::CopperCoil, Item::SteelRod, Item::Bearing].map(|i| stock[i.idx()]),
            _ => unreachable!(),
        };
        let made = [Item::CopperCoil, Item::Steel, Item::SteelRod, Item::Bearing].map(|i| w.econ.made[i.idx()]);
        assert!(
            motors.len() >= 2,
            "motors out: {} (made coil/steel/rod/bearing {made:?}; assembler holds {at_asm:?}, {:?}, EC {})",
            motors.len(),
            w.idle_reason(0, idx(30, 12)),
            w.ratio_at(0, 30, 12)
        );
        assert!(motors.iter().all(|i| *i == Item::ElectricMotor));
        assert!(w.econ.made[Item::Waste.idx()] > 0, "steel left waste behind");
    }
}
