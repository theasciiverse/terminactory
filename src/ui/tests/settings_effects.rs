//! What the small-pane, telegram and hint settings change on screen.

use super::*;
use crate::settings::{Cols, SmallView, TelegramStyle, ROWS};

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
    for _ in 0..4 {
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
    for (pick, row) in ROWS.iter().enumerate() {
        app.settings_pick = pick;
        let t = text(&render(&mut app, 64, 7));
        assert!(t.contains(row.name), "row {pick}: {t}");
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
fn changing_the_view_in_the_pane_takes_effect_at_once() {
    let mut app = app();
    press(&mut app, 'g');
    render(&mut app, 130, 40);
    assert!(!app.compact_now);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Right); // Always
    let t = text(&render(&mut app, 130, 40));
    assert!(app.compact_now && t.contains("Settings"), "{t}");
    key(&mut app, KeyCode::Right); // Never
    render(&mut app, 130, 40);
    assert!(!app.compact_now);
}

fn telegram() -> crate::mail::Message {
    crate::mail::Message {
        from: "my-project".into(),
        event: "finished".into(),
        md: "Tests pass. Ready for review.".into(),
        at: 0,
        gift: None,
        id: 1,
    }
}

fn app_with_mail(style: TelegramStyle) -> App {
    let mut app = app();
    app.settings.telegram_style = style;
    app.mail = Some(telegram());
    app
}

fn last_line(t: &str) -> String {
    lines(t).last().unwrap().to_string()
}

#[test]
fn telegram_style_sidebar_is_today() {
    let mut app = app_with_mail(TelegramStyle::Sidebar);
    // Full view: a box in the sidebar, no note, and n dismisses it.
    let t = text(&render(&mut app, 130, 40));
    assert!(t.contains("From my-project") && t.contains("n dismiss"), "{t}");
    assert!(!last_line(&t).contains("to view"), "{t}");
    // Small pane: the corner note, and n opens it over the map.
    let t = text(&render(&mut app, 64, 16));
    assert!(last_line(&t).trim_end().ends_with("agent finished · n to view"), "{t}");
    assert!(!lines(&t)[0].contains("opens"), "{t}"); // the small ticker already counts mail; only Ticker adds the key
    press(&mut app, 'n');
    assert!(app.mail_open);
    // And in the full view n closes it.
    let mut app = app_with_mail(TelegramStyle::Sidebar);
    render(&mut app, 130, 40);
    press(&mut app, 'n');
    assert!(app.mail.is_none() && !app.mail_open);
}

#[test]
fn telegram_style_note_is_only_the_note_in_both_views() {
    for (w, h) in [(130, 40), (200, 60), (64, 16), (36, 24)] {
        let mut app = app_with_mail(TelegramStyle::Note);
        let t = text(&render(&mut app, w, h));
        let last = last_line(&t);
        assert!(last.contains("to view") || last.contains("mail"), "{w}x{h}: {last}");
        if w >= 130 {
            // The full view: no box in the sidebar, and the sidebar keeps its build menu.
            assert!(!t.contains("From my-project") && !t.contains("n dismiss") && t.contains("BUILD"), "{w}x{h}: {t}");
            assert!(last.trim_end().ends_with("agent finished · n to view"), "{w}x{h}: {last}");
        }
        // The top line has no count: that is the Ticker's.
        assert!(!lines(&t)[0].contains("opens"), "{w}x{h}: {t}");
        // n opens it over the map, even in the full view, and n or Esc closes it.
        press(&mut app, 'n');
        assert!(app.mail_open && app.mail.is_some(), "{w}x{h}");
        let t = text(&render(&mut app, w, h));
        assert!(t.contains("From my-project") && t.contains("Tests pass"), "{w}x{h}: {t}");
        key(&mut app, KeyCode::Esc);
        assert!(!app.mail_open && app.mail.is_none(), "{w}x{h}");
        let t = text(&render(&mut app, w, h));
        assert!(!last_line(&t).contains("to view"), "{w}x{h}: {t}");
    }
}

#[test]
fn telegram_style_ticker_is_only_the_count_in_the_top_line() {
    for (w, h) in [(130, 40), (200, 60), (64, 16), (36, 24)] {
        let mut app = app_with_mail(TelegramStyle::Ticker);
        app.world.mail.push_back(telegram());
        let t = text(&render(&mut app, w, h));
        let top = lines(&t)[0].to_string();
        assert!(top.contains("2 mail"), "{w}x{h}: {top}");
        if w >= 64 {
            assert!(top.contains("n opens"), "{w}x{h}: {top}");
        }
        // No box and no note anywhere.
        assert!(!t.contains("From my-project") && !t.contains("to view") && !t.contains("n dismiss"), "{w}x{h}: {t}");
        assert!(lines(&t).iter().all(|l| l.chars().count() <= w as usize));
        press(&mut app, 'n');
        assert!(app.mail_open, "{w}x{h}");
        let t = text(&render(&mut app, w, h));
        assert!(t.contains("From my-project"), "{w}x{h}: {t}");
        key(&mut app, KeyCode::Esc);
        assert!(!app.mail_open, "{w}x{h}");
    }
    // With nothing waiting there is no count.
    let mut app = app();
    app.settings.telegram_style = TelegramStyle::Ticker;
    let t = text(&render(&mut app, 130, 40));
    assert!(!lines(&t)[0].contains("mail"), "{t}");
}

#[test]
fn the_log_and_the_saved_log_do_not_depend_on_the_style() {
    for style in [TelegramStyle::Sidebar, TelegramStyle::Note, TelegramStyle::Ticker] {
        let mut app = app();
        app.settings.telegram_style = style;
        app.world.receive(crate::mail::parse("from: api\nevent: finished\n# Done\n\nAll tests pass.", 0), 1_000);
        app.mail = app.world.mail.pop_front();
        press(&mut app, 'i');
        let t = text(&render(&mut app, 130, 40));
        assert!(app.log_open && t.contains("api"), "{style:?}: {t}");
        key(&mut app, KeyCode::Esc);
        // Reading it marks it read, in every style.
        press(&mut app, 'n');
        if app.mail_open {
            press(&mut app, 'n');
        }
        assert_eq!(app.world.unread(), 0, "{style:?}");
        assert_eq!(app.world.log.len(), 1, "{style:?}");
    }
}
