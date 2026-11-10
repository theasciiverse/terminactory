//! Key bindings, loaded from controls.json so players can change them.
//! Number keys (build menu), Ctrl+S (save) and Ctrl+C (quit) are fixed.

use std::collections::BTreeMap;
use std::path::Path;

use ratatui::crossterm::event::KeyCode;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Place,
    Remove,
    Rotate,
    Setup,
    Back,
    Tree,
    Power,
    FloorUp,
    FloorDown,
    Pause,
    Faster,
    Slower,
    Help,
    Bigger,
    Smaller,
    Compact,
    Mail,
    Book,
    Stats,
}

impl Action {
    pub const ALL: [Action; 23] = [
        Action::MoveUp,
        Action::MoveDown,
        Action::MoveLeft,
        Action::MoveRight,
        Action::Place,
        Action::Remove,
        Action::Rotate,
        Action::Setup,
        Action::Back,
        Action::Tree,
        Action::Power,
        Action::FloorUp,
        Action::FloorDown,
        Action::Pause,
        Action::Faster,
        Action::Slower,
        Action::Help,
        Action::Bigger,
        Action::Smaller,
        Action::Compact,
        Action::Mail,
        Action::Book,
        Action::Stats,
    ];
    /// The name used in controls.json.
    pub fn id(self) -> &'static str {
        match self {
            Action::MoveUp => "move_up",
            Action::MoveDown => "move_down",
            Action::MoveLeft => "move_left",
            Action::MoveRight => "move_right",
            Action::Place => "place",
            Action::Remove => "remove",
            Action::Rotate => "rotate",
            Action::Setup => "set_up",
            Action::Back => "back",
            Action::Tree => "research_tree",
            Action::Power => "power_range",
            Action::FloorUp => "floor_up",
            Action::FloorDown => "floor_down",
            Action::Pause => "pause",
            Action::Faster => "faster",
            Action::Slower => "slower",
            Action::Help => "help",
            Action::Bigger => "elevator_bigger",
            Action::Smaller => "elevator_smaller",
            Action::Compact => "small_pane",
            Action::Mail => "telegram",
            Action::Book => "recipe_book",
            Action::Stats => "production",
        }
    }
    pub fn defaults(self) -> &'static [&'static str] {
        match self {
            Action::MoveUp => &["w", "Up"],
            Action::MoveDown => &["s", "Down"],
            Action::MoveLeft => &["a", "Left"],
            Action::MoveRight => &["d", "Right"],
            Action::Place => &["Space", "Enter"],
            Action::Remove => &["x", "Delete", "Backspace"],
            Action::Rotate => &["r"],
            Action::Setup => &["f"],
            Action::Back => &["`", "₹", "~", "Esc"],
            Action::Tree => &["t"],
            Action::Power => &["v"],
            Action::FloorUp => &[">", "."],
            Action::FloorDown => &["<", ","],
            Action::Pause => &["p"],
            Action::Faster => &["]"],
            Action::Slower => &["["],
            Action::Help => &["?", "h"],
            Action::Bigger => &["+", "="],
            Action::Smaller => &["-", "_"],
            Action::Compact => &["m"],
            Action::Mail => &["n"],
            Action::Book => &["b"],
            Action::Stats => &["o"],
        }
    }
}

/// Reads a key name: a single character, or Esc, Enter, Space, Tab, Backspace, Delete, Up, Down, Left, Right.
pub fn parse_key(s: &str) -> Option<KeyCode> {
    let named = match s.to_lowercase().as_str() {
        "esc" | "escape" => Some(KeyCode::Esc),
        "enter" | "return" => Some(KeyCode::Enter),
        "space" => Some(KeyCode::Char(' ')),
        "tab" => Some(KeyCode::Tab),
        "backspace" => Some(KeyCode::Backspace),
        "delete" | "del" => Some(KeyCode::Delete),
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),
        _ => None,
    };
    named.or_else(|| {
        let mut chars = s.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => Some(KeyCode::Char(c)),
            _ => None,
        }
    })
}

pub fn key_label(k: KeyCode) -> String {
    match k {
        KeyCode::Char(' ') => "Space".into(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Esc => "Esc".into(),
        KeyCode::Enter => "Enter".into(),
        KeyCode::Tab => "Tab".into(),
        KeyCode::Backspace => "Bksp".into(),
        KeyCode::Delete => "Del".into(),
        KeyCode::Up => "Up".into(),
        KeyCode::Down => "Down".into(),
        KeyCode::Left => "Left".into(),
        KeyCode::Right => "Right".into(),
        other => format!("{other:?}"),
    }
}

pub struct Controls {
    binds: Vec<(Action, Vec<KeyCode>)>,
}

impl Controls {
    pub fn defaults() -> Self {
        let binds = Action::ALL
            .iter()
            .map(|a| (*a, a.defaults().iter().filter_map(|s| parse_key(s)).collect()))
            .collect();
        Controls { binds }
    }

    fn defaults_json() -> String {
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        map.insert(
            "_how_to".into(),
            vec![
                "List the keys for each action. A key is one character, or Esc, Enter, Space, Tab, Backspace, Delete, Up, Down, Left, Right.".into(),
                "Holding Shift while moving lays belts. Number keys, Ctrl+S (save) and Ctrl+C (quit) can't be changed.".into(),
            ],
        );
        for a in Action::ALL {
            map.insert(a.id().into(), a.defaults().iter().map(|s| s.to_string()).collect());
        }
        serde_json::to_string_pretty(&map).unwrap_or_default()
    }

    /// Loads controls.json, creating it with the defaults if it doesn't exist.
    /// Returns a note for the player when something needs their attention.
    pub fn load_or_create(path: &Path) -> (Self, Option<String>) {
        let Ok(text) = std::fs::read_to_string(path) else {
            let note = match std::fs::write(path, Self::defaults_json()) {
                Ok(()) => None,
                Err(e) => Some(format!("Couldn't write controls.json ({e}); using default keys")),
            };
            return (Self::defaults(), note);
        };
        let map: BTreeMap<String, Vec<String>> = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => return (Self::defaults(), Some(format!("controls.json has a mistake ({e}); using default keys"))),
        };
        let mut c = Self::defaults();
        let mut problems = Vec::new();
        for (name, keys) in &map {
            // "quit" was retired: Ctrl+C quits (and saves).
            if name.starts_with('_') || name == "quit" {
                continue;
            }
            let Some(action) = Action::ALL.iter().find(|a| a.id() == name) else {
                problems.push(format!("unknown action \"{name}\""));
                continue;
            };
            let mut parsed = Vec::new();
            for k in keys {
                match parse_key(k) {
                    Some(code) => parsed.push(code),
                    None => problems.push(format!("unknown key \"{k}\"")),
                }
            }
            if let Some(slot) = c.binds.iter_mut().find(|(a, _)| a == action) {
                slot.1 = parsed;
            }
        }
        let note = (!problems.is_empty()).then(|| format!("controls.json: {}", problems.join(", ")));
        (c, note)
    }

    pub fn action(&self, code: KeyCode) -> Option<Action> {
        self.binds.iter().find(|(_, keys)| keys.contains(&code)).map(|(a, _)| *a)
    }

    /// The first couple of keys for an action, for on-screen hints.
    pub fn label(&self, a: Action) -> String {
        let keys = self.binds.iter().find(|(x, _)| *x == a).map(|(_, k)| k.as_slice()).unwrap_or(&[]);
        if keys.is_empty() {
            return "(unbound)".into();
        }
        keys.iter().take(2).map(|k| key_label(*k)).collect::<Vec<_>>().join("/")
    }

    /// Just the first key for an action, for short hints.
    pub fn key(&self, a: Action) -> String {
        self.label(a).split('/').next().unwrap_or_default().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rupee_and_backtick_go_back_by_default() {
        let c = Controls::defaults();
        assert_eq!(c.action(KeyCode::Char('₹')), Some(Action::Back));
        assert_eq!(c.action(KeyCode::Char('`')), Some(Action::Back));
        assert_eq!(c.action(KeyCode::Esc), Some(Action::Back));
    }

    #[test]
    fn controls_file_rebinds_keys() {
        let path = std::env::temp_dir().join(format!("factory-controls-{}.json", std::process::id()));
        std::fs::write(&path, r#"{ "back": ["b"], "research_tree": ["Tab"], "nonsense": ["z"] }"#).unwrap();
        let (c, note) = Controls::load_or_create(&path);
        std::fs::remove_file(&path).ok();
        assert_eq!(c.action(KeyCode::Char('b')), Some(Action::Back));
        assert_eq!(c.action(KeyCode::Char('`')), None);
        assert_eq!(c.action(KeyCode::Tab), Some(Action::Tree));
        assert_eq!(c.action(KeyCode::Char('x')), Some(Action::Remove)); // untouched actions keep defaults
        assert!(note.unwrap().contains("nonsense"));
    }

    #[test]
    fn missing_file_is_created_with_defaults() {
        let path = std::env::temp_dir().join(format!("factory-controls-new-{}.json", std::process::id()));
        std::fs::remove_file(&path).ok();
        let (_, note) = Controls::load_or_create(&path);
        assert!(note.is_none());
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert!(text.contains("\"back\"") && text.contains('₹'));
    }
}
