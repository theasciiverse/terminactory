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
pub const MAX_SLOTS: [(Category, usize); 4] = [(Category::Create, 7), (Category::Transport, 5), (Category::Power, 4), (Category::Misc, 3)];

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
            &[
                CrudeAssembler,
                BasicAssembler,
                AdvancedAssembler,
                PrecisionAssembler,
                LargeBasicAssembler,
                LargeAdvancedAssembler,
                LargePrecisionAssembler,
                Welder,
            ],
            &[Pump, AdvancedPump, Mixer, Evacuator, OilExtractor],
            &[Etcher, CrystalGrower, WaferSaw, FineEtcher],
            // Chemistry: the still, with the refinery and the bottler to come.
            &[Still],
        ],
        // Pipes carry things, so they sit with the belts, in slot 5.
        Category::Transport => &[&[Belt, FastBelt, Tunnel], &[Splitter, Sorter], &[Arm, LongArm], &[Elevator], &[Pipe]],
        Category::Power => &[&[Solar, BasicSolar], &[Pole], &[Boiler], &[SteamEngine]],
        // Slot 3 is kept for rail.
        Category::Misc => &[&[Crate, Warehouse], &[TradePost, LiquidTrader]],
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

/// A button on the browser's toolbar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BookButton {
    Back,
    Forward,
    Reload,
    Home,
    NewTab,
    Star,
    /// The start page's Search and Lucky buttons, and the box on the results page.
    Search,
    Lucky,
    Edit,
    /// The window's three dots: close (forget the tabs), minimize (keep them), fullscreen.
    CloseWindow,
    Minimize,
    Fullscreen,
    /// The status bar's Copy button: the page's text goes to the clipboard.
    Copy,
    /// The start page's History button and the history page's Clear button.
    History,
    ClearHistory,
    /// The address field: click it to type an address or a search.
    Address,
}

/// One browser tab: a page, its own history and where it was scrolled and picked.
#[derive(Clone, Debug, PartialEq)]
pub struct Tab {
    pub slug: String,
    pub history: Vec<String>,
    pub forward: Vec<String>,
    pub selected: usize,
    pub scroll: u16,
    /// The posts folded away on this tab's thread (post indexes).
    pub folded: Vec<usize>,
}

/// The sign-in form on a gated page: the two boxes, which one has the keys, and whether the
/// last try failed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoginForm {
    pub user: String,
    pub pass: String,
    pub on_pass: bool,
    pub failed: bool,
}

impl Tab {
    pub fn home() -> Self {
        Tab { slug: "start".into(), history: Vec::new(), forward: Vec::new(), selected: 0, scroll: 0, folded: Vec::new() }
    }

    /// The page's title, for the tab strip.
    pub fn title(&self) -> String {
        if crate::web::is_addr(&self.slug) {
            return crate::web::title(&self.slug);
        }
        match self.slug.as_str() {
            "start" => "termweb".into(),
            "home" => "Home".into(),
            "history" => "History".into(),
            "production" => "Production".into(),
            "research" => "Research".into(),
            s if s.starts_with("search?q=") => s["search?q=".len()..].to_string(),
            "items" => "Items".into(),
            "machines" => "Machines".into(),
            "topics" => "Topics".into(),
            s => crate::wiki::page(s).and_then(|md| md.lines().next()).unwrap_or(s).trim_start_matches('#').trim().to_string(),
        }
    }
}

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
    /// The research site's selected TREE node id.
    pub tree_pick: usize,
    /// TREE node ids opened in the research tree for this session.
    pub tree_expanded: std::collections::HashSet<usize>,
    /// Where its rows were drawn, for the mouse.
    pub tree_rows: Vec<(Rect, usize)>,
    /// Recipe book: open, the page shown, how far it's scrolled, and how many columns the grid had.
    pub book: bool,
    pub book_entry: Entry,
    pub book_scroll: u16,
    /// Search text (`/`), and whether keys are typing into it.
    pub book_query: String,
    pub book_typing: bool,
    /// Links on the page as last drawn (where, and where they go), grid cells and tabs too,
    /// and the pages Backspace goes back to.
    pub book_links: Vec<(Rect, Entry)>,
    pub book_urls: Vec<(Rect, String)>,
    /// The header row of every post drawn on a thread, and the `[+]` rows of folded posts.
    pub book_post_hits: Vec<(Rect, usize)>,
    pub book_fold_hits: Vec<(Rect, usize)>,
    /// The posts folded away on the page being read, kept with its tab.
    pub book_folded: Vec<usize>,
    /// Who is signed in where (domain, username), for as long as the game runs.
    pub book_logins: Vec<(String, String)>,
    /// The sign-in form on the page being read, and whether its boxes have the keys.
    pub book_login: Option<LoginForm>,
    pub book_login_typing: bool,
    /// Links that can be clicked but are not stepped to with the arrow keys (the contents list, the category box).
    pub book_click_only: Vec<(Rect, String)>,
    /// The results page's pick: which result is selected, counted over all of them, and whether the
    /// next draw scrolls to it.
    pub book_pick: usize,
    pub book_pick_follow: bool,
    pub book_first_hit: usize,
    pub book_sel_seen: usize,
    /// The browser window's last drawn bounds, used to dismiss it on an outside click.
    pub book_area: Rect,
    /// The browser's toolbar buttons, with where they were drawn, for clicks.
    pub book_buttons: Vec<(Rect, BookButton)>,
    /// Fullscreen, the places on the title bar and toolbar a double click can land, and the last click.
    pub book_full: bool,
    /// The address field while it is being edited, and whether its text is still all selected.
    pub book_url_edit: Option<String>,
    pub book_url_fresh: bool,
    pub book_nav: Vec<Rect>,
    pub book_last_click: Option<(Instant, u16, u16)>,
    /// The browser's tabs, and which one is showing. The `book_*` page fields below are the
    /// active tab's live copy; they are saved into its `Tab` whenever another tab takes over.
    pub book_tabs: Vec<Tab>,
    pub book_active: usize,
    /// The tab strip's chips and their close crosses, for clicks.
    pub book_tab_hits: Vec<(Rect, usize)>,
    pub book_tab_close: Vec<(Rect, usize)>,
    /// The bookmarks bar's links, where they were drawn, and whether the page has a scrollbar column.
    pub book_mark_hits: Vec<(Rect, String)>,
    pub book_scrollbar: bool,
    /// When the Copy button was last pressed, for its "Copied" note.
    pub book_copied: Option<Instant>,
    /// The last search drawn: its words and its results.
    pub book_hits: Option<(String, Vec<crate::search::Hit>)>,
    /// When the page last opened: the load bar fills for a moment after it.
    pub book_opened: Option<std::time::Instant>,
    pub book_slug: String,
    pub book_history: Vec<String>,
    pub book_forward: Vec<String>,
    pub book_selected: usize,
    /// How far the current page can scroll, set while drawing it (u16::MAX when unknown).
    pub book_max_scroll: u16,
    /// How far one scroll step moves the current page (a whole row of tiles on the index grids).
    pub book_scroll_step: u16,
    pub book_seed: usize,
    pub book_results: Vec<String>,
    pub book_result_capacity: usize,
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
    /// The production site's scroll.
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
            tree_pick: 1,
            tree_expanded: std::collections::HashSet::new(),
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
            book_query: String::new(),
            book_typing: false,
            book_links: Vec::new(),
            book_urls: Vec::new(),
            book_post_hits: Vec::new(),
            book_fold_hits: Vec::new(),
            book_folded: Vec::new(),
            book_logins: Vec::new(),
            book_login: None,
            book_login_typing: false,
            book_click_only: Vec::new(),
            book_pick: 0,
            book_pick_follow: false,
            book_first_hit: 0,
            book_sel_seen: 0,
            book_area: Rect::default(),
            book_buttons: Vec::new(),
            book_full: false,
            book_url_edit: None,
            book_url_fresh: false,
            book_nav: Vec::new(),
            book_last_click: None,
            book_tabs: vec![Tab::home()],
            book_active: 0,
            book_tab_hits: Vec::new(),
            book_tab_close: Vec::new(),
            book_mark_hits: Vec::new(),
            book_scrollbar: false,
            book_copied: None,
            book_hits: None,
            book_opened: None,
            book_slug: "home".into(),
            book_history: Vec::new(),
            book_forward: Vec::new(),
            book_selected: 0,
            book_seed: 0,
            book_max_scroll: u16::MAX,
            book_scroll_step: 1,
            book_results: Vec::new(),
            book_result_capacity: 1,
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
        // A telegram can be read from over the book too (it covers the sidebar).
        if (self.book && !self.book_typing) && action == Some(Action::Mail) && self.mail.is_some() {
            self.mail_open = true;
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
            Action::Tree => self.open_site("research"),
            Action::Stats => self.open_site("production"),
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
        self.book_seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos() as usize);
        let (x, y) = self.cursor;
        let here = self.world.tile(self.floor, x, y).and_then(|b| match &b.st {
            crate::sim::St::Drill { mines: Some(m), .. } => Some(Entry::Item(*m)),
            _ => b
                .recipe()
                .and_then(|r| r.outputs.first())
                .map(|o| Entry::Item(o.0))
                .or(BOOK_PARTS.contains(&b.kind).then_some(Entry::Part(b.kind))),
        });
        // The page to show: what is under the cursor, or the start page. A tab that is only a
        // blank start page is used as it is; any other tab stays as it was and a new one opens.
        self.book_store();
        let blank_start = self.book_slug == "start" && self.book_history.is_empty();
        self.book_scroll = 0;
        match here {
            Some(e) => {
                self.book_query.clear();
                if blank_start {
                    self.book_show(e);
                    self.book_history.clear();
                    self.book_forward.clear();
                    self.book_folded.clear();
                } else {
                    self.book_new_tab(&crate::wiki::slug(e.name()));
                    self.book_entry = e;
                }
            }
            None if blank_start => {}
            None => self.book_new_tab("start"),
        }
        self.book_opened = Some(std::time::Instant::now());
        self.book = true;
        self.book_land();
    }

    /// Shows a page (and so its tab), remembering it as that tab's last page.
    pub fn book_show(&mut self, e: Entry) {
        self.book_entry = e;
        self.book_scroll = 0;
        self.book_slug = crate::wiki::slug(e.name());
        self.book_selected = 0;
        self.book_opened = Some(std::time::Instant::now());
    }

    /// Saves the live page state into its tab.
    fn book_store(&mut self) {
        if let Some(t) = self.book_tabs.get_mut(self.book_active) {
            t.slug = self.book_slug.clone();
            t.history = self.book_history.clone();
            t.forward = self.book_forward.clone();
            t.selected = self.book_selected;
            t.scroll = self.book_scroll;
            t.folded = self.book_folded.clone();
        }
    }

    /// Makes a tab the live one, keeping the page state below in step with it.
    fn book_load_tab(&mut self, i: usize) {
        let Some(t) = self.book_tabs.get(i).cloned() else { return };
        self.book_active = i;
        self.book_slug = t.slug;
        self.book_history = t.history;
        self.book_forward = t.forward;
        self.book_selected = t.selected;
        self.book_scroll = t.scroll;
        self.book_folded = t.folded;
        if let Some(e) = Self::book_entry_for_slug(&self.book_slug) {
            self.book_entry = e;
        }
        // Switching tabs shows a page already loaded: no progress bar.
        self.book_opened = None;
        self.book_land();
    }

    /// Opens a page in a new tab and switches to it.
    pub fn book_new_tab(&mut self, slug: &str) {
        self.book_store();
        let mut t = Tab::home();
        t.slug = slug.to_string();
        self.book_tabs.push(t);
        let i = self.book_tabs.len() - 1;
        self.book_load_tab(i);
        self.book_opened = Some(std::time::Instant::now());
    }

    /// Switches to a tab by its place in the strip.
    pub fn book_switch(&mut self, i: usize) {
        if i < self.book_tabs.len() && i != self.book_active {
            self.book_store();
            self.book_load_tab(i);
        }
    }

    /// Tab and Shift+Tab: round the strip, either way.
    fn book_cycle_tab(&mut self, forward: bool) {
        let n = self.book_tabs.len();
        if n < 2 {
            return;
        }
        let i = if forward { (self.book_active + 1) % n } else { (self.book_active + n - 1) % n };
        self.book_switch(i);
    }

    /// w or a chip's ✕: shut this tab. Shutting the last one closes the browser.
    pub fn book_close_tab(&mut self) {
        self.book_store();
        self.book_tabs.remove(self.book_active);
        if self.book_tabs.is_empty() {
            self.book_tabs.push(Tab::home());
            self.book_active = 0;
            self.book_load_tab(0);
            self.book = false;
            return;
        }
        let i = self.book_active.min(self.book_tabs.len() - 1);
        self.book_active = i;
        self.book_load_tab(i);
    }

    /// f: fold or unfold the post whose header row holds the selected link. F: fold every
    /// top-level post, or unfold everything when anything is folded.
    fn book_fold(&mut self, all: bool) {
        let Some(thread) = crate::web::thread(&self.book_slug) else { return };
        if all {
            self.book_folded = if self.book_folded.is_empty() {
                thread.posts.iter().enumerate().filter(|(_, p)| p.depth == 0).map(|(i, _)| i).collect()
            } else {
                Vec::new()
            };
        } else {
            let row = self.book_urls.get(self.book_selected).map(|(rect, _)| rect.y);
            let Some((_, post)) = self.book_post_hits.iter().find(|(rect, _)| Some(rect.y) == row).copied() else { return };
            match self.book_folded.iter().position(|i| *i == post) {
                Some(at) => {
                    self.book_folded.remove(at);
                }
                None => self.book_folded.push(post),
            }
            self.book_folded.sort_unstable();
        }
        // Folding takes rows away: keep the pick and the scroll on something that is still there.
        self.book_selected = self.book_selected.min(self.book_urls.len().saturating_sub(1));
        self.book_scroll = self.book_scroll.min(self.book_max_scroll);
    }

    /// Redraws the page where it is, as the reload button does.
    pub fn book_reload(&mut self) {
        self.book_scroll = 0;
        self.book_folded.clear();
        self.book_opened = Some(std::time::Instant::now());
    }

    /// Backspace's job: the last page this tab visited.
    fn book_back(&mut self) {
        if let Some(slug) = self.book_history.pop() {
            self.book_forward.push(self.book_slug.clone());
            self.book_slug = slug;
            if let Some(e) = Self::book_entry_for_slug(&self.book_slug) {
                self.book_entry = e;
            }
            self.book_scroll = 0;
            self.book_folded.clear();
            self.book_opened = Some(std::time::Instant::now());
            self.book_land();
        }
    }

    /// ]'s job: the page Backspace last left.
    fn book_forward(&mut self) {
        if let Some(slug) = self.book_forward.pop() {
            self.book_history.push(self.book_slug.clone());
            self.book_slug = slug;
            if let Some(e) = Self::book_entry_for_slug(&self.book_slug) {
                self.book_entry = e;
            }
            self.book_scroll = 0;
            self.book_folded.clear();
            self.book_opened = Some(std::time::Instant::now());
            self.book_land();
        }
    }

    /// Goes to a page from a link, remembering this one for Backspace.
    #[allow(dead_code)]
    pub fn book_go(&mut self, e: Entry) {
        let slug = crate::wiki::slug(e.name());
        self.book_go_slug(&slug);
    }

    /// The link on the nearest line above (or below) the selected one on screen, picking
    /// the one closest across; None when there is no such line.
    fn book_link_line(&self, down: bool) -> Option<usize> {
        let &(cur, _) = self.book_urls.get(self.book_selected)?;
        let centre = |r: &Rect| r.x as i32 + r.width as i32 / 2;
        let row = self
            .book_urls
            .iter()
            .map(|(r, _)| r.y)
            .filter(|&y| if down { y > cur.y } else { y < cur.y })
            .min_by_key(|&y| y.abs_diff(cur.y))?;
        self.book_urls
            .iter()
            .enumerate()
            .filter(|(_, (r, _))| r.y == row)
            .min_by_key(|(_, (r, _))| (centre(r) - centre(&cur)).abs())
            .map(|(i, _)| i)
    }

    pub(crate) fn book_go_slug(&mut self, slug: &str) {
        // Signing out shows the domain's index: the login is gone before the page is opened.
        if let Some(domain) = slug.trim_end_matches('/').strip_suffix("/logout").map(str::to_string) {
            self.book_logins.retain(|(d, _)| *d != domain);
            if crate::web::is_addr(&domain) {
                self.book_go_slug(&format!("{domain}/index"));
                return;
            }
        }
        if let Some(anchor) = slug.strip_prefix('#') {
            if let Some(page) = crate::wiki::page(&self.book_slug) {
                let label = anchor.replace('-', " ");
                if let Some(line) = page.lines().position(|line| line.eq_ignore_ascii_case(&format!("## {label}"))) {
                    self.book_scroll = line as u16;
                }
            }
            return;
        }
        // Home and the index pages are generated, so they have no markdown file.
        let generated = matches!(slug, "start" | "home" | "items" | "machines" | "topics" | "history" | "production" | "research")
            || slug.starts_with("search?q=");
        // A web address always opens, even when the page (or the server) isn't there: the page
        // that shows is T Web's own 404 or "server not found", not a refusal to move.
        if (crate::wiki::page(slug).is_none() && !generated && !crate::web::is_addr(slug)) || slug == self.book_slug {
            return;
        }
        self.book_history.push(self.book_slug.clone());
        self.book_forward.clear();
        self.book_slug = slug.to_string();
        self.book_scroll = 0;
        self.book_selected = 0;
        self.book_folded.clear();
        self.book_opened = Some(std::time::Instant::now());
        if let Some(e) = Self::book_entry_for_slug(slug) {
            self.book_entry = e;
        }
        self.book_land();
    }

    /// Where the typing cursor goes when a page opens: into the box on the start page, nowhere else.
    fn book_land(&mut self) {
        self.book_typing = self.book_slug == "start";
        self.book_pick = 0;
        self.book_pick_follow = false;
        self.book_query.clear();
        self.book_results.clear();
        self.book_login = None;
        self.book_login_typing = false;
        // A sign-in form has the keys from the start: the player opens it to type in it.
        if self.book_asks_login() {
            self.book_typing = false;
            self.start_login();
        }
    }

    /// The user signed in on a domain, if there is one.
    pub fn book_signed_in(&self, domain: &str) -> Option<&str> {
        self.book_logins.iter().find(|(d, _)| d == domain).map(|(_, user)| user.as_str())
    }

    /// Whether the page in the browser is showing a sign-in form: a gated page with nobody
    /// signed in on its domain, or the domain's own `login` page.
    pub fn book_asks_login(&self) -> bool {
        let slug = self.book_slug.as_str();
        if !crate::web::is_addr(slug) {
            return false;
        }
        let domain = crate::web::domain_of(slug);
        if slug.trim_end_matches('/') == format!("{domain}/login") {
            return true;
        }
        // A signed-in user who is not on the list is a 403, not another form.
        crate::web::access(slug).is_some() && self.book_signed_in(domain).is_none()
    }

    /// The text of the page in the browser, as plain text for the clipboard: a header with the
    /// title and address, then the page as the reader sees it. `None` where there is nothing to
    /// copy (the start page, a sign-in form).
    pub fn book_page_text(&self) -> Option<String> {
        let slug = self.book_slug.as_str();
        if slug == "start" || slug == "history" || slug == "production" || slug == "research" || self.book_asks_login() {
            return None;
        }
        let address = crate::ui::book_url(slug);
        if let Some(query) = slug.strip_prefix("search?q=") {
            let hits = crate::search::search(query);
            let mut out = format!("Search: {query}\n{address}\n\n{} results\n", hits.len());
            for h in &hits {
                out.push_str(&format!("\n{}\n{}\n{}\n", crate::ui::book_url(&h.slug), h.title, h.snippet));
            }
            return Some(out);
        }
        let body = if crate::web::is_addr(slug) {
            if let Some(thread) = crate::web::thread(slug) {
                let mut out = format!("{}\n", thread.title);
                for (k, v) in &thread.meta {
                    out.push_str(&format!("{k}: {v}\n"));
                }
                for post in &thread.posts {
                    let pad = "  ".repeat(post.depth);
                    out.push_str(&format!("\n{pad}{} · {}\n", post.author, post.date));
                    for line in &post.body {
                        out.push_str(&format!("{pad}{}\n", crate::search::plain(line)));
                    }
                }
                out
            } else {
                let md = match crate::web::page(slug) {
                    Some(md) => {
                        let mut text = crate::web::prepare(slug, md);
                        if let Some(section) = crate::web::profile_section(slug) {
                            text.push_str(&section);
                        }
                        text
                    }
                    None => crate::web::missing(slug),
                };
                crate::search::plain(&md)
            }
        } else {
            crate::search::plain(crate::wiki::page(slug)?)
        };
        Some(format!("{address}\n\n{body}"))
    }

    /// The Copy button: put the page's text on the clipboard.
    pub fn book_copy(&mut self) {
        if let Some(text) = self.book_page_text() {
            crate::clip::copy(&text);
            self.book_copied = Some(Instant::now());
        }
    }

    /// `i` or Enter on a login page: the boxes take the keys from here.
    fn start_login(&mut self) {
        self.book_login.get_or_insert_with(LoginForm::default).failed = false;
        self.book_login_typing = true;
    }

    /// A key while the sign-in boxes have the keys: characters go to the active box, Tab
    /// swaps them, and nothing else on the page hears the key.
    fn login_key(&mut self, k: KeyEvent) {
        if self.book_login.is_none() {
            self.book_login = Some(LoginForm::default());
        }
        let Some(form) = self.book_login.as_mut() else { return };
        let mut submit = false;
        match k.code {
            KeyCode::Char(c) if !k.modifiers.contains(ratatui::crossterm::event::KeyModifiers::CONTROL) => {
                if form.on_pass {
                    form.pass.push(c);
                } else {
                    form.user.push(c);
                }
            }
            KeyCode::Backspace => {
                if form.on_pass {
                    form.pass.pop();
                } else {
                    form.user.pop();
                }
            }
            KeyCode::Tab | KeyCode::BackTab => form.on_pass = !form.on_pass,
            KeyCode::Enter if form.on_pass => submit = true,
            KeyCode::Enter => form.on_pass = true,
            KeyCode::Esc => self.book_login_typing = false,
            _ => {}
        }
        if submit {
            self.submit_login();
        }
    }

    /// Tries the form's username and password against the domain's accounts. Usernames are
    /// compared without case, passwords exactly. A failure says only that it failed.
    fn submit_login(&mut self) {
        let domain = crate::web::domain_of(&self.book_slug).to_string();
        let Some(form) = self.book_login.clone() else { return };
        let account =
            crate::web::accounts(&domain).into_iter().find(|a| a.user.eq_ignore_ascii_case(form.user.trim()) && a.pass == form.pass);
        match account {
            Some(a) => {
                let on_login_page = self.book_slug.trim_end_matches('/') == format!("{domain}/login");
                self.book_logins.retain(|(d, _)| *d != domain);
                self.book_logins.push((domain.clone(), a.user));
                self.book_login = None;
                self.book_login_typing = false;
                self.book_selected = 0;
                // The domain's own login page leads on to the domain; a gated page just opens.
                if on_login_page {
                    self.book_go_slug(&domain);
                }
            }
            None => {
                if let Some(f) = self.book_login.as_mut() {
                    f.failed = true;
                    f.pass.clear();
                    f.on_pass = true;
                }
            }
        }
    }

    /// Runs a search: the results page for these words.
    pub fn book_submit_search(&mut self, query: &str) {
        let q = query.split_whitespace().collect::<Vec<_>>().join(" ");
        if q.is_empty() {
            return;
        }
        if !crate::web::is_addr(&q) {
            self.settings.search_history.retain(|h| !h.eq_ignore_ascii_case(&q));
            self.settings.search_history.insert(0, q.clone());
            self.settings.search_history.truncate(20);
            if let Some(path) = &self.settings_path {
                let _ = self.settings.save(path);
            }
        }
        // A domain or address typed into the box opens it, rather than searching for it.
        if crate::web::is_addr(&q) {
            self.book_go_slug(&q);
            return;
        }
        self.book_go_slug(&format!("search?q={q}"));
    }

    /// The slug of the best result for these words, or a random wiki page for an empty search.
    fn book_lucky(&self, query: &str) -> Option<String> {
        if let Some(hit) = crate::search::search(query).into_iter().next() {
            return Some(hit.slug);
        }
        let pages = crate::wiki::PAGES;
        (query.trim().is_empty() && !pages.is_empty()).then(|| pages[self.book_seed % pages.len()].0.to_string())
    }

    fn book_entry_for_slug(slug: &str) -> Option<Entry> {
        crate::sim::Item::ALL
            .iter()
            .copied()
            .map(Entry::Item)
            .chain(BOOK_PARTS.iter().copied().map(Entry::Part))
            .find(|e| crate::wiki::slug(e.name()) == slug)
    }
    pub(crate) fn book_entry_for_slug_for_ui(slug: &str) -> Option<Entry> {
        Self::book_entry_for_slug(slug)
    }

    fn book_search(&mut self) {
        let q = self.book_query.to_lowercase();
        // Only the item and machine grids filter as you type; every other page searches on Enter.
        self.book_results = if q.is_empty() || !(self.book_slug == "items" || self.book_slug == "machines") {
            Vec::new()
        } else {
            Entry::list(&q, self.book_slug == "machines").into_iter().map(|e| crate::wiki::slug(e.name())).collect()
        };
        self.book_selected = self.book_selected.min(self.book_results.len().saturating_sub(1));
    }

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

    /// `o` and `t`: one of the game's own sites (production, research) in the browser, in the
    /// tab that already shows it or a new one.
    pub fn open_site(&mut self, slug: &str) {
        self.book_store();
        self.book = true;
        self.book_typing = false;
        if let Some(i) = self.book_tabs.iter().position(|t| t.slug == slug) {
            self.book_switch(i);
        } else {
            if slug == "production" {
                self.stats_scroll = 0;
                self.stats_history = false;
            }
            self.book_new_tab(slug);
        }
    }

    /// Keys on the research site: the research screen in a page. Returns whether the key was used.
    fn research_key(&mut self, k: KeyEvent, action: Option<Action>) -> bool {
        let used = matches!(
            k.code,
            KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::PageUp
                | KeyCode::PageDown
                | KeyCode::Home
                | KeyCode::End
        ) || matches!(action, Some(Action::MoveUp | Action::MoveDown | Action::Place));
        if used {
            self.tree_key(k.code, action);
        }
        used
    }

    /// Keys on the production site: it is the production panel in a page, so Tab swaps its two
    /// tabs and the panel's own keys scroll it. Returns whether the key was used.
    fn production_key(&mut self, k: KeyEvent, action: Option<Action>) -> bool {
        let shift = k.modifiers.contains(ratatui::crossterm::event::KeyModifiers::SHIFT);
        match (k.code, action) {
            (KeyCode::Tab | KeyCode::BackTab, _) => self.stats_tab(!self.stats_history),
            _ if self.stats_history && self.history_key(k.code) => {}
            (KeyCode::PageUp, _) => self.stats_scroll = self.stats_scroll.saturating_sub(5),
            (KeyCode::PageDown, _) => self.stats_scroll += 5,
            (_, Some(Action::MoveUp)) if !shift => self.stats_scroll = self.stats_scroll.saturating_sub(1),
            (_, Some(Action::MoveDown)) if !shift => self.stats_scroll += 1,
            (_, Some(Action::Pause)) => self.paused = !self.paused,
            (_, Some(Action::Faster)) => self.speed = (self.speed * 2).min(8),
            (_, Some(Action::Slower)) => self.speed = (self.speed / 2).max(1),
            _ => return false,
        }
        true
    }

    /// What a typed address means: a page of termweb, a wiki page, a site, or a search.
    pub fn book_submit_address(&mut self, text: &str) {
        let t = text.trim();
        let t = t.strip_prefix("https://").or_else(|| t.strip_prefix("http://")).unwrap_or(t).trim_end_matches('/');
        if t.is_empty() {
            return;
        }
        if t == "termweb.search" {
            self.book_go_slug("start");
        } else if let Some(rest) = t.strip_prefix("termweb.search/") {
            self.book_go_slug(if rest == "history" { "history" } else { rest });
        } else if let Some(slug) = t.strip_prefix("terminactory.wiki/") {
            self.book_go_slug(slug);
        } else if t == "production.lan" {
            self.book_go_slug("production");
        } else if t == "research.lan" {
            self.book_go_slug("research");
        } else if t == "terminactory.wiki" {
            self.book_go_slug("home");
        } else {
            self.book_submit_search(t);
        }
    }

    /// Keys while the address field is being edited.
    fn url_key(&mut self, k: KeyEvent) {
        let Some(text) = self.book_url_edit.as_mut() else { return };
        match k.code {
            KeyCode::Esc => self.book_url_edit = None,
            KeyCode::Enter => {
                let t = text.clone();
                self.book_url_edit = None;
                self.book_submit_address(&t);
            }
            KeyCode::Backspace => {
                if self.book_url_fresh {
                    text.clear();
                } else {
                    text.pop();
                }
                self.book_url_fresh = false;
            }
            KeyCode::Char(c) if !k.modifiers.contains(ratatui::crossterm::event::KeyModifiers::CONTROL) => {
                if self.book_url_fresh {
                    text.clear();
                }
                text.push(c);
                self.book_url_fresh = false;
            }
            _ => {}
        }
    }

    /// Yellow dot, `b`, Esc: put the browser away. The tabs stay as they are.
    pub fn book_minimize(&mut self) {
        self.book_store();
        self.book = false;
    }

    /// Red dot: close the browser and forget its tabs.
    pub fn book_close_window(&mut self) {
        self.book = false;
        self.book_full = false;
        self.book_tabs = vec![Tab::home()];
        self.book_active = 0;
        self.book_load_tab(0);
    }

    /// Adds the page being read to the bookmarks bar, or takes it off, and saves the list.
    pub fn book_toggle_mark(&mut self) {
        let slug = self.book_slug.clone();
        if let Some(i) = self.settings.bookmarks.iter().position(|s| *s == slug) {
            self.settings.bookmarks.remove(i);
        } else {
            self.settings.bookmarks.push(slug);
        }
        if let Some(path) = &self.settings_path {
            let _ = self.settings.save(path);
        }
    }

    #[allow(dead_code)]
    fn book_key(&mut self, k: KeyEvent, action: Option<Action>) {
        if self.book_url_edit.is_some() {
            self.url_key(k);
            return;
        }
        // Whatever else had the keys, a sign-in form on the page takes them.
        if self.book_asks_login() && !self.book_login_typing && matches!(k.code, KeyCode::Char(_)) {
            self.book_typing = false;
            self.start_login();
        }
        if self.book_typing {
            match k.code {
                KeyCode::Char(c) if !k.modifiers.contains(ratatui::crossterm::event::KeyModifiers::CONTROL) => self.book_query.push(c),
                KeyCode::Backspace => {
                    // With nothing typed on the start page, Backspace goes back like anywhere else.
                    if self.book_query.is_empty() && self.book_slug == "start" {
                        self.book_back();
                    } else {
                        self.book_query.pop();
                    }
                }
                KeyCode::Enter => {
                    let grid = self.book_slug == "items" || self.book_slug == "machines";
                    if grid {
                        if let Some(slug) = self.book_results.get(self.book_selected).cloned() {
                            self.book_go_slug(&slug);
                            self.book_query.clear();
                            self.book_results.clear();
                        }
                        self.book_typing = false;
                    } else {
                        let q = self.book_query.clone();
                        self.book_typing = false;
                        self.book_query.clear();
                        self.book_submit_search(&q);
                    }
                }
                KeyCode::Tab => self.book_cycle_tab(true),
                KeyCode::BackTab => self.book_cycle_tab(false),
                KeyCode::Up => {
                    self.book_selected = self.book_selected.saturating_sub(1);
                    if self.book_selected < self.book_scroll as usize {
                        self.book_scroll = self.book_selected as u16;
                    }
                }
                KeyCode::Down => {
                    self.book_selected = (self.book_selected + 1).min(self.book_results.len().saturating_sub(1));
                    if self.book_selected >= self.book_scroll as usize + self.book_result_capacity {
                        self.book_scroll = self.book_selected.saturating_sub(self.book_result_capacity.saturating_sub(1)) as u16;
                    }
                }
                KeyCode::Esc => {
                    self.book_typing = false;
                    if !(self.book_slug == "items" || self.book_slug == "machines") {
                        self.book_query.clear();
                        self.book_results.clear();
                    }
                }
                _ => {}
            }
            self.book_search();
            return;
        }
        // The sign-in boxes take every key while they are open, and `i` or Enter opens them.
        if self.book_login_typing {
            self.login_key(k);
            return;
        }
        if self.book_asks_login() && matches!(k.code, KeyCode::Char('i') | KeyCode::Enter) {
            self.start_login();
            return;
        }
        if self.book_slug == "production" && self.production_key(k, action) {
            return;
        }
        if self.book_slug == "research" && self.research_key(k, action) {
            return;
        }
        // With nothing focused, a digit picks a tab.
        if let KeyCode::Char(c @ '1'..='9') = k.code {
            if !k.modifiers.contains(ratatui::crossterm::event::KeyModifiers::CONTROL) {
                self.book_switch(c as usize - '1' as usize);
                // Landing on a start page would focus its box; the next digit should still pick a tab.
                self.book_typing = false;
                return;
            }
        }
        match (k.code, action) {
            // In the browser, b opens a new tab and Esc puts the browser away.
            (KeyCode::Char('b'), _) | (_, Some(Action::Book)) => self.book_new_tab("start"),
            (KeyCode::Esc, _) | (_, Some(Action::Back)) => self.book_minimize(),
            (KeyCode::Enter, _) if k.modifiers.contains(ratatui::crossterm::event::KeyModifiers::SHIFT) => {
                // Shift+Enter: the selected link opens in a tab of its own.
                let slug = if self.book_query.is_empty() {
                    self.book_urls.get(self.book_selected).map(|(_, s)| s.clone())
                } else {
                    self.book_results.get(self.book_selected).cloned()
                };
                if let Some(slug) = slug {
                    self.book_query.clear();
                    self.book_results.clear();
                    self.book_new_tab(&slug);
                }
            }
            (KeyCode::Char('w'), _) => {
                self.book_close_tab();
            }
            (KeyCode::Tab, _) => {
                self.book_cycle_tab(true);
            }
            (KeyCode::BackTab, _) => {
                self.book_cycle_tab(false);
            }
            (KeyCode::Char('+'), _) => {
                self.book_new_tab("start");
            }
            (KeyCode::Char('*'), _) => {
                self.book_toggle_mark();
            }
            (KeyCode::Char('/'), _) => {
                self.book_typing = true;
                self.book_query = self.book_slug.strip_prefix("search?q=").unwrap_or("").to_string();
                self.book_search();
            }
            (KeyCode::PageUp, _) => {
                self.book_scroll = self.book_scroll.saturating_sub(5);
            }
            (KeyCode::PageDown, _) => {
                self.book_scroll += 5;
            }
            (KeyCode::Backspace, _) => {
                self.book_back();
            }
            (KeyCode::Char(']'), _) => {
                self.book_forward();
            }
            (KeyCode::Left | KeyCode::Right, _) if self.book_slug.starts_with("search?q=") => {
                // On a results page the side arrows jump straight to the previous or next result.
                let n = self.book_hits.as_ref().map_or(0, |(_, h)| h.len());
                if n > 0 {
                    self.book_pick = if k.code == KeyCode::Right { (self.book_pick + 1) % n } else { (self.book_pick + n - 1) % n };
                    self.book_pick_follow = true;
                }
            }
            (KeyCode::Left | KeyCode::Right, _) if self.book_query.is_empty() => {
                // Along the current line: the nearest link to the left or right on the same row.
                if let Some(&(cur, _)) = self.book_urls.get(self.book_selected) {
                    let right = k.code == KeyCode::Right;
                    let next = self
                        .book_urls
                        .iter()
                        .enumerate()
                        .filter(|(_, (r, _))| r.y == cur.y && if right { r.x > cur.x } else { r.x < cur.x })
                        .min_by_key(|(_, (r, _))| r.x.abs_diff(cur.x));
                    // At the end of the line, roll over to its other end.
                    let wrap = || {
                        let same = self.book_urls.iter().enumerate().filter(|(_, (r, _))| r.y == cur.y);
                        if right {
                            same.min_by_key(|(_, (r, _))| r.x)
                        } else {
                            same.max_by_key(|(_, (r, _))| r.x)
                        }
                    };
                    if let Some((i, _)) = next.or_else(wrap) {
                        self.book_selected = i;
                    }
                }
            }
            (KeyCode::Up, _) | (_, Some(Action::MoveUp)) => {
                if !self.book_query.is_empty() {
                    if self.book_selected == 0 {
                        // Roll over to the last result.
                        self.book_selected = self.book_results.len().saturating_sub(1);
                        self.book_scroll = self.book_selected.saturating_sub(self.book_result_capacity.saturating_sub(1)) as u16;
                    } else {
                        self.book_selected -= 1;
                    }
                    if self.book_selected < self.book_scroll as usize {
                        self.book_scroll = self.book_selected as u16;
                    }
                } else if let Some(i) = self.book_link_line(false) {
                    self.book_selected = i;
                } else if self.book_scroll > 0 {
                    self.book_scroll = self.book_scroll.saturating_sub(self.book_scroll_step);
                    self.book_selected = 0;
                } else {
                    // At the top of the page: roll over to the bottom and its last link.
                    self.book_scroll = self.book_max_scroll;
                    self.book_selected = usize::MAX;
                }
            }
            (KeyCode::Down, _) | (_, Some(Action::MoveDown)) => {
                if !self.book_query.is_empty() {
                    if self.book_selected + 1 >= self.book_results.len() {
                        // Roll over to the first result.
                        self.book_selected = 0;
                        self.book_scroll = 0;
                    } else {
                        self.book_selected += 1;
                    }
                    if self.book_selected >= self.book_scroll as usize + self.book_result_capacity {
                        self.book_scroll = self.book_selected.saturating_sub(self.book_result_capacity.saturating_sub(1)) as u16;
                    }
                } else if let Some(i) = self.book_link_line(true) {
                    self.book_selected = i;
                } else if self.book_scroll < self.book_max_scroll {
                    // No link further down on screen: scroll, keeping the cursor near the bottom.
                    self.book_scroll = self.book_scroll.saturating_add(self.book_scroll_step);
                    self.book_selected = self.book_urls.len().saturating_sub(1);
                } else {
                    // At the bottom of the page: roll over to the top and its first link.
                    self.book_scroll = 0;
                    self.book_selected = 0;
                }
            }
            (KeyCode::Enter, _) => {
                if !self.book_query.is_empty() {
                    if let Some(slug) = self.book_results.get(self.book_selected).cloned() {
                        self.book_go_slug(&slug);
                        self.book_query.clear();
                        self.book_results.clear();
                    }
                } else if let Some((_, slug)) = self.book_urls.get(self.book_selected) {
                    let slug = slug.clone();
                    if slug == "close-book" {
                        self.book = false;
                    } else {
                        self.book_go_slug(&slug);
                    }
                }
            }
            // f and F fold posts, on a thread only; everywhere else a letter starts a search.
            (KeyCode::Char('f'), _) if crate::web::thread(&self.book_slug).is_some() => self.book_fold(false),
            (KeyCode::Char('F'), _) if crate::web::thread(&self.book_slug).is_some() => self.book_fold(true),
            (KeyCode::Char(c), _) if c.is_alphabetic() => {
                self.book_typing = true;
                self.book_query.clear();
                self.book_query.push(c);
                self.book_search();
            }
            _ => {}
        }
    }

    /// Clicks in the book: a link goes there (Backspace comes back), a grid cell or tab
    /// shows that page; the wheel scrolls the page.
    fn book_mouse(&mut self, m: MouseEvent) {
        let at = ratatui::layout::Position::new(m.column, m.row);
        let inside = self.book_area.contains(at);
        if inside && self.book_slug == "research" {
            match m.kind {
                MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                    self.tree_mouse(m);
                    return;
                }
                MouseEventKind::Down(MouseButton::Left) if self.tree_rows.iter().any(|(r, _)| r.contains(at)) => {
                    self.tree_mouse(m);
                    return;
                }
                _ => {}
            }
        }
        if inside && self.book_slug == "production" {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    self.stats_scroll = self.stats_scroll.saturating_sub(1);
                    return;
                }
                MouseEventKind::ScrollDown => {
                    self.stats_scroll += 1;
                    return;
                }
                MouseEventKind::Down(MouseButton::Left) => match self.stats_hits.iter().find(|(r, _)| r.contains(at)).map(|(_, h)| *h) {
                    Some(StatsHit::Tab(h)) => {
                        self.stats_tab(h);
                        return;
                    }
                    Some(StatsHit::Range(r)) => {
                        self.hist_range = r;
                        return;
                    }
                    None => {}
                },
                _ => {}
            }
        }
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if !inside {
                    self.book_minimize();
                    return;
                }
                // A click anywhere but the box being typed in takes the keys away from it.
                let on = |b: BookButton| self.book_buttons.iter().any(|(r, x)| *x == b && r.contains(at));
                if !on(BookButton::Edit) {
                    self.book_typing = false;
                }
                if !on(BookButton::Address) {
                    self.book_url_edit = None;
                }
                if let Some((_, i)) = self.book_tab_close.iter().find(|(r, _)| r.contains(at)) {
                    self.book_switch(*i);
                    self.book_close_tab();
                    return;
                }
                if let Some((_, i)) = self.book_tab_hits.iter().find(|(r, _)| r.contains(at)) {
                    self.book_switch(*i);
                    return;
                }
                if let Some((_, button)) = self.book_buttons.iter().find(|(r, _)| r.contains(at)) {
                    match button {
                        BookButton::Back => self.book_back(),
                        BookButton::Forward => self.book_forward(),
                        BookButton::Reload => self.book_reload(),
                        BookButton::Home => self.book_go_slug("start"),
                        BookButton::NewTab => self.book_new_tab("start"),
                        BookButton::Star => self.book_toggle_mark(),
                        BookButton::Search => {
                            let q = self.book_query.clone();
                            self.book_submit_search(&q);
                        }
                        BookButton::Lucky => {
                            let q = self.book_query.clone();
                            if let Some(slug) = self.book_lucky(&q) {
                                self.book_go_slug(&slug);
                            }
                        }
                        BookButton::Copy => self.book_copy(),
                        BookButton::History => self.book_go_slug("history"),
                        BookButton::Address => {
                            self.book_typing = false;
                            self.book_url_edit = Some(crate::ui::book_url(&self.book_slug));
                            self.book_url_fresh = true;
                        }
                        BookButton::ClearHistory => {
                            self.settings.search_history.clear();
                            if let Some(path) = &self.settings_path {
                                let _ = self.settings.save(path);
                            }
                        }
                        BookButton::CloseWindow => self.book_close_window(),
                        BookButton::Minimize => self.book_minimize(),
                        BookButton::Fullscreen => self.book_full = !self.book_full,
                        BookButton::Edit => {
                            self.book_typing = true;
                            self.book_query = self.book_slug.strip_prefix("search?q=").unwrap_or("").to_string();
                        }
                    }
                    return;
                }
                // A double click on the blank part of the title bar or toolbar toggles fullscreen.
                if self.book_nav.iter().any(|r| r.contains(at)) {
                    let double =
                        self.book_last_click.is_some_and(|(t, x, y)| t.elapsed().as_millis() < 450 && x.abs_diff(at.x) <= 2 && y == at.y);
                    self.book_last_click = Some((Instant::now(), at.x, at.y));
                    if double {
                        self.book_full = !self.book_full;
                        self.book_last_click = None;
                    }
                    return;
                }
                if let Some(slug) = self.book_click_only.iter().find(|(r, _)| r.contains(at)).map(|(_, s)| s.clone()) {
                    self.book_go_slug(&slug);
                    return;
                }
                if let Some(slug) = self.book_mark_hits.iter().find(|(r, _)| r.contains(at)).map(|(_, s)| s.clone()) {
                    self.book_go_slug(&slug);
                    return;
                }
                // The `[+] n replies hidden` row of a folded post opens it again.
                if let Some((_, post)) = self.book_fold_hits.iter().find(|(r, _)| r.contains(at)).copied() {
                    self.book_folded.retain(|i| *i != post);
                    self.book_selected = self.book_selected.min(self.book_urls.len().saturating_sub(1));
                    return;
                }
                if let Some(slug) = self.book_urls.iter().find(|(r, _)| r.contains(at)).map(|(_, s)| s.clone()) {
                    self.book_go_slug(&slug);
                }
            }
            MouseEventKind::ScrollUp if inside => self.book_scroll = self.book_scroll.saturating_sub(2),
            MouseEventKind::ScrollDown if inside => self.book_scroll += 2,
            _ => {}
        }
    }

    /// What's picked on the research screen.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn tree_choice(&self) -> Tech {
        crate::research::tech_for_node(self.tree_pick).unwrap_or(Tech::Press)
    }

    fn move_tree(&mut self, code: KeyCode) {
        let id = self.tree_pick;
        let (parent, children, siblings) = crate::research::neighbors_in(id, &self.tree_expanded);
        match code {
            KeyCode::Left if self.tree_expanded.remove(&id) => {}
            KeyCode::Left => self.select_visible(parent),
            KeyCode::Right if crate::research::has_children(id) && !self.tree_expanded.contains(&id) => {
                self.tree_expanded.insert(id);
            }
            KeyCode::Right => self.select_visible(children.first().copied()),
            KeyCode::Up => self.select_visible(siblings.0),
            KeyCode::Down => self.select_visible(siblings.1),
            _ => {}
        }
    }

    fn select_visible(&mut self, id: Option<usize>) {
        if let Some(id) = id {
            self.tree_pick = id;
        }
    }

    pub fn ensure_tree_selection_visible(&mut self) {
        self.tree_expanded.extend(crate::research::ancestors(self.tree_pick));
    }

    /// The research screen: up/down pick, Enter researches.
    fn tree_key(&mut self, code: KeyCode, action: Option<Action>) {
        self.ensure_tree_selection_visible();
        let ids: Vec<usize> = crate::research::layout_expanded(&self.tree_expanded).iter().map(|p| p.id).collect();
        let at = ids.iter().position(|&id| id == self.tree_pick).unwrap_or(0);
        let n = ids.len();
        match (code, action) {
            (KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down, _) => self.move_tree(code),
            (KeyCode::PageUp, _) => self.select_visible(ids.get(at.saturating_sub(5)).copied()),
            (KeyCode::PageDown, _) => self.select_visible(ids.get((at + 5).min(n - 1)).copied()),
            (KeyCode::Home, _) => self.select_visible(ids.first().copied()),
            (KeyCode::End, _) => self.select_visible(ids.last().copied()),
            (_, Some(Action::MoveUp)) => self.select_visible(ids.get(at.saturating_sub(1)).copied()),
            (_, Some(Action::MoveDown)) => self.select_visible(ids.get((at + 1).min(n - 1)).copied()),
            (_, Some(Action::Place)) if crate::research::tech_for_node(self.tree_pick).is_some() => self.buy_research(),
            _ => {}
        }
    }

    fn buy_research(&mut self) {
        let Some(t) = crate::research::tech_for_node(self.tree_pick) else { return };
        match self.world.research(t) {
            Ok(()) => self.say(format!("Researched {}: {}", t.name(), t.about())),
            Err(e) => self.say(e),
        }
    }

    fn tree_mouse(&mut self, m: MouseEvent) {
        let at = ratatui::layout::Position::new(m.column, m.row);
        let rows = tree_rows();
        let n = rows.len();
        let current = crate::research::tech_for_node(self.tree_pick).and_then(|t| rows.iter().position(|&x| x == t)).unwrap_or(0);
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(&(_, i)) = self.tree_rows.iter().find(|(r, _)| r.contains(at)) {
                    // Click picks a row; clicking the picked row again researches it.
                    let id = crate::research::node_id_for_tech(tree_rows()[i]).unwrap();
                    if self.tree_pick == id {
                        self.buy_research();
                    }
                    self.tree_pick = id;
                }
            }
            MouseEventKind::ScrollUp => self.tree_pick = crate::research::node_id_for_tech(rows[current.saturating_sub(1)]).unwrap(),
            MouseEventKind::ScrollDown => self.tree_pick = crate::research::node_id_for_tech(rows[(current + 1).min(n - 1)]).unwrap(),
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
        if self.library.is_some() && !self.help && self.away.is_none() && !self.book {
            self.library_mouse(m);
            return;
        }
        if self.book || self.help || self.settings_open || self.away.is_some() {
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
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
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
        assert_eq!(got, vec![7, 5, 4, 3], "Create, Transport, EC, Misc");
        // Every category is in the table once.
        for c in Category::ALL {
            assert_eq!(MAX_SLOTS.iter().filter(|(k, _)| *k == c).count(), 1, "{c:?}");
        }
    }

    #[test]
    fn pipes_have_transport_slot_five_and_pumps_keep_create_five() {
        assert_eq!(slots(Category::Transport)[4], &[Kind::Pipe]);
        assert_eq!(slots(Category::Create)[4], &[Kind::Pump, Kind::AdvancedPump, Kind::Mixer, Kind::Evacuator, Kind::OilExtractor]);
        assert_eq!(slots(Category::Create)[5], &[Kind::Etcher, Kind::CrystalGrower, Kind::WaferSaw, Kind::FineEtcher]);
        // The assembler slot: the large precision assembler right after the large advanced one, and the welder last.
        let asm = slots(Category::Create)[3];
        let at = |k: Kind| asm.iter().position(|x| *x == k).unwrap();
        assert_eq!(at(Kind::LargePrecisionAssembler), at(Kind::LargeAdvancedAssembler) + 1);
        assert_eq!(asm.last(), Some(&Kind::Welder));
        assert_eq!(Kind::Pipe.category(), Category::Transport);
        assert!(!parts_in(Category::Create).contains(&Kind::Pipe));
        assert_eq!(slots(Category::Misc), &[&[Kind::Crate, Kind::Warehouse][..], &[Kind::TradePost, Kind::LiquidTrader][..]]);
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
            let wants = !k.recipes().is_empty() || k.is_drill() || k.is_pump() || k.is_extractor() || k.rules().is_some();
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
        assert_eq!(pipes, &[Tech::Pumps, Tech::AdvancedPump, Tech::OilExtractor, Tech::LiquidTrader]);
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
        assert!(app.book && app.book_slug == "research", "t opens the research site");
        assert_eq!(app.tree_choice(), Tech::Press);
        key(&mut app, KeyCode::Right);
        assert_eq!(app.tree_choice(), Tech::Press);
        key(&mut app, KeyCode::Right);
        assert_eq!(
            app.tree_pick,
            crate::content::TREE
                .iter()
                .position(|(n, _)| *n == crate::content::Node::Key(crate::content::Made::Item(crate::content::Item::CopperWire)))
                .unwrap()
        );
        key(&mut app, KeyCode::Right);
        key(&mut app, KeyCode::Right);
        assert_eq!(app.tree_choice(), Tech::BasicDrill);
        key(&mut app, KeyCode::Left);
        assert_eq!(
            app.tree_pick,
            crate::content::TREE
                .iter()
                .position(|(n, _)| *n == crate::content::Node::Key(crate::content::Made::Item(crate::content::Item::CopperWire)))
                .unwrap()
        );
        key(&mut app, KeyCode::Left);
        key(&mut app, KeyCode::Left);
        assert_eq!(app.tree_choice(), Tech::Press);
        key(&mut app, KeyCode::Down);
        assert_eq!(app.tree_choice(), Tech::Coking);
        key(&mut app, KeyCode::Up);
        assert_eq!(app.tree_choice(), Tech::Press);
        key(&mut app, KeyCode::Down);
        assert_eq!(app.tree_choice(), Tech::Coking);
        // Enter researches when there are points to spend.
        app.world.econ.research = 150.0;
        key(&mut app, KeyCode::Enter);
        assert!(app.world.progress.has(Tech::Coking));
        assert_eq!(app.world.econ.research, 50.0);
        key(&mut app, KeyCode::Esc);
        assert!(!app.book, "Esc puts the browser away");
    }
}
