//! The blueprint library (`l`): opening it, saving the hand under a name, taking a blueprint,
//! deleting after a confirm, files that can't be used, and how it draws at every size.

use super::*;
use crate::app::Library;
use crate::library::{self, Content, Entry};
use crate::sim::{Blueprint, Dir};
use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::path::{Path, PathBuf};

fn press(app: &mut App, c: char) {
    app.on_key(KeyEvent::from(KeyCode::Char(c)));
}

fn press_code(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::from(code));
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, c);
    }
}

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("factory-ui-library-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A blank unlocked world with a data folder, a settled camera and a small layout copied into the hand.
fn app_with_hand(data: &Path) -> App {
    let mut app = App::new(World::blank());
    app.data_dir = Some(data.to_path_buf());
    render(&mut app, 130, 42);
    let (x, y) = app.cursor;
    app.world.place(0, x, y, Kind::Furnace, Dir::E).unwrap();
    for i in 0..3 {
        app.world.place(0, x + 4 + i, y, Kind::Belt, Dir::E).unwrap();
    }
    press(&mut app, 'c');
    press(&mut app, ' ');
    app.cursor = (x + 7, y + 2);
    press(&mut app, ' ');
    press(&mut app, 'c');
    assert!(app.held.is_some());
    app.cursor = (x, y);
    app
}

fn names(app: &App) -> Vec<String> {
    app.library.as_ref().map(|l| l.entries.iter().map(|e| e.name.clone()).collect()).unwrap_or_default()
}

/// Saves the hand as `name` through the keys.
fn save_as(app: &mut App, name: &str) {
    if app.library.is_none() {
        press(app, 'l');
    }
    press(app, 'f');
    for _ in 0..40 {
        press_code(app, KeyCode::Backspace);
    }
    type_text(app, name);
    press_code(app, KeyCode::Enter);
}

#[test]
fn l_is_locked_without_blueprints_and_needs_the_data_folder() {
    let mut app = App::new(World::new());
    press(&mut app, 'l');
    assert!(app.library.is_none() && app.msg.contains("Blueprints are locked: research them first (t)"), "{}", app.msg);
    let mut app = App::new(World::blank());
    press(&mut app, 'l');
    assert!(app.library.is_none() && app.msg.contains("data folder"), "{}", app.msg);
}

#[test]
fn the_library_opens_lists_and_closes_without_pausing_the_game() {
    let data = temp("open");
    let mut app = app_with_hand(&data);
    press(&mut app, 'l');
    assert!(app.library.is_some());
    assert!(names(&app).is_empty());
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("Library · saved in ~/.terminactory/blueprints"), "{t}");
    assert!(t.contains("In hand: Blueprint · 7x3 · 4 parts · f saves it"), "{t}");
    assert!(t.contains("No blueprints saved yet") && t.contains("Esc close"), "{t}");
    // The game keeps running: nothing paused by it, and the pause key still works inside it.
    assert!(!app.paused);
    let before = app.world.tick;
    app.world.tick();
    assert_eq!(app.world.tick, before + 1);
    press(&mut app, 'p');
    assert!(app.paused && app.library.is_some());
    press(&mut app, 'p');
    // l and Esc close it.
    press(&mut app, 'l');
    assert!(app.library.is_none());
    press(&mut app, 'l');
    press_code(&mut app, KeyCode::Esc);
    assert!(app.library.is_none() && app.held.is_some(), "closing leaves the hand alone");
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn f_saves_the_hand_under_a_typed_name_and_the_file_is_there() {
    let data = temp("save");
    let mut app = app_with_hand(&data);
    press(&mut app, 'l');
    press(&mut app, 'f');
    // The name is prefilled; typing replaces what you delete.
    assert_eq!(app.library.as_ref().unwrap().typing.as_deref(), Some("Blueprint 1"));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("Name: Blueprint 1█") && t.contains("Esc cancels"), "{t}");
    // While typing, action keys are just letters (l, f, x and c do not close or save anything).
    for _ in 0..11 {
        press_code(&mut app, KeyCode::Backspace);
    }
    type_text(&mut app, "Smelter: l x c");
    assert_eq!(app.library.as_ref().unwrap().typing.as_deref(), Some("Smelter: l x c"));
    press_code(&mut app, KeyCode::Esc);
    assert!(app.library.as_ref().unwrap().typing.is_none() && app.library.is_some(), "Esc stops typing, not the library");
    assert!(names(&app).is_empty() && library::list(&data).is_empty(), "nothing saved");
    press(&mut app, 'f');
    for _ in 0..12 {
        press_code(&mut app, KeyCode::Backspace);
    }
    type_text(&mut app, "Smelter row");
    press_code(&mut app, KeyCode::Enter);
    assert!(app.msg.starts_with("Saved Smelter row to the library"), "{}", app.msg);
    let file = data.join("blueprints").join("smelter-row.json");
    assert!(file.exists());
    assert_eq!(names(&app), ["Smelter row"]);
    // The file holds a format number and the layout.
    let text_in_file = std::fs::read_to_string(&file).unwrap();
    assert!(text_in_file.contains("\"format\": 1") && text_in_file.contains("\"kind\": \"Furnace\""), "{text_in_file}");
    assert_eq!(app.held.as_ref().map(|b| b.name.as_str()), Some("Smelter row"), "the hand carries the name now");
    // An empty name saves nothing.
    press(&mut app, 'f');
    for _ in 0..30 {
        press_code(&mut app, KeyCode::Backspace);
    }
    press_code(&mut app, KeyCode::Enter);
    assert_eq!(library::list(&data).len(), 1);
    // With the hand empty, f saves the last copy.
    press_code(&mut app, KeyCode::Esc);
    press_code(&mut app, KeyCode::Esc);
    assert!(app.held.is_none() && app.last_copy.is_some());
    press(&mut app, 'l');
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("Last copy:") && t.contains("Smelter row"), "{t}");
    press(&mut app, 'f');
    assert_eq!(app.library.as_ref().unwrap().typing.as_deref(), Some("Blueprint 2"));
    for _ in 0..11 {
        press_code(&mut app, KeyCode::Backspace);
    }
    type_text(&mut app, "Smelter row two");
    press_code(&mut app, KeyCode::Enter);
    assert_eq!(names(&app), ["Smelter row", "Smelter row two"]);
    assert_eq!(app.library.as_ref().unwrap().pick, 1, "the new one is picked");
    // With nothing at all, there is nothing to save.
    let mut empty = App::new(World::blank());
    empty.data_dir = Some(data.clone());
    press(&mut empty, 'l');
    press(&mut empty, 'f');
    assert!(empty.library.as_ref().unwrap().typing.is_none() && empty.msg.contains("Nothing to save"), "{}", empty.msg);
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn enter_takes_the_picked_blueprint_into_the_hand_in_any_save() {
    let data = temp("take");
    let mut first = app_with_hand(&data);
    let original = first.held.clone().unwrap();
    save_as(&mut first, "Row");
    // A different save (another world) sees the same file: libraries belong to the player, not to a save.
    let mut other = App::new(World::blank());
    other.data_dir = Some(data.clone());
    render(&mut other, 130, 42);
    other.selected = Some(Kind::Belt);
    press(&mut other, 'l');
    assert_eq!(names(&other), ["Row"]);
    press_code(&mut other, KeyCode::Enter);
    assert!(other.library.is_none());
    let got = other.held.clone().unwrap();
    assert_eq!((got.parts.clone(), got.w, got.h, got.name.as_str()), (original.parts.clone(), original.w, original.h, "Row"));
    assert_eq!(other.selected, None, "the blueprint empties a picked part");
    assert!(other.msg.starts_with("Took Row (7x3, 4 parts, "), "{}", other.msg);
    // It pastes like any other.
    other.cursor = (other.cursor.0, other.cursor.1 + 10);
    press_code(&mut other, KeyCode::Char(' '));
    assert!(other.msg.starts_with("Pasted 4 parts"), "{}", other.msg);
    // A kind this save has not unlocked is named in the message.
    let mut fresh = App::new(World::new());
    fresh.world.progress.researched[Tech::Blueprints.idx()] = true;
    fresh.data_dir = Some(data.clone());
    let mut kilns = World::blank();
    kilns.place(0, 10, 10, Kind::Kiln, Dir::E).unwrap();
    let mut hand = kilns.capture(0, (0, 0, 40, 40)).bp;
    hand.name = "Kilns".into();
    library::save(&data, &hand).unwrap();
    press(&mut fresh, 'l');
    let row = text(&render(&mut fresh, 130, 42));
    assert!(row.contains("Kilns") && row.contains("(Kiln locked)"), "{row}");
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn delete_asks_twice_and_any_other_key_changes_its_mind() {
    let data = temp("delete");
    let mut app = app_with_hand(&data);
    save_as(&mut app, "Alpha");
    save_as(&mut app, "Beta");
    assert_eq!(names(&app).len(), 2, "both saved: {:?}", names(&app));
    assert_eq!(app.library.as_ref().unwrap().pick, 1);
    press(&mut app, 'x');
    assert!(app.library.as_ref().unwrap().armed && data.join("blueprints/beta.json").exists());
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("x again deletes Beta for good"), "{t}");
    // Another key disarms.
    press(&mut app, 'w');
    assert!(!app.library.as_ref().unwrap().armed);
    press(&mut app, 's');
    press(&mut app, 'x');
    press_code(&mut app, KeyCode::Delete);
    assert!(!data.join("blueprints/beta.json").exists(), "the second press deletes");
    assert_eq!(names(&app), ["Alpha"]);
    assert!(app.msg.starts_with("Deleted Beta"), "{}", app.msg);
    assert!(data.join("blueprints/alpha.json").exists());
    // The pick moved to what is left; deleting the last one leaves an empty list.
    press_code(&mut app, KeyCode::Backspace);
    press_code(&mut app, KeyCode::Backspace);
    assert!(names(&app).is_empty() && library::list(&data).is_empty());
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn bad_files_are_listed_say_why_and_are_never_deleted_or_crash_the_screen() {
    let data = temp("bad");
    let mut app = app_with_hand(&data);
    save_as(&mut app, "Good");
    let folder = data.join("blueprints");
    std::fs::write(folder.join("junk.json"), "not json at all").unwrap();
    std::fs::write(folder.join("future.json"), r#"{"format": 9, "name": "From tomorrow", "parts": []}"#).unwrap();
    let ok = std::fs::read_to_string(folder.join("good.json")).unwrap();
    std::fs::write(folder.join("alien.json"), ok.replace("Furnace", "Teleporter")).unwrap();
    press_code(&mut app, KeyCode::Esc);
    press(&mut app, 'l');
    // Sorted by name; the alien file still says "Good" inside.
    assert_eq!(names(&app), ["From tomorrow", "Good", "Good", "junk"]);
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("(can't be read)") && t.contains("(from a newer game)") && t.contains("From tomorrow"), "{t}");
    let hand = app.held.clone();
    for (i, e) in app.library.as_ref().unwrap().entries.clone().iter().enumerate() {
        if matches!(e.content, Content::Ok(_)) {
            continue;
        }
        // Enter says why; the library stays open and the hand is untouched.
        app.library.as_mut().unwrap().pick = i;
        press_code(&mut app, KeyCode::Enter);
        assert!(app.library.is_some() && app.held == hand);
        match &e.content {
            Content::Newer(f) => assert!(app.msg.contains("newer game") && app.msg.contains(&format!("format {f}")), "{}", app.msg),
            Content::Broken(why) => assert!(app.msg.starts_with("Can't read") && app.msg.contains(why.as_str()), "{}", app.msg),
            Content::Ok(_) => unreachable!(),
        }
        // Delete refuses to touch it, however many times it is pressed.
        press(&mut app, 'x');
        press(&mut app, 'x');
        assert!(e.path.exists(), "{:?} is still there", e.path);
        assert!(app.msg.contains("left alone"), "{}", app.msg);
    }
    for f in ["junk.json", "future.json", "alien.json", "good.json"] {
        assert!(folder.join(f).exists(), "{f}");
    }
    assert_eq!(std::fs::read_to_string(folder.join("junk.json")).unwrap(), "not json at all");
    // The one good file still takes.
    let good = app.library.as_ref().unwrap().entries.iter().position(|e| e.path.ends_with("good.json")).unwrap();
    app.library.as_mut().unwrap().pick = good;
    press_code(&mut app, KeyCode::Enter);
    assert!(app.library.is_none() && app.held.is_some());
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn clicking_picks_a_row_and_clicking_it_again_takes_it_and_the_wheel_scrolls() {
    let data = temp("mouse");
    let mut app = app_with_hand(&data);
    save_as(&mut app, "Alpha");
    save_as(&mut app, "Beta");
    save_as(&mut app, "Gamma");
    assert_eq!(names(&app), ["Alpha", "Beta", "Gamma"]);
    render(&mut app, 130, 42);
    let rows = app.library.as_ref().unwrap().rows.clone();
    assert_eq!(rows.len(), 3);
    let click = |app: &mut App, r: ratatui::layout::Rect| {
        app.on_mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x + 3, row: r.y, modifiers: KeyModifiers::NONE })
    };
    click(&mut app, rows[0].0);
    assert_eq!((app.library.as_ref().unwrap().pick, app.library.is_some()), (0, true));
    let wheel =
        |app: &mut App, kind| app.on_mouse(MouseEvent { kind, column: rows[0].0.x, row: rows[0].0.y, modifiers: KeyModifiers::NONE });
    wheel(&mut app, MouseEventKind::ScrollDown);
    assert_eq!(app.library.as_ref().unwrap().pick, 1);
    wheel(&mut app, MouseEventKind::ScrollUp);
    wheel(&mut app, MouseEventKind::ScrollUp);
    assert_eq!(app.library.as_ref().unwrap().pick, 0);
    click(&mut app, rows[2].0);
    assert_eq!(app.library.as_ref().unwrap().pick, 2);
    click(&mut app, rows[2].0);
    assert!(app.library.is_none() && app.held.as_ref().map(|b| b.name.as_str()) == Some("Gamma"));
    let _ = std::fs::remove_dir_all(&data);
}

/// Library states for the size test.
fn states(data: &Path) -> Vec<App> {
    let mut out = Vec::new();
    for state in 0..5 {
        let mut app = if state == 4 { App::new(World::new()) } else { app_with_hand(data) };
        app.world.progress.researched[Tech::Blueprints.idx()] = true;
        app.data_dir = Some(data.to_path_buf());
        let mut entries = Vec::new();
        if state != 3 {
            let bp = app.held.clone().unwrap_or_else(|| Blueprint::from_parts("x", 0, vec![]));
            let mut fine = bp.clone();
            fine.name = "Copper smelter row with a very long name indeed".into();
            let mut kiln = bp.clone();
            kiln.name = "Big kiln hall".into();
            if let Some(p) = kiln.parts.first_mut() {
                p.kind = Kind::Kiln;
            }
            entries.push(Entry { path: "a.json".into(), name: fine.name.clone(), content: Content::Ok(fine) });
            entries.push(Entry { path: "b.json".into(), name: kiln.name.clone(), content: Content::Ok(kiln) });
            entries.push(Entry { path: "c.json".into(), name: "junk".into(), content: Content::Broken("not JSON".into()) });
            entries.push(Entry { path: "d.json".into(), name: "From tomorrow".into(), content: Content::Newer(5) });
            for i in 0..30 {
                let mut b = bp.clone();
                b.name = format!("Extra blueprint number {i}");
                entries.push(Entry { path: format!("e{i}.json").into(), name: b.name.clone(), content: Content::Ok(b) });
            }
        }
        let mut lib = Library::new(entries);
        lib.pick = if state == 1 { 20 } else { 1 };
        lib.typing = (state == 2).then(|| "Typing a rather long blueprint name here".to_string());
        lib.armed = state == 1;
        app.library = Some(lib);
        out.push(app);
    }
    out
}

#[test]
fn the_library_fits_every_pane_size() {
    let data = temp("sizes");
    for (w, h) in [(30, 12), (36, 24), (44, 30), (44, 60), (50, 20), (64, 12), (64, 16), (80, 24), (99, 29), (130, 42), (220, 60)] {
        for (i, mut app) in states(&data).into_iter().enumerate() {
            let buf = render(&mut app, w, h);
            let t = text(&buf);
            assert_eq!(t.lines().count(), h as usize);
            assert!(t.lines().all(|l| l.chars().count() <= w as usize), "state {i} at {w}x{h}");
            let inner_w = app.map_area.width;
            if inner_w >= 14 && app.map_area.height >= 4 {
                assert!(t.contains("Library"), "state {i} at {w}x{h}: {t}");
                // The way out always shows in the foot of the list.
                assert!(matches!(i, 1 | 2) || t.contains("Esc"), "state {i} at {w}x{h}: {t}");
                if i == 2 {
                    assert!(t.contains("Name:"), "state {i} at {w}x{h}: {t}");
                }
                if i == 0 {
                    assert!(t.contains("Copper smelter") || inner_w < 24, "{w}x{h}: {t}");
                }
            }
            if w == 130 && h == 42 {
                std::fs::write(format!("target/snapshot-library-{i}.html"), to_html(&buf)).unwrap();
            }
        }
    }
    // The rows keep the name and drop columns from the right as the pane narrows.
    let mut app = states(&data).remove(0);
    let wide = text(&render(&mut app, 130, 42));
    assert!(wide.contains("parts") && wide.contains("7x3") && wide.contains('$'), "{wide}");
    assert!(wide.contains("(Kiln locked)") || wide.contains("Big kiln hall"), "{wide}");
    assert!(wide.contains("(can't be read)") && wide.contains("(from a newer game)"), "{wide}");
    let narrow = text(&render(&mut app, 50, 20));
    assert!(!narrow.contains("$") || narrow.contains("part"), "{narrow}");
    let _ = std::fs::remove_dir_all(&data);
}
