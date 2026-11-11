//! The area tool: marking a rectangle (keys and mouse), deconstructing it, the waste warning,
//! and how it looks at every pane size.

use super::*;
use crate::app::{Area, AreaStage};
use crate::sim::{rect_of, Building, Dir, Item, St};
use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

fn press(app: &mut App, c: char) {
    app.on_key(KeyEvent::from(KeyCode::Char(c)));
}

fn press_code(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::from(code));
}

/// A blank, fully unlocked world with the camera settled, so tiles near the cursor are on screen.
fn app() -> App {
    let mut app = App::new(World::blank());
    render(&mut app, 130, 42);
    app
}

/// Screen cell of a tile (the left cell of its pair).
fn at(app: &App, t: (i32, i32)) -> (u16, u16) {
    (app.map_area.x + ((t.0 - app.cam.0) * 2) as u16, app.map_area.y + (t.1 - app.cam.1) as u16)
}

fn mouse(app: &mut App, kind: MouseEventKind, t: (i32, i32)) {
    let (column, row) = at(app, t);
    app.on_mouse(MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE });
}

fn drag(app: &mut App, from: (i32, i32), to: (i32, i32)) {
    mouse(app, MouseEventKind::Down(MouseButton::Left), from);
    mouse(app, MouseEventKind::Drag(MouseButton::Left), to);
    mouse(app, MouseEventKind::Up(MouseButton::Left), to);
}

fn chosen(app: &App) -> Option<crate::sim::TileRect> {
    match app.area?.stage {
        AreaStage::Chosen { rect } => Some(rect),
        _ => None,
    }
}

fn fill(w: &mut World, f: usize, x: i32, y: i32, n: usize) {
    if let Some(Building { st: St::Crate { items }, .. }) = w.floors[f].tiles[idx(x, y)].as_mut() {
        items.extend(std::iter::repeat_n(Item::Waste, n));
    }
}

#[test]
fn two_corners_with_the_keyboard() {
    let mut app = app();
    let (x, y) = app.cursor;
    press(&mut app, 'c');
    assert_eq!(app.area, Some(Area { floor: 0, stage: AreaStage::Picking { corner: None } }));
    press_code(&mut app, KeyCode::Char(' '));
    assert_eq!(app.area.map(|a| a.stage), Some(AreaStage::Picking { corner: Some((x, y)) }));
    // The second corner follows the cursor.
    for c in ['d', 'd', 'd', 's', 's'] {
        press(&mut app, c);
    }
    assert_eq!(app.area_rect(), Some((x, y, x + 3, y + 2)));
    press_code(&mut app, KeyCode::Enter);
    assert_eq!(chosen(&app), Some((x, y, x + 3, y + 2)));
    // Space, Enter and c copy the area; an empty one says so and stays chosen.
    press_code(&mut app, KeyCode::Enter);
    assert_eq!(chosen(&app), Some((x, y, x + 3, y + 2)));
    assert!(app.msg.contains("Nothing in the area to copy"), "{}", app.msg);
    press(&mut app, 'c');
    assert!(chosen(&app).is_some() && app.held.is_none());
    // The cursor can walk off after choosing without changing the rectangle.
    press(&mut app, 'a');
    assert_eq!(chosen(&app), Some((x, y, x + 3, y + 2)));
    press_code(&mut app, KeyCode::Esc);
    assert_eq!(app.area, None);
}

#[test]
fn corners_in_any_order_make_the_same_rectangle() {
    let (x, y) = app().cursor;
    let want = (x - 2, y - 1, x, y);
    // Second corner up and to the left, by keys.
    let mut app = app();
    press(&mut app, 'c');
    press(&mut app, ' ');
    for c in ['a', 'a', 'w'] {
        press(&mut app, c);
    }
    press(&mut app, ' ');
    assert_eq!(chosen(&app), Some(want));
}

#[test]
fn a_left_drag_draws_the_rectangle_in_all_four_directions() {
    let probe = app();
    let (cx, cy) = (probe.cam.0 + 20, probe.cam.1 + 15);
    let start = (cx, cy);
    for (end, name) in
        [((cx + 6, cy + 4), "down-right"), ((cx - 6, cy + 4), "down-left"), ((cx + 6, cy - 4), "up-right"), ((cx - 6, cy - 4), "up-left")]
    {
        let mut app = app();
        press(&mut app, 'c');
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), start);
        assert_eq!(app.area.map(|a| a.stage), Some(AreaStage::Picking { corner: Some(start) }), "{name}");
        mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), end);
        // Live: the tint follows the cursor before the button is let go.
        assert_eq!(app.area_rect(), Some(rect_of(start, end)), "{name}");
        mouse(&mut app, MouseEventKind::Up(MouseButton::Left), end);
        assert_eq!(chosen(&app), Some(rect_of(start, end)), "{name}");
        assert_eq!(rect_of(start, end).2 - rect_of(start, end).0, 6, "{name}");
    }
}

#[test]
fn two_clicks_also_choose_and_a_new_press_starts_over() {
    let mut app = app();
    let (cx, cy) = (app.cam.0 + 20, app.cam.1 + 15);
    press(&mut app, 'c');
    for t in [(cx, cy), (cx + 3, cy + 2)] {
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), t);
        mouse(&mut app, MouseEventKind::Up(MouseButton::Left), t);
    }
    assert_eq!(chosen(&app), Some((cx, cy, cx + 3, cy + 2)));
    // A click with a rectangle chosen starts a new one; a drag finishes it.
    drag(&mut app, (cx + 5, cy + 5), (cx + 8, cy + 6));
    assert_eq!(chosen(&app), Some((cx + 5, cy + 5, cx + 8, cy + 6)));
    // A drag that ends where it began is just a first corner.
    press_code(&mut app, KeyCode::Esc);
    press(&mut app, 'c');
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), (cx, cy));
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), (cx, cy));
    assert_eq!(app.area.map(|a| a.stage), Some(AreaStage::Picking { corner: Some((cx, cy)) }));
}

#[test]
fn right_click_floor_change_and_digits_leave_the_tool() {
    let mut app = app();
    let t = (app.cam.0 + 20, app.cam.1 + 15);
    press(&mut app, 'c');
    mouse(&mut app, MouseEventKind::Down(MouseButton::Right), t);
    assert_eq!(app.area, None);
    // A right click never removes while the tool is open.
    app.world.place(0, t.0, t.1, Kind::Belt, Dir::E).unwrap();
    press(&mut app, 'c');
    mouse(&mut app, MouseEventKind::Down(MouseButton::Right), t);
    assert!(app.world.tile(0, t.0, t.1).is_some());
    // Changing floor cancels (the rectangle belongs to one floor).
    press(&mut app, 'c');
    press(&mut app, ' ');
    press(&mut app, '>');
    assert_eq!((app.area, app.floor), (None, app.world.floor_of(1).unwrap()));
    press(&mut app, '<');
    press(&mut app, 'c');
    mouse(&mut app, MouseEventKind::ScrollDown, t);
    assert_eq!(app.area, None);
    mouse(&mut app, MouseEventKind::ScrollUp, t);
    assert_eq!(app.floor, 0);
    // A digit closes the tool and opens that build category.
    press(&mut app, 'c');
    press(&mut app, '2');
    assert_eq!((app.area, app.menu), (None, Some(Category::Transport)));
    // Starting the tool puts the hand away.
    press(&mut app, '1');
    press(&mut app, '1');
    assert!(app.selected.is_some());
    press(&mut app, 'c');
    assert_eq!((app.selected, app.menu), (None, None));
    // Picking a part (middle-click on one) closes it too.
    mouse(&mut app, MouseEventKind::Down(MouseButton::Middle), t);
    assert_eq!((app.area, app.selected), (None, Some(Kind::Belt)));
}

#[test]
fn shift_with_a_move_key_only_moves_in_the_tool() {
    let mut app = app();
    app.selected = None;
    let (x, y) = app.cursor;
    press(&mut app, 'c');
    app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::SHIFT));
    app.on_key(KeyEvent::from(KeyCode::Char('D')));
    app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
    assert_eq!(app.cursor, (x + 3, y));
    assert!(app.world.tile(0, x, y).is_none() && app.world.tile(0, x + 1, y).is_none(), "no belts laid");
    // Other keys that would act on the part under the cursor are ignored.
    app.world.place(0, x + 3, y, Kind::Belt, Dir::E).unwrap();
    press(&mut app, 'r');
    press(&mut app, 'f');
    assert_eq!(app.world.tile(0, x + 3, y).map(|b| b.dir), Some(Dir::E));
    assert!(app.area.is_some());
}

#[test]
fn x_before_the_rectangle_is_chosen_removes_nothing() {
    let mut app = app();
    let (x, y) = app.cursor;
    app.world.place(0, x, y, Kind::Belt, Dir::E).unwrap();
    press(&mut app, 'c');
    press(&mut app, 'x');
    assert!(app.world.tile(0, x, y).is_some() && app.area.is_some());
    press(&mut app, ' ');
    press(&mut app, 'x');
    assert!(app.world.tile(0, x, y).is_some(), "one corner is not a rectangle yet");
}

#[test]
fn x_removes_the_area_with_a_full_refund_and_z_puts_it_back() {
    let mut app = app();
    let (x, y) = app.cursor;
    for i in 0..6 {
        app.world.place(0, x + i, y, Kind::Belt, Dir::E).unwrap();
    }
    app.world.place(0, x, y + 3, Kind::Furnace, Dir::E).unwrap();
    app.world.place(0, x + 20, y, Kind::Belt, Dir::E).unwrap(); // outside
    let money = app.world.econ.money;
    let refund = 6.0 * Kind::Belt.price(1) + Kind::Furnace.price(3);
    press(&mut app, 'c');
    press(&mut app, ' ');
    for c in ['d', 'd', 'd', 'd', 'd', 's', 's', 's'] {
        press(&mut app, c);
    }
    press(&mut app, ' ');
    assert_eq!(chosen(&app), Some((x, y, x + 5, y + 3)));
    press(&mut app, 'x');
    assert_eq!(app.area, None);
    assert!((app.world.econ.money - money - refund).abs() < 1e-9);
    assert!(app.world.tile(0, x, y).is_none() && app.world.tile(0, x, y + 3).is_none() && app.world.tile(0, x + 20, y).is_some());
    assert!(app.msg.contains("Removed 7 parts") && app.msg.contains("z puts them back"), "{}", app.msg);
    assert!(app.last_removed.is_some());
    // z on another floor says where to go; the parts wait.
    press(&mut app, '>');
    press(&mut app, 'z');
    assert!(app.msg.contains("go there") && app.last_removed.is_some(), "{}", app.msg);
    press(&mut app, '<');
    press(&mut app, 'z');
    assert!(app.msg.starts_with("Put back 7 parts"), "{}", app.msg);
    assert!(app.world.tile(0, x, y).is_some() && app.world.tile(0, x, y + 3).is_some());
    assert!((app.world.econ.money - money).abs() < 1e-9, "removing and restoring nets to nothing");
    assert!(app.last_removed.is_none());
    press(&mut app, 'z');
    assert!(app.msg.contains("Nothing to put back"), "{}", app.msg);
}

#[test]
fn an_empty_rectangle_removes_nothing_and_says_so() {
    let mut app = app();
    press(&mut app, 'c');
    press(&mut app, ' ');
    press(&mut app, 'd');
    press(&mut app, ' ');
    press(&mut app, 'x');
    assert_eq!(app.area, None);
    assert!(app.msg.contains("Nothing in the area"), "{}", app.msg);
    assert!(app.last_removed.is_none());
}

/// Marks the rectangle (x0, y0)-(x1, y1) by keys and leaves it chosen.
fn choose(app: &mut App, (x0, y0): (i32, i32), (x1, y1): (i32, i32)) {
    if app.area.is_none() {
        press(app, 'c');
    }
    app.cursor = (x0, y0);
    press(app, ' ');
    app.cursor = (x1, y1);
    press(app, ' ');
}

#[test]
fn waste_in_the_area_needs_a_second_x_and_pays_one_fee() {
    let mut app = app();
    let (x, y) = app.cursor;
    app.world.place(0, x, y, Kind::Crate, Dir::E).unwrap();
    app.world.place(0, x + 4, y, Kind::Crate, Dir::E).unwrap();
    app.world.place(0, x + 8, y, Kind::Belt, Dir::E).unwrap();
    // 30 waste each: $15 apiece, so neither would ask alone, but together it is $30.
    fill(&mut app.world, 0, x, y, 30);
    fill(&mut app.world, 0, x + 4, y, 30);
    assert!(app.world.removal_fee(0, x, y).1 <= 20.0);
    let money = app.world.econ.money;
    choose(&mut app, (x, y), (x + 9, y + 1));
    press(&mut app, 'x');
    // Nothing happened, and the warning shows one total for the whole area.
    assert!(chosen(&app).is_some() && app.world.tile(0, x, y).is_some() && app.world.econ.money == money);
    assert!(app.msg.contains("60 waste") && app.msg.contains("$30 in all") && app.msg.contains("x again to pay"), "{}", app.msg);
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("x again to pay the fee") && t.contains("waste fee $30"), "{t}");
    let t = text(&render(&mut app, 64, 16));
    assert!(lines(&t).last().unwrap().contains("fee $30") && lines(&t).last().unwrap().contains("again"), "{t}");
    // The second x pays it once and removes everything.
    press(&mut app, 'x');
    assert_eq!(app.area, None);
    assert!(app.world.tile(0, x, y).is_none() && app.world.tile(0, x + 4, y).is_none() && app.world.tile(0, x + 8, y).is_none());
    let refund = 2.0 * Kind::Crate.price(1) + Kind::Belt.price(1);
    assert!((app.world.econ.money - (money + refund - 30.0)).abs() < 1e-9, "{}", app.world.econ.money - money);
    assert!(app.msg.contains("Removed 3 parts") && app.msg.contains("waste fee $30"), "{}", app.msg);
}

fn lines(t: &str) -> Vec<&str> {
    t.lines().collect()
}

#[test]
fn a_small_waste_fee_needs_no_second_press() {
    let mut app = app();
    let (x, y) = app.cursor;
    app.world.place(0, x, y, Kind::Crate, Dir::E).unwrap();
    fill(&mut app.world, 0, x, y, 20); // $10
    choose(&mut app, (x, y), (x + 1, y));
    press(&mut app, 'x');
    assert_eq!(app.area, None);
    assert!(app.world.tile(0, x, y).is_none());
    assert!(app.msg.contains("waste fee $10"), "{}", app.msg);
}

#[test]
fn the_warning_is_for_that_rectangle_and_that_fee() {
    let mut app = app();
    let (x, y) = app.cursor;
    app.world.place(0, x, y, Kind::Crate, Dir::E).unwrap();
    fill(&mut app.world, 0, x, y, 60);
    choose(&mut app, (x, y), (x + 2, y));
    press(&mut app, 'x');
    assert!(app.confirm_area.is_some());
    // Waste piles up before the second press: the new fee has to be agreed to again.
    fill(&mut app.world, 0, x, y, 20);
    press(&mut app, 'x');
    assert!(app.world.tile(0, x, y).is_some(), "the fee went up, so it asks again");
    assert!(app.msg.contains("$40"), "{}", app.msg);
    // A different rectangle starts the warning over.
    press_code(&mut app, KeyCode::Esc);
    choose(&mut app, (x, y), (x + 3, y + 1));
    assert!(app.confirm_area.is_none());
    press(&mut app, 'x');
    assert!(app.world.tile(0, x, y).is_some() && app.confirm_area.is_some());
    // Esc forgets it.
    press_code(&mut app, KeyCode::Esc);
    assert!(app.confirm_area.is_none());
}

#[test]
fn an_elevator_at_the_edge_of_the_selection_is_removed_whole() {
    let mut app = app();
    let (x, y) = app.cursor;
    app.world.place_sized(0, x + 2, y + 2, Kind::Elevator, Dir::E, 3).unwrap(); // covers x+2..x+4
    app.world.place(0, x + 20, y, Kind::Belt, Dir::E).unwrap();
    let money = app.world.econ.money;
    // The drag ends on the shaft's top-left tile only.
    let (cx, cy) = (x + 2, y + 2);
    choose(&mut app, (cx - 3, cy - 3), (cx, cy));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("1 part") && t.contains("1 elevator"), "{t}");
    press(&mut app, 'x');
    assert!(app.world.lifts.is_empty());
    for f in 0..app.world.floors.len() {
        assert!(app.world.floors[f].tiles.iter().flatten().all(|b| b.kind != Kind::Elevator), "floor {f}");
    }
    assert!((app.world.econ.money - money - Kind::Elevator.price(3)).abs() < 1e-9);
    assert!(app.world.tile(0, x + 20, y).is_some());
    // z brings the shaft, its doors and its car back.
    press(&mut app, 'z');
    assert_eq!(app.world.lifts.len(), 1);
    assert!(app.world.floors.iter().filter(|f| f.tiles[idx(cx, cy)].is_some()).count() >= 2);
}

#[test]
fn a_long_machine_at_the_edge_is_taken_whole() {
    let mut app = app();
    let (x, y) = app.cursor;
    app.world.place(0, x, y, Kind::Kiln, Dir::E).unwrap(); // 5 wide, 3 tall
    let (w, h) = Kind::Kiln.dims(Dir::E);
    // The rectangle reaches only the kiln's far corner.
    choose(&mut app, (x + w - 1, y + h - 1), (x + w + 3, y + h + 3));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("1 part"), "{t}");
    press(&mut app, 'x');
    assert!(app.world.floors[0].owner.iter().all(|o| o.is_none()));
}

#[test]
fn parts_on_other_floors_stay() {
    let mut app = app();
    let (x, y) = app.cursor;
    app.world.place(0, x, y, Kind::Belt, Dir::E).unwrap();
    app.world.place(1, x, y, Kind::Belt, Dir::E).unwrap();
    choose(&mut app, (x - 1, y - 1), (x + 1, y + 1));
    press(&mut app, 'x');
    assert!(app.world.tile(0, x, y).is_none() && app.world.tile(1, x, y).is_some());
}

#[test]
fn controls_c_and_z_are_free_and_settings_moved_to_g() {
    use crate::controls::Action;
    let c = &app().controls;
    assert_eq!(c.conflicts(), Vec::<String>::new());
    assert_eq!((c.key(Action::Area), c.key(Action::Restore), c.key(Action::Settings)), ("c".into(), "z".into(), "g".into()));
    // g opens settings and c does not.
    let mut app = app();
    press(&mut app, 'c');
    assert!(!app.settings_open && app.area.is_some());
    press_code(&mut app, KeyCode::Esc);
    press(&mut app, 'g');
    assert!(app.settings_open);
}

/// Marks a state of the tool for the render tests: 0 picking (no corner yet), 1 first corner set,
/// 2 chosen, 3 chosen over waste and warned, 4 chosen over nothing.
fn tool_state(app: &mut App, state: usize) {
    app.area = None;
    app.confirm_area = None;
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    let (x, y) = app.cursor;
    let wide = (x - 6, y - 4, x + 6, y + 4);
    match state {
        0 => press(app, 'c'),
        1 => {
            press(app, 'c');
            app.cursor = (wide.0, wide.1);
            press(app, ' ');
            app.cursor = (x, y);
        }
        2 | 3 => {
            choose(app, (wide.0, wide.1), (wide.2, wide.3));
            if state == 3 {
                press(app, 'x');
            }
        }
        _ => choose(app, (x + 30, y + 20), (x + 31, y + 21)),
    }
    app.cursor = (x, y);
}

/// A factory worth looking at: the demo, plus a crate with enough waste for the warning.
fn busy() -> App {
    let mut w = World::new();
    w.build_demo().unwrap();
    for _ in 0..TPS * 20 {
        w.tick();
    }
    w.econ.money = 1_234_567.0;
    let mut app = App::new(w);
    render(&mut app, 130, 42);
    // Put a waste-holding crate near the cursor, so every state has something to say.
    let (cx, cy) = app.cursor;
    let spot = (-3..=3)
        .flat_map(|dx| (-3..=3).map(move |dy| (cx + dx, cy + dy)))
        .find(|&(x, y)| app.world.place_cost(0, x, y, Kind::Crate, Dir::E, 0).is_ok());
    let (x, y) = spot.expect("room for a crate");
    app.world.place(0, x, y, Kind::Crate, Dir::E).unwrap();
    fill(&mut app.world, 0, x, y, 80);
    app
}

#[test]
fn the_area_tool_fits_every_pane_size() {
    for w in [30u16, 36, 44, 50, 64, 90, 120, 130, 220, 300] {
        for h in [12u16, 16, 24, 30, 42, 60, 90] {
            let mut app = busy();
            render(&mut app, w, h);
            for state in 0..5 {
                tool_state(&mut app, state);
                let buf = render(&mut app, w, h);
                let t = text(&buf);
                let ls = lines(&t);
                assert_eq!(ls.len(), h as usize, "{w}x{h} state {state}");
                assert!(ls.iter().all(|l| l.chars().count() <= w as usize), "{w}x{h} state {state}");
                if app.compact_now {
                    // One status line: the way out always shows, and so do the counts once there is a rectangle.
                    let last = ls.last().unwrap();
                    assert!(last.contains("Esc"), "{w}x{h} state {state}: {last}");
                    if state >= 1 {
                        assert!(last.chars().any(|c| c.is_ascii_digit()), "{w}x{h} state {state}: {last}");
                    }
                    if w >= 44 {
                        assert!(last.contains("Area") || w < 50, "{w}x{h} state {state}: {last}");
                    }
                } else {
                    assert!(t.contains("AREA") && t.contains("Esc cancel") && t.contains("HERE"), "{w}x{h} state {state}:\n{t}");
                    if state == 2 {
                        assert!(t.contains("x remove") && t.contains("refund $"), "{w}x{h}:\n{t}");
                    }
                    if state == 3 {
                        assert!(t.contains("x again to pay the fee"), "{w}x{h}:\n{t}");
                    }
                }
                // The tint is on the map in every view.
                assert!(
                    state == 0 || state == 4 || (0..buf.area.width).any(|x| (0..buf.area.height).any(|y| tinted(&buf, x, y))),
                    "{w}x{h} state {state}"
                );
            }
        }
    }
}

/// Whether a cell has the area tint behind it (blue while marking, red once chosen).
fn tinted(buf: &Buffer, x: u16, y: u16) -> bool {
    buf.cell((x, y)).is_some_and(|c| matches!(c.bg, Color::Indexed(24) | Color::Indexed(52)))
}

#[test]
fn area_snapshots() {
    let mut app = busy();
    tool_state(&mut app, 2);
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("AREA") && t.contains("parts") && t.contains("x remove") && t.contains("HERE"), "{t}");
    std::fs::write("target/snapshot-area.html", to_html(&buf)).unwrap();

    tool_state(&mut app, 1);
    let buf = render(&mut app, 130, 42);
    assert!(text(&buf).contains("or let go: other corner"));
    std::fs::write("target/snapshot-area-picking.html", to_html(&buf)).unwrap();

    // Small pane: one status line, starting with the crumb.
    tool_state(&mut app, 2);
    let buf = render(&mut app, 64, 16);
    let t = text(&buf);
    assert!(lines(&t).last().unwrap().starts_with(" Area › "), "{t}");
    // Three verbs no longer fit as words next to the fee at 64 columns: the keys stay.
    assert!(lines(&t).last().unwrap().contains("c/x/u") && lines(&t).last().unwrap().contains("Esc"), "{t}");
    let wide = text(&render(&mut app, 90, 16));
    assert!(lines(&wide).last().unwrap().contains("x remove") && lines(&wide).last().unwrap().contains("u upgrade"), "{wide}");
    std::fs::write("target/snapshot-area-small.html", to_html(&buf)).unwrap();

    // Narrow and tall: every line within the width.
    let buf = render(&mut app, 44, 30);
    let t = text(&buf);
    assert!(lines(&t).iter().all(|l| l.chars().count() <= 44));
    assert!(lines(&t).last().unwrap().contains("Esc"), "{t}");
    std::fs::write("target/snapshot-area-narrow.html", to_html(&buf)).unwrap();

    // Zoomed out: far more cells than a normal window.
    let buf = render(&mut app, 300, 90);
    assert!(text(&buf).contains("AREA"));
    std::fs::write("target/snapshot-area-huge.html", to_html(&buf)).unwrap();
}

#[test]
fn the_status_line_drops_hints_before_the_numbers() {
    let mut app = busy();
    tool_state(&mut app, 2);
    let mut last = Vec::new();
    for w in [90u16, 64, 50, 44, 36, 30] {
        let t = text(&render(&mut app, w, 16));
        last.push(lines(&t).last().unwrap().trim_end().to_string());
    }
    // Widest first: the word "remove" goes before the key, the key before the word Area.
    assert!(last[0].contains("x remove") && last[0].contains("Esc cancel"), "{last:?}");
    for l in &last {
        assert!(l.contains("Esc") && l.contains("part") || l.chars().any(|c| c.is_ascii_digit()), "{l}");
    }
    assert!(!last[5].contains("remove"), "{last:?}");
}
