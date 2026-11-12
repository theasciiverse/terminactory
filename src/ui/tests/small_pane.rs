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
    // The menu opens with no message in the way and all six slots and the way out show.
    key(&mut app, '1');
    let buf = render(&mut app, 36, 24);
    snapshot("phone-36x24", &buf);
    let t = text(&buf);
    let last = lines(&t).last().unwrap().to_string();
    for want in ["1DR", "2FU", "3PR", "4A1", "5PU", "6ET", "Esc"] {
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
    app.tree_pick = crate::research::node_id_for_tech(tree_rows()[tech_row("Crude assembler")]).unwrap();
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
        for n in 1..=6 {
            assert!(last.contains(&n.to_string()), "{w}x{h} slot {n}: {last}");
        }
        // Six slots fill a phone's line: the message gives way there, the slots never do.
        assert!(w < 40 || last.contains('…') || last.contains("│"), "{w}x{h}: {last}");
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
    assert!(footer - rule > 28, "page rows {}: {t}", footer - rule - 1);
    assert_no_glued_hints(&t);
    app.book = false;
    // The whole research list fits and says which group the picked row is in.
    app.tree = true;
    app.tree_pick = crate::research::node_id_for_tech(tree_rows()[tech_row("Large basic drill")]).unwrap();
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
    let list = lines(&t).iter().filter(|l| ["✓", "○", "·", "●", "◔", "◑", "◕"].iter().any(|m| l.starts_with(&format!("│ {m} ")))).count();
    assert!(list >= 3, "{list} list rows: {t}");
    assert!(t.contains("Press") && t.contains("t/Esc close"), "{t}");
    app.tree = false;
    app.book = true;
    let t = text(&render(&mut app, 64, 12));
    let rows = lines(&t);
    let rule = rows.iter().position(|l| l.contains("────────") && l.starts_with('│')).unwrap();
    assert!(rows.len() - 3 - rule > 4, "page rows: {t}");
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

/// A library with the kinds of row it can show, built in memory.
fn library_rows(app: &App) -> crate::app::Library {
    use crate::library::{Content, Entry};
    let mut kiln_world = World::blank();
    kiln_world.place(0, 10, 10, Kind::Kiln, crate::sim::Dir::E).unwrap();
    let kiln = kiln_world.capture(0, (0, 0, 30, 30)).bp;
    let demo = app.world.capture(0, (0, 0, crate::sim::W - 1, crate::sim::H - 1)).bp;
    let mut entries = vec![
        Entry { path: "a.json".into(), name: "Copper smelter row with a very long name".into(), content: Content::Ok(demo) },
        Entry { path: "b.json".into(), name: "Big kiln hall".into(), content: Content::Ok(kiln.clone()) },
        Entry { path: "c.json".into(), name: "junk".into(), content: Content::Broken("not JSON".into()) },
        Entry { path: "d.json".into(), name: "From tomorrow".into(), content: Content::Newer(9) },
    ];
    for i in 0..20 {
        entries.push(Entry { path: format!("e{i}.json").into(), name: format!("Extra {i}"), content: Content::Ok(kiln.clone()) });
    }
    let mut lib = crate::app::Library::new(entries);
    lib.pick = 1;
    lib
}

#[test]
fn overlays_fit_every_pane_size() {
    // The last two are a zoomed-out terminal (Cmd -): far more cells than a normal window.
    for (w, h) in [(36, 24), (44, 60), (64, 12), (64, 16), (80, 24), (99, 29), (120, 30), (220, 60), (300, 90)] {
        let mut app = demo();
        app.mail = Some(telegram());
        app.world.mail.push_back(telegram());
        // The History tab needs something to draw, so it gets four months of earnings.
        super::history_tab::fill(&mut app);
        // Scenes 11 to 16 are states rather than overlays: a hint and an aside on the status line,
        // the Note and Ticker telegram styles, and the small-pane view forced Never and Always.
        let empty = (0..crate::sim::W)
            .flat_map(|x| (0..crate::sim::H).map(move |y| (x, y)))
            .find(|(x, y)| app.world.tile(0, *x, *y).is_none() && app.world.floors[0].dug[idx(*x, *y)])
            .unwrap();
        for scene in 0..25 {
            app.settings.telegram_style = match scene {
                13 => crate::settings::TelegramStyle::Note,
                14 => crate::settings::TelegramStyle::Ticker,
                _ => crate::settings::TelegramStyle::Sidebar,
            };
            app.settings.small_view = match scene {
                15 => crate::settings::SmallView::Never,
                16 => crate::settings::SmallView::Always,
                _ => crate::settings::SmallView::Auto,
            };
            app.hints.current = None;
            app.cursor = empty;
            app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
            let now = std::time::Instant::now();
            if scene == 11 {
                app.hints.force(
                    crate::hints::What::Hint(crate::hints::Hint::OutOfCoal),
                    "Your drill is out of coal: put a drill on coal next to it",
                    now,
                );
            } else if scene == 12 {
                app.hints.force(crate::hints::What::Aside(crate::hints::Aside::A3), "Hollowmere: Good coal. Dad would have wept.", now);
            }
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
            app.settings_open = scene == 10;
            // Scenes 17 to 20 are the area tool: no corner yet, one corner, a rectangle chosen, and a
            // rectangle over waste that has been warned about.
            let whole = (0, 0, crate::sim::W - 1, crate::sim::H - 1);
            let stage = |s| Some(crate::app::Area { floor: 0, stage: s });
            app.area = match scene {
                17 => stage(crate::app::AreaStage::Picking { corner: None }),
                18 => stage(crate::app::AreaStage::Picking { corner: Some((0, 0)) }),
                19 | 20 | 22..=24 => stage(crate::app::AreaStage::Chosen { rect: whole }),
                _ => None,
            };
            app.confirm_area = (scene == 20).then_some((0, whole, 50.0));
            // Scenes 22 to 24 are the upgrade planner: the preview after one u, the answer to a u before
            // the tech, and the green tint with nothing pressed yet.
            if scene >= 22 {
                // Everything is researched, so the targets are open, except the planner itself in scene 23.
                app.world.progress.researched = [true; crate::sim::TECHS];
                app.world.progress.opened = [true; crate::sim::COMPANIES];
                app.world.progress.researched[crate::sim::Tech::UpgradePlanner.idx()] = scene != 23;
            }
            app.confirm_upgrade = None;
            app.area_note = None;
            // Scene 21 is the blueprint library: a readable one, one with a locked part, a long name, a
            // file that can't be read and one from a newer game, and enough more to scroll.
            app.library = (scene == 21).then(|| library_rows(&app));
            app.mail_open = scene == 5;
            app.log_open = scene == 6;
            app.away = (scene == 7).then(|| "You were away for 1h 2m. Your factory kept going:\n\n  +$5\n  +1.0 research".to_string());
            // (After the other overlays are set, so the key reaches the map.)
            if scene == 22 || scene == 23 {
                key(&mut app, 'u')
            }
            let t = text(&render(&mut app, w, h));
            assert!(lines(&t).len() == h as usize);
            if scene == 10 {
                assert!(t.contains("Settings") && t.contains("Sidebar graph") && t.contains("Esc close"), "settings at {w}x{h}: {t}");
            }
            let last = lines(&t).last().unwrap().to_string();
            let top = lines(&t)[0].to_string();
            assert!(lines(&t).iter().all(|l| l.chars().count() <= w as usize), "scene {scene} at {w}x{h}");
            match scene {
                // The hint is on the bottom line, cut to fit beside the corner note; the aside only in the full view.
                11 => assert!(last.contains("Your drill"), "hint at {w}x{h}: {last}"),
                12 => assert_eq!(last.contains("Hollowmere"), !app.compact_now, "aside at {w}x{h}: {last}"),
                // Note: the note shows in both views, and nothing in the top line.
                13 => assert!(last.contains("to view") || last.contains("mail"), "note at {w}x{h}: {last}"),
                // Ticker: the count in the top line, no note.
                14 => assert!(top.contains("mail") && !last.contains("to view"), "ticker at {w}x{h}: {t}"),
                _ => {}
            }
            if scene == 22 {
                assert!(if app.compact_now { last.contains("u again") } else { t.contains("again") }, "preview at {w}x{h}: {t}");
            }
            if scene == 24 && !app.compact_now {
                assert!(t.contains("green:") && t.contains("to upgrade"), "tint count at {w}x{h}: {t}");
            }
            if scene == 23 && app.compact_now {
                assert!(last.contains("locked"), "gate at {w}x{h}: {last}");
            }
            if (17..=20).contains(&scene) || (22..=24).contains(&scene) {
                if app.compact_now {
                    assert!(
                        last.contains("Esc") && (scene == 17 || scene == 23 || last.chars().any(|c| c.is_ascii_digit())),
                        "area scene {scene} at {w}x{h}: {last}"
                    );
                } else {
                    assert!(t.contains("AREA") && t.contains("HERE") && t.contains("Esc cancel"), "area scene {scene} at {w}x{h}: {t}");
                }
            }
            if scene == 21 && app.map_area.width >= 14 && app.map_area.height >= 4 {
                assert!(t.contains("Library") && t.contains("Esc"), "library at {w}x{h}: {t}");
            }
            if scene == 15 {
                // Never: the full view wherever it fits (46x12 and up), else the small pane.
                assert_eq!(app.compact_now, w < crate::ui::FULL_MIN_W || h < crate::ui::FULL_MIN_H, "Never at {w}x{h}");
            }
            if scene == 16 {
                assert!(app.compact_now, "Always at {w}x{h}");
            }
            if (0..=3).contains(&scene) {
                assert_footer_filled(&t, &format!("scene {scene} at {w}x{h}"));
                assert_no_glued_hints(&t);
            }
        }
    }
}

#[test]
fn build_bar_shows_every_slot_at_every_size() {
    // Transport has five slots since pipes moved there; the bar must still list them all.
    for (w, h) in [(36, 24), (44, 60), (50, 20), (64, 12), (64, 16), (80, 24), (99, 29), (120, 30), (220, 60)] {
        for cat in Category::ALL {
            let mut app = demo();
            render(&mut app, w, h);
            app.menu = Some(cat);
            let t = text(&render(&mut app, w, h));
            assert!(lines(&t).iter().all(|l| l.chars().count() <= w as usize), "{cat:?} at {w}x{h}");
            if app.compact_now {
                let last = lines(&t).last().unwrap().to_string();
                for n in 1..=slots(cat).len() {
                    assert!(last.contains(&n.to_string()), "{cat:?} slot {n} at {w}x{h}: {last}");
                }
            } else {
                for n in 1..=slots(cat).len() {
                    assert!(t.contains(&format!(" {n} ")), "{cat:?} slot {n} at {w}x{h}: {t}");
                }
            }
        }
    }
    // The way the Transport bar reads in a phone-sized pane, with pipes last.
    let mut app = demo();
    app.menu = Some(Category::Transport);
    let buf = render(&mut app, 36, 24);
    snapshot("phone-transport-36x24", &buf);
    let t = text(&buf);
    let last = lines(&t).last().unwrap().to_string();
    assert!(last.contains("5=="), "{last}");
}

#[test]
fn the_pipe_page_prints_its_rules() {
    let mut app = demo();
    app.book = true;
    app.book_go(Entry::Part(Kind::Pipe));
    for (w, h) in [(44, 60), (50, 40), (130, 42)] {
        let t = text(&render(&mut app, w, h));
        assert!(t.contains("Pipe") && t.contains("Rules"), "{w}x{h}: {t}");
        assert!(t.contains("No flow, no pressure"), "{w}x{h}: {t}");
    }
}
