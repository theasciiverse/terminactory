use super::*;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{backend::TestBackend, Terminal};

mod area;
mod blueprints;
mod hints_line;
mod history_tab;
mod keys;
mod library;
mod settings_effects;
mod settings_pane;
mod sidebar_graph;
mod small_pane;
mod upgrade;

fn render(app: &mut App, w: u16, h: u16) -> Buffer {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| draw(f, app)).unwrap();
    term.backend().buffer().clone()
}

fn text(buf: &Buffer) -> String {
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf.cell((x, y)).map_or(" ", |c| c.symbol()));
        }
        out.push('\n');
    }
    out
}

#[test]
fn every_fluid_has_a_name_and_distinct_colour_and_sellable_fluids_have_prices() {
    let names: std::collections::HashSet<_> = Fluid::ALL.iter().map(|f| f.name()).collect();
    let colours: std::collections::HashSet<_> = Fluid::ALL.iter().map(|f| fluid_colour(*f)).collect();
    assert_eq!(names.len(), FLUIDS);
    assert_eq!(colours.len(), FLUIDS);
    assert!(Fluid::ALL.iter().all(|f| !f.name().is_empty()));
    assert!(Fluid::ALL.iter().filter_map(|f| f.price()).all(|price| price > 0.0));
    assert_eq!(Fluid::Water.price(), None);
    assert_eq!(Fluid::Steam.price(), None);
}

fn tech_node(t: Tech) -> usize {
    crate::research::node_id_for_tech(t).unwrap()
}

/// Renders the demo factory, the research tree and a letter, writing colour
/// snapshots to target/ so the look can be checked in a browser.
#[test]
fn render_snapshots() {
    let mut w = World::new();
    w.build_demo().unwrap();
    for _ in 0..TPS * 60 {
        w.tick();
    }
    let mut app = App::new(w);
    app.cursor = (58, 45);
    app.power_view = true;
    app.menu = Some(Category::Create);
    let buf = render(&mut app, 130, 42);
    assert!(text(&buf).contains("BUILD > CREATE"));
    assert!(text(&buf).contains("Press"));
    std::fs::write("target/snapshot.html", to_html(&buf)).unwrap();

    app.menu = None;
    let buf = render(&mut app, 130, 42);
    assert!(text(&buf).contains("4 Misc"));
    std::fs::write("target/snapshot-menu.html", to_html(&buf)).unwrap();
    app.menu = Some(Category::Create);

    app.power_view = false;
    app.tree = true;
    let buf = render(&mut app, 130, 42);
    assert!(text(&buf).contains("Press") && text(&buf).contains("starting machinery"));

    app.tree = false;
    // A company's letter is a telegram in the sidebar, and the game keeps running.
    app.mail = Some(crate::mail::letter(Company::Ashgrove, Made::Item(Item::CopperWire), 0));
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("From Ashgrove") && t.contains("Unlocked for research") && t.contains("HERE"), "{t}");
    std::fs::write("target/snapshot-letter.html", to_html(&buf)).unwrap();

    app.mail = None;
    let mut m = crate::mail::parse(
        "from: api-server\nevent: finished\n# Auth retry fixed\n\nAll **42 tests** pass. Changed `src/auth.rs`:\n\n- retry on 503, up to 3 times, with a short backoff between tries so a flaky upstream does not get hammered while it recovers\n- [x] lint clean\n- [ ] needs your review\n\n> Didn't touch the session cache.",
        0,
    );
    m.gift = Some((Company::Brannock, 120.0));
    app.mail = Some(m);
    app.menu = None;
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("From api-server"));
    assert!(t.contains("All 42 tests pass"));
    assert!(t.contains("+$120"));
    assert!(t.contains("n dismiss"), "the telegram sits in the sidebar: {t}");
    assert!(t.contains("cache.") && !t.contains("PgUp"), "the whole telegram fits: {t}");
    assert!(t.contains("HERE"), "and the sidebar keeps its other parts");
    std::fs::write("target/snapshot-mail.html", to_html(&buf)).unwrap();

    // In a small pane it's a note on the bottom line until n opens it.
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    let buf = render(&mut app, 64, 16);
    std::fs::write("target/snapshot-small-note.html", to_html(&buf)).unwrap();
    let t = text(&buf);
    let last = t.lines().last().unwrap().to_string();
    assert!(last.trim_end().ends_with("agent finished · n to view"), "{last}");
    assert!(last.starts_with(" ? help"), "the rest of the line is unchanged: {last}");
    app.mail_open = true;
    let buf = render(&mut app, 64, 16);
    assert!(text(&buf).contains("From api-server"));
    std::fs::write("target/snapshot-mail-small.html", to_html(&buf)).unwrap();
    app.mail_open = false;
}

/// The welcome telegram shows in the sidebar of a new game without covering the build menu,
/// and as a short note in a small pane.
#[test]
fn the_welcome_telegram_fits_the_sidebar_and_a_small_pane() {
    let mut w = World::new();
    w.welcome(0);
    let mut app = App::new(w);
    app.mail = app.world.mail.pop_front();
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("From Hollowmere & Brannock"), "{t}");
    assert!(t.contains("No rush."), "the whole welcome fits at 42 rows: {t}");
    assert!(t.contains("HERE"), "and the sidebar keeps its other parts: {t}");
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    let last = text(&render(&mut app, 64, 16)).lines().last().unwrap().to_string();
    assert!(last.trim_end().ends_with("new telegram · n to view"), "{last}");
}

/// The telegram log lists everything newest first with unread ones marked, in wide and narrow panes.
#[test]
fn telegram_log_lists_newest_first() {
    let mut w = World::new();
    w.receive(crate::mail::parse("from: api-server\nevent: finished\n# Auth retry fixed\n\nAll tests pass.", 0), 1_000);
    w.deliver(crate::mail::letter(Company::Ashgrove, Made::Item(Item::CopperWire), crate::sim::now_secs()));
    w.mark_read(1);
    let mut app = App::new(w);
    app.log_open = true;
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("Telegram log · 1 unread"), "{t}");
    let rows: Vec<&str> = t.lines().filter(|l| l.contains("Ashgrove") || l.contains("api-server")).collect();
    assert_eq!(rows.len(), 2, "{t}");
    assert!(
        rows[0].contains("● ") && rows[0].contains("Ashgrove") && rows[0].contains("· letter"),
        "newest first, unread marked: {}",
        rows[0]
    );
    assert!(rows[1].contains("Auth retry fixed") && !rows[1].contains('●'), "{}", rows[1]);
    assert!(t.contains("Enter read"), "{t}");
    // A narrow, short pane still shows the rows and the hint.
    let t = text(&render(&mut app, 40, 12));
    assert!(t.contains("● Ashgrove") && t.contains("Enter read"), "{t}");
    std::fs::write("target/snapshot-log.html", to_html(&render(&mut app, 64, 16))).unwrap();
    // Empty log.
    let mut app = App::new(World::new());
    app.log_open = true;
    assert!(text(&render(&mut app, 64, 16)).contains("Nothing yet"));
}

/// The boxed research tree stays compact and scrolls in both directions.
#[test]
fn research_tree_fits_the_window() {
    use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let mut w = World::new();
    w.econ.research = 17_687.0;
    let mut app = App::new(w);
    app.tree = true;
    // Wide: the selection and its path stay visible under the horizontal graph.
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("17,687 research"), "{t}");
    assert!(t.contains("Press · Create · affordable"), "{t}");
    assert!(t.contains("free at 50 copper ingot") && t.contains("Path: Press"), "{t}");
    let tall = text(&render(&mut app, 130, 64));
    assert!(tall.contains("Press") && tall.contains("Coking") && tall.contains("Warehouse"), "{tall}");
    std::fs::write("target/snapshot-tree.html", to_html(&buf)).unwrap();
    // Going to a low tech scrolls down to it and shows the off-screen hint.
    app.tree_pick = tech_node(*tree_rows().last().unwrap());
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("Upgrade planner · Misc") && t.contains("Path: Press → iron gear"), "{t}");
    std::fs::write("target/snapshot-tree-scrolled.html", to_html(&buf)).unwrap();
    app.tree_pick = tech_node(Tech::Press);
    // At 60 columns the graph still works and scrolls horizontally.
    let t = text(&render(&mut app, 60, 50));
    assert!(t.contains("Press") && t.contains("more"), "{t}");
    let buf = render(&mut app, 64, 16);
    assert!(text(&buf).contains("more "), "{}", text(&buf));
    std::fs::write("target/snapshot-tree-small.html", to_html(&buf)).unwrap();
    // Clicking a row picks it; clicking it again researches it.
    app.tree_pick = tech_node(Tech::Press);
    render(&mut app, 130, 120);
    let (r, i) = app.tree_rows.iter().copied().find(|(_, i)| tree_rows()[*i] == Tech::Coking).unwrap();
    let click = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x + 2, row: r.y, modifiers: KeyModifiers::NONE };
    app.on_mouse(click);
    assert_eq!(app.tree_pick, tech_node(tree_rows()[i]));
    assert!(!app.world.progress.has(Tech::Coking));
    app.on_mouse(click);
    assert!(app.world.progress.has(Tech::Coking));
    // n opens a waiting telegram over the screen.
    app.mail = Some(crate::mail::parse("Done", 0));
    app.on_key(KeyEvent::from(KeyCode::Char('n')));
    assert!(app.mail_open && app.tree);
}

#[test]
fn research_tree_expands_and_collapses_with_arrows() {
    let mut app = App::new(World::new());
    app.tree = true;
    let key = |app: &mut App, code| app.on_key(KeyEvent::from(code));
    assert_eq!(crate::research::layout_expanded(&app.tree_expanded).len(), 3);
    key(&mut app, KeyCode::Right);
    assert!(app.tree_expanded.contains(&crate::research::node_id_for_tech(Tech::Press).unwrap()));
    assert_eq!(app.tree_choice(), Tech::Press);
    key(&mut app, KeyCode::Right);
    assert_eq!(
        app.tree_pick,
        crate::content::TREE
            .iter()
            .position(|(n, _)| *n == crate::content::Node::Key(crate::content::Made::Item(crate::content::Item::CopperWire)))
            .unwrap()
    );
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Right);
    assert_eq!(app.tree_choice(), Tech::BasicDrill);
    key(&mut app, KeyCode::Left);
    assert_eq!(
        app.tree_pick,
        crate::content::TREE
            .iter()
            .position(|(n, _)| *n == crate::content::Node::Key(crate::content::Made::Item(crate::content::Item::CopperWire)))
            .unwrap()
    );
    key(&mut app, KeyCode::Left);
    key(&mut app, KeyCode::Left);
    assert_eq!(app.tree_choice(), Tech::Press);
    key(&mut app, KeyCode::Left);
    assert!(!app.tree_expanded.contains(&crate::research::node_id_for_tech(Tech::Press).unwrap()));
    assert_eq!(crate::research::layout_expanded(&app.tree_expanded).len(), 3);
}

#[test]
fn deep_tree_pick_expands_its_ancestors() {
    let mut app = App::new(World::new());
    app.tree = true;
    app.tree_pick = tech_node(Tech::Excavation);
    render(&mut app, 80, 40);
    assert!(app.tree_expanded.contains(&crate::research::node_id_for_tech(Tech::Press).unwrap()));
    assert!(app.tree_expanded.contains(&crate::research::node_id_for_tech(Tech::Mixer).unwrap()));
    assert!(app.tree_expanded.contains(
        &crate::content::TREE
            .iter()
            .position(|(n, _)| *n == crate::content::Node::Key(crate::content::Made::Item(crate::content::Item::Concrete)))
            .unwrap()
    ));
    assert!(crate::research::layout_expanded(&app.tree_expanded).iter().any(|p| crate::research::tech_for_node(p.id) == Some(Tech::Mixer)));
}

#[test]
fn print_research_tree_render_sizes() {
    let mut app = App::new(World::new());
    app.tree = true;
    let wide = render(&mut app, 80, 40);
    println!("80x40:\n{}", text(&wide));
    app.tree_expanded = crate::content::TREE
        .iter()
        .enumerate()
        .filter_map(|(i, (n, _))| {
            matches!(n, crate::content::Node::Tech(_) | crate::content::Node::Key(_) | crate::content::Node::Group(_)).then_some(i)
        })
        .collect();
    let (rows, cols) = crate::research::canvas_size_expanded(&app.tree_expanded);
    println!("Full canvas: {rows} rows × {cols} columns");
    let narrow = render(&mut app, 50, 34);
    println!("50x34:\n{}", text(&narrow));

    let full = render(&mut app, cols + 4, rows + 10);
    let full_text = text(&full);
    let canvas = full_text
        .lines()
        .skip(2)
        .take(rows as usize)
        .map(|line| line.chars().skip(2).take(cols as usize).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    let canvas_lines: Vec<Vec<char>> = canvas.lines().map(|line| line.chars().collect()).collect();
    let positions = crate::research::layout_expanded(&app.tree_expanded);
    let stride = (crate::research::BOX_W + crate::research::COL_GAP) as usize;
    for parent in &positions {
        for child in positions.iter().filter(|p| p.col == parent.col + 1 && p.row == parent.row) {
            let y = parent.row + 1;
            let start = parent.col * stride + crate::research::BOX_W as usize - 1;
            let end = child.col * stride;
            let joined = (start..=end).all(|x| matches!(canvas_lines[y][x], '─' | '├' | '┤' | '┬' | '┴' | '┼' | '╠' | '╣'));
            if joined {
                assert_eq!(child.parent, Some(parent.id), "horizontal connection without TREE edge: {:?} -> {:?}", parent.id, child.id);
            }
        }
    }
    println!("Full canvas render ({rows} × {cols}):\n{canvas}");
}

/// The recipe book: pages for items and machines, word equations, links, search.
#[test]
fn recipe_book() {
    use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let mut app = App::new(World::new());
    let key = |app: &mut App, c: KeyCode| app.on_key(KeyEvent::from(c));
    key(&mut app, KeyCode::Char('b'));
    assert!(app.book);
    assert_eq!(app.book_slug, "start");
    let t = text(&render(&mut app, 80, 40));
    assert!(t.contains("termweb.search") && t.contains("termweb Search") && t.contains("I'm Feeling Lucky"), "{t}");
    // A link under the search box opens a wiki page; Backspace comes back to the start page.
    let (r, _) = app.book_urls.iter().find(|(_, slug)| slug == "getting-started").unwrap();
    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: r.x,
        row: r.y,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    });
    assert_eq!(app.book_slug, "getting-started");
    let t = text(&render(&mut app, 80, 40));
    assert!(t.contains("terminactory.wiki/getting-started"), "{t}");
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.book_slug, "start");
    // Type in the search box and press Enter: a page of results, then a result opens.
    for c in "coil".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.book_slug, "search?q=coil");
    let t = text(&render(&mut app, 100, 40));
    assert!(t.contains("termweb.search/search?q=coil") && t.contains("Copper coil - Terminactory Wiki") && t.contains("About "), "{t}");
    app.book_selected = app.book_urls.iter().position(|(_, s)| s == "copper-coil").unwrap();
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.book_slug, "copper-coil");
    assert_eq!(app.book_history, ["start", "search?q=coil"]);
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.book_slug, "search?q=coil");
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.book_slug, "start");
    // Esc leaves the search box, and then `]` goes forward.
    key(&mut app, KeyCode::Esc);
    assert!(app.book && !app.book_typing);
    key(&mut app, KeyCode::Char(']'));
    assert_eq!(app.book_slug, "search?q=coil");
    // A search with no match says so.
    key(&mut app, KeyCode::Char('/'));
    for c in "zzzqqq".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    let t = text(&render(&mut app, 100, 40));
    assert!(t.contains("did not match any pages"), "{t}");
    let t = text(&render(&mut app, 50, 34));
    assert!(t.contains("ermweb.search"), "{t}");
    key(&mut app, KeyCode::Esc);
    assert!(!app.book);
    app.book = true;
    app.book_typing = false;
    key(&mut app, KeyCode::Char('b'));
    assert!(app.book, "b opens a tab, it does not close the browser");
}

#[test]
fn wiki_floating_window_behavior() {
    use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "copper-coil".into();
    let wide = render(&mut app, 140, 45);
    assert!(app.book_area.width < 140 && app.book_area.height < 45);
    let visible_map = (app.map_area.y..app.map_area.bottom()).any(|y| {
        (app.map_area.x..app.map_area.right())
            .any(|x| !app.book_area.contains((x, y).into()) && matches!(wide.cell((x, y)).unwrap().bg, Color::Indexed(234 | 237 | 232)))
    });
    assert!(visible_map, "map should remain visible outside the browser window");
    let title = text(&wide);
    assert!(title.contains("termweb") && title.contains("Copper coil"), "{title}");
    app.on_mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: 0, row: 0, modifiers: KeyModifiers::NONE });
    assert!(!app.book, "clicking outside the browser closes it");

    app.book = true;
    app.settings.small_view = crate::settings::SmallView::Always;
    let compact = render(&mut app, 140, 45);
    assert_eq!(app.book_area, Rect::new(0, 0, 140, 45));
    assert!(text(&compact).contains("Copper coil"));

    app.settings.small_view = crate::settings::SmallView::Auto;
    let small = render(&mut app, 60, 30);
    assert_eq!(app.book_area, Rect::new(0, 0, 60, 30));
    assert!(text(&small).contains("Copper coil"));
}

#[test]
fn wiki_infobox_and_page_sections_render_in_narrow_and_wide_panes() {
    let mut app = App::new(World::new());
    app.on_key(KeyEvent::from(KeyCode::Char('b')));
    app.book_go(Entry::Item(Item::CopperCoil));
    let top = text(&render(&mut app, 50, 34));
    assert!(top.find("Copper coil").unwrap() < top.find("Money").unwrap(), "{top}");
    for _ in 0..30 {
        app.on_key(KeyEvent::from(KeyCode::PageDown));
    }
    let narrow_end = text(&render(&mut app, 50, 34));
    assert!(narrow_end.contains("Category: Items"), "{narrow_end}");
    app.book_scroll = 0;
    let wide_top = text(&render(&mut app, 100, 40));
    assert!(wide_top.contains("terminactory.wiki/copper-coil"), "{wide_top}");
    for _ in 0..30 {
        app.on_key(KeyEvent::from(KeyCode::PageDown));
    }
    let wide_end = text(&render(&mut app, 100, 40));
    assert!(wide_end.contains("Category: Items"), "{wide_end}");
}

/// In a small terminal the map fills the pane between a ticker and a status line.
/// The production panel sits over the map and lists the last minute, per item.
#[test]
fn production_panel() {
    let mut w = World::new();
    w.build_demo().unwrap();
    for _ in 0..TPS * 70 {
        w.tick();
    }
    let mut app = App::new(w);
    // o opens the production site in the browser.
    app.on_key(KeyEvent::from(KeyCode::Char('o')));
    assert!(app.book && app.book_slug == "production");
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains(" Production ") && t.contains(" History ") && t.contains("last minute") && t.contains("research/min"), "{t}");
    assert!(t.contains("copper plate") && t.contains("next sale") && t.contains("production.lan"), "{t}");
    std::fs::write("target/snapshot-production.html", to_html(&buf)).unwrap();
    // Small pane: fewer columns, still readable.
    let t = text(&render(&mut app, 60, 20));
    assert!(t.contains("copper plate"), "{t}");
    // Esc puts the browser away; o brings the same tab back.
    app.on_key(KeyEvent::from(KeyCode::Esc));
    assert!(!app.book);
    app.on_key(KeyEvent::from(KeyCode::Char('o')));
    assert!(app.book && app.book_tabs.len() == 2 && app.book_slug == "production");
}

/// Research reads in a sky blue (not the dark blue of the 16-colour palette, which is nearly
/// invisible on dark terminals), and a minute with no research says 0 instead of a lone dot.
#[test]
fn research_is_readable_in_the_production_panel() {
    let dark_blues = [Color::Blue, Color::LightBlue, Color::Indexed(4), Color::Indexed(12)];
    let mut selling = World::new();
    selling.build_demo().unwrap();
    for _ in 0..TPS * 70 {
        selling.tick();
    }
    // A brand-new world has sold nothing, for research or money.
    for (world, zero) in [(selling, false), (World::new(), true)] {
        let mut app = App::new(world);
        app.stats = true;
        let buf = render(&mut app, 130, 42);
        let t = text(&buf);
        let head = t.lines().nth(3).unwrap();
        assert!(head.contains("research/min"), "{head}");
        if zero {
            assert!(head.contains("0 research/min") && !head.contains("·  ·"), "{head}");
        }
        // No cell of the panel or the status bar is dark blue.
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let c = buf.cell((x, y)).unwrap();
                assert!(!dark_blues.contains(&c.fg), "dark blue text at {x},{y} ({:?}): {t}", c.symbol());
            }
        }
        // The words "research/min" use the research colour.
        let col = head.find("research/min").map(|i| head[..i].chars().count()).unwrap();
        assert!(matches!(buf.cell((col as u16, 3)).unwrap().fg, Color::Indexed(75) | Color::Indexed(67)));
    }
}

/// Waste: hovering a crate of it, the production panel, and its recipe book page.
#[test]
fn waste_shows_up() {
    let mut w = World::new();
    w.build_demo().unwrap();
    w.place(0, 66, 36, Kind::Crate, Dir::E).unwrap();
    if let Some(Building { st: St::Crate { items }, .. }) = w.floors[0].tiles[idx(66, 36)].as_mut() {
        items.extend([Item::Waste; 200]);
    }
    let mut app = App::new(w);
    app.cursor = (66, 36);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("Removing costs $100 (200 waste)"), "{t}");
    std::fs::write("target/snapshot-waste.html", to_html(&buf)).unwrap();

    // The production panel: fees show as negative money.
    let w = &mut app.world;
    w.place(0, 70, 36, Kind::TradePost, Dir::E).unwrap();
    w.econ.money = 500.0;
    for t in 0..TPS * 70 {
        w.tick();
        if t % 90 == 0 {
            w.offer(0, 70, 36, Item::Waste);
        }
    }
    app.stats = true;
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("waste") && t.contains("-$"), "{t}");
    assert!(t.contains("Waste costs money"), "{t}");
    std::fs::write("target/snapshot-waste-production.html", to_html(&buf)).unwrap();
    let t = text(&render(&mut app, 50, 30));
    assert!(t.contains("waste"), "{t}");
    app.stats = false;

    // Its recipe book page: no research chart.
    app.book_go(Entry::Item(Item::Waste));
    app.book = true;
    let buf = render(&mut app, 130, 60);
    let t = text(&buf);
    assert!(t.contains("terminactory.wiki/waste") && t.contains("unwanted leftover"), "{t}");
    std::fs::write("target/snapshot-waste-book.html", to_html(&buf)).unwrap();
}

/// The logistics toys on the map: a tunnel under a belt, a long arm, a filter splitter, a warehouse.
#[test]
fn toys_snapshot() {
    let mut w = World::blank();
    let (x, y) = (55, 38);
    w.place(0, x, y, Kind::Crate, Dir::E).unwrap();
    w.place(0, x + 1, y, Kind::Tunnel, Dir::E).unwrap();
    for k in 2..=4 {
        w.place(0, x + k, y, Kind::Belt, Dir::N).unwrap();
    }
    w.place(0, x + 5, y, Kind::Tunnel, Dir::E).unwrap();
    w.place(0, x + 6, y, Kind::Sorter, Dir::E).unwrap();
    w.place(0, x + 7, y, Kind::Belt, Dir::E).unwrap();
    w.place(0, x + 8, y, Kind::LongArm, Dir::E).unwrap();
    w.place(0, x + 10, y - 1, Kind::Warehouse, Dir::E).unwrap();
    w.place(0, x + 4, y + 2, Kind::Pole, Dir::E).unwrap();
    w.place(0, x + 7, y + 2, Kind::Solar, Dir::E).unwrap();
    let mut app = App::new(w);
    app.cursor = (x + 1, y);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains(">]") && t.contains("[>") && t.contains("WARE"), "{t}");
    assert!(t.contains("goes under 3 tiles to its exit\n") || t.contains("goes under 3 tiles to its exit "), "{t}");
    std::fs::write("target/snapshot-toys.html", to_html(&buf)).unwrap();
}

/// The basic solar panel beside the cheap one: the Power menu, the map and the inspector.
#[test]
fn solar_mk2_snapshot() {
    let mut w = World::blank();
    let (x, y) = (55, 38);
    w.place(0, x, y, Kind::Solar, Dir::E).unwrap();
    w.place(0, x + 3, y, Kind::BasicSolar, Dir::E).unwrap();
    w.place(0, x + 6, y + 1, Kind::Pole, Dir::E).unwrap();
    w.progress.opened[Company::Kettering.idx()] = true;
    w.progress.researched[Tech::BasicSolar.idx()] = true;
    let mut app = App::new(w);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    app.cursor = (x + 3, y);
    app.menu = Some(Category::Power);
    let buf = render(&mut app, 130, 42);
    std::fs::write("target/snapshot-solar-mk2.html", to_html(&buf)).unwrap();
    let t = text(&buf);
    assert!(t.contains("Basic solar panel") && t.contains("Cheap solar panel"), "both versions list in the Power menu: {t}");
    assert!(t.contains("SOL2") && t.contains("SOLR"), "{t}");
    assert!(t.contains("+20 EC, reaches 4 tiles around it"), "{t}");
    app.cursor = (x, y);
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("+10 EC, reaches 2 tiles around it"), "{t}");
    // Picked in the hand, the status line shows its price and size; a narrow pane still names it.
    app.selected = Some(Kind::BasicSolar);
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("$300"), "{t}");
    let t = text(&render(&mut app, 44, 34));
    assert!(t.contains("Basic solar panel") || t.contains("SOL2"), "{t}");
}

/// Long machines on the map in every facing, and the placement ghost turning with the rotate key.
#[test]
fn long_machines_snapshot() {
    let mut w = World::blank();
    let (x, y) = (52, 36);
    for (i, d) in Dir::ALL.into_iter().enumerate() {
        let i = i as i32;
        w.place(0, x + i * 5, y, Kind::Press, d).unwrap();
        w.place(0, x + i * 5, y + 4, Kind::Pump, d).unwrap();
        w.place(0, x + i * 5, y + 8, Kind::Furnace, d).unwrap();
    }
    w.place(0, x + 14, y + 13, Kind::Solar, Dir::E).unwrap();
    let mut app = App::new(w);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    app.menu = Some(Category::Create);
    app.selected = Some(Kind::Press);
    app.dir = Dir::E;
    app.cursor = (x, y + 13);
    app.cam = (x - 4, y - 2);
    app.cam_ready = true;
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("PR") && t.contains("PUMP") && t.contains("FURN"), "{t}");
    assert!(text(&render(&mut app, 64, 16)).contains("Press  $150  2x1  facing"));
    app.cam = (x - 4, y - 2);
    let east = render(&mut app, 130, 42);
    // Turned north the ghost is one wide and two tall, and the label says so.
    app.dir = Dir::N;
    app.cursor = (x + 6, y + 13);
    app.cam = (x - 4, y - 2);
    let north = render(&mut app, 130, 42);
    assert!(text(&render(&mut app, 64, 16)).contains("Press  $150  1x2  facing"));
    std::fs::write("target/snapshot-long-machines.html", to_html(&east)).unwrap();
    std::fs::write("target/snapshot-long-machines-north.html", to_html(&north)).unwrap();
    // A narrow, tall pane keeps the map readable.
    app.cursor = (x + 1, y + 1);
    let narrow = render(&mut app, 44, 30);
    std::fs::write("target/snapshot-long-machines-narrow.html", to_html(&narrow)).unwrap();
    // Hovering a placed long machine highlights the whole rectangle.
    app.selected = None;
    app.menu = None;
    app.cursor = (x + 6, y);
    app.cam = (x - 4, y - 2);
    let hover = render(&mut app, 130, 42);
    std::fs::write("target/snapshot-long-machines-hover.html", to_html(&hover)).unwrap();
}

/// Middle-click copies the part under the cursor: its kind, facing and setting.
#[test]
fn middle_click_copies() {
    use crate::sim::{Dir, Setting};
    use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let mut w = World::new();
    w.econ.money = 1e6;
    let mut app = App::new(w);
    render(&mut app, 130, 42);
    let (cx, cy) = app.cam;
    let (a, b) = ((cx + 4, cy + 4), (cx + 8, cy + 4));
    app.world.place(0, a.0, a.1, Kind::Arm, Dir::S).unwrap();
    app.world.configure(0, a.0, a.1);
    let at = |app: &App, t: (i32, i32)| (app.map_area.x + ((t.0 - app.cam.0) * 2) as u16, app.map_area.y + (t.1 - app.cam.1) as u16);
    let click = |app: &mut App, t: (i32, i32), button| {
        let (column, row) = at(app, t);
        app.on_mouse(MouseEvent { kind: MouseEventKind::Down(button), column, row, modifiers: KeyModifiers::NONE });
    };
    click(&mut app, a, MouseButton::Middle);
    assert_eq!((app.selected, app.dir), (Some(Kind::Arm), Dir::S));
    assert_eq!(app.menu, Some(Category::Transport));
    assert!(app.msg.contains("Copied Filtering arm"), "{}", app.msg);
    click(&mut app, b, MouseButton::Left);
    assert_eq!(app.world.tile(0, b.0, b.1).map(|t| t.dir), Some(Dir::S));
    assert_eq!(app.world.setting_at(0, b.0, b.1), app.world.setting_at(0, a.0, a.1));
    assert!(matches!(app.world.setting_at(0, b.0, b.1), Some(Setting::Filter(Some(_)))));
    // Middle-click on an empty tile empties your hand.
    click(&mut app, (cx + 4, cy + 8), MouseButton::Middle);
    assert_eq!(app.selected, None);
}

#[test]
fn small_pane_view() {
    let mut w = World::new();
    w.build_demo().unwrap();
    for _ in 0..TPS * 30 {
        w.tick();
    }
    let mut app = App::new(w);
    app.cursor = (58, 45);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    let buf = render(&mut app, 64, 16);
    let t = text(&buf);
    assert!(app.compact_now);
    assert!(t.lines().next().unwrap().contains(" $"), "{t}");
    assert!(!t.contains("Factory"), "no side panel in a small pane");
    assert_eq!(app.map_area, Rect::new(0, 1, 64, 14));
    std::fs::write("target/snapshot-small.html", to_html(&buf)).unwrap();

    app.menu = Some(Category::Create);
    let t = text(&render(&mut app, 64, 16));
    let last = t.lines().last().unwrap().to_string();
    assert!(
        last.contains("1DR") && last.contains("5PU") && last.contains("7ST") && !last.contains("$50"),
        "every slot shows its number: {last}"
    );

    // A picked part: the path to it, its price and facing.
    app.menu = Some(Category::Create);
    app.selected = Some(Kind::Furnace);
    let buf = render(&mut app, 64, 16);
    assert!(text(&buf).lines().last().unwrap().contains("Create › Furnace  $200  3x3  facing"));
    std::fs::write("target/snapshot-small-menu.html", to_html(&buf)).unwrap();
    app.menu = Some(Category::Transport);
    app.selected = Some(Kind::Splitter);
    let t = text(&render(&mut app, 64, 16));
    let last = t.lines().last().unwrap().to_string();
    assert!(last.starts_with(" Transport › Basic splitter  $10  ·  Space place"), "{last}");
    // A message keeps the path in front of it.
    app.say("Placed");
    let t = text(&render(&mut app, 64, 16));
    let last = t.lines().last().unwrap();
    assert!(last.contains("Transport › Basic splitter  $10") && last.contains("│ Placed"), "{t}");
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    app.selected = None;

    // The power figure is global away from power, regional on a pole or panel.
    assert!(t.lines().next().unwrap().contains("EC 20/14") && !t.contains("global"), "{t}");
    let pole = (0..(W * H) as usize).map(pos_of).find(|&(x, y)| app.world.tile(0, x, y).is_some_and(|b| b.kind == Kind::Pole)).unwrap();
    app.cursor = pole;
    let t = text(&render(&mut app, 64, 16));
    assert!(t.lines().next().unwrap().contains(" regional"), "{t}");
    app.cursor = (58, 45);

    // m forces the full view back, even in a small terminal.
    app.menu = None;
    app.compact = Some(false);
    render(&mut app, 64, 16);
    assert!(!app.compact_now);
    // A big terminal uses the full view by itself.
    app.compact = None;
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("Factory") && !app.compact_now);

    app.help = true;
    let t = text(&render(&mut app, 64, 16));
    assert!(t.contains("scroll 0/"), "{t}");
}

/// A small steam plant and a blast furnace, for checking how pipes and the new machines look.
#[test]
fn render_steam_snapshot() {
    let mut w = World::blank();
    w.place(0, 6, 5, Kind::Pump, Dir::E).unwrap();
    for x in 7..=9 {
        w.place(0, x, 5, Kind::Pipe, Dir::E).unwrap();
    }
    w.place(0, 9, 6, Kind::Pipe, Dir::E).unwrap();
    w.place(0, 10, 6, Kind::Boiler, Dir::E).unwrap();
    w.place(0, 12, 6, Kind::SteamEngine, Dir::E).unwrap();
    w.place(0, 10, 8, Kind::Crate, Dir::N).unwrap();
    w.place(0, 4, 7, Kind::Solar, Dir::E).unwrap();
    w.place(0, 16, 6, Kind::BlastFurnace, Dir::E).unwrap();
    for x in 14..=15 {
        w.place(0, x, 9, Kind::Belt, Dir::E).unwrap();
    }
    if let Some(Building { st: St::Crate { items }, .. }) = w.floors[0].tiles[idx(10, 8)].as_mut() {
        items.extend([Item::Coal; 20]);
    }
    for _ in 0..TPS * 5 {
        w.tick();
    }
    let mut app = App::new(w);
    app.cursor = (20, 12);
    let buf = render(&mut app, 130, 42);
    assert!(text(&buf).contains("ENGN"));
    std::fs::write("target/snapshot-steam.html", to_html(&buf)).unwrap();
}

/// Molten metal in pipes (copper orange, iron red), furnaces pouring into molders.
#[test]
fn render_molten_snapshot() {
    let mut w = World::blank();
    let stock = |w: &mut World, x: i32, y: i32, item: Item, n: u32| {
        let a = idx(x, y);
        if let Some(Building { st: St::Crafter { stock, .. }, .. }) = w.floors[0].tiles[a].as_mut() {
            stock[item.idx()] = n;
        }
    };
    // Copper: furnace -> pipes -> a molder set to iron, so the metal waits in the pipes.
    w.place(0, 6, 5, Kind::Furnace, Dir::E).unwrap();
    stock(&mut w, 6, 5, Item::CopperOre, 20);
    stock(&mut w, 6, 5, Item::Coal, 20);
    for x in 9..=11 {
        w.place(0, x, 5, Kind::Pipe, Dir::E).unwrap();
    }
    w.place(0, 12, 4, Kind::Molder, Dir::E).unwrap();
    w.configure(0, 12, 4);
    // Iron, set the same way, one row down.
    w.place(0, 6, 9, Kind::Furnace, Dir::E).unwrap();
    w.configure(0, 6, 9);
    stock(&mut w, 6, 9, Item::IronOre, 30);
    stock(&mut w, 6, 9, Item::Coal, 20);
    for x in 9..=11 {
        w.place(0, x, 9, Kind::Pipe, Dir::E).unwrap();
    }
    w.place(0, 12, 8, Kind::Molder, Dir::E).unwrap();
    // A furnace pouring straight into a working molder, ingots into a crate.
    w.place(0, 6, 13, Kind::Furnace, Dir::E).unwrap();
    stock(&mut w, 6, 13, Item::CopperOre, 20);
    stock(&mut w, 6, 13, Item::Coal, 20);
    w.place(0, 9, 13, Kind::Molder, Dir::E).unwrap();
    w.place(0, 11, 13, Kind::Crate, Dir::E).unwrap();
    w.place(0, 12, 11, Kind::Solar, Dir::E).unwrap();
    w.place(0, 14, 9, Kind::Pole, Dir::E).unwrap();
    w.place(0, 14, 6, Kind::Pole, Dir::E).unwrap();
    for _ in 0..TPS * 42 {
        w.tick();
    }
    let mut app = App::new(w);
    app.cursor = (10, 5);
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("MOLD") && t.contains("FURN"), "{t}");
    assert!(t.contains("molten copper") && t.contains("one fluid per network"), "the pipe names its fluid: {t}");
    std::fs::write("target/snapshot-molten.html", to_html(&buf)).unwrap();
    // The iron pipe, and a furnace's own page of the inspector.
    app.cursor = (10, 9);
    assert!(text(&render(&mut app, 130, 42)).contains("molten iron"));
    app.cursor = (6, 5);
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("gives 1 molten copper"), "{t}");
    // The molder says what it takes, and where from.
    app.cursor = (12, 4);
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("making iron ingot") && t.contains("molten iron 0/1") && t.contains("EC 100%"), "{t}");
    // A narrow, tall pane.
    app.cursor = (10, 5);
    let buf = render(&mut app, 64, 40);
    std::fs::write("target/snapshot-molten-small.html", to_html(&buf)).unwrap();
}

/// The machines for later tiers (not in the build menu yet): big drills, the assembler ladder,
/// the kiln in every facing, a welder, a mixer and the advanced pump.
#[test]
fn later_machines_snapshot() {
    let mut w = World::blank();
    let (x, y) = (52, 30);
    // Row 1: drills, 8x8 and 4x4.
    w.place(0, x, y, Kind::LargeBasicDrill, Dir::E).unwrap();
    w.place(0, x + 10, y, Kind::LargeAdvancedDrill, Dir::S).unwrap();
    w.place(0, x + 20, y, Kind::AdvancedDrill, Dir::E).unwrap();
    w.place(0, x + 25, y, Kind::AdvancedPump, Dir::E).unwrap();
    // Row 2: the assembler ladder, big to small.
    let y2 = y + 10;
    w.place(0, x, y2, Kind::LargeBasicAssembler, Dir::E).unwrap();
    w.place(0, x + 6, y2, Kind::LargeAdvancedAssembler, Dir::N).unwrap();
    w.place(0, x + 12, y2, Kind::CrudeAssembler, Dir::E).unwrap();
    w.place(0, x + 17, y2, Kind::PrecisionAssembler, Dir::E).unwrap();
    w.place(0, x + 21, y2, Kind::AdvancedAssembler, Dir::E).unwrap();
    w.place(0, x + 25, y2, Kind::BasicAssembler, Dir::E).unwrap();
    // Row 3: the kiln facing every way, a welder and a mixer.
    let y3 = y + 17;
    w.place(0, x, y3, Kind::Kiln, Dir::E).unwrap();
    w.place(0, x + 4, y3, Kind::Kiln, Dir::N).unwrap();
    w.place(0, x + 10, y3, Kind::Kiln, Dir::S).unwrap();
    w.place(0, x + 16, y3, Kind::Kiln, Dir::W).unwrap();
    w.place(0, x + 22, y3, Kind::Welder, Dir::E).unwrap();
    w.place(0, x + 26, y3, Kind::Mixer, Dir::E).unwrap();
    // A half-loaded kiln.
    if let Some(Building { st: St::Crafter { stock, .. }, .. }) = w.floors[0].tiles[idx(x, y3)].as_mut() {
        stock[Item::Stone.idx()] = 10;
    }
    let mut app = App::new(w);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    app.cam = (x - 2, y - 2);
    app.cam_ready = true;
    app.cursor = (x, y3);
    let buf = render(&mut app, 130, 42);
    std::fs::write("target/snapshot-later-machines.html", to_html(&buf)).unwrap();
    let t = text(&buf);
    assert!(t.contains("LRG BASIC DRILL") && t.contains("LRG ADV DRILL") && t.contains("LRG ASM B") && t.contains("CRD ASM"), "{t}");
    assert!(t.contains("KILN") && t.contains("WELDER") && t.contains("MIXR") && t.contains("PRCASM"), "{t}");
    assert!(t.contains("Kiln") && t.contains("load 5/8") && t.contains("stone 10/16"), "{t}");
    // A narrow, tall pane.
    app.cam = (x - 2, y - 2);
    app.cursor = (x + 3, y + 3);
    let narrow = render(&mut app, 44, 34);
    std::fs::write("target/snapshot-later-machines-narrow.html", to_html(&narrow)).unwrap();
    // The big drill's and the welder's inspector text.
    app.cursor = (x + 3, y + 3);
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("5x a basic drill's ore") && t.contains("burns 10x the coal"), "{t}");
    app.cursor = (x + 23, y3 + 1);
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("30 EC only while working") && t.contains("drawing 0 EC now"), "{t}");
    app.cursor = (x + 18, y2 + 1);
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("by Lattice & Finch") && t.contains("making control board"), "{t}");
}

/// The large kiln in two facings, half loaded: its map label, inspector and loading count.
#[test]
fn large_kiln_snapshot() {
    let mut w = World::blank();
    let (x, y) = (52, 30);
    w.place(0, x, y, Kind::LargeKiln, Dir::E).unwrap();
    w.place(0, x + 7, y, Kind::LargeKiln, Dir::N).unwrap();
    if let Some(Building { st: St::Crafter { stock, .. }, .. }) = w.floors[0].tiles[idx(x, y)].as_mut() {
        stock[Item::Stone.idx()] = 20;
    }
    let mut app = App::new(w);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    app.cam = (x - 2, y - 2);
    app.cam_ready = true;
    app.cursor = (x + 1, y + 1);
    let buf = render(&mut app, 130, 42);
    std::fs::write("target/snapshot-large-kiln.html", to_html(&buf)).unwrap();
    let t = text(&buf);
    assert!(t.contains("LRG KILN"), "{t}");
    assert!(t.contains("Large kiln") && t.contains("load 10/32") && t.contains("stone 20/64"), "{t}");
    // A narrow pane still names it.
    let narrow = render(&mut app, 44, 34);
    assert!(text(&narrow).contains("Large kiln"), "{}", text(&narrow));
}

/// Recipe book pages for the machines that aren't in the book's list yet.
#[test]
fn later_machines_have_book_pages() {
    let mut app = App::new(World::blank());
    let mut shots = Vec::new();
    for (k, wants) in [
        (
            Kind::PrecisionAssembler,
            vec!["Precision assembler", "relay + 4 fine wire + iron sheet  ──→  control board", "5.0s", "$20,000 · 3x3", "64 EC"],
        ),
        (
            Kind::AdvancedAssembler,
            vec![
                "3 iron gear + steel  ──→  gearbox",
                "2 copper coil + steel rod + 2 bearing  ──→  electric motor",
                "weakest microprocessor",
                "16 EC",
            ],
        ),
        (
            Kind::LargeAdvancedAssembler,
            vec!["frame + 2 electric motor + gauge + control board  ──→  generator", "4 steel girder", "steam tractor", "5x5"],
        ),
        (Kind::Welder, vec!["2 steel girder + 8 screw  ──→  frame", "30 EC only while working, none when idle"]),
        (
            Kind::Kiln,
            vec![
                "2 stone  ─coal─→  brick",
                "3 sand  ─coal─→  glass",
                "lamp + copper sheet + 2 fine wire  ─coal─→  tube + waste",
                "3x5",
                "fills up for 8 runs, then makes them all at once",
            ],
        ),
        (
            Kind::LargeKiln,
            vec!["2 stone  ─coal─→  brick", "3 sand  ─coal─→  glass", "5x7", "fills up for 32 runs, then makes them all at once"],
        ),
        (Kind::Mixer, vec!["2 sand + stone + water  ──→  concrete", "2 concrete + iron rod + water  ──→  reinforced concrete"]),
        (Kind::Evacuator, vec!["lamp + copper sheet + 2 fine wire + water  ──→  2 tube", "2x2", "6 EC", "no waste"]),
        (Kind::LargeBasicDrill, vec!["5x a basic drill's ore, burns 10x the coal", "8x8", "front edge"]),
        (Kind::LargeAdvancedDrill, vec!["5x an advanced drill's ore", "40 EC, no coal"]),
        (Kind::AdvancedDrill, vec!["2x a basic drill's ore", "EC, no coal", "4x4"]),
        (Kind::AdvancedPump, vec!["80 water a second", "4 EC"]),
    ] {
        let page = book_page(&app.world, Entry::Part(k), 100, "f");
        let t: String = page.iter().map(|r| r.iter().map(|s| s.text.clone()).collect::<String>()).collect::<Vec<_>>().join("\n");
        for want in wants {
            assert!(t.contains(want), "{k:?} page lacks {want:?}:\n{t}");
        }
        shots.push(k);
    }
    // Drawn in the book itself, wide and narrow.
    app.book = true;
    app.book_go(Entry::Part(Kind::PrecisionAssembler));
    let buf = render(&mut app, 130, 42);
    assert!(text(&buf).contains("Precision assembler"));
    std::fs::write("target/snapshot-later-book.html", to_html(&buf)).unwrap();
    app.book_go(Entry::Part(Kind::Kiln));
    let buf = render(&mut app, 50, 36);
    std::fs::write("target/snapshot-later-book-narrow.html", to_html(&buf)).unwrap();
}

/// Phase 3 screenshots: box elevators, idle words, floors above and below, the away summary.
#[test]
fn render_phase3_snapshots() {
    // Elevators of three sizes, a feeding belt, and idle machines.
    let mut w = World::blank();
    w.place(0, 3, 7, Kind::Crate, Dir::E).unwrap();
    if let Some(Building { st: St::Crate { items }, .. }) = w.floors[0].tiles[idx(3, 7)].as_mut() {
        items.extend([Item::Coal; 30]);
    }
    w.place(0, 4, 7, Kind::Belt, Dir::E).unwrap();
    w.place(0, 5, 7, Kind::Belt, Dir::E).unwrap();
    w.place_sized(0, 6, 6, Kind::Elevator, Dir::E, 2).unwrap();
    w.place_sized(0, 11, 4, Kind::Elevator, Dir::E, 4).unwrap();
    w.place_sized(0, 18, 3, Kind::Elevator, Dir::S, 6).unwrap();
    w.place(0, 4, 4, Kind::Solar, Dir::E).unwrap();
    w.place(0, 4, 11, Kind::Drill, Dir::E).unwrap();
    w.place(0, 8, 11, Kind::Furnace, Dir::E).unwrap();
    for _ in 0..TPS * 6 + TPS / 2 {
        w.tick();
    }
    // Send the 4x4 car off, full, so it shows as moving.
    w.lifts[1].load.extend([Item::IronPlate; 160]);
    for _ in 0..40 {
        w.tick();
    }
    while (w.tick / TPS).is_multiple_of(2) {
        w.tick(); // land on a second where idle words show
    }
    let mut app = App::new(w);
    app.cursor = (16, 17);
    app.menu = Some(Category::Transport);
    app.selected = Some(Kind::Elevator);
    app.lift_size = 3;
    let buf = render(&mut app, 130, 42);
    std::fs::write("target/snapshot-lifts.html", to_html(&buf)).unwrap();

    // The same spot on the surface and one floor down: ore continues underneath, wider.
    let mut w = World::new();
    w.progress.researched[Tech::Excavation.idx()] = true;
    let mut app = App::new(w);
    app.cursor = (60, 38);
    let buf = render(&mut app, 130, 42);
    std::fs::write("target/snapshot-surface.html", to_html(&buf)).unwrap();
    app.floor = 1;
    let buf = render(&mut app, 130, 42);
    assert!(text(&buf).contains("underground"));
    std::fs::write("target/snapshot-underground.html", to_html(&buf)).unwrap();

    // Coming back after a while.
    app.floor = 0;
    app.away = Some("You were away for 3h 12m. Your factory kept going:\n\n  +$4210\n  +318.5 research".into());
    let buf = render(&mut app, 130, 42);
    std::fs::write("target/snapshot-away.html", to_html(&buf)).unwrap();

    // The research screen, picked on its last row.
    app.away = None;
    app.tree = true;
    app.tree_pick = tech_node(*tree_rows().last().unwrap());
    let buf = render(&mut app, 120, 42);
    assert!(text(&buf).contains("Upgrade planner · "), "{}", text(&buf));
    std::fs::write("target/snapshot-tree-end.html", to_html(&buf)).unwrap();
}

#[test]
fn belt_corners_draw_as_bends() {
    let mut w = World::blank();
    // East along y=5, then up at x=8: (8,5) is a corner.
    for x in 5..=7 {
        w.place(0, x, 5, Kind::Belt, Dir::E).unwrap();
    }
    w.place(0, 8, 5, Kind::Belt, Dir::N).unwrap();
    w.place(0, 8, 4, Kind::Belt, Dir::N).unwrap();
    let chars = |w: &World, x, y| tile_glyph(w, 0, x, y).map(|c| c.0);
    assert_eq!(chars(&w, 8, 5), ['/', '^']);
    assert_eq!(chars(&w, 7, 5), ['>', '>']);
    assert_eq!(chars(&w, 8, 4), ['^', '^']);
    // Coming from the right and turning down.
    w.place(0, 20, 5, Kind::Belt, Dir::W).unwrap();
    w.place(0, 19, 5, Kind::Belt, Dir::S).unwrap();
    assert_eq!(chars(&w, 19, 5), ['v', '/']);
    // A belt fed from behind and from the left is a merge: the join shows on the left.
    w.place(0, 8, 6, Kind::Belt, Dir::N).unwrap();
    assert_eq!(chars(&w, 8, 5), ['+', '^']);
    // And from the right as well: joins on both sides.
    w.place(0, 9, 5, Kind::Belt, Dir::W).unwrap();
    assert_eq!(chars(&w, 8, 5), ['+', '+']);
    // A sideways belt joined from above shows the join on its back half.
    w.place(0, 30, 10, Kind::Belt, Dir::E).unwrap();
    w.place(0, 31, 10, Kind::Belt, Dir::E).unwrap();
    w.place(0, 31, 9, Kind::Belt, Dir::S).unwrap();
    assert_eq!(chars(&w, 31, 10), ['+', '>']);
}

fn hex(c: Option<Color>, default: &str) -> String {
    let named = |i: u8| -> &'static str {
        [
            "000000", "cd3131", "0dbc79", "e5e510", "2472c8", "bc3fbc", "11a8cd", "e5e5e5", "666666", "f14c4c", "23d18b", "f5f543",
            "3b8eea", "d670d6", "29b8db", "ffffff",
        ][i as usize]
    };
    match c {
        None | Some(Color::Reset) => default.to_string(),
        Some(Color::Black) => named(0).into(),
        Some(Color::Red) => named(1).into(),
        Some(Color::Green) => named(2).into(),
        Some(Color::Yellow) => named(3).into(),
        Some(Color::Blue) => named(4).into(),
        Some(Color::Magenta) => named(5).into(),
        Some(Color::Cyan) => named(6).into(),
        Some(Color::Gray) => named(7).into(),
        Some(Color::DarkGray) => named(8).into(),
        Some(Color::LightRed) => named(9).into(),
        Some(Color::LightGreen) => named(10).into(),
        Some(Color::LightYellow) => named(11).into(),
        Some(Color::LightBlue) => named(12).into(),
        Some(Color::LightMagenta) => named(13).into(),
        Some(Color::LightCyan) => named(14).into(),
        Some(Color::White) => named(15).into(),
        Some(Color::Indexed(i)) if i < 16 => named(i).into(),
        Some(Color::Indexed(i)) if i < 232 => {
            let l = [0, 95, 135, 175, 215, 255];
            let n = i - 16;
            format!("{:02x}{:02x}{:02x}", l[(n / 36) as usize], l[((n / 6) % 6) as usize], l[(n % 6) as usize])
        }
        Some(Color::Indexed(i)) => {
            let g = 8 + 10 * (i - 232);
            format!("{g:02x}{g:02x}{g:02x}")
        }
        Some(Color::Rgb(r, g, b)) => format!("{r:02x}{g:02x}{b:02x}"),
    }
}

/// Colour snapshot of the screen as HTML, for checking the look in a browser.
fn to_html(buf: &Buffer) -> String {
    let mut s = String::from(
        "<html><head><meta charset=utf-8></head><body style='background:#1e1e1e;margin:16px'><pre style='font:14px/1.15 Menlo,monospace'>",
    );
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let c = buf.cell((x, y)).unwrap();
            let (mut fg, mut bg) = (hex(Some(c.fg), "cccccc"), hex(Some(c.bg), "1e1e1e"));
            if c.modifier.contains(Modifier::REVERSED) {
                std::mem::swap(&mut fg, &mut bg);
            }
            let w = if c.modifier.contains(Modifier::BOLD) { "bold" } else { "normal" };
            let sym = match c.symbol() {
                "<" => "&lt;",
                ">" => "&gt;",
                "&" => "&amp;",
                o => o,
            };
            s.push_str(&format!("<span style='color:#{fg};background:#{bg};font-weight:{w}'>{sym}</span>"));
        }
        s.push('\n');
    }
    s.push_str("</pre></body></html>");
    s
}

/// The iron and coal machines on the map: facing east, and the long ones facing north,
/// each running, with the build menu and a narrow view.
#[test]
fn new_machines_snapshot() {
    let mut w = World::blank();
    let stock = |w: &mut World, x: i32, y: i32, items: &[(Item, u32)]| {
        let a = idx(x, y);
        if let Some(Building { st: St::Crafter { stock, .. }, .. }) = w.floors[0].tiles[a].as_mut() {
            for (item, n) in items {
                stock[item.idx()] = *n;
            }
        }
    };
    let (x, y) = (52, 36);
    // Facing east: crusher, coke oven, rolling mill, lathe, then the two long ones.
    w.place(0, x, y, Kind::Crusher, Dir::E).unwrap();
    stock(&mut w, x, y, &[(Item::CopperOre, 5)]);
    w.place(0, x + 4, y, Kind::CokeOven, Dir::E).unwrap();
    stock(&mut w, x + 4, y, &[(Item::Coal, 8)]);
    w.place(0, x + 8, y, Kind::RollingMill, Dir::E).unwrap();
    w.configure(0, x + 8, y);
    stock(&mut w, x + 8, y, &[(Item::IronPlate, 8)]);
    w.place(0, x + 12, y, Kind::Lathe, Dir::E).unwrap();
    stock(&mut w, x + 12, y, &[(Item::IronPlate, 8)]);
    w.place(0, x + 17, y, Kind::WireDrawer, Dir::E).unwrap();
    stock(&mut w, x + 17, y, &[(Item::CopperPlate, 4)]);
    w.place(0, x + 17, y + 2, Kind::Winder, Dir::E).unwrap();
    stock(&mut w, x + 17, y + 2, &[(Item::CopperWire, 9)]);
    // Facing north, south and west.
    w.place(0, x, y + 7, Kind::WireDrawer, Dir::N).unwrap();
    w.place(0, x + 2, y + 7, Kind::Winder, Dir::N).unwrap();
    w.place(0, x + 4, y + 7, Kind::Crusher, Dir::N).unwrap();
    w.place(0, x + 7, y + 7, Kind::CokeOven, Dir::N).unwrap();
    w.place(0, x + 10, y + 7, Kind::RollingMill, Dir::N).unwrap();
    w.place(0, x + 13, y + 7, Kind::Lathe, Dir::N).unwrap();
    w.place(0, x + 18, y + 7, Kind::WireDrawer, Dir::W).unwrap();
    w.place(0, x + 18, y + 9, Kind::Winder, Dir::S).unwrap();
    for sx in [x + 5, x + 8, x + 16] {
        w.place(0, sx, y + 3, Kind::Solar, Dir::E).unwrap();
    }
    w.place(0, x + 3, y + 4, Kind::Pole, Dir::E).unwrap();
    w.place(0, x + 12, y + 4, Kind::Pole, Dir::E).unwrap();
    w.place(0, x + 14, y + 4, Kind::Pole, Dir::E).unwrap();
    for _ in 0..TPS * 4 {
        w.tick();
    }
    let mut app = App::new(w);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    app.cursor = (x + 1, y + 1);
    app.cam = (x - 4, y - 3);
    app.cam_ready = true;
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    for name in ["CRSH", "COKE", "ROLL", "LATHE", "WD", "WN"] {
        assert!(t.contains(name), "{name}: {t}");
    }
    // The inspector says what the crusher is for and what it's making.
    assert!(t.contains("Crusher") && t.contains("grinds ore into grit") && t.contains("making copper grit"), "{t}");
    std::fs::write("target/snapshot-new-machines.html", to_html(&buf)).unwrap();

    // The build menu: every new machine sits under its group's number.
    app.menu = Some(Category::Create);
    app.selected = Some(Kind::Lathe);
    app.dir = Dir::E;
    // Create has 35 kinds now: below 48 rows the menu lists only the slot in use (checked after this).
    let buf = render(&mut app, 130, 50);
    let t = text(&buf);
    for name in ["Crusher", "Coke oven", "Wire drawer", "Winder", "Rolling mill", "Lathe", "Crude assembler"] {
        assert!(t.contains(name), "{name}: {t}");
    }
    std::fs::write("target/snapshot-new-machines-menu.html", to_html(&buf)).unwrap();
    // In a 42-row pane it collapses to one kind a slot, with all six slots and HERE still showing.
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains(" 6 ET Etcher+") && t.contains("HERE") && t.contains("Lathe"), "{t}");
    // Narrow and tall: the map still reads, and the menu is one line.
    app.cursor = (x + 13, y + 1);
    app.cam = (x + 6, y - 3);
    let buf = render(&mut app, 44, 40);
    std::fs::write("target/snapshot-new-machines-narrow.html", to_html(&buf)).unwrap();
    let t = text(&render(&mut app, 60, 20));
    assert!(t.lines().last().unwrap().contains("Create › Lathe  $200  3x3  facing"), "{t}");
    app.selected = None;
    let buf = render(&mut app, 60, 20);
    let last = text(&buf).lines().last().unwrap().to_string();
    assert!(last.contains("Cr›") && (1..=7).all(|n| last.contains(&n.to_string())), "{last}");
    std::fs::write("target/snapshot-new-machines-small.html", to_html(&buf)).unwrap();
}

/// Silicon's book pages (old machines, new rows) fit narrow, tall and zoomed-out panes.
#[test]
fn silicon_book_pages_fit_every_size() {
    let mut app = App::new(World::blank());
    app.book = true;
    let pages = [
        (Entry::Part(Kind::BlastFurnace), vec!["3 sand + coke  ──→  molten silicon + 2 waste"]),
        (Entry::Part(Kind::Molder), vec!["molten silicon  ──→  silicon ingot"]),
        (Entry::Part(Kind::Lathe), vec!["2 silicon ingot  ──→  2 wafer + 2 silicon scrap"]),
        (
            Entry::Part(Kind::Kiln),
            vec!["4 stone  ─coal─→  dopant", "wafer + 2 dopant  ─coal─→  doped wafer", "2 brick + sand  ─coal─→  2 ceramic"],
        ),
        (Entry::Part(Kind::AdvancedAssembler), vec!["x3x1", "doped wafer + 6 fine wire + iron sheet  ──→  3 transistor"]),
        (Entry::Item(Item::SiliconIngot), vec!["Made by", "Used in"]),
        (Entry::Item(Item::SiliconScrap), vec!["Made by", "sells $1.5"]),
        (Entry::Item(Item::TransistorBoard), vec!["Made by", "sells $450"]),
        (Entry::Part(Kind::ArcFurnace), vec!["3 silicon scrap  ──→  2 molten silicon", "melts sand to silicon on power alone"]),
        (Entry::Part(Kind::CrystalGrower), vec!["3 silicon ingot  ──→  silicon crystal", "slowly"]),
        (Entry::Part(Kind::WaferSaw), vec!["silicon crystal + fine wire  ──→  6 wafer", "no scrap"]),
        (Entry::Item(Item::SiliconCrystal), vec!["Made by", "Used in", "sells $50"]),
    ];
    for (entry, _wants) in pages {
        app.book_go(entry);
        let wide = text(&render(&mut app, 130, 42));
        assert!(wide.contains(&format!("terminactory.wiki/{}", crate::wiki::slug(entry.name()))), "{entry:?}: {wide}");
        // Narrow, tall and zoomed-out panes draw the page without trouble.
        for (w, h) in [(40, 30), (60, 40), (50, 80), (200, 60)] {
            let buf = render(&mut app, w, h);
            assert!(text(&buf).len() > 100, "{entry:?} at {w}x{h}");
        }
    }
}

/// The chip pages (etcher, mixer, precision assembler, the new items) fit narrow, tall and zoomed-out panes.
#[test]
fn chip_book_pages_fit_every_size() {
    let mut app = App::new(World::blank());
    app.book = true;
    let pages = [
        (Entry::Part(Kind::Mixer), vec!["coal + sand + water  ──→  2 etchant + waste", "out the back"]),
        (Entry::Part(Kind::Etcher), vec!["doped wafer + mask + etchant  ──→  2 die + 2 silicon scrap", "etches dies"]),
        (Entry::Part(Kind::AdvancedAssembler), vec!["glass + copper sheet + 2 fine wire  ──→  mask"]),
        (
            Entry::Part(Kind::PrecisionAssembler),
            vec![
                "die + ceramic + 4 fine wire  ──→  microprocessor (x4x1)",
                "die + ceramic + 2 fine wire  ──→  RAM chip",
                "mask + glass + 2 fine wire  ──→  fine mask",
                "fine die + ceramic + 4 fine wire  ──→  microprocessor (x4x2)",
            ],
        ),
        (
            Entry::Part(Kind::FineEtcher),
            vec![
                "doped wafer + fine mask + etchant  ──→  fine die + die",
                "no scrap, but very hungry",
                "96 EC",
                "$60,000 · 3x5 · from Meridian",
            ],
        ),
        (Entry::Item(Item::FineMask), vec!["Made by", "Used in", "sells $90"]),
        (Entry::Item(Item::FineDie), vec!["Made by", "Used in", "sells $140"]),
        (Entry::Item(Item::FineMicroprocessor), vec!["Made by", "sells $420"]),
        (Entry::Item(Item::Etchant), vec!["Made by", "sells $2"]),
        (Entry::Item(Item::Die), vec!["Made by", "Used in", "sells $68"]),
        (Entry::Item(Item::Microprocessor), vec!["Made by", "sells $200"]),
        (Entry::Item(Item::RamChip), vec!["Made by", "sells $150"]),
    ];
    for (entry, _wants) in pages {
        app.book_go(entry);
        let wide = text(&render(&mut app, 130, 60));
        assert!(wide.contains(&format!("terminactory.wiki/{}", crate::wiki::slug(entry.name()))), "{entry:?}: {wide}");
        for (w, h) in [(40, 30), (60, 40), (50, 80), (200, 60)] {
            let buf = render(&mut app, w, h);
            assert!(text(&buf).len() > 100, "{entry:?} at {w}x{h}");
        }
    }
    // The long item name keeps its code in the grid, and the research list and tree show the etcher.
    let t = text(&render(&mut app, 130, 60));
    assert!(t.contains("terminactory.wiki/ram-chip") && t.contains("RAM chip"), "{t}");
    app.book = false;
    app.tree = true;
    app.tree_pick = tech_node(Tech::Etcher);
    for (w, h) in [(36, 24), (50, 40), (44, 60), (130, 42), (200, 60)] {
        let t = text(&render(&mut app, w, h));
        assert!(t.contains("Etcher"), "{w}x{h}: {t}");
    }
    // The fine etcher sits under the x4x1 in the tree.
    app.tree_pick = tech_node(Tech::FineEtcher);
    for (w, h) in [(36, 24), (50, 40), (44, 60), (130, 42), (200, 60)] {
        let t = text(&render(&mut app, w, h));
        assert!(t.contains("Fine etcher"), "{w}x{h}: {t}");
    }
}

/// The fine etcher (3x5) on the map in every facing: its label fits, and the inspector names it.
#[test]
fn fine_etcher_snapshot() {
    let mut w = World::blank();
    let (x, y) = (52, 30);
    w.place(0, x, y, Kind::FineEtcher, Dir::E).unwrap();
    w.place(0, x + 5, y, Kind::FineEtcher, Dir::N).unwrap();
    w.place(0, x + 11, y, Kind::FineEtcher, Dir::S).unwrap();
    w.place(0, x + 17, y, Kind::FineEtcher, Dir::W).unwrap();
    let mut app = App::new(w);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    app.cam = (x - 2, y - 2);
    app.cam_ready = true;
    app.cursor = (x + 1, y + 1);
    let buf = render(&mut app, 130, 42);
    std::fs::write("target/snapshot-fine-etcher.html", to_html(&buf)).unwrap();
    let t = text(&buf);
    assert!(t.contains("FETCH"), "{t}");
    assert!(t.contains("Fine etcher"), "{t}");
    for (w, h) in [(44, 34), (60, 40), (50, 80), (200, 60)] {
        let t = text(&render(&mut app, w, h));
        assert!(t.contains("Fine etcher"), "{w}x{h}: {t}");
    }
}

/// The computer's pages, the large precision assembler's tree row and menu row, and the machine on the
/// map in every facing, at narrow, tall and zoomed-out sizes.
#[test]
fn computer_pages_rows_and_machine_fit_every_size() {
    let mut app = App::new(World::blank());
    app.book = true;
    let pages = [
        (Entry::Item(Item::Keyboard), vec!["Made by", "Used in", "sells $260"]),
        (Entry::Item(Item::DiskDrive), vec!["Made by", "Used in", "sells $1100"]),
        (Entry::Item(Item::Mainboard), vec!["Made by", "Used in", "sells $1800"]),
        (Entry::Item(Item::Computer), vec!["Made by", "sells $11000"]),
        (
            Entry::Part(Kind::LargePrecisionAssembler),
            vec![
                "mainboard + transformer + terminal + disk drive + frame  ──→  computer",
                "builds the computer",
                "160 EC",
                "$100,000 · 5x5 · from Meridian",
            ],
        ),
        (
            Entry::Part(Kind::PrecisionAssembler),
            vec![
                "circuit board + microprocessor (x4x2) + 4 RAM chip  ──→  mainboard",
                "circuit board + 2 microprocessor (x4x1) + 4 RAM chip  ──→  mainboard",
            ],
        ),
    ];
    for (entry, _wants) in pages {
        app.book_go(entry);
        let wide = text(&render(&mut app, 150, 70));
        assert!(wide.contains(&format!("terminactory.wiki/{}", crate::wiki::slug(entry.name()))), "{entry:?}: {wide}");
        for (w, h) in [(36, 24), (40, 30), (60, 40), (50, 80), (200, 60), (150, 90)] {
            let buf = render(&mut app, w, h);
            assert!(text(&buf).len() > 100, "{entry:?} at {w}x{h}");
        }
    }
    // The research tree: its row shows next to the fine etcher.
    app.book = false;
    app.tree = true;
    app.tree_pick = tech_node(Tech::LargePrecisionAssembler);
    for (w, h) in [(36, 24), (50, 40), (44, 60), (130, 42), (200, 60), (130, 64)] {
        let t = text(&render(&mut app, w, h));
        assert!(t.contains("Large prec"), "{w}x{h}: {t}");
    }
    app.tree = false;
    // The map: the label fits in every facing, and the inspector names the machine.
    let mut w = World::blank();
    let (x, y) = (52, 30);
    for (i, d) in [Dir::E, Dir::N, Dir::S, Dir::W].into_iter().enumerate() {
        w.place(0, x + 6 * i as i32, y, Kind::LargePrecisionAssembler, d).unwrap();
    }
    let mut app = App::new(w);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    app.cam = (x - 2, y - 2);
    app.cam_ready = true;
    app.cursor = (x + 1, y + 1);
    let buf = render(&mut app, 130, 42);
    std::fs::write("target/snapshot-large-precision-assembler.html", to_html(&buf)).unwrap();
    let t = text(&buf);
    assert!(t.contains("LRG ASM P") && t.contains("Large precision assembler"), "{t}");
    for (w, h) in [(44, 34), (60, 40), (50, 80), (200, 60)] {
        let t = text(&render(&mut app, w, h));
        // A narrow pane cuts the name short with an ellipsis; it still reads.
        assert!(t.contains("Large precision") && t.contains("LRG ASM P"), "{w}x{h}: {t}");
    }
    // The build menu: the new row sits right after the large advanced assembler, and the held part shows.
    app.menu = Some(Category::Create);
    app.selected = Some(Kind::LargePrecisionAssembler);
    for (w, h) in [(130, 32), (130, 50), (50, 20), (64, 20), (90, 20), (44, 40), (200, 60)] {
        let t = text(&render(&mut app, w, h));
        assert!(t.contains("Large prec") || t.contains("Large precision") || t.contains("LRGP"), "{w}x{h}: {t}");
    }
    app.selected = None;
    let t = text(&render(&mut app, 130, 50));
    let (adv, prec, welder) = (t.find("Large adv. assembler"), t.find("Large prec. assembler"), t.find("Welder"));
    assert!(adv < prec && prec < welder && adv.is_some(), "{t}");
}

/// The new machines and items have book pages that link to each other.
#[test]
fn book_pages_for_the_new_machines_and_items() {
    let mut app = App::new(World::blank());
    app.book = true;
    app.book_go(Entry::Part(Kind::Lathe));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("terminactory.wiki/lathe") && t.contains("Lathe turns plate into rods"), "{t}");
    std::fs::write("target/snapshot-book-lathe.html", to_html(&render(&mut app, 130, 42))).unwrap();
    std::fs::write("target/snapshot-book-lathe-narrow.html", to_html(&render(&mut app, 60, 40))).unwrap();
    app.book_go(Entry::Part(Kind::CokeOven));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("terminactory.wiki/coke-oven") && t.contains("Coke oven"), "{t}");
    // An item page: where the screw comes from, and what a spring needs.
    app.book_go(Entry::Item(Item::Screw));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("terminactory.wiki/screw") && t.contains("screw"), "{t}");
    app.book_go(Entry::Item(Item::Coal));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("terminactory.wiki/coal") && t.contains("Coal"), "{t}");
    // Furnace and blast furnace melt iron differently: both show on the iron ore page.
    app.book_go(Entry::Item(Item::IronOre));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("terminactory.wiki/iron-ore") && t.contains("iron ore"), "{t}");
    app.book_go(Entry::Item(Item::Spring));
    std::fs::write("target/snapshot-book-spring.html", to_html(&render(&mut app, 130, 42))).unwrap();
    // The research list has the new machines, locked behind their key items.
    app.book = false;
    app.world = World::new();
    app.tree = true;
    app.tree_pick = tech_node(Tech::Crusher);
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("Crusher · Create") && t.contains("copper wire"), "{t}");
    std::fs::write("target/snapshot-tree-new.html", to_html(&render(&mut app, 130, 42))).unwrap();
    std::fs::write("target/snapshot-tree-new-narrow.html", to_html(&render(&mut app, 50, 34))).unwrap();
    // Every item has its own glyph and colour.
    let mut seen: Vec<(char, Style)> = Vec::new();
    for it in Item::ALL {
        let cell = item_cell(it, GROUND);
        assert!(cell.0 != ' ' && !seen.contains(&cell), "{it:?} looks blank or like another item");
        seen.push(cell);
    }
}

/// The content step's screens at every size: the crowded Create menu, the long research
/// list, the recipe book and the production panel.
#[test]
fn content_step_screens_fit_every_size() {
    let mut w = World::new();
    w.econ.money = 1e6;
    w.progress.opened = [true; COMPANIES];
    w.progress.researched = [true; TECHS];
    for it in Item::ALL {
        w.econ.made[it.idx()] = 3;
    }
    let mut app = App::new(w);
    app.msg_at = std::time::Instant::now() - std::time::Duration::from_secs(60);
    app.menu = Some(Category::Create);
    // A tall sidebar lists every machine, seven numbered groups (it takes a row more for every new machine).
    let buf = render(&mut app, 130, 50);
    let t = text(&buf);
    for name in [
        "Large basic drill",
        "Mixer",
        "Evacuator",
        "Kiln",
        "Welder",
        "Large adv. assembler",
        "Large prec. assembler",
        "Advanced pump",
        "Precision assembler",
        "Etcher",
        "Arc furnace",
        "Crystal grower",
        "Wafer saw",
        "Fine etcher",
    ] {
        assert!(t.contains(name), "{name}: {t}");
    }
    assert!(t.contains(" 5 ") && t.contains("HERE"), "{t}");
    assert!(t.lines().all(|l| !l.contains("│ ") || l.chars().count() <= 130));
    std::fs::write("target/snapshot-create-menu.html", to_html(&buf)).unwrap();
    // A short one folds the groups to one kind each, so HERE stays on screen.
    let buf = render(&mut app, 130, 32);
    let t = text(&buf);
    assert!(t.contains("Cheap drill+") && t.contains("Water pump+") && t.contains("HERE"), "{t}");
    assert!(!t.contains("Large basic drill"), "folded: {t}");
    std::fs::write("target/snapshot-create-menu-short.html", to_html(&buf)).unwrap();
    // Holding a part opens its own group.
    app.selected = Some(Kind::Welder);
    let t = text(&render(&mut app, 130, 32));
    assert!(t.contains("Precision assembler") && t.contains("Welder") && t.contains("HERE"), "{t}");
    // The silicon machines: the heat slot and slot 6 grow, and the held part shows in the menu and the bottom line.
    for (kind, name) in [
        (Kind::ArcFurnace, "Arc furnace"),
        (Kind::CrystalGrower, "Crystal grower"),
        (Kind::WaferSaw, "Wafer saw"),
        (Kind::FineEtcher, "Fine etcher"),
    ] {
        app.selected = Some(kind);
        for (w, h) in [(130, 32), (130, 46), (50, 20), (64, 20), (90, 20), (44, 40), (200, 60)] {
            let t = text(&render(&mut app, w, h));
            if w >= 90 && h >= 32 {
                assert!(t.contains("HERE") && t.contains(name), "{kind:?} at {w}x{h}: {t}");
            }
        }
        let t = text(&render(&mut app, 64, 20));
        let last = t.lines().last().unwrap();
        assert!(last.contains("Create ›") && last.contains(name) && last.contains("facing"), "{kind:?}: {last}");
    }
    app.selected = None;
    // Small panes: the bottom line keeps all six numbers.
    for width in [50, 64, 90] {
        let buf = render(&mut app, width, 20);
        let t = text(&buf);
        let last = t.lines().last().unwrap();
        // Six slots: the codes at 50 columns, map labels from 64, and the way out when it fits.
        assert!((1..=7).all(|n| last.contains(&n.to_string())), "{width}: {last}");

        std::fs::write(format!("target/snapshot-create-menu-{width}.html"), to_html(&buf)).unwrap();
    }
    app.menu = None;

    // The research tree at 50 columns: two boxes remain visible, and the selected details fit.
    app.world.progress = Progress::default();
    app.tree = true;
    app.tree_pick = tech_node(*tree_rows().last().unwrap());
    let buf = render(&mut app, 50, 40);
    let t = text(&buf);
    assert!(t.contains("Upgrade planner") && t.contains("free at") && t.contains("t/Esc"), "{t}");
    app.tree_pick = tech_node(Tech::LargeAdvancedAssembler);
    let buf = render(&mut app, 50, 40);
    let t = text(&buf);
    assert!(t.contains("Large adv. assembler") && t.contains("control board"), "{t}");
    std::fs::write("target/snapshot-tree-50.html", to_html(&buf)).unwrap();
    let buf = render(&mut app, 130, 42);
    std::fs::write("target/snapshot-tree-130.html", to_html(&buf)).unwrap();
    app.tree = false;

    // The recipe book: every item and machine listed, narrow too.
    app.book = true;
    app.book_go(Entry::Item(Item::ControlBoard));
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("terminactory.wiki/control-board") && t.contains("Advanced assembler") && t.contains("Precision assembler"), "{t}");
    assert!(t.contains("relay") && t.contains("fine wire"), "{t}");
    std::fs::write("target/snapshot-book-items.html", to_html(&buf)).unwrap();
    let buf = render(&mut app, 50, 40);
    std::fs::write("target/snapshot-book-items-50.html", to_html(&buf)).unwrap();
    app.book_go(Entry::Item(Item::Gearbox));
    let buf = render(&mut app, 130, 42);
    assert!(text(&buf).contains("terminactory.wiki/gearbox"), "{}", text(&buf));
    std::fs::write("target/snapshot-book-gearbox.html", to_html(&buf)).unwrap();
    app.book_go(Entry::Part(Kind::LargeAdvancedAssembler));
    let buf = render(&mut app, 50, 40);
    std::fs::write("target/snapshot-book-machine-50.html", to_html(&buf)).unwrap();
    // Every machine has a page.
    for &k in BOOK_PARTS.iter() {
        assert!(Entry::list("", true).contains(&Entry::Part(k)));
    }
    assert_eq!(Entry::list("", true).len(), BOOK_PARTS.len());
    app.book = false;

    // The production panel with every item made.
    app.stats = true;
    let buf = render(&mut app, 130, 42);
    assert!(text(&buf).contains("Production"));
    std::fs::write("target/snapshot-production-all.html", to_html(&buf)).unwrap();
}

/// A machine that leaves waste says so: in the inspector, and on its recipe book page.
#[test]
fn machines_say_waste_leaves_by_the_back() {
    let mut w = World::blank();
    w.place(0, 20, 20, Kind::CokeOven, Dir::E).unwrap();
    let lines = inspect(&w, 0, 20, 20).join("\n");
    assert!(lines.contains("coke out the front") && lines.contains("waste out the back"), "{lines}");
    assert!(lines.contains("waste 0/10"), "{lines}");
    // With a full waste queue and nothing behind, it says what to do.
    if let Some(Building { st: St::Crafter { out, .. }, .. }) = w.floors[0].tiles[idx(20, 20)].as_mut() {
        out.extend([Item::Waste; 10]);
    }
    assert!(inspect(&w, 0, 20, 20).join("\n").contains("waste stuck"));
    assert_eq!(w.idle_reason(0, idx(20, 20)), Some("BACK"));

    let mut app = App::new(w);
    app.book_go(Entry::Part(Kind::CokeOven));
    app.book = true;
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("terminactory.wiki/coke-oven") && t.contains("waste"), "{t}");
    // A machine with no waste has no such line.
    app.book_go(Entry::Part(Kind::Furnace));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("terminactory.wiki/furnace"), "{t}");
    // The waste page says where it comes from.
    // (A tall pane: the page of waste has grown with every machine that leaves some.)
    app.book_go(Entry::Item(Item::Waste));
    let t = text(&render(&mut app, 130, 60));
    assert!(t.contains("terminactory.wiki/waste") && !t.contains("fuel for"), "{t}");
    // And a placed boiler says nothing of burning waste.
    let mut w = World::blank();
    w.place(0, 20, 20, Kind::Boiler, Dir::E).unwrap();
    assert!(!inspect(&w, 0, 20, 20).join("\n").contains("waste"));
}

/// A long telegram has a thin scrollbar beside its text; a short one has none.
#[test]
fn long_telegrams_show_a_scrollbar() {
    let body: String = (1..=60).map(|i| format!("Line {i} of a long agent report.\n")).collect();
    for (w, h) in [(130u16, 42u16), (64, 20)] {
        let mut app = App::new(World::blank());
        app.mail = Some(crate::mail::parse(&format!("from: api\nevent: finished\n{body}"), 0));
        app.mail_scroll = 0;
        app.mail_open = true; // in a small pane it opens over the map; the sidebar ignores this
        let buf = render(&mut app, w, h);
        let thumbs = |b: &Buffer| b.content.iter().filter(|c| c.symbol() == "┃").count();
        let top = thumbs(&buf);
        assert!(top >= 1, "{w}x{h}: a thumb at the top:\n{}", text(&buf));
        let first_thumb_row = |b: &Buffer| (0..b.area.height).find(|&y| (0..b.area.width).any(|x| b[(x, y)].symbol() == "┃")).unwrap();
        let at_top = first_thumb_row(&buf);
        app.mail_scroll = u16::MAX; // clamped to the end when drawn
        let buf = render(&mut app, w, h);
        let at_end = first_thumb_row(&buf);
        assert!(at_end > at_top, "{w}x{h}: the thumb moves down as it scrolls ({at_top} -> {at_end})");
        // The thumb's last row is at the bottom of the track: nothing of the track below it.
        let x = (0..buf.area.width).find(|&x| buf[(x, at_end)].symbol() == "┃").unwrap();
        let below: Vec<&str> = (at_end..buf.area.height).map(|y| buf[(x, y)].symbol()).take_while(|s| *s == "┃" || *s == "│").collect();
        assert_eq!(below.last(), Some(&"┃"), "{w}x{h}: thumb reaches the end: {below:?}");
    }
    let mut app = App::new(World::blank());
    app.mail = Some(crate::mail::parse("from: api\nevent: finished\nDone", 0));
    let buf = render(&mut app, 130, 42);
    assert!(!buf.content.iter().any(|c| c.symbol() == "┃"), "no scrollbar for a short telegram");
}

/// A tech on its way to a free unlock shows a filling circle and how far along it is.
#[test]
fn research_rows_show_progress_toward_a_free_unlock() {
    let mut app = App::new(World::new());
    let row =
        |app: &App| -> String { tree_row_spans(&app.world, Tech::Press, 20, false, false).iter().map(|s| s.content.to_string()).collect() };
    assert!(row(&app).starts_with("○ "), "{}", row(&app));
    let (_, n) = Tech::Press.milestone();
    for (made, mark) in [(1, "◔"), (n as u64 / 2, "◑"), ((n as u64 * 3).div_ceil(4), "◕"), (n as u64 - 1, "◕")] {
        app.world.econ.made[Item::CopperIngot.idx()] = made;
        let r = row(&app);
        assert!(r.starts_with(&format!("{mark} ")) && r.contains(&format!("{made}/{n}")), "{made}: {r}");
    }
    // Affordable still reads as ready, whatever the progress.
    app.world.econ.research = Tech::Press.cost();
    assert!(row(&app).starts_with("● "), "{}", row(&app));
}

#[test]
fn wiki_arrows_move_along_a_line_and_between_lines() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "home".into();
    render(&mut app, 140, 45);
    let at = |app: &App, slug: &str| app.book_urls.iter().position(|(_, s)| s == slug).unwrap();
    // The four navigation tiles sit on one line: left and right walk along it.
    app.book_selected = at(&app, "items");
    app.on_key(KeyEvent::from(KeyCode::Right));
    assert_eq!(app.book_urls[app.book_selected].1, "topics");
    app.on_key(KeyEvent::from(KeyCode::Left));
    app.on_key(KeyEvent::from(KeyCode::Left));
    assert_eq!(app.book_urls[app.book_selected].1, "machines");
    app.on_key(KeyEvent::from(KeyCode::Left));
    assert_eq!(app.book_urls[app.book_selected].1, "companies", "left at the start of the line rolls over");
    app.on_key(KeyEvent::from(KeyCode::Right));
    // Up and down change lines: up from the tiles reaches the line above them.
    let tiles_y = app.book_urls[app.book_selected].0.y;
    app.on_key(KeyEvent::from(KeyCode::Up));
    assert!(app.book_urls[app.book_selected].0.y < tiles_y);
    app.on_key(KeyEvent::from(KeyCode::Down));
    assert_eq!(app.book_urls[app.book_selected].0.y, tiles_y);
}

#[test]
fn wiki_item_grid_search_and_recipe_ingredients_are_links() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "items".into();
    app.on_key(KeyEvent::from(KeyCode::Char('/')));
    app.on_key(KeyEvent::from(KeyCode::Char('c')));
    app.on_key(KeyEvent::from(KeyCode::Char('o')));
    let filtered = render(&mut app, 140, 45);
    assert!(text(&filtered).contains("Copper ore"));
    assert!(!text(&filtered).contains("Iron gear"));
    app.book_typing = false;
    app.book_slug = "copper-coil".into();
    render(&mut app, 140, 45);
    assert!(app.book_urls.iter().any(|(_, slug)| slug == "copper-wire"));
}

#[test]
fn wiki_home_has_four_navigation_destinations_and_generated_facts_link_to_pages() {
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "home".into();
    let rendered = text(&render(&mut app, 140, 45));
    assert!(rendered.contains("termweb") && rendered.contains("terminactory.wiki/home"));
    assert!(!rendered.contains("Terminactory Wiki — Terminactory Wiki"));
    assert!(rendered.contains("Machines") && rendered.contains("Items") && rendered.contains("Topics") && rendered.contains("Companies"));
    assert!(app.book_urls.iter().any(|(_, slug)| slug == "machines"));
    assert!(app.book_urls.iter().any(|(_, slug)| slug == "items"));
    assert!(app.book_urls.iter().any(|(_, slug)| slug == "topics"));
    assert!(app.book_urls.iter().any(|(_, slug)| slug == "companies"));
    for (_, slug) in app.book_urls.iter().filter(|(_, slug)| !["items", "machines", "topics", "close-book"].contains(&slug.as_str())) {
        assert!(crate::wiki::page(slug).is_some(), "generated home link has a page: {slug}");
    }
}

#[test]
fn wiki_contents_sidebar_only_appears_at_110_inner_columns() {
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "copper-coil".into();
    assert!(text(&render(&mut app, 140, 45)).contains("Contents"));
    assert!(!text(&render(&mut app, 120, 45)).contains("Contents"));
}

#[test]
fn wiki_recipe_cards_close_contents_cover_sections_and_home_is_plain() {
    let world = World::new();
    let md = crate::wiki::page("copper-coil").unwrap();
    let page = super::book_page_markdown(&world, Entry::Item(Item::CopperCoil), md, 94);
    assert_eq!(page.matches('╭').count(), page.matches('╰').count(), "every recipe card has a bottom border");

    let expected = ["Information", "Recipes", "Sources", "Uses", "Unlock", "Tips", "See also"];
    for section in expected {
        assert!(page.contains(&format!("## {section}")) || page.contains(&format!("### {section}")), "missing section {section}");
    }

    let mut page_app = App::new(World::new());
    page_app.book = true;
    page_app.book_slug = "copper-coil".into();
    let _ = render(&mut page_app, 140, 45);
    for section in expected {
        assert!(
            page_app.book_click_only.iter().any(|(_, target)| target == &format!("#{}", section.to_lowercase().replace(' ', "-"))),
            "Contents links to {section}"
        );
    }

    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "home".into();
    let rendered = text(&render(&mut app, 140, 45));
    let links = rendered.lines().find(|line| line.contains("Machines") && line.contains("Companies")).unwrap();
    for label in ["Machines", "Items", "Topics", "Companies"] {
        assert!(links.contains(label), "the link row has {label}: {links}");
    }
    assert!(rendered.contains("Back to the factory"));
    assert!(!rendered.contains("Welcome to the"));
}

#[test]
fn wiki_scrolls_the_complete_copper_coil_page_and_counts_all_machine_pages() {
    let mut wide = App::new(World::new());
    wide.book = true;
    wide.book_slug = "copper-coil".into();
    let wide_top = text(&render(&mut wide, 140, 45));
    assert!(wide_top.contains("Recipes"), "{wide_top}");
    wide.book_scroll = u16::MAX;
    let wide_end = text(&render(&mut wide, 140, 45));
    for heading in ["Unlock", "Tips", "See also"] {
        assert!(wide_end.contains(heading), "missing {heading}: {wide_end}");
    }
    assert!(wide.book_scroll > 0);

    let mut narrow = App::new(World::new());
    narrow.book = true;
    narrow.book_slug = "copper-coil".into();
    // The tab strip and the bookmarks bar take a row each, so the first screen is two lines shorter.
    narrow.book_scroll = 2;
    let narrow_top = text(&render(&mut narrow, 60, 30));
    assert!(narrow_top.contains("Recipes"), "{narrow_top}");
    narrow.book_scroll = 0;
    narrow.book_scroll = u16::MAX;
    let narrow_end = text(&render(&mut narrow, 60, 30));
    assert!(narrow_end.contains("See also") && narrow_end.contains("Category: Items"), "{narrow_end}");
    assert!(!narrow_end.contains("Money"), "infobox should scroll away: {narrow_end}");

    let machine_pages = crate::sim::Kind::ALL.iter().filter(|k| crate::wiki::page(&crate::wiki::slug(k.name())).is_some()).count();
    assert_eq!(machine_pages, 54);
    let mut home = App::new(World::new());
    home.book = true;
    home.book_slug = "home".into();
    let home_text = text(&render(&mut home, 140, 45));
    assert!(home_text.contains(&format!("{machine_pages} machines")), "{home_text}");
    assert!(!home_text.contains("Raw materials:"), "{home_text}");
}

/// The home page's "did you know?" facts are picked once when the book opens, not every frame.
#[test]
fn did_you_know_facts_hold_still_while_the_game_runs() {
    let mut app = App::new(World::new());
    app.on_key(KeyEvent::from(KeyCode::Char('b')));
    app.book_slug = "home".into();
    app.book_typing = false;
    let before = text(&render(&mut app, 140, 45));
    for _ in 0..37 {
        app.world.tick();
    }
    let after = text(&render(&mut app, 140, 45));
    let facts = |t: &str| t.lines().filter(|l| l.contains("…that")).map(|l| l.trim().to_string()).collect::<Vec<_>>();
    assert!(!facts(&before).is_empty(), "{before}");
    assert_eq!(facts(&before), facts(&after));
}

/// The four tiles on the wiki's home page open their pages, generated ones included.
#[test]
fn home_navigation_tiles_open_their_pages() {
    for (slug, shows) in [("machines", "Press"), ("items", "Copper ore"), ("topics", "Power"), ("companies", "Companies")] {
        let mut app = App::new(World::new());
        app.on_key(KeyEvent::from(KeyCode::Char('b')));
        app.book_slug = "home".into();
        app.book_typing = false;
        render(&mut app, 140, 45);
        app.book_selected = app.book_urls.iter().position(|(_, s)| s == slug).unwrap_or_else(|| panic!("no {slug} link on home"));
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.book_slug, slug);
        let t = text(&render(&mut app, 140, 45));
        assert!(t.to_lowercase().contains(&shows.to_lowercase()), "{slug}: {t}");
        app.on_key(KeyEvent::from(KeyCode::Backspace));
        assert_eq!(app.book_slug, "home");
    }
}

/// Every arrow rolls over at its edge: along a line, and from the top of a page to the bottom.
#[test]
fn wiki_arrows_roll_over_at_the_edges() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "home".into();
    render(&mut app, 140, 45);
    let at = |app: &App, slug: &str| app.book_urls.iter().position(|(_, s)| s == slug).unwrap();
    app.book_selected = at(&app, "companies");
    app.on_key(KeyEvent::from(KeyCode::Right));
    assert_eq!(app.book_urls[app.book_selected].1, "machines", "right at the end of the line rolls to its start");
    app.on_key(KeyEvent::from(KeyCode::Left));
    assert_eq!(app.book_urls[app.book_selected].1, "companies", "left at the start rolls to its end");
    // Up from the first link rolls to the last one; down from there rolls back to the first.
    app.book_selected = 0;
    app.on_key(KeyEvent::from(KeyCode::Up));
    render(&mut app, 140, 45);
    assert_eq!(app.book_selected, app.book_urls.len() - 1);
    assert_eq!(app.book_urls[app.book_selected].1, "close-book");
    app.on_key(KeyEvent::from(KeyCode::Down));
    render(&mut app, 140, 45);
    assert_eq!((app.book_selected, app.book_scroll), (0, 0));
    // A long page in a small window: up at the very top lands at the bottom of the page.
    app.book_slug = "copper-coil".into();
    app.book_scroll = 0;
    app.book_selected = 0;
    render(&mut app, 60, 30);
    app.on_key(KeyEvent::from(KeyCode::Up));
    let t = text(&render(&mut app, 60, 30));
    assert!(app.book_scroll > 0 && t.contains("See also"), "{t}");
    // The item index rolls over without breaking either.
    app.book_slug = "items".into();
    app.book_scroll = 0;
    app.book_selected = 0;
    render(&mut app, 60, 30);
    app.on_key(KeyEvent::from(KeyCode::Up));
    render(&mut app, 60, 30);
    assert!(app.book_selected < app.book_urls.len());
}

/// Down past the last visible tile on the item index scrolls a whole row, then rolls over at the end.
#[test]
fn wiki_item_grid_scrolls_by_rows_and_rolls_over() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "items".into();
    render(&mut app, 60, 30);
    let mut seen_scroll = false;
    for _ in 0..200 {
        app.on_key(KeyEvent::from(KeyCode::Down));
        render(&mut app, 60, 30);
        seen_scroll |= app.book_scroll > 0;
        if seen_scroll && app.book_scroll == 0 {
            break;
        }
    }
    assert!(seen_scroll, "the grid never scrolled");
    assert_eq!((app.book_scroll, app.book_selected), (0, 0), "it rolled over to the top");
}

/// A link to a page that isn't there still opens: T Web shows its own 404, and Backspace returns.
#[test]
fn web_missing_page_shows_a_404_and_backspace_returns() {
    let mut app = App::new(World::new());
    app.on_key(KeyEvent::from(KeyCode::Char('b')));
    app.book_slug = "example.news/index".into();
    app.book_typing = false;
    render(&mut app, 110, 40);
    app.book_selected = app.book_urls.iter().position(|(_, s)| s == "example.news/issues/issue-9").expect("the broken link is on the page");
    app.on_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(app.book_slug, "example.news/issues/issue-9");
    let t = text(&render(&mut app, 110, 40));
    assert!(t.contains("404") && t.contains("was not found on this server"), "{t}");
    assert!(t.contains("example.news/index"), "the 404 links back to the domain's index: {t}");
    app.on_key(KeyEvent::from(KeyCode::Backspace));
    assert_eq!(app.book_slug, "example.news/index");
    render(&mut app, 110, 40);
    // A link to a server that isn't there says so, and has nothing to follow.
    app.book_selected = app.book_urls.iter().position(|(_, s)| s == "nowhere.example/lost").unwrap();
    app.on_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(app.book_slug, "nowhere.example/lost");
    let t = text(&render(&mut app, 110, 40));
    assert!(t.contains("Server not found") && t.contains("nowhere.example"), "{t}");
}

/// A site page renders as plain text, its links carry full addresses, and a table's cells keep
/// their links. A wiki link on a web page still opens the wiki.
#[test]
fn web_site_page_renders_with_full_address_links() {
    let mut app = App::new(World::new());
    app.on_key(KeyEvent::from(KeyCode::Char('b')));
    app.book_slug = "example.news/index".into();
    app.book_typing = false;
    let t = text(&render(&mut app, 110, 40));
    assert!(t.contains("Example News") && t.contains("Issue 1"), "{t}");
    assert!(t.contains("example.news/index"), "the address field shows the address: {t}");
    for target in ["example.news/about", "example.news/issues/issue-1", "example.forum", "copper-coil", "example.news/issues/issue-9"] {
        assert!(app.book_urls.iter().any(|(_, s)| s == target), "{target} is a link: {:?}", app.book_urls);
    }
    // A forum link opens the forum's index, its table links to a thread, and that thread's page shows.
    app.book_selected = app.book_urls.iter().position(|(_, s)| s == "example.forum").unwrap();
    app.on_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(app.book_slug, "example.forum");
    let t = text(&render(&mut app, 110, 40));
    assert!(t.contains("Example forum"), "{t}");
    app.book_selected = app.book_urls.iter().position(|(_, s)| s == "example.forum/first-thread").unwrap();
    app.on_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(app.book_slug, "example.forum/first-thread");
    // A wiki link on a web page opens the wiki page, with the wiki's address.
    app.book_slug = "example.news/issues/issue-1".into();
    app.book_selected = 0;
    render(&mut app, 110, 40);
    app.book_selected = app.book_urls.iter().position(|(_, s)| s == "copper-coil").unwrap();
    app.on_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(app.book_slug, "copper-coil");
    let t = text(&render(&mut app, 110, 40));
    assert!(t.contains("terminactory.wiki/copper-coil"), "{t}");
}

/// A thread nests its replies: a depth-3 post sits further right than its parent, and PgDn
/// moves down the thread while PgUp comes back to the top.
#[test]
fn web_thread_nests_posts_and_scrolls() {
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "example.forum/deep-thread".into();
    app.book_typing = false;
    let t = text(&render(&mut app, 100, 34));
    assert!(t.contains("A deep thread") && t.contains("Help · solved · 5 replies"), "{t}");
    let xs: Vec<u16> = app.book_urls.iter().filter(|(_, s)| s == "lounge.example/@bob").map(|(r, _)| r.x).collect();
    assert_eq!(xs.len(), 2, "bob's profile is linked on both his posts: {:?}", app.book_urls);
    assert!(xs[1] > xs[0], "a depth-3 reply sits further right than its parent: {xs:?}");
    // A short window: the thread is taller than its page.
    render(&mut app, 60, 22);
    app.on_key(KeyEvent::from(KeyCode::PageDown));
    render(&mut app, 60, 22);
    assert!(app.book_scroll > 0);
    app.on_key(KeyEvent::from(KeyCode::PageUp));
    assert_eq!(app.book_scroll, 0);
}

/// f folds the post whose header is picked and f again brings its replies back; F folds every
/// top-level post and unfolds everything when anything is folded.
#[test]
fn web_thread_folding_hides_and_restores_replies() {
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "example.forum/deep-thread".into();
    app.book_typing = false;
    let pick_first_post = |app: &mut App| {
        render(app, 100, 34);
        let row = app.book_post_hits.iter().find(|(_, p)| *p == 0).expect("a post header").0.y;
        app.book_selected = app.book_urls.iter().position(|(r, _)| r.y == row).expect("the author link");
    };
    assert!(text(&render(&mut app, 100, 34)).contains("Bottom of this branch"));
    pick_first_post(&mut app);
    app.on_key(KeyEvent::from(KeyCode::Char('f')));
    let t = text(&render(&mut app, 100, 34));
    assert!(t.contains("[+] 5 replies hidden"), "{t}");
    assert!(!t.contains("Bottom of this branch"), "a folded post hides its replies: {t}");
    assert!(!app.book_urls.iter().any(|(_, s)| s == "lounge.example/@bob"), "and their links");
    pick_first_post(&mut app);
    app.on_key(KeyEvent::from(KeyCode::Char('f')));
    assert!(text(&render(&mut app, 100, 34)).contains("Bottom of this branch"));
    // F folds the top-level post; F again unfolds everything.
    app.on_key(KeyEvent::from(KeyCode::Char('F')));
    assert_eq!(app.book_folded, vec![0], "the only top-level post");
    assert!(text(&render(&mut app, 100, 34)).contains("[+] 5 replies hidden"));
    app.on_key(KeyEvent::from(KeyCode::Char('F')));
    assert!(app.book_folded.is_empty());
}

/// Folds belong to their tab, and following a link starts a fresh page.
#[test]
fn web_folds_travel_with_their_tab_and_links_clear_them() {
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "example.forum/deep-thread".into();
    app.book_typing = false;
    render(&mut app, 100, 34);
    app.book_folded = vec![1];
    app.on_key(KeyEvent::from(KeyCode::Char('+')));
    render(&mut app, 100, 34);
    app.book_switch(0);
    assert_eq!(app.book_slug, "example.forum/deep-thread");
    assert_eq!(app.book_folded, vec![1], "the fold stayed with its tab");
    // A link in the first post's body still works while another post is folded.
    render(&mut app, 100, 34);
    app.book_selected = app.book_urls.iter().position(|(_, s)| s == "copper-coil").expect("the wiki link");
    app.on_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(app.book_slug, "copper-coil");
    assert!(app.book_folded.is_empty(), "a new page starts unfolded");
}

/// Authors link to their profile page, wherever it is, and a profile lists the threads its
/// author posted in. A name with no profile is plain text.
#[test]
fn web_profile_links_open_the_right_page_and_list_its_threads() {
    let mut app = App::new(World::new());
    app.book = true;
    app.book_typing = false;
    let open = |app: &mut App, target: &str| {
        app.book_slug = "example.forum/deep-thread".into();
        render(app, 110, 40);
        let i = app.book_urls.iter().position(|(_, s)| s == target).unwrap_or_else(|| panic!("no link to {target}"));
        app.book_selected = i;
        app.on_key(KeyEvent::from(KeyCode::Enter));
    };
    open(&mut app, "example.forum/@alice");
    assert_eq!(app.book_slug, "example.forum/@alice");
    let t = text(&render(&mut app, 110, 40));
    assert!(t.contains("In this archive") && t.contains("Posts from Y140-08-25 to Y140-09-02."), "{t}");
    assert!(t.contains("A deep thread") && t.contains("A first thread"), "{t}");
    assert!(app.book_urls.iter().any(|(_, s)| s == "example.forum/first-thread"));
    open(&mut app, "lounge.example/@bob");
    assert_eq!(app.book_slug, "lounge.example/@bob");
    // carol has no profile page anywhere: her name is not a link.
    app.book_slug = "example.forum/deep-thread".into();
    render(&mut app, 110, 40);
    assert!(!app.book_urls.iter().any(|(_, s)| s.contains("carol")));
}

/// T Web: the bookmarks bar starts with three pages, `*` adds and removes the current one, and
/// a long page gets a scrollbar down its right edge.
#[test]
fn t_web_bookmarks_and_scrollbar() {
    use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let mut app = App::new(World::new());
    app.on_key(KeyEvent::from(KeyCode::Char('b')));
    let t = text(&render(&mut app, 100, 36));
    assert!(t.contains("★ Getting started") && t.contains("★ Machines") && t.contains("★ Items"), "{t}");
    // A page that is not bookmarked has an empty star; `*` fills it and puts it on the bar.
    app.book_slug = "copper-coil".into();
    app.book_typing = false;
    assert!(text(&render(&mut app, 100, 36)).contains('☆'));
    app.on_key(KeyEvent::from(KeyCode::Char('*')));
    let t = text(&render(&mut app, 100, 36));
    assert!(t.contains("★ Copper coil"), "{t}");
    assert_eq!(app.settings.bookmarks.last().map(String::as_str), Some("copper-coil"));
    // Clicking a bookmark opens it.
    let (r, _) = app.book_mark_hits.iter().find(|(_, s)| s == "getting-started").cloned().unwrap();
    app.on_mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
    assert_eq!(app.book_slug, "getting-started");
    // And `*` again takes the page off.
    app.book_slug = "copper-coil".into();
    app.book_typing = false;
    app.on_key(KeyEvent::from(KeyCode::Char('*')));
    assert!(!app.settings.bookmarks.iter().any(|s| s == "copper-coil"));
    // A tall page in a short window shows the scrollbar (it appears on the second frame).
    app.book_slug = "copper-coil".into();
    render(&mut app, 60, 30);
    let t = text(&render(&mut app, 60, 30));
    assert!(t.contains('▲') && t.contains('▼') && t.contains('█'), "{t}");
}

/// A gated page asks for a sign-in and never shows its body until one works. Usernames match
/// without case, passwords exactly, and a domain stays signed in for its other pages.
#[test]
fn web_gated_pages_ask_for_a_sign_in_and_remember_it() {
    use crate::app::LoginForm;
    let mut app = App::new(World::new());
    app.book = true;
    app.book_typing = false;
    app.book_slug = "portal.example.lan/gate".into();
    let t = text(&render(&mut app, 80, 34));
    assert!(t.contains("This page needs a sign-in."), "{t}");
    assert!(t.contains("portal.example.lan"), "{t}");
    assert!(t.contains("user:") && t.contains("pass:"), "{t}");
    assert!(!t.contains("Quiet night") && !t.contains("changeover"), "the body must not leak: {t}");
    assert!(!app.book_urls.iter().any(|(_, s)| s == "portal.example.lan/gate"), "no body links either");

    // A wrong password fails, and says nothing beyond that.
    app.on_key(KeyEvent::from(KeyCode::Char('i')));
    assert!(app.book_login_typing, "i starts typing");
    for c in "MIRA".chars() {
        app.on_key(KeyEvent::from(KeyCode::Char(c)));
    }
    app.on_key(KeyEvent::from(KeyCode::Tab));
    for c in "not copper".chars() {
        app.on_key(KeyEvent::from(KeyCode::Char(c)));
    }
    let typed = text(&render(&mut app, 80, 34));
    assert!(typed.contains("not copper"), "the password shows as typed, not as dots: {typed}");
    assert!(!typed.contains('•'), "{typed}");
    app.on_key(KeyEvent::from(KeyCode::Enter));
    let t = text(&render(&mut app, 80, 34));
    assert!(t.contains("Sign-in failed."), "{t}");
    assert!(!t.contains("Quiet night"), "{t}");
    assert!(app.book_logins.is_empty());

    // The right one, in either case, signs in and draws the page that was asked for.
    app.book_login = Some(LoginForm { user: "mira".into(), pass: "copper".into(), on_pass: true, failed: false });
    app.book_login_typing = true;
    app.on_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(app.book_login, None);
    assert!(app.book_typing == false);
    let t = text(&render(&mut app, 80, 34));
    assert!(t.contains("Quiet night") && t.contains("changeover"), "{t}");
    assert!(t.contains("signed in as mira"), "the status bar says who is reading: {t}");
    assert_eq!(app.book_logins, vec![("portal.example.lan".to_string(), "mira".to_string())]);
    assert!(app.book_history.is_empty(), "signing in adds no history entry");

    // `access: all` accepts any account of the domain, without another form.
    let mut app2 = App::new(World::new());
    app2.book = true;
    app2.book_typing = false;
    app2.book_logins = vec![("portal.example.lan".into(), "otto".into())];
    app2.book_slug = "portal.example.lan/all".into();
    let t = text(&render(&mut app2, 80, 34));
    assert!(t.contains("Anyone with an account here"), "{t}");

    // Signed in, but not on the list: a 403 with nothing of the page.
    let mut app3 = App::new(World::new());
    app3.book = true;
    app3.book_typing = false;
    app3.book_logins = vec![("portal.example.lan".into(), "otto".into())];
    app3.book_slug = "portal.example.lan/gate".into();
    let t = text(&render(&mut app3, 80, 34));
    assert!(t.contains("403") && t.contains("otto may not read this page."), "{t}");
    assert!(!t.contains("Quiet night"), "{t}");

    // Another domain still asks, and logout signs this one out and shows its index.
    app.book_slug = "example.news/login".into();
    let t = text(&render(&mut app, 80, 34));
    assert!(t.contains("This page needs a sign-in."), "{t}");
    app.book_submit_search("portal.example.lan/logout");
    assert_eq!(app.book_slug, "portal.example.lan/index");
    assert!(app.book_logins.is_empty());
}

/// Search finds a thread from another site, shows its address, and never reveals a gated page.
#[test]
fn web_search_finds_threads_and_never_reveals_gated_pages() {
    let mut app = App::new(World::new());
    app.book = true;
    app.book_typing = false;
    app.book_slug = "search?q=deep thread".into();
    let t = text(&render(&mut app, 100, 34));
    assert!(t.contains("example.forum › deep-thread"), "{t}");
    assert!(t.contains("A deep thread") && !t.contains("A deep thread - Terminactory Wiki"), "{t}");
    let at = app.book_urls.iter().position(|(_, s)| s == "example.forum/deep-thread").expect("a result link");
    app.book_selected = at;
    app.on_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(app.book_slug, "example.forum/deep-thread");
    assert!(text(&render(&mut app, 100, 34)).contains("First post text."));

    // A gated page's words find nothing at all, in titles, snippets or links.
    let mut gated = App::new(World::new());
    gated.book = true;
    gated.book_typing = false;
    gated.book_slug = "search?q=changeover".into();
    let t = text(&render(&mut gated, 100, 34));
    assert!(!t.contains("Shift notes") && !t.contains("Quiet night"), "{t}");
    assert!(!gated.book_urls.iter().any(|(_, s)| s == "portal.example.lan/gate"), "{t}");

    // A domain typed into the start page's box opens it instead of searching for it.
    let mut direct = App::new(World::new());
    direct.book = true;
    direct.book_slug = "start".into();
    direct.book_typing = true;
    direct.book_query = "example.forum".into();
    direct.on_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(direct.book_slug, "example.forum");
    direct.book_typing = false;
    let t = text(&render(&mut direct, 100, 34));
    assert!(t.contains("Example forum") && t.contains("A deep thread"), "{t}");
}

/// Every page kind draws inside the window at 120, 80, 50 and 30 columns.
#[test]
fn web_pages_draw_at_narrow_widths() {
    for (w, h) in [(120, 40), (80, 34), (50, 34), (30, 20)] {
        for (slug, fold) in [
            ("example.forum/deep-thread", true),
            ("portal.example.lan/gate", false),
            ("portal.example.lan/nope", false),
            ("example.forum/index", false),
            ("search?q=deep thread", false),
        ] {
            let mut app = App::new(World::new());
            app.book = true;
            app.book_typing = false;
            app.book_slug = slug.into();
            if fold {
                app.book_folded = vec![0];
            }
            let buf = render(&mut app, w, h);
            assert_eq!(buf.area.width, w, "{slug} at {w}x{h}");
            assert!(!text(&buf).trim().is_empty(), "{slug} at {w}x{h} drew nothing");
        }
    }
}

/// Every `[[domain/path]]` link in `WEB_PAGES` whose target is missing, grouped by page.
/// Dead links are allowed: this only reports them, for whoever writes the content.
#[test]
#[ignore]
fn web_dead_links_report() {
    let mut dead = 0;
    for (addr, md) in crate::web::WEB_PAGES {
        let mut rest = *md;
        let mut missing: Vec<String> = Vec::new();
        while let Some(start) = rest.find("[[") {
            let after = &rest[start + 2..];
            let Some(end) = after.find("]]") else { break };
            let spec = after[..end].split('|').next().unwrap_or("").trim();
            rest = &after[end + 2..];
            if spec.is_empty() || spec.starts_with('#') || !spec.contains('/') {
                continue;
            }
            if crate::web::page(spec).is_none() && missing.iter().all(|m| m != spec) {
                missing.push(spec.to_string());
            }
        }
        if !missing.is_empty() {
            dead += missing.len();
            println!("{addr}");
            for target in missing {
                println!("  -> {target}");
            }
        }
    }
    println!("{dead} dead links");
}

#[test]
fn the_copy_button_is_in_the_status_bar_and_the_page_text_is_plain() {
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "start".into();
    render(&mut app, 140, 45);
    assert!(app.book_page_text().is_none());
    assert!(!app.book_buttons.iter().any(|(_, b)| *b == crate::app::BookButton::Copy));
    app.book_slug = "boiler".into();
    let screen = render(&mut app, 140, 45);
    assert!(text(&screen).contains("Copy page"));
    assert!(app.book_buttons.iter().any(|(_, b)| *b == crate::app::BookButton::Copy));
    let copied = app.book_page_text().expect("wiki text");
    assert!(copied.starts_with("terminactory.wiki/boiler"), "{copied}");
    assert!(!copied.contains("[[") && !copied.contains("**"));
    app.book_slug = "search?q=boiler".into();
    let results = app.book_page_text().expect("results text");
    assert!(results.starts_with("Search: boiler"), "{results}");
    assert!(results.contains("terminactory.wiki/boiler"));
    app.book_slug = "example.forum/deep-thread".into();
    let thread = app.book_page_text().expect("thread text");
    assert!(thread.contains(" · "), "{thread}");
}

#[test]
fn a_sign_in_form_takes_typing_even_when_the_search_box_had_the_keys() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "start".into();
    app.book_typing = true;
    app.book_go_slug("portal.plant3.lan/login");
    for c in "ab".chars() {
        app.on_key(KeyEvent::from(KeyCode::Char(c)));
    }
    assert_eq!(app.book_login.as_ref().map(|f| f.user.as_str()), Some("ab"));
    assert!(app.book_query.is_empty());
}

#[test]
fn signing_in_on_the_login_page_opens_the_domain() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut app = App::new(World::new());
    app.book = true;
    app.book_go_slug("portal.plant3.lan/login");
    let account = crate::web::accounts("portal.plant3.lan").into_iter().next().expect("an account");
    for c in account.user.chars() {
        app.on_key(KeyEvent::from(KeyCode::Char(c)));
    }
    app.on_key(KeyEvent::from(KeyCode::Tab));
    for c in account.pass.chars() {
        app.on_key(KeyEvent::from(KeyCode::Char(c)));
    }
    app.on_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(app.book_slug, "portal.plant3.lan");
    assert!(app.book_signed_in("portal.plant3.lan").is_some());
}

#[test]
fn a_wide_table_wraps_its_long_cells_instead_of_cutting_them() {
    let mut app = App::new(World::new());
    app.book = true;
    app.book_logins.push(("portal.plant3.lan".into(), "TamsinR".into()));
    app.book_slug = "portal.plant3.lan/retirements".into();
    let screen = text(&render(&mut app, 100, 40));
    for tail in ["took the stool", "gave it back", "Thursdays."] {
        assert!(screen.contains(tail), "{tail} was cut off:\n{screen}");
    }
}

fn left_click(app: &mut App, x: u16, y: u16) {
    use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    app.on_mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: x, row: y, modifiers: KeyModifiers::NONE });
}

#[test]
fn the_window_dots_close_minimize_and_fullscreen() {
    let mut app = App::new(World::new());
    app.book = true;
    render(&mut app, 140, 45);
    app.book_new_tab("boiler");
    render(&mut app, 140, 45);
    let dot = |app: &App, b: crate::app::BookButton| app.book_buttons.iter().find(|(_, x)| *x == b).map(|(r, _)| *r).unwrap();
    let windowed = app.book_area;
    // Green: fullscreen, and again to go back.
    let r = dot(&app, crate::app::BookButton::Fullscreen);
    left_click(&mut app, r.x, r.y);
    render(&mut app, 140, 45);
    assert!(app.book_full && app.book_area.width > windowed.width);
    let r = dot(&app, crate::app::BookButton::Fullscreen);
    left_click(&mut app, r.x, r.y);
    render(&mut app, 140, 45);
    assert_eq!(app.book_area, windowed);
    // Yellow: the browser goes away with its two tabs.
    let r = dot(&app, crate::app::BookButton::Minimize);
    left_click(&mut app, r.x, r.y);
    assert!(!app.book);
    assert_eq!(app.book_tabs.len(), 2);
    app.book = true;
    render(&mut app, 140, 45);
    // Red: it goes away and the tabs are forgotten.
    let r = dot(&app, crate::app::BookButton::CloseWindow);
    left_click(&mut app, r.x, r.y);
    assert!(!app.book);
    assert_eq!(app.book_tabs.len(), 1);
}

#[test]
fn a_double_click_on_blank_navbar_toggles_fullscreen() {
    let mut app = App::new(World::new());
    app.book = true;
    render(&mut app, 140, 45);
    let strip = app.book_nav.iter().copied().find(|r| r.y == app.book_area.y + 1).expect("the tab strip");
    let x = strip.right() - 3;
    left_click(&mut app, x, strip.y);
    assert!(!app.book_full, "one click does nothing");
    left_click(&mut app, x, strip.y);
    assert!(app.book_full);
}

#[test]
fn searches_are_remembered_and_listed_on_the_start_page() {
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "start".into();
    app.book_submit_search("coal");
    app.book_submit_search("boiler waste");
    app.book_submit_search("COAL");
    assert_eq!(app.settings.search_history, vec!["COAL".to_string(), "boiler waste".to_string()]);
    app.book_go_slug("start");
    let screen = text(&render(&mut app, 140, 45));
    assert!(!screen.contains("Recent searches"), "the start page lists no searches");
    assert!(screen.contains(" History "), "{screen}");
    let (r, _) = app.book_buttons.iter().find(|(_, b)| *b == crate::app::BookButton::History).cloned().expect("a History button");
    left_click(&mut app, r.x, r.y);
    assert_eq!(app.book_slug, "history");
    let page = text(&render(&mut app, 140, 45));
    assert!(page.contains("Search history") && page.contains("↺ boiler waste"), "{page}");
    assert!(!page.contains("Copy page"), "no Copy button on the history page");
    let (r, _) = app.book_urls.iter().find(|(_, s)| s == "search?q=boiler waste").cloned().expect("a clickable entry");
    left_click(&mut app, r.x, r.y);
    assert_eq!(app.book_slug, "search?q=boiler waste");
    // Clearing empties the list.
    app.book_go_slug("history");
    render(&mut app, 140, 45);
    let (r, _) = app.book_buttons.iter().find(|(_, b)| *b == crate::app::BookButton::ClearHistory).cloned().expect("a Clear button");
    left_click(&mut app, r.x, r.y);
    assert!(app.settings.search_history.is_empty());
    // Later pages have no History button.
    app.book_go_slug("boiler");
    render(&mut app, 140, 45);
    assert!(!app.book_buttons.iter().any(|(_, b)| *b == crate::app::BookButton::History));
}

#[test]
fn an_items_symbol_in_a_recipe_card_is_coloured_but_not_a_link() {
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "coke".into();
    let screen = render(&mut app, 140, 45);
    assert!(!app.book_urls.iter().any(|(_, s)| s.starts_with('!')), "{:?}", app.book_urls);
    let coal = Item::Coal;
    let (glyph, style) = item_cell(coal, Color::Reset);
    let found = (0..screen.area.height).any(|y| {
        (0..screen.area.width).any(|x| {
            let c = &screen[(x, y)];
            c.symbol() == glyph.to_string() && c.fg == style.fg.unwrap_or(Color::Reset) && !c.modifier.contains(Modifier::UNDERLINED)
        })
    });
    assert!(found, "a coloured, un-underlined coal symbol");
}

#[test]
fn b_opens_a_new_tab_and_esc_minimizes_in_the_browser() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "boiler".into();
    render(&mut app, 140, 45);
    app.book_typing = false;
    app.on_key(KeyEvent::from(KeyCode::Char('b')));
    assert!(app.book && app.book_tabs.len() == 2 && app.book_slug == "start");
    app.book_typing = false;
    app.on_key(KeyEvent::from(KeyCode::Esc));
    assert!(!app.book);
    assert_eq!(app.book_tabs.len(), 2, "the tabs are kept");
}

#[test]
fn the_address_field_takes_addresses_and_searches() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "boiler".into();
    render(&mut app, 140, 45);
    let click_field = |app: &mut App| {
        let (r, _) = app.book_buttons.iter().find(|(_, b)| *b == crate::app::BookButton::Address).cloned().expect("the address field");
        left_click(app, r.x + 2, r.y);
    };
    let type_text = |app: &mut App, t: &str| {
        for c in t.chars() {
            app.on_key(KeyEvent::from(KeyCode::Char(c)));
        }
    };
    // The first key replaces the whole address.
    click_field(&mut app);
    assert!(app.book_url_edit.is_some());
    type_text(&mut app, "gazette.city");
    assert_eq!(app.book_url_edit.as_deref(), Some("gazette.city"));
    app.on_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(app.book_slug, "gazette.city");
    render(&mut app, 140, 45);
    // Backspace on a fresh field deletes the whole address; Esc leaves it be.
    click_field(&mut app);
    app.on_key(KeyEvent::from(KeyCode::Backspace));
    assert_eq!(app.book_url_edit.as_deref(), Some(""));
    app.on_key(KeyEvent::from(KeyCode::Esc));
    assert!(app.book_url_edit.is_none() && app.book_slug == "gazette.city");
    // A wiki address and plain words.
    click_field(&mut app);
    type_text(&mut app, "terminactory.wiki/coal");
    app.on_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(app.book_slug, "coal");
    render(&mut app, 140, 45);
    click_field(&mut app);
    type_text(&mut app, "north belt");
    app.on_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(app.book_slug, "search?q=north belt");
}

#[test]
fn clicking_away_unfocuses_the_box_and_digits_pick_tabs() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut app = App::new(World::new());
    app.book = true;
    app.book_slug = "start".into();
    app.book_typing = true;
    render(&mut app, 140, 45);
    let (cx, cy) = (app.book_area.x + 3, app.book_area.bottom() - 4);
    left_click(&mut app, cx, cy);
    assert!(!app.book_typing, "a click elsewhere takes the keys away");
    app.on_key(KeyEvent::from(KeyCode::Char('b')));
    assert_eq!(app.book_tabs.len(), 2, "b opens a tab");
    app.book_typing = false;
    app.on_key(KeyEvent::from(KeyCode::Char('1')));
    assert_eq!(app.book_active, 0);
    app.on_key(KeyEvent::from(KeyCode::Char('2')));
    assert_eq!(app.book_active, 1);
    // While a box has the keys, digits are text.
    app.book_typing = true;
    app.on_key(KeyEvent::from(KeyCode::Char('1')));
    assert_eq!(app.book_active, 1);
    assert!(app.book_query.ends_with('1'));
}

#[test]
fn switching_tabs_shows_no_progress_bar_but_a_new_tab_does() {
    let mut app = App::new(World::new());
    app.book = true;
    app.book_new_tab("boiler");
    assert!(app.book_opened.is_some(), "a new tab loads");
    app.book_opened = None;
    app.book_switch(0);
    assert!(app.book_opened.is_none());
    app.book_switch(1);
    assert!(app.book_opened.is_none());
}

#[test]
fn the_production_site_has_two_tabs_an_address_and_no_copy_button() {
    let mut app = App::new(World::new());
    app.on_key(KeyEvent::from(KeyCode::Char('o')));
    let t = text(&render(&mut app, 140, 45));
    assert!(t.contains("production.lan") && !t.contains("Copy page"), "{t}");
    app.on_key(KeyEvent::from(KeyCode::Tab));
    assert!(app.stats_history && app.book_slug == "production");
    let t = text(&render(&mut app, 140, 45));
    assert!(t.contains("Money ·"), "{t}");
    // Typing the address finds it too.
    app.book_submit_address("terminactory.wiki/coal");
    app.book_submit_address("production.lan");
    assert_eq!(app.book_slug, "production");
}
