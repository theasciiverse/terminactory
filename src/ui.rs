//! Terminal drawing. Plain ASCII on a fixed dark map,
//! buildings as labelled coloured blocks (1x1, 2x2, 3x3), belts as arrows.
//! Every map tile is two terminal cells wide so the grid looks square.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};

use crate::app::{slots, tree_rows, App, AreaStage, AreaVerb, Entry, StatsHit, BOOK_PARTS};
use crate::controls::Action;
use crate::settings::TelegramStyle;
use crate::sim::*;
use std::collections::HashSet;
use std::time::Instant;

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
/// Research, wherever it's shown: a sky blue that reads on dark and light terminals alike.
const RESEARCH: Color = Color::Indexed(75);
/// Research that is out of reach or nothing yet: the same blue, softer, still readable.
const RESEARCH_DIM: Color = Color::Indexed(67);

/// Below this size the game switches to the small-pane view on its own.
pub const SMALL_W: u16 = 120;
pub const SMALL_H: u16 = 30;
/// The full view needs the sidebar and ten columns of map, and a dozen rows. Smaller than this
/// it can't draw, so even "Never" (small-pane view) falls back to the compact view.
pub const FULL_MIN_W: u16 = SIDE_W + 10;
pub const FULL_MIN_H: u16 = 12;

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let compact = app.compact.unwrap_or_else(|| app.settings.small_pane(area.width, area.height));
    app.compact_now = compact;
    let [top, mid, bottom] = Layout::vertical([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)]).areas(area);
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
        app.graph_areas.clear();
        let side = if app.mail.is_some() && app.settings.telegram_style == TelegramStyle::Sidebar {
            let room = side.height.saturating_sub(12).max(side.height / 2);
            let h = mail_height(app, side.width).min(room).max(8.min(side.height));
            let [panel, mail] = Layout::vertical([Constraint::Min(1), Constraint::Length(h)]).areas(side);
            app.mail_area = mail;
            draw_mail_in(f, mail, app, false);
            panel
        } else {
            side
        };
        // Charts go under the build menu with whatever room it leaves; they never take any of it.
        let (panel, graphs) = graph::split(app, side);
        draw_side(f, panel, app, side.height);
        graph::draw(f, &graphs, app);
        draw_bottom(f, bottom, app);
    }
    if app.log_open {
        maillog::draw_log(f, app.map_area, app);
    }
    if app.settings_open {
        settings::draw_settings(f, app.map_area, app);
    }
    if app.library.is_some() {
        library::draw_library(f, app.map_area, app);
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
    if let Some(text) = &app.away {
        // Wrapped to the pane, and sized from the wrapped text.
        let width = 62.min(f.area().width);
        let inner_w = width.saturating_sub(4).max(10) as usize;
        let mut lines: Vec<String> = vec![String::new()];
        for l in text.lines() {
            let indent = l.len() - l.trim_start().len();
            for (i, part) in wrap_words(l.trim_start(), inner_w.saturating_sub(indent).max(8)).into_iter().enumerate() {
                lines.push(format!(" {}{part}", " ".repeat(if i == 0 { indent } else { indent + 1 })));
            }
        }
        lines.push(String::new());
        lines.push(" press any key".to_string());
        let r = centered(f.area(), width, lines.len() as u16 + 2);
        f.render_widget(Clear, r);
        let head = Style::new().fg(KEY_ITEM).add_modifier(Modifier::BOLD);
        f.render_widget(Paragraph::new(lines.join("\n")).block(Block::bordered().title(" While you were away ").title_style(head)), r);
    }
}

pub fn floor_label(level: i32) -> String {
    match level {
        0 => "Earth · ground".into(),
        l if l < 0 => format!("Earth · {l} (underground, ore x{})", richness(l)),
        l => format!("Earth · +{l} (upper floor)"),
    }
}

/// "EC 20/12": the grid under the cursor (labelled "regional") when it's on a power
/// source or a powered machine, otherwise every grid together ("global").
fn power_spans(app: &App, label: &str) -> Vec<Span<'static>> {
    let (main, word, unused) = power_parts(app, label, false);
    std::iter::once(main).chain(word).chain(unused).collect()
}

/// The figure, the word after it ("regional" or "global") and the unused-supply note. A
/// small pane leaves "global" out: it is what you get unless it says "regional".
fn power_parts(app: &App, label: &str, compact: bool) -> (Span<'static>, Option<Span<'static>>, Option<Span<'static>>) {
    let w = &app.world;
    let colour = |r: f64| {
        if r >= 1.0 {
            Color::Green
        } else if r >= 0.5 {
            Color::Yellow
        } else {
            Color::Red
        }
    };
    let dim = Style::new().fg(Color::DarkGray);
    let here = if app.selected.is_none() { w.grid_at(app.floor, app.cursor.0, app.cursor.1) } else { None };
    match here {
        Some(id) => {
            let n = w.grid.nets[id];
            let ratio = if n.demand > 0.0 { n.ratio } else { 1.0 };
            (
                Span::styled(format!("{label}{:.0}/{:.0}", n.supply, n.demand), Style::new().fg(colour(ratio))),
                Some(Span::styled(" regional", dim)),
                None,
            )
        }
        None => {
            let (supply, demand, worst, _, _, unused) = w.power_summary();
            (
                Span::styled(format!("{label}{supply:.0}/{demand:.0}"), Style::new().fg(colour(worst))),
                (!compact).then(|| Span::styled(" global", dim)),
                (unused > 0.0).then(|| Span::styled(format!(" +{unused:.0} unused"), dim)),
            )
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
        Span::styled(format!(" {}", signed_money(w.econ.money)), Style::new().fg(Color::Green).add_modifier(bold)),
        Span::raw("   "),
        Span::styled(format!("Research {:.1}", w.econ.research), Style::new().fg(RESEARCH).add_modifier(bold)),
        Span::raw("   "),
    ];
    spans.extend(power_spans(app, "EC "));
    spans.extend([
        Span::styled(if unpowered > 0 { format!("  {unpowered} no EC") } else { String::new() }, Style::new().fg(Color::Indexed(214))),
        Span::raw("   "),
        Span::styled(format!("{:02}:{:02}", secs / 60, secs % 60), Style::new().fg(Color::DarkGray)),
        Span::raw("   "),
    ]);
    if app.paused {
        spans.push(Span::styled(" PAUSED ", Style::new().fg(Color::Black).bg(Color::Yellow)));
    } else if app.speed > 1 {
        spans.push(Span::styled(format!("{}x speed", app.speed), Style::new().fg(Color::Yellow)));
    }
    // The Ticker telegram style: just the count, here.
    if let Some(m) = mail_count(app) {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(m, Style::new().fg(KEY_ITEM)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Small-pane top line, by priority so a narrow pane drops the least useful bits from the
/// right end of the list rather than the end of the line: PAUSED or speed, the floor, mail,
/// money, EC, unpowered machines, research, idle machines, and why they are idle.
fn draw_ticker(f: &mut Frame, area: Rect, app: &App) {
    let w = &app.world;
    let (_, _, _, _, unpowered, _) = w.power_summary();
    let bold = Modifier::BOLD;
    let warn = Style::new().fg(Color::Indexed(214));
    // (priority, spans) in the order they show; a bigger priority number goes first when short of room.
    let mut parts: Vec<(u8, Vec<Span<'static>>)> = Vec::new();
    if app.paused {
        parts.push((0, vec![Span::styled(" PAUSED ", Style::new().fg(Color::Black).bg(Color::Yellow).add_modifier(bold))]));
    } else if app.speed > 1 {
        parts.push((0, vec![Span::styled(format!(" {}x ", app.speed), Style::new().fg(Color::Black).bg(Color::Yellow))]));
    }
    let level = w.floors[app.floor].level;
    if level != 0 {
        parts.push((1, vec![Span::styled(format!(" floor {level:+}"), Style::new().fg(Color::Cyan))]));
    }
    let mail = w.mail.len() + app.mail.is_some() as usize;
    if mail > 0 {
        let text = mail_count(app).map_or_else(|| format!(" {mail} mail"), |m| format!(" {m}"));
        parts.push((2, vec![Span::styled(text, Style::new().fg(KEY_ITEM))]));
    }
    parts.push((3, vec![Span::styled(format!(" {}", short_money(w.econ.money)), Style::new().fg(Color::Green).add_modifier(bold))]));
    let (power, place, unused) = power_parts(app, " EC ", true);
    parts.push((4, vec![power]));
    if let Some(p) = place {
        parts.push((9, vec![p]));
    }
    if let Some(u) = unused {
        parts.push((9, vec![u]));
    }
    if unpowered > 0 {
        parts.push((5, vec![Span::styled(format!(" {unpowered} no EC"), warn)]));
    }
    parts.push((6, vec![Span::styled(format!(" R {:.1}", w.econ.research), Style::new().fg(RESEARCH).add_modifier(bold))]));
    let (idle, why) = w.idle_summary();
    if idle > 0 {
        parts.push((7, vec![Span::styled(format!(" {idle} idle"), warn)]));
        if let Some(r) = why {
            parts.push((8, vec![Span::styled(format!(" ({r})"), warn)]));
        }
    }
    let width = |p: &(u8, Vec<Span<'static>>)| p.1.iter().map(|s| s.content.chars().count()).sum::<usize>();
    // Drop the least important until it fits (the first part is never dropped).
    while parts.iter().map(width).sum::<usize>() > area.width as usize && parts.len() > 1 {
        let worst = parts.iter().enumerate().max_by_key(|(i, p)| (p.0, *i)).map(|(i, _)| i).unwrap_or(0);
        parts.remove(worst);
    }
    // PAUSED and the speed block sit at the left edge, with the rest after a space.
    let spans: Vec<Span<'static>> = parts.into_iter().flat_map(|p| p.1).collect();
    f.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(Color::Indexed(236))), area);
}

/// What the build bar calls a part when its full name won't fit: the short label the
/// map draws on the block.
fn map_label(k: Kind) -> &'static str {
    match k {
        Kind::Drill => "DRIL",
        Kind::BasicDrill => "DRL2",
        Kind::AdvancedDrill => "ADRL",
        Kind::LargeBasicDrill => "LBDR",
        Kind::LargeAdvancedDrill => "LADR",
        Kind::Belt => "BELT",
        Kind::FastBelt => "FAST",
        Kind::Tunnel => "TUNL",
        Kind::Splitter => "SPLT",
        Kind::Sorter => "SORT",
        Kind::Arm => "ARM",
        Kind::LongArm => "LARM",
        Kind::Furnace => "FURN",
        Kind::Molder => "MOLD",
        Kind::BlastFurnace => "BLST",
        Kind::CokeOven => "COKE",
        Kind::Kiln => "KILN",
        Kind::LargeKiln => "LKLN",
        Kind::Press => "PRES",
        Kind::WireDrawer => "WIRE",
        Kind::Winder => "WIND",
        Kind::RollingMill => "ROLL",
        Kind::Lathe => "LATH",
        Kind::Crusher => "CRSH",
        Kind::CrudeAssembler => "CRUD",
        Kind::BasicAssembler => "BASM",
        Kind::AdvancedAssembler => "ADVA",
        Kind::PrecisionAssembler => "PREC",
        Kind::LargeBasicAssembler => "LRGB",
        Kind::LargeAdvancedAssembler => "LRGA",
        Kind::LargePrecisionAssembler => "LRGP",
        Kind::Welder => "WELD",
        Kind::Pump => "PUMP",
        Kind::AdvancedPump => "APMP",
        Kind::OilExtractor => "OILX",
        Kind::LiquidTrader => "TRDR",
        Kind::Still => "STIL",
        Kind::Pipe => "PIPE",
        Kind::Mixer => "MIXR",
        Kind::Evacuator => "EVAC",
        Kind::Etcher => "ETCH",
        Kind::ArcFurnace => "ARC",
        Kind::CrystalGrower => "GROW",
        Kind::WaferSaw => "SAW",
        Kind::FineEtcher => "FETC",
        Kind::Solar => "SOLR",
        Kind::BasicSolar => "SOL2",
        Kind::Pole => "POLE",
        Kind::Boiler => "BOIL",
        Kind::SteamEngine => "ENGN",
        Kind::Elevator => "LIFT",
        Kind::Crate => "CRAT",
        Kind::Warehouse => "WARE",
        Kind::TradePost => "TRAD",
    }
}

/// A part's name with "advanced" shortened, for tight spots.
fn short_name(k: Kind) -> String {
    k.name().replace("advanced", "adv.").replace("Advanced", "Adv.")
}

/// How the open category's slots show in the status line, at the richest level that fits `room`:
/// names with prices, names, map labels, then two-letter codes. A short "Esc back" ends the
/// line when there are columns to spare.
fn category_spans(app: &App, cat: Category, room: usize) -> Vec<Span<'static>> {
    let dim = Style::new().fg(Color::DarkGray);
    let w = &app.world;
    let entries: Vec<(usize, Kind, bool, Style)> = slots(cat)
        .iter()
        .enumerate()
        .map(|(n, slot)| {
            let k = app.selected.filter(|k| slot.contains(k)).unwrap_or(slot[0]);
            let unlocked = w.progress.part_unlocked(k);
            let mut st = if unlocked && w.econ.money >= k.cost() { Style::new() } else { dim };
            if app.selected == Some(k) {
                st = st.add_modifier(Modifier::REVERSED);
            }
            (n + 1, k, slot.len() > 1, st)
        })
        .collect();
    let bold = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let code = |k: Kind| -> String { kind_cells(k, Dir::E, true).iter().map(|c| c.0).collect() };
    // (crumb, entry text, gap between entries) per level.
    let level = |lvl: usize| -> (String, Vec<String>, usize) {
        let crumb =
            if lvl == 4 { format!(" {}›", cat.name().chars().take(2).collect::<String>()) } else { format!(" {} ›", cat.name()) };
        let texts = entries
            .iter()
            .map(|(n, k, more, _)| {
                let plus = if *more { "+" } else { "" };
                match lvl {
                    0 => format!(" {n} {}{plus} {} ", k.name(), price_label(w, *k, app.lift_size)),
                    1 => format!(" {n} {}{plus} ", k.name()),
                    2 => format!(" {n} {}{plus} ", map_label(*k)),
                    3 => format!(" {n} {} ", map_label(*k)),
                    _ => format!("{n}{}", code(*k)),
                }
            })
            .collect();
        (crumb, texts, if lvl == 4 { 1 } else { 0 })
    };
    let width = |lvl: usize| -> usize {
        let (crumb, texts, gap) = level(lvl);
        let lead = if lvl == 4 { 1 } else { 0 };
        crumb.chars().count() + lead + texts.iter().map(|t| t.chars().count()).sum::<usize>() + gap * texts.len().saturating_sub(1)
    };
    let lvl = (0..5).find(|l| width(*l) <= room).unwrap_or(4);
    let (crumb, texts, gap) = level(lvl);
    let mut spans = vec![Span::styled(crumb, bold)];
    if lvl == 4 {
        spans.push(Span::raw(" "));
    }
    for (i, (text, (_, _, _, st))) in texts.into_iter().zip(&entries).enumerate() {
        if i > 0 && gap > 0 {
            spans.push(Span::raw(" ".repeat(gap)));
        }
        spans.push(Span::styled(text, *st));
    }
    // The way out stays on the line: "Esc back" when there is room, else just the key.
    let key = app.controls.key(Action::Back);
    let back = format!("  {key} back");
    let bare = format!(" {key}");
    if room >= width(lvl) + back.chars().count() {
        spans.push(Span::styled(back, dim));
    } else if room >= width(lvl) + bare.chars().count() {
        spans.push(Span::styled(bare, dim));
    }
    spans
}

/// The picked part in the status line: price (or "locked") and facing matter most, so a long
/// name is shortened, and the keys go before the numbers do.
fn part_spans(app: &App, cat: Category, k: Kind, room: usize) -> Vec<Span<'static>> {
    let dim = Style::new().fg(Color::DarkGray);
    let bold = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let c = &app.controls;
    let w = &app.world;
    let afford = w.progress.part_unlocked(k) && w.econ.money >= k.price(app.lift_size);
    let price = price_label(w, k, app.lift_size);
    let price_st = if afford { Style::new().fg(Color::Green) } else { dim };
    let (fw, fh) = dims_of(k, app.dir, app.lift_size);
    let size = if fw * fh > 1 { format!("{fw}x{fh}") } else { String::new() };
    let rot = c.key(Action::Rotate);
    // Levels, from everything down to price and short name: (category, keys, name short, price first, size, rotate key)
    let levels: [(bool, u8, bool, bool, bool, bool); 7] = [
        (true, 2, false, false, true, true),
        (true, 1, false, false, true, true),
        (true, 0, false, false, true, true),
        (false, 1, false, false, true, true),
        (false, 0, false, false, true, true),
        (false, 0, true, true, true, false),
        (false, 0, true, true, false, false),
    ];
    let build = |lvl: usize, cut: Option<usize>| -> Vec<Span<'static>> {
        let (with_cat, keys, short, price_first, with_size, with_rot) = levels[lvl];
        let mut spans =
            if with_cat { vec![Span::styled(format!(" {}", cat.name()), bold), Span::styled(" › ", dim)] } else { vec![Span::raw(" ")] };
        let name = if short { short_name(k) } else { k.name().to_string() };
        let name = match cut {
            Some(n) => fit(&name, n),
            None => name,
        };
        let name_span = Span::styled(name, Style::new().add_modifier(Modifier::BOLD));
        let price_span = Span::styled(price.clone(), price_st);
        if price_first {
            spans.push(price_span);
            spans.push(Span::raw(" "));
            spans.push(name_span);
        } else {
            spans.push(name_span);
            spans.push(Span::raw("  "));
            spans.push(price_span);
        }
        if with_size && !size.is_empty() {
            spans.push(Span::styled(format!("  {size}"), dim));
        }
        if k.directional() {
            spans.push(Span::raw(format!("  facing {}", app.dir.arrow())));
            if with_rot {
                spans.push(Span::styled(format!(" ({rot})"), dim));
            }
        }
        match keys {
            2 => spans.push(Span::styled(format!("  ·  {} place  {} back", c.key(Action::Place), c.key(Action::Back)), dim)),
            1 => spans.push(Span::styled(format!("  ·  {} place", c.key(Action::Place)), dim)),
            _ => {}
        }
        spans
    };
    let width = |s: &[Span<'static>]| s.iter().map(|s| s.content.chars().count()).sum::<usize>();
    for lvl in 0..levels.len() {
        let spans = build(lvl, None);
        if width(&spans) <= room {
            return spans;
        }
    }
    // Even price and short name are too wide: cut the name.
    let last = levels.len() - 1;
    let over = width(&build(last, None)).saturating_sub(room);
    let name_len = short_name(k).chars().count();
    build(last, Some(name_len.saturating_sub(over).max(4)))
}

/// Room for a fresh message beside the build line: (room for the build line, the message).
/// The build line keeps at least `least` columns (and half the line); the message gets the
/// rest, cut with "…", or nothing at all if that leaves under 10 columns.
fn message_beside(app: &App, avail: usize, least: usize) -> (usize, Option<Span<'static>>) {
    if app.msg_at.elapsed().as_secs() >= 6 {
        return (avail, None);
    }
    let want = app.msg.chars().count() + 3;
    let spare = avail.saturating_sub(least);
    let msg_w = if want <= spare { want } else { want.min(avail / 2).min(spare) };
    if msg_w < 12 && msg_w < want {
        return (avail, None);
    }
    (avail - msg_w, Some(Span::styled(format!(" │ {}", fit(&app.msg, msg_w - 3)), Style::new().fg(Color::Yellow))))
}

/// The first line of `inspect` that says something is wrong, if any.
fn problem_line(lines: &[String]) -> Option<&String> {
    lines.iter().skip(1).find(|l| {
        ["NO EC", "needs ", "waiting", "idle:", "Won't take", "no filter", "put its exit"].iter().any(|p| l.starts_with(p))
            || l.contains("no fire")
            || l.contains("needs coal")
    })
}

/// What's under the cursor for the status line: what is wrong first, then the name and the
/// state, then the output side, with the EC figures last and no lore ("by Company": the
/// book has it).
fn inspect_short(w: &World, f: usize, x: i32, y: i32) -> Vec<String> {
    let lines = inspect(w, f, x, y);
    let problem = problem_line(&lines).cloned();
    let mut out: Vec<String> = problem.iter().cloned().collect();
    let (mut side, mut power) = (Vec::new(), Vec::new());
    for l in lines {
        if l.starts_with("by ") || Some(&l) == problem.as_ref() {
            continue;
        }
        if l.starts_with("output goes out") {
            side.push(l);
        } else if l.starts_with("EC ")
            || l.starts_with("grid:")
            || l.starts_with("draws ")
            || l.starts_with("drawing ")
            || l.contains("EC only while")
        {
            power.push(l);
        } else {
            out.push(l);
        }
    }
    out.extend(side);
    out.extend(power);
    out
}

/// Small-pane bottom line, most useful first: the open build menu (with a fresh message
/// beside it, never over it), what's under the cursor, or the main keys.
fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    let dim = Style::new().fg(Color::DarkGray);
    let c = &app.controls;
    let note = if app.settings.telegram_style == TelegramStyle::Ticker { None } else { mail_note(app, area.width) };
    let avail = (area.width as usize).saturating_sub(note.as_ref().map_or(0, |n| n.chars().count()));
    let spans_width = |s: &[Span<'static>]| s.iter().map(|s| s.content.chars().count()).sum::<usize>();
    let slot = status_slot(app, Instant::now());
    let line = if slot == Slot::Area {
        area_line(app, avail)
    } else if slot == Slot::Hand {
        hand_line(app, avail)
    } else if slot == Slot::Menu {
        let cat = app.menu.unwrap_or(Category::Create);
        // The part list or the picked part always shows: a message gets what is left.
        let tightest = match app.selected {
            Some(k) => spans_width(&part_spans(app, cat, k, 0)),
            None => spans_width(&category_spans(app, cat, 0)),
        };
        let (room, msg) = message_beside(app, avail, tightest);
        let mut spans = match app.selected {
            Some(k) => part_spans(app, cat, k, room),
            None => category_spans(app, cat, room),
        };
        spans.extend(msg);
        Line::from(spans)
    } else {
        match slot {
            Slot::Message => Line::styled(fit(&format!(" {}", app.msg), avail), Style::new().fg(Color::Yellow)),
            Slot::Here => {
                let (x, y) = app.cursor;
                let here = inspect_short(&app.world, app.floor, x, y);
                Line::raw(fit(&format!(" {}", here.iter().take(3).cloned().collect::<Vec<_>>().join(" · ")), avail))
            }
            Slot::Hint(t) | Slot::Aside(t) => hint_line(&t, avail),
            _ => {
                let (help, tree, book, stats) = (c.key(Action::Help), c.key(Action::Tree), c.key(Action::Book), c.key(Action::Stats));
                let hints = [
                    (0, format!("{help} help")),
                    (1, "1-4 build".to_string()),
                    (2, format!("{tree} research")),
                    (3, format!("{book} browser")),
                    (4, format!("{stats} production")),
                    (5, "Ctrl+C quit".to_string()),
                ];
                let refs: Vec<(u8, &str)> = hints.iter().map(|h| (h.0, h.1.as_str())).collect();
                let mut spans = vec![Span::styled(" ", dim)];
                spans.extend(footer_line(avail.saturating_sub(1) as u16, &refs, None).spans);
                Line::from(spans)
            }
        }
    };
    line_with_note(f, area, line, note);
}

/// The small pane's line while the area tool is open: ` Area › 12x5 · 23 parts · x remove  Esc cancel`.
/// Too wide, it drops the key words first, then the extras (fee, the word Area), and last cuts
/// the text. The counts and the way out always show.
fn area_line(app: &App, avail: usize) -> Line<'static> {
    let c = &app.controls;
    let (place, back) = (c.key(Action::Place), c.key(Action::Back));
    let cyan = Style::new().fg(Color::Cyan);
    let Some(area) = app.area else { return Line::raw("") };
    let options: Vec<String> = match app.area_rect() {
        None => vec![
            format!(" Area › {place} or click: first corner, or drag  {back} cancel"),
            format!(" Area › {place}: corner  {back}"),
            format!(" {place} {back}"),
        ],
        Some(r) => {
            let (w, h) = rect_size(r);
            let info = app.world.area_info(area.floor, r);
            let n = info.parts;
            let parts = format!("{n} part{}", if n == 1 { "" } else { "s" });
            if matches!(area.stage, AreaStage::Chosen { .. }) {
                let (verbs, keys) = (app.area_verb_hints(), AreaVerb::ALL.map(|v| c.key(v.action())).join("/"));
                let fee = if info.fee > 0.0 { format!(" · fee {}", money_text(info.fee)) } else { String::new() };
                if let Some((long, short)) = app.area_note.as_ref() {
                    vec![
                        format!(" Area › {long}  {back} cancel"),
                        format!(" Area › {long}  {back}"),
                        format!(" {long} {back}"),
                        format!(" {short} {back}"),
                    ]
                } else if app.confirm_area.is_some() {
                    let key = c.key(Action::Remove);
                    vec![
                        format!(" Area › {parts}{fee} · {key} again to pay  {back} cancel"),
                        format!(" Area › {parts}{fee} · {key} again  {back}"),
                        format!(" {parts}{fee} · {key} {back}"),
                        format!(" {n} {key} {back}"),
                    ]
                } else {
                    vec![
                        format!(" Area › {w}x{h} · {parts}{fee} · {verbs}  {back} cancel"),
                        format!(" Area › {w}x{h} · {parts}{fee} · {keys}  {back}"),
                        format!(" Area › {parts}{fee} · {keys}  {back}"),
                        format!(" {parts} · {keys} {back}"),
                        format!(" {n} {keys} {back}"),
                    ]
                }
            } else {
                vec![
                    format!(" Area › {w}x{h} · {parts} · {place}: other corner  {back} cancel"),
                    format!(" Area › {w}x{h} · {parts} · {place}  {back}"),
                    format!(" {parts} · {place} {back}"),
                    format!(" {n} {place} {back}"),
                ]
            }
        }
    };
    let text = options.iter().find(|o| o.chars().count() <= avail).or(options.last()).cloned().unwrap_or_default();
    Line::styled(fit(&text, avail), cyan)
}

/// The small pane's line while a blueprint is in the hand:
/// ` Hand › Smelter row 12x5 · $4,310 · 11/23 fit · r turn  Space paste  Esc`. Too wide, it
/// drops the hint words first, then the name, then the size. The cost, the count and the way
/// out always show.
fn hand_line(app: &App, avail: usize) -> Line<'static> {
    let Some(t) = held_text(app) else { return Line::raw("") };
    let c = &app.controls;
    let (rot, place, back) = (c.key(Action::Rotate), c.key(Action::Place), c.key(Action::Back));
    let (cost, name) = (money_text(t.cost), fit(&t.name, 16));
    let (w, h, n, fits) = (t.w, t.h, t.n, t.fits);
    let options = [
        format!(" Hand › {name} {w}x{h} · {cost} · {fits}/{n} fit · {rot} turn  {place} paste  {back}"),
        format!(" Hand › {name} {w}x{h} · {cost} · {fits}/{n} fit · {rot} {place} {back}"),
        format!(" Hand › {w}x{h} · {cost} · {fits}/{n} fit  {back}"),
        format!(" {cost} · {fits}/{n} fit {back}"),
        format!(" {fits}/{n} {back}"),
    ];
    let text = options.iter().find(|o| o.chars().count() <= avail).unwrap_or(&options[4]).clone();
    Line::styled(fit(&text, avail), Style::new().fg(Color::Cyan))
}

/// What holds the status line's one slot, most important first. A hint or aside never takes
/// the place of anything above it, and the key list is what shows when none of them has it.
#[derive(Clone, Debug, PartialEq)]
pub enum Slot {
    /// The small pane's area tool: the rectangle, and the keys. It says everything a message
    /// would (the waste warning included), so it never gives the line up.
    Area,
    /// The small pane's blueprint in the hand: the size, the cost and how much fits. A fresh
    /// message (the result of a paste) takes the line for its six seconds.
    Hand,
    /// The small pane's open build menu (a message shows beside it).
    Menu,
    /// A message from something the player did (6 seconds).
    Message,
    /// A stuck-machine reason or other what's-under-the-cursor text: in the small pane, whatever
    /// the cursor is on (the stuck reason comes first in it); in the full view, a machine that is
    /// stuck (the sidebar says why, and the slot stays the key list).
    Here,
    /// A quiet diagnosis (plan 06).
    Hint(String),
    /// A company's remark, large layout only, rare (plan 08).
    Aside(String),
    /// Nothing else to say: the key list.
    Keys,
}

fn message_fresh(app: &App, now: Instant) -> bool {
    now.saturating_duration_since(app.msg_at).as_secs() < 6
}

/// Whether the cursor is on something the status line would describe, and in the full view only
/// when that is a machine that has stopped.
fn cursor_holds_slot(app: &App) -> bool {
    let (x, y) = app.cursor;
    let w = &app.world;
    let Some(a) = w.floors[app.floor].anchor_at(x, y) else {
        // Not a building: the small pane still says "Solid rock" or "Open air" there.
        return app.compact_now && !w.floors[app.floor].dug[idx(x, y)];
    };
    app.compact_now || w.idle_reason(app.floor, a).is_some()
}

/// True when something the player did or is looking at holds the slot, so a hint or aside
/// must not start. Cheap: the main loop asks every frame.
pub fn slot_taken(app: &App, now: Instant) -> bool {
    (app.compact_now && (app.menu.is_some() || app.area.is_some() || app.held.is_some()))
        || message_fresh(app, now)
        || cursor_holds_slot(app)
}

/// The one rule for the status line's slot: messages from actions first, then a stuck-machine
/// reason (what is under the cursor), then hints, then asides, then the key list.
pub fn status_slot(app: &App, now: Instant) -> Slot {
    if app.compact_now && app.area.is_some() {
        Slot::Area
    } else if app.compact_now && app.held.is_some() && !message_fresh(app, now) {
        Slot::Hand
    } else if app.compact_now && app.menu.is_some() {
        Slot::Menu
    } else if message_fresh(app, now) {
        Slot::Message
    } else if cursor_holds_slot(app) {
        Slot::Here
    } else {
        match app.hints.current.as_ref() {
            Some(c) => match (c.what, app.hints.line(!app.compact_now)) {
                (crate::hints::What::Hint(_), Some(t)) => Slot::Hint(t.to_string()),
                (crate::hints::What::Aside(_), Some(t)) => Slot::Aside(t.to_string()),
                _ => Slot::Keys,
            },
            None => Slot::Keys,
        }
    }
}

/// A hint or aside as a dim line: quiet next to the yellow messages.
fn hint_line(text: &str, avail: usize) -> Line<'static> {
    Line::styled(fit(&format!(" {text}"), avail), Style::new().fg(Color::Gray).add_modifier(Modifier::ITALIC))
}

/// "2 mail · n opens" for the top line, only in the Ticker telegram style (the other styles
/// show the telegram themselves). Counts the one waiting to be read and the ones behind it.
fn mail_count(app: &App) -> Option<String> {
    if app.settings.telegram_style != TelegramStyle::Ticker {
        return None;
    }
    let n = app.world.mail.len() + app.mail.is_some() as usize;
    (n > 0).then(|| format!("{n} mail · {} opens", app.controls.key(Action::Mail)))
}

/// A waiting telegram as a short note for the right end of the bottom line:
/// "agent finished · n to view" (or just "n mail" when there's no room).
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
    Some(if full.chars().count() + 16 > width as usize { format!(" {} mail ", c.key(Action::Mail)) } else { full })
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
    let green = Style::new().fg(INK).bg(Color::Indexed(41)).add_modifier(Modifier::BOLD);
    // What the cursor covers: a ghost of the part being placed, or the whole building under it.
    let ghost_dims = |k: Kind| dims_of(k, app.dir, app.lift_size);
    let ghost = app.selected.map(|k| (k, app.world.check_place_sized(app.floor, cx, cy, k, app.dir, app.lift_size).is_ok()));
    let hover = match app.selected {
        None if app.area.is_none() && app.held.is_none() => {
            app.world.tile(app.floor, cx, cy).zip(app.world.anchor_of(app.floor, cx, cy)).map(|(b, a)| (a, b.dims()))
        }
        _ => None,
    };
    // The area tool: its rectangle gets a tint (blue while marking, red once only a verb is left),
    // the parts it would touch are lit like the hover, and the first corner shows a plus.
    let tool = app.area.filter(|a| a.floor == app.floor);
    let area_rect = app.area_rect().filter(|_| tool.is_some());
    let chosen = matches!(tool.map(|a| a.stage), Some(AreaStage::Chosen { .. }));
    let corner = match tool.map(|a| a.stage) {
        Some(AreaStage::Picking { corner }) => corner,
        _ => None,
    };
    let held_cells = app.held.as_ref().map(|bp| held_ghost(app, bp, yellow, vw, vh));
    // 0 = not in the area, 1 = a part the verb would touch (yellow), 2 = a part that can be upgraded (green).
    let mut lit = vec![0u8; (vw.max(0) * vh.max(0)) as usize];
    if let Some(r) = area_rect {
        let upgradable = upgradable_in(app, r);
        for (x, y) in app.world.parts_in(app.floor, r) {
            let Some(b) = app.world.tile(app.floor, x, y) else { continue };
            let (w, h) = b.dims();
            let state = if upgradable.contains(&(x, y)) { 2 } else { 1 };
            for (tx, ty) in footprint(x, y, w, h) {
                let (sx, sy) = (tx - app.cam.0, ty - app.cam.1);
                if sx >= 0 && sy >= 0 && sx < vw && sy < vh {
                    lit[(sy * vw + sx) as usize] = state;
                }
            }
        }
    }
    let tint = Color::Indexed(if chosen { 52 } else { 24 });
    let inside = |tx: i32, ty: i32, (ax, ay): (i32, i32), (w, h): (i32, i32)| tx >= ax && tx < ax + w && ty >= ay && ty < ay + h;
    // Power ranges show when asked (v), while placing a power source, or when hovering one.
    let hovering_source = app.world.tile(app.floor, cx, cy).is_some_and(|b| coverage(b.kind, (1, 1), 0, 0).is_some());
    let show_range =
        app.power_view || app.selected.is_some_and(|k| coverage(k, (1, 1), 0, 0).is_some()) || (app.selected.is_none() && hovering_source);
    // Hovering a source shades only its own grid; v shows every grid.
    let only_grid = if !app.power_view && app.selected.is_none() && hovering_source { app.world.grid_at(app.floor, cx, cy) } else { None };
    let new_range = app.selected.and_then(|k| coverage(k, ghost_dims(k), cx, cy));
    for sy in 0..vh {
        for sx in 0..vw {
            let (tx, ty) = (app.cam.0 + sx, app.cam.1 + sy);
            let mut cells = if in_bounds(tx, ty) { tile_glyph(&app.world, app.floor, tx, ty) } else { [(' ', Style::new()); 2] };
            if show_range && in_bounds(tx, ty) && cells[0].1.bg == Some(GROUND) {
                let in_new = new_range.is_some_and(|(x0, y0, x1, y1)| tx >= x0 && tx <= x1 && ty >= y0 && ty <= y1);
                let id = app.world.grid.cover[app.floor][idx(tx, ty)];
                let covered = match only_grid {
                    Some(g) => id == g as i32,
                    None => id >= 0,
                };
                let tint = if in_new {
                    Some(RANGE_NEW)
                } else if covered {
                    Some(RANGE)
                } else {
                    None
                };
                if let Some(t) = tint {
                    cells = cells.map(|(c, s)| (c, s.bg(t)));
                }
            }
            if area_rect.is_some_and(|r| tx >= r.0 && tx <= r.2 && ty >= r.1 && ty <= r.3) {
                let state = lit[(sy * vw + sx) as usize];
                if state == 2 {
                    cells = cells.map(|(c, _)| (c, green));
                } else if state == 1 {
                    cells = cells.map(|(c, _)| (c, yellow));
                } else {
                    cells = cells.map(|(c, s)| (c, s.bg(tint)));
                }
                if corner == Some((tx, ty)) {
                    cells[0] = ('+', Style::new().fg(PAPER).bg(tint).add_modifier(Modifier::BOLD));
                }
            }
            if let Some(cells_here) = held_cells.as_ref() {
                // A blueprint in the hand: its parts where they would go, or xx / .. where they can't.
                if let Some(g) = cells_here[(sy * vw + sx) as usize] {
                    cells = g;
                } else if (tx, ty) == app.cursor {
                    cells = cells.map(|(c, _)| (if c == ' ' { '+' } else { c }, yellow));
                }
            } else if let Some((kind, ok)) = ghost {
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

/// The anchors of the parts in `r` that the upgrade planner would swap (or would, with more money):
/// painted green while a rectangle is chosen. Nothing before the planner is researched.
fn upgradable_in(app: &App, r: TileRect) -> HashSet<(i32, i32)> {
    let chosen = matches!(app.area.map(|a| a.stage), Some(AreaStage::Chosen { .. }));
    if !chosen || !app.world.progress.has(Tech::UpgradePlanner) {
        return HashSet::new();
    }
    let plan = app.world.plan_upgrade(app.floor, r);
    plan.parts.iter().filter(|p| matches!(p.state, UpState::Fits | UpState::Short)).map(|p| (p.x, p.y)).collect()
}

/// The ghost of the blueprint in the hand, for each visible tile (row by row): yellow sprites where
/// a part will go, `..` on orange where the money stops, `xx` on red where it can't go. Parts that
/// are already there are not drawn over.
fn held_ghost(app: &App, bp: &Blueprint, yellow: Style, vw: i32, vh: i32) -> Vec<Option<Cells>> {
    let mut out: Vec<Option<Cells>> = vec![None; (vw.max(0) * vh.max(0)) as usize];
    let (cx, cy) = app.cursor;
    let plan = app.world.plan_paste(app.floor, cx, cy, bp);
    let orange = yellow.bg(Color::Indexed(208));
    let red = yellow.bg(Color::Indexed(196));
    for (p, pp) in bp.parts.iter().zip(&plan.parts) {
        let (st, mark) = match pp.state {
            PlanState::Present => continue,
            PlanState::Fits => (yellow, None),
            PlanState::Short => (orange, Some('.')),
            _ => (red, Some('x')),
        };
        let (w, h) = p.dims();
        for (tx, ty) in footprint(pp.x, pp.y, w, h) {
            let (sx, sy) = (tx - app.cam.0, ty - app.cam.1);
            if sx < 0 || sy < 0 || sx >= vw || sy >= vh {
                continue;
            }
            let (lx, ly) = (tx - pp.x, ty - pp.y);
            let chars = match mark {
                Some(m) => [m, m],
                None if p.kind == Kind::Elevator => lift_chars(w, p.dir, &[], lx, ly),
                None => sprite_chars(p.kind, p.dir, None, ' ', lx, ly),
            };
            out[(sy * vw + sx) as usize] = Some(chars.map(|c| (c, st)));
        }
    }
    out
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
        Item::CopperIngot => ('i', 209),
        Item::IronIngot => ('i', 153),
        Item::Stone => ('o', 245),
        // Powders are ':' in their ore's colour (sand is tan); sheets '-', rods '!', screws 'x', springs 'z'.
        Item::CopperGrit => (':', 209),
        Item::IronGrit => (':', 111),
        Item::Sand => (':', 187),
        Item::FineWire => ('~', 223),
        Item::CopperSheet => ('-', 220),
        Item::IronSheet => ('-', 153),
        Item::IronRod => ('!', 153),
        Item::Screw => ('x', 153),
        Item::Spring => ('z', 153),
        // Steel things are white or pale steel; stone things are red (brick) or glassy blue.
        Item::Brick => ('b', 167),
        Item::Glass => ('u', 159),
        Item::SteelRod => ('!', 255),
        Item::Bearing => ('O', 255),
        Item::SteelSheet => ('-', 255),
        Item::HardenedIronPlate => ('=', 67),
        Item::HardenedGear => ('g', 67),
        Item::ReinforcedConcrete => ('C', 246),
        // Parts, then the big goods (capitals, one colour each).
        Item::Valve => ('V', 180),
        Item::Gauge => ('Q', 229),
        Item::Lamp => ('L', 227),
        Item::Relay => ('r', 208),
        Item::ElectricMotor => ('M', 208),
        Item::Piston => ('P', 252),
        Item::ControlBoard => ('B', 40),
        Item::BoilerShell => ('D', 173),
        Item::SiloShell => ('S', 110),
        Item::Frame => ('F', 250),
        Item::PumpUnit => ('U', 45),
        Item::Generator => ('E', 226),
        Item::EngineBlock => ('N', 202),
        Item::ControlPanel => ('T', 77),
        Item::SteamTractor => ('Y', 220),
        // Glass things are blue or green; boards are green like the control board.
        Item::Tube => ('t', 123),
        Item::Transformer => ('X', 214),
        Item::LampMatrix => ('A', 229),
        Item::TubeBoard => ('W', 46),
        Item::Terminal => ('K', 51),
        // Silicon is slate blue-grey; boards stay green.
        Item::SiliconIngot => ('i', 109),
        Item::SiliconScrap => ('s', 66),
        Item::Wafer => ('f', 146),
        Item::Dopant => ('d', 176),
        Item::DopedWafer => ('v', 183),
        Item::Ceramic => ('e', 223),
        Item::CircuitBoard => ('J', 71),
        Item::Transistor => ('q', 80),
        Item::TransistorBoard => ('Z', 82),
        // Chip making: etchant is pale green, masks and dies are silver and blue, the chips pink.
        Item::Etchant => ('h', 150),
        Item::Mask => ('m', 250),
        Item::Die => ('k', 111),
        Item::Microprocessor => ('H', 207),
        Item::RamChip => ('R', 213),
        Item::SiliconCrystal => ('y', 153),
        Item::FineMask => ('n', 252),
        Item::FineDie => ('j', 75),
        Item::FineMicroprocessor => ('U', 219),
        // The computer and its parts: cyan and green boards, the computer in crown gold.
        Item::Keyboard => ('l', 51),
        Item::DiskDrive => ('I', 183),
        Item::Mainboard => ('w', 82),
        Item::Computer => ('&', 226),
        Item::CarbonBlack => ('#', 243),
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
        Kind::Molder => (INK, 208),
        Kind::Press => (INK, 180),
        Kind::Crate => (INK, 178),
        Kind::TradePost => match mode.unwrap_or(SellMode::Money) {
            SellMode::Money => (INK, 35),
            SellMode::Split => (INK, 80),
            SellMode::Research => (INK, 75),
        },
        Kind::Solar => (PAPER, 27),
        Kind::BasicSolar => (PAPER, 33),
        Kind::Pole => (Color::Indexed(186), 234),
        Kind::Elevator => (INK, 141),
        Kind::Pipe => (Color::Indexed(244), 234),
        Kind::Pump => (INK, 45),
        Kind::OilExtractor => (PAPER, 58),
        Kind::LiquidTrader => match mode.unwrap_or(SellMode::Money) {
            SellMode::Money => (INK, 30),
            SellMode::Split => (INK, 66),
            SellMode::Research => (INK, 74),
        },
        Kind::Still => (PAPER, 95),
        Kind::BlastFurnace => (PAPER, 124),
        Kind::Boiler => (PAPER, 130),
        Kind::SteamEngine => (PAPER, 61),
        Kind::FastBelt => (FAST_BELT, 234),
        Kind::CrudeAssembler => (INK, 179),
        Kind::BasicAssembler => (INK, 186),
        Kind::AdvancedAssembler => (INK, 150),
        Kind::PrecisionAssembler => (INK, 117),
        Kind::LargeBasicAssembler => (INK, 222),
        Kind::LargeAdvancedAssembler => (INK, 123),
        Kind::LargePrecisionAssembler => (INK, 159),
        Kind::Welder => (INK, 209),
        Kind::Kiln => (PAPER, 166),
        Kind::LargeKiln => (PAPER, 202),
        Kind::Mixer => (INK, 152),
        Kind::Evacuator => (INK, 158),
        Kind::Etcher => (INK, 183),
        Kind::ArcFurnace => (INK, 216),
        Kind::CrystalGrower => (INK, 153),
        Kind::WaferSaw => (INK, 187),
        Kind::FineEtcher => (INK, 189),
        Kind::AdvancedDrill => (INK, 74),
        Kind::LargeBasicDrill => (INK, 111),
        Kind::LargeAdvancedDrill => (INK, 38),
        Kind::AdvancedPump => (INK, 51),
        Kind::Tunnel => (BELT, 237),
        Kind::LongArm => (PAPER, 97),
        Kind::Sorter => (INK, 37),
        Kind::Warehouse => (INK, 136),
        Kind::CokeOven => (PAPER, 95),
        Kind::Crusher => (PAPER, 65),
        Kind::WireDrawer => (INK, 174),
        Kind::Winder => (INK, 152),
        Kind::RollingMill => (INK, 187),
        Kind::Lathe => (PAPER, 30),
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
        Kind::Molder => ('M', 'O'),
        Kind::Press => ('P', 'R'),
        Kind::Crate => ('C', a),
        Kind::TradePost => ('$', '$'),
        Kind::Solar => ('S', 'O'),
        Kind::BasicSolar => ('S', '2'),
        Kind::Pole => ('P', 'W'),
        Kind::Elevator => ('L', 'I'),
        Kind::Pipe => ('=', '='),
        Kind::Pump => ('P', 'U'),
        Kind::OilExtractor => ('O', 'L'),
        Kind::LiquidTrader => ('$', 'L'),
        Kind::Still => ('S', 'T'),
        Kind::BlastFurnace => ('B', 'F'),
        Kind::Boiler => ('B', 'O'),
        Kind::SteamEngine => ('E', 'N'),
        Kind::FastBelt => (a, a),
        Kind::CrudeAssembler => ('A', '1'),
        Kind::BasicAssembler => ('A', '2'),
        Kind::AdvancedAssembler => ('A', '3'),
        Kind::PrecisionAssembler => ('A', '4'),
        Kind::LargeBasicAssembler => ('L', '2'),
        Kind::LargeAdvancedAssembler => ('L', '3'),
        Kind::LargePrecisionAssembler => ('L', '4'),
        Kind::Welder => ('W', 'E'),
        Kind::Kiln => ('K', 'I'),
        Kind::LargeKiln => ('K', '2'),
        Kind::Mixer => ('M', 'X'),
        Kind::Evacuator => ('E', 'V'),
        Kind::Etcher => ('E', 'T'),
        Kind::ArcFurnace => ('A', 'R'),
        Kind::CrystalGrower => ('G', 'R'),
        Kind::WaferSaw => ('S', 'W'),
        Kind::FineEtcher => ('F', 'E'),
        Kind::AdvancedDrill => ('D', '3'),
        Kind::LargeBasicDrill => ('D', '4'),
        Kind::LargeAdvancedDrill => ('D', '5'),
        Kind::AdvancedPump => ('P', '2'),
        Kind::Tunnel => (a, ']'),
        Kind::LongArm => ('L', a),
        Kind::Sorter => ('S', a),
        Kind::Warehouse => ('W', 'H'),
        Kind::CokeOven => ('C', 'K'),
        Kind::Crusher => ('C', 'R'),
        Kind::WireDrawer => ('W', 'D'),
        Kind::Winder => ('W', 'N'),
        Kind::RollingMill => ('R', 'M'),
        Kind::Lathe => ('L', 'T'),
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
    // Long machines (2x1) facing east or west; a 1-wide column facing north or south.
    let strip = |name: &str| -> Vec<String> {
        match dir {
            Dir::E => vec![format!("{name}{tag}{a}")],
            Dir::W => vec![format!("{a}{tag}{name}")],
            Dir::N => vec![format!("{a}{tag}"), name.into()],
            Dir::S => vec![name.into(), format!("{a}{tag}")],
        }
    };
    match kind {
        Kind::Belt => vec![format!("{a}{a}")],
        Kind::Splitter => vec!["SP".into()],
        Kind::Arm => vec![format!("A{a}")],
        Kind::Crate => vec![format!("C{a}")],
        Kind::Drill => labelled("DRIL"),
        Kind::BasicDrill => labelled("DRL2"),
        Kind::Furnace => labelled("FURN"),
        Kind::Molder => labelled("MOLD"),
        Kind::Press => strip("PR"),
        Kind::WireDrawer => strip("WD"),
        Kind::Winder => strip("WN"),
        Kind::CokeOven => labelled("COKE"),
        Kind::Crusher => labelled("CRSH"),
        Kind::RollingMill => labelled("ROLL"),
        Kind::Lathe => labelled("LATHE"),
        Kind::Elevator => labelled("LIFT"),
        Kind::Solar => vec!["SOLR".into(), "    ".into()],
        Kind::BasicSolar => vec!["SOL2".into(), "    ".into()],
        // A small pylon, not a block.
        Kind::Pole => vec!["/\\".into()],
        Kind::Pipe => vec!["==".into()],
        // 1 wide and 2 tall facing east or west; lying down facing north or south.
        Kind::Pump => match dir {
            Dir::E | Dir::W => vec!["PU".into(), "MP".into()],
            Dir::N | Dir::S => vec!["PUMP".into()],
        },
        // A tall derrick: 1 wide and 5 tall, or lying down.
        Kind::OilExtractor => match dir {
            Dir::E => vec!["OL".into(), "  ".into(), format!(" {a}"), "  ".into(), "  ".into()],
            Dir::W => vec!["OL".into(), "  ".into(), format!("{a} "), "  ".into(), "  ".into()],
            Dir::N | Dir::S => vec!["  OIL     ".into()],
        },
        Kind::BlastFurnace => labelled("BLST"),
        Kind::Boiler => labelled("BOIL"),
        Kind::SteamEngine => vec!["ENGN".into(), "    ".into()],
        Kind::FastBelt => vec![format!("{a}{a}")],
        Kind::Tunnel => vec![tunnel_chars(dir, false).iter().collect()],
        Kind::LongArm => vec![format!("L{a}")],
        Kind::Sorter => vec![format!("S{a}")],
        Kind::Warehouse => labelled("WARE"),
        Kind::CrudeAssembler => labelled("CRD ASM"),
        Kind::BasicAssembler => labelled("BASM"),
        Kind::AdvancedAssembler => labelled("ADVASM"),
        Kind::PrecisionAssembler => labelled("PRCASM"),
        Kind::LargeBasicAssembler => labelled("LRG ASM B"),
        Kind::LargeAdvancedAssembler => labelled("LRG ASM A"),
        Kind::LargePrecisionAssembler => labelled("LRG ASM P"),
        Kind::Welder => labelled("WELDER"),
        Kind::Kiln => labelled("KILN"),
        Kind::LargeKiln => labelled("LRG KILN"),
        Kind::Mixer => labelled("MIXR"),
        Kind::Evacuator => labelled("EVAC"),
        Kind::Etcher => labelled("ETCH"),
        Kind::ArcFurnace => labelled("ARC"),
        Kind::CrystalGrower => labelled("GROW"),
        Kind::WaferSaw => strip("SW"),
        Kind::FineEtcher => labelled("FETCH"),
        Kind::AdvancedDrill => labelled("ADV DRL"),
        Kind::LargeBasicDrill => labelled("LRG BASIC DRILL"),
        Kind::LargeAdvancedDrill => labelled("LRG ADV DRILL"),
        Kind::AdvancedPump => labelled("APMP"),
        Kind::TradePost => {
            let m = match mode.unwrap_or(SellMode::Money) {
                SellMode::Money => "$",
                SellMode::Split => "$ R",
                SellMode::Research => "R",
            };
            vec!["TRADE ".into(), format!("{:^6}", m), "      ".into()]
        }
        Kind::LiquidTrader => {
            let m = match mode.unwrap_or(SellMode::Money) {
                SellMode::Money => "$",
                SellMode::Split => "$R",
                SellMode::Research => "R",
            };
            vec!["TRAD".into(), format!("{m:^4}")]
        }
        Kind::Still => labelled("STIL"),
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

/// The colour of a fluid in pipes and in the book.
fn fluid_colour(fl: Fluid) -> u8 {
    match fl {
        Fluid::Water => 45,
        Fluid::Steam => 255,
        Fluid::MoltenCopper => 208,
        Fluid::MoltenIron => 160,
        Fluid::MoltenSteel => 229,
        Fluid::MoltenSilicon => 117,
        Fluid::CrudeOil => 130,
        Fluid::Naphtha => 220,
        Fluid::FuelOil => 94,
        Fluid::Ethylene => 201,
    }
}

/// A pipe joins up with neighbouring pipes and machines that use them; its colour shows
/// what its network holds (blue water, white steam, orange copper, red iron, pale steel, light blue silicon, grey empty).
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
    let fg = fluid.map_or(244, fluid_colour);
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
        BeltShape::Merge(left, right) => [(if left { '+' } else { a }, st), (if right { '+' } else { a }, st)],
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
            let fed = if b.kind.burns_coal() { *fuel > 0 || *mines == Some(Item::Coal) } else { powered };
            (mines.is_some() && fed && out.len() < b.kind.drill_out_cap(), None)
        }
        St::Crafter { busy, .. } => (*busy, None),
        St::Trade { mode, .. } => (true, Some(*mode)),
        St::Crate { .. } | St::Solar | St::Pole | St::Elevator { .. } | St::Pipe => (true, None),
        St::Pump { busy } | St::Boiler { busy, .. } | St::Extractor { busy, .. } => (*busy, None),
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
        Some(c) if c.phase == LiftPhase::Closing && c.at == f => {
            vec!["closing".into(), "shut".into()]
        }
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
            Some(c) if c.phase == LiftPhase::Moving => {
                if w.floors[c.target].level > w.floors[c.at].level {
                    "▲▲".into()
                } else {
                    "▼▼".into()
                }
            }
            _ => "··".into(),
        };
        vec![m.into(), st]
    } else {
        vec!["LIFT".into(), word, status]
    };
    let chars = lift_chars(s, b.dir, &lines, lx, ly);
    let edge = Style::new().fg(if here { LIFT_EDGE } else { Color::Indexed(66) }).bg(LIFT_BG).add_modifier(Modifier::BOLD);
    let text = Style::new()
        .fg(match mode {
            DoorMode::Load => Color::Indexed(120),
            DoorMode::Unload => Color::Indexed(222),
            DoorMode::Pass => Color::Indexed(244),
        })
        .bg(LIFT_BG)
        .add_modifier(Modifier::BOLD);
    let pick = |c: char| {
        if "┌┐└┘─│▶◀▲▼".contains(c) && !(s == 2 && "▲▼".contains(c) && ly == 1) {
            edge
        } else {
            text
        }
    };
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

/// The sidebar's text: the build menu, then what's under the cursor. `full_h` is the sidebar's
/// height before the graph took any (a crowded menu collapses to fit it). Also returns how many
/// of the lines are the HERE details.
fn side_lines(app: &App, full_h: u16) -> (Vec<Line<'static>>, usize) {
    let head = Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan);
    let dim = Style::new().fg(Color::DarkGray);
    let w = &app.world;
    let mut lines: Vec<Line> = Vec::new();
    // The area tool takes the place of the build list (HERE stays below it).
    let menu = if app.area.is_some() || app.held.is_some() { None } else { app.menu };
    if app.area.is_some() {
        lines.extend(area_side(app));
    }
    if app.held.is_some() {
        lines.extend(held_side(app));
    }
    match menu {
        None if app.area.is_some() || app.held.is_some() => {}
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
            // A crowded menu (Create) would push "HERE" off a short sidebar: then only the slot
            // being used lists all its kinds, and the others show one kind with a "+".
            let all_kinds: usize = slots(cat).iter().map(|s| s.len()).sum();
            let collapse = all_kinds + 9 > full_h.saturating_sub(2) as usize;
            for (n, slot) in slots(cat).iter().enumerate() {
                many |= slot.len() > 1;
                let open = !collapse || app.selected.is_some_and(|s| slot.contains(&s));
                let shown: Vec<Kind> = if open {
                    slot.to_vec()
                } else {
                    let usable = |k: &&Kind| w.progress.part_unlocked(**k);
                    let pick = app.slot_pick.iter().find(|k| slot.contains(k)).or_else(|| slot.iter().find(usable)).unwrap_or(&slot[0]);
                    vec![*pick]
                };
                for (v, k) in shown.iter().enumerate() {
                    let unlocked = w.progress.part_unlocked(*k);
                    let afford = w.econ.money >= k.cost();
                    let mut st = if unlocked && afford { Style::new() } else { dim };
                    if app.selected == Some(*k) {
                        st = st.add_modifier(Modifier::REVERSED);
                    }
                    let (kw, kh) = dims_of(*k, Dir::E, app.lift_size);
                    let mut size = if kw * kh > 1 { format!("{kw}x{kh}") } else { String::new() };
                    let price = price_label(w, *k, app.lift_size);
                    // The name gets 17 columns beside the size, or 21 without it (long names
                    // say "adv." and lose the size, which the status line and book still give).
                    let more = if !open && slot.len() > 1 { "+" } else { "" };
                    let mut name = format!("{}{more}", k.name());
                    if name.chars().count() > 17 {
                        name = name.replace("advanced", "adv.").replace("precision", "prec.");
                        size.clear();
                    }
                    let room = if size.is_empty() { 21 } else { 17 };
                    let name: String = name.chars().take(room).collect();
                    // The slot's number goes on its first kind; the others sit under it.
                    let tag = if v == 0 { format!(" {} ", n + 1) } else { "   ".to_string() };
                    let mut spans = vec![Span::styled(tag, st.add_modifier(Modifier::BOLD))];
                    spans.extend(swatch(kind_cells(*k, Dir::E, unlocked)));
                    spans.push(Span::styled(
                        format!(" {:<room$}{:>w$}{:>7}", name, size, price, w = if size.is_empty() { 0 } else { 4 }),
                        st,
                    ));
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
    let here = inspect(w, app.floor, x, y);
    let here_rows = here.len();
    for s in here {
        lines.push(Line::raw(format!(" {}", s)));
    }
    (lines, here_rows)
}

/// The sidebar's AREA section: what the rectangle holds and what the keys do. Each line fits
/// the sidebar's 34 columns.
fn area_side(app: &App) -> Vec<Line<'static>> {
    let head = Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan);
    let dim = Style::new().fg(Color::DarkGray);
    let c = &app.controls;
    let (place, back) = (c.key(Action::Place), c.key(Action::Back));
    let Some(area) = app.area else { return Vec::new() };
    let mut lines = vec![Line::from(vec![Span::styled("AREA", head), Span::styled(format!("  {back} cancel"), dim)])];
    // Few lines, so HERE still shows below it in a short sidebar.
    if let Some(r) = app.area_rect() {
        let (w, h) = rect_size(r);
        let info = app.world.area_info(area.floor, r);
        let mut counts = format!(" {w}x{h} · {} part{}", info.parts, if info.parts == 1 { "" } else { "s" });
        if info.elevators > 0 {
            counts += &format!(" · {} elevator{}", info.elevators, if info.elevators == 1 { "" } else { "s" });
        }
        lines.push(Line::raw(counts));
        if matches!(area.stage, AreaStage::Chosen { .. }) {
            if info.parts > 0 {
                let fee = if info.fee > 0.0 { format!(" · waste fee {}", money_text(info.fee)) } else { String::new() };
                lines.push(Line::raw(format!(" refund {}{fee}", money_text(info.refund))));
            }
            if let Some(plan) = upgrade_numbers(app, area.floor, r) {
                lines.push(Line::styled(format!(" green: {} to upgrade, {}", plan.0, money_text(plan.1)), Style::new().fg(Color::Green)));
            }
            if let Some(q) = app.confirm_upgrade {
                lines.push(Line::styled(
                    format!(" {} again: upgrade {} for {}", c.key(Action::Upgrade), q.parts, money_text(q.cost)),
                    Style::new().fg(Color::Yellow),
                ));
            } else if app.confirm_area.is_some() {
                lines.push(Line::styled(format!(" {} again to pay the fee", c.key(Action::Remove)), Style::new().fg(Color::Yellow)));
            } else {
                lines.push(Line::raw(format!(" {}", app.area_verb_hints())));
            }
        } else {
            lines.push(Line::styled(format!(" {place} or let go: other corner"), dim));
        }
    } else {
        lines.push(Line::styled(format!(" {place} or click: first corner"), dim));
        lines.push(Line::styled(" or drag a rectangle", dim));
    }
    lines
}

/// How many parts in the chosen rectangle the planner would upgrade now, and for how much. None
/// when the planner isn't researched or there is nothing.
fn upgrade_numbers(app: &App, floor: usize, r: TileRect) -> Option<(usize, f64)> {
    if !app.world.progress.has(Tech::UpgradePlanner) {
        return None;
    }
    let plan = app.world.plan_upgrade(floor, r);
    let n = plan.count(UpState::Fits);
    (n > 0).then(|| (n, plan.cost(UpState::Fits)))
}

/// What the blueprint in the hand would do at the cursor, as the sidebar's short lines.
fn held_text(app: &App) -> Option<HeldText> {
    let bp = app.held.as_ref()?;
    let (cx, cy) = app.cursor;
    let plan = app.world.plan_paste(app.floor, cx, cy, bp);
    let n = bp.part_count();
    let cost = if plan.total > 0.0 { plan.total } else { bp.cost() };
    let fit = if plan.refused.is_some() {
        "this floor isn't open yet".to_string()
    } else if plan.fits == 0 && plan.short == 0 && plan.present < n {
        "nothing fits here".to_string()
    } else if plan.fits == n {
        format!("all {n} fit your money")
    } else {
        format!("{} of {n} fit your money", plan.fits)
    };
    let mut problems: Vec<String> = Vec::new();
    if plan.short > 0 {
        problems.push(format!("{} short {}", plan.short, money_text(plan.short_by)));
    }
    if plan.present > 0 {
        problems.push(format!("{} there", plan.present));
    }
    if plan.blocked > 0 && plan.refused.is_none() {
        problems.push(format!("{} blocked", plan.blocked));
    }
    if plan.locked > 0 {
        let what = plan.locked_kind.map_or(String::new(), |k| format!(" ({})", k.name()));
        problems.push(format!("{} locked{what}", plan.locked));
    }
    if plan.wrong_floor > 0 {
        problems.push(format!("{} not this floor", plan.wrong_floor));
    }
    Some(HeldText { name: bp.name.clone(), w: bp.w, h: bp.h, n, cost, fit, fits: plan.fits, problems })
}

struct HeldText {
    name: String,
    w: i32,
    h: i32,
    n: usize,
    cost: f64,
    fit: String,
    fits: usize,
    problems: Vec<String>,
}

/// The sidebar's BLUEPRINT section. Every line fits the sidebar's 34 columns.
fn held_side(app: &App) -> Vec<Line<'static>> {
    let Some(t) = held_text(app) else { return Vec::new() };
    let head = Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan);
    let dim = Style::new().fg(Color::DarkGray);
    let c = &app.controls;
    let mut lines = vec![Line::from(vec![Span::styled("BLUEPRINT", head), Span::styled(format!("  {}", fit(&t.name, 23)), Style::new())])];
    lines.push(Line::raw(fit(&format!(" {}x{} · {} part{} · {}", t.w, t.h, t.n, if t.n == 1 { "" } else { "s" }, money_text(t.cost)), 34)));
    lines.push(Line::raw(fit(&format!(" {}", t.fit), 34)));
    if !t.problems.is_empty() {
        lines.push(Line::styled(fit(&format!(" {}", t.problems.join(" · ")), 34), Style::new().fg(Color::Yellow)));
    }
    lines.push(Line::styled(
        fit(&format!(" {} turn · {} paste · {} drop", c.key(Action::Rotate), c.key(Action::Place), c.key(Action::Back)), 34),
        dim,
    ));
    lines
}

/// Rows the sidebar's text needs in a panel `width` wide (borders included), so the graph only
/// takes what's left. What's under the cursor counts for at least 6 rows, so the graph doesn't
/// jump about as the cursor moves over small and big things.
fn side_need(app: &App, width: u16, full_h: u16) -> u16 {
    let (lines, here) = side_lines(app, full_h);
    let inner = width.saturating_sub(2).max(1) as usize;
    let rows: usize = lines.iter().map(|l| l.width().div_ceil(inner).max(1)).sum();
    (rows + 6usize.saturating_sub(here) + 2) as u16
}

fn draw_side(f: &mut Frame, area: Rect, app: &App, full_h: u16) {
    let (lines, _) = side_lines(app, full_h);
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
    if b.kind.electric() {
        match w.building_net(f, idx(ax, ay)) {
            Some(n) => v.push(format!("EC {:.0}% (grid {:.0}/{:.0})", n.ratio * 100.0, n.supply, n.demand)),
            None => v.push("NO EC: out of range (v)".into()),
        }
        if b.kind.burst_draw() > 0.0 {
            let working = matches!(b.st, St::Crafter { busy: true, .. });
            v.push(format!("{:.0} EC only while working", b.kind.burst_draw()));
            v.push(format!("drawing {:.0} EC now", if working { b.kind.burst_draw() } else { 0.0 }));
        } else if b.kind.power_draw() >= 4.0 {
            v.push(format!("draws {:.0} EC", b.kind.power_draw()));
        }
    }
    if coverage(b.kind, (1, 1), 0, 0).is_some() {
        if let Some((n, sources)) = w.source_net(f, idx(ax, ay)) {
            let with = if sources == 1 { "on its own".to_string() } else { format!("{sources} sources") };
            if n.demand <= 0.0 {
                v.push(format!("grid: {:.0} supply, nothing to charge ({with})", n.supply));
            } else {
                v.push(format!("grid: {:.0} supply / {:.0} demand ({with})", n.supply, n.demand));
            }
        }
    }
    if let Some(t) = blurb(b.kind) {
        v.push(t.to_string());
    }
    match &b.st {
        St::Drill { mines, fuel, coal, out, .. } => {
            v.push(format!("mining {}", mines.map_or("nothing", |it| it.name())));
            let (mult, burn) = b.kind.drill_rates();
            if mult > 1 || burn > 1 {
                v.extend(drill_speed_lines(b.kind));
            }
            if b.kind.burns_coal() {
                v.push(format!("fuel {}   coal {}/{}", if *fuel > 0 { "burning" } else { "empty" }, coal, b.kind.drill_coal_cap()));
            }
            v.push(format!("output {}/{}", out.len(), b.kind.drill_out_cap()));
            if b.kind.burns_coal() && *fuel == 0 && *coal == 0 && *mines != Some(Item::Coal) {
                v.push("needs coal: belt some into it".into());
            }
            if b.dims().0 >= 8 {
                v.push("feeds anything along its front".into());
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
        St::Crafter { stock, fuel, out, busy, fluid, fluid_out, .. } => {
            if let Some(r) = b.recipe() {
                v.push(format!("making {}  (f changes)", r.name));
                let batch = b.kind.batch();
                let mut needs: Vec<String> =
                    r.inputs.iter().map(|(it, n)| format!("{} {}/{}", it.name(), stock[it.idx()], n * batch)).collect();
                if let Some((fl, n)) = r.fluid_in {
                    needs.push(format!("{} {}/{}", fl.name(), fluid, n * batch));
                }
                v.push(format!("has {}", needs.join(", ")));
                if batch > 1 {
                    // How many runs the loaded inputs cover (a whole batch fires at once).
                    let loads = r.inputs.iter().map(|(it, n)| stock[it.idx()] / n).min().unwrap_or(0).min(batch);
                    v.push(format!("load {loads}/{batch}"));
                    v.push(format!("fires {batch} at once when full"));
                }
                if r.fluid_in.is_some() {
                    v.push(format!("fluid comes from pipes (not the {} side)", b.dir.arrow()));
                }
                if let Some((fl, n)) = r.fluid_out {
                    v.push(format!("gives {n} {}  ({}/{} held)", fl.name(), fluid_out[0], CRAFTER_FLUID_OUT_CAP));
                }
                if let Some((fl, n)) = r.fluid_out2 {
                    v.push(format!("gives {n} {} out the back  ({}/{} held)", fl.name(), fluid_out[1], CRAFTER_FLUID_OUT_CAP));
                }
                let waste = r.outputs.iter().any(|o| o.0 == Item::Waste);
                if waste {
                    let products: Vec<&str> = r.outputs.iter().filter(|o| o.0 != Item::Waste).map(|o| o.0.name()).collect();
                    if !products.is_empty() {
                        v.push(format!("{} out the front {}", products.join(" + "), b.dir.arrow()));
                    }
                    v.push(format!("waste out the back (the {} side)", b.dir.opposite().arrow()));
                } else if r.outputs.len() > 1 {
                    let outs: Vec<&str> = r.outputs.iter().map(|o| o.0.name()).collect();
                    v.push(format!("gives {} (all out the front)", outs.join(" + ")));
                }
                if let Some(fuel_item) = r.fuel {
                    let lit = if *fuel > 0 { "burning" } else { "no fire" };
                    v.push(format!("{} {}   {}", fuel_item.name(), stock[fuel_item.idx()], lit));
                }
                let held_waste = out.iter().filter(|i| **i == Item::Waste).count();
                let cap = b.kind.out_cap();
                let state = if *busy { "working" } else { "waiting" };
                if waste {
                    v.push(format!("finished {}/{cap}   waste {held_waste}/{cap}   {state}", out.len() - held_waste));
                    if held_waste >= cap {
                        v.push("waste stuck: needs a belt or crate behind it".into());
                    }
                } else {
                    v.push(format!("finished {}/{cap}   {state}", out.len()));
                }
            }
        }
        St::Crate { items } => {
            let cap = CRATE_CAP * if b.kind == Kind::Warehouse { WAREHOUSE_CRATES } else { 1 };
            v.push(format!("{}/{} items", items.len(), cap));
            v.extend(item_counts(items.iter().copied()));
        }
        St::Trade { mode, money, research, count, .. } if b.kind.is_liquid_trader() => {
            v.push(format!("mode: {}  (f changes)", mode.label()));
            let _ = count;
            v.push(format!("sold ${money:.0}   research {research:.1}"));
            v.push("up to 4 units a second from the pipes it touches".into());
            v.push("water and steam cannot be sold".into());
            v.push("research for the next unit sold:".into());
            for fl in Fluid::ALL.iter().copied().filter(|f| f.curve().is_some()) {
                v.push(format!("  {:<19}{:>6.2}", fl.name(), w.econ.next_fluid_research(fl)));
            }
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
                    v.push(format!("  {:<19}{:>6.2}", it.name(), w.econ.next_research(it)));
                }
            }
        }
        St::Solar => v.push(format!("+{:.0} EC, reaches {} tiles around it", b.kind.solar_output(), b.kind.reach().unwrap_or(0))),
        St::Pole => v.push(format!("carries EC {} tiles around it", POLE_REACH)),
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
            v.push(format!("{:.0} water a second", b.kind.pump_rate()));
            v.push(if *busy { "pumping water into its pipes".into() } else { "idle: needs EC and a pipe with room".into() });
        }
        St::Extractor { busy, .. } => {
            let secs = CRUDE_TICKS / TPS as f64;
            v.push(format!("1 crude oil every {secs:.0} s, into a pipe at its front"));
            v.push(format!("deeper floors are richer: x{} down here", crate::sim::richness(f as i32)));
            v.push(if *busy { "drawing crude oil".into() } else { "idle: needs EC and a pipe with room".into() });
        }
        St::Boiler { coal, fuel, busy } => {
            v.push(format!("coal {}/{}   {}", coal, BOILER_COAL_CAP, if *fuel > 0 { "burning" } else { "no fire" }));
            v.push(format!("fire {}s", fuel / TPS as u32));
            v.push(format!("water in from pipes, steam out the {} side", b.dir.arrow()));
            v.push(if *busy { "making steam".into() } else { "waiting: needs coal, water, room for steam".into() });
        }
        St::Engine { steam, output } => {
            v.push(format!("steam {:.1}/{:.0}", steam, ENGINE_BUF));
            v.push(format!("giving {:.0} of {:.0} EC", output, ENGINE_POWER));
            v.push(format!("charges parts {} tiles around it", ENGINE_REACH));
        }
    }
    let (waste, fee) = w.removal_fee(f, x, y);
    if waste > 0 {
        v.push(format!("Removing costs {} ({waste} waste)", money_text(fee)));
    }
    v
}

/// What a fast drill does compared with a basic (or advanced) one, in short lines.
fn drill_speed_lines(kind: Kind) -> Vec<String> {
    let (mult, burn) = kind.drill_rates();
    match kind {
        Kind::LargeAdvancedDrill => vec![format!("{}x an advanced drill's ore", mult / 2)],
        _ if burn > 0 => vec![format!("{mult}x a basic drill's ore"), format!("burns {burn}x the coal")],
        _ => vec![format!("{mult}x a basic drill's ore")],
    }
}

/// One line per kind of item: its belt glyph, name and how many.
fn item_counts(items: impl Iterator<Item = Item>) -> Vec<String> {
    let mut n = [0usize; ITEMS];
    for it in items {
        n[it.idx()] += 1;
    }
    Item::ALL
        .iter()
        .filter(|it| n[it.idx()] > 0)
        .map(|it| format!("  {} {} x{}", item_cell(*it, GROUND).0, it.name(), n[it.idx()]))
        .collect()
}

fn draw_bottom(f: &mut Frame, area: Rect, app: &App) {
    // The Note style shows the note always; the sidebar style only while the tree or book covers the sidebar.
    let note = match app.settings.telegram_style {
        TelegramStyle::Note => mail_note(app, area.width),
        TelegramStyle::Sidebar if app.book => mail_note(app, area.width),
        _ => None,
    };
    let avail = (area.width as usize).saturating_sub(note.as_ref().map_or(0, |n| n.chars().count()));
    // The sidebar already says why a machine under the cursor is stuck, so that slot shows the keys.
    let line = match status_slot(app, Instant::now()) {
        Slot::Message => Line::styled(fit(&format!(" {}", app.msg), avail), Style::new().fg(Color::Yellow)),
        Slot::Hint(t) | Slot::Aside(t) => hint_line(&t, avail),
        _ => {
            let c = &app.controls;
            Line::styled(
                format!(
                    " {} help   1-4 build   {} research   {} browser   {} production   {} place   {} remove   Ctrl+C quit",
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
    line_with_note(f, area, line, note);
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h)
}

/// Cuts `text` to `width` columns, at a word boundary when that keeps most of it, and ends
/// it with "…" so a clipped message never looks like a whole one.
fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    match width {
        0 => return String::new(),
        1 => return "…".into(),
        _ => {}
    }
    let cut: String = text.chars().take(width - 1).collect();
    let at_word_end = text.chars().nth(width - 1) == Some(' ');
    let keep = match cut.rfind(' ') {
        Some(i) if !at_word_end && cut[..i].chars().count() * 2 >= width => &cut[..i],
        _ => cut.as_str(),
    };
    format!("{}…", keep.trim_end_matches([' ', ',', ':', ';', '-', '·']))
}

/// One row of key hints for the bottom of a box. `hints` are (priority, text), a lower
/// priority number being more important: hints are added in that order while they fit
/// (shown in the order given, joined with " · "), so the least important ones go first.
/// `more` ("more ▲▼") is right-aligned and never overlaps a hint.
fn footer_line(width: u16, hints: &[(u8, &str)], more: Option<&str>) -> Line<'static> {
    let width = width as usize;
    let more = more.map(|m| fit(m, width));
    let more_w = more.as_ref().map_or(0, |m| m.chars().count() + 1);
    let room = width.saturating_sub(more_w);
    let mut order: Vec<usize> = (0..hints.len()).collect();
    order.sort_by_key(|&i| hints[i].0);
    let mut chosen = vec![false; hints.len()];
    let mut used = 0;
    let mut count = 0;
    for i in order {
        let add = hints[i].1.chars().count() + if count > 0 { 3 } else { 0 };
        if used + add <= room {
            chosen[i] = true;
            used += add;
            count += 1;
        }
    }
    let mut text = hints.iter().zip(&chosen).filter(|(_, c)| **c).map(|(h, _)| h.1).collect::<Vec<_>>().join(" · ");
    if count == 0 {
        // Not even the most important one fits whole: cut it.
        if let Some(first) = hints.iter().min_by_key(|h| h.0) {
            text = fit(first.1, room);
        }
    }
    let dim = Style::new().fg(Color::DarkGray);
    let pad = room.saturating_sub(text.chars().count());
    let mut spans = vec![Span::styled(text, dim), Span::raw(" ".repeat(pad))];
    if let Some(m) = more {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(m, Style::new().fg(KEY_ITEM).add_modifier(Modifier::BOLD)));
    }
    Line::from(spans)
}

/// Money for a tight line: "$1,234" up to 99,999, then "$123k" and "$1.2M".
fn short_money(v: f64) -> String {
    let v = v.round();
    if v < 0.0 {
        format!("-{}", short_money(-v))
    } else if v < 100_000.0 {
        format!("${}", grouped(v))
    } else if v < 1_000_000.0 {
        format!("${:.0}k", v / 1e3)
    } else if v < 100_000_000.0 {
        format!("${:.1}M", v / 1e6)
    } else {
        format!("${:.0}M", v / 1e6)
    }
}

/// "$1,234" or "-$56".
fn signed_money(v: f64) -> String {
    if v < -0.5 {
        format!("-${}", grouped((-v).round()))
    } else {
        format!("${}", grouped(v.round()))
    }
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
    let dim = Style::new().fg(Color::DarkGray);
    f.render_widget(Clear, area);
    app.stats_hits.clear();
    let (mut rows, secs) = app.world.flow();
    // No border of its own: it sits inside the map's. The tabs are its title.
    let mut title: Vec<Span> = Vec::new();
    let mut x = area.x;
    for (name, history) in [("Production", false), ("History", true)] {
        let label = format!(" {name} ");
        let st = if history == app.stats_history {
            Style::new().fg(INK).bg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(Color::Gray)
        };
        let w = label.chars().count() as u16;
        app.stats_hits.push((Rect::new(x, area.y, w, 1), StatsHit::Tab(history)));
        x += w + 1;
        title.push(Span::styled(label, st));
        title.push(Span::raw(" "));
    }
    if !app.stats_history {
        title.push(Span::styled(if secs >= 60 || secs == 0 { "· last minute".to_string() } else { format!("· last {secs} s") }, dim));
    }
    let block = Block::new().title(Line::from(title));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width < 10 || inner.height < 3 {
        return;
    }
    let width = inner.width.saturating_sub(2) as usize;
    let body_h = inner.height.saturating_sub(1);
    let body = Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), body_h);
    let mut hits = Vec::new();
    let lines: Vec<Line> = if app.stats_history {
        let page = history::page(app, width, body_h as usize);
        hits = page.hits;
        page.lines
    } else {
        production_lines(&app.world, &mut rows, width)
    };
    let max_scroll = (lines.len() as u16).saturating_sub(body_h);
    app.stats_scroll = app.stats_scroll.min(max_scroll);
    for (row, col, w, hit) in hits {
        let y = (row as u16 + body.y).checked_sub(app.stats_scroll);
        if let Some(y) = y.filter(|y| *y >= body.y && *y < body.y + body_h) {
            app.stats_hits.push((Rect::new(body.x + col as u16, y, w as u16, 1), hit));
        }
    }
    // The Production text wraps; the History page is laid out to the width and never does.
    let text = Paragraph::new(lines).scroll((app.stats_scroll, 0));
    f.render_widget(if app.stats_history { text } else { text.wrap(Wrap { trim: false }) }, body);
    let foot = Rect::new(inner.x + 1, inner.y + inner.height - 1, inner.width.saturating_sub(2), 1);
    // In the browser the panel is a page: Esc puts the browser away.
    let close = if app.book { "Esc minimize".to_string() } else { format!("{}/Esc close", app.controls.key(Action::Stats)) };
    let more = match (app.stats_scroll > 0, app.stats_scroll < max_scroll) {
        (true, true) => Some("more ▲▼"),
        (true, false) => Some("more ▲"),
        (false, true) => Some("more ▼"),
        _ => None,
    };
    let line = if app.stats_history {
        let narrow = foot.width < 44;
        let mut hints = vec![
            (0, if narrow { "Tab switch" } else { "Tab production" }),
            (0, "1-4 range"),
            (0, if narrow { "Esc" } else { close.as_str() }),
            (3, "PgUp/PgDn scroll"),
            (4, "the game keeps running"),
        ];
        if !history::stacked(body_h as usize) {
            hints.insert(3, (1, "r research/money"));
        }
        footer_line(foot.width, &hints, more)
    } else {
        let hints = [(1, "↑↓ scroll"), (0, close.as_str()), (1, "Tab history"), (2, "the game keeps running")];
        footer_line(foot.width, &hints, more)
    };
    f.render_widget(Paragraph::new(line), foot);
}

/// The Production tab's text: the headline and a table of what each item did over the last minute.
fn production_lines(w: &World, rows: &mut [Flow], width: usize) -> Vec<Line<'static>> {
    let dim = Style::new().fg(Color::DarkGray);
    let money: f64 = rows.iter().map(|r| r.money).sum();
    let research: f64 = rows.iter().map(|r| r.research).sum();
    let mut lines: Vec<Line> = vec![Line::from(vec![
        Span::styled(
            format!("{}/min", signed_money(money)),
            Style::new().fg(if money < 0.0 { Color::Red } else { Color::Green }).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ·  ", dim),
        // No research selling is a real "0", not a bare dot that reads like a second separator.
        if research < 0.05 {
            Span::styled("0 research/min", Style::new().fg(RESEARCH_DIM))
        } else {
            Span::styled(format!("{} research/min", rate(research)), Style::new().fg(RESEARCH).add_modifier(Modifier::BOLD))
        },
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
        for r in rows.iter() {
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
                    "research" => (rate(r.research), Style::new().fg(RESEARCH)),
                    _ => {
                        // What the next sale for research would bring: it fades the more you sell.
                        if r.item == Item::Waste {
                            spans.push(Span::styled(format!("{:>cw$}", "none"), dim));
                            continue;
                        }
                        let next = w.econ.next_research(r.item);
                        let worth = r.item.curve().worth;
                        let st = if next >= worth * 0.5 {
                            Style::new().fg(Color::Green)
                        } else if next >= worth * 0.15 {
                            Style::new().fg(Color::Yellow)
                        } else {
                            dim
                        };
                        (format!("{} RP", num(next)), st)
                    }
                };
                spans.push(Span::styled(format!("{text:>cw$}"), style));
            }
            lines.push(Line::from(spans));
        }
        // Liquids a liquid trader sold: the same numbers, in the fluid's own colour.
        for f in w.flow_fluids() {
            let st = Style::new().fg(Color::Indexed(fluid_colour(f.fluid))).add_modifier(Modifier::BOLD);
            let name: String = f.fluid.name().chars().take(name_w).collect();
            lines.push(Line::from(vec![
                Span::styled("~ ", st),
                Span::raw(format!("  {name:<name_w$}")),
                Span::raw(format!("{:>7}", rate(f.sold))),
                Span::styled(format!("{:>8}", format!("${}", rate(f.money))), Style::new().fg(Color::Green)),
                Span::styled(format!("{:>10}", format!("{} RP", rate(f.research))), Style::new().fg(RESEARCH)),
            ]));
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled("Research per sale fades the more you sell of an item, so new items pay best.", dim));
        if rows.iter().any(|r| r.item == Item::Waste) {
            lines.push(Line::styled("Waste costs money to sell (shown negative) and earns no research.", dim));
        }
    }
    lines
}

/// A whole number with thousands marks: 17,687.
fn grouped(v: f64) -> String {
    let n = v.max(0.0).floor() as u64;
    let s = n.to_string();
    let mut out = String::new();
    for (k, ch) in s.chars().enumerate() {
        if k > 0 && (s.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// A tech partway to its free unlock: a circle filling a quarter at a time, with (made, needed).
/// None before the first milestone item is made.
fn milestone_mark(w: &World, t: Tech) -> Option<(&'static str, u64, u64)> {
    let (key, n) = t.milestone();
    let made = w.econ.made_of(key).min(n as _);
    let n = n as u64;
    if made == 0 || n == 0 {
        return None;
    }
    let mark = match made * 4 / n {
        0 | 1 => "◔",
        2 => "◑",
        _ => "◕",
    };
    Some((mark, made, n))
}

/// One row on the research screen: a mark, the name, and what it costs or needs.
fn tree_row_spans(w: &World, t: Tech, name_w: usize, short: bool, very_short: bool) -> Vec<Span<'static>> {
    let rp = w.econ.research;
    let dim = Style::new().fg(Color::DarkGray);
    let ready = Style::new().fg(RESEARCH).add_modifier(Modifier::BOLD);
    let saving = Style::new().fg(RESEARCH_DIM);
    let done = Style::new().fg(Color::Green);
    let (mark, ns, status) = if w.progress.has(t) {
        ("✓", done, Span::styled("done", dim))
    } else if !w.progress.is_open(t.company()) {
        let key = keys_text(t.requires());
        ("·", dim, Span::styled(if short { format!("needs {key}") } else { format!("make {key} first") }, dim))
    } else if rp + 1e-9 >= t.cost() {
        ("●", Style::new().add_modifier(Modifier::BOLD), Span::styled(format!("{} RP", grouped(t.cost())), ready))
    } else if let Some((mark, made, n)) = milestone_mark(w, t) {
        // On the way to the free unlock: a filling circle, and how far along when there's room.
        let text = if short { format!("{} RP", grouped(t.cost())) } else { format!("{} RP · {made}/{n}", grouped(t.cost())) };
        (mark, Style::new(), Span::styled(text, saving))
    } else {
        ("○", Style::new(), Span::styled(format!("{} RP", grouped(t.cost())), saving))
    };
    let mark_st = match mark {
        "●" => ready,
        "◔" | "◑" | "◕" => Style::new().fg(Color::Green),
        _ => ns,
    };
    let name = format!("{:<name_w$}", fit(t.name(), name_w));
    if very_short {
        // A fixed status column: the full "needs X" is in the details under the list.
        let text = if status.content.starts_with("needs") { "needs".to_string() } else { status.content.to_string() };
        let status = Span::styled(format!("{text:>8}"), status.style);
        return vec![Span::styled(format!("{mark} "), mark_st), Span::styled(name, ns), status];
    }
    vec![Span::styled(format!("{mark} "), mark_st), Span::styled(name, ns), status]
}

/// What the picked row does, in full, for under the list.
fn tree_details(w: &World, t: Tech) -> Vec<(String, Style)> {
    let head = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let note = Style::new().fg(RESEARCH);
    let rp = grouped(w.econ.research);
    let (key, n) = t.milestone();
    let made = w.econ.made_of(key).min(n);
    let status = if w.progress.has(t) {
        "Researched.".to_string()
    } else if !w.progress.is_open(t.company()) {
        let key = keys_text(t.requires());
        let hints: Vec<String> = t.requires().iter().map(|k| key_hint(*k)).filter(|h| !h.is_empty()).collect();
        format!("Make {key} once: it opens {}, and then this. Made with the {}", t.company().name(), hints.join("; or the "))
    } else {
        format!("Enter: research for {} RP (you have {rp}). Or free once you've made {n} {} ({made}/{n}).", grouped(t.cost()), key.name())
    };
    vec![(format!("{} · {}", t.name(), t.category().name()), head), (format!("{}.", capitalise(t.about())), Style::new()), (status, note)]
}

/// The research screen, over everything but the bottom line (where results show).
/// New parts in groups; the picked row's details sit under the list.
fn draw_tree(f: &mut Frame, area: Rect, app: &mut App) {
    if !crate::research::graph_enabled(area.width) || area.height < 16 {
        draw_tree_list(f, area, app);
    } else {
        draw_tree_graph(f, area, app);
    }
}

fn draw_tree_list(f: &mut Frame, area: Rect, app: &mut App) {
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
    let pts_st = Style::new().fg(RESEARCH).add_modifier(Modifier::BOLD);
    line(f, y, vec![Span::styled(points, pts_st)]);
    y += if r.height < 16 { 1 } else { 2 };

    // Details first (so the list knows how much room it has), wrapped to the window.
    let rows = tree_rows();
    let pick = crate::research::tech_for_node(app.tree_pick).and_then(|t| rows.iter().position(|x| *x == t)).unwrap_or(0);
    // In a short pane the details are three lines (title, about, status) so the list keeps its room.
    let short_pane = r.height < 24;
    // In a very short one the "about" line goes too.
    let squat = r.height < 16;
    let details: Vec<(String, Style)> = tree_details(&app.world, rows[pick])
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !(squat && *i == 1))
        .map(|(_, d)| d)
        .flat_map(|(text, st)| {
            let mut wrapped = wrap_words(&text, width.max(10) as usize);
            if short_pane {
                wrapped.truncate(1);
                wrapped[0] = fit(&wrapped[0], width as usize);
            }
            wrapped.into_iter().map(move |l| (l, st))
        })
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
    let room = footer_y.saturating_sub(details.len() as u16 + if squat { 1 } else { 2 }).max(y + 2);
    let list_bottom = (y + lines.len() as u16).min(room);
    let view = (list_bottom - y) as usize;
    let at = lines.iter().position(|l| l.1 == Some(pick)).unwrap_or(0);
    let top = if lines.len() <= view { 0 } else { (at + 2).saturating_sub(view).min(lines.len() - view) };
    // Names are never cut (some are long); narrow rows say "needs X" instead of "make X first".
    let room = (width - gutter) as usize;
    let short = room < 52;
    let very_short = room < 46;
    let name_w = if very_short { room.saturating_sub(10).max(8) } else { room.saturating_sub(22).clamp(14, 22) };
    // With no gutter the group's label may be scrolled off: the picked row carries it as a tag.
    let mut before = 0;
    let mut pick_group = None;
    for (label, n) in &groups {
        if pick < before + n {
            pick_group = Some(*label);
            break;
        }
        before += n;
    }
    let label_shown = lines.iter().skip(top).take(view).any(|(l, r)| r.is_none() && l.is_some() && *l == pick_group);
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
            let mut rs = tree_row_spans(&app.world, rows[*i], name_w, short, very_short);
            if *i == pick {
                // The picked row: one even band across the row, colours kept.
                let used: usize = rs.iter().map(|s| s.content.chars().count()).sum();
                let free = (width - gutter) as usize - used.min((width - gutter) as usize);
                match pick_group.filter(|g| gutter == 0 && !label_shown && free > g.chars().count() + 1) {
                    Some(g) => {
                        rs.push(Span::raw(" ".repeat(free - g.chars().count() - 1)));
                        rs.push(Span::styled(g.to_string(), dim));
                        rs.push(Span::raw(" "));
                    }
                    None => rs.push(Span::raw(" ".repeat(free))),
                }
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
    line(f, list_bottom, vec![Span::styled("─".repeat(width as usize), dim)]);
    for (dy, (text, st)) in (list_bottom + 1..footer_y).zip(details) {
        line(f, dy, vec![Span::styled(text, st)]);
    }

    // Footer: keys, and which way the list goes on.
    let close = "Esc minimize".to_string();
    let more = match (top > 0, top + view < lines.len()) {
        (true, true) => Some("more ▲▼"),
        (true, false) => Some("more ▲"),
        (false, true) => Some("more ▼"),
        _ => None,
    };
    let hints = [(2, "↑↓ pick"), (1, "Enter research"), (0, close.as_str())];
    f.render_widget(Paragraph::new(footer_line(width, &hints, more)), Rect::new(x0, footer_y, width, 1));
}

fn draw_tree_graph(f: &mut Frame, area: Rect, app: &mut App) {
    use crate::research::{BOX_W, COL_GAP};

    let head = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let dim = Style::new().fg(Color::DarkGray);
    let r = Rect { height: area.height.saturating_sub(1), ..area };
    f.render_widget(Clear, r);
    f.render_widget(Block::bordered().title(" Research ").title_style(head), r);
    if r.width < 34 || r.height < 9 {
        return;
    }

    let x0 = r.x + 2;
    let width = r.width.saturating_sub(4);
    let points = format!("{} research", grouped(app.world.econ.research));
    f.render_widget(
        Paragraph::new(Line::styled(points, Style::new().fg(RESEARCH).add_modifier(Modifier::BOLD))),
        Rect::new(x0, r.y + 1, width, 1),
    );
    let selected = app.tree_pick;
    app.ensure_tree_selection_visible();
    let positions = crate::research::layout_expanded(&app.tree_expanded);
    let here = positions.iter().find(|p| p.id == selected).copied();
    let graph_top = r.y + 2;
    let footer_y = r.y + r.height - 2;
    let detail_top = footer_y.saturating_sub(4);
    let graph_h = detail_top.saturating_sub(graph_top);
    let stride_x = BOX_W + COL_GAP;
    let (total_h, total_w) = crate::research::canvas_size_expanded(&app.tree_expanded);
    let visible_cols = ((width + COL_GAP) / stride_x).max(1);
    let graph_w = (visible_cols * stride_x - COL_GAP).min(width);
    let scroll_x = here.map_or(0, |p| {
        let left = p.col as u16 * stride_x;
        left.min(total_w.saturating_sub(graph_w))
    });
    let scroll_y = here.map_or(0, |p| {
        let focus = p.row as u16 + 1;
        focus.saturating_sub(graph_h / 2).min(total_h.saturating_sub(graph_h))
    });
    let gx = |p: &crate::research::NodePos| x0 as i32 + (p.col as u16 * stride_x) as i32 - scroll_x as i32;
    let gy = |p: &crate::research::NodePos| graph_top as i32 + p.row as i32 - scroll_y as i32;
    let put = |f: &mut Frame, x: i32, y: i32, ch: char, style: Style| {
        if x >= x0 as i32 && x < (x0 + graph_w) as i32 && y >= graph_top as i32 && y < detail_top as i32 {
            if let Some(cell) = f.buffer_mut().cell_mut((x as u16, y as u16)) {
                cell.set_char(ch).set_style(style);
            }
        }
    };
    let line_style = Style::new().fg(Color::DarkGray);

    // Each edge stays in the four-cell gap between consecutive depth columns.
    // Segment masks are merged into box-side and bus junction glyphs below.
    let mut lines = std::collections::HashMap::<(i32, i32), u8>::new();
    const U: u8 = 1;
    const D: u8 = 2;
    const L: u8 = 4;
    const R: u8 = 8;
    let connect = |lines: &mut std::collections::HashMap<(i32, i32), u8>, a: (i32, i32), b: (i32, i32)| {
        let (ba, bb) = if b.0 > a.0 {
            (R, L)
        } else if b.0 < a.0 {
            (L, R)
        } else if b.1 > a.1 {
            (D, U)
        } else {
            (U, D)
        };
        *lines.entry(a).or_default() |= ba;
        *lines.entry(b).or_default() |= bb;
    };
    let mut parents: Vec<_> = positions.iter().collect();
    parents.sort_by_key(|p| p.col);
    for parent in parents {
        let mut children: Vec<_> = positions.iter().filter(|child| child.parent == Some(parent.id)).collect();
        if children.is_empty() {
            continue;
        }
        children.sort_by_key(|p| p.row);
        let px = parent.col as i32 * stride_x as i32;
        let py = parent.row as i32 + 1;
        let bus_x = px + BOX_W as i32 + 2;
        if children.len() == 1 {
            connect(&mut lines, (px + BOX_W as i32 - 1, py), (px + BOX_W as i32, py));
            connect(&mut lines, (px + BOX_W as i32, py), (px + BOX_W as i32 + 1, py));
            connect(&mut lines, (px + BOX_W as i32 + 1, py), (bus_x, py));
            connect(&mut lines, (bus_x, py), (bus_x + 1, py));
            connect(&mut lines, (bus_x + 1, py), (bus_x + 2, py));
            connect(&mut lines, (bus_x + 2, py), (px + stride_x as i32 - 1, py));
        } else {
            let first_y = children[0].row as i32 + 1;
            let last_y = children.last().unwrap().row as i32 + 1;
            for y in first_y..last_y {
                connect(&mut lines, (bus_x, y), (bus_x, y + 1));
            }
            {
                let y = py;
                connect(&mut lines, (px + BOX_W as i32 - 1, y), (px + BOX_W as i32, y));
                connect(&mut lines, (px + BOX_W as i32, y), (px + BOX_W as i32 + 1, y));
                connect(&mut lines, (px + BOX_W as i32 + 1, y), (bus_x, y));
            }
            for child in children {
                let cy = child.row as i32 + 1;
                for x in bus_x..px + stride_x as i32 {
                    connect(&mut lines, (x, cy), (x + 1, cy));
                }
            }
        }
    }
    for (&(x, y), &mask) in &lines {
        let ch = match mask {
            12 => '─',
            3 => '│',
            10 => '┌',
            6 => '┐',
            9 => '└',
            5 => '┘',
            11 => '├',
            7 => '┤',
            14 => '┬',
            13 => '┴',
            15 => '┼',
            _ => continue,
        };
        put(f, x0 as i32 + x - scroll_x as i32, graph_top as i32 + y - scroll_y as i32, ch, line_style);
    }

    app.tree_rows.clear();
    for p in &positions {
        let x = gx(p);
        let y = gy(p);
        if x + BOX_W as i32 <= x0 as i32 || x >= (x0 + width) as i32 || y + 2 < graph_top as i32 || y >= detail_top as i32 {
            continue;
        }
        let picked = p.id == selected;
        let node = TREE[p.id].0;
        let section = !matches!(node, Node::Tech(_));
        let v = if picked && !section { '║' } else { '│' };
        let logical_x = p.col as i32 * stride_x as i32;
        let logical_y = p.row as i32 + 1;
        let left_has_edge = lines.get(&(logical_x, logical_y)).is_some_and(|mask| mask & L != 0);
        let right_has_edge = lines.get(&(logical_x + BOX_W as i32 - 1, logical_y)).is_some_and(|mask| mask & R != 0);
        put(
            f,
            x,
            y + 1,
            if left_has_edge {
                if picked {
                    '╣'
                } else {
                    '┤'
                }
            } else {
                v
            },
            if picked { head } else { line_style },
        );
        put(
            f,
            x + BOX_W as i32 - 1,
            y + 1,
            if right_has_edge {
                if picked {
                    '╠'
                } else {
                    '├'
                }
            } else {
                v
            },
            if picked { head } else { line_style },
        );
        let (tl, tr, bl, br, h, _v) = if section {
            ('╭', '╮', '╰', '╯', '─', '│')
        } else if picked {
            ('╔', '╗', '╚', '╝', '═', '║')
        } else {
            ('┌', '┐', '└', '┘', '─', '│')
        };
        for dx in 0..BOX_W as i32 {
            put(
                f,
                x + dx,
                y,
                if dx == 0 {
                    tl
                } else if dx == BOX_W as i32 - 1 {
                    tr
                } else {
                    h
                },
                if picked { head } else { line_style },
            );
            put(
                f,
                x + dx,
                y + 2,
                if dx == 0 {
                    bl
                } else if dx == BOX_W as i32 - 1 {
                    br
                } else {
                    h
                },
                if picked { head } else { line_style },
            );
        }
        let label = match node {
            Node::Tech(t) => fit(t.name(), 12),
            Node::Key(k) => fit(k.name(), 12),
            Node::Group(g) => fit(g, 12),
            Node::Start => String::new(),
        };
        let name_style = match node {
            Node::Key(Made::Item(i)) => Style::new().fg(item_cell(i, Color::Reset).1.fg.unwrap_or(KEY_ITEM)),
            // A fluid key reads in its own colour, like its pipes.
            Node::Key(Made::Fluid(f)) => Style::new().fg(Color::Indexed(fluid_colour(f))),
            _ if section => Style::new().fg(Color::White),
            _ => Style::new(),
        };
        let (mark, mark_style, name, name_style) = if section {
            (" ".to_string(), name_style, fit(&label, 13), name_style)
        } else if let Node::Tech(t) = node {
            let spans = tree_row_spans(&app.world, t, 12, true, true);
            (
                spans.first().map(|s| s.content.to_string()).unwrap_or_else(|| "· ".into()),
                spans.first().map(|s| s.style).unwrap_or(dim),
                spans.get(1).map(|s| s.content.to_string()).unwrap_or(label),
                spans.get(1).map(|s| s.style).unwrap_or(name_style),
            )
        } else {
            (String::new(), name_style, label, name_style)
        };
        for (dx, ch) in mark.chars().chain(name.chars()).take((BOX_W - 2) as usize).enumerate() {
            let style = if dx < 2 && !section { mark_style } else { name_style };
            put(f, x + 1 + dx as i32, y + 1, ch, style);
        }
        if crate::research::has_children(p.id) && !app.tree_expanded.contains(&p.id) {
            let stub = format!(" +{}", crate::research::hidden_descendants(p.id));
            put(f, x + BOX_W as i32 - 1, y + 1, if picked { '╟' } else { '├' }, line_style);
            put(f, x + BOX_W as i32, y + 1, '─', line_style);
            for (dx, ch) in stub.chars().take(4).enumerate() {
                put(f, x + BOX_W as i32 + 1 + dx as i32, y + 1, ch, dim);
            }
        }
        if y >= graph_top as i32 && y + 2 < detail_top as i32 && x >= x0 as i32 && x + BOX_W as i32 <= (x0 + width) as i32 {
            if let Some(t) = crate::research::tech_for_node(p.id) {
                app.tree_rows.push((Rect::new(x as u16, y as u16, BOX_W, 3), tree_rows().iter().position(|x| *x == t).unwrap_or(0)));
            }
        }
    }

    // Keep the selected tech and its unlock gate readable beneath the graph.
    let details = graph_node_details(&app.world, selected);
    for (i, (text, style)) in details.iter().take(4).enumerate() {
        let mut clipped = fit(text, width as usize);
        if clipped.chars().count() > width as usize {
            clipped = fit(&clipped, width as usize);
        }
        f.render_widget(Paragraph::new(Line::styled(clipped, *style)), Rect::new(x0, detail_top + i as u16, width, 1));
    }
    let hidden_left = scroll_x > 0;
    let hidden_right = scroll_x + graph_w < total_w;
    let hidden_up = scroll_y > 0;
    let hidden_down = scroll_y + graph_h < total_h;
    let mut more = Vec::new();
    if hidden_up {
        more.push("▲");
    }
    if hidden_left {
        more.push("◀");
    }
    if hidden_down {
        more.push("▼");
    }
    if hidden_right {
        more.push("▶");
    }
    let more = (!more.is_empty()).then(|| format!("{} more", more.join(" ")));
    let hints = if width < 60 {
        vec![(3, "↑↓ move"), (2, "→ open"), (2, "← close"), (1, "Enter"), (0, "Esc")]
    } else {
        vec![(3, "↑↓ move"), (2, "→ open"), (2, "← close"), (1, "Enter research"), (0, "Esc minimize")]
    };
    f.render_widget(Paragraph::new(footer_line(width, &hints, more.as_deref())), Rect::new(x0, footer_y, width, 1));
}

fn graph_node_details(w: &World, id: usize) -> Vec<(String, Style)> {
    match TREE[id].0 {
        Node::Tech(t) => graph_details(w, t),
        Node::Key(Made::Item(item)) => {
            let count = crate::research::hidden_descendants(id);
            let made = w.econ.made[item.idx()] > 0;
            let mut lines = vec![(
                format!("{} · {} to open {} techs", item.name(), if made { "made ✓" } else { "make 1" }, count),
                Style::new().fg(KEY_ITEM).add_modifier(Modifier::BOLD),
            )];
            let companies: Vec<_> = Company::ALL.iter().filter(|c| c.key_items().contains(&Made::Item(item))).map(|c| c.name()).collect();
            if !companies.is_empty() {
                lines.push((format!("Company: {}", companies.join(", ")), Style::new().fg(RESEARCH)));
            }
            lines
        }
        Node::Key(Made::Fluid(fluid)) => {
            let made = w.econ.made_fluid[fluid] > 0;
            let mut lines = vec![(
                format!(
                    "{} · {} to open {} techs",
                    fluid.name(),
                    if made { "made ✓" } else { "make 1" },
                    crate::research::hidden_descendants(id)
                ),
                Style::new().fg(KEY_ITEM).add_modifier(Modifier::BOLD),
            )];
            let companies: Vec<_> = Company::ALL.iter().filter(|c| c.key_items().contains(&Made::Fluid(fluid))).map(|c| c.name()).collect();
            if !companies.is_empty() {
                lines.push((format!("Company: {}", companies.join(", ")), Style::new().fg(RESEARCH)));
            }
            lines
        }
        Node::Group(name) => vec![(
            format!("{name} · {} techs", crate::research::hidden_descendants(id)),
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )],
        Node::Start => vec![],
    }
}

fn graph_details(w: &World, t: Tech) -> Vec<(String, Style)> {
    use crate::content::{Node, TREE};
    let head = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let note = Style::new().fg(RESEARCH);
    let dim = Style::new().fg(Color::DarkGray);
    let (key, n) = t.milestone();
    let made = w.econ.made_of(key).min(n);
    let status = if w.progress.has(t) {
        "researched".to_string()
    } else if w.econ.research + 1e-9 >= t.cost() {
        "affordable".to_string()
    } else {
        format!("{} RP", grouped(t.cost()))
    };
    let tree_idx = TREE.iter().position(|(node, _)| *node == Node::Tech(t));
    let mut chain = Vec::new();
    let mut at = tree_idx;
    while let Some(i) = at {
        match TREE[i].0 {
            Node::Tech(t) => chain.push(t.name().to_string()),
            Node::Key(k) => chain.push(k.name().to_string()),
            Node::Group(g) => chain.push(g.to_string()),
            Node::Start => {}
        }
        at = TREE[i].1.filter(|p| *p != 0);
    }
    chain.reverse();
    let path = chain.join(" → ");
    let gate_item = keys_text(t.requires());
    let gate = if w.progress.is_open(t.company()) {
        format!("Gate: {} · {}", t.company().name(), if gate_item == "its key item" { "starting machinery" } else { &gate_item })
    } else {
        format!("Opens with {} from {}", gate_item, t.company().name())
    };
    vec![
        (format!("{} · {} · {}", t.name(), t.category().name(), status), head),
        (format!("{} RP · free at {} {} ({} made)", grouped(t.cost()), grouped(n as f64), key.name(), grouped(made as f64)), note),
        (format!("Path: {path}"), dim),
        (gate, dim),
    ]
}

/// Machines that craft, in the order the book lists them.
const CRAFTERS: [Kind; 28] = [
    Kind::Crusher,
    Kind::Furnace,
    Kind::Molder,
    Kind::BlastFurnace,
    Kind::ArcFurnace,
    Kind::CokeOven,
    Kind::Kiln,
    Kind::LargeKiln,
    Kind::Mixer,
    Kind::Evacuator,
    Kind::Press,
    Kind::WireDrawer,
    Kind::Winder,
    Kind::RollingMill,
    Kind::Lathe,
    Kind::Welder,
    Kind::CrudeAssembler,
    Kind::BasicAssembler,
    Kind::AdvancedAssembler,
    Kind::PrecisionAssembler,
    Kind::Etcher,
    Kind::FineEtcher,
    Kind::CrystalGrower,
    Kind::WaferSaw,
    Kind::LargeBasicAssembler,
    Kind::LargeAdvancedAssembler,
    Kind::LargePrecisionAssembler,
    Kind::Still,
];

/// One plain line on what a machine is for (the inspector and its book page show it).
fn blurb(kind: Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::Crusher => "grinds ore into grit, which melts further, and stone into sand",
        Kind::Furnace => "melts ore or grit into molten metal, and bakes coal into coke",
        Kind::Molder => "casts molten metal and silicon into ingots",
        Kind::BlastFurnace => {
            "melts iron fast, makes molten steel and hardened plate from plate and coke or coal, and molten silicon from sand and coke; most leave waste out the back"
        }
        Kind::CokeOven => "cooks its own coal into coke: no EC, no fuel, but it leaves waste out the back",
        Kind::Press => "presses ingots into plates, and plates and steel into wire, pipe, gears and girders",
        Kind::WireDrawer => "draws plate into wire, and wire into fine wire",
        Kind::Winder => "winds wire into coils, and iron rods into springs",
        Kind::RollingMill => "rolls plate flat into sheets, and steel into steel sheets and girders",
        Kind::Lathe => "turns plate into rods and screws, steel into steel rods and bearings, and silicon ingots into wafers (and scrap)",
        Kind::Kiln => "fires eight at once, burning coal: stone into brick, sand into glass, tubes (leaving waste), dopant, doped wafers and ceramic",
        Kind::LargeKiln => "fires 32 at once, every 20 seconds: four kilns in one, burning four kilns' coal",
        Kind::Mixer => "takes water from a pipe and mixes concrete, then reinforced concrete, and etchant from coal and sand (leaving waste)",
        Kind::Evacuator => "pulls the air out of glass with a water jet from a pipe: 2 tubes a run, no waste",
        Kind::Etcher => "etches dies from doped wafers with a mask and etchant: 2 dies and 2 scrap a wafer",
        Kind::ArcFurnace => "melts sand to silicon on power alone, with half the blast furnace's waste, and remelts silicon scrap",
        Kind::CrystalGrower => "grows silicon crystals from ingots, slowly: 20 s each",
        Kind::FineEtcher => "etches a fine die and a spare die from each wafer with a fine mask: no scrap, but very hungry",
        Kind::WaferSaw => "saws a crystal into 6 wafers with a fine wire: twice the lathe's wafers per ingot, no scrap",
        Kind::Welder => "welds steel into boiler shells, silo shells and frames; EC only while it works",
        Kind::CrudeAssembler => "builds valves, gauges, lamps, relays, gearboxes, transformers and lamp matrices",
        Kind::BasicAssembler => "builds electric motors, pistons and terminals, and the crude parts twice as fast",
        Kind::AdvancedAssembler => {
            "builds control boards (the x1x1, the weakest microprocessor), then tube boards (x2x1), circuit boards, transistors, transistor boards (x3x1) and masks, and everything below faster"
        }
        Kind::PrecisionAssembler => "builds microprocessors (x4x1 and x4x2), RAM chips, fine masks and mainboards, and control boards twice as fast as an advanced one; nothing below",
        Kind::LargeBasicAssembler => "builds the big goods: pump units and engine blocks",
        Kind::LargeAdvancedAssembler => "builds generators, control panels, drilling rigs and steam tractors; the large basic goods faster",
        Kind::LargePrecisionAssembler => "builds the computer, and the large advanced goods twice as fast; nothing below",
        Kind::AdvancedDrill => "a drill on EC: twice a basic drill's ore, no coal",
        Kind::LargeBasicDrill => "an 8x8 drill: 5x a basic drill's ore, burns 10x the coal",
        Kind::LargeAdvancedDrill => "an 8x8 drill on EC: 5x an advanced drill's ore",
        Kind::AdvancedPump => "a pump that gives four times the water",
        Kind::OilExtractor => "draws crude oil out of the ground into a pipe at its front, faster on deeper floors",
        Kind::LiquidTrader => "a trade post for fluids: it sells what the pipes it touches hold, for money or research",
        Kind::Still => "splits crude oil into naphtha, and naphtha into ethylene, leaving waste out the back",
        _ => return None,
    })
}
#[allow(dead_code)]
const LINK: Color = Color::Indexed(117);

/// Whether the player can get this item yet (or already has).
#[allow(dead_code)]
fn item_known(w: &World, item: Item) -> bool {
    let mined = matches!(item, Item::CopperOre | Item::Coal | Item::IronOre | Item::Stone);
    mined
        || w.econ.made[item.idx()] > 0
        || CRAFTERS.iter().any(|&k| {
            w.progress.part_unlocked(k)
                && k.recipes().iter().any(|r| r.outputs.iter().any(|o| o.0 == item) && r.needs.is_none_or(|t| w.progress.has(t)))
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
#[allow(dead_code)]
struct Seg {
    text: String,
    style: Style,
    #[allow(dead_code)]
    link: Option<Entry>,
}

#[allow(dead_code)]
fn seg(text: impl Into<String>, style: Style) -> Seg {
    Seg { text: text.into(), style, link: None }
}

/// A link to a page; the page you're on isn't a link to itself.
#[allow(dead_code)]
fn link(e: Entry, here: Entry) -> Seg {
    if e == here {
        seg(e.name(), Style::new().add_modifier(Modifier::BOLD))
    } else {
        Seg { text: e.name().to_string(), style: Style::new().fg(LINK), link: Some(e) }
    }
}

type Row = Vec<Seg>;

/// Word-wraps pieces to `width`; rows after the first start at `hang`. Links stay links.
#[allow(dead_code)]
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
            if col + n > width && col > hang {
                rows.push(vec![seg(" ".repeat(hang), Style::new())]);
                col = hang;
            }
            continue;
        }
        col += n;
        rows.last_mut().unwrap().push(t);
    }
    rows
}

/// A recipe as a word equation: "2 iron gear + steel ─coal─→ gearbox", with the fuel over the arrow.
#[allow(dead_code)]
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
    // A fluid is plain coloured text (it has no page of its own).
    let fluid = |f: Option<(Fluid, u32)>, after: bool| -> Vec<Seg> {
        let Some((fl, n)) = f else { return Vec::new() };
        let mut v = Vec::new();
        if after {
            v.push(seg(" + ", Style::new().fg(Color::DarkGray)));
        }
        if n > 1 {
            v.push(seg(format!("{n} "), Style::new().add_modifier(Modifier::BOLD)));
        }
        v.push(seg(fl.name(), Style::new().fg(Color::Indexed(fluid_colour(fl)))));
        v
    };
    let mut out = side(r.inputs);
    out.extend(fluid(r.fluid_in, !r.inputs.is_empty()));
    out.push(seg(arrow, Style::new().fg(Color::Indexed(244))));
    // A fluid product comes first, then the items (molten steel + waste).
    out.extend(fluid(r.fluid_out, false));
    if r.fluid_out2.is_some() {
        out.push(seg(" + ", Style::new().fg(Color::DarkGray)));
        out.extend(fluid(r.fluid_out2, false));
        out.push(seg(" (back)", Style::new().fg(Color::DarkGray)));
    }
    if (r.fluid_out.is_some() || r.fluid_out2.is_some()) && !r.outputs.is_empty() {
        out.push(seg(" + ", Style::new().fg(Color::DarkGray)));
    }
    out.extend(side(r.outputs));
    out
}

/// Why a recipe can't be used yet, if it can't.
#[allow(dead_code)]
fn recipe_lock(w: &World, k: Kind, r: &Recipe) -> Option<String> {
    if !w.progress.part_unlocked(k) {
        Some(format!("needs the {}", k.name().to_lowercase()))
    } else {
        r.needs.filter(|t| !w.progress.has(*t)).map(|t| format!("needs {}", t.name()))
    }
}

/// The same recipe (what goes in and out), whatever machine runs it or how fast.
fn same_recipe(a: &Recipe, b: &Recipe) -> bool {
    a.name == b.name
        && a.inputs == b.inputs
        && a.outputs == b.outputs
        && a.fluid_in == b.fluid_in
        && a.fluid_out == b.fluid_out
        && a.fluid_out2 == b.fluid_out2
}

#[allow(dead_code)]
fn timing(r: &Recipe, made: u32) -> String {
    let per_min = made as f64 * 60.0 * TPS as f64 / r.ticks as f64;
    format!("{:.1}s · {per_min:.0}/min", r.ticks as f64 / TPS as f64)
}

/// A book page as rows: a title, then labelled rows (no headings), then (for items) the research chart.
#[allow(dead_code)]
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
                Item::CopperOre => {
                    Some((vec![Kind::Drill, Kind::BasicDrill], "anywhere: press f on a drill to pick its ore; faster underground"))
                }
                Item::Coal => Some((vec![Kind::Drill, Kind::BasicDrill], "anywhere (f picks the ore); a drill on coal fuels itself")),
                Item::IronOre => Some((vec![Kind::Drill, Kind::BasicDrill], "anywhere (f picks the ore); a cheap drill is half as fast")),
                Item::Stone => Some((vec![Kind::Drill, Kind::BasicDrill], "anywhere (f picks the ore)")),
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
                labelled(&mut out, label(&mut first, "Sources"), GUTTER, segs);
            }
            // Machines that share a recipe (the assembler ladder) are listed together, with its times.
            let mut makers: Vec<Vec<(Kind, &'static Recipe)>> = Vec::new();
            for k in CRAFTERS {
                for r in k.recipes().iter().filter(|r| r.outputs.iter().any(|o| o.0 == item)) {
                    match makers.iter_mut().find(|g| same_recipe(g[0].1, r)) {
                        Some(g) => g.push((k, r)),
                        None => makers.push(vec![(k, r)]),
                    }
                }
            }
            for group in &makers {
                for (i, (k, r)) in group.iter().enumerate() {
                    let made = r.outputs.iter().find(|o| o.0 == item).map_or(1, |o| o.1);
                    let mut segs = vec![link(Entry::Part(*k), e), seg(format!("  {}", timing(r, made)), dim)];
                    if let Some(l) = recipe_lock(w, *k, r) {
                        segs.push(seg(format!("  ({l})"), warn));
                    }
                    if i + 1 < group.len() {
                        segs.push(seg(",", dim));
                    }
                    labelled(&mut out, label(&mut first, "Sources"), GUTTER, segs);
                }
                labelled(&mut out, "", EQ, equation(group[0].1, e));
            }
            if first {
                labelled(&mut out, "Sources", GUTTER, vec![seg("nothing yet", dim)]);
            }

            // Uses: one equation per recipe, then the machines that run it.
            let mut uses: Vec<(&'static Recipe, Vec<Kind>)> = Vec::new();
            for k in CRAFTERS {
                for r in k.recipes().iter().filter(|r| r.inputs.iter().any(|i| i.0 == item)) {
                    match uses.iter_mut().find(|u| {
                        u.0.name == r.name
                            && u.0.inputs == r.inputs
                            && u.0.outputs == r.outputs
                            && u.0.fluid_out == r.fluid_out
                            && u.0.fluid_out2 == r.fluid_out2
                    }) {
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
                labelled(&mut out, label(&mut first, "Uses"), GUTTER, segs);
            }
            let fuel: Option<(Vec<Kind>, &str)> = match item {
                Item::Coal => Some((vec![Kind::Drill, Kind::Furnace, Kind::Boiler, Kind::Kiln, Kind::LargeKiln], "")),
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
                labelled(&mut out, label(&mut first, "Uses"), GUTTER, segs);
            }
            if first && item == Item::Waste {
                labelled(&mut out, "Uses", GUTTER, vec![seg("nothing: a trade post set to money takes it, for a fee", dim)]);
            } else if first {
                labelled(&mut out, "Uses", GUTTER, vec![seg("nothing yet: sell it at a trade post", dim)]);
            }

            let mut first = true;
            let key = Style::new().fg(KEY_ITEM);
            if let Some(i) = TREE.iter().position(|(n, _)| *n == Node::Key(Made::Item(item))) {
                let next: Vec<String> = tree_children(i).iter().map(|&k| node_name(TREE[k].0)).collect();
                let letter = key_letter_text(Made::Item(item));
                let done = if w.econ.made[item.idx()] > 0 { " (done)" } else { "" };
                let text = format!("your first one brings {}{letter}{done}", next.join(", "));
                labelled(&mut out, label(&mut first, "Unlocks"), GUTTER, vec![seg(text, key)]);
            }
            for t in Tech::ALL.iter().filter(|t| t.milestone().0 == Made::Item(item) && !w.progress.has(**t)) {
                let n = t.milestone().1;
                let text = format!("make {n} to unlock {} free ({}/{n})", t.name(), w.econ.made[item.idx()].min(n));
                labelled(&mut out, label(&mut first, "Unlocks"), GUTTER, vec![seg(text, key)]);
            }
            out.push(vec![]);
            if item == Item::Waste {
                out.push(vec![seg("Leaves its machine by the back edge, opposite the arrow; products leave the front.", dim)]);
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
            title.push(seg(format!("   ${} · {kw}x{kh}{from}", grouped(k.cost())), dim));
            out.push(title);
            out.push(vec![]);

            // Can you build it yet, and if not, what opens it.
            let status = if w.progress.part_unlocked(k) {
                vec![seg("ready to build", Style::new().fg(Color::Green))]
            } else {
                let mut segs = Vec::new();
                if let Some(c) = k.company().filter(|c| !w.progress.is_open(*c)) {
                    segs.push(seg("opens when you first make ", warn));
                    if c.key_items().is_empty() {
                        segs.push(seg("its key item", warn));
                    }
                    for (n, k) in c.key_items().iter().enumerate() {
                        if n > 0 {
                            segs.push(seg(" or ", warn));
                        }
                        segs.push(match k {
                            Made::Item(i) => link(Entry::Item(*i), e),
                            Made::Fluid(f) => seg(f.name(), warn),
                        });
                    }
                    if k.tech().is_some() {
                        segs.push(seg(", then ", warn));
                    }
                }
                if let Some(t) = k.tech().filter(|t| !w.progress.has(*t)) {
                    let (key, n) = t.milestone();
                    segs.push(seg(format!("research {} ({} RP) or make {n} ", t.name(), grouped(t.cost())), warn));
                    segs.push(match key {
                        Made::Item(i) => link(Entry::Item(i), e),
                        Made::Fluid(f) => seg(f.name(), warn),
                    });
                    segs.push(seg(format!(" ({}/{n})", w.econ.made_of(key).min(n)), warn));
                }
                segs
            };
            labelled(&mut out, "Status", GUTTER, status);
            if let Some(t) = blurb(k) {
                labelled(&mut out, "About", GUTTER, vec![seg(t, Style::new())]);
            }

            match k {
                _ if k.is_drill() => {
                    let mut segs = Vec::new();
                    for (i, m) in k.ores().iter().enumerate() {
                        if i > 0 {
                            segs.push(seg(" · ", dim));
                        }
                        segs.push(link(Entry::Item(*m), e));
                    }
                    labelled(&mut out, "Mines", GUTTER, segs);
                    if k.burns_coal() {
                        labelled(
                            &mut out,
                            "Burns",
                            GUTTER,
                            vec![link(Entry::Item(Item::Coal), e), seg(" (a drill on coal fuels itself)", dim)],
                        );
                    } else {
                        labelled(&mut out, "Runs on", GUTTER, vec![seg(format!("{:.0} EC, no coal", k.power_draw()), Style::new())]);
                    }
                    if k == Kind::Drill {
                        labelled(&mut out, "", GUTTER, vec![seg("iron at half the basic drill's speed", dim)]);
                    }
                    let (mult, burn) = k.drill_rates();
                    if mult > 1 || burn > 1 {
                        labelled(&mut out, "Speed", GUTTER, vec![seg(drill_speed_lines(k).join(", "), Style::new())]);
                    }
                    if k.dims(Dir::E).0 >= 8 {
                        labelled(&mut out, "", GUTTER, vec![seg("pushes into anything along its front edge", dim)]);
                    }
                }
                _ if k.is_pump() => {
                    labelled(
                        &mut out,
                        "Pumps",
                        GUTTER,
                        vec![seg(format!("{:.0} water a second, anywhere on the ground", k.pump_rate()), Style::new())],
                    );
                    labelled(&mut out, "Runs on", GUTTER, vec![seg(format!("{:.0} EC", k.power_draw()), Style::new())]);
                }
                _ if k.is_extractor() => {
                    labelled(
                        &mut out,
                        "Pumps",
                        GUTTER,
                        vec![seg("1 crude oil every 2 s, into a pipe at its front; richer below", Style::new())],
                    );
                    labelled(&mut out, "Runs on", GUTTER, vec![seg(format!("{:.0} EC", k.power_draw()), Style::new())]);
                }
                _ if k.recipes().is_empty() => {
                    // A part that crafts nothing: its rules, a few short lines.
                    for (i, line) in k.rules().unwrap_or(&[]).iter().enumerate() {
                        labelled(&mut out, if i == 0 { "Rules" } else { "" }, GUTTER, vec![seg(*line, Style::new())]);
                    }
                }
                _ => {
                    let mut first = true;
                    for r in k.recipes() {
                        let made = r.outputs.first().map(|o| o.1).or(r.fluid_out.map(|f| f.1)).or(r.fluid_out2.map(|f| f.1)).unwrap_or(1)
                            * k.batch();
                        let mut segs = equation(r, e);
                        segs.push(seg(format!("   {}", timing(r, made)), dim));
                        if let Some(l) = r.needs.filter(|t| !w.progress.has(*t)).map(|t| format!("needs {}", t.name())) {
                            segs.push(seg(format!("  ({l})"), warn));
                        }
                        labelled(&mut out, if std::mem::take(&mut first) { "Recipes" } else { "" }, GUTTER, segs);
                    }
                    labelled(&mut out, "", GUTTER, vec![seg(format!("{setup_key} on a placed one picks the recipe"), dim)]);
                    if k.recipes().iter().any(|r| r.outputs.iter().any(|o| o.0 == Item::Waste)) {
                        let text = " leaves out the back (opposite the arrow), never the front: belt it away or put a crate there. Blocked, the machine stops (BACK)";
                        labelled(&mut out, "Waste", GUTTER, vec![link(Entry::Item(Item::Waste), e), seg(text, Style::new())]);
                    }
                    if k.batch() > 1 {
                        let text = format!("fills up for {} runs, then makes them all at once", k.batch());
                        labelled(&mut out, "Batch", GUTTER, vec![seg(text, Style::new())]);
                    }
                    if k.burst_draw() > 0.0 {
                        let text = format!("{:.0} EC only while working, none when idle", k.burst_draw());
                        labelled(&mut out, "Power", GUTTER, vec![seg(text, Style::new())]);
                    } else if k.power_draw() > 0.0 {
                        labelled(&mut out, "Power", GUTTER, vec![seg(format!("{:.0} EC", k.power_draw()), Style::new())]);
                    }
                }
            }
        }
    }
    out
}

/// How much research each sale of an item gives, as a bar chart: it starts at the item's
/// worth and fades with every sale down to a floor. Sales already made are grey, the ones
/// ahead in colour; the y axis marks the start and the floor, and a marker shows where you are.
#[allow(dead_code)]
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
    let frame = if !app.book_full && !app.compact_now && area.width >= 90 && area.height >= 30 {
        centered(area, (area.width * 4 / 5).min(120), (area.height * 85 / 100).min(50))
    } else {
        area
    };
    app.book_area = frame;
    if frame.right() < area.right() {
        let shadow = Rect::new(frame.right(), frame.y.saturating_add(1), 1, frame.height.saturating_sub(1));
        f.render_widget(Paragraph::new(" ".repeat(shadow.height as usize)).style(Style::new().bg(Color::Indexed(232))), shadow);
    }
    if frame.bottom() < area.bottom() {
        let shadow = Rect::new(frame.x.saturating_add(1), frame.bottom(), frame.width.saturating_sub(1), 1);
        f.render_widget(Paragraph::new(" ".repeat(shadow.width as usize)).style(Style::new().bg(Color::Indexed(232))), shadow);
    }
    f.render_widget(Clear, frame);
    f.render_widget(Block::bordered(), frame);
    // The title bar: a window's three dots, then the browser's name.
    for (i, color) in [Color::Indexed(167), Color::Indexed(221), Color::Indexed(114)].into_iter().enumerate() {
        f.buffer_mut().set_string(frame.x + 2 + i as u16 * 2, frame.y, "●", Style::new().fg(color));
    }
    f.buffer_mut().set_stringn(frame.x + 8, frame.y, " termweb ", 9, Style::new().fg(Color::Indexed(249)).bold());
    let inner = Block::bordered().inner(frame);
    app.book_buttons.clear();
    app.book_nav.clear();
    for (i, button) in
        [crate::app::BookButton::CloseWindow, crate::app::BookButton::Minimize, crate::app::BookButton::Fullscreen].into_iter().enumerate()
    {
        app.book_buttons.push((Rect::new(frame.x + 2 + i as u16 * 2, frame.y, 1, 1), button));
    }
    // Blank bits of the title bar, the tab strip and the toolbar take a double click.
    app.book_nav.push(Rect::new(frame.x + 17, frame.y, frame.width.saturating_sub(18), 1));
    app.book_nav.push(Rect::new(inner.x, inner.y, inner.width, 1));
    // The tab strip: a chip per tab, above the toolbar.
    app.book_tab_hits.clear();
    app.book_tab_close.clear();
    let close_chips = inner.width >= 80;
    let mut chip_x = inner.x + 1;
    for (i, tab) in app.book_tabs.clone().iter().enumerate() {
        let room = inner.right().saturating_sub(chip_x + 3) as usize;
        if room < 6 {
            break;
        }
        let title = tab.title();
        let tail = if close_chips { " ✕ " } else { " " };
        let fixed = 1 + tail.chars().count();
        // A tab's name stays short, whatever the page is called.
        let mut label: String = title.chars().take(room.saturating_sub(fixed).clamp(1, 16)).collect();
        if label.chars().count() < title.chars().count() {
            label.push('…');
        }
        let chip = format!(" {label}{tail}");
        let w = chip.chars().count() as u16;
        let active = i == app.book_active;
        let style = if active {
            Style::new().fg(Color::Indexed(255)).bg(Color::Indexed(238)).bold()
        } else {
            Style::new().fg(Color::Indexed(245)).bg(Color::Indexed(234))
        };
        f.buffer_mut().set_stringn(chip_x, inner.y, &chip, w as usize, style);
        app.book_tab_hits.push((Rect::new(chip_x, inner.y, w, 1), i));
        if close_chips {
            app.book_tab_close.push((Rect::new(chip_x + w - 2, inner.y, 1, 1), i));
        }
        chip_x += w + 1;
    }
    if inner.right().saturating_sub(chip_x) > 3 {
        f.buffer_mut().set_string(chip_x + 1, inner.y, "+", Style::new().fg(Color::Indexed(245)));
        app.book_buttons.push((Rect::new(chip_x + 1, inner.y, 1, 1), crate::app::BookButton::NewTab));
    }
    // The toolbar: back, forward, reload and home, each two cells wide.
    let bar_y = inner.y + 1;
    app.book_nav.push(Rect::new(inner.x, bar_y, inner.width, 1));
    let dim = |on: bool| Style::new().fg(if on { Color::Indexed(250) } else { Color::Indexed(240) }).bg(Color::Indexed(236));
    for (i, (glyph, on, button)) in [
        ('◀', !app.book_history.is_empty(), crate::app::BookButton::Back),
        ('▶', !app.book_forward.is_empty(), crate::app::BookButton::Forward),
        ('⟳', true, crate::app::BookButton::Reload),
        ('⌂', true, crate::app::BookButton::Home),
    ]
    .into_iter()
    .enumerate()
    {
        let x = inner.x + i as u16 * 2;
        f.buffer_mut().set_string(x, bar_y, glyph.to_string(), dim(on));
        app.book_buttons.push((Rect::new(x, bar_y, 1, 1), button));
    }
    // The address field, a rounded box with a lock and no https://.
    // Below 40 columns the lock and the star step aside so the address itself still fits.
    let phone = inner.width < 40;
    let editing = app.book_url_edit.is_some();
    let url = if let Some(text) = &app.book_url_edit {
        if app.book_url_fresh {
            text.clone()
        } else {
            format!("{text}▏")
        }
    } else {
        format!(
            "{}{}",
            book_url(&app.book_slug),
            if app.book_typing && app.book_slug != "start" && !app.book_slug.starts_with("search?q=") {
                format!("?{}▏", app.book_query)
            } else {
                String::new()
            }
        )
    };
    let field_x = inner.x + 9;
    let field_w = inner.width.saturating_sub(if phone { 9 } else { 13 });
    if field_w >= 4 {
        let mut field = Style::new().fg(Color::Indexed(252)).bg(Color::Indexed(238));
        if editing {
            // A field being edited is brighter; its text is reversed while it is all selected.
            field = Style::new().fg(Color::Indexed(255)).bg(Color::Indexed(240));
        }
        f.buffer_mut().set_string(field_x, bar_y, " ".repeat(field_w as usize), field);
        let text = fit_left(&url, field_w.saturating_sub(2) as usize);
        let text_style = if editing && app.book_url_fresh { field.reversed() } else { field };
        f.buffer_mut().set_stringn(field_x + 1, bar_y, &text, field_w.saturating_sub(2) as usize, text_style);
        app.book_buttons.push((Rect::new(field_x, bar_y, field_w, 1), crate::app::BookButton::Address));
    }
    // The star that toggles a bookmark: filled when this page is on the bar.
    if !phone && inner.width >= 14 {
        let marked = app.settings.bookmarks.contains(&app.book_slug);
        let star = Rect::new(inner.right().saturating_sub(2), bar_y, 1, 1);
        f.buffer_mut().set_string(
            star.x,
            star.y,
            if marked { "★" } else { "☆" },
            Style::new().fg(if marked { Color::Indexed(221) } else { Color::Indexed(245) }).bg(Color::Indexed(236)),
        );
        app.book_buttons.push((star, crate::app::BookButton::Star));
    }
    f.buffer_mut().set_string(inner.x, bar_y, " ", Style::new().bg(Color::Indexed(236)));
    // The page sits under the bookmarks bar, with a column kept free for the scrollbar when it has one.
    let bar_col = u16::from(app.book_scrollbar && inner.width > 20);
    let content = Rect::new(inner.x, inner.y.saturating_add(4), inner.width.saturating_sub(bar_col), inner.height.saturating_sub(5));
    // A thin rule under the address, which the load bar fills for a moment after a page opens.
    let rule_y = inner.y + 2;
    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::new().fg(Color::DarkGray)),
        Rect::new(inner.x, rule_y, inner.width, 1),
    );
    if let Some(opened) = app.book_opened {
        let part = opened.elapsed().as_secs_f32() / 0.3;
        if part < 1.0 {
            let done = (inner.width as f32 * part) as u16;
            f.render_widget(
                Paragraph::new("━".repeat(done as usize)).style(Style::new().fg(Color::Indexed(117))),
                Rect::new(inner.x, rule_y, done, 1),
            );
        }
    }
    // The bookmarks bar: a star and the page's name for each bookmark.
    app.book_mark_hits.clear();
    {
        let y = inner.y + 3;
        let mut x = inner.x + 1;
        let style = Style::new().fg(Color::Indexed(250));
        for slug in app.settings.bookmarks.clone() {
            let title = match crate::wiki::page(&slug).and_then(|p| p.lines().next()) {
                _ if slug == "home" => "Terminactory Wiki".to_string(),
                Some(line) => line.trim_start_matches('#').trim().to_string(),
                None => slug.chars().next().map_or(String::new(), |c| c.to_uppercase().chain(slug.chars().skip(1)).collect()),
            };
            let room = inner.right().saturating_sub(x + 1) as usize;
            if room < 6 {
                break;
            }
            let mut label: String = format!("★ {title}").chars().take(room).collect();
            if label.chars().count() < title.chars().count() + 2 {
                label.pop();
                label.push('…');
            }
            let w = label.chars().count() as u16;
            let active = slug == app.book_slug;
            f.buffer_mut().set_string(x, y, &label, if active { style.underlined() } else { style });
            app.book_mark_hits.push((Rect::new(x, y, w, 1), slug));
            x += w + 2;
        }
    }
    app.book_links.clear();
    app.book_urls.clear();
    app.book_post_hits.clear();
    app.book_fold_hits.clear();
    app.book_click_only.clear();
    app.book_max_scroll = u16::MAX;
    app.book_scroll_step = 1;
    if crate::web::is_addr(&app.book_slug) {
        draw_web_page(f, content, app);
    } else if app.book_slug == "start" {
        draw_book_start(f, content, app);
    } else if app.book_slug == "history" {
        draw_book_history(f, content, app);
    } else if app.book_slug == "production" {
        draw_stats(f, content, app);
        app.book_max_scroll = 0;
    } else if app.book_slug == "research" {
        draw_tree(f, content, app);
        app.book_max_scroll = 0;
    } else if app.book_slug.starts_with("search?q=") {
        draw_book_results(f, content, app);
    } else if app.book_slug == "items" || app.book_slug == "machines" {
        draw_book_index(f, content, app, app.book_slug == "machines");
    } else if app.book_slug == "topics" {
        let topics = [
            "getting-started",
            "power",
            "waste",
            "floors-and-elevators",
            "research",
            "companies",
            "belts-and-logistics",
            "fluids-and-pipes",
            "tools-and-blueprints",
        ];
        let mut y = content.y;
        for slug in topics {
            let title = crate::wiki::page(slug).and_then(|p| p.lines().next()).unwrap_or(slug).trim_start_matches('#').trim();
            f.buffer_mut().set_string(content.x, y, format!("› {title}"), Style::new().fg(Color::Cyan).underlined());
            app.book_urls.push((Rect::new(content.x, y, content.width, 1), slug.to_string()));
            y += 1;
        }
    } else {
        let home = book_home_markdown(&app.world, app.book_seed);
        let md = if app.book_slug == "home" { home.as_str() } else { crate::wiki::page(&app.book_slug).unwrap_or("# Page missing") };
        {
            let narrow = content.width < 90;
            let has_contents = content.width >= 110 && app.book_slug != "home";
            let (prose, info) = if app.book_slug == "home"
                || Entry::list("", false).iter().chain(Entry::list("", true).iter()).all(|e| crate::wiki::slug(e.name()) != app.book_slug)
            {
                (content, None)
            } else if narrow {
                (content, Some(content))
            } else {
                let info_width = if has_contents {
                    30
                } else if area.width >= 120 {
                    46
                } else {
                    40
                };
                let usable = if has_contents { Rect { width: content.width.saturating_sub(16), ..content } } else { content };
                let [left, right] = Layout::horizontal([Constraint::Min(36), Constraint::Length(info_width)]).areas(usable);
                let _ = left;
                (usable, Some(right))
            };
            let info = info;
            let entry =
                Entry::list("", false).into_iter().chain(Entry::list("", true)).find(|e| crate::wiki::slug(e.name()) == app.book_slug);
            let page_md = entry.map_or_else(|| md.to_string(), |e| book_page_markdown(&app.world, e, md, prose.width as usize));
            let page_area = prose;
            if narrow {
                render_wiki_text(f, page_area, &page_md, app, None, entry, PageKind::WikiNarrow);
            } else {
                render_wiki_text(f, page_area, &page_md, app, info, entry, PageKind::Wiki);
            }
            if has_contents {
                draw_wiki_contents(f, Rect::new(content.right().saturating_sub(16), content.y, 16, content.height), &page_md, app);
            }
        }
    }
    // A roll-over to the bottom asks for the last link: keep the pick on a real link.
    if !app.book_urls.is_empty() && app.book_query.is_empty() {
        app.book_selected = app.book_selected.min(app.book_urls.len() - 1);
    }
    // The scrollbar down the right edge, whenever the page is taller than the view.
    let scrollable = app.book_max_scroll > 0 && app.book_max_scroll != u16::MAX && content.height >= 4;
    if scrollable && bar_col == 1 {
        let x = inner.right().saturating_sub(1);
        let track = content.height.saturating_sub(2) as u32;
        let total = app.book_max_scroll as u32 + content.height as u32;
        let thumb = (track * content.height as u32 / total).max(1).min(track);
        let pos = if app.book_max_scroll == 0 {
            0
        } else {
            (track - thumb) * app.book_scroll.min(app.book_max_scroll) as u32 / app.book_max_scroll as u32
        };
        let dim = Style::new().fg(Color::Indexed(240));
        f.buffer_mut().set_string(x, content.y, "▲", dim);
        f.buffer_mut().set_string(x, content.bottom().saturating_sub(1), "▼", dim);
        for i in 0..track {
            let on = i >= pos && i < pos + thumb;
            f.buffer_mut().set_string(
                x,
                content.y + 1 + i as u16,
                if on { "█" } else { "░" },
                if on { Style::new().fg(Color::Indexed(250)) } else { dim },
            );
        }
    }
    app.book_scrollbar = scrollable;
    // The status bar: where the selected link goes, and that the page has loaded.
    let foot = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
    f.buffer_mut().set_stringn(foot.x, foot.y, " ".repeat(foot.width as usize), foot.width as usize, Style::new().bg(Color::Indexed(236)));
    let left = if app.book_typing {
        "Type to search · Enter opens · Esc stops".to_string()
    } else {
        let slug = app.book_urls.get(app.book_selected).map_or(app.book_slug.clone(), |(_, s)| s.clone());
        book_url(&slug)
    };
    // The close hint shares the right end where there is room for it, and a signed-in
    // domain says who is reading.
    let close = if foot.width >= 44 {
        "b new tab · Esc minimize"
    } else if foot.width >= 30 {
        "Esc minimize"
    } else {
        "Esc"
    };
    let signed = crate::web::is_addr(&app.book_slug).then(|| app.book_signed_in(crate::web::domain_of(&app.book_slug))).flatten();
    let right = match signed {
        Some(user) if foot.width >= 44 => {
            let full = format!("signed in as {user} · {close}");
            if full.chars().count() + 2 <= foot.width as usize {
                full
            } else {
                close.to_string()
            }
        }
        _ => close.to_string(),
    };
    // The Copy button sits left of the right-hand text, where the page has something to copy.
    let copied = app.book_copied.is_some_and(|t| t.elapsed().as_secs() < 2);
    // On the first termweb.search page the same spot holds History instead.
    let on_start = app.book_slug == "start";
    let copy_label = if on_start {
        " History "
    } else if copied {
        " Copied "
    } else {
        " Copy page "
    };
    let copy_w = copy_label.chars().count() as u16;
    let can_copy = foot.width >= right.chars().count() as u16 + copy_w + 24
        && (on_start
            || (!app.book_typing
                && app.book_slug != "history"
                && app.book_slug != "production"
                && app.book_slug != "research"
                && !app.book_asks_login()));
    let reserved = if can_copy { copy_w + 2 } else { 0 };
    if can_copy {
        let cx = foot.right().saturating_sub(right.chars().count() as u16 + 2 + copy_w);
        f.buffer_mut().set_string(cx, foot.y, copy_label, Style::new().fg(Color::Indexed(252)).bg(Color::Indexed(240)));
        app.book_buttons.push((
            Rect::new(cx, foot.y, copy_w, 1),
            if on_start { crate::app::BookButton::History } else { crate::app::BookButton::Copy },
        ));
    }
    let left_room = foot.width.saturating_sub(right.chars().count() as u16 + 3 + reserved) as usize;
    let left = fit_left(&left, left_room);
    f.buffer_mut().set_stringn(foot.x + 1, foot.y, &left, left_room, Style::new().fg(Color::Indexed(245)).bg(Color::Indexed(236)));
    if foot.width as usize > right.chars().count() + 8 {
        f.buffer_mut().set_string(
            foot.right().saturating_sub(right.chars().count() as u16 + 1),
            foot.y,
            &right,
            Style::new().fg(Color::Indexed(245)).bg(Color::Indexed(236)),
        );
    }
}

fn draw_wiki_infobox(f: &mut Frame, area: Rect, entry: Entry) {
    if area.width < 12 || area.height < 6 {
        return;
    }
    f.render_widget(Block::bordered().border_style(Style::new().fg(Color::Indexed(240))), area);
    let title = match entry {
        Entry::Item(i) => capitalise(i.name()),
        Entry::Part(k) => k.name().to_string(),
    };
    f.buffer_mut().set_stringn(
        area.x + 1,
        area.y,
        &title,
        area.width.saturating_sub(2) as usize,
        Style::new().fg(Color::Indexed(229)).bold(),
    );
    let (glyph, style) = match entry {
        Entry::Item(i) => item_cell(i, Color::Reset),
        Entry::Part(k) => {
            let c = kind_cells(k, Dir::E, true);
            (c[0].0, c[0].1)
        }
    };
    // The "picture": the item's own glyph, once, centred in the three rows under the title.
    f.buffer_mut().set_string(area.x + area.width / 2, area.y + 2, glyph.to_string(), style.bold());
    f.buffer_mut().set_stringn(
        area.x + 1,
        area.y + 4,
        "┄".repeat(area.width.saturating_sub(2) as usize),
        area.width.saturating_sub(2) as usize,
        Style::new().fg(Color::DarkGray),
    );
    f.buffer_mut().set_stringn(
        area.x + 1,
        area.y + 5,
        "Value",
        area.width.saturating_sub(2) as usize,
        Style::new().fg(Color::Indexed(229)).bold(),
    );
    let rows: Vec<(String, String)> = match entry {
        Entry::Item(i) => vec![("Money".into(), format!("${}", i.price())), ("RP".into(), format!("RP {:.1}x", i.curve().worth))],
        Entry::Part(k) => {
            let (w, h) = k.base_dims();
            vec![
                ("Price".into(), format!("${}", grouped(k.cost()))),
                ("Size".into(), format!("{w}×{h}")),
                ("Power".into(), format!("{:.0} EC", k.power_draw())),
                ("Unlocked by".into(), k.tech().map_or("Available".into(), |t| t.name().into())),
            ]
        }
    };
    for (i, (label, value)) in rows.iter().enumerate() {
        let y = area.y + 6 + i as u16;
        if y >= area.bottom() {
            break;
        }
        f.buffer_mut().set_string(area.x + 1, y, label, Style::new().fg(Color::Gray).bold());
        let val_x = area.x + area.width.saturating_sub(value.len() as u16).saturating_sub(1);
        f.buffer_mut().set_string(val_x, y, value, Style::new().fg(if label == "Money" { Color::Yellow } else { Color::Gray }));
    }
}

fn fit_left(s: &str, width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    chars.into_iter().rev().take(width).collect::<Vec<_>>().into_iter().rev().collect()
}

fn recipe_item(item: Item, count: u32) -> String {
    let slug = crate::wiki::slug(item.name());
    let glyph = item_cell(item, Color::Reset).0;
    format!("{}[[!{slug}|{glyph}]] [[{slug}]]", if count > 1 { format!("{count}x ") } else { String::new() })
}

fn wiki_visible_width(text: &str) -> usize {
    let mut width = 0;
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("[[") {
            if let Some(end) = after.find("]]") {
                let spec = &after[..end];
                let label = spec.split_once('|').map_or_else(|| crate::wiki::link_label(spec), |(_, label)| label.to_string());
                width += label.chars().count();
                rest = &after[end + 2..];
                continue;
            }
        }
        let c = rest.chars().next().unwrap();
        width += 1;
        rest = &rest[c.len_utf8()..];
    }
    width
}

fn recipe_equation(r: &Recipe, machine: Option<Kind>, width: usize) -> Vec<String> {
    let side = |items: &[(Item, u32)]| items.iter().map(|(i, n)| recipe_item(*i, *n)).collect::<Vec<_>>().join(" + ");
    let inputs = side(r.inputs);
    let outputs = side(r.outputs);
    let batch = machine.map_or(1, Kind::batch);
    let machine_label = machine
        .map(|_| {
            let makers: Vec<_> = CRAFTERS
                .iter()
                .copied()
                .filter(|k| k.recipes().iter().any(|candidate| same_recipe(candidate, r)))
                .map(|k| format!("[[{}|{}]]", crate::wiki::slug(k.name()), k.name()))
                .collect();
            if makers.is_empty() {
                String::new()
            } else {
                format!(" ({})", makers.join(", "))
            }
        })
        .unwrap_or_default();
    let secs = r.ticks as f64 / TPS as f64;
    let count = r.outputs.first().map_or(1, |o| o.1) * batch;
    let rate = count as f64 * 60.0 / secs;
    let arrow = format!("──── {:.1}s ────▶", secs);
    let one_line = format!("│  {inputs}  {arrow}  {outputs}{machine_label}│");
    if wiki_visible_width(&one_line) <= width.saturating_sub(2) {
        vec![
            one_line,
            format!(
                "│{} {:.0}/min{}│",
                " ".repeat(width.saturating_sub(12) / 2),
                rate,
                " ".repeat(width.saturating_sub(12) - width.saturating_sub(12) / 2)
            ),
        ]
    } else {
        let mut lines = vec![format!("│  {inputs}"), format!("│  {arrow}  {rate:.0}/min")];
        if wiki_visible_width(&format!("{outputs}{machine_label}")) <= width.saturating_sub(4) {
            lines.push(format!("│  {outputs}{machine_label}"));
        } else {
            lines.push(format!("│  {outputs}"));
        }
        if !machine_label.is_empty() && lines.len() == 3 && !lines[2].contains(&machine_label) {
            lines.push(format!("│ {machine_label}"));
        }
        lines
    }
}

fn wiki_recipe_cards(world: &World, entry: Entry, width: usize) -> String {
    let (sources, uses, makes): (Vec<(Recipe, Option<Kind>)>, Vec<(Recipe, Option<Kind>)>, Vec<(Recipe, Option<Kind>)>) = match entry {
        Entry::Item(item) => {
            let mut sources = Vec::new();
            let mut uses = Vec::new();
            for k in CRAFTERS {
                for r in k.recipes() {
                    if r.outputs.iter().any(|o| o.0 == item)
                        && !(item == Item::CopperCoil && r.inputs == [(Item::CopperWire, 3)])
                        && !sources.iter().any(|(old, _): &(Recipe, Option<Kind>)| same_recipe(old, r))
                    {
                        sources.push((*r, Some(k)));
                    }
                    if r.inputs.iter().any(|i| i.0 == item) && !uses.iter().any(|(old, _): &(Recipe, Option<Kind>)| same_recipe(old, r)) {
                        uses.push((*r, Some(k)));
                    }
                }
            }
            (sources, uses, Vec::new())
        }
        Entry::Part(k) => (Vec::new(), Vec::new(), k.recipes().iter().map(|r| (*r, Some(k))).collect()),
    };
    if !makes.is_empty() {
        let mut out = String::from("## Recipes\n\n### Makes\n");
        for (r, k) in &makes {
            out.push_str(&render_recipe_card(world, *r, *k, width));
        }
        out
    } else {
        let mut out = String::from("## Recipes\n\n### Sources\n");
        for (r, k) in &sources {
            out.push_str(&render_recipe_card(world, *r, *k, width));
        }
        if sources.is_empty() {
            out.push_str("No recipes found.\n");
        }
        out.push_str("\n### Uses\n");
        for (r, k) in &uses {
            out.push_str(&render_recipe_card(world, *r, *k, width));
        }
        out
    }
}

fn render_recipe_card(world: &World, r: Recipe, machine: Option<Kind>, width: usize) -> String {
    let inner = width.saturating_sub(2).max(8);
    let mut out = format!("╭{}╮\n", "─".repeat(inner));
    for line in recipe_equation(&r, machine, inner) {
        let body = line.trim_start_matches('│').trim_end_matches('│').to_string();
        out.push('│');
        out.push_str(&body);
        out.push_str(&" ".repeat(inner.saturating_sub(wiki_visible_width(&body))));
        out.push('│');
        out.push('\n');
    }
    let _ = world;
    out.push_str(&format!("╰{}╯\n", "─".repeat(inner)));
    out
}

fn book_page_markdown(world: &World, entry: Entry, md: &str, width: usize) -> String {
    let mut lines = md.lines();
    let title = lines.next().unwrap_or("# Page");
    let body: Vec<_> = lines.collect();
    let section_at = body.iter().position(|line| line.starts_with("## ")).unwrap_or(body.len());
    let (intro, rest) = body.split_at(section_at);
    let mut out = format!("{title}\n\n## Information\n");
    for line in intro.iter().filter(|line| !line.trim().is_empty()) {
        out.push_str(line);
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&wiki_recipe_cards(world, entry, width));
    out.push_str("\n## Unlock\n");
    match entry {
        Entry::Item(item) => {
            let made = world.econ.made[item.idx()];
            if let Some(i) = TREE.iter().position(|(n, _)| *n == Node::Key(Made::Item(item))) {
                let next: Vec<String> = tree_children(i)
                    .iter()
                    .map(|&child| {
                        let name = node_name(TREE[child].0);
                        let slug = crate::wiki::slug(&name);
                        if crate::wiki::page(&slug).is_some() {
                            format!("[[{slug}|{name}]]")
                        } else {
                            name
                        }
                    })
                    .collect();
                let done = if made > 0 { " (done)" } else { "" };
                out.push_str(&format!("Your first one brings {}{}{}.\n", next.join(", "), key_letter_text(Made::Item(item)), done));
            }
            for tech in Tech::ALL.iter().filter(|tech| tech.milestone().0 == Made::Item(item) && !world.progress.has(**tech)) {
                let n = tech.milestone().1;
                out.push_str(&format!("Make {n} to unlock {} free ({}/{n}).\n", tech.name(), made.min(n)));
            }
        }
        Entry::Part(k) => {
            out.push_str(if world.progress.part_unlocked(k) { "Ready to build.\n" } else { "Locked by research or company progress.\n" });
        }
    }
    for line in rest {
        if !line.trim().is_empty() {
            out.push_str(line);
        }
        out.push('\n');
    }
    let (nav_title, category) = match entry {
        Entry::Item(_) => ("Items", "Items"),
        Entry::Part(_) => ("Machines", "Machines"),
    };
    let width = width.max(24);
    let links = "[[~items|Items]]  ·  [[~machines|Machines]]  ·  [[~topics|Topics]]";
    let title = format!(" {nav_title} ");
    let sides = width.saturating_sub(title.chars().count() + 2);
    out.push_str(&format!(
        "\n┌{}{}{}┐\n│ {}{} │\n└{}┘\nCategory: {category}\n",
        "─".repeat(sides / 2),
        title,
        "─".repeat(sides - sides / 2),
        links,
        // Pad by the shown text, not the markup, so the right border meets the box's edge.
        " ".repeat(width.saturating_sub(4 + "Items  ·  Machines  ·  Topics".chars().count())),
        "─".repeat(width.saturating_sub(2))
    ));
    out
}

/// The address a page shows in the address field and the status bar.
pub(crate) fn book_url(slug: &str) -> String {
    if slug == "start" {
        "termweb.search".into()
    } else if slug == "history" {
        "termweb.search/history".into()
    } else if slug == "production" {
        "production.lan".into()
    } else if slug == "research" {
        "research.lan".into()
    } else if slug.starts_with("search?q=") {
        format!("termweb.search/{slug}")
    } else if crate::web::is_addr(slug) {
        slug.to_string()
    } else {
        format!("terminactory.wiki/{slug}")
    }
}

/// A page from one of T Web's other sites: its markdown as a plain page, or T Web's own 404
/// (or "server not found") when there is nothing there.
fn draw_web_page(f: &mut Frame, area: Rect, app: &mut App) {
    let addr = app.book_slug.clone();
    // A gated page shows its form instead of its content until its domain is signed in.
    if app.book_asks_login() {
        draw_login(f, area, app);
        return;
    }
    let domain = crate::web::domain_of(&addr).to_string();
    if let Some(allowed) = crate::web::access(&addr) {
        if let Some(user) = app.book_signed_in(&domain).map(str::to_string) {
            if !allowed.iter().any(|a| a == "all" || a.eq_ignore_ascii_case(&user)) {
                let md = format!("# 403\n\n{user} may not read this page.\n");
                render_wiki_text(f, area, &md, app, None, None, PageKind::Web);
                return;
            }
        }
    }
    if let Some(thread) = crate::web::thread(&addr) {
        draw_thread(f, area, app, thread);
        return;
    }
    let md = match crate::web::page(&addr) {
        Some(md) => {
            let mut text = crate::web::prepare(&addr, md);
            // A profile page adds what the archive knows about its author.
            if let Some(section) = crate::web::profile_section(&addr) {
                text.push_str(&section);
            }
            text
        }
        None => crate::web::missing(&addr),
    };
    render_wiki_text(f, area, &md, app, None, None, PageKind::Web);
}

/// A gated page's sign-in form: the domain, the two boxes with what has been typed in them,
/// and the keys that work. A failed try says only that it failed.
fn draw_login(f: &mut Frame, area: Rect, app: &mut App) {
    let domain = crate::web::domain_of(&app.book_slug).to_string();
    let form = app.book_login.clone().unwrap_or_default();
    let typing = app.book_login_typing;
    let head = Style::new().fg(Color::Indexed(229)).bold();
    let dim = Style::new().fg(Color::DarkGray);
    let mut rows: Vec<(String, Style)> = vec![
        (domain, head),
        (String::new(), Style::default()),
        ("This page needs a sign-in.".to_string(), Style::new().fg(Color::Indexed(252))),
        (String::new(), Style::default()),
    ];
    // A box is 22 cells wide, with the caret in it while it has the keys.
    let boxed = |label: &str, value: &str, active: bool| {
        let mut inside: String = value.chars().take(20).collect();
        if active {
            inside.push('▏');
        }
        let pad = 22usize.saturating_sub(inside.chars().count());
        let style =
            if active { Style::new().fg(Color::Indexed(255)).bg(Color::Indexed(238)) } else { Style::new().fg(Color::Indexed(245)) };
        (format!("{label} [{inside}{}]", " ".repeat(pad)), style)
    };
    rows.push(boxed("user:", &form.user, typing && !form.on_pass));
    rows.push(boxed("pass:", &form.pass, typing && form.on_pass));
    rows.push((String::new(), Style::default()));
    rows.push(("Enter signs in · Tab switches box · Esc leaves the boxes".to_string(), dim));
    if form.failed {
        rows.push(("Sign-in failed.".to_string(), Style::new().fg(Color::Indexed(167))));
    }
    app.book_max_scroll = 0;
    app.book_scroll_step = 1;
    for (i, (text, style)) in rows.into_iter().enumerate() {
        let y = area.y.saturating_add(i as u16);
        if y >= area.bottom() {
            break;
        }
        let shown: String = text.chars().take(area.width as usize).collect();
        f.buffer_mut().set_stringn(area.x, y, &shown, area.width as usize, style);
    }
}

type PageChar = (char, Option<String>, bool, Style);

/// One post's nesting: the columns of its parents, as dim bars, then its own text.
fn thread_indent(shift: usize, depth: usize, step: usize) -> Vec<PageChar> {
    let mut out = vec![(' ', None, false, Style::default()); shift];
    for level in 0..depth {
        let x = level * step;
        if x >= shift {
            break;
        }
        out[x] = ('│', None, false, Style::new().fg(Color::DarkGray));
    }
    out
}

/// A thread: its title, one line from its header, then every post with its body, nested by
/// its depth. Folded posts show their first line and how many replies are hidden.
fn draw_thread(f: &mut Frame, area: Rect, app: &mut App, thread: &'static crate::web::Thread) {
    let width = area.width.max(1) as usize;
    let step = if width < 60 { 1 } else { 2 };
    let cap = (width / 3).max(step);
    let domain = crate::web::domain_of(&app.book_slug).to_string();
    let folded = app.book_folded.clone();
    let title_style = Style::new().fg(Color::Indexed(229)).bold();
    let dim = Style::new().fg(Color::DarkGray);
    let mut rows: Vec<Vec<PageChar>> = Vec::new();
    let mut headers: Vec<(usize, usize)> = Vec::new();
    let mut folds: Vec<(usize, usize)> = Vec::new();

    rows.push(thread.title.chars().map(|c| (c, None, false, title_style)).collect());
    // The header line: the thread's own board and status, and how many replies it has.
    let field = |key: &str| thread.meta.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    let mut line: Vec<String> = Vec::new();
    if let Some(board) = field("board") {
        line.push(board);
    }
    if let Some(status) = field("status") {
        line.push(status);
    }
    line.push(format!("{} replies", thread.posts.len().saturating_sub(1)));
    rows.push(line.join(" · ").chars().map(|c| (c, None, false, dim)).collect());
    rows.push(Vec::new());

    let mut hidden_below: Option<usize> = None;
    let mut tops = 0;
    for (i, post) in thread.posts.iter().enumerate() {
        // A folded post hides everything nested under it.
        if let Some(depth) = hidden_below {
            if post.depth > depth {
                continue;
            }
            hidden_below = None;
        }
        if post.depth == 0 {
            if tops > 0 {
                rows.push(Vec::new());
            }
            tops += 1;
        }
        let shift = (post.depth * step).min(cap);
        let mut header = thread_indent(shift, post.depth, step);
        if post.depth * step > cap {
            // Deeper than the cap: say how deep, rather than shifting further.
            header.extend(format!("↳ {} ", post.depth).chars().map(|c| (c, None, false, dim)));
        }
        let profile = (!post.author.starts_with('[')).then(|| crate::web::profile_addr(&domain, &post.author)).flatten();
        header.extend(post.author.chars().map(|c| (c, profile.clone(), false, Style::default())));
        header.extend(format!(" · {}", post.date).chars().map(|c| (c, None, false, dim)));
        headers.push((rows.len(), i));
        rows.push(header);

        let body_width = width.saturating_sub(shift).max(1);
        let mut body: Vec<Vec<(char, Option<String>, bool)>> = Vec::new();
        for text in &post.body {
            if text.is_empty() {
                body.push(Vec::new());
            } else {
                body.extend(wrap_wiki_chars(inline_chars(text), body_width));
            }
        }
        if folded.contains(&i) {
            let replies = thread.posts[i + 1..].iter().take_while(|p| p.depth > post.depth).count();
            if let Some(first) = body.first() {
                let mut row = thread_indent(shift, post.depth, step);
                row.extend(first.iter().map(|(c, link, bold)| (*c, link.clone(), *bold, Style::default())));
                rows.push(row);
            }
            let mut marker = thread_indent(shift, post.depth, step);
            marker.extend(format!("[+] {replies} replies hidden").chars().map(|c| (c, None, false, dim)));
            folds.push((rows.len(), i));
            rows.push(marker);
            hidden_below = Some(post.depth);
            continue;
        }
        for line in body {
            let mut row = thread_indent(shift, post.depth, step);
            row.extend(line.into_iter().map(|(c, link, bold)| (c, link, bold, Style::default())));
            rows.push(row);
        }
    }

    app.book_max_scroll = rows.len().saturating_sub(area.height as usize) as u16;
    app.book_scroll = app.book_scroll.min(app.book_max_scroll);
    app.book_post_hits.clear();
    app.book_fold_hits.clear();
    let start = app.book_scroll as usize;
    let mut link_index = 0usize;
    for (row_index, row) in rows.iter().enumerate().skip(start) {
        let y = area.y + (row_index - start) as u16;
        if y >= area.bottom() {
            break;
        }
        draw_wiki_row(f, area, y, row, false, app, &mut link_index);
        let hit = Rect::new(area.x, y, area.width, 1);
        if let Some((_, post)) = headers.iter().find(|(row, _)| *row == row_index) {
            app.book_post_hits.push((hit, *post));
        }
        if let Some((_, post)) = folds.iter().find(|(row, _)| *row == row_index) {
            app.book_fold_hits.push((hit, *post));
        }
    }
}

/// The letters of the termweb logo, lower case: seven pixel rows each, "term" in grey and "web" in white.
const LOGO: [[&str; 7]; 7] = [
    [" █  ", " █  ", "████", " █  ", " █  ", " █  ", " ███"],
    ["    ", "    ", "████", "█  █", "████", "█   ", "████"],
    ["    ", "    ", "█ ██", "██  ", "█   ", "█   ", "█   "],
    ["     ", "     ", "████ ", "█ █ █", "█ █ █", "█ █ █", "█ █ █"],
    ["     ", "     ", "█   █", "█   █", "█ █ █", "█ █ █", " █ █ "],
    ["    ", "    ", "████", "█  █", "████", "█   ", "████"],
    ["█   ", "█   ", "███ ", "█  █", "█  █", "█  █", "███ "],
];
/// The logo's width in pixels: the seven letters, with a pixel between each.
const LOGO_WIDTH: u16 = 4 + 4 + 4 + 5 + 5 + 4 + 4 + 6;
const LOGO_GREY: u8 = 245;
const LOGO_WHITE: u8 = 255;

/// The name in one line, for small panes and the results page: "term" in grey, "web" in white.
fn draw_wordmark(f: &mut Frame, x: u16, y: u16) {
    f.buffer_mut().set_string(x, y, "term", Style::new().fg(Color::Indexed(LOGO_GREY)).bold());
    f.buffer_mut().set_string(x + 4, y, "web", Style::new().fg(Color::Indexed(LOGO_WHITE)).bold());
}

/// A rounded box of one line of text, as the search fields are: returns the text's room.
fn search_box(f: &mut Frame, r: Rect, text: &str, placeholder: &str, focused: bool) {
    let border = Style::new().fg(Color::Indexed(if focused { 255 } else { 243 }));
    let inner_w = r.width.saturating_sub(2) as usize;
    f.buffer_mut().set_string(r.x, r.y, format!("╭{}╮", "─".repeat(inner_w)), border);
    f.buffer_mut().set_string(r.x, r.y + 1, "│", border);
    f.buffer_mut().set_string(r.right().saturating_sub(1), r.y + 1, "│", border);
    f.buffer_mut().set_string(r.x, r.y + 2, format!("╰{}╯", "─".repeat(inner_w)), border);
    let room = inner_w.saturating_sub(3);
    let (shown, style) = if text.is_empty() && !focused {
        (placeholder.to_string(), Style::new().fg(Color::Indexed(243)))
    } else {
        (format!("{text}{}", if focused { "▏" } else { "" }), Style::new().fg(Color::Indexed(255)))
    };
    f.buffer_mut().set_stringn(r.x + 2, r.y + 1, fit_left(&shown, room), room, style);
}

/// T Web's start page: the logo, one big search box and a few links, as a search engine's front page.
fn draw_book_start(f: &mut Frame, area: Rect, app: &mut App) {
    // Terminal cells are twice as tall as they are wide, so a logo pixel is two columns wide in
    // the large size and half a row tall in the compact one; either way the pixels are square.
    let large = area.height >= 20 && area.width >= 2 * LOGO_WIDTH + 4;
    let compact = !large && area.height >= 15 && area.width >= LOGO_WIDTH + 4;
    let logo_h: u16 = if large {
        7
    } else if compact {
        4
    } else {
        1
    };
    let total = logo_h + 2 + 3 + 1 + 1 + 2 + 1;
    let mut y = area.y + area.height.saturating_sub(total) / 3;
    if large || compact {
        let scale = if large { 2 } else { 1 };
        let mut x = area.x + area.width.saturating_sub(LOGO_WIDTH * scale) / 2;
        for (i, glyph) in LOGO.iter().enumerate() {
            let style = Style::new().fg(Color::Indexed(if i < 4 { LOGO_GREY } else { LOGO_WHITE }));
            let width = glyph[0].chars().count();
            let on = |row: usize, col: usize| glyph.get(row).and_then(|l| l.chars().nth(col)) == Some('█');
            if large {
                for (row, line) in glyph.iter().enumerate() {
                    let doubled: String = line.chars().flat_map(|c| [c, c]).collect();
                    f.buffer_mut().set_string(x, y + row as u16, doubled, style);
                }
            } else {
                for text_row in 0..4usize {
                    let line: String = (0..width)
                        .map(|col| match (on(text_row * 2, col), on(text_row * 2 + 1, col)) {
                            (true, true) => '█',
                            (true, false) => '▀',
                            (false, true) => '▄',
                            (false, false) => ' ',
                        })
                        .collect();
                    f.buffer_mut().set_string(x, y + text_row as u16, line, style);
                }
            }
            x += (width as u16 + 1) * scale;
        }
    } else {
        draw_wordmark(f, area.x + area.width.saturating_sub(7) / 2, y);
    }
    y += logo_h + 2;
    let box_w = area.width.saturating_sub(4).clamp(10, 56);
    let boxr = Rect::new(area.x + area.width.saturating_sub(box_w) / 2, y, box_w, 3);
    let query = app.book_query.clone();
    search_box(f, boxr, &query, "Search termweb", app.book_typing);
    app.book_buttons.push((boxr, crate::app::BookButton::Edit));
    y += 4;
    let labels = [(" termweb Search ", crate::app::BookButton::Search), (" I'm Feeling Lucky ", crate::app::BookButton::Lucky)];
    let total_w: u16 = labels.iter().map(|(l, _)| l.chars().count() as u16).sum::<u16>() + 2;
    let mut x = area.x + area.width.saturating_sub(total_w) / 2;
    for (label, button) in labels {
        let w = label.chars().count() as u16;
        if x + w > area.right() {
            break;
        }
        f.buffer_mut().set_string(x, y, label, Style::new().fg(Color::Indexed(252)).bg(Color::Indexed(237)));
        app.book_buttons.push((Rect::new(x, y, w, 1), button));
        x += w + 2;
    }
    y += 3;
    let links = [
        ("Terminactory Wiki", "home"),
        ("Getting started", "getting-started"),
        ("Machines", "machines"),
        ("Items", "items"),
        ("Topics", "topics"),
        ("Production", "production"),
        ("Research", "research"),
        ("Directory", "termweb.search/directory"),
    ];
    let widths: Vec<u16> = links.iter().map(|(l, _)| l.chars().count() as u16).collect();
    let mut shown = links.len();
    while shown > 1 && widths[..shown].iter().sum::<u16>() + 3 * (shown as u16 - 1) > area.width {
        shown -= 1;
    }
    let row_w = widths[..shown].iter().sum::<u16>() + 3 * (shown as u16 - 1);
    let mut x = area.x + area.width.saturating_sub(row_w) / 2;
    for (i, (label, slug)) in links[..shown].iter().enumerate() {
        let selected = !app.book_typing && i == app.book_selected;
        let style = if selected { Style::new().add_modifier(Modifier::REVERSED) } else { Style::new().fg(Color::Indexed(75)).underlined() };
        f.buffer_mut().set_string(x, y, label, style);
        app.book_urls.push((Rect::new(x, y, widths[i], 1), slug.to_string()));
        x += widths[i];
        if i + 1 < shown {
            f.buffer_mut().set_string(x + 1, y, "·", Style::new().fg(Color::Indexed(240)));
            x += 3;
        }
    }
    if !app.book_urls.is_empty() {
        app.book_selected = app.book_selected.min(app.book_urls.len() - 1);
    }
    app.book_max_scroll = 0;
}

/// The search history page: every search made, newest first, each one a link, and a button to
/// clear them.
fn draw_book_history(f: &mut Frame, area: Rect, app: &mut App) {
    let history = app.settings.search_history.clone();
    f.buffer_mut().set_string(area.x + 1, area.y, "Search history", Style::new().fg(Color::Indexed(229)).bold());
    let mut y = area.y + 2;
    if history.is_empty() {
        f.buffer_mut().set_string(area.x + 1, y, "No searches yet.", Style::new().fg(Color::Indexed(245)));
    }
    for (i, q) in history.iter().enumerate() {
        if y >= area.bottom().saturating_sub(2) {
            break;
        }
        let selected = !app.book_typing && i == app.book_selected;
        let style = if selected { Style::new().add_modifier(Modifier::REVERSED) } else { Style::new().fg(Color::Indexed(75)).underlined() };
        let label = fit_left(q, area.width.saturating_sub(5) as usize);
        f.buffer_mut().set_string(area.x + 1, y, "↺ ", Style::new().fg(Color::Indexed(245)));
        f.buffer_mut().set_string(area.x + 3, y, &label, style);
        app.book_urls.push((Rect::new(area.x + 3, y, label.chars().count() as u16, 1), format!("search?q={q}")));
        y += 1;
    }
    if !history.is_empty() && area.bottom() > y + 1 {
        let label = " Clear history ";
        let r = Rect::new(area.x + 1, y + 1, label.chars().count() as u16, 1);
        f.buffer_mut().set_string(r.x, r.y, label, Style::new().fg(Color::Indexed(252)).bg(Color::Indexed(237)));
        app.book_buttons.push((r, crate::app::BookButton::ClearHistory));
    }
    if !app.book_urls.is_empty() {
        app.book_selected = app.book_selected.min(app.book_urls.len() - 1);
    }
    app.book_max_scroll = 0;
}

/// A page of search results: the search box again at the top, then each result as a green
/// address, a blue title and a grey line of the page.
fn draw_book_results(f: &mut Frame, area: Rect, app: &mut App) {
    let query = app.book_slug.strip_prefix("search?q=").unwrap_or("").to_string();
    if app.book_hits.as_ref().is_none_or(|(q, _)| *q != query) {
        app.book_hits = Some((query.clone(), crate::search::search(&query)));
    }
    let hits = app.book_hits.as_ref().map(|(_, h)| h.clone()).unwrap_or_default();
    let width = area.width.min(96);
    // The header: the small logo and the search box with the words searched.
    let logo_w = 9u16;
    if area.width >= 30 {
        draw_wordmark(f, area.x + 1, area.y + 1);
    }
    let shown = if app.book_typing { app.book_query.clone() } else { query.clone() };
    let hx = area.x + if area.width >= 30 { logo_w } else { 0 };
    let boxr = Rect::new(hx, area.y, width.saturating_sub(hx - area.x).min(60), 3);
    search_box(f, boxr, &shown, "Search termweb", app.book_typing);
    app.book_buttons.push((boxr, crate::app::BookButton::Edit));
    let rule_y = area.y + 3;
    f.buffer_mut().set_string(area.x, rule_y, "─".repeat(area.width as usize), Style::new().fg(Color::Indexed(238)));
    // The results as rows of styled text; a title row also remembers which result it is.
    let text_w = width.saturating_sub(2).max(10) as usize;
    let mut rows: Vec<(String, Style, Option<String>)> = Vec::new();
    let grey = Style::new().fg(Color::Indexed(247));
    rows.push((
        if hits.len() == 1 { "About 1 result".to_string() } else { format!("About {} results", hits.len()) },
        Style::new().fg(Color::Indexed(243)),
        None,
    ));
    rows.push((String::new(), Style::new(), None));
    if hits.is_empty() {
        rows.push((format!("Your search - {query} - did not match any pages."), Style::new().fg(Color::Indexed(252)), None));
        rows.push((String::new(), Style::new(), None));
        rows.push(("Suggestions:".to_string(), grey, None));
        for tip in ["Make sure all words are spelled correctly.", "Try different keywords.", "Try more general keywords."] {
            rows.push((format!("  • {tip}"), grey, None));
        }
    }
    for hit in &hits {
        // A page from another site shows its address, and a thread's title has no wiki suffix.
        let web = crate::web::is_addr(&hit.slug);
        let address =
            if web { hit.slug.split('/').collect::<Vec<_>>().join(" › ") } else { format!("terminactory.wiki › {}", hit.slug) };
        rows.push((address, Style::new().fg(Color::Indexed(71)), None));
        let title = if web { hit.title.clone() } else { format!("{} - Terminactory Wiki", hit.title) };
        rows.push((title, Style::new().fg(Color::Indexed(75)).underlined(), Some(hit.slug.clone())));
        for line in wrap_words(&hit.snippet, text_w).into_iter().take(2) {
            rows.push((line, grey, None));
        }
        rows.push((String::new(), Style::new(), None));
    }
    let view = area.height.saturating_sub(4) as usize;
    app.book_max_scroll = rows.len().saturating_sub(view) as u16;
    app.book_scroll = app.book_scroll.min(app.book_max_scroll);
    app.book_scroll_step = 1;
    // Which result is picked: the side arrows set it directly and the page scrolls to show it;
    // otherwise it follows the link the up and down arrows are on.
    let title_rows: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| r.2.is_some()).map(|(i, _)| i).collect();
    if !title_rows.is_empty() {
        app.book_pick = app.book_pick.min(title_rows.len() - 1);
        if app.book_pick_follow {
            let row = title_rows[app.book_pick];
            let scroll = app.book_scroll as usize;
            if row < scroll + 1 {
                app.book_scroll = row.saturating_sub(1).min(app.book_max_scroll as usize) as u16;
            } else if row + 3 > scroll + view {
                app.book_scroll = (row + 3).saturating_sub(view).min(app.book_max_scroll as usize) as u16;
            }
            app.book_pick_follow = false;
        }
        let scroll = app.book_scroll as usize;
        let first = title_rows.iter().position(|r| *r >= scroll).unwrap_or(title_rows.len() - 1);
        let shown = title_rows.iter().filter(|r| **r >= scroll && **r < scroll + view).count();
        if app.book_selected != app.book_sel_seen {
            // The up and down arrows moved the selection: the pick goes with it.
            app.book_selected = app.book_selected.min(shown.saturating_sub(1));
            app.book_pick = first + app.book_selected;
        } else if app.book_pick >= first && app.book_pick < first + shown.max(1) {
            app.book_selected = app.book_pick - first;
        } else {
            app.book_selected = app.book_selected.min(shown.saturating_sub(1));
            app.book_pick = first + app.book_selected;
        }
        app.book_sel_seen = app.book_selected;
        app.book_first_hit = first;
    }
    let mut y = area.y + 4;
    for (text, style, link) in rows.iter().skip(app.book_scroll as usize).take(view) {
        let shown = fit_left(text, text_w);
        if let Some(slug) = link {
            let selected = app.book_urls.len() == app.book_selected && !app.book_typing;
            let style = if selected { Style::new().add_modifier(Modifier::REVERSED) } else { *style };
            f.buffer_mut().set_string(area.x + 1, y, &shown, style);
            app.book_urls.push((Rect::new(area.x + 1, y, shown.chars().count() as u16, 1), slug.clone()));
        } else {
            f.buffer_mut().set_string(area.x + 1, y, &shown, *style);
        }
        y += 1;
    }
    if !app.book_urls.is_empty() {
        app.book_selected = app.book_selected.min(app.book_urls.len() - 1);
    }
}

fn book_home_markdown(_w: &World, seed: usize) -> String {
    let items = Item::ALL.len();
    let machines = Kind::ALL.iter().filter(|k| crate::wiki::page(&crate::wiki::slug(k.name())).is_some()).count();
    let recipes = CRAFTERS.iter().map(|k| k.recipes().len()).sum::<usize>();
    let facts = [
        "…that [[copper-coil]] can be made in a [[press]] or a [[winder]]?".to_string(),
        "…that [[stone]] goes into [[concrete]], [[sand]] and [[brick]]?".to_string(),
        "…that the most valuable item is the [[computer]]?".to_string(),
        "…that a [[boiler]] burns only as much [[coal]] as its grid needs?".to_string(),
    ];
    let mut out = format!(
        "# Terminactory Wiki\n\n{}\n\n[[getting-started|New here? Start with Getting started]]\n\n[[machines|Machines]] · [[items|Items]] · [[topics|Topics]] · [[companies|Companies]]\n\n**{} pages · {} items · {} machines · {} recipes**\n\n## Did you know?\n",
        crate::wiki::page("home").unwrap_or("").lines().find(|l| l.starts_with("**Terminactory Wiki**")).unwrap_or("A factory building game about making things, one recipe at a time."),
        crate::wiki::PAGES.len() + 3,
        items,
        machines,
        recipes
    );
    out.push('\n');
    for i in 0..4 {
        if i > 0 {
            out.push('\n');
        }
        out.push_str("- ");
        out.push_str(&facts[(seed + i * 5) % facts.len()]);
    }
    out.push_str("\n\n[[close-book|Back to the factory]]\n");
    out
}

fn draw_book_index(f: &mut Frame, area: Rect, app: &mut App, machines: bool) {
    let query = if app.book_typing { &app.book_query } else { "" };
    let entries = Entry::list(query, machines);
    let hint =
        format!("Search {}... {}", if machines { "machines" } else { "items" }, if query.is_empty() { "(type to filter)" } else { query });
    f.buffer_mut().set_string(area.x, area.y, &hint, Style::new().fg(Color::Indexed(117)).bg(Color::Indexed(236)));
    let cols = (area.width / 18).max(1) as usize;
    let card_w = (area.width as usize / cols).max(1);
    // Scroll by whole rows of tiles, and never past the last row.
    let fits = (area.height as usize).saturating_sub(5) / 4 + 1;
    let rows = entries.len().div_ceil(cols);
    app.book_max_scroll = (rows.saturating_sub(fits) * cols) as u16;
    app.book_scroll_step = cols as u16;
    app.book_scroll = app.book_scroll.min(app.book_max_scroll) / cols as u16 * cols as u16;
    let mut idx = 0usize;
    for e in entries.iter().skip(app.book_scroll as usize) {
        let row = idx / cols;
        let col = idx % cols;
        let y = area.y + 2 + (row * 4) as u16;
        if y + 3 >= area.bottom() {
            break;
        }
        let x = area.x + (col * card_w) as u16;
        let width = card_w.min(area.right().saturating_sub(x) as usize).max(1) as u16;
        let title = match e {
            Entry::Item(i) => capitalise(i.name()),
            Entry::Part(k) => k.name().to_string(),
        };
        let style = if idx == app.book_selected {
            Style::new().fg(Color::White).bg(Color::Indexed(24))
        } else {
            Style::new().fg(Color::Indexed(238))
        };
        f.render_widget(Block::bordered().style(style), Rect::new(x, y, width, 4));
        let (glyph, gs) = match e {
            Entry::Item(i) => item_cell(*i, Color::Reset),
            Entry::Part(k) => (kind_cells(*k, Dir::E, true)[0].0, kind_cells(*k, Dir::E, true)[0].1),
        };
        let gx = x + width / 2;
        f.buffer_mut().set_string(gx, y + 1, glyph.to_string(), gs);
        let max = width.saturating_sub(2) as usize;
        let label: String = title.chars().take(max).collect();
        let lx = x + width.saturating_sub(label.chars().count() as u16) / 2;
        f.buffer_mut().set_string(lx, y + 2, &label, Style::new().fg(Color::Indexed(117)).underlined());
        let slug = crate::wiki::slug(e.name());
        app.book_urls.push((Rect::new(x, y, width, 4), slug.clone()));
        app.book_links.push((Rect::new(x, y, width, 4), *e));
        idx += 1;
    }
}

/// How a page is drawn: the wiki in a wide pane, the wiki in a narrow one (the infobox sits
/// over the text), or one of T Web's other sites (no infobox, and no "From" line).
#[derive(Clone, Copy, PartialEq)]
enum PageKind {
    Wiki,
    WikiNarrow,
    Web,
}

fn render_wiki_text(f: &mut Frame, area: Rect, md: &str, app: &mut App, info_area: Option<Rect>, entry: Option<Entry>, kind: PageKind) {
    let (narrow, wiki) = (kind == PageKind::WikiNarrow, kind != PageKind::Web);
    type WikiChar = (char, Option<String>, bool, Style);
    let width = area.width as usize;
    let center_rows = md.contains("Welcome to the");
    let mut rows: Vec<(Vec<WikiChar>, bool, bool, bool)> = Vec::new();
    let mut in_recipes = false;
    let lines: Vec<&str> = md.lines().collect();
    let mut li = 0;
    while li < lines.len() {
        let raw = lines[li];
        li += 1;
        if raw.starts_with("## Recipes") {
            in_recipes = true;
        }
        let intro_column = info_area.is_some() && !in_recipes;
        let line_width = if intro_column { info_area.unwrap().x.saturating_sub(area.x) as usize } else { width };
        if raw.trim().is_empty() {
            rows.push((Vec::new(), false, false, intro_column));
            continue;
        }
        // A markdown table is drawn cell by cell, with its columns padded to a common width.
        if raw.trim_start().starts_with('|') {
            let mut block = vec![raw];
            while li < lines.len() && lines[li].trim_start().starts_with('|') {
                block.push(lines[li]);
                li += 1;
            }
            for line in table_rows(&block, line_width.max(1)) {
                rows.push((
                    line.into_iter().map(|(c, link, bold)| (c, link, bold, Style::default())).collect(),
                    false,
                    false,
                    intro_column,
                ));
            }
            continue;
        }
        let title = raw.starts_with("# ");
        let heading = raw.starts_with("## ");
        let source = raw.trim_start_matches('#').trim().strip_prefix("- ").unwrap_or(raw.trim_start_matches('#').trim());
        let mut chars = inline_chars(source);
        if raw.starts_with("- ") {
            chars.insert(0, ('•', None, false));
            chars.insert(1, (' ', None, false));
        }
        let wrapped = wrap_wiki_chars(chars, line_width.max(1));
        for (line_no, line) in wrapped.into_iter().enumerate() {
            let dim = raw.starts_with("Category: ");
            rows.push((
                line.into_iter()
                    .map(|(c, link, bold)| (c, link, bold, if dim { Style::new().fg(Color::DarkGray) } else { Style::default() }))
                    .collect(),
                title,
                heading,
                intro_column,
            ));
            if heading {
                let rule_width = if intro_column { info_area.unwrap().x.saturating_sub(area.x) as usize } else { width };
                rows.push((
                    "─".repeat(rule_width).chars().map(|c| (c, None, false, Style::new().fg(Color::DarkGray))).collect(),
                    false,
                    false,
                    intro_column,
                ));
            }
            if wiki && title && line_no == 0 {
                rows.push((
                    "From Terminactory Wiki".chars().map(|c| (c, None, false, Style::new().fg(Color::DarkGray))).collect(),
                    false,
                    false,
                    intro_column,
                ));
            }
        }
    }

    if narrow && entry.is_some() {
        let info_top = 2.min(rows.len());
        rows.splice(info_top..info_top, std::iter::repeat_n((Vec::new(), false, false, false), 12));
    }
    let info_total = entry.map_or(0, |_| if narrow { 14 } else { 11 });
    let total_rows = rows.len().max(info_total);
    app.book_max_scroll = total_rows.saturating_sub(area.height as usize) as u16;
    app.book_scroll = app.book_scroll.min(app.book_max_scroll);
    let mut start = app.book_scroll as usize;
    let mut card_ranges = Vec::new();
    let mut open_card = None;
    for (i, (line, _, _, _)) in rows.iter().enumerate() {
        if line.first().is_some_and(|c| c.0 == '╭') {
            open_card = Some(i);
        } else if line.first().is_some_and(|c| c.0 == '╰') {
            if let Some(first) = open_card.take() {
                card_ranges.push((first, i));
            }
        }
    }
    for (first, last) in &card_ranges {
        if start > *first && start <= *last {
            start = if last.saturating_add(area.height as usize) >= rows.len() { last + 1 } else { *first };
            app.book_scroll = start as u16;
            break;
        }
    }
    let mut y = area.y;
    let mut link_index = 0usize;
    for (row_index, (line, title, heading, intro_column)) in rows.iter().enumerate().skip(start) {
        if y >= area.bottom() {
            break;
        }
        if card_ranges
            .iter()
            .any(|(first, last)| *first == row_index && area.y as usize + row_index - start + last - first >= area.bottom() as usize)
        {
            break;
        }
        let row_area = if *intro_column {
            Rect { width: info_area.map_or(area.width, |r| r.x.saturating_sub(area.x)), ..area }
        } else if center_rows {
            let offset = area.width.saturating_sub(line.len().min(area.width as usize) as u16) / 2;
            Rect { x: area.x + offset, width: area.width.saturating_sub(offset), ..area }
        } else {
            area
        };
        draw_wiki_row(f, row_area, y, line, *title || *heading, app, &mut link_index);
        y += 1;
    }
    if let Some(entry) = entry {
        let doc_y = area.y as i32 + if narrow { 2 } else { 0 } - start as i32;
        let y = doc_y.max(area.y as i32) as u16;
        let clipped = doc_y < area.y as i32;
        if !clipped && y < area.bottom() {
            let box_area = if narrow {
                Rect::new(area.x, y, area.width, 11.min(area.bottom() - y))
            } else {
                let info_area = info_area.unwrap_or(area);
                Rect::new(info_area.x, y, info_area.width, 11.min(area.bottom() - y))
            };
            draw_wiki_infobox(f, box_area, entry);
        }
    }
}

fn draw_wiki_contents(f: &mut Frame, area: Rect, md: &str, app: &mut App) {
    f.buffer_mut().set_string(area.x, area.y, "Contents", Style::new().fg(Color::Indexed(229)).bold());
    let mut y = area.y + 1;
    for line in md.lines().filter(|l| l.starts_with("## ") || l.starts_with("### ")) {
        if y >= area.bottom() {
            break;
        }
        let child = line.starts_with("### ");
        let label = line.trim_start_matches('#').trim();
        let slug = format!("#{}", label.to_lowercase().replace(' ', "-"));
        f.buffer_mut().set_stringn(
            area.x + u16::from(child),
            y,
            label,
            area.width.saturating_sub(u16::from(child)) as usize,
            Style::new().fg(Color::Cyan).underlined(),
        );
        app.book_click_only.push((Rect::new(area.x, y, area.width, 1), slug));
        y += 1;
    }
}

/// The characters of one line of page text: `**bold**` and `[[target|label]]` links marked, and
/// nothing else special. A link with no label shows the title of a web page, or a wiki page's
/// name in lower case.
fn inline_chars(source: &str) -> Vec<(char, Option<String>, bool)> {
    let mut chars = Vec::new();
    let mut bold = false;
    let mut rest = source;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("**") {
            bold = !bold;
            rest = after;
            continue;
        }
        if let Some(after) = rest.strip_prefix("[[") {
            if let Some(end) = after.find("]]") {
                let spec = &after[..end];
                let (target, label) = match spec.split_once('|') {
                    Some((target, label)) => (target, label.to_string()),
                    None if crate::web::is_addr(spec) => (spec, crate::web::title(spec)),
                    None => (spec, crate::wiki::link_label(spec)),
                };
                chars.extend(label.chars().map(|c| (c, Some(target.to_string()), bold)));
                rest = &after[end + 2..];
                continue;
            }
        }
        let c = rest.chars().next().unwrap();
        chars.push((c, None, bold));
        rest = &rest[c.len_utf8()..];
    }
    chars
}

/// A markdown table as page rows: the `|---|` line is dropped, each column is padded to its
/// widest cell, and the widest columns are cut down until a row fits the width.
fn table_rows(block: &[&str], width: usize) -> Vec<Vec<(char, Option<String>, bool)>> {
    let cells: Vec<Vec<&str>> = block.iter().map(|line| row_cells(line)).collect();
    let rows: Vec<Vec<&str>> =
        cells.into_iter().filter(|row| !row.iter().all(|cell| !cell.is_empty() && cell.chars().all(|c| c == '-' || c == ':'))).collect();
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if rows.is_empty() || cols == 0 {
        return Vec::new();
    }
    let mut widths = vec![0usize; cols];
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let gap = " │ ";
    let mut total = widths.iter().sum::<usize>() + gap.chars().count() * cols.saturating_sub(1);
    // Columns shrink from the widest down, never below a few letters; long cells wrap.
    let floor = |w: usize| w.min(8);
    while total > width {
        let Some((i, _)) = widths.iter().enumerate().filter(|(_, w)| **w > floor(**w)).max_by_key(|(_, w)| **w) else { break };
        widths[i] -= 1;
        total -= 1;
    }
    let mut out_rows = Vec::new();
    for row in &rows {
        let cells: Vec<Vec<Vec<(char, Option<String>, bool)>>> = widths
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let wrapped = wrap_wiki_chars(inline_chars(row.get(i).copied().unwrap_or("")), (*w).max(1));
                if wrapped.is_empty() {
                    vec![Vec::new()]
                } else {
                    wrapped
                }
            })
            .collect();
        let height = cells.iter().map(Vec::len).max().unwrap_or(1);
        for line_no in 0..height {
            let mut out = Vec::new();
            for (i, w) in widths.iter().enumerate() {
                if i > 0 {
                    out.extend(gap.chars().map(|c| (c, None, false)));
                }
                let cell = cells[i].get(line_no).cloned().unwrap_or_default();
                let len = cell.len();
                out.extend(cell);
                // Pad the cell, but not the last column: a row ends where its text does.
                out.extend(std::iter::repeat_n((' ', None, false), if i + 1 < cols { w.saturating_sub(len) } else { 0 }));
            }
            out_rows.push(out);
        }
    }
    out_rows
}

/// A table row's cells: split on `|`, but never on the one inside a `[[target|label]]` link.
fn row_cells(line: &str) -> Vec<&str> {
    let mut cells = Vec::new();
    let mut start = 0;
    let mut in_link = false;
    let mut chars = line.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match (c, chars.peek().map(|(_, c)| *c)) {
            ('[', Some('[')) => {
                in_link = true;
                chars.next();
            }
            (']', Some(']')) => {
                in_link = false;
                chars.next();
            }
            ('|', _) if !in_link => {
                cells.push(line[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    cells.push(line[start..].trim());
    // The row's own leading and trailing pipes leave an empty cell at each end.
    if cells.first().is_some_and(|c| c.is_empty()) {
        cells.remove(0);
    }
    if cells.last().is_some_and(|c| c.is_empty()) {
        cells.pop();
    }
    cells
}

fn draw_wiki_row(
    f: &mut Frame,
    area: Rect,
    y: u16,
    line: &[(char, Option<String>, bool, Style)],
    heading: bool,
    app: &mut App,
    link_index: &mut usize,
) {
    let mut x = 0usize;
    let mut i = 0usize;
    while i < line.len() {
        let link = line[i].1.clone();
        let is_bold = line[i].2;
        let mut end = i + 1;
        while end < line.len() && line[end].1 == link && line[end].2 == is_bold && line[end].3 == line[i].3 {
            end += 1;
        }
        let txt: String = line[i..end].iter().map(|x| x.0).collect();
        let click_only = link.as_deref().is_some_and(|l| l.starts_with('~'));
        // `!slug` is an item's coloured symbol: drawn like the item, but not a link.
        let symbol = link.as_deref().and_then(|l| l.strip_prefix('!')).and_then(crate::app::App::book_entry_for_slug_for_ui);
        let selected = link.is_some() && !click_only && symbol.is_none() && *link_index == app.book_selected;
        let mut style = if let Some(Entry::Item(item)) = symbol {
            item_cell(item, Color::Reset).1
        } else if let Some(slug) = link.as_deref() {
            if let Some(Entry::Item(item)) = crate::app::App::book_entry_for_slug_for_ui(slug) {
                let (glyph, glyph_style) = item_cell(item, Color::Reset);
                if txt == glyph.to_string() {
                    glyph_style.underlined()
                } else {
                    Style::new().fg(Color::Cyan).underlined()
                }
            } else {
                Style::new().fg(Color::Cyan).underlined()
            }
        } else {
            line[i].3
        };
        if is_bold || heading {
            style = style.fg(Color::Indexed(229)).bold();
        }
        if selected {
            style = style.reversed();
        }
        f.buffer_mut().set_stringn(area.x + x as u16, y, &txt, (area.width as usize).saturating_sub(x), style);
        if let Some(slug) = link.filter(|_| symbol.is_none()) {
            let rect = Rect::new(area.x + x as u16, y, txt.chars().count() as u16, 1);
            if let Some(real) = slug.strip_prefix('~') {
                // A link you can click but not step to with the arrows.
                app.book_click_only.push((rect, real.to_string()));
            } else {
                app.book_urls.push((rect, slug.clone()));
                if let Some(e) = crate::app::App::book_entry_for_slug_for_ui(&slug) {
                    app.book_links.push((rect, e));
                }
                *link_index += 1;
            }
        }
        x += txt.chars().count();
        i = end;
    }
}

fn wrap_wiki_chars(chars: Vec<(char, Option<String>, bool)>, width: usize) -> Vec<Vec<(char, Option<String>, bool)>> {
    let mut rows: Vec<Vec<(char, Option<String>, bool)>> = vec![Vec::new()];
    for item in chars {
        if item.0 == ' ' && rows.last().unwrap().is_empty() {
            continue;
        }
        rows.last_mut().unwrap().push(item);
        if rows.last().unwrap().len() > width {
            let row = rows.last_mut().unwrap();
            if let Some(space) = row.iter().take(width).rposition(|x| x.0 == ' ') {
                let tail = row.split_off(space + 1);
                row.pop();
                rows.push(tail);
            } else {
                let last = row.pop().unwrap();
                rows.push(vec![last]);
            }
        }
    }
    rows
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

/// " and a letter from X" for the companies a key opens. A company with another key says so:
/// "a letter from X (opens on A or B)".
#[allow(dead_code)]
fn key_letter_text(key: Made) -> String {
    let from: Vec<String> = Company::ALL
        .iter()
        .filter(|c| c.key_items().contains(&key))
        .map(|c| match c.key_items() {
            [_] => c.name().to_string(),
            keys => format!("{} (opens on {})", c.name(), keys_text(keys)),
        })
        .collect();
    if from.is_empty() {
        String::new()
    } else {
        format!(" and a letter from {}", from.join(" and "))
    }
}

#[allow(dead_code)]
fn node_name(n: Node) -> String {
    match n {
        Node::Start => "Copper Works".into(),
        Node::Tech(t) => t.name().into(),
        Node::Key(k) => format!("* {}", k.name()),
        Node::Group(name) => name.into(),
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}

/// Where a key item comes from.
fn key_hint(k: Made) -> String {
    match k {
        Made::Item(i) => recipe_hint(i),
        Made::Fluid(_) => String::new(),
    }
}

fn recipe_hint(item: Item) -> String {
    for (machine, list) in [
        ("press", PRESS_RECIPES),
        ("furnace", FURNACE_RECIPES),
        ("blast furnace", BLAST_RECIPES),
        ("coke oven", COKE_OVEN_RECIPES),
        ("crude assembler", CRUDE_RECIPES),
        ("mixer", MIXER_RECIPES),
        ("basic assembler", BASIC_RECIPES),
        ("advanced assembler", ADVANCED_RECIPES),
    ] {
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

/// Draws the current telegram as a box in `r`. `overlay`: open over the map (n or Esc closes it);
/// otherwise it sits in the sidebar while the game keeps running.
fn draw_mail_in(f: &mut Frame, r: Rect, app: &mut App, overlay: bool) {
    let Some(m) = &app.mail else { return };
    let head = Style::new().fg(KEY_ITEM).add_modifier(Modifier::BOLD);
    let inner_w = r.width.saturating_sub(3);
    let (top, body, mut foot) = mail_parts(m, inner_w);
    let waiting = app.world.mail.len();
    let key = app.controls.key(Action::Mail);
    // Work out the scroll room first, with a one-row hint; a wrapped hint just takes a row more.
    let view = r.height.saturating_sub(2 + (top.len() + foot.len()) as u16 + 2);
    let max_scroll = (body.len() as u16).saturating_sub(view);
    app.mail_scroll = app.mail_scroll.min(max_scroll);
    let scroll = if overlay {
        format!("↑↓ scroll {}/{max_scroll}", app.mail_scroll)
    } else {
        format!("PgUp/PgDn {}/{max_scroll}", app.mail_scroll)
    };
    let act = if overlay { format!("{key}/Esc close") } else { format!("{key} {}", if waiting > 0 { "next" } else { "dismiss" }) };
    let mut hints: Vec<(u8, &str)> = vec![(0, act.as_str())];
    if max_scroll > 0 {
        hints.insert(0, (1, scroll.as_str()));
    }
    foot.push(Line::raw(""));
    foot.push(footer_line(inner_w, &hints, None));
    let foot_h = foot.len() as u16;
    let title = match (waiting, overlay) {
        (0, _) => " Telegram ".to_string(),
        (n, _) => format!(" Telegram · {n} more "),
    };

    if overlay {
        f.render_widget(Clear, r);
    }
    let block = Block::bordered().title(title).title_style(head).border_style(Style::new().fg(KEY_ITEM));
    let inner = block.inner(r);
    f.render_widget(block, r);
    let [a, b, c] = Layout::vertical([Constraint::Length(top.len() as u16), Constraint::Min(1), Constraint::Length(foot_h)]).areas(inner);
    f.render_widget(Paragraph::new(top), a);
    f.render_widget(Paragraph::new(body).scroll((app.mail_scroll, 0)), b);
    // A thin scrollbar in the spare right column (the body is wrapped one column narrower),
    // so a long telegram shows how much is left.
    if max_scroll > 0 && b.width > 1 && b.height > 1 {
        let bar = Rect { x: b.x + b.width - 1, width: 1, ..b };
        let mut state =
            ScrollbarState::new(max_scroll as usize).position(app.mail_scroll as usize).viewport_content_length(b.height as usize);
        let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .track_style(Style::new().fg(Color::DarkGray))
            .thumb_symbol("┃")
            .thumb_style(Style::new().fg(KEY_ITEM));
        f.render_stateful_widget(sb, bar, &mut state);
    }
    f.render_widget(Paragraph::new(foot), c);
}

/// The telegram open over a small pane.
fn draw_mail(f: &mut Frame, area: Rect, app: &mut App) {
    let w = 78.min(area.width);
    let h = mail_height(app, w).min(area.height.saturating_sub(2)).max(8);
    let r = centered(area, w, h);
    draw_mail_in(f, r, app, true);
}

fn draw_help(f: &mut Frame, area: Rect, app: &mut App) {
    let text = "\
READING THE MAP
  Coloured blocks are buildings, labelled with their name. Production is
  big (2x2 up to 4x4), belts / splitters / arms / crates are 1x1. Presses, wire
  drawers, winders and pumps are long (2x1, 1x2) and turn with rotate (r).
  The arrow on a building's edge is its output side; machines also show a
  letter for what they make. A grey block is idle.
  Arrows are belts; items ride on them. Point at anything to see what it is.
  Drills mine anywhere: press f on one to pick copper, coal, iron or stone.
  Items: o copper ore  * coal  = plate  ~ wire  0 pipe  @ coil  # coke
         o iron ore (blue)  = iron plate (blue)  % steel  , waste  H girder
         c concrete  g gear  G gearbox  R drilling rig  i ingot  o stone
         : grit (copper, blue iron) and sand (tan)  ~ fine wire (pale)
         - sheet  ! iron rod  x screw  z spring (iron ones are blue)
         b brick  u glass  O bearing  C reinforced concrete
         ! - steel rod and sheet (white)  = g hardened plate, gear (steel blue)
         V valve  Q gauge  L lamp  r relay  M motor  P piston  B control board
         D boiler shell  S silo shell  F frame  U pump unit  E generator
         N engine block  T control panel  Y steam tractor
  Pipes show what they hold: blue water, white steam, orange copper,
         red iron, pale steel and silicon (molten) and grey when empty.

HOW IT WORKS
  Buildings take input from any side and output from the arrow side only:
    onto a belt, or straight into the next building (drill -> furnace).
  Drills burn coal; a drill on coal fuels itself. Cheap ones cut iron slower.
  Furnaces melt ore into molten metal and push it out the front, into a pipe
    or straight into a molder. Molders cast it into ingots (EC); presses
    make plates from ingots. Crushers grind ore into grit that melts further;
    a coke oven bakes its own coal (no EC) and leaves waste. Wire drawers,
    winders, rolling mills and lathes shape plates into wire, coils, sheets,
    rods, screws and springs. Machines make whatever recipe they're set to
    (f changes it). A pipe network holds one fluid. A machine with two
    outputs waits (FULL) until both have room. Waste always leaves by the
    back, opposite the arrow; with nothing there to take it the machine
    stops (BACK).
  Belts, splitters, arms, molders and presses need EC (electric charge).
    Solar panels and poles charge every electric part in range (v shows
    it); overlapping ranges join into one grid that shares only its own
    EC. Panels never join each other directly: put poles between them.
  Pumps draw water anywhere on the ground. Pipes join into networks, each holding one fluid.
    Boilers burn coal to turn water into steam
    (out their front); steam engines turn steam into EC, only as much as
    their grid needs, and a boiler burns only what its engines call for.
  Blast furnaces make molten steel (a molder casts it) and hardened plate
    from plate and coke or coal. Most recipes leave waste out the back: a
    trade post set to money takes it for $0.50 each; research posts refuse it
    and it backs up.
  Assemblers build parts and big goods (crude, basic, advanced, precision,
    then the large ones). A higher one runs the lower recipes faster. Welders
    join steel parts and draw EC only while working. Kilns fire 8 at once and
    burn coal. Mixers take water from a pipe.
  Floors: support beams open +1, excavation opens -1 (ore there is richer).
    Elevators (2x2 to 8x8, +/- to resize) have a door on every open floor;
    f sets each door to load, unload or pass. A car leaves only when full.
  Idle machines flash a word saying why (FULL, BACK, NEED, FLUD, COAL, EC?, LOAD...).
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
  Build menu ... 1-4 opens a category (1 Create, 2 Transport, 3 EC,
                 4 Misc); then a number picks a part (Create has 1-5). Press
                 it again to switch kind (belt / fast belt...). Esc goes back.
  Place ........ Space / Enter
  Rotate ....... r (output side)     Remove ....... x (full refund)
  Set up ....... f                   Research ..... t
  Area ......... c marks a rectangle (Space at two corners, or drag);
                 then x removes everything in it, full refund. Esc
                 cancels, z puts back what the last area remove took.
  Upgrades ..... (research the Upgrade planner, t) with a rectangle
                 marked, u shows what it would do ('u again to upgrade
                 12 parts for $360'), a second u does it: belts, cheap
                 drills and panels, big drills and assemblers become
                 their next version in place, same facing, settings and
                 recipe, and you pay only the price difference. It goes
                 in reading order and stops when the money runs out. A
                 green tint marks the parts it will swap; z swaps back.
  Blueprints ... (research Blueprints, t) with a rectangle marked, c or
                 Space copies it to your hand; a ghost follows the cursor
                 (yellow fits, xx can't, .. can't afford). r turns it,
                 Shift+r turns back, Space or a click pastes what fits
                 and what you can afford, in reading order; press again
                 at the same spot for the rest, nothing is charged twice.
                 Esc empties the hand. l opens the library: Enter takes
                 one, f saves your hand under a name, x twice deletes.
                 Files live in ~/.terminactory/blueprints, for every save.
  EC range ..... v                   Floors ....... < >
  Pause ........ p                   Speed ........ [ ]
  Save ......... Ctrl+S (it also saves every 2 minutes and when you quit)
  Production ... o (last minute per item; Tab: earnings history)
  Settings ..... g (sidebar graph, small-pane view and width, telegram
                 style, hints; kept in settings.json)
  Recipes ...... b (opens on what's under the cursor)
  Small pane ... m (it switches on by itself in a small terminal; g sets
                 Auto, Always or Never, and the width Auto uses)
  Telegrams .... n next (Shift+N dismisses all), PgUp/PgDn scroll,
                 i log of past telegrams (Enter reads one again).
                 g sets where they show: Sidebar, a Note, or a Ticker
                 (mail count on top); with Note or Ticker n opens one.
  Hints ........ quiet one-line notes on the bottom line when nothing else
                 is there: what is stuck, and now and then a company's
                 aside (full view only, rare). g turns them off.
  Mouse ........ click places, drag lays belts, right-click removes,
                 middle-click copies a part (facing and settings too),
                 middle-drag copies a rectangle to your hand
  Quit ......... Ctrl+C (saves first)
  These are the default keys. Change any of them in controls.json.";
    // A small pane gets the keys first and no item table (the recipe book has it).
    let small = area.width < 80 || area.height < 30;
    let mut sections: Vec<String> = text.split("\n\n").map(String::from).collect();
    if small {
        let c = &app.controls;
        let keys = format!(
            "KEYS\n  Move {} · Build 1-4 then a number · Place {} · Back {}\n  Research {} · Recipes {} · Production {} · Settings {} · Help {}\n  Area {} marks a rectangle, {} removes it, {} upgrades it, {} puts it back\n  Ctrl+C quits and saves. {} lists every item and machine.",
            "WASD/arrows",
            c.key(Action::Place),
            c.key(Action::Back),
            c.key(Action::Tree),
            c.key(Action::Book),
            c.key(Action::Stats),
            c.key(Action::Settings),
            c.key(Action::Help),
            c.key(Action::Area),
            c.key(Action::Remove),
            c.key(Action::Upgrade),
            c.key(Action::Restore),
            c.key(Action::Book),
        );
        let controls = sections.pop().unwrap_or_default();
        let mut map = sections.remove(0);
        if let (Some(a), Some(b)) = (map.find("  Items:"), map.find("  Pipes show")) {
            map.replace_range(a..b, "  Items: the recipe book (b) shows every item and what makes it.\n");
        }
        sections.insert(0, map);
        sections.insert(0, controls);
        sections.insert(0, keys);
    }
    let width = 80.min(area.width);
    let inner_w = width.saturating_sub(3).max(10) as usize;
    let heading = |l: &str| l.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && !l.starts_with("  ") && l == l.to_uppercase();
    let mut lines: Vec<Line> = Vec::new();
    for (n, section) in sections.iter().enumerate() {
        if n > 0 {
            lines.push(Line::raw(""));
        }
        for l in section.lines() {
            if heading(l) {
                lines.push(Line::styled(l.to_string(), Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
                continue;
            }
            if l.chars().count() <= inner_w {
                lines.push(Line::raw(l.to_string()));
                continue;
            }
            // Too wide: a two-column line splits at its gap, deep indents (the hand-wrapped
            // continuation lines) come in to 4, and a wrapped line hangs two spaces in.
            let indent = (l.len() - l.trim_start().len()).min(4);
            let parts: Vec<&str> = l.trim_start().split("     ").map(str::trim).filter(|p| !p.is_empty()).collect();
            for part in parts {
                for (i, row) in wrap_words(part, inner_w.saturating_sub(indent + 2).max(8)).into_iter().enumerate() {
                    let pad = " ".repeat(if i == 0 { indent } else { indent + 2 });
                    lines.push(Line::raw(format!("{pad}{row}")));
                }
            }
        }
    }
    let rows = lines.len() as u16;
    let h = (rows + 3).min(area.height);
    let r = centered(area, width, h);
    // The last inner row is the footer; arrows scroll the rest when it doesn't all fit.
    let body_h = h.saturating_sub(3);
    let max = rows.saturating_sub(body_h);
    app.help_scroll = app.help_scroll.min(max);
    f.render_widget(Clear, r);
    let block = Block::bordered().title(" Help ").title_style(Style::new().fg(Color::Cyan));
    let inner = block.inner(r);
    f.render_widget(block, r);
    let body = Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(1), body_h);
    f.render_widget(Paragraph::new(lines).scroll((app.help_scroll, 0)), body);
    let scroll = format!("↑↓ scroll {}/{max}", app.help_scroll);
    let hints: Vec<(u8, &str)> = if max > 0 { vec![(1, scroll.as_str()), (0, "any other key closes")] } else { vec![(0, "press any key")] };
    let more = (app.help_scroll < max).then_some("more ▼");
    let foot = Rect::new(inner.x + 1, inner.y + inner.height.saturating_sub(1), inner.width.saturating_sub(2), 1);
    f.render_widget(Paragraph::new(footer_line(foot.width, &hints, more)), foot);
}

mod graph;
mod history;
mod library;
mod maillog;
mod settings;
#[cfg(test)]
mod tests;
