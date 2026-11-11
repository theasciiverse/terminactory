//! Narrow, short and tall panes: the sizes a side pane or a phone screenshot gives. Each test
//! renders a scene, writes target/snapshot-<name>.html and asserts specific strings.

use super::*;

fn demo() -> App {
    let mut w = World::new();
    w.build_demo().unwrap();
    for _ in 0..TPS * 30 {
        w.tick();
    }
    w.econ.money = 1_234_567.0;
    let mut app = App::new(w);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    app
}

fn telegram() -> crate::mail::Message {
    crate::mail::Message {
        from: "my-project".into(),
        event: "finished".into(),
        md: "Tests pass. Ready for review.\n\n- one\n- two".into(),
        at: 0,
        gift: None,
        id: 1,
    }
}

fn key(app: &mut App, c: char) {
    app.on_key(KeyEvent::from(KeyCode::Char(c)));
}

fn lines(t: &str) -> Vec<&str> {
    t.lines().collect()
}

fn snapshot(name: &str, buf: &Buffer) {
    std::fs::write(format!("target/snapshot-{name}.html"), to_html(buf)).unwrap();
}

/// A hint overwritten by the "more" arrows shows up as a letter glued to "more".
fn assert_no_glued_hints(t: &str) {
    for l in t.lines() {
        let mut from = 0;
        while let Some(i) = l[from..].find("more") {
            let at = from + i;
            let before = l[..at].chars().next_back();
            assert!(!before.is_some_and(|c| c.is_alphabetic()), "a hint runs into 'more': {l}");
            from = at + 4;
        }
    }
}

/// Every row that ends a box (a footer) has something in it.
fn assert_footer_filled(t: &str, what: &str) {
    // The last row between a box's side borders (the box may sit among other text).
    let inner = lines(t)
        .iter()
        .rev()
        .filter_map(|l| Some(&l[l.find('│')? + '│'.len_utf8()..l.rfind('│')?]).filter(|m| !m.is_empty() && !m.contains('─')))
        .next()
        .map(|m| m.trim().to_string())
        .unwrap_or_default();
    assert!(!inner.is_empty(), "{what}: empty footer row in\n{t}");
}

fn tech_row(name: &str) -> usize {
    tree_rows().iter().position(|t| t.name() == name).unwrap_or_else(|| panic!("no tech {name}"))
}

#[test]
fn phone_36x24() {
    let mut app = demo();
    render(&mut app, 36, 24);
    // The menu opens with no message in the way and all five slots and the way out show.
    key(&mut app, '1');
    let buf = render(&mut app, 36, 24);
    snapshot("phone-36x24", &buf);
    let t = text(&buf);
    let last = lines(&t).last().unwrap().to_string();
    for want in ["1DR", "2FU", "3PR", "4A1", "5PU", "Esc"] {
        assert!(last.contains(want), "{want}: {last}");
    }
    // A paused ticker still says so, with money in short form.
    app.paused = true;
    let t = text(&render(&mut app, 36, 24));
    let top = lines(&t)[0].to_string();
    assert!(top.contains("PAUSED") && top.contains("$1.2M") && top.contains("EC"), "{top}");
    // Underground, the floor marker survives next to PAUSED.
    app.floor = app.world.floor_of(-1).unwrap();
    let t = text(&render(&mut app, 36, 24));
    let top = lines(&t)[0].to_string();
    assert!(top.contains("PAUSED") && top.contains("floor -1"), "{top}");
    app.paused = false;
    app.floor = app.world.floor_of(0).unwrap();
    // The research list keeps whole names, with a short status after them.
    app.menu = None;
    app.tree = true;
    app.tree_pick = tech_row("Crude assembler");
    let t = text(&render(&mut app, 36, 24));
    assert!(t.contains("Crude assembler ") && !t.contains("assemblerneeds"), "{t}");
    assert!(t.contains("needs") && t.contains("Assembling"), "{t}");
    assert_no_glued_hints(&t);
    app.tree = false;
    // The book's footer keeps `b/Esc close` and `/ search` beside the more arrows.
    app.book = true;
    let t = text(&render(&mut app, 36, 24));
    assert!(t.contains("/ search") && t.contains("close") && t.contains("more ▲▼"), "{t}");
    assert_no_glued_hints(&t);
}

#[test]
fn a_message_never_hides_the_build_line() {
    for (w, h) in [(36, 24), (50, 20), (64, 16), (80, 24), (99, 29)] {
        let mut app = demo();
        render(&mut app, w, h);
        key(&mut app, '1');
        // Even a long message beside the menu leaves every slot number in view.
        app.say("Researched Large advanced assembler: A 5x5 assembler for the biggest goods");
        let t = text(&render(&mut app, w, h));
        let last = lines(&t).last().unwrap().to_string();
        for n in 1..=5 {
            assert!(last.contains(&n.to_string()), "{w}x{h} slot {n}: {last}");
        }
        assert!(last.contains('…') || last.contains("│"), "{w}x{h}: {last}");
        // And with a part picked, its price stays.
        key(&mut app, '2');
        app.say("Furnace needs EC to run, and coal or ore beside it for the fire");
        let t = text(&render(&mut app, w, h));
        let last = lines(&t).last().unwrap().to_string();
        assert!(last.contains('$') && last.contains("Furn") || last.contains("FURN") || last.contains("2FU"), "{w}x{h}: {last}");
    }
}

#[test]
fn tall_44x60() {
    let mut app = demo();
    app.book = true;
    let buf = render(&mut app, 44, 60);
    snapshot("tall-44x60", &buf);
    let t = text(&buf);
    let rows = lines(&t);
    // The page gets at least half of the pane, however long the item list is.
    let rule = rows.iter().position(|l| l.contains("────────") && l.starts_with('│')).unwrap();
    let footer = rows.len() - 3;
    assert!(footer - rule - 1 >= 28, "page rows {}: {t}", footer - rule - 1);
    assert_no_glued_hints(&t);
    app.book = false;
    // The whole research list fits and says which group the picked row is in.
    app.tree = true;
    app.tree_pick = tech_row("Large basic drill");
    let t = text(&render(&mut app, 44, 60));
    assert!(t.contains("Drilling") && t.contains("Large basic drill"), "{t}");
    app.tree = false;
    // Help puts the keys first.
    app.help = true;
    let t = text(&render(&mut app, 44, 60));
    let first20 = lines(&t).iter().take(20).copied().collect::<Vec<_>>().join("\n");
    assert!(first20.contains("KEYS") && first20.contains("CONTROLS"), "{t}");
}

#[test]
fn short_64x12() {
    let mut app = demo();
    app.tree = true;
    let buf = render(&mut app, 64, 12);
    snapshot("short-64x12", &buf);
    let t = text(&buf);
    let list = lines(&t).iter().filter(|l| ["✓", "○", "·", "●"].iter().any(|m| l.starts_with(&format!("│ {m} ")))).count();
    assert!(list >= 3, "{list} list rows: {t}");
    assert!(t.contains("Press") && t.contains("t/Esc close"), "{t}");
    app.tree = false;
    app.book = true;
    let t = text(&render(&mut app, 64, 12));
    let rows = lines(&t);
    let rule = rows.iter().position(|l| l.contains("────────") && l.starts_with('│')).unwrap();
    assert!(rows.len() - 3 - rule - 1 >= 4, "page rows: {t}");
    assert_no_glued_hints(&t);
    app.book = false;
    // The production panel says when rows are hidden.
    app.stats = true;
    let t = text(&render(&mut app, 64, 12));
    assert!(t.contains("o/Esc close"), "{t}");
}

#[test]
fn classic_80x24() {
    let mut app = demo();
    app.help = true;
    let buf = render(&mut app, 80, 24);
    snapshot("classic-80x24", &buf);
    let t = text(&buf);
    assert!(t.contains("KEYS") && t.contains("any other key closes"), "{t}");
    assert!(lines(&t).iter().all(|l| l.chars().count() <= 80));
    app.help = false;
    // The away box wraps rather than being cut.
    app.away = Some("You were away for 12h 40m. Your factory kept going:\n\n  +$12345\n  +10.5 research".into());
    let t = text(&render(&mut app, 44, 24));
    assert!(t.contains("press any key") && t.contains("kept going") && t.contains("+$12345"), "{t}");
}

#[test]
fn edge_of_the_small_pane_view() {
    // 120x30 is the smallest full view: five slots in the sidebar even with a telegram waiting.
    let mut app = demo();
    app.mail = Some(telegram());
    app.menu = Some(Category::Create);
    let buf = render(&mut app, 120, 30);
    snapshot("edge-120x30", &buf);
    let t = text(&buf);
    assert!(!app.compact_now && t.contains("Factory"), "{t}");
    for n in 1..=5 {
        assert!(t.contains(&format!(" {n} ")), "slot {n}: {t}");
    }
    // One column or row less, and it's the small-pane view.
    for (w, h) in [(119, 30), (120, 29), (99, 29)] {
        let mut app = demo();
        app.mail = Some(telegram());
        let buf = render(&mut app, w, h);
        let t = text(&buf);
        assert!(app.compact_now, "{w}x{h}");
        assert!(lines(&t)[0].contains("$1.2M"), "{w}x{h}: {t}");
        let last = lines(&t).last().unwrap().to_string();
        assert!(last.contains("mail") || last.contains("to view"), "{w}x{h}: {last}");
        snapshot(&format!("edge-{w}x{h}"), &buf);
    }
}

#[test]
fn overlays_fit_every_pane_size() {
    for (w, h) in [(36, 24), (44, 60), (64, 12), (64, 16), (80, 24), (99, 29), (120, 30)] {
        let mut app = demo();
        app.mail = Some(telegram());
        app.world.mail.push_back(telegram());
        // The History tab needs something to draw, so it gets four months of earnings.
        super::history_tab::fill(&mut app);
        for scene in 0..10 {
            app.tree = scene == 0;
            app.book = scene == 1 || scene == 2;
            if scene == 2 {
                app.book_go(Entry::Part(Kind::Furnace));
            } else if scene == 1 {
                app.book_go(Entry::Item(Item::CopperOre));
            }
            app.help = scene == 3;
            app.stats = matches!(scene, 4 | 8 | 9);
            app.stats_history = matches!(scene, 8 | 9);
            // Scene 9 has no earnings at all: the empty state.
            if scene == 9 {
                app.world.history = Default::default();
            } else if app.world.history.is_empty() {
                super::history_tab::fill(&mut app);
            }
            app.mail_open = scene == 5;
            app.log_open = scene == 6;
            app.away = (scene == 7).then(|| "You were away for 1h 2m. Your factory kept going:\n\n  +$5\n  +1.0 research".to_string());
            let t = text(&render(&mut app, w, h));
            assert!(lines(&t).len() == h as usize);
            if matches!(scene, 0 | 1 | 2 | 3) {
                assert_footer_filled(&t, &format!("scene {scene} at {w}x{h}"));
                assert_no_glued_hints(&t);
            }
        }
    }
}
