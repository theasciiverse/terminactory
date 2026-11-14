//! Quiet hints and asides for the idle status line.
//!
//! A hint is a diagnosis of something the player already built ("Your drill is out of coal"),
//! never a goal or an order. An aside is a company's one-line remark about what was built. Both
//! take the slot where the key list sits when nothing else is going on, never replace a real
//! message or a stuck-machine reason (`ui::status_slot` ranks the four), fade by themselves, and
//! are switched off together by the Hints setting. Nothing here is saved.
//!
//! Pacing, in real time, all kept in `Hints` (and so reset on restart):
//! - a hint shows once its state has held for `HOLD`, stays `SHOW`, and then waits `REPEAT`
//!   before the same one can come back, at most `MAX_SHOWS` times a session;
//! - an aside shows no sooner than `ASIDE_FIRST` into the session, at most one per `ASIDE_GAP`,
//!   for `ASIDE_SHOW`, each at most `ASIDE_MAX` times, and only in the large (full) layout;
//! - a hint that is ready always wins over an aside.

use std::time::{Duration, Instant};

use crate::content::{Company, Tech};
use crate::controls::{Action, Controls};
use crate::sim::{Item, Kind, SellMode, St, World};

/// How long a hint's state must hold before it shows.
pub const HOLD: Duration = Duration::from_secs(20);
/// How long a hint stays up.
pub const SHOW: Duration = Duration::from_secs(12);
/// How long before the same hint may come back.
pub const REPEAT: Duration = Duration::from_secs(180);
/// The most times one hint shows in a session.
pub const MAX_SHOWS: u8 = 3;
/// No aside sooner than this into the session.
pub const ASIDE_FIRST: Duration = Duration::from_secs(8 * 60);
/// At least this long between any two asides (13-decisions: one per 10 minutes).
pub const ASIDE_GAP: Duration = Duration::from_secs(10 * 60);
pub const ASIDE_SHOW: Duration = Duration::from_secs(10);
pub const ASIDE_MAX: u8 = 2;
/// The world is looked over this often, not every frame.
const SCAN: Duration = Duration::from_secs(1);

/// The hints, in the order they win when several are ready.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hint {
    /// Nothing built on any floor.
    Empty,
    /// A drill, furnace or boiler has no coal.
    OutOfCoal,
    /// Electric parts outside every power range.
    NoEc,
    /// Producers but no trade post.
    NoTradePost,
    /// A drill or machine is full with nothing at its output edge.
    Full,
    /// A recipe machine is waiting for inputs.
    Waiting,
    /// Items sold, a trade post still on money, and no research earned yet.
    ResearchFromSales,
    /// Research points are enough for a part still to research.
    CanResearch,
}

impl Hint {
    const ALL: [Hint; 8] = [
        Hint::Empty,
        Hint::OutOfCoal,
        Hint::NoEc,
        Hint::NoTradePost,
        Hint::Full,
        Hint::Waiting,
        Hint::ResearchFromSales,
        Hint::CanResearch,
    ];
    fn idx(self) -> usize {
        self as usize
    }
}

/// An aside's id: (company that speaks, line). The lines are from plan 08 section 6. The
/// ones that read as nudges (A1, A7, A14) are cut, and so are those for companies and items
/// the game has no asides for yet (Halverson, Meridian, Sablon, Voss, oil and chips).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Aside {
    /// 15+ fast belts.
    A2,
    /// 4 to 9 drills on coal.
    A3,
    /// 10+ drills on coal.
    A4,
    /// Waste waiting somewhere.
    A5,
    /// 3+ blast furnaces.
    A8,
    /// A steam engine and a solar panel.
    A9,
    /// A building on a floor above the ground.
    A10,
    /// A building on a floor below it.
    A11,
    /// 200+ iron gears made and an assembler built.
    A12,
    /// An advanced or large drill built.
    A13,
    /// 500+ copper wire made.
    A15,
}

impl Aside {
    const ALL: [Aside; 11] =
        [Aside::A2, Aside::A3, Aside::A4, Aside::A5, Aside::A8, Aside::A9, Aside::A10, Aside::A11, Aside::A12, Aside::A13, Aside::A15];
    fn idx(self) -> usize {
        self as usize
    }
    /// The company that says it. Only open companies speak.
    pub fn company(self) -> Company {
        match self {
            Aside::A2 => Company::Brannock,
            Aside::A3 | Aside::A4 => Company::Hollowmere,
            Aside::A5 => Company::Cinderhall,
            Aside::A8 => Company::Wellbourne,
            Aside::A9 => Company::Kettering,
            Aside::A10 => Company::Pillar,
            Aside::A11 => Company::Underhill,
            Aside::A12 => Company::Gearwright,
            Aside::A13 => Company::Ampleworth,
            Aside::A15 => Company::Ashgrove,
        }
    }
    /// The whole line, at most 58 columns.
    pub fn text(self) -> &'static str {
        match self {
            Aside::A2 => "Brannock: Swiftline's belts? Ours are still in there.",
            Aside::A3 => "Hollowmere: Good coal. Dad would have wept.",
            Aside::A4 => "Hollowmere: Voss retired. Coal didn't.",
            Aside::A5 => "Cinderhall: Waste? That's just smoke you can carry.",
            Aside::A8 => "Wellbourne: Gently: a little less smoke, perhaps?",
            Aside::A9 => "Kettering: K says engines. V says panels. Both hum.",
            Aside::A10 => "Pillar: Everything up here is rated. Mostly.",
            Aside::A11 => "Underhill: Snug down here, isn't it?",
            Aside::A12 => "Gearwright: Those teeth are even. Don't tell anyone.",
            Aside::A13 => "Ampleworth: It hums. We call that purring.",
            Aside::A15 => "Ashgrove: All that copper. Charming, really.",
        }
    }
}

/// What a look at the world found. Everything the hints and asides decide from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    pub buildings: usize,
    /// Drills and machines that make things.
    pub producers: usize,
    /// A drill, and the name of another machine, that has no coal.
    pub coal_drill: bool,
    pub coal_machine: Option<&'static str>,
    pub unpowered: u32,
    pub trade_posts: usize,
    pub money_posts: usize,
    pub sold: bool,
    pub earned_research: bool,
    pub can_research: bool,
    pub full: bool,
    pub waiting: bool,
    // For the asides.
    pub fast_belts: usize,
    pub coal_drills: usize,
    pub waste: bool,
    pub blast_furnaces: usize,
    pub engines: usize,
    pub solars: usize,
    pub above: bool,
    pub below: bool,
    pub assemblers: usize,
    pub big_drill: bool,
    pub gears_made: u64,
    pub wire_made: u64,
    pub open: [bool; crate::sim::COMPANIES],
}

impl Facts {
    /// Looks over every building on every floor.
    pub fn scan(w: &World) -> Facts {
        let mut f = Facts { unpowered: w.grid.unpowered, ..Facts::default() };
        f.sold = w.econ.sold.iter().any(|n| *n > 0);
        f.earned_research = w.econ.earned_research > 0.0;
        f.can_research = Tech::ALL.iter().any(|t| !w.progress.has(*t) && w.progress.is_open(t.company()) && w.econ.research >= t.cost());
        f.gears_made = w.econ.made[Item::IronGear.idx()];
        f.wire_made = w.econ.made[Item::CopperWire.idx()];
        for c in Company::ALL {
            f.open[c.idx()] = w.progress.is_open(c);
        }
        for (fl, floor) in w.floors.iter().enumerate() {
            let mut any = false;
            for (a, b) in floor.tiles.occupied() {
                any = true;
                f.buildings += 1;
                match b.kind {
                    Kind::TradePost => {
                        f.trade_posts += 1;
                        if matches!(b.st, St::Trade { mode: SellMode::Money, .. }) {
                            f.money_posts += 1;
                        }
                    }
                    Kind::FastBelt => f.fast_belts += 1,
                    Kind::BlastFurnace => f.blast_furnaces += 1,
                    Kind::SteamEngine => f.engines += 1,
                    Kind::Solar | Kind::BasicSolar => f.solars += 1,
                    Kind::CrudeAssembler
                    | Kind::BasicAssembler
                    | Kind::AdvancedAssembler
                    | Kind::PrecisionAssembler
                    | Kind::LargeBasicAssembler
                    | Kind::LargeAdvancedAssembler
                    | Kind::LargePrecisionAssembler => f.assemblers += 1,
                    _ => {}
                }
                if matches!(b.kind, Kind::AdvancedDrill | Kind::LargeBasicDrill | Kind::LargeAdvancedDrill) {
                    f.big_drill = true;
                }
                match &b.st {
                    St::Drill { mines, .. } => {
                        f.producers += 1;
                        if *mines == Some(Item::Coal) {
                            f.coal_drills += 1;
                        }
                    }
                    St::Crafter { .. } => f.producers += 1,
                    St::Belt { items } if items.iter().any(|i| i.item == Item::Waste) => f.waste = true,
                    _ => {}
                }
                let Some(reason) = w.idle_reason(fl, a) else { continue };
                let makes = matches!(b.st, St::Drill { .. } | St::Crafter { .. });
                match reason {
                    "COAL" if b.kind.is_drill() => f.coal_drill = true,
                    "COAL" => f.coal_machine = f.coal_machine.or(Some(b.kind.name())),
                    "FULL" if makes && w.front_open(fl, a) => f.full = true,
                    "NEED" | "LOAD" | "FLUD" => f.waiting = true,
                    "WSTE" | "BACK" => f.waste = true,
                    _ => {}
                }
            }
            let level = floor.level;
            f.above |= any && level > 0;
            f.below |= any && level < 0;
        }
        f
    }

    fn hint_holds(&self, h: Hint) -> bool {
        match h {
            Hint::Empty => self.buildings == 0,
            Hint::OutOfCoal => self.coal_drill || self.coal_machine.is_some(),
            Hint::NoEc => self.unpowered > 0,
            Hint::NoTradePost => self.producers > 0 && self.trade_posts == 0,
            Hint::Full => self.full,
            Hint::Waiting => self.waiting,
            Hint::ResearchFromSales => self.trade_posts > 0 && self.sold && !self.earned_research && self.money_posts > 0,
            Hint::CanResearch => self.can_research,
        }
    }

    fn aside_holds(&self, a: Aside) -> bool {
        if !self.open[a.company().idx()] {
            return false;
        }
        match a {
            Aside::A2 => self.fast_belts >= 15,
            Aside::A3 => (4..10).contains(&self.coal_drills),
            Aside::A4 => self.coal_drills >= 10,
            Aside::A5 => self.waste,
            Aside::A8 => self.blast_furnaces >= 3,
            Aside::A9 => self.engines > 0 && self.solars > 0,
            Aside::A10 => self.above,
            Aside::A11 => self.below,
            Aside::A12 => self.gears_made >= 200 && self.assemblers > 0,
            Aside::A13 => self.big_drill,
            Aside::A15 => self.wire_made >= 500,
        }
    }
}

/// The keys the hint texts mention, taken from controls.json.
pub struct Keys {
    pub setup: String,
    pub tree: String,
    pub book: String,
    pub power: String,
}

impl Keys {
    pub fn of(c: &Controls) -> Keys {
        Keys { setup: c.key(Action::Setup), tree: c.key(Action::Tree), book: c.key(Action::Book), power: c.key(Action::Power) }
    }
}

/// A hint's text, at most 58 columns with the default keys.
pub fn hint_text(h: Hint, f: &Facts, k: &Keys) -> String {
    match h {
        Hint::Empty => format!("1 opens build · a drill mines ore anywhere ({} picks it)", k.setup),
        Hint::OutOfCoal => match f.coal_machine {
            Some(name) if !f.coal_drill => format!("Your {} is out of coal: belt some in", name.to_lowercase()),
            _ => "Your drill is out of coal: put a drill on coal next to it".to_string(),
        },
        Hint::NoEc => format!("Belts and molders need EC: solar + pole (3). {} shows range", k.power),
        Hint::NoTradePost => "No trade post yet: items have nowhere to go".to_string(),
        Hint::Full => "Output is full: put a belt at the arrow side".to_string(),
        Hint::Waiting => format!("{} opens its recipe to see what it needs", k.book),
        Hint::ResearchFromSales => format!("Research comes from selling: {} on a trade post", k.setup),
        Hint::CanResearch => format!("You can afford a new part: {}", k.tree),
    }
}

/// What is on the status line's idle slot right now.
#[derive(Clone, Debug, PartialEq)]
pub struct Current {
    pub text: String,
    pub what: What,
    until: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum What {
    Hint(Hint),
    Aside(Aside),
}

/// Session state of the hints and asides (not saved).
pub struct Hints {
    started: Instant,
    last_scan: Option<Instant>,
    facts: Facts,
    since: [Option<Instant>; Hint::ALL.len()],
    shown_at: [Option<Instant>; Hint::ALL.len()],
    shown: [u8; Hint::ALL.len()],
    aside_since: [Option<Instant>; Aside::ALL.len()],
    aside_shown: [u8; Aside::ALL.len()],
    last_aside: Option<Instant>,
    /// The hint or aside up now, if any.
    pub current: Option<Current>,
}

impl Hints {
    pub fn new(now: Instant) -> Self {
        Hints {
            started: now,
            last_scan: None,
            facts: Facts::default(),
            since: [None; Hint::ALL.len()],
            shown_at: [None; Hint::ALL.len()],
            shown: [0; Hint::ALL.len()],
            aside_since: [None; Aside::ALL.len()],
            aside_shown: [0; Aside::ALL.len()],
            last_aside: None,
            current: None,
        }
    }

    /// The text to show in the idle slot, if a hint or aside is up. An aside only counts in the
    /// large layout.
    pub fn line(&self, large: bool) -> Option<&str> {
        let c = self.current.as_ref()?;
        (large || matches!(c.what, What::Hint(_))).then_some(c.text.as_str())
    }

    /// Called every loop. `on`: the Hints setting. `large`: the full layout is up. `free`: nothing
    /// of the player's (a message, a stuck-machine reason) holds the slot, so a new one may start.
    pub fn update(&mut self, w: &World, controls: &Controls, now: Instant, on: bool, large: bool, free: bool) {
        if !on {
            // Off turns everything off, and timers start again from zero when it comes back on.
            self.current = None;
            self.since = [None; Hint::ALL.len()];
            self.aside_since = [None; Aside::ALL.len()];
            self.last_scan = None;
            return;
        }
        if self.current.as_ref().is_some_and(|c| now >= c.until) {
            self.current = None;
        }
        if self.last_scan.is_none_or(|t| now.saturating_duration_since(t) >= SCAN) {
            self.last_scan = Some(now);
            self.facts = Facts::scan(w);
            for h in Hint::ALL {
                let held = self.facts.hint_holds(h);
                let since = &mut self.since[h.idx()];
                *since = if held { since.or(Some(now)) } else { None };
            }
            for a in Aside::ALL {
                let held = self.facts.aside_holds(a);
                let since = &mut self.aside_since[a.idx()];
                *since = if held { since.or(Some(now)) } else { None };
            }
            // A diagnosis that no longer holds goes at once.
            if let Some(Current { what: What::Hint(h), .. }) = &self.current {
                if self.since[h.idx()].is_none() {
                    self.current = None;
                }
            }
        }
        // An aside that is up when the layout turns small goes too.
        if !large && matches!(self.current.as_ref().map(|c| c.what), Some(What::Aside(_))) {
            self.current = None;
        }
        if self.current.is_some() || !free {
            return;
        }
        if let Some(h) = self.ready_hint(now) {
            self.shown_at[h.idx()] = Some(now);
            self.shown[h.idx()] += 1;
            self.current = Some(Current { text: hint_text(h, &self.facts, &Keys::of(controls)), what: What::Hint(h), until: now + SHOW });
            return;
        }
        // Only when no hint is ready: a hint always wins the slot.
        if large {
            if let Some(a) = self.ready_aside(now) {
                self.last_aside = Some(now);
                self.aside_shown[a.idx()] += 1;
                self.current = Some(Current { text: a.text().to_string(), what: What::Aside(a), until: now + ASIDE_SHOW });
            }
        }
    }

    /// The first hint whose state has held long enough and which is not resting or used up.
    fn ready_hint(&self, now: Instant) -> Option<Hint> {
        Hint::ALL.into_iter().find(|h| {
            let i = h.idx();
            self.since[i].is_some_and(|s| now.saturating_duration_since(s) >= HOLD)
                && self.shown[i] < MAX_SHOWS
                && self.shown_at[i].is_none_or(|t| now.saturating_duration_since(t) >= REPEAT)
        })
    }

    /// The aside to show now, if the pacing allows one: the eligible one whose state became true
    /// most recently.
    fn ready_aside(&self, now: Instant) -> Option<Aside> {
        if now.saturating_duration_since(self.started) < ASIDE_FIRST {
            return None;
        }
        if self.last_aside.is_some_and(|t| now.saturating_duration_since(t) < ASIDE_GAP) {
            return None;
        }
        Aside::ALL
            .into_iter()
            .filter(|a| self.aside_shown[a.idx()] < ASIDE_MAX)
            .filter_map(|a| Some((self.aside_since[a.idx()]?, a)))
            .max_by_key(|(t, _)| *t)
            .map(|(_, a)| a)
    }
}

/// Test only: puts a hint or aside up as if its state had held.
#[cfg(test)]
impl Hints {
    pub fn force(&mut self, what: What, text: &str, now: Instant) {
        let show = if matches!(what, What::Hint(_)) { SHOW } else { ASIDE_SHOW };
        self.current = Some(Current { text: text.to_string(), what, until: now + show });
    }
}

#[cfg(test)]
mod tests;
