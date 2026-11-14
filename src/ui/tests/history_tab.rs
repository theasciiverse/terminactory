//! The production panel's History tab: tab switching, range keys, the charts
//! at narrow and wide sizes, the empty state and a few snapshots.

use super::*;
use crate::history::{Bucket, Range};
use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// 2026-11-11 12:00 UTC, a Wednesday: the clock every test draws at.
pub const NOON: u64 = 1_794_398_400;

fn key(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::from(code));
}

fn press(app: &mut App, c: char) {
    key(app, KeyCode::Char(c));
}

/// Gives an app four months of earnings (with some days off), the last two days by the hour and
/// the last hour by the minute, and fixes its clock at `NOON`.
pub fn fill(app: &mut App) {
    app.clock = Some((NOON, 0));
    let h = &mut app.world.history;
    let today = NOON / 86_400;
    for back in 0..120u64 {
        if back % 9 == 4 {
            continue; // a day away
        }
        let money = if back == 0 { 1_234.0 } else { ((back * 37) % 11 + 1) as f64 * 100.0 + back as f64 };
        h.days.push_front(Bucket { t: today - back, money, research: (back % 5) as f64 * 3.0 });
    }
    let hour = NOON / 3600;
    for back in 0..48u64 {
        h.hours.push_front(Bucket { t: hour - back, money: ((back * 7) % 13) as f64 * 60.0, research: (back % 4) as f64 });
    }
    let minute = NOON / 60;
    for back in 0..60u64 {
        h.minutes.push_front(Bucket { t: minute - back, money: ((back * 5) % 17) as f64 * 4.0, research: 0.0 });
    }
}

fn app_with_history() -> App {
    let mut w = World::new();
    w.build_demo().unwrap();
    let mut app = App::new(w);
    fill(&mut app);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    // These tests count the tab's own glyphs, so the sidebar graph stays out of the way.
    app.settings.sidebar_graph = crate::settings::GraphShow::Off;
    app.open_site("production");
    app.book_full = true;
    app.stats_history = true;
    app
}

fn rows(t: &str) -> Vec<&str> {
    t.lines().collect()
}

#[test]
fn tab_switches_between_production_and_history() {
    let mut app = app_with_history();
    app.stats_history = false;
    let t = text(&render(&mut app, 100, 30));
    assert!(t.contains(" Production ") && t.contains(" History ") && t.contains("last minute"), "{t}");
    assert!(t.contains("Tab history") && !t.contains("less"), "{t}");
    key(&mut app, KeyCode::Tab);
    assert!(app.book && app.stats_history);
    let t = text(&render(&mut app, 100, 30));
    assert!(t.contains("Money ·") && t.contains("Tab production"), "{t}");
    assert!(!t.contains("last minute"), "{t}");
    key(&mut app, KeyCode::Tab);
    assert!(app.book && !app.stats_history);
    // Shift+Tab too; Esc puts the browser away and o brings the production site back.
    key(&mut app, KeyCode::BackTab);
    assert!(app.stats_history);
    key(&mut app, KeyCode::Esc);
    assert!(!app.book);
    press(&mut app, 'o');
    assert!(app.book && app.book_slug == "production");
}

#[test]
fn clicking_a_tab_switches_to_it() {
    let mut app = app_with_history();
    render(&mut app, 100, 30);
    let (r, _) = *app.stats_hits.iter().find(|(_, h)| *h == crate::app::StatsHit::Tab(false)).unwrap();
    app.on_mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x + 1, row: r.y, modifiers: KeyModifiers::NONE });
    assert!(app.book && !app.stats_history);
    // And a click on a range pill picks it, a click on a day picks that day.
    app.stats_history = true;
    render(&mut app, 100, 30);
    let (r, _) = *app.stats_hits.iter().find(|(_, h)| *h == crate::app::StatsHit::Range(Range::Days)).unwrap();
    app.on_mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
    assert_eq!(app.hist_range, Range::Days);
    assert!(app.book, "still open");
}

#[test]
fn range_keys_pick_seconds_minutes_hours_and_days() {
    let mut app = app_with_history();
    for (c, r) in [('1', Range::Seconds), ('2', Range::Minutes), ('3', Range::Hours), ('4', Range::Days)] {
        press(&mut app, c);
        assert_eq!(app.hist_range, r, "{c}");
        assert!(app.book, "{c} keeps the panel open on the History tab");
    }
    // [ and ] stay the game's speed keys, and pause works too, all without closing the panel.
    let speed = app.speed;
    press(&mut app, ']');
    assert_eq!((app.speed, app.hist_range, app.book), (speed * 2, Range::Days, true));
    press(&mut app, '[');
    assert_eq!((app.speed, app.book), (speed, true));
    press(&mut app, 'p');
    assert!(app.paused && app.book);
    press(&mut app, 'p');
    press(&mut app, '1');
    // The chart says which range it is showing.
    let t = text(&render(&mut app, 100, 30));
    assert!(t.contains("last 60 seconds") && t.contains("-10s") && t.contains("now"), "{t}");
    press(&mut app, '2');
    let t = text(&render(&mut app, 100, 30));
    assert!(t.contains("last 60 minutes") && t.contains("-10m") && t.contains("now"), "{t}");
    press(&mut app, '3');
    let t = text(&render(&mut app, 100, 30));
    assert!(t.contains("last 48 hours") && t.contains("-12h"), "{t}");
    press(&mut app, '4');
    let t = text(&render(&mut app, 100, 30));
    assert!(t.contains("days") && t.contains("Nov"), "{t}");
    // On the Production tab the digits are the browser's: 1 picks the first browser tab.
    app.stats_history = false;
    press(&mut app, '1');
    assert!(app.book && app.book_slug != "production");
}

#[test]
fn research_swaps_with_money() {
    let mut app = app_with_history();
    press(&mut app, '3');
    let t = text(&render(&mut app, 100, 30));
    assert!(t.contains("Money ·") && t.contains('$'), "{t}");
    press(&mut app, 'r');
    assert!(app.hist_research);
    let t = text(&render(&mut app, 100, 30));
    assert!(t.contains("Research ·") && t.contains("research"), "{t}");
    press(&mut app, 'r');
    assert!(!app.hist_research);
}

#[test]
fn the_chart_has_a_y_axis_a_time_axis_a_total_and_the_best_bar() {
    let mut app = app_with_history();
    press(&mut app, '3');
    let t = text(&render(&mut app, 100, 36));
    // Hours: the best of the last 48 is 12 * 60 = $720 at some hour; the axis tops out above it.
    assert!(t.contains("best $720"), "{t}");
    assert!(t.contains("total $") && t.contains("$0┤") && t.contains("┤"), "{t}");
    assert!(t.contains("-6h") && t.contains("now"), "{t}");
    assert!(t.contains('█') || t.contains('▇') || t.contains('▆'), "bars: {t}");
}

#[test]
fn no_history_yet_is_a_friendly_empty_state() {
    let mut app = app_with_history();
    app.world.history = Default::default();
    let t = text(&render(&mut app, 100, 30));
    assert!(t.contains("Earnings show up here as your factory sells."), "{t}");
    assert!(!t.contains("less") && !t.contains('┤'), "{t}");
    // Days played with nothing earned are still empty.
    app.world.history.days.push_back(Bucket { t: NOON / 86_400, money: 0.0, research: 0.0 });
    let t = text(&render(&mut app, 100, 30));
    assert!(t.contains("Earnings show up here"), "{t}");
    // And it fits a phone-sized pane.
    let t = text(&render(&mut app, 36, 24));
    assert!(t.contains("Earnings show up here"), "{t}");
    assert!(t.contains("Esc") && t.contains("Tab"), "{t}");
}

#[test]
fn a_short_pane_scrolls_and_says_so() {
    let mut app = app_with_history();
    let t = text(&render(&mut app, 64, 14));
    assert!(t.contains("more ▼"), "{t}");
    for _ in 0..5 {
        key(&mut app, KeyCode::PageDown);
    }
    let t = text(&render(&mut app, 64, 14));
    assert!(t.contains("more ▲") && t.contains("now"), "{t}");
}

#[test]
fn history_snapshots() {
    let mut app = app_with_history();
    press(&mut app, '3');
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("Money") && !t.contains("less") && !t.contains("Mon "), "no heatmap: {t}");
    std::fs::write("target/snapshot-history.html", to_html(&buf)).unwrap();
    press(&mut app, '2');
    let buf = render(&mut app, 130, 42);
    std::fs::write("target/snapshot-history-hours.html", to_html(&buf)).unwrap();
    // The small pane.
    press(&mut app, '3');
    let buf = render(&mut app, 64, 24);
    let t = text(&buf);
    assert!(t.contains("Money ·") && t.contains("Tab"), "{t}");
    std::fs::write("target/snapshot-history-small.html", to_html(&buf)).unwrap();
    press(&mut app, 'r');
    let buf = render(&mut app, 36, 30);
    std::fs::write("target/snapshot-history-phone.html", to_html(&buf)).unwrap();
}

#[test]
fn the_chart_fills_the_width_of_the_pane() {
    let mut app = app_with_history();
    for key in ['1', '2', '3'] {
        press(&mut app, key);
        for width in [80u16, 130, 220] {
            let t = text(&render(&mut app, width, 36));
            // The time axis ends with "now" right by the panel's right border.
            let axis: Vec<char> = t.lines().find(|l| l.contains("now")).unwrap_or_else(|| panic!("no axis:\n{t}")).chars().collect();
            let now = (0..axis.len() - 2).find(|&i| axis[i..i + 3] == ['n', 'o', 'w']).unwrap() + 3;
            let border = (now..axis.len()).find(|&i| axis[i] == '│').unwrap_or(axis.len());
            assert!(border - now <= 3, "{key} at {width}: 'now' ends at {now}, border at {border}\n{t}");
        }
    }
}

#[test]
fn full_bar_cells_are_painted_so_terminals_show_no_seams() {
    let mut app = app_with_history();
    press(&mut app, '3');
    let buf = render(&mut app, 100, 36);
    let painted = buf.content.iter().filter(|c| c.bg == Color::Green && c.symbol() == " ").count();
    let drawn = buf.content.iter().filter(|c| c.symbol() == "█").count();
    assert!(painted > 20 && drawn == 0, "painted {painted}, block glyphs {drawn}");
}

/// How many chart blocks (a summary line starting "Money ·" or "Research ·") a render shows.
fn charts(t: &str) -> (usize, usize) {
    let sum = |name: &str| rows(t).iter().filter(|l| l.contains(&format!("{name} · "))).count();
    (sum("Money"), sum("Research"))
}

#[test]
fn a_tall_pane_stacks_money_and_research() {
    for (w, h) in [(150, 42), (80, 60), (44, 60), (300, 90)] {
        let mut app = app_with_history();
        app.settings.sidebar_graph = crate::settings::GraphShow::Off;
        let buf = render(&mut app, w, h);
        let t = text(&buf);
        assert_eq!(charts(&t), (1, 1), "{w}x{h}: {t}");
        // Research bars use the research colour, money the green ones.
        let (mut green, mut blue) = (false, false);
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let bg = buf.cell((x, y)).unwrap().bg;
                green |= bg == Color::Green;
                blue |= bg == RESEARCH;
            }
        }
        assert!(green && blue, "{w}x{h}: green {green} research {blue}");
        // Both charts fit without scrolling.
        assert!(!t.contains("more ▼"), "{w}x{h}: {t}");
        if (w, h) == (150, 42) {
            snapshot_tall(&buf);
        }
    }
}

fn snapshot_tall(buf: &Buffer) {
    std::fs::write("target/snapshot-history-stacked.html", to_html(buf)).unwrap();
}

#[test]
fn a_short_pane_keeps_the_money_research_toggle() {
    let mut app = app_with_history();
    // 100x16: the pane is too short for two charts.
    let t = text(&render(&mut app, 100, 16));
    assert_eq!(charts(&t), (1, 0), "{t}");
    press(&mut app, 'r');
    let t = text(&render(&mut app, 100, 16));
    assert_eq!(charts(&t), (0, 1), "{t}");
    // The same pane taller shows both whatever r says.
    let t = text(&render(&mut app, 100, 42));
    assert_eq!(charts(&t), (1, 1), "{t}");
}
