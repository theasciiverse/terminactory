//! The settings pane and the sidebar graph it controls.

use super::*;
use crate::settings::{GraphRange, GraphShow};

fn key(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::from(code));
}

fn press(app: &mut App, c: char) {
    key(app, KeyCode::Char(c));
}

fn lines(t: &str) -> Vec<&str> {
    t.lines().collect()
}

fn snapshot(name: &str, buf: &Buffer) {
    std::fs::write(format!("target/snapshot-{name}.html"), to_html(buf)).unwrap();
}

fn app() -> App {
    let mut w = World::new();
    w.build_demo().unwrap();
    let mut app = App::new(w);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    app
}

#[test]
fn g_opens_and_closes_the_pane_and_nothing_pauses() {
    let mut app = app();
    press(&mut app, 'g');
    assert!(app.settings_open && !app.paused);
    let t = text(&render(&mut app, 120, 30));
    assert!(t.contains(" Settings ") && t.contains("Sidebar graph") && t.contains("‹ Money ›"), "{t}");
    assert!(t.contains("Graph range") && t.contains("‹ Minutes ›") && t.contains("g/Esc close"), "{t}");
    // The sidebar stays in view beside it.
    assert!(t.contains("BUILD"), "{t}");
    press(&mut app, 'g');
    assert!(!app.settings_open);
    press(&mut app, 'g');
    key(&mut app, KeyCode::Esc);
    assert!(!app.settings_open);
    // The game keeps its pause and speed keys while the pane is open.
    press(&mut app, 'g');
    press(&mut app, 'p');
    assert!(app.paused && app.settings_open);
    press(&mut app, ']');
    assert_eq!(app.speed, 2);
}

#[test]
fn keys_move_between_rows_and_change_values() {
    let mut app = app();
    press(&mut app, 'g');
    assert_eq!(app.settings_pick, 0);
    // Enter and Right go on, Left goes back, and both wrap.
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.settings.sidebar_graph, GraphShow::Research);
    key(&mut app, KeyCode::Right);
    assert_eq!(app.settings.sidebar_graph, GraphShow::Both);
    key(&mut app, KeyCode::Right);
    assert_eq!(app.settings.sidebar_graph, GraphShow::Off);
    key(&mut app, KeyCode::Left);
    assert_eq!(app.settings.sidebar_graph, GraphShow::Both);
    // Down and j pick the next row; up and k the one before (wrapping).
    key(&mut app, KeyCode::Down);
    assert_eq!(app.settings_pick, 1);
    key(&mut app, KeyCode::Right);
    assert_eq!(app.settings.sidebar_graph_range, GraphRange::Hours);
    press(&mut app, 'j');
    assert_eq!(app.settings_pick, 0);
    press(&mut app, 'k');
    assert_eq!(app.settings_pick, 1);
    key(&mut app, KeyCode::Up);
    assert_eq!(app.settings_pick, 0);
    // The map isn't moved by the arrows meanwhile.
    let cursor = app.cursor;
    key(&mut app, KeyCode::Down);
    assert_eq!(app.cursor, cursor);
    // Other keys do nothing (and don't close it).
    press(&mut app, 'z');
    assert!(app.settings_open);
}

#[test]
fn changes_are_saved_when_there_is_a_file() {
    let path = std::env::temp_dir().join(format!("terminactory-settings-pane-{}.json", std::process::id()));
    std::fs::remove_file(&path).ok();
    let mut app = app();
    app.settings_path = Some(path.clone());
    press(&mut app, 'g');
    key(&mut app, KeyCode::Right);
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert!(text.contains("\"research\""), "{text}");
}

#[test]
fn the_pane_fits_small_panes() {
    for (w, h) in [(36, 24), (44, 60), (64, 12), (80, 24), (120, 30)] {
        let mut app = app();
        press(&mut app, 'g');
        let buf = render(&mut app, w, h);
        if (w, h) == (120, 30) {
            snapshot("settings", &buf);
        }
        let t = text(&buf);
        assert!(t.contains("Settings") && t.contains("Esc close") && t.contains("Graph range"), "{w}x{h}: {t}");
        assert!(lines(&t).iter().all(|l| l.chars().count() <= w as usize));
    }
}
