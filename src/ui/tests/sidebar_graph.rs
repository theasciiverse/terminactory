//! The sidebar graph (full view): shown and hidden by the settings, and never in the way.

use super::*;
use crate::settings::{GraphRange, GraphShow};

fn snapshot(name: &str, buf: &Buffer) {
    std::fs::write(format!("target/snapshot-{name}.html"), to_html(buf)).unwrap();
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

fn with_graph(show: GraphShow, range: GraphRange) -> App {
    let mut w = World::new();
    w.build_demo().unwrap();
    let mut app = App::new(w);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    super::history_tab::fill(&mut app);
    app.settings.sidebar_graph = show;
    app.settings.sidebar_graph_range = range;
    app
}

#[test]
fn the_graph_follows_the_settings() {
    let at = |show, range| {
        let mut app = with_graph(show, range);
        let buf = render(&mut app, 130, 42);
        (text(&buf), app.graph_areas.len())
    };
    let (t, n) = at(GraphShow::Off, GraphRange::Minutes);
    assert!(!t.contains("Earnings") && !t.contains("Research ·") && n == 0, "{t}");
    let (t, n) = at(GraphShow::Money, GraphRange::Minutes);
    assert!(t.contains("Earnings · last") && t.contains(" min ") && !t.contains("Research · last") && n == 1, "{t}");
    assert!(t.contains("now") && t.contains('┤'), "{t}");
    let (t, n) = at(GraphShow::Research, GraphRange::Minutes);
    assert!(t.contains("Research · last") && !t.contains("Earnings") && n == 1, "{t}");
    let (t, n) = at(GraphShow::Both, GraphRange::Minutes);
    assert!(t.contains("Earnings · last") && t.contains("Research · last") && n == 2, "{t}");
    // The range picks the unit; the title says how many bars fit.
    let (t, _) = at(GraphShow::Money, GraphRange::Seconds);
    assert!(t.contains("Earnings · last") && t.contains(" sec "), "{t}");
    let (t, _) = at(GraphShow::Money, GraphRange::Hours);
    assert!(t.contains("Earnings · last") && t.contains(" h "), "{t}");
    // Money is drawn in painted cells.
    let mut app = with_graph(GraphShow::Money, GraphRange::Minutes);
    let buf = render(&mut app, 130, 42);
    let g = app.graph_areas[0];
    let painted =
        (g.x..g.right()).flat_map(|x| (g.y..g.bottom()).map(move |y| (x, y))).any(|(x, y)| buf.cell((x, y)).unwrap().bg == Color::Green);
    assert!(painted, "no painted bars in the graph box");
}

#[test]
fn no_graph_in_the_small_pane_view() {
    for (w, h) in [(36, 24), (64, 16), (119, 40), (120, 29)] {
        let mut app = with_graph(GraphShow::Both, GraphRange::Minutes);
        let t = text(&render(&mut app, w, h));
        assert!(app.compact_now && app.graph_areas.is_empty() && !t.contains("Earnings ·"), "{w}x{h}: {t}");
    }
}

#[test]
fn the_graph_never_overlaps_the_telegram_or_pushes_the_menu_off() {
    for (w, h) in [(120, 30), (120, 34), (130, 42), (140, 38), (160, 50), (220, 60), (300, 90)] {
        for show in [GraphShow::Money, GraphShow::Research, GraphShow::Both] {
            for (menu, mail) in [(None, false), (None, true), (Some(Category::Create), true), (Some(Category::Power), false)] {
                let mut app = with_graph(show, GraphRange::Minutes);
                app.menu = menu;
                if mail {
                    app.mail = Some(telegram());
                }
                let t = text(&render(&mut app, w, h));
                let what = format!("{w}x{h} {show:?} menu {menu:?} mail {mail}");
                assert!(!app.compact_now, "{what}");
                // The sidebar's own parts all stay.
                assert!(t.contains("HERE") && t.contains("Factory"), "{what}: {t}");
                assert!(menu.is_some() || (1..=4).all(|n| t.contains(&format!(" {n} "))), "{what}: {t}");
                if mail {
                    assert!(t.contains("From my-project") && t.contains("dismiss"), "{what}: {t}");
                }
                let side_x = (w - SIDE_W) as usize;
                let rows: Vec<Vec<char>> = t.lines().map(|l| l.chars().collect()).collect();
                let here = rows.iter().position(|l| l[side_x..].iter().collect::<String>().contains("HERE")).unwrap() as u16;
                for g in &app.graph_areas {
                    assert!(g.x as usize >= side_x && g.right() <= w && g.bottom() < h, "{what}: {g:?}");
                    assert!(g.height >= 5 && g.height <= 8, "{what}: {g:?}");
                    if mail {
                        assert!(g.bottom() <= app.mail_area.y || g.y >= app.mail_area.bottom(), "{what}: {g:?} over {:?}", app.mail_area);
                    }
                    // It sits below the build list and what's under the cursor.
                    assert!(g.y > here, "{what}: graph at {} above HERE at {here}", g.y);
                }
                for (i, a) in app.graph_areas.iter().enumerate() {
                    for b in app.graph_areas.iter().skip(i + 1) {
                        assert!(a.bottom() <= b.y || b.bottom() <= a.y, "{what}");
                    }
                }
            }
        }
    }
}

#[test]
fn a_short_sidebar_loses_the_graph_first() {
    // The smallest full view with a telegram has no room: the graph is simply not drawn.
    let mut app = with_graph(GraphShow::Both, GraphRange::Minutes);
    app.mail = Some(telegram());
    let t = text(&render(&mut app, 120, 30));
    assert!(app.graph_areas.is_empty() && !t.contains("Earnings ·"), "{t}");
    for n in 1..=4 {
        assert!(t.contains(&format!(" {n} ")), "{t}");
    }
    // As the pane grows: no chart, then money alone, then both stacked, never fewer again.
    let mut seen = Vec::new();
    for h in 30..=70 {
        let mut app = with_graph(GraphShow::Both, GraphRange::Minutes);
        app.mail = Some(telegram());
        render(&mut app, 120, h);
        seen.push(app.graph_areas.len());
    }
    assert!(seen.windows(2).all(|w| w[0] <= w[1]), "{seen:?}");
    assert!(seen.contains(&0) && seen.contains(&1) && seen.contains(&2), "{seen:?}");
    // Without the telegram, the smallest full view has room for both.
    let mut app = with_graph(GraphShow::Both, GraphRange::Minutes);
    render(&mut app, 120, 30);
    assert_eq!(app.graph_areas.len(), 2);
    // An open menu (it collapses to fit) keeps its five slots and HERE; the graph takes the rest or none.
    app.menu = Some(Category::Create);
    app.mail = Some(telegram());
    let t = text(&render(&mut app, 120, 30));
    assert!(app.graph_areas.is_empty() && t.contains("BUILD > CREATE") && t.contains("HERE"), "{t}");
    for n in 1..=5 {
        assert!(t.contains(&format!(" {n} ")), "{t}");
    }
}

#[test]
fn sidebar_graph_snapshot() {
    let mut app = with_graph(GraphShow::Both, GraphRange::Minutes);
    app.mail = Some(telegram());
    let buf = render(&mut app, 130, 50);
    snapshot("sidebar-graph", &buf);
    assert_eq!(app.graph_areas.len(), 2);
}
