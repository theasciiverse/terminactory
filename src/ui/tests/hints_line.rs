//! Hints and asides on the idle status line, and the one rule for who gets the slot.

use super::*;
use crate::hints::{Aside, What};
use crate::settings::{OnOff, SmallView};
use std::time::{Duration, Instant};

fn key(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::from(code));
}

fn press(app: &mut App, c: char) {
    key(app, KeyCode::Char(c));
}

fn lines(t: &str) -> Vec<&str> {
    t.lines().collect()
}

fn last(t: &str) -> String {
    lines(t).last().unwrap().to_string()
}

const NO_POST: &str = "No trade post yet: items have nowhere to go";
const ASIDE: &str = "Hollowmere: Good coal. Dad would have wept.";

/// A world with one drill and no trade post, the cursor on empty ground, no message showing.
fn needs_a_post() -> App {
    let mut w = World::new();
    w.econ.money = 100_000.0;
    w.place(0, 50, 40, Kind::Drill, Dir::E).unwrap();
    let mut app = App::new(w);
    app.msg_at = Instant::now() - Duration::from_secs(60);
    app.cursor = (60, 40);
    app
}

/// Ticks the hints once a second, `from` to `to` seconds after `t0`.
fn tick(app: &mut App, t0: Instant, from: u64, to: u64) {
    for s in from..=to {
        app.tick_hints(t0 + Duration::from_secs(s));
    }
}

/// The app with the hint up, and the view it was drawn for.
fn with_hint(w: u16, h: u16) -> (App, Instant) {
    let mut app = needs_a_post();
    app.hints = crate::hints::Hints::new(Instant::now());
    let t = Instant::now();
    render(&mut app, w, h); // decides small or large
    tick(&mut app, t, 0, 21);
    assert!(app.hints.current.is_some());
    (app, t)
}

#[test]
fn a_hint_takes_the_idle_status_line_in_both_views() {
    for (w, h) in [(130, 40), (200, 60), (64, 16), (44, 60)] {
        let (mut app, _) = with_hint(w, h);
        let t = text(&render(&mut app, w, h));
        let bottom = last(&t);
        assert!(bottom.contains(&NO_POST[..24]), "{w}x{h}: {bottom}");
        assert!(!bottom.contains("help"), "the hint replaced the key list at {w}x{h}: {bottom}");
        assert!(bottom.chars().count() <= w as usize);
    }
    // Wide enough, the whole line is there.
    let (mut app, _) = with_hint(130, 40);
    assert!(last(&text(&render(&mut app, 130, 40))).contains(NO_POST));
}

#[test]
fn a_hint_fades_and_the_key_list_comes_back() {
    let (mut app, t) = with_hint(130, 40);
    tick(&mut app, t, 22, 40);
    assert!(app.hints.current.is_none());
    let bottom = last(&text(&render(&mut app, 130, 40)));
    assert!(bottom.contains("help") && !bottom.contains("No trade post"), "{bottom}");
}

#[test]
fn the_slot_goes_to_messages_then_stuck_reasons_then_hints_then_asides() {
    let (mut app, t) = with_hint(130, 40);
    let now = t + Duration::from_secs(22);
    render(&mut app, 130, 40);
    // Hint up, nothing above it: the hint.
    assert_eq!(super::super::status_slot(&app, now), super::super::Slot::Hint(NO_POST.into()));
    // A message from an action beats it, and the hint is not replaced for good: it comes back.
    app.msg_at = now;
    app.msg = "Placed a belt".into();
    assert_eq!(super::super::status_slot(&app, now), super::super::Slot::Message);
    let bottom = last(&text(&render(&mut app, 130, 40)));
    assert!(bottom.contains("Placed a belt") || bottom.contains(NO_POST), "{bottom}");
    app.msg_at = Instant::now() - Duration::from_secs(60);
    assert!(matches!(super::super::status_slot(&app, now), super::super::Slot::Hint(_)));

    // A stuck machine under the cursor beats it too. In the full view the sidebar says why, so the
    // line shows the keys; in the small pane the line itself says why.
    app.cursor = (50, 40);
    if let St::Drill { fuel, .. } = &mut app.world.floors[0].tiles[idx(50, 40)].as_mut().unwrap().st {
        *fuel = 0;
    }
    render(&mut app, 130, 40);
    assert_eq!(super::super::status_slot(&app, now), super::super::Slot::Here);
    let bottom = last(&text(&render(&mut app, 130, 40)));
    assert!(bottom.contains("help") && !bottom.contains("No trade post"), "{bottom}");
    app.settings.small_view = SmallView::Always;
    render(&mut app, 130, 40);
    assert_eq!(super::super::status_slot(&app, now), super::super::Slot::Here);
    let bottom = last(&text(&render(&mut app, 130, 40)));
    assert!(bottom.contains("needs coal") && !bottom.contains("No trade post"), "{bottom}");
    app.settings.small_view = SmallView::Auto;
    app.cursor = (60, 40);

    // An aside only ever takes a free slot with no hint: put one up in place of the hint.
    render(&mut app, 130, 40);
    app.hints.current = None;
    app.hints.force(What::Aside(Aside::A3), ASIDE, now);
    assert_eq!(super::super::status_slot(&app, now), super::super::Slot::Aside(ASIDE.into()));
    // A message beats an aside as well.
    app.msg_at = now;
    assert_eq!(super::super::status_slot(&app, now), super::super::Slot::Message);
}

#[test]
fn a_fresh_message_holds_the_slot_so_no_new_hint_starts() {
    let mut app = needs_a_post();
    app.hints = crate::hints::Hints::new(Instant::now());
    let t = Instant::now();
    render(&mut app, 130, 40);
    app.msg_at = t + Duration::from_secs(60);
    // The message keeps coming (as a player acting would make): the hint waits, unspent.
    for s in 0..=60 {
        app.say("Placed a belt");
        app.msg_at = t + Duration::from_secs(s);
        app.tick_hints(t + Duration::from_secs(s));
    }
    assert!(app.hints.current.is_none());
    tick(&mut app, t, 67, 67);
    assert!(app.hints.current.is_some(), "it comes up once the message has faded");
}

#[test]
fn hints_off_shows_none_and_nor_do_asides() {
    let mut app = needs_a_post();
    app.settings.hints = OnOff::Off;
    let t = Instant::now();
    render(&mut app, 130, 40);
    tick(&mut app, t, 0, 600);
    assert!(app.hints.current.is_none());
    let bottom = last(&text(&render(&mut app, 130, 40)));
    assert!(bottom.contains("help") && !bottom.contains("No trade post"), "{bottom}");
    // The same app with the switch on does show one.
    app.settings.hints = OnOff::On;
    tick(&mut app, t, 601, 625);
    assert!(app.hints.current.is_some());
    // Turning it off in the settings pane removes it on the next tick.
    press(&mut app, 'g');
    let hints_row = crate::settings::ROWS.iter().position(|r| r.name == "Hints").unwrap();
    for _ in 0..hints_row {
        key(&mut app, KeyCode::Down);
    }
    assert_eq!(crate::settings::ROWS[app.settings_pick].name, "Hints");
    key(&mut app, KeyCode::Right);
    assert_eq!(app.settings.hints, OnOff::Off);
    tick(&mut app, t, 626, 626);
    assert!(app.hints.current.is_none());
}

#[test]
fn an_aside_shows_in_the_large_layout_and_never_in_the_small_pane() {
    let mut w = World::new();
    w.econ.money = 100_000.0;
    // Four coal drills; a post, a solar panel and a pole so nothing is stuck.
    for i in 0..4 {
        w.place(0, 20 + i * 4, 20, Kind::Drill, Dir::E).unwrap();
        w.apply_setting(0, 20 + i * 4, 20, crate::sim::Setting::Mines(Item::Coal));
    }
    w.place(0, 60, 36, Kind::TradePost, Dir::E).unwrap();
    w.place(0, 60, 44, Kind::Solar, Dir::E).unwrap();
    w.place(0, 62, 44, Kind::Pole, Dir::E).unwrap();
    w.econ.sold[Item::CopperIngot.idx()] = 1;
    w.econ.earned_research = 1.0;
    let min = |m: u64| Duration::from_secs(m * 60);

    // Small pane: not at 8 minutes, not ever.
    let mut app = App::new(w);
    app.msg_at = Instant::now() - Duration::from_secs(60);
    app.cursor = (50, 50);
    let t = Instant::now();
    app.hints = crate::hints::Hints::new(t);
    render(&mut app, 64, 16);
    for m in [0, 8, 9, 30] {
        app.tick_hints(t + min(m));
        assert!(app.hints.current.is_none(), "small pane at {m} min");
        let bottom = last(&text(&render(&mut app, 64, 16)));
        assert!(!bottom.contains("Hollowmere"), "{bottom}");
    }
    // Large layout: the first aside at 31 minutes (it has not been used up), and in the bottom line.
    render(&mut app, 130, 40);
    app.tick_hints(t + min(31));
    let bottom = last(&text(&render(&mut app, 130, 40)));
    assert!(bottom.contains(ASIDE), "{bottom}");
    // The same aside seen from a small pane is hidden (and gone on the next tick).
    let bottom = last(&text(&render(&mut app, 64, 16)));
    assert!(!bottom.contains("Hollowmere"), "{bottom}");
    app.tick_hints(t + min(31) + Duration::from_secs(1));
    assert!(app.hints.current.is_none());
}

#[test]
fn the_hint_does_not_cover_the_build_menu_or_the_corner_note() {
    let (mut app, _) = with_hint(64, 16);
    // The small pane's build menu is the player's: it keeps the line.
    press(&mut app, '1');
    let bottom = last(&text(&render(&mut app, 64, 16)));
    assert!(bottom.contains("1DR") || bottom.contains("1 "), "{bottom}");
    assert!(!bottom.contains("No trade post"), "{bottom}");
    key(&mut app, KeyCode::Esc);
    // A waiting telegram's note stays at the right end beside the hint.
    app.mail = Some(crate::mail::Message { from: "api".into(), event: "finished".into(), md: "Done.".into(), at: 0, gift: None, id: 9 });
    let bottom = last(&text(&render(&mut app, 64, 16)));
    assert!(bottom.contains("No trade") && bottom.trim_end().ends_with("n to view"), "{bottom}");
}
