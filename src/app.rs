//! Player state and input handling (keyboard + mouse).

use std::sync::LazyLock;
use std::time::Instant;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::controls::{Action, Controls};
use crate::history::Range;
use crate::settings::{Settings, ROWS};
use crate::sim::{
    in_bounds, money_text, rect_of, rect_size, Blueprint, Category, Dir, Kind, PasteReport, Removed, Tech, TileRect, UpState,
    UpgradeReport, Upgraded, World, H, MAX_LIFT, MIN_LIFT, SPAWN, UNLOCK_GROUPS, W,
};

/// Where the area tool is. It marks a rectangle on one floor; the verb comes after.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AreaStage {
    /// Choosing the rectangle: the first corner once it is set (the second follows the cursor).
    Picking { corner: Option<(i32, i32)> },
    /// The rectangle is chosen; a verb key does something with what is in it.
    Chosen { rect: TileRect },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    /// The floor the rectangle is on (changing floors cancels the tool).
    pub floor: usize,
    pub stage: AreaStage,
}

impl Area {
    /// The rectangle being marked, if there is one yet (the cursor is the second corner).
    pub fn rect(&self, cursor: (i32, i32)) -> Option<TileRect> {
        match self.stage {
            AreaStage::Picking { corner } => corner.map(|c| rect_of(c, cursor)),
            AreaStage::Chosen { rect } => Some(rect),
        }
    }
}

/// What can be done with a chosen rectangle: each one is a key (`action`), a word for the hints
/// (`label`) and an arm in `App::area_verb`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AreaVerb {
    /// Copy it to the hand as a blueprint (Space and Enter do it too).
    Copy,
    Remove,
    /// Swap its parts for their next versions (needs the Upgrade planner tech).
    Upgrade,
}

impl AreaVerb {
    pub const ALL: [AreaVerb; 3] = [AreaVerb::Copy, AreaVerb::Remove, AreaVerb::Upgrade];

    pub fn action(self) -> Action {
        match self {
            AreaVerb::Copy => Action::Area,
            AreaVerb::Remove => Action::Remove,
            AreaVerb::Upgrade => Action::Upgrade,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AreaVerb::Copy => "copy",
            AreaVerb::Remove => "remove",
            AreaVerb::Upgrade => "upgrade",
        }
    }
}

/// An upgrade the player was shown and has not yet confirmed: the floor, the rectangle and what
/// the preview said (how many parts, for how much). Pressing the key again does it, as long as
/// the plan still says the same.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UpgradeQuote {
    pub floor: usize,
    pub rect: TileRect,
    pub parts: usize,
    pub cost: f64,
}

/// The blueprint library (`l`): the files in the library folder, and where the player is in them.
/// It sits over the map and the game keeps running.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub entries: Vec<crate::library::Entry>,
    pub pick: usize,
    /// Typing a name to save the blueprint in the hand (or the last copy) under.
    pub typing: Option<String>,
    /// The picked row was asked to be deleted: pressing delete again does it.
    pub armed: bool,
    /// Where each row was drawn, for the mouse.
    pub rows: Vec<(Rect, usize)>,
}

impl Library {
    pub fn new(entries: Vec<crate::library::Entry>) -> Library {
        Library { entries, ..Default::default() }
    }
}

/// The longest name a saved blueprint may have.
const NAME_MAX: usize = 40;

/// Something in the production panel a click can reach.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StatsHit {
    /// A tab: true for History.
    Tab(bool),
    Range(Range),
}

/// The most slots each build category may have (the number keys 1-9 allow more; this is what
/// the menus are made to fit). Slots are filled in order with no gaps, so a category with fewer
/// than its maximum simply has room: Misc slot 3 waits for rail, Create slot 6 for chemistry.
/// The layout is temporary: it changes after a playtest.
pub const MAX_SLOTS: [(Category, usize); 4] = [(Category::Create, 6), (Category::Transport, 5), (Category::Power, 4), (Category::Misc, 3)];

/// The most slots `cat` may have.
pub fn max_slots(cat: Category) -> usize {
    MAX_SLOTS.iter().find(|(c, _)| *c == cat).map_or(4, |(_, n)| *n)
}

/// The numbered slots in a build category. A slot can hold several kinds of the
/// same part (copper drill, basic drill...); pressing its number again switches between them.
pub fn slots(cat: Category) -> &'static [&'static [Kind]] {
    use Kind::*;
    match cat {
        // Create is crowded: six slots (drills, heat, shaping, assembling, water, chips and silicon).
        Category::Create => &[
            &[Drill, BasicDrill, AdvancedDrill, LargeBasicDrill, LargeAdvancedDrill],
            &[Furnace, Molder, BlastFurnace, CokeOven, Kiln, LargeKiln, ArcFurnace],
            &[Press, WireDrawer, Winder, RollingMill, Lathe, Crusher],
            &[CrudeAssembler, BasicAssembler, AdvancedAssembler, PrecisionAssembler, LargeBasicAssembler, LargeAdvancedAssembler, Welder],
            &[Pump, AdvancedPump, Mixer, Evacuator],
            &[Etcher, CrystalGrower, WaferSaw, FineEtcher],
        ],
        // Pipes carry things, so they sit with the belts, in slot 5.
        Category::Transport => &[&[Belt, FastBelt, Tunnel], &[Splitter, Sorter], &[Arm, LongArm], &[Elevator], &[Pipe]],
        Category::Power => &[&[Solar, BasicSolar], &[Pole], &[Boiler], &[SteamEngine]],
        // Slot 3 is kept for rail.
        Category::Misc => &[&[Crate, Warehouse], &[TradePost]],
    }
}

/// Every part in one build category, slot by slot.
#[cfg_attr(not(test), allow(dead_code))]
pub fn parts_in(cat: Category) -> Vec<Kind> {
    slots(cat).iter().flat_map(|s| s.iter().copied()).collect()
}

/// The rows of the research screen, in order, by group.
pub fn tree_rows() -> Vec<Tech> {
    UNLOCK_GROUPS.iter().flat_map(|g| g.1.iter().copied()).collect()
}

/// A page in the recipe book: an item, or a machine that makes or mines items.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Entry {
    Item(crate::sim::Item),
    Part(Kind),
}

/// Parts with their own page in the recipe book, in the order of `Kind::ALL`: every machine
/// that crafts, mines or pumps, and every part with `rules()` to read.
pub static BOOK_PARTS: LazyLock<Vec<Kind>> = LazyLock::new(|| Kind::ALL.iter().copied().filter(|k| k.has_book_page()).collect());

impl Entry {
    pub fn name(self) -> &'static str {
        match self {
            Entry::Item(i) => i.name(),
            Entry::Part(k) => k.name(),
        }
    }
    pub fn is_part(self) -> bool {
        matches!(self, Entry::Part(_))
    }
    /// The pages on one tab (items, or machines), filtered by a search (case doesn't matter).
    pub fn list(query: &str, parts: bool) -> Vec<Entry> {
        let q = query.to_lowercase();
        let all: Vec<Entry> = if parts {
            BOOK_PARTS.iter().map(|&k| Entry::Part(k)).collect()
        } else {
            crate::sim::Item::ALL.iter().map(|&i| Entry::Item(i)).collect()
        };
        all.into_iter().filter(|e| e.name().to_lowercase().contains(&q)).collect()
    }
}

/// Removing waste storage that costs more than this takes a second click.
const WASTE_CONFIRM: f64 = 20.0;

pub struct App {
    pub world: World,
    /// Waste storage the player was warned about: removing it again confirms the fee.
    pub confirm_remove: Option<(usize, i32, i32)>,
    /// The area tool (c), if it is open.
    pub area: Option<Area>,
    /// An area holding waste that the player was warned about: floor, rectangle and the fee
    /// quoted. Removing it again (with a fee no higher) pays.
    pub confirm_area: Option<(usize, TileRect, f64)>,
    /// What the last area deconstruct took, so `z` can put it back (not saved).
    pub last_removed: Option<Removed>,
    /// The upgrade shown by the preview line, waiting for a second press of the key.
    pub confirm_upgrade: Option<UpgradeQuote>,
    /// What the last upgrade swapped, so `z` can swap it back (not saved), and whether it is
    /// newer than `last_removed` (`z` undoes the most recent of the two first).
    pub last_upgrade: Option<Upgraded>,
    pub upgrade_is_latest: bool,
    /// A line from the area tool that the small pane's crumb shows in place of the verb keys
    /// (the preview, or why the verb did nothing): the long form and a short one.
    pub area_note: Option<(String, String)>,
    /// A blueprint in the hand (it and `selected` exclude each other): its ghost follows the cursor.
    pub held: Option<Blueprint>,
    /// The last area copied, kept in memory so the library can save it after the hand is emptied.
    pub last_copy: Option<Blueprint>,
    /// Where a middle-button press started, and whether a middle-drag is marking a rectangle to copy.
    pub middle_from: Option<(i32, i32)>,
    pub quick_area: bool,
    /// The blueprint library, when open (`l`), and the data folder it reads and writes (none in
    /// the demo and in most tests, which say the library isn't available).
    pub library: Option<Library>,
    pub data_dir: Option<std::path::PathBuf>,
    pub cursor: (i32, i32),
    pub floor: usize,
    pub cam: (i32, i32),
    pub cam_ready: bool,
    pub selected: Option<Kind>,
    pub dir: Dir,
    pub paused: bool,
    pub speed: u32,
    pub msg: String,
    pub msg_at: Instant,
    pub help: bool,
    /// Show where EC reaches.
    pub power_view: bool,
    /// The build category whose parts the number keys pick (None = keys pick a category).
    pub menu: Option<Category>,
    /// The research screen is open, and which row is picked.
    pub tree: bool,
    pub tree_pick: usize,
    /// Where its rows were drawn, for the mouse.
    pub tree_rows: Vec<(Rect, usize)>,
    /// Recipe book: open, the page shown, how far it's scrolled, and how many columns the grid had.
    pub book: bool,
    pub book_entry: Entry,
    pub book_scroll: u16,
    pub book_cols: usize,
    /// Search text (`/`), and whether keys are typing into it.
    pub book_query: String,
    pub book_typing: bool,
    /// Links on the page as last drawn (where, and where they go), grid cells and tabs too,
    /// and the pages Backspace goes back to.
    pub book_links: Vec<(Rect, Entry)>,
    pub book_cells: Vec<(Rect, Entry)>,
    pub book_back: Vec<Entry>,
    /// The last item and the last machine looked at, so Tab goes back to them.
    pub book_last: [Entry; 2],
    /// "While you were away" summary waiting to be read.
    pub away: Option<String>,
    /// An agent message being read, and how far it's scrolled.
    pub mail: Option<crate::mail::Message>,
    pub mail_scroll: u16,
    /// The telegram is open over the map (small pane); in the full view it sits in the sidebar.
    pub mail_open: bool,
    /// The telegram log is open over the map (the game keeps running), and the row picked (0 = newest).
    pub log_open: bool,
    pub log_pick: usize,
    /// Where the sidebar telegram was drawn, so the mouse wheel can scroll it.
    pub mail_area: Rect,
    /// Where the sidebar's earnings charts were drawn (none when hidden or in the small-pane view).
    pub graph_areas: Vec<Rect>,
    /// The production panel is open (over the map; the game keeps running), and its scroll.
    pub stats: bool,
    pub stats_scroll: u16,
    /// The panel's History tab is showing (Tab switches), with its chart range, whether the chart
    /// shows research instead of money.
    pub stats_history: bool,
    pub hist_range: Range,
    pub hist_research: bool,
    /// What a click does where the panel was last drawn.
    pub stats_hits: Vec<(Rect, StatsHit)>,
    /// A fixed (seconds since 1970, zone offset) for the History tab, so tests can draw a known date.
    pub clock: Option<(u64, i64)>,
    /// How far the help screen is scrolled (it's taller than a small pane).
    pub help_scroll: u16,
    /// Hints and asides for the idle status line (session only).
    pub hints: crate::hints::Hints,
    /// The settings pane is open over the map (the game keeps running), and the row picked.
    pub settings_open: bool,
    pub settings_pick: usize,
    /// The game's settings (settings.json) and where to save them (none in tests).
    pub settings: Settings,
    pub settings_path: Option<std::path::PathBuf>,
    /// The kind last picked in each slot, so a slot's number brings it back.
    pub slot_pick: Vec<Kind>,
    /// Size of the next elevator (2 to 8).
    pub lift_size: i32,
    /// A setting copied with a middle-click, given to each copy placed.
    pub paste: Option<crate::sim::Setting>,
    /// Key bindings (from controls.json).
    pub controls: Controls,
    /// Ctrl+S was pressed; main saves the game.
    pub save_requested: bool,
    pub drag_last: Option<(i32, i32)>,
    pub quit: bool,
    /// Small-pane view: None follows the terminal size, Some forces it on or off (m).
    pub compact: Option<bool>,
    /// Whether the last frame was drawn in the small-pane view.
    pub compact_now: bool,
    /// Where the map was last drawn, for mouse hit-testing.
    pub map_area: Rect,
}

impl App {
    pub fn new(world: World) -> Self {
        App {
            world,
            confirm_remove: None,
            area: None,
            confirm_area: None,
            last_removed: None,
            confirm_upgrade: None,
            last_upgrade: None,
            upgrade_is_latest: false,
            area_note: None,
            held: None,
            last_copy: None,
            middle_from: None,
            quick_area: false,
            library: None,
            data_dir: None,
            cursor: SPAWN,
            floor: 0,
            cam: (0, 0),
            cam_ready: false,
            selected: None,
            dir: Dir::E,
            paused: false,
            speed: 1,
            msg: "Welcome to Copper Works. Sell at a trade post, spend research on new parts (t). ? = help".into(),
            msg_at: Instant::now(),
            help: false,
            power_view: false,
            menu: None,
            tree: false,
            tree_pick: 0,
            stats: false,
            stats_scroll: 0,
            stats_history: false,
            hist_range: Range::Hours,
            hist_research: false,
            stats_hits: Vec::new(),
            clock: None,
            tree_rows: Vec::new(),
            book: false,
            book_entry: Entry::Item(crate::sim::Item::CopperOre),
            book_scroll: 0,
            book_cols: 1,
            book_query: String::new(),
            book_typing: false,
            book_links: Vec::new(),
            book_cells: Vec::new(),
            book_back: Vec::new(),
            book_last: [Entry::Item(crate::sim::Item::CopperOre), Entry::Part(Kind::Drill)],
            away: None,
            mail: None,
            mail_scroll: 0,
            mail_open: false,
            log_open: false,
            log_pick: 0,
            mail_area: Rect::default(),
            graph_areas: Vec::new(),
            help_scroll: 0,
            hints: crate::hints::Hints::new(Instant::now()),
            settings_open: false,
            settings_pick: 0,
            settings: Settings::default(),
            settings_path: None,
            slot_pick: Vec::new(),
            lift_size: 2,
            paste: None,
            save_requested: false,
            controls: Controls::defaults(),
            drag_last: None,
            quit: false,
            compact: None,
            compact_now: false,
            map_area: Rect::default(),
        }
    }

    /// Lets the hints and asides move on. Called every loop with the current time (tests pass
    /// their own). A new one only starts when nothing of the player's holds the status slot.
    pub fn tick_hints(&mut self, now: Instant) {
        let free = !crate::ui::slot_taken(self, now);
        let on = self.settings.hints == crate::settings::OnOff::On;
        self.hints.update(&self.world, &self.controls, now, on, !self.compact_now, free);
    }

    pub fn say(&mut self, s: impl Into<String>) {
        self.msg = s.into();
        self.msg_at = Instant::now();
    }

    /// Whether `n` opens the waiting telegram over the map (the small pane, or a Note or Ticker
    /// style) instead of it sitting in the sidebar.
    pub fn telegram_over_map(&self) -> bool {
        self.compact_now || self.settings.telegram_style != crate::settings::TelegramStyle::Sidebar
    }

    /// Closes the telegram being read: it counts as read, and the next waiting one comes up.
    pub fn dismiss_mail(&mut self) {
        if let Some(m) = self.mail.take() {
            self.world.mark_read(m.id);
        }
        self.mail_open = false;
    }

    /// Closes the telegram being read and every one waiting. They all stay in the log, marked read.
    pub fn dismiss_all_mail(&mut self) {
        self.dismiss_mail();
        let ids: Vec<u64> = self.world.mail.drain(..).map(|m| m.id).collect();
        for id in ids {
            self.world.mark_read(id);
        }
    }

    /// Keys in the telegram log: pick a row, Enter reads it again, i or Esc closes.
    fn log_key(&mut self, code: KeyCode) {
        let n = self.world.log.len();
        let last = n.saturating_sub(1);
        match code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => self.log_pick = self.log_pick.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('s') => self.log_pick = (self.log_pick + 1).min(last),
            KeyCode::PageUp => self.log_pick = self.log_pick.saturating_sub(8),
            KeyCode::PageDown => self.log_pick = (self.log_pick + 8).min(last),
            KeyCode::Home => self.log_pick = 0,
            KeyCode::End => self.log_pick = last,
            KeyCode::Enter => self.reopen_from_log(self.log_pick),
            KeyCode::Esc => self.log_open = false,
            c if self.controls.action(c) == Some(Action::MailLog) => self.log_open = false,
            _ => {}
        }
    }

    /// Keys in the settings pane: pick a row (arrows, j k, w s), change its value (Left and Right,
    /// Enter), Esc or the settings key closes it. Pause and speed still work.
    fn settings_key(&mut self, code: KeyCode, action: Option<Action>) {
        let last = ROWS.len() - 1;
        self.settings_pick = self.settings_pick.min(last);
        let change = match (code, action) {
            (KeyCode::Up | KeyCode::Char('k'), _) | (_, Some(Action::MoveUp)) => {
                self.settings_pick = if self.settings_pick == 0 { last } else { self.settings_pick - 1 };
                0
            }
            (KeyCode::Down | KeyCode::Char('j'), _) | (_, Some(Action::MoveDown)) => {
                self.settings_pick = if self.settings_pick == last { 0 } else { self.settings_pick + 1 };
                0
            }
            (KeyCode::Char('h'), _) | (_, Some(Action::MoveLeft)) => -1,
            (KeyCode::Char('l'), _) | (_, Some(Action::MoveRight | Action::Place)) => 1,
            (_, Some(Action::Settings | Action::Back)) => {
                self.settings_open = false;
                0
            }
            (_, Some(Action::Pause)) => {
                self.paused = !self.paused;
                0
            }
            (_, Some(Action::Faster)) => {
                self.speed = (self.speed * 2).min(8);
                0
            }
            (_, Some(Action::Slower)) => {
                self.speed = (self.speed / 2).max(1);
                0
            }
            _ => 0,
        };
        if change != 0 {
            (ROWS[self.settings_pick].change)(&mut self.settings, change);
            if let Some(path) = &self.settings_path {
                if let Err(e) = self.settings.save(path) {
                    self.say(format!("Couldn't save settings.json ({e})"));
                }
            }
        }
    }

    /// Shows the logged message `pick` rows from the newest as the current telegram. The one
    /// being read, if unread, goes back to the front of the queue; the log itself is unchanged.
    pub fn reopen_from_log(&mut self, pick: usize) {
        let Some(entry) = self.world.log.iter().rev().nth(pick) else { return };
        let msg = entry.msg.clone();
        self.log_open = false;
        if let Some(cur) = self.mail.take() {
            if self.world.log.iter().any(|e| e.msg.id == cur.id && !e.read) {
                self.world.mail.push_front(cur);
            }
        }
        self.world.mail.retain(|m| m.id != msg.id);
        self.mail = Some(msg);
        self.mail_scroll = 0;
        self.mail_open = self.telegram_over_map();
    }

    pub fn on_key(&mut self, k: KeyEvent) {
        if k.kind != KeyEventKind::Press {
            return;
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.away.is_some() {
            self.away = None;
            return;
        }
        if self.mail_open {
            // Arrows / j k / PgUp PgDn scroll a long message. Only n or Esc closes it (Shift+N closes
            // every waiting telegram), so a stray key can't skip one. It stays in the log either way.
            let lower = match k.code {
                KeyCode::Char(c) if c.is_uppercase() => Some(KeyCode::Char(c.to_lowercase().next().unwrap_or(c))),
                _ => None,
            };
            let is_mail = self.controls.action(k.code).or_else(|| lower.and_then(|l| self.controls.action(l))) == Some(Action::Mail);
            match k.code {
                KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => self.mail_scroll = self.mail_scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('s') => self.mail_scroll += 1,
                KeyCode::PageUp => self.mail_scroll = self.mail_scroll.saturating_sub(10),
                KeyCode::PageDown => self.mail_scroll += 10,
                KeyCode::Esc => self.dismiss_mail(),
                _ if is_mail && lower.is_some() => self.dismiss_all_mail(),
                _ if is_mail => self.dismiss_mail(),
                _ => {}
            }
            return;
        }
        if self.log_open {
            self.log_key(k.code);
            return;
        }
        if self.help {
            match k.code {
                KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => self.help_scroll = self.help_scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('s') => self.help_scroll += 1,
                KeyCode::PageUp => self.help_scroll = self.help_scroll.saturating_sub(10),
                KeyCode::PageDown => self.help_scroll += 10,
                _ => self.help = false,
            }
            return;
        }
        if self.settings_open {
            let action = self.controls.action(k.code);
            self.settings_key(k.code, action);
            return;
        }
        if self.library.is_some() {
            self.library_key(k.code);
            return;
        }
        // Shifted letters arrive uppercase: match them against their lowercase binding too.
        let lower = match k.code {
            KeyCode::Char(c) if c.is_uppercase() => Some(KeyCode::Char(c.to_lowercase().next().unwrap_or(c))),
            _ => None,
        };
        let action = self.controls.action(k.code).or_else(|| lower.and_then(|l| self.controls.action(l)));
        let shift = k.modifiers.contains(KeyModifiers::SHIFT) || lower.is_some();
        // A telegram can be read from over the tree or the book too (they cover the sidebar).
        if (self.tree || (self.book && !self.book_typing)) && action == Some(Action::Mail) && self.mail.is_some() {
            self.mail_open = true;
            return;
        }
        if self.tree {
            self.tree_key(k.code, action);
            return;
        }
        if self.stats && !self.book {
            // Arrows scroll it, o or Esc closes it, pause and speed work without closing it;
            // any other key closes it and goes on to the game.
            match (k.code, action) {
                (KeyCode::Tab | KeyCode::BackTab, _) => self.stats_tab(!self.stats_history),
                _ if self.stats_history && self.history_key(k.code) => {}
                (KeyCode::PageUp, _) => self.stats_scroll = self.stats_scroll.saturating_sub(5),
                (KeyCode::PageDown, _) => self.stats_scroll += 5,
                (_, Some(Action::MoveUp)) if !shift => self.stats_scroll = self.stats_scroll.saturating_sub(1),
                (_, Some(Action::MoveDown)) if !shift => self.stats_scroll += 1,
                (_, Some(Action::Stats | Action::Back)) => self.stats = false,
                (_, Some(Action::Pause)) => self.paused = !self.paused,
                (_, Some(Action::Faster)) => self.speed = (self.speed * 2).min(8),
                (_, Some(Action::Slower)) => self.speed = (self.speed / 2).max(1),
                _ => {
                    self.stats = false;
                    self.on_key(k);
                }
            }
            return;
        }
        if self.book {
            self.book_key(k, action);
            return;
        }
        if ctrl && k.code == KeyCode::Char('s') {
            self.save_requested = true;
            return;
        }
        // PgUp / PgDn scroll the telegram in the sidebar.
        if self.mail.is_some() && matches!(k.code, KeyCode::PageUp | KeyCode::PageDown) {
            self.mail_scroll = if k.code == KeyCode::PageUp { self.mail_scroll.saturating_sub(5) } else { self.mail_scroll + 5 };
            return;
        }
        if self.area.is_some() && self.area_key(k.code, action) {
            return;
        }
        if let KeyCode::Char(c @ '1'..='9') = k.code {
            self.number(c as usize - '0' as usize);
            return;
        }
        let Some(action) = action else { return };
        match action {
            Action::MoveUp | Action::MoveDown | Action::MoveLeft | Action::MoveRight => {
                let d = match action {
                    Action::MoveUp => Dir::N,
                    Action::MoveDown => Dir::S,
                    Action::MoveLeft => Dir::W,
                    _ => Dir::E,
                };
                if shift {
                    self.lay_belt(d)
                } else {
                    self.move_cursor(d)
                }
            }
            Action::Back => {
                // Esc backs out of a menu or a pick first; with nothing to back out of it closes the telegram.
                if self.menu.is_none() && self.selected.is_none() && self.held.is_none() && k.code == KeyCode::Esc && self.mail.is_some() {
                    self.dismiss_mail();
                }
                self.menu = None;
                self.selected = None;
                self.held = None;
            }
            Action::Tree => self.tree = true,
            Action::Stats => {
                self.stats = true;
                self.stats_scroll = 0;
                self.stats_history = false;
            }
            Action::Book => self.open_book(),
            Action::Settings => self.settings_open = true,
            Action::Area => self.start_area(),
            Action::Upgrade => self.start_upgrade(),
            Action::Restore => self.restore_removed(),
            Action::Library => self.open_library(),
            Action::MailLog => {
                self.log_open = true;
                self.log_pick = 0;
            }
            Action::Mail => {
                if self.mail.is_none() {
                    let key = self.controls.key(Action::MailLog);
                    let unread = self.world.unread();
                    self.say(if unread > 0 {
                        format!("No telegrams waiting. {key} opens the log ({unread} unread)")
                    } else {
                        format!("No telegrams waiting. {key} opens the log")
                    });
                } else if shift {
                    self.dismiss_all_mail();
                    self.say("All telegrams dismissed (the log keeps them)");
                } else if self.telegram_over_map() {
                    self.mail_open = true;
                } else {
                    self.dismiss_mail();
                }
            }
            Action::Compact => {
                self.compact = Some(!self.compact_now);
                self.say(if self.compact_now { "Full view" } else { "Small-pane view (m switches back)" });
            }
            Action::Power => {
                self.power_view = !self.power_view;
                self.say(if self.power_view { "Showing EC range (blue)" } else { "EC range hidden" });
            }
            Action::Place => self.place_here(),
            Action::Rotate => self.rotate(shift),
            Action::Remove => self.remove_here(true),
            Action::Setup => {
                let (x, y) = self.cursor;
                match self.world.configure(self.floor, x, y) {
                    Some(m) => self.say(m),
                    None => self.say("Nothing to set up here (arms, trade posts, furnaces, presses)"),
                }
            }
            Action::FloorDown => self.floor_down(),
            Action::FloorUp => self.floor_up(),
            Action::Pause => self.paused = !self.paused,
            Action::Faster => self.speed = (self.speed * 2).min(8),
            Action::Slower => self.speed = (self.speed / 2).max(1),
            Action::Help => {
                self.help = true;
                self.help_scroll = 0;
            }
            Action::Bigger | Action::Smaller => {
                let d = if action == Action::Bigger { 1 } else { -1 };
                self.lift_size = (self.lift_size + d).clamp(MIN_LIFT, MAX_LIFT);
                let s = self.lift_size;
                self.say(format!("Next elevator: {s}x{s}, holds {} items", 10 * s * s));
            }
        }
    }

    /// Number keys: outside a menu they open a category; inside one they pick a part.
    pub fn number(&mut self, n: usize) {
        match self.menu {
            None => {
                let Some(&cat) = Category::ALL.get(n - 1) else {
                    return;
                };
                self.held = None;
                self.menu = Some(cat);
                let count = slots(cat).len();
                debug_assert!(count <= max_slots(cat), "{cat:?} has more slots than the menus are made for");
                // The small-pane status line already lists the slots, so it needs no message.
                if !self.compact_now {
                    let back = self.controls.label(Action::Back);
                    self.say(format!("{}: press 1-{count} to pick, again to switch kind, {back} to go back", cat.name()));
                }
            }
            Some(cat) => {
                let Some(&slot) = slots(cat).get(n - 1) else {
                    return;
                };
                let open = |k: &Kind| self.world.progress.part_unlocked(*k);
                let kind = match self.selected.and_then(|s| slot.iter().position(|k| *k == s)) {
                    // Already holding something from this slot: move on to its next unlocked kind.
                    Some(i) => (1..=slot.len()).map(|d| slot[(i + d) % slot.len()]).find(open).unwrap_or(slot[i]),
                    // Otherwise bring back what was last picked here, or the first unlocked kind.
                    None => self
                        .slot_pick
                        .iter()
                        .copied()
                        .find(|k| slot.contains(k))
                        .filter(open)
                        .or_else(|| slot.iter().copied().find(open))
                        .unwrap_or(slot[0]),
                };
                if self.selected == Some(kind) && slot.len() > 1 {
                    let others: Vec<&str> = slot.iter().filter(|k| **k != kind).map(|k| k.name()).collect();
                    self.say(format!("{} is still locked", others.join(", ")));
                    return;
                }
                self.pick_part(kind);
                if self.selected == Some(kind) {
                    self.slot_pick.retain(|k| !slot.contains(k));
                    self.slot_pick.push(kind);
                }
            }
        }
    }

    fn pick_part(&mut self, kind: Kind) {
        if !self.world.progress.part_unlocked(kind) {
            let why = match kind.tech() {
                Some(t) if !self.world.progress.is_open(t.company()) => {
                    format!("make {} first, then research it (t)", crate::content::keys_text(t.requires()))
                }
                _ => "research it first (t)".into(),
            };
            self.say(format!("{} is locked: {}", kind.name(), why));
            return;
        }
        self.selected = Some(kind);
        self.held = None;
        self.cancel_area();
        self.paste = None;
        let note = if kind.preview() { " (preview: not in the base tier)" } else { "" };
        // The small-pane status line shows the price, size and facing instead.
        if !self.compact_now {
            self.say(format!("{} selected, ${:.0}{}", kind.name(), kind.cost(), note));
        }
    }

    /// Arrow keys walk the tree: left to the parent, right to the first child,
    /// up and down to the nearest box in the same column.
    /// Opens the recipe book on what's under the cursor: a machine's recipe, what a drill
    /// mines, or the machine itself.
    fn open_book(&mut self) {
        let (x, y) = self.cursor;
        let here = self.world.tile(self.floor, x, y).and_then(|b| match &b.st {
            crate::sim::St::Drill { mines: Some(m), .. } => Some(Entry::Item(*m)),
            _ => b
                .recipe()
                .and_then(|r| r.outputs.first())
                .map(|o| Entry::Item(o.0))
                .or(BOOK_PARTS.contains(&b.kind).then_some(Entry::Part(b.kind))),
        });
        if let Some(e) = here {
            self.book_query.clear();
            self.book_show(e);
        }
        self.book_scroll = 0;
        self.book_back.clear();
        self.book = true;
    }

    /// Shows a page (and so its tab), remembering it as that tab's last page.
    pub fn book_show(&mut self, e: Entry) {
        self.book_entry = e;
        self.book_last[e.is_part() as usize] = e;
        self.book_scroll = 0;
    }

    /// Goes to a page from a link, remembering this one for Backspace.
    pub fn book_go(&mut self, e: Entry) {
        if e == self.book_entry {
            return;
        }
        self.book_back.push(self.book_entry);
        if !Entry::list(&self.book_query, e.is_part()).contains(&e) {
            self.book_query.clear();
        }
        self.book_show(e);
    }

    /// Switches between the items tab and the machines tab.
    /// Switches the production panel between its Production and History tabs.
    pub fn stats_tab(&mut self, history: bool) {
        self.stats_history = history;
        self.stats_scroll = 0;
    }

    /// Keys on the History tab: 1 to 4 pick the chart's range, r swaps money and research
    /// (when the pane is too short to show both). True when the key was used.
    fn history_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char(c @ '1'..='4') => self.hist_range = Range::ALL[c as usize - '1' as usize],
            KeyCode::Char('r') => self.hist_research = !self.hist_research,
            _ => return false,
        }
        true
    }

    fn book_switch_tab(&mut self) {
        let parts = !self.book_entry.is_part();
        let list = Entry::list(&self.book_query, parts);
        let last = self.book_last[parts as usize];
        let e = if list.contains(&last) { Some(last) } else { list.first().copied() };
        match e {
            Some(e) => self.book_show(e),
            // Nothing on the other tab matches the search: show all of it.
            None => {
                self.book_query.clear();
                self.book_show(last);
            }
        }
    }

    fn book_key(&mut self, k: KeyEvent, action: Option<Action>) {
        if self.book_typing {
            // Typing a search: letters filter the grid; Enter or Esc stop typing.
            match k.code {
                KeyCode::Char(c) => self.book_query.push(c),
                KeyCode::Backspace => {
                    self.book_query.pop();
                }
                KeyCode::Enter | KeyCode::Esc => self.book_typing = false,
                _ => {}
            }
            // Stay on this tab if anything here matches, otherwise try the other one.
            let parts = self.book_entry.is_part();
            let here = Entry::list(&self.book_query, parts);
            let there = Entry::list(&self.book_query, !parts);
            if !here.contains(&self.book_entry) {
                if let Some(&first) = here.first().or(there.first()) {
                    self.book_show(first);
                }
            }
            return;
        }
        let list = Entry::list(&self.book_query, self.book_entry.is_part());
        let cols = self.book_cols.max(1);
        let sel = list.iter().position(|e| *e == self.book_entry).unwrap_or(0);
        let next = match (k.code, action) {
            (KeyCode::Char('/'), _) => {
                self.book_typing = true;
                return;
            }
            (KeyCode::Tab | KeyCode::BackTab, _) => {
                self.book_switch_tab();
                return;
            }
            (KeyCode::Esc, _) if !self.book_query.is_empty() => {
                self.book_query.clear();
                return;
            }
            (KeyCode::PageUp, _) => {
                self.book_scroll = self.book_scroll.saturating_sub(5);
                return;
            }
            (KeyCode::PageDown, _) => {
                self.book_scroll += 5;
                return;
            }
            (KeyCode::Backspace, _) => {
                if let Some(e) = self.book_back.pop() {
                    self.book_show(e);
                }
                return;
            }
            (_, Some(Action::MoveLeft)) => sel.checked_sub(1),
            (_, Some(Action::MoveRight)) => Some(sel + 1),
            (_, Some(Action::MoveUp)) => sel.checked_sub(cols),
            (_, Some(Action::MoveDown)) => Some(sel + cols),
            (_, Some(Action::Book | Action::Back)) => {
                self.book = false;
                None
            }
            _ => None,
        };
        if let Some(&e) = next.and_then(|j| list.get(j)) {
            self.book_show(e);
        }
    }

    /// Clicks in the book: a link goes there (Backspace comes back), a grid cell or tab
    /// shows that page; the wheel scrolls the page.
    fn book_mouse(&mut self, m: MouseEvent) {
        let at = ratatui::layout::Position::new(m.column, m.row);
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(&(_, e)) = self.book_links.iter().find(|(r, _)| r.contains(at)) {
                    self.book_go(e);
                } else if let Some(&(_, e)) = self.book_cells.iter().find(|(r, _)| r.contains(at)) {
                    self.book_show(e);
                }
            }
            MouseEventKind::ScrollUp => self.book_scroll = self.book_scroll.saturating_sub(2),
            MouseEventKind::ScrollDown => self.book_scroll += 2,
            _ => {}
        }
    }

    /// What's picked on the research screen.
    pub fn tree_choice(&self) -> Tech {
        let rows = tree_rows();
        rows[self.tree_pick.min(rows.len() - 1)]
    }

    /// The research screen: up/down pick, Enter researches.
    fn tree_key(&mut self, code: KeyCode, action: Option<Action>) {
        let n = tree_rows().len();
        let pick = &mut self.tree_pick;
        match (code, action) {
            (KeyCode::PageUp, _) => *pick = pick.saturating_sub(5),
            (KeyCode::PageDown, _) => *pick = (*pick + 5).min(n - 1),
            (KeyCode::Home, _) => *pick = 0,
            (KeyCode::End, _) => *pick = n - 1,
            (_, Some(Action::MoveUp)) => *pick = pick.saturating_sub(1),
            (_, Some(Action::MoveDown)) => *pick = (*pick + 1).min(n - 1),
            (_, Some(Action::Place)) => self.buy_research(),
            (_, Some(Action::Tree | Action::Back)) => self.tree = false,
            _ => {}
        }
    }

    fn buy_research(&mut self) {
        let t = self.tree_choice();
        match self.world.research(t) {
            Ok(()) => self.say(format!("Researched {}: {}", t.name(), t.about())),
            Err(e) => self.say(e),
        }
    }

    fn tree_mouse(&mut self, m: MouseEvent) {
        let at = ratatui::layout::Position::new(m.column, m.row);
        let n = tree_rows().len();
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(&(_, i)) = self.tree_rows.iter().find(|(r, _)| r.contains(at)) {
                    // Click picks a row; clicking the picked row again researches it.
                    if self.tree_pick == i {
                        self.buy_research();
                    }
                    self.tree_pick = i;
                }
            }
            MouseEventKind::ScrollUp => self.tree_pick = self.tree_pick.saturating_sub(1),
            MouseEventKind::ScrollDown => self.tree_pick = (self.tree_pick + 1).min(n - 1),
            _ => {}
        }
    }

    pub fn on_mouse(&mut self, m: MouseEvent) {
        let over_mail =
            self.mail.is_some() && !self.compact_now && self.mail_area.contains(ratatui::layout::Position::new(m.column, m.row));
        if self.log_open {
            match m.kind {
                MouseEventKind::ScrollUp => self.log_pick = self.log_pick.saturating_sub(1),
                MouseEventKind::ScrollDown => self.log_pick = (self.log_pick + 1).min(self.world.log.len().saturating_sub(1)),
                _ => {}
            }
            return;
        }
        if (self.mail_open || over_mail) && self.away.is_none() {
            match m.kind {
                MouseEventKind::ScrollUp => self.mail_scroll = self.mail_scroll.saturating_sub(2),
                MouseEventKind::ScrollDown => self.mail_scroll += 2,
                _ => {}
            }
            return;
        }
        if self.book && !self.mail_open && self.away.is_none() && !self.help {
            self.book_mouse(m);
            return;
        }
        if self.tree && !self.mail_open && self.away.is_none() && !self.help {
            self.tree_mouse(m);
            return;
        }
        if self.library.is_some() && !self.help && self.away.is_none() && !self.tree && !self.book {
            self.library_mouse(m);
            return;
        }
        if self.tree || self.book || self.help || self.settings_open || self.away.is_some() {
            return;
        }
        if self.stats {
            // The panel covers the map: the wheel scrolls it and a click closes it.
            match m.kind {
                MouseEventKind::ScrollUp => self.stats_scroll = self.stats_scroll.saturating_sub(1),
                MouseEventKind::ScrollDown => self.stats_scroll += 1,
                MouseEventKind::Down(_) => match self.stats_hits.iter().find(|(r, _)| r.contains((m.column, m.row).into())) {
                    Some((_, StatsHit::Tab(h))) => self.stats_tab(*h),
                    Some((_, StatsHit::Range(r))) => self.hist_range = *r,
                    None => self.stats = false,
                },
                _ => {}
            }
            return;
        }
        let tile = self.screen_to_tile(m.column, m.row);
        match m.kind {
            MouseEventKind::ScrollUp => self.floor_up(),
            MouseEventKind::ScrollDown => self.floor_down(),
            MouseEventKind::Moved => {
                if let Some(t) = tile {
                    self.cursor = t;
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(t) = tile {
                    self.cursor = t;
                    if self.area.is_some() {
                        self.area_click(t);
                    } else if self.selected.is_some() || self.held.is_some() {
                        self.place_here();
                    }
                    self.drag_last = Some(t);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let (Some(t), Some(last)) = (tile, self.drag_last) {
                    if t != last {
                        if self.selected.is_some_and(|k| k.is_belt()) {
                            self.belt_path(last, t);
                        } else if self.selected.is_some() {
                            self.cursor = t;
                            self.place_here();
                        }
                        self.cursor = t;
                        self.drag_last = Some(t);
                    }
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                // Letting go on another tile than the first corner finishes a dragged rectangle.
                if let Some(Area { stage: AreaStage::Picking { corner: Some(c) }, .. }) = self.area {
                    let t = tile.unwrap_or(self.cursor);
                    if self.drag_last.is_some() && t != c {
                        self.cursor = t;
                        self.area_set_corner();
                    }
                }
                self.drag_last = None;
            }
            MouseEventKind::Down(MouseButton::Middle) => {
                if let Some(t) = tile {
                    self.cursor = t;
                    self.middle_from = Some(t);
                    self.copy_here();
                }
            }
            MouseEventKind::Drag(MouseButton::Middle) => {
                // Dragging with the middle button marks a rectangle to copy straight to the hand.
                if let (Some(t), Some(from)) = (tile, self.middle_from) {
                    if t != from && !self.quick_area && self.world.progress.has(Tech::Blueprints) {
                        self.selected = None;
                        self.menu = None;
                        self.paste = None;
                        self.held = None;
                        self.confirm_area = None;
                        self.quick_area = true;
                        self.area = Some(Area { floor: self.floor, stage: AreaStage::Picking { corner: Some(from) } });
                    }
                    if self.quick_area {
                        self.cursor = t;
                    }
                }
            }
            MouseEventKind::Up(MouseButton::Middle) => {
                let from = self.middle_from.take();
                let t = tile.unwrap_or(self.cursor);
                if self.quick_area {
                    if let Some(Area { floor, stage: AreaStage::Picking { corner: Some(c) } }) = self.area {
                        self.cancel_area();
                        self.cursor = t;
                        self.copy_rect(floor, rect_of(c, t));
                    }
                    self.quick_area = false;
                } else if from.is_some_and(|f| f != t) && !self.world.progress.has(Tech::Blueprints) {
                    self.say_blueprints_locked();
                }
            }
            MouseEventKind::Down(MouseButton::Right) | MouseEventKind::Drag(MouseButton::Right) if self.area.is_some() => {
                self.cancel_area();
            }
            MouseEventKind::Down(MouseButton::Right) | MouseEventKind::Drag(MouseButton::Right) => {
                if let Some(t) = tile {
                    self.cursor = t;
                    // Only a fresh click confirms a waste fee, never a drag passing over it.
                    self.remove_here(matches!(m.kind, MouseEventKind::Down(_)));
                }
            }
            _ => {}
        }
    }

    fn screen_to_tile(&self, col: u16, row: u16) -> Option<(i32, i32)> {
        let a = self.map_area;
        if col < a.x || row < a.y || col >= a.x + (a.width / 2) * 2 || row >= a.y + a.height {
            return None;
        }
        let t = (self.cam.0 + ((col - a.x) / 2) as i32, self.cam.1 + (row - a.y) as i32);
        in_bounds(t.0, t.1).then_some(t)
    }

    fn move_cursor(&mut self, d: Dir) {
        let (dx, dy) = d.delta();
        self.cursor = ((self.cursor.0 + dx).clamp(0, W - 1), (self.cursor.1 + dy).clamp(0, H - 1));
    }

    /// Places a belt facing `d` (or turns the belt already here), then steps forward.
    fn lay_belt(&mut self, d: Dir) {
        let (x, y) = self.cursor;
        self.dir = d;
        if self.set_or_place_belt(x, y, d) {
            self.move_cursor(d);
        }
    }

    fn belt_path(&mut self, from: (i32, i32), to: (i32, i32)) {
        let (mut x, mut y) = from;
        while (x, y) != to {
            let d = if x != to.0 {
                if to.0 > x {
                    Dir::E
                } else {
                    Dir::W
                }
            } else if to.1 > y {
                Dir::S
            } else {
                Dir::N
            };
            self.dir = d;
            if !self.set_or_place_belt(x, y, d) {
                return;
            }
            let (dx, dy) = d.delta();
            x += dx;
            y += dy;
        }
        self.set_or_place_belt(x, y, self.dir);
    }

    fn set_or_place_belt(&mut self, x: i32, y: i32, d: Dir) -> bool {
        // Lays whichever belt is picked (fast belts if those are in hand).
        let belt = self.selected.filter(|k| k.is_belt()).unwrap_or(Kind::Belt);
        match self.world.tile(self.floor, x, y).map(|b| b.kind) {
            None => match self.world.place(self.floor, x, y, belt, d) {
                Ok(()) => true,
                Err(e) => {
                    self.say(e);
                    false
                }
            },
            Some(k) if k.is_belt() => {
                let _ = self.world.set_dir(self.floor, x, y, d);
                true
            }
            Some(_) => true, // step over other buildings
        }
    }

    fn place_here(&mut self) {
        if let Some(bp) = self.held.clone() {
            let (x, y) = self.cursor;
            let rep = self.world.paste(self.floor, x, y, &bp);
            self.say(paste_message(&rep));
            return;
        }
        let Some(kind) = self.selected else {
            self.say("Pick a part from the build list first");
            return;
        };
        let (x, y) = self.cursor;
        match self.world.place_sized(self.floor, x, y, kind, self.dir, self.lift_size) {
            Ok(()) => {
                if let Some(set) = self.paste {
                    self.world.apply_setting(self.floor, x, y, set);
                }
            }
            Err(e) => self.say(e),
        }
    }

    /// Middle-click: picks up the part under the cursor, facing and settings included,
    /// so clicks place copies of it. On an empty tile it empties your hand.
    fn copy_here(&mut self) {
        let (x, y) = self.cursor;
        let had_blueprint = self.held.take().is_some();
        let Some(b) = self.world.tile(self.floor, x, y) else {
            if self.selected.take().is_some() || had_blueprint {
                self.menu = None;
                self.say("Hand empty");
            }
            return;
        };
        let (kind, dir, size) = (b.kind, b.dir, b.dims().0);
        let set = self.world.setting_at(self.floor, x, y);
        self.pick_part(kind);
        if self.selected != Some(kind) {
            return; // locked; pick_part said why
        }
        self.menu = Category::ALL.iter().copied().find(|c| slots(*c).iter().any(|s| s.contains(&kind)));
        if kind.directional() {
            self.dir = dir;
        }
        if kind == Kind::Elevator {
            self.lift_size = size;
        }
        self.paste = set;
        let what = match (set, copied_recipe(kind, set)) {
            (_, Some(r)) => format!(", makes {r}"),
            (Some(crate::sim::Setting::Filter(Some(i))), _) => format!(", filter {}", i.name()),
            (Some(crate::sim::Setting::Sell(m)), _) => format!(", {}", m.label()),
            _ => String::new(),
        };
        self.say(format!("Copied {}{what} · click to place", kind.name()));
    }

    /// `fresh` is a key press or a new click (not a drag), which may confirm a waste fee.
    fn remove_here(&mut self, fresh: bool) {
        let (x, y) = self.cursor;
        let here = (self.floor, x, y);
        let (n, fee) = self.world.removal_fee(self.floor, x, y);
        if fee > WASTE_CONFIRM && !(fresh && self.confirm_remove == Some(here)) {
            if self.confirm_remove != Some(here) {
                self.confirm_remove = Some(here);
                self.say(format!("Holds {n} waste: removing it costs {}. Remove again to pay", crate::sim::money_text(fee)));
            }
            return;
        }
        self.confirm_remove = None;
        match self.world.try_remove(self.floor, x, y) {
            Ok(Some(k)) => self.say(format!("Removed {} (refunded in full)", k.name())),
            Ok(None) => {}
            Err(why) => self.say(why),
        }
    }

    /// `c`: starts marking a rectangle. The hand is emptied (the tool and a held part exclude each other).
    fn start_area(&mut self) {
        self.selected = None;
        self.held = None;
        self.menu = None;
        self.paste = None;
        self.forget_area_notes();
        self.area = Some(Area { floor: self.floor, stage: AreaStage::Picking { corner: None } });
        let (place, back) = (self.controls.key(Action::Place), self.controls.key(Action::Back));
        self.say(format!("Area: {place} marks a corner (or drag), then the opposite one · {back} cancels"));
    }

    /// Closes the area tool and forgets any waste warning and upgrade preview.
    fn cancel_area(&mut self) {
        if self.area.take().is_some() {
            self.forget_area_notes();
        }
        self.quick_area = false;
    }

    /// Forgets what a verb asked the player to confirm, and what it said about the rectangle.
    fn forget_area_notes(&mut self) {
        self.confirm_area = None;
        self.confirm_upgrade = None;
        self.area_note = None;
    }

    /// The area's rectangle right now (the cursor is the second corner while picking).
    pub fn area_rect(&self) -> Option<TileRect> {
        self.area.and_then(|a| a.rect(self.cursor))
    }

    /// A corner is set at the cursor: the first one, or the opposite one that chooses the rectangle.
    fn area_set_corner(&mut self) {
        let Some(area) = self.area.as_mut() else { return };
        let at = self.cursor;
        match area.stage {
            AreaStage::Picking { corner: None } => area.stage = AreaStage::Picking { corner: Some(at) },
            AreaStage::Picking { corner: Some(c) } => {
                let rect = rect_of(c, at);
                area.stage = AreaStage::Chosen { rect };
                self.forget_area_notes();
                let (w, h) = rect_size(rect);
                let info = self.world.area_info(self.area.map_or(self.floor, |a| a.floor), rect);
                if info.parts == 0 {
                    self.say(format!("{w}x{h} area, nothing in it · {} cancels", self.controls.key(Action::Back)));
                } else {
                    let verbs = self.area_verb_hints();
                    self.say(format!(
                        "{w}x{h} area, {} part{} · {verbs} · {} cancels",
                        info.parts,
                        plural(info.parts),
                        self.controls.key(Action::Back)
                    ));
                }
            }
            AreaStage::Chosen { .. } => {}
        }
    }

    /// A left click with the tool open: sets a corner, or starts the rectangle over once one is chosen.
    fn area_click(&mut self, t: (i32, i32)) {
        match self.area.map(|a| a.stage) {
            Some(AreaStage::Chosen { .. }) => {
                self.forget_area_notes();
                if let Some(a) = self.area.as_mut() {
                    a.stage = AreaStage::Picking { corner: Some(t) };
                }
            }
            _ => self.area_set_corner(),
        }
    }

    /// "x remove" for each verb, for messages and hints.
    pub fn area_verb_hints(&self) -> String {
        AreaVerb::ALL.iter().map(|v| format!("{} {}", self.controls.key(v.action()), v.label())).collect::<Vec<_>>().join("  ")
    }

    /// Keys while the area tool is open. Returns true when the key was used. Movement only moves
    /// (Shift never lays belts here), digits close the tool and go on to the build menu, and
    /// anything else that isn't the tool's (help, pause, speed, floors...) carries on as usual.
    fn area_key(&mut self, code: KeyCode, action: Option<Action>) -> bool {
        if matches!(code, KeyCode::Char('1'..='9')) {
            self.cancel_area();
            return false;
        }
        let Some(action) = action else { return true };
        match action {
            Action::MoveUp => self.move_cursor(Dir::N),
            Action::MoveDown => self.move_cursor(Dir::S),
            Action::MoveLeft => self.move_cursor(Dir::W),
            Action::MoveRight => self.move_cursor(Dir::E),
            Action::Back => {
                self.cancel_area();
                self.say("Area cancelled");
            }
            // Space and Enter set corners, and once the rectangle is chosen they copy it, like c.
            Action::Place => match self.area.map(|a| a.stage) {
                Some(AreaStage::Chosen { rect }) => self.area_verb(AreaVerb::Copy, rect),
                _ => self.area_set_corner(),
            },
            Action::Area => match self.area.map(|a| a.stage) {
                Some(AreaStage::Chosen { rect }) => self.area_verb(AreaVerb::Copy, rect),
                _ => self.say("Mark the rectangle first"),
            },
            Action::Rotate | Action::Setup | Action::Bigger | Action::Smaller => {}
            a => {
                let Some(verb) = AreaVerb::ALL.iter().copied().find(|v| v.action() == a) else { return false };
                match self.area.map(|a| a.stage) {
                    Some(AreaStage::Chosen { rect }) => self.area_verb(verb, rect),
                    _ => self.say("Mark the rectangle first: corner, then the opposite corner"),
                }
            }
        }
        true
    }

    /// Does `verb` to the chosen rectangle.
    fn area_verb(&mut self, verb: AreaVerb, rect: TileRect) {
        match verb {
            AreaVerb::Copy => self.area_copy(rect),
            AreaVerb::Remove => self.area_remove(rect),
            AreaVerb::Upgrade => self.area_upgrade(rect),
        }
    }

    /// `u` outside the area tool: starts it (the verb comes after the rectangle), or says what
    /// unlocks upgrading.
    fn start_upgrade(&mut self) {
        if !self.world.progress.has(Tech::UpgradePlanner) {
            self.say_upgrades_locked();
            return;
        }
        self.start_area();
        let (place, key) = (self.controls.key(Action::Place), self.controls.key(Action::Upgrade));
        self.say(format!("Upgrade: {place} marks a corner, then the opposite one · {key} swaps the parts in it for their next versions"));
    }

    /// `l`: opens the library over the map. It reads the folder each time, so files dropped in by
    /// hand show up.
    fn open_library(&mut self) {
        if !self.world.progress.has(Tech::Blueprints) {
            self.say_blueprints_locked();
            return;
        }
        let Some(data) = self.data_dir.clone() else {
            self.say("The blueprint library needs the game's data folder: it isn't available here");
            return;
        };
        self.library = Some(Library::new(crate::library::list(&data)));
    }

    /// What `f` in the library would save: the blueprint in the hand, or else the last one copied.
    pub fn savable(&self) -> Option<(&Blueprint, bool)> {
        self.held.as_ref().map(|b| (b, true)).or_else(|| self.last_copy.as_ref().map(|b| (b, false)))
    }

    /// Reads the folder again, keeping the pick on the same file when it is still there.
    fn library_refresh(&mut self, keep: Option<std::path::PathBuf>) {
        let Some(data) = self.data_dir.clone() else { return };
        let Some(lib) = self.library.as_mut() else { return };
        let was = keep.or_else(|| lib.entries.get(lib.pick).map(|e| e.path.clone()));
        lib.entries = crate::library::list(&data);
        lib.pick =
            was.and_then(|p| lib.entries.iter().position(|e| e.path == p)).unwrap_or(lib.pick.min(lib.entries.len().saturating_sub(1)));
        lib.armed = false;
    }

    /// Keys in the library: pick (arrows, w s, j k), Enter takes the picked blueprint into the hand,
    /// `f` saves the hand under a name you type, delete removes the picked file after asking twice,
    /// Esc or `l` closes. Pause and speed work; the game keeps running.
    fn library_key(&mut self, code: KeyCode) {
        let action = self.controls.action(code);
        // Typing a name: letters append, Backspace removes one, Enter saves, Esc stops.
        if let Some(buf) = self.library.as_mut().and_then(|l| l.typing.as_mut()) {
            match code {
                KeyCode::Char(c) if !c.is_control() => {
                    if buf.chars().count() < NAME_MAX {
                        buf.push(c);
                    }
                }
                KeyCode::Backspace => {
                    buf.pop();
                }
                KeyCode::Enter => self.library_save(),
                KeyCode::Esc => {
                    if let Some(l) = self.library.as_mut() {
                        l.typing = None;
                    }
                }
                _ => {}
            }
            return;
        }
        let Some(lib) = self.library.as_mut() else { return };
        let last = lib.entries.len().saturating_sub(1);
        let was_armed = std::mem::take(&mut lib.armed);
        match (code, action) {
            (KeyCode::Up | KeyCode::Char('k'), _) | (_, Some(Action::MoveUp)) => lib.pick = lib.pick.saturating_sub(1),
            (KeyCode::Down | KeyCode::Char('j'), _) | (_, Some(Action::MoveDown)) => lib.pick = (lib.pick + 1).min(last),
            (KeyCode::PageUp, _) => lib.pick = lib.pick.saturating_sub(8),
            (KeyCode::PageDown, _) => lib.pick = (lib.pick + 8).min(last),
            (KeyCode::Home, _) => lib.pick = 0,
            (KeyCode::End, _) => lib.pick = last,
            (_, Some(Action::Place)) => self.library_take(),
            (_, Some(Action::Setup)) => self.library_start_save(),
            (_, Some(Action::Remove)) => self.library_delete(was_armed),
            (_, Some(Action::Library | Action::Back)) => self.library = None,
            (_, Some(Action::Pause)) => self.paused = !self.paused,
            (_, Some(Action::Faster)) => self.speed = (self.speed * 2).min(8),
            (_, Some(Action::Slower)) => self.speed = (self.speed / 2).max(1),
            _ => {}
        }
    }

    /// Enter on a row: a readable blueprint goes into the hand and the library closes; one that
    /// can't be used says why and stays.
    fn library_take(&mut self) {
        let Some(entry) = self.library.as_ref().and_then(|l| l.entries.get(l.pick)).cloned() else {
            self.say("No blueprints saved yet");
            return;
        };
        match entry.content {
            crate::library::Content::Ok(mut bp) => {
                bp.name = entry.name.clone();
                self.library = None;
                let mut msg = format!(
                    "Took {} ({}x{}, {} part{}, {}) · {} pastes, {} turns",
                    bp.name,
                    bp.w,
                    bp.h,
                    bp.part_count(),
                    plural(bp.part_count()),
                    money_text(bp.cost()),
                    self.controls.key(Action::Place),
                    self.controls.key(Action::Rotate)
                );
                if let Some(k) = bp.first_locked(&self.world.progress) {
                    msg += &format!(" · {} is still locked", k.name());
                }
                self.take_into_hand(bp);
                self.say(msg);
            }
            crate::library::Content::Newer(f) => {
                self.say(format!("{} is from a newer game (format {f}): update the game to use it", entry.name));
            }
            crate::library::Content::Broken(why) => {
                self.say(format!(
                    "Can't read {}: {why}",
                    entry.path.file_name().map_or(entry.name.clone(), |n| n.to_string_lossy().into_owned())
                ));
            }
        }
    }

    /// `f`: starts typing a name for the blueprint in the hand (or the last copy).
    fn library_start_save(&mut self) {
        let Some((bp, _)) = self.savable() else {
            let key = self.controls.key(Action::Area);
            self.say(format!("Nothing to save: copy an area first ({key}, then {key} again)"));
            return;
        };
        let n = self.library.as_ref().map_or(0, |l| l.entries.len());
        let name = if bp.name.is_empty() || bp.name == "Blueprint" { format!("Blueprint {}", n + 1) } else { bp.name.clone() };
        if let Some(l) = self.library.as_mut() {
            l.typing = Some(name);
        }
    }

    /// Enter after typing a name: writes the file, then shows it picked.
    fn library_save(&mut self) {
        let Some(name) = self.library.as_mut().and_then(|l| l.typing.take()) else { return };
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        let (Some(data), Some((bp, in_hand))) = (self.data_dir.clone(), self.savable().map(|(b, h)| (b.clone(), h))) else { return };
        let mut bp = bp;
        bp.name = name.clone();
        match crate::library::save(&data, &bp) {
            Ok(path) => {
                if in_hand {
                    if let Some(h) = self.held.as_mut() {
                        h.name = name.clone();
                    }
                }
                self.library_refresh(Some(path.clone()));
                let file = path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
                self.say(format!("Saved {name} to the library ({file})"));
            }
            Err(e) => self.say(format!("Couldn't save the blueprint ({e})")),
        }
    }

    /// Delete on a row: the first press asks, the second removes the file. Files that can't be read
    /// are left alone (they may be from a newer game, or mid-edit).
    fn library_delete(&mut self, armed: bool) {
        let Some(entry) = self.library.as_ref().and_then(|l| l.entries.get(l.pick)).cloned() else { return };
        if !matches!(entry.content, crate::library::Content::Ok(_)) {
            let file = entry.path.display();
            self.say(format!("This file can't be read, so it is left alone: delete {file} yourself if you don't want it"));
            return;
        }
        if !armed {
            if let Some(l) = self.library.as_mut() {
                l.armed = true;
            }
            self.say(format!("{} again deletes {} for good", self.controls.key(Action::Remove), entry.name));
            return;
        }
        match crate::library::delete(&entry.path) {
            Ok(()) => {
                self.library_refresh(None);
                self.say(format!("Deleted {}", entry.name));
            }
            Err(e) => self.say(format!("Couldn't delete {} ({e})", entry.name)),
        }
    }

    /// The mouse in the library: a click picks a row and a second click takes it; the wheel scrolls.
    fn library_mouse(&mut self, m: MouseEvent) {
        let at = ratatui::layout::Position::new(m.column, m.row);
        let Some(lib) = self.library.as_mut() else { return };
        let last = lib.entries.len().saturating_sub(1);
        match m.kind {
            MouseEventKind::ScrollUp => {
                lib.pick = lib.pick.saturating_sub(1);
                lib.armed = false;
            }
            MouseEventKind::ScrollDown => {
                lib.pick = (lib.pick + 1).min(last);
                lib.armed = false;
            }
            MouseEventKind::Down(MouseButton::Left) if lib.typing.is_none() => {
                if let Some(&(_, i)) = lib.rows.iter().find(|(r, _)| r.contains(at)) {
                    let again = lib.pick == i;
                    lib.pick = i;
                    lib.armed = false;
                    if again {
                        self.library_take();
                    }
                }
            }
            _ => {}
        }
    }

    /// Says what unlocks blueprints.
    fn say_blueprints_locked(&mut self) {
        let t = Tech::Blueprints;
        let (item, n) = t.milestone();
        self.say(format!(
            "Blueprints are locked: research them first ({}) · {:.0} RP, or make {} {}",
            self.controls.key(Action::Tree),
            t.cost(),
            thousands(n),
            item.name().to_lowercase()
        ));
    }

    /// Copies the chosen rectangle into the hand. Needs the Blueprints tech: without it the
    /// rectangle stays chosen (x still removes) and the message says what unlocks copying.
    fn area_copy(&mut self, rect: TileRect) {
        let Some(floor) = self.area.map(|a| a.floor) else { return };
        if !self.world.progress.has(Tech::Blueprints) {
            self.say_blueprints_locked();
            return;
        }
        if self.world.area_info(floor, rect).parts == 0 {
            self.say(format!("Nothing in the area to copy · {} cancels", self.controls.key(Action::Back)));
            return;
        }
        self.cancel_area();
        self.copy_rect(floor, rect);
    }

    /// Makes a blueprint of the rectangle and puts it in the hand (the tech is already checked).
    fn copy_rect(&mut self, floor: usize, rect: TileRect) {
        let cap = self.world.capture(floor, rect);
        if cap.bp.parts.is_empty() {
            self.say("Nothing in that area to copy");
            return;
        }
        let bp = cap.bp;
        let mut msg = format!(
            "Copied {} part{} ({}x{}, {}) · {} pastes, {} turns, {} saves it",
            bp.part_count(),
            plural(bp.part_count()),
            bp.w,
            bp.h,
            money_text(bp.cost()),
            self.controls.key(Action::Place),
            self.controls.key(Action::Rotate),
            self.controls.key(Action::Library),
        );
        if cap.orphan_tunnels > 0 {
            let n = cap.orphan_tunnels;
            msg += &format!(" · {n} tunnel end{} no partner here", if n == 1 { " has" } else { "s have" });
        }
        self.last_copy = Some(bp.clone());
        self.take_into_hand(bp);
        self.say(msg);
    }

    /// Puts a blueprint in the hand, which empties any part held.
    pub fn take_into_hand(&mut self, bp: Blueprint) {
        self.selected = None;
        self.menu = None;
        self.paste = None;
        self.held = Some(bp);
    }

    /// Deconstructs everything in the rectangle: full refund, one fee for any waste held (after a
    /// second press when it is over the limit), and a snapshot for `z`.
    fn area_remove(&mut self, rect: TileRect) {
        let Some(floor) = self.area.map(|a| a.floor) else { return };
        let info = self.world.area_info(floor, rect);
        if info.parts == 0 {
            self.cancel_area();
            self.say("Nothing in the area to remove");
            return;
        }
        let quoted = matches!(self.confirm_area, Some((f, r, fee)) if f == floor && r == rect && info.fee <= fee + 1e-9);
        if info.fee > WASTE_CONFIRM && !quoted {
            self.confirm_area = Some((floor, rect, info.fee));
            let key = self.controls.key(Action::Remove);
            self.say(format!("The area holds {} waste: removing it costs {} in all. {key} again to pay", info.waste, money_text(info.fee)));
            return;
        }
        self.cancel_area();
        let (rep, undo) = self.world.remove_area(floor, rect);
        let mut msg = format!("Removed {} part{}, {} back", rep.parts, plural(rep.parts), money_text(rep.refund));
        if rep.fee > 0.0 {
            msg += &format!(" · waste fee {}", money_text(rep.fee));
        }
        if rep.kept > 0 {
            msg += &format!(" · kept {} holding waste: need {} to remove", rep.kept, money_text(rep.kept_fee));
        }
        if let Some(undo) = undo {
            msg += &format!(" · {} puts them back", self.controls.key(Action::Restore));
            self.last_removed = Some(undo);
            self.upgrade_is_latest = false;
        }
        self.say(msg);
    }

    /// Says what unlocks the upgrade planner.
    fn say_upgrades_locked(&mut self) {
        let t = Tech::UpgradePlanner;
        let (item, n) = t.milestone();
        let name = item.name().to_lowercase();
        let name = format!("{name}{}", if name.ends_with('x') { "es" } else { "s" });
        self.say(format!(
            "Upgrades are locked: research the {} ({}, from {}) · {} RP, or make {} {name}",
            t.name(),
            self.controls.key(Action::Tree),
            t.company().name(),
            thousands(t.cost() as u64),
            thousands(n)
        ));
    }

    /// The area tool's line for a verb that did nothing: shown in the sidebar's place and the
    /// small pane's crumb, as well as in the message line.
    fn say_area_note(&mut self, long: String, short: &str) {
        self.say(long.clone());
        self.area_note = Some((long, short.to_string()));
    }

    /// What is left out of an upgrade, as the tail of a message: money, locked parts, recipes.
    fn upgrade_tail(&self, short: usize, short_by: f64, locked: usize, locked_kind: Option<Kind>, no_recipe: usize) -> String {
        let mut tail = String::new();
        if short > 0 {
            tail += &format!(" · {short} short of money: {} more", money_text(short_by));
        }
        if locked > 0 {
            let what = locked_kind.map_or(String::new(), |k| format!(" ({} not researched)", k.name()));
            tail += &format!(" · {locked} locked{what}");
        }
        if no_recipe > 0 {
            tail += &format!(" · {no_recipe} kept: the new version can't make its recipe");
        }
        tail
    }

    /// `u` with a rectangle chosen. The first press shows what would happen ("u again to upgrade 12
    /// parts for $360"); the second does it: each part with a next version that is unlocked is
    /// swapped in place for the price difference, in reading order, until the money runs out.
    fn area_upgrade(&mut self, rect: TileRect) {
        let Some(floor) = self.area.map(|a| a.floor) else { return };
        if !self.world.progress.has(Tech::UpgradePlanner) {
            self.say_upgrades_locked();
            let short = format!("upgrade locked: {}", self.controls.key(Action::Tree));
            self.area_note = Some((self.msg.clone(), short));
            return;
        }
        let plan = self.world.plan_upgrade(floor, rect);
        let key = self.controls.key(Action::Upgrade);
        if plan.parts.is_empty() {
            self.confirm_upgrade = None;
            self.say_area_note(
                "Nothing in the area has a next version: belts, cheap drills and panels, big drills, assemblers do".into(),
                "nothing to upgrade",
            );
            return;
        }
        let (fits, short, locked, no_recipe) =
            (plan.count(UpState::Fits), plan.count(UpState::Short), plan.count(UpState::Locked), plan.count(UpState::NoRecipe));
        let cost = plan.cost(UpState::Fits);
        let tail = self.upgrade_tail(short, plan.short_by(self.world.econ.money), locked, plan.locked_kind(), no_recipe);
        if fits == 0 {
            self.confirm_upgrade = None;
            let long = format!("Nothing to upgrade right now{tail}");
            self.say_area_note(long, &format!("0 to upgrade{}", if locked > 0 { " · locked" } else { "" }));
            return;
        }
        let quote = UpgradeQuote { floor, rect, parts: fits, cost };
        let confirmed =
            self.confirm_upgrade.is_some_and(|q| q.floor == floor && q.rect == rect && q.parts == fits && (q.cost - cost).abs() < 0.005);
        if !confirmed {
            self.confirm_upgrade = Some(quote);
            let long = format!("{key} again to upgrade {fits} part{} for {}{tail}", plural(fits), money_text(cost));
            let short_form = format!("{key} again: {fits} for {}", money_text(cost));
            self.say_area_note(long, &short_form);
            return;
        }
        let (rep, undo) = self.world.upgrade_area(floor, rect);
        self.confirm_upgrade = None;
        self.area_note = None;
        let mut msg = self.upgrade_summary(&rep);
        if let Some(undo) = undo {
            msg += &format!(" · {} swaps them back", self.controls.key(Action::Restore));
            self.last_upgrade = Some(undo);
            self.upgrade_is_latest = true;
        }
        if rep.short > 0 {
            // The rectangle stays marked, so pressing again (after earning more) finishes the job.
            msg += &format!(" · {key} twice for the rest, once you can afford it");
        } else {
            self.cancel_area();
        }
        self.say(msg);
    }

    /// "Upgraded 42 fast belts, 3 basic drills for $714 · 2 short of money: $40 more".
    fn upgrade_summary(&self, rep: &UpgradeReport) -> String {
        let made: Vec<String> =
            rep.made.iter().map(|(k, n)| format!("{n} {}{}", k.name().to_lowercase(), if *n == 1 { "" } else { "s" })).collect();
        let mut msg = format!("Upgraded {} for {}", made.join(", "), money_text(rep.cost));
        msg += &self.upgrade_tail(rep.short, rep.short_by, rep.locked, rep.locked_kind, rep.no_recipe);
        msg
    }

    /// `z` after an upgrade: swaps the parts back and pays the difference back.
    fn undo_last_upgrade(&mut self) {
        let key = self.controls.key(Action::Restore);
        let Some(undo) = self.last_upgrade.take() else { return };
        self.upgrade_is_latest = false;
        if undo.floor != self.floor {
            let level = self.world.floors[undo.floor].level;
            self.last_upgrade = Some(undo);
            self.upgrade_is_latest = true;
            self.say(format!("Those were upgraded on floor {level:+}: go there and press {key}"));
            return;
        }
        let total = undo.len();
        let rep = self.world.undo_upgrade(&undo);
        let mut msg = format!("Swapped {} of {total} part{} back, {} back", rep.reverted, plural(total), money_text(rep.refund));
        if rep.kept > 0 {
            msg += &format!(" · {} left as they are now (removed, replaced or set to a new recipe)", rep.kept);
        }
        self.say(msg);
    }

    /// `z`: puts back what the last area deconstruct took, as far as it fits and the money goes;
    /// or, when an upgrade came after it, swaps the upgraded parts back.
    fn restore_removed(&mut self) {
        let key = self.controls.key(Action::Restore);
        if self.last_upgrade.is_some() && (self.upgrade_is_latest || self.last_removed.is_none()) {
            self.undo_last_upgrade();
            return;
        }
        let Some(mut undo) = self.last_removed.take() else {
            self.say("Nothing to put back: it keeps what the last area remove took, or the last upgrade");
            return;
        };
        if undo.floor != self.floor {
            let level = self.world.floors[undo.floor].level;
            self.last_removed = Some(undo);
            self.say(format!("Those came off floor {level:+}: go there and press {key}"));
            return;
        }
        let total = undo.len();
        let rep = self.world.restore_removed(&mut undo);
        let mut msg = if undo.is_empty() {
            format!("Put back {} part{}, {}", rep.placed, plural(rep.placed), money_text(rep.price))
        } else {
            format!("Put back {} of {total}, {}", rep.placed, money_text(rep.price))
        };
        if rep.blocked > 0 {
            msg += &format!(" · {} blocked", rep.blocked);
        }
        if rep.short > 0 {
            msg += &format!(" · {} short of money", rep.short);
        }
        if !undo.is_empty() {
            msg += &format!(" · {key} again for the rest");
            self.last_removed = Some(undo);
        }
        self.say(msg);
    }

    /// Turns what is in the hand a quarter (a blueprint: counter-clockwise with Shift), or the
    /// part under the cursor.
    fn rotate(&mut self, counter_clockwise: bool) {
        if let Some(bp) = self.held.as_ref() {
            self.held = Some(bp.rotated(if counter_clockwise { 3 } else { 1 }));
            return;
        }
        if self.selected.is_some_and(|k| k.directional()) {
            self.dir = self.dir.cw();
            return;
        }
        let (x, y) = self.cursor;
        if let Some(b) = self.world.tile(self.floor, x, y) {
            let nd = b.dir.cw();
            if let Err(e) = self.world.set_dir(self.floor, x, y, nd) {
                self.say(e);
            }
        } else {
            self.dir = self.dir.cw();
        }
    }

    fn floor_up(&mut self) {
        self.go_to_level(self.world.floors[self.floor].level + 1);
    }

    fn floor_down(&mut self) {
        self.go_to_level(self.world.floors[self.floor].level - 1);
    }

    /// Moves the view to another floor, if it's open yet.
    fn go_to_level(&mut self, level: i32) {
        match self.world.floor_of(level) {
            Some(f) if self.world.floor_open(f) => {
                // The rectangle belongs to one floor.
                self.cancel_area();
                self.floor = f;
                let what = match level {
                    0 => "Ground level".to_string(),
                    l if l < 0 => format!(
                        "Floor {l}: digging costs ${:.0} a tile, drills here mine {}x faster",
                        crate::sim::tile_cost(l),
                        crate::sim::richness(l)
                    ),
                    l => format!("Floor +{l}: decking costs ${:.0} a tile", crate::sim::tile_cost(l)),
                };
                self.say(what);
            }
            Some(_) => self.say(self.world.floor_hint(level)),
            None => {}
        }
    }
}

/// 1500 as "1,500".
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// What the status line says after a paste.
fn paste_message(rep: &PasteReport) -> String {
    if let Some(e) = &rep.refused {
        return e.to_string();
    }
    let left = rep.total - rep.present;
    let mut msg = if rep.placed == 0 && rep.present == rep.total {
        "Already all here: nothing to place".to_string()
    } else if rep.placed == 0 {
        "Nothing fits here".to_string()
    } else if rep.placed == left {
        format!("Pasted {} part{} for {}", rep.placed, plural(rep.placed), money_text(rep.cost))
    } else {
        format!("Placed {} of {}", rep.placed, left)
    };
    if rep.short > 0 {
        msg += &format!(", {} short", money_text(rep.short_by));
    }
    if rep.present > 0 && rep.present < rep.total {
        msg += &format!(" · {} already there", rep.present);
    }
    if rep.blocked > 0 {
        msg += &format!(" · {} blocked", rep.blocked);
    }
    if rep.locked > 0 {
        let what = rep.locked_kind.map_or(String::new(), |k| format!(" ({})", k.name()));
        msg += &format!(" · {} locked{what}", rep.locked);
    }
    if rep.wrong_floor > 0 {
        msg += &format!(" · {} not on this floor", rep.wrong_floor);
    }
    if rep.recipes_skipped > 0 {
        msg += &format!(" · {} recipe{} not unlocked yet", rep.recipes_skipped, plural(rep.recipes_skipped));
    }
    if rep.elevators > 0 {
        msg += &format!(" · {} elevator{}: other floors' doors reset to unload", rep.elevators, plural(rep.elevators));
    }
    if rep.short > 0 {
        msg += " · the rest waits in your hand";
    }
    msg
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// The recipe name a copied setting picks, for the "Copied" message.
fn copied_recipe(kind: Kind, set: Option<crate::sim::Setting>) -> Option<&'static str> {
    match set? {
        crate::sim::Setting::Recipe(r) if kind.recipes().len() > 1 => kind.recipes().get(r as usize).map(|r| r.name),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Company;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(app: &mut App, code: KeyCode) {
        app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[test]
    fn removing_costly_waste_storage_takes_a_second_press() {
        let mut w = World::blank();
        w.place(0, 5, 5, Kind::Crate, Dir::E).unwrap();
        let a = w.floors[0].anchor_at(5, 5).unwrap();
        if let Some(crate::sim::Building { st: crate::sim::St::Crate { items }, .. }) = w.floors[0].tiles[a].as_mut() {
            items.extend([crate::sim::Item::Waste; 100]);
        }
        w.econ.money = 1_000.0;
        let mut app = App::new(w);
        app.cursor = (5, 5);
        key(&mut app, KeyCode::Char('x'));
        assert!(app.world.tile(0, 5, 5).is_some(), "first press only warns");
        assert!(app.msg.contains("Remove again"), "{}", app.msg);
        key(&mut app, KeyCode::Char('x'));
        assert!(app.world.tile(0, 5, 5).is_none(), "second press removes and pays");
    }

    #[test]
    fn number_keys_open_a_category_then_pick_a_part() {
        let mut app = App::new(World::new());
        key(&mut app, KeyCode::Char('2'));
        assert_eq!(app.menu, Some(Category::Transport));
        key(&mut app, KeyCode::Char('1'));
        assert_eq!(app.selected, Some(Kind::Belt));
        key(&mut app, KeyCode::Char('2'));
        assert_eq!(app.selected, Some(Kind::Splitter));
        key(&mut app, KeyCode::Char('3'));
        assert_eq!(app.selected, Some(Kind::Arm));
        key(&mut app, KeyCode::Char('~'));
        assert_eq!((app.menu, app.selected), (None, None));
        // Create > 3 is the press, still locked in a new game.
        key(&mut app, KeyCode::Char('1'));
        key(&mut app, KeyCode::Char('3'));
        assert_eq!(app.selected, None);
        assert!(app.msg.contains("locked"));
        // Elevator sizes go 2 to 8; numbers past 4 do nothing.
        key(&mut app, KeyCode::Char('~'));
        key(&mut app, KeyCode::Char('='));
        key(&mut app, KeyCode::Char('='));
        assert_eq!(app.lift_size, 4);
        key(&mut app, KeyCode::Char('5'));
        assert_eq!(app.menu, None);
    }

    fn with_telegrams(n: usize) -> App {
        let mut app = App::new(World::new());
        for i in 0..n {
            app.world.receive(crate::mail::parse(&format!("from: t\nmsg {i}"), 0), 1_000);
        }
        app.mail = app.world.mail.pop_front();
        app
    }

    #[test]
    fn a_telegram_closes_only_on_n_or_esc() {
        let mut app = with_telegrams(2);
        app.compact = Some(true);
        app.compact_now = true;
        key(&mut app, KeyCode::Char('n')); // small pane: n opens it
        assert!(app.mail_open);
        for c in ['x', 'q', ' ', 'r', 'z'] {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert!(app.mail_open && app.mail.is_some(), "stray keys leave it open");
        assert_eq!(app.world.unread(), 2);
        key(&mut app, KeyCode::Char('n'));
        assert!(!app.mail_open && app.mail.is_none());
        assert_eq!(app.world.unread(), 1, "closing marks just that one read");
        assert!(app.world.log[0].read && !app.world.log[1].read);
        // Esc closes the next one too.
        app.mail = app.world.mail.pop_front();
        app.mail_open = true;
        key(&mut app, KeyCode::Esc);
        assert!(!app.mail_open && app.mail.is_none());
        assert_eq!(app.world.unread(), 0);
    }

    #[test]
    fn in_the_sidebar_n_dismisses_and_esc_only_when_nothing_else_is_open() {
        let mut app = with_telegrams(3);
        app.compact = Some(false);
        app.compact_now = false;
        app.menu = Some(Category::Create);
        key(&mut app, KeyCode::Esc);
        assert!(app.menu.is_none() && app.mail.is_some(), "the first Esc backs out of the menu");
        key(&mut app, KeyCode::Esc);
        assert!(app.mail.is_none(), "the second closes the telegram");
        assert_eq!(app.world.unread(), 2);
        app.mail = app.world.mail.pop_front();
        key(&mut app, KeyCode::Char('n'));
        assert!(app.mail.is_none());
        assert_eq!(app.world.unread(), 1);
    }

    #[test]
    fn shift_n_dismisses_every_waiting_telegram_but_keeps_them_in_the_log() {
        let mut app = with_telegrams(4);
        app.compact_now = false;
        key(&mut app, KeyCode::Char('N'));
        assert!(app.mail.is_none() && app.world.mail.is_empty());
        assert_eq!((app.world.log.len(), app.world.unread()), (4, 0));
    }

    #[test]
    fn the_log_reopens_a_telegram_you_closed() {
        let mut app = with_telegrams(3);
        app.compact_now = false;
        key(&mut app, KeyCode::Char('N')); // dismiss all by accident
        assert!(app.mail.is_none());
        key(&mut app, KeyCode::Char('i'));
        assert!(app.log_open);
        key(&mut app, KeyCode::Down); // 0 is the newest; this picks the one before
        key(&mut app, KeyCode::Char('x')); // stray keys do nothing
        assert!(app.log_open && app.log_pick == 1);
        key(&mut app, KeyCode::Enter);
        assert!(!app.log_open);
        let m = app.mail.as_ref().expect("reopened");
        assert_eq!(m.md, "msg 1");
        assert!(app.world.mail.is_empty());
        key(&mut app, KeyCode::Char('n'));
        assert!(app.mail.is_none());
        // Esc and i both close the log.
        key(&mut app, KeyCode::Char('i'));
        key(&mut app, KeyCode::Esc);
        assert!(!app.log_open);
        key(&mut app, KeyCode::Char('i'));
        key(&mut app, KeyCode::Char('i'));
        assert!(!app.log_open);
    }

    #[test]
    fn reopening_from_the_log_keeps_an_unread_telegram_waiting() {
        let mut app = with_telegrams(3);
        app.compact_now = false;
        // Reading #1 (unread) when #3 is picked from the log: #1 goes back to the queue.
        app.reopen_from_log(0);
        assert_eq!(app.mail.as_ref().unwrap().md, "msg 2");
        assert_eq!(app.world.mail.iter().map(|m| m.id).collect::<Vec<_>>(), [1, 2]);
    }

    #[test]
    fn the_log_does_not_pause_or_hide_the_world() {
        let mut app = with_telegrams(1);
        let before = app.world.tick;
        key(&mut app, KeyCode::Char('i'));
        app.world.tick();
        assert_eq!(app.world.tick, before + 1);
        assert!(!app.paused);
    }

    #[test]
    fn pressing_a_slot_again_switches_kind() {
        let mut app = App::new(World::new());
        key(&mut app, KeyCode::Char('2')); // Transport
        key(&mut app, KeyCode::Char('1'));
        assert_eq!(app.selected, Some(Kind::Belt));
        key(&mut app, KeyCode::Char('1')); // fast belts and tunnels are still locked
        assert_eq!(app.selected, Some(Kind::Belt));
        assert!(app.msg.contains("still locked"), "{}", app.msg);
        app.world.progress.researched[crate::sim::Tech::Tunnels.idx()] = true;
        key(&mut app, KeyCode::Char('1')); // skips the locked fast belt
        assert_eq!(app.selected, Some(Kind::Tunnel));
        key(&mut app, KeyCode::Char('1'));
        assert_eq!(app.selected, Some(Kind::Belt));
        app.world.progress.researched[crate::sim::Tech::FastBelts.idx()] = true;
        app.world.progress.opened[Company::Swiftline.idx()] = true;
        key(&mut app, KeyCode::Char('1'));
        assert_eq!(app.selected, Some(Kind::FastBelt));
        key(&mut app, KeyCode::Char('3'));
        assert_eq!(app.selected, Some(Kind::Arm));
        key(&mut app, KeyCode::Char('1')); // comes back to the fast belt, the last one picked
        assert_eq!(app.selected, Some(Kind::FastBelt));
        key(&mut app, KeyCode::Char('1'));
        assert_eq!(app.selected, Some(Kind::Tunnel));
        key(&mut app, KeyCode::Char('1'));
        assert_eq!(app.selected, Some(Kind::Belt));
    }

    #[test]
    fn every_part_has_one_slot_in_its_own_category() {
        for k in Kind::ALL {
            let homes: Vec<Category> = Category::ALL.iter().copied().filter(|c| parts_in(*c).contains(&k)).collect();
            assert_eq!(homes, vec![k.category()], "{k:?}");
        }
        for c in Category::ALL {
            let most = max_slots(c);
            assert!(slots(c).len() <= most, "{c:?} has more than {most} slots");
            assert!(slots(c).iter().all(|s| !s.is_empty()), "{c:?} has an empty slot");
        }
    }

    #[test]
    fn the_large_kiln_is_a_second_version_in_the_kilns_slot() {
        assert_eq!(
            slots(Category::Create)[1],
            &[Kind::Furnace, Kind::Molder, Kind::BlastFurnace, Kind::CokeOven, Kind::Kiln, Kind::LargeKiln, Kind::ArcFurnace]
        );
        let mut app = App::new(World::new());
        app.world.progress.opened[Company::Underhill.idx()] = true;
        app.world.progress.researched[Tech::Kiln.idx()] = true;
        app.menu = Some(Category::Create);
        app.selected = Some(Kind::Kiln);
        // Still locked: the key skips it and goes round to the first kind.
        key(&mut app, KeyCode::Char('2'));
        assert_eq!(app.selected, Some(Kind::Furnace));
        // Researched: from the kiln the same key moves on to it, and back round to the first kind.
        app.selected = Some(Kind::Kiln);
        app.world.progress.researched[Tech::LargeKiln.idx()] = true;
        key(&mut app, KeyCode::Char('2'));
        assert_eq!(app.selected, Some(Kind::LargeKiln));
        key(&mut app, KeyCode::Char('2'));
        assert_eq!(app.selected, Some(Kind::Furnace));
    }

    #[test]
    fn basic_solar_is_a_second_version_in_the_solar_slot() {
        assert_eq!(slots(Category::Power)[0], &[Kind::Solar, Kind::BasicSolar]);
        let mut app = App::new(World::new());
        app.world.progress.opened[Company::Kettering.idx()] = true;
        // Locked: the key picks the cheap panel and again says the other is locked.
        key(&mut app, KeyCode::Char('3'));
        key(&mut app, KeyCode::Char('1'));
        assert_eq!(app.selected, Some(Kind::Solar));
        key(&mut app, KeyCode::Char('1'));
        assert_eq!(app.selected, Some(Kind::Solar));
        assert!(app.msg.contains("Basic solar panel is still locked"), "{}", app.msg);
        // Researched: pressing the slot again switches versions, and back.
        app.world.progress.researched[Tech::BasicSolar.idx()] = true;
        key(&mut app, KeyCode::Char('1'));
        assert_eq!(app.selected, Some(Kind::BasicSolar));
        key(&mut app, KeyCode::Char('1'));
        assert_eq!(app.selected, Some(Kind::Solar));
    }

    #[test]
    fn max_slots_is_a_table_for_every_category() {
        let got: Vec<usize> = Category::ALL.iter().map(|c| max_slots(*c)).collect();
        assert_eq!(got, vec![6, 5, 4, 3], "Create, Transport, EC, Misc");
        // Every category is in the table once.
        for c in Category::ALL {
            assert_eq!(MAX_SLOTS.iter().filter(|(k, _)| *k == c).count(), 1, "{c:?}");
        }
    }

    #[test]
    fn pipes_have_transport_slot_five_and_pumps_keep_create_five() {
        assert_eq!(slots(Category::Transport)[4], &[Kind::Pipe]);
        assert_eq!(slots(Category::Create)[4], &[Kind::Pump, Kind::AdvancedPump, Kind::Mixer, Kind::Evacuator]);
        assert_eq!(slots(Category::Create)[5], &[Kind::Etcher, Kind::CrystalGrower, Kind::WaferSaw, Kind::FineEtcher]);
        assert_eq!(Kind::Pipe.category(), Category::Transport);
        assert!(!parts_in(Category::Create).contains(&Kind::Pipe));
        assert_eq!(slots(Category::Misc), &[&[Kind::Crate, Kind::Warehouse][..], &[Kind::TradePost][..]]);
        let mut app = App::new(World::new());
        app.world.progress.researched[Tech::Pumps.idx()] = true;
        app.world.progress.opened[Company::Wellbourne.idx()] = true;
        key(&mut app, KeyCode::Char('2'));
        key(&mut app, KeyCode::Char('5'));
        assert_eq!((app.menu, app.selected), (Some(Category::Transport), Some(Kind::Pipe)));
        key(&mut app, KeyCode::Char('`'));
        key(&mut app, KeyCode::Char('1'));
        key(&mut app, KeyCode::Char('5'));
        assert_eq!((app.menu, app.selected), (Some(Category::Create), Some(Kind::Pump)));
        // Slot 6 and Misc slot 3 are not there yet: the key does nothing.
        let before = app.selected;
        key(&mut app, KeyCode::Char('6'));
        assert_eq!(app.selected, before);
    }

    #[test]
    fn number_keys_one_to_five_pick_each_slot_in_every_category() {
        for cat in Category::ALL {
            for (n, slot) in slots(cat).iter().enumerate().take(5) {
                let mut app = App::new(World::new());
                // Everything is bought, so the first kind of the slot is what the key picks.
                app.world.progress.opened = [true; crate::sim::COMPANIES];
                app.world.progress.researched = [true; crate::sim::TECHS];
                app.menu = Some(cat);
                key(&mut app, KeyCode::Char(char::from_digit(n as u32 + 1, 10).unwrap()));
                assert_eq!(app.selected, Some(slot[0]), "{cat:?} slot {}", n + 1);
            }
        }
    }

    #[test]
    fn book_parts_come_from_what_each_part_can_show() {
        // Every crafter, drill and pump has a page, and so does every part with rules.
        for k in Kind::ALL {
            let wants = !k.recipes().is_empty() || k.is_drill() || k.is_pump() || k.rules().is_some();
            assert_eq!(BOOK_PARTS.contains(&k), wants, "{k:?}");
        }
        for k in [Kind::Kiln, Kind::Welder, Kind::AdvancedPump, Kind::LargeAdvancedDrill, Kind::Pipe] {
            assert!(BOOK_PARTS.contains(&k), "{k:?}");
        }
        for k in [Kind::Belt, Kind::Splitter, Kind::Crate, Kind::Pole] {
            assert!(!BOOK_PARTS.contains(&k), "{k:?}");
        }
        let mut seen = BOOK_PARTS.clone();
        seen.dedup();
        assert_eq!(seen.len(), BOOK_PARTS.len());
    }

    #[test]
    fn rules_are_short_lines() {
        for k in Kind::ALL {
            let Some(lines) = k.rules() else { continue };
            assert!((1..=4).contains(&lines.len()), "{k:?}: {} lines", lines.len());
            for l in lines {
                assert!(l.chars().count() <= 38, "{k:?}: {l}");
            }
        }
    }

    #[test]
    fn research_groups_are_whole() {
        let names: Vec<&str> = UNLOCK_GROUPS.iter().map(|g| g.0).collect();
        assert_eq!(names, ["Making", "Assembling", "Drilling", "Moving", "Pipes", "EC", "Floors", "Tools"]);
        assert!(UNLOCK_GROUPS.iter().all(|g| !g.1.is_empty()), "no empty group");
        let pipes = UNLOCK_GROUPS.iter().find(|g| g.0 == "Pipes").unwrap().1;
        assert_eq!(pipes, &[Tech::Pumps, Tech::AdvancedPump]);
        assert_eq!(tree_rows().len(), Tech::ALL.len());
    }

    #[test]
    fn rupee_key_backs_out_of_a_menu() {
        let mut app = App::new(World::new());
        key(&mut app, KeyCode::Char('2'));
        key(&mut app, KeyCode::Char('1'));
        assert_eq!(app.selected, Some(Kind::Belt));
        key(&mut app, KeyCode::Char('₹'));
        assert_eq!((app.menu, app.selected), (None, None));
    }

    #[test]
    fn research_screen_keys() {
        let mut app = App::new(World::new());
        key(&mut app, KeyCode::Char('t'));
        assert!(app.tree);
        assert_eq!(app.tree_choice(), Tech::Press);
        key(&mut app, KeyCode::Down);
        assert_eq!(app.tree_choice(), Tech::Coking);
        // Enter researches when there are points to spend.
        app.world.econ.research = 150.0;
        key(&mut app, KeyCode::Enter);
        assert!(app.world.progress.has(Tech::Coking));
        assert_eq!(app.world.econ.research, 50.0);
        key(&mut app, KeyCode::Char('t'));
        assert!(!app.tree);
    }
}
