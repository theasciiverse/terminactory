//! Game content: items, companies, research and recipes.
//! Kept apart from the mechanics so it can move into data files later.
//! Changing these lists changes the save format: bump SAVE_VERSION in sim.rs.

use serde::{Deserialize, Serialize};

pub const ITEMS: usize = 19;
pub const COMPANIES: usize = 10;
pub const TECHS: usize = 14;

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
        }
    }
    pub fn price(self) -> f64 {
        match self {
            Item::CopperOre => 0.5,
            Item::Coal => 0.2,
            Item::CopperPlate => 4.0,
            Item::CopperWire => 2.5,
            Item::CopperPipe => 9.0,
            Item::CopperCoil => 14.0,
            Item::Coke => 2.0,
            Item::IronOre => 2.5,
            Item::IronPlate => 2.0,
            Item::Steel => 22.0,
            // Waste costs money to get rid of.
            Item::Waste => -0.5,
            Item::SteelGirder => 50.0,
            Item::Concrete => 4.0,
            Item::IronGear => 14.0,
            Item::Gearbox => 70.0,
            Item::DrillingRig => 600.0,
            Item::CopperIngot => 2.0,
            Item::IronIngot => 1.5,
            Item::Stone => 0.3,
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
            Item::Steel => c(40.0, 0.996, 0.05),
            // Waste never earns research (worth 0).
            Item::Waste => c(0.0, 0.99, 0.02),
            Item::SteelGirder => c(80.0, 0.996, 0.06),
            Item::Concrete => c(10.0, 0.995, 0.04),
            Item::IronGear => c(30.0, 0.996, 0.05),
            Item::Gearbox => c(120.0, 0.997, 0.08),
            Item::DrillingRig => c(800.0, 0.998, 0.1),
            Item::CopperIngot => c(6.0, 0.995, 0.03),
            Item::IronIngot => c(8.0, 0.995, 0.03),
            Item::Stone => c(1.0, 0.99, 0.02),
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
            // for now: concrete has no recipe until the mixer arrives
            Company::Underhill => Some(Item::SteelGirder),
            Company::Gearwright => Some(Item::Gearbox),
            Company::Swiftline => Some(Item::IronGear),
        }
    }
    /// The letter a company sends when it opens.
    pub fn letter(self) -> &'static str {
        match self {
            Company::Hollowmere => "We've dug these hills for four generations. Our drills are cheap, honest, and they don't complain. Neither should you.",
            Company::Brannock => "Furnaces, molders, belts, panels, poles: if it works, the three of us will sell it to you. If it doesn't, one of us will fix it.",
            Company::Ashgrove => "We hear you've been drawing copper wire. Charming. Copper is pretty, but iron builds the world. Our basic drill cuts ironstone twice as fast as your cheap ones.",
            Company::Cinderhall => "Coke! Now you're cooking. Our blast furnaces melt ironstone fast and turn plate into steel. They leave waste behind, mind. Nobody buys it, so plan where it goes.",
            Company::Wellbourne => "A good pipe is a promise. Set one of our pumps down anywhere and the water will go wherever your pipes do, quietly and at your own pace.",
            Company::Kettering => "Mr Kettering says your coil is crude. Mr Vane says it's charming. We both say: give our boilers water and coal, and our engines will out-charge a field of panels.",
            Company::Pillar => "A girder! Let's see how much it holds. Hm. Enough. Pillar & Sons can raise you a floor above the ground, and when your beams get stronger, we'll raise you higher.",
            Company::Underhill => "A girder? You're one of us. Underhill Excavation digs the floor beneath your feet, cheerfully, and the ore down there is richer than anything on top.",
            Company::Gearwright => "A gearbox with tolerances like these deserves a proper home. Gearwright's assembler will make anything you've learned to make, precisely, and twice as fast.",
            Company::Swiftline => "Gears! Lovely. You know what gears are good for? Speed. Swiftline belts move twice what Brannock's do. Tell the sisters we said hello.",
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
            Tech::Assembler => "Assembler",
            Tech::FastBelts => "Fast belts",
            Tech::Tunnels => "Belt tunnels",
            Tech::LongArms => "Long arm",
            Tech::FilterSplitters => "Filter splitter",
            Tech::Warehouses => "Warehouse",
        }
    }
    pub fn about(self) -> &'static str {
        match self {
            Tech::Press => "a machine that presses ingots into plates, and plates into wire, pipe, coils and later gears and girders",
            Tech::Coking => "lets furnaces bake coal into coke",
            Tech::BasicDrill => "a drill that is faster, and cuts iron twice as fast as a cheap one",
            Tech::Pumps => "pumps that draw water from the ground, and pipes that carry it",
            Tech::Steam => "boilers that turn water and coal into steam, and engines that turn steam into EC",
            Tech::BlastFurnace => "melts iron ore fast with coke, and makes steel (leaving waste)",
            Tech::SupportBeams => "build one floor above the ground, and elevators to reach it",
            Tech::Excavation => "dig one floor below the ground, where ore is richer, and elevators to reach it",
            Tech::Assembler => "the crude assembler, a 4x4 machine that makes any press recipe twice as fast, plus the drilling rig",
            Tech::FastBelts => "belts that move twice as much",
            Tech::Tunnels => "belts that dive under up to 4 tiles: place an entrance, then its exit further along, and build anything in between",
            Tech::LongArms => "an arm that reaches 2 tiles: it grabs from 2 behind and drops 2 ahead, over whatever sits in between",
            Tech::FilterSplitters => "a splitter that sends one chosen item out its front and everything else out its sides",
            Tech::Warehouses => "a 2x2 crate that holds four times as much",
        }
    }
    pub fn company(self) -> Company {
        match self {
            Tech::Press | Tech::Coking => Company::Brannock,
            Tech::BasicDrill => Company::Ashgrove,
            Tech::Pumps => Company::Wellbourne,
            Tech::Steam => Company::Kettering,
            Tech::BlastFurnace => Company::Cinderhall,
            Tech::SupportBeams => Company::Pillar,
            Tech::Excavation => Company::Underhill,
            Tech::Assembler => Company::Gearwright,
            Tech::FastBelts => Company::Swiftline,
            Tech::Tunnels | Tech::LongArms | Tech::FilterSplitters | Tech::Warehouses => Company::Brannock,
        }
    }
    pub fn category(self) -> Category {
        match self {
            Tech::Press | Tech::Coking | Tech::BlastFurnace | Tech::Assembler | Tech::BasicDrill | Tech::Pumps => Category::Create,
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
    /// Research points: the quick way to unlock. Costs grow the further into the tree you go.
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
        }
    }
    /// The free way to unlock: make this many of an item (nothing is used up).
    pub fn milestone(self) -> (Item, u64) {
        match self {
            Tech::Press => (Item::CopperIngot, 50),
            Tech::Coking => (Item::Coal, 150),
            Tech::BasicDrill => (Item::CopperWire, 100),
            Tech::Pumps => (Item::CopperPipe, 40),
            Tech::Steam => (Item::CopperCoil, 20),
            Tech::BlastFurnace => (Item::Coke, 100),
            Tech::SupportBeams => (Item::SteelGirder, 25),
            // for now: concrete has no recipe until the mixer arrives
            Tech::Excavation => (Item::SteelGirder, 50),
            Tech::Assembler => (Item::Gearbox, 10),
            Tech::FastBelts => (Item::IronGear, 120),
            Tech::Tunnels => (Item::CopperPlate, 400),
            Tech::LongArms => (Item::CopperWire, 200),
            Tech::FilterSplitters => (Item::CopperPipe, 60),
            Tech::Warehouses => (Item::Coal, 2000),
        }
    }
}

/// The research screen, in groups.
pub const UNLOCK_GROUPS: [(&str, &[Tech]); 4] = [
    ("Making", &[Tech::Press, Tech::Coking, Tech::BasicDrill, Tech::BlastFurnace, Tech::Assembler]),
    ("Moving", &[Tech::Tunnels, Tech::LongArms, Tech::Warehouses, Tech::FilterSplitters, Tech::FastBelts]),
    ("EC", &[Tech::Pumps, Tech::Steam]),
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
    (Node::Start, None),                          // 0
    (Node::Tech(Tech::Press), Some(0)),           // 1
    (Node::Key(Item::CopperWire), Some(1)),       // 2
    (Node::Tech(Tech::BasicDrill), Some(2)),      // 3
    (Node::Key(Item::CopperCoil), Some(2)),       // 4
    (Node::Tech(Tech::Steam), Some(4)),           // 5
    (Node::Key(Item::CopperPipe), Some(1)),       // 6
    (Node::Tech(Tech::Pumps), Some(6)),           // 7
    (Node::Tech(Tech::Coking), Some(0)),          // 8
    (Node::Key(Item::Coke), Some(8)),             // 9
    (Node::Tech(Tech::BlastFurnace), Some(9)),    // 10
    (Node::Key(Item::SteelGirder), Some(10)),     // 11
    (Node::Tech(Tech::SupportBeams), Some(11)),   // 12
    // for now: Excavation hangs off the girder too (concrete returns with the mixer)
    (Node::Tech(Tech::Excavation), Some(11)),     // 13
    (Node::Key(Item::IronGear), Some(10)),        // 14
    (Node::Tech(Tech::FastBelts), Some(14)),      // 15
    (Node::Key(Item::Gearbox), Some(10)),         // 16
    (Node::Tech(Tech::Assembler), Some(16)),      // 17
    (Node::Key(Item::DrillingRig), Some(17)),     // 18
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

pub const PRESS_RECIPES: &[Recipe] = &[
    item_recipe("copper plate", &[(Item::CopperIngot, 1)], &[(Item::CopperPlate, 1)], 150, None, None, '='),
    item_recipe("iron plate", &[(Item::IronIngot, 1)], &[(Item::IronPlate, 1)], 150, None, None, '-'),
    item_recipe("copper wire", &[(Item::CopperPlate, 1)], &[(Item::CopperWire, 2)], 60, None, None, 'W'),
    item_recipe("copper pipe", &[(Item::CopperPlate, 2)], &[(Item::CopperPipe, 1)], 90, None, None, 'P'),
    item_recipe("copper coil", &[(Item::CopperWire, 4)], &[(Item::CopperCoil, 1)], 120, None, None, 'C'),
    item_recipe("iron gear", &[(Item::IronPlate, 2)], &[(Item::IronGear, 1)], 90, None, Some(Tech::BlastFurnace), 'G'),
    item_recipe("steel girder", &[(Item::Steel, 2)], &[(Item::SteelGirder, 1)], 120, None, Some(Tech::BlastFurnace), 'R'),
    item_recipe("gearbox", &[(Item::IronGear, 3), (Item::Steel, 1)], &[(Item::Gearbox, 1)], 180, None, Some(Tech::BlastFurnace), 'B'),
];

/// Blast furnaces melt iron ore fast, and make steel. Steel leaves waste behind: it comes
/// out the front with the steel and has to go somewhere.
pub const BLAST_RECIPES: &[Recipe] = &[
    Recipe {
        name: "molten iron",
        inputs: &[(Item::IronOre, 1)],
        outputs: &[],
        fluid_in: None,
        fluid_out: Some((Fluid::MoltenIron, 1)),
        ticks: 300,
        fuel: Some(Item::Coke),
        needs: None,
        tag: 'I',
    },
    item_recipe("steel", &[(Item::IronPlate, 3)], &[(Item::Steel, 1), (Item::Waste, 1)], 240, Some(Item::Coke), None, 'S'),
];

/// Assemblers come in tiers. Each recipe table below lists its tier's recipes, with the time
/// the lowest assembler that can make them takes. Higher assemblers know the lower tables too,
/// at a faster speed (see `assembler_list`).
///
/// Crude tier: every press recipe plus the drilling rig.
pub const CRUDE_RECIPES: &[Recipe] = &[
    item_recipe("copper plate", &[(Item::CopperIngot, 1)], &[(Item::CopperPlate, 1)], 75, None, None, '='),
    item_recipe("iron plate", &[(Item::IronIngot, 1)], &[(Item::IronPlate, 1)], 75, None, None, '-'),
    item_recipe("copper wire", &[(Item::CopperPlate, 1)], &[(Item::CopperWire, 2)], 30, None, None, 'W'),
    item_recipe("copper pipe", &[(Item::CopperPlate, 2)], &[(Item::CopperPipe, 1)], 45, None, None, 'P'),
    item_recipe("copper coil", &[(Item::CopperWire, 4)], &[(Item::CopperCoil, 1)], 60, None, None, 'C'),
    item_recipe("iron gear", &[(Item::IronPlate, 2)], &[(Item::IronGear, 1)], 45, None, None, 'G'),
    item_recipe("steel girder", &[(Item::Steel, 2)], &[(Item::SteelGirder, 1)], 60, None, None, 'R'),
    item_recipe("gearbox", &[(Item::IronGear, 3), (Item::Steel, 1)], &[(Item::Gearbox, 1)], 90, None, None, 'B'),
    // for now: steel stands in for concrete until the mixer arrives
    item_recipe(
        "drilling rig",
        &[(Item::SteelGirder, 4), (Item::Gearbox, 2), (Item::Steel, 4)],
        &[(Item::DrillingRig, 1)],
        600,
        None,
        None,
        'D',
    ),
];

// for now: the tiers below hold placeholder recipes so every assembler can be tested.
/// Basic tier (basic assembler and up).
pub const BASIC_RECIPES: &[Recipe] =
    &[item_recipe("bulk copper wire", &[(Item::CopperPlate, 4)], &[(Item::CopperWire, 10)], 120, None, None, 'w')];

/// Advanced tier (advanced and precision assemblers).
pub const ADVANCED_RECIPES: &[Recipe] =
    &[item_recipe("heavy gearbox", &[(Item::IronGear, 6), (Item::Steel, 2)], &[(Item::Gearbox, 3)], 240, None, None, 'b')];

/// Precision tier (precision assembler only).
pub const PRECISION_RECIPES: &[Recipe] =
    &[item_recipe("fine copper coil", &[(Item::CopperWire, 8)], &[(Item::CopperCoil, 3)], 200, None, None, 'c')];

/// Large basic tier (large assemblers).
pub const LARGE_BASIC_RECIPES: &[Recipe] =
    &[item_recipe("bulk girders", &[(Item::Steel, 8)], &[(Item::SteelGirder, 5)], 300, None, None, 'r')];

/// Large advanced tier (large advanced assembler only).
pub const LARGE_ADVANCED_RECIPES: &[Recipe] = &[item_recipe(
    "bulk drilling rigs",
    &[(Item::SteelGirder, 8), (Item::Gearbox, 4), (Item::Steel, 8)],
    &[(Item::DrillingRig, 3)],
    800,
    None,
    None,
    'd',
)];

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

// for now: placeholder recipes for the welder, kiln and mixer.
/// The welder joins parts. It draws EC only while it works.
pub const WELDER_RECIPES: &[Recipe] =
    &[item_recipe("weld girder", &[(Item::Steel, 3), (Item::IronPlate, 2)], &[(Item::SteelGirder, 2)], 180, None, None, 'w')];

/// The kiln fires 8 runs at once, burning coal.
pub const KILN_RECIPES: &[Recipe] =
    &[item_recipe("kiln iron plate", &[(Item::IronIngot, 1)], &[(Item::IronPlate, 1)], 240, Some(Item::Coal), None, 'k')];

/// The mixer takes water from a pipe, and items from belts.
pub const MIXER_RECIPES: &[Recipe] = &[Recipe {
    name: "concrete",
    inputs: &[(Item::Stone, 2)],
    outputs: &[(Item::Concrete, 2)],
    fluid_in: Some((Fluid::Water, 5)),
    fluid_out: None,
    ticks: 120,
    fuel: None,
    needs: None,
    tag: 'c',
}];
