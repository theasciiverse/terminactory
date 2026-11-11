//! What the small-pane, telegram and hint settings change on screen.

use super::*;
use crate::settings::{Cols, SmallView, ROWS};

fn key(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::from(code));
}

fn press(app: &mut App, c: char) {
    key(app, KeyCode::Char(c));
}

fn lines(t: &str) -> Vec<&str> {
    t.lines().collect()
}

fn app() -> App {
    let mut w = World::new();
    w.build_demo().unwrap();
    let mut app = App::new(w);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    app
}

#[test]
fn the_new_rows_show_with_their_hints() {
    let mut app = app();
    press(&mut app, 'g');
    let t = text(&render(&mut app, 120, 40));
    for want in ["Small-pane view", "‹ Auto ›", "Small-pane width", "‹ 120 ›", "Telegram style", "‹ Sidebar ›", "Hints", "‹ On ›"]
    {
        assert!(t.contains(want), "{want}: {t}");
    }
    // The picked row's hint shows under the list; the width row's says it is for Auto only.
    for _ in 0..3 {
        key(&mut app, KeyCode::Down);
    }
    let t = text(&render(&mut app, 120, 40));
    assert!(t.contains("(Auto only)"), "{t}");
}

#[test]
fn the_width_row_greys_out_when_the_view_is_not_auto() {
    let mut app = app();
    press(&mut app, 'g');
    let colour_of = |app: &mut App, name: &str| {
        let buf = render(app, 120, 40);
        let t = text(&buf);
        let (y, line) = t.lines().enumerate().find(|(_, l)| l.contains(name)).unwrap();
        let x = line.chars().position(|c| c == name.chars().next().unwrap()).unwrap();
        buf.cell((x as u16, y as u16)).unwrap().fg
    };
    let live = colour_of(&mut app, "Small-pane width");
    assert_ne!(live, Color::DarkGray);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Right); // Always
    assert_eq!(app.settings.small_view, SmallView::Always);
    assert_eq!(colour_of(&mut app, "Small-pane width"), Color::DarkGray);
    key(&mut app, KeyCode::Left); // back to Auto
    assert_ne!(colour_of(&mut app, "Small-pane width"), Color::DarkGray);
}

#[test]
fn a_short_pane_scrolls_the_rows_to_keep_the_pick_in_view() {
    let mut app = app();
    press(&mut app, 'g');
    // 64x7 leaves the pane room for only a few rows.
    for pick in 0..ROWS.len() {
        app.settings_pick = pick;
        let t = text(&render(&mut app, 64, 7));
        assert!(t.contains(ROWS[pick].name), "row {pick}: {t}");
    }
}

#[test]
fn small_pane_view_auto_always_never_at_a_few_sizes() {
    // (width, height, Auto, Always, Never): whether each view setting gives the small pane.
    let cases = [
        (36, 24, true, true, true),   // too small to draw the full view, whatever the setting
        (100, 40, true, true, false), // under 120 columns
        (120, 30, false, true, false),
        (130, 29, true, true, false), // a short pane
        (200, 60, false, true, false),
    ];
    for (w, h, auto, always, never) in cases {
        for (view, want) in [(SmallView::Auto, auto), (SmallView::Always, always), (SmallView::Never, never)] {
            let mut app = app();
            app.settings.small_view = view;
            let t = text(&render(&mut app, w, h));
            assert_eq!(app.compact_now, want, "{view:?} at {w}x{h}");
            // The full view has the sidebar's build menu; the small pane has a ticker in the top line.
            assert_eq!(t.contains("BUILD"), !want, "{view:?} at {w}x{h}: {t}");
            assert!(lines(&t).iter().all(|l| l.chars().count() <= w as usize));
        }
    }
}

#[test]
fn small_pane_width_moves_the_auto_cut_off() {
    for (cols, w, small) in
        [(80, 90, false), (80, 79, true), (100, 99, true), (100, 100, false), (140, 139, true), (160, 150, true), (160, 160, false)]
    {
        let mut app = app();
        app.settings.small_width = Cols(cols);
        render(&mut app, w, 40);
        assert_eq!(app.compact_now, small, "cut-off {cols} at {w} columns");
    }
    // It has no effect when the view isn't Auto.
    let mut app = app();
    app.settings.small_view = SmallView::Never;
    app.settings.small_width = Cols(160);
    render(&mut app, 130, 40);
    assert!(!app.compact_now);
}

#[test]
fn m_still_toggles_for_the_session_whatever_the_setting() {
    for view in [SmallView::Auto, SmallView::Always, SmallView::Never] {
        let mut app = app();
        app.settings.small_view = view;
        render(&mut app, 130, 40);
        let before = app.compact_now;
        press(&mut app, 'm');
        render(&mut app, 130, 40);
        assert_eq!(app.compact_now, !before, "{view:?}");
        press(&mut app, 'm');
        render(&mut app, 130, 40);
        assert_eq!(app.compact_now, before, "{view:?}");
    }
}

#[test]
fn changing_the_view_in_the_pane_takes_effect_at_once() {
    let mut app = app();
    press(&mut app, 'g');
    render(&mut app, 130, 40);
    assert!(!app.compact_now);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Right); // Always
    let t = text(&render(&mut app, 130, 40));
    assert!(app.compact_now && t.contains("Settings"), "{t}");
    key(&mut app, KeyCode::Right); // Never
    render(&mut app, 130, 40);
    assert!(!app.compact_now);
}
