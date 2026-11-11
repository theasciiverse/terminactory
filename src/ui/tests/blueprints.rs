//! Blueprints in the hand: copying an area, the ghost, turning, pasting (keys and mouse), the research
//! gate, and how the hand looks at every pane size.

use super::*;
use crate::app::{Area, AreaStage};
use crate::sim::{Blueprint, Building, Dir, Item, PlanState, St};
use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

fn press(app: &mut App, c: char) {
    app.on_key(KeyEvent::from(KeyCode::Char(c)));
}

fn press_code(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::from(code));
}

/// A blank, fully unlocked world with the camera settled.
fn app() -> App {
    let mut app = App::new(World::blank());
    render(&mut app, 130, 42);
    app
}

fn at(app: &App, t: (i32, i32)) -> (u16, u16) {
    (app.map_area.x + ((t.0 - app.cam.0) * 2) as u16, app.map_area.y + (t.1 - app.cam.1) as u16)
}

fn mouse(app: &mut App, kind: MouseEventKind, t: (i32, i32)) {
    let (column, row) = at(app, t);
    app.on_mouse(MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE });
}

/// A small layout near the cursor: a furnace, a press facing north and three belts, with the
/// cursor on its top-left corner.
fn layout(app: &mut App) -> (i32, i32) {
    let (x, y) = app.cursor;
    let w = &mut app.world;
    w.place(0, x, y, Kind::Furnace, Dir::E).unwrap();
    w.place(0, x + 4, y, Kind::Press, Dir::N).unwrap();
    for i in 0..3 {
        w.place(0, x + i, y + 4, Kind::Belt, Dir::E).unwrap();
    }
    (x, y)
}

/// c, then the two corners with the cursor, then c again: the layout is in the hand.
fn copy_all(app: &mut App, (x, y): (i32, i32)) {
    app.cursor = (x, y);
    press(app, 'c');
    press(app, ' ');
    app.cursor = (x + 5, y + 4);
    press(app, ' ');
    press(app, 'c');
}

#[test]
fn c_space_move_space_then_c_copies_the_area_into_the_hand() {
    let mut app = app();
    let (x, y) = layout(&mut app);
    copy_all(&mut app, (x, y));
    let bp = app.held.clone().expect("the layout is in the hand");
    assert_eq!((bp.part_count(), bp.w, bp.h), (5, 5, 5));
    assert!(app.area.is_none() && app.selected.is_none() && app.menu.is_none());
    assert_eq!(app.last_copy.as_ref(), Some(&bp));
    assert!(app.msg.starts_with("Copied 5 parts (5x5, $365)"), "{}", app.msg);
    assert!(app.msg.contains("Space pastes") && app.msg.contains("r turns"), "{}", app.msg);
    // Space and Enter copy a chosen rectangle too.
    press_code(&mut app, KeyCode::Esc);
    assert!(app.held.is_none(), "Esc empties the hand");
    for finish in [KeyCode::Char(' '), KeyCode::Enter] {
        app.cursor = (x, y);
        press(&mut app, 'c');
        press(&mut app, ' ');
        app.cursor = (x + 5, y + 4);
        press(&mut app, ' ');
        press_code(&mut app, finish);
        assert_eq!(app.held.as_ref().map(|b| b.part_count()), Some(5), "{finish:?}");
        press_code(&mut app, KeyCode::Esc);
    }
}

#[test]
fn copying_is_locked_until_blueprints_are_researched_and_says_what_unlocks_it() {
    let mut app = App::new(World::new());
    render(&mut app, 130, 42);
    let (x, y) = app.cursor;
    for i in 0..3 {
        app.world.place(0, x + i, y, Kind::Belt, Dir::E).unwrap();
    }
    press(&mut app, 'c');
    press(&mut app, ' ');
    app.cursor = (x + 2, y);
    press(&mut app, ' ');
    for key in ['c', ' '] {
        press(&mut app, key);
        assert!(app.held.is_none() && app.last_copy.is_none(), "{key:?}");
        assert!(
            app.msg.contains("Blueprints are locked: research them first (t)")
                && app.msg.contains("800 RP")
                && app.msg.contains("1,500 copper plate"),
            "{}",
            app.msg
        );
        // The rectangle stays chosen.
        assert!(matches!(app.area.map(|a| a.stage), Some(AreaStage::Chosen { .. })), "{key:?}");
    }
    // The area tool itself is free: x still removes it.
    press(&mut app, 'x');
    assert!(app.world.tile(0, x, y).is_none());
    // Research it and copying works.
    app.world.econ.research = 800.0;
    app.world.research(Tech::Blueprints).unwrap();
    app.world.place(0, x, y, Kind::Belt, Dir::E).unwrap();
    app.cursor = (x, y);
    press(&mut app, 'c');
    press(&mut app, ' ');
    press(&mut app, ' ');
    press(&mut app, 'c');
    assert_eq!(app.held.as_ref().map(|b| b.part_count()), Some(1));
    // Middle-drag copy is gated the same way.
    let mut locked = App::new(World::new());
    render(&mut locked, 130, 42);
    let t = locked.cursor;
    locked.world.place(0, t.0, t.1, Kind::Belt, Dir::E).unwrap();
    mouse(&mut locked, MouseEventKind::Down(MouseButton::Middle), t);
    mouse(&mut locked, MouseEventKind::Drag(MouseButton::Middle), (t.0 + 3, t.1 + 2));
    mouse(&mut locked, MouseEventKind::Up(MouseButton::Middle), (t.0 + 3, t.1 + 2));
    assert!(locked.held.is_none() && locked.area.is_none());
    assert!(locked.msg.contains("Blueprints are locked"), "{}", locked.msg);
    assert_eq!(locked.selected, Some(Kind::Belt), "the plain middle-click copy of one part still happened");
}

#[test]
fn r_turns_the_hand_and_shift_r_turns_it_back_and_space_pastes_the_turned_layout() {
    let mut app = app();
    let (x, y) = layout(&mut app);
    copy_all(&mut app, (x, y));
    let bp = app.held.clone().unwrap();
    press(&mut app, 'r');
    assert_eq!(app.held.as_ref(), Some(&bp.rotated(1)));
    press(&mut app, 'r');
    assert_eq!(app.held.as_ref(), Some(&bp.rotated(2)));
    app.on_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT));
    assert_eq!(app.held.as_ref(), Some(&bp.rotated(1)));
    app.on_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT));
    app.on_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT));
    assert_eq!(app.held.as_ref(), Some(&bp.rotated(3)), "counter-clockwise from the start is three clockwise turns");
    // All four turns of r are the start again.
    for _ in 0..1 {
        press(&mut app, 'r');
    }
    assert_eq!(app.held.as_ref(), Some(&bp));
    // The first Space after turning pastes the turned layout, its corner under the cursor.
    press(&mut app, 'r');
    app.cursor = (x + 20, y + 20);
    press_code(&mut app, KeyCode::Char(' '));
    let turned = bp.rotated(1);
    assert!(app.msg.starts_with("Pasted 5 parts for "), "{}", app.msg);
    let (cx, cy) = app.cursor;
    let got = app.world.capture(0, (cx, cy, cx + turned.w - 1, cy + turned.h - 1)).bp;
    assert_eq!(got.parts, turned.parts);
    // The hand still holds it, for the next one.
    assert!(app.held.is_some());
}

#[test]
fn the_hand_and_a_picked_part_exclude_each_other_and_esc_and_middle_click_empty_the_hand() {
    let mut app = app();
    let (x, y) = layout(&mut app);
    copy_all(&mut app, (x, y));
    // Picking a part empties the hand...
    press(&mut app, '2');
    assert!(app.held.is_none(), "opening a build menu puts the blueprint away");
    app.cursor = (x, y);
    copy_all(&mut app, (x, y));
    assert!(app.held.is_some() && app.selected.is_none());
    // ...and copying a part with a middle-click does too.
    mouse(&mut app, MouseEventKind::Down(MouseButton::Middle), (x + 1, y + 4));
    mouse(&mut app, MouseEventKind::Up(MouseButton::Middle), (x + 1, y + 4));
    assert_eq!((app.held.is_none(), app.selected), (true, Some(Kind::Belt)));
    // Copying a blueprint empties a picked part.
    copy_all(&mut app, (x, y));
    assert_eq!(app.selected, None);
    // A middle-click on empty ground empties the hand.
    let empty = (x + 12, y + 8);
    mouse(&mut app, MouseEventKind::Down(MouseButton::Middle), empty);
    mouse(&mut app, MouseEventKind::Up(MouseButton::Middle), empty);
    assert!(app.held.is_none() && app.msg == "Hand empty", "{}", app.msg);
    // Esc (and the backtick) empty it as well; the area tool starts with an empty hand.
    copy_all(&mut app, (x, y));
    press(&mut app, '`');
    assert!(app.held.is_none());
    copy_all(&mut app, (x, y));
    press(&mut app, 'c');
    assert!(app.held.is_none() && app.area.is_some());
    // Changing floor keeps a blueprint in the hand.
    press_code(&mut app, KeyCode::Esc);
    copy_all(&mut app, (x, y));
    press(&mut app, '>');
    assert_eq!(app.floor, app.world.floor_of(1).unwrap());
    assert!(app.held.is_some());
    press(&mut app, '<');
}

#[test]
fn a_click_pastes_once_and_dragging_does_not_smear() {
    let mut app = app();
    let (x, y) = layout(&mut app);
    copy_all(&mut app, (x, y));
    let spot = (x + 8, y + 8);
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), spot);
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), (spot.0 + 2, spot.1));
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), (spot.0 + 4, spot.1 + 1));
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), (spot.0 + 4, spot.1 + 1));
    assert!(app.world.tile(0, spot.0, spot.1).is_some(), "the click pasted at the cursor's corner");
    let count = app.world.floors[0].tiles.iter().flatten().count();
    assert_eq!(count, 5 + 5, "one copy beside the original");
}

#[test]
fn middle_drag_copies_a_rectangle_straight_to_the_hand() {
    let mut app = app();
    let (x, y) = layout(&mut app);
    // A plain middle-click still copies one part.
    mouse(&mut app, MouseEventKind::Down(MouseButton::Middle), (x + 1, y + 4));
    mouse(&mut app, MouseEventKind::Up(MouseButton::Middle), (x + 1, y + 4));
    assert_eq!(app.selected, Some(Kind::Belt));
    assert!(app.held.is_none());
    // Dragging draws a rectangle (live) and lets go with the blueprint in hand.
    mouse(&mut app, MouseEventKind::Down(MouseButton::Middle), (x - 1, y - 1));
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Middle), (x + 3, y + 2));
    assert!(matches!(app.area, Some(Area { stage: AreaStage::Picking { corner: Some(_) }, .. })));
    assert_eq!(app.area_rect(), Some((x - 1, y - 1, x + 3, y + 2)));
    assert!(app.selected.is_none());
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Middle), (x + 6, y + 5));
    mouse(&mut app, MouseEventKind::Up(MouseButton::Middle), (x + 6, y + 5));
    assert!(app.area.is_none() && !app.quick_area);
    assert_eq!(app.held.as_ref().map(|b| b.part_count()), Some(5));
    assert_eq!(app.selected, None);
    // A rectangle with nothing in it leaves the hand empty and says so.
    let blank = (x + 12, y + 8);
    mouse(&mut app, MouseEventKind::Down(MouseButton::Middle), blank);
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Middle), (blank.0 + 3, blank.1 + 3));
    mouse(&mut app, MouseEventKind::Up(MouseButton::Middle), (blank.0 + 3, blank.1 + 3));
    assert!(app.held.is_none() && app.msg.contains("Nothing in that area"), "{}", app.msg);
}

/// A layout with a crate in the way of the second belt, and money for three belts only.
fn short_and_blocked() -> App {
    let mut app = app();
    let (x, y) = app.cursor;
    let mut scratch = World::blank();
    for i in 0..5 {
        scratch.place(0, 10 + i, 10, Kind::Belt, Dir::E).unwrap();
    }
    let mut bp: Blueprint = scratch.capture(0, (10, 10, 14, 10)).bp;
    bp.name = "Belts".into();
    app.held = Some(bp);
    app.world.place(0, x + 1, y, Kind::Crate, Dir::E).unwrap();
    app.world.econ.money = 12.0;
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    app
}

#[test]
fn the_ghost_shows_what_fits_what_is_blocked_and_what_is_short() {
    let mut app = short_and_blocked();
    let (x, y) = app.cursor;
    let plan = app.world.plan_paste(0, x, y, app.held.as_ref().unwrap());
    let states: Vec<PlanState> = plan.parts.iter().map(|p| p.state).collect();
    assert_eq!(states, vec![PlanState::Fits, PlanState::Blocked, PlanState::Fits, PlanState::Short, PlanState::Short]);
    let buf = render(&mut app, 130, 42);
    let sym = |t: (i32, i32)| {
        let (c, r) = at(&app, t);
        format!("{}{}", buf.cell((c, r)).unwrap().symbol(), buf.cell((c + 1, r)).unwrap().symbol())
    };
    assert_eq!(sym((x, y)), ">>", "a part that will place shows its sprite");
    assert_eq!(sym((x + 1, y)), "xx", "blocked");
    assert_eq!(sym((x + 2, y)), ">>");
    assert_eq!(sym((x + 3, y)), "..", "cannot afford");
    assert_eq!(sym((x + 4, y)), "..");
    let (c, r) = at(&app, (x + 3, y));
    assert_eq!(buf.cell((c, r)).unwrap().bg, Color::Indexed(208), "orange");
    let (c, r) = at(&app, (x + 1, y));
    assert_eq!(buf.cell((c, r)).unwrap().bg, Color::Indexed(196), "red");
    snapshot_to("blueprint-ghost", &buf);
    let t = text(&buf);
    assert!(t.contains("BLUEPRINT") && t.contains("Belts") && t.contains("2 of 5 fit your money"), "{t}");
    assert!(t.contains("2 short") && t.contains("1 blocked"), "{t}");
    assert!(t.contains("r turn") && t.contains("Space paste") && t.contains("Esc drop"), "{t}");
    // The build list gives way to it; HERE stays.
    assert!(!t.contains("BUILD") && t.contains("HERE"), "{t}");
    // The same, pasted: what fits, in reading order, then Space again after earning more.
    press(&mut app, ' ');
    assert!(app.msg.starts_with("Placed 2 of 5, ") && app.msg.contains("short") && app.msg.contains("1 blocked"), "{}", app.msg);
    assert!(app.msg.contains("the rest waits in your hand"), "{}", app.msg);
    assert!(app.world.tile(0, x, y).is_some() && app.world.tile(0, x + 2, y).is_some() && app.world.tile(0, x + 3, y).is_none());
    app.world.econ.money = 100.0;
    app.world.remove(0, x + 1, y);
    press(&mut app, ' ');
    assert!(app.msg.contains("already there"), "{}", app.msg);
    assert!((0..5).all(|i| app.world.tile(0, x + i, y).is_some()), "everything is there now");
    // Once everything is placed it ghosts nothing and a further paste says so.
    press(&mut app, ' ');
    assert!(app.msg.starts_with("Already all here"), "{}", app.msg);
}

fn snapshot_to(name: &str, buf: &Buffer) {
    std::fs::write(format!("target/snapshot-{name}.html"), to_html(buf)).unwrap();
}

#[test]
fn the_hand_in_a_small_pane_is_one_line_with_the_cost_and_what_fits() {
    let mut app = short_and_blocked();
    app.compact = Some(true);
    let buf = render(&mut app, 64, 16);
    snapshot_to("blueprint-small", &buf);
    let t = text(&buf);
    let last = t.lines().last().unwrap();
    assert!(last.starts_with(" Hand › Belts 5x1 · $20 · 2/5 fit"), "{last}");
    assert!(last.contains("r turn") && last.contains("Space paste") && last.trim_end().ends_with("Esc"), "{last}");
    // A fresh message (the paste result) takes the line for a while.
    press(&mut app, ' ');
    let t = text(&render(&mut app, 64, 16));
    assert!(t.lines().last().unwrap().contains("Placed 2 of 5"), "{t}");
    // Narrower: the hint words go first, the numbers and the way out stay.
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    for w in [64u16, 50, 44, 36, 30] {
        let t = text(&render(&mut app, w, 16));
        let last = t.lines().last().unwrap().to_string();
        assert!(last.chars().count() <= w as usize, "{w}: {last}");
        assert!(last.contains("Esc") && last.contains("fit"), "{w}: {last}");
        assert!(w < 50 || last.contains('$'), "{w}: {last}");
    }
}

#[test]
fn the_hand_fits_every_pane_size() {
    for (w, h) in [(30, 12), (36, 24), (44, 30), (44, 60), (50, 20), (64, 12), (64, 16), (90, 30), (99, 29), (130, 42), (220, 60)] {
        for compact in [None, Some(true)] {
            let mut app = short_and_blocked();
            app.compact = compact;
            for turns in 0..4 {
                let buf = render(&mut app, w, h);
                let t = text(&buf);
                assert_eq!(t.lines().count(), h as usize);
                assert!(t.lines().all(|l| l.chars().count() <= w as usize), "{w}x{h}");
                let last = t.lines().last().unwrap();
                if app.compact_now {
                    assert!(last.contains("Esc"), "{w}x{h}: {last}");
                } else {
                    assert!(t.contains("BLUEPRINT") && t.contains("HERE"), "{w}x{h}: {t}");
                }
                press(&mut app, 'r');
                assert!(turns < 4);
            }
        }
    }
    // A big blueprint (the whole demo) hangs off the screen without trouble.
    let mut w = World::new();
    w.build_demo().unwrap();
    let bp = w.capture(0, (0, 0, crate::sim::W - 1, crate::sim::H - 1)).bp;
    let mut app = App::new(w);
    app.held = Some(bp);
    app.cursor = (crate::sim::W - 3, crate::sim::H - 3);
    for (w, h) in [(44, 30), (130, 42)] {
        let t = text(&render(&mut app, w, h));
        assert!(t.lines().all(|l| l.chars().count() <= w as usize));
    }
}

#[test]
fn waste_and_items_stay_behind_a_blueprint_copies_the_layout_only() {
    let mut app = app();
    let (x, y) = app.cursor;
    app.world.place(0, x, y, Kind::Crate, Dir::E).unwrap();
    if let Some(Building { st: St::Crate { items }, .. }) = app.world.floors[0].tiles[crate::sim::idx(x, y)].as_mut() {
        items.extend([Item::Waste; 50]);
    }
    app.cursor = (x, y);
    press(&mut app, 'c');
    press(&mut app, ' ');
    press(&mut app, ' ');
    press(&mut app, 'c');
    let bp = app.held.clone().unwrap();
    assert_eq!(bp.part_count(), 1);
    app.cursor = (x + 5, y);
    press(&mut app, ' ');
    match &app.world.tile(0, x + 5, y).unwrap().st {
        St::Crate { items } => assert!(items.is_empty(), "contents are not copied"),
        _ => panic!("not a crate"),
    }
    // Copying took nothing out of the original.
    match &app.world.tile(0, x, y).unwrap().st {
        St::Crate { items } => assert_eq!(items.len(), 50),
        _ => panic!("not a crate"),
    }
}
