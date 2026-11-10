//! Terminal drawing. Plain ASCII on a fixed dark map,
//! buildings as labelled coloured blocks (1x1, 2x2, 3x3), belts as arrows.
//! Every map tile is two terminal cells wide so the grid looks square.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::app::{slots, tree_rows, App, Entry, BOOK_PARTS};
use crate::controls::Action;
use crate::sim::*;

const SIDE_W: u16 = 36;

type Cells = [(char, Style); 2];

// The map always draws on its own dark background, so colours read the same in any terminal theme.
const GROUND: Color = Color::Indexed(234);
const ROCK: Color = Color::Indexed(237);
const ROCK_DOT: Color = Color::Indexed(241);
const AIR: Color = Color::Indexed(232);
const AIR_DOT: Color = Color::Indexed(238);
const LIFT_EDGE: Color = Color::Indexed(117);
const LIFT_BG: Color = Color::Indexed(236);
const BELT: Color = Color::Indexed(242); // dim, so the items riding on belts stand out
const FAST_BELT: Color = Color::Indexed(136);
const BELT_OFF: Color = Color::Indexed(95); // unpowered belts turn a muted red
const RANGE: Color = Color::Indexed(17); // powered area, when shown
const RANGE_NEW: Color = Color::Indexed(22); // area a pole or panel being placed would power
const OFF: Color = Color::Indexed(243);
const INK: Color = Color::Indexed(16);
const PAPER: Color = Color::Indexed(231);
const KEY_ITEM: Color = Color::Indexed(220);

/// Below this size the game switches to the small-pane view on its own.
pub const SMALL_W: u16 = 100;
pub const SMALL_H: u16 = 30;

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let compact = app.compact.unwrap_or(area.width < SMALL_W || area.height < SMALL_H);
    app.compact_now = compact;
    let [top, mid, bottom] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)]).areas(area);
    if compact {
        // Small pane: the map fills everything between a one-line ticker and a one-line status.
        draw_ticker(f, top, app);
        app.map_area = mid;
        update_camera(app, mid);
        draw_map(f.buffer_mut(), mid, app);
        draw_status(f, bottom, app);
    } else {
        let [map, side] = Layout::horizontal([Constraint::Min(10), Constraint::Length(SIDE_W)]).areas(mid);
        draw_top(f, top, app);
        let title = format!(" {} ", floor_label(app.world.floors[app.floor].level));
        let block = Block::bordered().title(title).title_style(Style::new().fg(Color::Cyan));
        let inner = block.inner(map);
        f.render_widget(block, map);
        app.map_area = inner;
        update_camera(app, inner);
        draw_map(f.buffer_mut(), inner, app);
        // A telegram takes the bottom of the sidebar, leaving 12 rows for the build menu and
        // what's under the cursor, and the game keeps running.
        app.mail_area = Rect::default();
        let side = if app.mail.is_some() {
            let room = side.height.saturating_sub(12).max(side.height / 2);
            let h = mail_height(app, side.width).min(room).max(8.min(side.height));
            let [panel, mail] = Layout::vertical([Constraint::Min(1), Constraint::Length(h)]).areas(side);
            app.mail_area = mail;
            draw_mail_in(f, mail, app, false);
            panel
        } else {
            side
        };
        draw_side(f, side, app);
        draw_bottom(f, bottom, app);
    }
    if app.stats {
        draw_stats(f, app.map_area, app);
    }
    if app.tree {
        draw_tree(f, f.area(), app);
    }
    if app.book {
        draw_book(f, f.area(), app);
    }
    if app.help {
        draw_help(f, f.area(), app);
    }
    if app.mail_open && app.mail.is_some() {
        draw_mail(f, f.area(), app);
    }
    if let Some(c) = app.letter {
        draw_letter(f, f.area(), c);
    }
    if let Some(text) = &app.away {
        let r = centered(f.area(), 62, 9);
        f.render_widget(Clear, r);
        let head = Style::new().fg(KEY_ITEM).add_modifier(Modifier::BOLD);
        f.render_widget(
            Paragraph::new(format!("\n {}\n\n press any key", text.replace('\n', "\n ")))
                .block(Block::bordered().title(" While you were away ").title_style(head)),
            r,
        );
    }
}

pub fn floor_label(level: i32) -> String {
    match level {
        0 => "Earth · ground".into(),
        l if l < 0 => format!("Earth · {l} (underground, ore x{})", richness(l)),
        l => format!("Earth · +{l} (upper floor)"),
    }
}

/// "Power 20/12": the grid under the cursor (labelled "regional") when it's on a power
/// source or a powered machine, otherwise every grid together ("global").
fn power_spans(app: &App, label: &str) -> Vec<Span<'static>> {
    let w = &app.world;
    let colour = |r: f64| if r >= 1.0 { Color::Green } else if r >= 0.5 { Color::Yellow } else { Color::Red };
    let dim = Style::new().fg(Color::DarkGray);
    let here = if app.selected.is_none() { w.grid_at(app.floor, app.cursor.0, app.cursor.1) } else { None };
    match here {
        Some(id) => {
            let n = w.grid.nets[id];
            let ratio = if n.demand > 0.0 { n.ratio } else { 1.0 };
            vec![Span::styled(format!("{label}{:.0}/{:.0}", n.supply, n.demand), Style::new().fg(colour(ratio))), Span::styled(" regional", dim)]
        }
        None => {
            let (supply, demand, worst, _, _, unused) = w.power_summary();
            let mut v = vec![Span::styled(format!("{label}{supply:.0}/{demand:.0}"), Style::new().fg(colour(worst))), Span::styled(" global", dim)];
            if unused > 0.0 {
                v.push(Span::styled(format!(" +{unused:.0} unused"), dim));
            }
            v
        }
    }
}

fn draw_top(f: &mut Frame, area: Rect, app: &App) {
    let w = &app.world;
    let (_, _, _, _, unpowered, _) = w.power_summary();
    let secs = w.tick / TPS;
    let bold = Modifier::BOLD;
    let mut spans = vec![
        Span::styled(" TERMINACTORY ", Style::new().fg(Color::Black).bg(Color::Cyan).add_modifier(bold)),
        Span::raw(" "),
        Span::styled(format!(" ${:.0}", w.econ.money), Style::new().fg(Color::Green).add_modifier(bold)),
        Span::raw("   "),
        Span::styled(format!("Research {:.1}", w.econ.research), Style::new().fg(Color::LightBlue).add_modifier(bold)),
        Span::raw("   "),
    ];
    spans.extend(power_spans(app, "Power "));
    spans.extend([
        Span::styled(
            if unpowered > 0 { format!("  {unpowered} unpowered") } else { String::new() },
            Style::new().fg(Color::Indexed(214)),
        ),
        Span::raw("   "),
        Span::styled(format!("{:02}:{:02}", secs / 60, secs % 60), Style::new().fg(Color::DarkGray)),
        Span::raw("   "),
    ]);
    if app.paused {
        spans.push(Span::styled(" PAUSED ", Style::new().fg(Color::Black).bg(Color::Yellow)));
    } else if app.speed > 1 {
        spans.push(Span::styled(format!("{}x speed", app.speed), Style::new().fg(Color::Yellow)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Small-pane top line: money, research, power, idle machines, floor, waiting mail.
fn draw_ticker(f: &mut Frame, area: Rect, app: &App) {
    let w = &app.world;
    let (_, _, _, _, unpowered, _) = w.power_summary();
    let bold = Modifier::BOLD;
    let warn = Style::new().fg(Color::Indexed(214));
    let mut spans = vec![
        Span::styled(format!(" ${:.0}", w.econ.money), Style::new().fg(Color::Green).add_modifier(bold)),
        Span::styled(format!("  R {:.1}", w.econ.research), Style::new().fg(Color::LightBlue).add_modifier(bold)),
    ];
    spans.extend(power_spans(app, "  pwr "));
    let (idle, why) = w.idle_summary();
    if idle > 0 {
        spans.push(Span::styled(format!("  {idle} idle"), warn));
        if let Some(r) = why {
            spans.push(Span::styled(format!(" ({r})"), warn));
        }
    }
    if unpowered > 0 {
        spans.push(Span::styled(format!("  {unpowered} unpowered"), warn));
    }
    let level = w.floors[app.floor].level;
    if level != 0 {
        spans.push(Span::styled(format!("  floor {level:+}"), Style::new().fg(Color::Cyan)));
    }
    let mail = w.mail.len() + app.mail.is_some() as usize;
    if mail > 0 {
        spans.push(Span::styled(format!("  {mail} mail"), Style::new().fg(KEY_ITEM)));
    }
    if app.paused {
        spans.push(Span::styled("  PAUSED", Style::new().fg(Color::Yellow).add_modifier(bold)));
    } else if app.speed > 1 {
        spans.push(Span::styled(format!("  {}x", app.speed), Style::new().fg(Color::Yellow)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(Color::Indexed(236))), area);
}

/// Small-pane bottom line, most useful first: a fresh message, the open build menu,
/// what's under the cursor, or the main keys.
fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    let dim = Style::new().fg(Color::DarkGray);
    let c = &app.controls;
    let w = &app.world;
    // Where you are in the build menu leads the line: "Create › Copper furnace".
    let mut crumb = Vec::new();
    if let Some(cat) = app.menu {
        crumb.push(Span::styled(format!(" {}", cat.name()), Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        if let Some(k) = app.selected {
            crumb.push(Span::styled(" › ", dim));
            crumb.push(Span::styled(k.name(), Style::new().add_modifier(Modifier::BOLD)));
        }
    }
    let line = if app.msg_at.elapsed().as_secs() < 6 {
        let mut spans = crumb;
        if !spans.is_empty() {
            spans.push(Span::styled("  │", dim));
        }
        spans.push(Span::styled(format!(" {}", app.msg), Style::new().fg(Color::Yellow)));
        Line::from(spans)
    } else if let (Some(_), Some(k)) = (app.menu, app.selected) {
        // A part is picked: its price, size and facing, and how to place it.
        let mut spans = crumb;
        let afford = w.progress.part_unlocked(k) && w.econ.money >= k.price(app.lift_size);
        spans.push(Span::styled(format!("  {}", price_label(w, k, app.lift_size)), if afford { Style::new().fg(Color::Green) } else { dim }));
        let (fw, fh) = dims_of(k, app.dir, app.lift_size);
        if fw * fh > 1 {
            spans.push(Span::styled(format!("  {fw}x{fh}"), dim));
        }
        if k.directional() {
            spans.push(Span::raw(format!("  facing {}", app.dir.arrow())));
            spans.push(Span::styled(format!(" ({})", c.key(Action::Rotate)), dim));
        }
        spans.push(Span::styled(format!("  ·  {} place  {} back", c.key(Action::Place), c.key(Action::Back)), dim));
        Line::from(spans)
    } else if let Some(cat) = app.menu {
        // A category is open: its parts by number.
        let mut spans = crumb;
        spans.push(Span::styled(" ›", dim));
        for (n, slot) in slots(cat).iter().enumerate() {
            // Each slot shows the kind picked in it (or its first kind).
            let k = app.selected.filter(|k| slot.contains(k)).unwrap_or(slot[0]);
            let unlocked = w.progress.part_unlocked(k);
            let mut st = if unlocked && w.econ.money >= k.cost() { Style::new() } else { dim };
            if app.selected == Some(k) {
                st = st.add_modifier(Modifier::REVERSED);
            }
            let more = if slot.len() > 1 { "+" } else { "" };
            spans.push(Span::styled(format!(" {} {}{more} {} ", n + 1, k.name(), price_label(w, k, app.lift_size)), st));
        }
        Line::from(spans)
    } else {
        let (x, y) = app.cursor;
        let here = inspect(&app.world, app.floor, x, y);
        if here.first().is_some_and(|h| h != "Empty ground") {
            Line::raw(format!(" {}", here.iter().take(3).cloned().collect::<Vec<_>>().join(" · ")))
        } else {
            Line::styled(
                format!(
                    " {} help · 1-4 build · {} research · {} recipes · {} production · Ctrl+C quit",
                    c.key(Action::Help),
                    c.key(Action::Tree),
                    c.key(Action::Book),
                    c.key(Action::Stats)
                ),
                dim,
            )
        }
    };
    line_with_note(f, area, line, mail_note(app, area.width));
}

/// A waiting telegram as a short note for the right end of the bottom line:
/// "agent finished · n to view" (or just "n ✉" when there's no room).
fn mail_note(app: &App, width: u16) -> Option<String> {
    let m = app.mail.as_ref()?;
    let c = &app.controls;
    let what = match m.event.as_str() {
        "finished" => "agent finished",
        "needs you" => "agent needs you",
        _ => "new telegram",
    };
    let more = match app.world.mail.len() {
        0 => String::new(),
        n => format!(" (+{n})"),
    };
    let full = format!(" {what}{more} · {} to view ", c.key(Action::Mail));
    Some(if full.chars().count() + 16 > width as usize { format!(" {} ✉ ", c.key(Action::Mail)) } else { full })
}

/// Draws `line` in `area`, with the waiting-telegram note (if any) at its right end.
fn line_with_note(f: &mut Frame, area: Rect, line: Line, note: Option<String>) {
    match note {
        Some(note) => {
            let w = note.chars().count() as u16;
            let [left, right] = Layout::horizontal([Constraint::Min(0), Constraint::Length(w)]).areas(area);
            f.render_widget(Paragraph::new(line), left);
            f.render_widget(Paragraph::new(Span::styled(note, Style::new().fg(INK).bg(KEY_ITEM).add_modifier(Modifier::BOLD))), right);
        }
        None => f.render_widget(Paragraph::new(line), area),
    }
}

/// A part's price, its research cost if it still needs researching, or "locked".
fn price_label(w: &World, k: Kind, lift_size: i32) -> String {
    let unlocked = w.progress.part_unlocked(k);
    match k.tech() {
        Some(t) if !w.progress.is_open(t.company()) => "locked".into(),
        Some(t) if !unlocked => format!("{:.0}R", t.cost()),
        _ if !unlocked => "locked".into(),
        _ => format!("${:.0}", k.price(lift_size)),
    }
}

fn update_camera(app: &mut App, area: Rect) {
    let vw = (area.width / 2) as i32;
    let vh = area.height as i32;
    if vw <= 0 || vh <= 0 {
        return;
    }
    if !app.cam_ready {
        app.cam = (app.cursor.0 - vw / 2, app.cursor.1 - vh / 2);
        app.cam_ready = true;
    }
    let mx = 3.min(vw / 3);
    let my = 2.min(vh / 3);
    let (cx, cy) = app.cursor;
    if cx < app.cam.0 + mx {
        app.cam.0 = cx - mx;
    }
    if cx > app.cam.0 + vw - 1 - mx {
        app.cam.0 = cx - vw + 1 + mx;
    }
    if cy < app.cam.1 + my {
        app.cam.1 = cy - my;
    }
    if cy > app.cam.1 + vh - 1 - my {
        app.cam.1 = cy - vh + 1 + my;
    }
    app.cam.0 = app.cam.0.clamp(0, (W - vw).max(0));
    app.cam.1 = app.cam.1.clamp(0, (H - vh).max(0));
}

fn draw_map(buf: &mut Buffer, area: Rect, app: &App) {
    let vw = (area.width / 2) as i32;
    let vh = area.height as i32;
    let (cx, cy) = app.cursor;
    let yellow = Style::new().fg(INK).bg(Color::Indexed(226)).add_modifier(Modifier::BOLD);
    // What the cursor covers: a ghost of the part being placed, or the whole building under it.
    let ghost_dims = |k: Kind| dims_of(k, app.dir, app.lift_size);
    let ghost = app.selected.map(|k| (k, app.world.check_place_sized(app.floor, cx, cy, k, app.dir, app.lift_size).is_ok()));
    let hover = match app.selected {
        None => app.world.tile(app.floor, cx, cy).zip(app.world.anchor_of(app.floor, cx, cy)).map(|(b, a)| (a, b.dims())),
        Some(_) => None,
    };
    let inside = |tx: i32, ty: i32, (ax, ay): (i32, i32), (w, h): (i32, i32)| tx >= ax && tx < ax + w && ty >= ay && ty < ay + h;
    // Power ranges show when asked (v), while placing a power source, or when hovering one.
    let hovering_source = app.world.tile(app.floor, cx, cy).is_some_and(|b| coverage(b.kind, (1, 1), 0, 0).is_some());
    let show_range = app.power_view
        || app.selected.is_some_and(|k| coverage(k, (1, 1), 0, 0).is_some())
        || (app.selected.is_none() && hovering_source);
    // Hovering a source shades only its own grid; v shows every grid.
    let only_grid = if !app.power_view && app.selected.is_none() && hovering_source { app.world.grid_at(app.floor, cx, cy) } else { None };
    let new_range = app.selected.and_then(|k| coverage(k, ghost_dims(k), cx, cy));
    for sy in 0..vh {
        for sx in 0..vw {
            let (tx, ty) = (app.cam.0 + sx, app.cam.1 + sy);
            let mut cells =
                if in_bounds(tx, ty) { tile_glyph(&app.world, app.floor, tx, ty) } else { [(' ', Style::new()); 2] };
            if show_range && in_bounds(tx, ty) && cells[0].1.bg == Some(GROUND) {
                let in_new = new_range.is_some_and(|(x0, y0, x1, y1)| tx >= x0 && tx <= x1 && ty >= y0 && ty <= y1);
                let id = app.world.grid.cover[app.floor][idx(tx, ty)];
                let covered = match only_grid {
                    Some(g) => id == g as i32,
                    None => id >= 0,
                };
                let tint = if in_new { Some(RANGE_NEW) } else if covered { Some(RANGE) } else { None };
                if let Some(t) = tint {
                    cells = cells.map(|(c, s)| (c, s.bg(t)));
                }
            }
            if let Some((kind, ok)) = ghost {
                let dims = ghost_dims(kind);
                if inside(tx, ty, (cx, cy), dims) {
                    let st = if ok { yellow } else { yellow.bg(Color::Indexed(196)) };
                    let chars = if kind == Kind::Elevator {
                        lift_chars(dims.0, app.dir, &[], tx - cx, ty - cy)
                    } else {
                        sprite_chars(kind, app.dir, None, ' ', tx - cx, ty - cy)
                    };
                    cells = chars.map(|c| (c, st));
                }
            } else if let Some((a, dims)) = hover {
                if inside(tx, ty, a, dims) {
                    cells = cells.map(|(c, _)| (c, yellow));
                }
            } else if (tx, ty) == app.cursor {
                cells = cells.map(|(c, _)| (if c == ' ' { '+' } else { c }, yellow));
            }
            for (k, (ch, st)) in cells.iter().enumerate() {
                let pos = (area.x + (sx * 2) as u16 + k as u16, area.y + sy as u16);
                if let Some(c) = buf.cell_mut(pos) {
                    c.set_char(*ch).set_style(*st);
                }
            }
        }
    }
}

fn item_cell(item: Item, bg: Color) -> (char, Style) {
    let (c, fg) = match item {
        Item::CopperOre => ('o', 209),
        Item::Coal => ('*', 252),
        Item::CopperPlate => ('=', 220),
        Item::CopperWire => ('~', 214),
        Item::CopperPipe => ('0', 180),
        Item::CopperCoil => ('@', 208),
        Item::Coke => ('#', 247),
        Item::IronOre => ('o', 111),
        Item::IronPlate => ('=', 153),
        Item::Steel => ('%', 255),
        Item::Waste => (',', 100),
        Item::SteelGirder => ('H', 250),
        Item::Concrete => ('c', 252),
        Item::IronGear => ('g', 153),
        Item::Gearbox => ('G', 110),
        Item::DrillingRig => ('R', 214),
    };
    (c, Style::new().fg(Color::Indexed(fg)).bg(bg).add_modifier(Modifier::BOLD))
}

/// Text colour and background colour of each part.
fn colours(kind: Kind, mode: Option<SellMode>) -> (Color, u8) {
    match kind {
        Kind::Belt => (BELT, 234),
        Kind::Drill => (INK, 250),
        Kind::BasicDrill => (INK, 110),
        Kind::Splitter => (INK, 44),
        Kind::Arm => (PAPER, 133),
        Kind::Furnace => (INK, 196),
        Kind::Press => (INK, 180),
        Kind::Crate => (INK, 178),
        Kind::TradePost => match mode.unwrap_or(SellMode::Money) {
            SellMode::Money => (INK, 35),
            SellMode::Split => (INK, 80),
            SellMode::Research => (INK, 75),
        },
        Kind::Solar => (PAPER, 27),
        Kind::Pole => (Color::Indexed(186), 234),
        Kind::Elevator => (INK, 141),
        Kind::Pipe => (Color::Indexed(244), 234),
        Kind::Pump => (INK, 45),
        Kind::BlastFurnace => (PAPER, 124),
        Kind::Boiler => (PAPER, 130),
        Kind::SteamEngine => (PAPER, 61),
        Kind::FastBelt => (FAST_BELT, 234),
        Kind::Assembler => (INK, 179),
        Kind::Tunnel => (BELT, 237),
        Kind::LongArm => (PAPER, 97),
        Kind::Sorter => (INK, 37),
        Kind::Warehouse => (INK, 136),
    }
}

fn part_style(kind: Kind, on: bool, mode: Option<SellMode>) -> Style {
    let (fg, bg) = colours(kind, mode);
    if kind.is_belt() {
        let on_fg = if kind == Kind::FastBelt { fg } else { BELT };
        return Style::new().fg(if on { on_fg } else { BELT_OFF }).bg(GROUND);
    }
    Style::new().fg(fg).bg(if on { Color::Indexed(bg) } else { OFF }).add_modifier(Modifier::BOLD)
}

/// Small two-cell icon of a part, for the build list.
pub fn kind_cells(kind: Kind, dir: Dir, on: bool) -> Cells {
    let a = dir.arrow();
    let (c0, c1) = match kind {
        Kind::Belt => (a, a),
        Kind::Drill => ('D', 'R'),
        Kind::BasicDrill => ('D', '2'),
        Kind::Splitter => ('S', 'P'),
        Kind::Arm => ('A', a),
        Kind::Furnace => ('F', 'U'),
        Kind::Press => ('P', 'R'),
        Kind::Crate => ('C', a),
        Kind::TradePost => ('$', '$'),
        Kind::Solar => ('S', 'O'),
        Kind::Pole => ('P', 'W'),
        Kind::Elevator => ('L', 'I'),
        Kind::Pipe => ('=', '='),
        Kind::Pump => ('P', 'U'),
        Kind::BlastFurnace => ('B', 'F'),
        Kind::Boiler => ('B', 'O'),
        Kind::SteamEngine => ('E', 'N'),
        Kind::FastBelt => (a, a),
        Kind::Assembler => ('A', 'S'),
        Kind::Tunnel => (a, ']'),
        Kind::LongArm => ('L', a),
        Kind::Sorter => ('S', a),
        Kind::Warehouse => ('W', 'H'),
    };
    let st = part_style(kind, on, None);
    [(c0, st), (c1, st)]
}

/// The full picture of a part: one string per row, two characters per tile, sized to
/// the part's footprint when it faces `dir`.
/// Big buildings show their name and an arrow on the edge their output leaves from;
/// machines also show a letter for their recipe (`tag`).
fn picture(kind: Kind, dir: Dir, mode: Option<SellMode>, tag: char) -> Vec<String> {
    let a = dir.arrow();
    // The name on one row (the last row facing north), an arrow in the middle of the
    // output edge, and the recipe letter on the far side of that row.
    let labelled = |name: &str| -> Vec<String> {
        let (w, h) = kind.dims(dir);
        let n = (w * 2) as usize;
        let name_row = if dir == Dir::N { h - 1 } else { 0 };
        let port_row = match dir {
            Dir::E | Dir::W => h / 2,
            Dir::N => 0,
            Dir::S => h - 1,
        };
        let mut rows = vec![vec![' '; n]; h as usize];
        let len = name.chars().count();
        for (i, c) in name.chars().enumerate() {
            rows[name_row as usize][(n - len) / 2 + i] = c;
        }
        let row = &mut rows[port_row as usize];
        match dir {
            Dir::E => (row[0], row[n - 1]) = (tag, a),
            Dir::W => (row[0], row[n - 1]) = (a, tag),
            Dir::N | Dir::S => (row[0], row[n / 2 - 1], row[n / 2]) = (tag, a, a),
        }
        rows.into_iter().map(|r| r.into_iter().collect()).collect()
    };
    match kind {
        Kind::Belt => vec![format!("{a}{a}")],
        Kind::Splitter => vec!["SP".into()],
        Kind::Arm => vec![format!("A{a}")],
        Kind::Crate => vec![format!("C{a}")],
        Kind::Drill => labelled("DRIL"),
        Kind::BasicDrill => labelled("DRL2"),
        Kind::Furnace => labelled("FURN"),
        // 2x1 facing east or west; a 1-wide column facing north or south.
        Kind::Press => match dir {
            Dir::E => vec![format!("PR{tag}{a}")],
            Dir::W => vec![format!("{a}{tag}PR")],
            Dir::N => vec![format!("{a}{tag}"), "PR".into()],
            Dir::S => vec!["PR".into(), format!("{a}{tag}")],
        },
        Kind::Elevator => labelled("LIFT"),
        Kind::Solar => vec!["SOLR".into(), "    ".into()],
        // A small pylon, not a block.
        Kind::Pole => vec!["/\\".into()],
        Kind::Pipe => vec!["==".into()],
        // 1 wide and 2 tall facing east or west; lying down facing north or south.
        Kind::Pump => match dir {
            Dir::E | Dir::W => vec!["PU".into(), "MP".into()],
            Dir::N | Dir::S => vec!["PUMP".into()],
        },
        Kind::BlastFurnace => labelled("BLST"),
        Kind::Boiler => labelled("BOIL"),
        Kind::SteamEngine => vec!["ENGN".into(), "    ".into()],
        Kind::FastBelt => vec![format!("{a}{a}")],
        Kind::Tunnel => vec![tunnel_chars(dir, false).iter().collect()],
        Kind::LongArm => vec![format!("L{a}")],
        Kind::Sorter => vec![format!("S{a}")],
        Kind::Warehouse => labelled("WARE"),
        Kind::Assembler => {
            let port = match dir {
                Dir::E => format!("     {a}"),
                Dir::W => format!("{a}     "),
                Dir::N | Dir::S => format!("  {a}{a}  "),
            };
            let mid = format!("{:^6}", tag);
            if dir == Dir::N {
                vec![port, mid, "ASSMBL".into()]
            } else {
                vec!["ASSMBL".into(), mid, port]
            }
        }
        Kind::TradePost => {
            let m = match mode.unwrap_or(SellMode::Money) {
                SellMode::Money => "$",
                SellMode::Split => "$ R",
                SellMode::Research => "R",
            };
            vec!["TRADE ".into(), format!("{:^6}", m), "      ".into()]
        }
    }
}

fn sprite_chars(kind: Kind, dir: Dir, mode: Option<SellMode>, tag: char, lx: i32, ly: i32) -> [char; 2] {
    let rows = picture(kind, dir, mode, tag);
    let row: Vec<char> = rows[ly as usize].chars().collect();
    [row[(lx * 2) as usize], row[(lx * 2 + 1) as usize]]
}

pub fn tile_glyph(w: &World, f: usize, x: i32, y: i32) -> Cells {
    let fl = &w.floors[f];
    let i = idx(x, y);
    if let Some(a) = fl.owner[i] {
        if let Some(b) = &fl.tiles[a as usize] {
            let (ax, ay) = pos_of(a as usize);
            let powered = w.building_ratio(f, a as usize) > 0.0;
            if let St::Belt { items } = &b.st {
                return belt_cell(b.kind, belt_shape(w, f, x, y, b.dir), b.dir, items, powered);
            }
            if b.kind == Kind::Pipe {
                return pipe_cell(w, f, x, y);
            }
            if b.kind == Kind::Elevator {
                return lift_cell(w, f, b, a as usize, x - ax, y - ay);
            }
            // Idle buildings flash a word saying why, every other second.
            let reason = w.idle_reason(f, a as usize).filter(|_| (w.tick / TPS) % 2 == 1);
            return building_cell(b, x - ax, y - ay, powered, reason);
        }
    }
    let blank = |bg: Color| [(' ', Style::new().bg(bg)); 2];
    match () {
        _ if fl.dug[i] => blank(GROUND),
        // Open air above the ground: decking it over costs a little.
        _ if fl.level > 0 => {
            let st = Style::new().fg(AIR_DOT).bg(AIR);
            [('·', st), (' ', st)]
        }
        // Solid rock underground: dotted, so it reads differently from open floor.
        _ => {
            let st = Style::new().fg(ROCK_DOT).bg(ROCK);
            [('.', st), (' ', st)]
        }
    }
}

/// A pipe joins up with neighbouring pipes and machines that use them; its colour shows
/// what its network holds (blue water, white steam, grey empty).
fn pipe_cell(w: &World, f: usize, x: i32, y: i32) -> Cells {
    let joins = |d: Dir| {
        let (dx, dy) = d.delta();
        w.tile(f, x + dx, y + dy).is_some_and(|b| b.kind.uses_pipes())
    };
    let h = joins(Dir::E) || joins(Dir::W);
    let v = joins(Dir::N) || joins(Dir::S);
    let (a, b) = match (h, v) {
        (true, false) => ('=', '='),
        (false, true) => ('|', '|'),
        (true, true) => ('+', '+'),
        (false, false) => ('[', ']'),
    };
    let fluid = w
        .fluids
        .pipe_pool
        .get(f)
        .map(|p| p[idx(x, y)])
        .filter(|&p| p >= 0)
        .and_then(|p| w.fluids.pools.get(p as usize))
        .and_then(|p| p.fluid);
    let fg = match fluid {
        Some(Fluid::Water) => 45,
        Some(Fluid::Steam) => 255,
        None => 244,
    };
    let st = Style::new().fg(Color::Indexed(fg)).bg(GROUND).add_modifier(Modifier::BOLD);
    [(a, st), (b, st)]
}

/// How a belt tile joins its neighbours.
#[derive(Clone, Copy, PartialEq, Debug)]
enum BeltShape {
    Straight,
    /// A corner: items arrive travelling in this direction.
    Turn(Dir),
    /// Belts join from the side: which of the two cells shows the join (`+`).
    Merge(bool, bool),
}

/// A belt is a corner when exactly one belt feeds it from the side and none from behind,
/// and a merge when side belts join a belt that is also fed from behind (or from both sides).
fn belt_shape(w: &World, f: usize, x: i32, y: i32, dir: Dir) -> BeltShape {
    let feeds_from = |side: Dir| {
        let (dx, dy) = side.delta();
        matches!(w.tile(f, x + dx, y + dy), Some(n) if n.kind.is_belt() && n.dir == side.opposite())
    };
    let rear = feeds_from(dir.opposite());
    let sides: Vec<Dir> = [dir.cw(), dir.cw().opposite()].into_iter().filter(|s| feeds_from(*s)).collect();
    match (rear, sides.as_slice()) {
        (_, []) => BeltShape::Straight,
        (false, [side]) => BeltShape::Turn(side.opposite()),
        _ => match dir {
            // Going up or down: the join shows on the side the belt comes in from.
            Dir::N | Dir::S => BeltShape::Merge(sides.contains(&Dir::W), sides.contains(&Dir::E)),
            // Going sideways: the join shows on the tile's back half.
            Dir::E => BeltShape::Merge(true, false),
            Dir::W => BeltShape::Merge(false, true),
        },
    }
}

/// A belt tile: arrows when straight; at a corner, a bend (/ or \) plus the new direction;
/// where belts join, a `+` on the side they join from.
fn belt_cell(kind: Kind, shape: BeltShape, dir: Dir, items: &[BeltItem], powered: bool) -> Cells {
    let st = part_style(kind, powered, None);
    let a = dir.arrow();
    let mut cells = match shape {
        BeltShape::Straight => [(a, st), (a, st)],
        BeltShape::Turn(came) => {
            // The bend joins the edge items come in from to the edge they leave by.
            let bend = match (came, dir) {
                (Dir::E, Dir::N) | (Dir::W, Dir::S) | (Dir::N, Dir::E) | (Dir::S, Dir::W) => '/',
                _ => '\\',
            };
            match dir {
                Dir::E => [(bend, st), (a, st)],
                Dir::W => [(a, st), (bend, st)],
                // Going up or down: the bend sits on the side items came from.
                _ if came == Dir::E => [(bend, st), (a, st)],
                _ => [(a, st), (bend, st)],
            }
        }
        BeltShape::Merge(left, right) => {
            [(if left { '+' } else { a }, st), (if right { '+' } else { a }, st)]
        }
    };
    if let Some(front) = items.first() {
        // An item shows in place of a bend or join, or the first arrow on a straight belt.
        let k = match shape {
            BeltShape::Straight => 0,
            _ if cells[0].0 != a => 0,
            _ => 1,
        };
        cells[k] = item_cell(front.item, GROUND);
    }
    cells
}

/// One tile (two cells) of a building; (lx, ly) is the tile's place inside it.
fn building_cell(b: &Building, lx: i32, ly: i32, powered: bool, reason: Option<&str>) -> Cells {
    let (on, mode) = match &b.st {
        St::Belt { items } => return belt_cell(b.kind, BeltShape::Straight, b.dir, items, powered),
        St::Arm { hold: Some(it), .. } => {
            let st = part_style(Kind::Arm, powered, None);
            return [item_cell(*it, st.bg.unwrap_or(OFF)), (b.dir.arrow(), st)];
        }
        St::Arm { .. } | St::Splitter { .. } => (powered, None),
        St::Tunnel { exit, items } => {
            let st = part_style(b.kind, *exit || !items.is_empty(), None);
            let c = tunnel_chars(b.dir, *exit);
            return [(c[0], st), (c[1], st)];
        }
        St::Drill { mines, fuel, out, .. } => {
            (mines.is_some() && (*fuel > 0 || *mines == Some(Item::Coal)) && out.len() < DRILL_OUT_CAP, None)
        }
        St::Crafter { busy, .. } => (*busy, None),
        St::Trade { mode, .. } => (true, Some(*mode)),
        St::Crate { .. } | St::Solar | St::Pole | St::Elevator { .. } | St::Pipe => (true, None),
        St::Pump { busy } | St::Boiler { busy, .. } => (*busy, None),
        St::Engine { steam, .. } => (*steam > 0.0, None),
    };
    let st = part_style(b.kind, on, mode);
    let tag = match &b.st {
        // Drills show what they're set to mine: Copper, coal (*), Iron.
        St::Drill { mines: Some(m), .. } => match m {
            Item::Coal => '*',
            Item::IronOre => 'I',
            _ => 'C',
        },
        _ => b.recipe().map_or(' ', |r| r.tag),
    };
    let mut c = sprite_chars(b.kind, b.dir, mode, tag, lx, ly);
    // The name row shows why it's idle instead (big buildings only).
    let (bw, bh) = b.dims();
    let name_row = if b.dir == Dir::N && b.kind.directional() { bh - 1 } else { 0 };
    if let Some(word) = reason.filter(|_| bw >= 2 && ly == name_row) {
        let text: Vec<char> = format!("{:^w$}", word, w = (bw * 2) as usize).chars().collect();
        c = [text[(lx * 2) as usize], text[(lx * 2 + 1) as usize]];
    }
    // A darker left edge keeps neighbouring buildings visibly apart.
    let edge = if lx == 0 { st.bg(darker(st.bg.unwrap_or(OFF))) } else { st };
    [(c[0], edge), (c[1], st)]
}

/// A tunnel end: the arrow, and a mouth where the belt goes under (entrance) or comes up
/// (exit). Facing east: ">]" goes in, "[>" comes out.
fn tunnel_chars(dir: Dir, exit: bool) -> [char; 2] {
    let a = dir.arrow();
    let back = matches!(dir, Dir::W);
    match (exit, back) {
        (false, false) => [a, ']'],
        (false, true) => ['[', a],
        (true, false) => ['[', a],
        (true, true) => [a, ']'],
    }
}

/// The characters of an elevator box of side `s` at tile (lx, ly): a box drawn with
/// line characters and a mark on the door side. `lines` = [title, door mode, car status]:
/// the title sits in the top border, the mode in the bottom border, the status inside.
fn lift_chars(s: i32, dir: Dir, lines: &[String], lx: i32, ly: i32) -> [char; 2] {
    let w = (s * 2) as usize;
    let edge = |l: char, r: char, text: Option<&String>| -> Vec<char> {
        let label = text.map_or(String::new(), |t| format!(" {t} "));
        let fill = (w - 2).saturating_sub(label.chars().count());
        let left = fill / 2;
        format!("{l}{}{label}{}{r}", "─".repeat(left), "─".repeat(fill - left)).chars().collect()
    };
    let mut row: Vec<char> = if ly == 0 {
        edge('┌', '┐', if s >= 3 { lines.first() } else { None })
    } else if ly == s - 1 {
        edge('└', '┘', if s >= 3 { lines.get(1) } else { None })
    } else {
        // The status sits on the middle row inside.
        let text = if ly == (s - 1) / 2 { lines.get(2).map_or("", |t| t.as_str()) } else { "" };
        let inner: String = format!("{:^n$}", text, n = w - 2).chars().take(w - 2).collect();
        format!("│{inner}│").chars().collect()
    };
    // Small boxes: the text sits in the borders.
    if s == 2 {
        if let Some(t) = lines.get(ly as usize) {
            let t: Vec<char> = format!("{:^2}", t).chars().take(2).collect();
            row[1] = t[0];
            row[2] = t[1];
        }
    }
    // The door side (where it unloads) gets an arrow in the middle of that edge.
    let mid = (s / 2) as usize;
    match dir {
        Dir::E if ly as usize == mid => row[w - 1] = '▶',
        Dir::W if ly as usize == mid => row[0] = '◀',
        Dir::N if ly == 0 => row[w - 1] = '▲',
        Dir::S if ly == s - 1 => row[w - 1] = '▼',
        _ => {}
    }
    [row[(lx * 2) as usize], row[(lx * 2 + 1) as usize]]
}

/// An elevator door: its box, what this door does, and where the car is.
fn lift_cell(w: &World, f: usize, b: &Building, anchor: usize, lx: i32, ly: i32) -> Cells {
    let s = b.dims().0;
    let mode = match &b.st {
        St::Elevator { mode, .. } => *mode,
        _ => DoorMode::Pass,
    };
    let car = w.lift(anchor);
    let here = car.is_some_and(|c| c.at == f && !matches!(c.phase, LiftPhase::Moving | LiftPhase::Closing));
    // Status choices from long to short: the first that fits inside the box is shown.
    let word = mode.name().to_uppercase();
    let status: Vec<String> = match car {
        Some(c) if here && c.phase == LiftPhase::Opening => vec!["opening".into(), "open".into()],
        Some(c) if here => vec![format!("{}/{}", c.load.len(), c.cap()), format!("{}%", c.load.len() * 100 / c.cap().max(1))],
        Some(c) if c.phase == LiftPhase::Moving => {
            let a = if w.floors[c.target].level > w.floors[c.at].level { "▲" } else { "▼" };
            vec![format!("{a} moving"), format!("{a} go"), a.into()]
        }
        Some(c) if c.phase == LiftPhase::Closing && c.at == f => vec!["closing".into(), "shut".into()],
        _ => vec!["car away".into(), "away".into(), "··".into()],
    };
    let inner = (s * 2 - 2) as usize;
    let status = status.iter().find(|t| t.chars().count() <= inner).cloned().unwrap_or_default();
    let lines: Vec<String> = if s == 2 {
        // Two characters each: door mode, then the load or where the car is.
        let m = match mode {
            DoorMode::Load => "LD",
            DoorMode::Unload => "UL",
            DoorMode::Pass => "--",
        };
        let st = match car {
            Some(c) if here => format!("{:>2}", (c.load.len() * 99 / c.cap().max(1)).min(99)),
            Some(c) if c.phase == LiftPhase::Moving => if w.floors[c.target].level > w.floors[c.at].level { "▲▲".into() } else { "▼▼".into() },
            _ => "··".into(),
        };
        vec![m.into(), st]
    } else {
        vec!["LIFT".into(), word, status]
    };
    let chars = lift_chars(s, b.dir, &lines, lx, ly);
    let edge = Style::new().fg(if here { LIFT_EDGE } else { Color::Indexed(66) }).bg(LIFT_BG).add_modifier(Modifier::BOLD);
    let text = Style::new().fg(match mode {
        DoorMode::Load => Color::Indexed(120),
        DoorMode::Unload => Color::Indexed(222),
        DoorMode::Pass => Color::Indexed(244),
    }).bg(LIFT_BG).add_modifier(Modifier::BOLD);
    let pick = |c: char| if "┌┐└┘─│▶◀▲▼".contains(c) && !(s == 2 && "▲▼".contains(c) && ly == 1) { edge } else { text };
    [(chars[0], pick(chars[0])), (chars[1], pick(chars[1]))]
}

/// A darker shade of a 256-colour palette entry.
fn darker(c: Color) -> Color {
    match c {
        Color::Indexed(i @ 16..=231) => {
            let n = i - 16;
            let (r, g, b) = (n / 36, (n / 6) % 6, n % 6);
            Color::Indexed(16 + 36 * r.saturating_sub(1) + 6 * g.saturating_sub(1) + b.saturating_sub(1))
        }
        Color::Indexed(i @ 232..=255) => Color::Indexed(i.saturating_sub(5).max(232)),
        other => other,
    }
}

fn swatch(cells: Cells) -> Vec<Span<'static>> {
    cells.iter().map(|(c, s)| Span::styled(c.to_string(), *s)).collect()
}

fn draw_side(f: &mut Frame, area: Rect, app: &App) {
    let head = Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan);
    let dim = Style::new().fg(Color::DarkGray);
    let w = &app.world;
    let mut lines: Vec<Line> = Vec::new();
    match app.menu {
        None => {
            lines.push(Line::styled("BUILD", head));
            for (n, cat) in Category::ALL.iter().enumerate() {
                lines.push(Line::from(vec![
                    Span::styled(format!(" {} ", n + 1), Style::new().add_modifier(Modifier::BOLD)),
                    Span::raw(cat.name()),
                ]));
            }
        }
        Some(cat) => {
            lines.push(Line::from(vec![
                Span::styled(format!("BUILD > {}", cat.name().to_uppercase()), head),
                Span::styled(format!("  {} back", app.controls.key(Action::Back)), dim),
            ]));
            let mut many = false;
            for (n, slot) in slots(cat).iter().enumerate() {
                many |= slot.len() > 1;
                for (v, k) in slot.iter().enumerate() {
                    let unlocked = w.progress.part_unlocked(*k);
                    let afford = w.econ.money >= k.cost();
                    let mut st = if unlocked && afford { Style::new() } else { dim };
                    if app.selected == Some(*k) {
                        st = st.add_modifier(Modifier::REVERSED);
                    }
                    let name = format!("{}{}", k.name(), if k.preview() { "*" } else { "" });
                    let (kw, kh) = dims_of(*k, Dir::E, app.lift_size);
                    let size = if kw * kh > 1 { format!("{kw}x{kh}") } else { String::new() };
                    let price = price_label(w, *k, app.lift_size);
                    // The slot's number goes on its first kind; the others sit under it.
                    let tag = if v == 0 { format!(" {} ", n + 1) } else { "   ".to_string() };
                    let mut spans = vec![Span::styled(tag, st.add_modifier(Modifier::BOLD))];
                    spans.extend(swatch(kind_cells(*k, Dir::E, unlocked)));
                    spans.push(Span::styled(format!(" {:<17}{:>4}{:>7}", name, size, price), st));
                    lines.push(Line::from(spans));
                }
            }
            if many {
                lines.push(Line::styled(" same number again: next kind", dim));
            }
        }
    }
    if app.selected == Some(Kind::Elevator) {
        let s = app.lift_size;
        lines.push(Line::styled(
            format!(" {s}x{s}, holds {}  ({}/{} size)", 10 * s * s, app.controls.key(Action::Bigger), app.controls.key(Action::Smaller)),
            dim,
        ));
    }
    if let Some(k) = app.selected {
        if k.directional() {
            lines.push(Line::styled(format!(" facing {}  ({} rotates)", app.dir.arrow(), app.controls.key(Action::Rotate)), dim));
        }
    }

    lines.push(Line::raw(""));
    let (x, y) = app.cursor;
    lines.push(Line::styled("HERE", head));
    for s in inspect(w, app.floor, x, y) {
        lines.push(Line::raw(format!(" {}", s)));
    }
    let p = Paragraph::new(lines).block(Block::bordered().title(" Factory ")).wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn inspect(w: &World, f: usize, x: i32, y: i32) -> Vec<String> {
    let fl = &w.floors[f];
    let i = idx(x, y);
    let mut v = Vec::new();
    let Some(b) = w.tile(f, x, y) else {
        v.push(
            match () {
                _ if fl.dug[i] => "Empty ground",
                _ if fl.level > 0 => "Open air (decking costs a little)",
                _ => "Solid rock (digging costs a little)",
            }
            .into(),
        );
        return v;
    };
    v.push(b.kind.name().to_string());
    if let Some(c) = b.kind.company() {
        v.push(format!("by {}", c.name()));
    }
    if b.kind.directional() && b.kind != Kind::Belt && b.kind != Kind::Arm {
        v.push(format!("output goes out the {} side", b.dir.arrow()));
    }
    let (ax, ay) = w.anchor_of(f, x, y).unwrap_or((x, y));
    if b.kind.power_draw() > 0.0 {
        match w.building_net(f, idx(ax, ay)) {
            Some(n) => v.push(format!("power {:.0}% (grid {:.0}/{:.0})", n.ratio * 100.0, n.supply, n.demand)),
            None => v.push("NO POWER: out of range (v)".into()),
        }
    }
    if coverage(b.kind, (1, 1), 0, 0).is_some() {
        if let Some((n, sources)) = w.source_net(f, idx(ax, ay)) {
            let with = if sources == 1 { "on its own".to_string() } else { format!("{sources} sources") };
            if n.demand <= 0.0 {
                v.push(format!("grid: {:.0} supply, nothing to power ({with})", n.supply));
            } else {
                v.push(format!("grid: {:.0} supply / {:.0} demand ({with})", n.supply, n.demand));
            }
        }
    }
    match &b.st {
        St::Drill { mines, fuel, coal, out, .. } => {
            v.push(format!("mining {}", mines.map_or("nothing", |it| it.name())));
            v.push(format!("fuel {}   coal {}/{}", if *fuel > 0 { "burning" } else { "empty" }, coal, DRILL_COAL_CAP));
            v.push(format!("output {}/{}", out.len(), DRILL_OUT_CAP));
            if *fuel == 0 && *coal == 0 && *mines != Some(Item::Coal) {
                v.push("needs coal: belt some into it".into());
            }
        }
        St::Belt { items } => {
            if items.is_empty() {
                v.push(format!("facing {}, empty", b.dir.arrow()));
            } else {
                v.push(format!("facing {}, carrying:", b.dir.arrow()));
                v.extend(item_counts(items.iter().map(|bi| bi.item)));
            }
        }
        St::Splitter { buf, filter, .. } => {
            v.push(format!("holding {}/2", buf.len()));
            if b.kind == Kind::Sorter {
                match filter {
                    Some(i) => v.push(format!("{} out the front {}, the rest out the sides", i.name(), b.dir.arrow())),
                    None => v.push("no filter yet: shares out every side".into()),
                }
                v.push("f picks the item".into());
            }
        }
        St::Tunnel { exit: true, .. } => v.push(format!("exit: items come up here and leave {}", b.dir.arrow())),
        St::Tunnel { exit: false, items } => match w.tunnel_exit(f, x, y) {
            Some(k) => {
                v.push(format!("goes under {} tile{} to its exit", k - 1, if k == 2 { "" } else { "s" }));
                v.push(format!("{} item{} under the ground", items.len(), if items.len() == 1 { "" } else { "s" }));
            }
            None => v.push(format!("entrance: put its exit within {} {}", TUNNEL_REACH + 1, b.dir.arrow())),
        },
        St::Arm { filter, hold, .. } => {
            v.push(format!("facing {}   filter: {}", b.dir.arrow(), filter.map_or("anything", |i| i.name())));
            v.push(format!("holding {}", hold.map_or("nothing".to_string(), |i| format!("{} {}", item_cell(i, GROUND).0, i.name()))));
            v.push("f changes the filter".into());
        }
        St::Crafter { stock, fuel, out, busy, .. } => {
            if let Some(r) = b.recipe() {
                v.push(format!("making {}  (f changes)", r.name));
                let needs: Vec<String> =
                    r.inputs.iter().map(|(it, n)| format!("{} {}/{}", it.name(), stock[it.idx()], n)).collect();
                v.push(format!("has {}", needs.join(", ")));
                if r.outputs.len() > 1 {
                    let outs: Vec<&str> = r.outputs.iter().map(|o| o.0.name()).collect();
                    v.push(format!("gives {} (all out the front)", outs.join(" + ")));
                }
                if let Some(fuel_item) = r.fuel {
                    let lit = if *fuel > 0 { "burning" } else { "no fire" };
                    v.push(format!("{} {}   {}", fuel_item.name(), stock[fuel_item.idx()], lit));
                }
                v.push(format!("finished {}/{}   {}", out.len(), CRAFTER_OUT_CAP, if *busy { "working" } else { "waiting" }));
            }
        }
        St::Crate { items } => {
            let cap = CRATE_CAP * if b.kind == Kind::Warehouse { WAREHOUSE_CRATES } else { 1 };
            v.push(format!("{}/{} items", items.len(), cap));
            v.extend(item_counts(items.iter().copied()));
        }
        St::Trade { mode, money, research, count, .. } => {
            v.push(format!("mode: {}  (f changes)", mode.label()));
            let sign = if *money < 0.0 { "-" } else { "" };
            v.push(format!("sold {}   {sign}${:.0}   research {:.1}", count, money.abs(), research));
            match w.econ.waste_refusal(*mode) {
                Some(why) => v.push(format!("Won't take waste: {why}")),
                None => v.push(format!("takes waste: costs {} each", money_text(-Item::Waste.price()))),
            }
            v.push("research for the next one sold:".into());
            for it in Item::ALL {
                if it == Item::Waste {
                    continue;
                }
                if w.econ.made[it.idx()] > 0 || matches!(it, Item::CopperOre | Item::Coal | Item::CopperPlate) {
                    v.push(format!("  {:<13}{:>6.2}", it.name(), w.econ.next_research(it)));
                }
            }
        }
        St::Solar => v.push(format!("+{:.0} power, reaches {} tiles around it", SOLAR_OUTPUT, SOLAR_REACH)),
        St::Pole => v.push(format!("carries power {} tiles around it", POLE_REACH)),
        St::Elevator { inbox, outbox, mode } => {
            let s = b.dims().0;
            v.push(format!("{s}x{s}, holds {}   door here: {}  (f changes)", LIFT_PER_TILE * (s * s) as usize, mode.name()));
            v.push(format!("waiting to load {}   waiting to leave {}", inbox.len(), outbox.len()));
            if let Some(car) = w.anchor_of(f, x, y).and_then(|(ax, ay)| w.lift(idx(ax, ay))) {
                let where_ = match car.phase {
                    LiftPhase::Moving => format!("moving to floor {}", w.floors[car.target].level),
                    p => format!("{:?} at floor {}", p, w.floors[car.at].level).to_lowercase(),
                };
                v.push(format!("car: {}, {}/{} aboard", where_, car.load.len(), car.cap()));
                v.push("leaves only when full".into());
            }
        }
        St::Pipe => {
            let pool = w.fluids.pipe_pool[f][idx(x, y)];
            if let Some(p) = w.fluids.pools.get(pool.max(0) as usize).filter(|_| pool >= 0) {
                let what = p.fluid.map_or("empty", |fl| fl.name());
                v.push(format!("network of {} pipes: {}", p.pipes, what));
                v.push(format!("holds {:.0} / {:.0}", p.amount, p.cap));
                v.push("one fluid per network".into());
            }
        }
        St::Pump { busy } => {
            v.push(if *busy { "pumping water into its pipes".into() } else { "idle: needs power and a pipe with room".into() });
        }
        St::Boiler { coal, fuel, busy } => {
            v.push(format!("coal {}/{}   {}", coal, BOILER_COAL_CAP, if *fuel > 0 { "burning" } else { "no fire" }));
            v.push(format!("water in from pipes, steam out the {} side", b.dir.arrow()));
            v.push(if *busy { "making steam".into() } else { "waiting: needs coal, water and room for steam".into() });
        }
        St::Engine { steam, output } => {
            v.push(format!("steam {:.1}/{:.0}", steam, ENGINE_BUF));
            v.push(format!("giving {:.0} of {:.0} power", output, ENGINE_POWER));
            v.push(format!("powers parts {} tiles around it", ENGINE_REACH));
        }
    }
    let (waste, fee) = w.removal_fee(f, x, y);
    if waste > 0 {
        v.push(format!("Removing costs {} ({waste} waste)", money_text(fee)));
    }
    v
}

/// One line per kind of item: its belt glyph, name and how many.
fn item_counts(items: impl Iterator<Item = Item>) -> Vec<String> {
    let mut n = [0usize; ITEMS];
    for it in items {
        n[it.idx()] += 1;
    }
    Item::ALL.iter().filter(|it| n[it.idx()] > 0).map(|it| format!("  {} {} x{}", item_cell(*it, GROUND).0, it.name(), n[it.idx()])).collect()
}

fn draw_bottom(f: &mut Frame, area: Rect, app: &App) {
    let fresh = app.msg_at.elapsed().as_secs() < 6;
    let line = if fresh {
        Line::styled(format!(" {}", app.msg), Style::new().fg(Color::Yellow))
    } else {
        {
            let c = &app.controls;
            Line::styled(
                format!(
                    " {} help   1-4 build   {} research   {} recipes   {} production   {} place   {} remove   Ctrl+C quit",
                    c.key(Action::Help),
                    c.key(Action::Tree),
                    c.key(Action::Book),
                    c.key(Action::Stats),
                    c.key(Action::Place),
                    c.key(Action::Remove)
                ),
                Style::new().fg(Color::DarkGray),
            )
        }
    };
    // The tree and the book cover the sidebar, so a waiting telegram shows down here meanwhile.
    let note = if app.tree || app.book { mail_note(app, area.width) } else { None };
    line_with_note(f, area, line, note);
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h)
}

/// "$1,234" or "-$56".
fn signed_money(v: f64) -> String {
    if v < -0.5 { format!("-${}", grouped((-v).round())) } else { format!("${}", grouped(v.round())) }
}

/// A per-minute amount: "·" for nothing, one decimal below 10, whole above.
fn rate(v: f64) -> String {
    if v < 0.05 {
        "·".into()
    } else if v < 10.0 {
        format!("{v:.1}")
    } else {
        grouped(v.round())
    }
}

/// The production panel: what each item did over the last minute. It sits over the map
/// and the game keeps running underneath.
fn draw_stats(f: &mut Frame, area: Rect, app: &mut App) {
    let w = &app.world;
    let head = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let dim = Style::new().fg(Color::DarkGray);
    f.render_widget(Clear, area);
    let (mut rows, secs) = w.flow();
    let title = if secs >= 60 || secs == 0 { " Production · last minute ".to_string() } else { format!(" Production · last {secs} s ") };
    // No border of its own: it sits inside the map's.
    let block = Block::new().title(title).title_style(head);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width < 10 || inner.height < 3 {
        return;
    }
    let width = inner.width.saturating_sub(2) as usize;
    let money: f64 = rows.iter().map(|r| r.money).sum();
    let research: f64 = rows.iter().map(|r| r.research).sum();
    let mut lines: Vec<Line> = vec![Line::from(vec![
        Span::styled(format!("{}/min", signed_money(money)), Style::new().fg(if money < 0.0 { Color::Red } else { Color::Green }).add_modifier(Modifier::BOLD)),
        Span::styled("  ·  ", dim),
        Span::styled(format!("{} research/min", rate(research)), Style::new().fg(Color::LightBlue).add_modifier(Modifier::BOLD)),
    ])];
    lines.push(Line::raw(""));
    if rows.is_empty() {
        lines.push(Line::styled("Nothing made or sold in the last minute yet.", dim));
    } else {
        // Columns drop off from the right as the window narrows.
        let cols: &[(&str, usize)] = if width >= 76 {
            &[("made", 7), ("used", 7), ("sold", 7), ("$/min", 8), ("research", 10), ("next sale", 11)]
        } else if width >= 56 {
            &[("made", 7), ("used", 7), ("sold", 7), ("research", 10)]
        } else {
            &[("made", 6), ("sold", 6), ("research", 9)]
        };
        let name_w = width.saturating_sub(cols.iter().map(|c| c.1).sum::<usize>() + 3).clamp(8, 18);
        let mut header = vec![Span::styled(format!("   {:<name_w$}", "per minute"), dim)];
        header.extend(cols.iter().map(|(n, cw)| Span::styled(format!("{n:>cw$}"), dim)));
        lines.push(Line::from(header));
        rows.sort_by(|a, b| b.research.total_cmp(&a.research).then(b.made.total_cmp(&a.made)));
        for r in &rows {
            let (ch, st) = item_cell(r.item, GROUND);
            let name: String = r.item.name().chars().take(name_w).collect();
            let mut spans = vec![Span::styled(format!("{ch}"), st), Span::raw(format!("  {name:<name_w$}"))];
            for (n, cw) in cols {
                let (text, style) = match *n {
                    "made" => (rate(r.made), Style::new()),
                    "used" => (rate(r.used), Style::new()),
                    "sold" => (rate(r.sold), Style::new()),
                    "$/min" => match r.money {
                        m if m > 0.0 => (format!("${}", rate(m)), Style::new().fg(Color::Green)),
                        m if m < -0.05 => (format!("-${}", rate(-m)), Style::new().fg(Color::Red)),
                        _ => ("·".into(), Style::new()),
                    },
                    "research" => (rate(r.research), Style::new().fg(Color::LightBlue)),
                    _ => {
                        // What the next sale for research would bring: it fades the more you sell.
                        if r.item == Item::Waste {
                            spans.push(Span::styled(format!("{:>cw$}", "none"), dim));
                            continue;
                        }
                        let next = w.econ.next_research(r.item);
                        let worth = r.item.curve().worth;
                        let st = if next >= worth * 0.5 { Style::new().fg(Color::Green) } else if next >= worth * 0.15 { Style::new().fg(Color::Yellow) } else { dim };
                        (format!("{} RP", num(next)), st)
                    }
                };
                spans.push(Span::styled(format!("{text:>cw$}"), style));
            }
            lines.push(Line::from(spans));
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled("Research per sale fades the more you sell of an item, so new items pay best.", dim));
        if rows.iter().any(|r| r.item == Item::Waste) {
            lines.push(Line::styled("Waste costs money to sell (shown negative) and earns no research.", dim));
        }
    }
    let footer = format!("↑↓ scroll · {} or Esc close · the game keeps running", app.controls.key(Action::Stats));
    let body_h = inner.height.saturating_sub(1);
    let max_scroll = (lines.len() as u16).saturating_sub(body_h);
    app.stats_scroll = app.stats_scroll.min(max_scroll);
    let body = Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), body_h);
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((app.stats_scroll, 0)), body);
    let foot = Rect::new(inner.x + 1, inner.y + inner.height - 1, inner.width.saturating_sub(2), 1);
    f.render_widget(Paragraph::new(Span::styled(footer, dim)), foot);
}

/// A whole number with thousands marks: 17,687.
fn grouped(v: f64) -> String {
    let n = v.max(0.0).floor() as u64;
    let s = n.to_string();
    let mut out = String::new();
    for (k, ch) in s.chars().enumerate() {
        if k > 0 && (s.len() - k) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// One row on the research screen: a mark, the name, and what it costs or needs.
fn tree_row_spans(w: &World, t: Tech, name_w: usize) -> Vec<Span<'static>> {
    let rp = w.econ.research;
    let dim = Style::new().fg(Color::DarkGray);
    let ready = Style::new().fg(Color::LightBlue).add_modifier(Modifier::BOLD);
    let saving = Style::new().fg(Color::Blue);
    let done = Style::new().fg(Color::Green);
    let (mark, ns, status) = if w.progress.has(t) {
        ("✓", done, Span::styled("done", dim))
    } else if !w.progress.is_open(t.company()) {
        ("·", dim, Span::styled(format!("make {} first", t.requires().map_or("its key item", |i| i.name())), dim))
    } else if rp + 1e-9 >= t.cost() {
        ("●", Style::new().add_modifier(Modifier::BOLD), Span::styled(format!("{} RP", grouped(t.cost())), ready))
    } else {
        ("○", Style::new(), Span::styled(format!("{} RP", grouped(t.cost())), saving))
    };
    let mark_st = if mark == "●" { ready } else { ns };
    let name = format!("{:<name_w$}", t.name().chars().take(name_w).collect::<String>());
    vec![Span::styled(format!("{mark} "), mark_st), Span::styled(name, ns), status]
}

/// What the picked row does, in full, for under the list.
fn tree_details(w: &World, t: Tech) -> Vec<(String, Style)> {
    let head = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let note = Style::new().fg(Color::LightBlue);
    let rp = grouped(w.econ.research);
    let (item, n) = t.milestone();
    let made = w.econ.made[item.idx()].min(n);
    let status = if w.progress.has(t) {
        "Researched.".to_string()
    } else if !w.progress.is_open(t.company()) {
        let key = t.requires().map_or("its key item", |i| i.name());
        format!("Make {key} once: it opens {}, and then this. Made with the {}", t.company().name(), t.requires().map_or(String::new(), recipe_hint))
    } else {
        format!("Enter: research for {} RP (you have {rp}). Or free once you've made {n} {} ({made}/{n}).", grouped(t.cost()), item.name())
    };
    vec![(format!("{} · {}", t.name(), t.category().name()), head), (format!("{}.", capitalise(t.about())), Style::new()), (status, note)]
}

/// The research screen, over everything but the bottom line (where results show).
/// New parts in groups; the picked row's details sit under the list.
fn draw_tree(f: &mut Frame, area: Rect, app: &mut App) {
    let head = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let dim = Style::new().fg(Color::DarkGray);
    let r = Rect { height: area.height.saturating_sub(1), ..area };
    f.render_widget(Clear, r);
    f.render_widget(Block::bordered().title(" Research ").title_style(head), r);
    if r.width < 12 || r.height < 6 {
        return;
    }
    let x0 = r.x + 2;
    let width = r.width.saturating_sub(4);
    let mut y = r.y + 1;
    let bottom = r.y + r.height - 1; // the border
    let footer_y = bottom - 1;
    let line = |f: &mut Frame, y: u16, spans: Vec<Span<'static>>| {
        f.render_widget(Paragraph::new(Line::from(spans)), Rect::new(x0, y, width, 1));
    };

    // The points, on the right of the top line.
    let points = format!("{} research", grouped(app.world.econ.research));
    let pts_st = Style::new().fg(Color::LightBlue).add_modifier(Modifier::BOLD);
    line(f, y, vec![Span::styled(points, pts_st)]);
    y += 2;

    // Details first (so the list knows how much room it has), wrapped to the window.
    let rows = tree_rows();
    let pick = app.tree_pick.min(rows.len() - 1);
    app.tree_pick = pick;
    let details: Vec<(String, Style)> = tree_details(&app.world, rows[pick])
        .into_iter()
        .flat_map(|(text, st)| wrap_words(&text, width.max(10) as usize).into_iter().map(move |l| (l, st)))
        .collect();

    // The list: group names in a left gutter when there's room, or as their own dim lines.
    let gutter: u16 = if width >= 64 { 14 } else { 0 };
    let groups: Vec<(&str, usize)> = UNLOCK_GROUPS.iter().map(|g| (g.0, g.1.len())).collect();
    // Lines: (group label, row index); a label line has no row.
    let mut lines: Vec<(Option<&str>, Option<usize>)> = Vec::new();
    let mut k = 0;
    for (gi, (label, n)) in groups.iter().enumerate() {
        if gutter == 0 {
            if gi > 0 {
                lines.push((None, None));
            }
            lines.push((Some(label), None));
            for j in 0..*n {
                lines.push((None, Some(k + j)));
            }
        } else {
            if gi > 0 {
                lines.push((None, None));
            }
            for j in 0..*n {
                lines.push((if j == 0 { Some(label) } else { None }, Some(k + j)));
            }
        }
        k += n;
    }
    // The list takes what it needs; the details go right under it.
    let room = footer_y.saturating_sub(details.len() as u16 + 2).max(y + 2);
    let list_bottom = (y + lines.len() as u16).min(room);
    let view = (list_bottom - y) as usize;
    let at = lines.iter().position(|l| l.1 == Some(pick)).unwrap_or(0);
    let top = if lines.len() <= view { 0 } else { (at + 2).saturating_sub(view).min(lines.len() - view) };
    let name_w = if width >= 50 { 18 } else { 15 };
    app.tree_rows.clear();
    for (n, (label, row)) in lines.iter().enumerate().skip(top).take(view) {
        let ly = y + (n - top) as u16;
        let mut spans = Vec::new();
        if gutter > 0 {
            spans.push(Span::styled(format!("{:<w$}", label.unwrap_or(""), w = gutter as usize), dim));
        } else if row.is_none() {
            spans.push(Span::styled(label.unwrap_or("").to_string(), dim));
        }
        if let Some(i) = row {
            let mut rs = tree_row_spans(&app.world, rows[*i], name_w);
            if *i == pick {
                // The picked row: one even band across the row, colours kept.
                let used: usize = rs.iter().map(|s| s.content.chars().count()).sum();
                rs.push(Span::raw(" ".repeat((width - gutter) as usize - used.min((width - gutter) as usize))));
                for s in rs.iter_mut() {
                    s.style = s.style.bg(Color::Indexed(238));
                }
            }
            spans.extend(rs);
            app.tree_rows.push((Rect::new(x0 + gutter, ly, width - gutter, 1), *i));
        }
        line(f, ly, spans);
    }

    // Details, under a rule.
    let mut dy = list_bottom + 1;
    line(f, list_bottom, vec![Span::styled("─".repeat(width as usize), dim)]);
    for (text, st) in details {
        if dy >= footer_y {
            break;
        }
        line(f, dy, vec![Span::styled(text, st)]);
        dy += 1;
    }

    // Footer: keys, and which way the list goes on.
    let c = &app.controls;
    let keys = format!("↑↓ pick · Enter research · {} close", c.key(Action::Tree));
    line(f, footer_y, vec![Span::styled(keys, dim)]);
    let mut more = Vec::new();
    if top > 0 {
        more.push("▲");
    }
    if top + view < lines.len() {
        more.push("▼");
    }
    if !more.is_empty() {
        let text = format!("more {}", more.join(" "));
        let tw = text.chars().count() as u16;
        f.render_widget(Paragraph::new(Span::styled(text, Style::new().fg(KEY_ITEM).add_modifier(Modifier::BOLD))), Rect::new(x0 + width.saturating_sub(tw), footer_y, tw.min(width), 1));
    }
}

/// Machines that craft, in the order the book lists them.
const CRAFTERS: [Kind; 4] = [Kind::Furnace, Kind::Press, Kind::BlastFurnace, Kind::Assembler];
const LINK: Color = Color::Indexed(117);

/// Whether the player can get this item yet (or already has).
fn item_known(w: &World, item: Item) -> bool {
    let mined = match item {
        Item::CopperOre | Item::Coal => true,
        Item::IronOre => w.progress.part_unlocked(Kind::BasicDrill),
        _ => false,
    };
    mined
        || w.econ.made[item.idx()] > 0
        || CRAFTERS.iter().any(|&k| {
            w.progress.part_unlocked(k)
                && k.recipes().iter().any(|r| r.outputs.iter().any(|o| o.0 == item) && r.needs.map_or(true, |t| w.progress.has(t)))
        })
}

/// A number for the book: whole above 100 ("800"), one decimal below ("12.5").
fn num(v: f64) -> String {
    if v >= 100.0 {
        format!("{v:.0}")
    } else if v >= 1.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

/// A piece of text on a book page; some pieces are links to another page.
#[derive(Clone)]
struct Seg {
    text: String,
    style: Style,
    link: Option<Entry>,
}

fn seg(text: impl Into<String>, style: Style) -> Seg {
    Seg { text: text.into(), style, link: None }
}

/// A link to a page; the page you're on isn't a link to itself.
fn link(e: Entry, here: Entry) -> Seg {
    if e == here {
        seg(e.name(), Style::new().add_modifier(Modifier::BOLD))
    } else {
        Seg { text: e.name().to_string(), style: Style::new().fg(LINK), link: Some(e) }
    }
}

type Row = Vec<Seg>;

/// Word-wraps pieces to `width`; rows after the first start at `hang`. Links stay links.
fn lay_out(segs: Vec<Seg>, width: usize, hang: usize) -> Vec<Row> {
    // Split into words and runs of spaces, each keeping its piece's style and link.
    let mut toks: Vec<Seg> = Vec::new();
    for s in segs {
        let mut cur = String::new();
        let mut space = None;
        for ch in s.text.chars() {
            let sp = ch == ' ';
            if space.is_some_and(|x| x != sp) {
                toks.push(Seg { text: std::mem::take(&mut cur), ..s.clone() });
            }
            space = Some(sp);
            cur.push(ch);
        }
        if !cur.is_empty() {
            toks.push(Seg { text: cur, ..s });
        }
    }
    let width = width.max(hang + 8);
    let mut rows: Vec<Row> = vec![Vec::new()];
    let mut col = 0;
    for t in toks {
        let n = t.text.chars().count();
        let is_space = t.text.starts_with(' ');
        if col + n > width && col > hang && !is_space {
            while rows.last().unwrap().last().is_some_and(|s: &Seg| s.text.starts_with(' ')) {
                rows.last_mut().unwrap().pop();
            }
            rows.push(vec![seg(" ".repeat(hang), Style::new())]);
            col = hang;
        }
        if is_space && (col == hang && rows.len() > 1 || col + n > width) {
            continue;
        }
        col += n;
        rows.last_mut().unwrap().push(t);
    }
    rows
}

/// A recipe as a word equation: "2 iron gear + steel ─coal─→ gearbox", with the fuel over the arrow.
fn equation(r: &Recipe, here: Entry) -> Vec<Seg> {
    let side = |list: &[(Item, u32)]| {
        let mut out = Vec::new();
        for (i, (item, n)) in list.iter().enumerate() {
            if i > 0 {
                out.push(seg(" + ", Style::new().fg(Color::DarkGray)));
            }
            if *n > 1 {
                out.push(seg(format!("{n} "), Style::new().add_modifier(Modifier::BOLD)));
            }
            out.push(link(Entry::Item(*item), here));
        }
        out
    };
    let arrow = match r.fuel {
        Some(f) => format!("  ─{}─→  ", f.name()),
        None => "  ──→  ".to_string(),
    };
    let mut out = side(r.inputs);
    out.push(seg(arrow, Style::new().fg(Color::Indexed(244))));
    out.extend(side(r.outputs));
    out
}

/// Why a recipe can't be used yet, if it can't.
fn recipe_lock(w: &World, k: Kind, r: &Recipe) -> Option<String> {
    if !w.progress.part_unlocked(k) {
        Some(format!("needs the {}", k.name().to_lowercase()))
    } else {
        r.needs.filter(|t| !w.progress.has(*t)).map(|t| format!("needs {}", t.name()))
    }
}

fn timing(r: &Recipe, made: u32) -> String {
    let per_min = made as f64 * 60.0 * TPS as f64 / r.ticks as f64;
    format!("{:.1}s · {per_min:.0}/min", r.ticks as f64 / TPS as f64)
}

/// A book page as rows: a title, then labelled rows (no headings), then (for items) the research chart.
fn book_page(w: &World, e: Entry, width: u16, setup_key: &str) -> Vec<Row> {
    let width = width as usize;
    let dim = Style::new().fg(Color::DarkGray);
    let warn = Style::new().fg(Color::Indexed(214));
    const GUTTER: usize = 10; // "Made by   "
    const EQ: usize = GUTTER + 2; // equations sit a little further in
    let mut out: Vec<Row> = Vec::new();
    let labelled = |out: &mut Vec<Row>, label: &str, indent: usize, mut segs: Vec<Seg>| {
        let pad = if label.is_empty() { " ".repeat(indent) } else { format!("{label:<indent$}") };
        segs.insert(0, seg(pad, Style::new().fg(Color::Cyan)));
        out.extend(lay_out(segs, width, indent));
    };
    match e {
        Entry::Item(item) => {
            let (glyph, gst) = item_cell(item, Color::Reset);
            out.push(vec![
                seg(glyph.to_string(), gst),
                seg(format!(" {}", capitalise(item.name())), Style::new().add_modifier(Modifier::BOLD)),
                seg(
                    if item.price() < 0.0 {
                        format!("   costs ${:.2} to sell · made {}", -item.price(), w.econ.made[item.idx()])
                    } else {
                        format!("   sells ${} · made {}", item.price(), w.econ.made[item.idx()])
                    },
                    dim,
                ),
            ]);
            out.push(vec![]);

            let mut first = true;
            let label = |first: &mut bool, l: &'static str| if std::mem::take(first) { l } else { "" };
            let mined: Option<(Vec<Kind>, &str)> = match item {
                Item::CopperOre => Some((vec![Kind::Drill, Kind::BasicDrill], "anywhere: press f on a drill to pick its ore; faster underground")),
                Item::Coal => Some((vec![Kind::Drill, Kind::BasicDrill], "anywhere (f picks the ore); a drill on coal fuels itself")),
                Item::IronOre => Some((vec![Kind::BasicDrill], "anywhere (f picks the ore); cheap drills can't cut it")),
                _ => None,
            };
            if let Some((drills, how)) = mined {
                let mut segs = Vec::new();
                for (i, d) in drills.iter().enumerate() {
                    if i > 0 {
                        segs.push(seg(" or ", Style::new()));
                    }
                    segs.push(link(Entry::Part(*d), e));
                }
                segs.push(seg(format!(" {how}"), Style::new()));
                labelled(&mut out, label(&mut first, "Made by"), GUTTER, segs);
            }
            for k in CRAFTERS {
                for r in k.recipes().iter().filter(|r| r.outputs.iter().any(|o| o.0 == item)) {
                    let made = r.outputs.iter().find(|o| o.0 == item).map_or(1, |o| o.1);
                    let mut segs = vec![link(Entry::Part(k), e), seg(format!("  {}", timing(r, made)), dim)];
                    if let Some(l) = recipe_lock(w, k, r) {
                        segs.push(seg(format!("  ({l})"), warn));
                    }
                    labelled(&mut out, label(&mut first, "Made by"), GUTTER, segs);
                    labelled(&mut out, "", EQ, equation(r, e));
                }
            }
            if first {
                labelled(&mut out, "Made by", GUTTER, vec![seg("nothing yet", dim)]);
            }

            // Uses: one equation per recipe, then the machines that run it.
            let mut uses: Vec<(&'static Recipe, Vec<Kind>)> = Vec::new();
            for k in CRAFTERS {
                for r in k.recipes().iter().filter(|r| r.inputs.iter().any(|i| i.0 == item)) {
                    match uses.iter_mut().find(|u| u.0.name == r.name) {
                        Some(u) => u.1.push(k),
                        None => uses.push((r, vec![k])),
                    }
                }
            }
            let mut first = true;
            for (r, machines) in &uses {
                let mut segs = equation(r, e);
                segs.push(seg("   in ", dim));
                for (i, k) in machines.iter().enumerate() {
                    if i > 0 {
                        segs.push(seg(", ", dim));
                    }
                    segs.push(link(Entry::Part(*k), e));
                }
                labelled(&mut out, label(&mut first, "Used in"), GUTTER, segs);
            }
            let fuel: Option<(Vec<Kind>, &str)> = match item {
                Item::Coal => Some((vec![Kind::Drill, Kind::Furnace, Kind::Boiler], "")),
                Item::Coke => Some((vec![Kind::BlastFurnace], " (burns twice as long as coal)")),
                _ => None,
            };
            if let Some((kinds, note)) = fuel {
                let mut segs = vec![seg("fuel for ", Style::new())];
                for (i, k) in kinds.iter().enumerate() {
                    if i > 0 {
                        segs.push(seg(", ", Style::new()));
                    }
                    segs.push(if BOOK_PARTS.contains(k) { link(Entry::Part(*k), e) } else { seg(k.name().to_lowercase(), Style::new()) });
                }
                segs.push(seg(note, dim));
                labelled(&mut out, label(&mut first, "Used in"), GUTTER, segs);
            }
            if first && item == Item::Waste {
                labelled(&mut out, "Used in", GUTTER, vec![seg("nothing: a trade post set to money takes it, for a fee", dim)]);
            } else if first {
                labelled(&mut out, "Used in", GUTTER, vec![seg("nothing yet: sell it at a trade post", dim)]);
            }

            let mut first = true;
            let key = Style::new().fg(KEY_ITEM);
            if let Some(i) = TREE.iter().position(|(n, _)| *n == Node::Key(item)) {
                let next: Vec<String> = tree_children(i).iter().map(|&k| node_name(TREE[k].0)).collect();
                let from: Vec<&str> = Company::ALL.iter().filter(|c| c.key_item() == Some(item)).map(|c| c.name()).collect();
                let letter = (!from.is_empty()).then(|| format!(" and a letter from {}", from.join(" and ")));
                let done = if w.econ.made[item.idx()] > 0 { " (done)" } else { "" };
                let text = format!("your first one brings {}{}{done}", next.join(", "), letter.unwrap_or_default());
                labelled(&mut out, label(&mut first, "Unlocks"), GUTTER, vec![seg(text, key)]);
            }
            for t in Tech::ALL.iter().filter(|t| t.milestone().0 == item && !w.progress.has(**t)) {
                let n = t.milestone().1;
                let text = format!("make {n} to unlock {} free ({}/{n})", t.name(), w.econ.made[item.idx()].min(n));
                labelled(&mut out, label(&mut first, "Unlocks"), GUTTER, vec![seg(text, key)]);
            }
            out.push(vec![]);
            if item == Item::Waste {
                out.push(vec![seg("Never earns research. Research and split trade posts won't take it,", dim)]);
                out.push(vec![seg("and money posts stop when the fees pass what you earn.", dim)]);
            } else {
                for l in research_chart(w, item, width as u16) {
                    out.push(l.spans.into_iter().map(|s| seg(s.content.into_owned(), s.style)).collect());
                }
            }
        }
        Entry::Part(k) => {
            let mut title = vec![seg(" ", Style::new())];
            for (c, st) in kind_cells(k, Dir::E, true) {
                title.push(seg(c.to_string(), st));
            }
            let (kw, kh) = k.base_dims();
            let from = k.company().map_or(String::new(), |c| format!(" · from {}", c.name()));
            title.push(seg(format!(" {}", k.name()), Style::new().add_modifier(Modifier::BOLD)));
            title.push(seg(format!("   ${:.0} · {kw}x{kh}{from}", k.cost()), dim));
            out.push(title);
            out.push(vec![]);

            // Can you build it yet, and if not, what opens it.
            let status = if w.progress.part_unlocked(k) {
                vec![seg("ready to build", Style::new().fg(Color::Green))]
            } else {
                let mut segs = Vec::new();
                if let Some(c) = k.company().filter(|c| !w.progress.is_open(*c)) {
                    segs.push(seg("opens when you first make ", warn));
                    match c.key_item() {
                        Some(i) => segs.push(link(Entry::Item(i), e)),
                        None => segs.push(seg("its key item", warn)),
                    }
                    if k.tech().is_some() {
                        segs.push(seg(", then ", warn));
                    }
                }
                if let Some(t) = k.tech().filter(|t| !w.progress.has(*t)) {
                    let (item, n) = t.milestone();
                    segs.push(seg(format!("research {} ({:.0}R) or make {n} ", t.name(), t.cost()), warn));
                    segs.push(link(Entry::Item(item), e));
                    segs.push(seg(format!(" ({}/{n})", w.econ.made[item.idx()].min(n)), warn));
                }
                segs
            };
            labelled(&mut out, "Status", GUTTER, status);

            match k {
                Kind::Drill | Kind::BasicDrill => {
                    let mines: &[Item] = if k == Kind::Drill { &[Item::CopperOre, Item::Coal] } else { &[Item::CopperOre, Item::Coal, Item::IronOre] };
                    let mut segs = Vec::new();
                    for (i, m) in mines.iter().enumerate() {
                        if i > 0 {
                            segs.push(seg(" · ", dim));
                        }
                        segs.push(link(Entry::Item(*m), e));
                    }
                    labelled(&mut out, "Mines", GUTTER, segs);
                    labelled(&mut out, "Burns", GUTTER, vec![link(Entry::Item(Item::Coal), e), seg(" (a drill on coal fuels itself)", dim)]);
                }
                _ => {
                    let mut first = true;
                    for r in k.recipes() {
                        let made = r.outputs[0].1;
                        let mut segs = equation(r, e);
                        segs.push(seg(format!("   {}", timing(r, made)), dim));
                        if let Some(l) = r.needs.filter(|t| !w.progress.has(*t)).map(|t| format!("needs {}", t.name())) {
                            segs.push(seg(format!("  ({l})"), warn));
                        }
                        labelled(&mut out, if std::mem::take(&mut first) { "Makes" } else { "" }, GUTTER, segs);
                    }
                    labelled(&mut out, "", GUTTER, vec![seg(format!("{setup_key} on a placed one picks the recipe"), dim)]);
                }
            }
        }
    }
    out
}

/// How much research each sale of an item gives, as a bar chart: it starts at the item's
/// worth and fades with every sale down to a floor. Sales already made are grey, the ones
/// ahead in colour; the y axis marks the start and the floor, and a marker shows where you are.
fn research_chart(w: &World, item: Item, width: u16) -> Vec<Line<'static>> {
    let dim = Style::new().fg(Color::DarkGray);
    let axis = Style::new().fg(Color::Indexed(240));
    let bold = Style::new().add_modifier(Modifier::BOLD);
    let curve = item.curve();
    let sold = w.econ.research_sales[item.idx()];
    let floor = curve.worth * curve.floor;
    // Where the curve reaches its floor, and how far the chart runs (a bit past that, and past you).
    let to_floor = if curve.decay < 1.0 && curve.floor > 0.0 { (curve.floor.ln() / curve.decay.ln()).ceil() as u64 } else { 100 };
    let span = (to_floor + to_floor / 4).max(sold + sold / 8 + 10).max(10);
    let rows = 6usize;
    // Which row the floor's bar tops out in, so its label sits level with it.
    let floor_row = rows - 1 - ((curve.floor * (rows * 8) as f64).round() as usize).saturating_sub(1) / 8;
    let labels: Vec<String> = (0..rows)
        .map(|r| match r {
            0 => num(curve.worth),
            r if r == floor_row => num(floor),
            r if r == rows - 1 => "0".into(),
            _ => String::new(),
        })
        .collect();
    let lw = labels.iter().map(|l| l.len()).max().unwrap_or(1);
    let label_w = lw + 2; // "320 ┤"
    let note = " ← floor";
    let cols = (width as usize).saturating_sub(label_w + note.len() + 1).clamp(10, 60);
    let n_at = |c: usize| (c as u64 * span) / (cols as u64 - 1).max(1);
    let you = (((sold as f64 / span as f64) * (cols - 1) as f64).round() as usize).min(cols - 1);
    let ahead = Style::new().fg(Color::Cyan);

    let mut out = vec![Line::from(vec![
        Span::styled("Research per sale: ", dim),
        Span::styled(num(w.econ.next_research(item)), bold),
        Span::styled(" now, fading to a floor of ", dim),
        Span::styled(num(floor), bold),
        Span::styled(format!(" after ~{to_floor} sales"), dim),
    ])];
    const BLOCKS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    for (row, label) in labels.iter().enumerate() {
        let tick = if label.is_empty() { '│' } else { '┤' };
        let mut spans = vec![Span::styled(format!("{label:>lw$} {tick}"), axis)];
        // Bars start at zero; each cell shows how much of its row the value fills, in eighths.
        let base = (rows - 1 - row) * 8;
        let mut run = String::new();
        let mut run_past = None;
        for c in 0..cols {
            let v = curve.value(n_at(c)) / curve.worth;
            let eighths = (v * (rows * 8) as f64).round() as usize;
            let past = c < you;
            if run_past.is_some_and(|p| p != past) {
                spans.push(Span::styled(std::mem::take(&mut run), if run_past == Some(true) { dim } else { ahead }));
            }
            run_past = Some(past);
            run.push(BLOCKS[eighths.saturating_sub(base).min(8)]);
        }
        spans.push(Span::styled(run, if run_past == Some(true) { dim } else { ahead }));
        if row == floor_row {
            spans.push(Span::styled(note, dim));
        }
        out.push(Line::from(spans));
    }
    // The x axis, then the marker and the ends of the axis.
    out.push(Line::styled(format!("{:>lw$} └{}", "", "─".repeat(cols)), axis));
    let marker = Style::new().fg(KEY_ITEM).add_modifier(Modifier::BOLD);
    let you_note = format!(" you: {sold} sold");
    let x0 = label_w + you;
    let marker_line = if you + 1 + you_note.len() <= cols {
        Line::from(vec![Span::raw(" ".repeat(x0)), Span::styled("▲", marker), Span::raw(you_note)])
    } else {
        let text = format!("{} ", you_note.trim_start());
        Line::from(vec![Span::raw(" ".repeat(x0.saturating_sub(text.len()))), Span::raw(text), Span::styled("▲", marker)])
    };
    out.push(marker_line);
    let end = format!("{span} sales");
    out.push(Line::styled(format!("{:>w$}0{:>e$}", "", end, w = label_w, e = cols - 1), dim));
    out
}

/// The recipe book: a tab of items and a tab of machines (Tab switches), each a grid
/// (searchable with /), and the selected page under it. Names in blue are links: click them.
fn draw_book(f: &mut Frame, area: Rect, app: &mut App) {
    let head = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let dim = Style::new().fg(Color::DarkGray);
    let r = Rect { height: area.height.saturating_sub(1), ..area };
    f.render_widget(Clear, r);
    let title = if app.book_query.is_empty() && !app.book_typing {
        " Recipe book ".to_string()
    } else {
        format!(" Recipe book · search: {}{} ", app.book_query, if app.book_typing { "_" } else { "" })
    };
    let block = Block::bordered().title(title).title_style(head);
    let inner = block.inner(r);
    f.render_widget(block, r);
    let body = Rect { x: inner.x + 1, width: inner.width.saturating_sub(2), ..inner };
    app.book_links.clear();
    app.book_cells.clear();
    if body.height < 3 || body.width < 10 {
        return;
    }
    const CELL: usize = 19;
    let parts = app.book_entry.is_part();
    let list = Entry::list(&app.book_query, parts);
    let n = list.len().max(1);
    let cols = (body.width as usize / CELL).clamp(1, n);
    app.book_cols = cols;
    let sel = list.iter().position(|e| *e == app.book_entry).unwrap_or(0);
    let w = &app.world;

    let page = book_page(w, app.book_entry, body.width, &app.controls.key(Action::Setup));
    // Rows: the tabs, the grid, a rule, the page, the footer.
    let avail = body.height as usize - 1;
    let grid_rows = list.len().div_ceil(cols).max(1);
    let gh = grid_rows.min(avail.saturating_sub(7).max(1));
    let gtop = (sel / cols + 1).saturating_sub(gh);
    let buf = f.buffer_mut();
    let mut y = body.y;
    // The tabs: the one you're on highlighted; clicking one goes to it.
    let mut x = body.x;
    for (name, on_parts) in [("Items", false), ("Machines", true)] {
        let label = format!(" {name} ");
        let st = if on_parts == parts { Style::new().fg(INK).bg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::new().fg(Color::Gray) };
        buf.set_string(x, y, &label, st);
        // The other tab opens on its last page, or its first match if a search hides that.
        let target = if on_parts == parts {
            app.book_entry
        } else {
            let l = Entry::list(&app.book_query, on_parts);
            let last = app.book_last[on_parts as usize];
            if l.contains(&last) || l.is_empty() { last } else { l[0] }
        };
        app.book_cells.push((Rect::new(x, y, label.chars().count() as u16, 1), target));
        x += label.chars().count() as u16 + 1;
    }
    buf.set_string(x + 1, y, "Tab switches", dim);
    y += 1;
    if list.is_empty() {
        buf.set_string(body.x, y, "nothing matches: Esc clears the search", dim);
    }
    for row in gtop..gtop + gh {
        let mut x = body.x;
        for col in 0..cols {
            let Some(&e) = list.get(row * cols + col) else { break };
            let (glyph, gst, known) = match e {
                Entry::Item(i) => {
                    let (g, st) = item_cell(i, Color::Reset);
                    (g, st, item_known(w, i))
                }
                Entry::Part(k) => {
                    let (g, st) = kind_cells(k, Dir::E, true)[0];
                    (g, st, w.progress.part_unlocked(k))
                }
            };
            let mut st = if known { Style::new() } else { dim };
            if e == app.book_entry {
                st = Style::new().fg(INK).bg(Color::Cyan).add_modifier(Modifier::BOLD);
            }
            buf.set_string(x, y, glyph.to_string(), gst);
            buf.set_string(x + 1, y, format!(" {:<w$}", e.name(), w = CELL - 3), st);
            app.book_cells.push((Rect::new(x, y, CELL as u16 - 1, 1), e));
            x += CELL as u16;
        }
        y += 1;
    }
    buf.set_string(body.x, y, "─".repeat(body.width as usize), dim);
    y += 1;

    // The page, scrolled; every visible link is recorded for the mouse and for Tab.
    let ph = (body.y as usize + avail).saturating_sub(y as usize);
    let max_scroll = page.len().saturating_sub(ph) as u16;
    app.book_scroll = app.book_scroll.min(max_scroll);
    for row in page.iter().skip(app.book_scroll as usize).take(ph) {
        let mut x = body.x;
        for s in row {
            let wdt = s.text.chars().count() as u16;
            let st = s.style;
            if let Some(target) = s.link {
                app.book_links.push((Rect::new(x, y, wdt, 1), target));
            }
            let room = (body.x + body.width).saturating_sub(x);
            buf.set_stringn(x, y, &s.text, room as usize, st);
            x += wdt;
        }
        y += 1;
    }

    // Footer: keys, and where there's more.
    let foot = Rect { y: body.y + avail as u16, height: 1, ..body };
    let key = app.controls.key(Action::Book);
    let back = if app.book_back.is_empty() { "" } else { " · Bksp back" };
    let keys = if app.book_typing {
        "type to search · Enter done · Esc stop".to_string()
    } else {
        format!("arrows pick · Tab items/machines · / search · click a link{back} · {key} close")
    };
    f.render_widget(Paragraph::new(Line::styled(keys, dim)), foot);
    let mut more = Vec::new();
    if gtop > 0 || gtop + gh < grid_rows {
        more.push("list ▲▼");
    }
    if max_scroll > 0 {
        more.push(if app.book_scroll < max_scroll { "page ▼" } else { "page ▲" });
    }
    if !more.is_empty() {
        let text = format!("more: {}", more.join(" "));
        f.render_widget(
            Paragraph::new(Line::styled(text, Style::new().fg(KEY_ITEM).add_modifier(Modifier::BOLD))).alignment(Alignment::Right),
            foot,
        );
    }
}

/// Splits text into lines of at most `width` characters, at spaces.
fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in text.split(' ') {
        let cur = lines.last_mut().unwrap();
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > width {
            lines.push(word.to_string());
        } else {
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(word);
        }
    }
    lines
}

fn node_name(n: Node) -> String {
    match n {
        Node::Start => "Copper Works".into(),
        Node::Tech(t) => t.name().into(),
        Node::Key(i) => format!("* {}", i.name()),
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}

/// Where a key item comes from.
fn recipe_hint(item: Item) -> String {
    for (machine, list) in [("press", PRESS_RECIPES), ("furnace", FURNACE_RECIPES), ("blast furnace", BLAST_RECIPES), ("assembler", ASSEMBLER_RECIPES)] {
        if let Some(r) = list.iter().find(|r| r.outputs.iter().any(|o| o.0 == item)) {
            let ins: Vec<String> = r.inputs.iter().map(|(it, n)| format!("{n} {}", it.name())).collect();
            let need = r.needs.map_or(String::new(), |t| format!(", needs {}", t.name()));
            return format!("{machine}: {}{}", ins.join(" + "), need);
        }
    }
    String::new()
}

/// How tall the current telegram's box would be at `w` columns wide (borders included).
fn mail_height(app: &App, w: u16) -> u16 {
    let Some(m) = &app.mail else { return 0 };
    let (top, body, foot) = mail_parts(m, w.saturating_sub(3));
    // + a blank row and the key hint, + borders.
    (top.len() + body.len() + foot.len()) as u16 + 2 + 2
}

/// A telegram's header, rendered Markdown body and delivery note, wrapped to `width`.
fn mail_parts(m: &crate::mail::Message, width: u16) -> (Vec<Line<'static>>, Vec<Line<'static>>, Vec<Line<'static>>) {
    let head = Style::new().fg(KEY_ITEM).add_modifier(Modifier::BOLD);
    let what = match m.event.as_str() {
        "" => String::new(),
        e => format!(" · {e}"),
    };
    let from = Line::from(vec![Span::styled(format!(" From {}", m.from), head), Span::styled(what, Style::new().fg(Color::Cyan))]);
    let mut top = crate::mail::wrap(&from, width);
    top.push(Line::raw(""));
    let body = crate::mail::render(&m.md, width);
    let mut foot = vec![Line::raw("")];
    if let Some((c, money)) = m.gift {
        let gift = Line::styled(format!(" +${money:.0}  {}", crate::mail::gift_note(c)), Style::new().fg(Color::Green));
        foot.extend(crate::mail::wrap(&gift, width));
    }
    (top, body, foot)
}

/// Draws the current telegram as a box in `r`. `overlay`: open over the map (any key closes it);
/// otherwise it sits in the sidebar while the game keeps running.
fn draw_mail_in(f: &mut Frame, r: Rect, app: &mut App, overlay: bool) {
    let Some(m) = &app.mail else { return };
    let head = Style::new().fg(KEY_ITEM).add_modifier(Modifier::BOLD);
    let dim = Style::new().fg(Color::DarkGray);
    let inner_w = r.width.saturating_sub(3);
    let (top, body, mut foot) = mail_parts(m, inner_w);
    let waiting = app.world.mail.len();
    let key = app.controls.key(Action::Mail);
    // Work out the scroll room first, with a one-row hint; a wrapped hint just takes a row more.
    let view = r.height.saturating_sub(2 + (top.len() + foot.len()) as u16 + 2);
    let max_scroll = (body.len() as u16).saturating_sub(view);
    app.mail_scroll = app.mail_scroll.min(max_scroll);
    let hint = match (overlay, max_scroll > 0) {
        (true, true) => format!(" ↑↓ scroll {}/{max_scroll} · any other key closes", app.mail_scroll),
        (true, false) => " press any key".to_string(),
        (false, true) => format!(" PgUp/PgDn {}/{max_scroll} · {key} {}", app.mail_scroll, if waiting > 0 { "next" } else { "dismiss" }),
        (false, false) => format!(" {key} {}", if waiting > 0 { "next" } else { "dismiss" }),
    };
    foot.push(Line::raw(""));
    foot.extend(crate::mail::wrap(&Line::styled(hint, dim), inner_w));
    let foot_h = foot.len() as u16;
    let title = match (waiting, overlay) {
        (0, _) => " Telegram ".to_string(),
        (n, true) => format!(" Telegram · {n} more (Esc dismisses all) "),
        (n, false) => format!(" Telegram · {n} more "),
    };

    if overlay {
        f.render_widget(Clear, r);
    }
    let block = Block::bordered().title(title).title_style(head).border_style(Style::new().fg(KEY_ITEM));
    let inner = block.inner(r);
    f.render_widget(block, r);
    let [a, b, c] = Layout::vertical([
        Constraint::Length(top.len() as u16),
        Constraint::Min(1),
        Constraint::Length(foot_h),
    ])
    .areas(inner);
    f.render_widget(Paragraph::new(top), a);
    f.render_widget(Paragraph::new(body).scroll((app.mail_scroll, 0)), b);
    f.render_widget(Paragraph::new(foot), c);
}

/// The telegram open over a small pane.
fn draw_mail(f: &mut Frame, area: Rect, app: &mut App) {
    let w = 78.min(area.width);
    let h = mail_height(app, w).min(area.height.saturating_sub(2)).max(8);
    let r = centered(area, w, h);
    draw_mail_in(f, r, app, true);
}

/// A letter from a company that just opened.
fn draw_letter(f: &mut Frame, area: Rect, c: Company) {
    let head = Style::new().fg(KEY_ITEM).add_modifier(Modifier::BOLD);
    let mut lines = vec![
        Line::styled(format!(" A letter from {}", c.name()), head),
        Line::raw(""),
        Line::raw(format!(" \"{}\"", c.letter())),
        Line::raw(""),
    ];
    for t in Tech::ALL.iter().filter(|t| t.company() == c) {
        lines.push(Line::raw(format!(" Unlocked for research: {} ({})", t.name(), t.category().name())));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(" press any key", Style::new().fg(Color::DarkGray)));
    let r = centered(area, 70, 12);
    f.render_widget(Clear, r);
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(Block::bordered().title(" A letter ").title_style(head)),
        r,
    );
}

fn draw_help(f: &mut Frame, area: Rect, app: &mut App) {
    let text = "\
READING THE MAP
  Coloured blocks are buildings, labelled with their name. Production is
  big (2x2 or 3x3), belts / splitters / arms / crates are 1x1. Presses and
  pumps are long (2x1, 1x2) and turn with the rotate key.
  The arrow on a building's edge is its output side; machines also show a
  letter for what they make. A grey block is idle.
  Arrows are belts; items ride on them. Point at anything to see what it is.
  Drills mine anywhere: press f on one to pick copper, coal or iron.
  Items: o copper ore  * coal  = plate  ~ wire  0 pipe  @ coil  # coke
         o iron ore (blue)  = iron plate (blue)  % steel  , waste  H girder
         c concrete  g gear  G gearbox  R drilling rig

HOW IT WORKS
  Buildings take input from any side and output from the arrow side only:
    onto a belt, or straight into the next building (drill -> furnace).
  Drills burn coal; a drill on coal fuels itself. Only basic drills cut iron.
  Furnaces and presses make whatever recipe they're set to (f changes it).
  Belts, splitters, arms and presses need power. Solar panels and poles
    power every electric part in range (v shows it); overlapping ranges
    join into one grid that shares only its own power. Panels never join
    each other directly: put poles between them.
  Pumps draw water anywhere on the ground. Pipes join into networks, each holding one fluid.
    Boilers burn coal to turn water into steam (out their front); steam
    engines turn steam into power, only as much as their grid needs.
  Blast furnaces burn coke. Steel leaves waste: a trade post set to money
    takes it for $0.50 each; research posts refuse it and it backs up.
  Floors: support beams open +1, excavation opens -1 (ore there is richer).
    Elevators (2x2 to 8x8, +/- to resize) have a door on every open floor;
    f sets each door to load, unload or pass. A car leaves only when full.
  Idle machines flash a word saying why (FULL, NEED, COAL, PWR, WATR...).
  Everything unlocks free once you've made enough of its milestone item;
    research is the shortcut.
  Trade posts sell what they receive: f switches money / 40-40 / research.
    Waste costs money and never earns research. Idle posts flash WSTE.
  Belt tunnels: place an entrance, then its exit up to 5 tiles on, same way.
    Long arms reach 2 tiles; filter splitters send their item out the front.
  Research (t) brings new parts; some need a key item made first.
    Nothing breaks or jams at random.

CONTROLS
  Move ......... arrows / WASD       Lay belts .... Shift+arrows / Shift+WASD
  Build menu ... 1-4 opens a category (1 Create, 2 Transport, 3 Power,
                 4 Misc); then 1-4 picks a part. Press it again to switch
                 kind (belt / fast belt / splitter...). ` ~ Esc back.
  Place ........ Space / Enter
  Rotate ....... r (output side)     Remove ....... x (full refund)
  Set up ....... f                   Research ..... t
  Power range .. v                   Floors ....... < >
  Pause ........ p                   Speed ........ [ ]
  Save ......... Ctrl+S (it also saves every 2 minutes and when you quit)
  Production ... o (the last minute, per item; the game keeps running)
  Recipes ...... b (opens on what's under the cursor)
  Small pane ... m (switches on by itself in a small terminal)
  Telegrams .... n next (Shift+N dismisses all), PgUp/PgDn scroll
  Mouse ........ click places, drag lays belts, right-click removes,
                 middle-click copies a part (facing and settings too)
  Quit ......... Ctrl+C (saves first)
  These are the default keys. Change any of them in controls.json.
                         press any key to close";
    let rows = text.lines().count() as u16;
    let r = centered(area, 80, rows + 2);
    // Too tall for the terminal: arrows scroll it.
    let max = (rows + 2).saturating_sub(r.height);
    app.help_scroll = app.help_scroll.min(max);
    let title = if max > 0 { format!(" Help · ↑↓ scroll {}/{max} ", app.help_scroll) } else { " Help ".into() };
    f.render_widget(Clear, r);
    f.render_widget(
        Paragraph::new(text)
            .scroll((app.help_scroll, 0))
            .block(Block::bordered().title(title).title_style(Style::new().fg(Color::Cyan))),
        r,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    use ratatui::{backend::TestBackend, Terminal};

    fn render(app: &mut App, w: u16, h: u16) -> Buffer {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, app)).unwrap();
        term.backend().buffer().clone()
    }

    fn text(buf: &Buffer) -> String {
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf.cell((x, y)).map_or(" ", |c| c.symbol()));
            }
            out.push('\n');
        }
        out
    }

    /// Renders the demo factory, the research tree and a letter, writing colour
    /// snapshots to target/ so the look can be checked in a browser.
    #[test]
    fn render_snapshots() {
        let mut w = World::new();
        w.build_demo().unwrap();
        for _ in 0..TPS * 60 {
            w.tick();
        }
        let mut app = App::new(w);
        app.cursor = (58, 45);
        app.power_view = true;
        app.menu = Some(Category::Create);
        let buf = render(&mut app, 130, 42);
        assert!(text(&buf).contains("BUILD > CREATE"));
        assert!(text(&buf).contains("Copper press"));
        std::fs::write("target/snapshot.html", to_html(&buf)).unwrap();

        app.menu = None;
        let buf = render(&mut app, 130, 42);
        assert!(text(&buf).contains("4 Misc"));
        std::fs::write("target/snapshot-menu.html", to_html(&buf)).unwrap();
        app.menu = Some(Category::Create);

        app.power_view = false;
        app.tree = true;
        let buf = render(&mut app, 130, 42);
        assert!(text(&buf).contains("Copper press") && text(&buf).contains("make copper wire first"));

        app.tree = false;
        app.letter = Some(Company::Ashgrove);
        let buf = render(&mut app, 130, 42);
        assert!(text(&buf).contains("A letter from"));
        std::fs::write("target/snapshot-letter.html", to_html(&buf)).unwrap();

        app.letter = None;
        let mut m = crate::mail::parse(
            "from: api-server\nevent: finished\n# Auth retry fixed\n\nAll **42 tests** pass. Changed `src/auth.rs`:\n\n- retry on 503, up to 3 times, with a short backoff between tries so a flaky upstream does not get hammered while it recovers\n- [x] lint clean\n- [ ] needs your review\n\n> Didn't touch the session cache.",
            0,
        );
        m.gift = Some((Company::Brannock, 120.0));
        app.mail = Some(m);
        app.menu = None;
        let buf = render(&mut app, 130, 42);
        let t = text(&buf);
        assert!(t.contains("From api-server"));
        assert!(t.contains("All 42 tests pass"));
        assert!(t.contains("+$120"));
        assert!(t.contains("n dismiss"), "the telegram sits in the sidebar: {t}");
        assert!(t.contains("cache.") && !t.contains("PgUp"), "the whole telegram fits: {t}");
        assert!(t.contains("HERE"), "and the sidebar keeps its other parts");
        std::fs::write("target/snapshot-mail.html", to_html(&buf)).unwrap();

        // In a small pane it's a note on the bottom line until n opens it.
        app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
        let buf = render(&mut app, 64, 16);
        std::fs::write("target/snapshot-small-note.html", to_html(&buf)).unwrap();
        let t = text(&buf);
        let last = t.lines().last().unwrap().to_string();
        assert!(last.trim_end().ends_with("agent finished · n to view"), "{last}");
        assert!(last.starts_with(" ? help"), "the rest of the line is unchanged: {last}");
        app.mail_open = true;
        let buf = render(&mut app, 64, 16);
        assert!(text(&buf).contains("From api-server"));
        std::fs::write("target/snapshot-mail-small.html", to_html(&buf)).unwrap();
        app.mail_open = false;
    }

    /// The research tree turns top-down in a tall, narrow window, and scrolls in a small one.
    #[test]
    fn research_tree_fits_the_window() {
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        let mut w = World::new();
        w.build_demo().unwrap();
        w.econ.research = 17_687.0;
        let mut app = App::new(w);
        app.tree = true;
        // Wide: group names in the gutter, every unlock showing, details under the list.
        let buf = render(&mut app, 130, 42);
        let t = text(&buf);
        assert!(t.contains("17,687 research") && !t.contains("Upgrades"), "{t}");
        assert!(t.contains("Making        ● Copper press"), "{t}");
        assert!(t.contains("make copper wire first") && t.contains("Floors") && t.contains("Belt tunnels"), "{t}");
        assert!(t.contains("Copper press · Create") && t.contains("Enter: research for 60 RP"), "{t}");
        assert!(!t.contains("more "), "{t}");
        std::fs::write("target/snapshot-tree.html", to_html(&buf)).unwrap();
        // Narrow: group names on their own lines; small: it scrolls, with markers.
        let t = text(&render(&mut app, 60, 50));
        assert!(t.lines().any(|l| l.trim_matches(|c| c == '│' || c == ' ') == "Making"), "{t}");
        let buf = render(&mut app, 64, 16);
        assert!(text(&buf).contains("more "), "{}", text(&buf));
        std::fs::write("target/snapshot-tree-small.html", to_html(&buf)).unwrap();
        // Clicking a row picks it; clicking it again researches it.
        render(&mut app, 130, 42);
        let (r, i) = app.tree_rows.iter().copied().find(|(_, i)| tree_rows()[*i] == Tech::Tunnels).unwrap();
        let click = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x + 2, row: r.y, modifiers: KeyModifiers::NONE };
        app.on_mouse(click);
        assert_eq!(app.tree_pick, i);
        assert!(!app.world.progress.has(Tech::Tunnels));
        app.on_mouse(click);
        assert!(app.world.progress.has(Tech::Tunnels));
        // n opens a waiting telegram over the screen.
        app.mail = Some(crate::mail::parse("Done", 0));
        app.on_key(KeyEvent::from(KeyCode::Char('n')));
        assert!(app.mail_open && app.tree);
    }

    /// The recipe book: pages for items and machines, word equations, links, search.
    #[test]
    fn recipe_book() {
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        let mut w = World::new();
        w.build_demo().unwrap();
        let mut app = App::new(w);
        let key = |app: &mut App, c: KeyCode| app.on_key(KeyEvent::from(c));
        // Pointing at the demo's furnace opens the book on copper plate.
        app.cursor = (54, 38);
        key(&mut app, KeyCode::Char('b'));
        assert!(app.book);
        assert_eq!(app.book_entry, Entry::Item(Item::CopperPlate));
        let buf = render(&mut app, 130, 42);
        let t = text(&buf);
        assert!(t.contains("Made by") && t.contains("Used in") && !t.contains("MADE BY"), "{t}");
        assert!(t.contains("copper ore  ─coal─→  copper plate"), "a word equation, fuel over the arrow: {t}");
        assert!(t.contains("copper plate  ──→  2 copper wire   in Copper press, Assembler"), "{t}");
        assert!(t.contains("12.0 now, fading to a floor of 0.36") && t.contains("0.36 ┤") && t.contains("← floor"), "{t}");
        std::fs::write("target/snapshot-book.html", to_html(&buf)).unwrap();

        // Tab switches to the machines tab and back to the item you were on.
        assert!(t.contains(" Items ") && t.contains(" Machines ") && !t.contains("Basic drill"), "items only: {t}");
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.book_entry, Entry::Part(Kind::Drill));
        let t = text(&render(&mut app, 130, 42));
        assert!(t.contains("Basic drill") && !t.contains("iron gear "), "machines only: {t}");
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.book_entry, Entry::Item(Item::CopperPlate));

        // Clicking a machine's name opens its page (on the machines tab); Backspace comes back.
        render(&mut app, 130, 42);
        let (r, _) = *app.book_links.iter().find(|(_, e)| *e == Entry::Part(Kind::Furnace)).unwrap();
        app.on_mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
        assert_eq!(app.book_entry, Entry::Part(Kind::Furnace));
        key(&mut app, KeyCode::Backspace);
        assert_eq!(app.book_entry, Entry::Item(Item::CopperPlate));
        app.book_go(Entry::Part(Kind::Furnace));
        let buf = render(&mut app, 130, 42);
        let t = text(&buf);
        assert!(t.contains("Status") && t.contains("ready to build") && t.contains("Makes"), "{t}");
        assert!(t.contains("2 coal  ──→  coke"), "{t}");
        std::fs::write("target/snapshot-book-machine.html", to_html(&buf)).unwrap();

        // Steel: the waste shows, and it's locked behind the blast furnace.
        app.book_go(Entry::Item(Item::Steel));
        let t = text(&render(&mut app, 130, 42));
        assert!(t.contains("3 iron plate  ─coke─→  steel + waste"), "{t}");
        assert!(t.contains("(needs the blast furnace)"), "{t}");

        // Search: / then letters filter the grid and jump to the first match; Esc clears it.
        key(&mut app, KeyCode::Char('/'));
        for c in "gear".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        assert_eq!(app.book_entry, Entry::Item(Item::IronGear));
        assert_eq!(Entry::list(&app.book_query, false).len(), 2);
        let t = text(&render(&mut app, 130, 42));
        assert!(t.contains("search: gear_"), "{t}");
        key(&mut app, KeyCode::Enter);
        assert!(!app.book_typing && app.book);
        key(&mut app, KeyCode::Esc);
        assert!(app.book_query.is_empty() && app.book, "the first Esc clears the search");
        // A search that only matches machines moves to the machines tab.
        key(&mut app, KeyCode::Char('/'));
        for c in "press".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        assert_eq!(app.book_entry, Entry::Part(Kind::Press));
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Esc);

        // After some sales the part already sold turns grey and the marker moves along.
        app.book_go(Entry::Item(Item::CopperPlate));
        app.world.econ.research_sales[Item::CopperPlate.idx()] = 150;
        let buf = render(&mut app, 130, 42);
        assert!(text(&buf).contains("you: 150 sold"));
        std::fs::write("target/snapshot-book-chart.html", to_html(&buf)).unwrap();

        // n opens a waiting telegram over the book.
        app.mail = Some(crate::mail::parse("from: api\nevent: finished\nDone", 0));
        let t = text(&render(&mut app, 130, 42));
        assert!(t.contains("agent finished · n to view"), "{t}");
        key(&mut app, KeyCode::Char('n'));
        assert!(app.mail_open && app.book);
        key(&mut app, KeyCode::Char('x'));
        assert!(!app.mail_open && app.book, "closing the telegram goes back to the book");

        // A small pane: fewer columns, and a scrollable page.
        let buf = render(&mut app, 64, 20);
        assert!(app.book_cols < 6);
        std::fs::write("target/snapshot-book-small.html", to_html(&buf)).unwrap();
        key(&mut app, KeyCode::Char('b'));
        assert!(!app.book);
    }

    /// In a small terminal the map fills the pane between a ticker and a status line.
    /// The production panel sits over the map and lists the last minute, per item.
    #[test]
    fn production_panel() {
        let mut w = World::new();
        w.build_demo().unwrap();
        for _ in 0..TPS * 70 {
            w.tick();
        }
        let mut app = App::new(w);
        app.on_key(KeyEvent::from(KeyCode::Char('o')));
        assert!(app.stats);
        let buf = render(&mut app, 130, 42);
        let t = text(&buf);
        assert!(t.contains("Production · last minute") && t.contains("research/min"), "{t}");
        assert!(t.contains("copper plate") && t.contains("next sale") && t.contains("Factory"), "beside the sidebar: {t}");
        std::fs::write("target/snapshot-production.html", to_html(&buf)).unwrap();
        // Small pane: fewer columns, still readable.
        let t = text(&render(&mut app, 60, 20));
        assert!(t.contains("copper plate") && !t.contains("next sale"), "{t}");
        // Any other key closes it and still does its job.
        app.on_key(KeyEvent::from(KeyCode::Char('t')));
        assert!(!app.stats && app.tree);
    }

    /// Waste: hovering a crate of it, the production panel, and its recipe book page.
    #[test]
    fn waste_shows_up() {
        let mut w = World::new();
        w.build_demo().unwrap();
        w.place(0, 66, 36, Kind::Crate, Dir::E).unwrap();
        if let Some(Building { st: St::Crate { items }, .. }) = w.floors[0].tiles[idx(66, 36)].as_mut() {
            items.extend([Item::Waste; 200]);
        }
        let mut app = App::new(w);
        app.cursor = (66, 36);
        app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
        let buf = render(&mut app, 130, 42);
        let t = text(&buf);
        assert!(t.contains("Removing costs $100 (200 waste)"), "{t}");
        std::fs::write("target/snapshot-waste.html", to_html(&buf)).unwrap();

        // The production panel: fees show as negative money.
        let w = &mut app.world;
        w.place(0, 70, 36, Kind::TradePost, Dir::E).unwrap();
        w.econ.money = 500.0;
        for t in 0..TPS * 70 {
            w.tick();
            if t % 90 == 0 {
                w.offer(0, 70, 36, Item::Waste);
            }
        }
        app.stats = true;
        let buf = render(&mut app, 130, 42);
        let t = text(&buf);
        assert!(t.contains("waste") && t.contains("-$"), "{t}");
        assert!(t.contains("Waste costs money"), "{t}");
        std::fs::write("target/snapshot-waste-production.html", to_html(&buf)).unwrap();
        let t = text(&render(&mut app, 50, 30));
        assert!(t.contains("waste"), "{t}");
        app.stats = false;

        // Its recipe book page: no research chart.
        app.book_go(Entry::Item(Item::Waste));
        app.book = true;
        let buf = render(&mut app, 130, 42);
        let t = text(&buf);
        assert!(t.contains("costs $0.50 to sell") && t.contains("Never earns research") && t.contains("steel + waste"), "{t}");
        std::fs::write("target/snapshot-waste-book.html", to_html(&buf)).unwrap();
    }

    /// The logistics toys on the map: a tunnel under a belt, a long arm, a filter splitter, a warehouse.
    #[test]
    fn toys_snapshot() {
        let mut w = World::blank();
        let (x, y) = (55, 38);
        w.place(0, x, y, Kind::Crate, Dir::E).unwrap();
        w.place(0, x + 1, y, Kind::Tunnel, Dir::E).unwrap();
        for k in 2..=4 {
            w.place(0, x + k, y, Kind::Belt, Dir::N).unwrap();
        }
        w.place(0, x + 5, y, Kind::Tunnel, Dir::E).unwrap();
        w.place(0, x + 6, y, Kind::Sorter, Dir::E).unwrap();
        w.place(0, x + 7, y, Kind::Belt, Dir::E).unwrap();
        w.place(0, x + 8, y, Kind::LongArm, Dir::E).unwrap();
        w.place(0, x + 10, y - 1, Kind::Warehouse, Dir::E).unwrap();
        w.place(0, x + 4, y + 2, Kind::Pole, Dir::E).unwrap();
        w.place(0, x + 7, y + 2, Kind::Solar, Dir::E).unwrap();
        let mut app = App::new(w);
        app.cursor = (x + 1, y);
        app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
        let buf = render(&mut app, 130, 42);
        let t = text(&buf);
        assert!(t.contains(">]") && t.contains("[>") && t.contains("WARE"), "{t}");
        assert!(t.contains("goes under 3 tiles to its exit\n") || t.contains("goes under 3 tiles to its exit "), "{t}");
        std::fs::write("target/snapshot-toys.html", to_html(&buf)).unwrap();
    }

    /// Long machines on the map in every facing, and the placement ghost turning with the rotate key.
    #[test]
    fn long_machines_snapshot() {
        let mut w = World::blank();
        let (x, y) = (52, 36);
        for (i, d) in Dir::ALL.into_iter().enumerate() {
            let i = i as i32;
            w.place(0, x + i * 5, y, Kind::Press, d).unwrap();
            w.place(0, x + i * 5, y + 4, Kind::Pump, d).unwrap();
            w.place(0, x + i * 5, y + 8, Kind::Furnace, d).unwrap();
        }
        w.place(0, x + 14, y + 13, Kind::Solar, Dir::E).unwrap();
        let mut app = App::new(w);
        app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
        app.menu = Some(Category::Create);
        app.selected = Some(Kind::Press);
        app.dir = Dir::E;
        app.cursor = (x, y + 13);
        app.cam = (x - 4, y - 2);
        app.cam_ready = true;
        let t = text(&render(&mut app, 130, 42));
        assert!(t.contains("PR") && t.contains("PUMP") && t.contains("FURN"), "{t}");
        assert!(text(&render(&mut app, 64, 16)).contains("Copper press  $150  2x1  facing"));
        app.cam = (x - 4, y - 2);
        let east = render(&mut app, 130, 42);
        // Turned north the ghost is one wide and two tall, and the label says so.
        app.dir = Dir::N;
        app.cursor = (x + 6, y + 13);
        app.cam = (x - 4, y - 2);
        let north = render(&mut app, 130, 42);
        assert!(text(&render(&mut app, 64, 16)).contains("Copper press  $150  1x2  facing"));
        std::fs::write("target/snapshot-long-machines.html", to_html(&east)).unwrap();
        std::fs::write("target/snapshot-long-machines-north.html", to_html(&north)).unwrap();
        // A narrow, tall pane keeps the map readable.
        app.cursor = (x + 1, y + 1);
        let narrow = render(&mut app, 44, 30);
        std::fs::write("target/snapshot-long-machines-narrow.html", to_html(&narrow)).unwrap();
        // Hovering a placed long machine highlights the whole rectangle.
        app.selected = None;
        app.menu = None;
        app.cursor = (x + 6, y);
        app.cam = (x - 4, y - 2);
        let hover = render(&mut app, 130, 42);
        std::fs::write("target/snapshot-long-machines-hover.html", to_html(&hover)).unwrap();
    }

    /// Middle-click copies the part under the cursor: its kind, facing and setting.
    #[test]
    fn middle_click_copies() {
        use crate::sim::{Dir, Setting};
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        let mut w = World::new();
        w.econ.money = 1e6;
        let mut app = App::new(w);
        render(&mut app, 130, 42);
        let (cx, cy) = app.cam;
        let (a, b) = ((cx + 4, cy + 4), (cx + 8, cy + 4));
        app.world.place(0, a.0, a.1, Kind::Arm, Dir::S).unwrap();
        app.world.configure(0, a.0, a.1);
        let at = |app: &App, t: (i32, i32)| (app.map_area.x + ((t.0 - app.cam.0) * 2) as u16, app.map_area.y + (t.1 - app.cam.1) as u16);
        let click = |app: &mut App, t: (i32, i32), button| {
            let (column, row) = at(app, t);
            app.on_mouse(MouseEvent { kind: MouseEventKind::Down(button), column, row, modifiers: KeyModifiers::NONE });
        };
        click(&mut app, a, MouseButton::Middle);
        assert_eq!((app.selected, app.dir), (Some(Kind::Arm), Dir::S));
        assert_eq!(app.menu, Some(Category::Transport));
        assert!(app.msg.contains("Copied Filtering arm"), "{}", app.msg);
        click(&mut app, b, MouseButton::Left);
        assert_eq!(app.world.tile(0, b.0, b.1).map(|t| t.dir), Some(Dir::S));
        assert_eq!(app.world.setting_at(0, b.0, b.1), app.world.setting_at(0, a.0, a.1));
        assert!(matches!(app.world.setting_at(0, b.0, b.1), Some(Setting::Filter(Some(_)))));
        // Middle-click on an empty tile empties your hand.
        click(&mut app, (cx + 4, cy + 8), MouseButton::Middle);
        assert_eq!(app.selected, None);
    }

    #[test]
    fn small_pane_view() {
        let mut w = World::new();
        w.build_demo().unwrap();
        for _ in 0..TPS * 30 {
            w.tick();
        }
        let mut app = App::new(w);
        app.cursor = (58, 45);
        app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
        let buf = render(&mut app, 64, 16);
        let t = text(&buf);
        assert!(app.compact_now);
        assert!(t.lines().next().unwrap().contains(" $"), "{t}");
        assert!(!t.contains("Factory"), "no side panel in a small pane");
        assert_eq!(app.map_area, Rect::new(0, 1, 64, 14));
        std::fs::write("target/snapshot-small.html", to_html(&buf)).unwrap();

        app.menu = Some(Category::Create);
        let t = text(&render(&mut app, 64, 16));
        assert!(t.lines().last().unwrap().contains(" Create › 1 Cheap drill+ $50"), "{t}");

        // A picked part: the path to it, its price and facing.
        app.menu = Some(Category::Create);
        app.selected = Some(Kind::Furnace);
        let buf = render(&mut app, 64, 16);
        assert!(text(&buf).lines().last().unwrap().contains("Create › Copper furnace  $200  3x3  facing"));
        std::fs::write("target/snapshot-small-menu.html", to_html(&buf)).unwrap();
        app.menu = Some(Category::Transport);
        app.selected = Some(Kind::Splitter);
        let t = text(&render(&mut app, 64, 16));
        let last = t.lines().last().unwrap().to_string();
        assert!(last.starts_with(" Transport › Basic splitter  $10  ·  Space place"), "{last}");
        // A message keeps the path in front of it.
        app.say("Placed");
        let t = text(&render(&mut app, 64, 16));
        assert!(t.lines().last().unwrap().contains("Transport › Basic splitter  │ Placed"), "{t}");
        app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
        app.selected = None;

        // The power figure is global away from power, regional on a pole or panel.
        assert!(t.lines().next().unwrap().contains(" global"), "{t}");
        let pole = (0..(W * H) as usize).map(pos_of).find(|&(x, y)| app.world.tile(0, x, y).is_some_and(|b| b.kind == Kind::Pole)).unwrap();
        app.cursor = pole;
        let t = text(&render(&mut app, 64, 16));
        assert!(t.lines().next().unwrap().contains(" regional"), "{t}");
        app.cursor = (58, 45);

        // m forces the full view back, even in a small terminal.
        app.menu = None;
        app.compact = Some(false);
        render(&mut app, 64, 16);
        assert!(!app.compact_now);
        // A big terminal uses the full view by itself.
        app.compact = None;
        let t = text(&render(&mut app, 130, 42));
        assert!(t.contains("Factory") && !app.compact_now);

        app.help = true;
        let t = text(&render(&mut app, 64, 16));
        assert!(t.contains("scroll 0/"), "{t}");
    }

    /// A small steam plant and a blast furnace, for checking how pipes and the new machines look.
    #[test]
    fn render_steam_snapshot() {
        let mut w = World::blank();
        w.place(0, 6, 5, Kind::Pump, Dir::E).unwrap();
        for x in 7..=9 {
            w.place(0, x, 5, Kind::Pipe, Dir::E).unwrap();
        }
        w.place(0, 9, 6, Kind::Pipe, Dir::E).unwrap();
        w.place(0, 10, 6, Kind::Boiler, Dir::E).unwrap();
        w.place(0, 12, 6, Kind::SteamEngine, Dir::E).unwrap();
        w.place(0, 10, 8, Kind::Crate, Dir::N).unwrap();
        w.place(0, 4, 7, Kind::Solar, Dir::E).unwrap();
        w.place(0, 16, 6, Kind::BlastFurnace, Dir::E).unwrap();
        for x in 14..=15 {
            w.place(0, x, 9, Kind::Belt, Dir::E).unwrap();
        }
        if let Some(Building { st: St::Crate { items }, .. }) = w.floors[0].tiles[idx(10, 8)].as_mut() {
            items.extend([Item::Coal; 20]);
        }
        for _ in 0..TPS * 5 {
            w.tick();
        }
        let mut app = App::new(w);
        app.cursor = (20, 12);
        let buf = render(&mut app, 130, 42);
        assert!(text(&buf).contains("ENGN"));
        std::fs::write("target/snapshot-steam.html", to_html(&buf)).unwrap();
    }

    /// Phase 3 screenshots: box elevators, idle words, floors above and below, the away summary.
    #[test]
    fn render_phase3_snapshots() {
        // Elevators of three sizes, a feeding belt, and idle machines.
        let mut w = World::blank();
        w.place(0, 3, 7, Kind::Crate, Dir::E).unwrap();
        if let Some(Building { st: St::Crate { items }, .. }) = w.floors[0].tiles[idx(3, 7)].as_mut() {
            items.extend([Item::Coal; 30]);
        }
        w.place(0, 4, 7, Kind::Belt, Dir::E).unwrap();
        w.place(0, 5, 7, Kind::Belt, Dir::E).unwrap();
        w.place_sized(0, 6, 6, Kind::Elevator, Dir::E, 2).unwrap();
        w.place_sized(0, 11, 4, Kind::Elevator, Dir::E, 4).unwrap();
        w.place_sized(0, 18, 3, Kind::Elevator, Dir::S, 6).unwrap();
        w.place(0, 4, 4, Kind::Solar, Dir::E).unwrap();
        w.place(0, 4, 11, Kind::Drill, Dir::E).unwrap();
        w.place(0, 8, 11, Kind::Furnace, Dir::E).unwrap();
        for _ in 0..TPS * 6 + TPS / 2 {
            w.tick();
        }
        // Send the 4x4 car off, full, so it shows as moving.
        w.lifts[1].load.extend([Item::IronPlate; 160]);
        for _ in 0..40 {
            w.tick();
        }
        while (w.tick / TPS) % 2 == 0 {
            w.tick(); // land on a second where idle words show
        }
        let mut app = App::new(w);
        app.cursor = (16, 17);
        app.menu = Some(Category::Transport);
        app.selected = Some(Kind::Elevator);
        app.lift_size = 3;
        let buf = render(&mut app, 130, 42);
        std::fs::write("target/snapshot-lifts.html", to_html(&buf)).unwrap();

        // The same spot on the surface and one floor down: ore continues underneath, wider.
        let mut w = World::new();
        w.progress.researched[Tech::Excavation.idx()] = true;
        let mut app = App::new(w);
        app.cursor = (60, 38);
        let buf = render(&mut app, 130, 42);
        std::fs::write("target/snapshot-surface.html", to_html(&buf)).unwrap();
        app.floor = 1;
        let buf = render(&mut app, 130, 42);
        assert!(text(&buf).contains("underground"));
        std::fs::write("target/snapshot-underground.html", to_html(&buf)).unwrap();

        // Coming back after a while.
        app.floor = 0;
        app.away = Some("You were away for 3h 12m. Your factory kept going:\n\n  +$4210\n  +318.5 research".into());
        let buf = render(&mut app, 130, 42);
        std::fs::write("target/snapshot-away.html", to_html(&buf)).unwrap();

        // The research screen, picked on its last row.
        app.away = None;
        app.tree = true;
        app.tree_pick = tree_rows().len() - 1;
        let buf = render(&mut app, 120, 42);
        assert!(text(&buf).contains("Excavation · "), "{}", text(&buf));
        std::fs::write("target/snapshot-tree-end.html", to_html(&buf)).unwrap();
    }

    #[test]
    fn belt_corners_draw_as_bends() {
        let mut w = World::blank();
        // East along y=5, then up at x=8: (8,5) is a corner.
        for x in 5..=7 {
            w.place(0, x, 5, Kind::Belt, Dir::E).unwrap();
        }
        w.place(0, 8, 5, Kind::Belt, Dir::N).unwrap();
        w.place(0, 8, 4, Kind::Belt, Dir::N).unwrap();
        let chars = |w: &World, x, y| tile_glyph(w, 0, x, y).map(|c| c.0);
        assert_eq!(chars(&w, 8, 5), ['/', '^']);
        assert_eq!(chars(&w, 7, 5), ['>', '>']);
        assert_eq!(chars(&w, 8, 4), ['^', '^']);
        // Coming from the right and turning down.
        w.place(0, 20, 5, Kind::Belt, Dir::W).unwrap();
        w.place(0, 19, 5, Kind::Belt, Dir::S).unwrap();
        assert_eq!(chars(&w, 19, 5), ['v', '/']);
        // A belt fed from behind and from the left is a merge: the join shows on the left.
        w.place(0, 8, 6, Kind::Belt, Dir::N).unwrap();
        assert_eq!(chars(&w, 8, 5), ['+', '^']);
        // And from the right as well: joins on both sides.
        w.place(0, 9, 5, Kind::Belt, Dir::W).unwrap();
        assert_eq!(chars(&w, 8, 5), ['+', '+']);
        // A sideways belt joined from above shows the join on its back half.
        w.place(0, 30, 10, Kind::Belt, Dir::E).unwrap();
        w.place(0, 31, 10, Kind::Belt, Dir::E).unwrap();
        w.place(0, 31, 9, Kind::Belt, Dir::S).unwrap();
        assert_eq!(chars(&w, 31, 10), ['+', '>']);
    }

    fn hex(c: Option<Color>, default: &str) -> String {
        let named = |i: u8| -> &'static str {
            ["000000", "cd3131", "0dbc79", "e5e510", "2472c8", "bc3fbc", "11a8cd", "e5e5e5", "666666", "f14c4c",
             "23d18b", "f5f543", "3b8eea", "d670d6", "29b8db", "ffffff"][i as usize]
        };
        match c {
            None | Some(Color::Reset) => default.to_string(),
            Some(Color::Black) => named(0).into(),
            Some(Color::Red) => named(1).into(),
            Some(Color::Green) => named(2).into(),
            Some(Color::Yellow) => named(3).into(),
            Some(Color::Blue) => named(4).into(),
            Some(Color::Magenta) => named(5).into(),
            Some(Color::Cyan) => named(6).into(),
            Some(Color::Gray) => named(7).into(),
            Some(Color::DarkGray) => named(8).into(),
            Some(Color::LightRed) => named(9).into(),
            Some(Color::LightGreen) => named(10).into(),
            Some(Color::LightYellow) => named(11).into(),
            Some(Color::LightBlue) => named(12).into(),
            Some(Color::LightMagenta) => named(13).into(),
            Some(Color::LightCyan) => named(14).into(),
            Some(Color::White) => named(15).into(),
            Some(Color::Indexed(i)) if i < 16 => named(i).into(),
            Some(Color::Indexed(i)) if i < 232 => {
                let l = [0, 95, 135, 175, 215, 255];
                let n = i - 16;
                format!("{:02x}{:02x}{:02x}", l[(n / 36) as usize], l[((n / 6) % 6) as usize], l[(n % 6) as usize])
            }
            Some(Color::Indexed(i)) => {
                let g = 8 + 10 * (i - 232);
                format!("{g:02x}{g:02x}{g:02x}")
            }
            Some(Color::Rgb(r, g, b)) => format!("{r:02x}{g:02x}{b:02x}"),
        }
    }

    /// Colour snapshot of the screen as HTML, for checking the look in a browser.
    fn to_html(buf: &Buffer) -> String {
        let mut s = String::from("<html><head><meta charset=utf-8></head><body style='background:#1e1e1e;margin:16px'><pre style='font:14px/1.15 Menlo,monospace'>");
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let c = buf.cell((x, y)).unwrap();
                let (mut fg, mut bg) = (hex(Some(c.fg), "cccccc"), hex(Some(c.bg), "1e1e1e"));
                if c.modifier.contains(Modifier::REVERSED) {
                    std::mem::swap(&mut fg, &mut bg);
                }
                let w = if c.modifier.contains(Modifier::BOLD) { "bold" } else { "normal" };
                let sym = match c.symbol() { "<" => "&lt;", ">" => "&gt;", "&" => "&amp;", o => o };
                s.push_str(&format!("<span style='color:#{fg};background:#{bg};font-weight:{w}'>{sym}</span>"));
            }
            s.push('\n');
        }
        s.push_str("</pre></body></html>");
        s
    }
}

