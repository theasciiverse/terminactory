//! The upgrade planner from the player's side: the research gate, the preview and the second
//! press, partial upgrades, locked targets, `z`, the tint, and how the line looks.

use super::*;
use crate::app::AreaStage;
use crate::sim::{Dir, Item, Setting, Tech};

fn lines(t: &str) -> Vec<&str> {
    t.lines().collect()
}

fn press(app: &mut App, c: char) {
    app.on_key(KeyEvent::from(KeyCode::Char(c)));
}

/// A blank, fully unlocked world (planner included) with the camera settled.
fn app() -> App {
    let mut app = App::new(World::blank());
    render(&mut app, 130, 42);
    app
}

fn lock_planner(app: &mut App) {
    app.world.progress.researched[Tech::UpgradePlanner.idx()] = false;
}

/// Marks the rectangle by keys and leaves it chosen.
fn choose(app: &mut App, (x0, y0): (i32, i32), (x1, y1): (i32, i32)) {
    if app.area.is_none() {
        press(app, 'c');
    }
    app.cursor = (x0, y0);
    press(app, ' ');
    app.cursor = (x1, y1);
    press(app, ' ');
}

fn click(app: &mut App, t: (i32, i32)) {
    use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let (column, row) = (app.map_area.x + ((t.0 - app.cam.0) * 2) as u16, app.map_area.y + (t.1 - app.cam.1) as u16);
    for kind in [MouseEventKind::Down(MouseButton::Left), MouseEventKind::Up(MouseButton::Left)] {
        app.on_mouse(MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE });
    }
}

fn chosen(app: &App) -> bool {
    matches!(app.area.map(|a| a.stage), Some(AreaStage::Chosen { .. }))
}

fn kind_at(app: &App, x: i32, y: i32) -> Kind {
    app.world.tile(0, x, y).unwrap().kind
}

/// Five belts, a cheap drill and a cheap panel in a row of the ground floor.
fn row(app: &mut App) -> (i32, i32) {
    let (x, y) = app.cursor;
    for i in 0..5 {
        app.world.place(0, x + i, y, Kind::Belt, Dir::E).unwrap();
    }
    app.world.place(0, x + 6, y, Kind::Drill, Dir::E).unwrap();
    app.world.place(0, x + 9, y, Kind::Solar, Dir::E).unwrap();
    (x, y)
}

fn price_of_row() -> f64 {
    5.0 * Kind::Belt.upgrade_cost(Kind::FastBelt) + Kind::Drill.upgrade_cost(Kind::BasicDrill) + Kind::Solar.upgrade_cost(Kind::BasicSolar)
}

#[test]
fn before_the_tech_u_says_what_unlocks_it_and_changes_nothing() {
    let mut app = app();
    lock_planner(&mut app);
    let (x, y) = row(&mut app);
    let money = app.world.econ.money;
    choose(&mut app, (x, y), (x + 10, y + 1));
    press(&mut app, 'u');
    assert!(chosen(&app), "the rectangle stays, so x and c still work");
    for want in ["Upgrades are locked", "Upgrade planner", "t", "Swiftline Conveyors", "3,000 RP", "60 gearboxes"] {
        assert!(app.msg.contains(want), "{want}: {}", app.msg);
    }
    press(&mut app, 'u');
    assert_eq!(kind_at(&app, x, y), Kind::Belt);
    assert_eq!(app.world.econ.money, money);
    assert!(app.confirm_upgrade.is_none() && app.last_upgrade.is_none());
    // The small pane's crumb says it too.
    let t = text(&render(&mut app, 64, 16));
    assert!(lines(&t).last().unwrap().contains("locked"), "{t}");
}

#[test]
fn u_outside_the_tool_says_the_same_before_the_tech_and_starts_the_tool_after() {
    let mut app = app();
    lock_planner(&mut app);
    press(&mut app, 'u');
    assert!(app.area.is_none() && app.msg.contains("Upgrades are locked"), "{}", app.msg);
    app.world.progress.researched[Tech::UpgradePlanner.idx()] = true;
    press(&mut app, 'u');
    assert!(app.area.is_some() && app.msg.contains("Upgrade:"), "{}", app.msg);
}

#[test]
fn u_before_the_rectangle_is_chosen_upgrades_nothing() {
    let mut app = app();
    let (x, y) = row(&mut app);
    press(&mut app, 'c');
    press(&mut app, 'u');
    assert_eq!(kind_at(&app, x, y), Kind::Belt);
    assert!(app.msg.contains("Mark the rectangle first"), "{}", app.msg);
}

#[test]
fn the_first_u_previews_and_the_second_does_it() {
    let mut app = app();
    let (x, y) = row(&mut app);
    let money = app.world.econ.money;
    choose(&mut app, (x, y), (x + 10, y + 1));
    press(&mut app, 'u');
    let want = format!("u again to upgrade 7 parts for ${}", price_of_row());
    assert!(app.msg.starts_with(&want), "{} / {want}", app.msg);
    assert_eq!((kind_at(&app, x, y), app.world.econ.money), (Kind::Belt, money), "the preview changes nothing");
    assert!(chosen(&app));
    press(&mut app, 'u');
    assert!(app.area.is_none(), "all done: the tool closes");
    for i in 0..5 {
        assert_eq!(kind_at(&app, x + i, y), Kind::FastBelt);
    }
    assert_eq!((kind_at(&app, x + 6, y), kind_at(&app, x + 9, y)), (Kind::BasicDrill, Kind::BasicSolar));
    assert!((money - app.world.econ.money - price_of_row()).abs() < 1e-9);
    for want in ["Upgraded 5 fast belts, 1 basic drill, 1 basic solar panel", &format!("for ${}", price_of_row()), "z swaps them back"] {
        assert!(app.msg.contains(want), "{want}: {}", app.msg);
    }
    assert!(app.last_upgrade.is_some());
}

#[test]
fn moving_the_rectangle_cancels_the_preview() {
    let mut app = app();
    let (x, y) = row(&mut app);
    choose(&mut app, (x, y), (x + 10, y + 1));
    press(&mut app, 'u');
    assert!(app.confirm_upgrade.is_some());
    // A click starts a new rectangle (here the belts only), which has to be previewed again.
    for t in [(x, y), (x + 4, y)] {
        click(&mut app, t);
    }
    assert!(chosen(&app) && app.confirm_upgrade.is_none() && app.area_note.is_none());
    press(&mut app, 'u');
    assert_eq!(kind_at(&app, x, y), Kind::Belt, "first press only previews");
    assert!(app.msg.contains("u again to upgrade 5 parts"), "{}", app.msg);
}

#[test]
fn the_preview_goes_stale_when_the_numbers_change() {
    let mut app = app();
    let (x, y) = row(&mut app);
    choose(&mut app, (x, y), (x + 10, y + 1));
    press(&mut app, 'u');
    // A part is removed before the second press: it previews again instead of doing something else.
    app.world.remove(0, x + 9, y).unwrap();
    press(&mut app, 'u');
    assert_eq!(kind_at(&app, x, y), Kind::Belt);
    assert!(app.msg.contains("u again to upgrade 6 parts"), "{}", app.msg);
}

#[test]
fn nothing_to_upgrade_says_so_and_costs_nothing() {
    let mut app = app();
    let (x, y) = app.cursor;
    app.world.place(0, x, y, Kind::Crate, Dir::E).unwrap();
    app.world.place(0, x + 2, y, Kind::FastBelt, Dir::E).unwrap();
    let money = app.world.econ.money;
    choose(&mut app, (x, y), (x + 5, y + 1));
    press(&mut app, 'u');
    assert!(app.msg.contains("Nothing in the area has a next version"), "{}", app.msg);
    assert!(chosen(&app) && app.world.econ.money == money && app.confirm_upgrade.is_none());
}

#[test]
fn short_money_upgrades_a_prefix_and_the_line_counts_the_rest() {
    let mut app = app();
    let (x, y) = row(&mut app);
    let belt = Kind::Belt.upgrade_cost(Kind::FastBelt);
    app.world.econ.money = belt * 3.0 + 1.0;
    choose(&mut app, (x, y), (x + 10, y + 1));
    press(&mut app, 'u');
    assert!(app.msg.contains("u again to upgrade 3 parts") && app.msg.contains("4 short of money"), "{}", app.msg);
    press(&mut app, 'u');
    for i in 0..3 {
        assert_eq!(kind_at(&app, x + i, y), Kind::FastBelt, "{i}");
    }
    assert_eq!((kind_at(&app, x + 3, y), kind_at(&app, x + 6, y)), (Kind::Belt, Kind::Drill));
    for want in ["Upgraded 3 fast belts", "4 short of money", "more"] {
        assert!(app.msg.contains(want), "{want}: {}", app.msg);
    }
    assert!(chosen(&app), "the rectangle stays, so the rest can follow when you can pay");
    // Earn more and go again: u previews, u does the rest.
    app.world.econ.money = 100_000.0;
    press(&mut app, 'u');
    press(&mut app, 'u');
    assert_eq!((kind_at(&app, x + 4, y), kind_at(&app, x + 9, y)), (Kind::FastBelt, Kind::BasicSolar));
    assert!(app.area.is_none());
}

#[test]
fn a_locked_target_is_left_and_counted_with_its_name() {
    let mut app = app();
    let (x, y) = row(&mut app);
    app.world.progress.researched[Tech::FastBelts.idx()] = false;
    choose(&mut app, (x, y), (x + 10, y + 1));
    press(&mut app, 'u');
    assert!(app.msg.contains("u again to upgrade 2 parts") && app.msg.contains("5 locked (Fast belt not researched)"), "{}", app.msg);
    press(&mut app, 'u');
    assert_eq!((kind_at(&app, x, y), kind_at(&app, x + 6, y)), (Kind::Belt, Kind::BasicDrill));
    assert!(app.msg.contains("5 locked"), "{}", app.msg);
    // Only locked parts in the rectangle: nothing to confirm.
    choose(&mut app, (x, y), (x + 4, y));
    press(&mut app, 'u');
    assert!(app.msg.contains("Nothing to upgrade right now") && app.msg.contains("5 locked"), "{}", app.msg);
    assert!(app.confirm_upgrade.is_none());
}

#[test]
fn settings_and_recipes_survive_through_the_keys() {
    let mut app = app();
    let (x, y) = app.cursor;
    app.world.place(0, x, y, Kind::Drill, Dir::S).unwrap();
    app.world.apply_setting(0, x, y, Setting::Mines(Item::Stone));
    app.world.place(0, x + 5, y, Kind::AdvancedAssembler, Dir::W).unwrap();
    app.world.configure(0, x + 5, y);
    while app.world.tile(0, x + 5, y).and_then(|b| b.recipe()).is_none_or(|r| r.name != "control board") {
        app.world.configure(0, x + 5, y);
    }
    choose(&mut app, (x, y), (x + 9, y + 3));
    press(&mut app, 'u');
    press(&mut app, 'u');
    let drill = app.world.tile(0, x, y).unwrap();
    assert_eq!((drill.kind, drill.dir), (Kind::BasicDrill, Dir::S));
    assert!(matches!(&drill.st, crate::sim::St::Drill { mines: Some(Item::Stone), .. }));
    let asm = app.world.tile(0, x + 5, y).unwrap();
    assert_eq!((asm.kind, asm.dir, asm.recipe().map(|r| r.name)), (Kind::PrecisionAssembler, Dir::W, Some("control board")));
}

#[test]
fn z_swaps_the_parts_back_and_refunds_the_difference() {
    let mut app = app();
    let (x, y) = row(&mut app);
    let money = app.world.econ.money;
    choose(&mut app, (x, y), (x + 10, y + 1));
    press(&mut app, 'u');
    press(&mut app, 'u');
    assert!(app.world.econ.money < money);
    press(&mut app, '>');
    press(&mut app, 'z');
    assert!(app.msg.contains("go there") && app.last_upgrade.is_some(), "{}", app.msg);
    press(&mut app, '<');
    press(&mut app, 'z');
    assert!(app.msg.starts_with("Swapped 7 of 7 parts back"), "{}", app.msg);
    assert!((app.world.econ.money - money).abs() < 1e-9, "whole again");
    assert_eq!((kind_at(&app, x, y), kind_at(&app, x + 6, y), kind_at(&app, x + 9, y)), (Kind::Belt, Kind::Drill, Kind::Solar));
    assert!(app.last_upgrade.is_none());
    press(&mut app, 'z');
    assert!(app.msg.contains("Nothing to put back"), "{}", app.msg);
}

#[test]
fn z_undoes_the_most_recent_of_a_remove_and_an_upgrade() {
    let mut app = app();
    let (x, y) = row(&mut app);
    // Remove the crate-free row's belts, then upgrade the rest: z swaps first, then puts back.
    choose(&mut app, (x, y), (x + 4, y));
    press(&mut app, 'x');
    assert!(app.last_removed.is_some());
    choose(&mut app, (x + 6, y), (x + 10, y));
    press(&mut app, 'u');
    press(&mut app, 'u');
    assert_eq!(kind_at(&app, x + 6, y), Kind::BasicDrill);
    press(&mut app, 'z');
    assert!(app.msg.starts_with("Swapped 2 of 2 parts back"), "{}", app.msg);
    assert_eq!(kind_at(&app, x + 6, y), Kind::Drill);
    assert!(app.world.tile(0, x, y).is_none(), "the belts are still gone");
    press(&mut app, 'z');
    assert!(app.msg.starts_with("Put back 5 parts"), "{}", app.msg);
    assert_eq!(kind_at(&app, x, y), Kind::Belt);
    // And the other way round: an upgrade, then a remove, then z puts the remove back first.
    choose(&mut app, (x, y), (x + 4, y));
    press(&mut app, 'u');
    press(&mut app, 'u');
    choose(&mut app, (x + 6, y), (x + 6, y));
    press(&mut app, 'x');
    press(&mut app, 'z');
    assert!(app.msg.starts_with("Put back 1 part"), "{}", app.msg);
    press(&mut app, 'z');
    assert!(app.msg.starts_with("Swapped 5 of 5 parts back"), "{}", app.msg);
}

#[test]
fn upgradable_parts_are_green_while_the_rectangle_is_chosen() {
    let mut app = app();
    let (x, y) = row(&mut app);
    app.world.place(0, x + 12, y, Kind::Crate, Dir::E).unwrap();
    let green = Some(ratatui::style::Color::Indexed(41));
    let bg = |app: &App, buf: &Buffer, t: (i32, i32)| {
        let (col, row) = (app.map_area.x + ((t.0 - app.cam.0) * 2) as u16, app.map_area.y + (t.1 - app.cam.1) as u16);
        buf.cell((col, row)).map(|c| c.style().bg)
    };
    choose(&mut app, (x, y), (x + 13, y + 1));
    let buf = render(&mut app, 130, 42);
    for t in [(x, y), (x + 4, y), (x + 6, y), (x + 9, y)] {
        assert_eq!(bg(&app, &buf, t).flatten(), green, "{t:?}");
    }
    // The crate is in the area but has no next version: lit like any part, not green.
    assert_ne!(bg(&app, &buf, (x + 12, y)).flatten(), green);
    // The sidebar says how many and what for.
    let t = text(&buf);
    assert!(t.contains(&format!("green: 7 to upgrade, ${}", price_of_row())), "{t}");
    // Picking a new rectangle clears the green until it is chosen again.
    press(&mut app, ' ');
    let buf = render(&mut app, 130, 42);
    assert_ne!(bg(&app, &buf, (x, y)).flatten(), green);
    // Before the tech there is no green at all.
    press(&mut app, '`');
    lock_planner(&mut app);
    choose(&mut app, (x, y), (x + 13, y + 1));
    let buf = render(&mut app, 130, 42);
    assert_ne!(bg(&app, &buf, (x, y)).flatten(), green);
    assert!(!text(&buf).contains("green:"));
}

#[test]
fn the_sidebar_and_the_crumb_carry_the_preview() {
    let mut app = app();
    let (x, y) = row(&mut app);
    choose(&mut app, (x, y), (x + 10, y + 1));
    let t = text(&render(&mut app, 64, 16));
    assert!(lines(&t).last().unwrap().contains("u upgrade"), "{t}");
    let t = text(&render(&mut app, 40, 16));
    assert!(lines(&t).last().unwrap().contains("c/x/u"), "{t}");
    press(&mut app, 'u');
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains(&format!("u again: upgrade 7 for ${}", price_of_row())), "{t}");
    let t = text(&render(&mut app, 100, 16));
    let last = lines(&t).last().unwrap().to_string();
    assert!(last.starts_with(" Area › u again to upgrade 7 parts for"), "{last}");
    // Narrower, the short form, and always the way out.
    for w in [64u16, 44, 36] {
        let t = text(&render(&mut app, w, 16));
        let last = lines(&t).last().unwrap().to_string();
        assert!(last.contains("u again") && last.contains("7"), "{w}: {last}");
        assert!(last.chars().count() <= w as usize);
    }
    let t = text(&render(&mut app, 36, 16));
    assert!(lines(&t).last().unwrap().contains("Esc"), "{t}");
}

#[test]
fn rebinding_the_upgrade_key_works_and_is_what_the_line_says() {
    let mut app = app();
    let (x, y) = row(&mut app);
    app.controls = {
        let path = std::env::temp_dir().join(format!("factory-controls-upgrade-{}.json", std::process::id()));
        std::fs::write(&path, r#"{ "upgrade": ["k"] }"#).unwrap();
        let (c, _) = crate::controls::Controls::load_or_create(&path);
        std::fs::remove_file(&path).ok();
        c
    };
    choose(&mut app, (x, y), (x + 10, y + 1));
    press(&mut app, 'u');
    assert_eq!(kind_at(&app, x, y), Kind::Belt);
    press(&mut app, 'k');
    assert!(app.msg.starts_with("k again to upgrade 7 parts"), "{}", app.msg);
    press(&mut app, 'k');
    assert_eq!(kind_at(&app, x, y), Kind::FastBelt);
}
