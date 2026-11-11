use super::*;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{backend::TestBackend, Terminal};

mod area;
mod blueprints;
mod hints_line;
mod history_tab;
mod keys;
mod settings_effects;
mod settings_pane;
mod sidebar_graph;
mod small_pane;

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
    assert!(text(&buf).contains("Press") && text(&buf).contains("make copper wire first"));

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

/// The research tree turns top-down in a tall, narrow window, and scrolls in a small one.
#[test]
fn research_tree_fits_the_window() {
    use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let mut w = World::new();
    w.econ.research = 17_687.0;
    let mut app = App::new(w);
    app.tree = true;
    // Wide: group names in the gutter, every unlock showing, details under the list.
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("17,687 research") && !t.contains("Upgrades"), "{t}");
    assert!(t.contains("Making        ● Press"), "{t}");
    assert!(t.contains("make copper wire first") && t.contains("Belt tunnels") && t.contains("Large adv. drill"), "{t}");
    assert!(t.contains("Press · Create") && t.contains("Enter: research for 60 RP"), "{t}");
    // 35 techs are more than 42 rows hold: it scrolls (and a tall window shows them all).
    assert!(t.contains("more ▼") && !t.contains("Blueprints"), "{t}");
    let tall = text(&render(&mut app, 130, 60));
    assert!(!tall.contains("more ") && tall.contains("Floors") && tall.contains("Excavation") && tall.contains("Blueprints"), "{tall}");
    std::fs::write("target/snapshot-tree.html", to_html(&buf)).unwrap();
    // Going to the last row scrolls it into view, and the top-of-list marker shows.
    app.tree_pick = tree_rows().len() - 1;
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("more ▲") && t.contains("Blueprints") && t.contains("Blueprints · Misc"), "{t}");
    std::fs::write("target/snapshot-tree-scrolled.html", to_html(&buf)).unwrap();
    app.tree_pick = 0;
    // Narrow: group names on their own lines; small: it scrolls, with markers.
    let t = text(&render(&mut app, 60, 50));
    assert!(t.lines().any(|l| l.trim_matches(|c| c == '│' || c == ' ') == "Making"), "{t}");
    let buf = render(&mut app, 64, 16);
    assert!(text(&buf).contains("more "), "{}", text(&buf));
    std::fs::write("target/snapshot-tree-small.html", to_html(&buf)).unwrap();
    // Clicking a row picks it; clicking it again researches it.
    render(&mut app, 130, 42);
    let (r, i) = app.tree_rows.iter().copied().find(|(_, i)| tree_rows()[*i] == Tech::Tunnels).unwrap();
    let click = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x + 2, row: r.y, modifiers: KeyModifiers::NONE };
    app.on_mouse(click);
    assert_eq!(app.tree_pick, i);
    assert!(!app.world.progress.has(Tech::Tunnels));
    app.on_mouse(click);
    assert!(app.world.progress.has(Tech::Tunnels));
    // n opens a waiting telegram over the screen.
    app.mail = Some(crate::mail::parse("Done", 0));
    app.on_key(KeyEvent::from(KeyCode::Char('n')));
    assert!(app.mail_open && app.tree);
}

/// The recipe book: pages for items and machines, word equations, links, search.
#[test]
fn recipe_book() {
    use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let mut w = World::new();
    w.build_demo().unwrap();
    let mut app = App::new(w);
    let key = |app: &mut App, c: KeyCode| app.on_key(KeyEvent::from(c));
    // Pointing at the demo molder opens the book on copper ingot.
    app.cursor = (56, 38);
    key(&mut app, KeyCode::Char('b'));
    assert!(app.book);
    assert_eq!(app.book_entry, Entry::Item(Item::CopperIngot));
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("Made by") && t.contains("Used in") && !t.contains("MADE BY"), "{t}");
    assert!(t.contains("molten copper  ──→  copper ingot"), "a word equation, with the fluid in it: {t}");
    assert!(t.contains("copper ingot  ──→  copper plate   in Press"), "{t}");
    assert!(t.contains("6.0 now, fading to a floor of 0.18") && t.contains("0.18 ┤") && t.contains("← floor"), "{t}");
    std::fs::write("target/snapshot-book.html", to_html(&buf)).unwrap();

    // Tab switches to the machines tab and back to the item you were on.
    assert!(t.contains(" Items ") && t.contains(" Machines ") && !t.contains("Basic drill"), "items only: {t}");
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.book_entry, Entry::Part(Kind::Drill));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("Basic drill") && !t.contains("iron gear "), "machines only: {t}");
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.book_entry, Entry::Item(Item::CopperIngot));

    // Clicking a machine's name opens its page (on the machines tab); Backspace comes back.
    render(&mut app, 130, 42);
    let (r, _) = *app.book_links.iter().find(|(_, e)| *e == Entry::Part(Kind::Molder)).unwrap();
    app.on_mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
    assert_eq!(app.book_entry, Entry::Part(Kind::Molder));
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.book_entry, Entry::Item(Item::CopperIngot));
    app.book_go(Entry::Part(Kind::Furnace));
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("Status") && t.contains("ready to build") && t.contains("Makes"), "{t}");
    assert!(t.contains("copper ore  ─coal─→  molten copper"), "{t}");
    assert!(t.contains("3 iron ore  ─coal─→  2 molten iron"), "{t}");
    assert!(t.contains("3 coal  ──→  coke"), "{t}");
    std::fs::write("target/snapshot-book-machine.html", to_html(&buf)).unwrap();
    app.book_go(Entry::Part(Kind::Molder));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("molten iron  ──→  iron ingot") && t.contains("molten steel  ──→  steel"), "{t}");
    // Stone and iron ore are mined by both drills.
    app.book_go(Entry::Item(Item::Stone));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("Cheap drill or Basic drill"), "{t}");
    app.book_go(Entry::Part(Kind::Drill));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("iron ore") && t.contains("stone") && t.contains("half the basic drill's speed"), "{t}");

    // Steel is cast from molten steel; the blast furnace's page shows the four ways to melt it, with their waste.
    app.book_go(Entry::Item(Item::Steel));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("molten steel  ──→  steel") && t.contains("Used in"), "{t}");
    app.book_go(Entry::Part(Kind::BlastFurnace));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("2 iron plate + coke  ──→  molten steel + waste"), "{t}");
    assert!(t.contains("2 iron plate + 2 coal  ──→  molten steel + 3 waste"), "{t}");
    assert!(t.contains("iron plate + coke  ──→  hardened iron plate"), "{t}");
    std::fs::write("target/snapshot-book-blast.html", to_html(&render(&mut app, 130, 42))).unwrap();

    // Search: / then letters filter the grid and jump to the first match; Esc clears it.
    key(&mut app, KeyCode::Char('/'));
    for c in "gear".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.book_entry, Entry::Item(Item::IronGear));
    assert_eq!(Entry::list(&app.book_query, false).len(), 3, "iron gear, gearbox, hardened gear");
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("search: gear_"), "{t}");
    key(&mut app, KeyCode::Enter);
    assert!(!app.book_typing && app.book);
    key(&mut app, KeyCode::Esc);
    assert!(app.book_query.is_empty() && app.book, "the first Esc clears the search");
    // A search that only matches machines moves to the machines tab.
    key(&mut app, KeyCode::Char('/'));
    for c in "press".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.book_entry, Entry::Part(Kind::Press));
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);

    // After some sales the part already sold turns grey and the marker moves along.
    app.book_go(Entry::Item(Item::CopperPlate));
    app.world.econ.research_sales[Item::CopperPlate.idx()] = 150;
    let buf = render(&mut app, 130, 42);
    assert!(text(&buf).contains("you: 150 sold"));
    std::fs::write("target/snapshot-book-chart.html", to_html(&buf)).unwrap();

    // n opens a waiting telegram over the book.
    app.mail = Some(crate::mail::parse("from: api\nevent: finished\nDone", 0));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("agent finished · n to view"), "{t}");
    key(&mut app, KeyCode::Char('n'));
    assert!(app.mail_open && app.book);
    key(&mut app, KeyCode::Char('x'));
    assert!(app.mail_open && app.mail.is_some(), "other keys don't skip a telegram");
    key(&mut app, KeyCode::Char('n'));
    assert!(!app.mail_open && app.mail.is_none() && app.book, "n closes the telegram and goes back to the book");

    // A small pane: fewer columns, and a scrollable page.
    let buf = render(&mut app, 64, 20);
    assert!(app.book_cols < 6);
    std::fs::write("target/snapshot-book-small.html", to_html(&buf)).unwrap();
    key(&mut app, KeyCode::Char('b'));
    assert!(!app.book);
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
    app.on_key(KeyEvent::from(KeyCode::Char('o')));
    assert!(app.stats);
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains(" Production ") && t.contains(" History ") && t.contains("last minute") && t.contains("research/min"), "{t}");
    assert!(t.contains("copper plate") && t.contains("next sale") && t.contains("Factory"), "beside the sidebar: {t}");
    std::fs::write("target/snapshot-production.html", to_html(&buf)).unwrap();
    // Small pane: fewer columns, still readable.
    let t = text(&render(&mut app, 60, 20));
    assert!(t.contains("copper plate") && !t.contains("next sale"), "{t}");
    // Any other key closes it and still does its job.
    app.on_key(KeyEvent::from(KeyCode::Char('t')));
    assert!(!app.stats && app.tree);
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
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("costs $0.50 to sell") && t.contains("Never earns research") && t.contains("Made by"), "{t}");
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
        last.contains(" Create › 1 DRIL+  2 FURN+") && last.contains("5 PUMP+") && !last.contains("$50"),
        "five slots all show their numbers: {last}"
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
        (Kind::Kiln, vec!["2 stone  ─coal─→  brick", "3 sand  ─coal─→  glass", "3x5", "fills up for 8 runs, then makes them all at once"]),
        (
            Kind::LargeKiln,
            vec!["2 stone  ─coal─→  brick", "3 sand  ─coal─→  glass", "5x7", "fills up for 32 runs, then makes them all at once"],
        ),
        (Kind::Mixer, vec!["2 sand + stone + water  ──→  concrete", "2 concrete + iron rod + water  ──→  reinforced concrete"]),
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
    app.tree_pick = tree_rows().len() - 1;
    let buf = render(&mut app, 120, 42);
    assert!(text(&buf).contains("Blueprints · "), "{}", text(&buf));
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
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    for name in ["Crusher", "Coke oven", "Wire drawer", "Winder", "Rolling mill", "Lathe", "Crude assembler"] {
        assert!(t.contains(name), "{name}: {t}");
    }
    std::fs::write("target/snapshot-new-machines-menu.html", to_html(&buf)).unwrap();
    // Narrow and tall: the map still reads, and the menu is one line.
    app.cursor = (x + 13, y + 1);
    app.cam = (x + 6, y - 3);
    let buf = render(&mut app, 44, 40);
    std::fs::write("target/snapshot-new-machines-narrow.html", to_html(&buf)).unwrap();
    let t = text(&render(&mut app, 60, 20));
    assert!(t.lines().last().unwrap().contains("Create › Lathe  $200  3x3  facing"), "{t}");
    app.selected = None;
    let buf = render(&mut app, 60, 20);
    assert!(text(&buf).lines().last().unwrap().contains("Create › 1 DRIL+  2 FURN+"), "{}", text(&buf));
    std::fs::write("target/snapshot-new-machines-small.html", to_html(&buf)).unwrap();
}

/// The new machines and items have book pages that link to each other.
#[test]
fn book_pages_for_the_new_machines_and_items() {
    let mut app = App::new(World::blank());
    app.book = true;
    app.book_go(Entry::Part(Kind::Lathe));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("About") && t.contains("iron plate  ──→  2 iron rod") && t.contains("iron rod  ──→  4 screw"), "{t}");
    assert!(t.contains("5.0s · 24/min") && t.contains("$200 · 3x3 · from Gearwright"), "{t}");
    std::fs::write("target/snapshot-book-lathe.html", to_html(&render(&mut app, 130, 42))).unwrap();
    std::fs::write("target/snapshot-book-lathe-narrow.html", to_html(&render(&mut app, 60, 40))).unwrap();
    app.book_go(Entry::Part(Kind::CokeOven));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("2 coal  ──→  coke + waste") && t.contains("no EC, no fuel"), "{t}");
    // An item page: where the screw comes from, and what a spring needs.
    app.book_go(Entry::Item(Item::Screw));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("Made by") && t.contains("Lathe") && t.contains("sells $2"), "{t}");
    app.book_go(Entry::Item(Item::Coal));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("2 coal  ──→  coke + waste"), "{t}");
    // Furnace and blast furnace melt iron differently: both show on the iron ore page.
    app.book_go(Entry::Item(Item::IronOre));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("3 iron ore  ─coal─→  2 molten iron") && t.contains("iron ore  ─coke─→  molten iron"), "{t}");
    app.book_go(Entry::Item(Item::Spring));
    std::fs::write("target/snapshot-book-spring.html", to_html(&render(&mut app, 130, 42))).unwrap();
    // The research list has the new machines, locked behind their key items.
    app.book = false;
    app.world = World::new();
    app.tree = true;
    app.tree_pick = tree_rows().iter().position(|t| *t == Tech::Crusher).unwrap();
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("Crusher · Create") && t.contains("Make copper wire once"), "{t}");
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
    // A tall sidebar lists every machine, five numbered groups.
    let buf = render(&mut app, 130, 46);
    let t = text(&buf);
    for name in ["Large basic drill", "Mixer", "Kiln", "Welder", "Large adv. assembler", "Advanced pump", "Precision assembler"] {
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
    app.selected = None;
    // Small panes: the bottom line keeps all five numbers.
    for width in [50, 64, 90] {
        let buf = render(&mut app, width, 20);
        let t = text(&buf);
        let last = t.lines().last().unwrap();
        assert!(last.contains(if width < 70 { "5 PUMP" } else { "5 Water" }), "{width}: {last}");
        std::fs::write(format!("target/snapshot-create-menu-{width}.html"), to_html(&buf)).unwrap();
    }
    app.menu = None;

    // The research list at ~50 columns: names whole, status short, scrolls to the end.
    app.world.progress = Progress::default();
    app.tree = true;
    app.tree_pick = tree_rows().len() - 1;
    let buf = render(&mut app, 50, 40);
    let t = text(&buf);
    assert!(t.contains("Excavation") && t.contains("Floors"), "{t}");
    app.tree_pick = tree_rows().iter().position(|x| *x == Tech::LargeAdvancedAssembler).unwrap();
    let buf = render(&mut app, 50, 40);
    let t = text(&buf);
    assert!(t.contains("Large adv. assembler") && t.contains("needs control board"), "{t}");
    std::fs::write("target/snapshot-tree-50.html", to_html(&buf)).unwrap();
    let buf = render(&mut app, 130, 42);
    std::fs::write("target/snapshot-tree-130.html", to_html(&buf)).unwrap();
    app.tree = false;

    // The recipe book: every item and machine listed, narrow too.
    app.book = true;
    app.book_go(Entry::Item(Item::ControlBoard));
    let buf = render(&mut app, 130, 42);
    let t = text(&buf);
    assert!(t.contains("Advanced assembler") && t.contains("Precision assembler") && t.contains("Made by"), "{t}");
    assert!(t.contains("reinforced concrete") && t.contains("hardened iron plate"), "long names show whole: {t}");
    std::fs::write("target/snapshot-book-items.html", to_html(&buf)).unwrap();
    let buf = render(&mut app, 50, 40);
    std::fs::write("target/snapshot-book-items-50.html", to_html(&buf)).unwrap();
    app.book_go(Entry::Item(Item::Gearbox));
    let buf = render(&mut app, 130, 42);
    assert!(text(&buf).contains("Crude assembler  10.0s"), "{}", text(&buf));
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
    assert!(t.contains("out the back") && t.contains("(BACK)"), "{t}");
    // A machine with no waste has no such line.
    app.book_go(Entry::Part(Kind::Furnace));
    let t = text(&render(&mut app, 130, 42));
    assert!(!t.contains("out the back"), "{t}");
    // The waste page says where it comes from.
    app.book_go(Entry::Item(Item::Waste));
    let t = text(&render(&mut app, 130, 42));
    assert!(t.contains("back edge"), "{t}");
    assert!(t.contains("fuel for boiler") && t.contains("4 waste burn like 1 coal"), "{t}");
    // And a placed boiler says it burns waste too.
    let mut w = World::blank();
    w.place(0, 20, 20, Kind::Boiler, Dir::E).unwrap();
    assert!(inspect(&w, 0, 20, 20).join("\n").contains("burns waste too: 4 = 1 coal"));
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
