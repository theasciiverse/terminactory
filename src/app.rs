//! Player state and input handling (keyboard + mouse).

use std::time::Instant;

use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

use crate::controls::{Action, Controls};
use crate::sim::{in_bounds, Category, Company, Dir, Kind, Tech, World, H, MAX_LIFT, MIN_LIFT, SPAWN, UNLOCK_GROUPS, W};

/// The numbered slots in a build category. A slot can hold several kinds of the
/// same part (copper drill, basic drill...); pressing its number again switches between them.
pub fn slots(cat: Category) -> &'static [&'static [Kind]] {
    use Kind::*;
    match cat {
        Category::Create => &[&[Drill, BasicDrill], &[Furnace, Molder, BlastFurnace], &[Press, Assembler], &[Pump, Pipe]],
        Category::Transport => &[&[Belt, FastBelt, Tunnel], &[Splitter, Sorter], &[Arm, LongArm], &[Elevator]],
        Category::Power => &[&[Solar], &[Pole], &[Boiler], &[SteamEngine]],
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

/// Machines with their own page in the recipe book.
pub const BOOK_PARTS: [Kind; 7] =
    [Kind::Drill, Kind::BasicDrill, Kind::Furnace, Kind::Molder, Kind::BlastFurnace, Kind::Press, Kind::Assembler];

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

pub struct App {
    pub world: World,
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
    /// A company's letter waiting to be read.
    pub letter: Option<Company>,
    /// "While you were away" summary waiting to be read.
    pub away: Option<String>,
    /// An agent message being read, and how far it's scrolled.
    pub mail: Option<crate::mail::Message>,
    pub mail_scroll: u16,
    /// The telegram is open over the map (small pane); in the full view it sits in the sidebar.
    pub mail_open: bool,
    /// Where the sidebar telegram was drawn, so the mouse wheel can scroll it.
    pub mail_area: Rect,
    /// The production panel is open (over the map; the game keeps running), and its scroll.
    pub stats: bool,
    pub stats_scroll: u16,
    /// How far the help screen is scrolled (it's taller than a small pane).
    pub help_scroll: u16,
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
            letter: None,
            away: None,
            mail: None,
            mail_scroll: 0,
            mail_open: false,
            mail_area: Rect::default(),
            help_scroll: 0,
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

    pub fn say(&mut self, s: impl Into<String>) {
        self.msg = s.into();
        self.msg_at = Instant::now();
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
        if self.letter.is_some() {
            self.letter = None;
            return;
        }
        if self.away.is_some() {
            self.away = None;
            return;
        }
        if self.mail_open {
            // Arrows / j k / PgUp PgDn scroll a long message; any other key closes (and dismisses) it.
            match k.code {
                KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => self.mail_scroll = self.mail_scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('s') => self.mail_scroll += 1,
                KeyCode::PageUp => self.mail_scroll = self.mail_scroll.saturating_sub(10),
                KeyCode::PageDown => self.mail_scroll += 10,
                KeyCode::Esc => {
                    self.mail = None;
                    self.mail_open = false;
                    self.world.mail.clear();
                }
                _ => {
                    self.mail = None;
                    self.mail_open = false;
                }
            }
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
            // Arrows scroll it, o or Esc closes it; any other key closes it and goes on to the game.
            match (k.code, action) {
                (KeyCode::PageUp, _) => self.stats_scroll = self.stats_scroll.saturating_sub(5),
                (KeyCode::PageDown, _) => self.stats_scroll += 5,
                (_, Some(Action::MoveUp)) if !shift => self.stats_scroll = self.stats_scroll.saturating_sub(1),
                (_, Some(Action::MoveDown)) if !shift => self.stats_scroll += 1,
                (_, Some(Action::Stats | Action::Back)) => self.stats = false,
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
                self.menu = None;
                self.selected = None;
            }
            Action::Tree => self.tree = true,
            Action::Stats => {
                self.stats = true;
                self.stats_scroll = 0;
            }
            Action::Book => self.open_book(),
            Action::Mail => {
                if self.mail.is_none() {
                    self.say("No telegrams waiting");
                } else if shift {
                    self.mail = None;
                    self.world.mail.clear();
                    self.say("All telegrams dismissed");
                } else if self.compact_now {
                    self.mail_open = true;
                } else {
                    self.mail = None;
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
            Action::Rotate => self.rotate(),
            Action::Remove => self.remove_here(),
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
                let Some(&cat) = Category::ALL.get(n - 1) else { return };
                self.menu = Some(cat);
                let count = slots(cat).len();
                let back = self.controls.label(Action::Back);
                self.say(format!("{}: press 1-{count} to pick, again to switch kind, {back} to go back", cat.name()));
            }
            Some(cat) => {
                let Some(&slot) = slots(cat).get(n - 1) else { return };
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
                    format!("make {} first, then research it (t)", t.requires().map_or("its key item", |i| i.name()))
                }
                _ => "research it first (t)".into(),
            };
            self.say(format!("{} is locked: {}", kind.name(), why));
            return;
        }
        self.selected = Some(kind);
        self.paste = None;
        let note = if kind.preview() { " (preview: not in the base tier)" } else { "" };
        self.say(format!("{} selected, ${:.0}{}", kind.name(), kind.cost(), note));
    }

    /// Arrow keys walk the tree: left to the parent, right to the first child,
    /// up and down to the nearest box in the same column.
    /// Opens the recipe book on what's under the cursor: a machine's recipe, what a drill
    /// mines, or the machine itself.
    fn open_book(&mut self) {
        let (x, y) = self.cursor;
        let here = self.world.tile(self.floor, x, y).and_then(|b| match &b.st {
            crate::sim::St::Drill { mines: Some(m), .. } => Some(Entry::Item(*m)),
            _ => b.recipe().and_then(|r| r.outputs.first()).map(|o| Entry::Item(o.0)).or(BOOK_PARTS.contains(&b.kind).then_some(Entry::Part(b.kind))),
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
        let over_mail = self.mail.is_some()
            && !self.compact_now
            && self.mail_area.contains(ratatui::layout::Position::new(m.column, m.row));
        if (self.mail_open || over_mail) && self.letter.is_none() && self.away.is_none() {
            match m.kind {
                MouseEventKind::ScrollUp => self.mail_scroll = self.mail_scroll.saturating_sub(2),
                MouseEventKind::ScrollDown => self.mail_scroll += 2,
                _ => {}
            }
            return;
        }
        if self.book && !self.mail_open && self.letter.is_none() && self.away.is_none() && !self.help {
            self.book_mouse(m);
            return;
        }
        if self.tree && !self.mail_open && self.letter.is_none() && self.away.is_none() && !self.help {
            self.tree_mouse(m);
            return;
        }
        if self.tree || self.book || self.help || self.letter.is_some() || self.away.is_some() {
            return;
        }
        if self.stats {
            // The panel covers the map: the wheel scrolls it and a click closes it.
            match m.kind {
                MouseEventKind::ScrollUp => self.stats_scroll = self.stats_scroll.saturating_sub(1),
                MouseEventKind::ScrollDown => self.stats_scroll += 1,
                MouseEventKind::Down(_) => self.stats = false,
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
                    if self.selected.is_some() {
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
            MouseEventKind::Up(MouseButton::Left) => self.drag_last = None,
            MouseEventKind::Down(MouseButton::Middle) => {
                if let Some(t) = tile {
                    self.cursor = t;
                    self.copy_here();
                }
            }
            MouseEventKind::Down(MouseButton::Right) | MouseEventKind::Drag(MouseButton::Right) => {
                if let Some(t) = tile {
                    self.cursor = t;
                    self.remove_here();
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
                if to.0 > x { Dir::E } else { Dir::W }
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
                self.world.set_dir(self.floor, x, y, d);
                true
            }
            Some(_) => true, // step over other buildings
        }
    }

    fn place_here(&mut self) {
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
        let Some(b) = self.world.tile(self.floor, x, y) else {
            if self.selected.take().is_some() {
                self.menu = None;
                self.say("Hand empty");
            }
            return;
        };
        let (kind, dir, size) = (b.kind, b.dir, b.size());
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

    fn remove_here(&mut self) {
        let (x, y) = self.cursor;
        if let Some(k) = self.world.remove(self.floor, x, y) {
            self.say(format!("Removed {} (refunded in full)", k.name()));
        }
    }

    fn rotate(&mut self) {
        if self.selected.is_some_and(|k| k.directional()) {
            self.dir = self.dir.cw();
            return;
        }
        let (x, y) = self.cursor;
        if let Some(b) = self.world.tile(self.floor, x, y) {
            let nd = b.dir.cw();
            if !self.world.set_dir(self.floor, x, y, nd) {
                self.say("This part has no output side to rotate");
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
                self.floor = f;
                let what = match level {
                    0 => "Ground level".to_string(),
                    l if l < 0 => format!("Floor {l}: digging costs ${:.0} a tile, drills here mine {}x faster", crate::sim::tile_cost(l), crate::sim::richness(l)),
                    l => format!("Floor +{l}: decking costs ${:.0} a tile", crate::sim::tile_cost(l)),
                };
                self.say(what);
            }
            Some(_) => self.say(self.world.floor_hint(level)),
            None => {}
        }
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
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(app: &mut App, code: KeyCode) {
        app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
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
        // Create > 3 is the copper press, still locked in a new game.
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
            assert!(slots(c).len() <= 4, "{c:?} has more than 4 slots");
        }
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
