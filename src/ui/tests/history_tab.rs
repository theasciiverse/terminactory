//! The production panel's History tab: tab switching, range keys, the chart, the heatmap
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
    app.stats = true;
    app.stats_history = true;
    app
}

fn rows(t: &str) -> Vec<&str> {
    t.lines().collect()
}

/// How many heatmap squares the first full week row of the grid has.
fn heat_cells(t: &str) -> usize {
    rows(t).iter().map(|l| l.matches('■').count() + l.matches('·').count()).filter(|n| *n > 0).max().unwrap_or(0)
}

#[test]
fn tab_switches_between_production_and_history() {
    let mut app = app_with_history();
    app.stats_history = false;
    let t = text(&render(&mut app, 100, 30));
    assert!(t.contains(" Production ") && t.contains(" History ") && t.contains("last minute"), "{t}");
    assert!(t.contains("Tab history") && !t.contains("less"), "{t}");
    key(&mut app, KeyCode::Tab);
    assert!(app.stats && app.stats_history);
    let t = text(&render(&mut app, 100, 30));
    assert!(t.contains("less") && t.contains("more") && t.contains("Tab production"), "{t}");
    assert!(!t.contains("last minute"), "{t}");
    key(&mut app, KeyCode::Tab);
    assert!(app.stats && !app.stats_history);
    // Shift+Tab too, and reopening starts on Production.
    key(&mut app, KeyCode::BackTab);
    assert!(app.stats_history);
    press(&mut app, 'o');
    assert!(!app.stats);
    press(&mut app, 'o');
    assert!(app.stats && !app.stats_history);
}

#[test]
fn clicking_a_tab_switches_to_it() {
    let mut app = app_with_history();
    render(&mut app, 100, 30);
    let (r, _) = *app.stats_hits.iter().find(|(_, h)| *h == crate::app::StatsHit::Tab(false)).unwrap();
    app.on_mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x + 1, row: r.y, modifiers: KeyModifiers::NONE });
    assert!(app.stats && !app.stats_history);
    // And a click on a range pill picks it, a click on a day picks that day.
    app.stats_history = true;
    render(&mut app, 100, 30);
    let (r, _) = *app.stats_hits.iter().find(|(_, h)| *h == crate::app::StatsHit::Range(Range::Days)).unwrap();
    app.on_mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
    assert_eq!(app.hist_range, Range::Days);
    let (r, _) = *app.stats_hits.iter().find(|(_, h)| *h == crate::app::StatsHit::Day(10)).unwrap();
    app.on_mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
    assert_eq!(app.hist_day, 10);
    assert!(app.stats, "still open");
}

#[test]
fn range_keys_pick_seconds_minutes_hours_and_days() {
    let mut app = app_with_history();
    for (c, r) in [('1', Range::Seconds), ('2', Range::Minutes), ('3', Range::Hours), ('4', Range::Days)] {
        press(&mut app, c);
        assert_eq!(app.hist_range, r, "{c}");
        assert!(app.stats, "{c} keeps the panel open on the History tab");
    }
    // [ and ] stay the game's speed keys, and pause works too, all without closing the panel.
    let speed = app.speed;
    press(&mut app, ']');
    assert_eq!((app.speed, app.hist_range, app.stats), (speed * 2, Range::Days, true));
    press(&mut app, '[');
    assert_eq!((app.speed, app.stats), (speed, true));
    press(&mut app, 'p');
    assert!(app.paused && app.stats);
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
    // On the Production tab the same keys are the game's: 1 closes the panel and picks a slot.
    app.stats_history = false;
    press(&mut app, '1');
    assert!(!app.stats);
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
fn the_heatmap_shows_fewer_weeks_in_a_narrow_pane() {
    let mut app = app_with_history();
    let wide = text(&render(&mut app, 150, 42));
    let medium = text(&render(&mut app, 80, 42));
    let narrow = text(&render(&mut app, 40, 42));
    // The grid is 7 rows by up to 53 weeks; the newest weeks are the ones kept.
    assert!(heat_cells(&wide) >= 53, "wide: {wide}");
    let (m, n) = (heat_cells(&medium), heat_cells(&narrow));
    assert!(m > n && (45..=53).contains(&m) || m == 53, "medium {m}: {medium}");
    assert!(n >= 20 && n < 40, "narrow {n}: {narrow}");
    for t in [&wide, &medium, &narrow] {
        assert!(t.contains("less") && t.contains("more"), "{t}");
        assert!(rows(t).iter().all(|l| l.chars().count() <= 150));
    }
    // Month labels sit above the grid: the newest month is always there.
    assert!(wide.contains("Nov") && wide.contains("Oct") && wide.contains("Jul"), "{wide}");
    assert!(narrow.contains("Nov") && narrow.contains("Oct"), "{narrow}");
    // Weekday names down the side (initials in a narrow pane).
    assert!(wide.contains("Mon") && wide.contains("Wed") && wide.contains("Fri"), "{wide}");
    // Seven rows: today (Wednesday) ends the newest column, so Thursday to Saturday are one short.
    let grid: Vec<usize> = rows(&wide)
        .iter()
        .filter(|l| l.matches('■').count() + l.matches('·').count() > 20)
        .map(|l| l.matches('■').count() + l.matches('·').count())
        .collect();
    assert_eq!(grid.len(), 7, "{grid:?}");
    assert_eq!(grid[0], 53);
    assert_eq!(grid[3], 53, "Wednesday is today's row");
    assert_eq!(grid[4], 52, "Thursday is still to come");
    // Days nobody played are a faint dot; played days are squares.
    assert!(wide.contains('·') && wide.contains('■'));
}

#[test]
fn arrows_pick_a_day_and_the_line_below_says_what_it_earned() {
    let mut app = app_with_history();
    let t = text(&render(&mut app, 120, 42));
    assert!(t.contains("Wed Nov 11: $1,234"), "today: {t}");
    key(&mut app, KeyCode::Up);
    assert_eq!(app.hist_day, 1);
    let t = text(&render(&mut app, 120, 42));
    assert!(t.contains("Tue Nov 10: $"), "{t}");
    key(&mut app, KeyCode::Left);
    assert_eq!(app.hist_day, 8);
    let t = text(&render(&mut app, 120, 42));
    assert!(t.contains("Tue Nov 3: $"), "a week earlier: {t}");
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Down);
    assert_eq!(app.hist_day, 0);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Right);
    assert_eq!(app.hist_day, 0, "can't go past today");
    // A day off is "not played"; 4 days back is one (day 4 of the fill).
    app.hist_day = 4;
    let t = text(&render(&mut app, 120, 42));
    assert!(t.contains("Sat Nov 7: not played"), "{t}");
    // WASD works as the arrows do (they are the game's move keys).
    press(&mut app, 'w');
    assert_eq!(app.hist_day, 5);
    // The picked day stays on screen: in a narrow pane it's clamped to the weeks shown.
    app.hist_day = 370;
    render(&mut app, 40, 42);
    assert!(app.hist_day < 370 && app.hist_day >= 100, "{}", app.hist_day);
    // Holding Shift (belts) doesn't move it.
    app.hist_day = 3;
    app.on_key(KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT));
    assert_eq!(app.hist_day, 3);
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
    assert!(t.contains("Esc close") && t.contains("Tab"), "{t}");
}

#[test]
fn a_short_pane_scrolls_and_says_so() {
    let mut app = app_with_history();
    let t = text(&render(&mut app, 64, 14));
    assert!(t.contains("more ▼") && !t.contains("less"), "{t}");
    key(&mut app, KeyCode::PageDown);
    key(&mut app, KeyCode::PageDown);
    key(&mut app, KeyCode::PageDown);
    let t = text(&render(&mut app, 64, 14));
    assert!(t.contains("more ▲") && t.contains("less"), "{t}");
    // Picking a day jumps to the heatmap.
    app.stats_scroll = 0;
    key(&mut app, KeyCode::Left);
    let t = text(&render(&mut app, 64, 14));
    assert!(t.contains("Wed Nov 4:") || t.contains("Nov 4"), "{t}");
}

#[test]
fn history_snapshots() {
    let mut app = app_with_history();
    press(&mut app, '3');
    app.hist_day = 12;
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("Money") && t.contains("less"), "{t}");
    std::fs::write("target/snapshot-history.html", to_html(&buf)).unwrap();
    press(&mut app, '2');
    let buf = render(&mut app, 130, 42);
    std::fs::write("target/snapshot-history-hours.html", to_html(&buf)).unwrap();
    // The small pane.
    press(&mut app, '3');
    let buf = render(&mut app, 64, 24);
    let t = text(&buf);
    assert!(t.contains("less") && t.contains("Tab"), "{t}");
    std::fs::write("target/snapshot-history-small.html", to_html(&buf)).unwrap();
    press(&mut app, 'r');
    let buf = render(&mut app, 36, 30);
    std::fs::write("target/snapshot-history-phone.html", to_html(&buf)).unwrap();
}
