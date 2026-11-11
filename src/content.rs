//! Game content: items, companies, research and recipes.
//! Kept apart from the mechanics so it can move into data files later.
//! Changing these lists changes the save format: bump SAVE_VERSION in sim.rs.

use serde::{Deserialize, Serialize};

pub const ITEMS: usize = 51;
pub const COMPANIES: usize = 12;
pub const TECHS: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Item {
    CopperOre,
    Coal,
    CopperPlate,
    CopperWire,
    CopperPipe,
    CopperCoil,
    Coke,
    IronOre,
    IronPlate,
    Steel,
    Waste,
    SteelGirder,
    Concrete,
    IronGear,
    Gearbox,
    DrillingRig,
    CopperIngot,
    IronIngot,
    Stone,
    CopperGrit,
    IronGrit,
    Sand,
    FineWire,
    CopperSheet,
    IronRod,
    Screw,
    Spring,
    IronSheet,
    Brick,
    Glass,
    SteelRod,
    Bearing,
    SteelSheet,
    HardenedIronPlate,
    HardenedGear,
    ReinforcedConcrete,
    Valve,
    Gauge,
    Lamp,
    Relay,
    ElectricMotor,
    Piston,
    ControlBoard,
    BoilerShell,
    SiloShell,
    Frame,
    PumpUnit,
    Generator,
    EngineBlock,
    ControlPanel,
    SteamTractor,
}

/// How much research an item gives as more of it is sold:
/// worth × max(floor, decay^n). Every item has one; some are just better.
#[derive(Clone, Copy, Debug)]
pub struct Curve {
    pub worth: f64,
    pub decay: f64,
    pub floor: f64,
}

impl Curve {
    pub fn value(&self, n: u64) -> f64 {
        self.worth * self.decay.powf(n as f64).max(self.floor)
    }
    /// Research from `n` more sales after `from` already made (n can be fractional).
    pub fn total(&self, from: f64, n: f64) -> f64 {
        if n <= 0.0 {
            return 0.0;
        }
        // Sales before this point fade; after it they all pay the floor.
        let knee = self.floor.ln() / self.decay.ln();
        let end = from + n;
        let fading_end = end.min(knee);
        let fading = if from < knee { (self.decay.powf(from) - self.decay.powf(fading_end)) / (1.0 - self.decay) } else { 0.0 };
        let flat = (end - from.max(knee)).max(0.0) * self.floor;
        self.worth * (fading + flat)
    }
}

impl Item {
    pub const ALL: [Item; ITEMS] = [
        Item::CopperOre,
        Item::Coal,
        Item::CopperPlate,
        Item::CopperWire,
        Item::CopperPipe,
        Item::CopperCoil,
        Item::Coke,
        Item::IronOre,
        Item::IronPlate,
        Item::Steel,
        Item::Waste,
        Item::SteelGirder,
        Item::Concrete,
        Item::IronGear,
        Item::Gearbox,
        Item::DrillingRig,
        Item::CopperIngot,
        Item::IronIngot,
        Item::Stone,
        Item::CopperGrit,
        Item::IronGrit,
        Item::Sand,
        Item::FineWire,
        Item::CopperSheet,
        Item::IronRod,
        Item::Screw,
        Item::Spring,
        Item::IronSheet,
        Item::Brick,
        Item::Glass,
        Item::SteelRod,
        Item::Bearing,
        Item::SteelSheet,
        Item::HardenedIronPlate,
        Item::HardenedGear,
        Item::ReinforcedConcrete,
        Item::Valve,
        Item::Gauge,
        Item::Lamp,
        Item::Relay,
        Item::ElectricMotor,
        Item::Piston,
        Item::ControlBoard,
        Item::BoilerShell,
        Item::SiloShell,
        Item::Frame,
        Item::PumpUnit,
        Item::Generator,
        Item::EngineBlock,
        Item::ControlPanel,
        Item::SteamTractor,
    ];
    pub fn idx(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        match self {
            Item::CopperOre => "copper ore",
            Item::Coal => "coal",
            Item::CopperPlate => "copper plate",
            Item::CopperWire => "copper wire",
            Item::CopperPipe => "copper pipe",
            Item::CopperCoil => "copper coil",
            Item::Coke => "coke",
            Item::IronOre => "iron ore",
            Item::IronPlate => "iron plate",
            Item::Steel => "steel",
            Item::Waste => "waste",
            Item::SteelGirder => "steel girder",
            Item::Concrete => "concrete",
            Item::IronGear => "iron gear",
            Item::Gearbox => "gearbox",
            Item::DrillingRig => "drilling rig",
            Item::CopperIngot => "copper ingot",
            Item::IronIngot => "iron ingot",
            Item::Stone => "stone",
            Item::CopperGrit => "copper grit",
            Item::IronGrit => "iron grit",
            Item::Sand => "sand",
            Item::FineWire => "fine wire",
            Item::CopperSheet => "copper sheet",
            Item::IronRod => "iron rod",
            Item::Screw => "screw",
            Item::Spring => "spring",
            Item::IronSheet => "iron sheet",
            Item::Brick => "brick",
            Item::Glass => "glass",
            Item::SteelRod => "steel rod",
            Item::Bearing => "bearing",
            Item::SteelSheet => "steel sheet",
            Item::HardenedIronPlate => "hardened iron plate",
            Item::HardenedGear => "hardened gear",
            Item::ReinforcedConcrete => "reinforced concrete",
            Item::Valve => "valve",
            Item::Gauge => "gauge",
            Item::Lamp => "lamp",
            Item::Relay => "relay",
            Item::ElectricMotor => "electric motor",
            Item::Piston => "piston",
            Item::ControlBoard => "control board",
            Item::BoilerShell => "boiler shell",
            Item::SiloShell => "silo shell",
            Item::Frame => "frame",
            Item::PumpUnit => "pump unit",
            Item::Generator => "generator",
            Item::EngineBlock => "engine block",
            Item::ControlPanel => "control panel",
            Item::SteamTractor => "steam tractor",
        }
    }
    pub fn price(self) -> f64 {
        match self {
            Item::CopperOre => 0.5,
            Item::Coal => 0.2,
            Item::CopperPlate => 4.0,
            Item::CopperWire => 3.0,
            Item::CopperPipe => 12.0,
            Item::CopperCoil => 18.0,
            Item::Coke => 2.5,
            Item::IronOre => 1.0,
            Item::IronPlate => 2.0,
            Item::Steel => 24.0,
            // Waste costs money to get rid of.
            Item::Waste => -0.5,
            Item::SteelGirder => 75.0,
            Item::Concrete => 2.0,
            Item::IronGear => 18.0,
            Item::Gearbox => 120.0,
            Item::DrillingRig => 3500.0,
            Item::CopperIngot => 2.0,
            Item::IronIngot => 1.5,
            Item::Stone => 0.3,
            Item::CopperGrit => 0.6,
            Item::IronGrit => 0.6,
            Item::Sand => 0.25,
            Item::FineWire => 2.5,
            Item::CopperSheet => 14.0,
            Item::IronRod => 5.0,
            Item::Screw => 2.0,
            Item::Spring => 15.0,
            Item::IronSheet => 18.0,
            Item::Brick => 1.0,
            Item::Glass => 2.0,
            Item::SteelRod => 16.0,
            Item::Bearing => 12.0,
            Item::SteelSheet => 36.0,
            Item::HardenedIronPlate => 14.0,
            Item::HardenedGear => 50.0,
            Item::ReinforcedConcrete => 16.0,
            Item::Valve => 50.0,
            Item::Gauge => 35.0,
            Item::Lamp => 15.0,
            Item::Relay => 80.0,
            Item::ElectricMotor => 160.0,
            Item::Piston => 60.0,
            Item::ControlBoard => 170.0,
            Item::BoilerShell => 80.0,
            Item::SiloShell => 260.0,
            Item::Frame => 280.0,
            Item::PumpUnit => 900.0,
            Item::Generator => 1100.0,
            Item::EngineBlock => 1150.0,
            Item::ControlPanel => 900.0,
            Item::SteamTractor => 7500.0,
        }
    }
    pub fn curve(self) -> Curve {
        let c = |worth, decay, floor| Curve { worth, decay, floor };
        match self {
            Item::CopperOre => c(3.0, 0.99, 0.02),
            Item::Coal => c(2.0, 0.99, 0.02),
            Item::CopperPlate => c(12.0, 0.995, 0.03),
            Item::CopperWire => c(8.0, 0.995, 0.03),
            Item::CopperPipe => c(20.0, 0.995, 0.04),
            Item::CopperCoil => c(35.0, 0.996, 0.04),
            Item::Coke => c(8.0, 0.99, 0.03),
            Item::IronOre => c(5.0, 0.99, 0.02),
            Item::IronPlate => c(15.0, 0.995, 0.03),
            Item::Steel => c(48.0, 0.996, 0.05),
            // Waste never earns research (worth 0).
            Item::Waste => c(0.0, 0.99, 0.02),
            Item::SteelGirder => c(150.0, 0.996, 0.06),
            Item::Concrete => c(10.0, 0.995, 0.04),
            Item::IronGear => c(30.0, 0.996, 0.05),
            Item::Gearbox => c(240.0, 0.997, 0.08),
            // Crown items are only for selling and research. Their floor is high, so they keep paying
            // good research however many you sell (every other item's floor is 0.1 or lower).
            Item::DrillingRig => c(7000.0, 0.99, 0.2),
            Item::CopperIngot => c(6.0, 0.995, 0.03),
            Item::IronIngot => c(8.0, 0.995, 0.03),
            Item::Stone => c(1.0, 0.99, 0.02),
            Item::CopperGrit => c(10.0, 0.98, 0.02),
            Item::IronGrit => c(10.0, 0.98, 0.02),
            Item::Sand => c(10.0, 0.98, 0.02),
            Item::FineWire => c(10.0, 0.995, 0.03),
            Item::CopperSheet => c(28.0, 0.995, 0.04),
            Item::IronRod => c(10.0, 0.995, 0.03),
            Item::Screw => c(10.0, 0.995, 0.03),
            Item::Spring => c(30.0, 0.996, 0.05),
            Item::IronSheet => c(36.0, 0.996, 0.05),
            Item::Brick => c(10.0, 0.98, 0.02),
            Item::Glass => c(10.0, 0.98, 0.02),
            Item::SteelRod => c(32.0, 0.995, 0.04),
            Item::Bearing => c(24.0, 0.995, 0.04),
            Item::SteelSheet => c(72.0, 0.996, 0.05),
            Item::HardenedIronPlate => c(28.0, 0.995, 0.04),
            Item::HardenedGear => c(100.0, 0.996, 0.05),
            Item::ReinforcedConcrete => c(32.0, 0.995, 0.04),
            Item::Valve => c(100.0, 0.996, 0.05),
            Item::Gauge => c(70.0, 0.996, 0.05),
            Item::Lamp => c(30.0, 0.995, 0.04),
            Item::Relay => c(160.0, 0.996, 0.05),
            Item::ElectricMotor => c(320.0, 0.997, 0.06),
            Item::Piston => c(120.0, 0.996, 0.05),
            Item::ControlBoard => c(340.0, 0.997, 0.06),
            Item::BoilerShell => c(160.0, 0.996, 0.05),
            Item::SiloShell => c(520.0, 0.997, 0.06),
            Item::Frame => c(560.0, 0.997, 0.06),
            Item::PumpUnit => c(1800.0, 0.998, 0.08),
            Item::Generator => c(2200.0, 0.998, 0.08),
            Item::EngineBlock => c(2300.0, 0.998, 0.08),
            Item::ControlPanel => c(1800.0, 0.998, 0.08),
            // A crown item too: same high floor as the drilling rig.
            Item::SteamTractor => c(15000.0, 0.99, 0.2),
        }
    }
    /// Fuel items and how many ticks one keeps a fire going.
    pub fn burn_ticks(self) -> Option<u32> {
        match self {
            Item::Coal => Some(4 * 60),
            Item::Coke => Some(8 * 60),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Company {
    Hollowmere,
    Brannock,
    Ashgrove,
    Cinderhall,
    Wellbourne,
    Kettering,
    Pillar,
    Underhill,
    Gearwright,
    Swiftline,
    Ampleworth,
    LatticeFinch,
}

impl Company {
    pub const ALL: [Company; COMPANIES] = [
        Company::Hollowmere,
        Company::Brannock,
        Company::Ashgrove,
        Company::Cinderhall,
        Company::Wellbourne,
        Company::Kettering,
        Company::Pillar,
        Company::Underhill,
        Company::Gearwright,
        Company::Swiftline,
        Company::Ampleworth,
        Company::LatticeFinch,
    ];
    pub fn idx(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        match self {
            Company::Hollowmere => "Hollowmere Mining Co.",
            Company::Brannock => "Brannock & Daughters",
            Company::Ashgrove => "Ashgrove Ironstone",
            Company::Cinderhall => "Cinderhall Foundry",
            Company::Wellbourne => "Wellbourne Waterworks",
            Company::Kettering => "Kettering & Vane",
            Company::Pillar => "Pillar & Sons",
            Company::Underhill => "Underhill Excavation",
            Company::Gearwright => "Gearwright Machine Works",
            Company::Swiftline => "Swiftline Conveyors",
            Company::Ampleworth => "Ampleworth Motor Works",
            Company::LatticeFinch => "Lattice & Finch",
        }
    }
    /// The industry it belongs to, for the story.
    #[allow(dead_code)]
    pub fn industry(self) -> &'static str {
        match self {
            Company::Hollowmere | Company::Brannock => "Copper Works",
            Company::Ashgrove | Company::Cinderhall => "Iron & Steel",
            Company::Wellbourne | Company::Kettering => "Steam & EC",
            Company::Pillar | Company::Underhill => "Construction",
            Company::Gearwright | Company::Swiftline => "Machinery",
            Company::Ampleworth | Company::LatticeFinch => "Motors & Chips",
        }
    }
    /// The item that opens this company when you first make it (starter companies are open already).
    pub fn key_item(self) -> Option<Item> {
        match self {
            Company::Hollowmere | Company::Brannock => None,
            Company::Ashgrove => Some(Item::CopperWire),
            Company::Cinderhall => Some(Item::Coke),
            Company::Wellbourne => Some(Item::CopperPipe),
            Company::Kettering => Some(Item::CopperCoil),
            Company::Pillar => Some(Item::SteelGirder),
            Company::Underhill => Some(Item::Concrete),
            Company::Gearwright => Some(Item::IronGear),
            Company::Swiftline => Some(Item::Gearbox),
            Company::Ampleworth => Some(Item::ElectricMotor),
            Company::LatticeFinch => Some(Item::ControlBoard),
        }
    }
    /// The letter a company sends when it opens.
    pub fn letter(self) -> &'static str {
        match self {
            Company::Hollowmere => "We've dug these hills for four generations. Our drills are cheap, honest, and they don't complain. Neither should you.",
            Company::Brannock => "Furnaces, molders, belts, panels, poles: if it works, the three of us will sell it to you. If it doesn't, one of us will fix it.",
            Company::Ashgrove => "Copper wire, is it? Fine hands. Ashgrove deals in ironstone: our basic drill cuts any ore faster, and our crusher grinds ore and stone to grit and sand. Grit melts further than whole rock, so you get a third more metal from every lump.",
            Company::Cinderhall => "Coke! Now you're cooking. Our blast furnaces melt ironstone fast, turn plate into molten steel for a molder to cast, and harden plate with coke or coal. Our coke oven bakes coal into coke with no fire of its own. They all leave waste behind, mind. Nobody buys it, so plan where it goes.",
            Company::Wellbourne => "A good pipe is a promise. Wellbourne sells the pumps and the pipes to keep it: set a pump down anywhere and the water will go wherever your pipes do. Our advanced pump gives four times the water, and our mixer takes it from a pipe to make concrete.",
            Company::Kettering => "Mr Kettering says your coil is crude. Mr Vane says it's charming. We both say: our boilers and engines will out-charge a field of panels, our drawer pulls wire finer than any press, and our winder will wind your coils for you.",
            Company::Pillar => "A girder! Let's see how much it holds. Hm. Enough. Pillar & Sons can raise you a floor above the ground. Our rolling mill squeezes plate and steel into sheets and girders, and our welder joins them into shells and frames. When your beams get stronger, we'll raise you higher.",
            Company::Underhill => "Concrete? You're one of us. Underhill Excavation digs the floor beneath your feet, cheerfully, and the ore down there is richer than anything on top. Our kiln bakes stone into brick and sand into glass, eight at a time.",
            Company::Gearwright => "A gear with teeth like that deserves a proper home. Gearwright's crude assembler builds valves, gauges, lamps, relays and gearboxes, patiently. Our lathe turns plate into rods and screws, and steel into steel rods and bearings.",
            Company::Swiftline => "A gearbox! Now that's something that turns. Swiftline belts move twice what Brannock's do, and our basic assembler builds electric motors and pistons. Tell the sisters we said hello.",
            Company::Ampleworth => "A motor, and a tidy one. Ampleworth sells things that turn on their own: an advanced drill that runs on EC and digs twice as fast, and for bigger hands, a large basic assembler and a large basic drill that eats coal by the shovel.",
            Company::LatticeFinch => "A control board! Mr Lattice has gone quiet, which means he's pleased. Ms Finch is already taking notes. We sell the advanced assembler, which wires boards four times faster than a crude one by hand, the precision assembler, the large advanced assembler for the biggest goods of all, and a large advanced drill. Mind the power bill.",
        }
    }
}

/// What a part does. The build menu and research tree are grouped by this;
/// companies are only story. Four groups, so the number keys stay on 1-4.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    /// Drills, furnaces, presses, pumps: things that make or extract.
    Create,
    /// Belts, splitters, arms, pipes, elevators: things that carry.
    Transport,
    /// Solar, poles, boilers, engines (EC: electric charge).
    Power,
    /// Crates and trade posts.
    Misc,
}

impl Category {
    pub const ALL: [Category; 4] = [Category::Create, Category::Transport, Category::Power, Category::Misc];
    pub fn name(self) -> &'static str {
        match self {
            Category::Create => "Create",
            Category::Transport => "Transport",
            Category::Power => "EC",
            Category::Misc => "Misc",
        }
    }
}

/// Things that can be unlocked.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Tech {
    Press,
    Coking,
    BasicDrill,
    Pumps,
    Steam,
    BlastFurnace,
    SupportBeams,
    Excavation,
    Assembler,
    FastBelts,
    Tunnels,
    LongArms,
    FilterSplitters,
    Warehouses,
    Crusher,
    WireDrawer,
    Winder,
    CokeOven,
    Lathe,
    RollingMill,
    Mixer,
    AdvancedPump,
    Welder,
    BasicAssembler,
    Kiln,
    AdvancedDrill,
    AdvancedAssembler,
    LargeBasicAssembler,
    LargeBasicDrill,
    PrecisionAssembler,
    LargeAdvancedAssembler,
    LargeAdvancedDrill,
}

impl Tech {
    pub const ALL: [Tech; TECHS] = [
        Tech::Press,
        Tech::Coking,
        Tech::BasicDrill,
        Tech::Pumps,
        Tech::Steam,
        Tech::BlastFurnace,
        Tech::SupportBeams,
        Tech::Excavation,
        Tech::Assembler,
        Tech::FastBelts,
        Tech::Tunnels,
        Tech::LongArms,
        Tech::FilterSplitters,
        Tech::Warehouses,
        Tech::Crusher,
        Tech::WireDrawer,
        Tech::Winder,
        Tech::CokeOven,
        Tech::Lathe,
        Tech::RollingMill,
        Tech::Mixer,
        Tech::AdvancedPump,
        Tech::Welder,
        Tech::BasicAssembler,
        Tech::Kiln,
        Tech::AdvancedDrill,
        Tech::AdvancedAssembler,
        Tech::LargeBasicAssembler,
        Tech::LargeBasicDrill,
        Tech::PrecisionAssembler,
        Tech::LargeAdvancedAssembler,
        Tech::LargeAdvancedDrill,
    ];
    pub fn idx(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        match self {
            Tech::Press => "Press",
            Tech::Coking => "Coking",
            Tech::BasicDrill => "Basic drill",
            Tech::Pumps => "Pumps & pipes",
            Tech::Steam => "Steam engines",
            Tech::BlastFurnace => "Blast furnace",
            Tech::SupportBeams => "Support beams",
            Tech::Excavation => "Excavation",
            Tech::Assembler => "Crude assembler",
            Tech::FastBelts => "Fast belts",
            Tech::Tunnels => "Belt tunnels",
            Tech::LongArms => "Long arm",
            Tech::FilterSplitters => "Filter splitter",
            Tech::Warehouses => "Warehouse",
            Tech::Crusher => "Crusher",
            Tech::WireDrawer => "Wire drawer",
            Tech::Winder => "Winder",
            Tech::CokeOven => "Coke oven",
            Tech::Lathe => "Lathe",
            Tech::RollingMill => "Rolling mill",
            Tech::Mixer => "Mixer",
            Tech::AdvancedPump => "Advanced pump",
            Tech::Welder => "Welder",
            Tech::BasicAssembler => "Basic assembler",
            Tech::Kiln => "Kiln",
            Tech::AdvancedDrill => "Advanced drill",
            Tech::AdvancedAssembler => "Advanced assembler",
            Tech::LargeBasicAssembler => "Large basic assembler",
            Tech::LargeBasicDrill => "Large basic drill",
            Tech::PrecisionAssembler => "Precision assembler",
            Tech::LargeAdvancedAssembler => "Large adv. assembler",
            Tech::LargeAdvancedDrill => "Large adv. drill",
        }
    }
    pub fn about(self) -> &'static str {
        match self {
            Tech::Press => "a machine that presses ingots into plates, and plates into wire, pipe, coils and later gears and girders",
            Tech::Coking => "lets furnaces bake coal into coke",
            Tech::BasicDrill => "a drill that is faster, and cuts iron twice as fast as a cheap one",
            Tech::Pumps => "pumps that draw water from the ground, and pipes that carry it",
            Tech::Steam => "boilers that turn water and coal into steam, and engines that turn steam into EC",
            Tech::BlastFurnace => "melts iron fast, makes molten steel and hardened plate from plate and coke or coal (leaving waste)",
            Tech::SupportBeams => "build one floor above the ground, and elevators to reach it",
            Tech::Excavation => "dig one floor below the ground, where ore is richer, and elevators to reach it",
            Tech::Assembler => "the crude assembler, a 4x4 machine that builds valves, gauges, lamps, relays and gearboxes",
            Tech::FastBelts => "belts that move twice as much",
            Tech::Tunnels => {
                "belts that dive under up to 4 tiles: place an entrance, then its exit further along, and build anything in between"
            }
            Tech::LongArms => "an arm that reaches 2 tiles: it grabs from 2 behind and drops 2 ahead, over whatever sits in between",
            Tech::FilterSplitters => "a splitter that sends one chosen item out its front and everything else out its sides",
            Tech::Warehouses => "a 2x2 crate that holds four times as much",
            Tech::Crusher => "grinds ore into grit (a third more metal when melted) and stone into sand",
            Tech::WireDrawer => "draws plate into three wire, and wire into fine wire",
            Tech::Winder => "winds wire into coils, and iron rods into springs",
            Tech::CokeOven => "bakes coal into coke on its own, no fire needed (leaving waste)",
            Tech::Lathe => "turns plate into rods and rods into screws, and steel into steel rods and bearings",
            Tech::RollingMill => "rolls copper or iron plate into sheets, and steel into steel sheets and girders",
            Tech::Mixer => "mixes sand, stone and water from a pipe into concrete, and concrete and rod into reinforced concrete",
            Tech::AdvancedPump => "a 2x2 pump that gives four times the water",
            Tech::Welder => "welds steel into boiler shells, silo shells and frames (only draws EC while it works)",
            Tech::BasicAssembler => "a 2x2 assembler that runs crude recipes twice as fast, and builds electric motors and pistons",
            Tech::Kiln => "fires eight at once, burning coal: stone into brick, sand into glass",
            Tech::AdvancedDrill => "a 4x4 drill that runs on EC and mines twice as fast as a basic drill",
            Tech::AdvancedAssembler => "a 3x3 assembler, four times the crude speed, that also builds control boards",
            Tech::LargeBasicAssembler => "a 5x5 assembler for the big goods: pump units and engine blocks",
            Tech::LargeBasicDrill => "an 8x8 drill that mines 16 times a basic drill, and burns 32 times the coal",
            Tech::PrecisionAssembler => "a 3x3 assembler that builds control boards fastest, and later chips",
            Tech::LargeAdvancedAssembler => "a 5x5 assembler for the biggest goods: generators, panels, drilling rigs and steam tractors",
            Tech::LargeAdvancedDrill => "an 8x8 drill on EC that mines 16 times an advanced drill",
        }
    }
    pub fn company(self) -> Company {
        match self {
            Tech::Press | Tech::Coking => Company::Brannock,
            Tech::BasicDrill => Company::Ashgrove,
            Tech::Pumps | Tech::Mixer | Tech::AdvancedPump => Company::Wellbourne,
            Tech::Steam => Company::Kettering,
            Tech::BlastFurnace => Company::Cinderhall,
            Tech::SupportBeams | Tech::RollingMill | Tech::Welder => Company::Pillar,
            Tech::Excavation | Tech::Kiln => Company::Underhill,
            Tech::Assembler => Company::Gearwright,
            Tech::FastBelts | Tech::BasicAssembler => Company::Swiftline,
            Tech::Tunnels | Tech::LongArms | Tech::FilterSplitters | Tech::Warehouses => Company::Brannock,
            Tech::Crusher => Company::Ashgrove,
            Tech::WireDrawer | Tech::Winder => Company::Kettering,
            Tech::CokeOven => Company::Cinderhall,
            Tech::Lathe => Company::Gearwright,
            Tech::AdvancedDrill | Tech::LargeBasicAssembler | Tech::LargeBasicDrill => Company::Ampleworth,
            Tech::AdvancedAssembler | Tech::PrecisionAssembler | Tech::LargeAdvancedAssembler | Tech::LargeAdvancedDrill => {
                Company::LatticeFinch
            }
        }
    }
    pub fn category(self) -> Category {
        match self {
            Tech::Press
            | Tech::Coking
            | Tech::BlastFurnace
            | Tech::Assembler
            | Tech::BasicDrill
            | Tech::Pumps
            | Tech::Crusher
            | Tech::WireDrawer
            | Tech::Winder
            | Tech::CokeOven
            | Tech::Lathe
            | Tech::RollingMill
            | Tech::Mixer
            | Tech::AdvancedPump
            | Tech::Welder
            | Tech::BasicAssembler
            | Tech::Kiln
            | Tech::AdvancedDrill
            | Tech::AdvancedAssembler
            | Tech::LargeBasicAssembler
            | Tech::LargeBasicDrill
            | Tech::PrecisionAssembler
            | Tech::LargeAdvancedAssembler
            | Tech::LargeAdvancedDrill => Category::Create,
            Tech::Steam => Category::Power,
            Tech::SupportBeams | Tech::Excavation => Category::Transport,
            Tech::FastBelts | Tech::Tunnels | Tech::LongArms | Tech::FilterSplitters => Category::Transport,
            Tech::Warehouses => Category::Misc,
        }
    }
    /// The key item you must make before this can be unlocked.
    pub fn requires(self) -> Option<Item> {
        self.company().key_item()
    }
    /// Research points: the quick way to unlock. Costs grow the further into the tree you go:
    /// copper and iron under 1,500; steel and stone machines 1,500 to 6,000; motor machines
    /// 6,000 to 25,000; processor machines 25,000 to 100,000.
    pub fn cost(self) -> f64 {
        match self {
            Tech::Press => 60.0,
            Tech::Coking => 100.0,
            Tech::BasicDrill => 500.0,
            Tech::Pumps => 600.0,
            Tech::BlastFurnace => 800.0,
            Tech::Steam => 1000.0,
            Tech::FastBelts => 2500.0,
            Tech::SupportBeams => 3000.0,
            Tech::Excavation => 3000.0,
            Tech::Assembler => 5000.0,
            Tech::Tunnels => 250.0,
            Tech::LongArms => 400.0,
            Tech::Warehouses => 500.0,
            Tech::FilterSplitters => 600.0,
            Tech::Crusher => 150.0,
            Tech::Winder => 200.0,
            Tech::WireDrawer => 250.0,
            Tech::CokeOven => 300.0,
            Tech::Lathe => 600.0,
            Tech::RollingMill => 1200.0,
            // Steel and stone stage.
            Tech::Mixer => 1500.0,
            Tech::Kiln => 2000.0,
            Tech::AdvancedPump => 2500.0,
            Tech::Welder => 4000.0,
            Tech::BasicAssembler => 6000.0,
            // Motor stage.
            Tech::AdvancedDrill => 8000.0,
            Tech::AdvancedAssembler => 14_000.0,
            Tech::LargeBasicDrill => 20_000.0,
            Tech::LargeBasicAssembler => 25_000.0,
            // Processor stage.
            Tech::PrecisionAssembler => 30_000.0,
            Tech::LargeAdvancedDrill => 60_000.0,
            Tech::LargeAdvancedAssembler => 100_000.0,
        }
    }
    /// The free way to unlock: make this many of an item (nothing is used up).
    pub fn milestone(self) -> (Item, u64) {
        match self {
            Tech::Press => (Item::CopperIngot, 50),
            Tech::Coking => (Item::Coal, 300),
            Tech::BasicDrill => (Item::CopperWire, 150),
            Tech::Pumps => (Item::CopperPipe, 60),
            Tech::Steam => (Item::CopperCoil, 40),
            Tech::BlastFurnace => (Item::Coke, 200),
            Tech::SupportBeams => (Item::SteelGirder, 40),
            Tech::Excavation => (Item::Concrete, 150),
            // The crude assembler is what makes gearboxes, so its milestone is the gear.
            Tech::Assembler => (Item::IronGear, 300),
            Tech::FastBelts => (Item::Gearbox, 40),
            Tech::Tunnels => (Item::CopperPlate, 400),
            Tech::LongArms => (Item::CopperWire, 200),
            Tech::FilterSplitters => (Item::CopperPipe, 60),
            Tech::Warehouses => (Item::Coal, 2000),
            Tech::Crusher => (Item::CopperOre, 300),
            Tech::Winder => (Item::CopperCoil, 40),
            Tech::WireDrawer => (Item::CopperWire, 400),
            Tech::CokeOven => (Item::Coke, 60),
            Tech::Lathe => (Item::IronPlate, 300),
            Tech::RollingMill => (Item::IronPlate, 500),
            Tech::Mixer => (Item::Stone, 1000),
            Tech::Kiln => (Item::Concrete, 250),
            Tech::AdvancedPump => (Item::CopperPipe, 250),
            Tech::Welder => (Item::SteelGirder, 150),
            Tech::BasicAssembler => (Item::Gearbox, 100),
            Tech::AdvancedDrill => (Item::ElectricMotor, 40),
            Tech::AdvancedAssembler => (Item::ControlBoard, 40),
            Tech::LargeBasicDrill => (Item::ElectricMotor, 150),
            Tech::LargeBasicAssembler => (Item::ElectricMotor, 300),
            Tech::PrecisionAssembler => (Item::ControlBoard, 120),
            Tech::LargeAdvancedDrill => (Item::ControlBoard, 300),
            Tech::LargeAdvancedAssembler => (Item::ControlBoard, 800),
        }
    }
}

/// The research screen, in groups.
pub const UNLOCK_GROUPS: [(&str, &[Tech]); 6] = [
    (
        "Making",
        &[
            Tech::Press,
            Tech::Coking,
            Tech::Crusher,
            Tech::Winder,
            Tech::WireDrawer,
            Tech::CokeOven,
            Tech::BlastFurnace,
            Tech::Lathe,
            Tech::RollingMill,
            Tech::Welder,
            Tech::Kiln,
            Tech::Mixer,
        ],
    ),
    (
        "Assembling",
        &[
            Tech::Assembler,
            Tech::BasicAssembler,
            Tech::AdvancedAssembler,
            Tech::PrecisionAssembler,
            Tech::LargeBasicAssembler,
            Tech::LargeAdvancedAssembler,
        ],
    ),
    ("Drilling", &[Tech::BasicDrill, Tech::AdvancedDrill, Tech::LargeBasicDrill, Tech::LargeAdvancedDrill]),
    ("Moving", &[Tech::Tunnels, Tech::LongArms, Tech::Warehouses, Tech::FilterSplitters, Tech::FastBelts]),
    ("EC", &[Tech::Pumps, Tech::AdvancedPump, Tech::Steam]),
    ("Floors", &[Tech::SupportBeams, Tech::Excavation]),
];

/// A link between key items and what they unlock (the recipe book uses it).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Node {
    /// What you start with.
    Start,
    /// Something to unlock: by research, or free by reaching its milestone.
    Tech(Tech),
    /// A key item: make it once to unlock what follows.
    Key(Item),
}

/// The research tree, read left to right: (node, parent).
pub const TREE: &[(Node, Option<usize>)] = &[
    (Node::Start, None),                                  // 0
    (Node::Tech(Tech::Press), Some(0)),                   // 1
    (Node::Key(Item::CopperWire), Some(1)),               // 2
    (Node::Tech(Tech::BasicDrill), Some(2)),              // 3
    (Node::Tech(Tech::Crusher), Some(2)),                 // 4
    (Node::Key(Item::CopperCoil), Some(2)),               // 5
    (Node::Tech(Tech::Steam), Some(5)),                   // 6
    (Node::Tech(Tech::WireDrawer), Some(5)),              // 7
    (Node::Tech(Tech::Winder), Some(5)),                  // 8
    (Node::Key(Item::CopperPipe), Some(1)),               // 9
    (Node::Tech(Tech::Pumps), Some(9)),                   // 10
    (Node::Tech(Tech::Mixer), Some(9)),                   // 11
    (Node::Tech(Tech::AdvancedPump), Some(9)),            // 12
    (Node::Key(Item::Concrete), Some(11)),                // 13
    (Node::Tech(Tech::Excavation), Some(13)),             // 14
    (Node::Tech(Tech::Kiln), Some(13)),                   // 15
    (Node::Tech(Tech::Coking), Some(0)),                  // 16
    (Node::Key(Item::Coke), Some(16)),                    // 17
    (Node::Tech(Tech::BlastFurnace), Some(17)),           // 18
    (Node::Tech(Tech::CokeOven), Some(17)),               // 19
    (Node::Key(Item::SteelGirder), Some(18)),             // 20
    (Node::Tech(Tech::SupportBeams), Some(20)),           // 21
    (Node::Tech(Tech::RollingMill), Some(20)),            // 22
    (Node::Tech(Tech::Welder), Some(20)),                 // 23
    (Node::Key(Item::IronGear), Some(1)),                 // 24: iron plate has a job from the start
    (Node::Tech(Tech::Lathe), Some(24)),                  // 25
    (Node::Tech(Tech::Assembler), Some(24)),              // 26
    (Node::Key(Item::Gearbox), Some(26)),                 // 27
    (Node::Tech(Tech::FastBelts), Some(27)),              // 28
    (Node::Tech(Tech::BasicAssembler), Some(27)),         // 29
    (Node::Key(Item::ElectricMotor), Some(29)),           // 30
    (Node::Tech(Tech::AdvancedDrill), Some(30)),          // 31
    (Node::Tech(Tech::LargeBasicDrill), Some(30)),        // 32
    (Node::Tech(Tech::LargeBasicAssembler), Some(30)),    // 33
    (Node::Key(Item::ControlBoard), Some(26)),            // 34: hand-wired in the crude assembler
    (Node::Tech(Tech::AdvancedAssembler), Some(34)),      // 35
    (Node::Tech(Tech::PrecisionAssembler), Some(34)),     // 36
    (Node::Tech(Tech::LargeAdvancedDrill), Some(34)),     // 37
    (Node::Tech(Tech::LargeAdvancedAssembler), Some(34)), // 38
];

pub fn tree_children(i: usize) -> Vec<usize> {
    TREE.iter().enumerate().filter(|(_, (_, p))| *p == Some(i)).map(|(j, _)| j).collect()
}

/// Things that flow through pipes. A pipe network holds one at a time.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fluid {
    Water,
    Steam,
    MoltenCopper,
    MoltenIron,
    MoltenSteel,
}

impl Fluid {
    pub fn name(self) -> &'static str {
        match self {
            Fluid::Water => "water",
            Fluid::Steam => "steam",
            Fluid::MoltenCopper => "molten copper",
            Fluid::MoltenIron => "molten iron",
            Fluid::MoltenSteel => "molten steel",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Recipe {
    pub name: &'static str,
    pub inputs: &'static [(Item, u32)],
    pub outputs: &'static [(Item, u32)],
    /// A fluid taken from pipes (or from a machine right behind it), in whole units.
    pub fluid_in: Option<(Fluid, u32)>,
    /// A fluid pushed out of the front, into a pipe or straight into a machine.
    pub fluid_out: Option<(Fluid, u32)>,
    pub ticks: u32,
    /// What it burns while working, if anything.
    pub fuel: Option<Item>,
    pub needs: Option<Tech>,
    /// Letter shown on the machine.
    pub tag: char,
}

/// A recipe with items only (no fluids).
const fn item_recipe(
    name: &'static str,
    inputs: &'static [(Item, u32)],
    outputs: &'static [(Item, u32)],
    ticks: u32,
    fuel: Option<Item>,
    needs: Option<Tech>,
    tag: char,
) -> Recipe {
    Recipe { name, inputs, outputs, fluid_in: None, fluid_out: None, ticks, fuel, needs, tag }
}

/// Furnaces melt ore into molten metal (5 s each; iron is slow) and bake coal into coke.
pub const FURNACE_RECIPES: &[Recipe] = &[
    Recipe {
        name: "molten copper",
        inputs: &[(Item::CopperOre, 1)],
        outputs: &[],
        fluid_in: None,
        fluid_out: Some((Fluid::MoltenCopper, 1)),
        ticks: 300,
        fuel: Some(Item::Coal),
        needs: None,
        tag: 'M',
    },
    Recipe {
        name: "molten iron",
        inputs: &[(Item::IronOre, 3)],
        outputs: &[],
        fluid_in: None,
        fluid_out: Some((Fluid::MoltenIron, 2)),
        ticks: 600,
        fuel: Some(Item::Coal),
        needs: None,
        tag: 'I',
    },
    item_recipe("coke", &[(Item::Coal, 3)], &[(Item::Coke, 1)], 300, None, Some(Tech::Coking), 'K'),
    Recipe {
        name: "molten copper (grit)",
        inputs: &[(Item::CopperGrit, 3)],
        outputs: &[],
        fluid_in: None,
        fluid_out: Some((Fluid::MoltenCopper, 2)),
        ticks: 150,
        fuel: Some(Item::Coal),
        needs: None,
        tag: 'M',
    },
    Recipe {
        name: "molten iron (grit)",
        inputs: &[(Item::IronGrit, 2)],
        outputs: &[],
        fluid_in: None,
        fluid_out: Some((Fluid::MoltenIron, 1)),
        ticks: 300,
        fuel: Some(Item::Coal),
        needs: None,
        tag: 'I',
    },
    // Fast and dirty: more coke per coal's time, and a lot of waste.
    item_recipe("coke (fast)", &[(Item::Coal, 7)], &[(Item::Coke, 2), (Item::Waste, 3)], 150, None, Some(Tech::Coking), 'K'),
];

/// Molders cast molten metal into ingots. The metal comes from pipes, or straight from a furnace.
pub const MOLDER_RECIPES: &[Recipe] = &[
    Recipe {
        name: "copper ingot",
        inputs: &[],
        outputs: &[(Item::CopperIngot, 1)],
        fluid_in: Some((Fluid::MoltenCopper, 1)),
        fluid_out: None,
        ticks: 150,
        fuel: None,
        needs: None,
        tag: 'C',
    },
    Recipe {
        name: "iron ingot",
        inputs: &[],
        outputs: &[(Item::IronIngot, 1)],
        fluid_in: Some((Fluid::MoltenIron, 1)),
        fluid_out: None,
        ticks: 150,
        fuel: None,
        needs: None,
        tag: 'I',
    },
    Recipe {
        name: "steel",
        inputs: &[],
        outputs: &[(Item::Steel, 1)],
        fluid_in: Some((Fluid::MoltenSteel, 1)),
        fluid_out: None,
        ticks: 150,
        fuel: None,
        needs: None,
        tag: 'S',
    },
];

/// Presses squeeze ingots into plates, and plates and steel into parts.
pub const PRESS_RECIPES: &[Recipe] = &[
    item_recipe("copper plate", &[(Item::CopperIngot, 1)], &[(Item::CopperPlate, 1)], 150, None, None, '='),
    item_recipe("iron plate", &[(Item::IronIngot, 1)], &[(Item::IronPlate, 1)], 150, None, None, '-'),
    item_recipe("copper wire", &[(Item::CopperPlate, 1)], &[(Item::CopperWire, 2)], 300, None, None, 'W'),
    item_recipe("copper pipe", &[(Item::CopperPlate, 2)], &[(Item::CopperPipe, 1)], 150, None, None, 'P'),
    item_recipe("copper coil", &[(Item::CopperWire, 4)], &[(Item::CopperCoil, 1)], 300, None, None, 'C'),
    item_recipe("iron gear", &[(Item::IronPlate, 2)], &[(Item::IronGear, 1)], 150, None, None, 'G'),
    item_recipe("steel girder", &[(Item::Steel, 2)], &[(Item::SteelGirder, 1)], 300, None, Some(Tech::BlastFurnace), 'R'),
    item_recipe("hardened gear", &[(Item::HardenedIronPlate, 2)], &[(Item::HardenedGear, 1)], 300, None, Some(Tech::BlastFurnace), 'H'),
    item_recipe("copper coil (fine wire)", &[(Item::FineWire, 5)], &[(Item::CopperCoil, 1)], 150, None, None, 'C'),
    // Crude sheets: a third more plate than the rolling mill takes, but no company to wait for.
    item_recipe("copper sheet (pressed)", &[(Item::CopperPlate, 3)], &[(Item::CopperSheet, 1)], 300, None, None, 'S'),
    item_recipe("iron sheet (pressed)", &[(Item::IronPlate, 3)], &[(Item::IronSheet, 1)], 300, None, None, 'I'),
];

/// Molten steel: the steel stays molten and leaves the front as fluid (into a pipe or a molder
/// that casts it), the waste leaves as items.
const fn molten_steel(name: &'static str, inputs: &'static [(Item, u32)], waste: &'static [(Item, u32)], ticks: u32) -> Recipe {
    Recipe {
        name,
        inputs,
        outputs: waste,
        fluid_in: None,
        fluid_out: Some((Fluid::MoltenSteel, 1)),
        ticks,
        fuel: None,
        needs: None,
        tag: 'S',
    }
}

/// Blast furnaces melt iron ore fast, make molten steel, and harden plate. Coke and coal are
/// used up as inputs in the steel and plate recipes; every one but the clean coke ones leaves
/// waste out the front with the product, and the waste has to go somewhere.
pub const BLAST_RECIPES: &[Recipe] = &[
    Recipe {
        name: "molten iron",
        inputs: &[(Item::IronOre, 1)],
        outputs: &[],
        fluid_in: None,
        fluid_out: Some((Fluid::MoltenIron, 1)),
        ticks: 150,
        fuel: Some(Item::Coke),
        needs: None,
        tag: 'I',
    },
    Recipe {
        name: "molten iron (grit)",
        inputs: &[(Item::IronGrit, 3)],
        outputs: &[],
        fluid_in: None,
        fluid_out: Some((Fluid::MoltenIron, 2)),
        ticks: 150,
        fuel: Some(Item::Coke),
        needs: None,
        tag: 'I',
    },
    molten_steel("molten steel", &[(Item::IronPlate, 2), (Item::Coke, 1)], &[(Item::Waste, 1)], 600),
    molten_steel("molten steel (coal)", &[(Item::IronPlate, 2), (Item::Coal, 2)], &[(Item::Waste, 3)], 600),
    molten_steel("molten steel (fast)", &[(Item::IronPlate, 2), (Item::Coke, 2)], &[(Item::Waste, 2)], 300),
    molten_steel("molten steel (fast, coal)", &[(Item::IronPlate, 2), (Item::Coal, 4)], &[(Item::Waste, 5)], 300),
    item_recipe("hardened iron plate", &[(Item::IronPlate, 1), (Item::Coke, 1)], &[(Item::HardenedIronPlate, 1)], 300, None, None, 'H'),
    item_recipe(
        "hardened iron plate (coal)",
        &[(Item::IronPlate, 1), (Item::Coal, 2)],
        &[(Item::HardenedIronPlate, 1), (Item::Waste, 2)],
        300,
        None,
        None,
        'H',
    ),
];

/// Assemblers come in tiers. Each recipe table below lists its tier's recipes, with the time
/// the lowest assembler that can make them takes. Higher assemblers know the lower tables too,
/// at a faster speed (see `assembler_list`).
///
/// Crude tier: small parts, 10 s each.
pub const CRUDE_RECIPES: &[Recipe] = &[
    item_recipe("valve", &[(Item::CopperPipe, 1), (Item::IronGear, 1)], &[(Item::Valve, 1)], 600, None, None, 'V'),
    item_recipe("gauge", &[(Item::Glass, 1), (Item::Spring, 1), (Item::CopperSheet, 1)], &[(Item::Gauge, 1)], 600, None, None, 'G'),
    item_recipe("lamp", &[(Item::Glass, 1), (Item::FineWire, 2)], &[(Item::Lamp, 1)], 600, None, None, 'L'),
    item_recipe("relay", &[(Item::CopperCoil, 1), (Item::Spring, 1), (Item::IronSheet, 1)], &[(Item::Relay, 1)], 600, None, None, 'R'),
    item_recipe("gearbox", &[(Item::IronGear, 3), (Item::Steel, 1)], &[(Item::Gearbox, 1)], 600, None, None, 'B'),
    // The slow way round: boards by hand, before steel and motors. Advanced assemblers and up
    // hide it (they have the 10 s recipe below).
    item_recipe(
        HAND_WIRED,
        &[(Item::Relay, 1), (Item::FineWire, 4), (Item::IronSheet, 1)],
        &[(Item::ControlBoard, 1)],
        2400,
        None,
        None,
        'C',
    ),
];

/// The crude tier's control board, hidden on advanced assemblers and above.
pub const HAND_WIRED: &str = "control board (hand-wired)";

/// Coke ovens cook their own coal (no fire to feed, no EC): half the coal per coke the
/// furnace needs, and some waste out the front.
pub const COKE_OVEN_RECIPES: &[Recipe] =
    &[item_recipe("coke", &[(Item::Coal, 2)], &[(Item::Coke, 1), (Item::Waste, 1)], 120, None, None, 'K')];

/// Crushers grind ore into grit (which melts further) and stone into sand.
pub const CRUSHER_RECIPES: &[Recipe] = &[
    item_recipe("copper grit", &[(Item::CopperOre, 1)], &[(Item::CopperGrit, 2)], 240, None, None, 'C'),
    item_recipe("iron grit", &[(Item::IronOre, 1)], &[(Item::IronGrit, 2)], 240, None, None, 'I'),
    item_recipe("sand", &[(Item::Stone, 1)], &[(Item::Sand, 2)], 150, None, None, 'S'),
];

/// Wire drawers pull plate into wire, and wire into fine wire.
pub const WIRE_DRAWER_RECIPES: &[Recipe] = &[
    item_recipe("copper wire", &[(Item::CopperPlate, 1)], &[(Item::CopperWire, 3)], 240, None, None, 'W'),
    item_recipe("fine wire", &[(Item::CopperWire, 1)], &[(Item::FineWire, 2)], 240, None, None, 'F'),
];

/// Rolling mills squeeze plate flat into sheets, and steel into steel sheets and girders.
pub const ROLLING_MILL_RECIPES: &[Recipe] = &[
    item_recipe("copper sheet", &[(Item::CopperPlate, 2)], &[(Item::CopperSheet, 1)], 300, None, None, 'C'),
    item_recipe("iron sheet", &[(Item::IronPlate, 2)], &[(Item::IronSheet, 1)], 150, None, None, 'I'),
    item_recipe("steel sheet", &[(Item::Steel, 1)], &[(Item::SteelSheet, 1)], 300, None, None, 'S'),
    item_recipe("steel girder", &[(Item::Steel, 3)], &[(Item::SteelGirder, 2)], 240, None, None, 'G'),
];

/// Lathes turn plate into rods and rods into screws, and steel into steel rods and bearings.
pub const LATHE_RECIPES: &[Recipe] = &[
    item_recipe("iron rod", &[(Item::IronPlate, 1)], &[(Item::IronRod, 2)], 300, None, None, 'R'),
    item_recipe("screw", &[(Item::IronRod, 1)], &[(Item::Screw, 4)], 300, None, None, 'S'),
    item_recipe("steel rod", &[(Item::Steel, 1)], &[(Item::SteelRod, 2)], 300, None, None, 'r'),
    item_recipe("bearing", &[(Item::SteelRod, 1)], &[(Item::Bearing, 2)], 150, None, None, 'b'),
];

/// Winders wind wire into coils, and rods into springs.
pub const WINDER_RECIPES: &[Recipe] = &[
    item_recipe("copper coil", &[(Item::CopperWire, 3)], &[(Item::CopperCoil, 1)], 240, None, None, 'C'),
    item_recipe("spring", &[(Item::IronRod, 2)], &[(Item::Spring, 1)], 300, None, None, 'S'),
];

/// Basic tier (basic assembler and up): motors and pistons, 10 s each.
pub const BASIC_RECIPES: &[Recipe] = &[
    item_recipe(
        "electric motor",
        &[(Item::CopperCoil, 2), (Item::SteelRod, 1), (Item::Bearing, 2)],
        &[(Item::ElectricMotor, 1)],
        600,
        None,
        None,
        'M',
    ),
    item_recipe(
        "piston",
        &[(Item::SteelRod, 1), (Item::HardenedIronPlate, 1), (Item::Screw, 2)],
        &[(Item::Piston, 1)],
        600,
        None,
        None,
        'P',
    ),
];

/// Advanced tier (advanced and precision assemblers). The control board is the x1x1, the
/// weakest microprocessor.
pub const ADVANCED_RECIPES: &[Recipe] = &[item_recipe(
    "control board",
    &[(Item::Relay, 1), (Item::FineWire, 4), (Item::IronSheet, 1)],
    &[(Item::ControlBoard, 1)],
    600,
    None,
    None,
    'C',
)];

/// Precision tier (precision assembler only). Nothing yet: the later processors go here, and
/// the precision assembler runs the advanced tier twice as fast meanwhile.
pub const PRECISION_RECIPES: &[Recipe] = &[];

/// Large basic tier (large assemblers): the big goods, 20 s each.
pub const LARGE_BASIC_RECIPES: &[Recipe] = &[
    item_recipe(
        "pump unit",
        &[(Item::ElectricMotor, 1), (Item::Valve, 2), (Item::SiloShell, 1), (Item::Gauge, 1)],
        &[(Item::PumpUnit, 1)],
        1200,
        None,
        None,
        'U',
    ),
    item_recipe(
        "engine block",
        &[(Item::Frame, 1), (Item::Piston, 4), (Item::Gearbox, 1), (Item::BoilerShell, 1)],
        &[(Item::EngineBlock, 1)],
        1200,
        None,
        None,
        'E',
    ),
];

/// Large advanced tier (large advanced assembler only).
pub const LARGE_ADVANCED_RECIPES: &[Recipe] = &[
    item_recipe(
        "generator",
        &[(Item::Frame, 1), (Item::ElectricMotor, 2), (Item::Gauge, 1), (Item::ControlBoard, 1)],
        &[(Item::Generator, 1)],
        1200,
        None,
        None,
        'G',
    ),
    item_recipe(
        "control panel",
        &[(Item::ControlBoard, 2), (Item::Lamp, 4), (Item::Gauge, 2), (Item::SteelSheet, 1)],
        &[(Item::ControlPanel, 1)],
        1200,
        None,
        None,
        'P',
    ),
    item_recipe(
        "drilling rig",
        &[(Item::SteelGirder, 4), (Item::Gearbox, 2), (Item::ReinforcedConcrete, 4), (Item::ElectricMotor, 1), (Item::PumpUnit, 1)],
        &[(Item::DrillingRig, 1)],
        2400,
        None,
        None,
        'D',
    ),
    item_recipe(
        "steam tractor",
        &[(Item::EngineBlock, 1), (Item::Frame, 1), (Item::HardenedGear, 4), (Item::ControlPanel, 1), (Item::Generator, 1)],
        &[(Item::SteamTractor, 1)],
        3600,
        None,
        None,
        'T',
    ),
];

/// What an assembler can make: each (table, speed) pair in turn, with the table's times
/// divided by the speed (rounded to a whole tick). Built once per assembler kind.
pub fn assembler_list(tiers: &[(&[Recipe], u32)]) -> Vec<Recipe> {
    let mut out = Vec::new();
    for (table, speed) in tiers {
        for r in table.iter() {
            out.push(Recipe { ticks: ((r.ticks as f64 / *speed as f64).round() as u32).max(1), ..*r });
        }
    }
    out
}

/// The welder joins parts. It draws EC only while it works.
pub const WELDER_RECIPES: &[Recipe] = &[
    item_recipe("boiler shell", &[(Item::SteelSheet, 1), (Item::Brick, 4)], &[(Item::BoilerShell, 1)], 600, None, None, 'B'),
    item_recipe("silo shell", &[(Item::SteelSheet, 3), (Item::Valve, 1)], &[(Item::SiloShell, 1)], 600, None, None, 'S'),
    item_recipe("frame", &[(Item::SteelGirder, 2), (Item::Screw, 8)], &[(Item::Frame, 1)], 600, None, None, 'F'),
];

/// The kiln fires 8 runs at once, burning coal. A batch machine fires for (one run's time x batch / 2):
/// 5 s x 8 / 2 = 20 s, so 24 a minute, not the 96 that made it an 8x speed boost in disguise.
pub const KILN_RECIPES: &[Recipe] = &[
    item_recipe("brick", &[(Item::Stone, 2)], &[(Item::Brick, 1)], 1200, Some(Item::Coal), None, 'b'),
    item_recipe("glass", &[(Item::Sand, 3)], &[(Item::Glass, 1)], 1200, Some(Item::Coal), None, 'g'),
];

/// The mixer takes water from a pipe, and items from belts.
pub const MIXER_RECIPES: &[Recipe] = &[
    Recipe {
        name: "concrete",
        inputs: &[(Item::Sand, 2), (Item::Stone, 1)],
        outputs: &[(Item::Concrete, 1)],
        fluid_in: Some((Fluid::Water, 1)),
        fluid_out: None,
        ticks: 300,
        fuel: None,
        needs: None,
        tag: 'c',
    },
    Recipe {
        name: "reinforced concrete",
        inputs: &[(Item::Concrete, 2), (Item::IronRod, 1)],
        outputs: &[(Item::ReinforcedConcrete, 1)],
        fluid_in: Some((Fluid::Water, 1)),
        fluid_out: None,
        ticks: 300,
        fuel: None,
        needs: None,
        tag: 'r',
    },
];
