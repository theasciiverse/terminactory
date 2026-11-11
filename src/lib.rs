//! Terminactory: a cozy factory game in the terminal.
//! Both commands, `terminactory` and its shortcut `tfac`, start the game through `start`.

mod app;
mod content;
mod controls;
mod mail;
mod saves;
mod sim;
mod ui;

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::SetTitle;

/// Where saves and key settings live: ~/.terminactory, so the game finds them from any folder.
fn data_dir() -> PathBuf {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".terminactory")).unwrap_or_else(|| PathBuf::from("."))
}

/// Copies the key settings from the old place (the folder the game was run from) the first time.
fn move_old_files(dir: &Path) {
    let (old, new) = (Path::new("controls.json"), dir.join("controls.json"));
    if old.exists() && !new.exists() {
        let _ = std::fs::copy(old, &new);
    }
}

/// What `tfac --help` prints.
const HELP: &str = "\
Terminactory: a cozy factory game in the terminal. `tfac` is its shortcut.

Playing
  tfac              continue the save you played last
  tfac save2        open a save (or just: tfac 2)
  tfac --saves      list your saves: money, research, time played, last played
  tfac --new        start a fresh save as the next number (the others are kept)
  tfac --demo       start next to a small working factory (its own save)

Agent telegrams (they show up in the game as you play)
  tfac msg TEXT     send Markdown to the game; \\n is a line break
  tfac msg          read the Markdown from stdin
  tfac msg --from NAME TEXT   say who it's from (default: this folder's name)
  tfac hook         for Claude Code's Stop / Notification hooks (reads their JSON)

In the game, ? shows every key. Saves and controls.json live in ~/.terminactory.
";

/// Starts the game. `as_full_name` is true when launched as `terminactory`, so it can mention the shortcut.
pub fn start(as_full_name: bool) -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    // `tfac msg` and `tfac hook` post agent mail and return without starting the game.
    if mail::run_cli(&args, &data_dir()) {
        return Ok(());
    }
    if args.iter().skip(1).any(|a| matches!(a.as_str(), "--help" | "-h" | "help")) {
        print!("{HELP}");
        return Ok(());
    }
    if let Some(bad) = args.iter().skip(1).find(|a| a.starts_with('-') && !matches!(a.as_str(), "--demo" | "--new" | "--saves")) {
        eprintln!("Unknown option `{bad}`. `tfac --help` lists what tfac can do.");
        return Ok(());
    }
    let demo = args.iter().any(|a| a == "--demo");
    let fresh = args.iter().any(|a| a == "--new");
    let dir = data_dir();
    let _ = std::fs::create_dir_all(saves::dir(&dir));
    move_old_files(&dir);
    if args.iter().any(|a| a == "--saves") {
        print!("{}", saves::describe(&dir));
        return Ok(());
    }
    // Which save: one named on the command line ("save2" or "2"), a new one, or the last played.
    let named = args.iter().skip(1).find(|a| !a.starts_with('-'));
    let name = match named {
        Some(arg) => match saves::parse_name(arg) {
            Some(n) if saves::path_of(&dir, &n).exists() => n,
            Some(n) => {
                eprintln!("There's no save called {n}.\n\n{}", saves::describe(&dir));
                return Ok(());
            }
            None => {
                eprintln!("Unknown option `{arg}`. `tfac --help` lists what tfac can do.");
                return Ok(());
            }
        },
        None if fresh => saves::next_name(&dir),
        None => saves::latest(&dir).map_or_else(|| saves::next_name(&dir), |s| s.name),
    };
    // The demo keeps its own save so it never touches the numbered ones.
    let save_path = if demo { saves::dir(&dir).join("demo.json") } else { saves::path_of(&dir, &name) };

    // One window per save: a second one would overwrite the first when it quits.
    let _lock = match saves::lock(&save_path) {
        Ok(l) => l,
        Err(pid) => {
            let what = if demo { "The demo".to_string() } else { name.clone() };
            eprintln!(
                "{what} is already open in another window (process {pid}). Switch to that window, or open another save: tfac --saves"
            );
            return Ok(());
        }
    };
    // Saves from an older version of the game aren't opened (and are never deleted).
    if !demo && save_path.exists() {
        if let Some(msg) = saves::version_problem(&save_path, &name) {
            eprintln!("{msg}");
            return Ok(());
        }
    }
    let (world, mut note) = if demo {
        let mut w = sim::World::new();
        let note = match w.build_demo() {
            Ok(()) => "Demo factory built near you. Press ? for help, t for research.".to_string(),
            Err(e) => format!("Could not build the demo: {e}"),
        };
        (w, Some(note))
    } else if save_path.exists() {
        match sim::World::load(&save_path) {
            Ok(w) => (w, Some(format!("Welcome back to {name}. (tfac --saves lists your saves)"))),
            Err(e) => {
                // Keep the unreadable save instead of overwriting it on quit.
                let backup = save_path.with_extension(format!("unreadable-{}.json", std::process::id()));
                let _ = std::fs::copy(&save_path, &backup);
                (sim::World::new(), Some(format!("Couldn't read the save ({e}); kept a copy at {}, starting fresh", backup.display())))
            }
        }
    } else {
        let note = format!("Started {name}, a fresh game. Your other saves are safe: tfac --saves lists them.");
        (sim::World::new(), Some(note))
    };
    if as_full_name {
        let tip = "Tip: `tfac` is a shortcut for `terminactory`.";
        note = Some(match note {
            Some(n) => format!("{n}  {tip}"),
            None => format!("Welcome to Terminactory.  {tip}"),
        });
    }
    let mut app = app::App::new(world);
    if let Some(n) = note {
        app.say(n);
    }
    // Whatever the factory earned while the game was closed.
    if !demo {
        if let Some((away, m, r)) = app.world.apply_offline(sim::now_secs()) {
            let (h, min) = (away / 3600, (away % 3600) / 60);
            app.away = Some(format!("You were away for {h}h {min}m. Your factory kept going:\n\n  +${m:.0}\n  +{r:.1} research"));
        }
    }
    // Key bindings live in controls.json; it's created with the defaults if missing.
    let (controls, problem) = controls::Controls::load_or_create(&dir.join("controls.json"));
    app.controls = controls;
    if let Some(p) = problem {
        app.say(p);
    }

    let mut terminal = ratatui::init();
    let title = if demo { "Terminactory · demo".to_string() } else { format!("Terminactory · {name}") };
    execute!(io::stdout(), EnableMouseCapture, SetTitle(title))?;
    let inbox_from = if demo { None } else { Some(dir.clone()) };
    let result = run(&mut terminal, &mut app, &save_path, inbox_from.as_deref());
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    // Always save on the way out.
    if let Err(e) = app.world.save(&save_path) {
        eprintln!("could not save the game: {e}");
    }
    result
}

/// How often the game saves by itself.
const AUTOSAVE: Duration = Duration::from_secs(30);

/// Set when the terminal closes (SIGHUP) or the game is asked to stop (SIGTERM), so the
/// main loop ends normally and the game saves instead of dying mid-play.
fn stop_signal() -> std::sync::Arc<std::sync::atomic::AtomicBool> {
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    #[cfg(unix)]
    for sig in [signal_hook::consts::SIGHUP, signal_hook::consts::SIGTERM] {
        let _ = signal_hook::flag::register(sig, flag.clone());
    }
    flag
}

/// Fixed 60 ticks per second for the simulation, about 30 frames per second for drawing.
/// `inbox` is the data folder whose agent mail to pick up (none in the demo).
fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut app::App, save_path: &PathBuf, inbox: Option<&Path>) -> io::Result<()> {
    let tick = Duration::from_nanos(1_000_000_000 / sim::TPS);
    let frame = Duration::from_millis(33);
    let mut last = Instant::now();
    let mut acc = Duration::ZERO;
    let mut last_draw = Instant::now() - frame;
    let mut last_mail_check = Instant::now() - Duration::from_secs(5);
    let mut last_autosave = Instant::now();
    let stop = stop_signal();

    while !app.quit {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let now = Instant::now();
        acc += now - last;
        last = now;
        if acc > Duration::from_millis(250) {
            acc = tick; // don't try to catch up after a stall
        }
        // The game pauses while a letter, the tree or help is open.
        let overlay = app.letter.is_some() || app.tree || app.book || app.help || app.away.is_some() || app.mail_open;
        while acc >= tick {
            if !app.paused && !overlay {
                for _ in 0..app.speed {
                    app.world.tick();
                }
            }
            acc -= tick;
        }
        if app.letter.is_none() {
            app.letter = app.world.progress.events.pop_front();
        }
        // Agent mail: check the inbox about once a second; show one message at a time.
        if let Some(dir) = inbox {
            if last_mail_check.elapsed() >= Duration::from_secs(1) {
                last_mail_check = Instant::now();
                for m in mail::collect(dir, sim::now_secs()) {
                    app.world.receive(m, sim::now_secs());
                }
            }
        }
        // The next telegram shows (in the sidebar, or as a note in a small pane) once the last is dismissed.
        if app.mail.is_none() {
            if let Some(m) = app.world.mail.pop_front() {
                app.mail = Some(m);
                app.mail_scroll = 0;
            }
        }
        if let Some(n) = app.world.progress.notes.pop_front() {
            app.say(n);
        }
        // Autosave every couple of minutes, quietly, so a crash loses little.
        if last_autosave.elapsed() >= AUTOSAVE {
            last_autosave = Instant::now();
            if let Err(e) = app.world.save(save_path) {
                app.say(format!("Could not autosave: {e}"));
            }
        }
        if app.save_requested {
            app.save_requested = false;
            match app.world.save(save_path) {
                Ok(()) => app.say("Game saved"),
                Err(e) => app.say(format!("Could not save: {e}")),
            }
        }

        if last_draw.elapsed() >= frame {
            terminal.draw(|f| ui::draw(f, app))?;
            last_draw = Instant::now();
        }

        let wait = tick.saturating_sub(acc).min(frame.saturating_sub(last_draw.elapsed()));
        if event::poll(wait)? {
            loop {
                match event::read()? {
                    Event::Key(k) => app.on_key(k),
                    Event::Mouse(m) => app.on_mouse(m),
                    _ => {}
                }
                if app.quit || !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
    }
    Ok(())
}
